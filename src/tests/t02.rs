#[test]
fn consumer_must_accept_for_committed_to_change() {
    struct S {
        v: TextValue,
        accept: bool,
    }
    let mut rig = Rig::new(
        S {
            v: TextValue::new("base"),
            accept: false,
        },
        |s: &mut S, m: Msg, _cx: &mut UpdateCtx<Msg>| {
            if let Msg::Edited(e) = m {
                if s.accept {
                    assert_eq!(s.v.accept(e.clone()), AcceptOutcome::Applied);
                }
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
    let rev = rig
        .rt
        .arena
        .get(node)
        .and_then(|n| match &n.data {
            NodeData::Editor { sync, .. } => Some(sync.peer_revision),
            _ => None,
        })
        .unwrap();

    // native commit arrives; consumer ignores it — committed unchanged,
    // but the peer is NOT reset (peer mirror advanced)
    rig.native_edit(
        node,
        "typed",
        rev.raw_val(),
        rev.raw_val() + 1,
        EditOrigin::NativePeer,
        binding,
    );
    rig.pump().unwrap();
    assert_eq!(&*rig.rt.state.v.text(), "base"); // app state unchanged
    // peer mirror advanced
    let peer_rev = rig
        .rt
        .arena
        .get(node)
        .and_then(|n| match &n.data {
            NodeData::Editor { sync, .. } => Some(sync.peer_revision),
            _ => None,
        })
        .unwrap();
    assert_eq!(peer_rev.raw_val(), rev.raw_val() + 1);

    // a later programmatic base compares to the PEER revision, not the app's
    rig.rt.state.v.replace("prog");
    rig.view().unwrap(); // base (old rev) < peer rev => conflict, not apply
    // conflict emitted once (no on_conflict handler -> diagnostic)
    assert!(
        rig.rt
            .diagnostics
            .contains(&UiDiagnostic::UnhandledTextConflict)
    );
}

#[test]
fn multiple_in_order_native_edits_batch_accept() {
    struct S {
        v: TextValue,
    }
    let mut rig = Rig::new(
        S {
            v: TextValue::new(""),
        },
        |s: &mut S, m: Msg, _cx: &mut UpdateCtx<Msg>| {
            if let Msg::Edited(e) = m {
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
    let mut rev = rig
        .rt
        .arena
        .get(node)
        .and_then(|n| match &n.data {
            NodeData::Editor { sync, .. } => Some(sync.peer_revision),
            _ => None,
        })
        .unwrap();

    for i in 0..3 {
        let text = format!("c{i}");
        rig.native_edit(
            node,
            &text,
            rev.raw_val(),
            rev.raw_val() + 1,
            EditOrigin::NativePeer,
            binding,
        );
        rev = TextRevision::raw(rev.raw_val() + 1);
    }
    rig.pump().unwrap();
    assert_eq!(&*rig.rt.state.v.text(), "c2");
}

#[test]
fn proposal_queued_during_composition() {
    struct S {
        v: TextValue,
    }
    let mut rig = Rig::new(
        S {
            v: TextValue::new("x"),
        },
        |s: &mut S, m: Msg, _cx: &mut UpdateCtx<Msg>| {
            if let Msg::Edited(e) = m {
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

    rig.rt.composition_start(node);
    rig.rt.state.v.replace("comp");
    rig.view().unwrap();
    // queued — no set-text while composing
    let sets = rig
        .peer_ops(peer)
        .iter()
        .filter(|o| matches!(o, PeerOp::SetText(t) if t == "comp"))
        .count();
    assert_eq!(sets, 0);

    // composition ends with the same base => apply
    rig.rt.composition_end(node).unwrap();
    let sets = rig
        .peer_ops(peer)
        .iter()
        .filter(|o| matches!(o, PeerOp::SetText(t) if t == "comp"))
        .count();
    assert_eq!(sets, 1);
}

#[test]
fn native_commit_during_pending_conflicts() {
    struct S {
        v: TextValue,
        conflicts: Vec<TextConflict>,
    }
    let mut rig = Rig::new(
        S {
            v: TextValue::new("a"),
            conflicts: Vec::new(),
        },
        |s: &mut S, m: Msg, _cx: &mut UpdateCtx<Msg>| match m {
            Msg::Edited(e) => {
                let _ = s.v.accept(e);
            }
            Msg::Conflicted(c) => s.conflicts.push(c),
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

    // request pending; native commits on the same base => conflict
    rig.rt.state.v.replace("prog");
    rig.view().unwrap(); // base == peer_rev => applied
    let peer_rev = rig
        .rt
        .arena
        .get(node)
        .and_then(|n| match &n.data {
            NodeData::Editor { sync, .. } => Some(sync.peer_revision),
            _ => None,
        })
        .unwrap();
    // a second replace while first is in flight
    rig.rt.state.v.replace("newer");
    // native commit lands on diverged base -> conflict for the NEW request
    rig.native_edit(
        node,
        "native",
        peer_rev.raw_val(),
        peer_rev.raw_val() + 1,
        EditOrigin::NativePeer,
        binding,
    );
    rig.pump().unwrap(); // native accepted => supersedes in-flight "prog"
    rig.view().unwrap();
    rig.pump().unwrap(); // dispatch the queued conflict event
    assert!(!rig.rt.state.conflicts.is_empty());
}

/// F05: the FINAL queued native commit must be acknowledged BEFORE a
/// composing-queued proposal resolves — otherwise a proposal whose base
/// predates the commit could still be applied over it (the IME end-order
/// bug). This test drives the exact ime_end ordering: drain the edit
/// (queued) -> pump (acknowledged) -> composition_end (resolves).
#[test]
fn final_composition_commit_acknowledged_before_proposal() {
    struct S {
        v: TextValue,
        conflicts: Vec<TextConflict>,
    }
    let mut rig = Rig::new(
        S {
            v: TextValue::new("a"),
            conflicts: Vec::new(),
        },
        |s: &mut S, m: Msg, _cx: &mut UpdateCtx<Msg>| match m {
            Msg::Edited(e) => {
                let _ = s.v.accept(e);
            }
            Msg::Conflicted(c) => s.conflicts.push(c),
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
    // PRECONDITION: composition active; proposal queued on pre-commit base
    rig.rt.composition_start(node);
    let base = rig
        .rt
        .arena
        .get(node)
        .and_then(|n| match &n.data {
            NodeData::Editor { sync, .. } => Some(sync.peer_revision),
            _ => None,
        })
        .unwrap();
    rig.rt.state.v.replace("proposal");
    rig.view().unwrap();
    assert!(
        rig.rt.state.v.pending().is_some(),
        "proposal must be queued while composing"
    );
    // ACTION: the final native commit lands — ENQUEUED first (the bug's
    // ordering), then pumped before composition_end resolves
    rig.native_edit(
        node,
        "final-commit",
        base.raw_val(),
        base.raw_val() + 1,
        EditOrigin::NativePeer,
        binding,
    );
    rig.rt.composition_clear(node);
    rig.pump().unwrap(); // acknowledge the commit into committed state
    rig.rt.composition_end(node).unwrap();
    rig.pump().unwrap();
    // POSTCONDITION: the stale-base proposal CONFLICTS — it must not
    // apply over the acknowledged commit; committed text is the native one
    assert!(
        !rig.rt.state.conflicts.is_empty(),
        "proposal on a pre-commit base must conflict after the final commit"
    );
    assert_eq!(&*rig.rt.state.v.text(), "final-commit");
}

#[test]
fn keep_native_clears_matching_request_only() {
    struct S {
        v: TextValue,
        outcome: Option<AcceptOutcome>,
    }
    let mut rig = Rig::new(
        S {
            v: TextValue::new("a"),
            outcome: None,
        },
        |s: &mut S, m: Msg, _cx: &mut UpdateCtx<Msg>| match m {
            Msg::Edited(e) => {
                let _ = s.v.accept(e);
            }
            Msg::Conflicted(c) => {
                s.outcome = Some(s.v.keep_native(c));
            }
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

    // request R1
    rig.rt.state.v.replace("r1");
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
    // native commit while R1 is in flight => conflict for R1
    rig.native_edit(
        node,
        "native-wins",
        peer_rev.raw_val(),
        peer_rev.raw_val() + 1,
        EditOrigin::NativePeer,
        binding,
    );
    rig.pump().unwrap(); // edit delivered; conflict queued
    rig.pump().unwrap(); // conflict delivered => keep_native applies "native-wins", tombstones R1
    assert_eq!(rig.rt.state.outcome, Some(AcceptOutcome::Applied));
    assert_eq!(&*rig.rt.state.v.text(), "native-wins");

    // a NEWER request R2 — a stale conflict naming R1's old revision must
    // not clear it or touch committed text
    rig.rt.state.v.replace("r2-newer");
    let stale = TextConflict {
        rejected_revision: TextRevision::raw(0), // never matches R2
        rejected_text: "ghost".into(),
        committed_revision: TextRevision::raw(0),
        committed_text: "zzz".into(),
        binding,
    };
    let out = rig.rt.state.v.keep_native(stale);
    assert_eq!(out, AcceptOutcome::IgnoredStale);
    assert_eq!(&*rig.rt.state.v.text(), "native-wins"); // unchanged
    // R2 is still live — later conflict delivery for R1 is equally inert
    assert!(rig.rt.state.v.revision() > TextRevision::raw(0));
}

#[test]
fn old_conflict_cannot_erase_newer_replace() {
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

    rig.rt.state.v.replace("old");
    rig.view().unwrap();
    // stale conflict for an older request id must not clear "new"
    let stale = TextConflict {
        rejected_revision: TextRevision::raw(999999), // never the pending one
        rejected_text: "ghost".into(),
        committed_revision: TextRevision::raw(0),
        committed_text: "zzz".into(),
        binding,
    };
    let out = rig.rt.state.v.keep_native(stale);
    assert_eq!(out, AcceptOutcome::IgnoredStale);
    // pending "old" is still live (not erased by a stale conflict)
    assert!(rig.rt.state.v.text() == "a");
}

#[test]
fn remount_rejects_old_binding_edits() {
    struct S {
        v: TextValue,
        show: bool,
        edits: u32,
    }
    let mut rig = Rig::new(
        S {
            v: TextValue::new("a"),
            show: true,
            edits: 0,
        },
        |s: &mut S, m: Msg, _cx: &mut UpdateCtx<Msg>| {
            if let Msg::Edited(e) = m {
                s.edits += 1;
                let _ = s.v.accept(e);
            }
        },
        |s: &S, ui: &mut Ui<Msg>| {
            if s.show {
                ui.text_input(&s.v).on_edit(Msg::Edited);
            }
        },
    );
    rig.view().unwrap();
    let node = editor_of(&rig);
    let binding1 = rig
        .rt
        .arena
        .get(node)
        .and_then(|n| match &n.data {
            NodeData::Editor { sync, .. } => sync.binding,
            _ => None,
        })
        .unwrap();

    // unmount + remount => fresh binding token
    rig.rt.state.show = false;
    rig.pump().unwrap();
    rig.view().unwrap();
    rig.rt.state.show = true;
    rig.pump().unwrap();
    rig.view().unwrap();
    let node2 = editor_of(&rig);
    let binding2 = rig
        .rt
        .arena
        .get(node2)
        .and_then(|n| match &n.data {
            NodeData::Editor { sync, .. } => sync.binding,
            _ => None,
        })
        .unwrap();
    assert_ne!(binding1, binding2);

    // an old-generation edit must be dropped (old binding is dead)
    rig.native_edit(node2, "stale", 0, 1, EditOrigin::NativePeer, binding1);
    rig.pump().unwrap();
    assert_eq!(rig.rt.state.edits, 0);
}

#[test]
fn pending_applies_on_first_mount() {
    struct S {
        v: TextValue,
        show: bool,
    }
    let mut rig = Rig::new(
        S {
            v: TextValue::new("a"),
            show: false,
        },
        |s: &mut S, m: Msg, _cx: &mut UpdateCtx<Msg>| {
            if let Msg::Edited(e) = m {
                let _ = s.v.accept(e);
            }
        },
        |s: &S, ui: &mut Ui<Msg>| {
            if s.show {
                ui.text_input(&s.v).on_edit(Msg::Edited);
            }
        },
    );
    // queue a replace while unmounted, then mount — the intent must apply
    // on the FIRST mount (not require a second view)
    rig.rt.state.v.replace("pre-mount");
    rig.view().unwrap();
    rig.rt.state.show = true;
    rig.pump().unwrap();
    rig.view().unwrap();
    let node = editor_of(&rig);
    let peer = rig.peer_id(node).unwrap();
    let ops = rig.peer_ops(peer);
    // two set-text: committed "a" then pending "pre-mount" — or a single
    // apply; either way the pending must have landed on first mount
    let has_pending_apply = ops
        .iter()
        .any(|o| matches!(o, PeerOp::SetText(t) if t == "pre-mount"));
    assert!(has_pending_apply);
}
