use super::*;
use std::rc::Rc;

impl<S, M, U, V> Backend<S, M, U, V>
where
    M: 'static,
    U: Fn(&mut S, M, &mut UpdateCtx<'_, M>),
    V: Fn(&S, &mut crate::Ui<'_, '_, M>),
{
    pub(crate) fn wake_pump(&self) -> UiResult {
        if self.hwnd.0.is_null() || self.closed.load(Ordering::SeqCst) || self.pump_queued.get() {
            return Ok(()); // nothing to wake, or already armed
        }
        self.pump_queued.set(true);
        let posted =
            unsafe { PostMessageW(Some(self.hwnd), WM_PUMP, WPARAM(0), LPARAM(0)).is_ok() };
        if !posted {
            self.pump_queued.set(false);
            return Err(UiError::Platform("PostMessageW(WM_PUMP) failed".into()));
        }
        Ok(())
    }

    /// One semantic turn: drain the peer sink into the runtime, pump
    /// (bounded), drain the scheduler, apply dirty layout/paint, present if
    /// needed. WndProc + WM_PUMP both funnel here.
    ///
    /// Reentrancy: `OnTxUIActivate` inside `apply_bounds`/`draw` can call
    /// `TxSetFocus` → synchronous `WM_SETFOCUS` → `turn()`. A nested turn
    /// would run view()/layout while the outer one is mid-layout, so a
    /// reentrant call instead posts one coalesced `WM_PUMP`; the deferred
    /// turn runs the body after the outer frame unwinds — the transition
    /// is deferred, never dropped.
    pub(crate) fn turn(&mut self) -> UiResult {
        if self.in_turn.get() {
            // a failed wake surfaces — never "scheduled" silently
            self.wake_pump()?;
            return Ok(());
        }
        self.in_turn.set(true);
        let r = self.turn_body();
        self.in_turn.set(false);
        r
    }

    pub(super) fn turn_body(&mut self) -> UiResult {
        // a pump post may still be in flight after this turn settles — it
        // clears the flag so a later reentrant call re-arms the post
        self.pump_queued.set(false);
        // safe-point: a cross-thread mailbox wake whose signal failed is a
        // broken progress guarantee — surface typed, same as wake_pump
        if self.rt.mailbox.take_wake_failure() {
            return Err(UiError::Platform("mailbox wake signal failed".into()));
        }
        // peer-emitted semantic events (ack edits, selections)
        let items: Vec<NativeSinkItem> = std::mem::take(&mut *self.peer_ctx.sink.lock().unwrap());
        for it in items {
            match it {
                NativeSinkItem::Edit { node, edit } => {
                    self.rt.edit_event(node, edit)?;
                }
                NativeSinkItem::Selection { node, sel } => {
                    self.rt.selection_event(node, sel)?;
                }
            }
        }
        let updated = self.rt.pump()?;
        // peer-creation spec stays current — new peers mount with the
        // live (appearance, theme)
        *self.peer_ctx.colors.borrow_mut() = (self.rt.appearance(), self.rt.theme.clone());
        // EFFECTIVE palette refresh — compare the RESOLVED color output
        // peers would apply, never the descriptors: a GetSysColor change
        // under identical (appearance, theme) is real palette drift, and
        // pre-seeding an applied-cache with descriptors would hide it.
        {
            let appearance = self.rt.appearance();
            let dark = self.rt.theme.dark;
            // V01 — keep the native non-client caption in sync with the
            // resolved light/dark appearance (no window recreation)
            self.sync_dark_caption(dark);
            let forced = appearance.forced_colors;
            let sys = self.peer_ctx.sys_resolver.get();
            let (_fg, sel_bg, sel_fg) = palette(&self.rt.theme, &appearance, &sys);
            let resolver = forced.then_some(&sys as &dyn Fn(crate::style::SystemColor) -> [f32; 4]);
            // per-peer effective tuple — keyed by full NodeId so a same-
            // slot recreate compares fresh
            let peers: Vec<(u32, std::rc::Rc<std::cell::RefCell<WindowlessPeer>>)> = self
                .peer_ctx
                .registry
                .lock()
                .unwrap()
                .iter()
                .filter_map(
                    |(slot, w): (&u32, &std::rc::Weak<std::cell::RefCell<WindowlessPeer>>)| {
                        w.upgrade().map(|p| (*slot, p))
                    },
                )
                .collect();
            let current: Vec<(NodeId, ([f32; 4], [f32; 4], [f32; 4]))> = peers
                .iter()
                .map(|(_, p)| {
                    let p = RefCell::borrow(&*p);
                    (p.node(), p.effective_colors(sel_bg, sel_fg, dark, resolver))
                })
                .collect();
            let mut applied = self.applied_palette.borrow_mut();
            if applied.as_ref() != Some(&current) {
                for (_, p) in &peers {
                    RefCell::borrow(&*p).set_colors(sel_bg, sel_fg, dark, resolver);
                }
                *applied = Some(current);
            }
        }
        // scheduler due work: frames -> events, chrome -> render side
        let chrome = self.rt.pump_sched(std::time::Instant::now())?;
        for ev in &chrome {
            self.apply_chrome(ev);
        }
        // dirty classification -> layout / paint work
        let dirty = self.rt.drain_dirty();
        // popup lifecycle bookkeeping: a NEW popup records the pre-open
        // focus and optionally focuses its first eligible descendant; a
        // VANISHED popup restores its saved focus BEFORE sanitize_focus
        // revalidates the focus owner
        {
            let live: HashMap<u32, (NodeId, bool)> = (0..self.rt.arena.slot_len() as u32)
                .filter_map(|slot| {
                    let n = self.rt.arena.slot(slot)?;
                    if !matches!(&n.data, NodeData::Popup { .. }) {
                        return None;
                    }
                    let f = matches!(&n.data, NodeData::Popup { focus_on_open: f, .. } if *f);
                    Some((
                        slot,
                        (
                            NodeId {
                                slot,
                                generation: self.rt.arena.generation_of(slot),
                            },
                            f,
                        ),
                    ))
                })
                .collect();
            let replaced: Vec<u32> = live
                .iter()
                .filter_map(|(slot, (id, _))| {
                    self.open_popups
                        .get(slot)
                        .is_some_and(|book| book.id != *id)
                        .then_some(*slot)
                })
                .collect();
            // a slot REUSED by a new popup generation in one commit
            // inherits the displaced book's focus-return — the pre-open
            // owner belongs to the opener, not the slot
            let mut carried: HashMap<u32, Option<NodeId>> = HashMap::new();
            for slot in replaced {
                if let Some(book) = self.open_popups.remove(&slot) {
                    carried.insert(slot, book.focus_return);
                }
            }
            // VANISHED popups restore BEFORE new books are written — a
            // book created in the same commit must record the
            // post-restore owner, not a focus about to die with the old
            // popup. Restore runs newest-layer-first — a nested stack
            // unwinds to the pre-stack owner; sibling closes land the
            // newest vanished book with a live return target.
            let mut vanished: Vec<(u64, u32)> = self
                .open_popups
                .iter()
                .filter(|(s, _)| !live.contains_key(s))
                .map(|(s, book)| (book.open_seq, *s))
                .collect();
            vanished.sort_by(|a, b| b.0.cmp(&a.0));
            for (_, slot) in vanished {
                let Some(book) = self.open_popups.remove(&slot) else {
                    continue;
                };
                // restore only when focus died with the popup or sits
                // inside the vanished subtree — a live outside focus wins
                let inside_dead = self
                    .focus
                    .is_none_or(|f| !self.rt.arena.is_live(f) || self.inside_subtree(book.id, f));
                if inside_dead {
                    let ret = book.focus_return.filter(|f| {
                        self.rt
                            .arena
                            .get(*f)
                            .is_some_and(|n| n.interactive() && n.visibility == Visibility::Visible)
                    });
                    self.set_focus(ret)?;
                }
            }
            // insert in sorted slot order — same-commit multi-open seq
            // assignment is commit-deterministic
            let mut fresh: Vec<(u32, NodeId, bool)> =
                live.iter().map(|(s, (id, f))| (*s, *id, *f)).collect();
            fresh.sort_by_key(|(s, _, _)| *s);
            for (slot, id, focus_on_open) in fresh {
                if let std::collections::hash_map::Entry::Vacant(e) = self.open_popups.entry(slot) {
                    e.insert(PopupBook {
                        id,
                        focus_return: carried.remove(&slot).flatten().or(self.focus),
                        focus_pending: focus_on_open,
                        open_seq: self.popup_seq,
                    });
                    self.popup_seq += 1;
                }
            }
        }
        // commit boundary: a removed/disabled focus or capture owner must
        // be resolved BEFORE layout/paint use stale state
        self.sanitize_focus()?;
        let mut needs_layout = !self.rects.is_empty() && self.order.is_empty();
        let mut needs_paint = !chrome.is_empty();
        let mut needs_uia = false;
        for (id, d) in dirty {
            if d & crate::runtime::Runtime::<S, M, U, V>::DIRTY_REMOVED != 0
                || !self
                    .rt
                    .arena
                    .get(id)
                    .is_some_and(|n| n.visibility == Visibility::Visible)
            {
                // gone or hidden — the transition clock and the shown
                // cache drop with it (a re-show snaps, never resumes)
                self.prune_transition(id);
            } else if d & crate::runtime::Runtime::<S, M, U, V>::DIRTY_VALUE != 0 {
                // animatable value flip (checked / selected / disabled /
                // invalid) — eligible controls ease; others snap
                self.begin_transition(id);
            }
            if d & crate::runtime::Runtime::<S, M, U, V>::DIRTY_LAYOUT != 0 {
                needs_layout = true;
            }
            if d & crate::runtime::Runtime::<S, M, U, V>::DIRTY_SEMANTICS != 0 {
                needs_uia = true;
            }
            if d & crate::runtime::Runtime::<S, M, U, V>::DIRTY_PAINT != 0 {
                needs_paint = true;
                // ink-damage union — the node's committed rect plus any
                // shadow footprint; cumulative between paints. (The window
                // still repaints fully today, but the union is the promised
                // damage contract and is tested.)
                if let Some(r) = self.rects.get(&id).copied() {
                    // OLD committed ink (where the node last painted —
                    // covers a moved/shrunk/removed shadow) UNION new ink
                    let mut dr = self
                        .committed_ink
                        .borrow()
                        .get(&id)
                        .copied()
                        .map(|old| old.union(r))
                        .unwrap_or(r);
                    let sh = match self.rt.arena.get(id).map(|n| &n.data) {
                        Some(crate::node::NodeData::Container { kind, props }) => props
                            .resolved_box(*kind)
                            .and_then(|b| b.shadow.map(|s| (b, s))),
                        Some(crate::node::NodeData::Popup { props, .. }) => props
                            .resolved_box(crate::node::KIND_POPUP)
                            .and_then(|b| b.shadow.map(|s| (b, s))),
                        Some(crate::node::NodeData::Action {
                            style, disabled, ..
                        }) => {
                            let b = style.resolve(*disabled, false, false, false);
                            b.shadow.map(|s| (b, s))
                        }
                        Some(crate::node::NodeData::Editor { patch, .. }) => {
                            let c = layout::editor_chrome(patch);
                            c.shadow.map(|s| (c, s))
                        }
                        _ => None,
                    };
                    if let Some((_b, s)) = sh {
                        let ink = super::render::shadow_ink_rect(&r, &s);
                        dr = dr.union(ink);
                    }
                    let acc = self.damage.get().map(|d| d.union(dr)).unwrap_or(dr);
                    self.damage.set(Some(acc));
                }
            }
        }
        if self.rects.is_empty() {
            needs_layout = true;
        }
        // built-in chrome state transitions feed the SAME classification —
        // a hover/pressed change that alters metric-bearing fields must
        // reach layout; a color-only transition only repaints
        {
            let d = self.state_dirty.replace(0);
            if d & 0b01 != 0 {
                needs_layout = true;
            }
            if d & 0b10 != 0 {
                needs_paint = true;
            }
        }
        // layout is driven by the CLASSIFIED work, not by "any message
        // arrived" — an update whose resolved output is unchanged does no
        // layout and no paint
        if needs_layout {
            self.relayout()?;
            needs_paint = true;
            needs_uia = true;
        }
        // pending focus resolves in PAINT order — the topmost (last)
        // popup deterministically wins when several open in one turn
        let pending_focus: Vec<NodeId> = self
            .order
            .iter()
            .copied()
            .filter(|id| {
                self.open_popups
                    .get(&id.slot)
                    .is_some_and(|book| book.id == *id && book.focus_pending)
            })
            .collect();
        for popup in pending_focus {
            if let Some(first) = self.first_focusable(popup) {
                self.set_focus(Some(first))?;
            }
            if let Some(book) = self.open_popups.get_mut(&popup.slot) {
                book.focus_pending = false;
            }
        }
        if !self.state_dirty.get().eq(&0) {
            let state = self.state_dirty.replace(0);
            if state & 0b01 != 0 {
                self.relayout()?;
            }
            if state & 0b11 != 0 {
                needs_paint = true;
            }
            needs_uia = true;
        }
        // live UIA state follows the committed tree — external clients see
        // name/enabled/bounds/order changes without a fresh WM_GETOBJECT.
        // `updated` still counts: focus/press semantics ride app messages
        // that need not flip a dirty bit.
        if needs_uia || updated || needs_paint {
            self.uia_refresh()?;
        }
        if needs_paint {
            self.paint()?;
        }
        self.service_peer_events()?;
        self.rearm_deadline()?;
        Ok(())
    }

    /// Chrome events from the scheduler — tooltip show, transition steps —
    /// become render-side state (the runtime emits no frames for these).
    pub(super) fn apply_chrome(&mut self, ev: &SchedEvent) {
        match ev {
            SchedEvent::TooltipShow { node } => {
                self.show_tooltip(*node);
            }
            SchedEvent::TransitionStep { node, .. } => {
                // eased chrome step — the turn already repaints on any
                // chrome event; the deadline timer re-arms from the
                // scheduler's next_deadline
                self.counters
                    .animation_callbacks
                    .fetch_add(1, Ordering::Relaxed);
                let _ = node;
            }
            SchedEvent::TransitionDone { node, .. } => {
                // settle — the interpolated entry drops; this turn's
                // repaint resolves the instant target
                self.counters
                    .animation_callbacks
                    .fetch_add(1, Ordering::Relaxed);
                self.end_transition(*node);
            }
            _ => {}
        }
    }
}
