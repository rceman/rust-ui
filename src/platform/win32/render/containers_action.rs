{
                    let bs = style.resolve(
                        *disabled,
                        be.resolved_pressed(id, n),
                        be.resolved_hot(id, n),
                        be.resolved_focus_visible(id, n),
                    );
                    unsafe {
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
                            &clip,
                            &bs,
                            dark,
                            forced,
                            dpi,
                            &self.shadow_cache,
                            self.target_gen.get(),
                        )?;
                        // focus ENFORCEMENT — the ring lives at paint, a
                        // layer consumer patches can never erase
                        if be.resolved_focus_visible(id, n) && !*disabled {
                            match n.role {
                                Some(
                                    crate::node::SemanticRole::MenuItem
                                    | crate::node::SemanticRole::MenuItemCheckbox
                                    | crate::node::SemanticRole::MenuItemRadio
                                    | crate::node::SemanticRole::MenuItemDestructive,
                                ) => {}
                                Some(crate::node::SemanticRole::SettingsNavItem) => {
                                    paint_ring_color_width(
                                        &target,
                                        &clip,
                                        &bs.radii,
                                        crate::style::Color::Role(
                                            crate::theme::ColorRole::SidebarRing,
                                        ),
                                        2.0,
                                        dark,
                                        forced,
                                    )?;
                                }
                                _ => paint_focus_ring(&target, &clip, &bs.radii, dark, forced)?,
                            }
                        }
                    }
}
