    }

    /// A placeholder for the frozen shell tabs that aren't part of this
    /// slice — the nav ids exist, the page is intentionally thin.
    fn stub_page(&self, ui: &mut Ui<Msg>, page: Page) {
        ui.typography(
            Typography::Muted,
            &format!("{} — not part of slice 2.", page.label()),
        );
