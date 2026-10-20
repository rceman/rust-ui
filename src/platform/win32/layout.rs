//! DIP-space layout: top-down width, natural heights, finite waterfill for
//! `Length::Fill` distribution (per the docs: at most 2N weighted min/max
//! breakpoints, under-min overflows as clip, over-max leaves unused
//! surplus). Group/scope nodes are layout-transparent — their children
//! project into the surrounding axis without losing identity.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use windows::Win32::Foundation::RECT;

use crate::geom::{Align, Length, Visibility};
use crate::node::{
    KIND_ACTION, KIND_BOX, KIND_CARD, KIND_COLUMN, KIND_GROUP, KIND_ROW, KIND_SCOPE, KIND_SCROLL,
    KIND_STACK, KIND_SURFACE, NodeData,
};
use crate::runtime::UpdateCtx;
use crate::style::Insets;
use crate::theme::{ControlSize, Space};
use crate::{NodeId, UiResult};

use super::space::{LogicalRect, ScaleFactor};
use super::{PeerCtx, WindowlessPeer};

// THE laid-out rect type is `space::LogicalRect` — the canonical layer owns
// DIP geometry and px conversion; nothing here may invent units.

// THE editor-content transform lives in `crate::layout` — the shared
// authority. These re-exports keep the win32 pass readable without a
// second copy of the policy.
pub(crate) use crate::layout::{editor_chrome, editor_content_rect, editor_inset};

fn space_px(s: Space) -> f32 {
    s.dp().0
}

/// Shared policy — the live interaction state is read once per pass.
fn node_insets(ctx: &PeerCtx, id: NodeId, n: &crate::node::Node) -> Insets {
    crate::layout::node_insets(
        id,
        n,
        &crate::layout::Interaction {
            hot: ctx.hot.get(),
            pressed: ctx.pressed.get(),
            focus: ctx.focus.get(),
            dark: ctx.colors.borrow().1.dark,
        },
    )
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
    measure: &mut dyn FnMut(&str, f32, &crate::style::TextStyle) -> (f32, f32),
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
    let (nw, nh) = match &n.data {
        NodeData::Label {
            text,
            wrap,
            patch,
            chrome,
            ..
        } => {
            // resolved text style drives metrics — a patched Exact size is
            // a real measurement input, not just paint
            let mut ts = crate::style::label_recipe();
            ts.patch(patch);
            let w = if *wrap {
                let ins = crate::layout::node_insets(
                    id,
                    n,
                    &crate::layout::Interaction {
                        dark: ctx.colors.borrow().1.dark,
                        ..Default::default()
                    },
                );
                (avail_w - ins.left.0 - ins.right.0).max(0.0)
            } else {
                f32::MAX
            };
            let (tw, th) = measure(text, w, &ts);
            let ins = crate::layout::node_insets(
                id,
                n,
                &crate::layout::Interaction {
                    dark: ctx.colors.borrow().1.dark,
                    ..Default::default()
                },
            );
            let _ = chrome;
            (
                if *wrap {
                    avail_w
                } else {
                    tw + ins.left.0 + ins.right.0
                },
                th + ins.top.0 + ins.bottom.0,
            )
        }
        NodeData::Icon { size, .. } => (size.0, size.0),
        NodeData::Button {
            text,
            variant,
            style,
            disabled,
            size,
            icon_start,
            icon_end,
            icon_only,
            invalid,
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
                *invalid,
            );
            let h = size.height().0;
            if *icon_only {
                return (h, h);
            }
            let ins = crate::layout::button_content_insets(&n.data, &vs);
            let (tw, _) = measure(text, f32::MAX, &vs.text_style);
            // insets already reserve icon size+gap on the icon side(s);
            // natural width = ink advance + content insets + a uniform
            // ink-vs-layout epsilon (GetMetrics width is the ink box —
            // the laid-out line can need a hair more; not per-label)
            let w = tw + ins.left.0 + ins.right.0 + 2.0;
            let _ = disabled;
            (w, h)
        }
        NodeData::Editor {
            multiline,
            max_lines,
            patch,
            ..
        } => {
            let chrome = editor_chrome(patch);
            let ins = editor_inset(&chrome);
            let inset_v = ins.top.0 + ins.bottom.0;
            if let Some(peer) = peer_of(ctx, id) {
                let peer = peer.borrow();
                let (_, h) = peer
                    .natural_size((avail_w - ins.left.0 - ins.right.0).max(0.0))
                    .unwrap_or((avail_w, 22.0));
                // the cap is msftedit's MEASURED line height — a hardcoded
                // guess clipped every line's descenders (P0). Falls back
                // to 25 until a natural_size pass latches the real metric.
                let line = {
                    let m = peer.line_height();
                    if m > 0.0 { m } else { 25.0 }
                };
                let cap = if *multiline {
                    max_lines.map(|n| n as f32 * line).unwrap_or(f32::MAX)
                } else {
                    line
                };
                (avail_w, h.min(cap) + inset_v)
            } else {
                (avail_w, if *multiline { 96.0 } else { 32.0 })
            }
        }
        NodeData::Toggle { kind, size, .. } => crate::layout::toggle_natural_size(*kind, *size),
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
    };
    // The node's OWN LayoutSpec is part of its intrinsic size, not just a
    // waterfill input: a `Fixed` length pins the axis regardless of
    // measured content (an empty Fixed(64) box still measures 64), and
    // min/max clamp the result. Parent containers consume this natural
    // size to shape THEIR measure — honoring it here is what lets a
    // fixed-size container propagate its authored extent upward instead
    // of collapsing to its (possibly empty) children's measure.
    let spec = n.layout;
    let axis = |len: Length, mn: Option<crate::geom::Dp>, mx: Option<crate::geom::Dp>, nat: f32| {
        let base = match len {
            Length::Fixed(d) => d.0,
            _ => nat,
        };
        base.clamp(
            mn.map(|d| d.0).unwrap_or(0.0),
            mx.map(|d| d.0).unwrap_or(f32::MAX),
        )
    };
    (
        axis(spec.width, spec.min_width, spec.max_width, nw),
        axis(spec.height, spec.min_height, spec.max_height, nh),
    )
}

/// One child's main-axis request before waterfill — the id rides along;
/// the shared policy (`crate::layout::waterfill`) consumes `FillItem`.
struct Item {
    id: NodeId,
    weight: f32,
    min: f32,
    max: f32,
    natural: f32,
}

/// Shared Fill waterfill — single authority in `crate::layout`.
fn waterfill(avail: f32, items: &[Item]) -> Vec<f32> {
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
pub(crate) struct LayoutCache {
    scale: ScaleFactor,
}

impl LayoutCache {
    pub(crate) fn new(scale: ScaleFactor) -> Self {
        LayoutCache { scale }
    }

    pub(crate) fn run<S, M, U, V>(
        &mut self,
        rt: &mut crate::runtime::Runtime<S, M, U, V>,
        ctx: &PeerCtx,
    ) -> UiResult<(
        HashMap<NodeId, LogicalRect>,
        Vec<NodeId>,
        HashMap<NodeId, LogicalRect>,
    )>
    where
        M: 'static,
        U: Fn(&mut S, M, &mut UpdateCtx<'_, M>),
        V: Fn(&S, &mut crate::Ui<'_, '_, M>),
    {
        let mut rects: HashMap<NodeId, LogicalRect> = HashMap::new();
        let mut order: Vec<NodeId> = Vec::new();
        // scroll-clip map: descendant NodeId -> the viewport clip that
        // bounds its paint/hit/peer geometry (nested viewports intersect)
        let mut clips: HashMap<NodeId, LogicalRect> = HashMap::new();
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
                LogicalRect {
                    x: padding,
                    y,
                    width: w - padding * 2.0,
                    // the window bounds the layout space: a root child
                    // never gets more than the remaining client height,
                    // so a Fill scroll viewport inside it measures
                    // against the VISIBLE area — identical while content
                    // fits (min is a no-op), engaging exactly when the
                    // page would overflow the window
                    height: (*size).min((h - y).max(0.0)),
                },
                &mut rects,
                &mut order,
                &mut clips,
                None,
                KIND_COLUMN,
            )?;
            y += *size + space_px(Space::Sm);
        }
        Ok((rects, order, clips))
    }

    /// Recursively place one node: layout containers split, transparent
    /// kinds project children onto the enclosing axis, leaves take the
    /// whole rect.
    pub(crate) fn walk<S, M, U, V>(
        &self,
        rt: &mut crate::runtime::Runtime<S, M, U, V>,
        ctx: &PeerCtx,
        id: NodeId,
        rect: LogicalRect,
        rects: &mut HashMap<NodeId, LogicalRect>,
        order: &mut Vec<NodeId>,
        clips: &mut HashMap<NodeId, LogicalRect>,
        clip: Option<LogicalRect>,
        parent_axis: u8,
    ) -> UiResult
    where
        M: 'static,
        U: Fn(&mut S, M, &mut UpdateCtx<'_, M>),
        V: Fn(&S, &mut crate::Ui<'_, '_, M>),
    {
        let Some(n) = rt.arena.get(id) else {
            return Ok(());
        };
        if n.visibility == Visibility::Hidden {
            // footprint retained — no rect/order entry
            return Ok(());
        }
        // a scroll viewport ancestor confines this node: paint, hit-test
        // and peer geometry all consume the same effective clip
        if let Some(c) = clip {
            clips.insert(id, c);
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
        // peer bounds update (no recreate) — editors keep native state.
        // `editor_content_rect` is THE transform — the peer's host bounds,
        // draw destination, pointer/caret spaces all agree because they
        // share this mapping.
        if let Some(peer) = peer_of(ctx, id)
            && let NodeData::Editor { patch, .. } = &n.data
        {
            // relatch errors surface through the layout result — a failed
            // deactivation must not leave a stale-scale view silently.
            // apply_bounds receives the FULL translated content rect —
            // the peer surface IS the whole document: the frame's pushed
            // clip cuts the visible band out of the peer's blit (peers
            // composite through a per-peer scratch target, so clips reach
            // them at blit time), and pointer/caret math maps window
            // coordinates onto TRUE document offsets — a top clip shows
            // mid-document content and clicks land on the glyphs seen.
            let full = editor_content_rect(rect, &editor_chrome(patch));
            peer.borrow()
                .apply_bounds(full, self.scale)
                .map_err(|e| crate::UiError::Platform(format!("apply_bounds: {e}")))?;
        }
        rects.insert(id, rect);
        order.push(id);
        let children = n.children.clone();
        if children.is_empty() {
            return Ok(());
        }
        let mut measurer = super::render::measure_fn();
        let pad = node_insets(ctx, id, n);
        let inner = LogicalRect {
            x: rect.x + pad.left.0,
            y: rect.y + pad.top.0,
            width: (rect.width - pad.left.0 - pad.right.0).max(0.0),
            height: (rect.height - pad.top.0 - pad.bottom.0).max(0.0),
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
            // Scroll viewport: children stack as a column at UNBOUNDED
            // height, translated by the app-owned offset clamped to the
            // measured extent. Descendant rects are stored TRANSLATED
            // (below-fold = off-viewport) — clipping is enforced at
            // paint/hit/peer-boundaries through the clips map.
            KIND_SCROLL => {
                let gap = props.gap.map(space_px).unwrap_or(0.0);
                let mut laid: Vec<(NodeId, crate::geom::LayoutSpec, f32, f32)> =
                    Vec::with_capacity(ids.len());
                for &cid in &ids {
                    let spec = rt.arena.get(cid).unwrap().layout;
                    let (nw, nh) = natural(rt, ctx, cid, inner.width, &mut measurer);
                    // unbounded axis — no waterfill: Fixed wins, every
                    // other length takes the natural measure, spec
                    // min/max still clamp
                    let h = match spec.height {
                        Length::Fixed(d) => d.0,
                        _ => nh,
                    }
                    .clamp(
                        spec.min_height.map(|d| d.0).unwrap_or(0.0),
                        spec.max_height.map(|d| d.0).unwrap_or(f32::MAX),
                    );
                    laid.push((cid, spec, nw, h));
                }
                let used = laid.iter().map(|i| i.3).sum::<f32>()
                    + gap * laid.len().saturating_sub(1) as f32;
                let max_scroll = (used - inner.height).max(0.0);
                let applied = props.scroll.0.clamp(0.0, max_scroll);
                // the viewport clip is the scroll's INNER rect; a nested
                // viewport intersects the inherited one
                let child_clip = clip.map_or(inner, |c| c.intersection(inner));
                let mut y = inner.y - applied;
                for (cid, spec, nw, h) in laid {
                    let (w, x) = cross(props.align.unwrap_or_default(), spec.width, inner, nw);
                    // transparent children project onto the column axis —
                    // the scroll arm is consumed HERE, not re-applied by
                    // a group/scope descendant
                    self.walk(
                        rt,
                        ctx,
                        cid,
                        LogicalRect {
                            x,
                            y,
                            width: w,
                            height: h,
                        },
                        rects,
                        order,
                        clips,
                        Some(child_clip),
                        KIND_COLUMN,
                    )?;
                    y += h + gap;
                }
            }
            // Card is column semantics — parts lay out vertically inside
            // the chrome. KIND_CARD joins the column branch.
            KIND_COLUMN | KIND_CARD => {
                let gap = props.gap.map(space_px).unwrap_or(0.0);
                let items: Vec<Item> = ids
                    .iter()
                    .map(|&cid| {
                        let spec = rt.arena.get(cid).unwrap().layout;
                        let (_, nh) = natural(rt, ctx, cid, inner.width, &mut measurer);
                        item(cid, spec, inner.width, nh, true)
                    })
                    .collect();
                let sizes = waterfill(
                    inner.height - gap * ids.len().saturating_sub(1) as f32,
                    &items,
                );
                // main-axis justify — symmetric with KIND_ROW through the
                // shared `justify_offset` policy: Start packs at inner.y,
                // Center/End offset the whole used span, overflow clamps
                // slack to zero so origins never shift negative. Fill
                // children consume the space first.
                let used: f32 =
                    sizes.iter().sum::<f32>() + gap * sizes.len().saturating_sub(1) as f32;
                let mut y = inner.y
                    + crate::layout::justify_offset(
                        props.justify.unwrap_or_default(),
                        inner.height,
                        used,
                    );
                for (it, sz) in items.iter().zip(sizes.iter()) {
                    // cross axis: align — Stretch fills, others keep natural
                    let (nw, _) = natural(rt, ctx, it.id, inner.width, &mut measurer);
                    let (w, x) = cross(
                        props.align.unwrap_or_default(),
                        rt.arena.get(it.id).unwrap().layout.width,
                        inner,
                        nw,
                    );
                    self.walk(
                        rt,
                        ctx,
                        it.id,
                        LogicalRect {
                            x,
                            y,
                            width: w,
                            height: *sz,
                        },
                        rects,
                        order,
                        clips,
                        clip,
                        kind,
                    )?;
                    y += *sz + gap;
                }
            }
            KIND_ROW => {
                let gap = props.gap.map(space_px).unwrap_or(0.0);
                let items: Vec<Item> = ids
                    .iter()
                    .map(|&cid| {
                        let spec = rt.arena.get(cid).unwrap().layout;
                        let (nw, _) = natural(rt, ctx, cid, inner.width, &mut measurer);
                        item(cid, spec, nw, inner.height, false)
                    })
                    .collect();
                let sizes = waterfill(
                    inner.width - gap * ids.len().saturating_sub(1) as f32,
                    &items,
                );
                // main-axis justify — Start (default) packs at inner.x,
                // Center/End offset the whole used span via the shared
                // `justify_offset` policy. No SpaceBetween variant exists
                // in the current Justify contract.
                let used: f32 =
                    sizes.iter().sum::<f32>() + gap * sizes.len().saturating_sub(1) as f32;
                let mut x = inner.x
                    + crate::layout::justify_offset(
                        props.justify.unwrap_or_default(),
                        inner.width,
                        used,
                    );
                for (it, sz) in items.iter().zip(sizes.iter()) {
                    // cross axis: align — Stretch fills the row height,
                    // Start/Center/End keep the child's natural height
                    let (_, nh) = natural(rt, ctx, it.id, inner.width, &mut measurer);
                    let (h, y) = cross_v(
                        props.align.unwrap_or_default(),
                        rt.arena.get(it.id).unwrap().layout.height,
                        inner,
                        nh,
                    );
                    self.walk(
                        rt,
                        ctx,
                        it.id,
                        LogicalRect {
                            x,
                            y,
                            width: *sz,
                            height: h,
                        },
                        rects,
                        order,
                        clips,
                        clip,
                        kind,
                    )?;
                    x += *sz + gap;
                }
            }
            // Stack/Surface/Box/Action: children take the full inner rect
            // (declaration order paints later siblings on top).
            _ => {
                for cid in ids {
                    self.walk(rt, ctx, cid, inner, rects, order, clips, clip, kind)?;
                }
            }
        }
        Ok(())
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
fn cross(align: Align, len: Length, inner: LogicalRect, natural_w: f32) -> (f32, f32) {
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
fn cross_v(align: Align, len: Length, inner: LogicalRect, natural_h: f32) -> (f32, f32) {
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
fn client_dip(hwnd: windows::Win32::Foundation::HWND, scale: ScaleFactor) -> UiResult<(f32, f32)> {
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
