use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

use crate::arena::Arena;
use crate::event::{EVENT_QUEUE_CAP, EventQueue};
use crate::geom::Visibility;
use crate::key::{ChildKey, ErasedKey, KeyId};
use crate::node::{
    ContainerProps, KIND_GROUP, Node, NodeData, NodeEvent, NodeId, PeerDecision, QueuedEvent,
    TextPeer,
};
use crate::sched::{SchedEvent, Scheduler};
use crate::tasks::{
    CancelWatch, Envelope, Executor, Job, MAILBOX_CAP, Mailbox, ScopePath, TaskRegistry,
    TaskStartError,
};
use crate::text::{
    TextConflict, TextEdit, TextRevision, TextValue, lease_id_of, validate_selection,
};
use crate::theme::{Appearance, ReducedMotion, Theme};
use crate::ui::{StagedNode, Tx, Ui, UiFrame};
use crate::{UiDiagnostic, UiError, UiResult};

/// Update context handed to `update` — task spawning, cancellation and
/// scoped namespaces. No view/arena access from here.
pub struct UpdateCtx<'cx, M> {
    registry: &'cx mut TaskRegistry,
    executor: &'cx Option<Arc<dyn Executor>>,
    mailbox: &'cx Arc<Mailbox>,
    scope: ScopePath,
    _m: std::marker::PhantomData<M>,
}

/// `spawn` is the only `M: Send` gate — worker bodies ship thread-side.
impl<'cx, M: Send + 'static> UpdateCtx<'cx, M> {
    /// Spawn a keyed task; `key` is (scope-path, key) namespaced — any prior
    /// registration is fenced synchronously before the new future starts.
    /// `M: Send` is required only here — the root `M` itself needn't be Send
    /// for apps that never spawn tasks.
    pub fn spawn<Fut>(
        &mut self,
        key: impl KeyId,
        job: impl FnOnce(Job<M>) -> Fut + Send + 'static,
    ) -> Result<(), TaskStartError>
    where
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let Some(executor) = self.executor else {
            return Err(TaskStartError::NoExecutor);
        };
        let executor = executor.clone();
        let (j, reg_id) = self.registry.spawn::<M>(&self.scope, key, self.mailbox)?;
        let (cancelled, completed, cancel_waiters) = self.registry.job_bits(reg_id);
        let fut = CancelWatch {
            inner: Box::pin(job(j)),
            cancelled,
            completed,
            cancel_waiters,
            mailbox: self.mailbox.clone(),
            watch_registered: false,
        };
        if executor.spawn(Box::pin(fut)).is_err() {
            // executor refused — fence the fresh registration so nothing leaks
            self.registry.cancel_reg(reg_id, self.mailbox);
            return Err(TaskStartError::Rejected);
        }
        Ok(())
    }
}

/// `cancel`/`scope` work for any `M` — they never touch the worker side.
impl<'cx, M: 'static> UpdateCtx<'cx, M> {
    /// Cancel the live task registered under `key` (scope-local).
    pub fn cancel(&mut self, key: impl KeyId) {
        self.registry.cancel(&self.scope, key, self.mailbox);
    }

    /// `cx.scope(key, map)` — child namespace: `spawn`/`cancel` keys are
    /// independent per scope path, and child messages lift through `map`.
    /// Scope adapters run UI-side only — no `Send`/`Sync` bound, so an
    /// `Rc` capture or a non-`Send` root `M` typechecks.
    pub fn scope<ChildMsg: 'static>(
        &mut self,
        key: impl KeyId,
        map: impl Fn(ChildMsg) -> M + 'static,
    ) -> UpdateCtx<'_, ChildMsg> {
        let lift: Rc<dyn Fn(Box<dyn Any>) -> Box<dyn Any>> =
            Rc::new(move |c| Box::new(map(*c.downcast::<ChildMsg>().unwrap())) as Box<dyn Any>);
        UpdateCtx {
            registry: self.registry,
            executor: self.executor,
            mailbox: self.mailbox,
            scope: self.scope.push(ErasedKey::new(key), lift),
            _m: std::marker::PhantomData,
        }
    }
}

/// The retained core: ONE arena + flat reusable staging buffers + bounded
/// task/mailbox delivery + a narrow chrome scheduler.
pub(crate) struct Runtime<S, M, U, V>
where
    M: 'static,
    U: Fn(&mut S, M, &mut UpdateCtx<M>),
    V: Fn(&S, &mut Ui<'_, '_, M>),
{
    pub(crate) state: S,
    update: U,
    view: V,
    pub(crate) arena: Arena,
    pub(crate) root: NodeId,
    pub(crate) events: EventQueue,
    pub(crate) mailbox: Arc<Mailbox>,
    pub(crate) registry: TaskRegistry,
    pub(crate) executor: Option<Arc<dyn Executor>>,
    pub(crate) sched: Scheduler,
    pub(crate) theme: Theme,
    /// actual OS appearance — independent of the selected theme
    appearance: Appearance,
    /// platform-supplied system-color lookup used ONLY when
    /// `appearance.forced_colors` — the classifier resolves roles to real
    /// system RGB so a theme/HC flip diffs honestly
    forced_resolver: Option<Box<dyn Fn(crate::style::SystemColor) -> [f32; 4]>>,
    /// platform OS reduced-motion probe — resolves an authored
    /// `ReducedMotion::System` at every re-resolution point; None = no OS
    /// signal (System behaves as NoPreference)
    motion_resolver: Option<Box<dyn Fn() -> bool>>,
    /// the AUTHORED reduced-motion policy — `theme.reduced_motion` keeps
    /// the authored value (System stays System) so an OS change can
    /// re-resolve it; `sched.reduced` holds the resolved bool
    authored_reduced: crate::theme::ReducedMotion,
    appearance_dirty: bool,
    /// how many times `view` has been evaluated
    pub(crate) view_count: u64,
    /// mounted-editor index: lease identity -> node (O(1) move detection)
    mounted_editors: HashMap<usize, NodeId>,
    /// flat reusable transaction buffer — allocated once per runtime
    scratch_nodes: Vec<StagedNode>,
    /// typed diagnostics collected across commits
    pub(crate) diagnostics: Vec<UiDiagnostic>,
    /// stale/foreign/out-of-order events dropped this session
    pub(crate) dropped_stale_events: u64,
    pub(crate) rejected_edits: u64,
    /// composing-snapshot selection drops
    pub(crate) suppressed_selections: u64,
    /// an enqueue overflowed during commit — surfaced as QueueOverflow
    last_queue_error: bool,
    peer_factory: Option<Box<dyn Fn(crate::node::PeerSpec) -> crate::UiResult<Box<dyn TextPeer>>>>,
    /// chrome scheduler events produced between polls (policy flips)
    chrome_backlog: Vec<SchedEvent>,
    _m: std::marker::PhantomData<fn() -> M>,
}

impl<S, M, U, V> Runtime<S, M, U, V>
where
    M: 'static,
    U: Fn(&mut S, M, &mut UpdateCtx<M>),
    V: Fn(&S, &mut Ui<'_, '_, M>),
{
    pub(crate) fn new(
        state: S,
        update: U,
        view: V,
        executor: Option<Arc<dyn Executor>>,
        peer_factory: Box<dyn Fn(crate::node::PeerSpec) -> crate::UiResult<Box<dyn TextPeer>>>,
        theme: Theme,
        appearance: Appearance,
        mailbox: Arc<Mailbox>,
    ) -> Self {
        let mut arena = Arena::new();
        // a virtual group root holds the view's children
        let root = arena.alloc(Node {
            key: ChildKey::Static {
                kind: KIND_GROUP,
                ordinal: 0,
            },
            parent: None,
            children: Vec::new(),
            data: NodeData::Container {
                kind: KIND_GROUP,
                props: ContainerProps::default(),
            },
            visibility: Visibility::Visible,
            layout: Default::default(),
            factories: Default::default(),
            adapters: Rc::new(Vec::new()),
            peer: None,
            dirty: Self::DIRTY_LAYOUT | Self::DIRTY_PAINT | Self::DIRTY_SEMANTICS,
        });
        // effective reduced-motion must be known before the first mount — an
        // initial Reduce custom never even briefly schedules frames
        let mut sched = Scheduler::default();
        sched.reduced = matches!(theme.reduced_motion, ReducedMotion::Reduce);
        Runtime {
            state,
            update,
            view,
            arena,
            root,
            events: EventQueue::new(),
            mailbox,
            registry: TaskRegistry::default(),
            executor,
            sched,
            theme: theme.clone(),
            appearance,
            forced_resolver: None,
            motion_resolver: None,
            authored_reduced: theme.reduced_motion,
            appearance_dirty: false,
            view_count: 0,
            mounted_editors: HashMap::new(),
            scratch_nodes: Vec::new(),
            diagnostics: Vec::new(),
            dropped_stale_events: 0,
            rejected_edits: 0,
            suppressed_selections: 0,
            last_queue_error: false,
            peer_factory: Some(peer_factory),
            chrome_backlog: Vec::new(),
            _m: std::marker::PhantomData,
        }
    }

    /// One turn of the run loop: the bounded native-event queue first, then
    /// at most MAILBOX_CAP worker envelopes — continuously producing workers
    /// can never starve native events or a Stop message. Returns `Ok(true)`
    /// when an update ran (which implies one review).
    pub(crate) fn pump(&mut self) -> UiResult<bool> {
        if self.last_queue_error {
            self.last_queue_error = false;
            return Err(UiError::QueueOverflow);
        }
        self.mailbox.wake_seen();
        let mut updated = false;

        // native events first — ONE bounded sweep per turn; a backend that
        // keeps refilling can't starve the mailbox or defer reap forever
        for _ in 0..EVENT_QUEUE_CAP {
            let Some(ev) = self.events.pop() else {
                break;
            };
            updated |= self.dispatch_event(ev)?;
        }

        // then at most one bounded mailbox sweep per turn
        for _ in 0..MAILBOX_CAP {
            let Some(env) = self.mailbox.pop() else {
                break;
            };
            updated |= self.deliver_envelope(env)?;
        }

        // completed registrations are reaped once their accepted envelopes
        // have drained — no tombstones accumulate
        self.registry.reap_completed();

        // leftovers (or work a commit just queued — e.g. ack events) ride the
        // posted continuation: next turn drains them, no polling needed
        if !self.events.is_empty() || self.mailbox.queue_len() > 0 {
            self.mailbox.poke_ui();
        }

        // first turn mounts the initial view — afterwards review only on
        // actual updates or OS-prop changes
        if self.view_count == 0 || updated || self.appearance_dirty {
            self.appearance_dirty = false;
            self.review()?;
        }
        Ok(updated)
    }

    /// Runtime-side OS appearance change — forces one review.
    pub(crate) fn set_appearance(&mut self, a: Appearance) {
        if self.appearance != a {
            self.appearance = a;
            self.appearance_dirty = true;
        }
    }

    /// Evaluate `view` into the flat staging buffer, validate the whole
    /// transaction, then commit. A failure leaves the arena and every native
    /// peer untouched.
    fn review(&mut self) -> UiResult {
        self.view_count += 1;
        let mut nodes = std::mem::take(&mut self.scratch_nodes);
        nodes.clear();
        let (root_children, theme, staged_nodes) = {
            let mut tx = Tx {
                action_depth: 0,
                nodes,
                frames: vec![UiFrame {
                    children: Vec::new(),
                    static_ordinals: 0,
                    key_buckets: HashMap::new(),
                    retained: Some(self.root),
                    retained_buckets: None,
                }],
                adapters: Rc::new(Vec::new()),
                theme: self.theme.clone(),
                appearance: self.appearance,
                diagnostics: Vec::new(),
                retained: &self.arena,
            };
            {
                let mut ui = Ui::<M> {
                    tx: &mut tx,
                    _m: std::marker::PhantomData,
                };
                (self.view)(&self.state, &mut ui);
            }
            // validate the whole staged transaction before ANY mutation
            if let Some(d) = tx.diagnostics.first() {
                let d: UiDiagnostic = *d;
                self.diagnostics.push(d);
                self.scratch_nodes = tx.nodes;
                return Err(UiError::InvalidUi(d));
            }
            if let Some(d) = validate_bindings(&tx) {
                self.diagnostics.push(d);
                self.scratch_nodes = tx.nodes;
                return Err(UiError::InvalidUi(d));
            }
            // style preflight — invalid values reject as InvalidStyle;
            // representable-but-unsupported (nonopaque peer backing or
            // foreground on native text) reject as Unsupported. Both land
            // BEFORE any native mutation.
            if let Some(e) = validate_styles(&tx, self.theme.dark) {
                if let UiError::InvalidUi(d) = e {
                    self.diagnostics.push(d);
                }
                self.scratch_nodes = tx.nodes;
                return Err(e);
            }
            // foreign-lease preflight: a staged editor whose lease is
            // occupied by a binding this runtime never mounted belongs to a
            // different runtime — reject BEFORE any peer mutation
            for s in &tx.nodes {
                if let NodeData::Editor { snapshot, .. } = &s.data
                    && snapshot.lease.get().is_some()
                    && !self
                        .mounted_editors
                        .contains_key(&lease_id_of(&snapshot.lease))
                {
                    let d = UiDiagnostic::DuplicateTextBinding;
                    self.diagnostics.push(d);
                    self.scratch_nodes = tx.nodes;
                    return Err(UiError::InvalidUi(d));
                }
            }
            (tx.frames[0].children.clone(), tx.theme.clone(), tx.nodes)
        };
        nodes = staged_nodes;
        // commit — peer mutations only happen here
        let commit = self.commit_children(self.root, &mut nodes, root_children);
        if commit.is_err() {
            // structural/native failure tears the runtime down orderly:
            // every live peer released once, every lease cleared
            self.shutdown();
        }
        // the flat buffer returns to scratch either way (capacity retained)
        nodes.clear();
        self.scratch_nodes = nodes;
        commit?;
        // staged theme propagates; reduced-motion flips settle running work.
        // The AUTHORED policy is stored (System stays System — re-resolvable
        // on OS change); `sched.reduced` gets the resolved bool.
        if theme != self.theme {
            self.authored_reduced = theme.reduced_motion;
            self.theme = theme;
            self.apply_reduced();
        }
        Ok(())
    }

    /// Commit staged children under `parent`: all matching first, unclaimed
    /// removals BEFORE any mount (a moved `TextValue` unmounts its old peer
    /// before the new one mounts).
    fn commit_children(
        &mut self,
        parent: NodeId,
        nodes: &mut Vec<StagedNode>,
        staged: Vec<u32>,
    ) -> UiResult {
        let old_children: Vec<u32> = self
            .arena
            .get(parent)
            .map(|n| n.children.clone())
            .unwrap_or_default();
        // bucket retained children by key hash — O(1) matching, no scans
        let mut buckets: HashMap<u64, Vec<u32>> = HashMap::new();
        for &slot in &old_children {
            let g = self.arena.generation_of(slot);
            if let Some(n) = self.arena.get(NodeId {
                slot,
                generation: g,
            }) {
                buckets.entry(n.key.hash()).or_default().push(slot);
            }
        }
        let mut matched: Vec<Option<NodeId>> = Vec::with_capacity(staged.len());
        let mut claimed: HashSet<u32> = HashSet::with_capacity(staged.len());
        for &si in &staged {
            let mut hit = None;
            if let Some(s) = nodes.get(si as usize) {
                let h = s.key.hash();
                for &slot in buckets.get(&h).into_iter().flatten() {
                    if claimed.contains(&slot) {
                        continue;
                    }
                    let g = self.arena.generation_of(slot);
                    let Some(n) = self.arena.get(NodeId {
                        slot,
                        generation: g,
                    }) else {
                        continue;
                    };
                    if !n.key.key_eq(&s.key) || n.kind_tag() != s.kind_tag() {
                        continue;
                    }
                    // same-key editor bound to a DIFFERENT TextValue => remount
                    // (new generation + fresh binding), never a silent rebind
                    let remount = matches!(
                        (&n.data, &s.data),
                        (
                            NodeData::Editor { snapshot: a, .. },
                            NodeData::Editor { snapshot: b, .. }
                        ) if lease_id_of(&a.lease) != lease_id_of(&b.lease)
                    );
                    if !remount {
                        hit = Some(NodeId {
                            slot,
                            generation: g,
                        });
                        claimed.insert(slot);
                    }
                    break;
                }
            }
            matched.push(hit);
        }
        // removals first — peers release before any mount below mutates them
        for &slot in &old_children {
            if !claimed.contains(&slot) {
                let g = self.arena.generation_of(slot);
                self.remove_node(NodeId {
                    slot,
                    generation: g,
                });
            }
        }
        // mounts/updates in staged order
        let mut new_children = Vec::with_capacity(staged.len());
        for (si, m) in staged.iter().zip(matched) {
            let staged_node =
                std::mem::replace(&mut nodes[*si as usize], StagedNode::placeholder());
            let id = match m {
                Some(id) if self.arena.is_live(id) => {
                    self.update_node(id, staged_node, nodes)?;
                    id
                }
                _ => self.create_node(Some(parent.slot), staged_node, nodes)?,
            };
            new_children.push(id.slot);
        }
        // reordered edges dirty the parent's layout and every moved child
        // subtree (position feeds layout) — same set in a new order still
        // counts; native text peers are untouched either way
        let order_changed = new_children != old_children;
        if order_changed {
            let new_set: HashSet<u32> = new_children.iter().copied().collect();
            let old_set: HashSet<u32> = old_children.iter().copied().collect();
            let kept_move = new_set == old_set;
            for &slot in &new_children {
                if kept_move {
                    let g = self.arena.generation_of(slot);
                    if let Some(n) = self.arena.get_mut(NodeId {
                        slot,
                        generation: g,
                    }) {
                        n.dirty |= Self::DIRTY_LAYOUT;
                    }
                }
            }
            if let Some(p) = self.arena.get_mut(parent) {
                p.dirty |= Self::DIRTY_LAYOUT;
            }
        }
        if let Some(p) = self.arena.get_mut(parent) {
            p.children = new_children;
        }
        Ok(())
    }

    /// Backend-facing dirty drain — returns (node, classification) for every
    /// node flagged since the last drain, then clears the flags.
    pub(crate) fn drain_dirty(&mut self) -> Vec<(NodeId, u8)> {
        let mut out = Vec::new();
        for slot in 0..self.arena.slot_count() as u32 {
            let g = self.arena.generation_of(slot);
            let id = NodeId {
                slot,
                generation: g,
            };
            if let Some(n) = self.arena.get_mut(id)
                && n.dirty != 0
            {
                out.push((id, n.dirty));
                n.dirty = 0;
            }
        }
        out
    }

    fn create_node(
        &mut self,
        parent: Option<u32>,
        staged: StagedNode,
        nodes: &mut Vec<StagedNode>,
    ) -> UiResult<NodeId> {
        let StagedNode {
            key,
            data,
            visibility,
            layout,
            factories,
            adapters,
            children,
        } = staged;

        // a lease already mounted elsewhere means the value MOVED — retire
        // the old peer (exactly once) before mounting the new one
        if let NodeData::Editor { snapshot, .. } = &data {
            let lid = lease_id_of(&snapshot.lease);
            if let Some(old) = self.mounted_editors.get(&lid).copied()
                && self.arena.is_live(old)
            {
                self.remove_node(old);
            }
            self.mounted_editors.remove(&lid);
            // a lease occupied but not owned by this runtime is a foreign
            // binding — reject BEFORE any peer creation
            if snapshot.lease.get().is_some() {
                return Err(UiError::InvalidUi(UiDiagnostic::DuplicateTextBinding));
            }
        }

        let multiline = matches!(
            &data,
            NodeData::Editor {
                multiline: true,
                ..
            }
        );
        let mut peer: Option<Box<dyn TextPeer>> = None;
        if let NodeData::Editor {
            patch,
            read_only,
            disabled,
            ..
        } = &data
            && let Some(pf) = &self.peer_factory
        {
            // capability boundary: the peer receives ONLY the resolved
            // editable foreground — authored value or the Foreground role
            let foreground = patch.foreground.unwrap_or(crate::style::Color::Role(
                crate::theme::ColorRole::Foreground,
            ));
            peer = Some(pf(crate::node::PeerSpec {
                multiline,
                read_only: *read_only,
                disabled: *disabled,
                foreground,
            })?);
        }

        let id = self.arena.alloc(Node {
            key,
            parent,
            children: Vec::new(),
            data,
            visibility,
            layout,
            factories,
            adapters,
            peer,
            dirty: Self::DIRTY_LAYOUT | Self::DIRTY_PAINT | Self::DIRTY_SEMANTICS,
        });

        // mount-time editor setup: fresh binding, initialize text once —
        // the ONLY place initial text reaches the peer
        {
            let n = self.arena.get_mut(id).unwrap();
            if let Some(p) = n.peer.as_mut() {
                p.attach(id);
            }
            if let NodeData::Editor { snapshot, sync, .. } = &mut n.data {
                let token = TextValue::acquire_lease(&snapshot.lease)
                    .ok_or(UiError::InvalidUi(UiDiagnostic::DuplicateTextBinding))?;
                match n.peer.as_mut() {
                    Some(p) => sync.mount(p.as_mut(), snapshot, token)?,
                    None => {
                        sync.binding = Some(token);
                        sync.peer_revision = snapshot.revision;
                        sync.applied_revision = snapshot.revision;
                        sync.applied_text = snapshot.committed.to_string();
                        sync.mounted = true;
                    }
                }
                self.mounted_editors
                    .insert(lease_id_of(&snapshot.lease), id);
            }
            if let NodeData::Custom { frame_events, .. } = &n.data {
                let now = std::time::Instant::now();
                self.sched.frame_demand(id, *frame_events, now);
                self.sched
                    .set_suspended(id, visibility != Visibility::Visible, now);
            }
        }

        for &ci in &children {
            let staged_child =
                std::mem::replace(&mut nodes[ci as usize], StagedNode::placeholder());
            let cid = self.create_node(Some(id.slot), staged_child, nodes)?;
            self.arena.get_mut(id).unwrap().children.push(cid.slot);
        }
        Ok(id)
    }

    /// In-place update: props/factories/layout swapped; editor peer + sync
    /// carried over, then the staged snapshot is committed against the peer.
    /// Structural removal flag — the backend prunes its cache entries.
    pub(crate) const DIRTY_REMOVED: u8 = 0b0000_0100;
    /// Layout-affecting change (props/layout/visibility/container props).
    pub(crate) const DIRTY_LAYOUT: u8 = 0b0000_0001;
    /// Paint-only change (text content, colors).
    pub(crate) const DIRTY_PAINT: u8 = 0b0000_0010;
    /// Semantics/accessibility-affecting change (labels, disabled, role).
    pub(crate) const DIRTY_SEMANTICS: u8 = 0b0000_1000;

    fn update_node(
        &mut self,
        id: NodeId,
        staged: StagedNode,
        nodes: &mut Vec<StagedNode>,
    ) -> UiResult {
        let StagedNode {
            data,
            visibility,
            layout,
            factories,
            adapters,
            children,
            ..
        } = staged;

        let now = std::time::Instant::now();
        let old_vis = self.arena.get(id).map(|n| n.visibility).unwrap_or_default();
        // swap data — for Editor->Editor keep the retained sync (peer mirror)
        let mut emit_conflict_for: Option<crate::text::TextSnapshot> = None;
        let commit_err: Option<UiError> = {
            let n = self.arena.get_mut(id).unwrap();
            let old_data = std::mem::replace(&mut n.data, data);
            // dirty classification compares RESOLVED styles — two authored
            // descriptions producing identical concrete output are a no-op
            let dark = self.theme.dark;
            let forced = self
                .appearance
                .forced_colors
                .then(|| self.forced_resolver.as_deref())
                .flatten();
            n.dirty = crate::node::dirty_diff(&old_data, &n.data, dark, forced);
            if n.layout != layout || old_vis != visibility {
                n.dirty |= Self::DIRTY_LAYOUT;
            }
            let mut commit_err = None;
            match (&mut n.data, old_data) {
                (
                    NodeData::Editor {
                        sync,
                        patch,
                        read_only,
                        disabled,
                        ..
                    },
                    NodeData::Editor {
                        sync: retained_sync,
                        patch: old_patch,
                        read_only: old_ro,
                        disabled: old_dis,
                        ..
                    },
                ) => {
                    *sync = retained_sync;
                    let patch_changed = *patch != old_patch;
                    if let Some(p) = n.peer.as_mut() {
                        // effective editability = read_only || disabled —
                        // the native service enforces it (EM_SETREADONLY)
                        if *read_only != old_ro || *disabled != old_dis {
                            p.set_read_only(*read_only || *disabled);
                        }
                        if patch_changed {
                            p.apply_foreground(patch.foreground.unwrap_or(
                                crate::style::Color::Role(crate::theme::ColorRole::Foreground),
                            ));
                        }
                        if let NodeData::Editor { snapshot, sync, .. } = &mut n.data {
                            match sync.commit(p.as_mut(), snapshot) {
                                Ok(PeerDecision::Conflict) => {
                                    emit_conflict_for = Some(snapshot.clone())
                                }
                                Err(e) => commit_err = Some(e),
                                _ => {}
                            }
                        }
                    }
                }
                _ => {}
            }
            n.visibility = visibility;
            n.layout = layout;
            n.factories = factories;
            n.adapters = adapters;
            commit_err
        };
        if let Some(snap) = emit_conflict_for {
            self.emit_conflict(id, &snap);
        }
        if let Some(e) = commit_err {
            return Err(e);
        }

        // visibility transitions suspend/resume demand (delta resets)
        if old_vis != visibility {
            self.sched
                .set_suspended(id, visibility != Visibility::Visible, now);
        }

        // custom frame demand tracks the CURRENT view's requested flag —
        // reduced motion gates the deadline, not the demand entry, so a
        // policy flip can't lose resume state or un-hide a suspended node
        let frame_req = {
            let n = self.arena.get(id).unwrap();
            matches!(
                &n.data,
                NodeData::Custom {
                    frame_events: true,
                    ..
                }
            )
        };
        self.sched.frame_demand(id, frame_req, now);

        self.commit_children(id, nodes, children)
    }

    /// Detach from the parent's edge list -> invalidate -> purge events ->
    /// cancel demand -> release peer -> clear lease -> recurse -> free.
    /// One pass, exactly once — and no stale slot edge ever survives.
    fn remove_node(&mut self, id: NodeId) {
        if !self.arena.is_live(id) {
            return;
        }
        // detach this slot from the parent's edge list FIRST: the slot may
        // be reused before the old parent recomits — a stale edge could then
        // claim/remove the new node
        let parent_slot = self.arena.get(id).and_then(|n| n.parent);
        if let Some(ps) = parent_slot {
            let pos = self
                .arena
                .slot_mut(ps)
                .and_then(|p| p.children.iter().position(|&x| x == id.slot));
            if let Some(i) = pos
                && let Some(p) = self.arena.slot_mut(ps)
            {
                p.children.remove(i);
            }
        }
        let (children, lease) = self
            .arena
            .get(id)
            .map(|n| (n.children.clone(), n.lease().map(|l| lease_id_of(&l))))
            .unwrap_or_default();
        if let Some(l) = lease {
            self.mounted_editors.remove(&l);
        }
        self.arena.invalidate(id);
        self.dropped_stale_events += self.events.purge_node(id.slot);
        self.sched.cancel_node(id);
        // teardown on the dead node via raw slot access
        if let Some(n) = self.arena.slot_mut(id.slot) {
            if let NodeData::Editor { snapshot, sync, .. } = &n.data
                && let Some(t) = sync.binding
            {
                TextValue::release_lease(&snapshot.lease, t);
            }
            if let Some(mut peer) = n.peer.take() {
                peer.release();
            }
        }
        for &slot in &children {
            let g = self.arena.generation_of(slot);
            if self.arena.is_live(NodeId {
                slot,
                generation: g,
            }) {
                self.remove_node(NodeId {
                    slot,
                    generation: g,
                });
            }
        }
        self.arena.free(id);
    }

    // ----- native event dispatch (post-commit; generation-fenced) ----------

    /// Route one queued native event. Stale generations drop silently;
    /// edit events are validated against the peer mirror BEFORE dispatch —
    /// foreign/stale edits never reach a callback or touch app state.
    /// Returns `true` when the app `update` actually ran.
    fn dispatch_event(&mut self, ev: QueuedEvent) -> UiResult<bool> {
        let QueuedEvent { node, payload } = ev;
        if !self.arena.is_live(node) {
            self.dropped_stale_events += 1;
            return Ok(false);
        }
        // editors: validate through the sync mirror first
        let mut superseded: Option<TextRevision> = None;
        if let NodeEvent::Edit(edit) = &payload {
            let n = self.arena.get_mut(node).unwrap();
            let ok = match &mut n.data {
                NodeData::Editor { sync, .. } => match sync.accept_native(edit) {
                    Some(sup) => {
                        superseded = sup;
                        true
                    }
                    None => false,
                },
                _ => false,
            };
            if !ok {
                self.rejected_edits += 1;
                return Ok(false); // foreign/stale: no dispatch, mirror untouched
            }
        }
        if let NodeEvent::Selection(sel) = &payload {
            let n = self.arena.get(node).unwrap();
            match &n.data {
                NodeData::Editor { sync, .. } => {
                    if sync.composing {
                        self.suppressed_selections += 1;
                        return Ok(false);
                    }
                    if !validate_selection(*sel, sync.applied_revision, &sync.applied_text) {
                        self.dropped_stale_events += 1;
                        return Ok(false);
                    }
                }
                _ => {
                    self.dropped_stale_events += 1;
                    return Ok(false);
                }
            }
        }

        // Activation events are gated by disabled/hidden. Committed native
        // edits/conflicts (peer-mirror validated above), Blur on a
        // just-hidden node, and open-change/scroll notifications still route
        // while the node lives — suppressing them would lose acknowledged
        // state or strand focus.
        let gated = matches!(
            payload,
            NodeEvent::Press
                | NodeEvent::Submit
                | NodeEvent::Key(_)
                | NodeEvent::Pointer(_, _)
                | NodeEvent::Frame(_)
                | NodeEvent::Focus(true)
        );
        if gated && !self.arena.get(node).is_some_and(|n| n.interactive()) {
            self.dropped_stale_events += 1;
            return Ok(false);
        }
        // resolve the NEWEST committed factory — the retained snapshot wins;
        // invoked under the arena borrow (no factory clones), then lifted
        let lifted: Option<Box<dyn Any>> = {
            let n = self.arena.get(node).unwrap();
            let msg = n.factories.dispatch(payload);
            // lift child messages through enclosing scope adapters
            msg.map(|mut m| {
                for a in n.adapters.iter() {
                    m = a(m);
                }
                m
            })
        };
        // a native commit that superseded an in-flight programmatic request
        // emits exactly one conflict (tombstoned in the sync)
        if let Some(req) = superseded {
            if let Some(n) = self.arena.get_mut(node)
                && let NodeData::Editor { sync, .. } = &mut n.data
            {
                sync.last_rejected = Some(req);
            }
            self.emit_conflict_for_request(node, req);
        }
        let Some(msg) = lifted else {
            return Ok(false);
        };
        let m = *msg.downcast::<M>().unwrap();
        let mut cx = UpdateCtx {
            registry: &mut self.registry,
            executor: &self.executor,
            mailbox: &self.mailbox,
            scope: ScopePath::root(),
            _m: std::marker::PhantomData,
        };
        (self.update)(&mut self.state, m, &mut cx);
        Ok(true)
    }

    fn deliver_envelope(&mut self, env: Envelope) -> UiResult<bool> {
        let Some(dq) = self.registry.resolve(env) else {
            return Ok(false); // stale/fenced envelope dropped
        };
        let m = *dq.msg.downcast::<M>().unwrap();
        let mut cx = UpdateCtx {
            registry: &mut self.registry,
            executor: &self.executor,
            mailbox: &self.mailbox,
            scope: ScopePath::root(),
            _m: std::marker::PhantomData,
        };
        (self.update)(&mut self.state, m, &mut cx);
        Ok(true)
    }

    // ----- text conflict routing ----------------------------------------------

    /// Conflict for a superseded in-flight request — the rejected request is
    /// identified by its requested revision (which the app's pending may or
    /// may not still hold; keep_native only matches the live pending).
    fn emit_conflict_for_request(&mut self, id: NodeId, rejected: TextRevision) {
        let conflict = {
            let n = self.arena.get(id).unwrap();
            match &n.data {
                NodeData::Editor { snapshot, sync, .. } => {
                    let Some(binding) = sync.binding else {
                        return;
                    };
                    TextConflict {
                        rejected_revision: rejected,
                        rejected_text: snapshot
                            .pending
                            .as_ref()
                            .filter(|p| p.requested == rejected)
                            .map(|p| p.text.to_string())
                            .unwrap_or_default(),
                        committed_revision: sync.applied_revision,
                        committed_text: sync.applied_text.clone(),
                        binding,
                    }
                }
                _ => return,
            }
        };
        let has_handler = self
            .arena
            .get(id)
            .and_then(|n| n.factories.on_conflict.as_ref())
            .is_some();
        if has_handler {
            if self
                .events
                .push(QueuedEvent {
                    node: id,
                    payload: NodeEvent::Conflict(conflict),
                })
                .is_err()
            {
                self.last_queue_error = true;
            }
        } else {
            self.diagnostics.push(UiDiagnostic::UnhandledTextConflict);
        }
    }

    fn emit_conflict(&mut self, id: NodeId, snapshot: &crate::text::TextSnapshot) {
        let Some(pending) = snapshot.pending.clone() else {
            return;
        };
        let conflict = {
            let n = self.arena.get(id).unwrap();
            match &n.data {
                NodeData::Editor { sync, .. } => {
                    let Some(binding) = sync.binding else {
                        return;
                    };
                    TextConflict {
                        rejected_revision: pending.requested,
                        rejected_text: pending.text.to_string(),
                        committed_revision: sync.applied_revision,
                        committed_text: sync.applied_text.clone(),
                        binding,
                    }
                }
                _ => return,
            }
        };
        let has_handler = self
            .arena
            .get(id)
            .and_then(|n| n.factories.on_conflict.as_ref())
            .is_some();
        if has_handler {
            if self
                .events
                .push(QueuedEvent {
                    node: id,
                    payload: NodeEvent::Conflict(conflict),
                })
                .is_err()
            {
                self.last_queue_error = true;
            }
        } else {
            // mandatory diagnostic, exactly once — the sync tombstone keeps
            // the same rejected proposal from emitting again
            self.diagnostics.push(UiDiagnostic::UnhandledTextConflict);
        }
    }

    // ----- peer-facing entry points (backend calls these) ------------------------

    /// A native peer committed an edit (typing, IME commit, paste).
    pub(crate) fn edit_event(&mut self, node: NodeId, edit: TextEdit) -> UiResult {
        self.events
            .push(QueuedEvent {
                node,
                payload: NodeEvent::Edit(edit),
            })
            .map_err(|_| UiError::QueueOverflow)
    }

    /// Native selection notification.
    pub(crate) fn selection_event(&mut self, node: NodeId, sel: crate::TextSelection) -> UiResult {
        self.events
            .push(QueuedEvent {
                node,
                payload: NodeEvent::Selection(sel),
            })
            .map_err(|_| UiError::QueueOverflow)
    }

    /// IME composition lifecycle — queued proposals resolve when it ends.
    pub(crate) fn composition_start(&mut self, node: NodeId) {
        if let Some(n) = self.arena.get_mut(node)
            && let NodeData::Editor { sync, .. } = &mut n.data
        {
            sync.composing = true;
        }
    }

    /// Composition ended: a queued proposal applies if its base survived,
    /// otherwise ONE conflict is emitted (now — never while marked input is
    /// active). All of the sync bookkeeping lives in
    /// IME composition active on this node — the keyboard/submit layer
    /// must not interpret keys while marked input is owned by the IME.
    pub(crate) fn composition_active(&self, node: NodeId) -> bool {
        self.arena
            .get(node)
            .is_some_and(|n| matches!(&n.data, NodeData::Editor { sync, .. } if sync.composing))
    }

    /// `TextPeerSync::composition_end` — ONE implementation.
    pub(crate) fn composition_end(&mut self, node: NodeId) -> UiResult {
        enum After {
            Apply,
            Conflict,
            Nothing,
        }
        let verdict = {
            let Some(n) = self.arena.get_mut(node) else {
                return Ok(());
            };
            match &mut n.data {
                NodeData::Editor { snapshot, sync, .. } => match n.peer.as_mut() {
                    Some(p) => match sync.composition_end(p.as_mut(), snapshot) {
                        Ok(crate::node::PeerDecision::Applied(_)) => After::Apply,
                        Ok(crate::node::PeerDecision::Conflict) => After::Conflict,
                        Ok(crate::node::PeerDecision::Nothing) => After::Nothing,
                        Err(e) => return Err(e),
                    },
                    None => {
                        sync.composing = false;
                        sync.queued_while_composing = false;
                        After::Nothing
                    }
                },
                _ => return Ok(()),
            }
        };
        match verdict {
            After::Apply | After::Nothing => Ok(()),
            After::Conflict => {
                if let Some(snap) = self.arena.get(node).and_then(|n| match &n.data {
                    NodeData::Editor { snapshot, .. } => Some(snapshot.clone()),
                    _ => None,
                }) {
                    self.emit_conflict(node, &snap);
                }
                Ok(())
            }
        }
    }

    /// Scheduler pump — frame demand becomes `on_frame` events; transition /
    /// tooltip chrome events are RETURNED for the backend to render.
    pub(crate) fn pump_sched(&mut self, now: std::time::Instant) -> UiResult<Vec<SchedEvent>> {
        let mut chrome = std::mem::take(&mut self.chrome_backlog);
        for ev in self.sched.poll(now) {
            match ev {
                SchedEvent::Frame { node, time } => {
                    if self.arena.is_live(node)
                        && self
                            .events
                            .push(QueuedEvent {
                                node,
                                payload: NodeEvent::Frame(time),
                            })
                            .is_err()
                    {
                        return Err(UiError::QueueOverflow);
                    }
                }
                chrome_ev => chrome.push(chrome_ev),
            }
        }
        Ok(chrome)
    }

    /// The scheduler's earliest deadline — the run loop's single timer.
    pub(crate) fn next_deadline(&self) -> Option<std::time::Instant> {
        self.sched.next_deadline()
    }

    /// Tooltip arm/disarm seams for the backend's pointer routing.
    pub(crate) fn sched_arm_tooltip(&mut self, node: NodeId) {
        self.sched.arm_tooltip(node, std::time::Instant::now());
    }
    pub(crate) fn sched_unarm_tooltip(&mut self, node: NodeId) {
        self.sched.cancel_tooltip(node);
    }
    pub(crate) fn node_has_tooltip(&self, node: NodeId) -> bool {
        self.arena.get(node).is_some_and(|n| {
            matches!(
                &n.data,
                NodeData::Button {
                    tooltip: Some(_),
                    ..
                }
            )
        })
    }
    /// Install the platform's OS reduced-motion probe — authored
    /// `ReducedMotion::System` resolves through this on every staged
    /// theme and every WM_SETTINGCHANGE (`os_change` -> `refresh_reduced`).
    pub(crate) fn set_motion_resolver(&mut self, f: Box<dyn Fn() -> bool>) {
        self.motion_resolver = Some(f);
    }

    /// Resolve the authored reduced-motion policy through the platform
    /// probe and push the result into the scheduler. Idempotent.
    pub(crate) fn refresh_reduced(&mut self) {
        self.apply_reduced();
    }
    fn apply_reduced(&mut self) {
        let reduced = match self.authored_reduced {
            crate::theme::ReducedMotion::Reduce => true,
            crate::theme::ReducedMotion::NoPreference => false,
            crate::theme::ReducedMotion::System => {
                self.motion_resolver.as_ref().map(|f| f()).unwrap_or(false)
            }
        };
        let evs = self.sched.set_reduced(reduced, std::time::Instant::now());
        self.chrome_backlog.extend(evs);
    }

    /// Install the platform's forced-colors resolver (win32 supplies
    /// GetSysColor; a portable runtime leaves it unset).
    pub(crate) fn set_forced_resolver(
        &mut self,
        f: Box<dyn Fn(crate::style::SystemColor) -> [f32; 4]>,
    ) {
        self.forced_resolver = Some(f);
    }
    pub(crate) fn appearance(&self) -> crate::theme::Appearance {
        self.appearance
    }

    /// The forced-colors resolver, live only while forced colors are on —
    /// `None` means "authored palette stands".
    pub(crate) fn forced_resolver(&self) -> Option<&dyn Fn(crate::style::SystemColor) -> [f32; 4]> {
        self.appearance
            .forced_colors
            .then(|| self.forced_resolver.as_deref())
            .flatten()
    }

    /// Window close: shut the mailbox (producers wake `Closed`), fence every
    /// registration, purge events, cancel demand, release every live peer
    /// exactly once, clear every lease.
    pub(crate) fn shutdown(&mut self) {
        self.mailbox.close();
        self.registry.fence_all(&self.mailbox);
        for slot in 0..self.arena.slot_count() as u32 {
            let g = self.arena.generation_of(slot);
            let id = NodeId {
                slot,
                generation: g,
            };
            if !self.arena.is_live(id) {
                continue;
            }
            if let Some(n) = self.arena.slot_mut(slot) {
                if let NodeData::Editor { snapshot, sync, .. } = &n.data
                    && let Some(t) = sync.binding
                {
                    TextValue::release_lease(&snapshot.lease, t);
                }
                if let Some(mut peer) = n.peer.take() {
                    peer.release();
                }
            }
        }
        self.mounted_editors.clear();
        self.events.drain();
        self.sched.reset();
    }

    // ----- test surface ---------------------------------------------------

    #[cfg(test)]
    pub(crate) fn review_for_test(&mut self) -> UiResult {
        self.review()
    }

    #[cfg(test)]
    pub(crate) fn ctx_for_test(&mut self) -> UpdateCtx<'_, M>
    where
        M: Send,
    {
        UpdateCtx {
            registry: &mut self.registry,
            executor: &self.executor,
            mailbox: &self.mailbox,
            scope: ScopePath::root(),
            _m: std::marker::PhantomData,
        }
    }

    #[cfg(test)]
    pub(crate) fn test_push_event(&mut self, ev: QueuedEvent) {
        let _ = self.events.push(ev);
    }

    #[cfg(test)]
    pub(crate) fn test_peer_id(&self, id: NodeId) -> Option<u64> {
        self.arena.get(id)?.peer.as_ref().map(|p| p.peer_id())
    }

    #[cfg(test)]
    pub(crate) fn push_msg(&mut self, m: M)
    where
        M: Send,
    {
        // deliver a message synchronously without pumping (ordering control)
        let mut cx = UpdateCtx {
            registry: &mut self.registry,
            executor: &self.executor,
            mailbox: &self.mailbox,
            scope: ScopePath::root(),
            _m: std::marker::PhantomData,
        };
        (self.update)(&mut self.state, m, &mut cx);
    }

    pub(crate) fn root_children(&self) -> Vec<NodeId> {
        self.arena
            .get(self.root)
            .map(|n| {
                n.children
                    .iter()
                    .map(|&s| NodeId {
                        slot: s,
                        generation: self.arena.generation_of(s),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Whole-transaction validation — every `TextValue` lease must bind at most
/// one staged editor; O(n) via a hash set, runs before any peer mutation.
/// Guaranteed teardown — every error path and normal drop runs shutdown:
/// peers released once, leases cleared, queues fenced.
impl<S, M, U, V> Drop for Runtime<S, M, U, V>
where
    U: Fn(&mut S, M, &mut UpdateCtx<'_, M>),
    V: Fn(&S, &mut Ui<'_, '_, M>),
{
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn validate_bindings(tx: &Tx) -> Option<UiDiagnostic> {
    let mut seen: HashSet<usize> = HashSet::new();
    for n in &tx.nodes {
        if let Some(lid) = n.lease_id()
            && !seen.insert(lid)
        {
            return Some(UiDiagnostic::DuplicateTextBinding);
        }
    }
    None
}

/// Style preflight — every staged style is validated BEFORE any native
/// mutation: finite numbers, nonnegative geometry, positive text sizes,
/// valid radii/shadow -> `InvalidUi(InvalidStyle)`; a nonopaque editor
/// backing/foreground (impossible platform capability) -> `Unsupported`.
fn validate_styles(tx: &Tx, dark: bool) -> Option<UiError> {
    for n in &tx.nodes {
        let ok = match &n.data {
            NodeData::Container { kind, props } => {
                let patch_ok = crate::style::box_patch_ok(&props.patch);
                let full_ok = props.full.is_none_or(|f| crate::style::box_style_ok(&f));
                let resolved_ok = props
                    .resolved_box(*kind)
                    .is_none_or(|b| crate::style::box_style_ok(&b));
                patch_ok && full_ok && resolved_ok
            }
            NodeData::Label { patch, .. } => crate::style::text_patch_ok(patch),
            NodeData::Button {
                style,
                variant,
                size,
                ..
            } => {
                crate::style::button_patch_ok(style)
                    && crate::style::visual_style_ok(&crate::style::resolve_button(
                        *variant,
                        *size,
                        style,
                        crate::style::StyleState::Normal,
                        false,
                        false,
                    ))
            }
            NodeData::Editor { patch, .. } => {
                if !crate::style::text_input_patch_ok(patch) {
                    return Some(UiError::InvalidUi(UiDiagnostic::InvalidStyle));
                }
                // opaque rule — checked against the RESOLVED colors
                let chrome = crate::style::resolve_text_input_chrome(patch);
                let fg = patch.foreground.unwrap_or(crate::style::Color::Role(
                    crate::theme::ColorRole::Foreground,
                ));
                if !crate::style::text_input_opaque(&chrome, fg, dark) {
                    return Some(UiError::Unsupported(
                        "native text requires opaque backing and foreground".into(),
                    ));
                }
                true
            }
            NodeData::Action { style, .. } => crate::style::action_style_ok(style),
            _ => true,
        };
        if !ok {
            return Some(UiError::InvalidUi(UiDiagnostic::InvalidStyle));
        }
    }
    None
}
