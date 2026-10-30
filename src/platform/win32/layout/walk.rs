use super::flex::*;
use super::natural::*;
use super::*;

impl LayoutCache {
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
        deferred: &mut Vec<NodeId>,
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
        // popups defer to the end-of-run placement pass — BEFORE the clip
        // insert so a scroll-viewport ancestor can never confine a popup
        if matches!(n.data, NodeData::Popup { .. }) {
            deferred.push(id);
            return Ok(());
        }
        // a scroll viewport ancestor confines this node: paint, hit-test
        // and peer geometry all consume the same effective clip
        if let Some(c) = clip {
            clips.insert(id, c);
        }
        // authored margins adjust the ALLOCATED slot — the parent's
        // sizing already consumed this node's share; negative margins
        // grow the border box past it (the frozen `-mx-1` breakout)
        let spec = n.layout;
        let rect = if spec.margin_x.0 != 0.0 || spec.margin_y.0 != 0.0 {
            LogicalRect {
                x: rect.x + spec.margin_x.0,
                y: rect.y + spec.margin_y.0,
                width: (rect.width - 2.0 * spec.margin_x.0).max(0.0),
                height: (rect.height - 2.0 * spec.margin_y.0).max(0.0),
            }
        } else {
            rect
        };
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
        let mut measurer = crate::platform::win32::render::measure_fn();
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
                    let (nw, nh) = natural::natural(rt, ctx, cid, inner.width, &mut measurer);
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
                        deferred,
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
                        let (_, nh) = natural::natural(rt, ctx, cid, inner.width, &mut measurer);
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
                    let (nw, _) = natural::natural(rt, ctx, it.id, inner.width, &mut measurer);
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
                        deferred,
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
                        let (nw, _) = natural::natural(rt, ctx, cid, inner.width, &mut measurer);
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
                    let (_, nh) = natural::natural(rt, ctx, it.id, inner.width, &mut measurer);
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
                        deferred,
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
                    self.walk(
                        rt, ctx, cid, inner, rects, order, clips, deferred, clip, kind,
                    )?;
                }
            }
        }
        Ok(())
    }
}
