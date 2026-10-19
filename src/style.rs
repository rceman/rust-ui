//! Authored style vocabulary per `docs/STYLE_CUSTOMIZATION_MODEL.md` —
//! `Color` (role or explicit rgba), per-side `Border`/`Insets`, per-corner
//! `CornerRadii`, one optional outer `Shadow`, `BoxStyle`/`TextStyle`/
//! `VisualStyle` full styles, and sparse `*Patch` types whose `None` fields
//! mean "untouched". Resolution order is fixed: theme tokens -> defaults ->
//! variant -> size -> recipe state overlay -> consumer base patch ->
//! consumer state patch -> OS enforcement.

use crate::geom::Dp;
use crate::theme::ColorRole;

/// Authored color — a semantic theme role or straight sRGB channels.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Color {
    Role(ColorRole),
    Rgba { r: u8, g: u8, b: u8, a: u8 },
}

impl Color {
    pub const fn role(role: ColorRole) -> Self {
        Color::Role(role)
    }
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color::Rgba { r, g, b, a: 255 }
    }
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Color::Rgba { r, g, b, a }
    }
}

impl Default for Color {
    /// transparent — keeps every composite `Default` coherent
    fn default() -> Self {
        Color::rgba(0, 0, 0, 0)
    }
}

#[derive(Copy, Clone, Default, PartialEq, Debug)]
pub struct Insets {
    pub top: Dp,
    pub right: Dp,
    pub bottom: Dp,
    pub left: Dp,
}

impl Insets {
    pub fn all(v: Dp) -> Self {
        Insets {
            top: v,
            right: v,
            bottom: v,
            left: v,
        }
    }
}

#[derive(Copy, Clone, Default, PartialEq, Debug)]
pub struct CornerRadii {
    pub top_left: Dp,
    pub top_right: Dp,
    pub bottom_right: Dp,
    pub bottom_left: Dp,
}

impl CornerRadii {
    pub fn all(v: Dp) -> Self {
        CornerRadii {
            top_left: v,
            top_right: v,
            bottom_right: v,
            bottom_left: v,
        }
    }
    /// uniform value when all corners match (D2D fast path)
    pub fn uniform(&self) -> Option<Dp> {
        if self.top_left == self.top_right
            && self.top_right == self.bottom_right
            && self.bottom_right == self.bottom_left
        {
            Some(self.top_left)
        } else {
            None
        }
    }
    /// Proportional normalization — the CSS rule: when the radii sharing
    /// one edge exceed it, ALL corners scale down by the same factor so
    /// the authored proportions survive. One factor across every pair
    /// keeps the shape stable; independent per-corner clamps distort it.
    pub fn normalized(&self, w: f32, h: f32) -> Self {
        let (tl, tr, br, bl) = (
            self.top_left.0.max(0.0),
            self.top_right.0.max(0.0),
            self.bottom_right.0.max(0.0),
            self.bottom_left.0.max(0.0),
        );
        let pair = |a: f32, b: f32, edge: f32| {
            if a + b > edge && a + b > 0.0 {
                edge / (a + b)
            } else {
                1.0
            }
        };
        let s = 1.0f32
            .min(pair(tl, tr, w))
            .min(pair(bl, br, w))
            .min(pair(tl, bl, h))
            .min(pair(tr, br, h));
        CornerRadii {
            top_left: Dp(tl * s),
            top_right: Dp(tr * s),
            bottom_right: Dp(br * s),
            bottom_left: Dp(bl * s),
        }
    }
}

#[derive(Copy, Clone, Default, PartialEq, Debug)]
pub struct BorderSide {
    pub width: Dp,
    pub color: Color,
}

impl BorderSide {
    pub fn new(width: Dp, color: Color) -> Self {
        BorderSide { width, color }
    }
}

#[derive(Copy, Clone, Default, PartialEq, Debug)]
pub struct Border {
    pub top: BorderSide,
    pub right: BorderSide,
    pub bottom: BorderSide,
    pub left: BorderSide,
}

impl Border {
    pub fn all(side: BorderSide) -> Self {
        Border {
            top: side,
            right: side,
            bottom: side,
            left: side,
        }
    }
    /// uniform side when all four match (D2D fast path)
    pub fn uniform(&self) -> Option<BorderSide> {
        if self.top == self.right && self.right == self.bottom && self.bottom == self.left {
            Some(self.top)
        } else {
            None
        }
    }
    pub fn any(&self) -> bool {
        [self.top, self.right, self.bottom, self.left]
            .iter()
            .any(|s| s.width.0 > 0.0)
    }
}

/// v0.1's only effect primitive — one optional outer box shadow.
/// `blur_sigma` is a Gaussian standard deviation in logical dp (not a CSS
/// blur-radius); `offset_x`/`offset_y` are signed; `dp(0.0)` sigma is a
/// sharp offset silhouette. `spread` uniformly expands the shadow
/// silhouette (the CSS `0 0 0 Npx` ring primitive — `spread:1 blur:0`
/// is the card's ring-1 edge); negative spread contracts it. No inset,
/// no list.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Shadow {
    pub color: Color,
    pub offset_x: Dp,
    pub offset_y: Dp,
    pub blur_sigma: Dp,
    pub spread: Dp,
}

#[derive(Copy, Clone, Default, PartialEq, Debug)]
pub struct BoxStyle {
    pub background: Color,
    pub border: Border,
    pub radii: CornerRadii,
    pub padding: Insets,
    pub shadow: Option<Shadow>,
}

impl BoxStyle {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn background(mut self, color: Color) -> Self {
        self.background = color;
        self
    }
    pub fn border(mut self, border: Border) -> Self {
        self.border = border;
        self
    }
    pub fn radii(mut self, radii: CornerRadii) -> Self {
        self.radii = radii;
        self
    }
    pub fn padding(mut self, padding: Insets) -> Self {
        self.padding = padding;
        self
    }
    pub fn shadow(mut self, shadow: Option<Shadow>) -> Self {
        self.shadow = shadow;
        self
    }
    /// Merge a sparse patch over this authored style — `None` untouched,
    /// explicit values (incl. zero/transparent) override. `ShadowPatch` is
    /// atomic set/remove/unchanged.
    pub fn patch(&mut self, p: &BoxStylePatch) {
        if let Some(c) = p.background {
            self.background = c;
        }
        macro_rules! side {
            ($b:ident) => {{
                if let Some(w) = p.border.$b.width {
                    self.border.$b.width = w;
                }
                if let Some(c) = p.border.$b.color {
                    self.border.$b.color = c;
                }
            }};
        }
        side!(top);
        side!(right);
        side!(bottom);
        side!(left);
        macro_rules! corner {
            ($c:ident) => {{
                if let Some(v) = p.radii.$c {
                    self.radii.$c = v;
                }
            }};
        }
        corner!(top_left);
        corner!(top_right);
        corner!(bottom_right);
        corner!(bottom_left);
        macro_rules! inset {
            ($i:ident) => {{
                if let Some(v) = p.padding.$i {
                    self.padding.$i = v;
                }
            }};
        }
        inset!(top);
        inset!(right);
        inset!(bottom);
        inset!(left);
        match &p.shadow {
            ShadowPatch::Set(s) => self.shadow = Some(*s),
            ShadowPatch::Remove => self.shadow = None,
            ShadowPatch::Unchanged => {}
        }
    }
}

/// Frozen `typography.weight_values` — numeric CSS weights.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum TextWeight {
    /// 400
    Normal,
    /// 500
    Medium,
    /// 600
    Semibold,
    /// 700
    Bold,
    /// 800
    Extrabold,
}

/// `Body` resolves to the theme base size; `Exact(dp)` is a requested
/// unscaled logical size (OS text scaling still applies).
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum TextSize {
    Body,
    Exact(Dp),
}

/// Display text style — foreground, size, weight, family, line box and
/// inline decorations. `line_height` is the per-line pitch in dp when set
/// (the backend centers the glyph band inside it, matching CSS
/// `line-height`); `letter_spacing` is a signed dp advance applied per
/// cluster (Tailwind `tracking-*`); `underline` draws the inline
/// decoration at the backend's underline position.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct TextStyle {
    pub foreground: Color,
    pub size: TextSize,
    pub weight: TextWeight,
    pub font: crate::theme::FontFamily,
    pub line_height: Option<Dp>,
    pub underline: bool,
    pub letter_spacing: f32,
}

impl TextStyle {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn foreground(mut self, color: Color) -> Self {
        self.foreground = color;
        self
    }
    pub fn size(mut self, size: Dp) -> Self {
        self.size = TextSize::Exact(size);
        self
    }
    pub fn weight(mut self, weight: TextWeight) -> Self {
        self.weight = weight;
        self
    }
    pub fn font(mut self, font: crate::theme::FontFamily) -> Self {
        self.font = font;
        self
    }
    pub fn line_height(mut self, lh: Dp) -> Self {
        self.line_height = Some(lh);
        self
    }
    pub fn underline(mut self, on: bool) -> Self {
        self.underline = on;
        self
    }
    pub fn letter_spacing(mut self, dp: f32) -> Self {
        self.letter_spacing = dp;
        self
    }
}

impl Default for TextStyle {
    fn default() -> Self {
        TextStyle {
            foreground: Color::Role(ColorRole::Foreground),
            size: TextSize::Body,
            weight: TextWeight::Normal,
            font: crate::theme::FontFamily::Sans,
            line_height: None,
            underline: false,
            letter_spacing: 0.0,
        }
    }
}

impl TextStyle {
    pub fn patch(&mut self, p: &TextStylePatch) {
        if let Some(c) = p.foreground {
            self.foreground = c;
        }
        if let Some(d) = p.size {
            self.size = TextSize::Exact(d);
        }
        if let Some(w) = p.weight {
            self.weight = w;
        }
        if let Some(f) = p.font {
            self.font = f;
        }
        if let Some(lh) = p.line_height {
            self.line_height = Some(lh);
        }
        if let Some(u) = p.underline {
            self.underline = u;
        }
        if let Some(ls) = p.letter_spacing {
            self.letter_spacing = ls;
        }
    }
}

/// A component's resolved visual surface — box + text parts + the
/// required focus indicator color. `focus_ring` is the RESOLVED ring
/// color (a recipe-set role): the painter's enforcement layer consumes it
/// so neither recipe internals nor consumer patches can erase focus
/// visibility.
#[derive(Copy, Clone, Default, PartialEq, Debug)]
pub struct VisualStyle {
    pub box_style: BoxStyle,
    pub text_style: TextStyle,
    pub focus_ring: Color,
}

impl VisualStyle {
    pub fn patch(&mut self, p: &VisualStylePatch) {
        self.box_style.patch(&p.box_style);
        self.text_style.patch(&p.text_style);
    }
}

// ---------------------------------------------------------------------------
// sparse patches — None = untouched; overrides travel by named field only
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, Default, PartialEq, Debug)]
pub struct BorderSidePatch {
    pub width: Option<Dp>,
    pub color: Option<Color>,
}

impl BorderSidePatch {
    pub fn width(mut self, w: Dp) -> Self {
        self.width = Some(w);
        self
    }
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }
}

#[derive(Copy, Clone, Default, PartialEq, Debug)]
pub struct BorderPatch {
    pub top: BorderSidePatch,
    pub right: BorderSidePatch,
    pub bottom: BorderSidePatch,
    pub left: BorderSidePatch,
}

impl BorderPatch {
    pub fn all(mut self, side: BorderSidePatch) -> Self {
        self.top = side;
        self.right = side;
        self.bottom = side;
        self.left = side;
        self
    }
}

#[derive(Copy, Clone, Default, PartialEq, Debug)]
pub struct CornerRadiiPatch {
    pub top_left: Option<Dp>,
    pub top_right: Option<Dp>,
    pub bottom_right: Option<Dp>,
    pub bottom_left: Option<Dp>,
}

#[derive(Copy, Clone, Default, PartialEq, Debug)]
pub struct InsetsPatch {
    pub top: Option<Dp>,
    pub right: Option<Dp>,
    pub bottom: Option<Dp>,
    pub left: Option<Dp>,
}

impl InsetsPatch {
    pub fn all(v: Dp) -> Self {
        InsetsPatch {
            top: Some(v),
            right: Some(v),
            bottom: Some(v),
            left: Some(v),
        }
    }
}

/// Atomic optional-property command for shadow — a partial shadow patch
/// modifies a shadow that may not exist, so the unit is the whole optional.
#[derive(Copy, Clone, Default, PartialEq, Debug)]
pub enum ShadowPatch {
    #[default]
    Unchanged,
    Set(Shadow),
    Remove,
}

#[derive(Copy, Clone, Default, PartialEq, Debug)]
pub struct BoxStylePatch {
    pub background: Option<Color>,
    pub border: BorderPatch,
    pub radii: CornerRadiiPatch,
    pub padding: InsetsPatch,
    pub shadow: ShadowPatch,
}

impl BoxStylePatch {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn background(mut self, c: Color) -> Self {
        self.background = Some(c);
        self
    }
    pub fn border(mut self, b: BorderPatch) -> Self {
        self.border = b;
        self
    }
    /// merge (fieldwise) — later `Some` wins within this build
    pub fn merge(&mut self, o: &BoxStylePatch) {
        macro_rules! m {
            ($f:ident) => {{
                if o.$f.is_some() {
                    self.$f = o.$f;
                }
            }};
        }
        m!(background);
        macro_rules! side {
            ($b:ident) => {{
                if o.border.$b.width.is_some() {
                    self.border.$b.width = o.border.$b.width;
                }
                if o.border.$b.color.is_some() {
                    self.border.$b.color = o.border.$b.color;
                }
            }};
        }
        side!(top);
        side!(right);
        side!(bottom);
        side!(left);
        macro_rules! rc {
            ($c:ident) => {{
                if o.radii.$c.is_some() {
                    self.radii.$c = o.radii.$c;
                }
            }};
        }
        rc!(top_left);
        rc!(top_right);
        rc!(bottom_right);
        rc!(bottom_left);
        macro_rules! ri {
            ($i:ident) => {{
                if o.padding.$i.is_some() {
                    self.padding.$i = o.padding.$i;
                }
            }};
        }
        ri!(top);
        ri!(right);
        ri!(bottom);
        ri!(left);
        if o.shadow != ShadowPatch::Unchanged {
            self.shadow = o.shadow;
        }
    }
}

#[derive(Copy, Clone, Default, PartialEq, Debug)]
pub struct TextStylePatch {
    pub foreground: Option<Color>,
    pub size: Option<Dp>,
    pub weight: Option<TextWeight>,
    pub font: Option<crate::theme::FontFamily>,
    pub line_height: Option<Dp>,
    pub underline: Option<bool>,
    pub letter_spacing: Option<f32>,
}

impl TextStylePatch {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn foreground(mut self, c: Color) -> Self {
        self.foreground = Some(c);
        self
    }
    pub fn size(mut self, d: Dp) -> Self {
        self.size = Some(d);
        self
    }
    pub fn weight(mut self, w: TextWeight) -> Self {
        self.weight = Some(w);
        self
    }
    pub fn font(mut self, f: crate::theme::FontFamily) -> Self {
        self.font = Some(f);
        self
    }
    pub fn line_height(mut self, lh: Dp) -> Self {
        self.line_height = Some(lh);
        self
    }
    pub fn underline(mut self, on: bool) -> Self {
        self.underline = Some(on);
        self
    }
    pub fn letter_spacing(mut self, dp: f32) -> Self {
        self.letter_spacing = Some(dp);
        self
    }
    pub fn merge(&mut self, o: &TextStylePatch) {
        if o.foreground.is_some() {
            self.foreground = o.foreground;
        }
        if o.size.is_some() {
            self.size = o.size;
        }
        if o.weight.is_some() {
            self.weight = o.weight;
        }
        if o.font.is_some() {
            self.font = o.font;
        }
        if o.line_height.is_some() {
            self.line_height = o.line_height;
        }
        if o.underline.is_some() {
            self.underline = o.underline;
        }
        if o.letter_spacing.is_some() {
            self.letter_spacing = o.letter_spacing;
        }
    }
}

/// The capability-limited patch for native editable text
/// (`docs/STYLE_CUSTOMIZATION_MODEL.md` "Native editable text boundary").
/// `chrome` maps to the painted frame around the peer (background/border/
/// radii/padding/shadow); only `foreground` reaches the peer through the
/// typed adapter — selection, caret, IME colors and fonts stay OS-owned.
/// There are deliberately no size/weight fields on native text.
#[derive(Copy, Clone, Default, PartialEq, Debug)]
pub struct TextInputStylePatch {
    pub chrome: BoxStylePatch,
    pub foreground: Option<Color>,
}

impl TextInputStylePatch {
    pub fn new() -> Self {
        Self::default()
    }
    /// peer foreground — routed through the typed adapter; v0.1 requires
    /// it to resolve fully opaque
    pub fn foreground(mut self, c: Color) -> Self {
        self.foreground = Some(c);
        self
    }
    /// chrome shorthand — the peer backing derives from this background
    pub fn background(mut self, c: Color) -> Self {
        self.chrome.background = Some(c);
        self
    }
    pub fn border_bottom_width(mut self, w: Dp) -> Self {
        self.chrome.border.bottom.width = Some(w);
        self
    }
    pub fn border_bottom_color(mut self, c: Color) -> Self {
        self.chrome.border.bottom.color = Some(c);
        self
    }
    /// fieldwise merge — `None` preserves, `Some` wins
    pub fn merge(&mut self, o: &TextInputStylePatch) {
        self.chrome.merge(&o.chrome);
        if o.foreground.is_some() {
            self.foreground = o.foreground;
        }
    }
}

#[derive(Copy, Clone, Default, PartialEq, Debug)]
pub struct VisualStylePatch {
    pub box_style: BoxStylePatch,
    pub text_style: TextStylePatch,
}

impl VisualStylePatch {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn merge(&mut self, o: &VisualStylePatch) {
        self.box_style.merge(&o.box_style);
        self.text_style.merge(&o.text_style);
    }
}

// ---------------------------------------------------------------------------
// resolved-concrete comparison — authored identity vs effective value
// ---------------------------------------------------------------------------

/// Are two borders concretely identical under the current theme? Two
/// authored roles collapsing to the same RGBA are a no-op; authored
/// identity alone must not force work.
fn side_resolved_eq(a: &BorderSide, b: &BorderSide, dark: bool) -> bool {
    a.width == b.width && resolve_color(a.color, dark) == resolve_color(b.color, dark)
}

fn shadow_resolved_eq(a: &Option<Shadow>, b: &Option<Shadow>, dark: bool) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => {
            x.offset_x == y.offset_x
                && x.offset_y == y.offset_y
                && x.blur_sigma == y.blur_sigma
                && x.spread == y.spread
                && resolve_color(x.color, dark) == resolve_color(y.color, dark)
        }
        _ => false,
    }
}

/// RESOLVED-concrete box equality — the dirty classifier's no-op gate.
/// Geometry fields compare authored values (a Dp is already concrete);
/// colors compare the effective RGBA under the current theme.
pub(crate) fn box_resolved_eq(a: &BoxStyle, b: &BoxStyle, dark: bool) -> bool {
    a.padding == b.padding
        && a.radii == b.radii
        && resolve_color(a.background, dark) == resolve_color(b.background, dark)
        && side_resolved_eq(&a.border.top, &b.border.top, dark)
        && side_resolved_eq(&a.border.right, &b.border.right, dark)
        && side_resolved_eq(&a.border.bottom, &b.border.bottom, dark)
        && side_resolved_eq(&a.border.left, &b.border.left, dark)
        && shadow_resolved_eq(&a.shadow, &b.shadow, dark)
}

/// RESOLVED-concrete visual equality — box + text foreground/metrics.
pub(crate) fn visual_resolved_eq(a: &VisualStyle, b: &VisualStyle, dark: bool) -> bool {
    a.text_style.size == b.text_style.size
        && a.text_style.weight == b.text_style.weight
        && a.text_style.font == b.text_style.font
        && a.text_style.line_height == b.text_style.line_height
        && a.text_style.underline == b.text_style.underline
        && a.text_style.letter_spacing == b.text_style.letter_spacing
        && resolve_color(a.text_style.foreground, dark)
            == resolve_color(b.text_style.foreground, dark)
        && resolve_color(a.focus_ring, dark) == resolve_color(b.focus_ring, dark)
        && box_resolved_eq(&a.box_style, &b.box_style, dark)
}

impl BoxStylePatch {
    /// Does this patch touch a metric-bearing field? A state branch that
    /// does must drive a layout pass on state transitions, not just paint.
    pub fn metric_affecting(&self) -> bool {
        self.padding.top.is_some()
            || self.padding.right.is_some()
            || self.padding.bottom.is_some()
            || self.padding.left.is_some()
            || self.radii.top_left.is_some()
            || self.radii.top_right.is_some()
            || self.radii.bottom_right.is_some()
            || self.radii.bottom_left.is_some()
            || self.border.top.width.is_some()
            || self.border.right.width.is_some()
            || self.border.bottom.width.is_some()
            || self.border.left.width.is_some()
    }
}

impl TextStylePatch {
    /// font metrics — size/weight/family/line-pitch/tracking changes
    /// reflow text
    pub fn metric_affecting(&self) -> bool {
        self.size.is_some()
            || self.weight.is_some()
            || self.font.is_some()
            || self.line_height.is_some()
            || self.letter_spacing.is_some()
    }
}

impl VisualStylePatch {
    pub fn metric_affecting(&self) -> bool {
        self.box_style.metric_affecting() || self.text_style.metric_affecting()
    }
}

impl StateStyles<VisualStylePatch> {
    /// any state branch carry metrics? — the transition classifier consults
    /// this once per node; the node's own patch set decides, not the state
    pub fn any_metric_branch(&self) -> bool {
        self.base.metric_affecting()
            || self
                .hover
                .as_ref()
                .is_some_and(VisualStylePatch::metric_affecting)
            || self
                .pressed
                .as_ref()
                .is_some_and(VisualStylePatch::metric_affecting)
            || self
                .disabled
                .as_ref()
                .is_some_and(VisualStylePatch::metric_affecting)
            || self
                .focus_visible
                .as_ref()
                .is_some_and(VisualStylePatch::metric_affecting)
    }
}

/// Per-state partial overlays — disabled/pressed/hover/normal is an
/// exclusive priority chain; `focus_visible` is orthogonal (applies after
/// the resolved state branch).
#[derive(Copy, Clone, Default, PartialEq, Debug)]
pub struct StateStyles<P> {
    pub base: P,
    pub hover: Option<P>,
    pub pressed: Option<P>,
    pub disabled: Option<P>,
    pub focus_visible: Option<P>,
}

/// The capability-typed patch surface for `button`/`icon_button`.
#[derive(Copy, Clone, Default, PartialEq, Debug)]
pub struct ButtonStylePatch {
    pub styles: StateStyles<VisualStylePatch>,
}

impl ButtonStylePatch {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn background(mut self, color: Color) -> Self {
        self.styles.base.box_style.background = Some(color);
        self
    }
    pub fn foreground(mut self, color: Color) -> Self {
        self.styles.base.text_style.foreground = Some(color);
        self
    }
    pub fn border_bottom_width(mut self, width: Dp) -> Self {
        self.styles.base.box_style.border.bottom.width = Some(width);
        self
    }
    pub fn border_bottom_color(mut self, color: Color) -> Self {
        self.styles.base.box_style.border.bottom.color = Some(color);
        self
    }
    pub fn hover(mut self, patch: VisualStylePatch) -> Self {
        match &mut self.styles.hover {
            Some(h) => h.merge(&patch),
            None => self.styles.hover = Some(patch),
        }
        self
    }
    pub fn pressed(mut self, patch: VisualStylePatch) -> Self {
        match &mut self.styles.pressed {
            Some(h) => h.merge(&patch),
            None => self.styles.pressed = Some(patch),
        }
        self
    }
    pub fn disabled(mut self, patch: VisualStylePatch) -> Self {
        match &mut self.styles.disabled {
            Some(h) => h.merge(&patch),
            None => self.styles.disabled = Some(patch),
        }
        self
    }
    pub fn focus_visible(mut self, patch: VisualStylePatch) -> Self {
        match &mut self.styles.focus_visible {
            Some(h) => h.merge(&patch),
            None => self.styles.focus_visible = Some(patch),
        }
        self
    }
    /// merge — a later `.style(..)` on the same node build deep-merges
    /// fieldwise; `None` never erases an earlier Some in the same build
    pub fn merge(&mut self, o: &ButtonStylePatch) {
        self.styles.base.merge(&o.styles.base);
        macro_rules! slot {
            ($s:ident) => {{
                if let Some(o) = &o.styles.$s {
                    match &mut self.styles.$s {
                        Some(m) => m.merge(o),
                        None => self.styles.$s = Some(*o),
                    }
                }
            }};
        }
        slot!(hover);
        slot!(pressed);
        slot!(disabled);
        slot!(focus_visible);
    }
}

/// `ui.action` — semantic activation wrapper: a full authored `BoxStyle`
/// base plus optional state patches (children own their own styles).
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct ActionStyle {
    pub base: BoxStyle,
    pub hover: Option<BoxStylePatch>,
    pub pressed: Option<BoxStylePatch>,
    pub disabled: Option<BoxStylePatch>,
    pub focus_visible: Option<BoxStylePatch>,
}

impl ActionStyle {
    pub fn new(base: BoxStyle) -> Self {
        ActionStyle {
            base,
            hover: None,
            pressed: None,
            disabled: None,
            focus_visible: None,
        }
    }
    /// repeated state setters MERGE fieldwise — `Some` fields overwrite
    /// and `None` preserves an earlier call, matching the patch contract
    /// (`docs/STYLE_CUSTOMIZATION_MODEL.md` §6)
    pub fn hover(mut self, patch: BoxStylePatch) -> Self {
        match &mut self.hover {
            Some(h) => h.merge(&patch),
            None => self.hover = Some(patch),
        }
        self
    }
    pub fn pressed(mut self, patch: BoxStylePatch) -> Self {
        match &mut self.pressed {
            Some(h) => h.merge(&patch),
            None => self.pressed = Some(patch),
        }
        self
    }
    pub fn disabled(mut self, patch: BoxStylePatch) -> Self {
        match &mut self.disabled {
            Some(h) => h.merge(&patch),
            None => self.disabled = Some(patch),
        }
        self
    }
    pub fn focus_visible(mut self, patch: BoxStylePatch) -> Self {
        match &mut self.focus_visible {
            Some(h) => h.merge(&patch),
            None => self.focus_visible = Some(patch),
        }
        self
    }
    /// Any state branch carry metrics? Transition classification consults
    /// this — a metric-bearing branch needs layout on enter/leave.
    pub fn any_metric_branch(&self) -> bool {
        self.hover
            .as_ref()
            .is_some_and(BoxStylePatch::metric_affecting)
            || self
                .pressed
                .as_ref()
                .is_some_and(BoxStylePatch::metric_affecting)
            || self
                .disabled
                .as_ref()
                .is_some_and(BoxStylePatch::metric_affecting)
            || self
                .focus_visible
                .as_ref()
                .is_some_and(BoxStylePatch::metric_affecting)
    }

    /// Resolve the effective `BoxStyle` for the given interaction state:
    /// exclusive disabled > pressed > hover > normal, then focus_visible.
    pub fn resolve(
        &self,
        disabled: bool,
        pressed: bool,
        hover: bool,
        focus_visible: bool,
    ) -> BoxStyle {
        let mut b = self.base;
        if disabled {
            if let Some(p) = &self.disabled {
                b.patch(p);
            }
        } else if pressed {
            if let Some(p) = &self.pressed {
                b.patch(p);
            }
        } else if hover {
            if let Some(p) = &self.hover {
                b.patch(p);
            }
        }
        if focus_visible
            && !disabled
            && let Some(p) = &self.focus_visible
        {
            b.patch(p);
        }
        b
    }
}

/// `ui.action` props — accessible name mandatory, style + disabled flag.
#[derive(Clone, PartialEq)]
pub struct Action {
    pub label: String,
    pub style: ActionStyle,
    pub disabled: bool,
}

impl Action {
    pub fn new() -> Self {
        Action {
            label: String::new(),
            style: ActionStyle::new(BoxStyle::new()),
            disabled: false,
        }
    }
    pub fn label(mut self, name: &str) -> Self {
        self.label = name.to_string();
        self
    }
    pub fn style(mut self, style: ActionStyle) -> Self {
        self.style = style;
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// `ui.box_` props — opaque: authored `BoxStyle` + retained layout props,
/// so visual style never overwrites size inputs.
#[derive(Copy, Clone, Default, PartialEq)]
pub struct BoxProps {
    pub style: BoxStyle,
    pub(crate) layout: crate::geom::LayoutSpec,
    /// show/hide — hidden boxes stage but suspend children + paint
    pub(crate) visibility: crate::Visibility,
}

impl BoxProps {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn style(mut self, style: BoxStyle) -> Self {
        self.style = style;
        self
    }
    pub fn width(mut self, width: crate::geom::Length) -> Self {
        self.layout.width = width;
        self
    }
    pub fn height(mut self, height: crate::geom::Length) -> Self {
        self.layout.height = height;
        self
    }
    pub fn visibility(mut self, v: crate::Visibility) -> Self {
        self.visibility = v;
        self
    }
    pub fn min_width(mut self, v: Dp) -> Self {
        self.layout.min_width = Some(v);
        self
    }
    pub fn max_width(mut self, v: Dp) -> Self {
        self.layout.max_width = Some(v);
        self
    }
    pub fn min_height(mut self, v: Dp) -> Self {
        self.layout.min_height = Some(v);
        self
    }
    pub fn max_height(mut self, v: Dp) -> Self {
        self.layout.max_height = Some(v);
        self
    }
}

// ---------------------------------------------------------------------------
// resolution — deterministic ordered composition; OS enforcement is a
// separate final step the backend applies to resolved colors
// ---------------------------------------------------------------------------

/// Interaction branch selection — exclusive priority, never specificity.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum StyleState {
    Disabled,
    Pressed,
    Hover,
    Normal,
}

impl StyleState {
    pub fn classify(disabled: bool, pressed: bool, hover: bool) -> Self {
        if disabled {
            StyleState::Disabled
        } else if pressed {
            StyleState::Pressed
        } else if hover {
            StyleState::Hover
        } else {
            StyleState::Normal
        }
    }
}

// ---------------------------------------------------------------------------
// color resolution — the shared palette (renderers convert to their format)
// ---------------------------------------------------------------------------

/// shadcn neutral palette per `ColorRole` — the frozen
/// `reference/shadcn-gallery-v0.1/tokens.json` sRGB values, ONE authority.
/// `rgba()` decodes `#rrggbb` + token alpha; translucent roles bake the
/// frozen alpha in.
pub fn role_color(role: ColorRole, dark: bool) -> [f32; 4] {
    use ColorRole::*;
    const fn rgba(hex: u32, a: f32) -> [f32; 4] {
        [
            ((hex >> 16) & 0xff) as f32 / 255.0,
            ((hex >> 8) & 0xff) as f32 / 255.0,
            (hex & 0xff) as f32 / 255.0,
            a,
        ]
    }
    match (role, dark) {
        (Background, false) => rgba(0xffffff, 1.0),
        (Background, true) => rgba(0x0a0a0a, 1.0),
        (Foreground, false) => rgba(0x0a0a0a, 1.0),
        (Foreground, true) => rgba(0xfafafa, 1.0),
        (Card, false) => rgba(0xffffff, 1.0),
        (Card, true) => rgba(0x171717, 1.0),
        (CardForeground, false) => rgba(0x0a0a0a, 1.0),
        (CardForeground, true) => rgba(0xfafafa, 1.0),
        (Primary, false) => rgba(0x171717, 1.0),
        (Primary, true) => rgba(0xe5e5e5, 1.0),
        (PrimaryForeground, false) => rgba(0xfafafa, 1.0),
        (PrimaryForeground, true) => rgba(0x171717, 1.0),
        (Secondary, false) => rgba(0xf5f5f5, 1.0),
        (Secondary, true) => rgba(0x262626, 1.0),
        (SecondaryForeground, false) => rgba(0x171717, 1.0),
        (SecondaryForeground, true) => rgba(0xfafafa, 1.0),
        (Muted, false) => rgba(0xf5f5f5, 1.0),
        (Muted, true) => rgba(0x262626, 1.0),
        (MutedForeground, false) => rgba(0x737373, 1.0),
        (MutedForeground, true) => rgba(0xa1a1a1, 1.0),
        (Accent, false) => rgba(0xf5f5f5, 1.0),
        (Accent, true) => rgba(0x262626, 1.0),
        (AccentForeground, false) => rgba(0x171717, 1.0),
        (AccentForeground, true) => rgba(0xfafafa, 1.0),
        (Border, false) => rgba(0xe5e5e5, 1.0),
        (Border, true) => rgba(0xffffff, 0.10),
        (Input, false) => rgba(0xe5e5e5, 1.0),
        (Input, true) => rgba(0xffffff, 0.15),
        (Ring, false) => rgba(0xa1a1a1, 1.0),
        (Ring, true) => rgba(0x737373, 1.0),
        (Surface, false) => rgba(0xf8f8f8, 1.0),
        (Surface, true) => rgba(0x161616, 1.0),
        (SurfaceForeground, false) => rgba(0x0a0a0a, 1.0),
        (SurfaceForeground, true) => rgba(0xa1a1a1, 1.0),
        (Code, false) => rgba(0xf8f8f8, 1.0),
        (Code, true) => rgba(0x161616, 1.0),
        (CodeForeground, false) => rgba(0x0a0a0a, 1.0),
        (CodeForeground, true) => rgba(0xa1a1a1, 1.0),
        (CodeHighlight, false) => rgba(0xf2f2f2, 1.0),
        (CodeHighlight, true) => rgba(0x262626, 1.0),
        (CodeNumber, false) => rgba(0x747474, 1.0),
        (CodeNumber, true) => rgba(0xa4a4a4, 1.0),
        (Selection, false) => rgba(0x000000, 1.0),
        (Selection, true) => rgba(0xe5e5e5, 1.0),
        (SelectionForeground, false) => rgba(0xffffff, 1.0),
        (SelectionForeground, true) => rgba(0x171717, 1.0),
        (Destructive, false) => rgba(0xe7000b, 1.0),
        (Destructive, true) => rgba(0xff6467, 1.0),
        (DestructiveForeground, false) => rgba(0xfcf3f3, 1.0),
        (DestructiveForeground, true) => rgba(0xdf2225, 1.0),
        // enforcement focus indicator — frozen `ring-50`
        (Focus, false) => rgba(0xa1a1a1, 0.5),
        (Focus, true) => rgba(0x737373, 0.5),
        (Shadow, false) => rgba(0x000000, 0.10),
        (Shadow, true) => rgba(0x000000, 0.35),
        // ---- translucent tokens --------------------------------------
        (Foreground10, false) => rgba(0x0a0a0a, 0.10),
        (Foreground10, true) => rgba(0xfafafa, 0.10),
        (Foreground60, false) => rgba(0x0a0a0a, 0.60),
        (Foreground60, true) => rgba(0xfafafa, 0.60),
        (Muted50, false) => rgba(0xf5f5f5, 0.5),
        (Muted50, true) => rgba(0x262626, 0.5),
        (Input30, _) => rgba(0xffffff, 0.045),
        (Input50, _) => rgba(0xffffff, 0.075),
        (Primary80, false) => rgba(0x171717, 0.8),
        (Primary80, true) => rgba(0xe5e5e5, 0.8),
        (SecondaryHover, false) => rgba(0xe7e7e7, 1.0),
        (SecondaryHover, true) => rgba(0x2f2f2f, 1.0),
        (DestructiveTint, false) => rgba(0xe7000b, 0.10),
        (DestructiveTint, true) => rgba(0xff6467, 0.20),
        (DestructiveTintHover, false) => rgba(0xe7000b, 0.20),
        (DestructiveTintHover, true) => rgba(0xff6467, 0.30),
        (DestructiveRing, false) => rgba(0xe7000b, 0.20),
        (DestructiveRing, true) => rgba(0xff6467, 0.40),
        (Destructive40, false) => rgba(0xe7000b, 0.40),
        (Destructive40, true) => rgba(0xff6467, 0.40),
        (Destructive50, _) => rgba(0xff6467, 0.50),
    }
}

/// Authored `Color` -> straight sRGB floats (backends premultiply as needed).
pub fn resolve_color(c: Color, dark: bool) -> [f32; 4] {
    match c {
        Color::Role(r) => role_color(r, dark),
        Color::Rgba { r, g, b, a } => [
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
            a as f32 / 255.0,
        ],
    }
}

// ---------------------------------------------------------------------------
// component recipes — ordinary inspectable functions/tables, theme -> values
// ---------------------------------------------------------------------------

use crate::theme::{ButtonVariant, ControlSize};

/// Button recipe — the frozen `cn-button` contract (base-nova/neutral):
/// base `VisualStyle` per variant/size + recipe state overlays as full
/// styles. Horizontal padding/radius/font per the frozen control tables;
/// `pressed` and `disabled` carry NO color change — pressed paints the
/// 1dp translate, disabled paints 0.5 opacity (both are whole-control
/// effects the style can't express). Focus-visible replaces the border
/// color (`border-ring` / `border-destructive/40`); the ring band itself
/// is the renderer's enforcement layer.
pub fn button_recipe(
    variant: ButtonVariant,
    size: ControlSize,
    dark: bool,
) -> StateStyles<VisualStyle> {
    let (pad_x, radius, font_px) = match size {
        ControlSize::Xs => (8.0, 8.0, 12.0),
        ControlSize::Sm => (10.0, 8.0, 12.8),
        ControlSize::Md => (10.0, 10.0, 14.0),
        ControlSize::Lg => (10.0, 10.0, 14.0),
    };
    let outline_border = if dark {
        Color::Role(ColorRole::Input)
    } else {
        Color::Role(ColorRole::Border)
    };
    let (bg, fg, border_color) = match variant {
        ButtonVariant::Primary => (
            Color::Role(ColorRole::Primary),
            Color::Role(ColorRole::PrimaryForeground),
            Color::rgba(0, 0, 0, 0),
        ),
        ButtonVariant::Secondary => (
            Color::Role(ColorRole::Secondary),
            Color::Role(ColorRole::SecondaryForeground),
            Color::rgba(0, 0, 0, 0),
        ),
        ButtonVariant::Outline => (
            if dark {
                Color::Role(ColorRole::Input30)
            } else {
                Color::Role(ColorRole::Background)
            },
            Color::Role(ColorRole::Foreground),
            outline_border,
        ),
        ButtonVariant::Ghost => (
            Color::rgba(0, 0, 0, 0),
            Color::Role(ColorRole::Foreground),
            Color::rgba(0, 0, 0, 0),
        ),
        ButtonVariant::Destructive => (
            Color::Role(ColorRole::DestructiveTint),
            Color::Role(ColorRole::Destructive),
            Color::rgba(0, 0, 0, 0),
        ),
        ButtonVariant::Link => (
            Color::rgba(0, 0, 0, 0),
            Color::Role(ColorRole::Primary),
            Color::rgba(0, 0, 0, 0),
        ),
    };
    let base = VisualStyle {
        box_style: BoxStyle {
            background: bg,
            // `border border-transparent` — a real 1dp transparent border
            // everywhere; outline carries the visible one
            border: Border::all(BorderSide::new(Dp(1.0), border_color)),
            radii: CornerRadii::all(Dp(radius)),
            padding: Insets {
                top: Dp(0.0),
                right: Dp(pad_x),
                bottom: Dp(0.0),
                left: Dp(pad_x),
            },
            shadow: None,
        },
        text_style: TextStyle {
            foreground: fg,
            size: TextSize::Exact(Dp(font_px)),
            weight: TextWeight::Medium,
            font: crate::theme::FontFamily::Sans,
            line_height: None,
            underline: false,
            letter_spacing: 0.0,
        },
        focus_ring: Color::Role(ColorRole::Focus),
    };
    // hover — full styles; the recipe state branch replaces the base
    let mut hover = base;
    match variant {
        ButtonVariant::Primary => hover.box_style.background = Color::Role(ColorRole::Primary80),
        ButtonVariant::Secondary => {
            hover.box_style.background = Color::Role(ColorRole::SecondaryHover)
        }
        ButtonVariant::Outline => {
            hover.box_style.background = if dark {
                Color::Role(ColorRole::Input50)
            } else {
                Color::Role(ColorRole::Muted)
            };
        }
        ButtonVariant::Ghost => {
            hover.box_style.background = if dark {
                Color::Role(ColorRole::Muted50)
            } else {
                Color::Role(ColorRole::Muted)
            };
        }
        ButtonVariant::Destructive => {
            hover.box_style.background = Color::Role(ColorRole::DestructiveTintHover)
        }
        ButtonVariant::Link => hover.text_style.underline = true,
    }
    // focus-visible border swap — `border-ring`, or
    // `border-destructive/40` on the destructive variant
    let mut focus_visible = base;
    let fv_border_color = match variant {
        ButtonVariant::Destructive => Color::Role(ColorRole::Destructive40),
        _ => Color::Role(ColorRole::Ring),
    };
    focus_visible.box_style.border = Border::all(BorderSide::new(Dp(1.0), fv_border_color));
    let _ = dark;
    StateStyles {
        base,
        hover: Some(hover),
        // pressed/disabled are whole-control effects (translate / opacity)
        // — no color branch; the painter owns the visual delta
        pressed: None,
        disabled: None,
        focus_visible: Some(focus_visible),
    }
}

/// Delta two full styles into a minimal patch.
fn state_delta(from: &VisualStyle, to: &VisualStyle) -> VisualStylePatch {
    let mut p = VisualStylePatch::new();
    if from.box_style.background != to.box_style.background {
        p.box_style.background = Some(to.box_style.background);
    }
    macro_rules! side {
        ($b:ident) => {{
            if from.box_style.border.$b != to.box_style.border.$b {
                p.box_style.border.$b.width = Some(to.box_style.border.$b.width);
                p.box_style.border.$b.color = Some(to.box_style.border.$b.color);
            }
        }};
    }
    side!(top);
    side!(right);
    side!(bottom);
    side!(left);
    macro_rules! corner {
        ($c:ident) => {{
            if from.box_style.radii.$c != to.box_style.radii.$c {
                p.box_style.radii.$c = Some(to.box_style.radii.$c);
            }
        }};
    }
    corner!(top_left);
    corner!(top_right);
    corner!(bottom_right);
    corner!(bottom_left);
    macro_rules! inset {
        ($i:ident) => {{
            if from.box_style.padding.$i != to.box_style.padding.$i {
                p.box_style.padding.$i = Some(to.box_style.padding.$i);
            }
        }};
    }
    inset!(top);
    inset!(right);
    inset!(bottom);
    inset!(left);
    if from.box_style.shadow != to.box_style.shadow {
        p.box_style.shadow = match &to.box_style.shadow {
            Some(s) => ShadowPatch::Set(*s),
            None => ShadowPatch::Remove,
        };
    }
    if from.text_style.foreground != to.text_style.foreground {
        p.text_style.foreground = Some(to.text_style.foreground);
    }
    if from.text_style.size != to.text_style.size
        && let TextSize::Exact(d) = to.text_style.size
    {
        p.text_style.size = Some(d);
    }
    if from.text_style.weight != to.text_style.weight {
        p.text_style.weight = Some(to.text_style.weight);
    }
    if from.text_style.font != to.text_style.font {
        p.text_style.font = Some(to.text_style.font);
    }
    if from.text_style.line_height != to.text_style.line_height
        && let Some(lh) = to.text_style.line_height
    {
        p.text_style.line_height = Some(lh);
    }
    if from.text_style.underline != to.text_style.underline {
        p.text_style.underline = Some(to.text_style.underline);
    }
    if from.text_style.letter_spacing != to.text_style.letter_spacing {
        p.text_style.letter_spacing = Some(to.text_style.letter_spacing);
    }
    p
}

/// Resolved button style for one interaction state — the deterministic
/// chain: recipe base -> recipe state overlay -> consumer base -> consumer
/// state overlay -> consumer focus overlay -> invalid semantic overlay.
/// OS enforcement comes after. `invalid` is the semantic `aria-invalid`
/// state (`border-destructive`, `ring-destructive/20` dark /40) — applied
/// AFTER all consumer layers so it can't be restyled away; it is
/// orthogonal to every interaction state.
pub fn resolve_button(
    variant: ButtonVariant,
    size: ControlSize,
    patch: &ButtonStylePatch,
    state: StyleState,
    focus_visible: bool,
    dark: bool,
    invalid: bool,
) -> VisualStyle {
    let recipe = button_recipe(variant, size, dark);
    let mut v = recipe.base;
    let recipe_branch = match state {
        StyleState::Disabled => recipe.disabled.as_ref(),
        StyleState::Pressed => recipe.pressed.as_ref(),
        StyleState::Hover => recipe.hover.as_ref(),
        StyleState::Normal => None,
    };
    if let Some(b) = recipe_branch {
        v = *b;
    }
    // recipe focus overlay — a recipe layer BEFORE consumer patches: a
    // consumer border patch may restyle it, but the painter draws the
    // required Focus ring independently (enforcement lives at render, so
    // no patch can erase focus visibility)
    if focus_visible
        && state != StyleState::Disabled
        && let Some(fv) = &recipe.focus_visible
    {
        v.box_style.border = fv.box_style.border;
    }
    v.patch(&patch.styles.base);
    let state_patch = match state {
        StyleState::Disabled => patch.styles.disabled.as_ref(),
        StyleState::Pressed => patch.styles.pressed.as_ref(),
        StyleState::Hover => patch.styles.hover.as_ref(),
        StyleState::Normal => None,
    };
    if let Some(p) = state_patch {
        v.patch(p);
    }
    if focus_visible
        && state != StyleState::Disabled
        && let Some(p) = &patch.styles.focus_visible
    {
        v.patch(p);
    }
    // invalid semantic overlay — destructive border always (light full /
    // dark /50); the enforcement ring switches to the destructive tint so
    // an invalid+focused control keeps the `ring-destructive/*` band.
    if invalid {
        let bc = if dark {
            Color::Role(ColorRole::Destructive50)
        } else {
            Color::Role(ColorRole::Destructive)
        };
        for s in [
            &mut v.box_style.border.top,
            &mut v.box_style.border.right,
            &mut v.box_style.border.bottom,
            &mut v.box_style.border.left,
        ] {
            s.color = bc;
        }
    }
    // required focus indicator color: destructive ring for invalid or the
    // destructive variant (`ring-destructive/*`), ring/50 otherwise
    v.focus_ring = if invalid || variant == ButtonVariant::Destructive {
        Color::Role(ColorRole::DestructiveRing)
    } else {
        Color::Role(ColorRole::Focus)
    };
    v
}

/// Native editor chrome recipe — the pill frame around the peer:
/// Background-role surface, Border-role 1dp outline, 6dp radii, and the
/// safety padding the peer surface insets by. Authored chrome patches
/// overlay this; the peer island remains opaque and unclipped.
pub fn text_input_chrome_recipe() -> BoxStyle {
    BoxStyle {
        background: Color::Role(ColorRole::Background),
        border: Border::all(BorderSide::new(Dp(1.0), Color::Role(ColorRole::Border))),
        radii: CornerRadii::all(Dp(6.0)),
        padding: Insets {
            top: Dp(5.0),
            right: Dp(6.0),
            bottom: Dp(5.0),
            left: Dp(6.0),
        },
        shadow: None,
    }
}

/// Resolve native editor chrome — recipe + authored patch. The paired
/// safety inset the peer sits inside derives from the RESOLVED authored
/// values (pre-normalization radii, per the approved rule).
pub fn resolve_text_input_chrome(patch: &TextInputStylePatch) -> BoxStyle {
    let mut b = text_input_chrome_recipe();
    b.patch(&patch.chrome);
    b
}

/// Surface recipe — muted panel chrome with border + radii + padding.
pub fn surface_recipe() -> BoxStyle {
    BoxStyle {
        background: Color::Role(ColorRole::Muted),
        border: Border::all(BorderSide::new(Dp(1.0), Color::Role(ColorRole::Border))),
        radii: CornerRadii::all(Dp(8.0)),
        padding: Insets::all(Dp(12.0)),
        shadow: None,
    }
}

/// Card container recipe — the frozen `cn-card` chrome: `bg-card`,
/// `rounded-xl` (14dp), `ring-1 ring-foreground/10` expressed as a 1dp
/// spread shadow (CSS `0 0 0 1px` — the edge ring, not a border), and
/// top padding = `--card-spacing` (16 default / 12 sm). Side/bottom
/// padding is 0 — the card's parts own their insets.
pub fn card_recipe(size: crate::theme::CardSize) -> BoxStyle {
    let spacing = card_spacing(size);
    BoxStyle {
        background: Color::Role(ColorRole::Card),
        border: Border::default(),
        radii: CornerRadii::all(Dp(14.0)),
        padding: Insets {
            top: spacing,
            right: Dp(0.0),
            bottom: Dp(0.0),
            left: Dp(0.0),
        },
        shadow: Some(Shadow {
            color: Color::Role(ColorRole::Foreground10),
            offset_x: Dp(0.0),
            offset_y: Dp(0.0),
            blur_sigma: Dp(0.0),
            spread: Dp(1.0),
        }),
    }
}

/// Frozen `--card-spacing` token for one card size.
pub fn card_spacing(size: crate::theme::CardSize) -> Dp {
    match size {
        crate::theme::CardSize::Default => Dp(16.0),
        crate::theme::CardSize::Sm => Dp(12.0),
    }
}

// ------------------------- card part recipes ------------------------------
// The parts composed inside a `ui.card` body (`#header`, `#title`,
// `#description`, `#content`, `#footer`) carry the frozen contract chrome
// and typography — these recipes centralize it so part geometry never
// inherits generic container defaults. `size` selects Default / Sm.

/// `#header` / `#content` horizontal inset — the `--card-spacing` token
/// (16 Default / 12 Sm) applied to the left/right edges only.
pub fn card_part_insets(size: crate::theme::CardSize) -> Insets {
    let p = card_spacing(size);
    Insets {
        right: p,
        left: p,
        ..Insets::default()
    }
}

/// `#header` / `#content` box chrome — carries only the horizontal inset
/// so the part's `padding` doesn't inherit a container default.
pub fn card_part_box(size: crate::theme::CardSize) -> BoxStyle {
    BoxStyle {
        padding: card_part_insets(size),
        ..BoxStyle::default()
    }
}

/// `#header.title` — Default `16px / lh22`, Sm `14px / lh19.25`, medium.
pub fn card_title_style(size: crate::theme::CardSize) -> TextStyle {
    let (fs, lh) = match size {
        crate::theme::CardSize::Default => (16.0, 22.0),
        crate::theme::CardSize::Sm => (14.0, 19.25),
    };
    TextStyle {
        foreground: Color::Role(ColorRole::CardForeground),
        size: TextSize::Exact(Dp(fs)),
        weight: TextWeight::Medium,
        line_height: Some(Dp(lh)),
        ..TextStyle::default()
    }
}

/// `#content` body text — `14px / lh20` in `CardForeground`.
pub fn card_content_style() -> TextStyle {
    TextStyle {
        foreground: Color::Role(ColorRole::CardForeground),
        size: TextSize::Exact(Dp(14.0)),
        line_height: Some(Dp(20.0)),
        ..TextStyle::default()
    }
}

/// `#footer` chrome — `bg-muted/50`, `1dp` `border-t`, `--card-spacing`
/// padding on all sides. Buttons inside are `Sm`, right-aligned.
pub fn card_footer_style(size: crate::theme::CardSize) -> BoxStyle {
    let p = card_spacing(size);
    BoxStyle {
        background: Color::Role(ColorRole::Muted50),
        border: Border {
            top: BorderSide::new(Dp(1.0), Color::Role(ColorRole::Border)),
            ..Border::default()
        },
        radii: CornerRadii::default(),
        padding: Insets {
            top: p,
            right: p,
            bottom: p,
            left: p,
        },
        shadow: None,
    }
}

/// Label recipe — foreground role + body text.
pub fn label_recipe() -> TextStyle {
    TextStyle::default()
}

// ---------------------------------------------------------------------------
// style preflight — finite numbers, nonnegative geometry, positive text size,
/// checked before ANY native mutation at commit time
// ---------------------------------------------------------------------------

fn dp_ok(d: &Dp) -> bool {
    d.0.is_finite() && d.0 >= 0.0
}
fn insets_ok(i: &Insets) -> bool {
    dp_ok(&i.top) && dp_ok(&i.right) && dp_ok(&i.bottom) && dp_ok(&i.left)
}
fn radii_ok(r: &CornerRadii) -> bool {
    dp_ok(&r.top_left) && dp_ok(&r.top_right) && dp_ok(&r.bottom_right) && dp_ok(&r.bottom_left)
}
fn border_ok(b: &Border) -> bool {
    [&b.top, &b.right, &b.bottom, &b.left]
        .iter()
        .all(|s| dp_ok(&s.width))
}
fn shadow_ok(s: &Shadow) -> bool {
    s.offset_x.0.is_finite()
        && s.offset_y.0.is_finite()
        && s.blur_sigma.0.is_finite()
        && s.blur_sigma.0 >= 0.0
}
fn text_ok(t: &TextStyle) -> bool {
    let size_ok = match t.size {
        TextSize::Body => true,
        TextSize::Exact(d) => d.0.is_finite() && d.0 > 0.0,
    };
    let lh_ok = t.line_height.is_none_or(|d| d.0.is_finite() && d.0 > 0.0);
    size_ok && lh_ok && t.letter_spacing.is_finite()
}

/// resolved `BoxStyle` — checked before paint/layout use
pub(crate) fn box_style_ok(s: &BoxStyle) -> bool {
    insets_ok(&s.padding)
        && radii_ok(&s.radii)
        && border_ok(&s.border)
        && s.shadow.as_ref().is_none_or(|sh| shadow_ok(sh))
}
/// resolved `VisualStyle`
pub(crate) fn visual_style_ok(s: &VisualStyle) -> bool {
    box_style_ok(&s.box_style) && text_ok(&s.text_style)
}
/// sparse box patch — every authored field checked
pub(crate) fn box_patch_ok(p: &BoxStylePatch) -> bool {
    let dp_opt = |d: &Option<Dp>| d.is_none_or(|d| dp_ok(&d));
    let sides_ok = [
        &p.border.top,
        &p.border.right,
        &p.border.bottom,
        &p.border.left,
    ]
    .iter()
    .all(|s| dp_opt(&s.width));
    let radii_ok = [
        &p.radii.top_left,
        &p.radii.top_right,
        &p.radii.bottom_right,
        &p.radii.bottom_left,
    ]
    .iter()
    .all(|r| dp_opt(r));
    let pad_ok = [
        &p.padding.top,
        &p.padding.right,
        &p.padding.bottom,
        &p.padding.left,
    ]
    .iter()
    .all(|i| dp_opt(i));
    let sh_ok = match &p.shadow {
        ShadowPatch::Set(s) => shadow_ok(s),
        _ => true,
    };
    sides_ok && radii_ok && pad_ok && sh_ok
}
/// sparse text patch — Exact sizes must be finite+positive
pub(crate) fn text_patch_ok(p: &TextStylePatch) -> bool {
    p.size.is_none_or(|d| d.0.is_finite() && d.0 > 0.0)
}
/// sparse visual patch
pub(crate) fn visual_patch_ok(p: &VisualStylePatch) -> bool {
    box_patch_ok(&p.box_style) && text_patch_ok(&p.text_style)
}
/// full `ButtonStylePatch` — base plus every state branch
pub(crate) fn button_patch_ok(p: &ButtonStylePatch) -> bool {
    visual_patch_ok(&p.styles.base)
        && [
            &p.styles.hover,
            &p.styles.pressed,
            &p.styles.disabled,
            &p.styles.focus_visible,
        ]
        .iter()
        .all(|s| s.is_none_or(|s| visual_patch_ok(&s)))
}
/// `TextInputStylePatch` — chrome fields validated; foreground opaque
/// checks happen against the resolved color (role → palette) at commit.
pub(crate) fn text_input_patch_ok(p: &TextInputStylePatch) -> bool {
    box_patch_ok(&p.chrome)
}

/// v0.1 opacity rule: peer backing (chrome background) and the editable
/// foreground must resolve fully opaque — translucent values would let
/// live editing pixels blend through and are `UiError::Unsupported`.
pub(crate) fn text_input_opaque(b: &BoxStyle, fg: Color, dark: bool) -> bool {
    resolve_color(b.background, dark)[3] >= 1.0 && resolve_color(fg, dark)[3] >= 1.0
}

/// `ActionStyle` — base + optional state patches
pub(crate) fn action_style_ok(s: &ActionStyle) -> bool {
    box_style_ok(&s.base)
        && [&s.hover, &s.pressed, &s.disabled, &s.focus_visible]
            .iter()
            .all(|p| p.is_none_or(|p| box_patch_ok(&p)))
}

// ---------------------------------------------------------------------------
// forced-colors — the semantic slot mapping is shared; the platform owns RGB
// ---------------------------------------------------------------------------

/// Semantic OS-control color slots. Under forced colors a role resolves to
/// a SYSTEM color slot; the platform adapter turns the slot into the real
/// OS RGB (GetSysColor on Windows). The mapping itself is shared policy —
/// a backend never invents its own role->system routing.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum SystemColor {
    /// COLOR_WINDOW — page/canvas background
    Window,
    /// COLOR_WINDOWTEXT — primary text on Window
    WindowText,
    /// COLOR_HIGHLIGHT — selection/accent fill
    Highlight,
    /// COLOR_HIGHLIGHTTEXT — text on Highlight
    HighlightText,
    /// COLOR_GRAYTEXT — disabled/muted text
    GrayText,
    /// COLOR_INACTIVEBORDER — secondary strokes/dividers
    InactiveBorder,
    /// COLOR_HOTLIGHT — interactive affordance
    HotTrack,
}

/// The shared role -> system-slot mapping used under forced colors.
/// Covers the whole `ColorRole` vocabulary — a role can never escape
/// enforcement.
pub fn system_slot(role: crate::theme::ColorRole) -> SystemColor {
    use crate::theme::ColorRole as R;
    match role {
        R::Background
        | R::Muted
        | R::Card
        | R::Surface
        | R::Code
        | R::Secondary
        | R::Muted50
        | R::DestructiveTint
        | R::DestructiveTintHover
        | R::Input30
        | R::Input50
        | R::SecondaryHover => SystemColor::Window,
        R::Foreground
        | R::Destructive
        | R::CardForeground
        | R::SecondaryForeground
        | R::SurfaceForeground
        | R::CodeForeground
        | R::CodeHighlight
        | R::Primary
        | R::DestructiveForeground => SystemColor::WindowText,
        R::MutedForeground | R::CodeNumber | R::Shadow | R::Foreground60 => SystemColor::GrayText,
        R::Accent
        | R::Focus
        | R::Ring
        | R::Primary80
        | R::Selection
        | R::DestructiveRing
        | R::Destructive40
        | R::Destructive50 => SystemColor::Highlight,
        R::AccentForeground | R::PrimaryForeground | R::SelectionForeground => {
            SystemColor::HighlightText
        }
        R::Border | R::Input | R::Foreground10 => SystemColor::InactiveBorder,
    }
}

/// OS/accessibility enforcement — applied AFTER all recipe+consumer
/// layers. Forced-colors/high-contrast: decorative shadow resolves to
/// `None` (the approved contract); required focus visibility lives in the
/// focus enforcement layer and survives. Colors keep their role
/// resolution — under forced colors the OS-level palette follows.
fn enforce_color(c: &mut Color, sys: &dyn Fn(SystemColor) -> [f32; 4]) {
    if let Color::Role(r) = c {
        let [r8, g8, b8, a8] = sys(system_slot(*r));
        *c = Color::rgba(
            (r8 * 255.0).round() as u8,
            (g8 * 255.0).round() as u8,
            (b8 * 255.0).round() as u8,
            (a8 * 255.0).round() as u8,
        );
    }
}

fn enforce_box_colors(b: &mut BoxStyle, sys: &dyn Fn(SystemColor) -> [f32; 4]) {
    enforce_color(&mut b.background, sys);
    for side in [
        &mut b.border.top,
        &mut b.border.right,
        &mut b.border.bottom,
        &mut b.border.left,
    ] {
        enforce_color(&mut side.color, sys);
    }
    if let Some(s) = &mut b.shadow {
        enforce_color(&mut s.color, sys);
    }
}

/// `forced_resolver` = the platform's system-color lookup — `None` keeps
/// the authored palette (forced-colors off). `Some` rewrites every role to
/// its system slot's real RGB and removes decorative shadow.
pub(crate) fn os_enforce_box(
    b: &mut BoxStyle,
    forced_resolver: Option<&dyn Fn(SystemColor) -> [f32; 4]>,
) {
    if let Some(sys) = forced_resolver {
        b.shadow = None;
        enforce_box_colors(b, sys);
    }
}
pub(crate) fn os_enforce_visual(
    v: &mut VisualStyle,
    forced_resolver: Option<&dyn Fn(SystemColor) -> [f32; 4]>,
) {
    os_enforce_box(&mut v.box_style, forced_resolver);
    if let Some(sys) = forced_resolver {
        enforce_color(&mut v.text_style.foreground, sys);
        v.text_style.size = TextSize::Body; // OS text size wins
    }
}

/// Does a box patch touch any field that feeds layout insets (padding) or
/// content-consuming border widths? Shadow/background/color never do.
pub(crate) fn patch_touches_layout(p: &BoxStylePatch) -> bool {
    p.padding.top.is_some()
        || p.padding.right.is_some()
        || p.padding.bottom.is_some()
        || p.padding.left.is_some()
        || p.border.top.width.is_some()
        || p.border.right.width.is_some()
        || p.border.bottom.width.is_some()
        || p.border.left.width.is_some()
}
