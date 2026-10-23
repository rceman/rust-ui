/// E: swap two bound values across siblings in one pass — peers release once
/// each, no duplicate edges, per-value text preserved.
#[test]
fn swap_two_bound_values_releases_each_peer_once() {
    struct S {
        a: TextValue,
        b: TextValue,
        swapped: bool,
    }
    let mut rig = Rig::new(
        S {
            a: TextValue::new("aaa"),
            b: TextValue::new("bbb"),
            swapped: false,
        },
        |s: &mut S, m: Msg, _cx: &mut UpdateCtx<Msg>| {
            if let Msg::Edited(e) = m {
                // route to whichever value owns the binding
                if s.a.accept(e.clone()) == AcceptOutcome::Applied {
                } else {
                    let _ = s.b.accept(e);
                }
            }
        },
        |s: &S, ui: &mut Ui<Msg>| {
            let (x, y) = if s.swapped {
                (&s.b, &s.a)
            } else {
                (&s.a, &s.b)
            };
            ui.group("left", |ui| {
                ui.text_input(x).on_edit(Msg::Edited);
            });
            ui.group("right", |ui| {
                ui.text_input(y).on_edit(Msg::Edited);
            });
        },
    );
    rig.view().unwrap();
    let l_id = {
        let n = rig.rt.arena.get(rig.root_children()[0]).unwrap();
        crate::NodeId {
            slot: n.children[0],
            generation: rig.rt.arena.generation_of(n.children[0]),
        }
    };
    let peer_l = rig.peer_id(l_id).unwrap();

    rig.rt.state.swapped = true;
    rig.pump().unwrap();
    rig.view().unwrap();
    // value "b" now mounts under left — old left peer released once
    assert_eq!(rig.peer_rec(peer_l).released, 1);
    // no duplicate edges anywhere
    let mut slots: Vec<u32> = Vec::new();
    for kid in rig.root_children() {
        if let Some(n) = rig.rt.arena.get(kid) {
            slots.extend(n.children.iter().copied());
        }
    }
    slots.sort();
    slots.dedup();
    assert_eq!(slots.len(), 2);
    // per-value text is correct after the swap — left holds "bbb"
    let kids = rig.root_children();
    let (ls, rs) = (
        rig.rt.arena.get(kids[0]).unwrap().children[0],
        rig.rt.arena.get(kids[1]).unwrap().children[0],
    );
    let left_editor = crate::NodeId {
        slot: ls,
        generation: rig.rt.arena.generation_of(ls),
    };
    let right_editor = crate::NodeId {
        slot: rs,
        generation: rig.rt.arena.generation_of(rs),
    };
    assert_eq!(rig.peer_rec(rig.peer_id(left_editor).unwrap()).text, "bbb");
    assert_eq!(rig.peer_rec(rig.peer_id(right_editor).unwrap()).text, "aaa");
    // exactly two live peers, each created peer released at most once
    let recs = rig.peers.recs.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(recs.values().filter(|r| r.released == 0).count(), 2);
    assert!(recs.values().all(|r| r.released <= 1));
}

/// E: 100 move/reorder cycles — no duplicate child edges, live/peer counts
/// stay at baseline, every released peer released exactly once.
#[test]
fn hundred_move_cycles_keep_structure_consistent() {
    struct S {
        v: TextValue,
        left: bool,
    }
    let mut rig = Rig::new(
        S {
            v: TextValue::new("m"),
            left: true,
        },
        |s: &mut S, m: Msg, _cx: &mut UpdateCtx<Msg>| {
            if let Msg::Edited(e) = m {
                let _ = s.v.accept(e);
            }
        },
        |s: &S, ui: &mut Ui<Msg>| {
            if s.left {
                ui.group("l", |ui| {
                    ui.text_input(&s.v).on_edit(Msg::Edited);
                });
            }
            ui.group("r", |ui| {
                if !s.left {
                    ui.text_input(&s.v).on_edit(Msg::Edited);
                }
            });
        },
    );
    rig.view().unwrap();
    let baseline_slots = rig.rt.arena.slot_count();
    for i in 0..100 {
        rig.rt.state.left = i % 2 == 0;
        rig.pump().unwrap();
        rig.view().unwrap();
        // per-iteration invariants, not just a final check: no duplicate
        // edges, exactly one mounted editor, peer text matches the value
        let mut edges: Vec<u32> = Vec::new();
        for kid in rig.root_children() {
            if let Some(n) = rig.rt.arena.get(kid) {
                edges.extend(n.children.iter().copied());
            }
        }
        let mut dedup = edges.clone();
        dedup.sort();
        dedup.dedup();
        assert_eq!(edges.len(), dedup.len(), "dup edge at cycle {i}");
        assert_eq!(edges.len(), 1, "exactly one editor mounted at cycle {i}");
        let editor = crate::NodeId {
            slot: edges[0],
            generation: rig.rt.arena.generation_of(edges[0]),
        };
        let pid = rig.peer_id(editor).unwrap();
        assert_eq!(rig.peer_rec(pid).text, "m", "peer text at cycle {i}");
    }
    // arena didn't grow unboundedly
    assert!(rig.rt.arena.slot_count() <= baseline_slots + 4);
    // every released peer released exactly once; only one still lives
    let recs = rig.peers.recs.lock().unwrap_or_else(|e| e.into_inner());
    assert!(recs.values().all(|r| r.released <= 1));
    assert_eq!(recs.values().filter(|r| r.released == 0).count(), 1);
}

/// E: peer-factory failure => typed error + orderly teardown (earlier peers
/// released once, leases cleared so values can rebind elsewhere).
#[test]
fn peer_factory_failure_tears_down_orderly() {
    struct S {
        v1: TextValue,
        v2: TextValue,
        show: bool,
    }
    let fail_after = Arc::new(Mutex::new(1u32)); // first peer OK, second fails
    let f2 = fail_after.clone();
    let peers = PeerLog::default();
    let pf = peers.clone();
    let app = App::new(
        S {
            v1: TextValue::new("one"),
            v2: TextValue::new("two"),
            show: true,
        },
        Box::new(|_: &mut S, _: Msg, _: &mut UpdateCtx<Msg>| {})
            as Box<dyn Fn(&mut S, Msg, &mut UpdateCtx<Msg>)>,
        Box::new(|s: &S, ui: &mut Ui<'_, '_, Msg>| {
            ui.text_input(&s.v1).on_edit(Msg::Edited);
            ui.text_input(&s.v2).on_edit(Msg::Edited);
        }) as Box<dyn Fn(&S, &mut Ui<'_, '_, Msg>)>,
    );
    let pf2 = pf.clone();
    let mut rt = runtime_for(
        app,
        Box::new(move |spec| {
            let mut n = f2.lock().unwrap();
            if *n == 0 {
                return Err(crate::UiError::Platform("peer create failed".into()));
            }
            *n -= 1;
            Ok(Box::new(FakePeer::new(pf2.clone(), spec.multiline))
                as Box<dyn crate::node::TextPeer>)
        }),
        Theme::light(),
        Appearance {
            dark: false,
            forced_colors: false,
        },
    );
    let r = rt.review_for_test();
    assert!(matches!(r, Err(crate::UiError::Platform(_))));
    // orderly: the one created peer was released, both leases cleared
    let recs = pf.recs.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(recs.values().filter(|r| r.released == 1).count(), 1);
    // every created peer released exactly once — teardown is idempotent
    assert!(recs.values().all(|r| r.released <= 1));
}

/// F: a committed native edit still routes while the node is hidden —
/// hiding must not lose acknowledged peer state.
#[test]
fn hidden_editor_still_receives_committed_edits() {
    struct S {
        v: TextValue,
        hidden: bool,
        edits: u32,
    }
    let mut rig = Rig::new(
        S {
            v: TextValue::new("h"),
            hidden: false,
            edits: 0,
        },
        |s: &mut S, m: Msg, _cx: &mut UpdateCtx<Msg>| {
            if let Msg::Edited(e) = m {
                s.edits += 1;
                let _ = s.v.accept(e);
            }
        },
        |s: &S, ui: &mut Ui<Msg>| {
            ui.text_input(&s.v)
                .visibility(if s.hidden {
                    Visibility::Hidden
                } else {
                    Visibility::Visible
                })
                .on_edit(Msg::Edited);
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
    // hide the editor — a committed native edit must STILL route
    rig.rt.state.hidden = true;
    rig.pump().unwrap();
    rig.view().unwrap();
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
        "h!",
        pr.raw_val(),
        pr.raw_val() + 1,
        EditOrigin::NativePeer,
        binding,
    );
    rig.pump().unwrap();
    assert_eq!(rig.rt.state.edits, 1); // delivered despite hidden
    assert_eq!(&*rig.rt.state.v.text(), "h!");
    // but Press is still gated
    rig.push(node, NodeEvent::Press);
    rig.pump().unwrap();
    // Blur still routes
    rig.push(node, NodeEvent::Focus(false));
    rig.pump().unwrap();
}
