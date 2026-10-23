{
                    let ink = inherited_action_ink(be, id, n)
                        .map(crate::style::Color::Role)
                        .unwrap_or(*color);
                    let mut c = resolve_render(ink, dark, forced);
                    if inherited_action_disabled(be, n) {
                        c.a *= 0.5;
                    }
                    let brush = unsafe { target.CreateSolidColorBrush(&c, None)? };
                    unsafe {
                        draw_icon(&target, &self.factory, icon, *size, &r, &brush)?;
                    }
}
