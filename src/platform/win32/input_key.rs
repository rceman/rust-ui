use super::*;

impl<S, M, U, V> Backend<S, M, U, V>
where
    M: 'static,
    U: Fn(&mut S, M, &mut UpdateCtx<'_, M>),
    V: Fn(&S, &mut crate::Ui<'_, '_, M>),
{
    /// Keyboard: focused editor gets the raw message FIRST — native
    /// handling decides whether the key remains available for framework
    /// interpretation (a key consumed by msftedit — e.g. Enter confirming
    /// an IME composition — never produces Submit). Everything else is a
    /// semantic `NodeEvent::Key`/`Press`.
    pub(crate) fn key_msg(&mut self, msg: u32, vk: usize, lparam: isize) -> UiResult {
        let key = vk as u32;
        let ev = match key {
            k if k == VK_TAB.0 as u32 => Key::Tab,
            k if k == VK_RETURN.0 as u32 => Key::Enter,
            k if k == VK_SPACE.0 as u32 => Key::Space,
            k if k == VK_ESCAPE.0 as u32 => Key::Escape,
            k if k == VK_BACK.0 as u32 => Key::Backspace,
            k if k == VK_DELETE.0 as u32 => Key::Delete,
            k if k == VK_LEFT.0 as u32 => Key::ArrowLeft,
            k if k == VK_RIGHT.0 as u32 => Key::ArrowRight,
            k if k == VK_UP.0 as u32 => Key::ArrowUp,
            k if k == VK_DOWN.0 as u32 => Key::ArrowDown,
            k if k == windows::Win32::UI::Input::KeyboardAndMouse::VK_HOME.0 as u32 => Key::Home,
            k if k == windows::Win32::UI::Input::KeyboardAndMouse::VK_END.0 as u32 => Key::End,
            other => Key::Other(other),
        };
        // Escape dismisses the topmost open popup before peer routing —
        // a focused editor inside it must not swallow the dismissal
        if msg == WM_KEYDOWN && ev == Key::Escape {
            if let Some(id) = self.topmost_popup() {
                self.push_input(id, NodeEvent::OpenChange(false))?;
                return self.turn();
            }
        }
        // Tab traversal — focus moves through eligible nodes
        if msg == WM_KEYDOWN && key == VK_TAB.0 as u32 {
            self.focus_step(unsafe { GetKeyState(VK_SHIFT.0 as i32) } < 0)?;
            return self.turn();
        }
        // focused editor gets the raw key (native editing, IME) —
        // a deferred replay delivers to the arrival-time owner
        if let Some(focus) = self.focused_target()
            && self.peer_for(focus).is_some()
        {
            // SUBMIT CONTRACT — the canonical boundary is: plain Enter
            // submits, Shift+Enter edits (newline). The decision is made
            // BEFORE delivery because a submitting Enter must NOT reach
            // msftedit (it would insert a newline AND submit — two effects
            // for one keypress, which the contract forbids).
            if msg == WM_KEYDOWN && ev == Key::Enter && !self.rt.composition_active(focus) {
                match self.submit_decision(focus) {
                    SubmitDecision::Submit => {
                        self.push_input(focus, NodeEvent::Submit)?;
                        return self.turn();
                    }
                    SubmitDecision::Edit => {
                        // editing Enter (newline / modifier-not-met) — falls
                        // through to normal delivery, never also submits
                    }
                }
            }
            // send first — the consumed signal decides availability; a
            // deferred delivery reports no result, and an active
            // composition owns the keyboard either way
            let delivered = self.deliver_native(focus, msg, vk, lparam)?;
            self.service_peer_events()?;
            let consumed = match delivered {
                Some(s) => s.consumed(),
                // no synchronous result — treat as consumed (never submit
                // on an unknown; the queued key still reaches the peer)
                None => true,
            };
            if msg == WM_KEYDOWN && !self.rt.composition_active(focus) {
                // unconsumed → policy decides; a consumed key is editing
                // alone (the Enter/submit fork happened BEFORE delivery —
                // a consumed Enter was never a submit candidate)
                if !consumed && let Some(sub) = self.check_submit(focus, ev) {
                    self.push_input(focus, sub)?;
                }
            }
            return self.turn();
        }
        if msg != WM_KEYDOWN {
            return Ok(());
        }
        // directional group navigation — roving focus inside a semantic
        // group (tablist arrows move focus AND activate the target;
        // listbox options move focus only). Shared policy lives in
        // `crate::layout::nav_target`.
        if let Some(focus) = self.focused_target()
            && let Some(n) = self.rt.arena.get(focus)
            && matches!(
                n.role,
                Some(
                    crate::node::SemanticRole::Tab
                        | crate::node::SemanticRole::Option
                        | crate::node::SemanticRole::MenuItem
                        | crate::node::SemanticRole::MenuItemCheckbox
                        | crate::node::SemanticRole::MenuItemRadio
                        | crate::node::SemanticRole::MenuItemDestructive
                )
            )
            && let Some(target) = crate::layout::nav_target(&self.rt.arena, &self.order, focus, ev)
            && target != focus
        {
            let activate = matches!(n.role, Some(crate::node::SemanticRole::Tab));
            self.set_focus(Some(target))?;
            if activate {
                self.push_input(target, NodeEvent::Press)?;
            }
            return self.turn();
        }
        // semantic focus: Space/Enter on a button is Press — a deferred
        // replay delivers to the ARRIVAL-captured owner: an explicitly
        // absent owner must not activate a newly focused button, and a
        // captured A must not be redirected to B
        if let Some(focus) = self.focused_target() {
            match ev {
                Key::Space | Key::Enter => {
                    self.push_input(focus, NodeEvent::Press)?;
                    return self.turn();
                }
                _ => {
                    self.push_input(
                        focus,
                        NodeEvent::Key(KeyEvent {
                            key: ev,
                            modifiers: modifiers_now(),
                        }),
                    )?;
                    return self.turn();
                }
            }
        }
        Ok(())
    }

    /// The Enter decision — one authority for the submit/edit fork.
    /// IME-committed Enter never reaches here (composition owns it).
    pub(super) fn submit_decision(&self, node: NodeId) -> SubmitDecision {
        let Some(n) = self.rt.arena.get(node) else {
            return SubmitDecision::Edit;
        };
        let NodeData::Editor {
            submit, multiline, ..
        } = &n.data
        else {
            return SubmitDecision::Edit;
        };
        let shift =
            unsafe { windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(VK_SHIFT.0 as i32) }
                < 0;
        let ctrl = unsafe {
            windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(VK_CONTROL.0 as i32)
        } < 0;
        match (submit, multiline, shift, ctrl) {
            // single-line has no newline affordance — Enter submits under
            // ANY modifier state (Shift is meaningless without a newline)
            (SubmitPolicy::Enter, false, _, _) => SubmitDecision::Submit,
            // multiline: plain Enter submits; Shift+Enter is the newline
            (SubmitPolicy::Enter, true, true, _) => SubmitDecision::Edit,
            (SubmitPolicy::Enter, true, false, _) => SubmitDecision::Submit,
            (SubmitPolicy::ModifierEnter, _, _, true) => SubmitDecision::Submit,
            _ => SubmitDecision::Edit,
        }
    }

    /// Non-Enter keys that arrived unconsumed — reserved for future
    /// semantic keys; today only Enter has policy.
    pub(super) fn check_submit(&self, node: NodeId, key: Key) -> Option<NodeEvent> {
        if key != Key::Enter {
            return None;
        }
        match self.submit_decision(node) {
            SubmitDecision::Submit => Some(NodeEvent::Submit),
            SubmitDecision::Edit => None,
        }
    }
    /// Focus next/previous eligible node (Tab/Shift-Tab traversal).
    pub(crate) fn focus_step(&mut self, back: bool) -> UiResult {
        let modal = self.modal_popup();
        let eligible: Vec<NodeId> = self
            .order
            .iter()
            .copied()
            .filter(|&id| {
                // a live modal popup traps traversal in its subtree
                if let Some(m) = modal
                    && !self.inside_subtree(m, id)
                {
                    return false;
                }
                let Some(n) = self.rt.arena.get(id) else {
                    return false;
                };
                // a scroll clip that fully excludes the node's rect means
                // the control renders nothing — Tab must not land on an
                // invisible target (same clip truth devtools/UIA report)
                let clipped_out = self
                    .rects
                    .get(&id)
                    .zip(self.clips.get(&id))
                    .is_some_and(|(r, c)| !r.intersects(*c));
                n.visibility == Visibility::Visible
                    && !clipped_out
                    && n.interactive()
                    && matches!(
                        n.data,
                        NodeData::Button { .. }
                            | NodeData::Editor { .. }
                            | NodeData::Action { .. }
                            | NodeData::Toggle { .. }
                    )
            })
            .collect();
        if eligible.is_empty() {
            return Ok(());
        }
        let idx = self
            .focus
            .and_then(|f| eligible.iter().position(|&id| id == f))
            .unwrap_or(usize::MAX);
        let next = if back {
            match idx {
                usize::MAX => eligible.len() - 1,
                0 => eligible.len() - 1,
                i => i - 1,
            }
        } else {
            match idx {
                usize::MAX => 0,
                i => (i + 1) % eligible.len(),
            }
        };
        self.set_focus(Some(eligible[next]))?;
        // route the focus send's host events before the caller's turn
        self.service_peer_events()
    }

    /// First focusable descendant of `root` in paint order — used by
    /// popups' `focus_on_open`. Same eligibility as `focus_step`; a
    /// `selected` descendant wins so a listbox opens with the selected
    /// option live.
    pub(super) fn first_focusable(&self, root: NodeId) -> Option<NodeId> {
        let eligible = |id: NodeId| -> Option<NodeId> {
            (id != root
                && self.inside_subtree(root, id)
                && self.rt.arena.get(id).is_some_and(|n| {
                    n.visibility == Visibility::Visible
                        && n.interactive()
                        && matches!(
                            n.data,
                            NodeData::Button { .. }
                                | NodeData::Editor { .. }
                                | NodeData::Action { .. }
                                | NodeData::Toggle { .. }
                        )
                }))
            .then_some(id)
        };
        self.order
            .iter()
            .copied()
            .find(|&id| {
                eligible(id).is_some()
                    && matches!(
                        self.rt.arena.get(id).map(|n| &n.data),
                        Some(NodeData::Button { selected: true, .. })
                    )
            })
            .or_else(|| self.order.iter().copied().find_map(eligible))
    }

    /// Focus a node — blur the old (still routed even when it just hid),
    /// focus the new. Peers get the raw focus messages so caret/edit state
    /// tracks the framework focus owner.
    pub(crate) fn set_focus(&mut self, node: Option<NodeId>) -> UiResult {
        if self.focus == node {
            return Ok(());
        }
        let olds: Vec<(NodeId, InteractState)> = [self.focus, node]
            .into_iter()
            .flatten()
            .map(|id| (id, self.interact(id)))
            .collect();
        if let Some(old) = self.focus {
            if self.rt.arena.is_live(old) {
                self.push_input(old, NodeEvent::Focus(false))?;
            }
            self.deliver_native(old, WM_KILLFOCUS, 0, 0)?;
        }
        self.focus = node;
        // mark AFTER the assignment — `mark_state_dirty` recomputes the
        // NEW resolved state; running it before `self.focus` flips
        // compared old-to-old and scheduled no repaint on every focus
        // transition (ring never appeared/disappeared until an
        // incidental repaint)
        for (id, old) in olds {
            self.mark_state_dirty(id, old);
        }
        if let Some(u) = &self.uia {
            u.set_focus(node);
        }
        if let Some(id) = node {
            self.push_input(id, NodeEvent::Focus(true))?;
            self.deliver_native(id, WM_SETFOCUS, 0, 0)?;
        }
        Ok(())
    }

    /// The complete focus boundary: WM_SETFOCUS/KILLFOCUS delivered to a
    /// peer queues host events (caret, invalidate, selchange) — and any
    /// Change/SelChange already pending — that must be routed into the
    /// runtime BEFORE the turn's pump. Draining at the turn tail leaves
    /// their sink items stranded across the boundary: wait-idle then sees
    /// `native_notifications > 0` instead of quiescence.
    #[cfg(feature = "devtools")]
    pub(crate) fn focus_and_settle(&mut self, node: Option<NodeId>) -> UiResult {
        self.set_focus(node)?;
        self.service_peer_events()?;
        self.turn()
    }

    /// peer holding focus (if any) — deferred replays resolve the
    /// arrival-time owner so a focus move mid-queue can't redirect input
    pub(crate) fn focus_peer(&self) -> Option<Rc<RefCell<WindowlessPeer>>> {
        self.focused_target().and_then(|id| self.peer_for(id))
    }
    /// WM_CHAR/WM_SYSCHAR — the focused peer sees the raw message with its
    /// real lparam (repeat count, scan code, alt flag are part of the
    /// native text-input contract)
    /// WM_CHAR/WM_SYSCHAR — a focused NATIVE peer sees the raw message;
    /// a focused painted/custom node sees the public semantic route
    /// (`Key::Char`). Surrogate pairs are joined here so consumers get a
    /// complete `char` — never a lone lead unit.
    pub(crate) fn char_msg(&mut self, msg: u32, wparam: usize, lparam: isize) -> UiResult {
        let Some(id) = self.focused_target() else {
            return self.turn();
        };
        if self.peer_for(id).is_some() {
            self.deliver_native(id, msg, wparam, lparam)?;
            self.service_peer_events()?;
            return self.turn();
        }
        if msg != WM_CHAR {
            return Ok(()); // WM_SYSCHAR carries no portable semantic
        }
        let unit = wparam as u32;
        let ch = if (0xD800..0xDC00).contains(&unit) {
            self.pending_lead_surrogate.set(Some(unit as u16));
            return self.turn(); // pair not yet complete
        } else if (0xDC00..0xE000).contains(&unit) {
            let Some(lead) = self.pending_lead_surrogate.take() else {
                return self.turn(); // stray trail unit — drop
            };
            char::from_u32(0x10000 + (((lead as u32) - 0xD800) << 10) + (unit - 0xDC00))
        } else {
            char::from_u32(unit)
        };
        if let Some(c) = ch {
            self.push_input(
                id,
                NodeEvent::Key(KeyEvent {
                    key: Key::Char(c),
                    modifiers: modifiers_now(),
                }),
            )?;
        }
        self.turn()
    }
}
