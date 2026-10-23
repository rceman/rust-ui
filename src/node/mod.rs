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
pub(crate) const KIND_ICON: u8 = 14;
pub(crate) const KIND_CARD: u8 = 15;
/// Scroll viewport container — children lay out as a vertical column at
/// unbounded height, translated by the app-owned offset; painting and
/// hit-testing clip descendants to the viewport inner rect.
pub(crate) const KIND_SCROLL: u8 = 16;
/// Painted toggle leaf — the three frozen selection controls
/// (`ToggleKind::{Checkbox,RadioItem,Switch}`) share one node kind: an
/// actionable leaf owning hit/focus/press and its own painted indicator/
/// track/thumb.
pub(crate) const KIND_TOGGLE: u8 = 17;
/// Anchored overlay layer — staged inline, laid out LAST: the layout pass
/// defers popups, resolves each against its anchor rect inside the client
/// viewport (`crate::layout::resolve_popup_placement`), and appends them
/// at the end of paint/hit order outside any scroll clip.
pub(crate) const KIND_POPUP: u8 = 18;

/// Semantic overlay role — drives the UIA control type. `Popup` is the
/// generic floating pane; `Menu`/`Listbox` mark menu/list-popups.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum PopupRole {
    #[default]
    Popup,
    Menu,
    Listbox,
}

/// Semantic control role — the ARIA-analog carried when the painted kind
/// doesn't fully express the control's semantics: a Button staged as a
/// combobox trigger or a listbox option, a styled row as a tablist.
/// Drives the UIA control type and the shared directional-navigation
/// policy (`crate::layout::nav_target`); `None` means the node kind
/// speaks for itself.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SemanticRole {
    /// select trigger — a button that opens a listbox (UIA ComboBox)
    ComboBox,
    /// listbox option — directional nav (arrows/Home/End) moves focus
    /// among same-role siblings inside the enclosing popup
    Option,
    /// tab list container — bounds directional nav for `Tab` descendants
    /// (UIA Tab)
    TabList,
    /// tab item — directional nav moves focus AND activates the target
    /// (UIA TabItem)
    Tab,
    /// dropdown menu entry — directional nav skips disabled menu entries
    MenuItem,
    MenuItemCheckbox,
    MenuItemRadio,
    MenuItemDestructive,
    MenuButton,
    Group,
    /// settings navigation link, exposed as a button-like item
    SettingsNavItem,
}

/// Everything the platform needs to build one native editor peer —
/// behavior gates are part of the spec so a mounted editor never defaults
/// to writable when the node is read-only/disabled.
#[derive(Clone, Debug)]
pub struct PeerSpec {
    pub multiline: bool,
    pub read_only: bool,
    pub disabled: bool,
    /// resolved editable foreground — the ONLY color input a native peer
    /// receives (selection/caret/IME colors and fonts stay OS-owned)
    pub foreground: crate::style::Color,
    /// authored exact line box — `None` keeps the font's natural height;
    /// a geometry knob, not a style channel (mount-time like `multiline`)
    pub line_height: Option<crate::geom::Dp>,
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
    /// Staged selection — UTF-16 `[start, end)` char range. `None` leaves
    /// the selection to native/user control; `Some` is authoritative —
    /// the peer applies it when its current selection differs. Default
    /// no-op — core tests use peers that don't select.
    fn apply_selection(&mut self, _sel: Option<(usize, usize)>) -> UiResult {
        Ok(())
    }
    /// latched authored line box (twips) for snapshot introspection —
    /// `None` for peers that don't carry a line box
    fn line_twips_latched(&self) -> Option<i32> {
        None
    }
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

/// Presentation-only staged visual state — painted/laid-out exactly like
/// the runtime interaction state it mirrors (`runtime || presented`), but
/// never grants interactivity, real focus, or semantics. Specimen pages
/// stage the interaction cells declaratively instead of driving them
/// through input. Devtools/UIA keep reporting the runtime truth.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Presented {
    pub hover: bool,
    pub pressed: bool,
    pub focus_visible: bool,
}

impl Presented {
    pub const HOVER: Presented = Presented {
        hover: true,
        pressed: false,
        focus_visible: false,
    };
    pub const PRESSED: Presented = Presented {
        hover: false,
        pressed: true,
        focus_visible: false,
    };
    pub const FOCUS_VISIBLE: Presented = Presented {
        hover: false,
        pressed: false,
        focus_visible: true,
    };

    /// Any presented bit set — the "is this a staged specimen" query
    /// devtools uses to report presentation honestly.
    pub fn any(&self) -> bool {
        self.hover || self.pressed || self.focus_visible
    }
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
mod data;
mod diff;
pub(crate) use data::*;
pub(crate) use diff::*;
