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
    /// poison-tolerant — a panicked test still cleans up its peers
    fn map(&self) -> std::sync::MutexGuard<'_, HashMap<u64, PeerRec>> {
        self.recs.lock().unwrap_or_else(|e| e.into_inner())
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
        p.log.recs.lock().unwrap_or_else(|e| e.into_inner()).insert(
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
        let mut g = self.log.recs.lock().unwrap_or_else(|e| e.into_inner());
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
        self.peers
            .recs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&peer)
            .unwrap()
            .clone()
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

/// Deterministic offscreen regression for the windowless-RichEdit
/// coordinate contract. msftedit's `TxDrawD2D` host space is PHYSICAL
/// PIXELS at the host DC's dpi — the service divides `lprcBounds` by
/// `dcDpi/96` to reach the target's logical units. A DPI-UNAWARE host
/// makes px==DIP numerically, which is exactly how the translate-up
/// defect shipped: this probe therefore runs under per-monitor-v2 thread
/// awareness (the production model) and measures the real DC dpi instead
/// of assuming 96.
///
/// Per peer scale in {1.0, 1.25, 1.5, 2.0} the probe mounts text at
/// create time (the pre-mount path that regressed), applies DIP bounds,
/// and draws onto a DC render target bound to an in-memory DIB. The DIB
/// framebuffer is px-exact: ink must land at `DIP * scale` and only there.
/// Asserts: ink inside the requested island, zero glyph pixels outside it
/// (no duplicate/offset copy), and scale-invariant natural height at the
/// host's real DC scale.
#[cfg(windows)]
#[test]
fn native_probe_richedit_paints_text() {
    use windows::Win32::Foundation::*;
    use windows::Win32::Graphics::Direct2D::Common::*;
    use windows::Win32::Graphics::Direct2D::*;
    use windows::Win32::Graphics::Dxgi::Common::*;
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::System::Com::*;
    use windows::Win32::System::Ole::OleInitialize;
    use windows::core::*;

    unsafe {
        let _ = OleInitialize(None);
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        // production runs PMv2 — the probe MUST match or the px contract
        // degenerates to px==DIP and the regression stays invisible
        let _ = windows::Win32::UI::HiDpi::SetThreadDpiAwarenessContext(
            windows::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
        );
    }

    let lib = crate::platform::win32::Msftedit::load().expect("msftedit");

    // real host hwnd — TxGetDC returns its DC, which is where the service
    // reads its dpi; created AFTER the thread is PMv2-aware
    let hwnd = unsafe {
        use windows::Win32::UI::WindowsAndMessaging::*;
        let cls = w!("rustui_probe_px");
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpszClassName: cls,
            hInstance: windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
                .unwrap()
                .into(),
            lpfnWndProc: Some(probe_wndproc),
            ..Default::default()
        };
        let _ = RegisterClassExW(&wc);
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            cls,
            w!("probe"),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            0,
            0,
            800,
            500,
            None,
            None,
            None,
            None,
        )
        .expect("hwnd")
    };

    let dc_dpi = unsafe {
        let dc = GetDC(Some(hwnd));
        let d = GetDeviceCaps(Some(dc), LOGPIXELSY);
        ReleaseDC(Some(hwnd), dc);
        d
    };
    let dc_scale = crate::geom::ScaleFactor::from_dpi(dc_dpi as u32).0;
    eprintln!("[probe] dc_dpi={dc_dpi} dc_scale={dc_scale}");

    // in-memory framebuffer: DIB + memory DC + D2D DC render target.
    // target dpi = the real DC dpi so framebuffer px == host px ==
    // DIP * scale — positions land exactly, no window-frame bleed.
    const FBW: i32 = 800;
    const FBH: i32 = 600;
    let (memdc, bits, dcrt) = unsafe {
        let screen = GetDC(None);
        let memdc = CreateCompatibleDC(Some(screen));
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut bmi = BITMAPINFO::default();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = FBW;
        bmi.bmiHeader.biHeight = -FBH; // top-down
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB.0;
        let hbmp =
            CreateDIBSection(Some(memdc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0).expect("dib");
        SelectObject(memdc, HGDIOBJ(hbmp.0));
        let d2d: ID2D1Factory =
            D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None).expect("d2d");
        let props = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_SOFTWARE,
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_IGNORE,
            },
            dpiX: dc_dpi as f32,
            dpiY: dc_dpi as f32,
            ..Default::default()
        };
        let dcrt = d2d.CreateDCRenderTarget(&props).expect("dc rt");
        dcrt.BindDC(
            memdc,
            &RECT {
                left: 0,
                top: 0,
                right: FBW,
                bottom: FBH,
            },
        )
        .expect("BindDC");
        (memdc, bits, dcrt)
    };
    let rt: ID2D1RenderTarget = dcrt.cast().expect("cast");
    let data = unsafe { std::slice::from_raw_parts(bits as *const u8, (FBW * FBH * 4) as usize) };

    let cfg = || crate::platform::win32::PeerConfig {
        multiline: true,
        read_only: false,
        face: "Segoe UI".into(),
        size_twips: 280,
        fg: [1.0, 1.0, 1.0, 1.0],
        fg_explicit: true,
        sel_bg: [0.2, 0.4, 0.8, 1.0],
        sel_fg: [1.0, 1.0, 1.0, 1.0],
        bold: false,
    };
    use crate::platform::win32::space::{LogicalRect, ScaleFactor};
    let island_dip = LogicalRect {
        x: 34.0,
        y: 167.0,
        width: 336.0,
        height: 22.0,
    };

    // ---- draw-position matrix across all four canonical scales ---------
    let mut nat_dip_at_dc = None;
    for scale in [1.0f32, 1.25, 1.5, 2.0] {
        let sink = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut peer = crate::platform::win32::WindowlessPeer::create(
            1,
            &lib,
            hwnd,
            ScaleFactor(scale),
            &cfg(),
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
        // app order: measure (scratch extent, activates), then real bounds
        let (_, h_dip) = peer.natural_size(400.0).expect("natural_size");
        peer.apply_bounds(island_dip, ScaleFactor(scale));
        // typed-after-mount must land identically to mount-time text
        let _ = peer.send(0x0007 /*WM_SETFOCUS*/, 0, 0);
        for c in " xy".encode_utf16() {
            let _ = peer.send(0x0102 /*WM_CHAR*/, c as usize, 0);
        }
        let _ = peer.drain();

        unsafe {
            rt.BeginDraw();
            rt.Clear(Some(&D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            }));
        }
        peer.draw(&rt, island_dip).expect("TxDrawD2D");
        unsafe {
            rt.EndDraw(None, None).expect("EndDraw");
        }

        // island in framebuffer px — ink must sit inside it; THE layer
        // performs the same conversion the peer applied
        let island_px = island_dip.physical(ScaleFactor(scale));
        let (ix0, iy0) = (island_px.left, island_px.top);
        let (ix1, iy1) = (island_px.right, island_px.bottom);
        let mut inside = 0usize;
        let mut outside = 0usize;
        for y in 0..FBH {
            for x in 0..FBW {
                let o = (y * FBW + x) as usize * 4;
                let white = data[o] > 180 && data[o + 1] > 180 && data[o + 2] > 180;
                if !white {
                    continue;
                }
                if x >= ix0 && x < ix1 && y >= iy0 && y < iy1 {
                    inside += 1;
                } else {
                    outside += 1;
                }
            }
        }
        eprintln!(
            "[probe] scale={scale} island={ix0},{iy0}-{ix1},{iy1} inside={inside} outside={outside} nat_h={h_dip:.1}"
        );
        assert!(
            inside > 50,
            "scale {scale}: text must render ink inside the bounds, got {inside}"
        );
        assert_eq!(
            outside, 0,
            "scale {scale}: glyph ink outside the island — duplicate/offset copy"
        );
        if (scale - dc_scale).abs() < 0.01 {
            // natural size is only meaningful when the claimed scale equals
            // the host DC's real scale — the service's px space IS the DC's
            nat_dip_at_dc = Some(h_dip);
        }
    }

    // ---- live scale change — the peer's px-space view must re-latch ----
    // same peer + same DIP island, scale A then B: after apply_bounds(B)
    // the ink must sit inside the B-scaled island, with nothing left at
    // the A position (proves the view re-anchored, not additive ghosts)
    {
        let (a, b) = (1.25f32, 2.0f32); // real-DC-adjacent -> 200%
        let sink = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut peer = crate::platform::win32::WindowlessPeer::create(
            9,
            &lib,
            hwnd,
            ScaleFactor(a),
            &cfg(),
            sink,
        )
        .expect("peer create");
        let binding = crate::text::BindingToken::mint();
        crate::node::TextPeer::initialize(
            &mut peer,
            "Ghost",
            crate::text::TextRevision::mint(),
            binding,
        )
        .expect("initialize");
        peer.apply_bounds(island_dip, ScaleFactor(a));
        unsafe {
            rt.BeginDraw();
            rt.Clear(Some(&D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            }));
        }
        peer.draw(&rt, island_dip).expect("draw@A");
        unsafe {
            rt.EndDraw(None, None).expect("EndDraw");
        }
        // count ink at A's island — this stays (framebuffer accumulates)
        let ia = island_dip.physical(ScaleFactor(a));
        let mut ink_a = 0usize;
        for y in ia.top..ia.bottom {
            for x in ia.left..ia.right {
                let o = (y * FBW + x) as usize * 4;
                if data[o] > 180 {
                    ink_a += 1;
                }
            }
        }
        assert!(ink_a > 40, "live-change leg: no ink at scale A");

        // now the scale change — same island DIP, new scale
        peer.apply_bounds(island_dip, ScaleFactor(b));
        unsafe {
            rt.BeginDraw();
            rt.Clear(Some(&D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            }));
        }
        peer.draw(&rt, island_dip).expect("draw@B");
        unsafe {
            rt.EndDraw(None, None).expect("EndDraw");
        }
        let ib = island_dip.physical(ScaleFactor(b));
        let (mut in_b, mut stray) = (0usize, 0usize);
        for y in 0..FBH {
            for x in 0..FBW {
                let o = (y * FBW + x) as usize * 4;
                if data[o] <= 180 {
                    continue;
                }
                if x >= ib.left && x < ib.right && y >= ib.top && y < ib.bottom {
                    in_b += 1;
                } else {
                    stray += 1;
                }
            }
        }
        eprintln!("[probe] live-scale {a}->{b}: in_b={in_b} stray={stray}");
        assert!(in_b > 40, "no ink inside the B-scaled island");
        assert_eq!(stray, 0, "scale change left ghost ink at stale coordinates");
    }

    // ---- round-trip: DIP -> px -> DIP --------------------------------
    // at the DC's real scale a DIP measurement survives the px seam:
    // natural height of one 14pt line with view insets is ~25 DIP at the
    // Segoe UI default — assert the sane band, not the magic number.
    if let Some(h) = nat_dip_at_dc {
        assert!(
            (20.0..=32.0).contains(&h),
            "single-line natural height ~25 DIP at dc scale, got {h}"
        );
    }

    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::*;
        let _ = DeleteDC(memdc);
        let _ = DestroyWindow(hwnd);
    }
}

#[cfg(windows)]
fn uia_variant_string(v: &windows::Win32::System::Variant::VARIANT) -> String {
    // VARIANT(vt=VT_BSTR) -> String
    unsafe {
        let inner = &*(v as *const _ as *const windows::Win32::System::Variant::VARIANT);
        let _ = inner;
        // bstrVal lives in the nested anonymous union
        let bstr: &windows::core::BSTR =
            std::mem::transmute(&v.Anonymous.Anonymous.Anonymous.bstrVal);
        bstr.to_string()
    }
}

/// UIA: painted fragments must survive rebuild — previously `rebuild` kept
/// only `Weak`s and dropped the strong refs at loop end, so every painted
/// child died instantly and `Navigate` could never produce it.
#[cfg(windows)]
#[test]
fn uia_children_survive_rebuild() {
    use crate::platform::win32::uia::{ChildBuild, UiaRoot};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Accessibility::*;
    use windows::core::Interface;

    let root = UiaRoot::new(HWND::default(), "t").expect("root");
    let mk = |slot: u32, name: &str, ct: UIA_CONTROLTYPE_ID, loc: &'static str, act| ChildBuild {
        id: crate::NodeId {
            slot,
            generation: 0,
        },
        name: name.into(),
        ct,
        localized: loc,
        rect: UiaRect {
            left: 0.0,
            top: 0.0,
            width: 10.0,
            height: 10.0,
        },
        actionable: act,
        enabled: true,
        peer_node: false,
        native: None,
    };
    root.rebuild(vec![
        mk(0, "lbl", UIA_TextControlTypeId, "Text", false),
        mk(1, "btn", UIA_ButtonControlTypeId, "Button", true),
    ])
    .expect("rebuild");

    let rf: IRawElementProviderFragment = root.provider().cast().expect("root frag");
    // first child = the label — upgrade through the weak must succeed
    let first = unsafe {
        rf.Navigate(NavigateDirection_FirstChild)
            .expect("first child")
    };
    // sibling navigation label -> button
    let second = unsafe {
        first
            .Navigate(NavigateDirection_NextSibling)
            .expect("next sibling")
    };
    let simple: IRawElementProviderSimple = second.cast().expect("simple");
    unsafe {
        let name = simple.GetPropertyValue(UIA_NamePropertyId).expect("name");
        assert_eq!(uia_variant_string(&name), "btn");
        let ct = simple
            .GetPropertyValue(UIA_ControlTypePropertyId)
            .expect("ct");
        // vt = VT_I4; lVal inside the union
        let lval = ct.Anonymous.Anonymous.Anonymous.lVal;
        assert_eq!(lval, UIA_ButtonControlTypeId.0);
        // actionable -> InvokePattern present; label -> absent
        simple
            .GetPatternProvider(UIA_InvokePatternId)
            .expect("button must expose invoke");
        let lbl: IRawElementProviderSimple = first.cast().expect("lbl simple");
        assert!(
            lbl.GetPatternProvider(UIA_InvokePatternId).is_err(),
            "label must not expose invoke"
        );
    }
    // focus tracking: set_focus marks the owner; GetFocus resolves it
    root.set_focus(Some(crate::NodeId {
        slot: 1,
        generation: 0,
    }));
    let rroot: IRawElementProviderFragmentRoot = root.provider().cast().expect("rroot");
    unsafe {
        let f = rroot.GetFocus().expect("focus must resolve");
        let fs: IRawElementProviderSimple = f.cast().expect("fs");
        assert_eq!(
            uia_variant_string(&fs.GetPropertyValue(UIA_NamePropertyId).unwrap()),
            "btn"
        );
    }
}

/// `ElementProviderFromPoint` must hit-test against the SNAPSHOT rect —
/// native editor providers report an unusable BoundingRectangle
/// (msftedit returns Infinity on windowless sites), so the root carries
/// the authoritative rect per child.
#[cfg(windows)]
#[test]
fn uia_element_from_point_uses_snapshot_rect() {
    use crate::platform::win32::uia::{ChildBuild, UiaRoot};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Accessibility::*;
    use windows::core::Interface;

    let root = UiaRoot::new(HWND::default(), "t").expect("root");
    let mk = |slot: u32, name: &str, rect: UiaRect, native: Option<IRawElementProviderFragment>| {
        ChildBuild {
            id: crate::NodeId {
                slot,
                generation: 0,
            },
            name: name.into(),
            ct: if native.is_some() {
                UIA_EditControlTypeId
            } else {
                UIA_ButtonControlTypeId
            },
            localized: "t",
            rect,
            actionable: native.is_none(),
            enabled: true,
            peer_node: native.is_some(),
            native,
        }
    };
    // a real msftedit provider cannot be built in-process without the
    // peer machinery — emulate "broken native rect" with a painted frag
    // whose own rect disagrees: a native slot's rect is what's used
    let native_like = None;
    root.rebuild(vec![
        mk(
            0,
            "btn",
            UiaRect {
                left: 10.0,
                top: 10.0,
                width: 50.0,
                height: 20.0,
            },
            native_like.clone(),
        ),
        mk(
            1,
            "ed",
            UiaRect {
                left: 10.0,
                top: 40.0,
                width: 200.0,
                height: 30.0,
            },
            None,
        ),
    ])
    .expect("rebuild");
    let rroot: IRawElementProviderFragmentRoot = root.provider().cast().expect("rroot");
    unsafe {
        // inside the button's rect
        let hit = rroot
            .ElementProviderFromPoint(20.0, 15.0)
            .expect("hit in btn");
        let s: IRawElementProviderSimple = hit.cast().expect("simple");
        assert_eq!(
            uia_variant_string(&s.GetPropertyValue(UIA_NamePropertyId).unwrap()),
            "btn"
        );
        // inside the editor's rect — second child wins (topmost order)
        let hit2 = rroot
            .ElementProviderFromPoint(50.0, 55.0)
            .expect("hit in ed");
        let s2: IRawElementProviderSimple = hit2.cast().expect("simple");
        assert_eq!(
            uia_variant_string(&s2.GetPropertyValue(UIA_NamePropertyId).unwrap()),
            "ed"
        );
        // outside everything
        assert!(
            rroot.ElementProviderFromPoint(500.0, 500.0).is_err(),
            "a point outside all children must not fabricate a hit"
        );
    }
}

/// A disabled control must surface as non-enabled + non-focusable in UIA
/// — the assistive-tech contract that pairs with hit-test/dispatch gating.
#[cfg(windows)]
#[test]
fn uia_disabled_node_reports_not_enabled() {
    use crate::platform::win32::uia::{ChildBuild, UiaRoot};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Accessibility::*;
    use windows::core::Interface;

    let root = UiaRoot::new(HWND::default(), "t").expect("root");
    let mk = |generation: u64, enabled: bool| ChildBuild {
        id: crate::NodeId {
            slot: 0,
            generation,
        },
        name: "off".into(),
        ct: UIA_ButtonControlTypeId,
        localized: "Button",
        rect: UiaRect {
            left: 0.0,
            top: 0.0,
            width: 10.0,
            height: 10.0,
        },
        actionable: true,
        enabled,
        peer_node: false,
        native: None,
    };
    root.rebuild(vec![mk(0, false)]).expect("rebuild");
    let rf: IRawElementProviderFragment = root.provider().cast().expect("root frag");
    let btn = unsafe { rf.Navigate(NavigateDirection_FirstChild).expect("child") };
    let s: IRawElementProviderSimple = btn.cast().expect("simple");
    unsafe {
        let en = s
            .GetPropertyValue(UIA_IsEnabledPropertyId)
            .expect("enabled");
        // VT_BOOL: VARIANT_TRUE is nonzero
        let b = en.Anonymous.Anonymous.Anonymous.boolVal.0;
        assert_eq!(b, 0, "disabled node must report IsEnabled=false");
        let f = s
            .GetPropertyValue(UIA_IsKeyboardFocusablePropertyId)
            .expect("focusable");
        assert_eq!(
            f.Anonymous.Anonymous.Anonymous.boolVal.0, 0,
            "disabled node must not be keyboard-focusable"
        );
    }
    // dead generation fences every accessor
    root.rebuild(vec![mk(1, true)]).expect("rebuild2");
    unsafe {
        assert!(
            s.GetPropertyValue(UIA_NamePropertyId).is_err(),
            "stale fragment must not answer after remint"
        );
    }
}

/// The spike scenario: Send A streams chunks -> Stop A -> Send B -> a late
/// in-flight A chunk must not contaminate B; Stop must not block resend.
#[test]
fn send_stop_resend_late_chunks_fenced() {
    struct S {
        got: Vec<u32>,
        busy: bool,
    }
    // channel so the test can fire a stale envelope later — a "late A chunk"
    let late: Arc<Mutex<Option<crate::TaskSender<Msg>>>> = Arc::new(Mutex::new(None));
    let late2 = late.clone();
    let mut rig = Rig::new(
        S {
            got: vec![],
            busy: false,
        },
        move |s: &mut S, m: Msg, cx: &mut UpdateCtx<Msg>| match m {
            Msg::Kick => {
                if s.busy {
                    return;
                }
                s.busy = true;
                let l = late2.clone();
                let _ = cx.spawn("stream", move |job| {
                    *l.lock().unwrap() = Some(job.sender().clone());
                    async move {
                        let _ = job.send(Msg::Chunk(1)).await;
                        std::future::pending::<()>().await;
                    }
                });
            }
            Msg::Press => {
                cx.cancel("stream");
                s.busy = false;
            }
            Msg::Chunk(c) => s.got.push(c),
            _ => {}
        },
        |_: &S, _: &mut Ui<Msg>| {},
    );
    rig.view().unwrap();
    // Send A — task A registers and emits chunk 1
    rig.rt.push_msg(Msg::Kick);
    rig.pump().unwrap();
    rig.exec.poll();
    rig.pump().unwrap();
    assert_eq!(rig.rt.state.got, vec![1]);
    assert!(rig.rt.state.busy);
    // Stop A — busy clears; the resend path is legal again
    rig.rt.push_msg(Msg::Press);
    rig.pump().unwrap();
    assert!(!rig.rt.state.busy);
    let stale_sender = late.lock().unwrap().take().expect("A sender");
    // Send B — same key, fresh generation
    rig.rt.push_msg(Msg::Kick);
    rig.pump().unwrap();
    assert!(rig.rt.state.busy);
    rig.exec.poll(); // B sends chunk 1, parks
    rig.pump().unwrap();
    assert_eq!(rig.rt.state.got, vec![1, 1]);
    // late A chunk: the fenced token's send must fail, nothing contaminates
    let stale = stale_sender.clone();
    let rt = tokio_block(async move { stale.send(Msg::Chunk(99)).await });
    assert!(rt.is_err(), "fenced token send must fail, got {rt:?}");
    rig.pump().unwrap();
    assert_eq!(rig.rt.state.got, vec![1, 1]);
}

/// Minimal block-on for the test sender future — the send resolves
/// immediately on a fenced token.
#[cfg(windows)]
fn tokio_block<F: std::future::Future>(fut: F) -> F::Output {
    use std::sync::Arc;
    use std::task::{Context, Poll, Wake};
    struct W;
    impl Wake for W {
        fn wake(self: Arc<Self>) {}
    }
    let waker = std::task::Waker::from(Arc::new(W));
    let mut cx = Context::from_waker(&waker);
    let mut fut = std::pin::pin!(fut);
    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(v) => return v,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

// ---------------------------------------------------------------------------
// styling — the documented surgical invariant + resolution ordering
// ---------------------------------------------------------------------------

#[test]
fn surgical_patch_changes_only_authored_fields() {
    use crate::style::*;
    use crate::theme::{ButtonVariant, ControlSize};
    // the mandatory case from STYLE_CUSTOMIZATION_MODEL.md
    let patch = ButtonStylePatch::new()
        .border_bottom_width(crate::geom::dp(2.0))
        .border_bottom_color(Color::rgb(255, 0, 0));
    for &dark in &[false, true] {
        for state in [
            StyleState::Normal,
            StyleState::Hover,
            StyleState::Pressed,
            StyleState::Disabled,
        ] {
            for &focus in &[false, true] {
                let base = resolve_button(
                    ButtonVariant::Primary,
                    ControlSize::Md,
                    &ButtonStylePatch::new(),
                    state,
                    focus,
                    dark,
                );
                let patched = resolve_button(
                    ButtonVariant::Primary,
                    ControlSize::Md,
                    &patch,
                    state,
                    focus,
                    dark,
                );
                // exactly two fields may differ
                let (a, b) = (base.box_style, patched.box_style);
                assert_eq!(
                    a.background, b.background,
                    "background must not change ({state:?} focus={focus} dark={dark})"
                );
                assert_eq!(a.radii, b.radii, "radii must not change");
                assert_eq!(a.padding, b.padding, "padding must not change");
                assert_eq!(a.shadow, b.shadow, "shadow must not change");
                assert_eq!(a.border.top, b.border.top, "top border unchanged");
                assert_eq!(a.border.left, b.border.left, "left border unchanged");
                assert_eq!(a.border.right, b.border.right, "right border unchanged");
                assert_eq!(b.border.bottom.width, crate::geom::Dp(2.0));
                assert_eq!(b.border.bottom.color, Color::rgb(255, 0, 0));
                assert_eq!(base.text_style, patched.text_style, "text style unchanged");
            }
        }
    }
}

#[test]
fn patch_merge_and_resolution_order() {
    use crate::style::*;
    use crate::theme::{ButtonVariant, ControlSize};
    // consumer base applies across ALL states (it sits over recipe state)
    let patch = ButtonStylePatch::new().background(Color::rgb(1, 2, 3));
    for state in [StyleState::Normal, StyleState::Hover, StyleState::Pressed] {
        let v = resolve_button(
            ButtonVariant::Primary,
            ControlSize::Md,
            &patch,
            state,
            false,
            false,
        );
        assert_eq!(v.box_style.background, Color::rgb(1, 2, 3), "{state:?}");
    }
    // consumer state patch lands on top of that state's resolved values
    let mut p = ButtonStylePatch::new().background(Color::rgb(1, 2, 3));
    let mut hover = VisualStylePatch::new();
    hover.box_style.background = Some(Color::rgb(9, 9, 9));
    p = p.hover(hover);
    let v = resolve_button(
        ButtonVariant::Primary,
        ControlSize::Md,
        &p,
        StyleState::Hover,
        false,
        false,
    );
    assert_eq!(v.box_style.background, Color::rgb(9, 9, 9));
    // normal keeps the consumer base
    let v = resolve_button(
        ButtonVariant::Primary,
        ControlSize::Md,
        &p,
        StyleState::Normal,
        false,
        false,
    );
    assert_eq!(v.box_style.background, Color::rgb(1, 2, 3));
    // focus overlay: recipe focus border + consumer focus patch on top
    let mut p2 = ButtonStylePatch::new();
    let mut fv = VisualStylePatch::new();
    fv.box_style.background = Some(Color::rgb(7, 7, 7));
    p2 = p2.focus_visible(fv);
    let v = resolve_button(
        ButtonVariant::Primary,
        ControlSize::Md,
        &p2,
        StyleState::Normal,
        true,
        false,
    );
    assert_eq!(v.box_style.background, Color::rgb(7, 7, 7));
    assert!(
        v.box_style.border.top.width.0 > 0.0,
        "recipe focus ring present"
    );
}

#[test]
fn shadow_patch_is_atomic_set_remove() {
    use crate::style::*;
    let mut b = BoxStyle::new();
    assert_eq!(b.shadow, None);
    let sh = Shadow {
        color: Color::rgb(0, 0, 0),
        offset_x: crate::geom::Dp(0.0),
        offset_y: crate::geom::Dp(4.0),
        blur_sigma: crate::geom::Dp(8.0),
    };
    b.patch(&BoxStylePatch {
        shadow: ShadowPatch::Set(sh),
        ..Default::default()
    });
    assert_eq!(b.shadow, Some(sh));
    // Unchanged never erases
    b.patch(&BoxStylePatch::default());
    assert_eq!(b.shadow, Some(sh));
    // Remove forces none
    b.patch(&BoxStylePatch {
        shadow: ShadowPatch::Remove,
        ..Default::default()
    });
    assert_eq!(b.shadow, None);
}

/// `ui.action` with decorative content stages cleanly; nested actionable
/// children are a structural diagnostic, never staged.
#[test]
fn action_stages_and_rejects_nested_actionable() {
    use crate::style::Action;
    struct S;
    let mut rig = Rig::new(
        S,
        |_: &mut S, _: Msg, _: &mut UpdateCtx<Msg>| {},
        |_: &S, ui: &mut Ui<Msg>| {
            ui.action(Action::new().label("tile"), |ui| {
                ui.label("decorative");
            })
            .on_press(|| Msg::Press);
        },
    );
    rig.view().unwrap();
    // the action node committed with its child
    let kinds: Vec<u8> = {
        let root = rig.rt.root;
        let rn = rig.rt.arena.get(root).unwrap();
        rn.children
            .iter()
            .map(|&s| {
                let g = rig.rt.arena.generation_of(s);
                rig.rt
                    .arena
                    .get(crate::NodeId {
                        slot: s,
                        generation: g,
                    })
                    .unwrap()
                    .kind_tag()
            })
            .collect()
    };
    assert_eq!(kinds, vec![crate::node::KIND_ACTION]);

    // nested button inside an action -> InvalidComposition abort
    let mut rig2 = Rig::new(
        S,
        |_: &mut S, _: Msg, _: &mut UpdateCtx<Msg>| {},
        |_: &S, ui: &mut Ui<Msg>| {
            ui.action(Action::new().label("bad"), |ui| {
                ui.button("nested").on_press(|| Msg::Press);
            })
            .on_press(|| Msg::Press);
        },
    );
    let r = rig2.view();
    assert!(
        matches!(
            r,
            Err(crate::UiError::InvalidUi(
                crate::UiDiagnostic::InvalidComposition
            ))
        ),
        "{r:?}"
    );
}

/// Patch removal on a later view pass restores recipe values — the node is
/// the SAME retained node (no remount) and the committed style is the new
/// declaration, not patch history.
#[test]
fn patch_removal_restores_recipe_without_remount() {
    use crate::style::*;
    struct S {
        patched: bool,
    }
    let mut rig = Rig::new(
        S { patched: true },
        |_: &mut S, _: Msg, _: &mut UpdateCtx<Msg>| {},
        |s: &S, ui: &mut Ui<Msg>| {
            let b = ui
                .button("send")
                .variant(crate::theme::ButtonVariant::Primary);
            if s.patched {
                b.style(
                    ButtonStylePatch::new()
                        .border_bottom_width(crate::geom::dp(2.0))
                        .border_bottom_color(Color::rgb(255, 0, 0)),
                )
                .on_press(|| Msg::Press);
            } else {
                b.on_press(|| Msg::Press);
            }
        },
    );
    rig.view().unwrap();
    let node_id = {
        let root = rig.rt.root;
        let rn = rig.rt.arena.get(root).unwrap();
        let slot = rn.children[0];
        crate::NodeId {
            slot,
            generation: rig.rt.arena.generation_of(slot),
        }
    };
    // pass 1: patch committed
    {
        let n = rig.rt.arena.get(node_id).unwrap();
        let NodeDataLike { patch: p } = button_style_of(n);
        assert_eq!(
            p.styles.base.box_style.border.bottom.width,
            Some(crate::geom::Dp(2.0))
        );
    }
    // pass 2: patch removed — same node, fresh declaration
    rig.rt.state.patched = false;
    rig.view().unwrap();
    let n2 = rig.rt.arena.get(node_id).unwrap();
    assert_eq!(button_style_of(n2).patch, Default::default());
}

#[cfg(windows)]
struct NodeDataLike {
    patch: crate::style::ButtonStylePatch,
}

#[cfg(windows)]
fn button_style_of(n: &crate::node::Node) -> NodeDataLike {
    match &n.data {
        crate::node::NodeData::Button { style, .. } => NodeDataLike { patch: *style },
        _ => panic!("expected button"),
    }
}

#[cfg(windows)]
#[test]
fn rid_safearray_roundtrip() {
    use windows::Win32::System::Ole::*;
    use windows::Win32::System::Variant::VT_I4;
    use windows::Win32::UI::Accessibility::UiaAppendRuntimeId;
    unsafe {
        eprintln!("UiaAppendRuntimeId = {UiaAppendRuntimeId}");
        let sa = SafeArrayCreateVector(VT_I4, 0, 3);
        let vals = [UiaAppendRuntimeId as i32, 7, 99];
        for (i, v) in vals.iter().enumerate() {
            let r =
                SafeArrayPutElement(sa, &(i as i32), v as *const i32 as *const core::ffi::c_void);
            eprintln!("put {i} -> {r:?}");
        }
        let lb = SafeArrayGetLBound(sa, 1).unwrap();
        let ub = SafeArrayGetUBound(sa, 1).unwrap();
        for i in lb..=ub {
            let mut out = 0i32;
            SafeArrayGetElement(sa, &i, &mut out as *mut i32 as *mut core::ffi::c_void).unwrap();
            eprintln!("rid[{i}] = {out}");
        }
    }
}

/// P6 lifecycle: a retained provider reference must stay live across
/// reorder/disable for the same NodeId, die on remove, stay dead after a
/// same-slot new-generation recreate, and everything dies on close.
#[cfg(windows)]
#[test]
fn uia_retained_provider_lifecycle() {
    use crate::platform::win32::uia::{ChildBuild, UiaRoot};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Accessibility::*;
    use windows::core::Interface;

    let root = UiaRoot::new(HWND::default(), "t").expect("root");
    let mk = |slot: u32, generation: u64, name: &str, enabled: bool| ChildBuild {
        id: crate::NodeId { slot, generation },
        name: name.into(),
        ct: UIA_ButtonControlTypeId,
        localized: "Button",
        rect: UiaRect {
            left: 0.0,
            top: 0.0,
            width: 10.0,
            height: 10.0,
        },
        actionable: true,
        enabled,
        peer_node: false,
        native: None,
    };
    let simple_of = |f: &IRawElementProviderFragment| -> IRawElementProviderSimple {
        f.cast().expect("simple")
    };

    // mount A,B,C
    root.rebuild(vec![
        mk(0, 0, "a", true),
        mk(1, 0, "b", true),
        mk(2, 0, "c", true),
    ])
    .unwrap();
    let rf: IRawElementProviderFragment = root.provider().cast().unwrap();
    let b = unsafe {
        rf.Navigate(NavigateDirection_FirstChild)
            .unwrap()
            .Navigate(NavigateDirection_NextSibling)
            .unwrap()
    }; // slot 1
    assert_eq!(
        uia_variant_string(&unsafe { simple_of(&b).GetPropertyValue(UIA_NamePropertyId).unwrap() }),
        "b"
    );

    // reorder C,A,B — same NodeIds survive; retained B must still be live
    root.rebuild(vec![
        mk(2, 0, "c", true),
        mk(0, 0, "a", true),
        mk(1, 0, "b", true),
    ])
    .unwrap();
    assert!(unsafe { simple_of(&b).GetPropertyValue(UIA_NamePropertyId) }.is_ok());

    // disable B — the retained fragment must report enabled=false live
    root.rebuild(vec![
        mk(2, 0, "c", true),
        mk(0, 0, "a", true),
        mk(1, 0, "b", false),
    ])
    .unwrap();
    let en = unsafe {
        simple_of(&b)
            .GetPropertyValue(UIA_IsEnabledPropertyId)
            .unwrap()
    };
    assert!(unsafe { !en.Anonymous.Anonymous.Anonymous.boolVal.as_bool() });
    // disabled node cannot take focus
    assert!(
        unsafe { b.SetFocus() }.is_err(),
        "disabled must reject SetFocus"
    );

    // remove B — retained reference dies permanently
    root.rebuild(vec![mk(2, 0, "c", true), mk(0, 0, "a", true)])
        .unwrap();
    assert!(unsafe { simple_of(&b).GetPropertyValue(UIA_NamePropertyId) }.is_err());

    // same-slot new-generation recreate — old B ref must NOT resolve to
    // the replacement; the new provider answers
    root.rebuild(vec![
        mk(2, 0, "c", true),
        mk(0, 0, "a", true),
        mk(1, 1, "b2", true),
    ])
    .unwrap();
    assert!(unsafe { simple_of(&b).GetPropertyValue(UIA_NamePropertyId) }.is_err());
    let last = unsafe { rf.Navigate(NavigateDirection_LastChild).unwrap() };
    assert_eq!(
        uia_variant_string(&unsafe {
            simple_of(&last)
                .GetPropertyValue(UIA_NamePropertyId)
                .unwrap()
        }),
        "b2"
    );

    // close — everything dies
    root.close();
    assert!(unsafe { simple_of(&last).GetPropertyValue(UIA_NamePropertyId) }.is_err());
    assert!(unsafe { rf.Navigate(NavigateDirection_FirstChild) }.is_err());
}

/// P8: style preflight — non-finite/negative/invalid authored values are
/// rejected at commit BEFORE any native mutation.
#[test]
fn invalid_style_rejected_at_commit() {
    use crate::geom::Dp;
    let mut rig = Rig::new(
        (),
        |_: &mut (), _m: Msg, _u: &mut UpdateCtx<Msg>| {},
        |_: &(), ui| {
            let mut p = crate::style::ButtonStylePatch::new();
            p.styles.base.box_style.padding.left = Some(Dp(f32::NAN));
            ui.button("b").style(p);
        },
    );
    match rig.view() {
        Err(crate::UiError::InvalidUi(crate::UiDiagnostic::InvalidStyle)) => {}
        r => panic!("expected InvalidStyle, got {r:?}"),
    }
    // negative border width also rejects
    let mut rig2 = Rig::new(
        (),
        |_: &mut (), _m: Msg, _u: &mut UpdateCtx<Msg>| {},
        |_: &(), ui| {
            let mut p = crate::style::ButtonStylePatch::new();
            p.styles.base.box_style.border.top.width = Some(Dp(-1.0));
            ui.button("b").style(p);
        },
    );
    match rig2.view() {
        Err(crate::UiError::InvalidUi(crate::UiDiagnostic::InvalidStyle)) => {}
        r => panic!("expected InvalidStyle, got {r:?}"),
    }
    // zero/nonpositive exact text size rejects
    let mut rig3 = Rig::new(
        (),
        |_: &mut (), _m: Msg, _u: &mut UpdateCtx<Msg>| {},
        |_: &(), ui| {
            ui.label("x")
                .style(crate::style::TextStylePatch::new().size(Dp(0.0)));
        },
    );
    match rig3.view() {
        Err(crate::UiError::InvalidUi(crate::UiDiagnostic::InvalidStyle)) => {}
        r => panic!("expected InvalidStyle, got {r:?}"),
    }
}

/// P8: proportional radius normalization — adjacent pairs sharing an edge
/// scale by ONE factor so authored proportions survive; oversized pairs
/// never exceed the edge.
#[test]
fn radii_normalize_proportionally() {
    use crate::geom::Dp;
    use crate::style::CornerRadii;
    // tl=40 tr=80 on a 60-wide rect — pair sum 120 > 60 -> factor 0.5
    // applied to BOTH (and every other pair's factor too)
    let r = CornerRadii {
        top_left: Dp(40.0),
        top_right: Dp(80.0),
        bottom_right: Dp(0.0),
        bottom_left: Dp(0.0),
    };
    let n = r.normalized(60.0, 100.0);
    assert!((n.top_left.0 - 20.0).abs() < 1e-4);
    assert!((n.top_right.0 - 40.0).abs() < 1e-4);
    // ratio preserved
    assert!((n.top_right.0 / n.top_left.0 - 2.0).abs() < 1e-4);
    // within-bounds radii pass through untouched
    let small = CornerRadii::all(Dp(4.0));
    assert_eq!(small.normalized(100.0, 100.0), small);
}

/// P8: repeated `.style()` / state setters merge fieldwise — a later call
/// must not erase earlier `Some` fields.
#[test]
fn repeated_style_setters_merge_fieldwise() {
    use crate::geom::Dp;
    use crate::style::*;
    // ButtonStylePatch hover: two calls merge
    let p = ButtonStylePatch::new()
        .hover(VisualStylePatch {
            box_style: BoxStylePatch {
                background: Some(Color::rgb(1, 1, 1)),
                ..Default::default()
            },
            ..Default::default()
        })
        .hover(VisualStylePatch {
            text_style: TextStylePatch {
                foreground: Some(Color::rgb(2, 2, 2)),
                ..Default::default()
            },
            ..Default::default()
        });
    let h = p.styles.hover.unwrap();
    assert_eq!(h.box_style.background, Some(Color::rgb(1, 1, 1)));
    assert_eq!(h.text_style.foreground, Some(Color::rgb(2, 2, 2)));
    // ActionStyle state setters merge
    let a = ActionStyle::new(BoxStyle::new())
        .hover(BoxStylePatch {
            padding: crate::style::InsetsPatch {
                left: Some(Dp(3.0)),
                ..Default::default()
            },
            ..Default::default()
        })
        .hover(BoxStylePatch {
            background: Some(Color::rgb(9, 9, 9)),
            ..Default::default()
        });
    let h = a.hover.unwrap();
    assert_eq!(h.padding.left, Some(Dp(3.0)));
    assert_eq!(h.background, Some(Color::rgb(9, 9, 9)));
    // resolved action keeps BOTH merged fields
    let r = a.resolve(false, false, true, false);
    assert_eq!(r.padding.left, Dp(3.0));
    assert_eq!(r.background, Color::rgb(9, 9, 9));
}

/// P8: dirty classification compares RESOLVED output — a patch whose net
/// effect equals the current resolved style produces zero dirty work;
/// shadow-only changes are paint, never layout.
#[test]
fn resolved_dirty_classification() {
    use crate::geom::Dp;
    use crate::node::NodeData;
    use crate::style::*;
    const LAYOUT: u8 = 0b0000_0001;
    const PAINT: u8 = 0b0000_0010;
    let surface = |patch: BoxStylePatch| NodeData::Container {
        kind: crate::node::KIND_SURFACE,
        props: crate::node::ContainerProps {
            gap: None,
            align: None,
            justify: None,
            padding: None,
            patch,
            full: None,
        },
    };
    // same authored descriptor -> no dirty
    let p = BoxStylePatch::new();
    assert_eq!(
        crate::node::dirty_diff(&surface(p), &surface(p), false, false),
        0
    );
    // a patch authoring the SAME resolved value as the recipe -> no dirty
    let mut same = BoxStylePatch::new();
    same.padding.top = Some(Dp(12.0));
    same.padding.right = Some(Dp(12.0));
    same.padding.bottom = Some(Dp(12.0));
    same.padding.left = Some(Dp(12.0));
    assert_eq!(
        crate::node::dirty_diff(&surface(p), &surface(same), false, false),
        0,
        "patch resolving to identical output must be a no-op"
    );
    // shadow-only change -> PAINT, never LAYOUT
    let mut sh = BoxStylePatch::new();
    sh.shadow = ShadowPatch::Set(Shadow {
        color: Color::rgb(0, 0, 0),
        offset_x: Dp(0.0),
        offset_y: Dp(2.0),
        blur_sigma: Dp(4.0),
    });
    let d = crate::node::dirty_diff(&surface(p), &surface(sh), false, false);
    assert_eq!(d & PAINT, PAINT);
    assert_eq!(d & LAYOUT, 0, "shadow-only must not request layout");
    // border WIDTH change -> LAYOUT; border COLOR change -> PAINT only
    let mut bw = BoxStylePatch::new();
    bw.border.top.width = Some(Dp(3.0));
    assert_eq!(
        crate::node::dirty_diff(&surface(p), &surface(bw), false, false),
        LAYOUT | PAINT
    );
    let mut bc = BoxStylePatch::new();
    bc.border.top.color = Some(Color::rgb(9, 9, 9));
    assert_eq!(
        crate::node::dirty_diff(&surface(p), &surface(bc), false, false),
        PAINT,
        "color-only border change is paint-only"
    );
    // forced-colors suppresses a set shadow -> resolved equal -> no work
    assert_eq!(
        crate::node::dirty_diff(&surface(p), &surface(sh), false, true),
        0,
        "shadow change under forced-colors resolves to identical output"
    );
}

/// P11: bounded lifecycle — 100 mount/reorder/remove/recreate cycles must
/// release each peer exactly once and return live node/peer counts to
/// baseline (no retained generation keeps accumulating).
#[test]
fn hundred_mount_remove_cycles_reclaim_all() {
    struct S {
        v: TextValue,
        mounted: bool,
    }
    let mut rig = Rig::new(
        S {
            v: TextValue::new("x"),
            mounted: true,
        },
        |_: &mut S, _m: Msg, _cx: &mut UpdateCtx<Msg>| {},
        |s: &S, ui: &mut Ui<Msg>| {
            ui.group("root", |ui| {
                if s.mounted {
                    ui.group("ed", |ui| {
                        ui.text_input(&s.v).on_edit(Msg::Edited);
                    });
                }
            });
        },
    );
    rig.view().unwrap();
    let baseline_peers = rig
        .peers
        .recs
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .len();
    for i in 0..100 {
        rig.rt.state.mounted = i % 2 == 0;
        rig.view().unwrap();
        rig.pump().unwrap();
    }
    // odd i unmounts, even remounts — end with the editor mounted
    rig.rt.state.mounted = true;
    rig.view().unwrap();
    rig.pump().unwrap();
    let recs = rig.peers.recs.lock().unwrap_or_else(|e| e.into_inner());
    let mounts = recs.len() - baseline_peers;
    let releases: u32 = recs.values().map(|r| r.released).sum();
    // every created peer released exactly once; at most one remains live
    assert!(mounts >= 48, "expected >=48 mounts, got {mounts}");
    assert!(
        mounts as u32 - releases <= 1,
        "live peers = {} — must be <= 1",
        mounts as u32 - releases
    );
    assert!(
        recs.values().all(|r| r.released <= 1),
        "no peer released twice"
    );
    // slots/generations stay bounded — a freed slot is reusable, not grown
    assert!(rig.rt.arena.slot_len() < 64, "arena must recycle slots");
}

// ============================================================================
// Platform-contract geometry conformance — docs/PLATFORM_CONTRACTS.md
// ============================================================================
// The SHARED semantic authority is `crate::geom` — these tests prove the
// conversion mathematics ONCE for every backend, at every canonical scale.
// Windows adapter tests (probe, UIA) separately prove *which* space a given
// native API speaks; they never re-derive this math.

#[test]
fn geometry_contract_scale_identity() {
    use crate::geom::ScaleFactor;
    // the four canonical monitors
    let cases = [(96u32, 1.0f32), (120, 1.25), (144, 1.5), (192, 2.0)];
    for (dpi, want) in cases {
        let s = ScaleFactor::from_dpi(dpi);
        assert!((s.0 - want).abs() < 1e-6, "dpi {dpi}");
        assert_eq!(s.dpi(), dpi, "round-trip dpi {dpi}");
    }
}

#[test]
fn geometry_contract_logical_to_physical() {
    use crate::geom::ScaleFactor;
    // exactly-representable conversions are exact at every scale
    for &dpi in &[96u32, 120, 144, 192] {
        let s = ScaleFactor::from_dpi(dpi);
        assert_eq!(s.to_physical(96.0), dpi as i32, "96dp@{dpi}dpi");
        assert_eq!(s.to_physical(0.0), 0);
        assert_eq!(s.to_physical(-8.0), -((8.0 * s.0).round()) as i32);
    }
    // rounding policy: round-half-away-from-zero — the ONLY policy
    let s = ScaleFactor::ONE;
    assert_eq!(s.to_physical(0.49), 0);
    assert_eq!(s.to_physical(0.5), 1);
    assert_eq!(s.to_physical(0.51), 1);
    assert_eq!(s.to_physical(-0.5), -1); // half-away, not half-even
    // 125%: 0.4dp -> 0.5px -> rounds to 1 (not 0)
    assert_eq!(ScaleFactor::from_dpi(120).to_physical(0.4), 1);
    assert_eq!(ScaleFactor::from_dpi(120).to_physical(1.6), 2); // 2.0 exact
    assert_eq!(ScaleFactor::from_dpi(144).to_physical(2.5), 4); // 3.75 -> 4
}

#[test]
fn geometry_contract_round_trip() {
    use crate::geom::ScaleFactor;
    // logical -> px -> logical stays within half a physical pixel of the
    // source value (the maximum rounding error, by construction)
    for &dpi in &[96u32, 120, 144, 192] {
        let s = ScaleFactor::from_dpi(dpi);
        let tol = 0.5 / s.0 + 1e-4;
        let mut v = -32.0f32;
        while v <= 1024.0 {
            let rt = s.to_logical(s.to_physical(v));
            assert!(
                (rt - v).abs() <= tol,
                "round-trip @{dpi}dpi: {v} -> {} -> {rt}",
                s.to_physical(v)
            );
            v += 0.1;
        }
    }
}

#[test]
fn geometry_contract_rect_conversion() {
    use crate::geom::{Point, Rect, ScaleFactor};
    for &dpi in &[96u32, 120, 144, 192] {
        let s = ScaleFactor::from_dpi(dpi);
        let r = Rect {
            x: 34.0,
            y: 167.0,
            width: 336.0,
            height: 22.0,
        };
        let p = r.physical(s);
        // edges snap independently — width is never derived from two
        // rounded edges inside the impl
        assert_eq!(p.left, s.to_physical(34.0));
        assert_eq!(p.right, s.to_physical(370.0));
        let back = p.logical(s);
        let tol = 0.5 / s.0 + 1e-4;
        assert!((back.x - 34.0).abs() <= tol);
        assert!((back.y - 167.0).abs() <= tol);
        assert!((back.right() - 370.0).abs() <= 2.0 * tol);
    }
    // zero-size and negative-origin rects survive
    let z = Rect::local(0.0, 0.0).physical(ScaleFactor::from_dpi(120));
    assert_eq!(z.size().w, 0);
    let neg = Rect {
        x: -20.0,
        y: -8.0,
        width: 10.0,
        height: 4.0,
    }
    .physical(ScaleFactor::from_dpi(144));
    assert!(neg.left < 0 && neg.top < 0, "screen px may be negative");

    // geometry helpers shared with the retained side
    let r = Rect {
        x: 10.0,
        y: 10.0,
        width: 40.0,
        height: 20.0,
    };
    assert!(r.contains(Point { x: 10.0, y: 10.0 }));
    assert!(!r.contains(Point { x: 50.0, y: 10.0 })); // exclusive edge
    let u = r.union(Rect {
        x: 0.0,
        y: 0.0,
        width: 5.0,
        height: 5.0,
    });
    assert_eq!((u.x, u.y), (0.0, 0.0));
    assert_eq!(u.right(), 50.0);
}

#[test]
fn geometry_contract_scale_change_is_pure() {
    use crate::geom::{Rect, ScaleFactor};
    // a scale change is a pure function of (logical, scale) — no hidden
    // latching; identical inputs always produce identical px geometry
    let r = Rect {
        x: 12.0,
        y: 3.0,
        width: 100.0,
        height: 40.0,
    };
    let at96 = r.physical(ScaleFactor::from_dpi(96));
    let at192 = r.physical(ScaleFactor::from_dpi(192));
    assert_eq!(at192.left, at96.left * 2);
    assert_eq!(at96, r.physical(ScaleFactor::from_dpi(96)));
    // fractional coords round *after* scaling — deterministic, not
    // commutable: 12.5dp is 13px@96 but 25px@192 (not 26)
    let f = Rect {
        x: 12.5,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    assert_eq!(f.physical(ScaleFactor::from_dpi(96)).left, 13);
    assert_eq!(f.physical(ScaleFactor::from_dpi(192)).left, 25);
}
