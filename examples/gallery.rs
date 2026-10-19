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

    /// Slice-1 ships the Components and Typography pages; the rest are
    /// present in the shell (frozen ids) but not yet implemented.
    fn implemented(self) -> bool {
        matches!(self, Page::Components | Page::Typography)
    }
}

struct Gallery {
    page: Page,
    theme: ThemeMode,
}

enum Msg {
    SetPage(Page),
    SetTheme(ThemeMode),
    Invoke(&'static str),
}

impl Gallery {
    fn update(&mut self, msg: Msg, _cx: &mut UpdateCtx<Msg>) {
        match msg {
            Msg::SetPage(p) => self.page = p,
            Msg::SetTheme(t) => self.theme = t,
            Msg::Invoke(_) => {}
        }
    }

    fn view(&self, ui: &mut Ui<Msg>) {
        ui.theme(Theme::resolve(self.theme, ui.appearance()));
        ui.named("shell.root", |ui| {
            ui.column(
                Column::new()
                    .gap(Space::Md)
                    .padding(Space::Lg)
                    .align(Align::Stretch),
                |ui| {
                    self.header(ui);
                    match self.page {
                        Page::Components => self.components_page(ui),
                        Page::Typography => self.typography_page(ui),
                        other => self.stub_page(ui, other),
                    }
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
                        theme_btn(
                            ui,
                            "shell.theme.light",
                            ThemeMode::Light,
                            Icon::Sun,
                            "Light theme",
                        );
                        theme_btn(
                            ui,
                            "shell.theme.dark",
                            ThemeMode::Dark,
                            Icon::Moon,
                            "Dark theme",
                        );
                        theme_btn(
                            ui,
                            "shell.theme.system",
                            ThemeMode::System,
                            Icon::Monitor,
                            "System theme",
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
            // Cards
            ui.row(Row::new().gap(Space::Md).align(Align::Start), |ui| {
                ui.named("card.default", |ui| {
                    ui.card(Card::new().width(dp(288.0)), |ui| {
                        ui.part("header", |ui| {
                            ui.column(Column::new().gap(Space::Xs), |ui| {
                                ui.part("title", |ui| {
                                    ui.label("Card Title");
                                });
                                ui.part("description", |ui| {
                                    ui.typography(
                                        Typography::Muted,
                                        "A short supporting description.",
                                    );
                                });
                            });
                        });
                        ui.part("content", |ui| {
                            ui.typography(Typography::Muted, "Card content region.");
                        });
                        ui.part("footer", |ui| {
                            ui.row(Row::new().gap(Space::Sm).justify(Justify::End), |ui| {
                                ui.button("Cancel")
                                    .variant(ButtonVariant::Outline)
                                    .size(ControlSize::Sm);
                                ui.button("Save")
                                    .size(ControlSize::Sm)
                                    .on_press(|| Msg::Invoke("card.default"));
                            });
                        });
                    });
                });
                ui.named("card.default.size-sm", |ui| {
                    ui.card(Card::new().size(CardSize::Sm).width(dp(288.0)), |ui| {
                        ui.part("header", |ui| {
                            ui.column(Column::new().gap(Space::Xs), |ui| {
                                ui.part("title", |ui| {
                                    ui.label("Card Title");
                                });
                                ui.part("description", |ui| {
                                    ui.typography(
                                        Typography::Muted,
                                        "A short supporting description.",
                                    );
                                });
                            });
                        });
                        ui.part("content", |ui| {
                            ui.typography(Typography::Muted, "Card content region.");
                        });
                        ui.part("footer", |ui| {
                            ui.row(Row::new().gap(Space::Sm).justify(Justify::End), |ui| {
                                ui.button("Cancel")
                                    .variant(ButtonVariant::Outline)
                                    .size(ControlSize::Sm);
                                ui.button("Save")
                                    .size(ControlSize::Sm)
                                    .on_press(|| Msg::Invoke("card.default.size-sm"));
                            });
                        });
                    });
                });
            });
        });
    }

    /// A slice-1 placeholder for the frozen shell tabs that aren't part
    /// of this slice — the nav ids exist, the page is intentionally thin.
    fn stub_page(&self, ui: &mut Ui<Msg>, page: Page) {
        ui.typography(
            Typography::Muted,
            &format!("{} — not part of slice 1.", page.label()),
        );
    }

    /// The frozen `typography` slice — the CORE display recipes.
    fn typography_page(&self, ui: &mut Ui<Msg>) {
        ui.column(
            Column::new().gap(Space::Lg).align(Align::Stretch),
            |ui| {
                ui.named("typography.h1", |ui| {
                    ui.typography(Typography::H1, "Design is intelligence made visible");
                });
                ui.named("typography.h2", |ui| {
                    ui.typography(Typography::H2, "Heading 2 with a border rule");
                });
                ui.named("typography.h3", |ui| {
                    ui.typography(Typography::H3, "Heading 3");
                });
                ui.named("typography.h4", |ui| {
                    ui.typography(Typography::H4, "Heading 4");
                });
                ui.named("typography.p", |ui| {
                    ui.typography(
                        Typography::Paragraph,
                        "The king's pawn. A paragraph of body text that wraps to the available width and keeps a measured leading of seven.",
                    );
                });
                ui.named("typography.lead", |ui| {
                    ui.typography(
                        Typography::Lead,
                        "A lead paragraph with muted foreground and a larger size to set the tone.",
                    );
                });
                ui.named("typography.small", |ui| {
                    ui.typography(Typography::Small, "Small caption text");
                });
                ui.named("typography.muted", |ui| {
                    ui.typography(Typography::Muted, "Muted helper text");
                });
                ui.named("typography.inline-code", |ui| {
                    ui.typography(Typography::InlineCode, "rust-ui --install");
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
    }
}

fn main() -> UiResult {
    let app = App::new(
        Gallery {
            page: Page::Components,
            theme: ThemeMode::System,
        },
        Gallery::update,
        Gallery::view,
    )
    .title("rust-ui Gallery");
    app.run()
}
