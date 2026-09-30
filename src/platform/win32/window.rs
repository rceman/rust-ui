//! HWND lifecycle + WndProc: DPI-aware class registration, userdata routing
//! to the pinned `Backend`, deadline/native timers, caret plumbing, IME
//! routing, tooltip/capture, `WM_GETOBJECT` for UIA, and clean shutdown.

use std::sync::atomic::Ordering;

use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Accessibility::*;
use windows::Win32::UI::Controls::WM_MOUSELEAVE;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::*;

use crate::runtime::UpdateCtx;
use crate::{Ui, UiResult};

use super::{Backend, WM_PUMP, WM_UIA_FOCUS, WM_UIA_PRESS};

pub(super) const DEADLINE_TIMER: usize = 1;
const NATIVE_TIMER_BASE: usize = 0x4000_0000;

pub(super) mod wndproc {
    use super::*;

    pub(super) unsafe extern "system" fn trampoline<S, M, U, V>(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT
    where
        M: 'static,
        U: Fn(&mut S, M, &mut UpdateCtx<'_, M>),
        V: Fn(&S, &mut Ui<'_, '_, M>),
    {
        unsafe {
            let ptr = if msg == WM_NCCREATE {
                let cs = &*(lparam.0 as *const CREATESTRUCTW);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
                cs.lpCreateParams as *mut Backend<S, M, U, V>
            } else {
                GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Backend<S, M, U, V>
            };
            if ptr.is_null() {
                return DefWindowProcW(hwnd, msg, wparam, lparam);
            }
            let be = &mut *ptr;
            match dispatch(hwnd, msg, wparam, lparam, be) {
                Ok(r) => r,
                Err(e) => {
                    be.mark_fatal(e);
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
            }
        }
    }

    fn dispatch<S, M, U, V>(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        be: &mut Backend<S, M, U, V>,
    ) -> UiResult<LRESULT>
    where
        M: 'static,
        U: Fn(&mut S, M, &mut UpdateCtx<'_, M>),
        V: Fn(&S, &mut Ui<'_, '_, M>),
    {
        Ok(match msg {
            WM_PUMP => {
                be.turn()?;
                LRESULT(0)
            }
            WM_PAINT => {
                unsafe {
                    let _ = ValidateRect(Some(hwnd), None);
                }
                be.paint()?;
                LRESULT(0)
            }
            WM_MOUSEMOVE => {
                unsafe {
                    let _ = TrackMouseEvent(&mut TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: hwnd,
                        dwHoverTime: 0,
                    });
                }
                be.update_cursor();
                be.hover(be.pt(lparam))?;
                LRESULT(0)
            }
            WM_MOUSELEAVE => {
                be.leave()?;
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                unsafe {
                    SetCapture(hwnd);
                }
                let pt = be.pt(lparam);
                be.pointer(
                    crate::node::PointerPhase::Down,
                    pt,
                    Some(crate::PointerButton::Primary),
                )?;
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                unsafe {
                    let _ = ReleaseCapture();
                }
                let pt = be.pt(lparam);
                be.pointer(
                    crate::node::PointerPhase::Up,
                    pt,
                    Some(crate::PointerButton::Primary),
                )?;
                LRESULT(0)
            }
            WM_CHAR | WM_SYSCHAR => {
                be.char(wparam.0 as u32)?;
                LRESULT(0)
            }
            WM_KEYDOWN => {
                // Space/Enter on a button = Press; everything else routes
                be.key(wparam.0 as u32)?;
                LRESULT(0)
            }
            WM_IME_STARTCOMPOSITION => {
                be.ime_start();
                LRESULT(0)
            }
            WM_IME_ENDCOMPOSITION => {
                be.ime_end()?;
                LRESULT(0)
            }
            WM_IME_COMPOSITION => {
                // richedit sees the raw message; preedit never becomes an edit
                be.send_focused(msg, wparam.0, lparam.0);
                be.service_peer_events()?;
                LRESULT(0)
            }
            WM_IME_NOTIFY => {
                be.send_focused(msg, wparam.0, lparam.0);
                be.service_peer_events()?;
                LRESULT(0)
            }
            WM_SETFOCUS => {
                be.send_focused(msg, wparam.0, lparam.0);
                be.turn()?;
                LRESULT(0)
            }
            WM_KILLFOCUS => {
                be.send_focused(msg, wparam.0, lparam.0);
                be.turn()?;
                LRESULT(0)
            }
            WM_MOUSEWHEEL => {
                be.send_focused(msg, wparam.0, lparam.0);
                be.service_peer_events()?;
                LRESULT(0)
            }
            WM_SIZE => {
                be.relayout()?;
                be.paint()?;
                LRESULT(0)
            }
            WM_DPICHANGED => {
                unsafe {
                    let rc = &*(lparam.0 as *const RECT);
                    let _ = SetWindowPos(
                        hwnd,
                        None,
                        rc.left,
                        rc.top,
                        rc.right - rc.left,
                        rc.bottom - rc.top,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
                be.dpi_changed((wparam.0 & 0xffff) as u32)?;
                LRESULT(0)
            }
            WM_SETTINGCHANGE => {
                be.os_change()?;
                LRESULT(0)
            }
            WM_TIMER => {
                if wparam.0 == DEADLINE_TIMER {
                    unsafe {
                        let _ = KillTimer(Some(hwnd), DEADLINE_TIMER);
                    }
                    be.deadline_fire()?;
                } else if wparam.0 >= NATIVE_TIMER_BASE {
                    let tid = wparam.0 - NATIVE_TIMER_BASE;
                    be.native_timer_fire(tid)?;
                }
                LRESULT(0)
            }
            WM_GETOBJECT => {
                if lparam.0 as i64 == UiaRootObjectId as i64
                    && let Some(root) = be.uia_provider()?
                {
                    return Ok(unsafe {
                        UiaReturnRawElementProvider(
                            hwnd,
                            WPARAM(wparam.0),
                            LPARAM(lparam.0),
                            &root.cast::<IRawElementProviderSimple>()?,
                        )
                    });
                }
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WM_UIA_PRESS => {
                be.uia_action(wparam.0 as u32, lparam.0 as u64, false)?;
                LRESULT(0)
            }
            WM_UIA_FOCUS => {
                be.uia_action(wparam.0 as u32, lparam.0 as u64, true)?;
                LRESULT(0)
            }
            WM_NCDESTROY => {
                // fence the wake seam BEFORE the HWND dies
                be.closed.store(true, Ordering::SeqCst);
                be.rt.mailbox.close();
                be.shutdown();
                unsafe {
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                    PostQuitMessage(0);
                }
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        })
    }
}

/// Register the window class once and create the main HWND — the route
/// pointer rides lpCreateParams so `WM_NCCREATE` installs userdata before
/// any message dispatch.
pub(super) fn create<S, M, U, V>(title: &str, route: *mut Backend<S, M, U, V>) -> UiResult<HWND>
where
    M: 'static,
    U: Fn(&mut S, M, &mut UpdateCtx<'_, M>),
    V: Fn(&S, &mut crate::Ui<'_, '_, M>),
{
    unsafe {
        // DPI awareness is set once in run() (PerMonitorV2) — calling the
        // V1 API here would downgrade the context
        let inst = GetModuleHandleW(None)
            .map_err(|e| crate::UiError::Platform(format!("GetModuleHandleW: {e}")))?;
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc::trampoline::<S, M, U, V>),
            hInstance: HINSTANCE(inst.0),
            lpszClassName: w!("RustUiWindow"),
            style: CS_DBLCLKS,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        };
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("RustUiWindow"),
            PCWSTR(HSTRING::from(title).as_ptr()),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            980,
            900,
            None,
            None,
            Some(inst.into()),
            Some(route as *const std::ffi::c_void),
        )
        .map_err(|e| crate::UiError::Platform(format!("CreateWindowExW: {e}")))?;
        let _ = ShowWindow(hwnd, SW_SHOW);
        Ok(hwnd)
    }
}
