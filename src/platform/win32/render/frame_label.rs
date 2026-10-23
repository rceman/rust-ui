{
                    // resolved text style: recipe (role fg, body, normal)
                    // + the surgical patch (color_role already folded in)
                    let mut ts = crate::style::label_recipe();
                    ts.patch(patch);
                    // color_role is the recipe fg unless a patch set one
                    if patch.foreground.is_none() {
                        ts.foreground = crate::style::Color::Role(*color_role);
                    }
                    if let Some(ink) = inherited_action_ink(be, id, n) {
                        ts.foreground = crate::style::Color::Role(ink);
                    }
                    unsafe {
                        // framed chrome under the text — h2 rule line,
                        // inline-code surface
                        if let Some(c) = chrome {
                            paint_box(
                                &target,
                                &clip,
                                c,
                                dark,
                                forced,
                                dpi,
                                &self.shadow_cache,
                                self.target_gen.get(),
                            )?;
                        }
                        let ins = crate::layout::node_insets(
                            id,
                            n,
                            &crate::layout::Interaction {
                                dark,
                                ..Default::default()
                            },
                        );
                        let tx = text.as_ref();
                        let draw_max_w = if *wrap {
                            (r.width - ins.left.0 - ins.right.0).max(1.0)
                        } else {
                            f32::MAX
                        };
                        if let Ok(layout) = layout_for(tx, draw_max_w, r.height.max(1.0), &ts) {
                            let ink = inherited_action_ink(be, id, n)
                                .map(crate::style::Color::Role)
                                .unwrap_or(ts.foreground);
                            let mut c = resolve_render(ink, dark, forced);
                            if inherited_action_disabled(be, n) {
                                c.a *= 0.5;
                            }
                            let brush = target.CreateSolidColorBrush(&c, None)?;
                            target.DrawTextLayout(
                                Vector2 {
                                    X: r.x + ins.left.0,
                                    Y: r.y + ins.top.0,
                                },
                                &layout,
                                &brush,
                                D2D1_DRAW_TEXT_OPTIONS_NONE,
                            );
                        }
                    }
}
