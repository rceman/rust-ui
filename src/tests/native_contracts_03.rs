    /// F01 — the systemic invariant on REAL OS primitives: a receiver
    /// thread blocked in `MsgWaitForMultipleObjectsEx` on BOTH the event
    /// and its message queue; its posted queue carries a bounded busy
    /// load (512 unrelated posted messages) and it is briefly blocked —
    /// the accepted envelope still delivers because progress lives on
    /// the waitable event, independent of posted-queue fullness, send
    /// cooperation, timers, or a second producer.
    #[test]
    fn mailbox_event_wakes_through_busy_message_queue() {
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
        use std::sync::mpsc;
        static MB: AtomicUsize = AtomicUsize::new(0);
        static EV: AtomicUsize = AtomicUsize::new(0);
        static DELIVERED: AtomicBool = AtomicBool::new(false);
        let (tx, rx) = mpsc::channel::<u32>();
        let receiver = std::thread::spawn(move || {
            unsafe {
                // create the thread's message queue, then hand its id to
                // the producer that fills the posted queue
                let mut m = MSG::default();
                PeekMessageW(
                    &mut m,
                    None,
                    0,
                    0,
                    windows::Win32::UI::WindowsAndMessaging::PM_NOREMOVE,
                );
                tx.send(windows::Win32::System::Threading::GetCurrentThreadId())
                    .unwrap();
            }
            // BLOCK while the producer fills the posted queue — the
            // event route is independent of posted-message load
            std::thread::sleep(std::time::Duration::from_millis(700));
            // the run loop's real shape — drain queued input first, then
            // the event wait returns WAIT_OBJECT_0 for the mailbox pulse
            unsafe {
                let ev = windows::Win32::Foundation::HANDLE(
                    EV.load(Ordering::SeqCst) as *mut std::ffi::c_void
                );
                let mut m = MSG::default();
                while PeekMessageW(
                    &mut m,
                    None,
                    0,
                    0,
                    windows::Win32::UI::WindowsAndMessaging::PM_REMOVE,
                )
                .as_bool()
                {}
                let wait = windows::Win32::UI::WindowsAndMessaging::MsgWaitForMultipleObjectsEx(
                    Some(&[ev]),
                    10000,
                    windows::Win32::UI::WindowsAndMessaging::QS_ALLINPUT,
                    windows::Win32::UI::WindowsAndMessaging::MWMO_INPUTAVAILABLE,
                );
                assert_eq!(
                    wait,
                    windows::Win32::Foundation::WAIT_OBJECT_0,
                    "the mailbox event must fire the wait after the busy queue drains"
                );
                let mbp = MB.load(Ordering::SeqCst) as *const crate::tasks::Mailbox;
                if (*mbp).pop().is_some() {
                    DELIVERED.store(true, Ordering::SeqCst);
                }
            }
        });
        let receiver_tid = rx.recv().unwrap();
        let mb = crate::tasks::Mailbox::new();
        let ev = std::sync::Arc::new(crate::platform::win32::MailboxEvent::create().unwrap());
        EV.store(ev.raw().0 as usize, Ordering::SeqCst);
        MB.store(
            std::sync::Arc::into_raw(mb.clone()) as usize,
            Ordering::SeqCst,
        );
        // keep the receiver's posted queue busy — the event authority is
        // independent of posted-message load by construction. The fill is
        // bounded (512 posts) to keep busy-queue load honest without
        // starving the desktop heap sibling tests need.
        unsafe {
            let mut posted = 0usize;
            while PostThreadMessageW(
                receiver_tid,
                WM_USER,
                windows::Win32::Foundation::WPARAM(0),
                windows::Win32::Foundation::LPARAM(0),
            )
            .is_ok()
            {
                posted += 1;
                if posted >= 512 {
                    break;
                }
            }
            assert!(posted > 0, "the posted queue actually carries load");
        }
        // install the REAL owned-event callback — the accepted try_send
        // itself establishes the signal through the live authority
        let ev2 = ev.clone();
        mb.install_wake(std::sync::Arc::new(move || ev2.signal()))
            .unwrap();
        crate::tasks::UiProxy::<u32>::new(mb.clone())
            .try_send(42u32)
            .unwrap();
        receiver.join().unwrap();
        assert!(
            DELIVERED.load(Ordering::SeqCst),
            "the accepted envelope was delivered through the event wait"
        );
        assert!(
            !mb.take_wake_failure(),
            "a signaled event must not latch a failure"
        );
        unsafe {
            drop(std::sync::Arc::from_raw(
                MB.swap(0, Ordering::SeqCst) as *const crate::tasks::Mailbox
            ));
            drop(ev);
        }
    }

    /// F01 — idle must stay blocked: an unsignaled event + empty queue
    /// leaves the shared wait sleeping (timeout) — no self-wake, no
    /// polling loop.
    #[test]
    fn mailbox_wait_blocks_when_idle() {
        unsafe {
            let ev = windows::Win32::System::Threading::CreateEventExW(
                None,
                windows::core::PCWSTR::null(),
                windows::Win32::System::Threading::CREATE_EVENT(0),
                windows::Win32::System::Threading::EVENT_ALL_ACCESS.0,
            )
            .unwrap();
            let wait = windows::Win32::UI::WindowsAndMessaging::MsgWaitForMultipleObjectsEx(
                Some(&[ev]),
                200,
                windows::Win32::UI::WindowsAndMessaging::QS_ALLINPUT,
                windows::Win32::UI::WindowsAndMessaging::MWMO_INPUTAVAILABLE,
            );
            assert_eq!(
                wait,
                windows::Win32::Foundation::WAIT_TIMEOUT,
                "idle wait must block — no self-firing wake"
            );
            let _ = windows::Win32::Foundation::CloseHandle(ev);
        }
    }

    /// F01 — shutdown race: a producer after terminal close is rejected
    /// with Closed — no envelope can be accepted behind a dead wake.
    #[test]
    fn mailbox_send_after_close_is_terminal_closed() {
        let mb = crate::tasks::Mailbox::new();
        let proxy = crate::tasks::UiProxy::<u32>::new(mb.clone());
        proxy.try_send(1u32).unwrap();
        mb.close();
        assert!(matches!(
            proxy.try_send(2u32),
            Err(crate::tasks::ProxySendError::Closed)
        ));
        assert_eq!(mb.queue_len(), 0, "close dropped the pending envelope");
    }

    /// F01 — coalescing + exact delivery: several producer pushes may
    /// collapse into one event pulse (auto-reset); one turn still drains
    /// every envelope exactly once — no lost work, no duplicates.
    #[test]
    fn mailbox_event_coalescing_delivers_everything_once() {
        struct S {
            got: Vec<u32>,
        }
        let mut rig = Rig::new(
            S { got: Vec::new() },
            |s: &mut S, m: Msg, _: &mut UpdateCtx<Msg>| {
                if let Msg::Scoped(n, _) = m {
                    s.got.push(n);
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
        // real counting seam installed BEFORE the pushes — the first
        // send signals; the rest reuse the Confirmed edge
        let calls = Arc::new(Mutex::new(0u32));
        let c2 = calls.clone();
        be.rt
            .mailbox
            .install_wake(Arc::new(move || {
                *c2.lock().unwrap() += 1;
                true
            }))
            .unwrap();
        let proxy = crate::tasks::UiProxy::<Msg>::new(be.rt.mailbox.clone());
        for i in 0..5 {
            proxy.try_send(Msg::Scoped(i, 0)).unwrap();
        }
        // before the turn — one Confirmed edge covers all five pushes;
        // the queue is NOT stranded behind unobserved pulses
        assert_eq!(
            *calls.lock().unwrap(),
            1,
            "five pushes coalesce to one signal"
        );
        // one bounded turn — all five envelopes deliver exactly once,
        // in order, regardless of how many wake signals coalesced
        be.turn().unwrap();
        assert_eq!(be.rt.state.got, vec![0, 1, 2, 3, 4]);
    }

    /// F01 — painted-key replay ownership: a queued Enter captured with
    /// NO owner must not press the newly focused button; a queued Enter
    /// captured for A must not be redirected to B.
    #[test]
    fn replayed_painted_key_honors_captured_owner() {
        struct S {
            a: u32,
            b: u32,
        }
        let s = S { a: 0, b: 0 };
        let mut rig = Rig::new(
            s,
            |s: &mut S, m: Msg, _: &mut UpdateCtx<Msg>| match m {
                Msg::Scoped(1, _) => s.a += 1,
                Msg::Scoped(2, _) => s.b += 1,
                _ => {}
            },
            |_: &S, ui: &mut Ui<Msg>| {
                ui.button("a").on_press(|| Msg::Scoped(1, 0));
                ui.button("b").on_press(|| Msg::Scoped(2, 0));
            },
        );
        rig.view().unwrap();
        let kids = rig.root_children();
        let (a, b) = (kids[0], kids[1]);
        let Rig { rt, .. } = rig;
        let mut be = crate::platform::win32::Backend::for_test(
            rt,
            crate::platform::win32::PeerCtx::for_test().unwrap(),
        )
        .unwrap();
        // focus moved to B AFTER the messages were queued
        be.focus = Some(b);
        let key = |owner: Option<Option<NodeId>>| QueuedMsg {
            msg: WM_KEYDOWN,
            wparam: windows::Win32::UI::Input::KeyboardAndMouse::VK_RETURN.0 as usize,
            lparam: 0,
            rect: None,
            arrival_focus: owner,
        };
        // captured owner = explicitly None — must NOT activate focused B
        be.reentrant_queue.borrow_mut().push_back(key(Some(None)));
        be.drain_reentrant().unwrap();
        assert_eq!(
            (be.rt.state.a, be.rt.state.b),
            (0, 0),
            "a no-owner replay must not press the newly focused button"
        );
        // captured owner = A — must press A even though B holds focus
        be.reentrant_queue
            .borrow_mut()
            .push_back(key(Some(Some(a))));
        be.drain_reentrant().unwrap();
        assert_eq!(
            (be.rt.state.a, be.rt.state.b),
            (1, 0),
            "a captured owner must receive the key, not current focus"
        );
    }

    /// A Backend over a REAL msftedit peer — the runtime's peer factory
    /// is the production `make_factory`, so registry/attach/focus and
    /// `ime_end`'s reconcile all run the production path.
    struct ImeS {
        v: TextValue,
        conflicts: Vec<TextConflict>,
    }

    type ImeBackend = crate::platform::win32::Backend<
        ImeS,
        Msg,
        Box<dyn Fn(&mut ImeS, Msg, &mut UpdateCtx<Msg>)>,
        Box<dyn Fn(&ImeS, &mut Ui<'_, '_, Msg>)>,
    >;

    fn ime_backend(text: &str) -> (ImeBackend, NodeId) {
        let peer_ctx = crate::platform::win32::PeerCtx::for_test().unwrap();
        let hwnd = probe_hwnd();
        peer_ctx.hwnd.set(hwnd);
        let factory = peer_ctx.make_factory();
        let app = App::new(
            ImeS {
                v: TextValue::new(text),
                conflicts: Vec::new(),
            },
            Box::new(|s: &mut ImeS, m: Msg, _: &mut UpdateCtx<Msg>| match m {
                Msg::Edited(e) => {
                    let _ = s.v.accept(e);
                }
                Msg::Conflicted(c) => s.conflicts.push(c),
                _ => {}
            }) as Box<dyn Fn(&mut ImeS, Msg, &mut UpdateCtx<Msg>)>,
            Box::new(|s: &ImeS, ui: &mut Ui<Msg>| {
                ui.text_input(&s.v)
                    .on_edit(Msg::Edited)
                    .on_conflict(Msg::Conflicted)
                    .automation_id("probe.editor");
            }) as Box<dyn Fn(&ImeS, &mut Ui<Msg>)>,
        );
        let rt = crate::app::runtime_for(
            app,
            Box::new(factory),
            Theme::light(),
            Appearance {
                dark: false,
                forced_colors: false,
            },
        );
        let mut be = ImeBackend::for_test(rt, peer_ctx).unwrap();
        // be.hwnd stays NULL — pump/review/attach/IME all run production
        // paths, but paint() is a no-op by design so no D2D target is ever
        // created on a test thread (a foreign-thread HwndRenderTarget
        // perturbs other native paint tests running in parallel)
        be.turn().unwrap();
        let node = be.rt.root_children()[0];
        (be, node)
    }

    /// A `text_input` peer PLUS a semantic button — the peer-vs-non-peer
    /// focus handoff needs both on one backend.
    fn mixed_backend(text: &str) -> (ImeBackend, NodeId, NodeId) {
        let peer_ctx = crate::platform::win32::PeerCtx::for_test().unwrap();
        let hwnd = probe_hwnd();
        peer_ctx.hwnd.set(hwnd);
        let factory = peer_ctx.make_factory();
        let app = App::new(
            ImeS {
                v: TextValue::new(text),
                conflicts: Vec::new(),
            },
            Box::new(|s: &mut ImeS, m: Msg, _: &mut UpdateCtx<Msg>| match m {
                Msg::Edited(e) => {
                    let _ = s.v.accept(e);
                }
                Msg::Conflicted(c) => s.conflicts.push(c),
                _ => {}
            }) as Box<dyn Fn(&mut ImeS, Msg, &mut UpdateCtx<Msg>)>,
            Box::new(|s: &ImeS, ui: &mut Ui<Msg>| {
                ui.text_input(&s.v)
                    .on_edit(Msg::Edited)
                    .on_conflict(Msg::Conflicted)
                    .automation_id("probe.editor");
                ui.button("btn").on_press(press_msg);
            }) as Box<dyn Fn(&ImeS, &mut Ui<'_, '_, Msg>)>,
        );
        let rt = crate::app::runtime_for(
            app,
            Box::new(factory),
            Theme::light(),
            Appearance {
                dark: false,
                forced_colors: false,
            },
        );
        let mut be = ImeBackend::for_test(rt, peer_ctx).unwrap();
        be.turn().unwrap();
        let kids = be.rt.root_children();
        (be, kids[0], kids[1])
    }

