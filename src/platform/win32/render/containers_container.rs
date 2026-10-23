{
                    // painted box containers resolve to a BoxStyle and go
                    // through the shared painter — surface = recipe + patch,
                    // box_ = authored full style
                    if let Some(bs) = props.resolved_box(*kind) {
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
                    }
}
