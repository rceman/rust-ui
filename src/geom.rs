use std::fmt;

/// Logical device-independent pixel (dp). Physical-pixel snapping happens at
/// the backend boundary only.
#[derive(Copy, Clone, Debug, Default, PartialEq, PartialOrd)]
pub struct Dp(pub f32);

/// `dp(12.0)` — logical-pixel constructor used by examples.
pub fn dp(value: f32) -> Dp {
    Dp(value)
}

impl Dp {
    pub const ZERO: Dp = Dp(0.0);
    pub(crate) fn valid(&self) -> bool {
        self.0.is_finite() && self.0 >= 0.0
    }
}

impl fmt::Display for Dp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}dp", self.0)
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub fn new(width: Dp, height: Dp) -> Self {
        Size {
            width: width.0,
            height: height.0,
        }
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    /// Is `local` (node-local coordinates) inside this rect?
    pub fn contains_local(&self, local: Point) -> bool {
        local.x >= 0.0 && local.y >= 0.0 && local.x <= self.width && local.y <= self.height
    }

    /// Shrink each edge by `d` (clamped at zero).
    pub fn inset(&self, d: f32) -> Rect {
        Rect {
            x: self.x + d,
            y: self.y + d,
            width: (self.width - 2.0 * d).max(0.0),
            height: (self.height - 2.0 * d).max(0.0),
        }
    }
}

/// Box offered to a child during measure; `constrain` clamps a desired size
/// into the available box.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Constraints {
    pub min: Size,
    pub max: Size,
}

impl Constraints {
    pub fn constrain(&self, desired: Size) -> Size {
        Size {
            width: desired.width.clamp(self.min.width, self.max.width),
            height: desired.height.clamp(self.min.height, self.max.height),
        }
    }
}

/// No `Eq`/`Hash` on `Length`/`Dp`: f32 NaN would break the equality law.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub enum Length {
    #[default]
    Content,
    Fixed(Dp),
    Fill(u16),
}

/// Explicit sizing hints staged on every leaf/container — validated at stage
/// time (finite, non-negative, `min <= max`), stored on the retained node.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct LayoutSpec {
    pub width: Length,
    pub height: Length,
    pub min_width: Option<Dp>,
    pub min_height: Option<Dp>,
    pub max_width: Option<Dp>,
    pub max_height: Option<Dp>,
}

impl LayoutSpec {
    /// None = valid; typed diagnostic for invalid sizing.
    pub(crate) fn validate(&self) -> Option<crate::UiDiagnostic> {
        let len_ok = |l: &Length| match l {
            Length::Content => true,
            Length::Fixed(d) => d.valid(),
            Length::Fill(f) => *f > 0,
        };
        let d_ok = |d: &Option<Dp>| d.map_or(true, |d| d.valid());
        if !len_ok(&self.width)
            || !len_ok(&self.height)
            || !d_ok(&self.min_width)
            || !d_ok(&self.min_height)
            || !d_ok(&self.max_width)
            || !d_ok(&self.max_height)
        {
            return Some(crate::UiDiagnostic::InvalidLayout);
        }
        let pair_ok = |min: &Option<Dp>, max: &Option<Dp>| {
            matches!((min, max), (Some(a), Some(b)) if a.0 <= b.0)
                || !(min.is_some() && max.is_some())
        };
        if !pair_ok(&self.min_width, &self.max_width)
            || !pair_ok(&self.min_height, &self.max_height)
        {
            return Some(crate::UiDiagnostic::InvalidLayout);
        }
        None
    }
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
    Stretch,
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum Justify {
    #[default]
    Start,
    Center,
    End,
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum Visibility {
    #[default]
    Visible,
    Hidden,
}
