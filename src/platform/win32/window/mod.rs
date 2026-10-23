//! HWND lifecycle + WndProc: DPI-aware class registration, userdata routing
//! to the pinned `Backend`, deadline/native timers, caret plumbing, IME
//! routing, tooltip/capture, `WM_GETOBJECT` for UIA, and clean shutdown.

use std::sync::atomic::Ordering;
use std::time::Instant;

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

use super::{Backend, RESIZE, ResizeSample, WM_PUMP, WM_UIA_FOCUS, WM_UIA_PRESS};

pub(super) const DEADLINE_TIMER: usize = 1;
/// Peer-timer ID namespace — framework timers are small ints; every
/// richedit-requested timer is NATIVE_TIMER_BASE + n so the WM_TIMER route
/// is unambiguous forever.
pub(crate) const NATIVE_TIMER_BASE: usize = 0x4000_0000;
/// bound on simultaneously live native timer ids
pub(crate) const NATIVE_TIMER_CAP: usize = 0x1000;
/// WM_TIMER's wparam is a native peer timer iff it carries the base.
pub(super) fn is_native_timer_id(tid: usize) -> bool {
    tid >= NATIVE_TIMER_BASE && tid < NATIVE_TIMER_BASE + NATIVE_TIMER_CAP
}
/// recover the richedit-facing timer id from a win32 id — the map holds
/// the authoritative pair; this only strips the namespace offset
pub(super) fn native_timer_decode(tid: usize) -> Option<usize> {
    is_native_timer_id(tid).then_some(tid - NATIVE_TIMER_BASE)
}

/// One queued reentrant arrival — an OWNED payload. `rect` exists only
/// for WM_DPICHANGED: the original message borrowed `const RECT*` from
/// the sender's stack, which may not be reposted; we copy it here so the
/// deferred delivery never dereferences a borrowed pointer.
#[derive(Clone, Copy)]
pub(crate) struct QueuedMsg {
    pub msg: u32,
    pub wparam: usize,
    pub lparam: isize,
    pub rect: Option<RECT>,
    /// For focus-targeted input: the focused NodeId captured AT ARRIVAL.
    /// DISCRIMINATED — `None` = not a focus-targeted class (replay must
    /// not consult it); `Some(None)` = focus-targeted and NOTHING was
    /// focused at arrival (explicit absent owner — must NOT fall through
    /// to a newly focused node); `Some(Some(id))` = `id` owned the input.
    pub arrival_focus: Option<Option<crate::NodeId>>,
}

/// teardown must never wait behind ordinary work — its dispatch runs the
/// full backend shutdown
pub(crate) fn is_teardown(msg: u32) -> bool {
    msg == WM_NCDESTROY
}

/// Pop the next drain item. Ordinary messages count against the per-pass
/// budget; TEARDOWN bypasses it entirely — a mandatory shutdown queued
/// behind ordinary items still runs THIS drain: when the budget is
/// exhausted the backlog is SWEPT for teardown, not just the front.
/// Budget-exhausted ordinary items stay queued for the continuation.
pub(crate) fn next_drain_item(
    q: &mut std::collections::VecDeque<QueuedMsg>,
    consumed: usize,
) -> Option<QueuedMsg> {
    if let Some(m) = q.front()
        && is_teardown(m.msg)
    {
        return q.pop_front();
    }
    if consumed < REENTRANT_DRAIN_MAX {
        return q.pop_front();
    }
    // budget exhausted — teardown is mandatory cleanup, never ordinary
    // work: sweep the whole backlog so a destruction queued behind 128+
    // items is delivered THIS pass instead of stranding on the
    // continuation (which a closed window suppresses by contract).
    let idx = q.iter().position(|m| is_teardown(m.msg))?;
    q.remove(idx)
}

/// Outcome of one `drain_queue` pass — the caller maps it onto wake
/// continuation and overflow surfacing.
pub(crate) struct DrainOutcome {
    /// mandatory teardown was dispatched this pass
    pub tore: bool,
    /// ordinary items DISPOSED without dispatch after teardown ran —
    /// a closed window owns no further work
    pub disposed: usize,
    /// first ordinary-dispatch error (draining continues regardless —
    /// an early failure can never skip a later teardown)
    pub first_err: Option<crate::UiError>,
    /// ordinary work remains after the budget — caller must schedule the
    /// continuation (skipped entirely when `tore`)
    pub remainder: bool,
}

/// THE reentrant-queue drain policy — verbatim-testable authority shared
/// by `Backend::drain_reentrant`. Contract:
///
/// - ordinary items dispatch FIFO under `REENTRANT_DRAIN_MAX`; `consumed`
///   carries the spent budget ACROSS caller merge-passes, so arrivals
///   produced mid-dispatch drain under the same per-call cap;
/// - `deceased` marks teardown already delivered in an earlier pass —
///   every late arrival on a dead window is disposed without dispatch;
/// - an item error is RECORDED and the pass continues — mandatory
///   teardown can never be skipped by an earlier failed input;
/// - teardown is delivered THIS pass however deep it sits (sweep);
/// - once teardown ran, every remaining ordinary item is disposed of
///   WITHOUT dispatch — a dead window takes no further work;
/// - a non-teardown remainder means the caller owes a continuation wake.
pub(crate) fn drain_queue(
    q: &mut std::collections::VecDeque<QueuedMsg>,
    consumed: &mut usize,
    deceased: bool,
    mut dispatch: impl FnMut(QueuedMsg) -> crate::UiResult,
) -> DrainOutcome {
    let mut out = DrainOutcome {
        tore: false,
        disposed: 0,
        first_err: None,
        remainder: false,
    };
    if deceased {
        out.disposed += q.len();
        q.clear();
        return out;
    }
    loop {
        let Some(m) = next_drain_item(q, *consumed) else {
            break;
        };
        if is_teardown(m.msg) {
            out.tore = true;
        } else {
            *consumed += 1;
        }
        if let Err(e) = dispatch(m)
            && out.first_err.is_none()
        {
            out.first_err = Some(e);
        }
        if out.tore {
            // closed-window contract — the ordinary remainder is dropped
            // deterministically; nothing may be dispatched or deferred for
            // a dead hwnd (inclusive of items the loop never reached)
            out.disposed += q.len();
            q.clear();
            break;
        }
    }
    out.remainder = !q.is_empty();
    out
}

/// Bound on the owned reentrant queue — allocation is bounded, not merely
/// the drain. Overflow is a typed failure (surfaced via `queue_overflowed`),
/// never silent input loss.
pub(crate) const REENTRANT_QUEUE_CAP: usize = crate::event::EVENT_QUEUE_CAP * 2;
/// The per-drain work budget — DISTINCT from queue capacity: a full
/// queue holds REENTRANT_QUEUE_CAP items, a drain consumes at most
/// DRAIN_MAX then posts a checked continuation. Exact-boundary drains
/// are NOT overflow.
pub(crate) const REENTRANT_DRAIN_MAX: usize = crate::event::EVENT_QUEUE_CAP;

/// Messages whose semantics target the focused node — arrival-time focus
/// must be captured into the queue item.
pub(crate) fn focus_targeted(msg: u32) -> bool {
    matches!(
        msg,
        WM_CHAR
            | WM_SYSCHAR
            | WM_KEYDOWN
            | WM_KEYUP
            | WM_SYSKEYDOWN
            | WM_SYSKEYUP
            | WM_IME_STARTCOMPOSITION
            | WM_IME_ENDCOMPOSITION
            | WM_IME_COMPOSITION
            | WM_IME_NOTIFY
            | WM_UIA_FOCUS
            | WM_SETFOCUS
            | WM_KILLFOCUS
            | WM_MOUSEWHEEL
            | WM_MOUSEHWHEEL
    )
}

/// The reentrant-arrival classes whose wparam/lparam are pure scalars —
/// everything else either carries a borrowed pointer (handled explicitly:
/// WM_DPICHANGED) or needs a synchronous result (WM_GETOBJECT → default).
pub(crate) fn deferrable_arrival(msg: u32) -> bool {
    matches!(
        msg,
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
            // NOTE: WM_SIZE is listed for completeness — the explicit
            // live-dispatch arm in reentrant() MUST precede the
            // deferrable_arrival guard or the queued-WM_SIZE drag bug
            // silently returns
            | WM_SIZE
            | WM_MOVE
            | WM_SETTINGCHANGE
            | WM_TIMER
            | WM_UIA_PRESS
            | WM_UIA_FOCUS
    )
}

pub(crate) mod wndproc;
mod wndproc_dispatch;

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
            // surface-matched class brush — the unfilled client shows the
            // resolved surface until the first D2D frame lands instead of
            // flashing OS-white (the user's launch-flash report)
            hbrBackground: unsafe {
                CreateSolidBrush(COLORREF(if super::os_appearance().dark {
                    0x001B1818
                } else {
                    0x00FAFAFA
                }))
            },
            ..Default::default()
        };
        RegisterClassW(&wc);
        // Prefer a secondary monitor for the window's start position —
        // keeps the dev/preview surface off the primary display when a
        // second one is connected (CW_USEDEFAULT otherwise).
        let (x, y) = secondary_monitor_origin()
            .map(|(ox, oy)| (ox + 48, oy + 48))
            .unwrap_or((CW_USEDEFAULT, CW_USEDEFAULT));
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("RustUiWindow"),
            PCWSTR(HSTRING::from(title).as_ptr()),
            WS_OVERLAPPEDWINDOW,
            x,
            y,
            980,
            1600,
            None,
            None,
            Some(inst.into()),
            Some(route as *const std::ffi::c_void),
        )
        .map_err(|e| crate::UiError::Platform(format!("CreateWindowExW: {e}")))?;
        // stays hidden — run() shows it only AFTER the first guarded_turn
        // painted the themed frame, so no OS-white client ever flashes
        Ok(hwnd)
    }
}

/// Work-area origin of the first non-primary monitor, if any. Used to open
/// the dev/preview window on a secondary display rather than the main one.
fn secondary_monitor_origin() -> Option<(i32, i32)> {
    unsafe extern "system" fn pick(
        hmon: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        data: LPARAM,
    ) -> BOOL {
        let out = unsafe { &mut *(data.0 as *mut Option<(i32, i32)>) };
        let mut mi = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if unsafe { GetMonitorInfoW(hmon, &mut mi) }.as_bool()
            && (mi.dwFlags & MONITORINFOF_PRIMARY) == 0
        {
            *out = Some((mi.rcWork.left, mi.rcWork.top));
        }
        TRUE
    }
    let mut found: Option<(i32, i32)> = None;
    unsafe {
        EnumDisplayMonitors(
            None,
            None,
            Some(pick),
            LPARAM(&mut found as *mut _ as isize),
        );
    }
    found
}
