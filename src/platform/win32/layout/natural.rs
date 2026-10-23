use super::*;

pub(super) fn space_px(s: Space) -> f32 {
    s.dp().0
}

/// Shared policy — the live interaction state is read once per pass.
pub(super) fn node_insets(ctx: &PeerCtx, id: NodeId, n: &crate::node::Node) -> Insets {
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
pub(super) fn peer_of(ctx: &PeerCtx, id: NodeId) -> Option<Rc<RefCell<WindowlessPeer>>> {
    ctx.registry
        .lock()
        .unwrap()
        .get(&id.slot)
        .and_then(|w: &RcWeak<RefCell<WindowlessPeer>>| w.upgrade())
}

/// Natural-size measure for one leaf at a proposed width.
///
/// Labels and buttons measure through the renderer's DWrite pipeline;
/// editors ask the peer (`EM_REQUESTRESIZE` at the layout width); custom
/// nodes have no intrinsic size (0×0 unless LayoutSpec constrains them).
pub(super) fn natural<S, M, U, V>(
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
            selected,
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
                *selected,
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
            // the laid-out line can need a hair more; not per-label).
            // The frozen select/tabs cells measure ink+insets exactly.
            let eps = match variant {
                crate::theme::ButtonVariant::Select
                | crate::theme::ButtonVariant::SelectItem
                | crate::theme::ButtonVariant::Tab
                | crate::theme::ButtonVariant::TabLine => 0.0,
                _ => 2.0,
            };
            let w = tw + ins.left.0 + ins.right.0 + eps;
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
        NodeData::Popup { .. } => {
            // popup children stack — max box inside the chrome insets
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
        NodeData::Action { .. } => {
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
