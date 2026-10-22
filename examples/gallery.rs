// Gallery — the native rust-ui Shadcn specimen host. Built on the same
// retained tree + component recipes as any app: Gallery code composes
// Button / IconButton / Card / Typography / Icon, never reimplements
// them. The frozen `reference/shadcn-gallery-v0.1/` contract owns the
// automation ids, geometry and styling this host must reproduce.
// `src/bin/rust-ui-gallery.rs` includes this file verbatim — keep
// top-of-file comments as `//` (inner `//!` docs cannot be included).

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

    /// All stays a stub until the Gallery-completeness milestone.
    fn implemented(self) -> bool {
        matches!(
            self,
            Page::Components
                | Page::Typography
                | Page::Forms
                | Page::Navigation
                | Page::Overlays
                | Page::NativeText
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
    env: &'static str,
    select_open: bool,
    menu_open: bool,
    menu_toolbar: bool,
    menu_wrap: bool,
    menu_panel: &'static str,
    tabs_default: &'static str,
    tabs_line: &'static str,
    nav_selected: &'static str,
    nav_collapsed: bool,
    settings_theme: &'static str,
    settings_theme_open: bool,
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
    SelectOpen(&'static str, bool),
    SelectPick(&'static str, &'static str),
    MenuOpen(bool),
    MenuAction(&'static str),
    MenuCheck(&'static str, bool),
    MenuRadio(&'static str),
    SetTab(bool, &'static str),
    SetNav(&'static str),
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
            Msg::SelectOpen(id, open) => {
                if id == "select.default.open" {
                    self.select_open = open;
                } else if id == "settings-nav.theme" {
                    self.settings_theme_open = open;
                }
            }
            Msg::SelectPick(id, key) => {
                if id == "select.default.open" {
                    self.env = key;
                    self.select_open = false;
                } else if id == "settings-nav.theme" {
                    self.settings_theme = key;
                    self.settings_theme_open = false;
                }
            }
            Msg::MenuOpen(open) => self.menu_open = open,
            Msg::MenuAction(_) => self.menu_open = false,
            Msg::MenuCheck(key, value) => {
                match key {
                    "toolbar" => self.menu_toolbar = value,
                    "word-wrap" => self.menu_wrap = value,
                    _ => {}
                }
                self.menu_open = false;
            }
            Msg::MenuRadio(key) => {
                self.menu_panel = key;
                self.menu_open = false;
            }
            Msg::SetTab(line, key) => {
                if line {
                    self.tabs_line = key;
                } else {
                    self.tabs_default = key;
                }
            }
            Msg::SetNav(key) => self.nav_selected = key,
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
        });
    }

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
                                        width: Some(dp(640.0)),
                                    },
                                    |ui| {
                                        ui.part("content", |ui| {
                                            ui.column(
                                                Column::new()
                                                    .padding(Space::None)
                                                    .gap(Space::Xs)
                                                    .align(Align::Start),
                                                |ui| {
                                                    ui.text("Theme")
                                                        .style(field_label_style(false));
                                                    ui.text("Select the interface theme.")
                                                        .style(field_description_style());
                                                },
                                            );
                                        });
                                        ui.box_(BoxProps::new().width(Length::Fill(1)), |_| {});
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
                                            width: Some(dp(640.0)),
                                        },
                                        |ui| {
                                            ui.part("content", |ui| {
                                                ui.column(
                                                    Column::new()
                                                        .padding(Space::None)
                                                        .gap(Space::Xs)
                                                        .align(Align::Start),
                                                    |ui| {
                                                        ui.text(label)
                                                            .style(field_label_style(false));
                                                        ui.text(desc)
                                                            .style(field_description_style());
                                                    },
                                                );
                                            });
                                            ui.box_(BoxProps::new().width(Length::Fill(1)), |_| {});
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
                                .width(Length::Fixed(dp(208.0)))
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

    fn overlays_page(&self, ui: &mut Ui<Msg>) {
        ui.column(Column::new().gap(Space::Lg).align(Align::Start), |ui| {
            ui.named("select.default.open", |ui| {
                ui.row(Row::new(), |ui| {
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
                            SelectChange::Pick(key) => Msg::SelectPick("select.default.open", key),
                        },
                        |items| {
                            items.group("environment", "Environment", |items| {
                                for (key, text, disabled, presented) in [
                                    ("development", "Development", false, Presented::default()),
                                    ("staging", "Staging", false, Presented::HOVER),
                                    ("production", "Production", false, Presented::default()),
                                    ("preview", "Preview", false, Presented::default()),
                                    ("deprecated", "Deprecated", true, Presented::default()),
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
                                    "team-members",
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
                                "delete-account",
                                "Delete account",
                                DropdownItemOpts {
                                    kind: DropdownItemKind::Destructive,
                                    icon: Some(Icon::LogOut),
                                    ..Default::default()
                                },
                            );
                            menu.item(
                                "delete-workspace",
                                "Delete workspace",
                                DropdownItemOpts {
                                    kind: DropdownItemKind::Destructive,
                                    presented: Presented::HOVER,
                                    icon: Some(Icon::Trash2),
                                    ..Default::default()
                                },
                            );
                        },
                    );
                });
            });
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
            // deterministic page staging for devctl/evidence runs —
            // boots directly onto a page without pointer navigation
            page: match std::env::var("RUI_GALLERY_PAGE").as_deref() {
                Ok("forms") => Page::Forms,
                Ok("navigation") => Page::Navigation,
                Ok("typography") => Page::Typography,
                Ok("overlays") => Page::Overlays,
                Ok("native-text") => Page::NativeText,
                _ => Page::Components,
            },
            theme: ThemeMode::System,
            scroll: 0.0,
            texts,
            cb: [false, true, true, false, true, false, true],
            radio: ["", "option-a", "option-a", "option-a", "option-a"],
            sw: [false, true, true, false, true, false, true, false],
            env: "production",
            select_open: true,
            menu_open: true,
            menu_toolbar: true,
            menu_wrap: false,
            menu_panel: "left",
            tabs_default: "account",
            tabs_line: "account",
            nav_selected: "appearance",
            nav_collapsed: std::env::var("RUI_SETTINGS_NAV_COLLAPSED")
                .is_ok_and(|value| value == "1"),
            settings_theme: "system",
            settings_theme_open: false,
        },
        Gallery::update,
        Gallery::view,
    )
    .title("rust-ui Gallery");
    app.run()
}
