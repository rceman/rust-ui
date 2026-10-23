use super::*;
use std::rc::Rc;

impl<S, M, U, V> Backend<S, M, U, V>
where
    M: 'static,
    U: Fn(&mut S, M, &mut UpdateCtx<'_, M>),
    V: Fn(&S, &mut crate::Ui<'_, '_, M>),
{
    /// A node nested inside a semantic `Action` is decorative content —
    /// never the hit target; the enclosing Action owns press/focus.
    /// Parents are slot-indexed — walk slots, generation is irrelevant to
    /// ancestry (a dead parent can't have live children).
    pub(super) fn inside_action(&self, id: NodeId) -> bool {
        let mut cur = self.rt.arena.get(id).and_then(|n| n.parent);
        while let Some(slot) = cur {
            match self.rt.arena.slot(slot) {
                Some(pn) => {
                    if matches!(pn.data, NodeData::Action { .. }) {
                        return true;
                    }
                    cur = pn.parent;
                }
                None => return false,
            }
        }
        false
    }

    /// Is `id` inside `root`'s subtree (inclusive of root)? Ancestor
    /// slot-walk — same indexing contract as `inside_action`.
    pub(super) fn inside_subtree(&self, root: NodeId, id: NodeId) -> bool {
        if id == root {
            return true;
        }
        let mut cur = self.rt.arena.get(id).and_then(|n| n.parent);
        while let Some(slot) = cur {
            match self.rt.arena.slot(slot) {
                Some(pn) => {
                    if slot == root.slot {
                        return true;
                    }
                    cur = pn.parent;
                }
                None => return false,
            }
        }
        false
    }

    /// The topmost live MODAL popup (last in paint order) — the hit,
    /// focus-step and scroll gates all read this one authority.
    pub(super) fn modal_popup(&self) -> Option<NodeId> {
        self.order.iter().rev().copied().find(|&id| {
            self.rt
                .arena
                .get(id)
                .is_some_and(|n| matches!(&n.data, NodeData::Popup { modal: true, .. }))
        })
    }

    pub(super) fn topmost_popup(&self) -> Option<NodeId> {
        self.order
            .iter()
            .rev()
            .copied()
            .find(|id| {
                self.rt
                    .arena
                    .get(*id)
                    .is_some_and(|n| matches!(n.data, NodeData::Popup { .. }))
            })
            .or_else(|| {
                self.open_popups
                    .values()
                    .max_by_key(|book| book.open_seq)
                    .map(|book| book.id)
            })
    }

    /// Hit-test a DIP point: topmost visible interactive node wins; nodes
    /// inside an Action defer to it (Action owns the semantic target).
    /// A live modal popup gates the result — a hit outside its subtree
    /// resolves to the popup backdrop itself (the press lands, the
    /// underlying UI never sees it).
    pub(crate) fn hit_test(&self, p: Point) -> Option<NodeId> {
        let hit = self.hit_test_inner(p);
        // modal gate: a live modal popup owns the whole client — a press
        // outside its subtree resolves to the popup backdrop
        if let Some(m) = self.modal_popup()
            && !hit.is_some_and(|h| self.inside_subtree(m, h))
        {
            return Some(m);
        }
        hit
    }

    /// The ungated topmost-hit walk — hit_test's body before the modal
    /// gate consumes it (kept private; callers want the gated result).
    pub(super) fn hit_test_inner(&self, p: Point) -> Option<NodeId> {
        for &id in self.order.iter().rev() {
            let Some(n) = self.rt.arena.get(id) else {
                continue;
            };
            if n.visibility != Visibility::Visible || self.inside_action(id) {
                continue;
            }
            let Some(r) = self.rects.get(&id) else {
                continue;
            };
            if r.contains(p) {
                if let Some(c) = self.clips.get(&id)
                    && !c.contains(p)
                {
                    continue;
                }
                match &n.data {
                    NodeData::Button { .. }
                    | NodeData::Editor { .. }
                    | NodeData::Action { .. }
                    | NodeData::Toggle { .. }
                        if n.interactive() =>
                    {
                        return Some(id);
                    }
                    // a painted popup captures hits within its rect
                    // unconditionally — paint-last implies hit-first;
                    // `dismiss_light`/`modal` govern dismissal, not
                    // whether its own surface is opaque to input
                    NodeData::Popup { .. } => {
                        return Some(id);
                    }
                    NodeData::Custom { render, .. } if n.interactive() => {
                        let local = Point {
                            x: p.x - r.x,
                            y: p.y - r.y,
                        };
                        if render.hit_test(local, *r) {
                            return Some(id);
                        }
                    }
                    _ => {}
                }
            }
        }
        None
    }

    /// Route a pointer event — editors get the raw Win32 message (the
    /// caller's wparam carries MK_* modifier/button flags, preserved
    /// end-to-end); semantic nodes get `NodeEvent` phases.
    pub(crate) fn pointer(
        &mut self,
        phase: crate::node::PointerPhase,
        pos_px: space::ClientPhysicalPoint,
        button: Option<PointerButton>,
        wparam: usize,
    ) -> UiResult {
        let pos: Point = {
            let s = self.peer_ctx.scale.get();
            pos_px.0.logical(s)
        };
        let hit = if self.native_capture.is_some() {
            self.native_capture
        } else {
            self.hit_test(pos)
        };
        // outside-press dismissal (Radix light-dismiss): a Down outside a
        // dismiss_light popup's subtree emits OpenChange(false) BEFORE the
        // press routes — the press still lands on its real target
        if phase == crate::node::PointerPhase::Down {
            // light-dismiss one LAYER per press — the topmost (last in
            // paint order) dismiss_light popup whose subtree misses the
            // hit. Deeper layers dismiss on subsequent presses, matching
            // Escape's topmost-only semantics.
            let dismiss: Option<NodeId> = self.order.iter().rev().copied().find(|&id| {
                self.open_popups.values().any(|book| book.id == id)
                    && self.rt.arena.get(id).is_some_and(|n| {
                        matches!(
                            &n.data,
                            NodeData::Popup {
                                dismiss_light: true,
                                ..
                            }
                        )
                    })
                    && !hit.is_some_and(|hit_id| self.inside_subtree(id, hit_id))
            });
            if let Some(id) = dismiss {
                self.push_input(id, NodeEvent::OpenChange(false))?;
            }
        }
        self.mouse = pos;
        match phase {
            crate::node::PointerPhase::Enter => {}
            crate::node::PointerPhase::Leave => {}
            _ => {}
        }
        // hot tracking for semantic nodes
        if hit != self.hot {
            if let Some(old) = self.hot {
                self.push_input(
                    old,
                    NodeEvent::Pointer(
                        PointerEvent {
                            position: pos,
                            button,
                            modifiers: modifiers_now(),
                        },
                        crate::node::PointerPhase::Leave,
                    ),
                )?;
            }
            if let Some(id) = hit
                && !self.peer_for(id).is_some()
            {
                self.push_input(
                    id,
                    NodeEvent::Pointer(
                        PointerEvent {
                            position: pos,
                            button,
                            modifiers: modifiers_now(),
                        },
                        crate::node::PointerPhase::Enter,
                    ),
                )?;
            }
            self.hot = hit;
        }
        if let Some(id) = hit {
            // native peer? forward the raw message + focus on down
            if self.peer_for(id).is_some() {
                // peer-local = content-local — THE shared transform:
                // snapped physical origin, integer subtraction only
                // (fractional-origin re-rounding disagreement resolved by
                // geom::PeerOrigin — Gate 6/20)
                let Some(origin) = self.peer_origin(id) else {
                    return Ok(()); // unrepresentable peer origin — drop
                };
                let Some(lp_px) = origin.to_local(pos_px).map(|l| l.0) else {
                    return Ok(()); // unrepresentable coordinate — drop
                };
                let msg = match (phase, button) {
                    (crate::node::PointerPhase::Down, Some(PointerButton::Primary)) => {
                        self.set_focus(Some(id))?;
                        WM_LBUTTONDOWN
                    }
                    (crate::node::PointerPhase::Up, Some(PointerButton::Primary)) => WM_LBUTTONUP,
                    _ => WM_MOUSEMOVE,
                };
                // out-of-MAKELPARAM-range px coordinates cannot reach the
                // peer — a contract failure, surfaced not truncated
                let Some(lp) = space::try_lparam_px(lp_px) else {
                    return Err(crate::UiError::Platform(
                        "peer-local coordinate outside LPARAM range".into(),
                    ));
                };
                // wparam is the caller's MK_* flags — preserved metadata
                self.deliver_native(id, msg, wparam, lp)?;
                self.service_peer_events()?;
                return self.turn();
            }
            if phase == crate::node::PointerPhase::Down
                && button == Some(PointerButton::Primary)
                && !matches!(
                    self.rt.arena.get(id).map(|n| &n.data),
                    Some(NodeData::Popup { .. })
                )
            {
                self.set_focus(Some(id))?;
                // KILLFOCUS on a peer old-owner queues host events —
                // drain before this boundary's turn, same as the peer
                // branch above
                self.service_peer_events()?;
            }
            // semantic node events
            let ev = match phase {
                crate::node::PointerPhase::Down => {
                    let old = self.interact(id);
                    self.pressed = Some(id);
                    self.mark_state_dirty(id, old);
                    NodeEvent::Pointer(
                        PointerEvent {
                            position: pos,
                            button,
                            modifiers: modifiers_now(),
                        },
                        crate::node::PointerPhase::Down,
                    )
                }
                crate::node::PointerPhase::Up => {
                    let was_pressed = self.pressed == Some(id);
                    let old = self.interact(id);
                    self.pressed = None;
                    if was_pressed {
                        self.mark_state_dirty(id, old);
                    }
                    // press = down+up on the same node
                    if was_pressed {
                        self.push_input(id, NodeEvent::Press)?;
                        return self.turn();
                    }
                    NodeEvent::Pointer(
                        PointerEvent {
                            position: pos,
                            button,
                            modifiers: modifiers_now(),
                        },
                        crate::node::PointerPhase::Up,
                    )
                }
                _ => NodeEvent::Pointer(
                    PointerEvent {
                        position: pos,
                        button,
                        modifiers: Modifiers::default(),
                    },
                    phase,
                ),
            };
            self.push_input(id, ev)?;
        }
        self.turn()
    }

    pub(super) fn push_input(&mut self, node: NodeId, ev: NodeEvent) -> UiResult {
        self.rt
            .events
            .push(QueuedEvent { node, payload: ev })
            .map_err(|_| UiError::QueueOverflow)
    }

    /// Registry lookup that also proves generation identity — a slot reuse
    /// never routes input to a stale peer's replacement.
    /// A node's resolved editor chrome (recipe + authored patch) — the
    /// transform contract's style input.
    pub(crate) fn editor_chrome_of(&self, node: NodeId) -> crate::style::BoxStyle {
        match self.rt.arena.get(node).map(|n| &n.data) {
            Some(crate::node::NodeData::Editor { patch, .. }) => layout::editor_chrome(patch),
            _ => crate::style::text_input_chrome_recipe(),
        }
    }

    pub(crate) fn peer_for(&self, node: NodeId) -> Option<Rc<RefCell<WindowlessPeer>>> {
        self.peer_ctx
            .registry
            .lock()
            .unwrap()
            .get(&node.slot)
            .and_then(|w: &RcWeak<RefCell<WindowlessPeer>>| w.upgrade())
            .filter(|p: &Rc<RefCell<WindowlessPeer>>| p.borrow().node() == node)
    }

    /// IDC_IBEAM inside an eligible editor, IDC_ARROW elsewhere (the class
    /// cursor stays arrow so nothing inherits a busy cursor).
    pub(crate) fn update_cursor(&self) {
        unsafe {
            let cur = if self.peer_for_opt(self.hot).is_some() {
                IDC_IBEAM
            } else {
                IDC_ARROW
            };
            let _ = SetCursor(LoadCursorW(None, cur).ok());
        }
    }
    /// peer at hit location — editor = Some; painted node = None
    pub(crate) fn peer_for_opt(&self, hit: Option<NodeId>) -> Option<Rc<RefCell<WindowlessPeer>>> {
        hit.and_then(|id| self.peer_for(id))
    }

    /// physical-window-px -> window-client DIP
    /// client px point from an LPARAM — the ONLY packed-coords decoder;
    /// logical conversion happens inside pointer/hover through ScaleFactor
    pub(crate) fn pt_px(&self, lp: LPARAM) -> space::ClientPhysicalPoint {
        space::ClientPhysicalPoint(space::PhysicalPoint {
            x: lp.0 as i16 as i32,
            y: ((lp.0 >> 16) as i16) as i32,
        })
    }
    /// client px -> window DIP via the shared ScaleFactor
    pub(crate) fn pt(&self, lp: LPARAM) -> Point {
        let s = self.peer_ctx.scale.get();
        let d = self.pt_px(lp).0.logical(s);
        Point { x: d.x, y: d.y }
    }
}
