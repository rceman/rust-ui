impl Gallery {
    pub fn view(&self, ui: &mut Ui<Msg>) {
        ui.theme(Theme::resolve(self.theme, ui.appearance()));
        ui.named("shell.root", |ui| {
            ui.column(
                Column::new()
                    .gap(Space::Md)
                    .padding(Space::Lg)
                    .align(Align::Stretch)
                    // the root column fills the client: the page
                    // scroll viewport inside gets the column's
                    // remaining space after the header (a Content
                    // column would cap it at natural content height —
                    // zero scroll extent and dead space below)
                    .height(Length::Fill(1)),
                |ui| {
                    self.header(ui);
                    // the page area scrolls inside its viewport — the
                    // offset is Gallery state dispatched back by the
                    // scroll node's on_scroll factory
                    ui.named("page.scroll", |ui| {
                        ui.scroll(
                            // padding None — the viewport must not inset
                            // the page: zero-offset geometry is the
                            // frozen slice-1 layout
                            Scroll::new().padding(Space::None),
                            dp(self.scroll),
                            |o| Msg::Scroll(o.y),
                            |ui| match self.page {
                                Page::Components => self.components_page(ui),
                                Page::Typography => self.typography_page(ui),
                                Page::Forms => self.forms_page(ui),
                                Page::Navigation => self.navigation_page(ui),
                                Page::Overlays => self.overlays_page(ui),
                                Page::NativeText => self.native_text_page(ui),
                                other => self.stub_page(ui, other),
                            },
                        );
                    });
                },
            );
        });
}
}
