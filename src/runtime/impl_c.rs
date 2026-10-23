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
    pub(crate) fn dispatch_event(&mut self, ev: QueuedEvent) -> UiResult<bool> {
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

    pub(crate) fn deliver_envelope(&mut self, env: Envelope) -> UiResult<bool> {
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
    pub(crate) fn emit_conflict_for_request(&mut self, id: NodeId, rejected: TextRevision) {
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

    pub(crate) fn emit_conflict(&mut self, id: NodeId, snapshot: &crate::text::TextSnapshot) {
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
}
