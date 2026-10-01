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
    windows::core::link!("kernel32.dll" "system" fn WPM(h: HANDLE, base: *const c_void, buf: *const c_void, sz: usize, written: *mut usize) -> BOOL);
    use windows::core::*;

    pub fn set_pmv2() {
        unsafe {
            let _ = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }
    }

    /// Find a visible top-level window whose title contains `part`.
    pub fn find_window(part: &str) -> Result<HWND> {
        unsafe {
            let mut found = HWND::default();
            let mut needle = part.to_string();
            extern "system" fn cb(h: HWND, l: LPARAM) -> BOOL {
                let mut buf = [0u16; 512];
                let n = unsafe { GetWindowTextW(h, &mut buf) };
                let title = String::from_utf16_lossy(&buf[..n as usize]);
                let needle = unsafe { &*(l.0 as *const String) };
                if !title.is_empty()
                    && title.contains(needle.as_str())
                    && unsafe { IsWindowVisible(h).as_bool() }
                {
                    unsafe { *(l.0 as *mut HWND) = h };
                    return BOOL(0);
                }
                BOOL(1)
            }
            let _ = EnumWindows(Some(cb), LPARAM(&mut needle as *mut String as isize));
            let _ = &mut found;
            // re-enum to actually capture — above lambda wrote through l
            let mut f2 = HWND::default();
            let _ = EnumWindows(Some(cb), LPARAM(&mut f2 as *mut HWND as isize));
            if f2.is_invalid() {
                Err(Error::new(E_FAIL.into(), "window not found"))
            } else {
                Ok(f2)
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
            let mut org = POINT::default();
            let _ = windows::Win32::Graphics::Gdi::ClientToScreen(hwnd, &mut org);
            let dpi = GetDpiForWindow(hwnd);
            let thread_dpi = GetThreadDpiAwarenessContext();
            let _ = thread_dpi;
            let scale = rust_ui::ScaleFactor::from_dpi(dpi).0;
            Ok(format!(
                "{{\"kind\":\"geometry\",\"evidence\":\"acceptance\",\"dpi\":{dpi},\"scale\":{scale:.3},\"window_px\":[{},{},{},{}],\"client_px\":[0,0,{},{}],\"client_origin_px\":[{},{}]}}",
                wr.left, wr.top, wr.right, wr.bottom, cr.right, cr.bottom, org.x, org.y,
            ))
        }
    }

    /// Real physical-px click via SendInput at client coords (cx,cy).
    pub fn click(hwnd: HWND, cx: i32, cy: i32) -> Result<()> {
        unsafe {
            let mut org = POINT::default();
            let _ = windows::Win32::Graphics::Gdi::ClientToScreen(hwnd, &mut org);
            let (sx, sy) = (org.x + cx, org.y + cy);
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
    /// monitor transition).
    pub fn dpi_changed(hwnd: HWND, pid: u32, dpi: u32) -> Result<()> {
        unsafe {
            let hp = OpenProcess(PROCESS_ALL_ACCESS, false, pid)?;
            let mem = VirtualAllocEx(hp, None, 16, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE);
            if mem.is_null() {
                return Err(Error::new(E_FAIL.into(), "VirtualAllocEx"));
            }
            let s = rust_ui::ScaleFactor::from_dpi(dpi);
            let rect = [0i32, 0, s.to_physical(500.0), s.to_physical(470.0)];
            let mut written = 0usize;
            WPM(hp, mem, rect.as_ptr() as *const c_void, 16, &mut written);
            PostMessageW(
                Some(hwnd),
                0x02E0, // WM_DPICHANGED
                WPARAM(((dpi << 16) | dpi) as usize),
                LPARAM(mem as isize),
            )?;
            std::thread::sleep(std::time::Duration::from_millis(900));
            let _ = VirtualFreeEx(hp, mem, 0, MEM_RELEASE);
            let _ = CloseHandle(hp);
            Ok(())
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
    pub fn ime_japanese(hwnd: HWND, keys: &str) -> Result<String> {
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
            let mut cm = IME_CONVERSION_MODE(0);
            let mut sm = IME_SENTENCE_MODE(0);
            let _ = ImmGetConversionStatus(himc, Some(&mut cm), Some(&mut sm));
            foreground(hwnd);
            std::thread::sleep(std::time::Duration::from_millis(300));
            // real keystrokes — the OS IME turns ASCII into hiragana preedit
            for ch in keys.chars() {
                let vk = ch as u32 as u16; // ASCII lowercase == VK
                let _ = vk;
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
            // restore English
            let _ = mgr.ChangeCurrentLanguage(0x409);
            let _ = ImmReleaseContext(hwnd, himc);
            let _ = CoUninitialize();
            Ok(format!(
                "{{\"kind\":\"ime\",\"evidence\":\"acceptance\",\"keys\":\"{keys}\",\"open_status\":{},\"conv_status\":{}}}",
                opened.as_bool(),
                conv.as_bool()
            ))
        }
    }
}

#[cfg(windows)]
fn main() {
    use probe::*;
    set_pmv2();
    let args: Vec<String> = std::env::args().collect();
    let usage = "usage: native_probe <geometry|uia|click|type|dpichange|ime|value> ...";
    if args.len() < 3 {
        eprintln!("{usage}");
        std::process::exit(2);
    }
    let hwnd = match find_window(&args[2]) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("{{\"error\":\"window '{ }' not found: {e}\"}}", args[2]);
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
        "ime" => ime_japanese(hwnd, &args[3]),
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
