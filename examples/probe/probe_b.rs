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
                "{{\"kind\":\"uia-rect\",\"name\":{},\"rect_px\":[{},{},{},{}]}}",
                crate::json_str(name),
                r.left,
                r.top,
                r.right,
                r.bottom
            ))
        }
    }

    /// UIA bounding rect of an element whose NAME starts with `prefix` —
    /// the only honest lookup for dynamic label text (turn/draft-echo/
    /// draft-edits change every run). Returns name + rect.
    pub fn uia_rect_prefix(hwnd: HWND, prefix: &str) -> Result<String> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let uia: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL)?;
            let root = uia.ElementFromHandle(hwnd)?;
            unsafe fn find(
                uia: &IUIAutomation,
                el: &IUIAutomationElement,
                prefix: &str,
            ) -> Option<IUIAutomationElement> {
                let n = el.CurrentName().unwrap_or_default().to_string();
                if n.starts_with(prefix) {
                    return Some(el.clone());
                }
                let walker = uia.RawViewWalker().ok()?;
                let mut ch = walker.GetFirstChildElement(el).ok();
                while let Some(c) = ch {
                    if let Some(f) = find(uia, &c, prefix) {
                        return Some(f);
                    }
                    ch = walker.GetNextSiblingElement(&c).ok();
                }
                None
            }
            let el = find(&uia, &root, prefix)
                .ok_or_else(|| Error::new(E_FAIL.into(), "uia element with prefix not found"))?;
            let r = el.CurrentBoundingRectangle()?;
            let n = el.CurrentName().unwrap_or_default().to_string();
            CoUninitialize();
            Ok(format!(
                "{{\"kind\":\"uia-rect-prefix\",\"name\":{},\"rect_px\":[{},{},{},{}]}}",
                crate::json_str(&n),
                r.left,
                r.top,
                r.right,
                r.bottom
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
                "{{\"kind\":\"uia-enabled\",\"name\":{},\"enabled\":{en}}}",
                crate::json_str(name)
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
    /// UIA trees rebuild on state flips, so a single find can land a stale
    /// fenced element mid-refresh — one settle+retry is the honest retry
    /// boundary (a second failure is reported with the REAL element state).
    pub fn invoke(hwnd: HWND, name: &str) -> Result<String> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let uia: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL)?;
            let mut last_err = None;
            for attempt in 0..2 {
                let root = uia.ElementFromHandle(hwnd)?;
                let Some(el) = uia_find(&uia, &root, name) else {
                    last_err = Some(Error::new(E_FAIL.into(), "uia element not found"));
                    std::thread::sleep(std::time::Duration::from_millis(300));
                    continue;
                };
                // disabled may be a stale element mid-refresh — settle and
                // re-find before declaring the contract state
                if !el.CurrentIsEnabled().map(|b| b.as_bool()).unwrap_or(false) {
                    if attempt == 0 {
                        std::thread::sleep(std::time::Duration::from_millis(300));
                        continue;
                    }
                    return Err(Error::new(
                        E_FAIL.into(),
                        format!("uia element '{name}' is disabled"),
                    ));
                }
                match el
                    .GetCurrentPattern(UIA_InvokePatternId)
                    .and_then(|p| p.cast::<IUIAutomationInvokePattern>())
                    .and_then(|inv| inv.Invoke())
                {
                    Ok(()) => {
                        std::thread::sleep(std::time::Duration::from_millis(200));
                        CoUninitialize();
                        return Ok(format!(
                            "{{\"kind\":\"invoke\",\"evidence\":\"acceptance\",\"name\":{}}}",
                            crate::json_str(name)
                        ));
                    }
                    Err(e) => {
                        last_err = Some(e);
                        std::thread::sleep(std::time::Duration::from_millis(300));
                    }
                }
            }
            CoUninitialize();
            Err(last_err.unwrap_or_else(|| Error::new(E_FAIL.into(), "invoke failed")))
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
                "{{\"kind\":\"click-named\",\"evidence\":\"regression\",\"name\":{},\"client_px\":[{},{}]}}",
                crate::json_str(name),
                pt.0.x,
                pt.0.y
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
                    "{{\"depth\":{},\"ct\":{},\"name\":{},\"rect_px\":[{:.0},{:.0},{:.0},{:.0}],\"enabled\":{},\"focusable\":{}}}",
                    depth, ct,
                    crate::json_str(&name),
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

