impl Gallery {
    /// The frozen `typography` slice — the CORE display recipes, on the
    /// frozen 720dp content column so wrapped headings measure right.
    fn typography_page(&self, ui: &mut Ui<Msg>) {
        // a row wrapper keeps the 720 box at its spec width — a Stretch
        // column would stretch the box itself to the page width
        ui.row(Row::new(), |ui| {
            ui.box_(BoxProps::new().width(Length::Fixed(dp(720.0))), |ui| {
                ui.column(
                    // no inner padding — the box is the frozen 720 content
                    // column; elements span it edge-to-edge
                    Column::new()
                        .gap(Space::Lg)
                        .padding(Space::None)
                        .align(Align::Stretch),
                |ui| {
                    ui.named("typography.h1", |ui| {
                        ui.typography(Typography::H1, "Taxing Laughter: The Joke Tax Chronicles");
                    });
                    ui.named("typography.h2", |ui| {
                        ui.typography(Typography::H2, "The People of the Kingdom");
                    });
                    ui.named("typography.h3", |ui| {
                        ui.typography(Typography::H3, "The Joke Tax");
                    });
                    ui.named("typography.h4", |ui| {
                        ui.typography(Typography::H4, "People stopped telling jokes");
                    });
                    ui.named("typography.p", |ui| {
                        ui.typography(
                            Typography::Paragraph,
                            "The king, seeing how much happier his subjects were, realized the error of his ways and repealed the joke tax.",
                        );
                    });
                    ui.named("typography.lead", |ui| {
                        ui.typography(
                            Typography::Lead,
                            "A modal dialog that interrupts the user with important content and expects a response.",
                        );
                    });
                    // small is a content-hugging label like inline-code —
                    // a row wrapper keeps it at content width inside the
                    // Stretch column (N2)
                    ui.row(Row::new(), |ui| {
                        ui.typography(Typography::Small, "Email address")
                            .automation_id("typography.small");
                    });
                    ui.named("typography.muted", |ui| {
                        ui.typography(Typography::Muted, "Enter your email address.");
                    });
                    // inline-code is a content-hugging box — a row wrapper
                    // keeps it at Content width inside the Stretch column
                    ui.row(Row::new(), |ui| {
                        ui.typography(Typography::InlineCode, "npx shadcn@latest init")
                            .automation_id("typography.inline-code");
                    });
                    ui.named("typography.code-block", |ui| {
                        ui.code_block(&[
                            "fn render_button(rect: Rect) {",
                            "ctx.fill(rect, theme.primary)",
                            "}",
                        ]);
                    });
                    },
                );
            });
        });
}
}
