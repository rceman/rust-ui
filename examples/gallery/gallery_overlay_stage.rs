impl Gallery {
    /// Frozen `.overlay-stage` panel chrome — `rounded-xl` (r14)
    /// `bg-muted/40` panel with a 1dp `foreground/10` ring and fixed
    /// demo height; the popup itself escapes into the deferred overlay
    /// layer, only the panel bounds the specimen.
    fn overlay_stage(
        ui: &mut Ui<'_, '_, Msg>,
        height: f32,
        padding: Insets,
        draw: impl FnOnce(&mut Ui<'_, '_, Msg>),
    ) {
        let style = BoxStyle::new()
            .background(Color::Role(ColorRole::Muted40))
            .border(Border::all(BorderSide::new(
                dp(1.0),
                Color::Role(ColorRole::Foreground10),
            )))
            .radii(CornerRadii::all(dp(14.0)))
            .padding(padding);
        ui.box_(
            BoxProps::new()
                .style(style)
                .width(Length::Fill(1))
                .height(Length::Fixed(dp(height))),
            draw,
        );
}
}
