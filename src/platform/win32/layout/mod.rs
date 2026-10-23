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
use crate::theme::Space;
use crate::{NodeId, UiResult};

use super::space::{LogicalRect, ScaleFactor};
pub(super) use super::{PeerCtx, RcWeak, WindowlessPeer};

// THE laid-out rect type is `space::LogicalRect` — the canonical layer owns
// DIP geometry and px conversion; nothing here may invent units.

// THE editor-content transform lives in `crate::layout` — the shared
// authority. These re-exports keep the win32 pass readable without a
// second copy of the policy.
pub(crate) use crate::layout::{editor_chrome, editor_content_rect, editor_inset};

mod flex;
mod natural;
mod walk;

pub(super) use flex::*;
pub(super) use natural::*;

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
        // popups are deferred: they lay out LAST — each resolves against
        // its anchor's final rect inside the client viewport, lands at
        // the END of `order` (paint-last/hit-first), and carries no clip
        let mut deferred: Vec<NodeId> = Vec::new();
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
                &mut deferred,
                None,
                KIND_COLUMN,
            )?;
            y += *size + space_px(Space::Sm);
        }
        // popup placement — staging order, each against its anchor's
        // committed rect (translated anchor rects make reposition free on
        // scroll). A clipped-out anchor hides the popup truthfully.
        // `nested` is a REAL worklist: a popup staged inside another
        // popup's subtree defers into it during the children walk and is
        // appended back for its own placement pass — arbitrary nesting
        // depth resolves because parents are placed first.
        let viewport = LogicalRect {
            x: 0.0,
            y: 0.0,
            width: w,
            height: h,
        };
        let mut nested: Vec<NodeId> = Vec::new();
        let mut i = 0;
        while i < deferred.len() {
            let id = deferred[i];
            i += 1;
            let Some(n) = rt.arena.get(id) else {
                continue;
            };
            let NodeData::Popup {
                anchor,
                side,
                align,
                offset,
                modal,
                ..
            } = &n.data
            else {
                continue;
            };
            let (side, align, offset, modal) = (*side, *align, *offset, *modal);
            // anchor rect = its laid-out rect intersected with its own
            // scroll clip — the popup tracks the VISIBLE anchor
            let anchor_rect = anchor.as_deref().and_then(|wanted| {
                (0..rt.arena.slot_len() as u32).find_map(|slot| {
                    let n = rt.arena.slot(slot)?;
                    (n.automation_id.as_deref() == Some(wanted)).then(|| NodeId {
                        slot,
                        generation: rt.arena.generation_of(slot),
                    })
                })
            });
            let Some(ar) = anchor_rect.and_then(|a| rects.get(&a).copied()).map(|r| {
                anchor_rect
                    .and_then(|a| clips.get(&a).copied())
                    .map_or(r, |c| r.intersection(c))
            }) else {
                continue; // unanchored or unlaid-out anchor — hidden
            };
            if ar.width <= 0.0 || ar.height <= 0.0 {
                continue; // anchor fully clipped — truthful hide
            }
            let (nw, nh) = natural(rt, ctx, id, viewport.width, &mut measurer);
            let content = crate::layout::resolve_popup_placement(
                crate::geom::Rect {
                    x: ar.x,
                    y: ar.y,
                    width: ar.width,
                    height: ar.height,
                },
                side,
                align,
                offset,
                crate::geom::Size {
                    width: nw,
                    height: nh,
                },
                crate::geom::Rect {
                    x: viewport.x,
                    y: viewport.y,
                    width: viewport.width,
                    height: viewport.height,
                },
            );
            // the popup lands LAST in paint order => paint-last /
            // hit-test-first. Modal takes the whole client as its rect
            // (the scrim); non-modal takes the resolved placement. Either
            // way its CHILDREN lay out inside the content box minus the
            // popup chrome insets — walking `id` itself here would
            // re-defer the popup into a discarded list and drop the
            // entire subtree out of rects/order/clips.
            rects.insert(id, if modal { viewport } else { content });
            order.push(id);
            let pad = node_insets(ctx, id, n);
            let inner = LogicalRect {
                x: content.x + pad.left.0,
                y: content.y + pad.top.0,
                width: (content.width - pad.left.0 - pad.right.0).max(0.0),
                height: (content.height - pad.top.0 - pad.bottom.0).max(0.0),
            };
            for slot in n.children.clone() {
                let g = rt.arena.generation_of(slot);
                let cid = NodeId {
                    slot,
                    generation: g,
                };
                if rt.arena.get(cid).is_some() {
                    self.walk(
                        rt,
                        ctx,
                        cid,
                        inner,
                        &mut rects,
                        &mut order,
                        &mut clips,
                        &mut nested,
                        None,
                        KIND_STACK,
                    )?;
                }
            }
            deferred.extend(nested.drain(..));
        }
        Ok((rects, order, clips))
    }
}
