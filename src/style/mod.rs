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

include!("patches.rs");
include!("action.rs");
include!("palette_tokens.rs");
include!("recipes_button.rs");
include!("recipes_surface_display.rs");
include!("recipes_forms_nav_overlays.rs");
include!("validate.rs");
