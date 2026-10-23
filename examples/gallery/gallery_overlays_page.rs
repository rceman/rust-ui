impl Gallery {
    fn overlays_page(&self, ui: &mut Ui<Msg>) {
        ui.column(Column::new().gap(Space::Lg).align(Align::Stretch), |ui| {
            ui.named("select.default.open", |ui| {
                Self::overlay_stage(
                    ui,
                    360.0,
                    Insets {
                        top: dp(24.0),
                        ..Default::default()
                    },
                    |ui| {
                        ui.column(Column::new().align(Align::Center), |ui| {
                            let mut select = Select::new("select.default.open.trigger");
                            select.content_id = Some("select.default.open.content");
                            select.open = self.select_open;
                            select.selected = Some(self.env);
                            select.value = Some(match self.env {
                                "development" => "Development",
                                "staging" => "Staging",
                                "preview" => "Preview",
                                "deprecated" => "Deprecated",
                                _ => "Production",
                            });
                            ui.select(
                                select,
                                |event| match event {
                                    SelectChange::Open(open) => {
                                        Msg::SelectOpen("select.default.open", open)
                                    }
                                    SelectChange::Pick(key) => {
                                        Msg::SelectPick("select.default.open", key)
                                    }
                                },
                                |items| {
                                    items.group("environment", "Environment", |items| {
                                        for (key, text, disabled, presented) in [
                                            (
                                                "development",
                                                "Development",
                                                false,
                                                Presented::default(),
                                            ),
                                            ("staging", "Staging", false, Presented::HOVER),
                                            (
                                                "production",
                                                "Production",
                                                false,
                                                Presented::default(),
                                            ),
                                            ("preview", "Preview", false, Presented::default()),
                                            (
                                                "deprecated",
                                                "Deprecated",
                                                true,
                                                Presented::default(),
                                            ),
                                        ] {
                                            items.item(
                                                key,
                                                text,
                                                SelectItemOpts {
                                                    disabled,
                                                    automation_id: if key == "staging" {
                                                        Some("select.default.open.item-highlighted")
                                                    } else {
                                                        None
                                                    },
                                                    presented,
                                                },
                                            );
                                        }
                                    });
                                    items.separator();
                                    items.group("region", "Region", |items| {
                                        items.item("eu-west", "EU West", SelectItemOpts::default());
                                    });
                                },
                            );
                        });
                    },
                );
            });
            ui.box_(
                BoxProps::new()
                    .width(Length::Fixed(dp(1.0)))
                    .height(Length::Fixed(dp(40.0))),
                |_| {},
            );
            let props = DropdownMenu {
                automation_id: "dropdown-menu.default.trigger",
                content_id: Some("dropdown-menu.default.content"),
                label: "Open menu",
                open: self.menu_open,
                width: dp(192.0),
            };
            ui.named("dropdown-menu.default", |ui| {
                Self::overlay_stage(
                    ui,
                    464.0,
                    Insets {
                        top: dp(24.0),
                        left: dp(32.0),
                        ..Default::default()
                    },
                    |ui| {
                        ui.row(Row::new(), |ui| {
                            ui.dropdown_menu(
                                props,
                                |event: DropdownMenuChange| -> Msg {
                                    match event {
                                        DropdownMenuChange::Open(open) => Msg::MenuOpen(open),
                                        DropdownMenuChange::Activate(key) => Msg::MenuAction(key),
                                        DropdownMenuChange::Checkbox(key, value) => {
                                            Msg::MenuCheck(key, value)
                                        }
                                        DropdownMenuChange::Radio(key) => Msg::MenuRadio(key),
                                    }
                                },
                                |menu| {
                                    menu.group("account", "My Account", |items| {
                                        items.item(
                                            "profile",
                                            "Profile",
                                            DropdownItemOpts {
                                                icon: Some(Icon::User),
                                                shortcut: Some("Ctrl+Shift+P"),
                                                ..Default::default()
                                            },
                                        );
                                        items.item(
                                            "settings",
                                            "Settings",
                                            DropdownItemOpts {
                                                icon: Some(Icon::Settings),
                                                shortcut: Some("Ctrl+,"),
                                                ..Default::default()
                                            },
                                        );
                                        items.item(
                                            "team",
                                            "Team members",
                                            DropdownItemOpts {
                                                icon: Some(Icon::User),
                                                automation_id: Some(
                                                    "dropdown-menu.default.item-highlighted",
                                                ),
                                                presented: Presented::HOVER,
                                                ..Default::default()
                                            },
                                        );
                                        items.item(
                                            "billing",
                                            "Billing",
                                            DropdownItemOpts {
                                                icon: Some(Icon::Settings),
                                                disabled: true,
                                                ..Default::default()
                                            },
                                        );
                                    });
                                    menu.separator();
                                    menu.group("appearance", "Appearance", |items| {
                                        items.item(
                                            "toolbar",
                                            "Show toolbar",
                                            DropdownItemOpts {
                                                kind: DropdownItemKind::Checkbox {
                                                    checked: self.menu_toolbar,
                                                },
                                                ..Default::default()
                                            },
                                        );
                                        items.item(
                                            "word-wrap",
                                            "Word wrap",
                                            DropdownItemOpts {
                                                kind: DropdownItemKind::Checkbox {
                                                    checked: self.menu_wrap,
                                                },
                                                ..Default::default()
                                            },
                                        );
                                    });
                                    menu.separator();
                                    menu.group("panel-position", "Panel Position", |items| {
                                        items.item(
                                            "left",
                                            "Panel left",
                                            DropdownItemOpts {
                                                kind: DropdownItemKind::Radio {
                                                    selected: self.menu_panel == "left",
                                                },
                                                ..Default::default()
                                            },
                                        );
                                        items.item(
                                            "right",
                                            "Panel right",
                                            DropdownItemOpts {
                                                kind: DropdownItemKind::Radio {
                                                    selected: self.menu_panel == "right",
                                                },
                                                ..Default::default()
                                            },
                                        );
                                    });
                                    menu.separator();
                                    menu.item(
                                        "delete",
                                        "Delete account",
                                        DropdownItemOpts {
                                            kind: DropdownItemKind::Destructive,
                                            icon: Some(Icon::LogOut),
                                            ..Default::default()
                                        },
                                    );
                                    menu.item(
                                "delete-ws",
                                "Delete workspace",
                                DropdownItemOpts {
                                    kind: DropdownItemKind::Destructive,
                                    presented: Presented::HOVER,
                                    icon: Some(Icon::LogOut),
                                    automation_id: Some(
                                        "dropdown-menu.default.item-destructive-highlighted",
                                    ),
                                    ..Default::default()
                                },
                            );
                                },
                            );
                        });
                    },
                );
            });
        });
}
}
