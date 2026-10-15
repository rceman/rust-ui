//! D04 — automation-ID invariants are enforced through the PUBLIC
//! `App::run` entry point, in a crate compiled WITHOUT cfg(test) and
//! WITHOUT the devtools feature. The view stages a real window, the
//! duplicate ID rejects before any commit, and the typed error surfaces
//! out of the run loop (the first WndProc turn records it; the next safe
//! point tears the window down and returns it).
#![cfg(windows)]

use rust_ui::prelude::*;

enum Never {}

fn update(_: &mut (), _: Never, _: &mut UpdateCtx<Never>) {}

#[test]
fn duplicate_automation_id_rejected_by_public_run() {
    let app = App::new((), update, |_: &(), ui: &mut Ui<Never>| {
        ui.label("a").automation_id("dup");
        ui.label("b").automation_id("dup");
    });
    let r = app.run();
    assert!(
        matches!(
            r,
            Err(UiError::InvalidUi(UiDiagnostic::DuplicateAutomationId))
        ),
        "{r:?}"
    );
}

#[test]
fn invalid_automation_id_rejected_by_public_run() {
    let app = App::new((), update, |_: &(), ui: &mut Ui<Never>| {
        ui.label("a").automation_id("");
    });
    let r = app.run();
    assert!(
        matches!(
            r,
            Err(UiError::InvalidUi(UiDiagnostic::DuplicateAutomationId))
        ),
        "{r:?}"
    );
}
