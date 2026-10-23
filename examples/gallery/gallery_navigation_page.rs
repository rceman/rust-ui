impl Gallery {
    fn navigation_page(&self, ui: &mut Ui<Msg>) {
        ui.column(Column::new().gap(Space::Lg).align(Align::Start), |ui| {
            for (line, variant) in [(false, TabsVariant::Default), (true, TabsVariant::Line)] {
                let prefix = if line { "tabs.line" } else { "tabs.default" };
                for (state, target, selected, opts) in [
                    ("active", "account", "account", TabOpts::default()),
                    ("inactive", "account", "", TabOpts::default()),
                    (
                        "hover",
                        "password",
                        "account",
                        TabOpts {
                            presented: Presented::HOVER,
                            ..Default::default()
                        },
                    ),
                    (
                        "focus-visible",
                        "password",
                        "account",
                        TabOpts {
                            presented: Presented::FOCUS_VISIBLE,
                            ..Default::default()
                        },
                    ),
                    (
                        "disabled",
                        "password",
                        "account",
                        TabOpts {
                            disabled: true,
                            ..Default::default()
                        },
                    ),
                ] {
                    let id = format!("{prefix}.{state}");
                    let selected = if state == "active" {
                        if line {
                            self.tabs_line
                        } else {
                            self.tabs_default
                        }
                    } else {
                        selected
                    };
                    ui.named(&id, |ui| {
                        ui.tabs(
                            Tabs { variant, selected },
                            move |key| Msg::SetTab(line, key),
                            |tabs| {
                                for (key, label) in [
                                    ("account", "Account"),
                                    ("password", "Password"),
                                    ("settings", "Settings"),
                                ] {
                                    let mut item = if key == target {
                                        opts
                                    } else {
                                        TabOpts::default()
                                    };
                                    tabs.tab(key, label, item);
                                }
                            },
                        );
                    });
                }
            }

            ui.row(Row::new().gap(Space::None).align(Align::Start), |ui| {
                ui.settings_nav(
                    SettingsNav {
                        automation_id: "settings-nav.default",
                        collapsed: self.nav_collapsed,
                        selected: self.nav_selected,
                    },
                    Msg::SetNav,
                    |nav| {
                        nav.group("workspace", "Workspace", |items| {
                            items.item(
                                "general",
                                "General",
                                Icon::Settings,
                                SettingsNavItemOpts::default(),
                            );
                            items.item(
                                "appearance",
                                "Appearance",
                                Icon::Palette,
                                SettingsNavItemOpts::default(),
                            );
                        });
                        nav.group("assistant", "Assistant", |items| {
                            items.item("agents", "Agents", Icon::Bot, Default::default());
                            items.item("voice", "Voice", Icon::Mic, Default::default());
                        });
                        nav.group("system", "System", |items| {
                            items.item(
                                "integrations",
                                "Integrations",
                                Icon::Plug,
                                Default::default(),
                            );
                            items.item(
                                "advanced",
                                "Advanced",
                                Icon::SlidersHorizontal,
                                Default::default(),
                            );
                        });
                    },
                );
                ui.box_(
                    BoxProps::new()
                        .width(Length::Fill(1))
                        .height(Length::Fixed(dp(if self.nav_collapsed {
                            270.0
                        } else {
                            380.0
                        })))
                        .style(BoxStyle {
                            background: Color::Role(ColorRole::Background),
                            padding: Insets::all(dp(16.0)),
                            ..Default::default()
                        }),
                    |ui| {
                        ui.column(
                            Column::new()
                                .align(Align::Stretch)
                                .padding(Space::None)
                                .gap(Space::Xs),
                            |ui| {
                                ui.part("header", |ui| {
                                    ui.column(
                                        Column::new()
                                            .padding(Space::None)
                                            .gap(Space::Xs)
                                            .align(Align::Start),
                                        |ui| {
                                            ui.text("Appearance").style(TextStyle {
                                                size: TextSize::Exact(dp(16.0)),
                                                weight: TextWeight::Semibold,
                                                line_height: Some(dp(24.0)),
                                                ..Default::default()
                                            });
                                            ui.text("Customize how the app looks.").style(
                                                TextStyle {
                                                    foreground: Color::Role(
                                                        ColorRole::MutedForeground,
                                                    ),
                                                    ..Default::default()
                                                },
                                            );
                                        },
                                    );
                                });
                                ui.separator(Separator::horizontal());
                                ui.field(
                                    Field {
                                        orientation: FieldOrientation::Horizontal,
                                        width: None,
                                    },
                                    |ui| {
                                        ui.part("content", |ui| {
                                            ui.column(
                                                Column::new()
                                                    .padding(Space::None)
                                                    .gap(Space::Xs)
                                                    .align(Align::Start)
                                                    .width(Length::Fill(1)),
                                                |ui| {
                                                    ui.text("Theme")
                                                        .style(field_label_style(false));
                                                    ui.text("Select the interface theme.")
                                                        .style(field_description_style());
                                                },
                                            );
                                        });
                                        let mut select = Select::new("settings-nav.theme");
                                        select.content_id = Some("settings-nav.theme.content");
                                        select.open = self.settings_theme_open;
                                        select.value = Some(match self.settings_theme {
                                            "system" => "System",
                                            "light" => "Light",
                                            _ => "Dark",
                                        });
                                        select.selected = Some(self.settings_theme);
                                        ui.select(
                                            select,
                                            |event| match event {
                                                SelectChange::Open(open) => {
                                                    Msg::SelectOpen("settings-nav.theme", open)
                                                }
                                                SelectChange::Pick(key) => {
                                                    Msg::SelectPick("settings-nav.theme", key)
                                                }
                                            },
                                            |items| {
                                                items.item(
                                                    "system",
                                                    "System",
                                                    SelectItemOpts::default(),
                                                );
                                                items.item(
                                                    "light",
                                                    "Light",
                                                    SelectItemOpts::default(),
                                                );
                                                items.item(
                                                    "dark",
                                                    "Dark",
                                                    SelectItemOpts::default(),
                                                );
                                            },
                                        );
                                    },
                                );
                                ui.separator(Separator::horizontal());
                                for (idx, label, desc) in [
                                    (6, "Animations", "UI transitions and motion."),
                                    (7, "Compact mode", "Reduce padding."),
                                ] {
                                    ui.field(
                                        Field {
                                            orientation: FieldOrientation::Horizontal,
                                            width: None,
                                        },
                                        |ui| {
                                            ui.part("content", |ui| {
                                                ui.column(
                                                    Column::new()
                                                        .padding(Space::None)
                                                        .gap(Space::Xs)
                                                        .align(Align::Start)
                                                        .width(Length::Fill(1)),
                                                    |ui| {
                                                        ui.text(label)
                                                            .style(field_label_style(false));
                                                        ui.text(desc)
                                                            .style(field_description_style());
                                                    },
                                                );
                                            });
                                            ui.switch(
                                                Switch {
                                                    checked: self.sw[idx],
                                                    ..Default::default()
                                                },
                                                move || Msg::ToggleSw(idx),
                                            );
                                        },
                                    );
                                    if idx == 6 {
                                        ui.separator(Separator::horizontal());
                                    }
                                }
                            },
                        );
                    },
                );
            });

            let states = [
                ("default", false, false, Presented::default()),
                ("hover", false, false, Presented::HOVER),
                ("focus-visible", false, false, Presented::FOCUS_VISIBLE),
                ("active", true, false, Presented::default()),
                ("disabled", false, true, Presented::default()),
            ];
            for chunk in states.chunks(3) {
                ui.row(Row::new().gap(Space::Lg).align(Align::Start), |ui| {
                    for &(state, active, disabled, presented) in chunk {
                        let id = match state {
                            "default" => "settings-nav-item.default",
                            "hover" => "settings-nav-item.default.hover",
                            "focus-visible" => "settings-nav-item.default.focus-visible",
                            "active" => "settings-nav-item.default.active",
                            _ => "settings-nav-item.default.disabled",
                        };
                        ui.box_(
                            BoxProps::new()
                                .width(Length::Fixed(dp(224.0)))
                                .style(BoxStyle {
                                    background: Color::Role(ColorRole::Sidebar),
                                    padding: Insets::all(dp(8.0)),
                                    ..Default::default()
                                }),
                            |ui| {
                                ui.settings_nav_item(
                                    id,
                                    "Appearance",
                                    Icon::Palette,
                                    active,
                                    SettingsNavItemOpts {
                                        disabled,
                                        automation_id: None,
                                        presented,
                                    },
                                    || Msg::SetNav("appearance"),
                                );
                            },
                        );
                    }
                });
            }
        });
}
}
