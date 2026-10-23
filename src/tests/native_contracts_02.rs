    /// F01 — the wake authority's outcome legs. A live event signals
    /// (durable, already awaited by the run loop); a dead handle reports
    /// Latched to the caller (the mailbox signal path owns the terminal
    /// contract for its context); a closed window skips the attempt.
    #[test]
    fn mailbox_wake_failure_is_a_deterministic_outcome() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};
        type Be = crate::platform::win32::Backend<
            (),
            Msg,
            fn(&mut (), Msg, &mut UpdateCtx<Msg>),
            fn(&(), &mut Ui<Msg>),
        >;
        let closed = Arc::new(AtomicBool::new(false));
        // live event -> Signaled
        let ev = crate::platform::win32::MailboxEvent::create().unwrap();
        assert_eq!(
            Be::mailbox_wake_for_test(&ev, &closed),
            crate::platform::win32::WakeOutcome::Signaled
        );
        // a NEVER-VALID handle (not a closed-but-reusable one) -> Latched
        let dead = crate::platform::win32::MailboxEvent::from_raw(
            windows::Win32::Foundation::HANDLE(0xDEAD_BEEFusize as *mut std::ffi::c_void),
        );
        assert_eq!(
            Be::mailbox_wake_for_test(&dead, &closed),
            crate::platform::win32::WakeOutcome::Latched
        );
        std::mem::forget(dead); // don't CloseHandle a fabricated value
        // closed suppresses the wake entirely — no attempt
        closed.store(true, Ordering::SeqCst);
        assert_eq!(
            Be::mailbox_wake_for_test(&ev, &closed),
            crate::platform::win32::WakeOutcome::Skipped
        );
    }

    /// F01 — the TRIGGERING synchronous send must observe the wake
    /// failure: UiProxy::try_send pushes, the installed wake fails, the
    /// EXACT envelope is rolled back, the send returns typed Wake — not
    /// Ok(()) over discarded work. Terminal state: closed + latch; later
    /// sends get Closed.
    #[test]
    fn mailbox_wake_failure_rejects_the_triggering_send() {
        use crate::tasks::{Mailbox, ProxySendError, UiProxy};
        let mb = Mailbox::new();
        // real installed seam — nothing owed, empty queue: install itself
        // is healthy; the signal authority reports failure AT acceptance
        mb.install_wake(std::sync::Arc::new(|| false)).unwrap();
        let proxy = UiProxy::<u32>::new(mb.clone());
        // the send that establishes the failing wake -> typed failure
        assert_eq!(proxy.try_send(7u32), Err(ProxySendError::Wake));
        // its envelope was rolled back — never queued, never delivered
        assert_eq!(mb.queue_len(), 0);
        assert!(mb.pop().is_none(), "rolled-back envelope must not exist");
        assert!(mb.is_closed(), "wake failure closes the mailbox");
        assert!(mb.take_wake_failure(), "typed failure is latched");
        // later producers see the terminal state
        assert_eq!(proxy.try_send(8u32), Err(ProxySendError::Closed));
    }

    /// F01 — the TRIGGERING async send must observe the wake failure:
    /// a real TaskSender::send (SendFuture polled by the executor)
    /// resolves Err(SendError::Wake) — not Ok then Closed on the next.
    #[test]
    fn mailbox_wake_failure_rejects_the_triggering_async_send() {
        use std::sync::{Arc, Mutex};
        let result: Arc<Mutex<Option<crate::tasks::SendError>>> = Arc::new(Mutex::new(None));
        struct S;
        let res2 = result.clone();
        let mut rig = Rig::new(
            S,
            move |_: &mut S, m: Msg, cx: &mut UpdateCtx<Msg>| {
                if let Msg::Kick = m {
                    let res = res2.clone();
                    let _ = cx.spawn("waker", |job| async move {
                        *res.lock().unwrap() = job.send(Msg::Chunk(1)).await.err();
                    });
                }
            },
            |_: &S, _: &mut Ui<Msg>| {},
        );
        rig.view().unwrap();
        // the real installed seam fails — the send's own acceptance path
        rig.rt
            .mailbox
            .install_wake(std::sync::Arc::new(|| false))
            .unwrap();
        rig.rt.push_msg(Msg::Kick);
        rig.pump().unwrap(); // update runs, spawns the task
        rig.exec.poll(); // SendFuture poll -> push_or_wait -> WakeFailed
        assert_eq!(
            *result.lock().unwrap(),
            Some(crate::tasks::SendError::Wake),
            "the triggering async send must resolve typed Wake, not Ok"
        );
        assert!(rig.rt.mailbox.is_closed());
    }

    /// F01 — a waiter registered on a FULL mailbox must be awakened by
    /// the terminal wake failure (not left parked): fill to cap so the
    /// async send pends, install the failing seam, poke -> the waiter
    /// wakes and its next poll resolves terminally.
    #[test]
    fn mailbox_wake_failure_wakes_registered_waiters() {
        use std::sync::{Arc, Mutex};
        let result: Arc<Mutex<Option<crate::tasks::SendError>>> = Arc::new(Mutex::new(None));
        struct S;
        let res2 = result.clone();
        let mut rig = Rig::new(
            S,
            move |_: &mut S, m: Msg, cx: &mut UpdateCtx<Msg>| {
                if let Msg::Kick = m {
                    let res = res2.clone();
                    let _ = cx.spawn("waiter", |job| async move {
                        *res.lock().unwrap() = job.send(Msg::Chunk(9)).await.err();
                    });
                }
            },
            |_: &S, _: &mut Ui<Msg>| {},
        );
        rig.view().unwrap();
        rig.rt
            .mailbox
            .install_wake(std::sync::Arc::new(|| true))
            .unwrap();
        rig.rt.push_msg(Msg::Kick);
        rig.pump().unwrap(); // update runs, spawns the task
        // fill the mailbox to cap AFTER the spawn — the async send pends
        let proxy = crate::tasks::UiProxy::<Msg>::new(rig.rt.mailbox.clone());
        for _ in 0..crate::tasks::MAILBOX_CAP {
            proxy.try_send(Msg::Chunk(0)).unwrap();
        }
        rig.exec.poll(); // send pends on the full mailbox -> waiter registered
        assert!(
            result.lock().unwrap().is_none(),
            "send must still be pending"
        );
        // now the wake authority dies — install_wake's own pending-pulse
        // invokes the failing seam: terminal close + latch
        assert_eq!(
            rig.rt.mailbox.install_wake(std::sync::Arc::new(|| false)),
            Err(crate::tasks::ProxySendError::Wake),
            "a failing install pulse reports the typed Wake error"
        );
        assert!(rig.rt.mailbox.is_closed(), "dead wake -> terminal close");
        assert!(rig.rt.mailbox.take_wake_failure());
        // the woken waiter resolves TERMINALLY: CancelWatch sees the
        // closed mailbox, drops the parked SendFuture (its Drop
        // unregisters the waiter — no dead waiter), task completes
        rig.exec.poll();
        assert_eq!(
            rig.exec.live(),
            0,
            "the woken waiter must terminate, not remain parked"
        );
    }

    /// F01 — handle lifetime: an admitted signaler (cloned callback Arc)
    /// outlives detach — its SetEvent still lands on a VALID handle; the
    /// HANDLE closes only when the last owner drops.
    #[test]
    fn mailbox_event_outlives_detached_signaler() {
        let ev = std::sync::Arc::new(crate::platform::win32::MailboxEvent::create().unwrap());
        let mb = crate::tasks::Mailbox::new();
        let ev2 = ev.clone();
        mb.install_wake(std::sync::Arc::new(move || ev2.signal()))
            .unwrap();
        // clone the stored callback as an in-flight signaler would
        let signaler = mb.clone_wake_cb().unwrap();
        // shutdown detaches the seam AND drops the run loop's Arc
        mb.close();
        let event_ptr = ev.raw().0 as usize;
        drop(ev);
        // the in-flight signaler still owns an Arc — signal must land
        assert!(signaler(), "detached signaler's event must still be valid");
        unsafe {
            let evp = windows::Win32::Foundation::HANDLE(event_ptr as *mut std::ffi::c_void);
            assert_eq!(
                windows::Win32::System::Threading::WaitForSingleObject(evp, 0),
                windows::Win32::Foundation::WAIT_OBJECT_0,
                "the admitted signaler's SetEvent landed on the live event"
            );
        }
        drop(signaler); // last owner gone — handle closes exactly once
    }

    /// F01 — Confirmed-edge interleaving on the REAL owned event: the
    /// native side consumes the signaled event WITHOUT `wake_seen` (the
    /// pump is mid-turn); a second push reuses the Confirmed edge and
    /// does NOT re-signal (count unchanged — the running pump drains
    /// both); after `wake_seen` a fresh push signals a NEW edge the
    /// native wait can observe.
    #[test]
    fn wake_seen_edge_interleaving_uses_real_event() {
        use std::sync::atomic::AtomicUsize;
        use windows::Win32::Foundation::WAIT_OBJECT_0;
        use windows::Win32::System::Threading::WaitForSingleObject;
        let mb = crate::tasks::Mailbox::new();
        let ev = std::sync::Arc::new(crate::platform::win32::MailboxEvent::create().unwrap());
        let calls = std::sync::Arc::new(AtomicUsize::new(0));
        let (ev2, c2) = (ev.clone(), calls.clone());
        mb.install_wake(std::sync::Arc::new(move || {
            c2.fetch_add(1, Ordering::SeqCst);
            ev2.signal()
        }))
        .unwrap();
        let proxy = crate::tasks::UiProxy::<u32>::new(mb.clone());
        // first push — a fresh edge: cb invoked once, event signaled
        proxy.try_send(1).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            unsafe { WaitForSingleObject(ev.raw(), 0) },
            WAIT_OBJECT_0,
            "the signaled event was consumed by the native wait — the \
             delivered edge means the pump drains what followed it"
        );
        // a push BEFORE wake_seen — the consumed-but-not-observed edge is
        // reused: no re-signal, no lost work (the in-flight pump's sweep
        // picks it up)
        proxy.try_send(2).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1, "Confirmed edge reused");
        // the pump observes the edge — Confirmed consumed back to Idle
        mb.wake_seen();
        // a push AFTER wake_seen — a fresh signal on the real event
        proxy.try_send(3).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 2, "consumed edge re-signals");
        // the same sweep drains all three, FIFO — no duplicates
        for want in [1u32, 2, 3] {
            assert_eq!(*mb.pop().unwrap().payload.downcast::<u32>().unwrap(), want);
        }
        assert!(mb.pop().is_none(), "no duplicate envelope");
        // the fresh signal is observable natively on a subsequent wait
        assert_eq!(
            unsafe { WaitForSingleObject(ev.raw(), 0) },
            WAIT_OBJECT_0,
            "the post-wake_seen signal is a real new edge"
        );
        // and the cycle repeats — observe, push, count+1, deliver
        mb.wake_seen();
        proxy.try_send(4).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 3);
        assert_eq!(
            unsafe { WaitForSingleObject(ev.raw(), 0) },
            WAIT_OBJECT_0,
            "the repeated edge also signals"
        );
        assert_eq!(*mb.pop().unwrap().payload.downcast::<u32>().unwrap(), 4);
        assert!(mb.pop().is_none());
    }

    /// F01 — the event turn enters through the SAME native-entry guard
    /// as every other Backend entry: a reentrant arrival queued during
    /// the guarded turn is drained before guarded_turn returns.
    #[test]
    fn mailbox_event_turn_uses_the_native_entry_guard() {
        struct S {
            hits: u32,
        }
        let mut rig = Rig::new(
            S { hits: 0 },
            |s: &mut S, m: Msg, _: &mut UpdateCtx<Msg>| {
                if let Msg::Scoped(9, _) = m {
                    s.hits += 1;
                }
            },
            |_: &S, ui: &mut Ui<Msg>| {
                ui.label("x");
            },
        );
        rig.view().unwrap();
        let Rig { rt, .. } = rig;
        let mut be = crate::platform::win32::Backend::for_test(
            rt,
            crate::platform::win32::PeerCtx::for_test().unwrap(),
        )
        .unwrap();
        // accepted mailbox work + a reentrant QueuedMsg arriving
        // mid-drain — the guarded turn must drain BOTH before returning
        crate::tasks::UiProxy::<Msg>::new(be.rt.mailbox.clone())
            .try_send(Msg::Scoped(9, 0))
            .unwrap();
        {
            let mut q = be.reentrant_queue.borrow_mut();
            q.push_back(QueuedMsg {
                msg: WM_TIMER,
                wparam: 1usize,
                lparam: 0,
                rect: None,
                arrival_focus: None,
            });
        }
        be.guarded_turn().unwrap();
        assert_eq!(be.rt.state.hits, 1, "mailbox envelope delivered");
        assert!(
            be.reentrant_queue.borrow().is_empty(),
            "reentrant arrivals drain inside the guarded turn"
        );
    }

    /// F01 — init failure: a failing wake-event creation produces a
    /// typed UiError::Platform. Because init_wake_event runs BEFORE
    /// window::create, an Err unwinds run() with no HWND, no installed
    /// route pointer — nothing dangles on the dropped Backend.
    #[test]
    fn wake_event_init_failure_is_typed() {
        let bad = crate::platform::win32::init_wake_event(Err(windows::core::Error::new(
            windows::core::HRESULT(-1),
            "forced",
        )));
        assert!(matches!(bad, Err(crate::UiError::Platform(_))));
        let good =
            crate::platform::win32::init_wake_event(crate::platform::win32::MailboxEvent::create());
        assert!(good.is_ok());
    }

    /// F01 — wait classification: every legal MsgWait result maps to its
    /// class; WAIT_FAILED and undocumented codes are typed terminal
    /// errors — never silently treated as "input ready".
    #[test]
    fn wait_classification_is_total() {
        use crate::platform::win32::{WaitClass, classify_wait};
        assert_eq!(
            classify_wait(windows::Win32::Foundation::WAIT_OBJECT_0, 1).unwrap(),
            WaitClass::Event
        );
        assert_eq!(
            classify_wait(
                windows::Win32::Foundation::WAIT_EVENT(
                    windows::Win32::Foundation::WAIT_OBJECT_0.0 + 1
                ),
                1
            )
            .unwrap(),
            WaitClass::Input
        );
        assert!(classify_wait(windows::Win32::Foundation::WAIT_FAILED, 1).is_err());
        assert!(
            classify_wait(windows::Win32::Foundation::WAIT_EVENT(0x1234), 1).is_err(),
            "undocumented results are typed terminal errors"
        );
    }

