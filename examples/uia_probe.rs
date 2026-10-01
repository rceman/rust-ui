//! External UIA client probe — real cross-process UI Automation client:
//! raw tree walk + pairwise element comparison + runtime-id dump.

#[cfg(windows)]
fn main() {
    use std::ffi::c_void;
    use windows::Win32::Foundation::*;
    use windows::Win32::System::Com::*;
    use windows::Win32::UI::Accessibility::*;
    use windows::Win32::UI::WindowsAndMessaging::*;
    use windows::core::*;

    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let mut hwnd = HWND::default();
        extern "system" fn enum_cb(h: HWND, l: LPARAM) -> BOOL {
            let mut buf = [0u16; 256];
            let n = unsafe { GetWindowTextW(h, &mut buf) };
            unsafe {
                if String::from_utf16_lossy(&buf[..n as usize]).contains("rust-ui composer")
                    && IsWindowVisible(h).as_bool()
                {
                    *(l.0 as *mut HWND) = h;
                    return BOOL(0);
                }
            }
            BOOL(1)
        }
        let _ = EnumWindows(Some(enum_cb), LPARAM(&mut hwnd as *mut _ as isize));
        if hwnd.is_invalid() {
            eprintln!("composer window not found");
            std::process::exit(2);
        }
        let uia: IUIAutomation =
            CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL).expect("CUIAutomation");
        let root = uia.ElementFromHandle(hwnd).expect("element");
        // raw walker — enumerate every sibling, compare pairwise
        let walker = uia.RawViewWalker().expect("walker");
        let mut els: Vec<IUIAutomationElement> = Vec::new();
        let mut cur = walker.GetFirstChildElement(&root).ok();
        while let Some(el) = cur {
            els.push(el.clone());
            cur = walker.GetNextSiblingElement(&el).ok();
        }
        println!("walked: {}", els.len());
        // FindAll on the FRAGMENT ROOT element (child's Navigate(Parent))
        if let Some(first) = els.first() {
            let parent = walker.GetParentElement(first).ok();
            if let Some(pr) = parent {
                let kids = pr
                    .FindAll(TreeScope_Children, &uia.CreateTrueCondition().unwrap())
                    .expect("FindAll on frag root");
                println!("frag-root FindAll children: {}", kids.Length().unwrap_or(0));
                // and on the hwnd element for comparison
                let kids2 = root
                    .FindAll(TreeScope_Children, &uia.CreateTrueCondition().unwrap())
                    .expect("FindAll hwnd");
                println!("hwnd FindAll children: {}", kids2.Length().unwrap_or(0));
            }
        }
        for (i, el) in els.iter().enumerate() {
            let name = el.CurrentName().unwrap_or_default();
            let ct = el.CurrentControlType().map(|c| c.0).unwrap_or_default();
            let rid = el
                .GetRuntimeId()
                .and_then(|sa| safearray_i4(sa))
                .unwrap_or_default();
            let same_as_0 = i > 0
                && uia
                    .CompareElements(el, &els[0])
                    .map(|b| b.as_bool())
                    .unwrap_or(false);
            println!("  {i}: ct={ct} name='{name}' rid={rid:?} ==first:{same_as_0}");
        }
        CoUninitialize();
    }

    unsafe fn safearray_i4(sa: *const windows::Win32::System::Com::SAFEARRAY) -> Result<Vec<i32>> {
        use windows::Win32::System::Ole::*;
        if sa.is_null() {
            return Ok(vec![]);
        }
        let lb = SafeArrayGetLBound(sa, 1)?;
        let ub = SafeArrayGetUBound(sa, 1)?;
        let mut out = Vec::new();
        for i in lb..=ub {
            let mut v = 0i32;
            SafeArrayGetElement(sa, &i, &mut v as *mut i32 as *mut c_void)?;
            out.push(v);
        }
        Ok(out)
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("uia_probe is Windows-only");
}
