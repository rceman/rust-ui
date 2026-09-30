//! DIP-space layout: top-down width, natural heights, finite waterfill for
//! `Length::Fill` distribution (per the docs: at most 2N weighted min/max
//! breakpoints, under-min overflows as clip, over-max leaves unused
//! surplus). Group/scope nodes are layout-transparent — their children
//! project into the surrounding axis without losing identity.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use windows::Win32::Foundation::RECT;

use crate::geom::{Align, Length, Point, Visibility};
use crate::node::{
    KIND_ACTION, KIND_BOX, KIND_COLUMN, KIND_GROUP, KIND_ROW, KIND_SCOPE, KIND_STACK,
    KIND_SURFACE, NodeData,
};
use crate::style::Insets;
use crate::runtime::UpdateCtx;
use crate::theme::{ControlSize, Space};
use crate::{NodeId, UiResult};

use super::{PeerCtx, WindowlessPeer};

/// A laid-out rect in DIP units.
#[derive(Copy, Clone, Debug, Default)]
pub(crate) struct DipRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl DipRect {
    pub(crate) fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.x < self.x + self.w && p.y >= self.y && p.y < self.y + self.h
    }
    fn win(&self) -> RECT {
        RECT {
            left: self.x.round() as i32,
            top: self.y.round() as i32,
            right: (self.x + self.w).round() as i32,
            bottom: (self.y + self.h).round() as i32,
        }
    }
}

fn space_px(s: Space) -> f32 {
    s.dp().0
}

/// Content insets for a node: painted box containers take padding from the
/// resolved `BoxStyle` (per-side, patch-aware); actions inset by the live
/// state's resolved style; layout-only containers keep `Space` padding.
/// Border widths also consume insets — the border stroke is inset from the
/// outer edge, so content sits inside the border line.
fn node_insets(ctx: &PeerCtx, id: NodeId, n: &crate::node::Node) -> Insets {
    match &n.data {
        NodeData::Container { kind, props } => {
            let b = props
                .resolved_box(*kind)
                .unwrap_or_default();
            let mut i = b.padding;
            // uniform border consumes insets inside its stroke
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
                ctx.pressed.get() == Some(id),
                ctx.hot.get() == Some(id),
                ctx.focus.get() == Some(id) && !*disabled,
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

/// Access a node's peer through the backend routing registry.
fn peer_of(ctx: &PeerCtx, id: NodeId) -> Option<Rc<RefCell<WindowlessPeer>>> {
    ctx.registry
        .lock()
        .unwrap()
        .get(&id.slot)
        .and_then(|w| w.upgrade())
}

/// Natural-size measure for one leaf at a proposed width.
///
/// Labels and buttons measure through the renderer's DWrite pipeline;
/// editors ask the peer (`EM_REQUESTRESIZE` at the layout width); custom
/// nodes have no intrinsic size (0×0 unless LayoutSpec constrains them).
fn natural<S, M, U, V>(
    rt: &crate::runtime::Runtime<S, M, U, V>,
    ctx: &PeerCtx,
    id: NodeId,
    avail_w: f32,
    measure: &mut dyn FnMut(&str, f32, f32) -> (f32, f32),
) -> (f32, f32)
where
    M: 'static,
    U: Fn(&mut S, M, &mut UpdateCtx<'_, M>),
    V: Fn(&S, &mut crate::Ui<'_, '_, M>),
{
    let Some(n) = rt.arena.get(id) else {
        return (0.0, 0.0);
    };
    if n.visibility == Visibility::Hidden {
        return (0.0, 0.0);
    }
    match &n.data {
        NodeData::Label {
            text, wrap, patch, ..
        } => {
            // resolved text style drives metrics — a patched Exact size is
            // a real measurement input, not just paint
            let mut ts = crate::style::label_recipe();
            ts.patch(patch);
            let sz = match ts.size {
                crate::style::TextSize::Body => 14.0,
                crate::style::TextSize::Exact(d) => d.0,
            };
            let w = if *wrap { avail_w } else { f32::MAX };
            measure(text, w, sz)
        }
        NodeData::Button {
            text,
            variant,
            style,
            disabled,
            size,
            ..
        } => {
            // resolved Normal-state style drives the natural size — state
            // branches change paint/insets; the reserved row height comes
            // from the size class, not the resolved padding
            let vs = crate::style::resolve_button(
                *variant,
                *size,
                style,
                crate::style::StyleState::Normal,
                false,
                ctx.colors.borrow().1.dark,
            );
            let ts = vs.text_style;
            let tsz = match ts.size {
                crate::style::TextSize::Body => 14.0,
                crate::style::TextSize::Exact(d) => d.0,
            };
            let (tw, _) = measure(text, f32::MAX, tsz);
            let bs = &vs.box_style;
            let (bw, bh) = border_insets(bs);
            let h = match size {
                ControlSize::Sm => 28.0,
                ControlSize::Md => 36.0,
                ControlSize::Lg => 44.0,
            };
            let w = tw + bs.padding.left.0 + bs.padding.right.0 + bw;
            let _ = disabled;
            let _ = bh;
            (w, h)
        }
        NodeData::Editor {
            multiline,
            max_lines,
            ..
        } => {
            if let Some(peer) = peer_of(ctx, id) {
                let (_, h) = peer
                    .borrow()
                    .natural_size(avail_w)
                    .unwrap_or((avail_w, 22.0));
                let line = 20.0;
                // cap applies to CONTENT height; the chrome inset (12) sits
                // on top — a single-line editor needs ~20+12 = 32 DIP or the
                // inner strip is too short for the line and draws nothing
                let cap = if *multiline {
                    max_lines.map(|n| n as f32 * line).unwrap_or(f32::MAX)
                } else {
                    line
                };
                (avail_w, h.min(cap) + 12.0)
            } else {
                (avail_w, if *multiline { 96.0 } else { 32.0 })
            }
        }
        NodeData::Custom { render, .. } => {
            // intrinsic size from CustomRender::measure within the offered box
            let want = render.measure(crate::geom::Constraints {
                min: crate::geom::Size::default(),
                max: crate::geom::Size {
                    width: avail_w,
                    height: f32::MAX,
                },
            });
            (want.width.min(avail_w), want.height)
        }
        NodeData::Container { kind, props } => {
            let pad = node_insets(ctx, id, n);
            let (pad_h, pad_v) = (pad.left.0 + pad.right.0, pad.top.0 + pad.bottom.0);
            let gap = props.gap.map(space_px).unwrap_or(0.0);
            let inner_w = (avail_w - pad_h).max(0.0);
            let kids: Vec<(f32, f32)> = n
                .children
                .iter()
                .filter_map(|slot| {
                    let g = rt.arena.generation_of(*slot);
                    let cid = NodeId {
                        slot: *slot,
                        generation: g,
                    };
                    rt.arena.get(cid)?;
                    Some(natural(rt, ctx, cid, inner_w, measure))
                })
                .collect();
            match *kind {
                // Row: width sums children + gaps, height is the max
                KIND_ROW => {
                    let w = kids.iter().map(|k| k.0).sum::<f32>()
                        + gap * kids.len().saturating_sub(1) as f32
                        + pad_h;
                    let h = kids.iter().map(|k| k.1).fold(0.0f32, f32::max) + pad_v;
                    (w.min(avail_w.max(0.0)), h)
                }
                // Stack/Surface/Box: children overlay — take the max box
                KIND_STACK | KIND_SURFACE | KIND_BOX => {
                    let w = kids.iter().map(|k| k.0).fold(0.0f32, f32::max) + pad_h;
                    let h = kids.iter().map(|k| k.1).fold(0.0f32, f32::max) + pad_v;
                    (w.min(avail_w.max(0.0)), h)
                }
                // Column, and transparent group/scope projecting onto the
                // enclosing vertical axis: heights sum like a column
                _ => {
                    let h = kids.iter().map(|k| k.1).sum::<f32>()
                        + gap * kids.len().saturating_sub(1) as f32
                        + pad_v;
                    (avail_w, h)
                }
            }
        }
        NodeData::Action { .. } => {
            // semantic action wraps decorative content — stack semantics:
            // children overlay inside the resolved live-state padding
            let pad = node_insets(ctx, id, n);
            let inner_w = (avail_w - pad.left.0 - pad.right.0).max(0.0);
            let kids: Vec<(f32, f32)> = n
                .children
                .iter()
                .filter_map(|slot| {
                    let g = rt.arena.generation_of(*slot);
                    let cid = NodeId {
                        slot: *slot,
                        generation: g,
                    };
                    rt.arena.get(cid)?;
                    Some(natural(rt, ctx, cid, inner_w, measure))
                })
                .collect();
            let w = kids.iter().map(|k| k.0).fold(0.0f32, f32::max) + pad.left.0 + pad.right.0;
            let h = kids.iter().map(|k| k.1).fold(0.0f32, f32::max) + pad.top.0 + pad.bottom.0;
            (w.min(avail_w.max(0.0)), h)
        }
        _ => (avail_w, 0.0),
    }
}

/// One child's main-axis request before waterfill.
struct Item {
    id: NodeId,
    weight: f32,
    min: f32,
    max: f32,
    natural: f32,
}

/// The documented finite waterfill: fixed/natural sizes first, then the
/// remaining axis distributes by weight; clamps at min/max. Under-min
/// overflows (the child keeps its size and the container clips it);
/// over-max leaves unused surplus at the axis end.
fn waterfill(avail: f32, items: &[Item]) -> Vec<f32> {
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

/// The whole layout pass — consumes the retained arena, produces DIP rects
/// for VISIBLE nodes and the depth-first paint/hit order.
pub(crate) struct LayoutCache {
    scale: f32,
}

impl LayoutCache {
    pub(crate) fn new(scale: f32) -> Self {
        LayoutCache { scale }
    }

    pub(crate) fn run<S, M, U, V>(
        &mut self,
        rt: &mut crate::runtime::Runtime<S, M, U, V>,
        ctx: &PeerCtx,
    ) -> UiResult<(HashMap<NodeId, DipRect>, Vec<NodeId>)>
    where
        M: 'static,
        U: Fn(&mut S, M, &mut UpdateCtx<'_, M>),
        V: Fn(&S, &mut crate::Ui<'_, '_, M>),
    {
        let mut rects: HashMap<NodeId, DipRect> = HashMap::new();
        let mut order: Vec<NodeId> = Vec::new();
        // window client size in DIP
        let (w, h) = client_dip(ctx.hwnd.get(), self.scale)?;
        let root_children = rt
            .arena
            .get(rt.root)
            .map(|n| n.children.clone())
            .unwrap_or_default();
        let padding = 16.0;
        let mut measurer = super::render::measure_fn();
        let items: Vec<Item> = root_children
            .iter()
            .filter_map(|slot| {
                let g = rt.arena.generation_of(*slot);
                let id = NodeId {
                    slot: *slot,
                    generation: g,
                };
                rt.arena.get(id)?;
                Some(id)
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|id| {
                let spec = rt.arena.get(id).unwrap().layout;
                let (nw, nh) = natural(rt, ctx, id, w - padding * 2.0, &mut measurer);
                item(id, spec, nw, nh, true)
            })
            .collect();
        let gaps = space_px(Space::Sm) * items.len().saturating_sub(1) as f32;
        let main = waterfill(h - padding * 2.0 - gaps, &items);
        let mut y = padding;
        for (it, size) in items.iter().zip(main.iter()) {
            self.walk(
                rt,
                ctx,
                it.id,
                DipRect {
                    x: padding,
                    y,
                    w: w - padding * 2.0,
                    h: *size,
                },
                &mut rects,
                &mut order,
                KIND_COLUMN,
            );
            y += *size + space_px(Space::Sm);
        }
        Ok((rects, order))
    }

    /// Recursively place one node: layout containers split, transparent
    /// kinds project children onto the enclosing axis, leaves take the
    /// whole rect.
    fn walk<S, M, U, V>(
        &self,
        rt: &mut crate::runtime::Runtime<S, M, U, V>,
        ctx: &PeerCtx,
        id: NodeId,
        rect: DipRect,
        rects: &mut HashMap<NodeId, DipRect>,
        order: &mut Vec<NodeId>,
        parent_axis: u8,
    ) where
        M: 'static,
        U: Fn(&mut S, M, &mut UpdateCtx<'_, M>),
        V: Fn(&S, &mut crate::Ui<'_, '_, M>),
    {
        let Some(n) = rt.arena.get(id) else {
            return;
        };
        if n.visibility == Visibility::Hidden {
            // footprint retained — no rect/order entry
            return;
        }
        let kind = match &n.data {
            NodeData::Container { kind, .. } => *kind,
            NodeData::Action { .. } => KIND_ACTION,
            _ => 0,
        };
        let props = match &n.data {
            NodeData::Container { props, .. } => *props,
            _ => Default::default(),
        };
        // peer bounds update (no recreate) — editors keep native state
        if let Some(peer) = peer_of(ctx, id) {
            peer.borrow().apply_bounds(rect.win(), self.scale);
        }
        rects.insert(id, rect);
        order.push(id);
        let children = n.children.clone();
        if children.is_empty() {
            return;
        }
        let mut measurer = super::render::measure_fn();
        let pad = node_insets(ctx, id, n);
        let inner = DipRect {
            x: rect.x + pad.left.0,
            y: rect.y + pad.top.0,
            w: (rect.w - pad.left.0 - pad.right.0).max(0.0),
            h: (rect.h - pad.top.0 - pad.bottom.0).max(0.0),
        };
        let ids: Vec<NodeId> = children
            .iter()
            .filter_map(|slot| {
                let g = rt.arena.generation_of(*slot);
                let cid = NodeId {
                    slot: *slot,
                    generation: g,
                };
                rt.arena.get(cid).map(|_| cid)
            })
            .collect::<Vec<NodeId>>();
        // Group/scope are layout-transparent: children project onto the
        // enclosing container's axis (default vertical) instead of
        // overlapping on the group's rect.
        let effective = match kind {
            KIND_GROUP | KIND_SCOPE => parent_axis,
            other => other,
        };
        match effective {
            KIND_COLUMN => {
                let gap = props.gap.map(space_px).unwrap_or(0.0);
                let items: Vec<Item> = ids
                    .iter()
                    .map(|&cid| {
                        let spec = rt.arena.get(cid).unwrap().layout;
                        let (_, nh) = natural(rt, ctx, cid, inner.w, &mut measurer);
                        item(cid, spec, inner.w, nh, true)
                    })
                    .collect();
                let sizes = waterfill(inner.h - gap * ids.len().saturating_sub(1) as f32, &items);
                let mut y = inner.y;
                for (it, sz) in items.iter().zip(sizes.iter()) {
                    // cross axis: align — Stretch fills, others keep natural
                    let (nw, _) = natural(rt, ctx, it.id, inner.w, &mut measurer);
                    let (w, x) = cross(
                        props.align.unwrap_or_default(),
                        rt.arena.get(it.id).unwrap().layout.width,
                        inner,
                        nw,
                    );
                    self.walk(rt, ctx, it.id, DipRect { x, y, w, h: *sz }, rects, order, kind);
                    y += *sz + gap;
                }
            }
            KIND_ROW => {
                let gap = props.gap.map(space_px).unwrap_or(0.0);
                let items: Vec<Item> = ids
                    .iter()
                    .map(|&cid| {
                        let spec = rt.arena.get(cid).unwrap().layout;
                        let (nw, _) = natural(rt, ctx, cid, inner.w, &mut measurer);
                        item(cid, spec, nw, inner.h, false)
                    })
                    .collect();
                let sizes = waterfill(inner.w - gap * ids.len().saturating_sub(1) as f32, &items);
                let mut x = inner.x;
                for (it, sz) in items.iter().zip(sizes.iter()) {
                    self.walk(
                        rt,
                        ctx,
                        it.id,
                        DipRect {
                            x,
                            y: inner.y,
                            w: *sz,
                            h: inner.h,
                        },
                        rects,
                        order,
                        kind,
                    );
                    x += *sz + gap;
                }
            }
            // Stack/Surface/Box/Action: children take the full inner rect
            // (declaration order paints later siblings on top).
            _ => {
                for cid in ids {
                    self.walk(rt, ctx, cid, inner, rects, order, kind);
                }
            }
        }
    }
}

/// Build a fill item from a LayoutSpec.
fn item(id: NodeId, spec: crate::geom::LayoutSpec, nw: f32, nh: f32, vertical: bool) -> Item {
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
fn cross(align: Align, len: Length, inner: DipRect, natural_w: f32) -> (f32, f32) {
    let want = match len {
        Length::Fixed(d) => d.0,
        Length::Fill(_) | Length::Content => natural_w.min(inner.w),
    };
    match align {
        Align::Stretch => (inner.w, inner.x),
        Align::Start => (want, inner.x),
        Align::Center => (want, inner.x + (inner.w - want) / 2.0),
        Align::End => (want, inner.x + inner.w - want),
    }
}

/// Window client size in DIP.
fn client_dip(hwnd: windows::Win32::Foundation::HWND, scale: f32) -> UiResult<(f32, f32)> {
    unsafe {
        let mut rc = RECT::default();
        windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut rc)
            .map_err(|e| crate::UiError::Platform(format!("GetClientRect: {e}")))?;
        Ok((
            (rc.right - rc.left) as f32 / scale,
            (rc.bottom - rc.top) as f32 / scale,
        ))
    }
}

/// Total border thickness consumed around content (horizontal, vertical).
fn border_insets(b: &crate::style::BoxStyle) -> (f32, f32) {
    if let Some(side) = b.border.uniform() {
        (side.width.0 * 2.0, side.width.0 * 2.0)
    } else {
        (
            b.border.left.width.0 + b.border.right.width.0,
            b.border.top.width.0 + b.border.bottom.width.0,
        )
    }
}
