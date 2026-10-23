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
                                crate::style::input_overlay_style(*disabled, *invalid).radii
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
                            peer.borrow().draw(&target, c, self.target_gen.get())?;
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
                                let col = resolve_render(ts.foreground, dark, forced);
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
