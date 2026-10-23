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
    SetSelection(usize, usize),
    Released,
}

#[derive(Clone)]
struct PeerRec {
    id: u64,
    ops: Vec<PeerOp>,
    text: String,
    sel: Option<(usize, usize)>,
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
                sel: None,
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
    fn apply_selection(&mut self, sel: Option<(usize, usize)>) -> crate::UiResult {
        if let Some((s, e)) = sel {
            if self
                .log
                .recs
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get(&self.id)
                .and_then(|r| r.sel)
                == Some((s, e))
            {
                return Ok(());
            }
            self.push(PeerOp::SetSelection(s, e));
            self.log
                .recs
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get_mut(&self.id)
                .unwrap()
                .sel = Some((s, e));
        }
        Ok(())
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
                let r = t.as_mut().poll(&mut cx);
                if r.is_pending() {
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

// slices — include! keeps cargo test --list paths byte-identical

include!("t01.rs");
include!("t02.rs");
include!("t03.rs");
include!("t04.rs");
include!("t05.rs");
include!("t06.rs");
include!("t07.rs");
include!("probes.rs");
include!("t08.rs");
include!("t09.rs");
include!("t10.rs");

#[cfg(windows)]
mod native_contract_tests {
    include!("native_contracts_01.rs");
    include!("native_contracts_02.rs");
    include!("native_contracts_03.rs");
    include!("native_contracts_04.rs");
    include!("native_contracts_05.rs");
}

include!("tail_01.rs");
include!("t11.rs");
include!("t12.rs");
include!("t13.rs");
include!("t14.rs");
