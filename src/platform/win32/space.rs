//! Canonical Windows coordinate layer — the ONE authority for unit
//! conversion between rust-ui's DIP space and the physical-pixel space the
//! Win32/RichEdit/UIA boundaries actually speak.
//!
//! Contract (verified — see `docs/NATIVE_CONTROL_BOUNDARY.md`):
//!
//! ```text
//!   retained/layout/UI semantics ............ logical DIP
//!   msftedit host space, window client, caret
//!   lparams, UIA rects, real input .......... physical px
//!   conversion .............................. explicit, here only
//! ```
//!
//! Rules:
//! - no `* scale` / `/ scale` / `dpi / 96` arithmetic outside this module —
//!   adapters name the space, the layer owns the math;
//! - px quantities at COM seams are integral (`i32`); the ONE rounding
//!   policy is round-half-away-from-zero applied at DIP→px conversion;
//! - exceptions (a Windows API that wants neither DIP nor px — e.g.
//!   HIMETRIC for `TxGetExtent`) are documented at the call site, not
//!   smuggled through ad-hoc factors.

use std::fmt;

use windows::Win32::Foundation::{POINT, RECT, RECTL, SIZE};

/// px-per-DIP ratio — `dpi / 96`. The ONLY `dpi/96` in the codebase.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Scale(pub f32);

impl Scale {
    pub const ONE: Scale = Scale(1.0);

    /// `Scale::from_dpi(120)` = the 125% monitor.
    pub fn from_dpi(dpi: u32) -> Scale {
        Scale(dpi as f32 / 96.0)
    }

    pub fn dpi(self) -> u32 {
        (self.0 * 96.0).round() as u32
    }

    /// DIP -> px with the single rounding policy (round-half-away).
    pub fn dip_to_px(self, dip: f32) -> i32 {
        (dip * self.0).round() as i32
    }

    /// DIP -> px without rounding (fractional px — antialiased chrome).
    pub fn dip_to_px_f(self, dip: f32) -> f32 {
        dip * self.0
    }

    /// px -> DIP (exact — the conversion back from an already-rounded
    /// physical pixel can legitimately be fractional).
    pub fn px_to_dip(self, px: i32) -> f32 {
        px as f32 / self.0
    }

    /// fractional px -> DIP (unrounded quantities — raster math).
    pub fn px_to_dip_f(self, px: f32) -> f32 {
        px / self.0
    }
}

/// A DIP-space point.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct DipPoint {
    pub x: f32,
    pub y: f32,
}

/// A DIP-space rect (origin + extent).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct DipRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// A physical-pixel point (client-local or screen — see marker types).
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct PxPoint {
    pub x: i32,
    pub y: i32,
}

/// A physical-pixel size.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct PxSize {
    pub w: i32,
    pub h: i32,
}

/// A physical-pixel rect (left/top/right/bottom — the RECT lattice).
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct PxRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// Marker: a px point in *window-client* space (origin at the window's
/// client 0,0). Distinct from `ScreenPxPoint` so a screen coordinate can
/// never be silently subtracted against a client one.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct ClientPxPoint(pub PxPoint);

/// Marker: a px point in *screen* space.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct ScreenPxPoint(pub PxPoint);

/// `ClientToScreen` — physical px both sides under PMv2.
pub fn client_to_screen(
    hwnd: windows::Win32::Foundation::HWND,
    p: ClientPxPoint,
) -> ScreenPxPoint {
    let mut pt: POINT = p.0.into();
    unsafe {
        let _ = windows::Win32::Graphics::Gdi::ClientToScreen(hwnd, &mut pt);
    }
    ScreenPxPoint(pt.into())
}

/// `ScreenToClient` — physical px both sides under PMv2.
pub fn screen_to_client(
    hwnd: windows::Win32::Foundation::HWND,
    p: ScreenPxPoint,
) -> ClientPxPoint {
    let mut pt: POINT = p.0.into();
    unsafe {
        let _ = windows::Win32::Graphics::Gdi::ScreenToClient(hwnd, &mut pt);
    }
    ClientPxPoint(pt.into())
}

impl DipPoint {
    pub fn px(self, s: Scale) -> PxPoint {
        PxPoint {
            x: s.dip_to_px(self.x),
            y: s.dip_to_px(self.y),
        }
    }
}

impl DipRect {
    pub fn px(self, s: Scale) -> PxRect {
        PxRect {
            left: s.dip_to_px(self.x),
            top: s.dip_to_px(self.y),
            right: s.dip_to_px(self.x + self.w),
            bottom: s.dip_to_px(self.y + self.h),
        }
    }

    /// A local rect `(0,0,w,h)` — the space peers hand `TxGetClientRect`.
    pub fn local(w: f32, h: f32) -> DipRect {
        DipRect { x: 0.0, y: 0.0, w, h }
    }

    pub fn right(self) -> f32 {
        self.x + self.w
    }
    pub fn bottom(self) -> f32 {
        self.y + self.h
    }

    /// is `p` (a DIP point in this rect's space) inside it?
    pub fn contains(&self, p: crate::geom::Point) -> bool {
        p.x >= self.x && p.x < self.x + self.w && p.y >= self.y && p.y < self.y + self.h
    }

    /// smallest rect covering both — the ink-damage union primitive
    pub fn union(&self, o: DipRect) -> DipRect {
        let (x0, y0) = (self.x.min(o.x), self.y.min(o.y));
        DipRect {
            x: x0,
            y: y0,
            w: self.right().max(o.right()) - x0,
            h: self.bottom().max(o.bottom()) - y0,
        }
    }
}

impl PxPoint {
    pub fn dip(self, s: Scale) -> DipPoint {
        DipPoint {
            x: s.px_to_dip(self.x),
            y: s.px_to_dip(self.y),
        }
    }
}

impl PxRect {
    pub fn dip(self, s: Scale) -> DipRect {
        DipRect {
            x: s.px_to_dip(self.left),
            y: s.px_to_dip(self.top),
            w: s.px_to_dip(self.right - self.left),
            h: s.px_to_dip(self.bottom - self.top),
        }
    }

    pub fn size(self) -> PxSize {
        PxSize {
            w: self.right - self.left,
            h: self.bottom - self.top,
        }
    }
}

// ---- Win32 seam -----------------------------------------------------------

// NB: no `From<DipRect> for RECTL` — DIP->px is impossible without a Scale,
// so the conversion is explicit at the call site (`rectl(scale)`).

impl DipRect {
    /// `RECTL` wants physical px — this is THE DIP->RECTL conversion.
    pub fn rectl(self, s: Scale) -> RECTL {
        let p = self.px(s);
        RECTL {
            left: p.left,
            top: p.top,
            right: p.right,
            bottom: p.bottom,
        }
    }
}

impl From<PxRect> for RECT {
    fn from(r: PxRect) -> RECT {
        RECT {
            left: r.left,
            top: r.top,
            right: r.right,
            bottom: r.bottom,
        }
    }
}

impl From<RECT> for PxRect {
    fn from(r: RECT) -> PxRect {
        PxRect {
            left: r.left,
            top: r.top,
            right: r.right,
            bottom: r.bottom,
        }
    }
}

impl From<PxPoint> for POINT {
    fn from(p: PxPoint) -> POINT {
        POINT { x: p.x, y: p.y }
    }
}

impl From<POINT> for PxPoint {
    fn from(p: POINT) -> PxPoint {
        PxPoint { x: p.x, y: p.y }
    }
}

impl From<PxSize> for SIZE {
    fn from(s: PxSize) -> SIZE {
        SIZE { cx: s.w, cy: s.h }
    }
}

/// pack a client-px point into a Win32 `LPARAM` (MAKELPARAM convention).
pub fn lparam_px(p: PxPoint) -> isize {
    ((p.y as isize) << 16) | (p.x as isize & 0xffff)
}

impl fmt::Display for Scale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}dpi", self.dpi())
    }
}
