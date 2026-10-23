// ============================ keys & diagnostics =============================

#[test]
fn duplicate_sibling_keys_are_typed_and_no_commit() {
    struct S;
    let mut rig = Rig::new(
        S,
        |_: &mut S, _: Msg, _: &mut UpdateCtx<Msg>| {},
        |_: &S, ui: &mut Ui<Msg>| {
            ui.group("k", |ui| {
                ui.label("a");
            });
            ui.group("k", |ui| {
                ui.label("b");
            });
        },
    );
    let r = rig.view();
    assert!(matches!(
        r,
        Err(UiError::InvalidUi(UiDiagnostic::DuplicateKey))
    ));
    assert!(rig.root_children().is_empty()); // no committed mutations
}

#[test]
fn hash_collision_unequal_keys_stay_distinct() {
    struct S;
    // force a hash collision via keys whose hashes we control
    #[derive(Clone)]
    struct Collide(u64);
    impl PartialEq for Collide {
        fn eq(&self, o: &Self) -> bool {
            self.0 == o.0
        }
    }
    impl Eq for Collide {}
    impl std::hash::Hash for Collide {
        fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
            h.write_u64(7); // same bucket for every Collide
        }
    }
    let mut rig = Rig::new(
        S,
        |_: &mut S, _: Msg, _: &mut UpdateCtx<Msg>| {},
        |_: &S, ui: &mut Ui<Msg>| {
            ui.keyed(
                &[Collide(1), Collide(2), Collide(3)],
                |c| c.clone(),
                |ui, c| {
                    ui.label(&format!("k{}", c.0));
                },
            );
        },
    );
    rig.view().unwrap();
    // all three mounted — collision stayed a hint, equality disambiguated
    assert_eq!(rig.root_children().len(), 1);
    let list = rig.root_children()[0];
    let kids = rig
        .rt
        .arena
        .get(list)
        .map(|n| n.children.clone())
        .unwrap_or_default();
    assert_eq!(kids.len(), 3);
}

#[test]
fn int_width_keys_are_distinct() {
    struct S;
    // 1u64 and 1u32 must never alias even if their hashes matched
    let mut rig = Rig::new(
        S,
        |_: &mut S, _: Msg, _: &mut UpdateCtx<Msg>| {},
        |_: &S, ui: &mut Ui<Msg>| {
            ui.keyed(
                &[1u64, 2u64],
                |k| *k,
                |ui, k| {
                    ui.label(&format!("{k}"));
                },
            );
            ui.keyed(
                &[1u32, 2u32],
                |k| *k,
                |ui, k| {
                    ui.label(&format!("{k}"));
                },
            );
        },
    );
    rig.view().unwrap();
    assert_eq!(rig.root_children().len(), 2); // two distinct implicit list scopes
}

#[test]
fn duplicate_text_binding_is_typed() {
    struct S {
        v: TextValue,
    }
    let s = S {
        v: TextValue::new("x"),
    };
    // we need the SAME TextValue bound twice — simulate via a shared lease:
    // binding the same value in two views is caught by validate_bindings
    let mut rig = Rig::new(
        s,
        |_: &mut S, _: Msg, _: &mut UpdateCtx<Msg>| {},
        |s: &S, ui: &mut Ui<Msg>| {
            ui.text_input(&s.v).on_edit(Msg::Edited);
        },
    );
    rig.view().unwrap();
    // a second binding of the same value in the same transaction aborts
    struct S2 {
        v: TextValue,
    }
    let mut rig2 = Rig::new(
        S2 {
            v: TextValue::new("y"),
        },
        |_: &mut S2, _: Msg, _: &mut UpdateCtx<Msg>| {},
        |s: &S2, ui: &mut Ui<Msg>| {
            ui.text_input(&s.v).on_edit(Msg::Edited);
            ui.text_input(&s.v).on_edit(Msg::Edited); // same value twice
        },
    );
    let r = rig2.view();
    assert!(matches!(
        r,
        Err(UiError::InvalidUi(UiDiagnostic::DuplicateTextBinding))
    ));
    assert!(rig2.root_children().is_empty());
}

// ============================ reconcile =======================================

#[test]
fn keyed_reorder_keeps_ids_and_peers() {
    struct S {
        order: Vec<char>,
        v: TextValue,
    }
    let mut rig = Rig::new(
        S {
            order: vec!['a', 'b', 'c'],
            v: TextValue::new("v"),
        },
        |_: &mut S, _: Msg, _: &mut UpdateCtx<Msg>| {},
        |s: &S, ui: &mut Ui<Msg>| {
            ui.keyed(
                &s.order,
                |c| *c,
                |ui, _| {
                    ui.label("x");
                },
            );
            ui.text_input(&s.v).on_edit(Msg::Edited);
        },
    );
    rig.view().unwrap();
    let kids = rig.root_children();
    let list = kids[0];
    let editor = kids[1];
    let ed_peer = rig.peer_id(editor).unwrap();

    let row_ids: Vec<crate::NodeId> = rig
        .rt
        .arena
        .get(list)
        .unwrap()
        .children
        .iter()
        .map(|&s| crate::NodeId {
            slot: s,
            generation: rig.rt.arena.generation_of(s),
        })
        .collect();

    rig.rt.state.order = vec!['c', 'a', 'b'];
    rig.pump().unwrap();
    rig.view().unwrap();

    // same NodeIds, order changed only
    let new_rows: Vec<crate::NodeId> = rig
        .rt
        .arena
        .get(list)
        .unwrap()
        .children
        .iter()
        .map(|&s| crate::NodeId {
            slot: s,
            generation: rig.rt.arena.generation_of(s),
        })
        .collect();
    assert_eq!(new_rows, vec![row_ids[2], row_ids[0], row_ids[1]]);
    assert_eq!(rig.peer_id(editor), Some(ed_peer)); // peer identity by key
}

#[test]
fn removal_fences_events_and_releases_peer_once() {
    struct S {
        show: bool,
        show_editor: bool,
        v: TextValue,
        log: Vec<Msg>,
    }
    let mut rig = Rig::new(
        S {
            show: true,
            show_editor: true,
            v: TextValue::new("ed"),
            log: Vec::new(),
        },
        |s: &mut S, m: Msg, _cx: &mut UpdateCtx<Msg>| {
            if let Msg::Edited(e) = m {
                let _ = s.v.accept(e);
            } else {
                s.log.push(m);
            }
        },
        |s: &S, ui: &mut Ui<Msg>| {
            // the conditional lives in a stable group scope — static sibling
            // identity is positional, so the editor keeps its slot
            ui.group("maybe", |ui| {
                if s.show {
                    ui.button("press").on_press(press_msg);
                }
            });
            if s.show_editor {
                ui.text_input(&s.v).on_edit(Msg::Edited);
            }
        },
    );
    rig.view().unwrap();
    let kids = rig.root_children();
    let editor = kids[1];
    let ed_peer = rig.peer_id(editor).unwrap();
    // the button lives inside the group
    let grp = kids[0];
    let btn = {
        let g = rig.rt.arena.get(grp).unwrap();
        crate::NodeId {
            slot: g.children[0],
            generation: rig.rt.arena.generation_of(g.children[0]),
        }
    };

    // remove the button; stale generation must not deliver
    rig.rt.state.show = false;
    rig.pump().unwrap();
    rig.view().unwrap();
    let new_kids = rig.root_children();
    assert_eq!(new_kids, vec![grp, editor]); // editor untouched

    // queue an event with the OLD generation — must be dropped
    rig.push(btn, NodeEvent::Press);
    rig.pump().unwrap();
    assert!(rig.rt.state.log.is_empty());
    assert_eq!(rig.rt.dropped_stale_events, 1);

    // recreate B => same slot, different generation
    rig.rt.state.show = true;
    rig.pump().unwrap();
    rig.view().unwrap();
    let kids2 = rig.root_children();
    let btn2 = {
        let g = rig.rt.arena.get(kids2[0]).unwrap();
        crate::NodeId {
            slot: g.children[0],
            generation: rig.rt.arena.generation_of(g.children[0]),
        }
    };
    assert_eq!(btn2.slot, btn.slot);
    assert_ne!(btn2.generation, btn.generation);
    assert!(rig.rt.arena.is_live(btn2));

    // stale press/edit/selection/frame on the OLD id are all dropped
    rig.push(btn, NodeEvent::Press);
    rig.push(
        btn,
        NodeEvent::Edit(TextEdit {
            text: "ghost".into(),
            base: TextRevision::raw(0),
            result: TextRevision::raw(1),
            origin: EditOrigin::NativePeer,
            binding: BindingToken::raw(1),
        }),
    );
    rig.push(
        btn,
        NodeEvent::Selection(TextSelection {
            revision: TextRevision::raw(0),
            anchor: 0,
            focus: 1,
        }),
    );
    rig.push(
        btn,
        NodeEvent::Frame(FrameTime {
            delta: Duration::from_millis(16),
            absolute: Instant::now(),
        }),
    );
    rig.pump().unwrap();
    assert!(rig.rt.state.log.is_empty());
    assert_eq!(rig.rt.dropped_stale_events, 5); // press + edit + sel + frame + (first drop)
    assert_eq!(&*rig.rt.state.v.text(), "ed"); // stale edit never applied

    // remove the editor — its peer releases exactly once
    rig.rt.state.show_editor = false;
    rig.pump().unwrap();
    rig.view().unwrap();
    assert_eq!(rig.peer_rec(ed_peer).released, 1);
}

#[test]
fn group_toggle_does_not_remount_sibling() {
    struct S {
        on: bool,
        v: TextValue,
    }
    let mut rig = Rig::new(
        S {
            on: true,
            v: TextValue::new("x"),
        },
        |_: &mut S, _: Msg, _: &mut UpdateCtx<Msg>| {},
        |s: &S, ui: &mut Ui<Msg>| {
            ui.group("wrap", |ui| {
                if s.on {
                    ui.label("inner");
                }
            });
            ui.text_input(&s.v).on_edit(Msg::Edited);
        },
    );
    rig.view().unwrap();
    let kids = rig.root_children();
    let editor = kids[1];
    let peer = rig.peer_id(editor).unwrap();
    let old_gen = editor.generation;

    rig.rt.state.on = false;
    rig.pump().unwrap();
    rig.view().unwrap();
    rig.rt.state.on = true;
    rig.pump().unwrap();
    rig.view().unwrap();

    let kids2 = rig.root_children();
    assert_eq!(kids2[1].slot, editor.slot);
    assert_eq!(kids2[1].generation, old_gen); // same generation — never remounted
    assert_eq!(rig.peer_id(kids2[1]), Some(peer));
}

#[test]
fn adjacent_keyed_lists_are_independent() {
    struct S {
        a: Vec<i32>,
        b: Vec<i32>,
    }
    let mut rig = Rig::new(
        S {
            a: vec![1, 2],
            b: vec![10, 20],
        },
        |_: &mut S, _: Msg, _: &mut UpdateCtx<Msg>| {},
        |s: &S, ui: &mut Ui<Msg>| {
            ui.keyed(
                &s.a,
                |i| *i,
                |ui, i| {
                    ui.label(&format!("a{i}"));
                },
            );
            ui.keyed(
                &s.b,
                |i| *i,
                |ui, i| {
                    ui.label(&format!("b{i}"));
                },
            );
        },
    );
    rig.view().unwrap();
    let kids = rig.root_children();
    assert_eq!(kids.len(), 2); // two separate list scopes — no DuplicateKey
    // reorder B only; A unchanged
    rig.rt.state.b = vec![20, 10];
    rig.pump().unwrap();
    rig.view().unwrap();
    let kids2 = rig.root_children();
    assert_eq!(kids2.len(), 2);
}

// ============================ text protocol ===================================

fn editor_of<S: 'static>(rig: &Rig<S>) -> crate::NodeId {
    rig.root_children()[0]
}

#[test]
fn unchanged_text_never_resets_peer() {
    struct S {
        v: TextValue,
    }
    let mut rig = Rig::new(
        S {
            v: TextValue::new("hello"),
        },
        |_: &mut S, _: Msg, _: &mut UpdateCtx<Msg>| {},
        |s: &S, ui: &mut Ui<Msg>| {
            ui.text_input(&s.v).on_edit(Msg::Edited);
        },
    );
    rig.view().unwrap();
    let peer = rig.peer_id(editor_of(&rig)).unwrap();
    let ops = rig.peer_ops(peer);
    // mount sets text once; a pure re-view resets nothing
    let resets = ops
        .iter()
        .filter(|o| matches!(o, PeerOp::SetText(_)))
        .count();
    assert_eq!(resets, 1);
    rig.pump().unwrap();
    rig.view().unwrap();
    let ops2 = rig.peer_ops(peer);
    let resets2 = ops2
        .iter()
        .filter(|o| matches!(o, PeerOp::SetText(_)))
        .count();
    assert_eq!(resets2, 1); // unchanged text => zero resets
}

#[test]
fn programmatic_replace_applies_once_and_acks() {
    struct S {
        v: TextValue,
        acked: bool,
    }
    let mut rig = Rig::new(
        S {
            v: TextValue::new("a"),
            acked: false,
        },
        |s: &mut S, m: Msg, _cx: &mut UpdateCtx<Msg>| {
            if let Msg::Edited(e) = m {
                if e.origin() == EditOrigin::Programmatic {
                    s.acked = true;
                }
                let _ = s.v.accept(e);
            }
        },
        |s: &S, ui: &mut Ui<Msg>| {
            ui.text_input(&s.v).on_edit(Msg::Edited);
        },
    );
    rig.view().unwrap();
    let node = editor_of(&rig);
    let peer = rig.peer_id(node).unwrap();
    let binding = BindingToken::raw(1); // first minted binding for this peer

    // programmatic replace -> exactly one set-text
    let base_rev = rig.rt.state.v.revision().raw_val();
    rig.rt.state.v.replace("replaced");
    rig.view().unwrap();
    let sets = rig
        .peer_ops(peer)
        .iter()
        .filter(|o| matches!(o, PeerOp::SetText(t) if t == "replaced"))
        .count();
    assert_eq!(sets, 1);

    // peer acknowledges as Programmatic — consumer accept lands it
    let applied_rev = rig
        .rt
        .arena
        .get(node)
        .and_then(|n| match &n.data {
            NodeData::Editor { sync, .. } => Some(sync.applied_revision),
            _ => None,
        })
        .unwrap();
    let (b, in_flight) = rig
        .rt
        .arena
        .get(node)
        .and_then(|n| match &n.data {
            NodeData::Editor { sync, .. } => Some((sync.binding.unwrap(), sync.in_flight.unwrap())),
            _ => None,
        })
        .unwrap();
    // the peer acknowledges the apply: base = pre-apply rev, result = requested
    let _ = applied_rev;
    rig.native_edit(
        node,
        "replaced",
        base_rev,
        in_flight.raw_val(),
        EditOrigin::Programmatic,
        b,
    );
    rig.pump().unwrap();
    assert!(rig.rt.state.acked);
    assert_eq!(&*rig.rt.state.v.text(), "replaced");

    // repeated views produce no echo
    rig.view().unwrap();
    let sets2 = rig
        .peer_ops(peer)
        .iter()
        .filter(|o| matches!(o, PeerOp::SetText(t) if t == "replaced"))
        .count();
    assert_eq!(sets2, 1);
    let _ = binding;
}

#[test]
fn foreign_and_stale_edits_never_reach_app() {
    struct S {
        v: TextValue,
        edits: u32,
    }
    let mut rig = Rig::new(
        S {
            v: TextValue::new("hi"),
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

    // foreign binding — dropped before dispatch, mirror untouched
    rig.native_edit(
        node,
        "x",
        0,
        1,
        EditOrigin::NativePeer,
        BindingToken::raw(9999),
    );
    rig.pump().unwrap();
    assert_eq!(rig.rt.state.edits, 0);
    assert_eq!(&*rig.rt.state.v.text(), "hi");
    assert_eq!(rig.rt.rejected_edits, 1);

    // stale/out-of-order result — dropped too
    let rev = rig
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
        "x",
        rev.raw_val(),
        rev.raw_val(),
        EditOrigin::NativePeer,
        binding,
    ); // result not >
    rig.pump().unwrap();
    assert_eq!(rig.rt.state.edits, 0);
    assert_eq!(rig.rt.rejected_edits, 2);

    // a valid in-order edit DOES reach the app
    rig.native_edit(
        node,
        "hi!",
        rev.raw_val(),
        rev.raw_val() + 1,
        EditOrigin::NativePeer,
        binding,
    );
    rig.pump().unwrap();
    assert_eq!(rig.rt.state.edits, 1);
    assert_eq!(&*rig.rt.state.v.text(), "hi!");
}
