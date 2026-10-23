    use super::*;
    use crate::NodeId;
    use crate::platform::win32::text::TimerPool;
    use crate::platform::win32::window::{
        NATIVE_TIMER_BASE, NATIVE_TIMER_CAP, QueuedMsg, REENTRANT_DRAIN_MAX, REENTRANT_QUEUE_CAP,
        deferrable_arrival, drain_queue, focus_targeted, is_teardown, next_drain_item,
        wndproc::{dispatch_owned, reentrant},
    };
    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::*;

    /// Only scalar/copyable payloads may be deferred — a borrowed-pointer
    /// or sync-result message must never reach the FIFO.
    #[test]
    fn reentrant_classes_are_owned_only() {
        // scalar-deferrable classes
        for m in [
            WM_MOUSEMOVE,
            WM_LBUTTONDOWN,
            WM_KEYDOWN,
            WM_CHAR,
            WM_SETFOCUS,
            WM_TIMER,
            WM_SIZE,
            WM_MOVE,
        ] {
            assert!(deferrable_arrival(m), "msg {m:#x} must be deferrable");
        }
        // sync-result / pointer-bearing / non-queued classes are NOT
        for m in [
            WM_GETOBJECT, // needs a synchronous provider answer
            WM_NCCREATE,  // carries CREATESTRUCT* — borrowed
            WM_CREATE,    // borrowed create params
            WM_NOTIFY,    // NMHDR* — borrowed
            0x02E0,       // WM_DPICHANGED — borrowed RECT* (handled explicitly)
            WM_NCDESTROY, // teardown runs its own contract
            WM_PAINT,     // coalesced, not queued
        ] {
            assert!(
                !deferrable_arrival(m),
                "msg {m:#x} must NOT be scalar-deferred"
            );
        }
    }

    /// F01 contract — the queue's capacity and the drain budget are
    /// DIFFERENT authorities: capacity bounds total backlog, drain bounds
    /// per-pass work and continues via WM_PUMP. An exact-boundary drain
    /// must not report overflow.
    #[test]
    fn reentrant_queue_capacity_and_drain_are_distinct() {
        assert!(
            REENTRANT_QUEUE_CAP > REENTRANT_DRAIN_MAX,
            "queue cap must exceed the drain bound — a full drain pass must be able to continue"
        );
        // focus-targeted messages capture the arrival-time focus —
        // including the focus transitions and wheel routes that deliver
        // via send_focused
        for m in [
            WM_KEYDOWN,
            WM_KEYUP,
            WM_CHAR,
            WM_IME_ENDCOMPOSITION,
            WM_SETFOCUS,
            WM_KILLFOCUS,
            WM_MOUSEWHEEL,
        ] {
            assert!(focus_targeted(m), "msg {m:#x} must snapshot arrival focus");
        }
        for m in [WM_MOUSEMOVE, WM_SIZE, WM_TIMER] {
            assert!(
                !focus_targeted(m),
                "msg {m:#x} is positional, not focus-owned"
            );
        }
    }

    /// F01 exact-boundary drains — a full budget pass empties without
    /// continuation; teardown NEVER waits behind budget-exhausted work.
    #[test]
    fn reentrant_drain_boundary_and_teardown_bypass() {
        let mk = |msg: u32| QueuedMsg {
            msg,
            wparam: 0,
            lparam: 0,
            rect: None,
            arrival_focus: None,
        };
        // exactly DRAIN_MAX ordinary items -> all drained, nothing left
        let mut q: std::collections::VecDeque<QueuedMsg> =
            (0..REENTRANT_DRAIN_MAX).map(|_| mk(WM_KEYDOWN)).collect();
        let mut consumed = 0usize;
        let mut n = 0usize;
        while let Some(m) = next_drain_item(&mut q, consumed) {
            if !is_teardown(m.msg) {
                consumed += 1;
            }
            n += 1;
        }
        assert_eq!(n, REENTRANT_DRAIN_MAX);
        assert!(q.is_empty());
        // beyond the bound — ordinary remainder stays for the pump
        let mut q: std::collections::VecDeque<QueuedMsg> = (0..REENTRANT_DRAIN_MAX + 10)
            .map(|_| mk(WM_KEYDOWN))
            .collect();
        let mut consumed = 0usize;
        let mut n = 0usize;
        while let Some(m) = next_drain_item(&mut q, consumed) {
            if !is_teardown(m.msg) {
                consumed += 1;
            }
            n += 1;
        }
        assert_eq!(n, REENTRANT_DRAIN_MAX);
        assert_eq!(q.len(), 10, "remainder waits for the continuation");
        // TEARDOWN queued at position 129 — beyond the budget — must
        // still be delivered this pass
        let mut q: std::collections::VecDeque<QueuedMsg> =
            (0..REENTRANT_DRAIN_MAX).map(|_| mk(WM_KEYDOWN)).collect();
        q.push_back(mk(WM_NCDESTROY));
        let mut consumed = 0usize;
        let mut tore = false;
        while let Some(m) = next_drain_item(&mut q, consumed) {
            if is_teardown(m.msg) {
                tore = true;
            } else {
                consumed += 1;
            }
        }
        assert!(tore, "teardown past the drain bound must still dispatch");
        assert!(q.is_empty());
    }

    /// F01 BLOCKER — the reproduced trace: 138 ordinary items THEN
    /// destruction. The teardown must be dispatched THIS pass (swept out
    /// of the backlog past the exhausted budget), the pass's ordinary
    /// dispatch count is exactly the budget, and the remainder is
    /// DISPOSED — a dead window takes no further work.
    #[test]
    fn drain_teardown_behind_exhausted_budget_disposes_remainder() {
        let mk = |msg: u32| QueuedMsg {
            msg,
            wparam: 0,
            lparam: 0,
            rect: None,
            arrival_focus: None,
        };
        let mut q: std::collections::VecDeque<QueuedMsg> = (0..REENTRANT_DRAIN_MAX + 10)
            .map(|_| mk(WM_KEYDOWN))
            .collect();
        q.push_back(mk(WM_NCDESTROY));
        let mut ordinary = 0usize;
        let mut shutdowns = 0usize;
        let mut consumed = 0usize;
        let out = drain_queue(&mut q, &mut consumed, false, |m| {
            if is_teardown(m.msg) {
                shutdowns += 1;
            } else {
                ordinary += 1;
            }
            Ok(())
        });
        assert_eq!(ordinary, REENTRANT_DRAIN_MAX, "budget bound is exact");
        assert_eq!(shutdowns, 1, "mandatory cleanup ran exactly once");
        assert!(out.tore);
        assert_eq!(
            out.disposed, 10,
            "the ordinary remainder is disposed, never dispatched"
        );
        assert!(q.is_empty());
        assert!(
            !out.remainder,
            "no continuation is owed after a dead window"
        );
    }

    /// F01 — an earlier delivery error must not skip a later teardown:
    /// the first error is recorded AND destruction still runs.
    #[test]
    fn drain_delivery_error_does_not_skip_teardown() {
        let mk = |msg: u32| QueuedMsg {
            msg,
            wparam: 0,
            lparam: 0,
            rect: None,
            arrival_focus: None,
        };
        let mut q: std::collections::VecDeque<QueuedMsg> =
            std::collections::VecDeque::from([mk(WM_KEYDOWN), mk(WM_NCDESTROY), mk(WM_CHAR)]);
        let mut shutdowns = 0usize;
        let mut post_teardown_dispatched = 0usize;
        let mut consumed = 0usize;
        let out = drain_queue(&mut q, &mut consumed, false, |m| {
            if is_teardown(m.msg) {
                shutdowns += 1;
                return Ok(());
            }
            if shutdowns > 0 {
                post_teardown_dispatched += 1;
            }
            Err(UiError::Platform("delivery failed".into()))
        });
        assert_eq!(shutdowns, 1, "teardown ran despite the earlier error");
        assert!(matches!(out.first_err, Some(UiError::Platform(_))));
        assert_eq!(
            post_teardown_dispatched, 0,
            "nothing dispatches after teardown"
        );
        assert_eq!(out.disposed, 1, "the trailing ordinary item is disposed");
        assert!(q.is_empty());
    }

    /// F01 — the REAL backend drain against a mounted peer: 138 ordinary
    /// items + destruction must run `shutdown()` exactly once (the peer is
    /// released) and leave the queue empty.
    #[test]
    fn backend_drain_runs_shutdown_and_disposes_remainder() {
        struct S {
            v: TextValue,
        }
        let s = S {
            v: TextValue::new("x"),
        };
        let mut rig = Rig::new(
            s,
            |_: &mut S, _: Msg, _: &mut UpdateCtx<Msg>| {},
            |s: &S, ui: &mut Ui<Msg>| {
                ui.text_input(&s.v).on_edit(Msg::Edited);
            },
        );
        rig.view().unwrap();
        let editor = rig.root_children()[0];
        let peer = rig.peer_id(editor).unwrap();
        let Rig { rt, peers, .. } = rig;
        let mut be = crate::platform::win32::Backend::for_test(
            rt,
            crate::platform::win32::PeerCtx::for_test().unwrap(),
        )
        .unwrap();
        let mk = |msg: u32| QueuedMsg {
            msg,
            wparam: 0,
            lparam: 0,
            rect: None,
            arrival_focus: None,
        };
        for _ in 0..REENTRANT_DRAIN_MAX + 10 {
            be.reentrant_queue.borrow_mut().push_back(mk(WM_KEYDOWN));
        }
        be.reentrant_queue.borrow_mut().push_back(mk(WM_NCDESTROY));
        be.drain_reentrant().unwrap();
        // REAL shutdown proof: the mounted peer was released exactly once
        let rec = peers.recs.lock().unwrap().get(&peer).unwrap().clone();
        assert_eq!(
            rec.released, 1,
            "shutdown must release the mounted peer exactly once"
        );
        assert!(be.closed.load(std::sync::atomic::Ordering::SeqCst));
        assert!(
            be.reentrant_queue.borrow().is_empty(),
            "the ordinary remainder is disposed with the dead window"
        );
    }

    /// Held title-bar move semantics: the modal move loop delivers every
    /// arrival reentrantly, so WM_MOVE must coalesce into a single pending
    /// flag (bounded memory, no per-pixel position history) and the
    /// session markers must arm/clear `sizing_session` — the flag that
    /// keeps reentrant WM_PAINT dispatched live while the button is held.
    #[test]
    fn held_move_session_coalesces_and_arms() {
        struct S {
            v: TextValue,
        }
        let s = S {
            v: TextValue::new("x"),
        };
        let mut rig = Rig::new(
            s,
            |_: &mut S, _: Msg, _: &mut UpdateCtx<Msg>| {},
            |s: &S, ui: &mut Ui<Msg>| {
                ui.text_input(&s.v).on_edit(Msg::Edited);
            },
        );
        rig.view().unwrap();
        let Rig { rt, .. } = rig;
        let mut be = crate::platform::win32::Backend::for_test(
            rt,
            crate::platform::win32::PeerCtx::for_test().unwrap(),
        )
        .unwrap();
        let ptr: *mut _ = &mut be;
        let h = HWND::default();
        unsafe {
            reentrant(h, WM_ENTERSIZEMOVE, WPARAM(0), LPARAM(0), ptr);
        }
        assert!(be.sizing_session.get(), "ENTER must arm the session");
        // hundreds of reentrant moves during a held drag collapse into
        // one pending flag — the queue must not grow at all
        for _ in 0..256 {
            unsafe { reentrant(h, WM_MOVE, WPARAM(0), LPARAM(0), ptr) };
        }
        assert!(be.move_pending.get(), "moves must coalesce to pending");
        assert_eq!(
            be.reentrant_queue.borrow().len(),
            0,
            "reentrant WM_MOVE must never enqueue a position history"
        );
        // the real EXIT consume path runs through dispatch (the reentrant
        // arm re-enters it via the trampoline) — exercise the same arm
        dispatch_owned(
            h,
            QueuedMsg {
                msg: WM_EXITSIZEMOVE,
                wparam: 0,
                lparam: 0,
                rect: None,
                arrival_focus: None,
            },
            &mut be,
        )
        .unwrap();
        assert!(!be.sizing_session.get(), "EXIT must clear the session");
        assert!(
            !be.move_pending.get(),
            "EXIT must consume the coalesced move exactly once"
        );
    }

    /// F01 — drain-TIME arrival: the reproduced blocker. One ordinary
    /// queued item whose dispatch itself produces the reentrant
    /// WM_NCDESTROY arrival (closed + mailbox close + owned queue push —
    /// the same atomic leg the trampoline's `reentrant` performs) —
    /// previously `out.remainder` was computed before the merge, the
    /// wake was suppressed by closed, and cleanup stranded. Now the
    /// merged queue is re-evaluated and mandatory teardown runs in THIS
    /// drain.
    #[test]
    fn drain_teardown_arriving_mid_dispatch_still_shuts_down() {
        // handles the update callback uses to inject the exact reentrant
        // teardown arrival — the backend generic is unnameable inside the
        // callback, so it reaches the live queue + closed + mailbox
        // through concrete handles (identical to `reentrant`'s legs)
        struct Inj {
            q: usize, // *mut RefCell<VecDeque<QueuedMsg>>
            closed: std::sync::Arc<std::sync::atomic::AtomicBool>,
            mailbox: std::sync::Arc<crate::tasks::Mailbox>,
        }
        struct S {
            v: TextValue,
            inj: std::rc::Rc<std::cell::RefCell<Option<Inj>>>,
            delivered: u32,
        }
        let inj = std::rc::Rc::new(std::cell::RefCell::new(None::<Inj>));
        let inj2 = inj.clone();
        let mut rig = Rig::new(
            S {
                v: TextValue::new("x"),
                inj: inj2,
                delivered: 0,
            },
            |s: &mut S, m: Msg, _: &mut UpdateCtx<Msg>| {
                if let Msg::Scoped(7, _) = m {
                    s.delivered += 1;
                    if let Some(h) = s.inj.borrow_mut().take() {
                        // verbatim reentrant WM_NCDESTROY arrival
                        h.closed.store(true, std::sync::atomic::Ordering::SeqCst);
                        h.mailbox.close();
                        let q =
                            h.q as *mut std::cell::RefCell<std::collections::VecDeque<QueuedMsg>>;
                        unsafe {
                            (*q).borrow_mut().push_back(QueuedMsg {
                                msg: WM_NCDESTROY,
                                wparam: 0,
                                lparam: 0,
                                rect: None,
                                arrival_focus: None,
                            });
                        }
                    }
                }
            },
            |s: &S, ui: &mut Ui<Msg>| {
                ui.text_input(&s.v).on_edit(Msg::Edited);
                ui.button("b").on_press(|| Msg::Scoped(7, 0));
            },
        );
        rig.view().unwrap();
        let editor = rig.root_children()[0];
        let btn = rig.root_children()[1];
        let peer = rig.peer_id(editor).unwrap();
        let Rig { rt, peers, .. } = rig;
        let mut be = crate::platform::win32::Backend::for_test(
            rt,
            crate::platform::win32::PeerCtx::for_test().unwrap(),
        )
        .unwrap();
        be.turn().unwrap(); // mount the peer
        *inj.borrow_mut() = Some(Inj {
            q: std::ptr::addr_of!(be.reentrant_queue) as usize,
            closed: be.closed.clone(),
            mailbox: be.rt.mailbox.clone(),
        });
        // PRECONDITION: one ordinary queued dispatch (a pump that delivers
        // the queued Press -> update injects teardown WHILE draining)
        be.rt
            .events
            .push(QueuedEvent {
                node: btn,
                payload: NodeEvent::Press,
            })
            .unwrap();
        be.reentrant_queue.borrow_mut().push_back(QueuedMsg {
            msg: crate::platform::win32::WM_PUMP,
            wparam: 0,
            lparam: 0,
            rect: None,
            arrival_focus: None,
        });
        // ACTION: ONE drain call — the teardown that arrived mid-dispatch
        // must still run mandatory cleanup
        be.drain_reentrant().unwrap();
        // POSTCONDITIONS
        let rec = peers.recs.lock().unwrap().get(&peer).unwrap().clone();
        assert_eq!(
            rec.released, 1,
            "teardown arrived mid-dispatch and STILL released the peer"
        );
        assert!(be.closed.load(std::sync::atomic::Ordering::SeqCst));
        assert!(
            be.reentrant_queue.borrow().is_empty(),
            "no mandatory work remains stranded after the drain"
        );
    }

