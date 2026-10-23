{
                    // deterministic chain: recipe -> consumer patch -> state
                    let state = crate::style::StyleState::classify(
                        *disabled,
                        be.resolved_pressed(id, n),
                        be.resolved_hot(id, n),
                    );
                    let vs = crate::style::resolve_button(
                        *variant,
                        *size,
                        style,
                        state,
                        be.resolved_focus_visible(id, n),
                        dark,
                        *selected,
                        *invalid,
                    );
                    let bs = &vs.box_style;
                    let pressed = be.resolved_pressed(id, n);
                    let ins = crate::layout::button_content_insets(&n.data, &vs);
                    // active:translate-y-px — the whole control shifts 1dp
                    let dy = if pressed && !*disabled { 1.0 } else { 0.0 };
                    let br = D2D_RECT_F {
                        left: r.x,
                        top: r.y + dy,
                        right: r.x + r.width,
                        bottom: r.y + r.height + dy,
                    };
                    unsafe {
                        // disabled:opacity-50 — the whole control alpha-
                        // fades; the layer wraps chrome + content. RAII:
                        // a `?` below must not strand the pushed layer on
                        // the shared target for the rest of the frame
                        let _layer_guard = (*disabled).then(|| {
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
                            &br,
                            bs,
                            dark,
                            forced,
                            dpi,
                            &self.shadow_cache,
                            self.target_gen.get(),
                        )?;
                        // focus ring — the RESOLVED style's ring (3dp
                        // ring/50 band outside the border); enforcement
                        // supplies it for Focus-role colors too. The
                        // invalid state carries the same 3dp band in
                        // ring-destructive and does NOT require focus.
                        if (be.resolved_focus_visible(id, n) || *invalid) && !*disabled {
                            paint_ring_color(&target, &br, &bs.radii, vs.focus_ring, dark, forced)?;
                        }
                        let fg = resolve_render(vs.text_style.foreground, dark, forced);
                        let fg_brush = target.CreateSolidColorBrush(&fg, None)?;
                        // icon tint — `icon_color` overrides the text
                        // foreground (the select chevron stays muted
                        // against a filled value)
                        let icon_brush = match icon_color {
                            Some(c) => target
                                .CreateSolidColorBrush(&resolve_render(*c, dark, forced), None)?,
                            None => fg_brush.clone(),
                        };
                        // icon content — the part rect is shared geometry
                        // with measure/devtools
                        let rect_dp = crate::geom::Rect {
                            x: r.x,
                            y: r.y + dy,
                            width: r.width,
                            height: r.height,
                        };
                        for (edge, icon) in [(false, *icon_start), (true, *icon_end)] {
                            if let Some(icon) = icon
                                && let Some(ir) =
                                    crate::layout::button_icon_rect(&n.data, &vs, &rect_dp, edge)
                            {
                                draw_icon_at(&target, &self.factory, &icon, &ir, &icon_brush)?;
                            }
                        }
                        if *icon_only
                            && let Some(icon) = icon_start.or(*icon_end)
                            && let Some(ir) =
                                crate::layout::button_icon_rect(&n.data, &vs, &rect_dp, false)
                        {
                            draw_icon_at(&target, &self.factory, &icon, &ir, &icon_brush)?;
                        }
                        if !*icon_only {
                            let wide: Vec<u16> = text.as_ref().encode_utf16().collect();
                            let lay = layout_for(
                                text.as_ref(),
                                (br.right - br.left - ins.left.0 - ins.right.0).max(1.0),
                                (br.bottom - br.top).max(1.0),
                                &vs.text_style,
                            )?;
                            // select-family controls are `text-left`
                            // (justify-between value + indicator);
                            // button labels center
                            lay.SetTextAlignment(match variant {
                                crate::theme::ButtonVariant::Select
                                | crate::theme::ButtonVariant::SelectItem => {
                                    DWRITE_TEXT_ALIGNMENT_LEADING
                                }
                                _ => DWRITE_TEXT_ALIGNMENT_CENTER,
                            })?;
                            lay.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
                            // whitespace-nowrap — button labels are always
                            // single-line; a tight box must never wrap
                            lay.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
                            target.DrawTextLayout(
                                Vector2 {
                                    X: br.left + ins.left.0,
                                    Y: br.top,
                                },
                                &lay,
                                &fg_brush,
                                D2D1_DRAW_TEXT_OPTIONS_NONE,
                            );
                        }
                        // line-variant active tab — the frozen `::after`
                        // underline (bottom-[-5px] h-0.5 bg-foreground)
                        if *selected && *variant == crate::theme::ButtonVariant::TabLine {
                            let ur = crate::layout::tab_underline_rect(&rect_dp);
                            let ub = target.CreateSolidColorBrush(
                                &resolve_render(
                                    crate::style::Color::Role(crate::theme::ColorRole::Foreground),
                                    dark,
                                    forced,
                                ),
                                None,
                            )?;
                            target.FillRectangle(
                                &D2D_RECT_F {
                                    left: ur.x,
                                    top: ur.y,
                                    right: ur.x + ur.width,
                                    bottom: ur.y + ur.height,
                                },
                                &ub,
                            );
                        }
                    }
}
