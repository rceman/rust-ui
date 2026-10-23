{
            // label — frozen `label.default` (214×54 block): part `label`
            // ("Label") stacked over a sibling input whose recorded hint
            // is "With a label" (frozen flex-col gap-2 = 8dp)
            ui.named("label.default", |ui| {
                ui.box_(
                    BoxProps::new()
                        .width(Length::Fixed(dp(214.0)))
                        .height(Length::Fixed(dp(54.0))),
                    |ui| {
                        ui.column(Column::new().padding(Space::None).gap(Space::Sm), |ui| {
                            ui.part("label", |ui| {
                                ui.text("Label").style(field_label_style(false));
                            });
                            ui.input(&self.texts[T_LABEL])
                                .width(Length::Fixed(dp(214.0)))
                                .height(Length::Fixed(dp(32.0)))
                                .line_height(dp(20.0))
                                .placeholder("With a label")
                                .on_edit(move |e| Msg::Edit(T_LABEL, e))
                                .on_conflict(move |c| Msg::Conflict(T_LABEL, c));
                        });
                    },
                );
            });
            // inputs — placeholder/filled/focus-visible/disabled/invalid/
            // invalid-focus-visible at 360×32. Frozen strings: only the
            // placeholder + disabled specimens carry a placeholder
            // attribute ("Placeholder text" / "Disabled input") — the
            // disabled one mounts EMPTY so the muted hint is its
            // recorded text
            for (i, (id, ph, dis, inv)) in [
                (
                    "input.default.placeholder",
                    Some("Placeholder text"),
                    false,
                    false,
                ),
                ("input.default.filled", None, false, false),
                ("input.default.focus-visible", None, false, false),
                (
                    "input.default.disabled",
                    Some("Disabled input"),
                    true,
                    false,
                ),
                ("input.default.invalid", None, false, true),
                ("input.default.invalid-focus-visible", None, false, true),
            ]
            .iter()
            .enumerate()
            {
                let idx = T_INPUTS + i;
                let mut b = ui
                    .input(&self.texts[idx])
                    .automation_id(id)
                    .width(Length::Fixed(dp(360.0)))
                    .height(Length::Fixed(dp(32.0)))
                    .line_height(dp(20.0));
                if let Some(p) = *ph {
                    b = b.placeholder(p);
                }
                // focus-visible is presentation-staged; invalid stays
                // semantic (`invalid-focus-visible` carries both)
                if id.ends_with("focus-visible") {
                    b = b.presented(Presented::FOCUS_VISIBLE);
                }
                b.disabled(*dis)
                    .invalid(*inv)
                    .on_edit(move |e| Msg::Edit(idx, e))
                    .on_conflict(move |c| Msg::Conflict(idx, c));
            }
            // textareas — same state set at 360×64; frozen hints are
            // "Write something..." / "Disabled textarea" (disabled empty)
            for (i, (id, ph, dis, inv)) in [
                (
                    "textarea.default.placeholder",
                    Some("Write something..."),
                    false,
                    false,
                ),
                ("textarea.default.filled", None, false, false),
                ("textarea.default.focus-visible", None, false, false),
                (
                    "textarea.default.disabled",
                    Some("Disabled textarea"),
                    true,
                    false,
                ),
                ("textarea.default.invalid", None, false, true),
                ("textarea.default.invalid-focus-visible", None, false, true),
            ]
            .iter()
            .enumerate()
            {
                let idx = T_AREAS + i;
                let mut b = ui
                    .textarea(&self.texts[idx])
                    .automation_id(id)
                    .width(Length::Fixed(dp(360.0)))
                    .height(Length::Fixed(dp(64.0)))
                    .line_height(dp(20.0));
                if let Some(p) = *ph {
                    b = b.placeholder(p);
                }
                if id.ends_with("focus-visible") {
                    b = b.presented(Presented::FOCUS_VISIBLE);
                }
                b.disabled(*dis)
                    .invalid(*inv)
                    .on_edit(move |e| Msg::Edit(idx, e))
                    .on_conflict(move |c| Msg::Conflict(idx, c));
            }
            // checkboxes — 7 frozen specimens; app model owns `checked`.
            // The control element stays its frozen 16×16 and the label is
            // owner-attributed, drawn ~24dp right of the box; a bare Lg
            // pitch row packs controls 32dp apart so every label collided
            // with its neighbour (N1). Spacer cells reserve the label run
            // (longest ≈102dp) inside a two-row layout like the frozen
            // 4+3 wrapping.
            // frozen label text = state name with dashes as spaces
            // (lowercase); the focus-visible row passes "Focus visible"
            const CHECKBOXES: [(&str, bool, bool, &str); 7] = [
                ("checkbox.default.unchecked", false, false, "unchecked"),
                ("checkbox.default.checked", false, false, "checked"),
                (
                    "checkbox.default.focus-visible",
                    false,
                    false,
                    "Focus visible",
                ),
                ("checkbox.default.disabled", true, false, "disabled"),
                (
                    "checkbox.default.disabled-checked",
                    true,
                    false,
                    "disabled checked",
                ),
                ("checkbox.default.invalid", false, true, "invalid"),
                (
                    "checkbox.default.invalid-checked",
                    false,
                    true,
                    "invalid checked",
                ),
            ];
            for chunk in CHECKBOXES.chunks(4) {
                ui.row(Row::new().gap(Space::Lg).align(Align::Center), |ui| {
                    for &(id, dis, inv, lbl) in chunk {
                        let i = CHECKBOXES.iter().position(|c| c.0 == id).unwrap();
                        ui.named(id, |ui| {
                            ui.checkbox(
                                Checkbox {
                                    checked: self.cb[i],
                                    disabled: dis,
                                    invalid: inv,
                                    label: Some(lbl),
                                    // frozen fv cell is checked + ringed
                                    presented: if id == "checkbox.default.focus-visible" {
                                        Presented::FOCUS_VISIBLE
                                    } else {
                                        Presented::default()
                                    },
                                    ..Default::default()
                                },
                                move || Msg::ToggleCb(i),
                            );
                        });
                        // label room: the painted label runs ~126dp past
                        // the 16dp control; the spacer keeps the next
                        // specimen out of it
                        ui.box_(BoxProps::new().width(Length::Fixed(dp(96.0))), |_| {});
                    }
                });
            }
            // radio groups — 5 frozen specimens, app-owned `selected`.
            // Wrapped at 2 per row like the frozen page: five 360dp
            // groups never fit one row inside the client
            const RADIO: [(&str, bool, bool); 5] = [
                ("radio-group.default.unselected", false, false),
                ("radio-group.default.selected", false, false),
                ("radio-group.default.focus-visible", false, false),
                ("radio-group.default.invalid", false, true),
                ("radio-group.default.disabled", true, false),
            ];
            for chunk in RADIO.chunks(2) {
                ui.row(Row::new().gap(Space::Lg).align(Align::Start), |ui| {
                    for &(id, dis, inv) in chunk {
                        let g = RADIO.iter().position(|r| r.0 == id).unwrap();
                        let sel = self.radio[g];
                        // frozen group box is 360×40 — a fixed box carries
                        // the element rect; items stack at the 8dp gap
                        ui.named(id, |ui| {
                            ui.box_(BoxProps::new().width(Length::Fixed(dp(360.0))), |ui| {
                                ui.column(
                                    Column::new().gap(Space::Sm).padding(Space::None),
                                    |ui| {
                                        ui.radio_group(
                                            sel,
                                            move |k| Msg::SetRadio(g, k),
                                            |rg| {
                                                rg.item(
                                                    "option-a",
                                                    "Option A",
                                                    RadioItemOpts {
                                                        disabled: dis,
                                                        invalid: inv,
                                                        ..Default::default()
                                                    },
                                                );
                                                rg.item(
                                                    "option-b",
                                                    "Option B",
                                                    RadioItemOpts {
                                                        disabled: dis,
                                                        invalid: inv,
                                                        // fv cell: option
                                                        // B ringed while A
                                                        // stays selected
                                                        presented: if id
                                                            == "radio-group.default.focus-visible"
                                                        {
                                                            Presented::FOCUS_VISIBLE
                                                        } else {
                                                            Presented::default()
                                                        },
                                                    },
                                                );
                                            },
                                        );
                                    },
                                );
                            });
                        });
                    }
                });
            }
}