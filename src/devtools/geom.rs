use super::*;
use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
impl Rect {
    pub fn valid(self) -> bool {
        [self.x, self.y, self.width, self.height]
            .iter()
            .all(|x| x.is_finite())
            && self.width >= 0.
            && self.height >= 0.
    }
    /// Grow by `pad` Dp on each side; the result still snaps through
    /// `snap_rect`, so comparison padding follows the single authority.
    pub fn grow(self, pad: f64) -> Rect {
        Rect {
            x: self.x - pad,
            y: self.y - pad,
            width: self.width + 2. * pad,
            height: self.height + 2. * pad,
        }
    }
}
/// Integer pixel rect on the left/top/right/bottom lattice (shared geom type).
pub type PixelRect = crate::geom::PhysicalRect;
/// Grow an already-snapped pixel rect by `pad` pixels on each side
/// (saturating at the i32 lattice; a degenerate result fails bounds checks).
pub fn grow_px(r: PixelRect, pad: i64) -> PixelRect {
    let sat = |v: i64| v.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
    PixelRect {
        left: sat(i64::from(r.left) - pad),
        top: sat(i64::from(r.top) - pad),
        right: sat(i64::from(r.right) + pad),
        bottom: sat(i64::from(r.bottom) + pad),
    }
}
/// THE devctl coordinate snap: logical Dp rect -> integer pixel rect at
/// `scale` px/Dp. Every edge delegates to `crate::geom::Rect::physical` /
/// `ScaleFactor` (round half away from zero; width = right - left). The
/// f64 -> f32 cast happens exactly once, here; no other devtools/CLI code may
/// round rects. Non-finite/non-positive scale, non-finite or unrepresentable
/// coordinates/edges fail INVALID_ARGUMENT (`try_to_physical` semantics).
pub fn snap_rect(logical: Rect, scale: f64) -> Result<PixelRect> {
    if !scale.is_finite() || scale <= 0. {
        return Err(Error::new(
            "INVALID_ARGUMENT",
            "scale must be positive finite",
        ));
    }
    let s = crate::geom::ScaleFactor::new(scale as f32)
        .ok_or_else(|| Error::new("INVALID_ARGUMENT", "scale unrepresentable"))?;
    let conv = |v: f64| -> Result<f32> {
        if !v.is_finite() || v.abs() > f64::from(f32::MAX) {
            return Err(Error::new(
                "INVALID_ARGUMENT",
                "rect coordinate unrepresentable",
            ));
        }
        Ok(v as f32)
    };
    let r = crate::geom::Rect {
        x: conv(logical.x)?,
        y: conv(logical.y)?,
        width: conv(logical.width)?,
        height: conv(logical.height)?,
    };
    for v in [r.x, r.y, r.x + r.width, r.y + r.height] {
        s.try_to_physical(v)
            .ok_or_else(|| Error::new("INVALID_ARGUMENT", "rect edge unrepresentable at scale"))?;
    }
    Ok(r.physical(s))
}
pub(crate) fn px_dims(r: &PixelRect) -> (i64, i64) {
    (
        i64::from(r.right) - i64::from(r.left),
        i64::from(r.bottom) - i64::from(r.top),
    )
}
