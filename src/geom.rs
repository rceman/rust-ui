use std::fmt;

/// rust-ui logical layout/design unit. `Dp` is NOT a physical-size promise —
/// how many physical pixels one logical unit occupies is the backend's
/// `ScaleFactor`, which the display environment supplies.
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

    /// Is `p` (a point in this rect's own coordinate space) inside it?
    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.x < self.x + self.width && p.y >= self.y && p.y < self.y + self.height
    }

    /// Smallest rect covering both — the ink-damage union primitive.
    pub fn union(&self, o: Rect) -> Rect {
        let (x0, y0) = (self.x.min(o.x), self.y.min(o.y));
        Rect {
            x: x0,
            y: y0,
            width: (self.x + self.width).max(o.x + o.width) - x0,
            height: (self.y + self.height).max(o.y + o.height) - y0,
        }
    }

    /// A local-space rect `(0,0,w,h)` — peer/client surfaces hand these
    /// to native APIs as their own origin.
    pub fn local(w: f32, h: f32) -> Rect {
        Rect {
            x: 0.0,
            y: 0.0,
            width: w,
            height: h,
        }
    }

    /// DIP -> physical px rect (right/bottom rounded independently — the
    /// *edges* snap, never the width).
    pub fn physical(self, s: ScaleFactor) -> PhysicalRect {
        PhysicalRect {
            left: s.to_physical(self.x),
            top: s.to_physical(self.y),
            right: s.to_physical(self.x + self.width),
            bottom: s.to_physical(self.y + self.height),
        }
    }

    pub fn right(&self) -> f32 {
        self.x + self.width
    }
    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }
}

impl Point {
    /// DIP point -> physical px point.
    pub fn physical(self, s: ScaleFactor) -> PhysicalPoint {
        PhysicalPoint {
            x: s.to_physical(self.x),
            y: s.to_physical(self.y),
        }
    }
}

// ---------------------------------------------------------------------------
// Platform-contract geometry — the SHARED semantic model (docs/PLATFORM_CONTRACTS.md).
//
// rust-ui semantics are DIP/Logical. The physical-px space is what real
// display servers, native text services and accessibility trees speak; each
// backend converts at its own seam. `* scale`/`dpi/96` arithmetic outside
// these methods is a contract violation.
// ---------------------------------------------------------------------------

/// Physical pixels per rust-ui logical unit — platform-neutral meaning.
/// A backend supplies the ratio its environment reports (e.g. an OS DPI
/// mapping on Windows, a backing-store scale elsewhere); the shared
/// semantics know nothing about dpi/96.
///
/// Invariant: positive and finite. Construct via `ScaleFactor::new` for
/// checked creation; the raw field remains readable.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ScaleFactor(pub f32);

impl ScaleFactor {
    pub const ONE: ScaleFactor = ScaleFactor(1.0);

    /// Checked constructor — `None` on non-positive or non-finite input.
    pub fn new(ratio: f32) -> Option<ScaleFactor> {
        (ratio.is_finite() && ratio > 0.0).then_some(ScaleFactor(ratio))
    }

    /// True for a usable ratio (positive + finite).
    pub fn valid(self) -> bool {
        self.0.is_finite() && self.0 > 0.0
    }

    /// logical -> physical px — THE rounding policy:
    /// round-half-away-from-zero at the conversion boundary.
    pub fn to_physical(self, logical: f32) -> i32 {
        (logical * self.0).round() as i32
    }

    /// logical -> fractional physical px (antialiased chrome raster math
    /// that must not snap mid-pipeline).
    pub fn to_physical_f(self, logical: f32) -> f32 {
        logical * self.0
    }

    /// physical px -> logical (exact — a rounded physical pixel may
    /// legitimately map to a fractional logical unit).
    pub fn to_logical(self, px: i32) -> f32 {
        px as f32 / self.0
    }

    /// fractional physical px -> logical.
    pub fn to_logical_f(self, px: f32) -> f32 {
        px / self.0
    }
}

/// A physical-pixel scalar.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct PhysicalPx(pub i32);

/// A physical-pixel point — unmarked. When origin ambiguity matters use
/// the marker wrappers `ClientPhysicalPoint` / `ScreenPhysicalPoint`.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct PhysicalPoint {
    pub x: i32,
    pub y: i32,
}

/// A physical-pixel size.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct PhysicalSize {
    pub w: i32,
    pub h: i32,
}

/// A physical-pixel rect on the left/top/right/bottom lattice — the shape
/// display servers and native controls actually exchange.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct PhysicalRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// Marker: a physical px point in a *surface-client* space (a window's or
/// peer's own 0,0 origin) — never confuse it with screen coordinates.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct ClientPhysicalPoint(pub PhysicalPoint);

/// Marker: a physical px point in *screen* space.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct ScreenPhysicalPoint(pub PhysicalPoint);

/// Marker: a physical px point in a peer's own surface space (origin =
/// the peer's snapped client origin) — never confuse it with the
/// window's client space.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct PeerLocalPoint(pub PhysicalPoint);

/// THE window-client ↔ peer-local transform — one snapped physical origin,
/// integer subtraction/addition only. Constructing the origin snaps the
/// logical rect ONCE (`origin_from`); every consumer then shares exactly
/// that snapped pixel, so pointer/caret/host-callback coordinates cannot
/// drift by the half-px that a per-call logical re-rounding produces.
///
///   local  = client  - snapped_origin
///   client = local   + snapped_origin
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct PeerOrigin(pub PhysicalPoint);

impl PeerOrigin {
    /// Snap `peer_logical_bounds` ONCE at `scale` — the shared origin.
    pub fn from_logical(bounds: Rect, s: ScaleFactor) -> PeerOrigin {
        let p = bounds.physical(s);
        PeerOrigin(PhysicalPoint {
            x: p.left,
            y: p.top,
        })
    }

    /// window/client px -> peer-local px.
    pub fn to_local(self, p: ClientPhysicalPoint) -> PeerLocalPoint {
        PeerLocalPoint(PhysicalPoint {
            x: p.0.x - self.0.x,
            y: p.0.y - self.0.y,
        })
    }

    /// peer-local px -> window/client px.
    pub fn to_client(self, p: PeerLocalPoint) -> ClientPhysicalPoint {
        ClientPhysicalPoint(PhysicalPoint {
            x: p.0.x + self.0.x,
            y: p.0.y + self.0.y,
        })
    }
}

impl PhysicalPoint {
    /// physical px -> DIP point.
    pub fn logical(self, s: ScaleFactor) -> Point {
        Point {
            x: s.to_logical(self.x),
            y: s.to_logical(self.y),
        }
    }
}

impl PhysicalRect {
    /// physical px -> DIP rect.
    pub fn logical(self, s: ScaleFactor) -> Rect {
        Rect {
            x: s.to_logical(self.left),
            y: s.to_logical(self.top),
            width: s.to_logical(self.right - self.left),
            height: s.to_logical(self.bottom - self.top),
        }
    }

    pub fn size(self) -> PhysicalSize {
        PhysicalSize {
            w: self.right - self.left,
            h: self.bottom - self.top,
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
