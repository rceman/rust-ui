//! Shared logical-space layout policy — platform-neutral (PLATFORM_CONTRACTS).
//!
//! Everything here is pure `Dp` semantics: no native handles, no pixels,
//! no measurement calls. A backend supplies its own measurements and calls
//! these policies; it never redefines Fill distribution or editor insets.
//!
//!   Fill waterfill, editor chrome insets, node content insets — the ONE
//!   authority. Win32's `layout.rs` drives the pass; this file owns the
//!   arithmetic policy.

use crate::geom::{Align, Dp, Justify, Point, Rect, Side, Size};
use crate::node::{NodeData, NodeId};
use crate::style::{BoxStyle, Insets, TextInputStylePatch};

// ---------------------------------------------------------------------------
// Fill distribution — the documented finite waterfill
// ---------------------------------------------------------------------------

/// One child's main-axis request before waterfill.
#[derive(Copy, Clone, Debug)]
pub struct FillItem {
    pub weight: f32,
    pub min: f32,
    pub max: f32,
    pub natural: f32,
}

/// The documented finite waterfill: fixed/natural sizes first, then the
/// remaining axis distributes by weight; clamps at min/max. Under-min
/// overflows (the child keeps its size and the container clips it);
/// over-max leaves unused surplus at the axis end.
pub fn waterfill(avail: f32, items: &[FillItem]) -> Vec<f32> {
    let total_weight: f32 = items.iter().map(|i| i.weight).sum();
    let fixed: f32 = items
        .iter()
        .filter(|i| i.weight <= 0.0)
        .map(|i| i.natural.clamp(i.min, i.max))
        .sum();
    let remaining = (avail - fixed).max(0.0);
    // locked set: items whose clamp hit; redistribute among the rest
    let mut locked = vec![false; items.len()];
    let mut rem = remaining;
    let mut weight_left = total_weight;
    // at most N+1 rounds — each round locks >=1 item or converges
    for _ in 0..=items.len() {
        if weight_left <= 0.0 {
            break;
        }
        let mut progressed = false;
        for (i, it) in items.iter().enumerate() {
            if locked[i] || it.weight <= 0.0 {
                continue;
            }
            let share = rem * it.weight / weight_left;
            let clamped = share.clamp(it.min, it.max);
            if clamped != share {
                locked[i] = true;
                rem -= clamped;
                weight_left -= it.weight;
                progressed = true;
            }
        }
        if !progressed {
            break;
        }
    }
    items
        .iter()
        .enumerate()
        .map(|(i, it)| {
            if it.weight <= 0.0 {
                it.natural.clamp(it.min, it.max)
            } else if weight_left > 0.0 && !locked[i] {
                (rem * it.weight / weight_left).clamp(it.min, it.max)
            } else {
                // locked items kept their clamped share (recompute)
                let share = remaining * it.weight / total_weight;
                share.clamp(it.min, it.max)
            }
        })
        .collect()
}

/// Main-axis origin offset for a justified container — the ONE policy
/// shared by the Row (horizontal) and Column/Card (vertical) walk
/// branches so the two axes can never drift. `used` is the summed child
/// main-axis sizes plus gaps; slack is clamped at zero so overflow never
/// shifts origins negative, and Fill children have already consumed the
/// axis (leaving no slack) before this runs.
pub fn justify_offset(justify: Justify, inner_main: f32, used: f32) -> f32 {
    let slack = (inner_main - used).max(0.0);
    match justify {
        Justify::Start => 0.0,
        Justify::Center => slack / 2.0,
        Justify::End => slack,
    }
}

// ---------------------------------------------------------------------------
// Editor content inset — THE pill-chrome -> peer-surface transform
// ---------------------------------------------------------------------------

/// A node's resolved editor chrome — recipe + authored patch.
pub fn editor_chrome(patch: &TextInputStylePatch) -> BoxStyle {
    crate::style::resolve_text_input_chrome(patch)
}

/// The mechanical safety inset the rectangular peer sits inside
/// (approved rule — uses RESOLVED authored radii, pre-normalization):
///   left   = border.left.width   + padding.left   + max(top_left, bottom_left)
///   right  = border.right.width  + padding.right  + max(top_right, bottom_right)
///   top    = border.top.width    + padding.top    + max(top_left, top_right)
///   bottom = border.bottom.width + padding.bottom + max(bottom_left, bottom_right)
/// Shadow is ignored here (painted chrome only, never peer geometry).
pub fn editor_inset(c: &BoxStyle) -> Insets {
    let m = f32::max;
    Insets {
        left: Dp(c.border.left.width.0
            + c.padding.left.0
            + m(c.radii.top_left.0, c.radii.bottom_left.0)),
        right: Dp(c.border.right.width.0
            + c.padding.right.0
            + m(c.radii.top_right.0, c.radii.bottom_right.0)),
        top: Dp(c.border.top.width.0
            + c.padding.top.0
            + m(c.radii.top_left.0, c.radii.top_right.0)),
        bottom: Dp(c.border.bottom.width.0
            + c.padding.bottom.0
            + m(c.radii.bottom_left.0, c.radii.bottom_right.0)),
    }
}

/// Chrome rect → content rect (logical Dp). The peer's native surface
/// covers exactly this area; native coordinates are content-local.
pub fn editor_content_rect(r: Rect, c: &BoxStyle) -> Rect {
    let i = editor_inset(c);
    Rect {
        x: r.x + i.left.0,
        y: r.y + i.top.0,
        width: (r.width - i.left.0 - i.right.0).max(0.0),
        height: (r.height - i.top.0 - i.bottom.0).max(0.0),
    }
}

// ---------------------------------------------------------------------------
// Node content insets — padding + border consume inside the stroke
// ---------------------------------------------------------------------------

/// Interaction state at resolve time (layout sees live state because state
/// branches can carry insets).
#[derive(Copy, Clone, Default)]
pub struct Interaction {
    pub hot: Option<NodeId>,
    pub pressed: Option<NodeId>,
    pub focus: Option<NodeId>,
    pub dark: bool,
}

// ---------------------------------------------------------------------------
// Popup placement — anchored overlay geometry (pure policy; the backend
// supplies anchor rect, measured natural size, and the client viewport)
// ---------------------------------------------------------------------------

/// Place `natural` against `anchor` on `side` (the anchor edge the popup
/// opens from: Bottom = below), aligned on the cross axis per `align`.
/// `offset.x` shifts along the align axis, `offset.y` along the side axis
/// for Top/Bottom (swapped for Left/Right — offset is authored in
/// side/align terms). If the placed rect overflows `viewport` on the side
/// axis the side flips once; the result is then clamped inside the
/// viewport on both axes. Never negative extents.
pub fn resolve_popup_placement(
    anchor: Rect,
    side: Side,
    align: Align,
    offset: Point,
    natural: Size,
    viewport: Rect,
) -> Rect {
    let place = |side: Side| -> Rect {
        let (x, y) = match side {
            Side::Bottom => (0.0, anchor.y + anchor.height + offset.y),
            Side::Top => (0.0, anchor.y - natural.height - offset.y),
            Side::Right => (anchor.x + anchor.width + offset.x, 0.0),
            Side::Left => (anchor.x - natural.width - offset.x, 0.0),
        };
        // cross-axis align: horizontal for top/bottom, vertical for sides
        let (x, y) = match side {
            Side::Top | Side::Bottom => (
                match align {
                    Align::Start => anchor.x,
                    Align::End => anchor.x + anchor.width - natural.width,
                    // Center (and a nonsensical Stretch) centers
                    _ => anchor.x + (anchor.width - natural.width) / 2.0,
                } + offset.x,
                y,
            ),
            Side::Left | Side::Right => (
                x,
                match align {
                    Align::Start => anchor.y,
                    Align::End => anchor.y + anchor.height - natural.height,
                    _ => anchor.y + (anchor.height - natural.height) / 2.0,
                } + offset.y,
            ),
        };
        Rect {
            x,
            y,
            width: natural.width,
            height: natural.height,
        }
    };
    let mut r = place(side);
    // side-axis overflow → flip once to the opposite edge
    let overflows = |side: Side, r: &Rect| match side {
        Side::Bottom => r.y + r.height > viewport.y + viewport.height,
        Side::Top => r.y < viewport.y,
        Side::Right => r.x + r.width > viewport.x + viewport.width,
        Side::Left => r.x < viewport.x,
    };
    if overflows(side, &r) {
        let flipped = place(side.flipped());
        if !overflows(side.flipped(), &flipped) {
            r = flipped;
        }
    }
    // clamp inside the viewport — an oversized popup pins to the edge
    let max_x = (viewport.x + viewport.width - r.width).max(viewport.x);
    let max_y = (viewport.y + viewport.height - r.height).max(viewport.y);
    r.x = r.x.clamp(viewport.x, max_x);
    r.y = r.y.clamp(viewport.y, max_y);
    r
}

/// Content insets for a node: painted box containers take padding from the
/// resolved `BoxStyle` (per-side, patch-aware); actions inset by the live
/// state's resolved style; layout-only containers keep `Space` padding.
/// Border widths also consume insets — the border stroke is inset from the
/// outer edge, so content sits inside the border line.
pub fn node_insets(id: NodeId, n: &crate::node::Node, ix: &Interaction) -> Insets {
    match &n.data {
        NodeData::Container { kind, props } => {
            let b = props.resolved_box(*kind).unwrap_or_default();
            let mut i = b.padding;
            if let Some(side) = b.border.uniform() {
                let w = side.width.0;
                i.top.0 += w;
                i.right.0 += w;
                i.bottom.0 += w;
                i.left.0 += w;
            } else {
                i.top.0 += b.border.top.width.0;
                i.right.0 += b.border.right.width.0;
                i.bottom.0 += b.border.bottom.width.0;
                i.left.0 += b.border.left.width.0;
            }
            if i == Insets::default() {
                return props
                    .padding
                    .map(|s| Insets::all(s.dp()))
                    .unwrap_or_default();
            }
            i
        }
        NodeData::Popup { props, .. } => {
            // popup chrome resolves like `surface` — padding + border
            let b = props
                .resolved_box(crate::node::KIND_POPUP)
                .unwrap_or_default();
            let mut i = b.padding;
            if let Some(side) = b.border.uniform() {
                let w = side.width.0;
                i.top.0 += w;
                i.right.0 += w;
                i.bottom.0 += w;
                i.left.0 += w;
            } else {
                i.top.0 += b.border.top.width.0;
                i.right.0 += b.border.right.width.0;
                i.bottom.0 += b.border.bottom.width.0;
                i.left.0 += b.border.left.width.0;
            }
            i
        }
        NodeData::Action {
            style, disabled, ..
        } => {
            // effective visual state = runtime OR staged presentation
            let b = style.resolve(
                *disabled,
                ix.pressed == Some(id) || n.presented.pressed,
                ix.hot == Some(id) || n.presented.hover,
                (ix.focus == Some(id) || n.presented.focus_visible) && !*disabled,
            );
            let mut i = b.padding;
            if let Some(side) = b.border.uniform() {
                let w = side.width.0;
                i.top.0 += w;
                i.right.0 += w;
                i.bottom.0 += w;
                i.left.0 += w;
            }
            i
        }
        NodeData::Label {
            chrome: Some(chrome),
            ..
        } => {
            // framed text (h2 rule / inline-code surface) — padding +
            // border inset the text measure, mirroring a `box_` child
            let mut i = chrome.padding;
            i.top.0 += chrome.border.top.width.0;
            i.right.0 += chrome.border.right.width.0;
            i.bottom.0 += chrome.border.bottom.width.0;
            i.left.0 += chrome.border.left.width.0;
            i
        }
        _ => Insets::default(),
    }
}

/// Effective visual-state bit = runtime owner OR staged presented flag.
/// The one OR rule every paint/layout/dirty-resolution path shares —
/// presented never substitutes the runtime fields, it only adds.
pub fn presented_or(runtime: Option<crate::NodeId>, id: crate::NodeId, flag: bool) -> bool {
    runtime == Some(id) || flag
}

/// The frozen ring's stroke-centerline radius for an element corner of
/// `component_radius` (already normalized): the 3dp `ring/50` band is
/// stroked 1.5dp outside the element edge — centerline = r + 1.5 (CSS
/// `0 0 0 3px` box-shadow spread semantics; outer edge = r + 3).
mod measure;
pub(crate) use measure::*;
