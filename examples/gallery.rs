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

mod pages {
    use super::*;
    use rust_ui::prelude::*;
    use rust_ui::{Icon, Typography};
    include!("gallery/gallery_update.rs");
    include!("gallery/gallery_view.rs");
    include!("gallery/gallery_header.rs");
    include!("gallery/gallery_overlay_stage.rs");
    include!("gallery/gallery_components_page.rs");
    include!("gallery/gallery_forms_page.rs");
    include!("gallery/gallery_navigation_page.rs");
    include!("gallery/gallery_overlays_page.rs");
    include!("gallery/gallery_native_text_page.rs");
    include!("gallery/gallery_stub_page.rs");
    include!("gallery/gallery_typography_page.rs");
}

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
