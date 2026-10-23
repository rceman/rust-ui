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
use super::*;

impl<S, M, U, V> Runtime<S, M, U, V>
where
    M: 'static,
    U: Fn(&mut S, M, &mut UpdateCtx<M>),
    V: Fn(&S, &mut Ui<'_, '_, M>),
{
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

    /// The committed mirror for `node` — (applied_text, peer_revision) —
    /// used by the IME-end reconcile to compare the peer's true text
    /// against acknowledged committed state.
    pub(crate) fn committed_editor(&self, node: NodeId) -> Option<(String, TextRevision)> {
        self.arena.get(node).and_then(|n| match &n.data {
            NodeData::Editor { sync, .. } => Some((sync.applied_text.clone(), sync.peer_revision)),
            _ => None,
        })
    }

    /// Composition flag falls before the final native commit drains —
    /// `composition_end` still runs the proposal verdict.
    pub(crate) fn composition_clear(&mut self, node: NodeId) {
        if let Some(n) = self.arena.get_mut(node)
            && let NodeData::Editor { sync, .. } = &mut n.data
        {
            sync.composing = false;
        }
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
    pub(crate) fn apply_reduced(&mut self) {
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
