use std::any::Any;
use std::rc::Rc;

use crate::UiResult;
use crate::event::{EventFactorySet, FrameTime};
use crate::geom::{Align, Justify};
use crate::geom::{LayoutSpec, Visibility};
use crate::key::ChildKey;
use crate::style::ButtonStylePatch;
use crate::text::{
    BindingToken, EditOrigin, LeaseCell, TextConflict, TextEdit, TextRevision, TextSelection,
    TextSnapshot,
};
use crate::theme::{ButtonVariant, MotionToken, Space, SubmitPolicy};

/// Widget kind discriminant — feeds static-sibling ordinal identity.
pub(crate) const KIND_COLUMN: u8 = 1;
pub(crate) const KIND_ROW: u8 = 2;
pub(crate) const KIND_STACK: u8 = 3;
pub(crate) const KIND_SURFACE: u8 = 4;
pub(crate) const KIND_LABEL: u8 = 5;
pub(crate) const KIND_BUTTON: u8 = 6;
pub(crate) const KIND_TEXT_INPUT: u8 = 7;
pub(crate) const KIND_TEXT_AREA: u8 = 8;
pub(crate) const KIND_CUSTOM: u8 = 9;
pub(crate) const KIND_GROUP: u8 = 10;
pub(crate) const KIND_SCOPE: u8 = 11;
pub(crate) const KIND_BOX: u8 = 12;
pub(crate) const KIND_ACTION: u8 = 13;

/// Everything the platform needs to build one native editor peer —
/// behavior gates are part of the spec so a mounted editor never defaults
/// to writable when the node is read-only/disabled.
#[derive(Clone, Debug)]
pub struct PeerSpec {
    pub multiline: bool,
    pub read_only: bool,
    pub disabled: bool,
    pub style: crate::style::TextStyle,
}

/// Backend surface the retained core talks to. Windowless rich-text peers
/// implement this; tests drive `FakePeer`s.
pub(crate) trait TextPeer {
    /// Opaque peer identity (survives reconciles, dies with the node).
    fn peer_id(&self) -> u64;
    /// Mount-time initialize: set text at the snapshot's COMMITTED revision
    /// under `binding`. Native implementations suppress any edit ack for
    /// this initial content — initialization is not an acknowledgement.
    fn initialize(
        &mut self,
        text: &str,
        revision: crate::text::TextRevision,
        binding: crate::text::BindingToken,
    ) -> UiResult;
    /// Apply a programmatic proposal: `base` is the expected current peer
    /// revision, `requested` the proposed result revision. The peer must
    /// produce a REAL Programmatic acknowledgment (actual read-back), never
    /// a fabricated one. Failable — a native failure is typed, not swallowed.
    fn set_text(
        &mut self,
        text: &str,
        base: crate::text::TextRevision,
        requested: crate::text::TextRevision,
    ) -> UiResult;
    /// Re-style live text: a changed surgical TextStylePatch resolves to a
    /// full style and re-applies the peer's char format (fg/size/weight).
    /// Default no-op — core tests use peers that don't style.
    fn apply_text_style(&mut self, _style: &crate::style::TextStyle) {}
    /// The node's effective editability changed — push it into native
    /// behavior (EM_SETREADONLY gates edits inside msftedit, not just in
    /// the input router). The runtime passes `read_only || disabled`.
    /// Default no-op — core tests use peers that don't gate.
    fn set_read_only(&mut self, _ro: bool) {}
    /// Teardown — called exactly once per peer.
    fn release(&mut self);
    /// bound after the arena assigns the NodeId — peers tag their native
    /// callbacks with it
    fn attach(&mut self, node: NodeId) {
        let _ = node;
    }
}

/// Stable handle into the retained arena; `generation` invalidates the id the
/// moment the slot is freed.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct NodeId {
    pub slot: u32,
    pub generation: u64,
}

/// Private peer-side text model for one mounted editable node.
///
/// The peer keeps its OWN committed mirror (`peer_revision`/`applied_*`) —
/// it advances on every VALIDATED native commit even when the consumer's
/// `TextValue` never accepts it, so a later programmatic base is compared
/// against the peer's truth, not the app's stale acknowledgement.
#[derive(Debug, Default)]
pub(crate) struct TextPeerSync {
    pub binding: Option<BindingToken>,
    /// latest committed/applied revision the peer reports
    pub peer_revision: TextRevision,
    /// last applied (set-text or validated native-committed) snapshot
    pub applied_text: String,
    pub applied_revision: TextRevision,
    /// proposal sent, awaiting the peer's Programmatic ack — the ack must
    /// carry this requested result AND the proposal's original base
    pub in_flight: Option<TextRevision>,
    pub in_flight_base: Option<TextRevision>,
    /// a proposal arrived while composition was active — resolved at end
    pub queued_while_composing: bool,
    pub composing: bool,
    /// the request rejected by the last emitted conflict — a tombstone so
    /// the same rejected proposal never produces a second conflict or resend
    pub last_rejected: Option<TextRevision>,
    pub mounted: bool,
}

/// What `commit`/`composition_end` decided for the peer.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum PeerDecision {
    Nothing,
    /// a proposal was applied (set_text emitted)
    Applied(TextRevision),
    /// base diverged — emit `TextConflict` (unless composing: defer)
    Conflict,
}

impl TextPeerSync {
    /// Mount-time initialize: set text at the snapshot's COMMITTED revision
    /// (survives remount with the acknowledged revision), then apply a live
    /// pending proposal immediately — the first mount does not wait for a
    /// second view.
    pub(crate) fn mount(
        &mut self,
        peer: &mut dyn TextPeer,
        snapshot: &TextSnapshot,
        binding: BindingToken,
    ) -> UiResult {
        peer.initialize(&snapshot.committed, snapshot.revision, binding)?;
        self.binding = Some(binding);
        self.applied_text = snapshot.committed.to_string();
        self.applied_revision = snapshot.revision;
        self.peer_revision = snapshot.revision;
        self.mounted = true;
        // apply a queued intent right away — base must still match the peer
        if let Some(p) = &snapshot.pending
            && p.base == self.peer_revision
            && !self.composing
        {
            peer.set_text(&p.text, p.base, p.requested)?;
            self.applied_text = p.text.to_string();
            self.applied_revision = p.requested;
            self.in_flight = Some(p.requested);
            self.in_flight_base = Some(p.base);
            self.peer_revision = p.requested;
        }
        Ok(())
    }

    /// Validate a native edit against the peer mirror.
    /// - foreign binding / stale / out-of-order => `None`: mirror untouched,
    ///   the event is dropped before any callback.
    /// - valid Programmatic ack => `Some(None)` — in-flight cleared.
    /// - valid NativePeer commit => `Some(maybe_superseded)`: when a
    ///   programmatic apply was in flight, the native commit wins — the
    ///   request is superseded and reported for ONE tombstoned conflict.
    pub(crate) fn accept_native(&mut self, edit: &TextEdit) -> Option<Option<TextRevision>> {
        if self.binding != Some(edit.binding()) {
            return None;
        }
        match edit.origin() {
            EditOrigin::Programmatic => {
                // peer acknowledged our set-text — result must match the
                // in-flight request AND base must be the proposal's original
                // base (an arbitrary matching id can't overwrite the mirror)
                if self.in_flight != Some(edit.result_revision())
                    || self.in_flight_base != Some(edit.base_revision())
                    || edit.text() != self.applied_text
                {
                    return None;
                }
                self.in_flight = None;
                self.in_flight_base = None;
            }
            EditOrigin::NativePeer => {
                if edit.base_revision() != self.peer_revision
                    || edit.result_revision() <= self.peer_revision
                {
                    return None;
                }
            }
        }
        let superseded = match edit.origin() {
            // a native commit while a request is in flight rejects it
            EditOrigin::NativePeer => self.in_flight.take(),
            EditOrigin::Programmatic => None,
        };
        self.peer_revision = edit.result_revision();
        self.applied_revision = edit.result_revision();
        self.applied_text = edit.text().to_string();
        Some(superseded)
    }

    /// Commit-time reconciliation of the staged snapshot against the peer.
    /// During composition the proposal is queued and resolved at end — a
    /// conflict is never emitted while input is marked active.
    pub(crate) fn commit(
        &mut self,
        peer: &mut dyn TextPeer,
        snapshot: &TextSnapshot,
    ) -> Result<PeerDecision, crate::UiError> {
        if !self.mounted {
            return Ok(PeerDecision::Nothing);
        }
        let Some(p) = &snapshot.pending else {
            return Ok(PeerDecision::Nothing);
        };
        // tombstoned rejection: never resend, never re-report
        if self.last_rejected == Some(p.requested) {
            return Ok(PeerDecision::Nothing);
        }
        if self.in_flight == Some(p.requested) {
            return Ok(PeerDecision::Nothing); // sent, awaiting ack
        }
        // base must equal the peer's committed revision exactly — a stale
        // OR malformed future base both conflict
        if p.base != self.peer_revision {
            if self.composing {
                self.queued_while_composing = true;
                return Ok(PeerDecision::Nothing);
            }
            self.last_rejected = Some(p.requested);
            return Ok(PeerDecision::Conflict);
        }
        if self.composing {
            self.queued_while_composing = true;
            return Ok(PeerDecision::Nothing);
        }
        peer.set_text(&p.text, p.base, p.requested)?;
        self.applied_text = p.text.to_string();
        self.applied_revision = p.requested;
        self.in_flight = Some(p.requested);
        self.in_flight_base = Some(p.base);
        self.peer_revision = p.requested;
        Ok(PeerDecision::Applied(p.requested))
    }

    /// Composition ended: resolve a queued proposal — apply if its base
    /// survived, otherwise emit ONE conflict (tombstoned).
    pub(crate) fn composition_end(
        &mut self,
        peer: &mut dyn TextPeer,
        snapshot: &TextSnapshot,
    ) -> Result<PeerDecision, crate::UiError> {
        self.composing = false;
        if !self.queued_while_composing {
            return Ok(PeerDecision::Nothing);
        }
        self.queued_while_composing = false;
        match &snapshot.pending {
            Some(p) if self.last_rejected != Some(p.requested) => {
                // exact equality — a stale OR malformed future base conflicts
                if p.base == self.peer_revision {
                    peer.set_text(&p.text, p.base, p.requested)?;
                    self.applied_text = p.text.to_string();
                    self.applied_revision = p.requested;
                    self.in_flight = Some(p.requested);
                    self.in_flight_base = Some(p.base);
                    self.peer_revision = p.requested;
                    Ok(PeerDecision::Applied(p.requested))
                } else {
                    self.last_rejected = Some(p.requested);
                    Ok(PeerDecision::Conflict)
                }
            }
            _ => Ok(PeerDecision::Nothing),
        }
    }
}

/// Platform/container props retained on container nodes.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub(crate) struct ContainerProps {
    pub gap: Option<Space>,
    pub align: Option<Align>,
    pub justify: Option<Justify>,
    pub padding: Option<Space>,
    /// surface: surgical patch over `surface_recipe`
    pub patch: crate::style::BoxStylePatch,
    /// box_: full authored style (no recipe merge)
    pub full: Option<crate::style::BoxStyle>,
}

impl ContainerProps {
    /// Resolved box style for surface/box containers; None for plain
    /// layout-only containers (row/column/stack/group/scope).
    pub(crate) fn resolved_box(&self, kind: u8) -> Option<crate::style::BoxStyle> {
        match kind {
            KIND_SURFACE => {
                let mut s = crate::style::surface_recipe();
                s.patch(&self.patch);
                Some(s)
            }
            KIND_BOX => Some(self.full.unwrap_or_default()),
            _ => None,
        }
    }
}

/// Retained node payload per kind.
pub(crate) enum NodeData {
    /// placeholder while a staged node is being moved out of the flat buffer
    Empty,
    Container {
        kind: u8,
        props: ContainerProps,
    },
    Label {
        text: Rc<str>,
        wrap: bool,
        color_role: crate::theme::ColorRole,
        /// surgical text patch — `.color_role` is a shorthand that sets only
        /// `patch.foreground = Some(Color::Role(..))`
        patch: crate::style::TextStylePatch,
    },
    Button {
        text: Rc<str>,
        variant: ButtonVariant,
        style: ButtonStylePatch,
        disabled: bool,
        size: crate::theme::ControlSize,
        motion: Option<MotionToken>,
        tooltip: Option<Rc<str>>,
    },
    Editor {
        /// immutable staged snapshot of the bound TextValue this view
        snapshot: TextSnapshot,
        multiline: bool,
        read_only: bool,
        disabled: bool,
        submit: SubmitPolicy,
        max_lines: Option<u32>,
        placeholder: Option<Rc<str>>,
        accessible_label: Option<Rc<str>>,
        /// surgical text style — native peers consume fg/size/weight only;
        /// face stays the system editable face (one render path)
        patch: crate::style::TextStylePatch,
        sync: TextPeerSync,
    },
    Custom {
        render: Rc<dyn crate::ui::CustomRender>,
        frame_events: bool,
    },
    /// `ui.action` — semantic activation wrapper: children are decorative
    /// content; the action node owns hit/focus/press/invoke
    Action {
        label: Rc<str>,
        style: crate::style::ActionStyle,
        disabled: bool,
    },
}

/// Message adapter chain (ui.scope) stored per node: leaf-most first —
/// apply in stored order to lift a child message to the root `M`.
pub(crate) type MsgAdapters = Rc<Vec<Rc<dyn Fn(Box<dyn Any>) -> Box<dyn Any>>>>;

pub(crate) struct Node {
    pub key: ChildKey,
    pub parent: Option<u32>,
    pub children: Vec<u32>,
    pub data: NodeData,
    pub visibility: Visibility,
    /// explicit sizing hints (validated at stage time)
    pub layout: LayoutSpec,
    /// committed event factories; replaced wholesale each commit so dispatch
    /// always calls the newest snapshot
    pub factories: EventFactorySet,
    /// message adapters installed by enclosing `ui.scope` calls
    pub adapters: MsgAdapters,
    /// native peer for editable text nodes (opaque to the core)
    pub peer: Option<Box<dyn TextPeer>>,
    /// dirty classification from the last commit — the backend prunes its
    /// cache to what actually changed (see DIRTY_* on Runtime)
    pub dirty: u8,
}

/// Compare committed payloads for the dirty classification — factories-only
/// updates produce NO dirty bits; same-value props are cheap.
pub(crate) fn dirty_diff(old: &NodeData, new: &NodeData) -> u8 {
    const LAYOUT: u8 = 0b0000_0001;
    const PAINT: u8 = 0b0000_0010;
    const SEMANTICS: u8 = 0b0000_1000;
    match (old, new) {
        (
            NodeData::Container { kind: ak, props: a },
            NodeData::Container { kind: bk, props: b },
        ) => {
            if ak != bk {
                return LAYOUT | PAINT | SEMANTICS;
            }
            // split: visual props (patch/full) -> PAINT, structural -> LAYOUT;
            // a patch that touches padding also consumes content insets
            let (av, bv) = (a.visual_clone(), b.visual_clone());
            let (al, bl) = (a.layout_clone(), b.layout_clone());
            let mut d = 0;
            if al != bl {
                d |= LAYOUT;
            }
            if av != bv {
                d |= PAINT;
                // patch affecting padding/radii feeds layout insets
                if crate::style::patch_touches_layout(&a.patch)
                    || crate::style::patch_touches_layout(&b.patch)
                    || a.full != b.full
                {
                    d |= LAYOUT;
                }
            }
            d
        }
        (
            NodeData::Label {
                text: at,
                wrap: aw,
                color_role: ac,
                ..
            },
            NodeData::Label {
                text: bt,
                wrap: bw,
                color_role: bc,
                ..
            },
        ) => {
            if at != bt || aw != bw {
                LAYOUT | PAINT
            } else {
                let mut d = 0;
                if ac != bc {
                    d |= PAINT;
                }
                match (old, new) {
                    (NodeData::Label { patch: ap, .. }, NodeData::Label { patch: bp, .. }) => {
                        if ap != bp {
                            d |= PAINT;
                            if ap.size != bp.size || ap.weight != bp.weight {
                                d |= LAYOUT; // metrics change
                            }
                        }
                    }
                    _ => {}
                }
                d
            }
        }
        (
            NodeData::Button {
                text: at,
                variant: av,
                style: ast,
                disabled: ad,
                size: asz,
                motion: am,
                tooltip: atp,
            },
            NodeData::Button {
                text: bt,
                variant: bv,
                style: bst,
                disabled: bd,
                size: bsz,
                motion: bm,
                tooltip: btp,
            },
        ) => {
            let mut d = 0;
            if at != bt || asz != bsz {
                d |= LAYOUT;
            }
            if at != bt || av != bv || ast != bst || am != bm {
                d |= PAINT;
            }
            if ad != bd || atp != btp {
                d |= SEMANTICS;
            }
            d
        }
        (
            NodeData::Editor {
                snapshot: asnap,
                read_only: aro,
                disabled: ad,
                max_lines: aml,
                placeholder: aph,
                accessible_label: aal,
                submit: asu,
                ..
            },
            NodeData::Editor {
                snapshot: bsnap,
                read_only: bro,
                disabled: bd,
                max_lines: bml,
                placeholder: bph,
                accessible_label: bal,
                submit: bsu,
                ..
            },
        ) => {
            let mut d = 0;
            if aml != bml {
                d |= LAYOUT;
            }
            if aph != bph || asnap.committed != bsnap.committed {
                d |= PAINT;
            }
            match (old, new) {
                (NodeData::Editor { patch: ap, .. }, NodeData::Editor { patch: bp, .. }) => {
                    if ap != bp {
                        d |= PAINT;
                        if ap.size != bp.size || ap.weight != bp.weight {
                            d |= LAYOUT; // peer metrics change
                        }
                    }
                }
                _ => {}
            }
            if aro != bro || ad != bd || aal != bal || asu != bsu {
                d |= SEMANTICS;
            }
            d
        }
        (
            NodeData::Custom {
                render: ar,
                frame_events: af,
            },
            NodeData::Custom {
                render: br,
                frame_events: bf,
            },
        ) => {
            let mut d = 0;
            if !std::rc::Rc::ptr_eq(ar, br) {
                d |= PAINT;
            }
            if af != bf {
                d |= SEMANTICS;
            }
            d
        }
        (
            NodeData::Action {
                label: al,
                style: ast,
                disabled: ad,
            },
            NodeData::Action {
                label: bl,
                style: bst,
                disabled: bd,
            },
        ) => {
            let mut d = 0;
            if ast != bst {
                d |= PAINT;
                // base padding feeds content insets — a changed base or any
                // patch that touches padding may change child rects
                if ast.base != bst.base
                    || [ast.hover, ast.pressed, ast.disabled, ast.focus_visible]
                        .iter()
                        .chain([bst.hover, bst.pressed, bst.disabled, bst.focus_visible].iter())
                        .flatten()
                        .any(crate::style::patch_touches_layout)
                {
                    d |= LAYOUT;
                }
            }
            if ad != bd {
                d |= SEMANTICS;
            }
            if al != bl {
                d |= SEMANTICS;
            }
            d
        }
        _ => 0b0000_0111, // kind swap — everything
    }
}

impl ContainerProps {
    fn visual_clone(&self) -> (crate::style::BoxStylePatch, Option<crate::style::BoxStyle>) {
        (self.patch, self.full)
    }
    fn layout_clone(&self) -> (Option<Space>, Option<Align>, Option<Justify>, Option<Space>) {
        (self.gap, self.align, self.justify, self.padding)
    }
}

impl Node {
    pub(crate) fn kind_tag(&self) -> u8 {
        match &self.data {
            NodeData::Empty => 0,
            NodeData::Container { kind, .. } => *kind,
            NodeData::Label { .. } => KIND_LABEL,
            NodeData::Button { .. } => KIND_BUTTON,
            NodeData::Editor { multiline, .. } => {
                if *multiline {
                    KIND_TEXT_AREA
                } else {
                    KIND_TEXT_INPUT
                }
            }
            NodeData::Custom { .. } => KIND_CUSTOM,
            NodeData::Action { .. } => KIND_ACTION,
        }
    }

    /// Interactive gate: factory events (press/pointer/key/focus/frame) are
    /// withheld while hidden or disabled.
    pub(crate) fn interactive(&self) -> bool {
        if self.visibility != Visibility::Visible {
            return false;
        }
        match &self.data {
            NodeData::Button { disabled, .. } => !disabled,
            NodeData::Editor { disabled, .. } => !disabled,
            NodeData::Action { disabled, .. } => !disabled,
            _ => true,
        }
    }

    /// Which lease this node's editor owns (for the mounted-editor index).
    pub(crate) fn lease(&self) -> Option<LeaseCell> {
        match &self.data {
            NodeData::Editor { snapshot, .. } => Some(snapshot.lease.clone()),
            _ => None,
        }
    }
}

/// Event delivered to a node's retained factory after commit.
pub(crate) enum NodeEvent {
    Press,
    Edit(TextEdit),
    /// native submit gesture (per-node SubmitPolicy)
    Submit,
    Selection(TextSelection),
    Frame(FrameTime),
    Key(crate::event::KeyEvent),
    Conflict(TextConflict),
    Pointer(crate::event::PointerEvent, PointerPhase),
    Focus(bool),
    OpenChange(bool),
    Scroll(crate::event::ScrollOffset),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum PointerPhase {
    Enter,
    Leave,
    Down,
    Up,
}

/// A pending native-origin event carrying its emitter's generation.
pub(crate) struct QueuedEvent {
    pub node: NodeId,
    pub payload: NodeEvent,
}
