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

