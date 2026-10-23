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
    pub(crate) fn create_node(
        &mut self,
        parent: Option<u32>,
        staged: StagedNode,
        nodes: &mut Vec<StagedNode>,
    ) -> UiResult<NodeId> {
        let StagedNode {
            automation_id,
            part_key,
            role,
            key,
            data,
            presented,
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
            line_height,
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
                line_height: *line_height,
            })?);
        }

        let id = self.arena.alloc(Node {
            automation_id,
            part_key,
            role,
            key,
            parent,
            children: Vec::new(),
            data,
            presented,
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
            if let NodeData::Editor {
                snapshot,
                sync,
                selection,
                ..
            } = &mut n.data
            {
                let token = TextValue::acquire_lease(&snapshot.lease)
                    .ok_or(UiError::InvalidUi(UiDiagnostic::DuplicateTextBinding))?;
                match n.peer.as_mut() {
                    Some(p) => {
                        sync.mount(p.as_mut(), snapshot, token)?;
                        // staged selection applies once mounted — the peer
                        // skips the write when already there
                        p.apply_selection(*selection)?;
                    }
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

    pub(crate) fn update_node(
        &mut self,
        id: NodeId,
        staged: StagedNode,
        nodes: &mut Vec<StagedNode>,
    ) -> UiResult {
        let StagedNode {
            automation_id,
            role,
            data,
            presented,
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
            let identity_changed = n.automation_id != automation_id || n.role != role;
            n.automation_id = automation_id;
            n.role = role;
            n.dirty = crate::node::dirty_diff(&old_data, &n.data, dark, forced);
            if identity_changed {
                n.dirty |= Self::DIRTY_SEMANTICS;
            }
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
                        if let NodeData::Editor {
                            snapshot,
                            sync,
                            selection,
                            ..
                        } = &mut n.data
                        {
                            match sync.commit(p.as_mut(), snapshot) {
                                Ok(PeerDecision::Conflict) => {
                                    emit_conflict_for = Some(snapshot.clone())
                                }
                                Err(e) => commit_err = Some(e),
                                _ => {}
                            }
                            // AFTER text sync — a staged selection is
                            // authoritative; the peer writes only when
                            // its current selection differs
                            if let Err(e) = p.apply_selection(*selection) {
                                commit_err = Some(e);
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
            // presented visual state — paint always; layout only when a
            // metric-bearing state branch could respond to it
            if n.presented != presented {
                n.presented = presented;
                n.dirty |= Self::DIRTY_PAINT;
                if n.state_metric_affecting() {
                    n.dirty |= Self::DIRTY_LAYOUT;
                }
            }
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
    pub(crate) fn remove_node(&mut self, id: NodeId) {
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
}
