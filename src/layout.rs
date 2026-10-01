//! Shared logical-space layout policy — platform-neutral (PLATFORM_CONTRACTS).
//!
//! Everything here is pure `Dp` semantics: no native handles, no pixels,
//! no measurement calls. A backend supplies its own measurements and calls
//! these policies; it never redefines Fill distribution or editor insets.
//!
//!   Fill waterfill, editor chrome insets, node content insets — the ONE
//!   authority. Win32's `layout.rs` drives the pass; this file owns the
//!   arithmetic policy.

use crate::geom::{Dp, Rect};
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
        NodeData::Action {
            style, disabled, ..
        } => {
            let b = style.resolve(
                *disabled,
                ix.pressed == Some(id),
                ix.hot == Some(id),
                ix.focus == Some(id) && !*disabled,
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
        _ => Insets::default(),
    }
}
