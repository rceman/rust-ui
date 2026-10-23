impl Gallery {
    /// The frozen `native-text` slice — single-line and multiline editors
    /// under the same input/textarea stack chrome.
    fn native_text_page(&self, ui: &mut Ui<Msg>) {
        ui.column(Column::new().gap(Space::Lg).align(Align::Start), |ui| {
            // frozen hints: "Placeholder text" on both placeholder
            // specimens; the disabled pair mounts EMPTY with "Disabled
            // input" / "Disabled textarea" — the muted hint is the
            // recorded text
            for (i, (id, ph, dis, ro, inv, h)) in [
                (
                    "native-text.single-line.placeholder",
                    Some("Placeholder text"),
                    false,
                    false,
                    false,
                    32.0,
                ),
                (
                    "native-text.single-line.filled",
                    None,
                    false,
                    false,
                    false,
                    32.0,
                ),
                (
                    "native-text.single-line.focused",
                    None,
                    false,
                    false,
                    false,
                    32.0,
                ),
                (
                    "native-text.single-line.selection",
                    None,
                    false,
                    false,
                    false,
                    32.0,
                ),
                (
                    "native-text.single-line.disabled",
                    Some("Disabled input"),
                    true,
                    false,
                    false,
                    32.0,
                ),
                (
                    "native-text.single-line.readonly",
                    None,
                    false,
                    true,
                    false,
                    32.0,
                ),
                (
                    "native-text.single-line.invalid",
                    None,
                    false,
                    false,
                    true,
                    32.0,
                ),
                (
                    "native-text.multiline.placeholder",
                    Some("Placeholder text"),
                    false,
                    false,
                    false,
                    64.0,
                ),
                (
                    "native-text.multiline.filled",
                    None,
                    false,
                    false,
                    false,
                    78.0,
                ),
                (
                    "native-text.multiline.focused",
                    None,
                    false,
                    false,
                    false,
                    64.0,
                ),
                (
                    "native-text.multiline.selection",
                    None,
                    false,
                    false,
                    false,
                    78.0,
                ),
                (
                    "native-text.multiline.disabled",
                    Some("Disabled textarea"),
                    true,
                    false,
                    false,
                    64.0,
                ),
                (
                    "native-text.multiline.readonly",
                    None,
                    false,
                    true,
                    false,
                    64.0,
                ),
                (
                    "native-text.multiline.invalid",
                    None,
                    false,
                    false,
                    true,
                    64.0,
                ),
            ]
            .iter()
            .enumerate()
            {
                let idx = T_NATIVE + i;
                let b = if *h > 40.0 {
                    ui.textarea(&self.texts[idx])
                } else {
                    ui.input(&self.texts[idx])
                };
                let mut b = b
                    .automation_id(id)
                    .width(Length::Fixed(dp(360.0)))
                    .height(Length::Fixed(dp(*h)))
                    .line_height(dp(20.0));
                if let Some(p) = *ph {
                    b = b.placeholder(p);
                }
                // frozen `focused`/`selection` cells show the focus-visible
                // ring; selection cells stage their UTF-16 range on the
                // peer — no real focus required
                if id.ends_with(".focused") || id.ends_with(".selection") {
                    b = b.presented(Presented::FOCUS_VISIBLE);
                }
                match *id {
                    "native-text.single-line.selection" => {
                        // "Select" in "Select this phrase"
                        b = b.selection(0, 6);
                    }
                    "native-text.multiline.selection" => {
                        // "Second line"
                        b = b.selection(22, 33);
                    }
                    _ => {}
                }
                b.disabled(*dis)
                    .read_only(*ro)
                    .invalid(*inv)
                    .on_edit(move |e| Msg::Edit(idx, e))
                    .on_conflict(move |c| Msg::Conflict(idx, c));
            }
        });
}
}
