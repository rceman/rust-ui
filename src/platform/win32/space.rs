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
    ClientPhysicalPoint, PeerLocalPoint, PeerOrigin, PhysicalPoint, PhysicalRect, PhysicalSize,
    Rect as LogicalRect, ScaleFactor, ScreenPhysicalPoint,
};

// ---------------------------------------------------------------------------
// Windows-specific scale mapping — the 96-DPI baseline is Win32/D2D's
// convention, NOT shared rust-ui semantics (a future backend supplies its
// own ratio and never sees `96`).
// ---------------------------------------------------------------------------

/// Win32 monitor DPI -> shared ratio (`dpi / 96` is Windows' own rule).
pub fn scale_from_dpi(dpi: u32) -> ScaleFactor {
    ScaleFactor(dpi as f32 / 96.0)
}

/// Shared ratio -> Win32/D2D DPI (SetDpi, WNDCLASS reasoning, etc.).
pub fn dpi_of(s: ScaleFactor) -> u32 {
    (s.0 * 96.0).round() as u32
}

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
/// `None` on native failure (invalid hwnd / non-mappable point) — callers
/// must not treat a failed conversion as identity.
pub fn client_to_screen(
    hwnd: windows::Win32::Foundation::HWND,
    p: ClientPhysicalPoint,
) -> Option<ScreenPhysicalPoint> {
    let mut pt: POINT = p.0.into();
    let ok = unsafe { ClientToScreen(hwnd, &mut pt) };
    ok.as_bool().then_some(ScreenPhysicalPoint(pt.into()))
}

/// `ScreenToClient` — physical px both sides under PMv2; `None` on failure.
pub fn screen_to_client(
    hwnd: windows::Win32::Foundation::HWND,
    p: ScreenPhysicalPoint,
) -> Option<ClientPhysicalPoint> {
    let mut pt: POINT = p.0.into();
    let ok = unsafe { ScreenToClient(hwnd, &mut pt) };
    ok.as_bool().then_some(ClientPhysicalPoint(pt.into()))
}

/// Pack a client/peer-local px point into a Win32 `LPARAM`
/// (MAKELPARAM convention — signed i16 x in the low word, signed i16 y in
/// the high word). `None` when a coordinate cannot be represented —
/// callers must not silently truncate ±32768-range coordinates.
pub fn try_lparam_px(p: PhysicalPoint) -> Option<isize> {
    let ok = |v: i32| (i16::MIN as i32..=i16::MAX as i32).contains(&v);
    if ok(p.x) && ok(p.y) {
        Some(((p.y as u16 as usize) << 16 | (p.x as u16 as usize)) as isize)
    } else {
        None
    }
}
