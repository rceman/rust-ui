{
            // switches — 8 frozen specimens
            ui.row(Row::new().gap(Space::Lg).align(Align::Center), |ui| {
                for (i, (id, dis, inv, sm)) in [
                    ("switch.default.unchecked", false, false, false),
                    ("switch.default.checked", false, false, false),
                    ("switch.default.focus-visible", false, false, false),
                    ("switch.default.disabled", true, false, false),
                    ("switch.default.disabled-checked", true, false, false),
                    ("switch.default.invalid", false, true, false),
                    ("switch.default.size-sm", false, false, true),
                    ("switch.default.size-sm-unchecked", false, false, true),
                ]
                .iter()
                .enumerate()
                {
                    ui.named(id, |ui| {
                        ui.switch(
                            Switch {
                                checked: self.sw[i],
                                disabled: *dis,
                                invalid: *inv,
                                size: if *sm {
                                    ToggleSize::Sm
                                } else {
                                    ToggleSize::Default
                                },
                                // frozen fv cell is checked + ringed
                                presented: if *id == "switch.default.focus-visible" {
                                    Presented::FOCUS_VISIBLE
                                } else {
                                    Presented::default()
                                },
                                ..Default::default()
                            },
                            move || Msg::ToggleSw(i),
                        );
                    });
                }
            });
            // fields — vertical default (+invalid +disabled) + horizontal.
            // Wrapped: four 288dp fields never fit one row inside the
            // client — the frozen page wraps them too
            const FIELDS: [(&str, bool, bool); 4] = [
                ("field.default", false, false),
                ("field.default.invalid", true, false),
                ("field.default.disabled", false, true),
                ("field.horizontal", false, false),
            ];
            for chunk in FIELDS.chunks(2) {
                ui.row(Row::new().gap(Space::Lg).align(Align::Start), |ui| {
                    for &(id, inv, dis) in chunk {
                        if id == "field.horizontal" {
                            ui.named(id, |ui| {
                                ui.field(
                                    Field {
                                        orientation: FieldOrientation::Horizontal,
                                        width: Some(dp(288.0)),
                                    },
                                    |ui| {
                                        ui.part("content", |ui| {
                                            ui.box_(
                                                BoxProps::new()
                                                    .width(Length::Fixed(dp(248.0)))
                                                    .height(Length::Fixed(dp(37.0))),
                                                |ui| {
                                                    ui.column(
                                                        Column::new()
                                                            .padding(Space::None)
                                                            .gap(Space::Xs),
                                                        |ui| {
                                                            ui.part("label", |ui| {
                                                                ui.text("Two-factor auth")
                                                                    .style(field_label_style(
                                                                        false,
                                                                    ));
                                                            });
                                                            ui.part("description", |ui| {
                                                                ui.text("Require a second factor at sign-in.")
                                                                    .style(
                                                                        field_description_style(),
                                                                    );
                                                            });
                                                        },
                                                    );
                                                },
                                            );
                                        });
                                        ui.switch(
                                            Switch {
                                                checked: self.sw[1],
                                                ..Default::default()
                                            },
                                            || Msg::ToggleSw(1),
                                        );
                                    },
                                );
                            });
                            continue;
                        }
                        let fi = FIELDS.iter().position(|f| f.0 == id).unwrap();
                        // frozen field() composition: label + control +
                        // tail. The tail is part "error" only on invalid
                        // (else "description"), and only the disabled
                        // specimen's input carries data-part="control".
                        // Frozen control strings: default/disabled mount
                        // placeholder "you@example.com" (empty value);
                        // invalid mounts value "not-an-email", no hint
                        let (tail, control_part, control_ph) = match id {
                            "field.default.invalid" => {
                                ("Enter a valid email address.", false, None)
                            }
                            "field.default.disabled" => (
                                "Locked for this account.",
                                true,
                                Some("you@example.com"),
                            ),
                            _ => (
                                "We'll only use this for notifications.",
                                false,
                                Some("you@example.com"),
                            ),
                        };
                        ui.named(id, |ui| {
                            ui.field(
                                Field {
                                    orientation: FieldOrientation::Vertical,
                                    width: Some(dp(288.0)),
                                },
                                |ui| {
                                    ui.part("label", |ui| {
                                        ui.text("Email").style(field_label_style(inv));
                                    });
                                    let idx = T_FIELDS + fi;
                                    let stage_control = |ui: &mut Ui<Msg>| {
                                        let mut b = ui
                                            .input(&self.texts[idx])
                                            .height(Length::Fixed(dp(32.0)))
                                            .line_height(dp(20.0))
                                            .disabled(dis)
                                            .invalid(inv);
                                        if let Some(p) = control_ph {
                                            b = b.placeholder(p);
                                        }
                                        b.on_edit(move |e| Msg::Edit(idx, e));
                                    };
                                    if control_part {
                                        ui.part("control", stage_control);
                                    } else {
                                        stage_control(ui);
                                    }
                                    if inv {
                                        ui.part("error", |ui| {
                                            ui.text(tail).style(field_error_style());
                                        });
                                    } else {
                                        ui.part("description", |ui| {
                                            ui.text(tail).style(field_description_style());
                                        });
                                    }
                                },
                            );
                        });
                    }
                });
            }
            ui.column(Column::new().gap(Space::Sm).align(Align::Start), |ui| {
                for states in [
                    [
                        (
                            "select.default.placeholder",
                            None,
                            false,
                            false,
                            false,
                            ControlSize::Md,
                        ),
                        (
                            "select.default.filled",
                            Some("Production"),
                            false,
                            false,
                            false,
                            ControlSize::Md,
                        ),
                        (
                            "select.default.focus-visible",
                            Some("Production"),
                            false,
                            false,
                            true,
                            ControlSize::Md,
                        ),
                        (
                            "select.default.disabled",
                            Some("Production"),
                            true,
                            false,
                            false,
                            ControlSize::Md,
                        ),
                    ],
                    [
                        (
                            "select.default.invalid",
                            Some("Production"),
                            false,
                            true,
                            false,
                            ControlSize::Md,
                        ),
                        (
                            "select.default.size-sm",
                            Some("Production"),
                            false,
                            false,
                            false,
                            ControlSize::Sm,
                        ),
                        ("", None, false, false, false, ControlSize::Md),
                        ("", None, false, false, false, ControlSize::Md),
                    ],
                ] {
                    ui.row(Row::new().gap(Space::Lg).align(Align::Center), |ui| {
                        for (id, value, disabled, invalid, focus, size) in states {
                            if id.is_empty() {
                                continue;
                            }
                            let mut props = Select::new(id);
                            props.value = value;
                            props.disabled = disabled;
                            props.invalid = invalid;
                            props.size = size;
                            props.presented = if focus {
                                Presented::FOCUS_VISIBLE
                            } else {
                                Presented::default()
                            };
                            ui.select(props, move |_| Msg::Invoke(id), |_| {});
                        }
                    });
                }
            });
}