{
                    let state = crate::style::StyleState::classify(
                        *disabled,
                        be.resolved_pressed(id, n),
                        be.resolved_hot(id, n),
                    );
                    let vs = crate::style::resolve_toggle_visual(
                        *kind,
                        *size,
                        *checked,
                        *invalid,
                        state,
                        be.resolved_focus_visible(id, n),
                        dark,
                    );
                    let r_dp = crate::geom::Rect {
                        x: r.x,
                        y: r.y,
                        width: r.width,
                        height: r.height,
                    };
                    unsafe {
                        // same disabled alpha-fade as Button — RAII so a
                        // `?` below can never strand the 0.5 layer
                        let _layer_guard = (*disabled).then(|| {
                            // contentBounds must be unbounded — a
                            // Default (0,0,0,0) bounds clips every draw
                            // inside the layer to nothing
                            target.PushLayer(
                                &D2D1_LAYER_PARAMETERS {
                                    contentBounds: D2D_RECT_F {
                                        left: f32::MIN,
                                        top: f32::MIN,
                                        right: f32::MAX,
                                        bottom: f32::MAX,
                                    },
                                    opacity: 0.5,
                                    ..Default::default()
                                },
                                None,
                            );
                            LayerGuard(&target)
                        });
                        paint_box(
                            &target,
                            &clip,
                            &vs.box_style,
                            dark,
                            forced,
                            dpi,
                            &self.shadow_cache,
                            self.target_gen.get(),
                        )?;
                        let mark = resolve_render(
                            crate::style::Color::Role(crate::theme::ColorRole::PrimaryForeground),
                            dark,
                            forced,
                        );
                        let mark_brush = target.CreateSolidColorBrush(&mark, None)?;
                        match kind {
                            crate::theme::ToggleKind::Checkbox => {
                                if *checked {
                                    let ir = crate::layout::toggle_checkbox_indicator_rect(&r_dp);
                                    draw_icon_at(
                                        &target,
                                        &self.factory,
                                        &crate::icons::Icon::Check,
                                        &ir,
                                        &mark_brush,
                                    )?;
                                }
                            }
                            crate::theme::ToggleKind::RadioItem => {
                                if *checked {
                                    let dr = crate::layout::toggle_radio_dot_rect(&r_dp);
                                    target.FillEllipse(
                                        &D2D1_ELLIPSE {
                                            point: Vector2 {
                                                X: dr.x + dr.width / 2.0,
                                                Y: dr.y + dr.height / 2.0,
                                            },
                                            radiusX: dr.width / 2.0,
                                            radiusY: dr.height / 2.0,
                                        },
                                        &mark_brush,
                                    );
                                }
                            }
                            crate::theme::ToggleKind::Switch => {
                                let tr = crate::layout::toggle_thumb_rect(*size, *checked, &r_dp);
                                let white = resolve_render(
                                    crate::style::Color::rgba(255, 255, 255, 255),
                                    dark,
                                    forced,
                                );
                                let wb = target.CreateSolidColorBrush(&white, None)?;
                                target.FillEllipse(
                                    &D2D1_ELLIPSE {
                                        point: Vector2 {
                                            X: tr.x + tr.width / 2.0,
                                            Y: tr.y + tr.height / 2.0,
                                        },
                                        radiusX: tr.width / 2.0,
                                        radiusY: tr.height / 2.0,
                                    },
                                    &wb,
                                );
                            }
                        }
                        // focus-visible OR unconditional invalid ring —
                        // destructive tint when invalid
                        if (be.resolved_focus_visible(id, n) || *invalid) && !*disabled {
                            paint_ring_color(
                                &target,
                                &clip,
                                &vs.box_style.radii,
                                vs.focus_ring,
                                dark,
                                forced,
                            )?;
                        }
                        if let Some(lbl) = label {
                            if let Ok(lay) =
                                layout_for(lbl, f32::MAX, r.height.max(1.0), &vs.text_style)
                            {
                                let fg = resolve_render(vs.text_style.foreground, dark, forced);
                                let fgb = target.CreateSolidColorBrush(&fg, None)?;
                                target.DrawTextLayout(
                                    Vector2 {
                                        X: r.x + 24.0,
                                        Y: r.y + 1.0,
                                    },
                                    &lay,
                                    &fgb,
                                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                                );
                            }
                        }
                    }
}
