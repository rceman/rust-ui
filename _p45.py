p='src/platform/win32/mod.rs'; s=open(p,encoding='utf-8').read()

# 1. submit_decision: single-line Enter submits regardless of Shift
s=s.replace("""        match (submit, multiline, shift, ctrl) {
            (SubmitPolicy::Enter, true, true, _) => SubmitDecision::Edit, // newline
            (SubmitPolicy::Enter, _, false, _) => SubmitDecision::Submit,
            (SubmitPolicy::ModifierEnter, _, _, true) => SubmitDecision::Submit,
            _ => SubmitDecision::Edit,
        }""","""        match (submit, multiline, shift, ctrl) {
            // single-line has no newline affordance — Enter submits under
            // ANY modifier state (Shift is meaningless without a newline)
            (SubmitPolicy::Enter, false, _, _) => SubmitDecision::Submit,
            // multiline: plain Enter submits; Shift+Enter is the newline
            (SubmitPolicy::Enter, true, true, _) => SubmitDecision::Edit,
            (SubmitPolicy::Enter, true, false, _) => SubmitDecision::Submit,
            (SubmitPolicy::ModifierEnter, _, _, true) => SubmitDecision::Submit,
            _ => SubmitDecision::Edit,
        }""")

# 2. native_selection: checked conversion + composition gate
s=s.replace("""    fn native_selection(&mut self, _slot: u32, peer: &Rc<RefCell<WindowlessPeer>>) -> UiResult {
        let (text, anchor, focus, rev, node) = {
            let p = peer.borrow();
            let (a, f) = p.selection_utf16()?;
            (p.text()?, a, f, p.peer_rev(), p.node())
        };
        let sel = TextSelection {
            revision: rev,
            anchor: crate::text::utf16_index_to_utf8(&text, anchor).unwrap_or(text.len()),
            focus: crate::text::utf16_index_to_utf8(&text, focus).unwrap_or(text.len()),
        };
        self.rt.selection_event(node, sel)?;
        Ok(())
    }""","""    fn native_selection(&mut self, _slot: u32, peer: &Rc<RefCell<WindowlessPeer>>) -> UiResult {
        let (text, anchor, focus, rev, node) = {
            let p = peer.borrow();
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
            crate::text::utf16_index_to_utf8(&text, i).ok_or_else(|| {
                UiError::Platform(format!("selection offset {i} not representable"))
            })
        };
        let sel = TextSelection {
            revision: rev,
            anchor: to_utf8(anchor)?,
            focus: to_utf8(focus)?,
        };
        self.rt.selection_event(node, sel)?;
        Ok(())
    }""")

# 3. char_msg — the public Key::Char route for non-peer focus + arrival focus
s=s.replace("""    pub(crate) fn char_msg(&mut self, msg: u32, wparam: usize, lparam: isize) -> UiResult {
        if let Some(id) = self.focus {
            self.deliver_native(id, msg, wparam, lparam)?;
            self.service_peer_events()?;
        }
        self.turn()
    }""","""    /// WM_CHAR/WM_SYSCHAR — a focused NATIVE peer sees the raw message;
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
    }""")

# 4. field
s=s.replace("""    /// a reentrant push exceeded REENTRANT_QUEUE_CAP — surfaced as
    /// QueueOverflow by the next drain (typed failure, never silent loss)
    queue_overflowed: std::cell::Cell<bool>,""","""    /// a reentrant push exceeded REENTRANT_QUEUE_CAP — surfaced as
    /// QueueOverflow by the next drain (typed failure, never silent loss)
    queue_overflowed: std::cell::Cell<bool>,
    /// pending UTF-16 lead surrogate awaiting its trail unit (WM_CHAR
    /// pairing for the public Key::Char route)
    pending_lead_surrogate: std::cell::Cell<Option<u16>>,""")
s=s.replace("""        arrival_focus: std::cell::Cell::new(None),
        queue_overflowed: std::cell::Cell::new(false),""","""        arrival_focus: std::cell::Cell::new(None),
        queue_overflowed: std::cell::Cell::new(false),
        pending_lead_surrogate: std::cell::Cell::new(None),""")

# 5. ime_end ordering — clear flag BEFORE draining final commits
s=s.replace("""    pub(crate) fn ime_end(&mut self) -> UiResult {
        if let Some(id) = self.focus {
            self.rt.composition_end(id)?;
        }
        self.turn()
    }""","""    /// The composition-boundary flag falls BEFORE the final drain — a
    /// commit emitted by the end must route as committed text, not be
    /// discarded as residual preedit. Proposals resolve last, against the
    /// TRUE committed revision.
    pub(crate) fn ime_end(&mut self) -> UiResult {
        if let Some(id) = self.focus {
            if let Some(peer) = self.peer_for(id) {
                peer.borrow().set_composing(false);
            }
            self.rt.composition_clear(id);
        }
        self.service_peer_events()?;
        if let Some(id) = self.focus {
            self.rt.composition_end(id)?;
            if let Some(peer) = self.peer_for(id) {
                peer.borrow().finish_pending_relatch()?;
            }
        }
        self.turn()
    }""")

# 6. ime_start sets the peer-side flag too
s=s.replace("""    pub(crate) fn ime_start(&mut self) {
        if let Some(id) = self.focus {
            self.rt.composition_start(id);
        }
    }""","""    pub(crate) fn ime_start(&mut self) {
        if let Some(id) = self.focus {
            self.rt.composition_start(id);
            if let Some(peer) = self.peer_for(id) {
                peer.borrow().set_composing(true);
            }
        }
    }""")
open(p,'w',encoding='utf-8').write(s)

# ---- window.rs: ENDCOMPOSITION order + syskey arm ----
p='src/platform/win32/window.rs'; s=open(p,encoding='utf-8').read()
s=s.replace("""            WM_IME_ENDCOMPOSITION => {
                // peer commits first, its events drain, THEN queued
                // proposals resolve against the final text
                be.send_focused(msg, wparam.0, lparam.0)?;
                be.service_peer_events()?;
                be.ime_end()?;
                LRESULT(0)
            }""","""            WM_IME_ENDCOMPOSITION => {
                // native end first; ime_end clears the flag THEN drains —
                // the final commit must land as committed text
                be.send_focused(msg, wparam.0, lparam.0)?;
                be.ime_end()?;
                LRESULT(0)
            }""")
# syskey branch — advertised deferrable, must dispatch to its contract
s=s.replace("""            WM_CHAR | WM_SYSCHAR => {""","""            WM_SYSKEYDOWN | WM_SYSKEYUP => {
                // system-key semantics are OS-owned — the deferrable route
                // exists so a reentrant arrival still reaches THIS branch;
                // dispatch resolves to the OS default, never drops it
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WM_CHAR | WM_SYSCHAR => {""")
open(p,'w',encoding='utf-8').write(s)
print("ok")
