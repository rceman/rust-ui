use super::*;
use std::rc::Rc;

impl<S, M, U, V> Backend<S, M, U, V>
where
    M: 'static,
    U: Fn(&mut S, M, &mut UpdateCtx<'_, M>),
    V: Fn(&S, &mut crate::Ui<'_, '_, M>),
{
    pub(crate) fn ime_target(&self) -> Option<NodeId> {
        match self.ime_owner.get() {
            Some(owner) => owner,
            None => self.focused_target(),
        }
    }

    /// Native IME traffic goes to the composition OWNER — a focus change
    /// mid-composition must not redirect preedit/candidate handling or
    /// the ending commit to another peer.
    pub(crate) fn send_ime(&mut self, msg: u32, wparam: usize, lparam: isize) -> UiResult {
        if let Some(id) = self.ime_target() {
            self.deliver_native(id, msg, wparam, lparam)?;
        }
        Ok(())
    }

    /// Drain every live peer's host-event queue: caret/capture/timer/change
    /// notifications msftedit posted while a call ran.
    pub(crate) fn service_peer_events(&mut self) -> UiResult {
        // deferred native sends first — FIFO drain; the borrow that blocked
        // them is unwound by the time a turn calls this. Sends can re-queue
        // (a reentrant delivery while the peer is still borrowed), so the
        // pass is bounded at the queue cap — anything still pending drains
        // on the next service call.
        let mut budget = crate::event::EVENT_QUEUE_CAP;
        while budget > 0
            && let Some(item) = self.deferred_native.pop_front()
        {
            budget -= 1;
            let DeferredNative::Message {
                node,
                msg,
                wparam,
                lparam,
            } = item;
            let peer: Option<Rc<RefCell<WindowlessPeer>>> = self
                .peer_ctx
                .registry
                .lock()
                .unwrap()
                .get(&node.slot)
                .and_then(|w: &std::rc::Weak<std::cell::RefCell<WindowlessPeer>>| w.upgrade())
                .filter(|p: &std::rc::Rc<std::cell::RefCell<WindowlessPeer>>| {
                    p.borrow().node() == node
                });
            if let Some(peer) = peer {
                // generation-checked; the send's host callbacks may defer
                // further messages — they enqueue behind, drained next pass.
                // Delivery failure surfaces — a silently dropped message is
                // a lost input, not a transient skip.
                let was = self.in_turn.replace(true);
                let r = RefCell::borrow(&*peer).send(msg, wparam, lparam);
                self.in_turn.set(was);
                r?;
            }
        }
        // bounded drain left work — arm an explicit continuation instead
        // of depending on an unrelated later message; failure is typed
        if !self.deferred_native.is_empty() {
            self.wake_pump()?;
        }
        let slots: Vec<u32> = self
            .peer_ctx
            .registry
            .lock()
            .unwrap()
            .keys()
            .copied()
            .collect();
        for slot in slots {
            let Some(peer) = self
                .peer_ctx
                .registry
                .lock()
                .unwrap()
                .get(&slot)
                .and_then(|w: &std::rc::Weak<std::cell::RefCell<WindowlessPeer>>| w.upgrade())
            else {
                continue;
            };
            let events = RefCell::borrow(&*peer).drain()?;
            for ev in events {
                match ev {
                    HostEvent::Invalidate => {
                        self.counters
                            .requested_redraws
                            .fetch_add(1, Ordering::Relaxed);
                        self.invalidate_slot(slot);
                    }
                    HostEvent::Change => {
                        // committed native edit — read-back text, mint a
                        // revision, route the semantic event
                        self.native_commit(slot, &peer)?;
                    }
                    HostEvent::SelChange => {
                        self.native_selection(slot, &peer)?;
                    }
                    HostEvent::Capture(on) => {
                        if on {
                            unsafe {
                                windows::Win32::UI::Input::KeyboardAndMouse::SetCapture(self.hwnd);
                            }
                            self.native_capture = RefCell::borrow(&*peer).node_opt();
                        } else {
                            unsafe {
                                let _ =
                                    windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture();
                            }
                            self.native_capture = None;
                        }
                    }
                    HostEvent::Caret => {
                        self.counters
                            .native_caret_redraws
                            .fetch_add(1, Ordering::Relaxed);
                        self.apply_caret(&peer);
                    }
                    HostEvent::GrabFocus => {}
                }
            }
        }
        // armed-timer cleanup is the peer Drop's job — the pool is the
        // authority, there is no second store to prune
        Ok(())
    }

    fn invalidate_slot(&self, slot: u32) {
        if let Some(r) = self
            .rects
            .keys()
            .find(|id| id.slot == slot)
            .and_then(|id| self.rects.get(id))
        {
            let s = self.peer_ctx.scale.get();
            let mut p = r.physical(s);
            // invalidation must never under-cover — +1 keeps the bottom/
            // right edge inside the damage rect (conservative clip, not a
            // unit conversion)
            p.right += 1;
            p.bottom += 1;
            let rc: RECT = p.into();
            unsafe {
                let _ = windows::Win32::Graphics::Gdi::InvalidateRect(
                    Some(self.hwnd),
                    Some(&rc),
                    false,
                );
            }
        }
    }

    /// A committed native edit: read the peer's committed text, mint a
    /// checked revision, emit the semantic `Edit` event. An IME composition
    /// in progress forwards the FINAL commit before CompositionEnd — the
    /// transient preedit never reaches `on_edit`. The peer borrow NEVER
    /// spans an `rt.*` call — a consumer handler may touch its own
    /// `TextValue`/peer handle during the event.
    fn native_commit(
        &mut self,
        slot: u32,
        peer: &std::rc::Rc<std::cell::RefCell<WindowlessPeer>>,
    ) -> UiResult {
        let (text, anchor, focus, node, base, binding) = {
            let p = RefCell::borrow(&*peer);
            (
                p.text()?,
                p.selection_utf16()?.0,
                p.selection_utf16()?.1,
                p.node(),
                p.peer_rev(),
                p.binding(),
            )
        };
        // EN_CHANGE during an active composition means preedit leaked into
        // the committed channel — the runtime contract is committed-only,
        // so a preedit notification is suppressed, never routed to
        // TextValue or Submit.
        if self.rt.composition_active(node) {
            self.counters
                .preedit_commits_suppressed
                .fetch_add(1, Ordering::Relaxed);
            return Ok(());
        }
        let result = crate::text::TextRevision::mint();
        if let Some(binding) = binding {
            // UTF-8 positions of the selection at THIS commit — a failed
            // index conversion is a typed failure, never (0,0)/end-of-text
            let to_utf8 = |i: usize| {
                crate::text::utf16_index_to_utf8(&text, i).ok_or_else(|| {
                    UiError::Platform(format!("selection offset {i} not representable"))
                })
            };
            let sel = TextSelection {
                revision: result,
                anchor: to_utf8(anchor)?,
                focus: to_utf8(focus)?,
            };
            self.rt.edit_event(
                node,
                TextEdit {
                    text,
                    base,
                    result,
                    origin: EditOrigin::NativePeer,
                    binding,
                },
            )?;
            self.rt.selection_event(node, sel)?;
            RefCell::borrow(&*peer).record_commit(result);
        }
        let _ = slot;
        Ok(())
    }

    fn native_selection(
        &mut self,
        _slot: u32,
        peer: &std::rc::Rc<std::cell::RefCell<WindowlessPeer>>,
    ) -> UiResult {
        let (text, anchor, focus, rev, node) = {
            let p = RefCell::borrow(&*peer);
            let (a, f) = p.selection_utf16()?;
            (p.text()?, a, f, p.peer_rev(), p.node())
        };
        // a selection notification during composition reflects PREEDIT
        // coordinates — suppressed like commits; the committed snapshot
        // is the only state consumers may observe
        if self.rt.composition_active(node) {
            return Ok(());
        }
        // checked conversions — a failed UTF-16->UTF-8 index is a typed
        // failure, never a fallback to (0,0) or end-of-text
        let to_utf8 = |i: usize| {
            crate::text::utf16_index_to_utf8(&text, i)
                .ok_or_else(|| UiError::Platform(format!("selection offset {i} not representable")))
        };
        let sel = TextSelection {
            revision: rev,
            anchor: to_utf8(anchor)?,
            focus: to_utf8(focus)?,
        };
        self.rt.selection_event(node, sel)?;
        Ok(())
    }

    /// Peer-targeted coordinate authority — ONE effective geometry.
    /// While a composition-pending relatch is parked, `effective_geometry`
    /// still returns the old bounds/scale — pointer/caret math agrees
    /// with the surface msftedit actually renders, not the pending layout.
    pub(super) fn peer_origin(&self, id: NodeId) -> Option<space::PeerOrigin> {
        let peer: Rc<RefCell<WindowlessPeer>> = self.peer_for(id)?;
        let p = RefCell::borrow(&*peer);
        let (r, sc) = p.effective_geometry();
        space::PeerOrigin::from_logical(r, sc)
    }

    /// Keyboard/pointer focus moved — caret drawn only for the focus owner.
    fn apply_caret(&self, peer: &std::rc::Rc<std::cell::RefCell<WindowlessPeer>>) {
        let p = RefCell::borrow(&*peer);
        let Some(node) = p.node_opt() else { return };
        if self.focus != Some(node) {
            return;
        }
        let (created, shown, pos, size) = p.caret();
        if !created {
            return;
        }
        // the caret reports peer-local px — land it through THE effective
        // peer geometry (the snapped origin both pointer and host callbacks
        // share); during a pending relatch this is still the OLD space —
        // matching where msftedit actually draws the caret
        let Some(org) = self.peer_origin(node) else {
            return; // unrepresentable peer origin — drop the placement
        };
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::*;
            let Some(at) = org.to_client(space::PeerLocalPoint(pos.into())) else {
                return; // unrepresentable — drop the placement
            };
            // peer bounds are the FULL document rect now — a caret can
            // legitimately sit outside the node's scroll-viewport clip.
            // The system caret ignores D2D clips, so an out-of-band caret
            // would paint over the viewport's neighbors; hide it instead.
            if let Some(clip) = self.clips.get(&node) {
                let cpx = clip.physical(self.peer_ctx.scale.get());
                let inside = at.0.x >= cpx.left
                    && at.0.x + size.cx <= cpx.right
                    && at.0.y >= cpx.top
                    && at.0.y + size.cy <= cpx.bottom;
                if !inside {
                    let _ = HideCaret(Some(self.hwnd));
                    return;
                }
            }
            let _ = CreateCaret(self.hwnd, None, size.cx, size.cy);
            let _ = SetCaretPos(at.0.x, at.0.y);
            if shown {
                let _ = ShowCaret(Some(self.hwnd));
            }
        }
    }

    // ----- native timers ----------------------------------------------------
    //
    // One ID namespace: framework timers are 1 (deadline); peer timers live
    // at NATIVE_TIMER_BASE + n. The map keys on the WIN32 id and carries the
    // full NodeId (slot+generation), so a stale fire or a reused slot can
    // never deliver into a replacement peer. A repeated (node, richedit-id)
    // request reschedules the SAME win32 id rather than allocating a new
    // one; disarm kills every win32 id for that (node, nid).

    /// IME composition boundaries — composition gates conflict handling and
    /// pending-proposal resolution in the runtime.
    pub(crate) fn ime_start(&mut self) {
        // the REPLAY/captured owner starts composition — not whatever is
        // focused now; ownership is pinned until END arrives for it. An
        // explicitly absent captured owner stays ownerless FOREVER (Some
        // (None)) — delivery/end never fall back to a later-focused peer.
        let owner = self.focused_target();
        self.ime_owner.set(Some(owner));
        if let Some(id) = owner {
            self.rt.composition_start(id);
            if let Some(peer) = self.peer_for(id) {
                RefCell::borrow(&*peer).set_composing(true);
            }
        }
    }
    /// The composition-boundary flag falls BEFORE the final drain — a
    /// commit emitted by the end must route as committed text, not be
    /// discarded as residual preedit. Proposals resolve last, against the
    /// TRUE committed revision.
    pub(crate) fn ime_end(&mut self) -> UiResult {
        // the composition belongs to the peer STARTCOMPOSITION reached —
        // never to a node that gained focus mid-composition; an ownerless
        // start ends ownerless (no peer acquires the bookkeeping)
        let owner = match self.ime_owner.take() {
            Some(captured) => captured,
            None => self.focused_target(),
        };
        if let Some(id) = owner {
            if let Some(peer) = self.peer_for(id) {
                RefCell::borrow(&*peer).set_composing(false);
            }
            self.rt.composition_clear(id);
        }
        self.service_peer_events()?;
        if let Some(id) = owner {
            // ACKNOWLEDGE FIRST: a final committed EN_CHANGE drained while
            // composing was down may already be QUEUED — pump it into the
            // shared mirror BEFORE comparing, or reconcile would emit a
            // duplicate commit that splits native/shared revisions
            self.rt.pump()?;
            // RECONCILE only the remaining divergence — the ending commit
            // that arrived while `composing` was still up (suppressed as
            // preedit) never queued, so peer-vs-mirror divergence is the
            // ground truth; agreement = nothing to rebuild
            if let Some(peer) = self.peer_for(id) {
                self.reconcile_commit(&peer)?;
            }
            // acknowledge the reconciliation edit, THEN resolve proposals
            // against the true committed revision, then relatch
            self.rt.pump()?;
            self.rt.composition_end(id)?;
            if let Some(peer) = self.peer_for(id) {
                RefCell::borrow(&*peer).finish_pending_relatch()?;
            }
        }
        self.turn()
    }

    /// Post-composition truth reconcile: if the peer's text differs from
    /// the runtime's committed mirror, the final commit was suppressed as
    /// preedit during composition — emit it now, before proposal verdicts.
    /// No-op when peer and mirror already agree (IME cancelled, or the
    /// commit already routed normally).
    fn reconcile_commit(
        &mut self,
        peer: &std::rc::Rc<std::cell::RefCell<WindowlessPeer>>,
    ) -> UiResult {
        let (text, node, base, binding) = {
            let p = RefCell::borrow(&*peer);
            (p.text()?, p.node(), p.peer_rev(), p.binding())
        };
        let Some(binding) = binding else {
            return Ok(());
        };
        let Some((committed, mirror_base)) = self.rt.committed_editor(node) else {
            return Ok(());
        };
        if text == committed {
            return Ok(()); // already in agreement — nothing to commit
        }
        // the reconcile base must be the mirror's peer_revision — the
        // sequence of real commits, not the peer's possibly-advanced counter
        let result = crate::text::TextRevision::mint();
        let _ = base;
        self.rt.edit_event(
            node,
            TextEdit {
                text,
                base: mirror_base,
                result,
                origin: EditOrigin::NativePeer,
                binding,
            },
        )?;
        RefCell::borrow(&*peer).record_commit(result);
        self.counters
            .ime_reconciled_commits
            .fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}
