use super::*;

pub(super) fn dispatch<S, M, U, V>(
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
            let tl0 = RESIZE.text_layout_creates.load(Ordering::Relaxed);
            let sh0 = RESIZE.shadow_rasters.load(Ordering::Relaxed);
            let t0 = Instant::now();
            // resize-deferred work — the OS coalesces invalid rects
            // during a drag so this runs per DELIVERED paint, not per
            // WM_SIZE arrival
            let t = Instant::now();
            // re-verify the live client against what was actually laid
            // out — a WM_SIZE/coalesce path that never reached a relayout
            // must still converge rather than leave the retained tree at
            // a stale width; renderer.resize() retries here too so a
            // swallowed target-resize failure can't persist
            be.renderer.borrow_mut().resize();
            let stale = super::super::layout::client_dip(hwnd, be.peer_ctx.scale.get())
                .map(|sz| sz != be.laid_client.get())
                .unwrap_or(false);
            let pending = be.size_pending.replace(false);
            if pending || stale {
                be.relayout()?;
                be.uia_refresh()?;
            }
            if be.resize_trace_path.is_some() {
                let mut rc = RECT::default();
                unsafe {
                    let _ = GetClientRect(hwnd, &mut rc);
                }
                let (tw, th) = be.renderer.borrow().target_px();
                let root = be.rt.root;
                let rr = be.rects.get(&root).copied().unwrap_or_default();
                let laid = be.laid_client.get();
                let edge = be
                    .rects
                    .values()
                    .map(|r| r.x + r.width)
                    .fold(0.0f32, f32::max);
                be.resize_trace(format!(
                        "PAINT client={}x{} target={tw}x{th} pending={pending} stale={stale} relaid={} laid={}x{} root={}x{} edge={edge} epoch={}",
                        rc.right - rc.left,
                        rc.bottom - rc.top,
                        pending || stale,
                        laid.0,
                        laid.1,
                        rr.width,
                        rr.height,
                        be.paint_epoch.get(),
                    ));
            }
            let layout_us = t.elapsed().as_micros() as u64;
            let t = Instant::now();
            be.paint()?;
            RESIZE.record(ResizeSample {
                kind: 1,
                wm_size_us: t0.elapsed().as_micros() as u64,
                resize_us: 0,
                layout_us,
                uia_us: 0,
                paint_us: t.elapsed().as_micros() as u64,
                text_layouts: RESIZE.text_layout_creates.load(Ordering::Relaxed) - tl0,
                shadow_rasters: RESIZE.shadow_rasters.load(Ordering::Relaxed) - sh0,
            });
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
            be.hover(be.pt_px(lparam), wparam.0)?;
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
            let pt = be.pt_px(lparam);
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
            let pt = be.pt_px(lparam);
            be.pointer(
                crate::node::PointerPhase::Up,
                pt,
                Some(crate::PointerButton::Primary),
                wparam.0,
            )?;
            LRESULT(0)
        }
        WM_SYSKEYDOWN | WM_SYSKEYUP => {
            // system-key semantics are OS-owned — the deferrable route
            // exists so a reentrant arrival still reaches THIS branch;
            // dispatch resolves to the OS default, never drops it
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
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
            // then the peer sees the real composition start — the pin
            // is established inside ime_start from the arrival owner
            be.ime_start();
            be.send_ime(msg, wparam.0, lparam.0)?;
            be.service_peer_events()?;
            LRESULT(0)
        }
        WM_IME_ENDCOMPOSITION => {
            // native end first — delivered to the PINNED composition
            // owner; ime_end clears the flag THEN drains — the final
            // commit must land as committed text
            be.send_ime(msg, wparam.0, lparam.0)?;
            be.ime_end()?;
            LRESULT(0)
        }
        WM_IME_COMPOSITION | WM_IME_NOTIFY => {
            // richedit sees the raw message; preedit never becomes an
            // edit — routed to the pinned owner, not current focus
            be.send_ime(msg, wparam.0, lparam.0)?;
            be.service_peer_events()?;
            LRESULT(0)
        }
        WM_SETFOCUS => {
            be.send_focused(msg, wparam.0, lparam.0)?;
            be.turn()?;
            LRESULT(0)
        }
        WM_KILLFOCUS => {
            be.send_focused(msg, wparam.0, lparam.0)?;
            be.turn()?;
            LRESULT(0)
        }
        WM_MOUSEWHEEL => {
            be.wheel(msg, wparam.0, lparam.0)?;
            LRESULT(0)
        }
        WM_ENTERSIZEMOVE | WM_EXITSIZEMOVE => {
            let enter = msg == WM_ENTERSIZEMOVE;
            be.sizing_session.set(enter);
            if be.resize_trace_path.is_some() {
                be.size_seq.set(0);
                be.resize_trace(format!(
                    "{}SIZEMOVE session={enter}",
                    if enter { "ENTER" } else { "EXIT" }
                ));
            }
            if !enter && be.move_pending.replace(false) {
                // one converged UIA screen-bounds refresh for the whole
                // move gesture — never a replay of queued positions
                be.uia_refresh()?;
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_SIZE => {
            // the frame target tracks the client size — resize before
            // layout so paint never draws into a stale surface. Layout+
            // paint are deferred to the OS-coalesced WM_PAINT delivery:
            // a drag emits WM_SIZE per mouse-move step, and painting per
            // step was the measured stall.
            let t0 = Instant::now();
            let t = Instant::now();
            if be.resize_trace_path.is_some() {
                let seq = be.size_seq.get() + 1;
                be.size_seq.set(seq);
                let mut rc = RECT::default();
                unsafe {
                    let _ = GetClientRect(hwnd, &mut rc);
                }
                let (tw, th) = be.renderer.borrow().target_px();
                be.resize_trace(format!(
                    "SIZE {seq} lparam={}x{} client={}x{} target={tw}x{th} pending={}->true",
                    (lparam.0 & 0xffff) as u16 as i16,
                    ((lparam.0 >> 16) & 0xffff) as u16 as i16,
                    rc.right - rc.left,
                    rc.bottom - rc.top,
                    be.size_pending.get(),
                ));
            }
            be.renderer.borrow_mut().resize();
            let resize_us = t.elapsed().as_micros() as u64;
            be.size_pending.set(true);
            unsafe {
                let _ = InvalidateRect(Some(hwnd), None, false);
            }
            RESIZE.record(ResizeSample {
                kind: 0,
                wm_size_us: t0.elapsed().as_micros() as u64,
                resize_us,
                layout_us: 0,
                uia_us: 0,
                paint_us: 0,
                text_layouts: 0,
                shadow_rasters: 0,
            });
            LRESULT(0)
        }
        WM_ERASEBKGND => {
            // before the first D2D frame commits, let DefWindowProcW fill
            // the surface-matched class brush; afterwards suppress GDI
            // erase so resize/repaint never flickers
            if be.committed_ink.borrow().is_empty() {
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            } else {
                LRESULT(1)
            }
        }
        WM_MOVE => {
            // client-space geometry is unchanged but SCREEN rects move —
            // UIA's cached bounds must refresh or clients hit-test stale
            be.uia_refresh()?;
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
            // SetWindowPos's nested WM_SIZE was delivered LIVE (the
            // reentrant route re-dispatches it through the trampoline)
            // — drain any other queued items NOW so pending work lands
            // before this frame's paint and size_pending settles.
            // drain_reentrant clears in_dispatch around each item, so
            // the outer dispatch's ownership guard is restored after.
            let outer = be.in_dispatch.replace(true);
            let drained = be.drain_reentrant();
            be.in_dispatch.set(outer);
            drained?;
            // the resize above may be a no-op — guarantee the repaint
            be.relayout()?;
            be.uia_refresh()?;
            be.paint()?;
            // this synchronous pass IS the coalesced frame for the
            // new scale + size — nothing may keep size_pending armed
            // or the next delivered WM_PAINT re-runs the whole
            // layout+uia+paint it already produced
            be.size_pending.set(false);
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
