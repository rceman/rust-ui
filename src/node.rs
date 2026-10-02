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
    /// resolved editable foreground — the ONLY style input a native peer
    /// receives (selection/caret/IME colors and fonts stay OS-owned)
    pub foreground: crate::style::Color,
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
    /// Re-style live text: a changed `foreground` re-applies the peer's
    /// char format. Capability-limited — no size/weight knobs reach the
    /// peer. Default no-op — core tests use peers that don't style.
    fn apply_foreground(&mut self, _fg: crate::style::Color) {}
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
        /// capability-limited chrome+foreground patch — chrome paints the
        /// frame, only `foreground` reaches the peer (approved boundary)
        patch: crate::style::TextInputStylePatch,
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

impl Node {
    /// Can an interaction-state transition on this node change layout?
    /// True iff any authored state branch carries a metric-bearing patch —
    /// the classifier consults this once per transition, not per paint.
    pub(crate) fn state_metric_affecting(&self) -> bool {
        match &self.data {
            NodeData::Button { style, .. } => style.styles.any_metric_branch(),
            NodeData::Action { style, .. } => style.any_metric_branch(),
            NodeData::Editor { patch, .. } => patch.chrome.metric_affecting(),
            _ => false,
        }
    }
}

/// Diff two RESOLVED box styles into dirty classes: padding/border-width/
/// radii changes may reflow content (LAYOUT); background/border-color/
/// shadow are paint-only. Identical output = no bits.
pub(crate) fn box_dirty(
    a: &crate::style::BoxStyle,
    b: &crate::style::BoxStyle,
    dark: bool,
    out: &mut u8,
) {
    const LAYOUT: u8 = 0b0000_0001;
    const PAINT: u8 = 0b0000_0010;
    if a == b {
        return;
    }
    // the no-op decision compares CONCRETE output — authored roles that
    // resolve to the same RGBA under the live theme produce no work
    if crate::style::box_resolved_eq(a, b, dark) {
        return;
    }
    *out |= PAINT;
    if a.padding != b.padding || a.radii != b.radii {
        *out |= LAYOUT;
    }
    // border widths consume insets — colors don't
    let bw = |b: &crate::style::BoxStyle| {
        [
            b.border.top.width,
            b.border.right.width,
            b.border.bottom.width,
            b.border.left.width,
        ]
    };
    if bw(a) != bw(b) {
        *out |= LAYOUT;
    }
}

/// Resolved `VisualStyle` diff — text metrics feed layout too.
fn visual_dirty(
    a: &crate::style::VisualStyle,
    b: &crate::style::VisualStyle,
    dark: bool,
    out: &mut u8,
) {
    const LAYOUT: u8 = 0b0000_0001;
    const PAINT: u8 = 0b0000_0010;
    if a == b {
        return;
    }
    if crate::style::visual_resolved_eq(a, b, dark) {
        return;
    }
    *out |= PAINT;
    if a.text_style.size != b.text_style.size || a.text_style.weight != b.text_style.weight {
        *out |= LAYOUT;
    }
    box_dirty(&a.box_style, &b.box_style, dark, out);
}

/// Compare committed payloads for the dirty classification — decisions use
/// RESOLVED concrete output (theme/state applied, OS enforcement applied),
/// not authored descriptors: two authored forms producing identical
/// output are a strict no-op; a shadow-only change is paint-only and
/// never requests layout.
pub(crate) fn dirty_diff(
    old: &NodeData,
    new: &NodeData,
    dark: bool,
    forced: Option<&dyn Fn(crate::style::SystemColor) -> [f32; 4]>,
) -> u8 {
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
            let (al, bl) = (a.layout_clone(), b.layout_clone());
            let mut d = 0;
            if al != bl {
                d |= LAYOUT;
            }
            // resolved boxes — recipe+patch+full, OS-enforced
            let mut ra = a.resolved_box(*ak);
            let mut rb = b.resolved_box(*bk);
            if let Some(v) = &mut ra {
                crate::style::os_enforce_box(v, forced);
            }
            if let Some(v) = &mut rb {
                crate::style::os_enforce_box(v, forced);
            }
            match (ra, rb) {
                (Some(ra), Some(rb)) => box_dirty(&ra, &rb, dark, &mut d),
                (ra, rb) => {
                    if ra.is_some() != rb.is_some() {
                        d |= PAINT | LAYOUT;
                    }
                }
            }
            d
        }
        (
            NodeData::Label {
                text: at,
                wrap: aw,
                color_role: ac,
                patch: ap,
            },
            NodeData::Label {
                text: bt,
                wrap: bw,
                color_role: bc,
                patch: bp,
            },
        ) => {
            if at != bt || aw != bw {
                LAYOUT | PAINT
            } else {
                // resolved label text = recipe + patch
                let mut ra = crate::style::label_recipe();
                ra.patch(ap);
                let mut rb = crate::style::label_recipe();
                rb.patch(bp);
                let mut d = 0;
                if ac != bc {
                    d |= PAINT;
                }
                if ra != rb {
                    d |= PAINT;
                    if ra.size != rb.size || ra.weight != rb.weight {
                        d |= LAYOUT;
                    }
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
            if at != bt || av != bv || am != bm {
                d |= PAINT;
            }
            if ad != bd || atp != btp {
                d |= SEMANTICS;
            }
            // RESOLVED normal-state styles — recipe+patch applied, then
            // OS enforcement: two authored forms producing the same
            // concrete output are a strict no-op
            let mut ra = crate::style::resolve_button(
                *av,
                *asz,
                ast,
                crate::style::StyleState::Normal,
                false,
                dark,
            );
            crate::style::os_enforce_visual(&mut ra, forced);
            let mut rb = crate::style::resolve_button(
                *bv,
                *bsz,
                bst,
                crate::style::StyleState::Normal,
                false,
                dark,
            );
            crate::style::os_enforce_visual(&mut rb, forced);
            visual_dirty(&ra, &rb, dark, &mut d);
            // state branches' metrics feed layout too — padding in hover/
            // pressed/disabled/focus branches
            for st in [
                crate::style::StyleState::Hover,
                crate::style::StyleState::Pressed,
                crate::style::StyleState::Disabled,
            ] {
                let ra = crate::style::resolve_button(*av, *asz, ast, st, false, dark);
                let rb = crate::style::resolve_button(*bv, *bsz, bst, st, false, dark);
                if ra.box_style.padding != rb.box_style.padding
                    || ra.box_style.border.top.width != rb.box_style.border.top.width
                    || ra.box_style.border.right.width != rb.box_style.border.right.width
                    || ra.box_style.border.bottom.width != rb.box_style.border.bottom.width
                    || ra.box_style.border.left.width != rb.box_style.border.left.width
                {
                    d |= PAINT | LAYOUT;
                } else if ra != rb {
                    d |= PAINT;
                }
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
                patch: ap,
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
                patch: bp,
                ..
            },
        ) => {
            let mut d = 0;
            if aml != bml {
                d |= LAYOUT;
            }
            // committed text feeds the peer's natural measurement — a
            // content change can change the natural extent, so it is a
            // layout-classified dirty, not paint-only
            if asnap.committed != bsnap.committed {
                d |= PAINT | LAYOUT;
            }
            if aph != bph {
                d |= PAINT;
            }
            if ap != bp {
                // foreground -> peer repaint; chrome -> resolved box diff
                if ap.foreground != bp.foreground {
                    d |= PAINT;
                }
                let mut ca = crate::style::resolve_text_input_chrome(ap);
                let mut cb = crate::style::resolve_text_input_chrome(bp);
                crate::style::os_enforce_box(&mut ca, forced);
                crate::style::os_enforce_box(&mut cb, forced);
                box_dirty(&ca, &cb, dark, &mut d);
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
            // resolved normal-state box — padding/border widths feed the
            // action's content insets; colors/shadow are paint-only
            let ra = ast.resolve(*ad, false, false, false);
            let rb = bst.resolve(*bd, false, false, false);
            box_dirty(&ra, &rb, dark, &mut d);
            // state branches can also carry metrics-bearing fields — only
            // when a branch's patch actually changed
            for (ap, bp) in [ast.hover, ast.pressed, ast.disabled, ast.focus_visible]
                .iter()
                .zip([bst.hover, bst.pressed, bst.disabled, bst.focus_visible].iter())
            {
                if ap == bp {
                    continue;
                }
                d |= PAINT;
                if ap.is_some_and(|p| crate::style::patch_touches_layout(&p))
                    || bp.is_some_and(|p| crate::style::patch_touches_layout(&p))
                {
                    d |= LAYOUT;
                }
            }
            if ad != bd || al != bl {
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
