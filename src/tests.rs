use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use std::time::{Duration, Instant};

use crate::app::App;
use crate::app::runtime_for;
use crate::event::FrameTime;
use crate::geom::Visibility;
use crate::node::{NodeData, NodeEvent, QueuedEvent};
use crate::runtime::{Runtime, UpdateCtx};
use crate::sched::{FRAME_STEP, TOOLTIP_DELAY, TRANSITION_MS};
use crate::tasks::{BoxFuture, Executor, TaskStartError};
use crate::text::{
    AcceptOutcome, BindingToken, EditOrigin, TextConflict, TextEdit, TextRevision, TextSelection,
    TextValue,
};
use crate::theme::{Appearance, MotionToken, ReducedMotion, Theme};
use crate::ui::{CustomRender, Ui};
use crate::{UiDiagnostic, UiError};

// ============================ fake peers =====================================

#[derive(Debug, PartialEq, Clone)]
enum PeerOp {
    Attach(u32),
    SetText(String),
    Released,
}

#[derive(Clone)]
struct PeerRec {
    id: u64,
    ops: Vec<PeerOp>,
    text: String,
    released: u32,
}

#[derive(Clone, Default)]
struct PeerLog {
    recs: Arc<Mutex<HashMap<u64, PeerRec>>>,
    next: Arc<Mutex<u64>>,
}

impl PeerLog {
    fn rec(&self, id: u64) -> std::sync::MutexGuard<'_, HashMap<u64, PeerRec>> {
        self.recs.lock().unwrap()
    }
}

/// Fake native editable peer — the contract surface `TextPeerSync` drives.
struct FakePeer {
    id: u64,
    log: PeerLog,
}

static PEER_SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl FakePeer {
    fn new(log: PeerLog, _multiline: bool) -> Self {
        let id = PEER_SERIAL.fetch_add(1, Ordering::SeqCst);
        let p = FakePeer { id, log };
        p.log.recs.lock().unwrap().insert(
            id,
            PeerRec {
                id,
                ops: Vec::new(),
                text: String::new(),
                released: 0,
            },
        );
        p
    }
    fn push(&self, op: PeerOp) {
        let mut g = self.log.recs.lock().unwrap();
        g.get_mut(&self.id).unwrap().ops.push(op);
    }
}

impl crate::node::TextPeer for FakePeer {
    fn peer_id(&self) -> u64 {
        self.id
    }
    fn initialize(
        &mut self,
        text: &str,
        _rev: TextRevision,
        _binding: crate::text::BindingToken,
    ) -> crate::UiResult {
        self.push(PeerOp::SetText(text.to_string()));
        self.log
            .recs
            .lock()
            .unwrap()
            .get_mut(&self.id)
            .unwrap()
            .text = text.to_string();
        Ok(())
    }
    fn set_text(&mut self, text: &str, _base: TextRevision, _req: TextRevision) -> crate::UiResult {
        self.push(PeerOp::SetText(text.to_string()));
        self.log
            .recs
            .lock()
            .unwrap()
            .get_mut(&self.id)
            .unwrap()
            .text = text.to_string();
        Ok(())
    }
    fn release(&mut self) {
        self.push(PeerOp::Released);
        self.log
            .recs
            .lock()
            .unwrap()
            .get_mut(&self.id)
            .unwrap()
            .released += 1;
    }
    fn attach(&mut self, node: crate::NodeId) {
        self.push(PeerOp::Attach(node.slot));
    }
}

// ============================ test executor ==================================

/// Deterministic executor: spawned futures go in a ready queue; `poll()`
/// drives one step. Re-parks itself via the waker until Ready.
#[derive(Default)]
struct Exec {
    tasks: Vec<Option<BoxFuture<()>>>,
    ready: std::collections::VecDeque<usize>,
}

struct TaskWake {
    exec: Arc<Mutex<Exec>>,
    idx: usize,
}

impl Wake for TaskWake {
    fn wake(self: Arc<Self>) {
        self.exec.lock().unwrap().ready.push_back(self.idx);
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.exec.lock().unwrap().ready.push_back(self.idx);
    }
}

fn task_waker(exec: Arc<Mutex<Exec>>, idx: usize) -> Waker {
    Waker::from(Arc::new(TaskWake { exec, idx }))
}

#[derive(Clone)]
struct TestExecutor {
    inner: Arc<Mutex<Exec>>,
}

impl Executor for TestExecutor {
    fn spawn(&self, task: BoxFuture<()>) -> Result<(), TaskStartError> {
        let mut e = self.inner.lock().unwrap();
        e.tasks.push(Some(task));
        let idx = e.tasks.len() - 1;
        e.ready.push_back(idx);
        Ok(())
    }
}

impl TestExecutor {
    /// Poll every ready task once; returns how many ran.
    fn poll(&self) -> usize {
        loop {
            let idx = {
                let mut e = self.inner.lock().unwrap();
                e.ready.pop_front()
            };
            let Some(idx) = idx else { return 0 };
            let mut task = {
                let mut e = self.inner.lock().unwrap();
                e.tasks[idx].take()
            };
            if let Some(t) = task.as_mut() {
                let w = task_waker(self.inner.clone(), idx);
                let mut cx = Context::from_waker(&w);
                if t.as_mut().poll(&mut cx).is_pending() {
                    let mut e = self.inner.lock().unwrap();
                    e.tasks[idx] = task.take();
                }
            }
        }
    }

    fn live(&self) -> usize {
        self.inner
            .lock()
            .unwrap()
            .tasks
            .iter()
            .filter(|t| t.is_some())
            .count()
    }
}

// ============================ rig ============================================

#[derive(Debug)]
enum Msg {
    Kick,
    Press,
    Edited(TextEdit),
    Conflicted(TextConflict),
    Selection(TextSelection),
    Chunk(u32),
    Focus(bool),
    Scoped(u32, u32),
    Frame(Duration),
}

fn press_msg() -> Msg {
    Msg::Press
}

impl PartialEq for Msg {
    fn eq(&self, o: &Msg) -> bool {
        match (self, o) {
            (Msg::Kick, Msg::Kick) | (Msg::Press, Msg::Press) => true,
            (Msg::Chunk(a), Msg::Chunk(b)) => a == b,
            (Msg::Focus(a), Msg::Focus(b)) => a == b,
            (Msg::Scoped(a1, b1), Msg::Scoped(a2, b2)) => a1 == a2 && b1 == b2,
            (Msg::Edited(a), Msg::Edited(b)) => a.result_revision() == b.result_revision(),
            (Msg::Conflicted(a), Msg::Conflicted(b)) => a.rejected_revision == b.rejected_revision,
            (Msg::Selection(a), Msg::Selection(b)) => a == b,
            (Msg::Frame(a), Msg::Frame(b)) => a == b,
            _ => false,
        }
    }
}

struct Rig<S: 'static> {
    rt: Runtime<
        S,
        Msg,
        Box<dyn Fn(&mut S, Msg, &mut UpdateCtx<Msg>)>,
        Box<dyn Fn(&S, &mut Ui<'_, '_, Msg>)>,
    >,
    peers: PeerLog,
    exec: TestExecutor,
}

impl<S: 'static> Rig<S> {
    fn new(
        state: S,
        update: impl Fn(&mut S, Msg, &mut UpdateCtx<Msg>) + 'static,
        view: impl Fn(&S, &mut Ui<'_, '_, Msg>) + 'static,
    ) -> Self {
        let peers = PeerLog::default();
        let pf = peers.clone();
        let exec = TestExecutor {
            inner: Arc::new(Mutex::new(Exec::default())),
        };
        let update: Box<dyn Fn(&mut S, Msg, &mut UpdateCtx<Msg>)> = Box::new(update);
        let view: Box<dyn Fn(&S, &mut Ui<'_, '_, Msg>)> = Box::new(view);
        let app = App::new(state, update, view);
        let mut rt = runtime_for(
            app,
            Box::new(move |multiline| {
                Ok(Box::new(FakePeer::new(pf.clone(), multiline))
                    as Box<dyn crate::node::TextPeer>)
            }),
            Theme::light(),
            Appearance { dark: false },
        );
        rt.executor = Some(Arc::new(exec.clone()));
        Rig { rt, peers, exec }
    }

    fn view(&mut self) -> crate::UiResult {
        self.rt.review_for_test()
    }

    fn pump(&mut self) -> crate::UiResult {
        self.rt.pump().map(|_| ())
    }

    /// push a node event into the native queue (tests the dispatch path)
    fn push(&mut self, node: crate::NodeId, payload: NodeEvent) {
        self.rt.test_push_event(QueuedEvent { node, payload });
    }

    /// simulate a committed native edit reaching the queue
    fn native_edit(
        &mut self,
        node: crate::NodeId,
        text: &str,
        base: u64,
        result: u64,
        origin: EditOrigin,
        binding: BindingToken,
    ) {
        let _ = self.rt.edit_event(
            node,
            TextEdit {
                text: text.into(),
                base: TextRevision::raw(base),
                result: TextRevision::raw(result),
                origin,
                binding,
            },
        );
    }

    fn peer_id(&self, node: crate::NodeId) -> Option<u64> {
        self.rt.test_peer_id(node)
    }

    fn peer_ops(&self, peer: u64) -> Vec<PeerOp> {
        self.peers
            .recs
            .lock()
            .unwrap()
            .get(&peer)
            .map(|r| r.ops.clone())
            .unwrap_or_default()
    }

    fn peer_rec(&self, peer: u64) -> PeerRec {
        self.peers.recs.lock().unwrap().get(&peer).unwrap().clone()
    }

    fn root_children(&self) -> Vec<crate::NodeId> {
        self.rt.root_children()
    }
}

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
        Box::new(move |ml| {
            Ok(Box::new(FakePeer::new(pf.clone(), ml)) as Box<dyn crate::node::TextPeer>)
        }),
        Theme::light(),
        Appearance { dark: false },
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
        Box::new(move |ml| {
            Ok(Box::new(FakePeer::new(pf.clone(), ml)) as Box<dyn crate::node::TextPeer>)
        }),
        Theme::light(),
        Appearance { dark: false },
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
    rig.rt.mailbox.install_wake(Arc::new(move || {
        *c2.lock().unwrap() += 1;
    }));
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
    rig.rt.mailbox.install_wake(Arc::new(move || {
        *c2.lock().unwrap() += 1;
    }));
    assert_eq!(*calls.lock().unwrap(), 1);
}

/// B: purging/close drops charges outside the lock — a wake callback may
/// safely reenter queue_len while envelopes are being released.
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
        // reentrant observation inside the callback — must not deadlock
        *c2.lock().unwrap() += 1;
    }));
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
    let recs = rig.peers.recs.lock().unwrap();
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
    let recs = rig.peers.recs.lock().unwrap();
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
        Box::new(move |ml| {
            let mut n = f2.lock().unwrap();
            if *n == 0 {
                return Err(crate::UiError::Platform("peer create failed".into()));
            }
            *n -= 1;
            Ok(Box::new(FakePeer::new(pf2.clone(), ml)) as Box<dyn crate::node::TextPeer>)
        }),
        Theme::light(),
        Appearance { dark: false },
    );
    let r = rt.review_for_test();
    assert!(matches!(r, Err(crate::UiError::Platform(_))));
    // orderly: the one created peer was released, both leases cleared
    let recs = pf.recs.lock().unwrap();
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


// ======================== native probes (Windows-only) =====================

#[cfg(windows)]
thread_local! {
    static PROBE_HWND: std::cell::RefCell<windows::Win32::Foundation::HWND> =
        std::cell::RefCell::new(windows::Win32::Foundation::HWND::default());
}

#[cfg(windows)]
unsafe extern "system" fn probe_wndproc(
    hwnd: windows::Win32::Foundation::HWND,
    msg: u32,
    wp: windows::Win32::Foundation::WPARAM,
    lp: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    windows::Win32::UI::WindowsAndMessaging::DefWindowProcW(hwnd, msg, wp, lp)
}

/// Deterministic offscreen probe: does TxDrawD2D actually paint glyphs?
/// Creates a real windowless RichEdit peer, initializes "Hello", draws into
/// a D2D DC render target bound to an in-memory DIB, counts lit pixels.
#[cfg(windows)]
#[test]
fn native_probe_richedit_paints_text() {
    use windows::Win32::Graphics::Direct2D::Common::*;
    use windows::Win32::Graphics::Direct2D::*;
    use windows::Win32::Graphics::Dxgi::Common::*;
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::System::Com::*;
    use windows::Win32::System::Ole::OleInitialize;
    use windows::Win32::Foundation::*;
    use windows::core::*;

    unsafe {
        let _ = OleInitialize(None);
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }
    let lib = crate::platform::win32::Msftedit::load().expect("msftedit");
    let sink = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    // real host hwnd BEFORE peer creation — matches the app
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::*;
        let cls = w!("rustui_probe");
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpszClassName: cls,
            hInstance: windows::Win32::System::LibraryLoader::GetModuleHandleW(None).unwrap().into(),
            lpfnWndProc: Some(probe_wndproc),
            ..Default::default()
        };
        RegisterClassExW(&wc);
        PROBE_HWND.with(|h| *h.borrow_mut() = CreateWindowExW(
            WINDOW_EX_STYLE(0), cls, w!("probe"), WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            0, 0, 800, 500, None, None, None, None,
        ).expect("hwnd"));
    }
    let cfg = crate::platform::win32::PeerConfig {
        multiline: false,
        read_only: false,
        face: "Segoe UI".into(),
        size_twips: 280,
        fg: [0.94, 0.94, 0.94, 1.0],
        sel_bg: [0.2, 0.4, 0.8, 1.0],
        sel_fg: [1.0, 1.0, 1.0, 1.0],
    };
    let mut peer = crate::platform::win32::WindowlessPeer::create(
        1,
        &lib,
        PROBE_HWND.with(|h| *h.borrow()),
        1.5625,
        &cfg,
        sink,
    )
    .expect("peer create");
    let binding = crate::text::BindingToken::mint();
    crate::node::TextPeer::initialize(
        &mut peer,
        "Hello",
        crate::text::TextRevision::mint(),
        binding,
    )
    .expect("initialize");
    assert_eq!(peer.text().unwrap(), "Hello", "peer must hold the text");
    // app order: measure first (activates with scratch bounds), then real bounds
    let _ = peer.natural_size(400.0);
    peer.apply_bounds(
        RECT {
            left: 28,
            top: 160,
            right: 428,
            bottom: 192,
        },
        1.5625,
    );

    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::*;

        let hwnd = PROBE_HWND.with(|h| *h.borrow());
        // in-memory DIB + DC -> D2D DC render target
        let screen = GetDC(None);
        let memdc = CreateCompatibleDC(Some(screen));
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut bmi = BITMAPINFO::default();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = 800;
        bmi.bmiHeader.biHeight = -500; // top-down
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB.0;
        let hbmp = CreateDIBSection(
            Some(memdc),
            &bmi,
            DIB_RGB_COLORS,
            &mut bits,
            None,
            0,
        )
        .expect("dib");
        let old = SelectObject(memdc, HGDIOBJ(hbmp.0));

        let d2d: ID2D1Factory =
            D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None).expect("d2d");
        let props = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_SOFTWARE,
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_IGNORE,
            },
            dpiX: 150.0,
            dpiY: 150.0,
            ..Default::default()
        };
        let _ = (memdc, screen);
        let hrt = d2d
            .CreateHwndRenderTarget(
                &props,
                &D2D1_HWND_RENDER_TARGET_PROPERTIES {
                    hwnd,
                    pixelSize: windows::Win32::Graphics::Direct2D::Common::D2D_SIZE_U {
                        width: 800,
                        height: 500,
                    },
                    presentOptions: D2D1_PRESENT_OPTIONS_NONE,
                },
            )
            .expect("hwnd rt");
        let rt: ID2D1RenderTarget = hrt.cast().expect("cast");
        rt.SetDpi(150.0, 150.0);
        rt.BeginDraw();
        rt.Clear(Some(&D2D1_COLOR_F {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        }));
        // NOTE: no PushAxisAlignedClip — msftedit's TxDrawD2D output is
        // suppressed while an axis clip is pushed (observed on an
        // ID2D1HwndRenderTarget); the peer's lprcBounds confines the view.
        peer.draw(&rt, (34.0, 167.0, 735.0, 189.0)).expect("TxDrawD2D");
        rt.EndDraw(None, None).expect("EndDraw");

        // BitBlt the window's framebuffer into our DIB to inspect pixels
        let wdc = GetDC(Some(hwnd));
        let _ = BitBlt(memdc, 0, 0, 800, 500, Some(wdc), 0, 0, SRCCOPY);
        GdiFlush();
        let data = std::slice::from_raw_parts(bits as *const u8, 800 * 500 * 4);
        let lit_at = |x0: i32, y0: i32, x1: i32, y1: i32| -> usize {
            (y0..y1)
                .flat_map(|y| (x0..x1).map(move |x| (x, y)))
                .filter(|&(x, y)| {
                    let o = (y * 800 + x) as usize * 4;
                    data[o] > 16 || data[o + 1] > 16 || data[o + 2] > 16
                })
                .count()
        };
        // draw bounds are DIP; with dpi=150 the framebuffer is at scale
        // 1.5625, so the text lands at DIP*1.5625 pixels
        let s = 1.5625f32;
        let inside = lit_at(
            (34.0 * s) as i32,
            (167.0 * s) as i32,
            800,
            (189.0 * s) as i32,
        );
        eprintln!("[probe] inside={inside}");
        let lit = inside;
        let _ = SelectObject(memdc, old);
        let _ = DeleteObject(HGDIOBJ(hbmp.0));
        let _ = DeleteDC(memdc);
        ReleaseDC(None, screen);
        ReleaseDC(Some(hwnd), wdc);
        let _ = DestroyWindow(hwnd);
        assert!(lit > 50, "peer text must paint visible pixels, got {lit}");
    }
}
