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

