use super::super::*;
use super::guards::{box_geometry, brush_color, resolve_render};
use super::*;

impl Renderer {
    pub(crate) fn show_tooltip(
        &mut self,
        text: &str,
        anchor: HWND,
        scale: super::super::space::ScaleFactor,
    ) {
        let hwnd = match self.tip.borrow().as_ref() {
            Some(t) => t.hwnd,
            None => self.create_tip(text),
        };
        unsafe {
            let (w, h) = measure_text(
                text,
                400.0,
                &crate::style::TextStyle {
                    size: crate::style::TextSize::Exact(crate::geom::Dp(13.0)),
                    ..Default::default()
                },
            )
            .unwrap_or((80.0, 20.0));
            let (pw, ph) = (w + 16.0, h + 10.0);
            // position: centered under the anchor's first editor rect —
            // callers pass the node rect through anchor client space; we
            // place it just below the rect in screen space
            let mut rc = RECT::default();
            let _ = GetClientRect(anchor, &mut rc);
            let mut pt = POINT { x: 0, y: rc.bottom };
            let _ = ClientToScreen(anchor, &mut pt);
            let x = pt.x;
            let y = pt.y + 4;
            let mut tip_rc = RECT {
                left: 0,
                top: 0,
                right: scale.to_physical(pw),
                bottom: scale.to_physical(ph),
            };
            let _ = AdjustWindowRect(&mut tip_rc, WS_POPUP, false);
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                x,
                y,
                scale.to_physical(pw),
                scale.to_physical(ph),
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
            let _ = InvalidateRect(Some(hwnd), None, false);
        }
        *self.tip_fade_start.borrow_mut() = Some(std::time::Instant::now());
    }

    pub(crate) fn create_tip(&self, text: &str) -> HWND {
        TIP_CLASS_REGISTERED.get_or_init(|| unsafe {
            let inst = GetModuleHandleW(None).unwrap();
            let wc = WNDCLASSW {
                lpfnWndProc: Some(tip_wndproc),
                hInstance: HINSTANCE(inst.0),
                lpszClassName: w!("RustUiTooltip"),
                ..Default::default()
            };
            let _ = RegisterClassW(&wc);
        });
        unsafe {
            let inst = GetModuleHandleW(None).unwrap();
            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_LAYERED,
                w!("RustUiTooltip"),
                PCWSTR::null(),
                WS_POPUP,
                0,
                0,
                0,
                0,
                None,
                None,
                Some(inst.into()),
                None,
            )
            .unwrap_or_default();
            *self.tip.borrow_mut() = Some(Tip {
                hwnd,
                target: None,
                factory: Some(self.factory.clone()),
                text: text.to_string(),
            });
            hwnd
        }
    }

    pub(crate) fn hide_tooltip(&mut self) {
        if let Some(t) = self.tip.borrow().as_ref() {
            unsafe {
                let _ = ShowWindow(t.hwnd, SW_HIDE);
            }
        }
    }

    /// Tooltip paint (its own target — inverse shadcn chrome).
    pub(crate) fn tip_paint(hwnd: HWND) {
        unsafe {
            let _ = ValidateRect(Some(hwnd), None);
        }
    }
}

unsafe extern "system" fn tip_wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            // minimal chrome: inverse neutral bg + fg text via GDI text — the
            // tip is chrome-only (never a native text island)
            unsafe {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rc = RECT::default();
                let _ = GetClientRect(hwnd, &mut rc);
                let bg = CreateSolidBrush(COLORREF(0x00232323)); // near-black
                FillRect(hdc, &rc, bg);
                let _ = DeleteObject(bg.into());
                let _ = SetTextColor(hdc, COLORREF(0x00fafafa));
                let _ = SetBkMode(hdc, TRANSPARENT);
                let _ = EndPaint(hwnd, &ps);
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wp, lp) },
    }
}
