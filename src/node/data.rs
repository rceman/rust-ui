use super::*;

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
    /// card: density token selecting the `--card-spacing` recipe
    pub card_size: Option<crate::theme::CardSize>,
    /// scroll viewport: app-owned vertical offset in Dp — staged state,
    /// never backend-hidden (the layout pass clamps it to the measured
    /// content extent; `Dp::default()` = top)
    pub scroll: crate::geom::Dp,
}

impl ContainerProps {
    /// Resolved box style for surface/box containers; None for plain
    /// layout-only containers (row/column/stack/group/scope).
    pub(crate) fn resolved_box(&self, kind: u8) -> Option<crate::style::BoxStyle> {
        match kind {
            KIND_SURFACE | KIND_POPUP => {
                let mut s = crate::style::surface_recipe();
                s.patch(&self.patch);
                Some(s)
            }
            KIND_CARD => {
                let mut s = crate::style::card_recipe(self.card_size.unwrap_or_default());
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
        /// optional painted chrome under the text (h2 rule, inline-code
        /// surface) — authored full style, resolved like a `box_` frame
        chrome: Option<crate::style::BoxStyle>,
    },
    Button {
        text: Rc<str>,
        variant: ButtonVariant,
        style: ButtonStylePatch,
        disabled: bool,
        size: crate::theme::ControlSize,
        motion: Option<MotionToken>,
        tooltip: Option<Rc<str>>,
        /// decorative icons (Lucide path geometry) at the start/end edge;
        /// both are inert — the button node owns all interaction
        icon_start: Option<crate::icons::Icon>,
        icon_end: Option<crate::icons::Icon>,
        /// icon-only control (`icon_button`) — square size-class measure,
        /// no visible text; `accessible_name` carries the UIA name
        icon_only: bool,
        /// accessible name when the visible text isn't the name (required
        /// for icon-only buttons)
        accessible_name: Option<Rc<str>>,
        /// semantic `aria-invalid` — destructive chrome, orthogonal to
        /// every interaction state
        invalid: bool,
        /// semantic `data-active`/`aria-selected` — the recipe's selected
        /// overlay (variant-dependent), orthogonal to interaction state.
        /// `focus_on_open` popups prefer a `selected` descendant for the
        /// initial focus (listbox opens with the selected option live).
        selected: bool,
        /// expanded/collapsed state for popup triggers, if applicable
        expanded: Option<bool>,
        /// icon tint override — `None` paints icons in the resolved
        /// foreground (the `currentColor` analog); the select trigger's
        /// chevron keeps `muted-foreground` against a filled value
        icon_color: Option<crate::style::Color>,
    },
    /// Decorative vector icon leaf (Lucide stroke geometry) — paints a
    /// stroke path at `size`; `color` is the `currentColor` analog.
    Icon {
        icon: crate::icons::Icon,
        size: crate::geom::Dp,
        color: crate::style::Color,
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
        /// semantic `aria-invalid` — the frozen invalid ring is
        /// unconditional (always-on `ring-destructive`), orthogonal to
        /// focus/disabled
        invalid: bool,
        /// staged selection — UTF-16 `[start, end)`; `None` leaves the
        /// peer's selection unmanaged (user/native owns it)
        selection: Option<(usize, usize)>,
        /// authored exact line box — reaches the peer at mount
        /// (`PeerSpec`), `None` keeps the font's natural height
        line_height: Option<crate::geom::Dp>,
        sync: TextPeerSync,
    },
    /// `ui.checkbox` / `ui.switch` / `ui.radio_group` item — one painted
    /// selection-control leaf. App model owns `checked`; `on_change`
    /// factories ride the shared Press path (click + Space/Enter).
    Toggle {
        kind: crate::theme::ToggleKind,
        checked: bool,
        disabled: bool,
        /// semantic `aria-invalid` — destructive indicator border + ring
        invalid: bool,
        size: crate::theme::ToggleSize,
        /// optional visible label painted at the frozen +24dp offset —
        /// app-owned text, decorative (the node owns all interaction)
        label: Option<Rc<str>>,
        /// accessible name when the visible label isn't the control name
        accessible_name: Option<Rc<str>>,
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
        checked: Option<bool>,
    },
    /// `ui.popover` — open anchored overlay. Staged inline; the backend
    /// resolves `anchor`'s laid-out rect each pass and places the popup
    /// over it per `side`/`align`/`offset` (signed Dp, `x` = align axis,
    /// `y` = side axis for top/bottom). `dismiss_light` =
    /// outside-press-dismiss, `focus_on_open` moves focus inside on open
    /// (restored on close), `modal` paints a scrim + gates hit/focus to
    /// the popup subtree. `props` chrome resolves like `surface`.
    Popup {
        anchor: Option<Rc<str>>,
        side: crate::geom::Side,
        align: Align,
        offset: crate::geom::Point,
        role: PopupRole,
        modal: bool,
        dismiss_light: bool,
        focus_on_open: bool,
        props: ContainerProps,
    },
}

/// Message adapter chain (ui.scope) stored per node: leaf-most first —
/// apply in stored order to lift a child message to the root `M`.
pub(crate) type MsgAdapters = Rc<Vec<Rc<dyn Fn(Box<dyn Any>) -> Box<dyn Any>>>>;

pub(crate) struct Node {
    pub automation_id: Option<Rc<str>>,
    /// semantic part identity inside the owner's automation surface —
    /// `ui.part("header", …)`/`ui.part("header.title", …)`; emitted by
    /// devtools as `owner#part`
    pub part_key: Option<Rc<str>>,
    /// semantic control role (`SemanticRole` docs) — staged when the
    /// painted kind doesn't express the control's full semantics
    /// (combobox trigger / option / tablist / tab)
    pub role: Option<SemanticRole>,
    pub key: ChildKey,
    pub parent: Option<u32>,
    pub children: Vec<u32>,
    pub data: NodeData,
    /// staged presentation-only visual state (`Presented` docs)
    pub presented: Presented,
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

/// Resolved `VisualStyle` diff — text metrics feed layout too. THE shared
/// interaction-state classifier: the backend's mark_state_dirty consumes
/// this — it must never reimplement a box-only approximation.
pub(crate) fn visual_dirty(
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
