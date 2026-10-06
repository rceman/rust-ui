use rust_ui::prelude::*;
struct State {
    count: u32,
    draft: TextValue,
}
enum Msg {
    Click,
    Edit(TextEdit),
}
fn main() -> UiResult {
    App::new(
        State {
            count: 0,
            draft: TextValue::new("Calibration text"),
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
                                    .label("Calibration input")
                                    .on_edit(Msg::Edit);
                            });
                        })
                    });
                })
            });
        },
    )
    .title("rust-ui devctl calibration")
    .run()
}
