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
/// sharp offset silhouette. No spread, no inset, no list.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Shadow {
    pub color: Color,
    pub offset_x: Dp,
    pub offset_y: Dp,
    pub blur_sigma: Dp,
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

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum TextWeight {
    Normal,
    Medium,
    Bold,
}

/// `Body` resolves to the theme base size; `Exact(dp)` is a requested
/// unscaled logical size (OS text scaling still applies).
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum TextSize {
    Body,
    Exact(Dp),
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct TextStyle {
    pub foreground: Color,
    pub size: TextSize,
    pub weight: TextWeight,
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
}

impl Default for TextStyle {
    fn default() -> Self {
        TextStyle {
            foreground: Color::Role(ColorRole::Foreground),
            size: TextSize::Body,
            weight: TextWeight::Normal,
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
    }
}

/// A component's resolved visual surface — box + text parts.
#[derive(Copy, Clone, Default, PartialEq, Debug)]
pub struct VisualStyle {
    pub box_style: BoxStyle,
    pub text_style: TextStyle,
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
    pub fn hover(mut self, patch: BoxStylePatch) -> Self {
        self.hover = Some(patch);
        self
    }
    pub fn pressed(mut self, patch: BoxStylePatch) -> Self {
        self.pressed = Some(patch);
        self
    }
    pub fn disabled(mut self, patch: BoxStylePatch) -> Self {
        self.disabled = Some(patch);
        self
    }
    pub fn focus_visible(mut self, patch: BoxStylePatch) -> Self {
        self.focus_visible = Some(patch);
        self
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
        if focus_visible && !disabled && let Some(p) = &self.focus_visible {
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

/// shadcn neutral palette per `ColorRole` — straight sRGB floats.
// ---------------------------------------------------------------------------
// color resolution — the shared palette (renderers convert to their format)
// ---------------------------------------------------------------------------

/// shadcn neutral palette per `ColorRole` — straight sRGB floats.
pub fn role_color(role: ColorRole, dark: bool) -> [f32; 4] {
    use ColorRole::*;
    match (role, dark) {
        (Background, false) => [0.980, 0.980, 0.980, 1.0],
        (Background, true) => [0.043, 0.043, 0.043, 1.0],
        (Foreground, false) => [0.090, 0.090, 0.090, 1.0],
        (Foreground, true) => [0.929, 0.929, 0.929, 1.0],
        (Muted, false) => [0.961, 0.961, 0.961, 1.0],
        (Muted, true) => [0.149, 0.149, 0.149, 1.0],
        (MutedForeground, false) => [0.451, 0.451, 0.451, 1.0],
        (MutedForeground, true) => [0.639, 0.639, 0.639, 1.0],
        (Accent, false) => [0.094, 0.094, 0.106, 1.0],
        (Accent, true) => [0.980, 0.980, 0.980, 1.0],
        (AccentForeground, false) => [0.980, 0.980, 0.980, 1.0],
        (AccentForeground, true) => [0.094, 0.094, 0.106, 1.0],
        (Border, false) => [0.898, 0.898, 0.898, 1.0],
        (Border, true) => [0.180, 0.180, 0.180, 1.0],
        (Destructive, _) => [0.863, 0.149, 0.149, 1.0],
        (DestructiveForeground, _) => [0.980, 0.980, 0.980, 1.0],
        (Focus, false) => [0.35, 0.35, 0.40, 1.0],
        (Focus, true) => [0.65, 0.65, 0.70, 1.0],
        (Shadow, false) => [0.0, 0.0, 0.0, 0.16],
        (Shadow, true) => [0.0, 0.0, 0.0, 0.35],
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

/// Button recipe: base `VisualStyle` per variant/size + recipe state
/// overlays expressed as field diffs. The renderer picks the branch, applies
/// the consumer base, then the consumer state patch.
pub fn button_recipe(
    variant: ButtonVariant,
    size: ControlSize,
    dark: bool,
) -> StateStyles<VisualStyle> {
    let (bg, fg, border) = match variant {
        ButtonVariant::Primary => (
            Color::Role(ColorRole::Accent),
            Color::Role(ColorRole::AccentForeground),
            Border::default(),
        ),
        ButtonVariant::Secondary => (
            Color::Role(ColorRole::Muted),
            Color::Role(ColorRole::Foreground),
            Border::all(BorderSide::new(Dp(1.0), Color::Role(ColorRole::Border))),
        ),
        ButtonVariant::Ghost => (
            Color::Role(ColorRole::Background),
            Color::Role(ColorRole::Foreground),
            Border::default(),
        ),
        ButtonVariant::Destructive => (
            Color::Role(ColorRole::Destructive),
            Color::Role(ColorRole::DestructiveForeground),
            Border::default(),
        ),
    };
    let pad = match size {
        ControlSize::Sm => Insets {
            top: Dp(4.0),
            right: Dp(10.0),
            bottom: Dp(4.0),
            left: Dp(10.0),
        },
        ControlSize::Md => Insets {
            top: Dp(6.0),
            right: Dp(14.0),
            bottom: Dp(6.0),
            left: Dp(14.0),
        },
        ControlSize::Lg => Insets {
            top: Dp(8.0),
            right: Dp(18.0),
            bottom: Dp(8.0),
            left: Dp(18.0),
        },
    };
    let base = VisualStyle {
        box_style: BoxStyle {
            background: bg,
            border,
            radii: CornerRadii::all(Dp(6.0)),
            padding: pad,
            shadow: None,
        },
        text_style: TextStyle {
            foreground: fg,
            size: TextSize::Body,
            weight: TextWeight::Medium,
        },
    };
    // recipe state branches as full styles; state_delta diffs them into the
    // shared patch vocabulary
    let branch = |alpha: f32, fg_alpha: Option<f32>| {
        let mut b = base;
        let [r, g, bl, a] = resolve_color(base.box_style.background, dark);
        b.box_style.background = Color::rgba(
            (r * 255.0) as u8,
            (g * 255.0) as u8,
            (bl * 255.0) as u8,
            (a * alpha * 255.0) as u8,
        );
        if let Some(fa) = fg_alpha {
            let [r, g, bl, a] = resolve_color(base.text_style.foreground, dark);
            b.text_style.foreground = Color::rgba(
                (r * 255.0) as u8,
                (g * 255.0) as u8,
                (bl * 255.0) as u8,
                (a * fa * 255.0) as u8,
            );
        }
        b
    };
    StateStyles {
        base,
        hover: Some(branch(0.88, None)),
        pressed: Some(branch(0.72, None)),
        disabled: Some(branch(0.45, Some(0.45))),
        // focus ring is the recipe's focus overlay — a 1.5dp Focus border
        focus_visible: Some({
            let mut b = base;
            b.box_style.border = Border::all(BorderSide::new(
                Dp(1.5),
                Color::Role(ColorRole::Focus),
            ));
            b
        }),
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
    p
}

/// Resolved button style for one interaction state — the deterministic
/// chain: recipe base -> recipe state overlay -> consumer base -> consumer
/// state overlay -> consumer focus overlay. OS enforcement comes after.
pub fn resolve_button(
    variant: ButtonVariant,
    size: ControlSize,
    patch: &ButtonStylePatch,
    state: StyleState,
    focus_visible: bool,
    dark: bool,
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
    // recipe focus overlay (the ring) — part of the recipe's overlay stage,
    // before any consumer patch
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
    v
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

/// Label recipe — foreground role + body text.
pub fn label_recipe() -> TextStyle {
    TextStyle::default()
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
