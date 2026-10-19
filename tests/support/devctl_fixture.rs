//! Internal devctl protocol test fixture — the deterministic instrumented
//! target that `tests/devctl_windows.rs` launches via `rust-ui-devctl`.
//! Test infrastructure only: not a product example, not a UI acceptance
//! app, not a visual authority. The Shadcn Gallery acceptance application
//! is `examples/gallery.rs`.
//!
//! Self-gated on `all(windows, feature = "devtools")` so the Cargo bin
//! target always builds — the CARGO_BIN_EXE_ path reaches the integration
//! test deterministically on every configuration.
#[cfg(all(windows, feature = "devtools"))]
mod imp {
    use rust_ui::prelude::*;
    pub(super) struct State {
        pub count: u32,
        pub draft: TextValue,
    }
    pub(super) enum Msg {
        Click,
        Edit(TextEdit),
    }
    pub(super) fn run() -> UiResult {
        App::new(
            State {
                count: 0,
                draft: TextValue::new("Fixture text"),
            },
            |s: &mut State, m, _| match m {
                Msg::Click => s.count += 1,
                Msg::Edit(e) => {
                    let _ = s.draft.accept(e);
                }
            },
            |s, ui| {
                ui.named("probe.root", |ui| {
                    ui.column(Column::new().gap(Space::Md).padding(Space::Lg), |ui| {
                        ui.named("probe.surface", |ui| {
                            ui.surface(Surface::new().padding(Space::Md), |ui| {
                                ui.column(Column::new().gap(Space::Md), |ui| {
                                    ui.label(format!("Clicks: {}", s.count))
                                        .automation_id("probe.label");
                                    ui.button("Count")
                                        .automation_id("probe.button")
                                        .motion(None)
                                        .on_press(|| Msg::Click);
                                    ui.text_input(&s.draft)
                                        .automation_id("probe.input")
                                        .label("Fixture input")
                                        .on_edit(Msg::Edit);
                                });
                            })
                        });
                    })
                });
            },
        )
        .title("rust-ui devctl test fixture")
        .run()
    }
}

#[cfg(all(windows, feature = "devtools"))]
fn main() -> rust_ui::prelude::UiResult {
    imp::run()
}

#[cfg(not(all(windows, feature = "devtools")))]
fn main() {}
