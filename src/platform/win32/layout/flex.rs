use super::*;

/// One child's main-axis request before waterfill — the id rides along;
/// the shared policy (`crate::layout::waterfill`) consumes `FillItem`.
pub(super) struct Item {
    pub(super) id: NodeId,
    pub(super) weight: f32,
    pub(super) min: f32,
    pub(super) max: f32,
    pub(super) natural: f32,
}

/// Shared Fill waterfill — single authority in `crate::layout`.
pub(super) fn waterfill(avail: f32, items: &[Item]) -> Vec<f32> {
    crate::layout::waterfill(
        avail,
        &items
            .iter()
            .map(|i| crate::layout::FillItem {
                weight: i.weight,
                min: i.min,
                max: i.max,
                natural: i.natural,
            })
            .collect::<Vec<_>>(),
    )
}

/// The whole layout pass — consumes the retained arena, produces DIP rects
/// for VISIBLE nodes and the depth-first paint/hit order.

/// Build a fill item from a LayoutSpec.
pub(super) fn item(
    id: NodeId,
    spec: crate::geom::LayoutSpec,
    nw: f32,
    nh: f32,
    vertical: bool,
) -> Item {
    let (len, mn, mx) = if vertical {
        (spec.height, spec.min_height, spec.max_height)
    } else {
        (spec.width, spec.min_width, spec.max_width)
    };
    let nat = if vertical { nh } else { nw };
    let (weight, natural) = match len {
        Length::Fill(w) => (w as f32, nat),
        Length::Fixed(d) => (0.0, d.0),
        Length::Content => (0.0, nat),
    };
    Item {
        id,
        weight,
        min: mn.map(|d| d.0).unwrap_or(0.0),
        max: mx.map(|d| d.0).unwrap_or(f32::MAX),
        natural,
    }
}

/// Cross-axis placement inside `inner` per `Align` and the child's width
/// spec.
pub(super) fn cross(align: Align, len: Length, inner: LogicalRect, natural_w: f32) -> (f32, f32) {
    let want = match len {
        Length::Fixed(d) => d.0,
        Length::Fill(_) | Length::Content => natural_w.min(inner.width),
    };
    match align {
        // stretch fills the cross axis only for UNSIZED children — an
        // authored Fixed length is an explicit extent and wins (the
        // frozen `align-items:stretch` rule: stretch applies when the
        // cross size is auto)
        Align::Stretch => match len {
            Length::Fixed(_) => (want, inner.x),
            _ => (inner.width, inner.x),
        },
        Align::Start => (want, inner.x),
        Align::Center => (want, inner.x + (inner.width - want) / 2.0),
        Align::End => (want, inner.x + inner.width - want),
    }
}

/// Vertical cross-axis for rows — same align contract as `cross` but on
/// `inner.y`/`inner.height` with the child's natural height.
pub(super) fn cross_v(align: Align, len: Length, inner: LogicalRect, natural_h: f32) -> (f32, f32) {
    let want = match len {
        Length::Fixed(d) => d.0,
        Length::Fill(_) | Length::Content => natural_h.min(inner.height),
    };
    match align {
        // same stretch-vs-fixed rule as `cross` — an authored Fixed
        // height keeps its extent inside a stretch-aligned row
        Align::Stretch => match len {
            Length::Fixed(_) => (want, inner.y),
            _ => (inner.height, inner.y),
        },
        Align::Start => (want, inner.y),
        Align::Center => (want, inner.y + (inner.height - want) / 2.0),
        Align::End => (want, inner.y + inner.height - want),
    }
}

/// Window client size in DIP — GetClientRect returns physical px.
pub(crate) fn client_dip(
    hwnd: windows::Win32::Foundation::HWND,
    scale: ScaleFactor,
) -> UiResult<(f32, f32)> {
    unsafe {
        let mut rc = RECT::default();
        windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut rc)
            .map_err(|e| crate::UiError::Platform(format!("GetClientRect: {e}")))?;
        Ok((
            scale.to_logical(rc.right - rc.left),
            scale.to_logical(rc.bottom - rc.top),
        ))
    }
}

/// Total border thickness consumed around content (horizontal, vertical).
pub(super) fn border_insets(b: &crate::style::BoxStyle) -> (f32, f32) {
    if let Some(side) = b.border.uniform() {
        (side.width.0 * 2.0, side.width.0 * 2.0)
    } else {
        (
            b.border.left.width.0 + b.border.right.width.0,
            b.border.top.width.0 + b.border.bottom.width.0,
        )
    }
}
