impl Gallery {
    /// The frozen `components` slice — Button variants x states, sizes,
    /// inline icons, IconButton grid, Card specimens. Interaction cells
    /// (hover/pressed/focus-visible) are presentation-staged specimens
    /// (`.presented`), never devctl-driven runtime state; disabled and
    /// invalid remain semantic properties.
    fn components_page(&self, ui: &mut Ui<Msg>) {
        ui.column(Column::new().gap(Space::Md).align(Align::Stretch), |ui| {
            // one variant per row — the frozen 6-cell state order:
            // normal, hover, pressed, focus-visible, disabled, invalid
            for (v, id, label) in [
                (ButtonVariant::Primary, "button.default", "Default"),
                (ButtonVariant::Secondary, "button.secondary", "Secondary"),
                (ButtonVariant::Outline, "button.outline", "Outline"),
                (ButtonVariant::Ghost, "button.ghost", "Ghost"),
                (
                    ButtonVariant::Destructive,
                    "button.destructive",
                    "Destructive",
                ),
                (ButtonVariant::Link, "button.link", "Link"),
            ] {
                ui.row(Row::new().gap(Space::Md).align(Align::Center), |ui| {
                    ui.button(label)
                        .automation_id(id)
                        .variant(v)
                        .on_press(|| Msg::Invoke(id));
                    ui.button(label)
                        .automation_id(&format!("{id}.hover"))
                        .variant(v)
                        .presented(Presented::HOVER)
                        .on_press(|| Msg::Invoke(id));
                    ui.button(label)
                        .automation_id(&format!("{id}.pressed"))
                        .variant(v)
                        .presented(Presented::PRESSED)
                        .on_press(|| Msg::Invoke(id));
                    ui.button(label)
                        .automation_id(&format!("{id}.focus-visible"))
                        .variant(v)
                        .presented(Presented::FOCUS_VISIBLE)
                        .on_press(|| Msg::Invoke(id));
                    ui.button("Disabled")
                        .automation_id(&format!("{id}.disabled"))
                        .variant(v)
                        .disabled(true)
                        .on_press(|| Msg::Invoke(id));
                    ui.button("Invalid")
                        .automation_id(&format!("{id}.invalid"))
                        .variant(v)
                        .invalid(true)
                        .on_press(|| Msg::Invoke(id));
                });
            }
            // sizes + inline icons on the shared next row
            ui.row(Row::new().gap(Space::Md).align(Align::Center), |ui| {
                ui.button("Size xs")
                    .automation_id("button.default.size-xs")
                    .size(ControlSize::Xs)
                    .on_press(|| Msg::Invoke("button.default.size-xs"));
                ui.button("Size sm")
                    .automation_id("button.default.size-sm")
                    .size(ControlSize::Sm)
                    .on_press(|| Msg::Invoke("button.default.size-sm"));
                ui.button("Size lg")
                    .automation_id("button.default.size-lg")
                    .size(ControlSize::Lg)
                    .on_press(|| Msg::Invoke("button.default.size-lg"));
                ui.button("Add item")
                    .automation_id("button.default.icon-inline-start")
                    .icon_start(Icon::ArrowLeftCircle)
                    .on_press(|| Msg::Invoke("button.default.icon-inline-start"));
                ui.button("Continue")
                    .automation_id("button.default.icon-inline-end")
                    .icon_end(Icon::ArrowRight)
                    .on_press(|| Msg::Invoke("button.default.icon-inline-end"));
            });
            // IconButton — one variant per row, the frozen 5-cell state
            // order: normal, hover, pressed, focus-visible, disabled
            // (no link, no invalid)
            for (v, id) in [
                (ButtonVariant::Primary, "icon-button.default"),
                (ButtonVariant::Secondary, "icon-button.secondary"),
                (ButtonVariant::Outline, "icon-button.outline"),
                (ButtonVariant::Ghost, "icon-button.ghost"),
                (ButtonVariant::Destructive, "icon-button.destructive"),
            ] {
                ui.row(Row::new().gap(Space::Md).align(Align::Center), |ui| {
                    ui.icon_button(Icon::ArrowRight, id)
                        .automation_id(id)
                        .variant(v)
                        .on_press(|| Msg::Invoke(id));
                    ui.icon_button(Icon::ArrowRight, id)
                        .automation_id(&format!("{id}.hover"))
                        .variant(v)
                        .presented(Presented::HOVER)
                        .on_press(|| Msg::Invoke(id));
                    ui.icon_button(Icon::ArrowRight, id)
                        .automation_id(&format!("{id}.pressed"))
                        .variant(v)
                        .presented(Presented::PRESSED)
                        .on_press(|| Msg::Invoke(id));
                    ui.icon_button(Icon::ArrowRight, id)
                        .automation_id(&format!("{id}.focus-visible"))
                        .variant(v)
                        .presented(Presented::FOCUS_VISIBLE)
                        .on_press(|| Msg::Invoke(id));
                    ui.icon_button(Icon::ArrowRight, "disabled")
                        .automation_id(&format!("{id}.disabled"))
                        .variant(v)
                        .disabled(true);
                });
            }
            // IconButton sizes
            ui.row(Row::new().gap(Space::Md).align(Align::Center), |ui| {
                for (size, id) in [
                    (ControlSize::Xs, "icon-button.default.size-icon-xs"),
                    (ControlSize::Sm, "icon-button.default.size-icon-sm"),
                    (ControlSize::Md, "icon-button.default.size-icon"),
                    (ControlSize::Lg, "icon-button.default.size-icon-lg"),
                ] {
                    ui.icon_button(Icon::ArrowRight, id)
                        .automation_id(id)
                        .size(size)
                        .on_press(|| Msg::Invoke(id));
                }
            });
            // Badges — the frozen variant set (link wraps to its own row)
            ui.row(Row::new().gap(Space::Md).align(Align::Center), |ui| {
                for (v, id) in [
                    (BadgeVariant::Default, "badge.default"),
                    (BadgeVariant::Secondary, "badge.secondary"),
                    (BadgeVariant::Outline, "badge.outline"),
                    (BadgeVariant::Destructive, "badge.destructive"),
                    (BadgeVariant::Ghost, "badge.ghost"),
                ] {
                    ui.named(id, |ui| {
                        ui.badge(v, "Badge");
                    });
                }
            });
            ui.row(Row::new().gap(Space::Md).align(Align::Center), |ui| {
                ui.named("badge.link", |ui| {
                    ui.badge(BadgeVariant::Link, "Badge");
                });
            });
            // Separators — the frozen `w-48` horizontal / `h-16
            // items-stretch` vertical specimen wrappers
            ui.row(Row::new().gap(Space::Md).align(Align::Stretch), |ui| {
                ui.box_(
                    BoxProps::new()
                        .width(Length::Fixed(dp(192.0)))
                        .height(Length::Fixed(dp(64.0))),
                    |ui| {
                        ui.column(
                            Column::new()
                                .padding(Space::None)
                                .gap(Space::None)
                                .align(Align::Stretch)
                                .justify(Justify::Center),
                            |ui| {
                                ui.named("separator.horizontal", |ui| {
                                    ui.separator(Separator::horizontal());
                                });
                            },
                        );
                    },
                );
                ui.box_(
                    BoxProps::new()
                        .width(Length::Fixed(dp(192.0)))
                        .height(Length::Fixed(dp(64.0))),
                    |ui| {
                        ui.row(
                            Row::new()
                                .padding(Space::None)
                                .gap(Space::None)
                                .align(Align::Stretch)
                                .justify(Justify::Center),
                            |ui| {
                                ui.named("separator.vertical", |ui| {
                                    ui.separator(Separator::vertical());
                                });
                            },
                        );
                    },
                );
            });
            // Cards
            ui.row(Row::new().gap(Space::Md).align(Align::Start), |ui| {
                ui.named("card.default", |ui| {
                    ui.card(Card::new().width(dp(288.0)), |ui| {
                        card_parts(ui, CardSize::Default, "card.default")
                    });
                });
                ui.named("card.default.size-sm", |ui| {
                    ui.card(Card::new().size(CardSize::Sm).width(dp(288.0)), |ui| {
                        card_parts(ui, CardSize::Sm, "card.default.size-sm")
                    });
                });
            });
            // Alerts — the frozen default + destructive specimens (icon,
            // title, description parts against the alert recipes)
            ui.row(Row::new().gap(Space::Md).align(Align::Start), |ui| {
                for (v, id, title, desc) in [
                    (
                        AlertVariant::Default,
                        "alert.default",
                        "Heads up",
                        "Your workspace was updated successfully.",
                    ),
                    (
                        AlertVariant::Destructive,
                        "alert.destructive",
                        "Something went wrong",
                        // the frozen capture is a 2-line wrap at 274dp
                        // ("…sign in" / "again."); DirectWrite measures
                        // the one-line advance ~1dp inside 274 where
                        // Chromium's wraps — the explicit break
                        // reproduces the frozen lines at the same wrap
                        // boundary (content differs by whitespace kind)
                        "Your session has expired. Please sign in\nagain.",
                    ),
                ] {
                    ui.named(id, |ui| {
                        ui.alert(Alert::new().variant(v).width(dp(320.0)), |ui| {
                            ui.row(
                                Row::new()
                                    .gap(Space::Sm)
                                    .align(Align::Start)
                                    .padding(Space::None),
                                |ui| {
                                    // the icon centers on the first 20dp
                                    // text line inside its 16×20 slot
                                    ui.box_(
                                        BoxProps::new()
                                            .width(Length::Fixed(dp(16.0)))
                                            .height(Length::Fixed(dp(20.0))),
                                        |ui| {
                                            ui.column(
                                                Column::new()
                                                    .padding(Space::None)
                                                    .gap(Space::None)
                                                    .justify(Justify::Center),
                                                |ui| {
                                                    ui.part("icon", |ui| {
                                                        ui.icon(Icon::CircleAlert)
                                                            .size(dp(16.0))
                                                            .color_role(alert_icon_role(v));
                                                    });
                                                },
                                            );
                                        },
                                    );
                                    ui.box_(BoxProps::new().width(Length::Fill(1)), |ui| {
                                        ui.column(
                                            Column::new().padding(Space::None).gap(Space::None),
                                            |ui| {
                                                ui.part("title", |ui| {
                                                    ui.text(title).style(alert_title_style(v));
                                                });
                                                // the frozen `gap-0.5`
                                                // (2dp) grid row gap
                                                ui.box_(
                                                    BoxProps::new().height(Length::Fixed(dp(2.0))),
                                                    |_| {},
                                                );
                                                ui.part("description", |ui| {
                                                    ui.text(desc)
                                                        .style(alert_description_style(v))
                                                        .wrap(true);
                                                });
                                            },
                                        );
                                    });
                                },
                            );
                        });
                    });
                }
            });
            // Skeleton — the frozen `w-56 items-center gap-4` specimen
            ui.named("skeleton.default", |ui| {
                ui.box_(
                    BoxProps::new()
                        .width(Length::Fixed(dp(224.0)))
                        .height(Length::Fixed(dp(40.0))),
                    |ui| {
                        ui.row(
                            Row::new()
                                .gap(Space::Lg)
                                .align(Align::Center)
                                .padding(Space::None),
                            |ui| {
                                ui.part("avatar", |ui| {
                                    ui.skeleton(Skeleton::new().circle().size(dp(40.0), dp(40.0)));
                                });
                                ui.box_(
                                    BoxProps::new()
                                        .width(Length::Fixed(dp(168.0)))
                                        .height(Length::Fixed(dp(40.0))),
                                    |ui| {
                                        ui.column(
                                            Column::new().gap(Space::Sm).padding(Space::None),
                                            |ui| {
                                                ui.part("line[1]", |ui| {
                                                    ui.skeleton(
                                                        Skeleton::new().size(dp(168.0), dp(16.0)),
                                                    );
                                                });
                                                ui.part("line[2]", |ui| {
                                                    ui.skeleton(
                                                        Skeleton::new().size(dp(112.0), dp(16.0)),
                                                    );
                                                });
                                            },
                                        );
                                    },
                                );
                            },
                        );
                    },
                );
            });
            // Kbd — a single key cap plus the frozen 3-key group
            ui.row(Row::new().gap(Space::Md).align(Align::Center), |ui| {
                ui.named("kbd.default", |ui| {
                    ui.kbd("Ctrl K");
                });
                ui.named("kbd.group", |ui| {
                    ui.kbd_group(|ui| {
                        for (key, text) in [("ctrl", "Ctrl"), ("shift", "Shift"), ("p", "P")] {
                            ui.part(&format!("key[{key}]"), |ui| {
                                ui.kbd(text);
                            });
                        }
                    });
                });
            });
        });
}
}
