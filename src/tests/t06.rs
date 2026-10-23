/// F01 — one Confirmed edge covers any number of pushes: the installed
/// seam is invoked once until wake_seen consumes the edge.
#[test]
fn confirmed_edge_coalesces_across_pushes() {
    let mb = crate::tasks::Mailbox::new();
    let calls = Arc::new(Mutex::new(0u32));
    let c2 = calls.clone();
    mb.install_wake(Arc::new(move || {
        *c2.lock().unwrap() += 1;
        true
    }))
    .unwrap();
    let proxy = crate::tasks::UiProxy::<u32>::new(mb.clone());
    proxy.try_send(1).unwrap();
    proxy.try_send(2).unwrap();
    proxy.try_send(3).unwrap();
    assert_eq!(
        *calls.lock().unwrap(),
        1,
        "a single Confirmed edge covers all three pushes"
    );
    // FIFO, exactly-once
    for want in [1u32, 2, 3] {
        let env = mb.pop().expect("queued");
        assert_eq!(*env.payload.downcast::<u32>().unwrap(), want);
    }
    assert!(mb.pop().is_none());
    // wake_seen consumes the edge — the next push signals afresh
    mb.wake_seen();
    proxy.try_send(4).unwrap();
    assert_eq!(*calls.lock().unwrap(), 2, "a consumed edge re-signals");
}

/// F01 — pre-install acceptance accrues a wake OBLIGATION (Uninstalled
/// owed), not a delivered edge: a failing install reports typed Wake,
/// terminal-closes, drains the queued work (payload drops exactly once),
/// and later sends see Closed.
#[test]
fn preinstall_owed_then_failing_install_reports_wake() {
    use crate::tasks::{Mailbox, ProxySendError, UiProxy};
    let mb = Mailbox::new();
    let drops = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let proxy = UiProxy::<DropPayload>::new(mb.clone());
    // accepted pre-install — the obligation accrues, nothing signaled
    proxy
        .try_send(DropPayload {
            id: 1,
            drops: drops.clone(),
        })
        .unwrap();
    assert_eq!(mb.queue_len(), 1);
    // the failing seam owes the pulse — it fails under the same lock that
    // serialized the acceptance
    assert_eq!(
        mb.install_wake(std::sync::Arc::new(|| false)),
        Err(ProxySendError::Wake)
    );
    assert!(mb.is_closed(), "failed install terminal-closes");
    assert_eq!(mb.queue_len(), 0, "terminal drain empties the queue");
    assert_eq!(
        drops.load(Ordering::SeqCst),
        1,
        "the queued payload dropped exactly once"
    );
    assert!(mb.take_wake_failure(), "the failure is latched");
    assert_eq!(
        proxy.try_send(DropPayload { id: 2, drops }),
        Err(ProxySendError::Closed)
    );
}

/// F01 — a lifecycle-only obligation (bare poke, empty queue) survives
/// `wake_seen` — only `install_wake` consumes it; a failing install then
/// reports typed Wake and terminal-closes.
#[test]
fn lifecycle_owed_survives_wake_seen_until_install() {
    use crate::tasks::{Mailbox, ProxySendError};
    let mb = Mailbox::new();
    assert!(mb.poke_ui(), "pre-install poke accrues the obligation");
    mb.wake_seen(); // consumes only Confirmed — owed survives
    assert_eq!(
        mb.install_wake(std::sync::Arc::new(|| false)),
        Err(ProxySendError::Wake)
    );
    assert!(mb.is_closed());
    // healthy variant — an owed lifecycle poke installs and pulses once
    let mb2 = Mailbox::new();
    assert!(mb2.poke_ui());
    mb2.wake_seen();
    let calls = Arc::new(Mutex::new(0u32));
    let c2 = calls.clone();
    mb2.install_wake(Arc::new(move || {
        *c2.lock().unwrap() += 1;
        true
    }))
    .unwrap();
    assert_eq!(*calls.lock().unwrap(), 1, "the owed obligation pulses once");
    // a second wake_seen with nothing new does not re-pulse
    mb2.wake_seen();
    assert_eq!(*calls.lock().unwrap(), 1);
}

/// F01 — nothing owed, empty queue: install is Ok with NO pulse — the
/// first real send establishes the signal.
#[test]
fn install_with_no_obligation_does_not_pulse() {
    let mb = crate::tasks::Mailbox::new();
    let calls = Arc::new(Mutex::new(0u32));
    let c2 = calls.clone();
    mb.install_wake(Arc::new(move || {
        *c2.lock().unwrap() += 1;
        true
    }))
    .unwrap();
    assert_eq!(*calls.lock().unwrap(), 0, "no pulse until actual work");
    crate::tasks::UiProxy::<u32>::new(mb.clone())
        .try_send(9)
        .unwrap();
    assert_eq!(*calls.lock().unwrap(), 1, "the first send signals");
}

/// B: purging/close drops charges outside the lock — envelope Charge
/// drops and waiter wakers are the permitted reentry AFTER unlock; the
/// installed signal callback itself NEVER reenters mailbox state.
#[test]
fn purge_and_close_release_charges_outside_the_lock() {
    struct S;
    let mut rig = Rig::new(
        S,
        |_: &mut S, _: Msg, _: &mut UpdateCtx<Msg>| {},
        |_: &S, _: &mut Ui<Msg>| {},
    );
    rig.view().unwrap();
    let mb = rig.rt.mailbox.clone();
    let calls = Arc::new(Mutex::new(0u32));
    let c2 = calls.clone();
    mb.install_wake(Arc::new(move || {
        *c2.lock().unwrap() += 1;
        true
    }))
    .unwrap();
    // a task completes after queueing a send — the envelope's charge is the
    // last outstanding work; dropping it via close wakes the loop
    {
        let mut cx = rig.rt.ctx_for_test();
        let _ = cx.spawn("s", |job| async move {
            let _ = job.send(Msg::Chunk(1)).await;
        });
    }
    rig.exec.poll();
    rig.rt.mailbox.close();
    assert!(*calls.lock().unwrap() >= 1);
    // close cleared the seam — a later poke cannot resurrect it
    mb.poke_ui();
    let n = *calls.lock().unwrap();
    mb.poke_ui();
    assert_eq!(*calls.lock().unwrap(), n);
}

/// B: a completed registration whose last envelope drains must reap even
/// when the completion wake is the only signal (no new envelope).
#[test]
fn last_envelope_charge_drop_lifecycle_wakes_reap() {
    struct S {
        got: Vec<u32>,
    }
    let mut rig = Rig::new(
        S { got: Vec::new() },
        |s: &mut S, m: Msg, cx: &mut UpdateCtx<Msg>| {
            match m {
                Msg::Chunk(v) => s.got.push(v),
                Msg::Kick => {
                    let _ = cx.spawn("d", |job| async move {
                        let _ = job.send(Msg::Chunk(1)).await;
                        // completes — registration stays until drain
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
    // chunk queued + completed flag; the charge lives on the envelope —
    // drop(drain) releases it -> reap must run without a new envelope
    rig.pump().unwrap();
    assert_eq!(rig.rt.state.got, vec![1]);
    assert_eq!(rig.rt.registry.live_count(), 0);
}

/// C: consecutive frames keep nonzero deltas; unchanged re-view preserves the
/// due time (no reset); hidden stays deadline-free across Reduce toggles.
#[test]
fn frame_demand_deltas_and_unchanged_view() {
    struct S {
        deltas: Vec<Duration>,
        hidden: bool,
    }
    let mut rig = Rig::new(
        S {
            deltas: Vec::new(),
            hidden: false,
        },
        |s: &mut S, m: Msg, _cx: &mut UpdateCtx<Msg>| {
            if let Msg::Frame(d) = m {
                s.deltas.push(d);
            }
        },
        |s: &S, ui: &mut Ui<Msg>| {
            ui.custom(Rc::new(Tile))
                .visibility(if s.hidden {
                    Visibility::Hidden
                } else {
                    Visibility::Visible
                })
                .frame_events(true)
                .on_frame(|t: FrameTime| Msg::Frame(t.delta));
        },
    );
    rig.view().unwrap();
    let t0 = Instant::now();
    // three consecutive frame deliveries: deltas ~0, then 16ms, 16ms
    rig.rt.pump_sched(t0).unwrap();
    rig.pump().unwrap();
    rig.rt.pump_sched(t0 + FRAME_STEP).unwrap();
    rig.pump().unwrap();
    rig.rt.pump_sched(t0 + FRAME_STEP * 2).unwrap();
    rig.pump().unwrap();
    assert_eq!(rig.rt.state.deltas.len(), 3);
    assert_eq!(rig.rt.state.deltas[0], Duration::ZERO); // first ~0
    assert_eq!(rig.rt.state.deltas[1], FRAME_STEP); // nonzero later deltas
    assert_eq!(rig.rt.state.deltas[2], FRAME_STEP);

    // an unchanged re-view must NOT reset the next deadline
    let before = rig.rt.next_deadline().unwrap();
    rig.view().unwrap();
    assert_eq!(rig.rt.next_deadline().unwrap(), before);

    // hidden => parked; Reduce flips do NOT resume it
    rig.rt.state.hidden = true;
    rig.pump().unwrap();
    rig.view().unwrap();
    rig.rt.sched.set_reduced(true, Instant::now());
    rig.rt.sched.set_reduced(false, Instant::now());
    assert!(rig.rt.next_deadline().is_none());

    // visible again => first delta ~0, then 16ms steps
    rig.rt.state.hidden = false;
    rig.pump().unwrap();
    rig.view().unwrap();
    let now = Instant::now();
    let n0 = rig.rt.state.deltas.len();
    rig.rt.pump_sched(now).unwrap();
    rig.pump().unwrap();
    rig.rt.pump_sched(now + FRAME_STEP).unwrap();
    rig.pump().unwrap();
    assert_eq!(rig.rt.state.deltas[n0], Duration::ZERO);
    assert_eq!(rig.rt.state.deltas[n0 + 1], FRAME_STEP);
}

/// C: a custom node mounted while Reduce is effective never schedules.
#[test]
fn initial_reduce_custom_never_schedules() {
    struct S;
    let mut rig = Rig::new(
        S,
        |_: &mut S, _: Msg, _: &mut UpdateCtx<Msg>| {},
        |_: &S, ui: &mut Ui<Msg>| {
            ui.custom(Rc::new(Tile)).frame_events(true);
        },
    );
    rig.rt.theme = Theme::light().reduced_motion(ReducedMotion::Reduce);
    rig.rt.sched.reduced = true;
    rig.view().unwrap();
    assert!(rig.rt.next_deadline().is_none());
    assert!(rig.rt.sched.is_idle());
}

/// D: a stale conflict snapshot behind newer native commits is ignored even
/// when the pending request matches.
#[test]
fn stale_conflict_behind_newer_commits_is_ignored() {
    struct S {
        v: TextValue,
        outcomes: Vec<AcceptOutcome>,
    }
    let mut rig = Rig::new(
        S {
            v: TextValue::new("a"),
            outcomes: Vec::new(),
        },
        |s: &mut S, m: Msg, _cx: &mut UpdateCtx<Msg>| match m {
            Msg::Edited(e) => {
                let _ = s.v.accept(e);
            }
            Msg::Conflicted(c) => s.outcomes.push(s.v.keep_native(c)),
            _ => {}
        },
        |s: &S, ui: &mut Ui<Msg>| {
            ui.text_input(&s.v)
                .on_edit(Msg::Edited)
                .on_conflict(Msg::Conflicted);
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

    // pending proposal queued while composing — never leaves the value
    rig.rt.composition_start(node);
    rig.rt.state.v.replace("pend");
    rig.view().unwrap();
    // native commits land while composing (peer mirror + app both advance)
    let pr = rig
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
        "n1",
        pr.raw_val(),
        pr.raw_val() + 1,
        EditOrigin::NativePeer,
        binding,
    );
    rig.native_edit(
        node,
        "n1x",
        pr.raw_val() + 1,
        pr.raw_val() + 2,
        EditOrigin::NativePeer,
        binding,
    );
    rig.pump().unwrap();
    assert_eq!(&*rig.rt.state.v.text(), "n1x"); // revision advanced past them

    // a conflict DELAYED behind those commits — names the pending request,
    // but its committed snapshot is behind the app's current revision
    let req = rig.rt.state.v.pending().map(|p| p.requested).unwrap();
    let stale = TextConflict {
        rejected_revision: req,
        rejected_text: "pend".into(),
        committed_revision: TextRevision::raw(pr.raw_val() + 1), // < revision
        committed_text: "n1".into(),
        binding,
    };
    let out = rig.rt.state.v.keep_native(stale);
    assert_eq!(out, AcceptOutcome::IgnoredStale);
    assert_eq!(&*rig.rt.state.v.text(), "n1x"); // newer text remains
    assert!(rig.rt.state.v.pending().is_some()); // pending untouched

    // composition end => the real diverged-base conflict fires once
    rig.rt.composition_end(node).unwrap();
    rig.pump().unwrap();
    assert_eq!(
        rig.rt.state.outcomes.last().copied(),
        Some(AcceptOutcome::Applied)
    );
}

/// D: a programmatic ack must carry the proposal's original base AND the
/// in-flight result — a wrong-base ack is rejected even with a matching id.
#[test]
fn programmatic_ack_wrong_base_rejected() {
    struct S {
        v: TextValue,
        edits: u32,
    }
    let mut rig = Rig::new(
        S {
            v: TextValue::new("a"),
            edits: 0,
        },
        |s: &mut S, m: Msg, _cx: &mut UpdateCtx<Msg>| {
            if let Msg::Edited(e) = m {
                s.edits += 1;
                let _ = s.v.accept(e);
            }
        },
        |s: &S, ui: &mut Ui<Msg>| {
            ui.text_input(&s.v).on_edit(Msg::Edited);
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
    let (req, base) = rig
        .rt
        .arena
        .get(node)
        .and_then(|n| match &n.data {
            NodeData::Editor { sync, .. } => {
                Some((sync.in_flight.unwrap(), sync.in_flight_base.unwrap()))
            }
            _ => None,
        })
        .unwrap();
    // wrong base — matching requested id — must NOT clear in-flight
    rig.native_edit(
        node,
        "prog",
        base.raw_val() + 99,
        req.raw_val(),
        EditOrigin::Programmatic,
        binding,
    );
    rig.pump().unwrap();
    assert_eq!(rig.rt.state.edits, 0);
    let still_in_flight = rig
        .rt
        .arena
        .get(node)
        .and_then(|n| match &n.data {
            NodeData::Editor { sync, .. } => Some(sync.in_flight.is_some()),
            _ => None,
        })
        .unwrap();
    assert!(still_in_flight);
    // the real ack lands
    rig.native_edit(
        node,
        "prog",
        base.raw_val(),
        req.raw_val(),
        EditOrigin::Programmatic,
        binding,
    );
    rig.pump().unwrap();
    assert_eq!(rig.rt.state.edits, 1);
    assert_eq!(&*rig.rt.state.v.text(), "prog");
}

/// E: same TextValue moves to a NEW parent visited before the old — the old
/// peer unmounts first, edges stay consistent, the peer is released once.
#[test]
fn value_moves_to_earlier_parent_release_once() {
    struct S {
        v: TextValue,
        left: bool,
    }
    let mut rig = Rig::new(
        S {
            v: TextValue::new("mv"),
            left: false,
        },
        |s: &mut S, m: Msg, _cx: &mut UpdateCtx<Msg>| {
            if let Msg::Edited(e) = m {
                let _ = s.v.accept(e);
            }
        },
        |s: &S, ui: &mut Ui<Msg>| {
            // BOTH groups always staged — the value moves between parents;
            // moving right->left mounts under l, committed BEFORE r
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
    let r_editor = {
        let grp = rig.rt.arena.get(rig.root_children()[1]).unwrap();
        crate::NodeId {
            slot: grp.children[0],
            generation: rig.rt.arena.generation_of(grp.children[0]),
        }
    };
    let peer_r = rig.peer_id(r_editor).unwrap();

    // move right -> left: l commits first — the old mount under r must
    // retire before the new peer claims the lease
    rig.rt.state.left = true;
    rig.pump().unwrap();
    rig.view().unwrap();
    assert_eq!(rig.peer_rec(peer_r).released, 1);
    let l_kids = rig
        .rt
        .arena
        .get(rig.root_children()[0])
        .map(|n| n.children.clone())
        .unwrap_or_default();
    assert_eq!(l_kids.len(), 1); // value remounted under l
    let r_kids = rig
        .rt
        .arena
        .get(rig.root_children()[1])
        .map(|n| n.children.clone())
        .unwrap_or_default();
    assert!(r_kids.is_empty()); // old edge detached, not dangling
}
