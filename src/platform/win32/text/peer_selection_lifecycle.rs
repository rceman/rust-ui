impl WindowlessPeer {
    pub(crate) fn set_backward_selection_utf16(&self, anchor: usize, focus: usize) -> UiResult {
        self.restore_selection_utf16(anchor, focus)
    }

    /// Staged-specimen selection setter — a forward `[start, end)` UTF-16
    /// range applied WITHOUT requiring real focus (TXTBIT_HIDESELECTION
    /// stays unset, so an unfocused selection renders). Same TOM route
    /// the relatch restore uses.
    pub(crate) fn set_selection_utf16(&self, start: usize, end: usize) -> UiResult {
        self.restore_selection_utf16(start, end)
    }

    /// Re-apply a directional (anchor, focus) selection after a relatch —
    /// SetStart/SetEnd + the tomSelStartActive flag restore direction.
    fn restore_selection_utf16(&self, anchor: usize, focus: usize) -> UiResult {
        unsafe {
            let mut doc_raw: *mut c_void = std::ptr::null_mut();
            let vt = Interface::vtable(self.tx());
            (vt.base__.base__.QueryInterface)(
                self.tx().as_raw(),
                &ITextDocument::IID as *const GUID,
                &mut doc_raw,
            )
            .ok()
            .map_err(|e| UiError::Platform(format!("QI(ITextDocument): {e}")))?;
            let doc = ITextDocument::from_raw(doc_raw);
            let sel: ITextSelection = doc
                .GetSelection()
                .map_err(|e| UiError::Platform(format!("GetSelection: {e}")))?;
            // DIRECTIONAL RESTORE — SetEnd collapses when anchor > focus
            // (TOM contract): restore the SORTED extent first, then the
            // active-end flag establishes direction without collapsing.
            let (lo, hi) = (anchor.min(focus), anchor.max(focus));
            sel.SetStart(lo as i32)
                .and_then(|_| sel.SetEnd(hi as i32))
                .map_err(|e| UiError::Platform(format!("restore selection: {e}")))?;
            let mut flags = sel
                .GetFlags()
                .map_err(|e| UiError::Platform(format!("GetFlags: {e}")))?;
            if anchor != focus {
                if focus < anchor {
                    flags |= tomSelStartActive.0;
                } else {
                    flags &= !tomSelStartActive.0;
                }
                sel.SetFlags(flags)
                    .map_err(|e| UiError::Platform(format!("SetFlags: {e}")))?;
            }
            Ok(())
        }
    }

    /// Activate IN PLACE once bounds are non-empty. Checked — the state
    /// only advances when msftedit accepts the activation.
    /// Idempotent. UI activation is a separate transition (focus-driven).
    fn ensure_in_place(&self) -> UiResult {
        if self.activation.get() != Activation::Inactive || self.tx.is_none() {
            return Ok(());
        }
        let b = self.shared().host.bounds;
        if b.width <= 0.0 || b.height <= 0.0 {
            return Ok(());
        }
        let sc = self.shared().host.scale;
        // activation rect in the host's px units (see the unit contract at
        // the top of the file / space.rs)
        let mut local: RECT = LogicalRect::local(b.width, b.height).physical(sc).into();
        unsafe {
            self.tx()
                .OnTxInPlaceActivate(&mut local)
                .map_err(|e| UiError::Platform(format!("OnTxInPlaceActivate: {e}")))?;
        }
        self.activation.set(Activation::InPlace);
        Ok(())
    }

    /// UI activation — the focus-owner transition. Checked: the state
    /// advances only when the service accepts. Called by `send` when the
    /// backend routes WM_SETFOCUS/WM_KILLFOCUS to this peer.
    fn set_ui_active(&self, on: bool) -> UiResult {
        match (self.activation.get(), on) {
            (Activation::InPlace, true) => {
                unsafe {
                    self.tx()
                        .OnTxUIActivate()
                        .map_err(|e| UiError::Platform(format!("OnTxUIActivate: {e}")))?;
                }
                self.activation.set(Activation::Ui);
            }
            (Activation::Ui, false) => {
                unsafe {
                    self.tx()
                        .OnTxUIDeactivate()
                        .map_err(|e| UiError::Platform(format!("OnTxUIDeactivate: {e}")))?;
                }
                self.activation.set(Activation::InPlace);
            }
            _ => {}
        }
        Ok(())
    }

    /// Theme colors — CFE_AUTOCOLOR resolves `COLOR_WINDOWTEXT` through
    /// `TxGetSysColor` at draw time; no run-format rewrite needed.
    /// Theme palette flip — selection colors follow the palette and the
    /// AUTHORED foreground re-resolves: a role tracks the new theme; an
    /// authored literal is already concrete and stands.
    /// Selection palette + authored-foreground refresh. `forced` is the
    /// OS system-color resolver — a role foreground under forced colors
    /// resolves through it (same authority as the render path); literal
    /// colors pass through the ordinary resolver.
    /// The effective (fg, sel_bg, sel_fg) this peer would apply — the
    /// RESOLVED output, including forced-color system slots. Comparing
    /// these values (not appearance/theme descriptors) is how the
    /// backend detects a real palette change.
    pub(crate) fn effective_colors(
        &self,
        sel_bg: [f32; 4],
        sel_fg: [f32; 4],
        dark: bool,
        forced: Option<&dyn Fn(crate::style::SystemColor) -> [f32; 4]>,
    ) -> ([f32; 4], [f32; 4], [f32; 4]) {
        let s = self.shared();
        let fg = match (s.host.fg_authored, forced) {
            (crate::style::Color::Role(r), Some(f)) => f(crate::style::system_slot(r)),
            (c, _) => crate::style::resolve_color(c, dark),
        };
        (fg, sel_bg, sel_fg)
    }

    pub(crate) fn set_colors(
        &self,
        sel_bg: [f32; 4],
        sel_fg: [f32; 4],
        dark: bool,
        forced: Option<&dyn Fn(crate::style::SystemColor) -> [f32; 4]>,
    ) {
        let (resolved, sel_bg, sel_fg) = self.effective_colors(sel_bg, sel_fg, dark, forced);
        let mut s = self.shared_mut();
        s.host.fg = colorref(resolved);
        s.host.cf.crTextColor = colorref(resolved);
        s.host.sel_bg = colorref(sel_bg);
        s.host.sel_fg = colorref(sel_fg);
        // COLOR_WINDOW is the selection band's blend BASE (band =
        // 0.4·sel_bg + 0.6·win_bg under D2DDWRITE — UI activation does
        // NOT change it, verified live). A near-black base makes the
        // contract's #000 band render ≈#010101 instead of the washed
        // #9b9b9b that a truthful white window color produced; must
        // stay ≠ sel_bg or the band is skipped entirely.
        s.host.win_bg = COLORREF(0x010101);
        s.host.ev(HostEvent::Invalidate);
    }

    /// The host's read-only gate (separate from disabled input routing) —
    /// pushes ES_READONLY behavior into the service so native editing,
    /// selection changes through keys, and paste all stop.
    /// Live read-only toggle — TXTBIT_READONLY is a host PROPERTY BIT in
    /// the windowless contract (EM_SETREADONLY/EM_SETOPTIONS don't apply):
    /// update the bits the host reports, then OnTxPropertyBitsChange makes
    /// the service re-query. Editing, paste and IME are all dead inside
    /// msftedit — not just gated in the input router.
    /// latched authored line box (twips) — 0 = natural pitch
    pub(crate) fn line_twips_latched(&self) -> i32 {
        self.line_twips
    }

    pub(crate) fn set_read_only(&self, ro: bool) {
        {
            let mut s = self.shared_mut();
            s.host.read_only = ro;
            if ro {
                s.host.bits |= TXTBIT_READONLY;
            } else {
                s.host.bits &= !TXTBIT_READONLY;
            }
        }
        if let Some(tx) = &self.tx {
            let bits = self.shared().host.bits;
            let _ = unsafe { tx.OnTxPropertyBitsChange(TXTBIT_READONLY, bits) };
        }
    }

    /// The peer's host hwnd (assigned when the window exists).
    pub(crate) fn set_hwnd(&self, hwnd: HWND) {
        self.shared_mut().host.hwnd = hwnd;
    }

    /// Richedit-requested native timer ids currently armed.
    pub(crate) fn native_timers(&self) -> Vec<u32> {
        self.shared().host.armed_timers.keys().copied().collect()
    }

    /// Caret state as `(created, shown, pos, size)` — the backend draws the
    /// real Win32 system caret when this peer owns focus.
    pub(crate) fn caret(&self) -> (bool, bool, POINT, SIZE) {
        let s = self.shared();
        (
            s.host.caret_created,
            s.host.caret_shown,
            s.host.caret_pos,
            s.host.caret_size,
        )
    }

    /// Drain host events queued while a text-services call ran.
    pub(crate) fn drain(&self) -> UiResult<Vec<HostEvent>> {
        let (overflow, events) = {
            let mut s = self.shared_mut();
            let ov = std::mem::replace(&mut s.host.overflow, false);
            (ov, std::mem::take(&mut s.host.events))
        };
        if overflow {
            return Err(UiError::QueueOverflow);
        }
        Ok(events)
    }

    /// Queue a semantic event for the backend to route into the runtime.
    fn emit(&self, item: super::NativeSinkItem) {
        let mut sink = self.sink.lock().unwrap();
        if sink.len() < crate::event::EVENT_QUEUE_CAP {
            sink.push(item);
        } else {
            // bounded — the backend's drain surfaces overflow as typed
            self.shared_mut().host.overflow = true;
        }
    }

    /// QI the text services for an arbitrary IID (windowless accessibility).
    pub(crate) fn query_iid(&self, iid: &GUID) -> UiResult<*mut c_void> {
        unsafe {
            let mut p: *mut c_void = std::ptr::null_mut();
            let vt = Interface::vtable(self.tx());
            (vt.base__.base__.QueryInterface)(self.tx().as_raw(), iid as *const GUID, &mut p)
                .ok()
                .map_err(|e| UiError::Platform(format!("query_iid: {e}")))?;
            Ok(p)
        }
    }
}

/// One synchronous `TxSendMessage` outcome: the HRESULT carries
/// processing/fallback information (`S_OK` = the service handled it,
/// `S_FALSE` = the host should process it), the LRESULT is the message's
/// own result.
pub(crate) struct NativeSend {
    pub hr: HRESULT,
    pub lr: isize,
}

impl NativeSend {
    /// `S_FALSE` → the service did not consume the key; the framework may
    /// interpret it (submit/shortcut). During IME composition a confirm
    /// key is consumed (`S_OK`), so it never submits.
    pub(crate) fn consumed(&self) -> bool {
        self.hr != S_FALSE
    }
}

/// `CreateTextServices` config — theme/format at peer creation.
pub(crate) struct PeerConfig {
    pub multiline: bool,
    pub read_only: bool,
    pub face: String,
    pub size_twips: i32,
    /// target line box in twips — msftedit's D2DDWRITE layout ignores
    /// EVERY paragraph-spacing control (EM_SETPARAFORMAT, the
    /// TxGetParaFormat default, TOM ITextPara/ITextPara2
    /// SetLineSpacing, RTF \sl, negative spaceBefore/After: all
    /// measured inert/E_FAIL), so the peer reaches the target pitch by
    /// calibrating `yHeight` instead — see `create`.
    /// 0 keeps the font's natural height.
    pub line_twips: i32,
    pub fg: [f32; 4],
    /// the consumer's authored foreground `Color` (roles re-resolve)
    pub fg_authored: crate::style::Color,
    pub sel_bg: [f32; 4],
    pub sel_fg: [f32; 4],
    /// bold flag folded into CHARFORMAT dwEffects (CFM_BOLD)
    pub bold: bool,
}

impl crate::node::TextPeer for WindowlessPeer {
    fn peer_id(&self) -> u64 {
        self.id
    }

    fn initialize(
        &mut self,
        text: &str,
        revision: TextRevision,
        binding: BindingToken,
    ) -> UiResult {
        unsafe {
            let h = HSTRING::from(text);
            self.tx()
                .TxSetText(PCWSTR(h.as_ptr()))
                .map_err(|e| UiError::Platform(format!("TxSetText(init): {e}")))?;
        }
        self.binding = Some(binding);
        self.peer_rev.set(revision);
        // mount-time init must NOT fabricate an acknowledgement — the sync
        // only accepts a real Programmatic ack for a queued proposal
        self.drain().map(|_| ())
    }

    fn set_text(&mut self, text: &str, base: TextRevision, requested: TextRevision) -> UiResult {
        unsafe {
            let h = HSTRING::from(text);
            self.tx()
                .TxSetText(PCWSTR(h.as_ptr()))
                .map_err(|e| UiError::Platform(format!("TxSetText: {e}")))?;
        }
        // actual read-back — the ack must describe what the peer committed
        let committed = self.text()?;
        self.peer_rev.set(requested);
        if let Some(b) = self.binding {
            self.emit(super::NativeSinkItem::Edit {
                node: self.node,
                edit: crate::text::TextEdit {
                    text: committed,
                    base,
                    result: requested,
                    origin: crate::text::EditOrigin::Programmatic,
                    binding: b,
                },
            });
        }
        Ok(())
    }

    fn release(&mut self) {
        // text services release first (may call back into the host)
        // the compat-target surface dies with the peer — its bitmap is
        // frame-target-bound and must not outlive it
        drop(self.surface.borrow_mut().take());
        drop(self.tx.take());
        self.kill_armed_timers();
        unsafe {
            HostBox::Release(self.host);
        }
        self.host = std::ptr::null_mut();
    }

    fn attach(&mut self, node: NodeId) {
        self.node = node;
        self.shared_mut().host.node = Some(node);
    }

    /// A staged selection is authoritative — apply it whenever the peer's
    /// live selection differs (idempotent: no write when already staged);
    /// `None` leaves selection to user/native control.
    fn apply_selection(&mut self, sel: Option<(usize, usize)>) -> UiResult {
        // never write a staged range into an in-flight composition — a
        // SetStart/SetEnd while msftedit holds a marked range would
        // clobber the IME's target
        if self.composing.get() {
            return Ok(());
        }
        if let Some((s, e)) = sel {
            // clamp the staged range to the story length so an
            // out-of-range author value doesn't re-apply every commit
            // (TOM clamps on write but the staged value never equals
            // read-back → idempotence broken)
            let story = self.story_len_utf16().unwrap_or(usize::MAX);
            let (s, e) = (s.min(story), e.min(story));
            if self.selection_utf16().ok() != Some((s, e)) {
                self.set_selection_utf16(s, e)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
impl WindowlessPeer {
    /// TEST SEAM — write text into the real service WITHOUT the
    /// programmatic-ack channel (no sink Edit) — the same bytes a real
    /// IME commit leaves behind.
    pub(crate) fn native_write_for_test(&self, text: &str) -> UiResult {
        unsafe {
            let h = windows_core::HSTRING::from(text);
            self.tx()
                .TxSetText(windows_core::PCWSTR(h.as_ptr()))
                .map_err(|e| UiError::Platform(format!("TxSetText: {e}")))?;
        }
        Ok(())
    }

    /// TEST SEAM — emit the host Change notification EN_CHANGE raises.
    pub(crate) fn emit_change_for_test(&self) {
        self.shared_mut().host.ev(HostEvent::Change);
    }

    /// TEST SEAM — the colors actually applied to the host
    /// (fg, sel_bg, sel_fg as COLORREF channels) — the F08 assertion
    /// surface: "the peer palette refreshed" must mean THESE changed.
    pub(crate) fn applied_colors_for_test(&self) -> (u32, u32, u32) {
        let s = self.shared();
        (s.host.fg.0, s.host.sel_bg.0, s.host.sel_fg.0)
    }
}

/// Kill every native timer this host armed — used by release() AND
/// Drop; an orphan periodic timer must not outlive its peer.
impl WindowlessPeer {
    pub(crate) fn kill_armed_timers(&mut self) {
        if self.host.is_null() {
            return;
        }
        let (hwnd, tids, pool) = {
            let mut s = self.shared_mut();
            let t: Vec<usize> = s.host.armed_timers.values().copied().collect();
            s.host.armed_timers.clear();
            (s.host.hwnd, t, s.host.timer_pool.clone())
        };
        for tid in tids {
            pool.lock().unwrap().kill(tid);
            if !hwnd.is_invalid() {
                unsafe {
                    let _ = windows::Win32::UI::WindowsAndMessaging::KillTimer(Some(hwnd), tid);
                }
            }
        }
    }
}

impl Drop for WindowlessPeer {
    fn drop(&mut self) {
        if !self.host.is_null() {
            drop(self.tx.take());
            // a dead peer can never receive WM_TIMER — tombstone its ids
            self.kill_armed_timers();
            unsafe {
                HostBox::Release(self.host);
            }
        }
    }
}
