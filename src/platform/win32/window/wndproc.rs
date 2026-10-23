use super::wndproc_dispatch::dispatch;
use super::*;

pub(crate) fn dispatch_owned<S, M, U, V>(
    hwnd: HWND,
    m: QueuedMsg,
    be: &mut Backend<S, M, U, V>,
) -> UiResult<LRESULT>
where
    M: 'static,
    U: Fn(&mut S, M, &mut UpdateCtx<'_, M>),
    V: Fn(&S, &mut Ui<'_, '_, M>),
{
    let l = match &m.rect {
        Some(rc) => LPARAM(rc as *const RECT as isize),
        None => LPARAM(m.lparam),
    };
    dispatch(hwnd, m.msg, WPARAM(m.wparam), l, be)
}

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
        // deferred native arrivals — owned payloads only — run FIFO
        // now that no &mut would alias. A drain error is fatal.
        if let Err(e) = be.drain_reentrant() {
            be.mark_fatal(e);
        }
        #[cfg(feature = "devtools")]
        be.complete_devtools_idle();
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
pub(crate) unsafe fn reentrant<S, M, U, V>(
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
                // teardown cannot defer a detached pointer — do the
                // atomic parts now, queue the full cleanup so the
                // post-dispatch drain still runs be.shutdown().
                // TEARDOWN BYPASSES THE CAP: a full queue must never
                // drop mandatory cleanup.
                (*ptr).closed.store(true, Ordering::SeqCst);
                (*ptr).rt.mailbox.close();
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                (*ptr).reentrant_queue.borrow_mut().push_back(QueuedMsg {
                    msg,
                    wparam: wparam.0,
                    lparam: lparam.0,
                    rect: None,
                    arrival_focus: None,
                });
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_ENTERSIZEMOVE | WM_EXITSIZEMOVE => {
                // modal sizing/move loop marker — arms the session flag
                // that keeps reentrant WM_PAINT live for the whole
                // gesture (a move can expose off-screen client pixels
                // with size unchanged), and runs session-end work once
                let enter = msg == WM_ENTERSIZEMOVE;
                (*ptr).sizing_session.set(enter);
                if (*ptr).resize_trace_path.is_some() {
                    (*ptr).size_seq.set(0);
                    (*ptr).resize_trace(format!(
                        "{}SIZEMOVE session={enter}",
                        if enter { "ENTER" } else { "EXIT" }
                    ));
                }
                if !enter {
                    // consume the coalesced move work through the
                    // dispatch arm — its DefWindowProcW call doubles as
                    // this message's default handling
                    let outer = (*ptr).in_dispatch.replace(false);
                    let r = trampoline::<S, M, U, V>(hwnd, msg, wparam, lparam);
                    (*ptr).in_dispatch.set(outer);
                    r
                } else {
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
            }
            WM_MOVE => {
                // bounded coalescing — during a held title-bar drag the
                // OS emits a move per pixel; the latest position is the
                // only one that matters for UIA screen bounds, so arm a
                // pending flag consumed once at session end instead of
                // queueing every intermediate position
                (*ptr).move_pending.set(true);
                LRESULT(0)
            }
            WM_SIZE => {
                // CANNOT defer: the modal sizing loop nests inside our
                // WM_NCLBUTTONDOWN dispatch, so EVERY arrival during a
                // live border-drag is reentrant — a queued WM_SIZE only
                // drains after button release, leaving the grown client
                // painted with the pre-drag layout (the user's stale
                // narrow-content report). Payload is scalars and the
                // outer borrow is dormant inside DefWindowProcW's modal
                // loop — run the normal dispatch now by dropping the
                // guard and re-entering the trampoline (same juggling
                // drain_reentrant uses around each item).
                // LOAD-BEARING INVARIANT: any call that can deliver a
                // message synchronously (DefWindowProcW, SetWindowPos,
                // DestroyWindow, TrackPopupMenu, host callbacks) must
                // never be made while a RefCell borrow is held or while
                // a caller reads a non-Cell Backend field cached across
                // the call — that is what keeps this sibling-&mut legal.
                let outer = (*ptr).in_dispatch.replace(false);
                let r = trampoline::<S, M, U, V>(hwnd, msg, wparam, lparam);
                (*ptr).in_dispatch.set(outer);
                r
            }
            WM_ERASEBKGND => {
                // before the first D2D frame commits, let DefWindowProcW
                // fill the surface-matched class brush (0x18181B dark /
                // 0xFAFAFA light); once committed, suppress GDI erase so
                // resize/repaint never flickers
                if (*ptr).committed_ink.borrow().is_empty() {
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                } else {
                    LRESULT(1)
                }
            }
            WM_PAINT => {
                // a resize frame must run LIVE during the sizing
                // session — pending from a live WM_SIZE, or stale
                // because a size change somehow reached the client
                // without arming pending (defensive: same check the
                // dispatch arm runs)
                let scale = (&(*ptr).peer_ctx).scale.get();
                let stale = super::super::layout::client_dip(hwnd, scale)
                    .map(|sz| sz != (*ptr).laid_client.get())
                    .unwrap_or(false);
                if (*ptr).size_pending.get() || stale || (*ptr).sizing_session.get() {
                    // run the frame now — deferring paints stale
                    // geometry into the grown target while held
                    let outer = (*ptr).in_dispatch.replace(false);
                    let r = trampoline::<S, M, U, V>(hwnd, msg, wparam, lparam);
                    (*ptr).in_dispatch.set(outer);
                    r
                } else {
                    // coalescible — the in-flight frame covers it
                    let _ = ValidateRect(Some(hwnd), None);
                    LRESULT(0)
                }
            }
            WM_DPICHANGED => {
                // CANNOT defer: a held cross-monitor drag can cross a
                // DPI boundary mid-gesture — scale, target DPI, layout
                // and peers must converge while the button is held, not
                // after release. The lparam RECT is borrowed but valid
                // for this synchronous call; the dispatch arm reads it
                // immediately — safe to run live via the trampoline.
                let outer = (*ptr).in_dispatch.replace(false);
                let r = trampoline::<S, M, U, V>(hwnd, msg, wparam, lparam);
                (*ptr).in_dispatch.set(outer);
                r
            }
            m if deferrable_arrival(m) => {
                // scalar/copyable payload + arrival-time focus snapshot
                // — owned, bounded, one queue
                // DISCRIMINATED capture: focus-targeted messages record
                // `Some(current)` (possibly Some(None)); others record
                // None — never consult the snapshot
                let arrival_focus = focus_targeted(m).then_some((*ptr).focus);
                let mut q = (*ptr).reentrant_queue.borrow_mut();
                if q.len() < REENTRANT_QUEUE_CAP {
                    q.push_back(QueuedMsg {
                        msg,
                        wparam: wparam.0,
                        lparam: lparam.0,
                        rect: None,
                        arrival_focus,
                    });
                } else {
                    // bounded capacity — record the overflow; the drain
                    // surfaces it as a typed failure
                    (*ptr).queue_overflowed.set(true);
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
