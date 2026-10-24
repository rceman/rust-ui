use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

use crate::arena::Arena;
use crate::event::{EVENT_QUEUE_CAP, EventQueue};
use crate::geom::Visibility;
use crate::key::ChildKey;
use crate::node::{ContainerProps, KIND_GROUP, Node, NodeData, NodeId, TextPeer};
use crate::sched::Scheduler;
use crate::tasks::{Executor, MAILBOX_CAP, Mailbox, TaskRegistry};
use crate::text::lease_id_of;
use crate::theme::{Appearance, ReducedMotion, Theme};
use crate::ui::{StagedNode, Tx, Ui, UiFrame};
use crate::{UiDiagnostic, UiError, UiResult};

/// Update context handed to `update` — task spawning, cancellation and
use super::*;

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
            automation_id: None,
            part_key: None,
            role: None,
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
            presented: Default::default(),
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
        // same signal authority: next turn drains them, no polling needed.
        // The UI turn owns this result — a terminal wake failure propagates
        // typed instead of masking stranded work behind the next wait.
        if !self.events.is_empty() || self.mailbox.queue_len() > 0 {
            if !self.mailbox.poke_ui() {
                return Err(UiError::Platform(
                    "mailbox continuation signal failed".into(),
                ));
            }
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
    pub(crate) fn review(&mut self) -> UiResult {
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
            // Automation IDs are invariants in EVERY build (not only
            // devtools): non-empty, <= 256 bytes, no control characters,
            // unique across the staged tree. Reject BEFORE any commit.
            let mut ids = std::collections::HashSet::new();
            for n in &tx.nodes {
                if let Some(id) = &n.automation_id
                    && (id.is_empty()
                        || id.len() > 256
                        || id.chars().any(char::is_control)
                        || !ids.insert(id.clone()))
                {
                    let d = UiDiagnostic::DuplicateAutomationId;
                    self.diagnostics.push(d);
                    self.scratch_nodes = tx.nodes;
                    return Err(UiError::InvalidUi(d));
                }
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
    pub(crate) fn commit_children(
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
}
