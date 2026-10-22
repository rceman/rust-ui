use std::any::Any;
use std::collections::HashMap;
use std::rc::Rc;

use crate::UiDiagnostic;
use crate::arena::Arena;
use crate::event::{EventFactorySet, FrameTime, KeyEvent, PointerEvent, ScrollOffset};
use crate::geom::{Align, Justify, LayoutSpec, Length, Point, Visibility};
use crate::key::{ChildKey, ErasedKey, KeyId};
use crate::node::{ContainerProps, MsgAdapters, NodeData, NodeId, Presented, SemanticRole};
use crate::style::{Action, BoxProps, ButtonStylePatch};
use crate::text::{TextConflict, TextEdit, TextSelection, TextValue};
use crate::theme::{
    AlertVariant, Appearance, BadgeVariant, ButtonVariant, ColorRole, ControlSize, MotionToken,
    SeparatorOrientation, Space, SubmitPolicy, TabsVariant, Theme,
};

/// What a custom view contributes to the tree — an immutable `Rc` snapshot;
/// no borrowed view data is retained.
pub trait CustomRender {
    fn measure(&self, constraints: crate::geom::Constraints) -> crate::geom::Size;
    fn paint(&self, canvas: &mut dyn Canvas, bounds: crate::geom::Rect);
    fn hit_test(&self, local: Point, bounds: crate::geom::Rect) -> bool {
        let _ = local;
        let _ = bounds;
        true
    }
    fn semantics(&self) -> Semantics;
}

#[derive(Clone, Debug)]
pub struct Semantics {
    pub role: Role,
    pub label: String,
    pub actions: Vec<SemanticsAction>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Role {
    Button,
    Image,
    Text,
    Custom,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SemanticsAction {
    Press,
    Focus,
}

/// Small platform-neutral canvas op set — path and text only in v0.1; no
/// raw OS or GPU handles.
pub trait Canvas {
    fn path(&mut self, path: &Path2d, paint: Paint);
    fn text(&mut self, run: &TextRun, origin: Point);
}

#[derive(Clone, Debug, Default)]
pub struct Path2d {
    pub ops: Vec<PathOp>,
}

#[derive(Clone, Debug)]
pub enum PathOp {
    MoveTo(Point),
    LineTo(Point),
    QuadTo(Point, Point),
    Close,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Paint {
    FillRole(ColorRole),
    Rgba(u8, u8, u8, u8),
}

impl Paint {
    /// Theme-role fill — resolved against the active theme at draw time.
    pub fn fill_role(role: ColorRole) -> Paint {
        Paint::FillRole(role)
    }
}

#[derive(Clone, Debug)]
pub struct TextRun {
    pub text: Rc<str>,
    pub size: f32,
    pub color_role: ColorRole,
}

// ------------------------- container props ---------------------------------

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Row {
    pub gap: Space,
    pub align: Align,
    pub justify: Justify,
    pub padding: Space,
    /// cross-axis (height) sizing hint — `Content` (default) measures;
    /// `Fill` takes the parent's remaining cross-axis space
    pub height: Length,
    /// main-axis (width) sizing hint — same Length contract as leaves
    pub width: Length,
}

impl Row {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn gap(mut self, g: Space) -> Self {
        self.gap = g;
        self
    }
    pub fn align(mut self, a: Align) -> Self {
        self.align = a;
        self
    }
    pub fn justify(mut self, j: Justify) -> Self {
        self.justify = j;
        self
    }
    pub fn padding(mut self, p: Space) -> Self {
        self.padding = p;
        self
    }
    pub fn height(mut self, h: Length) -> Self {
        self.height = h;
        self
    }
    pub fn width(mut self, w: Length) -> Self {
        self.width = w;
        self
    }
    pub(crate) fn props(&self) -> ContainerProps {
        ContainerProps {
            gap: Some(self.gap),
            align: Some(self.align),
            justify: Some(self.justify),
            padding: Some(self.padding),
            ..Default::default()
        }
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Column {
    pub gap: Space,
    pub align: Align,
    pub justify: Justify,
    pub padding: Space,
    /// main-axis (height) sizing hint — `Content` (default) measures;
    /// `Fill` takes the parent's remaining main-axis space. A ROOT
    /// column fills the window client with `Fill(1)` — that is how an
    /// interior `Fill` child (e.g. a scroll viewport) sees leftover
    /// space instead of the column's natural content height.
    pub height: Length,
    /// cross-axis (width) sizing hint — same Length contract as leaves
    pub width: Length,
}

impl Column {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn gap(mut self, g: Space) -> Self {
        self.gap = g;
        self
    }
    pub fn align(mut self, a: Align) -> Self {
        self.align = a;
        self
    }
    pub fn justify(mut self, j: Justify) -> Self {
        self.justify = j;
        self
    }
    pub fn padding(mut self, p: Space) -> Self {
        self.padding = p;
        self
    }
    pub fn height(mut self, h: Length) -> Self {
        self.height = h;
        self
    }
    pub fn width(mut self, w: Length) -> Self {
        self.width = w;
        self
    }
    pub(crate) fn props(&self) -> ContainerProps {
        ContainerProps {
            gap: Some(self.gap),
            align: Some(self.align),
            justify: Some(self.justify),
            padding: Some(self.padding),
            ..Default::default()
        }
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Stack {
    pub padding: Space,
}

impl Stack {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn padding(mut self, p: Space) -> Self {
        self.padding = p;
        self
    }
    pub(crate) fn props(&self) -> ContainerProps {
        ContainerProps {
            padding: Some(self.padding),
            ..Default::default()
        }
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Surface {
    pub padding: Space,
    /// surgical patch — merges fieldwise within this build
    pub patch: crate::style::BoxStylePatch,
}

impl Surface {
    pub fn new() -> Self {
        Self::default()
    }
    /// recipe convenience — populates `BoxStyle.padding` before the patch
    pub fn padding(mut self, p: Space) -> Self {
        self.padding = p;
        self
    }
    /// surgical patch — later `.style` calls deep-merge, `None` never erases
    pub fn style(mut self, p: crate::style::BoxStylePatch) -> Self {
        self.patch.merge(&p);
        self
    }
    pub(crate) fn props(&self) -> ContainerProps {
        let mut patch = self.patch;
        // Space padding is a recipe convenience feeding the same BoxStyle
        // padding — a patch field wins over the convenience value
        let d = self.padding.dp();
        for f in [
            &mut patch.padding.top,
            &mut patch.padding.right,
            &mut patch.padding.bottom,
            &mut patch.padding.left,
        ] {
            if f.is_none() {
                *f = Some(d);
            }
        }
        ContainerProps {
            padding: None, // resolved from the box style, not Space
            patch,
            ..Default::default()
        }
    }
}

/// Card container props — `size` selects the frozen `--card-spacing`
/// token (Default 16dp / Sm 12dp); it drives the card's top padding,
/// the gap between its semantic parts, and their side padding.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Card {
    pub size: crate::theme::CardSize,
    /// optional fixed width — the frozen card specimen is 288dp; unset
    /// keeps content/stretch sizing
    pub width: Option<crate::geom::Dp>,
    /// surgical patch over `card_recipe`
    pub patch: crate::style::BoxStylePatch,
}

impl Card {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn size(mut self, s: crate::theme::CardSize) -> Self {
        self.size = s;
        self
    }
    pub fn width(mut self, w: crate::geom::Dp) -> Self {
        self.width = Some(w);
        self
    }
    /// surgical patch — later `.style` calls deep-merge, `None` never erases
    pub fn style(mut self, p: crate::style::BoxStylePatch) -> Self {
        self.patch.merge(&p);
        self
    }
    pub(crate) fn props(&self) -> ContainerProps {
        // card-spacing doubles as the part gap (16 -> Lg, 12 -> Md)
        let gap = match self.size {
            crate::theme::CardSize::Default => Space::Lg,
            crate::theme::CardSize::Sm => Space::Md,
        };
        ContainerProps {
            gap: Some(gap),
            align: Some(Align::Stretch),
            patch: self.patch,
            card_size: Some(self.size),
            ..Default::default()
        }
    }
}

/// Scroll viewport props — children lay out as a vertical column inside
/// the viewport at unbounded height. `offset` is APP-OWNED state passed
/// back in each view (like `Checkbox`'s checked) — the runtime clamps it
/// to the measured content extent at layout, it never lives in the
/// backend. Wheel/devctl input arrives through `on_scroll` deltas.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Scroll {
    /// app-owned vertical offset in Dp — the `ui.scroll` `offset`
    /// argument is authoritative when both are supplied
    pub offset: crate::geom::Dp,
    pub gap: Space,
    pub padding: Space,
}

impl Scroll {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn offset(mut self, o: crate::geom::Dp) -> Self {
        self.offset = o;
        self
    }
    pub fn gap(mut self, g: Space) -> Self {
        self.gap = g;
        self
    }
    pub fn padding(mut self, p: Space) -> Self {
        self.padding = p;
        self
    }
    pub(crate) fn props(&self) -> ContainerProps {
        ContainerProps {
            gap: Some(self.gap),
            padding: Some(self.padding),
            scroll: self.offset,
            ..Default::default()
        }
    }
}

/// Separator props — the frozen `bg-border` rule. `orientation` picks the
/// rule axis; `length` pins the major-axis extent (the frozen specimens
/// fix it: `w-48` = 192dp, `h-16` = 64dp). Unset, the separator fills
/// the offered main axis (`Fill(1)`); a `Start`-aligned parent that
/// offers no main-axis extent needs `length` or a stretch context.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Separator {
    pub orientation: SeparatorOrientation,
    pub length: Option<crate::geom::Dp>,
}

impl Separator {
    /// 1dp-tall horizontal rule — fills the offered width.
    pub fn horizontal() -> Self {
        Separator {
            orientation: SeparatorOrientation::Horizontal,
            length: None,
        }
    }
    /// 1dp-wide vertical rule — fills the offered height.
    pub fn vertical() -> Self {
        Separator {
            orientation: SeparatorOrientation::Vertical,
            length: None,
        }
    }
    /// Pin the major-axis extent (`w-48` / `h-16` in the frozen
    /// specimens) instead of filling what the parent offers.
    pub fn length(mut self, l: crate::geom::Dp) -> Self {
        self.length = Some(l);
        self
    }
    fn layout(&self) -> LayoutSpec {
        let major = self.length.map(Length::Fixed).unwrap_or(Length::Fill(1));
        match self.orientation {
            SeparatorOrientation::Horizontal => LayoutSpec {
                width: major,
                height: Length::Fixed(crate::geom::Dp(1.0)),
                ..Default::default()
            },
            SeparatorOrientation::Vertical => LayoutSpec {
                width: Length::Fixed(crate::geom::Dp(1.0)),
                height: major,
                ..Default::default()
            },
        }
    }
}

/// Alert container props — `variant` selects the frozen palette
/// (`alert.default` / `alert.destructive`), `width` an optional fixed
/// width (the frozen specimen is 320dp), `patch` the surgical override
/// over `alert_recipe`.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Alert {
    pub variant: AlertVariant,
    pub width: Option<crate::geom::Dp>,
    /// surgical patch — merges fieldwise within this build
    pub patch: crate::style::BoxStylePatch,
}

impl Alert {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn variant(mut self, v: AlertVariant) -> Self {
        self.variant = v;
        self
    }
    pub fn width(mut self, w: crate::geom::Dp) -> Self {
        self.width = Some(w);
        self
    }
    /// surgical patch — later `.style` calls deep-merge, `None` never erases
    pub fn style(mut self, p: crate::style::BoxStylePatch) -> Self {
        self.patch.merge(&p);
        self
    }
}

/// Skeleton props — a static `bg-muted` placeholder block. `width`/
/// `height` are `Length` specs (the frozen specimen fixes both: avatar
/// 40×40, lines 168×16 and 112×16); `circle` selects the `rounded-full`
/// radius for avatar shapes. No animation — reduced-motion product scope.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Skeleton {
    pub width: Length,
    pub height: Length,
    pub circle: bool,
}

impl Skeleton {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn width(mut self, w: Length) -> Self {
        self.width = w;
        self
    }
    pub fn height(mut self, h: Length) -> Self {
        self.height = h;
        self
    }
    /// fixed-size convenience — `width(Fixed(w)) + height(Fixed(h))`
    pub fn size(mut self, w: crate::geom::Dp, h: crate::geom::Dp) -> Self {
        self.width = Length::Fixed(w);
        self.height = Length::Fixed(h);
        self
    }
    /// `rounded-full` — the avatar shape (radius normalizes to the
    /// half-extent at paint)
    pub fn circle(mut self) -> Self {
        self.circle = true;
        self
    }
}

/// Checkbox props — the app model owns `checked`; `on_change` rides the
/// shared Press path (click + Space/Enter on the focused control).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Checkbox {
    pub checked: bool,
    pub disabled: bool,
    /// semantic `aria-invalid` — destructive border + ring
    pub invalid: bool,
    /// optional visible label painted at the frozen +24dp offset
    pub label: Option<&'static str>,
    /// accessible name when the visible label isn't the control name
    pub accessible_name: Option<&'static str>,
    /// presentation-only staged visual state (`Presented` docs)
    pub presented: Presented,
}

/// Switch props — same app-owned `checked` model; `size` selects the
/// frozen 32×18.4 track or the 24×14 `size-sm` track.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Switch {
    pub checked: bool,
    pub disabled: bool,
    pub invalid: bool,
    pub size: crate::theme::ToggleSize,
    pub label: Option<&'static str>,
    pub accessible_name: Option<&'static str>,
    /// presentation-only staged visual state (`Presented` docs)
    pub presented: Presented,
}

/// Radio-item options inside `ui.radio_group` — per-item `disabled` /
/// `invalid` overlays; `checked` derives from the group's `selected`.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct RadioItemOpts {
    pub disabled: bool,
    pub invalid: bool,
    /// presentation-only staged visual state (`Presented` docs)
    pub presented: Presented,
}

/// Field props — `orientation` picks the frozen vertical stack
/// (label/control/description) or the horizontal `content + control`
/// row; `width` pins the 288dp frozen box (unset = fill).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Field {
    pub orientation: crate::theme::FieldOrientation,
    pub width: Option<crate::geom::Dp>,
}

/// `ui.select` props — the app owns `open` + `selected` (key); `value`
/// is the trigger's display text (`None` paints `placeholder` in
/// `data-placeholder` muted-foreground). `automation_id` is REQUIRED —
/// it names the trigger AND is the popup's anchor identity. `width` is
/// the popup lane (frozen 208dp baseline); the trigger itself is w-fit.
#[derive(Clone, Debug, PartialEq)]
pub struct Select<'a> {
    pub automation_id: &'a str,
    /// popup chrome automation id — e.g. `select.default.open.content`
    pub content_id: Option<&'a str>,
    pub open: bool,
    /// key of the selected item — marks `item[key]` with the check
    /// indicator + `selected` semantics (the open focus lands on it)
    pub selected: Option<&'static str>,
    /// display text inside the trigger — `None` paints `placeholder`
    pub value: Option<&'a str>,
    pub placeholder: &'a str,
    pub disabled: bool,
    pub invalid: bool,
    pub size: ControlSize,
    /// popup lane width — `None` = the frozen 208dp baseline
    pub width: Option<crate::geom::Dp>,
    /// presentation-only staged visual state (`Presented` docs)
    pub presented: Presented,
}

impl<'a> Select<'a> {
    /// Minimal live select — `Select::new("select.env")`.
    pub fn new(automation_id: &'a str) -> Self {
        Select {
            automation_id,
            content_id: None,
            open: false,
            selected: None,
            value: None,
            placeholder: "Choose an option",
            disabled: false,
            invalid: false,
            size: ControlSize::default(),
            width: None,
            presented: Presented::default(),
        }
    }
}

/// Select open/selection events delivered through `on_change`.
pub enum SelectChange {
    /// open-state request — trigger press (`!open` at stage time, so a
    /// dismiss+press pair collapses to a close) or popup dismiss (`false`
    /// on outside-press/Escape)
    Open(bool),
    /// an option activated — click or Enter/Space on the live item;
    /// commit `selected`, close, focus returns to the trigger
    Pick(&'static str),
}

/// Per-item options inside `ui.select` content — `disabled` skips nav
/// and activation; `automation_id` gives the item its own element id
/// (specimen stages like `select.default.open.item-highlighted`).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct SelectItemOpts {
    pub disabled: bool,
    pub automation_id: Option<&'static str>,
    /// presentation-only staged visual state (`Presented` docs)
    pub presented: Presented,
}

/// `ui.tabs` props — `variant` picks the frozen list chrome, `selected`
/// is the app-owned active tab key.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Tabs<'a> {
    pub variant: TabsVariant,
    pub selected: &'a str,
}

/// Per-tab options — `disabled` is inert + skipped by directional nav;
/// `automation_id` gives the trigger its own element id.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct TabOpts {
    pub disabled: bool,
    pub automation_id: Option<&'static str>,
    /// presentation-only staged visual state (`Presented` docs)
    pub presented: Presented,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct DropdownMenu<'a> {
    pub automation_id: &'a str,
    pub content_id: Option<&'a str>,
    pub label: &'a str,
    pub open: bool,
    pub width: crate::geom::Dp,
}

impl<'a> DropdownMenu<'a> {
    pub fn new(automation_id: &'a str, label: &'a str) -> Self {
        Self {
            automation_id,
            content_id: None,
            label,
            open: false,
            width: crate::geom::dp(192.0),
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum DropdownMenuChange {
    Open(bool),
    Activate(&'static str),
    Checkbox(&'static str, bool),
    Radio(&'static str),
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum DropdownItemKind {
    #[default]
    Normal,
    Destructive,
    Checkbox {
        checked: bool,
    },
    Radio {
        selected: bool,
    },
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct DropdownItemOpts {
    pub kind: DropdownItemKind,
    pub disabled: bool,
    pub icon: Option<crate::icons::Icon>,
    pub shortcut: Option<&'static str>,
    pub automation_id: Option<&'static str>,
    pub presented: Presented,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct SettingsNav<'a> {
    pub automation_id: &'a str,
    pub collapsed: bool,
    pub selected: &'static str,
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct SettingsNavItemOpts {
    pub disabled: bool,
    pub automation_id: Option<&'static str>,
    pub presented: Presented,
}

// ------------------------- staging internals -------------------------------

/// Flat staged node — children are indices into `Tx.nodes`, so the whole
/// transaction is ONE reusable flat buffer, never a persistent second tree.
pub(crate) struct StagedNode {
    pub automation_id: Option<Rc<str>>,
    /// semantic part key inside the nearest automation-id owner —
    /// `ui.part("header", …)` / `ui.part("line", …)`; devtools emits it
    /// as `owner#part` (with parent part chains resolved)
    pub part_key: Option<Rc<str>>,
    /// semantic control role — set post-stage like `automation_id` by
    /// composite builders (combobox/option/tablist/tab)
    pub role: Option<SemanticRole>,
    pub key: ChildKey,
    pub data: NodeData,
    /// staged presentation-only visual state — builders set it
    /// post-stage like `automation_id` (never grants interactivity)
    pub presented: Presented,
    pub visibility: Visibility,
    pub layout: LayoutSpec,
    pub factories: EventFactorySet,
    /// adapters from enclosing `ui.scope` calls (leaf -> root)
    pub adapters: MsgAdapters,
    /// indices into `Tx.nodes`
    pub children: Vec<u32>,
}

impl StagedNode {
    /// Slot placeholder while a node is moved out of the flat buffer.
    pub(crate) fn placeholder() -> Self {
        StagedNode {
            automation_id: None,
            part_key: None,
            role: None,
            key: ChildKey::Static {
                kind: 0,
                ordinal: u32::MAX,
            },
            data: NodeData::Empty,
            presented: Presented::default(),
            visibility: Visibility::Hidden,
            layout: LayoutSpec::default(),
            factories: EventFactorySet::default(),
            adapters: Rc::new(Vec::new()),
            children: Vec::new(),
        }
    }

    pub(crate) fn kind_tag(&self) -> u8 {
        match &self.data {
            NodeData::Empty => 0,
            NodeData::Container { kind, .. } => *kind,
            NodeData::Label { .. } => crate::node::KIND_LABEL,
            NodeData::Button { .. } => crate::node::KIND_BUTTON,
            NodeData::Editor { multiline, .. } => {
                if *multiline {
                    crate::node::KIND_TEXT_AREA
                } else {
                    crate::node::KIND_TEXT_INPUT
                }
            }
            NodeData::Custom { .. } => crate::node::KIND_CUSTOM,
            NodeData::Action { .. } => crate::node::KIND_ACTION,
            NodeData::Icon { .. } => crate::node::KIND_ICON,
            NodeData::Toggle { .. } => crate::node::KIND_TOGGLE,
            NodeData::Popup { .. } => crate::node::KIND_POPUP,
        }
    }

    /// lease identity for the mounted-editor index / duplicate checks
    pub(crate) fn lease_id(&self) -> Option<usize> {
        match &self.data {
            NodeData::Editor { snapshot, .. } => Some(Rc::as_ptr(&snapshot.lease) as usize),
            _ => None,
        }
    }
}

/// One frame = one parent's staging context. `retained` is the matched
/// retained node the frame corresponds to — its children drive O(1) matching
/// and Rc reuse.
pub(crate) struct UiFrame {
    /// child indices into `Tx.nodes` (emission order)
    pub children: Vec<u32>,
    /// single counter for static (no explicit key) siblings — identity is
    /// (kind, emission ordinal)
    pub static_ordinals: u32,
    /// duplicate detection bucketed by key hash — real equality inside the
    /// bucket, never hash-as-identity
    pub key_buckets: HashMap<u64, Vec<ErasedKey>>,
    /// the retained node this frame is staging against (None = no prior)
    pub retained: Option<NodeId>,
    /// retained children bucketed by key hash — built lazily on first match
    pub retained_buckets: Option<HashMap<u64, Vec<u32>>>,
}

impl UiFrame {
    /// Allocate the next static-sibling identity: single emission counter +
    /// kind tag (NOT a per-kind counter).
    fn child_key(
        &mut self,
        kind: u8,
        explicit: Option<ErasedKey>,
    ) -> Result<ChildKey, UiDiagnostic> {
        match explicit {
            Some(k) => {
                let h = k.hash();
                if let Some(bucket) = self.key_buckets.get(&h) {
                    // true equality — collisions can't alias
                    if bucket.iter().any(|e| e.key_eq(&k)) {
                        return Err(UiDiagnostic::DuplicateKey);
                    }
                }
                self.key_buckets.entry(h).or_default().push(k.clone());
                Ok(ChildKey::Explicit(k))
            }
            None => {
                let o = self.static_ordinals;
                self.static_ordinals += 1;
                Ok(ChildKey::Static { kind, ordinal: o })
            }
        }
    }
}

/// Non-generic transaction staging — flat node buffer + frame stack; the
/// retained arena is read-only through `view` and used for matching + Rc
/// reuse. Buffers are cleared (capacity retained) each view pass.
pub(crate) struct Tx<'r> {
    /// >0 while an `ui.action` draw closure runs — actionable/native-peer
    /// children staged inside are InvalidComposition diagnostics
    pub(crate) action_depth: u32,
    /// flat staged node arena — `StagedNode.children` index into this
    pub nodes: Vec<StagedNode>,
    pub frames: Vec<UiFrame>,
    /// active scope-adapter chain (leaf -> root)
    pub adapters: MsgAdapters,
    /// current window theme (staged changes propagate at commit)
    pub theme: Theme,
    /// OS appearance — supplied by the runtime, independent of `theme`
    pub appearance: Appearance,
    /// diagnostics staged by builders (fatal structural errors)
    pub diagnostics: Vec<UiDiagnostic>,
    /// read-only retained arena for matching + `Rc<str>` reuse
    pub retained: &'r Arena,
}

impl Tx<'_> {
    /// The retained node a frame is staging against, and its children.
    fn retained_children(&self, frame: &UiFrame) -> &[u32] {
        frame
            .retained
            .and_then(|id| self.retained.get(id))
            .map(|n| n.children.as_slice())
            .unwrap_or(&[])
    }

    /// Find the retained child matching `key`/`kind` in the current frame —
    /// hash-bucketed, equality-confirmed; never a quadratic sibling scan.
    fn match_retained(&mut self, frame_idx: usize, key: &ChildKey, kind: u8) -> Option<NodeId> {
        let h = key.hash();
        if self.frames[frame_idx].retained_buckets.is_none() {
            let frame = &self.frames[frame_idx];
            let mut buckets: HashMap<u64, Vec<u32>> = HashMap::new();
            for &slot in self.retained_children(frame) {
                let g = self.retained.generation_of(slot);
                if let Some(n) = self.retained.get(NodeId {
                    slot,
                    generation: g,
                }) {
                    buckets.entry(n.key.hash()).or_default().push(slot);
                }
            }
            self.frames[frame_idx].retained_buckets = Some(buckets);
        }
        let buckets = self.frames[frame_idx].retained_buckets.as_ref().unwrap();
        for &slot in buckets.get(&h).into_iter().flatten() {
            let g = self.retained.generation_of(slot);
            if let Some(n) = self.retained.get(NodeId {
                slot,
                generation: g,
            }) && n.key.key_eq(key)
                && n.kind_tag() == kind
            {
                return Some(NodeId {
                    slot,
                    generation: g,
                });
            }
        }
        None
    }

    /// Stage a leaf: key allocation + retained match + push into flat buffer.
    /// `build` gets the matched retained node (for Rc reuse).
    pub(crate) fn stage_leaf(
        &mut self,
        kind: u8,
        explicit: Option<ErasedKey>,
        visibility: Visibility,
        layout: LayoutSpec,
        factories: EventFactorySet,
        build: impl FnOnce(Option<&crate::node::Node>) -> NodeData,
    ) {
        if let Some(d) = layout.validate() {
            self.diagnostics.push(d);
            return;
        }
        // action content is decorative — no actionable/focusable/native
        // peer children inside a semantic action
        if self.action_depth > 0
            && matches!(
                kind,
                crate::node::KIND_BUTTON
                    | crate::node::KIND_ACTION
                    | crate::node::KIND_TEXT_INPUT
                    | crate::node::KIND_TEXT_AREA
                    | crate::node::KIND_TOGGLE
                    | crate::node::KIND_POPUP
            )
        {
            self.diagnostics.push(UiDiagnostic::InvalidComposition);
            return;
        }
        let frame_idx = self.frames.len() - 1;
        let key = match self.frames[frame_idx].child_key(kind, explicit) {
            Ok(k) => k,
            Err(d) => {
                self.diagnostics.push(d);
                return;
            }
        };
        let retained_node = self
            .match_retained(frame_idx, &key, kind)
            .and_then(|id| self.retained.get(id));
        let data = build(retained_node);
        let idx = self.nodes.len() as u32;
        self.nodes.push(StagedNode {
            automation_id: None,
            part_key: None,
            role: None,
            key,
            data,
            presented: Presented::default(),
            visibility,
            layout,
            factories,
            adapters: self.adapters.clone(),
            children: Vec::new(),
        });
        self.frames[frame_idx].children.push(idx);
    }

    /// Stage a container: draw children into a fresh frame against the
    /// matched retained node, then append the container. `build` produces
    /// the node data (Container for layout kinds, Action for `ui.action`).
    fn stage_container<M: 'static>(
        &mut self,
        kind: u8,
        explicit: Option<ErasedKey>,
        props: ContainerProps,
        layout: LayoutSpec,
        factories: EventFactorySet,
        visibility: Visibility,
        draw: impl FnOnce(&mut Ui<'_, '_, M>),
        action_depth: bool,
        build: impl FnOnce(ContainerProps) -> NodeData,
    ) {
        if let Some(d) = layout.validate() {
            self.diagnostics.push(d);
            return;
        }
        // nested actionable inside an action's decorative content is a
        // structural diagnostic, not event bubbling
        if self.action_depth > 0
            && matches!(
                kind,
                crate::node::KIND_BUTTON
                    | crate::node::KIND_ACTION
                    | crate::node::KIND_TEXT_INPUT
                    | crate::node::KIND_TEXT_AREA
                    | crate::node::KIND_POPUP
            )
        {
            self.diagnostics.push(UiDiagnostic::InvalidComposition);
            return;
        }
        let frame_idx = self.frames.len() - 1;
        let key = match self.frames[frame_idx].child_key(kind, explicit) {
            Ok(k) => k,
            Err(d) => {
                self.diagnostics.push(d);
                return;
            }
        };
        let retained = self.match_retained(frame_idx, &key, kind);
        // push a child frame bound to the matched retained node
        self.frames.push(UiFrame {
            children: Vec::new(),
            static_ordinals: 0,
            retained_buckets: None,
            key_buckets: HashMap::new(),
            retained,
        });
        if action_depth {
            self.action_depth += 1;
        }
        // hand a Ui view of the same Tx to draw — self re-borrowed
        {
            let mut ui = Ui::<M> {
                tx: self,
                _m: std::marker::PhantomData,
            };
            draw(&mut ui);
        }
        if action_depth {
            self.action_depth -= 1;
        }
        let child_frame = self.frames.pop().unwrap();
        let data = build(props);
        // leaf kinds staged inside an action are checked at stage_leaf;
        // the container itself inside an action is still invalid
        let idx = self.nodes.len() as u32;
        self.nodes.push(StagedNode {
            automation_id: None,
            part_key: None,
            role: None,
            key,
            data,
            presented: Presented::default(),
            visibility,
            layout,
            factories,
            adapters: self.adapters.clone(),
            children: child_frame.children,
        });
        self.frames[frame_idx].children.push(idx);
    }
}

/// The traversal visitor passed to `view(&S, &mut Ui<M>)`. Nothing it stages
/// touches the retained arena until the transaction commits.
pub struct Ui<'ui, 'tx, M> {
    pub(crate) tx: &'tx mut Tx<'ui>,
    pub(crate) _m: std::marker::PhantomData<&'ui M>,
}

impl<'ui, 'tx, M: 'static> Ui<'ui, 'tx, M> {
    // ----- static-identity primitives --------------------------------------

    /// Explicit-keyed container — `group(key, |ui| ...)` stabilises
    /// conditional regions under the key's identity.
    pub fn group(&mut self, key: impl KeyId, draw: impl FnOnce(&mut Ui<'_, '_, M>)) {
        self.tx.stage_container(
            crate::node::KIND_GROUP,
            Some(ErasedKey::new(key)),
            ContainerProps::default(),
            LayoutSpec::default(),
            EventFactorySet::default(),
            Visibility::Visible,
            draw,
            false,
            |props| NodeData::Container {
                kind: crate::node::KIND_GROUP,
                props,
            },
        );
    }

    /// Assign a stable semantic ID to exactly one child produced by `draw`.
    /// This metadata is independent of retained/keyed identity.
    pub fn named(&mut self, id: &str, draw: impl FnOnce(&mut Ui<'_, '_, M>)) {
        let before = self.tx.frames.last().unwrap().children.len();
        draw(self);
        let children = &self.tx.frames.last().unwrap().children;
        if children.len() != before + 1 {
            self.tx.diagnostics.push(UiDiagnostic::InvalidComposition);
            return;
        }
        let index = children[before];
        let existing = self.tx.nodes[index as usize].automation_id.clone();
        match existing.as_deref() {
            // An id produced inside `draw` (e.g. a builder's `.automation_id`)
            // stays authoritative: identical is idempotent, a different id is
            // a staged conflict — diagnostic, never an overwrite.
            Some(existing) if existing == id => {}
            Some(_) => self
                .tx
                .diagnostics
                .push(UiDiagnostic::DuplicateAutomationId),
            None => self.tx.nodes[index as usize].automation_id = Some(Rc::from(id)),
        }
    }

    /// Mark exactly one child produced by `draw` as a semantic PART of the
    /// enclosing automation-id owner (`owner#part`). Part keys are scoped
    /// to the owner and chain with enclosing parts — `part("header")` +
    /// `part("title")` inside emits `header.title`. Generic container
    /// chrome only: the part marker never changes layout, painting, or
    /// semantics beyond snapshot identity.
    pub fn part(&mut self, key: &str, draw: impl FnOnce(&mut Ui<'_, '_, M>)) {
        let before = self.tx.frames.last().unwrap().children.len();
        draw(self);
        let children = &self.tx.frames.last().unwrap().children;
        if children.len() != before + 1 {
            self.tx.diagnostics.push(UiDiagnostic::InvalidComposition);
            return;
        }
        let index = children[before];
        let node = &mut self.tx.nodes[index as usize];
        if node.part_key.is_some() {
            self.tx.diagnostics.push(UiDiagnostic::InvalidComposition);
            return;
        }
        node.part_key = Some(Rc::from(key));
    }

    pub fn column(&mut self, props: Column, draw: impl FnOnce(&mut Ui<'_, '_, M>)) {
        self.tx.stage_container(
            crate::node::KIND_COLUMN,
            None,
            props.props(),
            LayoutSpec {
                width: props.width,
                height: props.height,
                ..Default::default()
            },
            EventFactorySet::default(),
            Visibility::Visible,
            draw,
            false,
            |props| NodeData::Container {
                kind: crate::node::KIND_COLUMN,
                props,
            },
        );
    }
    pub fn row(&mut self, props: Row, draw: impl FnOnce(&mut Ui<'_, '_, M>)) {
        self.tx.stage_container(
            crate::node::KIND_ROW,
            None,
            props.props(),
            LayoutSpec {
                width: props.width,
                height: props.height,
                ..Default::default()
            },
            EventFactorySet::default(),
            Visibility::Visible,
            draw,
            false,
            |props| NodeData::Container {
                kind: crate::node::KIND_ROW,
                props,
            },
        );
    }
    pub fn stack(&mut self, props: Stack, draw: impl FnOnce(&mut Ui<'_, '_, M>)) {
        self.tx.stage_container(
            crate::node::KIND_STACK,
            None,
            props.props(),
            LayoutSpec::default(),
            EventFactorySet::default(),
            Visibility::Visible,
            draw,
            false,
            |props| NodeData::Container {
                kind: crate::node::KIND_STACK,
                props,
            },
        );
    }
    pub fn surface(&mut self, props: Surface, draw: impl FnOnce(&mut Ui<'_, '_, M>)) {
        self.tx.stage_container(
            crate::node::KIND_SURFACE,
            None,
            props.props(),
            LayoutSpec::default(),
            EventFactorySet::default(),
            Visibility::Visible,
            draw,
            false,
            |props| NodeData::Container {
                kind: crate::node::KIND_SURFACE,
                props,
            },
        );
    }

    /// `ui.scroll` — vertical scroll viewport. `offset` is the app's
    /// committed scroll position (Dp); wheel and devctl scroll intents
    /// arrive as `ScrollOffset` deltas through `on_scroll`, producing `M`
    /// so `update` can own the state. The viewport fills the remaining
    /// main-axis space it is offered (`height: Fill(1)`); children stack
    /// as a column at unbounded height and clip to the viewport inner.
    pub fn scroll(
        &mut self,
        props: Scroll,
        offset: crate::geom::Dp,
        on_scroll: impl Fn(crate::event::ScrollOffset) -> M + 'static,
        draw: impl FnOnce(&mut Ui<'_, '_, M>),
    ) {
        let mut factories = EventFactorySet::default();
        factories.on_scroll = Some(Box::new(move |o| Box::new(on_scroll(o))));
        // the explicit argument is authoritative over any staged field
        let props = Scroll { offset, ..props };
        self.tx.stage_container(
            crate::node::KIND_SCROLL,
            None,
            props.props(),
            LayoutSpec {
                height: crate::geom::Length::Fill(1),
                ..Default::default()
            },
            factories,
            Visibility::Visible,
            draw,
            false,
            |props| NodeData::Container {
                kind: crate::node::KIND_SCROLL,
                props,
            },
        );
    }

    /// Card container — `bg-card`, `rounded-xl`, `ring-1
    /// ring-foreground/10`, top padding = `--card-spacing`. Column
    /// semantics: children are the card's parts (header/title/
    /// description/content/footer), spaced by the card-spacing token.
    /// Compose parts with generic `column`/`row`/`box_`/`part` — the
    /// card itself stays a plain styled container.
    pub fn card(&mut self, props: Card, draw: impl FnOnce(&mut Ui<'_, '_, M>)) {
        let layout = props
            .width
            .map_or_else(LayoutSpec::default, |w| LayoutSpec {
                width: crate::geom::Length::Fixed(w),
                ..Default::default()
            });
        self.tx.stage_container(
            crate::node::KIND_CARD,
            None,
            props.props(),
            layout,
            EventFactorySet::default(),
            Visibility::Visible,
            draw,
            false,
            |props| NodeData::Container {
                kind: crate::node::KIND_CARD,
                props,
            },
        );
    }

    /// `ui.separator` — the frozen `bg-border` rule: a decorative 1dp
    /// line in the `Border` role. `Horizontal` fills the offered width
    /// (stretch/fill), `Vertical` the offered height — use
    /// `Separator::length` or a stretch-aligned parent where no extent
    /// is offered.
    pub fn separator(&mut self, props: Separator) {
        self.tx.stage_container(
            crate::node::KIND_BOX,
            None,
            ContainerProps {
                full: Some(crate::style::separator_recipe()),
                ..Default::default()
            },
            props.layout(),
            EventFactorySet::default(),
            Visibility::Visible,
            |_: &mut Ui<'_, '_, M>| {},
            false,
            |props| NodeData::Container {
                kind: crate::node::KIND_BOX,
                props,
            },
        );
    }

    /// `ui.alert` — the frozen `cn-alert` container: card chrome,
    /// `rounded-lg` (10dp), 1dp border, padding-block 8 / inline 10.
    /// Compose the `icon`/`title`/`description` parts inside with
    /// `ui.part` + the `alert_*` style recipes.
    pub fn alert(&mut self, props: Alert, draw: impl FnOnce(&mut Ui<'_, '_, M>)) {
        let mut full = crate::style::alert_recipe(props.variant);
        full.patch(&props.patch);
        let layout = props
            .width
            .map_or_else(LayoutSpec::default, |w| LayoutSpec {
                width: Length::Fixed(w),
                ..Default::default()
            });
        self.tx.stage_container(
            crate::node::KIND_BOX,
            None,
            ContainerProps {
                full: Some(full),
                ..Default::default()
            },
            layout,
            EventFactorySet::default(),
            Visibility::Visible,
            draw,
            false,
            |props| NodeData::Container {
                kind: crate::node::KIND_BOX,
                props,
            },
        );
    }

    /// `ui.skeleton` — a static `bg-muted` placeholder block at an
    /// authored size (radius-md; `circle()` selects `rounded-full`).
    /// Deliberately no animation or timers — the frozen contract is
    /// static (reduced-motion product scope).
    pub fn skeleton(&mut self, props: Skeleton) {
        self.tx.stage_container(
            crate::node::KIND_BOX,
            None,
            ContainerProps {
                full: Some(crate::style::skeleton_recipe(props.circle)),
                ..Default::default()
            },
            LayoutSpec {
                width: props.width,
                height: props.height,
                ..Default::default()
            },
            EventFactorySet::default(),
            Visibility::Visible,
            |_: &mut Ui<'_, '_, M>| {},
            false,
            |props| NodeData::Container {
                kind: crate::node::KIND_BOX,
                props,
            },
        );
    }

    /// `ui.kbd_group` — the frozen `cn-kbd-group` inline row (gap-1,
    /// items-center). Children are `ui.kbd` caps marked as
    /// `ui.part("key[..]", ..)` parts of the group's automation owner.
    pub fn kbd_group(&mut self, draw: impl FnOnce(&mut Ui<'_, '_, M>)) {
        self.row(
            Row::new()
                .gap(Space::Xs)
                .align(Align::Center)
                .padding(Space::None),
            draw,
        );
    }

    // ----- leaves -----------------------------------------------------------

    /// Decorative vector icon leaf — Lucide stroke geometry at `size`.
    /// Inert (no interaction); an icon inside a button contributes the
    /// `icon` part, not a separate semantic node.
    pub fn icon(&mut self, icon: crate::icons::Icon) -> IconBuilder<'_, 'ui, M> {
        IconBuilder {
            automation_id: None,
            tx: self.tx,
            icon,
            size: crate::icons::Icon::DEFAULT_SIZE,
            color: crate::style::Color::Role(ColorRole::Foreground),
            visibility: Visibility::Visible,
            layout: LayoutSpec::default(),
            _m: std::marker::PhantomData,
        }
    }

    /// Icon-only button — one `Button` node with a decorative central
    /// icon, square size-class measure (`icon-xs/sm/icon/icon-lg`),
    /// and a REQUIRED accessible name (no visible text). `text` is the
    /// button's name; it is never painted.
    pub fn icon_button<'a>(
        &'a mut self,
        icon: crate::icons::Icon,
        text: &'a str,
    ) -> ButtonBuilder<'a, 'ui, M> {
        self.button(text)
            .icon_start(icon)
            .icon_only(true)
            .accessible_name(text)
    }

    /// `impl AsRef<str>` — an owned `String`/`format!` value or a `&str`
    /// both stage directly; the retained `Rc<str>` is reused when the text
    /// is unchanged, so borrowed strings cost no copy.
    pub fn label<'a, T: AsRef<str>>(&'a mut self, text: T) -> LabelBuilder<'a, 'ui, T, M> {
        LabelBuilder {
            automation_id: None,
            tx: self.tx,
            text,
            wrap: false,
            color_role: ColorRole::Foreground,
            patch: crate::style::TextStylePatch::default(),
            chrome: None,
            visibility: Visibility::Visible,
            layout: LayoutSpec::default(),
            factories: EventFactorySet::default(),
            _m: std::marker::PhantomData,
        }
    }

    /// `ui.badge` — a content-hugging `cn-badge` pill: the variant
    /// recipe's chrome + `text-xs` medium label. Returns the underlying
    /// `LabelBuilder` so automation ids and surgical text patches
    /// compose exactly as on `ui.label`. Not interactive — no press or
    /// hover states beyond the frozen `normal` capture.
    pub fn badge<'a, T: AsRef<str>>(
        &'a mut self,
        variant: BadgeVariant,
        text: T,
    ) -> LabelBuilder<'a, 'ui, T, M> {
        let vs = crate::style::badge_recipe(variant);
        self.label(text)
            .chrome(vs.box_style)
            .style(text_to_patch(&vs.text_style))
    }

    /// `ui.kbd` — a single `cn-kbd` key cap: `bg-muted` radius-sm box,
    /// `text-xs` medium `muted-foreground`, 20dp minimum width. Content-
    /// hugging — group caps with `ui.kbd_group` + `ui.part("key[..]",..)`.
    pub fn kbd<'a, T: AsRef<str>>(&'a mut self, text: T) -> LabelBuilder<'a, 'ui, T, M> {
        let vs = crate::style::kbd_recipe();
        self.label(text)
            .chrome(vs.box_style)
            .style(text_to_patch(&vs.text_style))
            .min_width(crate::geom::Dp(20.0))
    }

    pub fn button<'a>(&'a mut self, text: &'a str) -> ButtonBuilder<'a, 'ui, M> {
        ButtonBuilder {
            automation_id: None,
            tx: self.tx,
            text,
            variant: ButtonVariant::default(),
            style: ButtonStylePatch::default(),
            disabled: false,
            size: ControlSize::default(),
            motion: Some(MotionToken::Hover),
            tooltip: None,
            icon_start: None,
            icon_end: None,
            icon_only: false,
            accessible_name: None,
            invalid: false,
            selected: false,
            expanded: None,
            icon_color: None,
            presented: Presented::default(),
            visibility: Visibility::Visible,
            layout: LayoutSpec::default(),
            factories: EventFactorySet::default(),
            _m: std::marker::PhantomData,
        }
    }

    pub fn text_input<'a, 'b>(
        &'a mut self,
        value: &'b TextValue,
    ) -> TextInputBuilder<'a, 'ui, 'b, M> {
        TextInputBuilder {
            automation_id: None,
            tx: self.tx,
            value,
            multiline: false,
            placeholder: None,
            read_only: false,
            disabled: false,
            submit: SubmitPolicy::default(),
            max_lines: None,
            accessible_label: None,
            invalid: false,
            selection: None,
            line_height: None,
            presented: Presented::default(),
            patch: crate::style::TextInputStylePatch::default(),
            visibility: Visibility::Visible,
            layout: LayoutSpec::default(),
            factories: EventFactorySet::default(),
            _m: std::marker::PhantomData,
        }
    }

    pub fn text_area<'a, 'b>(
        &'a mut self,
        value: &'b TextValue,
    ) -> TextInputBuilder<'a, 'ui, 'b, M> {
        TextInputBuilder {
            automation_id: None,
            tx: self.tx,
            value,
            multiline: true,
            placeholder: None,
            read_only: false,
            disabled: false,
            submit: SubmitPolicy::None,
            max_lines: None,
            accessible_label: None,
            invalid: false,
            selection: None,
            line_height: None,
            presented: Presented::default(),
            patch: crate::style::TextInputStylePatch::default(),
            visibility: Visibility::Visible,
            layout: LayoutSpec::default(),
            factories: EventFactorySet::default(),
            _m: std::marker::PhantomData,
        }
    }

    pub fn custom(&mut self, render: Rc<dyn CustomRender>) -> CustomBuilder<'_, 'ui, M> {
        CustomBuilder {
            automation_id: None,
            tx: self.tx,
            render,
            frame_events: false,
            visibility: Visibility::Visible,
            layout: LayoutSpec::default(),
            factories: EventFactorySet::default(),
            _m: std::marker::PhantomData,
        }
    }

    // ----- scoped composition ------------------------------------------------

    /// `ui.scope(key, map, draw)` — a child message namespace: the child's
    /// `M2` is lifted to `M` through `map` at dispatch.
    pub fn scope<ChildMsg: 'static>(
        &mut self,
        key: impl KeyId,
        map: impl Fn(ChildMsg) -> M + 'static,
        draw: impl FnOnce(&mut Ui<'_, '_, ChildMsg>),
    ) {
        // adapter chain for children of this scope
        let parent_adapters = self.tx.adapters.clone();
        let mut lifted: Vec<Rc<dyn Fn(Box<dyn Any>) -> Box<dyn Any>>> =
            Vec::with_capacity(parent_adapters.len() + 1);
        lifted.push(Rc::new(move |child| {
            Box::new(map(*child.downcast::<ChildMsg>().unwrap())) as Box<dyn Any>
        }));
        for a in parent_adapters.iter() {
            lifted.push(a.clone());
        }
        let child_adapters: MsgAdapters = Rc::new(lifted);

        // stage the scope container in the parent frame
        let frame_idx = self.tx.frames.len() - 1;
        let key = match self.tx.frames[frame_idx]
            .child_key(crate::node::KIND_SCOPE, Some(ErasedKey::new(key)))
        {
            Ok(k) => k,
            Err(d) => {
                self.tx.diagnostics.push(d);
                return;
            }
        };
        let retained = self
            .tx
            .match_retained(frame_idx, &key, crate::node::KIND_SCOPE);
        self.tx.frames.push(UiFrame {
            children: Vec::new(),
            static_ordinals: 0,
            retained_buckets: None,
            key_buckets: HashMap::new(),
            retained,
        });
        // child's adapters swapped in for the draw body
        let saved = std::mem::replace(&mut self.tx.adapters, child_adapters);
        {
            let mut child_ui = Ui::<ChildMsg> {
                tx: self.tx,
                _m: std::marker::PhantomData,
            };
            draw(&mut child_ui);
        }
        self.tx.adapters = saved;
        let child_frame = self.tx.frames.pop().unwrap();
        let idx = self.tx.nodes.len() as u32;
        self.tx.nodes.push(StagedNode {
            automation_id: None,
            part_key: None,
            role: None,
            key,
            data: NodeData::Container {
                kind: crate::node::KIND_SCOPE,
                props: ContainerProps::default(),
            },
            presented: Presented::default(),
            visibility: Visibility::Visible,
            layout: LayoutSpec::default(),
            factories: EventFactorySet::default(),
            adapters: self.tx.adapters.clone(),
            children: child_frame.children,
        });
        self.tx.frames[frame_idx].children.push(idx);
    }

    /// `keyed(items, key, draw)` — an implicit static-ordinal list scope (two
    /// adjacent `keyed` calls are distinct siblings), then a keyed scope per
    /// item inside it.
    pub fn keyed<T, K: Eq + std::hash::Hash + Clone + 'static>(
        &mut self,
        items: &[T],
        key: impl Fn(&T) -> K,
        draw: impl Fn(&mut Ui<'_, '_, M>, &T),
    ) {
        // implicit scope by static emission ordinal — no fixed constant key,
        // so adjacent `keyed` lists in one parent stay distinct
        self.tx.stage_container(
            crate::node::KIND_GROUP,
            None,
            ContainerProps::default(),
            LayoutSpec::default(),
            EventFactorySet::default(),
            Visibility::Visible,
            |ui: &mut Ui<'_, '_, M>| {
                for item in items {
                    let k = ErasedKey::new(key(item));
                    ui.tx.stage_container(
                        crate::node::KIND_GROUP,
                        Some(k),
                        ContainerProps::default(),
                        LayoutSpec::default(),
                        EventFactorySet::default(),
                        Visibility::Visible,
                        |u| draw(u, item),
                        false,
                        |props| NodeData::Container {
                            kind: crate::node::KIND_GROUP,
                            props,
                        },
                    );
                }
            },
            false,
            |props| NodeData::Container {
                kind: crate::node::KIND_GROUP,
                props,
            },
        );
    }

    /// `ui.box_` — noninteractive painted box container; `BoxProps` carries
    /// the authored full `BoxStyle` plus layout inputs (size stays layout).
    pub fn box_(&mut self, props: BoxProps, draw: impl FnOnce(&mut Ui<'_, '_, M>)) {
        let cp = ContainerProps {
            full: Some(props.style),
            ..Default::default()
        };
        self.tx.stage_container(
            crate::node::KIND_BOX,
            None,
            cp,
            props.layout,
            EventFactorySet::default(),
            props.visibility,
            draw,
            false,
            |props| NodeData::Container {
                kind: crate::node::KIND_BOX,
                props,
            },
        );
    }

    /// `ui.action` — semantic activation wrapper over arbitrary decorative
    /// content. The action node is the single hit/focus/press owner; nested
    /// actionable or native-peer children are InvalidComposition.
    pub fn action<'a>(
        &'a mut self,
        props: Action,
        draw: impl FnOnce(&mut Ui<'_, '_, M>),
    ) -> ActionBuilder<'a, 'ui, M> {
        let before = self.tx.nodes.len();
        self.tx.stage_container(
            crate::node::KIND_ACTION,
            None,
            ContainerProps::default(),
            LayoutSpec::default(),
            EventFactorySet::default(),
            Visibility::Visible,
            draw,
            true,
            move |_| NodeData::Action {
                label: Rc::from(props.label.as_str()),
                style: props.style,
                disabled: props.disabled,
                checked: None,
            },
        );
        // the staged node is the last pushed (containers append last)
        let staged = (self.tx.nodes.len() > before).then_some(self.tx.nodes.len() as u32 - 1);
        ActionBuilder {
            tx: self.tx,
            staged,
            _m: std::marker::PhantomData,
        }
    }

    /// `ui.popover` — the generic anchored overlay layer. Stages a
    /// KIND_POPUP node inline ONLY when `open`; closed stages nothing.
    /// Placement is backend-resolved against the `anchor` node's laid-out
    /// rect each pass (flip-on-overflow + viewport clamp, shared policy in
    /// `crate::layout::resolve_popup_placement`). `dismiss_light`
    /// (default on) emits `on_open_change(false)` on an outside press;
    /// Escape dismisses the topmost open popup; `focus_on_open` (default
    /// on) moves focus to the first eligible descendant and restores the
    /// pre-open focus on close. `modal` paints a dim scrim and gates
    /// hit/focus/scroll to the popup subtree.
    ///
    /// Constraint: do NOT stage a `popover` inside an `ui.action`'s draw
    /// closure — `inside_action` covers the whole subtree and would make
    /// the popup inert (arena-live but unhittable). Popups anchor against
    /// sibling nodes by automation id instead.
    pub fn popover<'a>(
        &'a mut self,
        open: bool,
        draw: impl FnOnce(&mut Ui<'_, '_, M>),
    ) -> PopupBuilder<'a, 'ui, M> {
        let before = self.tx.nodes.len();
        if open {
            self.tx.stage_container(
                crate::node::KIND_POPUP,
                None,
                ContainerProps::default(),
                LayoutSpec::default(),
                EventFactorySet::default(),
                Visibility::Visible,
                draw,
                false,
                |props| NodeData::Popup {
                    anchor: None,
                    side: crate::geom::Side::default(),
                    align: Align::Center,
                    offset: Point::default(),
                    role: crate::node::PopupRole::default(),
                    modal: false,
                    dismiss_light: true,
                    focus_on_open: true,
                    props,
                },
            );
        }
        let staged = (self.tx.nodes.len() > before).then_some(self.tx.nodes.len() as u32 - 1);
        PopupBuilder {
            tx: self.tx,
            staged,
            _m: std::marker::PhantomData,
        }
    }

    /// CORE typography specimen — the frozen `typography.<kind>` recipe
    /// (style + chrome + wrap) applied to a display text leaf.
    /// CodeBlock is composite — use `ui.code_block`.
    pub fn typography<'a>(
        &'a mut self,
        kind: crate::typography::Typography,
        text: &'a str,
    ) -> TextBuilder<'a, 'ui, &'a str, M> {
        let mut b = self.text(text).style(kind.style()).wrap(kind.wraps());
        if let Some(c) = kind.chrome() {
            b = b.chrome(c);
        }
        // inline-code is an inline content-hugging box — it must not
        // stretch to its parent's width in a Stretch-aligned column
        if kind == crate::typography::Typography::InlineCode {
            b = b.width(crate::geom::Length::Content);
        }
        b
    }

    /// `typography.code-block` — mono code lines inside a `Surface`-role
    /// rounded container (`radius-2xl` 18dp). Semantic parts: `pre`
    /// marks the container itself (the frozen `pre` part rect is the
    /// element rect) and `pre.line[N]` the 1-based code lines. Attach
    /// the specimen's `automation_id` through `ui.named` — it lands on
    /// the outer container.
    pub fn code_block<'a>(&'a mut self, lines: &[&'a str]) {
        let chrome = crate::style::BoxStyle {
            background: crate::style::Color::Role(ColorRole::Code),
            border: crate::style::Border::default(),
            radii: crate::style::CornerRadii::all(crate::geom::Dp(18.0)),
            padding: crate::style::Insets {
                top: crate::geom::Dp(14.0),
                right: crate::geom::Dp(16.0),
                bottom: crate::geom::Dp(14.0),
                left: crate::geom::Dp(16.0),
            },
            shadow: None,
        };
        let mono = crate::style::TextStyle {
            font: crate::theme::FontFamily::Mono,
            size: crate::style::TextSize::Exact(crate::geom::Dp(14.0)),
            weight: crate::style::TextWeight::Normal,
            foreground: crate::style::Color::Role(ColorRole::CodeForeground),
            line_height: Some(crate::geom::Dp(20.0)),
            ..crate::style::TextStyle::default()
        };
        self.part("pre", |ui| {
            ui.box_(BoxProps::new().style(chrome), |ui| {
                // zero-pad/zero-gap — the chrome owns the frozen 14/16
                // insets; lines stack at their 20dp line-height
                ui.column(Column::new().padding(Space::None).gap(Space::None), |ui| {
                    for (i, line) in lines.iter().enumerate() {
                        let key = format!("line[{}]", i + 1);
                        ui.part(&key, |ui| {
                            ui.text(*line).style(mono);
                        });
                    }
                });
            });
        });
    }

    /// `ui.text` — styled text leaf; `label` is a recipe convenience over
    /// the same renderer.
    pub fn text<'a, T: AsRef<str>>(&'a mut self, text: T) -> TextBuilder<'a, 'ui, T, M> {
        TextBuilder {
            automation_id: None,
            tx: self.tx,
            text,
            wrap: false,
            style: crate::style::TextStyle::default(),
            chrome: None,
            visibility: Visibility::Visible,
            layout: LayoutSpec::default(),
            _m: std::marker::PhantomData,
        }
    }

    /// Stage a theme override — propagated to the runtime at commit.
    pub fn theme(&mut self, theme: Theme) {
        self.tx.theme = theme;
    }

    /// Actual OS appearance — supplied by the runtime, not derived from the
    /// selected theme.
    pub fn appearance(&self) -> Appearance {
        self.tx.appearance
    }

    /// The active window theme.
    pub fn theme_ref(&self) -> &Theme {
        &self.tx.theme
    }
}

/// Project a full recipe `TextStyle` into a `TextStylePatch` — the same
/// conversion `TextBuilder` applies when a primitive hands the label
/// pipeline a complete style (`TextSize::Body` stays unset so the label
/// recipe's base size survives).
fn text_to_patch(ts: &crate::style::TextStyle) -> crate::style::TextStylePatch {
    let mut p = crate::style::TextStylePatch::default();
    p.foreground = Some(ts.foreground);
    if let crate::style::TextSize::Exact(d) = ts.size {
        p.size = Some(d);
    }
    p.weight = Some(ts.weight);
    p.font = Some(ts.font);
    p.line_height = ts.line_height;
    p.underline = Some(ts.underline);
    p.letter_spacing = Some(ts.letter_spacing);
    p
}

// ============================ builders ======================================

/// Every builder stages on `Drop` — `.on_*(...)` setters register factories
/// retained at commit only.
pub struct LabelBuilder<'a, 'ui, T: AsRef<str>, M: 'static> {
    automation_id: Option<Rc<str>>,
    tx: &'a mut Tx<'ui>,
    text: T,
    wrap: bool,
    color_role: ColorRole,
    patch: crate::style::TextStylePatch,
    /// optional painted chrome under the text (rule lines, inline-code
    /// surface) — a full authored `BoxStyle`, no recipe merge
    chrome: Option<crate::style::BoxStyle>,
    visibility: Visibility,
    layout: LayoutSpec,
    factories: EventFactorySet,
    _m: std::marker::PhantomData<fn() -> M>,
}

impl<'a, 'ui, T: AsRef<str>, M: 'static> LabelBuilder<'a, 'ui, T, M> {
    pub fn automation_id(mut self, id: &str) -> Self {
        self.automation_id = Some(Rc::from(id));
        self
    }

    pub fn wrap(mut self, wrap: bool) -> Self {
        self.wrap = wrap;
        self
    }
    /// typed shorthand — sets only the recipe patch's foreground role
    pub fn color_role(mut self, r: ColorRole) -> Self {
        self.color_role = r;
        self
    }
    /// surgical text patch — merges fieldwise within this build
    pub fn style(mut self, p: crate::style::TextStylePatch) -> Self {
        self.patch.merge(&p);
        self
    }
    /// painted chrome under the text — the h2 rule line / inline-code
    /// surface. Authored full style; insets the text measure.
    pub fn chrome(mut self, c: crate::style::BoxStyle) -> Self {
        self.chrome = Some(c);
        self
    }
    pub fn visibility(mut self, v: Visibility) -> Self {
        self.visibility = v;
        self
    }
    pub fn layout(mut self, l: LayoutSpec) -> Self {
        self.layout = l;
        self
    }
    pub fn width(mut self, w: crate::geom::Length) -> Self {
        self.layout.width = w;
        self
    }
    pub fn height(mut self, h: crate::geom::Length) -> Self {
        self.layout.height = h;
        self
    }
    pub fn min_width(mut self, d: crate::geom::Dp) -> Self {
        self.layout.min_width = Some(d);
        self
    }
    pub fn max_width(mut self, d: crate::geom::Dp) -> Self {
        self.layout.max_width = Some(d);
        self
    }
}

impl<'a, 'ui, T: AsRef<str>, M: 'static> Drop for LabelBuilder<'a, 'ui, T, M> {
    fn drop(&mut self) {
        let before = self.tx.frames.last().unwrap().children.len();
        let text = self.text.as_ref();
        let wrap = self.wrap;
        let role = self.color_role;
        let chrome = self.chrome;
        let mut patch = self.patch;
        // color_role shorthand populates the patch's foreground only when
        // the consumer didn't set one explicitly
        if patch.foreground.is_none() {
            patch.foreground = Some(crate::style::Color::Role(role));
        }
        self.tx.stage_leaf(
            crate::node::KIND_LABEL,
            None,
            self.visibility,
            self.layout,
            std::mem::take(&mut self.factories),
            move |retained| {
                // reuse the retained Rc<str> when text is unchanged — no
                // per-view copy
                let text = match retained {
                    Some(crate::node::Node {
                        data: NodeData::Label { text: old, .. },
                        ..
                    }) if &**old == text => old.clone(),
                    _ => Rc::from(text),
                };
                NodeData::Label {
                    text,
                    wrap,
                    color_role: role,
                    patch,
                    chrome,
                }
            },
        );
        if let Some(id) = self.automation_id.take()
            && self.tx.frames.last().unwrap().children.len() > before
            && let Some(index) = self.tx.frames.last().unwrap().children.last().copied()
        {
            self.tx.nodes[index as usize].automation_id = Some(id);
        }
    }
}

pub struct ButtonBuilder<'a, 'ui, M: 'static> {
    automation_id: Option<Rc<str>>,
    tx: &'a mut Tx<'ui>,
    text: &'a str,
    variant: ButtonVariant,
    style: ButtonStylePatch,
    disabled: bool,
    size: ControlSize,
    motion: Option<MotionToken>,
    tooltip: Option<Rc<str>>,
    icon_start: Option<crate::icons::Icon>,
    icon_end: Option<crate::icons::Icon>,
    icon_only: bool,
    accessible_name: Option<Rc<str>>,
    invalid: bool,
    selected: bool,
    expanded: Option<bool>,
    icon_color: Option<crate::style::Color>,
    presented: Presented,
    visibility: Visibility,
    layout: LayoutSpec,
    factories: EventFactorySet,
    _m: std::marker::PhantomData<fn() -> M>,
}

impl<'a, 'ui, M: 'static> ButtonBuilder<'a, 'ui, M> {
    pub fn automation_id(mut self, id: &str) -> Self {
        self.automation_id = Some(Rc::from(id));
        self
    }

    pub fn variant(mut self, v: ButtonVariant) -> Self {
        self.variant = v;
        self
    }
    pub fn size(mut self, s: ControlSize) -> Self {
        self.size = s;
        self
    }
    /// surgical patch — merges fieldwise into the same build's patch;
    /// each view pass rebuilds from declaration (no history)
    pub fn style(mut self, s: ButtonStylePatch) -> Self {
        self.style.merge(&s);
        self
    }
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
    pub fn motion(mut self, t: Option<MotionToken>) -> Self {
        self.motion = t;
        self
    }
    /// neutral tooltip — semantic delayed show, reduced-motion-aware
    pub fn tooltip(mut self, text: &str) -> Self {
        self.tooltip = Some(Rc::from(text));
        self
    }
    /// decorative Lucide icon at the start edge (inline-start) — inert,
    /// styled by the button recipe's foreground
    pub fn icon_start(mut self, icon: crate::icons::Icon) -> Self {
        self.icon_start = Some(icon);
        self
    }
    /// decorative Lucide icon at the end edge (inline-end)
    pub fn icon_end(mut self, icon: crate::icons::Icon) -> Self {
        self.icon_end = Some(icon);
        self
    }
    /// icon-only control — `ui.icon_button` sets this; square
    /// size-class measure, no visible text
    pub(crate) fn icon_only(mut self, on: bool) -> Self {
        self.icon_only = on;
        self
    }
    /// accessible name used when the visible text isn't the control
    /// name — required on icon-only buttons (the button name, not the
    /// icon file)
    pub fn accessible_name(mut self, name: &str) -> Self {
        self.accessible_name = Some(Rc::from(name));
        self
    }
    /// semantic `aria-invalid` — destructive chrome; orthogonal to
    /// disabled/hover/pressed/focus
    pub fn invalid(mut self, on: bool) -> Self {
        self.invalid = on;
        self
    }
    /// semantic `data-active`/`aria-selected` — the recipe's selected
    /// overlay (variant-dependent); `focus_on_open` popups prefer a
    /// `selected` descendant for the open focus
    pub fn selected(mut self, on: bool) -> Self {
        self.selected = on;
        self
    }
    /// expanded/collapsed semantics for a popup-owning trigger
    pub fn expanded(mut self, expanded: Option<bool>) -> Self {
        self.expanded = expanded;
        self
    }
    /// icon tint override — `None` paints icons in the resolved
    /// foreground (`currentColor`); the select chevron keeps
    /// `muted-foreground` against a filled value
    pub fn icon_color(mut self, c: Option<crate::style::Color>) -> Self {
        self.icon_color = c;
        self
    }
    /// presentation-only staged visual state — paints/lays out exactly
    /// like the runtime state it mirrors; never grants real focus,
    /// hover, or interactivity
    pub fn presented(mut self, p: Presented) -> Self {
        self.presented = p;
        self
    }
    pub fn visibility(mut self, v: Visibility) -> Self {
        self.visibility = v;
        self
    }
    pub fn layout(mut self, l: LayoutSpec) -> Self {
        self.layout = l;
        self
    }
    pub fn width(mut self, w: crate::geom::Length) -> Self {
        self.layout.width = w;
        self
    }
    pub fn height(mut self, h: crate::geom::Length) -> Self {
        self.layout.height = h;
        self
    }
    pub fn on_press(mut self, f: impl Fn() -> M + 'static) -> Self {
        self.factories.on_press = Some(Box::new(move || Box::new(f())));
        self
    }
    pub fn on_pointer_enter(mut self, f: impl Fn(PointerEvent) -> M + 'static) -> Self {
        self.factories.on_pointer_enter = Some(Box::new(move |e| Box::new(f(e))));
        self
    }
    pub fn on_pointer_leave(mut self, f: impl Fn(PointerEvent) -> M + 'static) -> Self {
        self.factories.on_pointer_leave = Some(Box::new(move |e| Box::new(f(e))));
        self
    }
    pub fn on_pointer_down(mut self, f: impl Fn(PointerEvent) -> M + 'static) -> Self {
        self.factories.on_pointer_down = Some(Box::new(move |e| Box::new(f(e))));
        self
    }
    pub fn on_pointer_up(mut self, f: impl Fn(PointerEvent) -> M + 'static) -> Self {
        self.factories.on_pointer_up = Some(Box::new(move |e| Box::new(f(e))));
        self
    }
    pub fn on_focus(mut self, f: impl Fn() -> M + 'static) -> Self {
        self.factories.on_focus = Some(Box::new(move || Box::new(f())));
        self
    }
    pub fn on_blur(mut self, f: impl Fn() -> M + 'static) -> Self {
        self.factories.on_blur = Some(Box::new(move || Box::new(f())));
        self
    }
    pub fn on_key(mut self, f: impl Fn(KeyEvent) -> Option<M> + 'static) -> Self {
        self.factories.on_key = Some(Box::new(move |e| f(e).map(|m| Box::new(m) as _)));
        self
    }
}

impl<'a, 'ui, M: 'static> Drop for ButtonBuilder<'a, 'ui, M> {
    fn drop(&mut self) {
        let before = self.tx.frames.last().unwrap().children.len();
        let text = self.text;
        let (
            variant,
            style,
            disabled,
            size,
            motion,
            tooltip,
            icon_start,
            icon_end,
            icon_only,
            accessible_name,
            invalid,
            selected,
            expanded,
            icon_color,
        ) = (
            self.variant,
            self.style,
            self.disabled,
            self.size,
            self.motion,
            self.tooltip.clone(),
            self.icon_start,
            self.icon_end,
            self.icon_only,
            self.accessible_name.clone(),
            self.invalid,
            self.selected,
            self.expanded,
            self.icon_color,
        );
        let presented = self.presented;
        self.tx.stage_leaf(
            crate::node::KIND_BUTTON,
            None,
            self.visibility,
            self.layout,
            std::mem::take(&mut self.factories),
            move |retained| {
                let text = match retained {
                    Some(crate::node::Node {
                        data: NodeData::Button { text: old, .. },
                        ..
                    }) if &**old == text => old.clone(),
                    _ => Rc::from(text),
                };
                NodeData::Button {
                    text,
                    variant,
                    style,
                    disabled,
                    size,
                    motion,
                    tooltip,
                    icon_start,
                    icon_end,
                    icon_only,
                    accessible_name,
                    invalid,
                    selected,
                    expanded,
                    icon_color,
                }
            },
        );
        if let Some(index) = self
            .tx
            .frames
            .last()
            .unwrap()
            .children
            .last()
            .copied()
            .filter(|_| self.tx.frames.last().unwrap().children.len() > before)
        {
            if let Some(id) = self.automation_id.take() {
                self.tx.nodes[index as usize].automation_id = Some(id);
            }
            self.tx.nodes[index as usize].presented = presented;
        }
    }
}

/// `text_input`/`text_area` share one builder — `multiline` picks the peer.
pub struct TextInputBuilder<'a, 'ui, 'b, M: 'static> {
    automation_id: Option<Rc<str>>,
    tx: &'a mut Tx<'ui>,
    value: &'b TextValue,
    multiline: bool,
    placeholder: Option<Rc<str>>,
    read_only: bool,
    disabled: bool,
    submit: SubmitPolicy,
    max_lines: Option<u32>,
    accessible_label: Option<Rc<str>>,
    /// semantic `aria-invalid` — unconditional destructive ring
    invalid: bool,
    /// staged selection — UTF-16 `[start, end)`
    selection: Option<(usize, usize)>,
    /// authored exact line box — `None` keeps the font's natural height
    line_height: Option<crate::geom::Dp>,
    presented: Presented,
    /// capability-limited patch — chrome paints the frame, `foreground`
    /// is the only peer-routed field (approved boundary: no size/weight)
    patch: crate::style::TextInputStylePatch,
    visibility: Visibility,
    layout: LayoutSpec,
    factories: EventFactorySet,
    _m: std::marker::PhantomData<fn(&'ui (), &'b ()) -> M>,
}

impl<'a, 'ui, 'b, M: 'static> TextInputBuilder<'a, 'ui, 'b, M> {
    pub fn automation_id(mut self, id: &str) -> Self {
        self.automation_id = Some(Rc::from(id));
        self
    }

    /// capability-limited patch — chrome (`background`/`border`/`radii`/
    /// `padding`/`shadow`) paints the frame; `foreground` is routed to the
    /// peer through the typed adapter. Repeated `.style(..)` merges
    /// fieldwise — `None` preserves, `Some` wins.
    pub fn style(mut self, p: crate::style::TextInputStylePatch) -> Self {
        self.patch.merge(&p);
        self
    }
    /// semantic `aria-invalid` — the frozen invalid ring is unconditional
    /// (`ring-destructive` always-on); orthogonal to disabled/focus
    pub fn invalid(mut self, on: bool) -> Self {
        self.invalid = on;
        self
    }
    /// staged selection — UTF-16 `[start, end)` applied to the peer while
    /// staged; omit to leave selection to native/user control
    pub fn selection(mut self, start: usize, end: usize) -> Self {
        self.selection = Some((start, end));
        self
    }
    /// exact line box — the authored Dp count is calibrated into the
    /// peer font (D2DDWRITE ignores paragraph line-spacing; pitch comes
    /// from `CHARFORMAT.yHeight`). Omit for natural height. Mount-time
    /// only: not diffed/applied on update — a changed value needs a
    /// remount (same as `multiline`)
    pub fn line_height(mut self, h: crate::geom::Dp) -> Self {
        self.line_height = Some(h);
        self
    }
    /// presentation-only staged visual state (`Presented` docs)
    pub fn presented(mut self, p: Presented) -> Self {
        self.presented = p;
        self
    }
    pub fn placeholder(mut self, p: &str) -> Self {
        self.placeholder = Some(Rc::from(p));
        self
    }
    pub fn read_only(mut self, r: bool) -> Self {
        self.read_only = r;
        self
    }
    /// accessible name for the editable control (screen readers)
    pub fn label(mut self, l: &str) -> Self {
        self.accessible_label = Some(Rc::from(l));
        self
    }
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
    pub fn submit_policy(mut self, p: SubmitPolicy) -> Self {
        self.submit = p;
        self
    }
    pub fn max_lines(mut self, n: u32) -> Self {
        self.max_lines = Some(n);
        self
    }
    pub fn visibility(mut self, v: Visibility) -> Self {
        self.visibility = v;
        self
    }
    pub fn layout(mut self, l: LayoutSpec) -> Self {
        self.layout = l;
        self
    }
    pub fn width(mut self, w: crate::geom::Length) -> Self {
        self.layout.width = w;
        self
    }
    pub fn height(mut self, h: crate::geom::Length) -> Self {
        self.layout.height = h;
        self
    }
    /// committed edits — the payload is the actual `TextEdit`; the app
    /// accepts it via `value.accept(edit)` inside `update`.
    pub fn on_edit(mut self, f: impl Fn(TextEdit) -> M + 'static) -> Self {
        self.factories.on_edit = Some(Box::new(move |e| Box::new(f(e))));
        self
    }
    pub fn on_submit(mut self, f: impl Fn() -> M + 'static) -> Self {
        self.factories.on_submit = Some(Box::new(move || Box::new(f())));
        self
    }
    pub fn on_conflict(mut self, f: impl Fn(TextConflict) -> M + 'static) -> Self {
        self.factories.on_conflict = Some(Box::new(move |e| Box::new(f(e))));
        self
    }
    pub fn on_selection_changed(mut self, f: impl Fn(TextSelection) -> M + 'static) -> Self {
        self.factories.on_selection_changed = Some(Box::new(move |e| Box::new(f(e))));
        self
    }
}

impl<'a, 'ui, 'b, M: 'static> Drop for TextInputBuilder<'a, 'ui, 'b, M> {
    fn drop(&mut self) {
        let before = self.tx.frames.last().unwrap().children.len();
        let kind = if self.multiline {
            crate::node::KIND_TEXT_AREA
        } else {
            crate::node::KIND_TEXT_INPUT
        };
        // staged snapshot: Rc clones of the value's committed/pending — the
        // peer sync itself is carried by the retained node at commit
        let snapshot = self.value.snapshot();
        let (
            placeholder,
            accessible_label,
            read_only,
            submit,
            max_lines,
            disabled,
            invalid,
            multiline,
            patch,
            selection,
            line_height,
        ) = (
            self.placeholder.clone(),
            self.accessible_label.clone(),
            self.read_only,
            self.submit,
            self.max_lines,
            self.disabled,
            self.invalid,
            self.multiline,
            self.patch,
            self.selection,
            self.line_height,
        );
        let presented = self.presented;
        self.tx.stage_leaf(
            kind,
            None,
            self.visibility,
            self.layout,
            std::mem::take(&mut self.factories),
            move |_| NodeData::Editor {
                snapshot,
                multiline,
                read_only,
                disabled,
                submit,
                max_lines,
                placeholder,
                accessible_label,
                patch,
                invalid,
                selection,
                line_height,
                sync: Default::default(),
            },
        );
        if let Some(index) = self
            .tx
            .frames
            .last()
            .unwrap()
            .children
            .last()
            .copied()
            .filter(|_| self.tx.frames.last().unwrap().children.len() > before)
        {
            if let Some(id) = self.automation_id.take() {
                self.tx.nodes[index as usize].automation_id = Some(id);
            }
            self.tx.nodes[index as usize].presented = presented;
        }
    }
}

/// Custom leaf — immutable `Rc<dyn CustomRender>` snapshot.
pub struct CustomBuilder<'a, 'ui, M: 'static> {
    automation_id: Option<Rc<str>>,
    tx: &'a mut Tx<'ui>,
    render: Rc<dyn CustomRender>,
    frame_events: bool,
    visibility: Visibility,
    layout: LayoutSpec,
    factories: EventFactorySet,
    _m: std::marker::PhantomData<fn() -> M>,
}

impl<'a, 'ui, M: 'static> CustomBuilder<'a, 'ui, M> {
    pub fn automation_id(mut self, id: &str) -> Self {
        self.automation_id = Some(Rc::from(id));
        self
    }

    /// decorative frame demand — gated by visibility + reduced motion
    pub fn frame_events(mut self, on: bool) -> Self {
        self.frame_events = on;
        self
    }
    pub fn visibility(mut self, v: Visibility) -> Self {
        self.visibility = v;
        self
    }
    pub fn layout(mut self, l: LayoutSpec) -> Self {
        self.layout = l;
        self
    }
    pub fn width(mut self, w: crate::geom::Length) -> Self {
        self.layout.width = w;
        self
    }
    pub fn height(mut self, h: crate::geom::Length) -> Self {
        self.layout.height = h;
        self
    }
    pub fn on_frame(mut self, f: impl Fn(FrameTime) -> M + 'static) -> Self {
        self.factories.on_frame = Some(Box::new(move |t| Box::new(f(t))));
        self
    }
    /// custom actionable content wires the same activation as a real button
    pub fn on_press(mut self, f: impl Fn() -> M + 'static) -> Self {
        self.factories.on_press = Some(Box::new(move || Box::new(f())));
        self
    }
    pub fn on_scroll(mut self, f: impl Fn(ScrollOffset) -> M + 'static) -> Self {
        self.factories.on_scroll = Some(Box::new(move |o| Box::new(f(o))));
        self
    }
}

impl<'a, 'ui, M: 'static> Drop for CustomBuilder<'a, 'ui, M> {
    fn drop(&mut self) {
        let before = self.tx.frames.last().unwrap().children.len();
        let render = self.render.clone();
        let frame_events = self.frame_events;
        self.tx.stage_leaf(
            crate::node::KIND_CUSTOM,
            None,
            self.visibility,
            self.layout,
            std::mem::take(&mut self.factories),
            move |_| NodeData::Custom {
                render,
                frame_events,
            },
        );
        if let Some(id) = self.automation_id.take()
            && self.tx.frames.last().unwrap().children.len() > before
            && let Some(index) = self.tx.frames.last().unwrap().children.last().copied()
        {
            self.tx.nodes[index as usize].automation_id = Some(id);
        }
    }
}

/// `ui.action` builder — the node stages immediately (children must draw
/// inside it); factory methods retrofit onto the staged node before the
/// transaction commits.
pub struct ActionBuilder<'a, 'ui, M: 'static> {
    tx: &'a mut Tx<'ui>,
    /// staged node index in `tx.nodes` — None when the stage was rejected
    staged: Option<u32>,
    _m: std::marker::PhantomData<fn() -> M>,
}

impl<'a, 'ui, M: 'static> ActionBuilder<'a, 'ui, M> {
    pub fn automation_id(self, id: &str) -> Self {
        if let Some(i) = self.staged {
            self.tx.nodes[i as usize].automation_id = Some(Rc::from(id));
        }
        self
    }
    /// presentation-only staged visual state (`Presented` docs)
    pub fn presented(self, p: Presented) -> Self {
        if let Some(i) = self.staged {
            self.tx.nodes[i as usize].presented = p;
        }
        self
    }
    pub fn semantic_role(self, role: SemanticRole) -> Self {
        if let Some(i) = self.staged {
            self.tx.nodes[i as usize].role = Some(role);
        }
        self
    }
    pub fn semantic_checked(self, checked: Option<bool>) -> Self {
        if let Some(i) = self.staged
            && let NodeData::Action {
                checked: staged, ..
            } = &mut self.tx.nodes[i as usize].data
        {
            *staged = checked;
        }
        self
    }
    fn set(&mut self, f: impl FnOnce(&mut EventFactorySet)) {
        if let Some(i) = self.staged {
            f(&mut self.tx.nodes[i as usize].factories);
        }
    }
    pub fn on_press(mut self, f: impl Fn() -> M + 'static) -> Self {
        self.set(|fac| fac.on_press = Some(Box::new(move || Box::new(f()))));
        self
    }
    pub fn on_focus(mut self, f: impl Fn() -> M + 'static) -> Self {
        self.set(|fac| fac.on_focus = Some(Box::new(move || Box::new(f()))));
        self
    }
    pub fn on_blur(mut self, f: impl Fn() -> M + 'static) -> Self {
        self.set(|fac| fac.on_blur = Some(Box::new(move || Box::new(f()))));
        self
    }
    pub fn on_key(mut self, f: impl Fn(KeyEvent) -> Option<M> + 'static) -> Self {
        self.set(|fac| fac.on_key = Some(Box::new(move |e| f(e).map(|m| Box::new(m) as _))));
        self
    }
    pub fn on_pointer_enter(mut self, f: impl Fn(PointerEvent) -> M + 'static) -> Self {
        self.set(|fac| fac.on_pointer_enter = Some(Box::new(move |e| Box::new(f(e)))));
        self
    }
    pub fn on_pointer_leave(mut self, f: impl Fn(PointerEvent) -> M + 'static) -> Self {
        self.set(|fac| fac.on_pointer_leave = Some(Box::new(move |e| Box::new(f(e)))));
        self
    }
}

/// `ui.popover` builder — the popup node stages at `popover()` (children
/// draw inside it); setters retrofit onto the staged node. When `open` is
/// false nothing staged and every setter is a no-op.
pub struct PopupBuilder<'a, 'ui, M: 'static> {
    tx: &'a mut Tx<'ui>,
    /// staged node index in `tx.nodes` — None when closed or rejected
    staged: Option<u32>,
    _m: std::marker::PhantomData<fn() -> M>,
}

impl<'a, 'ui, M: 'static> PopupBuilder<'a, 'ui, M> {
    /// Mutate the staged Popup payload — no-op when nothing staged.
    fn edit(&mut self, f: impl FnOnce(&mut PopupFields<'_>)) {
        if let Some(i) = self.staged
            && let StagedNode {
                data:
                    NodeData::Popup {
                        anchor,
                        side,
                        align,
                        offset,
                        role,
                        modal,
                        dismiss_light,
                        focus_on_open,
                        props,
                    },
                ..
            } = &mut self.tx.nodes[i as usize]
        {
            f(&mut PopupFields {
                anchor,
                side,
                align,
                offset,
                role,
                modal,
                dismiss_light,
                focus_on_open,
                props,
            });
        }
    }

    /// automation id of the node the popup anchors to (e.g. the trigger)
    pub fn anchor(mut self, automation_id: &str) -> Self {
        self.edit(|p| *p.anchor = Some(Rc::from(automation_id)));
        self
    }
    /// anchor edge the popup opens from (default Bottom)
    pub fn side(mut self, s: crate::geom::Side) -> Self {
        self.edit(|p| *p.side = s);
        self
    }
    /// cross-axis alignment against the anchor (default Center)
    pub fn align(mut self, a: Align) -> Self {
        self.edit(|p| *p.align = a);
        self
    }
    /// gap along the side axis in Dp (Radix `sideOffset`)
    pub fn side_offset(mut self, d: crate::geom::Dp) -> Self {
        self.edit(|p| match *p.side {
            crate::geom::Side::Top | crate::geom::Side::Bottom => p.offset.y = d.0,
            crate::geom::Side::Left | crate::geom::Side::Right => p.offset.x = d.0,
        });
        self
    }
    /// shift along the align axis in Dp (Radix `alignOffset`)
    pub fn align_offset(mut self, d: crate::geom::Dp) -> Self {
        self.edit(|p| match *p.side {
            crate::geom::Side::Top | crate::geom::Side::Bottom => p.offset.x = d.0,
            crate::geom::Side::Left | crate::geom::Side::Right => p.offset.y = d.0,
        });
        self
    }
    /// semantic overlay role — drives the UIA control type
    pub fn role(mut self, r: crate::node::PopupRole) -> Self {
        self.edit(|p| *p.role = r);
        self
    }
    /// modal overlay — scrim + hit/focus/scroll gate to the subtree.
    /// `modal(true)` also clears `dismiss_light` (a modal surface never
    /// light-dismisses); re-enable it afterwards only for deliberate
    /// hybrid behavior.
    pub fn modal(mut self, on: bool) -> Self {
        self.edit(|p| {
            *p.modal = on;
            if on {
                *p.dismiss_light = false;
            }
        });
        self
    }
    /// outside-press dismisses via `on_open_change(false)` (default on)
    pub fn dismiss_light(mut self, on: bool) -> Self {
        self.edit(|p| *p.dismiss_light = on);
        self
    }
    /// move focus to the first eligible descendant on open, restore on
    /// close (default on)
    pub fn focus_on_open(mut self, on: bool) -> Self {
        self.edit(|p| *p.focus_on_open = on);
        self
    }
    pub fn automation_id(self, id: &str) -> Self {
        if let Some(i) = self.staged {
            self.tx.nodes[i as usize].automation_id = Some(Rc::from(id));
        }
        self
    }
    /// surgical chrome patch over the popup's surface recipe — later
    /// `.style` calls deep-merge, `None` never erases
    pub fn style(mut self, p: crate::style::BoxStylePatch) -> Self {
        let mut patch = self
            .staged
            .and_then(|i| match &self.tx.nodes[i as usize].data {
                NodeData::Popup { props, .. } => Some(props.patch),
                _ => None,
            })
            .unwrap_or_default();
        patch.merge(&p);
        self.edit(|f| f.props.patch = patch);
        self
    }
    /// open-state events — `false` on outside-press/Escape dismissals
    pub fn on_open_change(self, f: impl Fn(bool) -> M + 'static) -> Self {
        if let Some(i) = self.staged {
            self.tx.nodes[i as usize].factories.on_open_change =
                Some(Box::new(move |o| Box::new(f(o))));
        }
        self
    }
    /// popup width — the natural measure is children-driven; a select
    /// content pins the frozen 208dp lane
    pub fn width(self, w: crate::geom::Length) -> Self {
        if let Some(i) = self.staged {
            self.tx.nodes[i as usize].layout.width = w;
        }
        self
    }
    /// popup height override — natural (content-measured) by default
    pub fn height(self, h: crate::geom::Length) -> Self {
        if let Some(i) = self.staged {
            self.tx.nodes[i as usize].layout.height = h;
        }
        self
    }
}

/// Borrowed field views for `PopupBuilder::edit` — keeps the destructure
/// out of every setter.
struct PopupFields<'a> {
    anchor: &'a mut Option<Rc<str>>,
    side: &'a mut crate::geom::Side,
    align: &'a mut Align,
    offset: &'a mut Point,
    role: &'a mut crate::node::PopupRole,
    modal: &'a mut bool,
    dismiss_light: &'a mut bool,
    focus_on_open: &'a mut bool,
    props: &'a mut ContainerProps,
}

/// `ui.text` — styled text leaf with a full authored `TextStyle`.
pub struct TextBuilder<'a, 'ui, T: AsRef<str>, M: 'static> {
    automation_id: Option<Rc<str>>,
    tx: &'a mut Tx<'ui>,
    text: T,
    wrap: bool,
    style: crate::style::TextStyle,
    /// painted chrome under the text (rule lines, code surfaces)
    chrome: Option<crate::style::BoxStyle>,
    visibility: Visibility,
    layout: LayoutSpec,
    _m: std::marker::PhantomData<fn() -> M>,
}

impl<'a, 'ui, T: AsRef<str>, M: 'static> TextBuilder<'a, 'ui, T, M> {
    pub fn automation_id(mut self, id: &str) -> Self {
        self.automation_id = Some(Rc::from(id));
        self
    }

    /// full authored style — primitives want complete styles, not patches
    pub fn style(mut self, s: crate::style::TextStyle) -> Self {
        self.style = s;
        self
    }
    /// painted chrome under the text — rule lines / code surfaces
    pub fn chrome(mut self, c: crate::style::BoxStyle) -> Self {
        self.chrome = Some(c);
        self
    }
    pub fn wrap(mut self, w: bool) -> Self {
        self.wrap = w;
        self
    }
    pub fn visibility(mut self, v: Visibility) -> Self {
        self.visibility = v;
        self
    }
    pub fn width(mut self, w: crate::geom::Length) -> Self {
        self.layout.width = w;
        self
    }
    pub fn height(mut self, h: crate::geom::Length) -> Self {
        self.layout.height = h;
        self
    }
}

impl<'a, 'ui, T: AsRef<str>, M: 'static> Drop for TextBuilder<'a, 'ui, T, M> {
    fn drop(&mut self) {
        let before = self.tx.frames.last().unwrap().children.len();
        // authored full style lands as a patch over the label recipe — the
        // staged label shape is shared with `ui.label`
        let mut patch = crate::style::TextStylePatch::default();
        patch.foreground = Some(self.style.foreground);
        if let crate::style::TextSize::Exact(d) = self.style.size {
            patch.size = Some(d);
        }
        patch.weight = Some(self.style.weight);
        patch.font = Some(self.style.font);
        patch.line_height = self.style.line_height;
        patch.underline = Some(self.style.underline);
        patch.letter_spacing = Some(self.style.letter_spacing);
        let text = self.text.as_ref();
        let wrap = self.wrap;
        let chrome = self.chrome;
        self.tx.stage_leaf(
            crate::node::KIND_LABEL,
            None,
            self.visibility,
            self.layout,
            EventFactorySet::default(),
            move |retained| {
                let text = match retained {
                    Some(crate::node::Node {
                        data: NodeData::Label { text: old, .. },
                        ..
                    }) if &**old == text => old.clone(),
                    _ => Rc::from(text),
                };
                NodeData::Label {
                    text,
                    wrap,
                    color_role: ColorRole::Foreground,
                    patch,
                    chrome,
                }
            },
        );
        if let Some(id) = self.automation_id.take()
            && self.tx.frames.last().unwrap().children.len() > before
            && let Some(index) = self.tx.frames.last().unwrap().children.last().copied()
        {
            self.tx.nodes[index as usize].automation_id = Some(id);
        }
    }
}

/// `ui.icon` — decorative vector icon leaf (Lucide stroke geometry).
/// Carries no interaction; `size`/`color` are authored directly.
pub struct IconBuilder<'a, 'ui, M: 'static> {
    automation_id: Option<Rc<str>>,
    tx: &'a mut Tx<'ui>,
    icon: crate::icons::Icon,
    size: crate::geom::Dp,
    color: crate::style::Color,
    visibility: Visibility,
    layout: LayoutSpec,
    _m: std::marker::PhantomData<fn() -> M>,
}

impl<'a, 'ui, M: 'static> IconBuilder<'a, 'ui, M> {
    pub fn automation_id(mut self, id: &str) -> Self {
        self.automation_id = Some(Rc::from(id));
        self
    }
    /// icon edge in dp — the viewBox scales `size / 24`; the frozen
    /// specimen size is 16 (`size-4`)
    pub fn size(mut self, s: crate::geom::Dp) -> Self {
        self.size = s;
        self
    }
    /// stroke color — the `currentColor` analog; recipes/authors set it
    pub fn color(mut self, c: crate::style::Color) -> Self {
        self.color = c;
        self
    }
    /// role shorthand — `Icon` strokes resolve through the theme palette
    pub fn color_role(mut self, r: ColorRole) -> Self {
        self.color = crate::style::Color::Role(r);
        self
    }
    pub fn visibility(mut self, v: Visibility) -> Self {
        self.visibility = v;
        self
    }
    pub fn width(mut self, w: crate::geom::Length) -> Self {
        self.layout.width = w;
        self
    }
    pub fn height(mut self, h: crate::geom::Length) -> Self {
        self.layout.height = h;
        self
    }
}

impl<'a, 'ui, M: 'static> Drop for IconBuilder<'a, 'ui, M> {
    fn drop(&mut self) {
        let before = self.tx.frames.last().unwrap().children.len();
        let (icon, size, color) = (self.icon, self.size, self.color);
        self.tx.stage_leaf(
            crate::node::KIND_ICON,
            None,
            self.visibility,
            self.layout,
            EventFactorySet::default(),
            move |_| NodeData::Icon { icon, size, color },
        );
        if let Some(id) = self.automation_id.take()
            && self.tx.frames.last().unwrap().children.len() > before
            && let Some(index) = self.tx.frames.last().unwrap().children.last().copied()
        {
            self.tx.nodes[index as usize].automation_id = Some(id);
        }
    }
}

// ------------------------- forms APIs ---------------------------------------

impl<'ui, 'tx, M: 'static> Ui<'ui, 'tx, M> {
    /// Stage a Toggle leaf. Automation ids come from the enclosing
    /// `ui.named`/`ui.part` wrapper — the leaf stages plain.
    fn stage_toggle(
        &mut self,
        kind: crate::theme::ToggleKind,
        size: crate::theme::ToggleSize,
        checked: bool,
        disabled: bool,
        invalid: bool,
        label: Option<&'static str>,
        accessible_name: Option<&'static str>,
        presented: Presented,
        layout: LayoutSpec,
        factories: EventFactorySet,
    ) {
        self.tx.stage_leaf(
            crate::node::KIND_TOGGLE,
            None,
            Visibility::Visible,
            layout,
            factories,
            move |_| NodeData::Toggle {
                kind,
                checked,
                disabled,
                invalid,
                size,
                label: label.map(Rc::from),
                accessible_name: accessible_name.map(Rc::from),
            },
        );
        if presented.any()
            && let Some(index) = self.tx.frames.last().unwrap().children.last().copied()
        {
            self.tx.nodes[index as usize].presented = presented;
        }
    }

    /// `ui.checkbox` — 16×16 selection box; app model owns `checked`,
    /// `on_change` fires on the shared Press path. Stage inside
    /// `ui.named`/`ui.part` for automation ids (like `ui.icon`).
    pub fn checkbox(&mut self, props: Checkbox, on_change: impl Fn() -> M + 'static) {
        let mut fac = EventFactorySet::default();
        fac.on_press = Some(Box::new(move || Box::new(on_change()) as Box<dyn Any>));
        self.stage_toggle(
            crate::theme::ToggleKind::Checkbox,
            crate::theme::ToggleSize::Default,
            props.checked,
            props.disabled,
            props.invalid,
            props.label,
            props.accessible_name,
            props.presented,
            LayoutSpec::default(),
            fac,
        );
    }

    /// `ui.switch` — pill track + thumb; `props.size` picks the frozen
    /// default or `size-sm` track.
    pub fn switch(&mut self, props: Switch, on_change: impl Fn() -> M + 'static) {
        let mut fac = EventFactorySet::default();
        fac.on_press = Some(Box::new(move || Box::new(on_change()) as Box<dyn Any>));
        self.stage_toggle(
            crate::theme::ToggleKind::Switch,
            props.size,
            props.checked,
            props.disabled,
            props.invalid,
            props.label,
            props.accessible_name,
            props.presented,
            LayoutSpec::default(),
            fac,
        );
    }

    /// `ui.radio_group` — the group's `selected` string owns the checked
    /// item; each `item` is a `part("item[key]")` 16×16 RadioItem toggle
    /// whose Press fires `on_select(key)`. Stage as a column (frozen 8dp
    /// item gap) inside the caller's `ui.named`/`part` scope.
    pub fn radio_group<F>(
        &mut self,
        selected: &str,
        on_select: F,
        draw: impl FnOnce(&mut RadioGroup<'_, 'ui, 'tx, M>),
    ) where
        F: Fn(&'static str) -> M + 'static,
    {
        let mut rg = RadioGroup {
            ui: self,
            selected: selected.to_string(),
            on_select: Rc::new(on_select),
        };
        draw(&mut rg);
    }

    /// Post-stage role tag on the last child pushed in the current
    /// frame — used by composite builders (`ui.select`, `ui.tabs`) whose
    /// children are ordinary kinds carrying richer semantics.
    fn set_role_last(&mut self, role: SemanticRole) {
        if let Some(i) = self.tx.frames.last().unwrap().children.last().copied() {
            self.tx.nodes[i as usize].role = Some(role);
        }
    }

    /// `ui.select` — the frozen combobox: a `Select`-variant trigger
    /// (value text or `data-placeholder`, chevron end icon, combobox
    /// semantics) plus an anchored `PopupRole::Listbox` popup staged only
    /// while `open`. Focus model: `focus_on_open` moves focus INTO the
    /// listbox (the `selected` item when set, else the first enabled);
    /// arrows/Home/End walk enabled options (`nav_target`), Enter/Space
    /// activates the live option, Escape/outside-press dismiss via the
    /// popup foundation; closing restores focus to the trigger.
    pub fn select<F>(
        &mut self,
        props: Select<'_>,
        on_change: F,
        draw: impl FnOnce(&mut SelectItems<'_, '_, '_, M>),
    ) where
        F: Fn(SelectChange) -> M + 'static,
    {
        let on_change = Rc::new(on_change);
        // trigger — the combobox value button (w-fit natural)
        let text = props.value.unwrap_or(props.placeholder);
        let mut b = self
            .button(text)
            .variant(ButtonVariant::Select)
            .size(props.size)
            .icon_end(crate::icons::Icon::ChevronDown)
            .icon_color(Some(crate::style::Color::Role(ColorRole::MutedForeground)))
            .expanded(Some(props.open))
            .disabled(props.disabled)
            .invalid(props.invalid)
            .presented(props.presented)
            .automation_id(props.automation_id)
            .on_press({
                let cb = on_change.clone();
                let open = props.open;
                move || cb(SelectChange::Open(!open))
            });
        if props.value.is_none() {
            b = b.style(crate::style::select_placeholder_patch());
        }
        drop(b);
        self.set_role_last(SemanticRole::ComboBox);
        // listbox popup — chrome + content column
        let selected = props.selected;
        let pw = props.width.unwrap_or(crate::geom::dp(208.0));
        let mut p = self
            .popover(props.open, |ui| {
                ui.column(
                    Column::new().align(Align::Stretch).padding(Space::None),
                    |ui| {
                        let mut st = SelectItems {
                            ui,
                            selected,
                            popup_width: pw.0,
                            on_change: on_change.clone(),
                        };
                        draw(&mut st);
                    },
                );
            })
            .anchor(props.automation_id)
            .side(crate::geom::Side::Bottom)
            .align(Align::Center)
            .side_offset(crate::geom::dp(4.0))
            .role(crate::node::PopupRole::Listbox)
            .width(Length::Fixed(pw))
            .style(crate::style::select_content_patch());
        if let Some(id) = props.content_id {
            p = p.automation_id(id);
        }
        p.on_open_change(move |o| on_change(SelectChange::Open(o)));
    }

    /// `ui.tabs` — the frozen tab list: `list` part chrome (muted pill /
    /// line lane) wrapping a row of keyed `tab[key]` triggers. Press
    /// delivers `on_select(key)`; directional nav is the shared policy —
    /// ArrowLeft/Right move focus AND activate the target (Radix
    /// automatic activation), Home/End jump first/last, disabled tabs
    /// are skipped.
    pub fn tabs<F>(
        &mut self,
        props: Tabs<'_>,
        on_select: F,
        draw: impl FnOnce(&mut TabList<'_, '_, '_, M>),
    ) where
        F: Fn(&'static str) -> M + 'static,
    {
        let variant = props.variant;
        let selected = props.selected;
        let on_select = Rc::new(on_select);
        self.part("list", |ui| {
            ui.box_(
                BoxProps::new()
                    .style(crate::style::tabs_list_recipe(variant))
                    .height(Length::Fixed(crate::geom::dp(32.0))),
                |ui| {
                    ui.row(
                        Row::new()
                            .gap(match variant {
                                TabsVariant::Line => Space::Xs,
                                TabsVariant::Default => Space::None,
                            })
                            .align(Align::Center),
                        |ui| {
                            let mut tl = TabList {
                                ui,
                                variant,
                                selected,
                                on_select: on_select.clone(),
                            };
                            draw(&mut tl);
                        },
                    );
                },
            );
            ui.set_role_last(SemanticRole::TabList);
        });
    }

    /// Reusable desktop dropdown menu. The application owns `open` and
    /// every checked/radio value; menu callbacks are typed requests.
    pub fn dropdown_menu(
        &mut self,
        props: DropdownMenu<'_>,
        on_change: impl Fn(DropdownMenuChange) -> M + 'static,
        draw: impl FnOnce(&mut DropdownMenuItems<'_, '_, '_, M>),
    ) {
        let on_change = Rc::new(on_change);
        let trigger = self
            .button(props.label)
            .automation_id(props.automation_id)
            .variant(ButtonVariant::Outline)
            .expanded(Some(props.open))
            .on_press({
                let cb = on_change.clone();
                let open = props.open;
                move || cb(DropdownMenuChange::Open(!open))
            });
        drop(trigger);
        self.set_role_last(SemanticRole::MenuButton);
        let mut popup = self
            .popover(props.open, |ui| {
                ui.column(
                    Column::new().align(Align::Stretch).padding(Space::None),
                    |ui| {
                        let mut items = DropdownMenuItems {
                            ui,
                            on_change: on_change.clone(),
                            width: props.width.0,
                        };
                        draw(&mut items);
                    },
                );
            })
            .anchor(props.automation_id)
            .side(crate::geom::Side::Bottom)
            .align(Align::Start)
            .side_offset(crate::geom::dp(4.0))
            .role(crate::node::PopupRole::Menu)
            .width(Length::Fixed(props.width))
            .style(crate::style::select_content_patch());
        if let Some(id) = props.content_id {
            popup = popup.automation_id(id);
        }
        popup.on_open_change(move |open| on_change(DropdownMenuChange::Open(open)));
    }

    /// Sidebar-13 desktop SettingsNav. This component owns only the
    /// navigation rail; compose its content pane from ordinary components.
    pub fn settings_nav(
        &mut self,
        props: SettingsNav<'_>,
        on_select: impl Fn(&'static str) -> M + 'static,
        draw: impl FnOnce(&mut SettingsNavGroups<'_, '_, '_, M>),
    ) {
        let selected = props.selected;
        let on_select = Rc::new(on_select);
        let width = crate::geom::dp(if props.collapsed { 48.0 } else { 256.0 });
        self.named(props.automation_id, |ui| {
            ui.box_(
                crate::style::BoxProps::new()
                    .style(crate::style::settings_nav_surface())
                    .width(Length::Fixed(width)),
                |ui| {
                    ui.column(
                        Column::new()
                            .align(Align::Stretch)
                            .padding(Space::None)
                            .gap(Space::None),
                        |ui| {
                            if !props.collapsed {
                                ui.part("header", |ui| {
                                    ui.text("Settings")
                                        .style(crate::style::settings_nav_header_style())
                                        .chrome(crate::style::settings_nav_header_box())
                                        .height(Length::Fixed(crate::geom::dp(44.0)));
                                });
                            }
                            let mut groups = SettingsNavGroups {
                                ui,
                                collapsed: props.collapsed,
                                selected,
                                on_select: on_select.clone(),
                            };
                            draw(&mut groups);
                        },
                    );
                },
            );
        });
    }

    /// Standalone SettingsNavItem state specimen. It uses the same
    /// recipe and runtime semantics as an item inside `settings_nav`.
    pub fn settings_nav_item(
        &mut self,
        id: &str,
        label: &'static str,
        icon: crate::icons::Icon,
        active: bool,
        opts: SettingsNavItemOpts,
        on_press: impl Fn() -> M + 'static,
    ) {
        stage_settings_nav_item(
            self,
            id,
            id,
            label,
            icon,
            active,
            false,
            opts,
            Rc::new(on_press),
        );
    }

    /// `ui.field` — the frozen 288dp field container: `Vertical` stacks
    /// label/control/description at 8dp gaps (Stretch); `Horizontal` is a
    /// `Center`-aligned row at 8dp gap. Compose `ui.part` + the
    /// `field_*_style` recipes inside.
    pub fn field(&mut self, props: Field, draw: impl FnOnce(&mut Ui<'_, '_, M>)) {
        let layout = LayoutSpec {
            width: props.width.map(Length::Fixed).unwrap_or(Length::Fill(1)),
            ..Default::default()
        };
        match props.orientation {
            crate::theme::FieldOrientation::Vertical => self.tx.stage_container(
                crate::node::KIND_COLUMN,
                None,
                Column::new()
                    .gap(Space::Sm)
                    .align(Align::Stretch)
                    .padding(Space::None)
                    .props(),
                layout,
                EventFactorySet::default(),
                Visibility::Visible,
                draw,
                false,
                |props| NodeData::Container {
                    kind: crate::node::KIND_COLUMN,
                    props,
                },
            ),
            crate::theme::FieldOrientation::Horizontal => self.tx.stage_container(
                crate::node::KIND_ROW,
                None,
                Row::new()
                    .gap(Space::Sm)
                    .align(Align::Center)
                    .padding(Space::None)
                    .props(),
                layout,
                EventFactorySet::default(),
                Visibility::Visible,
                draw,
                false,
                |props| NodeData::Container {
                    kind: crate::node::KIND_ROW,
                    props,
                },
            ),
        }
    }

    /// `ui.input` — the frozen `input` element: a KIND_STACK pairing the
    /// editor leaf (automation id, opaque borderless content chrome) with
    /// a transparent overlay box carrying the 10dp radius + 1dp border +
    /// disabled `bg-input/50` fill. The overlay owns the frame so the
    /// radius never clips the windowless peer.
    pub fn input<'a, 'b>(&'a mut self, value: &'b TextValue) -> InputBuilder<'a, 'ui, 'b, M> {
        InputBuilder::new(self.tx, value, false)
    }

    /// `ui.textarea` — same stack composition at the multiline chrome
    /// (`py-2` = 9dp top/bottom incl. the 1dp border).
    pub fn textarea<'a, 'b>(&'a mut self, value: &'b TextValue) -> InputBuilder<'a, 'ui, 'b, M> {
        InputBuilder::new(self.tx, value, true)
    }
}

/// `ui.radio_group` item stager — passed to the group draw closure.
pub struct RadioGroup<'a, 'ui, 'tx, M: 'static> {
    ui: &'a mut Ui<'ui, 'tx, M>,
    selected: String,
    on_select: Rc<dyn Fn(&'static str) -> M>,
}
impl<'a, 'ui, 'tx, M: 'static> RadioGroup<'a, 'ui, 'tx, M> {
    /// One `item[key]` part: 16×16 radio toggle, checked when the group's
    /// `selected == key`; Press delivers `on_select(key)`.
    pub fn item(&mut self, key: &'static str, label: &'static str, opts: RadioItemOpts) {
        let cb = self.on_select.clone();
        let checked = self.selected == key;
        self.ui.part(&format!("item[{key}]"), |ui| {
            let mut fac = EventFactorySet::default();
            fac.on_press = Some(Box::new(move || Box::new(cb(key)) as Box<dyn Any>));
            ui.stage_toggle(
                crate::theme::ToggleKind::RadioItem,
                crate::theme::ToggleSize::Default,
                checked,
                opts.disabled,
                opts.invalid,
                Some(label),
                None,
                opts.presented,
                LayoutSpec::default(),
                fac,
            );
        });
    }
}

/// `ui.select` content stager — passed to the select draw closure. Items
/// stage inside the listbox column; each `item[key]` carries option
/// semantics, `label[key]` is a group heading, `separator()` is the
/// bleeding hairline between groups.
pub struct SelectItems<'a, 'ui, 'tx, M: 'static> {
    ui: &'a mut Ui<'ui, 'tx, M>,
    selected: Option<&'static str>,
    popup_width: f32,
    on_change: Rc<dyn Fn(SelectChange) -> M>,
}
impl<'a, 'ui, 'tx, M: 'static> SelectItems<'a, 'ui, 'tx, M> {
    pub fn group(
        &mut self,
        key: &'static str,
        text: &'static str,
        draw: impl FnOnce(&mut SelectItems<'_, '_, '_, M>),
    ) {
        let selected = self.selected;
        let popup_width = self.popup_width;
        let on_change = self.on_change.clone();
        self.ui.part(&format!("group[{key}]"), |ui| {
            ui.column(
                Column::new().align(Align::Stretch).padding(Space::None),
                |ui| {
                    let mut items = SelectItems {
                        ui,
                        selected,
                        popup_width,
                        on_change,
                    };
                    items.label(key, text);
                    draw(&mut items);
                },
            );
        });
        self.ui.set_role_last(SemanticRole::Group);
    }

    /// `label[key]` part — group heading (`h-6 px-1.5 py-1 text-xs
    /// muted-foreground`).
    pub fn label(&mut self, key: &str, text: &str) {
        self.ui.part(&format!("label[{key}]"), |ui| {
            ui.text(text)
                .style(crate::style::select_label_style())
                .chrome(crate::style::BoxStyle {
                    padding: crate::style::Insets {
                        top: crate::geom::dp(4.0),
                        right: crate::geom::dp(6.0),
                        bottom: crate::geom::dp(4.0),
                        left: crate::geom::dp(6.0),
                    },
                    ..crate::style::BoxStyle::default()
                })
                .height(Length::Fixed(crate::geom::dp(24.0)));
        });
    }

    /// `item[key]` part — the option row (h-7, `SelectItem` recipe,
    /// check indicator when `selected`); Press delivers
    /// `SelectChange::Pick(key)`.
    pub fn item(&mut self, key: &'static str, text: &'static str, opts: SelectItemOpts) {
        let cb = self.on_change.clone();
        let selected = self.selected == Some(key);
        self.ui.part(&format!("item[{key}]"), |ui| {
            let mut b = ui
                .button(text)
                .variant(ButtonVariant::SelectItem)
                .selected(selected)
                .disabled(opts.disabled)
                .presented(opts.presented)
                .width(Length::Fill(1))
                .height(Length::Fixed(crate::geom::dp(28.0)))
                .on_press(move || cb(SelectChange::Pick(key)));
            if selected {
                b = b.icon_end(crate::icons::Icon::Check);
            }
            if let Some(id) = opts.automation_id {
                b = b.automation_id(id);
            }
            drop(b);
            ui.set_role_last(SemanticRole::Option);
        });
    }

    /// `separator` part — the `-mx-1 my-1 h-px` bleeding hairline:
    /// popup-width + 8dp centered inside a padded wrapper so the line
    /// reaches 4dp past each side of the content lane.
    pub fn separator(&mut self) {
        let w = self.popup_width + 8.0;
        self.ui.column(
            Column::new().padding(Space::Sm).align(Align::Center),
            |ui| {
                ui.part("separator", |ui| {
                    ui.box_(
                        BoxProps::new()
                            .width(Length::Fixed(crate::geom::dp(w)))
                            .height(Length::Fixed(crate::geom::dp(1.0)))
                            .style(crate::style::BoxStyle {
                                background: crate::style::Color::Role(ColorRole::Border),
                                ..crate::style::BoxStyle::default()
                            }),
                        |_| {},
                    );
                });
            },
        );
    }
}

/// `ui.tabs` trigger stager — passed to the tabs draw closure inside the
/// list's row.
pub struct TabList<'a, 'ui, 'tx, M: 'static> {
    ui: &'a mut Ui<'ui, 'tx, M>,
    variant: TabsVariant,
    selected: &'a str,
    on_select: Rc<dyn Fn(&'static str) -> M>,
}
impl<'a, 'ui, 'tx, M: 'static> TabList<'a, 'ui, 'tx, M> {
    /// One `tab[key]` part — the keyed trigger (h-25px tab semantics);
    /// `selected` paints the recipe's active chrome (chip + shadow, or
    /// the line underline marker). Press delivers `on_select(key)`.
    pub fn tab(&mut self, key: &'static str, label: &'static str, opts: TabOpts) {
        let cb = self.on_select.clone();
        let active = self.selected == key;
        let variant = match self.variant {
            TabsVariant::Line => ButtonVariant::TabLine,
            TabsVariant::Default => ButtonVariant::Tab,
        };
        self.ui.part(&format!("tab[{key}]"), |ui| {
            let mut b = ui
                .button(label)
                .variant(variant)
                .selected(active)
                .disabled(opts.disabled)
                .presented(opts.presented)
                .height(Length::Fixed(crate::geom::dp(25.0)))
                .on_press(move || cb(key));
            if let Some(id) = opts.automation_id {
                b = b.automation_id(id);
            }
            drop(b);
            ui.set_role_last(SemanticRole::Tab);
        });
    }
}

pub struct DropdownMenuItems<'a, 'ui, 'tx, M: 'static> {
    ui: &'a mut Ui<'ui, 'tx, M>,
    on_change: Rc<dyn Fn(DropdownMenuChange) -> M>,
    width: f32,
}

impl<'a, 'ui, 'tx, M: 'static> DropdownMenuItems<'a, 'ui, 'tx, M> {
    pub fn group(
        &mut self,
        key: &'static str,
        label: &'static str,
        draw: impl FnOnce(&mut DropdownMenuItems<'_, '_, '_, M>),
    ) {
        let cb = self.on_change.clone();
        let width = self.width;
        self.ui.part(&format!("group[{key}]"), |ui| {
            ui.column(
                Column::new().align(Align::Stretch).padding(Space::None),
                |ui| {
                    ui.part(&format!("label[{key}]"), |ui| {
                        ui.text(label)
                            .style(crate::style::dropdown_label_style())
                            .chrome(crate::style::dropdown_label_box())
                            .height(Length::Fixed(crate::geom::dp(24.0)));
                    });
                    draw(&mut DropdownMenuItems {
                        ui,
                        on_change: cb,
                        width,
                    });
                },
            );
        });
        self.ui.set_role_last(SemanticRole::Group);
    }

    pub fn item(&mut self, key: &'static str, label: &'static str, opts: DropdownItemOpts) {
        let cb = self.on_change.clone();
        let role = match opts.kind {
            DropdownItemKind::Normal => SemanticRole::MenuItem,
            DropdownItemKind::Destructive => SemanticRole::MenuItemDestructive,
            DropdownItemKind::Checkbox { .. } => SemanticRole::MenuItemCheckbox,
            DropdownItemKind::Radio { .. } => SemanticRole::MenuItemRadio,
        };
        let checked = match opts.kind {
            DropdownItemKind::Checkbox { checked } => Some(checked),
            DropdownItemKind::Radio { selected } => Some(selected),
            _ => None,
        };
        let destructive = opts.kind == DropdownItemKind::Destructive;
        let activation = match opts.kind {
            DropdownItemKind::Normal | DropdownItemKind::Destructive => {
                DropdownMenuChange::Activate(key)
            }
            DropdownItemKind::Checkbox { checked } => DropdownMenuChange::Checkbox(key, !checked),
            DropdownItemKind::Radio { .. } => DropdownMenuChange::Radio(key),
        };
        self.ui.part(&format!("item[{key}]"), |ui| {
            let mut action = ui
                .action(
                    Action::new()
                        .label(label)
                        .style(crate::style::dropdown_item_style(destructive))
                        .disabled(opts.disabled),
                    |ui| {
                        ui.row(
                            Row::new()
                                .width(Length::Fill(1))
                                .height(Length::Fixed(crate::geom::dp(20.0)))
                                .align(Align::Center)
                                .padding(Space::None),
                            |ui| {
                                ui.row(
                                    Row::new()
                                        .align(Align::Center)
                                        .gap(Space::Xs)
                                        .padding(Space::None),
                                    |ui| {
                                        if let Some(icon) = opts.icon {
                                            ui.part("icon", |ui| {
                                                ui.icon(icon)
                                                    .size(crate::geom::dp(16.0))
                                                    .color_role(if destructive {
                                                        ColorRole::Destructive
                                                    } else {
                                                        ColorRole::Foreground
                                                    });
                                            });
                                        }
                                        ui.text(label).style(
                                            crate::style::dropdown_item_text_style(
                                                if destructive {
                                                    ColorRole::Destructive
                                                } else {
                                                    ColorRole::Foreground
                                                },
                                            ),
                                        );
                                    },
                                );
                                ui.box_(BoxProps::new().width(Length::Fill(1)), |_| {});
                                match opts.kind {
                                    DropdownItemKind::Checkbox { checked }
                                    | DropdownItemKind::Radio { selected: checked } => {
                                        ui.part("indicator", |ui| {
                                            ui.box_(
                                                BoxProps::new()
                                                    .width(Length::Fixed(crate::geom::dp(18.0))),
                                                |ui| {
                                                    if checked {
                                                        ui.icon(crate::icons::Icon::Check)
                                                            .size(crate::geom::dp(16.0));
                                                    }
                                                },
                                            );
                                        });
                                    }
                                    _ => {
                                        if let Some(shortcut) = opts.shortcut {
                                            ui.part("shortcut", |ui| {
                                                ui.text(shortcut)
                                                    .style(crate::style::dropdown_shortcut_style());
                                            });
                                        }
                                    }
                                }
                            },
                        );
                    },
                )
                .semantic_role(role)
                .semantic_checked(checked)
                .presented(opts.presented)
                .on_press(move || cb(activation));
            if let Some(id) = opts.automation_id {
                action = action.automation_id(id);
            }
            drop(action);
        });
    }

    pub fn separator(&mut self) {
        let width = self.width;
        self.ui.part("separator", |ui| {
            ui.column(
                Column::new().padding(Space::Xs).align(Align::Center),
                |ui| {
                    ui.box_(
                        BoxProps::new()
                            .width(Length::Fixed(crate::geom::dp(width)))
                            .height(Length::Fixed(crate::geom::dp(1.0)))
                            .style(crate::style::BoxStyle {
                                background: crate::style::Color::Role(ColorRole::Border),
                                ..Default::default()
                            }),
                        |_| {},
                    );
                },
            );
        });
    }
}

pub struct SettingsNavGroups<'a, 'ui, 'tx, M: 'static> {
    ui: &'a mut Ui<'ui, 'tx, M>,
    collapsed: bool,
    selected: &'static str,
    on_select: Rc<dyn Fn(&'static str) -> M>,
}

impl<'a, 'ui, 'tx, M: 'static> SettingsNavGroups<'a, 'ui, 'tx, M> {
    pub fn group(
        &mut self,
        key: &'static str,
        label: &'static str,
        draw: impl FnOnce(&mut SettingsNavItems<'_, '_, '_, M>),
    ) {
        let collapsed = self.collapsed;
        let selected = self.selected;
        let on_select = self.on_select.clone();
        self.ui.part(&format!("group[{key}]"), |ui| {
            ui.column(
                Column::new()
                    .align(Align::Stretch)
                    .padding(Space::Sm)
                    .gap(Space::None),
                |ui| {
                    if !collapsed {
                        ui.part("group-label", |ui| {
                            ui.text(label)
                                .style(crate::style::settings_nav_group_style())
                                .chrome(crate::style::settings_nav_group_box())
                                .height(Length::Fixed(crate::geom::dp(32.0)));
                        });
                    }
                    draw(&mut SettingsNavItems {
                        ui,
                        collapsed,
                        selected,
                        on_select,
                    });
                },
            );
        });
        self.ui.set_role_last(SemanticRole::Group);
    }
}

pub struct SettingsNavItems<'a, 'ui, 'tx, M: 'static> {
    ui: &'a mut Ui<'ui, 'tx, M>,
    collapsed: bool,
    selected: &'static str,
    on_select: Rc<dyn Fn(&'static str) -> M>,
}

impl<'a, 'ui, 'tx, M: 'static> SettingsNavItems<'a, 'ui, 'tx, M> {
    pub fn item(
        &mut self,
        key: &'static str,
        label: &'static str,
        icon: crate::icons::Icon,
        opts: SettingsNavItemOpts,
    ) {
        let id = opts.automation_id.unwrap_or_else(|| settings_nav_id(key));
        let cb = self.on_select.clone();
        stage_settings_nav_item(
            self.ui,
            id,
            key,
            label,
            icon,
            self.selected == key,
            self.collapsed,
            opts,
            Rc::new(move || cb(key)),
        );
    }
}

fn settings_nav_id(key: &'static str) -> &'static str {
    match key {
        "general" => "settings-nav.general",
        "appearance" => "settings-nav.appearance",
        "agents" => "settings-nav.agents",
        "voice" => "settings-nav.voice",
        "integrations" => "settings-nav.integrations",
        "advanced" => "settings-nav.advanced",
        _ => key,
    }
}

fn stage_settings_nav_item<'ui, 'tx, M: 'static>(
    ui: &mut Ui<'ui, 'tx, M>,
    id: &str,
    part_key: &str,
    label: &'static str,
    icon: crate::icons::Icon,
    active: bool,
    collapsed: bool,
    opts: SettingsNavItemOpts,
    on_press: Rc<dyn Fn() -> M>,
) {
    let item_id = id.to_string();
    ui.part(&format!("item[{part_key}]"), |ui| {
        let mut action = ui
            .action(
                Action::new()
                    .label(label)
                    .style(crate::style::settings_nav_item_style(active))
                    .disabled(opts.disabled),
                |ui| {
                    ui.row(
                        Row::new()
                            .width(Length::Fill(1))
                            .height(Length::Fixed(crate::geom::dp(32.0)))
                            .align(Align::Center)
                            .justify(if collapsed {
                                Justify::Center
                            } else {
                                Justify::Start
                            })
                            .gap(if collapsed { Space::None } else { Space::Sm })
                            .padding(if collapsed { Space::None } else { Space::Sm }),
                        |ui| {
                            ui.icon(icon)
                                .size(crate::geom::dp(16.0))
                                .color_role(if active {
                                    ColorRole::SidebarAccentForeground
                                } else {
                                    ColorRole::SidebarForeground
                                });
                            if !collapsed {
                                let style = crate::style::settings_nav_item_text_style(active);
                                ui.text(label).style(style);
                            }
                        },
                    );
                },
            )
            .semantic_role(SemanticRole::SettingsNavItem)
            .semantic_checked(Some(active))
            .presented(opts.presented)
            .on_press(move || on_press());
        if let Some(id) = opts.automation_id {
            action = action.automation_id(id);
        } else {
            action = action.automation_id(&item_id);
        }
        drop(action);
    });
}

/// `ui.input`/`ui.textarea` builder — same surface as `TextInputBuilder`;
/// Drop stages the editor+overlay stack instead of a bare editor leaf.
pub struct InputBuilder<'a, 'ui, 'b, M: 'static> {
    automation_id: Option<Rc<str>>,
    tx: &'a mut Tx<'ui>,
    value: &'b TextValue,
    multiline: bool,
    placeholder: Option<Rc<str>>,
    read_only: bool,
    disabled: bool,
    submit: SubmitPolicy,
    max_lines: Option<u32>,
    accessible_label: Option<Rc<str>>,
    invalid: bool,
    selection: Option<(usize, usize)>,
    /// authored exact line box — `None` keeps the font's natural height
    line_height: Option<crate::geom::Dp>,
    presented: Presented,
    patch: crate::style::TextInputStylePatch,
    visibility: Visibility,
    layout: LayoutSpec,
    factories: EventFactorySet,
    _m: std::marker::PhantomData<fn(&'ui (), &'b ()) -> M>,
}

impl<'a, 'ui, 'b, M: 'static> InputBuilder<'a, 'ui, 'b, M> {
    fn new(tx: &'a mut Tx<'ui>, value: &'b TextValue, multiline: bool) -> Self {
        InputBuilder {
            automation_id: None,
            tx,
            value,
            multiline,
            placeholder: None,
            read_only: false,
            disabled: false,
            submit: if multiline {
                SubmitPolicy::None
            } else {
                SubmitPolicy::default()
            },
            max_lines: None,
            accessible_label: None,
            invalid: false,
            selection: None,
            line_height: None,
            presented: Presented::default(),
            patch: crate::style::TextInputStylePatch::default(),
            visibility: Visibility::Visible,
            layout: LayoutSpec::default(),
            factories: EventFactorySet::default(),
            _m: std::marker::PhantomData,
        }
    }
    pub fn automation_id(mut self, id: &str) -> Self {
        self.automation_id = Some(Rc::from(id));
        self
    }
    pub fn style(mut self, p: crate::style::TextInputStylePatch) -> Self {
        self.patch.merge(&p);
        self
    }
    pub fn invalid(mut self, on: bool) -> Self {
        self.invalid = on;
        self
    }
    /// staged selection — UTF-16 `[start, end)` applied to the peer while
    /// staged; omit to leave selection to native/user control
    pub fn selection(mut self, start: usize, end: usize) -> Self {
        self.selection = Some((start, end));
        self
    }
    /// exact line box — the authored Dp count is calibrated into the
    /// peer font (D2DDWRITE ignores paragraph line-spacing; pitch comes
    /// from `CHARFORMAT.yHeight`). Omit for natural height. Mount-time
    /// only: not diffed/applied on update — a changed value needs a
    /// remount (same as `multiline`)
    pub fn line_height(mut self, h: crate::geom::Dp) -> Self {
        self.line_height = Some(h);
        self
    }
    /// presentation-only staged visual state (`Presented` docs)
    pub fn presented(mut self, p: Presented) -> Self {
        self.presented = p;
        self
    }
    pub fn placeholder(mut self, p: &str) -> Self {
        self.placeholder = Some(Rc::from(p));
        self
    }
    pub fn read_only(mut self, r: bool) -> Self {
        self.read_only = r;
        self
    }
    pub fn label(mut self, l: &str) -> Self {
        self.accessible_label = Some(Rc::from(l));
        self
    }
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
    pub fn submit_policy(mut self, p: SubmitPolicy) -> Self {
        self.submit = p;
        self
    }
    pub fn max_lines(mut self, n: u32) -> Self {
        self.max_lines = Some(n);
        self
    }
    pub fn visibility(mut self, v: Visibility) -> Self {
        self.visibility = v;
        self
    }
    pub fn layout(mut self, l: LayoutSpec) -> Self {
        self.layout = l;
        self
    }
    pub fn width(mut self, w: crate::geom::Length) -> Self {
        self.layout.width = w;
        self
    }
    pub fn height(mut self, h: crate::geom::Length) -> Self {
        self.layout.height = h;
        self
    }
    pub fn on_edit(mut self, f: impl Fn(TextEdit) -> M + 'static) -> Self {
        self.factories.on_edit = Some(Box::new(move |e| Box::new(f(e))));
        self
    }
    pub fn on_submit(mut self, f: impl Fn() -> M + 'static) -> Self {
        self.factories.on_submit = Some(Box::new(move || Box::new(f())));
        self
    }
    pub fn on_conflict(mut self, f: impl Fn(TextConflict) -> M + 'static) -> Self {
        self.factories.on_conflict = Some(Box::new(move |e| Box::new(f(e))));
        self
    }
    pub fn on_selection_changed(mut self, f: impl Fn(TextSelection) -> M + 'static) -> Self {
        self.factories.on_selection_changed = Some(Box::new(move |e| Box::new(f(e))));
        self
    }
}

impl<'a, 'ui, 'b, M: 'static> Drop for InputBuilder<'a, 'ui, 'b, M> {
    /// Stage the two-node stack: (1) editor leaf carrying the automation
    /// id + factories at the full element rect with the borderless
    /// `input_editor_chrome`; (2) overlay `box_` painted after, carrying
    /// border/radii-10/invalid/disabled-translucent. The stack container
    /// takes this builder's layout so element geometry is unchanged.
    fn drop(&mut self) {
        let kind = if self.multiline {
            crate::node::KIND_TEXT_AREA
        } else {
            crate::node::KIND_TEXT_INPUT
        };
        let snapshot = self.value.snapshot();
        let (
            placeholder,
            accessible_label,
            read_only,
            submit,
            max_lines,
            disabled,
            invalid,
            multiline,
            patch,
            layout,
            visibility,
            selection,
            line_height,
            presented,
        ) = (
            self.placeholder.clone(),
            self.accessible_label.clone(),
            self.read_only,
            self.submit,
            self.max_lines,
            self.disabled,
            self.invalid,
            self.multiline,
            self.patch,
            self.layout,
            self.visibility,
            self.selection,
            self.line_height,
            self.presented,
        );
        // authored foreground still reaches the peer; chrome is the stack
        // chrome (author's `chrome` fields merge over the frozen inset),
        // flattened to a full patch so the recipe's border/radii are gone
        let mut chrome = crate::style::input_editor_chrome(multiline);
        chrome.patch(&patch.chrome);
        let mut epatch = patch;
        epatch.chrome = full_box_patch(&chrome);
        let auto = self.automation_id.clone();
        let mut factories = std::mem::take(&mut self.factories);
        self.tx.stage_container(
            crate::node::KIND_STACK,
            None,
            ContainerProps::default(),
            layout,
            EventFactorySet::default(),
            visibility,
            move |ui: &mut Ui<'_, '_, M>| {
                let before = ui.tx.frames.last().unwrap().children.len();
                ui.tx.stage_leaf(
                    kind,
                    None,
                    Visibility::Visible,
                    LayoutSpec::default(),
                    std::mem::take(&mut factories),
                    move |_| NodeData::Editor {
                        snapshot,
                        multiline,
                        read_only,
                        disabled,
                        submit,
                        max_lines,
                        placeholder,
                        accessible_label,
                        patch: epatch,
                        invalid,
                        selection,
                        line_height,
                        sync: Default::default(),
                    },
                );
                if ui.tx.frames.last().unwrap().children.len() > before
                    && let Some(i) = ui.tx.frames.last().unwrap().children.last().copied()
                {
                    if let Some(id) = auto {
                        ui.tx.nodes[i as usize].automation_id = Some(id);
                    }
                    ui.tx.nodes[i as usize].presented = presented;
                }
                ui.box_(
                    crate::style::BoxProps::new()
                        .style(crate::style::input_overlay_style(disabled, invalid)),
                    |_| {},
                );
            },
            false,
            |props| NodeData::Container {
                kind: crate::node::KIND_STACK,
                props,
            },
        );
    }
}

/// Flatten a resolved `BoxStyle` into a full `BoxStylePatch` — every
/// field Some/Set so a base recipe's values can never leak through the
/// merge (used by the input/textarea stack chrome).
fn full_box_patch(b: &crate::style::BoxStyle) -> crate::style::BoxStylePatch {
    let mut p = crate::style::BoxStylePatch::default();
    p.background = Some(b.background);
    for (dst, src) in [
        (&mut p.border.top, &b.border.top),
        (&mut p.border.right, &b.border.right),
        (&mut p.border.bottom, &b.border.bottom),
        (&mut p.border.left, &b.border.left),
    ] {
        dst.width = Some(src.width);
        dst.color = Some(src.color);
    }
    for (dst, src) in [
        (&mut p.radii.top_left, &b.radii.top_left),
        (&mut p.radii.top_right, &b.radii.top_right),
        (&mut p.radii.bottom_right, &b.radii.bottom_right),
        (&mut p.radii.bottom_left, &b.radii.bottom_left),
    ] {
        *dst = Some(*src);
    }
    for (dst, src) in [
        (&mut p.padding.top, &b.padding.top),
        (&mut p.padding.right, &b.padding.right),
        (&mut p.padding.bottom, &b.padding.bottom),
        (&mut p.padding.left, &b.padding.left),
    ] {
        *dst = Some(*src);
    }
    p.shadow = match &b.shadow {
        Some(s) => crate::style::ShadowPatch::Set(*s),
        None => crate::style::ShadowPatch::Remove,
    };
    p
}
