    }

    /// The frozen `forms` slice — contract order: label → input →
    /// textarea → checkbox → radio-group → switch → field.
    fn forms_page(&self, ui: &mut Ui<Msg>) {
        let _ = include!("gallery_forms_a.rs");
        let _ = include!("gallery_forms_b.rs");
    }
