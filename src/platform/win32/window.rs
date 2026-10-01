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
/// Peer-timer ID namespace — framework timers are small ints; every
/// richedit-requested timer is NATIVE_TIMER_BASE + n so the WM_TIMER route
/// is unambiguous forever.
pub(super) const NATIVE_TIMER_BASE: usize = 0x4000_0000;
/// bound on simultaneously live native timer ids
const NATIVE_TIMER_CAP: usize = 0x1000;

/// Map a monotonically increasing sequence to a native timer id.
pub(super) fn native_timer_id(seq: usize) -> usize {
    NATIVE_TIMER_BASE + (seq % NATIVE_TIMER_CAP)
}
/// WM_TIMER's wparam is a native peer timer iff it carries the base.
pub(super) fn is_native_timer_id(tid: usize) -> bool {
    tid >= NATIVE_TIMER_BASE && tid < NATIVE_TIMER_BASE + NATIVE_TIMER_CAP
}
/// recover the richedit-facing timer id from a win32 id — the map holds
/// the authoritative pair; this only strips the namespace offset
pub(super) fn native_timer_decode(tid: usize) -> Option<usize> {
    is_native_timer_id(tid).then_some(tid - NATIVE_TIMER_BASE)
}

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
            // SAFETY BOUNDARY — checked BEFORE any exclusive `&mut Backend`
            // exists: a synchronous native callback (msftedit's TxSetFocus
            // inside a peer send, UIA into WM_GETOBJECT, a timer inside a
            // call) re-enters this trampoline while the outer dispatch still
            // holds `be`. A second `&mut` here would be UB; the flag is read
            // through the raw pointer only — no borrow is formed.
            if (*ptr).in_dispatch.get() {
                return reentrant::<S, M, U, V>(hwnd, msg, wparam, lparam, ptr);
            }
            (*ptr).in_dispatch.set(true);
            let be = &mut *ptr;
            let out = match dispatch(hwnd, msg, wparam, lparam, be) {
                Ok(r) => r,
                Err(e) => {
                    be.mark_fatal(e);
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
            };
            be.in_dispatch.set(false);
            out
        }
    }

    /// A message arrived while the backend is mid-dispatch. An exclusive
    /// `&mut` can never be formed here — route by message class:
    ///
    /// - sync-result messages can't be deferred: `WM_GETOBJECT` gets the
    ///   OS default (clients retry; the next non-reentrant request serves
    ///   a complete provider);
    /// - teardown can't be deferred: `WM_NCDESTROY` runs only the pieces
    ///   that need no `&mut` — atomic close flag, mailbox close, route
    ///   pointer detach, quit post;
    /// - `WM_PAINT` is coalesced by definition — validate and let the
    ///   in-flight frame cover it;
    /// - every other routed message carries value-copied parameters and is
    ///   re-posted to the same queue — Win32 posted messages stay FIFO, so
    ///   the re-delivery runs after the in-flight call unwinds without
    ///   reordering ahead of unrelated queued input.
    unsafe fn reentrant<S, M, U, V>(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        ptr: *mut Backend<S, M, U, V>,
    ) -> LRESULT
    where
        M: 'static,
        U: Fn(&mut S, M, &mut UpdateCtx<'_, M>),
        V: Fn(&S, &mut Ui<'_, '_, M>),
    {
        unsafe {
            match msg {
                WM_GETOBJECT => DefWindowProcW(hwnd, msg, wparam, lparam),
                WM_NCDESTROY => {
                    // shared/atomic teardown only — no &mut Backend here
                    (*ptr).closed.store(true, Ordering::SeqCst);
                    (*ptr).rt.mailbox.close();
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                    PostQuitMessage(0);
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
                WM_PAINT => {
                    let _ = ValidateRect(Some(hwnd), None);
                    LRESULT(0)
                }
                WM_PUMP
                | WM_MOUSEMOVE
                | WM_MOUSELEAVE
                | WM_LBUTTONDOWN
                | WM_LBUTTONUP
                | WM_LBUTTONDBLCLK
                | WM_RBUTTONDOWN
                | WM_RBUTTONUP
                | WM_CHAR
                | WM_SYSCHAR
                | WM_KEYDOWN
                | WM_KEYUP
                | WM_SYSKEYDOWN
                | WM_SYSKEYUP
                | WM_IME_STARTCOMPOSITION
                | WM_IME_ENDCOMPOSITION
                | WM_IME_COMPOSITION
                | WM_IME_NOTIFY
                | WM_SETFOCUS
                | WM_KILLFOCUS
                | WM_MOUSEWHEEL
                | WM_SIZE
                | WM_DPICHANGED
                | WM_SETTINGCHANGE
                | WM_TIMER
                | WM_UIA_PRESS
                | WM_UIA_FOCUS => {
                    let _ = PostMessageW(Some(hwnd), msg, wparam, lparam);
                    LRESULT(0)
                }
                _ => DefWindowProcW(hwnd, msg, wparam, lparam),
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
                    wparam.0,
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
                    wparam.0,
                )?;
                LRESULT(0)
            }
            WM_CHAR | WM_SYSCHAR => {
                // full (msg,wparam,lparam) — the repeat/scan/alt flags in
                // lparam are part of the native contract
                be.char_msg(msg, wparam.0, lparam.0)?;
                LRESULT(0)
            }
            WM_KEYDOWN | WM_KEYUP => {
                be.key_msg(msg, wparam.0, lparam.0)?;
                LRESULT(0)
            }
            WM_IME_STARTCOMPOSITION => {
                // runtime boundary first (EN_CHANGE routing keys off it),
                // then the peer sees the real composition start
                be.ime_start();
                be.send_focused(msg, wparam.0, lparam.0);
                be.service_peer_events()?;
                LRESULT(0)
            }
            WM_IME_ENDCOMPOSITION => {
                // peer commits first, its events drain, THEN queued
                // proposals resolve against the final text
                be.send_focused(msg, wparam.0, lparam.0);
                be.service_peer_events()?;
                be.ime_end()?;
                LRESULT(0)
            }
            WM_IME_COMPOSITION | WM_IME_NOTIFY => {
                // richedit sees the raw message; preedit never becomes an edit
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
                // the frame target tracks the client size — resize before
                // layout so paint never draws into a stale surface
                be.renderer.borrow_mut().resize();
                be.relayout()?;
                be.paint()?;
                LRESULT(0)
            }
            WM_DPICHANGED => {
                // update scale + peers BEFORE SetWindowPos — the sync
                // WM_SIZE it fires would otherwise layout/paint against
                // the stale scale
                be.dpi_changed((wparam.0 & 0xffff) as u32)?;
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
                // the resize above may be a no-op — guarantee the repaint
                be.relayout()?;
                be.paint()?;
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
                } else if native_timer_decode(wparam.0).is_some() {
                    // tid is the win32 id — the map resolves owner+richedit id
                    be.native_timer_fire(wparam.0)?;
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
