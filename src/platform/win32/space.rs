//! Win32 geometry adapter — the platform realization of the SHARED
//! coordinate contract (`crate::geom` + `docs/PLATFORM_CONTRACTS.md`).
//!
//! This module owns ONLY the Win32 seams: `RECT`/`RECTL`/`POINT`/`SIZE`
//! conversions, `LPARAM` packing, and `ClientToScreen`/`ScreenToClient`.
//! The semantic types (`ScaleFactor`, `Logical*`/`Physical*` spaces,
//! rounding policy) live in `crate::geom` — a future backend implements
//! the same contract, never a second copy of it.
//!
//! Contract (verified — `docs/NATIVE_CONTROL_BOUNDARY.md`):
//!
//! ```text
//!   rust-ui retained/layout semantics ..... logical Dp
//!   msftedit host space, window client, caret
//!   lparams, UIA rects, real input ........ physical px
//!   conversion ............................ geom::* <-> win32 types, here
//! ```
//!
//! Exceptions (APIs that want neither Dp nor px — e.g. HIMETRIC for
//! `TxGetExtent`) are documented at the adapter's call site, never
//! smuggled through ad-hoc factors.

use windows::Win32::Foundation::{POINT, RECT, RECTL, SIZE};
use windows::Win32::Graphics::Gdi::{ClientToScreen, ScreenToClient};

// re-export the shared spaces so win32 modules name one import root —
// the types live in `crate::geom`; this module adds only the Win32 seams
pub(crate) use crate::geom::{
    ClientPhysicalPoint, PhysicalPoint, PhysicalRect, PhysicalSize, Point as LogicalPoint,
    Rect as LogicalRect, ScaleFactor, ScreenPhysicalPoint,
};

impl LogicalRect {
    /// `RECTL` wants physical px — THE logical->RECTL conversion.
    /// (`TxDrawD2D` `lprcBounds`; the service divides by `dcDpi/96`.)
    pub fn rectl(self, s: ScaleFactor) -> RECTL {
        let p = self.physical(s);
        RECTL {
            left: p.left,
            top: p.top,
            right: p.right,
            bottom: p.bottom,
        }
    }
}

impl From<PhysicalRect> for RECT {
    fn from(r: PhysicalRect) -> RECT {
        RECT {
            left: r.left,
            top: r.top,
            right: r.right,
            bottom: r.bottom,
        }
    }
}

impl From<RECT> for PhysicalRect {
    fn from(r: RECT) -> PhysicalRect {
        PhysicalRect {
            left: r.left,
            top: r.top,
            right: r.right,
            bottom: r.bottom,
        }
    }
}

impl From<PhysicalPoint> for POINT {
    fn from(p: PhysicalPoint) -> POINT {
        POINT { x: p.x, y: p.y }
    }
}

impl From<POINT> for PhysicalPoint {
    fn from(p: POINT) -> PhysicalPoint {
        PhysicalPoint { x: p.x, y: p.y }
    }
}

impl From<PhysicalSize> for SIZE {
    fn from(s: PhysicalSize) -> SIZE {
        SIZE { cx: s.w, cy: s.h }
    }
}

/// `ClientToScreen` — physical px both sides under PMv2.
pub(crate) fn client_to_screen(
    hwnd: windows::Win32::Foundation::HWND,
    p: ClientPhysicalPoint,
) -> ScreenPhysicalPoint {
    let mut pt: POINT = p.0.into();
    unsafe {
        let _ = ClientToScreen(hwnd, &mut pt);
    }
    ScreenPhysicalPoint(pt.into())
}

/// `ScreenToClient` — physical px both sides under PMv2.
pub(crate) fn screen_to_client(
    hwnd: windows::Win32::Foundation::HWND,
    p: ScreenPhysicalPoint,
) -> ClientPhysicalPoint {
    let mut pt: POINT = p.0.into();
    unsafe {
        let _ = ScreenToClient(hwnd, &mut pt);
    }
    ClientPhysicalPoint(pt.into())
}

/// Pack a client/peer-local px point into a Win32 `LPARAM`
/// (MAKELPARAM convention — low i16 x, high i16 y).
pub(crate) fn lparam_px(p: PhysicalPoint) -> isize {
    ((p.y as isize) << 16) | (p.x as isize & 0xffff)
}
