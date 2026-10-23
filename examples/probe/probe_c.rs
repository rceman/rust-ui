    pub fn uia_label_text(hwnd: HWND, prefix: &str) -> Result<Option<String>> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let uia: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL)?;
            let root = uia.ElementFromHandle(hwnd)?;
            unsafe fn walk(
                uia: &IUIAutomation,
                el: &IUIAutomationElement,
                prefix: &str,
            ) -> Option<String> {
                let n = el.CurrentName().unwrap_or_default().to_string();
                if n.starts_with(prefix) {
                    return Some(n);
                }
                let walker = uia.RawViewWalker().ok()?;
                let mut ch = walker.GetFirstChildElement(el).ok();
                while let Some(c) = ch {
                    if let Some(v) = walk(uia, &c, prefix) {
                        return Some(v);
                    }
                    ch = walker.GetNextSiblingElement(&c).ok();
                }
                None
            }
            let out = walk(&uia, &root, prefix);
            CoUninitialize();
            Ok(out)
        }
    }

    /// Observable submit signal — the composer's `turn N` label is the
    /// real submit counter (Send increments it; a label's name IS its
    /// text). The label lives inside the details group — the scenario
    /// must expand details first; a MISSING signal is a test failure,
    /// not a zero.
    pub fn turn_counter(hwnd: HWND) -> Result<Option<u64>> {
        Ok(uia_label_text(hwnd, "turn ")?.and_then(|n| {
            n.strip_prefix("turn ")
                .and_then(|rest| rest.split('|').next())
                .and_then(|t| t.trim().parse().ok())
        }))
    }

    pub fn ime_japanese(hwnd: HWND, editor: &str, keys: &str) -> Result<String> {
        // record the BEFORE value AND submit counter — a commit must extend
        // the text, never replace it, and MUST NOT submit. The counter is
        // a REQUIRED observable signal: absent = failure, not zero.
        let before = uia_value(hwnd, editor)?.unwrap_or_default();
        let submits = turn_counter(hwnd)?.ok_or_else(|| {
            Error::new(
                E_FAIL.into(),
                "turn counter label not visible — expand details before the IME scenario",
            )
        })?;
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
            // The COMPOSER thread's input context is what matters — TSF
            // activation is per-thread, so ask the target window to take
            // the Japanese layout itself. WM_INPUTLANGCHANGEREQUEST
            // (handled by DefWindowProc on the receiving thread) activates
            // the posted HKL for THAT thread; the HKL is discovered from
            // the loaded-layout list by langid, never hardcoded.
            let target_hkl = {
                let mut list = [HKL::default(); 16];
                let n = GetKeyboardLayoutList(Some(&mut list));
                let mut found = list[..n as usize]
                    .iter()
                    .copied()
                    .find(|h| (h.0 as u32 & 0xffff) == 0x0411);
                if found.is_none() {
                    let h = LoadKeyboardLayoutW(windows::core::w!("00000411"), KLF_NOTELLSHELL)?;
                    found = (h.0 as u32 & 0xffff == 0x411).then_some(h);
                }
                found.ok_or_else(|| {
                    Error::new(E_FAIL.into(), "no Japanese keyboard layout available")
                })?
            };
            foreground(hwnd);
            std::thread::sleep(std::time::Duration::from_millis(300));
            // verified target-thread activation: post the HKL request,
            // CHECK the post, then bounded wait + READ-BACK of the
            // composer thread's actual keyboard layout until it reports
            // Japanese — an unverifiable activation is a typed failure,
            // never a silent assumption
            const WM_INPUTLANGCHANGEREQUEST: u32 = 0x0050;
            const INPUTLANGCHANGE_FORWARD: usize = 0x0002;
            PostMessageW(
                Some(hwnd),
                WM_INPUTLANGCHANGEREQUEST,
                WPARAM(INPUTLANGCHANGE_FORWARD),
                LPARAM(target_hkl.0 as isize),
            )
            .map_err(|_| Error::new(E_FAIL.into(), "WM_INPUTLANGCHANGEREQUEST post failed"))?;
            let composer_tid = GetWindowThreadProcessId(hwnd, None);
            let mut layout_on_target = false;
            for _ in 0..25 {
                let cur = GetKeyboardLayout(composer_tid);
                if (cur.0 as u32 & 0xffff) == 0x0411 {
                    layout_on_target = true;
                    break;
                }
                // re-post inside the wait: the first request can race a
                // still-starting target thread
                let _ = PostMessageW(
                    Some(hwnd),
                    WM_INPUTLANGCHANGEREQUEST,
                    WPARAM(INPUTLANGCHANGE_FORWARD),
                    LPARAM(target_hkl.0 as isize),
                );
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            if !layout_on_target {
                return Err(Error::new(
                    E_FAIL.into(),
                    "composer thread never activated the Japanese HKL",
                ));
            }
            // MS-IME PERSISTS its conversion mode in the profile — a
            // blind VK_DBE_HIRAGANA toggle can flip OFF a leftover
            // hiragana session, so it is NOT an acceptable fallback: the
            // deterministic route is the target thread's default IME
            // window — OPEN state + exact mode set + read-back verify,
            // typed failure when verification is unavailable.
            const WM_IME_CONTROL: u32 = 0x0283;
            const IMC_GETOPENSTATUS: usize = 0x0005;
            const IMC_GETCONVERSIONMODE: usize = 0x0003;
            const IMC_SETCONVERSIONMODE: usize = 0x0004;
            const IMC_SETOPENSTATUS: usize = 0x0006;
            let ime_wnd = windows::Win32::UI::Input::Ime::ImmGetDefaultIMEWnd(hwnd);
            if ime_wnd.is_invalid() {
                return Err(Error::new(
                    E_FAIL.into(),
                    "no default IME window on the composer thread — engagement unverifiable",
                ));
            }
            let ime_ctl = |w: windows::Win32::Foundation::HWND, op: usize, arg: isize| -> i32 {
                SendMessageW(w, WM_IME_CONTROL, Some(WPARAM(op)), Some(LPARAM(arg))).0 as i32
            };
            // open state: set + read back
            let mut open = false;
            for _ in 0..10 {
                let _ = ime_ctl(ime_wnd, IMC_SETOPENSTATUS, 1);
                if ime_ctl(ime_wnd, IMC_GETOPENSTATUS, 0) == 1 {
                    open = true;
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(150));
            }
            if !open {
                return Err(Error::new(E_FAIL.into(), "IME open state never engaged"));
            }
            // exact conversion mode: NATIVE|FULLSHAPE|ROMAN is the
            // hiragana contract — ALL required bits, not merely NATIVE
            let want_bits = (IME_CMODE_NATIVE | IME_CMODE_FULLSHAPE | IME_CMODE_ROMAN).0 as i32;
            let mut engaged = false;
            for _ in 0..10 {
                let _ = ime_ctl(ime_wnd, IMC_SETCONVERSIONMODE, want_bits as isize);
                let mode = ime_ctl(ime_wnd, IMC_GETCONVERSIONMODE, 0);
                if mode & want_bits == want_bits {
                    engaged = true;
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            if !engaged {
                return Err(Error::new(
                    E_FAIL.into(),
                    format!(
                        "hiragana mode did not engage: conversion mode {:#x} (want mask {:#x})",
                        ime_ctl(ime_wnd, IMC_GETCONVERSIONMODE, 0),
                        want_bits
                    ),
                ));
            }
            // every required input is asserted — SendInput returns the
            // number of events inserted; 0 means the call was blocked
            let send1 = |inp: &INPUT| -> Result<()> {
                let n = SendInput(
                    std::slice::from_ref(inp),
                    std::mem::size_of::<INPUT>() as i32,
                );
                if n != 1 {
                    return Err(Error::new(
                        E_FAIL.into(),
                        format!("SendInput inserted {n}/1 events"),
                    ));
                }
                Ok(())
            };
            // real keystrokes — the OS IME turns ASCII into hiragana preedit
            for ch in keys.chars() {
                let mut inp = INPUT::default();
                inp.r#type = INPUT_KEYBOARD;
                inp.Anonymous.ki.wVk = VIRTUAL_KEY(ch.to_ascii_uppercase() as u16);
                send1(&inp)?;
                inp.Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;
                send1(&inp)?;
                std::thread::sleep(std::time::Duration::from_millis(120));
            }
            // Enter commits the composition
            let mut enter = INPUT::default();
            enter.r#type = INPUT_KEYBOARD;
            enter.Anonymous.ki.wVk = VK_RETURN;
            send1(&enter)?;
            enter.Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;
            send1(&enter)?;
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
            let submits_after = turn_counter(hwnd)?.ok_or_else(|| {
                Error::new(
                    E_FAIL.into(),
                    "turn counter label disappeared during the IME run",
                )
            })?;
            if submits_after != submits_before {
                return Err(Error::new(
                    E_FAIL.into(),
                    format!(
                        "IME committed THROUGH Enter: submit count {submits_before} -> {submits_after}"
                    ),
                ));
            }
            // the APPLICATION's committed TextValue must equal the native
            // provider's value — the draft-echo label renders
            // `draft.text()`; agreement proves native and shared state
            // converge (a provider-only read could hide a lost commit).
            let echo = uia_label_text(hwnd, "draft-echo ")?.ok_or_else(|| {
                Error::new(
                    E_FAIL.into(),
                    "draft-echo label missing — the app committed value is unobservable",
                )
            })?;
            let app_committed = echo.trim_start_matches("draft-echo ").to_string();
            if app_committed != after {
                return Err(Error::new(
                    E_FAIL.into(),
                    format!("app committed {app_committed:?} != native provider value {after:?}"),
                ));
            }
            Ok(format!(
                "{{\"kind\":\"ime\",\"evidence\":\"acceptance\",\"keys\":{},\"before\":{},\"gained\":{},\"submits\":{}}}",
                crate::json_str(keys),
                crate::json_str(before),
                crate::json_str(gained),
                submits_after,
            ))
        }
    }
