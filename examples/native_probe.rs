//! native_probe — repository-local native Windows harness.
//!
//! Boundary: PowerShell owns orchestration (build/run/PID lifecycle/scenario
//! sequencing/artifacts); THIS tool owns native semantics — physical-pixel
//! geometry, real input, DPI transitions, UIA comparison, and real IME.
//!
//! Every subcommand prints one JSON evidence object to stdout and exits
//! non-zero on a failed invariant. Evidence classes:
//!   - "regression"  — synthetic/sent messages (deterministic)
//!   - "acceptance"  — real OS behavior (SendInput / real IME / real DPI)
//!
//! All coordinates are PHYSICAL PIXELS — this process is Per-Monitor-V2,
//! matching the production composer's awareness model (Gate 16).

#[cfg(windows)]
mod probe {
    use std::ffi::c_void;
    use windows::Win32::Foundation::*;
    use windows::Win32::System::Com::*;
    use windows::Win32::System::Memory::*;
    use windows::Win32::System::Threading::*;
    use windows::Win32::UI::Accessibility::*;
    use windows::Win32::UI::HiDpi::*;
    use windows::Win32::UI::Input::Ime::*;
    use windows::Win32::UI::Input::KeyboardAndMouse::*;
    use windows::Win32::UI::TextServices::*;
    use windows::Win32::UI::WindowsAndMessaging::*;
    windows::core::link!("kernel32.dll" "system" fn WriteProcessMemory(h: HANDLE, base: *const c_void, buf: *const c_void, sz: usize, written: *mut usize) -> BOOL);
    use windows::core::*;

    pub fn set_pmv2() {
        unsafe {
            let _ = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }
    }

    /// Resolve a target window: `0x<hex>`/decimal digits = an explicit HWND;
    /// anything else is a title-substring search (visible top-level only).
    pub fn resolve_window(arg: &str) -> Result<HWND> {
        let parsed = arg
            .strip_prefix("0x")
            .and_then(|h| isize::from_str_radix(h, 16).ok())
            .or_else(|| arg.parse::<isize>().ok());
        if let Some(v) = parsed {
            let h = HWND(v as *mut c_void);
            if unsafe { IsWindow(Some(h)).as_bool() } {
                return Ok(h);
            }
            return Err(Error::new(E_FAIL.into(), "hwnd is not a live window"));
        }
        find_window(arg)
    }

    /// Find a visible top-level window whose title contains `part`.
    pub fn find_window(part: &str) -> Result<HWND> {
        struct Ctx {
            needle: String,
            found: HWND,
        }
        unsafe {
            let mut ctx = Ctx {
                needle: part.to_string(),
                found: HWND::default(),
            };
            extern "system" fn cb(h: HWND, l: LPARAM) -> BOOL {
                let ctx = unsafe { &mut *(l.0 as *mut Ctx) };
                let mut buf = [0u16; 512];
                let n = unsafe { GetWindowTextW(h, &mut buf) };
                let title = String::from_utf16_lossy(&buf[..n as usize]);
                if !title.is_empty()
                    && title.contains(ctx.needle.as_str())
                    && unsafe { IsWindowVisible(h).as_bool() }
                {
                    ctx.found = h;
                    return BOOL(0);
                }
                BOOL(1)
            }
            let _ = EnumWindows(Some(cb), LPARAM(&mut ctx as *mut Ctx as isize));
            if ctx.found.is_invalid() {
                Err(Error::new(E_FAIL.into(), "window not found"))
            } else {
                Ok(ctx.found)
            }
        }
    }

    /// Physical-px geometry snapshot of a window.
    pub fn geometry(hwnd: HWND) -> Result<String> {
        unsafe {
            let mut wr = RECT::default();
            GetWindowRect(hwnd, &mut wr)?;
            let mut cr = RECT::default();
            GetClientRect(hwnd, &mut cr)?;
            let org = rust_ui::dev::client_to_screen(
                hwnd,
                rust_ui::dev::ClientPhysicalPoint(rust_ui::dev::PhysicalPoint { x: 0, y: 0 }),
            )
            .ok_or_else(|| Error::new(E_FAIL.into(), "ClientToScreen failed"))?;
            let dpi = GetDpiForWindow(hwnd);
            let scale = rust_ui::dev::scale_from_dpi(dpi).0;
            // record the awareness the probe ACTUALLY runs under — Gate 16
            let aware = GetThreadDpiAwarenessContext();
            let pmv2 =
                AreDpiAwarenessContextsEqual(aware, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)
                    .as_bool();
            if !pmv2 {
                return Err(Error::new(
                    E_FAIL.into(),
                    "probe is not PerMonitorV2-aware — evidence invalid",
                ));
            }
            Ok(format!(
                "{{\"kind\":\"geometry\",\"evidence\":\"acceptance\",\"awareness\":\"pmv2\",\"dpi\":{dpi},\"scale\":{scale:.3},\"window_px\":[{},{},{},{}],\"client_px\":[0,0,{},{}],\"client_origin_px\":[{},{}]}}",
                wr.left, wr.top, wr.right, wr.bottom, cr.right, cr.bottom, org.0.x, org.0.y,
            ))
        }
    }

    /// Real physical-px click via SendInput at client coords (cx,cy).
    pub fn click(hwnd: HWND, cx: i32, cy: i32) -> Result<()> {
        unsafe {
            let org = rust_ui::dev::client_to_screen(
                hwnd,
                rust_ui::dev::ClientPhysicalPoint(rust_ui::dev::PhysicalPoint { x: 0, y: 0 }),
            )
            .ok_or_else(|| Error::new(E_FAIL.into(), "ClientToScreen failed"))?;
            let (sx, sy) = (org.0.x + cx, org.0.y + cy);
            foreground(hwnd);
            std::thread::sleep(std::time::Duration::from_millis(120));
            SetCursorPos(sx, sy)?;
            std::thread::sleep(std::time::Duration::from_millis(60));
            mouse_event(MOUSEEVENTF_LEFTDOWN, 0, 0, 0, 0);
            std::thread::sleep(std::time::Duration::from_millis(40));
            mouse_event(MOUSEEVENTF_LEFTUP, 0, 0, 0, 0);
            std::thread::sleep(std::time::Duration::from_millis(120));
            Ok(())
        }
    }

    /// Bring hwnd to the foreground for real-input delivery.
    /// AttachThreadInput trick — plain SetForegroundWindow is denied to a
    /// non-foreground process.
    pub fn foreground(hwnd: HWND) {
        unsafe {
            let fg = GetForegroundWindow();
            let cur = GetCurrentThreadId();
            let fg_tid = GetWindowThreadProcessId(fg, None);
            let tgt_tid = GetWindowThreadProcessId(hwnd, None);
            if fg_tid != 0 && fg_tid != cur {
                let _ = AttachThreadInput(cur, fg_tid, true);
            }
            let _ = SetForegroundWindow(hwnd);
            let _ = SetFocus(Some(hwnd));
            let _ = BringWindowToTop(hwnd);
            if fg_tid != 0 && fg_tid != cur {
                let _ = AttachThreadInput(cur, fg_tid, false);
            }
            let _ = tgt_tid;
        }
    }

    /// Real ASCII text via SendInput (VkKeyScan + SHIFT state).
    pub fn type_text(hwnd: HWND, text: &str) -> Result<()> {
        unsafe {
            foreground(hwnd);
            std::thread::sleep(std::time::Duration::from_millis(120));
            for ch in text.chars() {
                let mut buf = [0u16; 1];
                ch.encode_utf16(&mut buf);
                let vk = VkKeyScanExW(buf[0], GetKeyboardLayout(0));
                if (vk as i32) < 0 {
                    continue;
                }
                let v = (vk & 0xff) as u16;
                let shift = (vk >> 8) & 0x01 != 0;
                if shift {
                    keybd_event(0x10, 0, KEYBD_EVENT_FLAGS(0), 0);
                }
                keybd_event(v as u8, 0, KEYBD_EVENT_FLAGS(0), 0);
                keybd_event(v as u8, 0, KEYEVENTF_KEYUP, 0);
                if shift {
                    keybd_event(0x10, 0, KEYEVENTF_KEYUP, 0);
                }
                std::thread::sleep(std::time::Duration::from_millis(30));
            }
            Ok(())
        }
    }

    /// Synthetic WM_DPICHANGED — regression-class evidence only (not a real
    /// monitor transition). `SendMessage` marshals the suggested RECT for
    /// this known system message (PostMessage refuses cross-process pointer
    /// lparams — the prior VirtualAllocEx path silently no-opped).
    /// Rust owns the semantic proof: expected physical size is computed via
    /// the shared scale authority and the post-change window rect is read
    /// back and asserted HERE — PowerShell only collects the result.
    pub fn dpi_changed(hwnd: HWND, _pid: u32, dpi: u32) -> Result<String> {
        unsafe {
            let s = rust_ui::dev::scale_from_dpi(dpi);
            let want_w = s.to_physical(500.0);
            let want_h = s.to_physical(470.0);
            let rect = RECT {
                left: 0,
                top: 0,
                right: want_w,
                bottom: want_h,
            };
            SendMessageW(
                hwnd,
                0x02E0, // WM_DPICHANGED
                Some(WPARAM(((dpi << 16) | dpi) as usize)),
                Some(LPARAM(&rect as *const RECT as isize)),
            );
            std::thread::sleep(std::time::Duration::from_millis(300));
            // READ-BACK — the window must actually sit at the suggested size
            let mut wr = RECT::default();
            windows::Win32::UI::WindowsAndMessaging::GetWindowRect(hwnd, &mut wr)?;
            let got_w = wr.right - wr.left;
            let got_h = wr.bottom - wr.top;
            if (got_w - want_w).abs() > 4 || (got_h - want_h).abs() > 4 {
                return Err(Error::new(
                    E_FAIL.into(),
                    format!("dpi {dpi}: expected ~{want_w}x{want_h}px, got {got_w}x{got_h}px"),
                ));
            }
            Ok(format!(
                "{{\"kind\":\"dpi\",\"evidence\":\"regression\",\"dpi\":{dpi},\"window_px\":[{},{},{},{}],\"expected_px\":[{},{}]}}",
                wr.left, wr.top, wr.right, wr.bottom, want_w, want_h
            ))
        }
    }

    /// Posted WM_LBUTTON* at client-px (cx,cy) — deterministic regression
    /// class: the REAL dispatch path, no foreground needed. The LPARAM is
    /// packed HERE (i16 pair) — never in PowerShell.
    pub fn click_post(hwnd: HWND, cx: i32, cy: i32) -> Result<()> {
        unsafe {
            // THE checked packing authority — out-of-i16-range coordinates
            // fail the scenario instead of silently truncating
            let lp = LPARAM(
                rust_ui::dev::try_lparam_px(rust_ui::dev::PhysicalPoint { x: cx, y: cy })
                    .ok_or_else(|| Error::new(E_FAIL.into(), "coordinates out of LPARAM range"))?,
            );
            let _ = PostMessageW(Some(hwnd), WM_LBUTTONDOWN, WPARAM(0x0001), lp);
            std::thread::sleep(std::time::Duration::from_millis(60));
            let _ = PostMessageW(Some(hwnd), WM_LBUTTONUP, WPARAM(0), lp);
            std::thread::sleep(std::time::Duration::from_millis(200));
            Ok(())
        }
    }

    /// Posted WM_CHAR per UTF-16 code unit — regression-class typing.
    /// Surrogate pairs land as two messages (what a real IME produces);
    /// WM_KEYDOWN is NOT posted (msftedit would double-insert).
    pub fn type_post(hwnd: HWND, text: &str) -> Result<()> {
        unsafe {
            for ch in text.encode_utf16() {
                let _ = PostMessageW(Some(hwnd), WM_CHAR, WPARAM(ch as usize), LPARAM(0));
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            std::thread::sleep(std::time::Duration::from_millis(150));
            Ok(())
        }
    }

    /// Posted KEYDOWN/KEYUP with real modifier state — keybd_event pokes
    /// the GLOBAL key state (msftedit reads GetKeyState, not the wparam),
    /// so Ctrl/Shift must be physically down while the key arrives.
    pub fn key_post(hwnd: HWND, vk: u32, ctrl: bool, shift: bool) -> Result<()> {
        unsafe {
            if ctrl {
                keybd_event(VK_CONTROL.0 as u8, 0, KEYBD_EVENT_FLAGS(0), 0);
            }
            if shift {
                keybd_event(VK_SHIFT.0 as u8, 0, KEYBD_EVENT_FLAGS(0), 0);
            }
            std::thread::sleep(std::time::Duration::from_millis(40));
            let _ = PostMessageW(Some(hwnd), WM_KEYDOWN, WPARAM(vk as usize), LPARAM(0));
            std::thread::sleep(std::time::Duration::from_millis(60));
            let _ = PostMessageW(Some(hwnd), WM_KEYUP, WPARAM(vk as usize), LPARAM(0));
            if shift {
                keybd_event(VK_SHIFT.0 as u8, 0, KEYEVENTF_KEYUP, 0);
            }
            if ctrl {
                keybd_event(VK_CONTROL.0 as u8, 0, KEYEVENTF_KEYUP, 0);
            }
            std::thread::sleep(std::time::Duration::from_millis(120));
            Ok(())
        }
    }

    unsafe fn uia_find(
        uia: &IUIAutomation,
        el: &IUIAutomationElement,
        name: &str,
    ) -> Option<IUIAutomationElement> {
        let n = el.CurrentName().unwrap_or_default().to_string();
        if n == name {
            return Some(el.clone());
        }
        let walker = uia.RawViewWalker().ok()?;
        let mut ch = walker.GetFirstChildElement(el).ok();
        while let Some(c) = ch {
            if let Some(f) = uia_find(uia, &c, name) {
                return Some(f);
            }
            ch = walker.GetNextSiblingElement(&c).ok();
        }
        None
    }

    /// UIA bounding rect (screen px, physical) of a named element as JSON.
    pub fn uia_rect(hwnd: HWND, name: &str) -> Result<String> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let uia: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL)?;
            let root = uia.ElementFromHandle(hwnd)?;
            let el = uia_find(&uia, &root, name)
                .ok_or_else(|| Error::new(E_FAIL.into(), "uia element not found"))?;
            let r = el.CurrentBoundingRectangle()?;
            CoUninitialize();
            Ok(format!(
                "{{\"kind\":\"uia-rect\",\"name\":\"{name}\",\"rect_px\":[{},{},{},{}]}}",
                r.left, r.top, r.right, r.bottom
            ))
        }
    }

    /// Enabled state of a named element as JSON — checked, not printed.
    pub fn uia_enabled(hwnd: HWND, name: &str) -> Result<String> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let uia: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL)?;
            let root = uia.ElementFromHandle(hwnd)?;
            let el = uia_find(&uia, &root, name)
                .ok_or_else(|| Error::new(E_FAIL.into(), "uia element not found"))?;
            let en = el.CurrentIsEnabled()?.as_bool();
            CoUninitialize();
            Ok(format!(
                "{{\"kind\":\"uia-enabled\",\"name\":\"{name}\",\"enabled\":{en}}}"
            ))
        }
    }

    /// First-level UIA child count — the scale scenario's settle check.
    pub fn uia_count(hwnd: HWND) -> Result<String> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let uia: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL)?;
            let root = uia.ElementFromHandle(hwnd)?;
            let walker = uia.RawViewWalker()?;
            let mut n = 0u32;
            let mut ch = walker.GetFirstChildElement(&root).ok();
            while let Some(c) = ch {
                n += 1;
                ch = walker.GetNextSiblingElement(&c).ok();
            }
            CoUninitialize();
            Ok(format!("{{\"kind\":\"uia-count\",\"children\":{n}}}"))
        }
    }

    /// UIA Invoke on a named element — the action path real clients take.
    pub fn invoke(hwnd: HWND, name: &str) -> Result<String> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let uia: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL)?;
            let root = uia.ElementFromHandle(hwnd)?;
            let el = uia_find(&uia, &root, name)
                .ok_or_else(|| Error::new(E_FAIL.into(), "uia element not found"))?;
            let pat = el.GetCurrentPattern(UIA_InvokePatternId)?;
            let inv: IUIAutomationInvokePattern = pat.cast()?;
            inv.Invoke()?;
            std::thread::sleep(std::time::Duration::from_millis(200));
            CoUninitialize();
            Ok(format!(
                "{{\"kind\":\"invoke\",\"evidence\":\"acceptance\",\"name\":\"{name}\"}}"
            ))
        }
    }

    /// Click a NAMED element — UIA screen rect -> client px -> posted click.
    /// All coordinate math lives here (the shared dev seam), not in PS.
    pub fn click_named(hwnd: HWND, name: &str) -> Result<String> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let uia: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL)?;
            let root = uia.ElementFromHandle(hwnd)?;
            let el = uia_find(&uia, &root, name)
                .ok_or_else(|| Error::new(E_FAIL.into(), "uia element not found"))?;
            let r = el.CurrentBoundingRectangle()?;
            CoUninitialize();
            // screen px -> client px through THE shared adapter
            let cx = (r.left + r.right) / 2;
            let cy = (r.top + r.bottom) / 2;
            let pt = rust_ui::dev::screen_to_client(
                hwnd,
                rust_ui::dev::ScreenPhysicalPoint(rust_ui::dev::PhysicalPoint {
                    x: cx as i32,
                    y: cy as i32,
                }),
            )
            .ok_or_else(|| Error::new(E_FAIL.into(), "ScreenToClient failed"))?;
            click_post(hwnd, pt.0.x, pt.0.y)?;
            Ok(format!(
                "{{\"kind\":\"click-named\",\"evidence\":\"regression\",\"name\":\"{name}\",\"client_px\":[{},{}]}}",
                pt.0.x, pt.0.y
            ))
        }
    }

    /// UIA tree dump with physical-px bounding rects as JSON.
    pub fn uia_tree(hwnd: HWND) -> Result<String> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let uia: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL)?;
            let root = uia.ElementFromHandle(hwnd)?;
            let mut out = String::from("{\"kind\":\"uia\",\"evidence\":\"acceptance\",\"items\":[");
            unsafe fn walk(
                uia: &IUIAutomation,
                el: &IUIAutomationElement,
                depth: u32,
                out: &mut String,
            ) {
                let name = el.CurrentName().unwrap_or_default().to_string();
                let ct = el.CurrentControlType().map(|c| c.0).unwrap_or_default();
                let r = el.CurrentBoundingRectangle().unwrap_or_default();
                let enabled = el.CurrentIsEnabled().map(|b| b.as_bool()).unwrap_or(false);
                let focusable = el
                    .CurrentIsKeyboardFocusable()
                    .map(|b| b.as_bool())
                    .unwrap_or(false);
                if depth > 0 {
                    out.push(',');
                }
                out.push_str(&format!(
                    "{{\"depth\":{},\"ct\":{},\"name\":\"{}\",\"rect_px\":[{:.0},{:.0},{:.0},{:.0}],\"enabled\":{},\"focusable\":{}}}",
                    depth, ct,
                    name.replace('\\', "\\\\").replace('"', "\\\""),
                    r.left, r.top, r.right, r.bottom, enabled, focusable
                ));
                let walker = uia.RawViewWalker().unwrap();
                let mut ch = walker.GetFirstChildElement(el).ok();
                while let Some(c) = ch {
                    walk(uia, &c, depth + 1, out);
                    ch = walker.GetNextSiblingElement(&c).ok();
                }
            }
            walk(&uia, &root, 0, &mut out);
            out.push_str("]}");
            CoUninitialize();
            Ok(out)
        }
    }

    /// Read the ValuePattern text of a named UIA element (peer text check).
    pub fn uia_value(hwnd: HWND, name: &str) -> Result<Option<String>> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let uia: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL)?;
            let root = uia.ElementFromHandle(hwnd)?;
            unsafe fn find(
                uia: &IUIAutomation,
                el: &IUIAutomationElement,
                name: &str,
            ) -> Option<IUIAutomationElement> {
                let n = el.CurrentName().unwrap_or_default().to_string();
                if n == name {
                    return Some(el.clone());
                }
                let walker = uia.RawViewWalker().unwrap();
                let mut ch = walker.GetFirstChildElement(el).ok();
                while let Some(c) = ch {
                    if let Some(f) = find(uia, &c, name) {
                        return Some(f);
                    }
                    ch = walker.GetNextSiblingElement(&c).ok();
                }
                None
            }
            let el = find(&uia, &root, name);
            let out = el.and_then(|e| unsafe {
                e.GetCurrentPattern(UIA_ValuePatternId)
                    .ok()
                    .and_then(|o| o.cast::<IUIAutomationValuePattern>().ok())
                    .and_then(|vp: IUIAutomationValuePattern| vp.CurrentValue().ok())
                    .map(|v: BSTR| v.to_string())
            });
            CoUninitialize();
            Ok(out)
        }
    }

    /// Real IME driver — activates the Japanese TIP, opens the IMC in
    /// hiragana, sends REAL SendInput keys, then commits with Enter.
    /// This is acceptance-class evidence: the composition path runs through
    /// the OS input pipeline into real WM_IME_* delivery.
    /// Observable submit signal — the composer's `turn N` label is the
    /// real submit counter (Send increments it; a label's name IS its text).
    pub fn turn_counter(hwnd: HWND) -> Result<Option<u64>> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let uia: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL)?;
            let root = uia.ElementFromHandle(hwnd)?;
            unsafe fn walk(uia: &IUIAutomation, el: &IUIAutomationElement) -> Option<u64> {
                let n = el.CurrentName().unwrap_or_default().to_string();
                if let Some(rest) = n.strip_prefix("turn ") {
                    if let Some(n) = rest.split('|').next().and_then(|t| t.trim().parse().ok()) {
                        return Some(n);
                    }
                }
                let walker = uia.RawViewWalker().ok()?;
                let mut ch = walker.GetFirstChildElement(el).ok();
                while let Some(c) = ch {
                    if let Some(v) = walk(uia, &c) {
                        return Some(v);
                    }
                    ch = walker.GetNextSiblingElement(&c).ok();
                }
                None
            }
            let out = walk(&uia, &root);
            CoUninitialize();
            Ok(out)
        }
    }

    pub fn ime_japanese(hwnd: HWND, editor: &str, keys: &str) -> Result<String> {
        // record the BEFORE value AND submit counter — a commit must extend
        // the text, never replace it, and MUST NOT submit
        let before = uia_value(hwnd, editor)?.unwrap_or_default();
        let submits = turn_counter(hwnd)?.unwrap_or(0);
        ime_japanese_inner(hwnd, editor, keys, &before, submits)
    }

    pub fn ime_japanese_inner(
        hwnd: HWND,
        editor: &str,
        keys: &str,
        before: &str,
        submits_before: u64,
    ) -> Result<String> {
        // Microsoft Japanese IME — CLSID + keyboard profile GUID
        let clsid = GUID::from_u128(0x03b5835f_f03c_411b_9ce2_aa23e1171e36);
        let profile = GUID::from_u128(0xa76c93d9_5523_4e90_aafa_4db112f9ac76);
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let mgr: ITfInputProcessorProfiles =
                CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_ALL)?;
            mgr.ChangeCurrentLanguage(0x411)?; // LANG_JAPANESE
            mgr.ActivateLanguageProfile(&clsid, 0x411, &profile)?;
            // open the target window's IMC in hiragana native mode
            let himc = ImmGetContext(hwnd);
            let opened = ImmSetOpenStatus(himc, true);
            let conv = ImmSetConversionStatus(
                himc,
                IME_CMODE_NATIVE | IME_CMODE_FULLSHAPE | IME_CMODE_ROMAN,
                IME_SENTENCE_MODE(0),
            );
            // NOTE: cross-process ImmGetContext returns NULL (IMC is
            // per-thread); under TSF the real preedit lives in the target's
            // TSF stack anyway — keystrokes below exercise THAT path.
            foreground(hwnd);
            std::thread::sleep(std::time::Duration::from_millis(300));
            // force the IME open in hiragana mode — VK_DBE_HIRAGANA is the
            // native MS-IME input-mode switch for the focused thread
            let mut k = INPUT::default();
            k.r#type = INPUT_KEYBOARD;
            k.Anonymous.ki.wVk = VIRTUAL_KEY(0xF2); // VK_DBE_HIRAGANA
            let _ = SendInput(&[k], std::mem::size_of::<INPUT>() as i32);
            k.Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;
            let _ = SendInput(&[k], std::mem::size_of::<INPUT>() as i32);
            std::thread::sleep(std::time::Duration::from_millis(300));
            // real keystrokes — the OS IME turns ASCII into hiragana preedit
            for ch in keys.chars() {
                let mut inp = INPUT::default();
                inp.r#type = INPUT_KEYBOARD;
                inp.Anonymous.ki.wVk = VIRTUAL_KEY(ch.to_ascii_uppercase() as u16);
                let _ = SendInput(&[inp], std::mem::size_of::<INPUT>() as i32);
                inp.Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;
                let _ = SendInput(&[inp], std::mem::size_of::<INPUT>() as i32);
                std::thread::sleep(std::time::Duration::from_millis(120));
            }
            // Enter commits the composition
            let mut enter = INPUT::default();
            enter.r#type = INPUT_KEYBOARD;
            enter.Anonymous.ki.wVk = VK_RETURN;
            let _ = SendInput(&[enter], std::mem::size_of::<INPUT>() as i32);
            enter.Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;
            let _ = SendInput(&[enter], std::mem::size_of::<INPUT>() as i32);
            std::thread::sleep(std::time::Duration::from_millis(600));
            // restore English — drop the COM object BEFORE CoUninitialize
            let _ = mgr.ChangeCurrentLanguage(0x409);
            drop(mgr);
            CoUninitialize();
            // ASSERT, don't print — every leg is an exact delta:
            //   before == committed prefix, gained == new committed text,
            //   gained nonempty AND contains new kana, submit counter
            //   unchanged (a real observable signal, not prefix inference).
            let after = uia_value(hwnd, editor)?.unwrap_or_default();
            if !after.starts_with(before) {
                return Err(Error::new(
                    E_FAIL.into(),
                    format!(
                        "IME commit broke the draft contract: before={before:?} after={after:?}"
                    ),
                ));
            }
            let gained = &after[before.len()..];
            if gained.is_empty() {
                return Err(Error::new(
                    E_FAIL.into(),
                    "IME acceptance failed: nothing committed",
                ));
            }
            // the GAINED suffix must carry real kana — searching `after`
            // would pass on pre-existing text
            let kana = gained
                .chars()
                .filter(|c| ('\u{3040}'..='\u{30ff}').contains(c))
                .count();
            if kana == 0 {
                return Err(Error::new(
                    E_FAIL.into(),
                    format!("IME acceptance failed: gained {gained:?} has no kana"),
                ));
            }
            let submits_after = turn_counter(hwnd)?.unwrap_or(0);
            if submits_after != submits_before {
                return Err(Error::new(
                    E_FAIL.into(),
                    format!(
                        "IME committed THROUGH Enter: submit count {submits_before} -> {submits_after}"
                    ),
                ));
            }
            Ok(format!(
                "{{\"kind\":\"ime\",\"evidence\":\"acceptance\",\"keys\":\"{keys}\",\"before\":\"{}\",\"gained\":\"{}\",\"submits\":{}}}",
                before.replace('"', "\\\""),
                gained.replace('"', "\\\""),
                submits_after,
            ))
        }
    }
}

#[cfg(windows)]
fn main() {
    use probe::*;
    set_pmv2();
    // UTF-8 stdout — kana/unicode values must serialize correctly through
    // the PowerShell capture pipe; the OEM codepage mangles them
    unsafe {
        windows::Win32::System::Console::SetConsoleOutputCP(65001);
    }
    let args: Vec<String> = std::env::args().collect();
    let usage = concat!(
        "usage: native_probe <sub> <window> [args...]
",
        "  window = title substring | 0xHWND | decimal HWND
",
        "  subs: geometry uia uia-rect uia-enabled uia-count value invoke
",
        "        click click-post click-named type type-post type-post-hex key-post
",
        "        dpichange ime"
    );
    if args.len() < 3 {
        eprintln!("{usage}");
        std::process::exit(2);
    }
    let hwnd = match resolve_window(&args[2]) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("{{\"error\":\"window '{}' not found: {e}\"}}", args[2]);
            std::process::exit(2);
        }
    };
    let res: std::result::Result<String, windows::core::Error> = match args[1].as_str() {
        "geometry" => geometry(hwnd),
        "uia" => uia_tree(hwnd),
        "click" => {
            let cx: i32 = args[3].parse().unwrap_or(0);
            let cy: i32 = args[4].parse().unwrap_or(0);
            click(hwnd, cx, cy).map(|_| {
                format!(
                    "{{\"kind\":\"click\",\"evidence\":\"acceptance\",\"client_px\":[{cx},{cy}]}}"
                )
            })
        }
        "type" => type_text(hwnd, &args[3]).map(|_| {
            format!(
                "{{\"kind\":\"type\",\"evidence\":\"acceptance\",\"text\":\"{}\"}}",
                args[3]
            )
        }),
        "click-post" => {
            let cx: i32 = args[3].parse().unwrap_or(0);
            let cy: i32 = args[4].parse().unwrap_or(0);
            click_post(hwnd, cx, cy).map(|_| {
                format!(
                    "{{\"kind\":\"click-post\",\"evidence\":\"regression\",\"client_px\":[{cx},{cy}]}}"
                )
            })
        }
        "type-post" => type_post(hwnd, &args[3]).map(|_| {
            format!(
                "{{\"kind\":\"type-post\",\"evidence\":\"regression\",\"text\":\"{}\"}}",
                args[3]
            )
        }),
        // unicode payload as HEX-encoded UTF-8 — argv is unsafe for
        // non-ASCII text under Windows PowerShell 5.1 (native argv is
        // ANSI-encoded AND a BOM-less .ps1 is parsed as ANSI); hex is
        // ASCII-safe end to end
        "type-post-hex" => {
            let hexs = args[3].trim();
            let mut bytes = Vec::with_capacity(hexs.len() / 2);
            let mut ok = true;
            for pair in hexs.as_bytes().chunks(2) {
                match std::str::from_utf8(pair).ok().and_then(|h| u8::from_str_radix(h, 16).ok()) {
                    Some(b) => bytes.push(b),
                    None => { ok = false; break; }
                }
            }
            match String::from_utf8(bytes) {
                Ok(text) if ok => type_post(hwnd, &text).map(|_| {
                    "{\"kind\":\"type-post\",\"evidence\":\"regression\",\"via\":\"hex\"}".to_string()
                }),
                _ => Err(windows::core::Error::new(
                    windows::Win32::Foundation::E_FAIL.into(),
                    "type-post-hex: malformed hex or non-UTF-8 payload",
                )),
            }
        }
        "key-post" => {
            let vk: u32 = args[3]
                .trim_start_matches("0x")
                .parse()
                .or_else(|_| u32::from_str_radix(args[3].trim_start_matches("0x"), 16))
                .unwrap_or(0);
            let ctrl = args.get(4).is_some_and(|a| a == "ctrl" || a == "1");
            let shift = args.get(4).is_some_and(|a| a == "shift" || a == "2")
                || args.get(5).is_some_and(|a| a == "shift");
            key_post(hwnd, vk, ctrl, shift).map(|_| {
                format!("{{\"kind\":\"key-post\",\"evidence\":\"regression\",\"vk\":{vk}}}")
            })
        }
        "uia-rect" => uia_rect(hwnd, &args[3]),
        "uia-enabled" => uia_enabled(hwnd, &args[3]),
        "uia-count" => uia_count(hwnd),
        "invoke" => invoke(hwnd, &args[3]),
        "click-named" => click_named(hwnd, &args[3]),
        "value" => uia_value(hwnd, &args[3]).map(|v| {
            format!(
                "{{\"kind\":\"value\",\"evidence\":\"acceptance\",\"name\":\"{}\",\"value\":{}}}",
                args[3],
                v.map(|s| format!("\"{}\"", s.replace('"', "\\\"")))
                    .unwrap_or("null".into())
            )
        }),
        "dpichange" => {
            let dpi: u32 = args[3].parse().unwrap_or(96);
            let pid = unsafe {
                let mut p = 0u32;
                windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(
                    hwnd,
                    Some(&mut p),
                );
                p
            };
            dpi_changed(hwnd, pid, dpi)
                .map(|_| "{\"kind\":\"dpichange\",\"evidence\":\"regression\"}".to_string())
        }
        "ime" => ime_japanese(hwnd, &args[3], &args[4]),
        other => {
            eprintln!("unknown subcommand '{other}'\n{usage}");
            std::process::exit(2);
        }
    };
    match res {
        Ok(json) => {
            println!("{json}");
        }
        Err(e) => {
            eprintln!("{{\"error\":\"{e}\"}}");
            std::process::exit(1);
        }
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("native_probe is Windows-only");
    std::process::exit(2);
}
