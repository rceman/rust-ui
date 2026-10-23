use super::*;
use std::rc::Rc;

impl<S, M, U, V> Backend<S, M, U, V>
where
    M: 'static,
    U: Fn(&mut S, M, &mut UpdateCtx<'_, M>),
    V: Fn(&S, &mut crate::Ui<'_, '_, M>),
{
    /// The ONE backend→peer native delivery funnel — see `DeferredNative`
    /// for the contract. Returns the message LRESULT for the synchronous
    /// path; a queued delivery returns 0 (deferred messages are
    /// fire-and-forget by contract — senders needing a result must be on
    /// the synchronous path).
    ///
    /// Errors: `QueueOverflow` when the deferred queue is full;
    /// `UiError::Platform` when a non-deferrable message hits a borrowed
    /// peer (contract violation — such messages may never sit in the
    /// queue).
    /// The ONE backend→peer native delivery funnel — see `DeferredNative`
    /// for the contract. `Ok(Some(send))` = executed synchronously with the
    /// real `TxSendMessage` outcome (HRESULT consumption + LRESULT);
    /// `Ok(None)` = queued (deferred sends are fire-and-forget by contract
    /// — callers needing a result are on the synchronous path and treat
    /// `None` as "availability unknown").
    ///
    /// Errors: `QueueOverflow` when the deferred queue is full;
    /// `UiError::Platform` when a non-deferrable message hits a borrowed
    /// peer (contract violation — such messages may never sit in the
    /// queue).
    pub(crate) fn deliver_native(
        &mut self,
        id: NodeId,
        msg: u32,
        wparam: usize,
        lparam: isize,
    ) -> UiResult<Option<text::NativeSend>> {
        // FIFO: a live delivery may never overtake a queued one — while
        // anything is pending, enqueue behind it
        if !self.deferred_native.is_empty() {
            return self.enqueue_native(id, msg, wparam, lparam);
        }
        // NOTE: no peer_for() — its generation filter borrows the cell and
        // would panic during a live borrow; the deferred delivery runs the
        // slot lookup then instead
        let peer = self
            .peer_ctx
            .registry
            .lock()
            .unwrap()
            .get(&id.slot)
            .and_then(|w: &std::rc::Weak<std::cell::RefCell<WindowlessPeer>>| w.upgrade());
        let Some(peer): Option<std::rc::Rc<std::cell::RefCell<WindowlessPeer>>> = peer else {
            return Ok(None); // fenced: dead slot — nothing to deliver to
        };
        match peer.as_ref().try_borrow() {
            Ok(p) if p.node() == id => {
                // msftedit runs host callbacks inside the send — SetFocus,
                // timers, invalidation all re-enter the WndProc
                // synchronously; in_turn keeps a nested turn() deferred
                // until this send unwinds (restore, not clear — the send
                // may itself run inside an outer turn)
                let was = self.in_turn.replace(true);
                let r = p.send(msg, wparam, lparam);
                self.in_turn.set(was);
                r.map(Some)
                    .map_err(|e| UiError::Platform(format!("peer send msg={msg:#x}: {e}")))
            }
            Ok(_) => Ok(None), // fenced: slot holds a different generation
            Err(_) => self.enqueue_native(id, msg, wparam, lparam),
        }
    }

    /// FIFO enqueue with progress + overflow semantics per the contract.
    pub(super) fn enqueue_native(
        &mut self,
        id: NodeId,
        msg: u32,
        wparam: usize,
        lparam: isize,
    ) -> UiResult<Option<text::NativeSend>> {
        if !deferrable(msg) {
            return Err(UiError::Platform(format!(
                "native msg {msg:#x} is not deferrable (pointer or sync result)"
            )));
        }
        if self.deferred_native.len() >= crate::event::EVENT_QUEUE_CAP {
            return Err(UiError::QueueOverflow);
        }
        self.deferred_native.push_back(DeferredNative::Message {
            node: id,
            msg,
            wparam,
            lparam,
        });
        // guaranteed progress: outside a turn nothing else will reach
        // service_peer_events — arm one pump (coalesced); a failed post
        // is a typed failure, not silent starvation
        if !self.in_turn.get() {
            self.wake_pump()?;
        }
        Ok(None)
    }

    /// Raw window message to the focused peer (IME, focus, wheel routing).
    /// Delivery failures (queue overflow, contract violation) propagate —
    /// callers must not silently drop them.
    /// Focus resolution: `Some(c)` arrival capture = replay — `c` governs
    /// (and `None` inside it means NO owner at arrival — do not fall
    /// through to whatever is focused now); `None` = live dispatch →
    /// current focus. Generation fencing still applies at `peer_for`.
    pub(super) fn focused_target(&self) -> Option<NodeId> {
        self.arrival_focus.get().unwrap_or(self.focus)
    }
    pub(crate) fn send_focused(&mut self, msg: u32, wparam: usize, lparam: isize) -> UiResult {
        if let Some(id) = self.focused_target() {
            self.deliver_native(id, msg, wparam, lparam)?;
        }
        Ok(())
    }

    /// The innermost KIND_SCROLL viewport containing `pos` (respecting
    /// its own inherited clip when nested). Wheel scroll targets the
    /// viewport under the point, not focus — depth-first `order` means
    /// a later (deeper/more nested) viewport wins in rev iteration.
    pub(super) fn scroll_viewport_at(&self, pos: Point) -> Option<NodeId> {
        let modal = self.modal_popup();
        for &id in self.order.iter().rev() {
            if let Some(m) = modal
                && !self.inside_subtree(m, id)
            {
                continue;
            }
            let Some(n) = self.rt.arena.get(id) else {
                continue;
            };
            if n.visibility != Visibility::Visible
                || !matches!(&n.data, NodeData::Container { kind, .. } if *kind == crate::node::KIND_SCROLL)
            {
                continue;
            }
            let Some(r) = self.rects.get(&id) else {
                continue;
            };
            if !r.contains(pos) {
                continue;
            }
            if let Some(c) = self.clips.get(&id)
                && !c.contains(pos)
            {
                continue;
            }
            return Some(id);
        }
        None
    }

    /// `WM_MOUSEWHEEL` — wheel lparams arrive in SCREEN px (unlike other
    /// mouse messages). Routing: an interactive editor peer UNDER the
    /// point consumes the wheel natively (msftedit internal scroll);
    /// otherwise the semantic scroll viewport under the point receives a
    /// `NodeEvent::Scroll` delta through its retained factories — the
    /// offset stays app-owned. Absent both, the legacy focused-peer
    /// route stands.
    pub(crate) fn wheel(&mut self, msg: u32, wparam: usize, lparam: isize) -> UiResult {
        let pos = space::screen_to_client(
            self.hwnd,
            space::ScreenPhysicalPoint(space::PhysicalPoint {
                x: lparam as i16 as i32,
                y: ((lparam >> 16) as i16) as i32,
            }),
        )
        .map(|c| {
            let s = self.peer_ctx.scale.get();
            c.0.logical(s)
        });
        if let Some(p) = pos
            && let Some(id) = self.hit_test(p)
            && self.peer_for(id).is_some()
        {
            self.deliver_native(id, msg, wparam, lparam)?;
            self.service_peer_events()?;
            return Ok(());
        }
        if let Some(p) = pos
            && let Some(id) = self.scroll_viewport_at(p)
        {
            // WHEEL_DELTA(120) per notch; ~3 lines × 16dp per notch —
            // the delta is already in logical Dp, no px conversion
            let delta = ((wparam >> 16) & 0xffff) as i16 as f32;
            let dy = -(delta / 120.0) * 48.0;
            self.push_input(id, NodeEvent::Scroll(ScrollOffset { x: 0.0, y: dy }))?;
            return self.turn();
        }
        self.send_focused(msg, wparam, lparam)?;
        self.service_peer_events()?;
        Ok(())
    }
}
