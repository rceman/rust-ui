#[test]
fn scheduler_hidden_then_resume_resets_delta() {
    struct S {
        show: bool,
        frames: Vec<Duration>,
    }
    let mut rig = Rig::new(
        S {
            show: true,
            frames: Vec::new(),
        },
        |s: &mut S, m: Msg, _cx: &mut UpdateCtx<Msg>| {
            if let Msg::Press = m {
                s.frames.push(Duration::from_millis(0));
            }
        },
        |s: &S, ui: &mut Ui<Msg>| {
            if s.show {
                ui.custom(Rc::new(Tile))
                    .frame_events(true)
                    .on_frame(|_| Msg::Press);
            }
        },
    );
    rig.view().unwrap();
    assert!(rig.rt.next_deadline().is_some());

    // hide: demand suspends
    rig.rt.state.show = false;
    rig.pump().unwrap();
    rig.view().unwrap();
    assert!(rig.rt.next_deadline().is_none());

    // resume: first delta resets (~0, not the whole hidden window)
    rig.rt.state.show = true;
    rig.pump().unwrap();
    rig.view().unwrap();
    assert!(rig.rt.next_deadline().is_some());
}

#[test]
fn scheduler_transition_steps_and_done() {
    let mut sched = crate::sched::Scheduler::default();
    let node = crate::NodeId {
        slot: 0,
        generation: 1,
    };
    let t0 = Instant::now();
    // a transition starts — earliest deadline is a frame step away
    let ev = sched.start_transition(node, MotionToken::Hover, t0);
    assert!(ev.is_none());
    let dl = sched.next_deadline().unwrap();
    assert_eq!(dl, t0 + FRAME_STEP);
    // before the deadline nothing is due
    assert!(sched.poll(t0 + Duration::from_millis(5)).is_empty());
    // at 16ms a step; at 150ms+ the done
    let evs = sched.poll(t0 + FRAME_STEP);
    assert!(
        evs.iter()
            .any(|e| matches!(e, crate::sched::SchedEvent::TransitionStep { .. }))
    );
    let evs2 = sched.poll(t0 + Duration::from_millis(TRANSITION_MS));
    assert!(
        evs2.iter()
            .any(|e| matches!(e, crate::sched::SchedEvent::TransitionDone { .. }))
    );
    assert!(sched.next_deadline().is_none());
}

#[test]
fn scheduler_reduce_during_activity_settles() {
    let mut sched = crate::sched::Scheduler::default();
    let node = crate::NodeId {
        slot: 0,
        generation: 1,
    };
    let t0 = Instant::now();
    let _ = sched.start_transition(node, MotionToken::Hover, t0);
    // flip to Reduce mid-activity => settled (TransitionDone), no deadline
    let evs = sched.set_reduced(true, t0 + Duration::from_millis(50));
    assert!(
        evs.iter()
            .any(|e| matches!(e, crate::sched::SchedEvent::TransitionDone { .. }))
    );
    assert!(sched.next_deadline().is_none());
}

#[test]
fn scheduler_tooltip_delay_semantic_under_reduce() {
    let mut sched = crate::sched::Scheduler::default();
    sched.reduced = true;
    let node = crate::NodeId {
        slot: 0,
        generation: 1,
    };
    let t0 = Instant::now();
    sched.arm_tooltip(node, t0);
    // semantic delay survives Reduce — the deadline is still armed
    assert_eq!(sched.next_deadline(), Some(t0 + TOOLTIP_DELAY));
    let evs = sched.poll(t0 + TOOLTIP_DELAY);
    assert!(
        evs.iter()
            .any(|e| matches!(e, crate::sched::SchedEvent::TooltipShow { .. }))
    );
}

#[test]
fn scheduler_initial_hidden_custom_no_deadline() {
    struct S {
        show: bool,
    }
    let mut rig = Rig::new(
        S { show: false },
        |_: &mut S, _: Msg, _: &mut UpdateCtx<Msg>| {},
        |s: &S, ui: &mut Ui<Msg>| {
            if s.show {
                ui.custom(Rc::new(Tile)).frame_events(true);
            }
        },
    );
    rig.view().unwrap();
    assert!(rig.rt.next_deadline().is_none());
    assert!(rig.rt.sched.is_idle());
}

#[test]
fn empty_pump_never_reviews() {
    struct S {
        views: u64,
    }
    let mut rig = Rig::new(
        S { views: 0 },
        |_: &mut S, _: Msg, _: &mut UpdateCtx<Msg>| {},
        |_: &S, _: &mut Ui<Msg>| {},
    );
    rig.view().unwrap();
    let v0 = rig.rt.view_count;
    rig.pump().unwrap();
    assert_eq!(rig.rt.view_count, v0); // no update => no review
}

#[test]
fn disabled_node_gets_no_callback() {
    struct S {
        hits: u32,
        disabled: bool,
    }
    let mut rig = Rig::new(
        S {
            hits: 0,
            disabled: true,
        },
        |s: &mut S, m: Msg, _cx: &mut UpdateCtx<Msg>| {
            if m == Msg::Press {
                s.hits += 1;
            }
        },
        |s: &S, ui: &mut Ui<Msg>| {
            ui.button("x").disabled(s.disabled).on_press(press_msg);
        },
    );
    rig.view().unwrap();
    let node = rig.root_children()[0];
    rig.push(node, NodeEvent::Press);
    rig.pump().unwrap();
    assert_eq!(rig.rt.state.hits, 0); // disabled => no callback
}

#[test]
fn hidden_node_gets_no_callback() {
    struct S {
        hits: u32,
    }
    let mut rig = Rig::new(
        S { hits: 0 },
        |s: &mut S, m: Msg, _cx: &mut UpdateCtx<Msg>| {
            if m == Msg::Press {
                s.hits += 1;
            }
        },
        |_: &S, ui: &mut Ui<Msg>| {
            ui.button("x")
                .visibility(Visibility::Hidden)
                .on_press(press_msg);
        },
    );
    rig.view().unwrap();
    let node = rig.root_children()[0];
    rig.push(node, NodeEvent::Press);
    rig.pump().unwrap();
    assert_eq!(rig.rt.state.hits, 0);
}

#[test]
fn unhandled_conflict_is_typed_diagnostic_once() {
    struct S {
        v: TextValue,
    }
    let mut rig = Rig::new(
        S {
            v: TextValue::new("a"),
        },
        |s: &mut S, m: Msg, _cx: &mut UpdateCtx<Msg>| {
            if let Msg::Edited(e) = m {
                let _ = s.v.accept(e);
            }
        },
        |s: &S, ui: &mut Ui<Msg>| {
            ui.text_input(&s.v).on_edit(Msg::Edited); // no on_conflict handler
        },
    );
    rig.view().unwrap();
    let node = editor_of(&rig);
    let binding = rig
        .rt
        .arena
        .get(node)
        .and_then(|n| match &n.data {
            NodeData::Editor { sync, .. } => sync.binding,
            _ => None,
        })
        .unwrap();
    rig.rt.state.v.replace("prog");
    rig.view().unwrap();
    let peer_rev = rig
        .rt
        .arena
        .get(node)
        .and_then(|n| match &n.data {
            NodeData::Editor { sync, .. } => Some(sync.peer_revision),
            _ => None,
        })
        .unwrap();
    rig.native_edit(
        node,
        "native",
        peer_rev.raw_val(),
        peer_rev.raw_val() + 1,
        EditOrigin::NativePeer,
        binding,
    );
    rig.pump().unwrap();
    rig.view().unwrap();
    rig.view().unwrap(); // the same rejected proposal must NOT re-diagnose
    let n = rig
        .rt
        .diagnostics
        .iter()
        .filter(|d| **d == UiDiagnostic::UnhandledTextConflict)
        .count();
    assert_eq!(n, 1);
}

// ============================ rework round-2 regressions =====================

thread_local! {
    static WAKE_CTR: RefCell<Arc<Mutex<u32>>> = RefCell::new(Arc::new(Mutex::new(0)));
}

/// Documented counter example must compile verbatim — elided `Ui` lifetime,
/// `&str` label, on_press factory closure (API_EXAMPLES counter).
#[test]
fn counter_example_documented_shape_compiles() {
    // exact documented shape — API_EXAMPLES counter, as-written
    struct S {
        count: i32,
    }
    enum CMsg {
        Increment,
    }
    let _app = App::new(
        S { count: 0 },
        |s: &mut S, m: CMsg, _cx: &mut UpdateCtx<CMsg>| {
            let CMsg::Increment = m;
            s.count += 1;
        },
        |s: &S, ui: &mut Ui<CMsg>| {
            ui.column(crate::Column::new().gap(crate::Space::Md), |ui| {
                ui.label(&s.count.to_string());
                ui.label(format!("count: {}", s.count));
                ui.button("Increment").on_press(|| CMsg::Increment);
            });
        },
    );
}

/// Executable counter on the real Runtime path.
#[test]
fn counter_example_executes_press() {
    struct S {
        count: i32,
    }
    let mut rig = Rig::new(
        S { count: 0 },
        |s: &mut S, m: Msg, _cx: &mut UpdateCtx<Msg>| {
            if m == Msg::Press {
                s.count += 1;
            }
        },
        |s: &S, ui: &mut Ui<Msg>| {
            ui.label(s.count.to_string()); // owned String — no extra borrow dance
            ui.button("Increment").on_press(press_msg);
        },
    );
    rig.view().unwrap();
    let btn = rig.root_children()[1];
    rig.push(btn, NodeEvent::Press);
    rig.pump().unwrap();
    assert_eq!(rig.rt.state.count, 1);
}

/// Root `M` need not be `Send` — `scope`/`cancel` work for !Send messages and
/// the scope adapter may capture `Rc` (runs UI-side only).
#[test]
fn scope_adapter_captures_rc_and_root_msg_need_not_send() {
    use std::rc::Rc as RRc;
    // !Send + !Sync message (Rc inside)
    #[derive(Debug)]
    enum LocalMsg {
        Tagged(RRc<String>),
    }
    impl PartialEq for LocalMsg {
        fn eq(&self, o: &Self) -> bool {
            match (self, o) {
                (LocalMsg::Tagged(a), LocalMsg::Tagged(b)) => a == b,
            }
        }
    }
    struct S {
        got: Vec<String>,
    }
    let tag = RRc::new("rc-tag".to_string());
    let tag2 = tag.clone();
    let app = App::new(
        S { got: Vec::new() },
        Box::new(
            move |s: &mut S, m: LocalMsg, cx: &mut UpdateCtx<LocalMsg>| {
                match m {
                    LocalMsg::Tagged(sv) => s.got.push((*sv).clone()),
                }
                // scope works for a !Send root msg — adapter captures an Rc
                let captured = tag.clone();
                let mut child = cx.scope("child-ns", move |c: u32| {
                    LocalMsg::Tagged(RRc::new(format!("{}:{c}", captured)))
                });
                child.cancel("any"); // cancel must not require Send either
            },
        ) as Box<dyn Fn(&mut S, LocalMsg, &mut UpdateCtx<LocalMsg>)>,
        Box::new(|_: &S, ui: &mut Ui<'_, '_, LocalMsg>| {
            ui.button("go")
                .on_press(|| LocalMsg::Tagged(RRc::new("x".into())));
        }) as Box<dyn Fn(&S, &mut Ui<'_, '_, LocalMsg>)>,
    );
    let peers = PeerLog::default();
    let pf = peers.clone();
    let mut rt = runtime_for(
        app,
        Box::new(move |spec| {
            Ok(Box::new(FakePeer::new(pf.clone(), spec.multiline))
                as Box<dyn crate::node::TextPeer>)
        }),
        Theme::light(),
        Appearance {
            dark: false,
            forced_colors: false,
        },
    );
    rt.review_for_test().unwrap();
    let btn = rt.root_children()[0];
    rt.test_push_event(QueuedEvent {
        node: btn,
        payload: NodeEvent::Press,
    });
    rt.pump().unwrap();
    assert_eq!(rt.state.got, vec!["x".to_string()]);
    let _ = tag2;
}

/// B: install_wake fires the callback once for pre-installed queued work.
#[test]
fn install_wake_fires_for_earlier_proxy_send() {
    struct S {
        presses: u32,
    }
    let mut rig = Rig::new(
        S { presses: 0 },
        |s: &mut S, m: Msg, _cx: &mut UpdateCtx<Msg>| {
            if m == Msg::Press {
                s.presses += 1;
            }
        },
        |_: &S, _: &mut Ui<Msg>| {},
    );
    rig.view().unwrap();
    // a proxy send BEFORE any wake seam is installed must not be stranded
    let proxy = crate::UiProxy::new(rig.rt.mailbox.clone());
    proxy.try_send(Msg::Press).unwrap();
    // now the run-loop installs its wake seam — must observe the queued work
    let calls = Arc::new(Mutex::new(0u32));
    let c2 = calls.clone();
    rig.rt
        .mailbox
        .install_wake(Arc::new(move || {
            *c2.lock().unwrap() += 1;
            true
        }))
        .unwrap();
    assert_eq!(*calls.lock().unwrap(), 1, "install must fire once");
    rig.pump().unwrap();
    assert_eq!(rig.rt.state.presses, 1, "the pre-install message delivered");
}

/// B: cancel wakes every distinct cancelled() consumer + the watch — and the
/// waker may reenter queue state without deadlock.
#[test]
fn cancel_wakes_all_consumers_and_reentrant_waker_is_safe() {
    struct S {
        kicks: u32,
    }
    // waker that reenters queue observation from inside wake() — proves the
    // cancellation wake happens outside the waiter-set lock
    let wakes = Arc::new(Mutex::new(0u32));
    struct ReWaker;
    impl Wake for ReWaker {
        fn wake(self: Arc<Self>) {
            // reenters the cancellation counter from inside wake() — safe
            // only because wakes happen outside the waiter-set lock
            WAKE_CTR.with(|c| *c.borrow().lock().unwrap() += 1);
        }
    }
    let waker_ctr = wakes.clone();
    let mut rig = Rig::new(
        S { kicks: 0 },
        |s: &mut S, m: Msg, cx: &mut UpdateCtx<Msg>| {
            if m == Msg::Kick {
                s.kicks += 1;
                let _ = cx.spawn("k", |job| async move {
                    // TWO independent cancelled() consumers + the internal
                    // CancelWatch — three distinct waiter slots total
                    let cancel = job.cancellation();
                    let mut a = Box::pin(cancel.cancelled());
                    let mut b = Box::pin(cancel.cancelled());
                    {
                        let w = std::task::Waker::from(std::sync::Arc::new(ReWaker));
                        let mut ctx = Context::from_waker(&w);
                        assert!(a.as_mut().poll(&mut ctx).is_pending());
                        assert!(b.as_mut().poll(&mut ctx).is_pending());
                    }
                    // one consumer retires before cancel — slot must free
                    drop(b);
                    std::future::pending::<()>().await;
                });
            }
        },
        |_: &S, _: &mut Ui<Msg>| {},
    );
    rig.view().unwrap();
    WAKE_CTR.with(|c| *c.borrow_mut() = waker_ctr);
    rig.rt.push_msg(Msg::Kick);
    rig.exec.poll();
    // cancel fires: watch + surviving consumer a both wake (the retired b
    // slot is already gone — three distinct slots were registered, one
    // removed on drop)
    {
        let mut ctx = rig.rt.ctx_for_test();
        ctx.cancel("k");
    }
    // consumer a's waker fired (b's slot was dropped — no retired wake)
    assert_eq!(*wakes.lock().unwrap(), 1, "surviving consumer woke once");
    // the watch's own waker re-queued the task — it exits on next poll
    rig.exec.poll();
    assert_eq!(rig.rt.registry.live_count(), 0);
}

/// B: install_wake must deliver a LIFECYCLE pending even with an empty
/// envelope queue — a completed task's last charge is pure lifecycle work.
#[test]
fn install_wake_fires_for_lifecycle_pending() {
    struct S;
    let mut rig = Rig::new(
        S,
        |_: &mut S, _: Msg, _: &mut UpdateCtx<Msg>| {},
        |_: &S, _: &mut Ui<Msg>| {},
    );
    rig.view().unwrap();
    // a bare lifecycle poke (no envelope) before install must still fire
    rig.rt.mailbox.poke_ui();
    let calls = Arc::new(Mutex::new(0u32));
    let c2 = calls.clone();
    rig.rt
        .mailbox
        .install_wake(Arc::new(move || {
            *c2.lock().unwrap() += 1;
            true
        }))
        .unwrap();
    assert_eq!(*calls.lock().unwrap(), 1);
}

// ---------------------------------------------------------------------------
// F01 overlap — lifecycle signal vs producer acceptance
// ---------------------------------------------------------------------------
