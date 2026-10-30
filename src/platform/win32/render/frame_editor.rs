{
                    // authored chrome paints the frame (the peer draws its
                    // own text/selection inside the safety inset)
                    let mut chrome = super::super::layout::editor_chrome(patch);
                    crate::style::os_enforce_box(&mut chrome, forced);
                    unsafe {
                        paint_box(
                            &target,
                            &clip,
                            &chrome,
                            dark,
                            forced,
                            dpi,
                            &self.shadow_cache,
                            self.target_gen.get(),
                        )?;
                        // frameless chrome => a STACKED input: the sibling
                        // overlay is a transparent shell, so the EDITOR
                        // paints the element border — runtime focus/
                        // invalid resolution only exists here, never in
                        // the overlay's stage-baked style.
                        if !(chrome.border.any()
                            || chrome.radii != crate::style::CornerRadii::default())
                        {
                            let border_color = if *invalid {
                                crate::style::Color::Role(crate::theme::ColorRole::InputDestructive)
                            } else if *disabled {
                                crate::style::Color::Role(
                                    crate::theme::ColorRole::InputDisabledBorder,
                                )
                            } else if be.resolved_focus_visible(id, n) {
                                crate::style::Color::Role(crate::theme::ColorRole::Ring)
                            } else {
                                crate::style::Color::Role(crate::theme::ColorRole::Input)
                            };
                            let frame = crate::style::BoxStyle {
                                background: crate::style::Color::rgba(0, 0, 0, 0),
                                border: crate::style::Border::all(
                                    crate::style::BorderSide::new(
                                        crate::geom::Dp(1.0),
                                        border_color,
                                    ),
                                ),
                                radii: crate::style::CornerRadii::all(crate::geom::Dp(10.0)),
                                padding: crate::style::Insets::default(),
                                shadow: None,
                            };
                            paint_box(
                                &target,
                                &clip,
                                &frame,
                                dark,
                                forced,
                                dpi,
                                &self.shadow_cache,
                                self.target_gen.get(),
                            )?;
                        }
                        // focus ENFORCEMENT — same independent ring layer;
                        // `invalid` fires the destructive ring
                        // unconditionally (frozen `aria-invalid` rule)
                        if (be.resolved_focus_visible(id, n) || *invalid) && !*disabled {
                            // the stack's leaf chrome is frameless — the
                            // sibling overlay owns the element frame, so
                            // the ring follows the OVERLAY radius (10dp);
                            // a bare editor's own chrome is the frame
                            let ring_radii = if chrome.border.any()
                                || chrome.radii != crate::style::CornerRadii::default()
                            {
                                chrome.radii
                            } else {
                                crate::style::input_overlay_style(*disabled).radii
                            };
                            paint_ring_color(
                                &target,
                                &clip,
                                &ring_radii,
                                crate::style::Color::Role(crate::layout::ring_color_role(*invalid)),
                                dark,
                                forced,
                            )?;
                        }
                    }
                    // The peer composites through its own scratch
                    // surface and `DrawBitmap`s into the frame — a
                    // normal draw op, so the pushed scroll clip DOES
                    // confine it (unlike `TxDrawD2D` into the shared
                    // target, which a pushed clip suppresses — the clip
                    // guard stays pushed).
                    if clip.right > clip.left
                        && clip.bottom > clip.top
                        && let Some(peer) = be.peer_for(id)
                    {
                        // `editor_content_rect` is THE pill→content
                        // transform — shared with layout/pointer/caret.
                        let c = super::super::layout::editor_content_rect(r, &chrome);
                        // fully clipped by the scroll viewport → nothing
                        // to blit; the peer's bounds still track the full
                        // rect so hit/caret math stays document-true
                        let clipped_out = be.clips.get(&id).map_or(false, |cl| !c.intersects(*cl));
                        if !clipped_out {
                            // `disabled:opacity-50` — the frozen rule is
                            // whole-control alpha, text included
                            peer.borrow().draw(
                                &target,
                                c,
                                self.target_gen.get(),
                                if *disabled { 0.5 } else { 1.0 },
                            )?;
                        }
                    }
                    // placeholder — staged hint text drawn muted over the
                    // (empty) peer surface; matches the peer's 14dp font
                    if snapshot.committed.is_empty()
                        && let Some(ph) = placeholder
                    {
                        let c = super::super::layout::editor_content_rect(r, &chrome);
                        if c.width > 1.0 && c.height > 1.0 {
                            let mut ts = crate::style::TextStyle::new();
                            ts.foreground =
                                crate::style::Color::Role(crate::theme::ColorRole::MutedForeground);
                            ts.size = crate::style::TextSize::Exact(crate::geom::Dp(14.0));
                            if let Ok(layout) = layout_for(ph, c.width, c.height, &ts) {
                                let mut col = resolve_render(ts.foreground, dark, forced);
                                // `disabled:opacity-50` — whole control
                                if *disabled {
                                    col.a *= 0.5;
                                }
                                unsafe {
                                    let brush = target.CreateSolidColorBrush(&col, None)?;
                                    target.DrawTextLayout(
                                        Vector2 { X: c.x, Y: c.y },
                                        &layout,
                                        &brush,
                                        D2D1_DRAW_TEXT_OPTIONS_NONE,
                                    );
                                }
                            }
                        }
                    }
}
