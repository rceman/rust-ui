impl Gallery {
    /// Gallery chrome — the navbar (page tabs) + theme toggle. The
    /// frozen shell is the reference layout; the tabs drive the page.
    fn header(&self, ui: &mut Ui<Msg>) {
        ui.named("shell.header", |ui| {
            ui.row(Row::new().gap(Space::Sm).align(Align::Center), |ui| {
                ui.named("shell.nav", |ui| {
                    ui.row(Row::new().gap(Space::Xs).align(Align::Center), |ui| {
                        for p in Page::ALL {
                            ui.button(p.label())
                                .automation_id(&format!("shell.nav.{}", p.id()))
                                .variant(if self.page == p {
                                    ButtonVariant::Secondary
                                } else {
                                    ButtonVariant::Ghost
                                })
                                .size(ControlSize::Xs)
                                .disabled(!p.implemented())
                                .on_press(move || Msg::SetPage(p));
                        }
                    });
                });
                // theme toggle — icon-only buttons, real native names
                ui.named("shell.theme", |ui| {
                    ui.row(Row::new().gap(Space::Xs).align(Align::Center), |ui| {
                        let mut theme_btn =
                            |ui: &mut Ui<Msg>,
                             id: &'static str,
                             mode: ThemeMode,
                             icon: Icon,
                             name: &'static str| {
                                ui.icon_button(icon, name)
                                    .automation_id(id)
                                    .variant(if self.theme == mode {
                                        ButtonVariant::Secondary
                                    } else {
                                        ButtonVariant::Outline
                                    })
                                    .size(ControlSize::Xs)
                                    .on_press(move || Msg::SetTheme(mode));
                            };
                        // frozen shell.theme.* recorded content is the
                        // bare mode name ("Light"/"Dark"/"System")
                        theme_btn(
                            ui,
                            "shell.theme.light",
                            ThemeMode::Light,
                            Icon::Sun,
                            "Light",
                        );
                        theme_btn(ui, "shell.theme.dark", ThemeMode::Dark, Icon::Moon, "Dark");
                        theme_btn(
                            ui,
                            "shell.theme.system",
                            ThemeMode::System,
                            Icon::Monitor,
                            "System",
                        );
                    });
                });
            });
        });
}
}
