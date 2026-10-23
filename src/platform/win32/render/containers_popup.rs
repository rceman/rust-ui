{
                    // modal: the popup's own rect is the full-client
                    // backdrop — dim scrim fills it; chrome + children
                    // paint at the resolved content box (the children
                    // were walked at content-minus-insets, so the frame
                    // rect is the first child's rect inflated by insets)
                    let bs = props
                        .resolved_box(crate::node::KIND_POPUP)
                        .unwrap_or_default();
                    let frame = if *modal {
                        n.children
                            .first()
                            .map(|s| NodeId {
                                slot: *s,
                                generation: be.rt.arena.generation_of(*s),
                            })
                            .and_then(|cid| be.rects.get(&cid).copied())
                            .map(|ir| {
                                let ins = crate::layout::node_insets(
                                    id,
                                    n,
                                    &crate::layout::Interaction {
                                        dark,
                                        ..Default::default()
                                    },
                                );
                                LogicalRect {
                                    x: ir.x - ins.left.0,
                                    y: ir.y - ins.top.0,
                                    width: ir.width + ins.left.0 + ins.right.0,
                                    height: ir.height + ins.top.0 + ins.bottom.0,
                                }
                            })
                            .unwrap_or(r)
                    } else {
                        r
                    };
                    let frame_clip = D2D_RECT_F {
                        left: frame.x,
                        top: frame.y,
                        right: frame.x + frame.width,
                        bottom: frame.y + frame.height,
                    };
                    unsafe {
                        if *modal {
                            let scrim = crate::style::Color::rgba(0, 0, 0, 26);
                            let c = resolve_render(scrim, dark, forced);
                            let b = target.CreateSolidColorBrush(&c, None)?;
                            target.FillRectangle(&clip, &b);
                        }
                        paint_box(
                            &target,
                            &frame_clip,
                            &bs,
                            dark,
                            forced,
                            dpi,
                            &self.shadow_cache,
                            self.target_gen.get(),
                        )?;
                    }
}
