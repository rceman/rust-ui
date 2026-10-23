#[test]
fn moved_textvalue_unmounts_old_before_new() {
    struct S {
        v: TextValue,
        left: bool,
    }
    let mut rig = Rig::new(
        S {
            v: TextValue::new("mv"),
            left: true,
        },
        |s: &mut S, m: Msg, _cx: &mut UpdateCtx<Msg>| {
            if let Msg::Edited(e) = m {
                let _ = s.v.accept(e);
            }
        },
        |s: &S, ui: &mut Ui<Msg>| {
            ui.group("l", |ui| {
                if s.left {
                    ui.text_input(&s.v).on_edit(Msg::Edited);
                }
            });
            ui.group("r", |ui| {
                if !s.left {
                    ui.text_input(&s.v).on_edit(Msg::Edited);
                }
            });
        },
    );
    rig.view().unwrap();
    let kids = rig.root_children();
    let l_node = rig
        .rt
        .arena
        .get(kids[0])
        .map(|n| n.children.clone())
        .unwrap_or_default();
    let peer_l = rig.peer_id(crate::NodeId {
        slot: l_node[0],
        generation: rig.rt.arena.generation_of(l_node[0]),
    });
    assert!(peer_l.is_some());

    // move the same TextValue to the right group — old peer must release
    // BEFORE the new mount claims the lease
    rig.rt.state.left = false;
    rig.pump().unwrap();
    rig.view().unwrap();
    let kids2 = rig.root_children();
    let r_node = rig
        .rt
        .arena
        .get(kids2[1])
        .map(|n| n.children.clone())
        .unwrap_or_default();
    let peer_r = rig.peer_id(crate::NodeId {
        slot: r_node[0],
        generation: rig.rt.arena.generation_of(r_node[0]),
    });
    assert!(peer_r.is_some());
    assert_eq!(rig.peer_rec(peer_l.unwrap()).released, 1);
}

#[test]
fn utf16_conversions_are_checked() {
    let s = "a🦀b\u{0301}"; // ascii + emoji + combining mark
    // surrogate-pair interior and out-of-range must reject
    let u16s: Vec<u16> = s.encode_utf16().collect();
    let back = crate::text::utf16_to_utf8(&u16s).unwrap();
    assert_eq!(back, s);
    // lone surrogate is invalid
    assert!(crate::text::utf16_to_utf8(&[0xD800]).is_err());
    // utf16 index 0 -> byte 0; index inside emoji pair -> None
    assert_eq!(crate::text::utf16_index_to_utf8(s, 0), Some(0));
    assert_eq!(crate::text::utf16_index_to_utf8(s, 2), None); // inside 🦀
    assert!(crate::text::utf16_index_to_utf8(s, 1000).is_none());
    // byte 1 is inside 'a'? no — boundary checks
    assert!(crate::text::utf8_index_to_utf16(s, 1).is_some());
    assert!(crate::text::utf8_index_to_utf16(s, 2).is_none()); // inside 🦀
}

// ============================ tasks ===========================================

#[test]
fn task_cancel_replacement_fences() {
    struct S {
        got: Vec<u32>,
    }
    let mut rig = Rig::new(
        S { got: Vec::new() },
        |s: &mut S, m: Msg, cx: &mut UpdateCtx<Msg>| {
            match m {
                Msg::Chunk(v) => s.got.push(v),
                Msg::Kick => {
                    // spawn A then B under the same key — A is fenced
                    let _ = cx.spawn("k", |job| async move {
                        let _ = job.send(Msg::Chunk(1)).await;
                        let _ = job.send(Msg::Chunk(2)).await;
                    });
                    let _ = cx.spawn("k", |job| async move {
                        let _ = job.send(Msg::Chunk(10)).await;
                    });
                }
                _ => {}
            }
        },
        |_: &S, _: &mut Ui<Msg>| {},
    );
    rig.view().unwrap();
    rig.rt.push_msg(Msg::Kick);
    rig.exec.poll();
    rig.pump().unwrap();
    // A's chunks were fenced before delivery; only B's chunk lands
    assert!(rig.rt.state.got.iter().all(|&v| v != 1 && v != 2));
    assert!(rig.rt.state.got.contains(&10));
}

#[test]
fn sends_after_cancel_return_err() {
    struct S {
        phase: u8,
    }
    let held: Arc<Mutex<Option<crate::TaskSender<Msg>>>> = Arc::new(Mutex::new(None));
    let h2 = held.clone();
    let mut rig = Rig::new(
        S { phase: 0 },
        move |s: &mut S, m: Msg, cx: &mut UpdateCtx<Msg>| {
            let _ = m;
            match s.phase {
                0 => {
                    s.phase = 1;
                    let h3 = h2.clone();
                    let _ = cx.spawn("k", move |job| {
                        let sender = job.sender().clone();
                        *h3.lock().unwrap() = Some(sender);
                        async move { std::future::pending::<()>().await }
                    });
                }
                1 => {
                    s.phase = 2;
                    cx.cancel("k");
                }
                _ => {}
            }
        },
        |_: &S, _: &mut Ui<Msg>| {},
    );
    rig.view().unwrap();
    rig.rt.push_msg(Msg::Kick);
    rig.exec.poll();
    rig.rt.push_msg(Msg::Kick);
    let sender = held.lock().unwrap().clone().unwrap();
    let mut send = Box::pin(sender.send(Msg::Chunk(42)));
    let w = task_waker(Arc::new(Mutex::new(Exec::default())), 0);
    let mut cx = Context::from_waker(&w);
    assert!(matches!(
        send.as_mut().poll(&mut cx),
        Poll::Ready(Err(crate::SendError::Cancelled | crate::SendError::Closed))
    ));
}

#[test]
fn completed_registration_reclaims_after_drain() {
    struct S {
        got: Vec<u32>,
    }
    let mut rig = Rig::new(
        S { got: Vec::new() },
        |s: &mut S, m: Msg, cx: &mut UpdateCtx<Msg>| match m {
            Msg::Chunk(v) => s.got.push(v),
            Msg::Kick => {
                let _ = cx.spawn("t", |job| async move {
                    let _ = job.send(Msg::Chunk(5)).await;
                    let _ = job.send(Msg::Chunk(6)).await;
                });
            }
            _ => {}
        },
        |_: &S, _: &mut Ui<Msg>| {},
    );
    rig.view().unwrap();
    rig.rt.push_msg(Msg::Kick);
    rig.exec.poll();
    rig.pump().unwrap();
    assert_eq!(rig.rt.state.got, vec![5, 6]);
    // completed + drained => reclaimed
    rig.exec.poll(); // let the CancelWatch observe completion
    rig.pump().unwrap();
    assert_eq!(rig.rt.registry.live_count(), 0);
}

#[test]
fn bounded_mailbox_64_then_65th_waits() {
    struct S {
        got: Vec<u32>,
    }
    let mut rig = Rig::new(
        S { got: Vec::new() },
        |s: &mut S, m: Msg, cx: &mut UpdateCtx<Msg>| match m {
            Msg::Chunk(v) => s.got.push(v),
            Msg::Kick => {
                let _ = cx.spawn("fill", |job| async move {
                    for i in 0..65u32 {
                        if job.send(Msg::Chunk(i)).await.is_err() {
                            break;
                        }
                    }
                });
            }
            _ => {}
        },
        |_: &S, _: &mut Ui<Msg>| {},
    );
    rig.view().unwrap();
    rig.rt.push_msg(Msg::Kick);
    rig.exec.poll();
    // 64 land; 65th parks
    assert_eq!(rig.rt.mailbox.queue_len(), 64);
    rig.pump().unwrap();
    // draining frees capacity — the parked send lands
    rig.exec.poll();
    rig.pump().unwrap();
    rig.exec.poll();
    rig.pump().unwrap();
    // everything arrives in order
    for (i, v) in rig.rt.state.got.iter().enumerate() {
        assert_eq!(*v, i as u32);
    }
}

#[test]
fn proxy_reports_typed_full_and_closed() {
    struct S;
    let mut rig = Rig::new(
        S,
        |_: &mut S, _: Msg, _: &mut UpdateCtx<Msg>| {},
        |_: &S, _: &mut Ui<Msg>| {},
    );
    rig.view().unwrap();
    // fill via direct try_push is internal; use a proxy-shaped check:
    // UiProxy<M> is produced by App::proxy — route through runtime mailbox
    let proxy: crate::UiProxy<Msg> = crate::UiProxy::new(rig.rt.mailbox.clone());
    for _ in 0..64 {
        proxy.try_send(Msg::Press).unwrap();
    }
    assert!(matches!(
        proxy.try_send(Msg::Press),
        Err(crate::ProxySendError::Full)
    ));
    rig.rt.shutdown();
    assert!(matches!(
        proxy.try_send(Msg::Press),
        Err(crate::ProxySendError::Closed)
    ));
}

#[test]
fn scoped_task_namespaces_are_independent() {
    struct S {
        a: Vec<u32>,
        b: Vec<u32>,
    }
    let mut rig = Rig::new(
        S {
            a: Vec::new(),
            b: Vec::new(),
        },
        |s: &mut S, m: Msg, cx: &mut UpdateCtx<Msg>| match m {
            Msg::Scoped(side, v) => {
                if side == 0 {
                    s.a.push(v)
                } else {
                    s.b.push(v)
                }
            }
            Msg::Kick => {
                let mut s1 = cx.scope("one", |c: u32| Msg::Scoped(0, c));
                let _ = s1.spawn("k", |job| async move {
                    let _ = job.send(1u32).await;
                });
                let mut s2 = cx.scope("two", |c: u32| Msg::Scoped(1, c));
                let _ = s2.spawn("k", |job| async move {
                    let _ = job.send(2u32).await;
                });
            }
            _ => {}
        },
        |_: &S, _: &mut Ui<Msg>| {},
    );
    rig.view().unwrap();
    rig.rt.push_msg(Msg::Kick);
    rig.exec.poll();
    rig.pump().unwrap();
    assert_eq!(rig.rt.state.a, vec![1]);
    assert_eq!(rig.rt.state.b, vec![2]);
}

#[test]
fn unique_spawn_keys_leave_zero_registrations() {
    struct S {
        n: u32,
    }
    let mut rig = Rig::new(
        S { n: 0 },
        |s: &mut S, m: Msg, cx: &mut UpdateCtx<Msg>| {
            if let Msg::Chunk(v) = m {
                s.n = v;
                // values < 12 spawn a uniquely-keyed task that sends v+100 —
                // the >=100 chunks terminate the chain
                if v < 12 {
                    let key = format!("t{v}");
                    let next = v + 100;
                    let _ = cx.spawn(key, move |job| async move {
                        let _ = job.send(Msg::Chunk(next)).await;
                    });
                }
            }
        },
        |_: &S, _: &mut Ui<Msg>| {},
    );
    rig.view().unwrap();
    for i in 0..12u32 {
        rig.rt.push_msg(Msg::Chunk(i));
    }
    // drain until quiet — chain tasks spawn then complete
    for _ in 0..40 {
        rig.exec.poll();
        rig.pump().unwrap();
        if rig.rt.registry.live_count() == 0 && rig.rt.mailbox.queue_len() == 0 {
            break;
        }
    }
    // every spawned task completed and drained — nothing lingers
    assert_eq!(rig.rt.registry.live_count(), 0);
}

#[test]
fn spawn_without_executor_is_typed_error() {
    struct S;
    let mut rig = Rig::new(
        S,
        |_: &mut S, _: Msg, _cx: &mut UpdateCtx<Msg>| {},
        |_: &S, _: &mut Ui<Msg>| {},
    );
    rig.view().unwrap();
    // strip the executor — spawn must return the typed NoExecutor error
    rig.rt.executor = None;
    rig.rt.push_msg(Msg::Kick);
    let mut cx = rig.rt.ctx_for_test();
    assert!(matches!(
        cx.spawn("k", |job| async move {
            let _ = job.send(Msg::Press).await;
        }),
        Err(TaskStartError::NoExecutor)
    ));
}

#[test]
fn executor_rejection_fences_registration() {
    struct S;
    struct RejectAll;
    impl Executor for RejectAll {
        fn spawn(&self, _t: BoxFuture<()>) -> Result<(), TaskStartError> {
            Err(TaskStartError::Rejected)
        }
    }
    let peers = PeerLog::default();
    let pf = peers.clone();
    let app = App::new(
        S,
        Box::new(|_: &mut S, _: Msg, _: &mut UpdateCtx<Msg>| {})
            as Box<dyn Fn(&mut S, Msg, &mut UpdateCtx<Msg>)>,
        Box::new(|_: &S, _: &mut Ui<'_, '_, Msg>| {}) as Box<dyn Fn(&S, &mut Ui<'_, '_, Msg>)>,
    );
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
    rt.executor = Some(Arc::new(RejectAll));
    rt.review_for_test().unwrap();
    let mut cx = rt.ctx_for_test();
    let r = cx.spawn("k", |job| async move {
        let _ = job.send(Msg::Press).await;
    });
    assert!(matches!(r, Err(TaskStartError::Rejected)));
    assert_eq!(rt.registry.live_count(), 0); // rejected spawn left nothing
}

#[test]
fn a_hundred_replacements_leave_zero_tombstones() {
    struct S {
        n: u32,
    }
    let mut rig = Rig::new(
        S { n: 0 },
        |s: &mut S, m: Msg, cx: &mut UpdateCtx<Msg>| {
            match m {
                Msg::Chunk(v) => s.n = v,
                Msg::Kick => {
                    // every spawn replaces "k" — no tombstones accumulate
                    for _ in 0..100 {
                        let _ = cx.spawn("k", |job| async move {
                            let _ = job.send(Msg::Chunk(1)).await;
                        });
                    }
                }
                _ => {}
            }
        },
        |_: &S, _: &mut Ui<Msg>| {},
    );
    rig.view().unwrap();
    rig.rt.push_msg(Msg::Kick);
    rig.exec.poll();
    rig.pump().unwrap();
    rig.exec.poll();
    rig.pump().unwrap();
    assert_eq!(rig.rt.registry.live_count(), 0);
}

// ============================ scheduler =======================================

struct Tile;
impl CustomRender for Tile {
    fn measure(&self, c: crate::geom::Constraints) -> crate::geom::Size {
        c.constrain(crate::geom::Size::new(
            crate::geom::dp(10.0),
            crate::geom::dp(10.0),
        ))
    }
    fn paint(&self, _c: &mut dyn crate::Canvas, _b: crate::geom::Rect) {}
    fn semantics(&self) -> crate::ui::Semantics {
        crate::ui::Semantics {
            role: crate::ui::Role::Custom,
            label: "tile".into(),
            actions: Vec::new(),
        }
    }
}

#[test]
fn scheduler_deadline_lifecycle() {
    struct S {
        show: bool,
    }
    let mut rig = Rig::new(
        S { show: true },
        |_: &mut S, _: Msg, _: &mut UpdateCtx<Msg>| {},
        |s: &S, ui: &mut Ui<Msg>| {
            if s.show {
                ui.custom(Rc::new(Tile))
                    .frame_events(true)
                    .on_frame(|_| Msg::Press);
            }
        },
    );
    rig.view().unwrap();
    // one activity => one deadline
    assert!(rig.rt.next_deadline().is_some());

    // settle: stop demand => no deadline
    rig.rt.state.show = false;
    rig.pump().unwrap();
    rig.view().unwrap();
    assert!(rig.rt.next_deadline().is_none());
    assert!(rig.rt.sched.is_idle());
}

#[test]
fn scheduler_reduced_motion_settles_and_stays_quiet() {
    struct S;
    let mut rig = Rig::new(
        S,
        |_: &mut S, _: Msg, _: &mut UpdateCtx<Msg>| {},
        |_: &S, ui: &mut Ui<Msg>| {
            ui.custom(Rc::new(Tile))
                .frame_events(true)
                .on_frame(|_| Msg::Press);
        },
    );
    rig.rt.theme = Theme::light().reduced_motion(ReducedMotion::Reduce);
    rig.rt.sched.reduced = true;
    rig.view().unwrap();
    // reduced motion: zero animation callbacks
    let evs = rig.rt.pump_sched(Instant::now()).unwrap();
    assert!(
        evs.iter()
            .all(|e| !matches!(e, crate::sched::SchedEvent::TransitionStep { .. }))
    );
    // no frame demand while reduced
    rig.pump().unwrap();
    assert!(rig.rt.sched.is_idle());
}
