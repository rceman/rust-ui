//! Gallery — the native rust-ui Shadcn specimen host. Built on the same
//! retained tree + component recipes as any app: Gallery code composes
//! Button / IconButton / Card / Typography / Icon, never reimplements
//! them. The frozen `reference/shadcn-gallery-v0.1/` contract owns the
//! automation ids, geometry and styling this host must reproduce.

use rust_ui::prelude::*;
use rust_ui::{Icon, Typography};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Page {
    All,
    Components,
    Forms,
    Navigation,
    Typography,
    Overlays,
    NativeText,
}

impl Page {
    const ALL: [Page; 7] = [
        Page::All,
        Page::Components,
        Page::Forms,
        Page::Navigation,
        Page::Typography,
        Page::Overlays,
        Page::NativeText,
    ];

    fn id(self) -> &'static str {
        match self {
            Page::All => "all",
            Page::Components => "components",
            Page::Forms => "forms",
            Page::Navigation => "navigation",
            Page::Typography => "typography",
            Page::Overlays => "overlays",
            Page::NativeText => "native-text",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Page::All => "All",
            Page::Components => "Components",
            Page::Forms => "Forms",
            Page::Navigation => "Navigation",
            Page::Typography => "Typography",
            Page::Overlays => "Overlays",
            Page::NativeText => "Native Text",
        }
    }

    /// Slice-2 ships Components, Typography, Forms and Native Text; the
    /// rest are present in the shell (frozen ids) but not yet implemented.
    fn implemented(self) -> bool {
        matches!(
            self,
            Page::Components | Page::Typography | Page::Forms | Page::NativeText
        )
    }
}

/// forms text bindings: inputs 0..6, textareas 6..12, native-text 12..26,
/// field controls 26..29, the `label.default` sibling input 29 — every
/// mounted editor needs a DISTINCT lease
/// (two peers on one `TextValue` is `DuplicateTextBinding`).
const T_INPUTS: usize = 0;
const T_AREAS: usize = 6;
const T_NATIVE: usize = 12;
const T_FIELDS: usize = 26;
const T_LABEL: usize = 29;
const T_COUNT: usize = 30;

struct Gallery {
    page: Page,
    theme: ThemeMode,
    /// app-owned page scroll offset (Dp) — reset on page change
    scroll: f32,
    /// bound text values for input/textarea/native-text specimens
    texts: Vec<TextValue>,
    /// app-owned toggle state — never widget-hidden
    cb: [bool; 7],
    radio: [&'static str; 5],
    sw: [bool; 8],
}

enum Msg {
    SetPage(Page),
    SetTheme(ThemeMode),
    Invoke(&'static str),
    Scroll(f32),
    Edit(usize, TextEdit),
    Conflict(usize, TextConflict),
    ToggleCb(usize),
    SetRadio(usize, &'static str),
    ToggleSw(usize),
}

impl Gallery {
    fn update(&mut self, msg: Msg, _cx: &mut UpdateCtx<Msg>) {
        match msg {
            Msg::SetPage(p) => {
                self.page = p;
                self.scroll = 0.0;
            }
            Msg::SetTheme(t) => self.theme = t,
            Msg::Invoke(_) => {}
            Msg::Scroll(dy) => self.scroll = (self.scroll + dy).max(0.0),
            Msg::Edit(i, e) => {
                let _ = self.texts[i].accept(e);
            }
            Msg::Conflict(_, _) => {}
            Msg::ToggleCb(i) => self.cb[i] = !self.cb[i],
            Msg::SetRadio(g, k) => self.radio[g] = k,
            Msg::ToggleSw(i) => self.sw[i] = !self.sw[i],
        }
    }

    fn view(&self, ui: &mut Ui<Msg>) {
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
                                Page::NativeText => self.native_text_page(ui),
                                other => self.stub_page(ui, other),
                            },
                        );
                    });
                },
            );
        });
    }

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

    /// The frozen `components` slice — Button variants x states, sizes,
    /// inline icons, IconButton grid, Card specimens. State cells beyond
    /// `normal` are devctl-driven (hover/press/focus), not force-styled.
    fn components_page(&self, ui: &mut Ui<Msg>) {
        ui.column(Column::new().gap(Space::Md).align(Align::Stretch), |ui| {
            for pair in [
                [
                    (ButtonVariant::Primary, "button.default", "Default"),
                    (ButtonVariant::Secondary, "button.secondary", "Secondary"),
                ],
                [
                    (ButtonVariant::Outline, "button.outline", "Outline"),
                    (ButtonVariant::Ghost, "button.ghost", "Ghost"),
                ],
                [
                    (
                        ButtonVariant::Destructive,
                        "button.destructive",
                        "Destructive",
                    ),
                    (ButtonVariant::Link, "button.link", "Link"),
                ],
            ] {
                ui.row(Row::new().gap(Space::Md).align(Align::Center), |ui| {
                    // state cells — devctl drives hover/press/focus on
                    // `id`, `id.disabled`/`id.invalid` are semantic
                    for (v, id, label) in pair {
                        ui.button(label)
                            .automation_id(id)
                            .variant(v)
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
                    }
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
            // IconButton — variants × states on one row; a second row for
            // the disabled cells keeps the grid inside the client height
            for half in [
                &[
                    (ButtonVariant::Primary, "icon-button.default"),
                    (ButtonVariant::Secondary, "icon-button.secondary"),
                    (ButtonVariant::Outline, "icon-button.outline"),
                ][..],
                &[
                    (ButtonVariant::Ghost, "icon-button.ghost"),
                    (ButtonVariant::Destructive, "icon-button.destructive"),
                ][..],
            ] {
                ui.row(Row::new().gap(Space::Md).align(Align::Center), |ui| {
                    for &(v, id) in half {
                        ui.icon_button(Icon::ArrowRight, id)
                            .automation_id(id)
                            .variant(v)
                            .on_press(|| Msg::Invoke(id));
                        ui.icon_button(Icon::ArrowRight, "disabled")
                            .automation_id(&format!("{id}.disabled"))
                            .variant(v)
                            .disabled(true);
                    }
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

    /// The frozen `forms` slice — contract order: label → input →
    /// textarea → checkbox → radio-group → switch → field.
    fn forms_page(&self, ui: &mut Ui<Msg>) {
        ui.column(Column::new().gap(Space::Lg).align(Align::Start), |ui| {
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
                    .height(Length::Fixed(dp(32.0)));
                if let Some(p) = *ph {
                    b = b.placeholder(p);
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
                    .height(Length::Fixed(dp(64.0)));
                if let Some(p) = *ph {
                    b = b.placeholder(p);
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
                                                    },
                                                );
                                                rg.item(
                                                    "option-b",
                                                    "Option B",
                                                    RadioItemOpts {
                                                        disabled: dis,
                                                        invalid: inv,
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
        });
    }

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
                    .height(Length::Fixed(dp(*h)));
                if let Some(p) = *ph {
                    b = b.placeholder(p);
                }
                b.disabled(*dis)
                    .read_only(*ro)
                    .invalid(*inv)
                    .on_edit(move |e| Msg::Edit(idx, e))
                    .on_conflict(move |c| Msg::Conflict(idx, c));
            }
        });
    }

    /// A placeholder for the frozen shell tabs that aren't part of this
    /// slice — the nav ids exist, the page is intentionally thin.
    fn stub_page(&self, ui: &mut Ui<Msg>, page: Page) {
        ui.typography(
            Typography::Muted,
            &format!("{} — not part of slice 2.", page.label()),
        );
    }

    /// The frozen `typography` slice — the CORE display recipes, on the
    /// frozen 720dp content column so wrapped headings measure right.
    fn typography_page(&self, ui: &mut Ui<Msg>) {
        // a row wrapper keeps the 720 box at its spec width — a Stretch
        // column would stretch the box itself to the page width
        ui.row(Row::new(), |ui| {
            ui.box_(BoxProps::new().width(Length::Fixed(dp(720.0))), |ui| {
                ui.column(
                    // no inner padding — the box is the frozen 720 content
                    // column; elements span it edge-to-edge
                    Column::new()
                        .gap(Space::Lg)
                        .padding(Space::None)
                        .align(Align::Stretch),
                |ui| {
                    ui.named("typography.h1", |ui| {
                        ui.typography(Typography::H1, "Taxing Laughter: The Joke Tax Chronicles");
                    });
                    ui.named("typography.h2", |ui| {
                        ui.typography(Typography::H2, "The People of the Kingdom");
                    });
                    ui.named("typography.h3", |ui| {
                        ui.typography(Typography::H3, "The Joke Tax");
                    });
                    ui.named("typography.h4", |ui| {
                        ui.typography(Typography::H4, "People stopped telling jokes");
                    });
                    ui.named("typography.p", |ui| {
                        ui.typography(
                            Typography::Paragraph,
                            "The king, seeing how much happier his subjects were, realized the error of his ways and repealed the joke tax.",
                        );
                    });
                    ui.named("typography.lead", |ui| {
                        ui.typography(
                            Typography::Lead,
                            "A modal dialog that interrupts the user with important content and expects a response.",
                        );
                    });
                    // small is a content-hugging label like inline-code —
                    // a row wrapper keeps it at content width inside the
                    // Stretch column (N2)
                    ui.row(Row::new(), |ui| {
                        ui.typography(Typography::Small, "Email address")
                            .automation_id("typography.small");
                    });
                    ui.named("typography.muted", |ui| {
                        ui.typography(Typography::Muted, "Enter your email address.");
                    });
                    // inline-code is a content-hugging box — a row wrapper
                    // keeps it at Content width inside the Stretch column
                    ui.row(Row::new(), |ui| {
                        ui.typography(Typography::InlineCode, "npx shadcn@latest init")
                            .automation_id("typography.inline-code");
                    });
                    ui.named("typography.code-block", |ui| {
                        ui.code_block(&[
                            "fn render_button(rect: Rect) {",
                            "ctx.fill(rect, theme.primary)",
                            "}",
                        ]);
                    });
                    },
                );
            });
        });
    }
}

/// The frozen `card.*` semantic parts — header/title/description/content/
/// footer composed against the Card contract recipes (`card_part_box`,
/// `card_title_style`, `card_content_style`, `card_footer_style`) so part
/// geometry and chrome come from the component contract, never generic
/// container defaults. `cs` selects Default or Sm.
fn card_parts(ui: &mut Ui<Msg>, cs: CardSize, id: &'static str) {
    ui.part("header", |ui| {
        ui.box_(BoxProps::new().style(card_part_box(cs)), |ui| {
            ui.column(Column::new().gap(Space::Xs).padding(Space::None), |ui| {
                ui.part("title", |ui| {
                    ui.text("Card Title").style(card_title_style(cs));
                });
                ui.part("description", |ui| {
                    ui.typography(Typography::Muted, "A short supporting description.");
                });
            });
        });
    });
    ui.part("content", |ui| {
        ui.box_(BoxProps::new().style(card_part_box(cs)), |ui| {
            ui.text("Card content region.").style(card_content_style());
        });
    });
    ui.part("footer", |ui| {
        ui.box_(BoxProps::new().style(card_footer_style(cs)), |ui| {
            ui.row(
                Row::new()
                    .gap(Space::Sm)
                    .padding(Space::None)
                    .justify(Justify::End),
                |ui| {
                    ui.button("Cancel")
                        .variant(ButtonVariant::Outline)
                        .size(ControlSize::Sm);
                    ui.button("Save")
                        .size(ControlSize::Sm)
                        .on_press(move || Msg::Invoke(id));
                },
            );
        });
    });
}

fn main() -> UiResult {
    // frozen recorded text is the authority — every staged string below
    // is the contract `captures.text.content` (text_runs source "value")
    // for that specimen's owner id, in T_* binding order. Specimens
    // whose frozen text runs carry source "placeholder" mount EMPTY
    // here; their hint string lives in the staged placeholder above
    const TEXT_SEEDS: [&str; T_COUNT] = [
        // input.default.{placeholder,filled,focus-visible,disabled,invalid,invalid-focus-visible}
        "",
        "Filled value",
        "Focused input",
        "",
        "bad@input",
        "bad@input",
        // textarea.default.* (same state order)
        "",
        "A longer message spanning\ntwo lines of text.",
        "Focused textarea",
        "",
        "invalid",
        "invalid",
        // native-text.single-line.{placeholder,filled,focused,selection,disabled,readonly,invalid}
        "",
        "A line of prose",
        "Focused text",
        "Select this phrase",
        "",
        "Read-only value",
        "not-an-email",
        // native-text.multiline.* (same state order)
        "",
        "First line of a note.\nSecond line with more text.\nThird line closes it out.",
        "Focused multiline text\nwith a second line.",
        "First line of a note.\nSecond line with more text.\nThird line closes it out.",
        "",
        "Read-only body.\nNo editing allowed.",
        "not-an-email",
        // field.default / .invalid / .disabled controls
        "",
        "not-an-email",
        "",
        // label.default sibling input (hint "With a label")
        "",
    ];
    let texts: Vec<TextValue> = TEXT_SEEDS.iter().map(|s| TextValue::new(*s)).collect();
    let app = App::new(
        Gallery {
            page: Page::Components,
            theme: ThemeMode::System,
            scroll: 0.0,
            texts,
            cb: [false, true, true, false, true, false, true],
            radio: ["", "option-a", "option-a", "option-a", "option-a"],
            sw: [false, true, true, false, true, false, true, false],
        },
        Gallery::update,
        Gallery::view,
    )
    .title("rust-ui Gallery");
    app.run()
}
