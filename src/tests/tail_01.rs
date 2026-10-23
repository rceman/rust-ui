#[test]
fn automation_duplicate_aborts_and_semantic_id_survives_reconcile() {
    let mut rig = Rig::new(
        false,
        |_: &mut bool, _: Msg, _: &mut UpdateCtx<Msg>| {},
        |s: &bool, ui: &mut Ui<Msg>| {
            ui.label("first").automation_id("test.first");
            ui.label("second")
                .automation_id(if *s { "test.first" } else { "test.second" });
        },
    );
    rig.view().unwrap();
    let ids = rig.root_children();
    let original = ids[0];
    rig.view().unwrap();
    assert_eq!(rig.root_children()[0], original);
    assert_eq!(
        rig.rt.arena.get(original).unwrap().automation_id.as_deref(),
        Some("test.first")
    );
    rig.rt.state = true;
    assert!(matches!(
        rig.view(),
        Err(UiError::InvalidUi(UiDiagnostic::DuplicateAutomationId))
    ));
    assert_eq!(rig.root_children(), ids);
    assert_eq!(
        rig.rt.diagnostics.last(),
        Some(&UiDiagnostic::DuplicateAutomationId),
        "the rejection is recorded on the runtime"
    );
}

/// D04 — the ID invariants run in EVERY build: non-empty, <= 256 bytes,
/// no control characters. Each violation rejects atomically and records
/// the diagnostic on the runtime.
#[test]
fn automation_id_invariants_reject_in_all_builds() {
    let cases: Vec<String> = vec![String::new(), "a".repeat(300), "ctl\u{0007}char".into()];
    for bad in cases {
        let case = bad.clone();
        let mut rig = Rig::new(
            (),
            |_: &mut (), _: Msg, _: &mut UpdateCtx<Msg>| {},
            move |_: &(), ui: &mut Ui<Msg>| {
                ui.label("x").automation_id(&bad);
            },
        );
        assert!(
            matches!(
                rig.view(),
                Err(UiError::InvalidUi(UiDiagnostic::DuplicateAutomationId))
            ),
            "{case:?}"
        );
        assert_eq!(
            rig.rt.diagnostics.last(),
            Some(&UiDiagnostic::DuplicateAutomationId)
        );
        assert!(rig.root_children().is_empty(), "rejection is atomic");
    }
}

/// D04 — `named` never overwrites an ID the staged child already carries:
/// identical is idempotent, different is a staged conflict whose tx
/// diagnostic rejects the whole view before any commit.
#[test]
fn named_id_conflict_is_diagnostic_not_overwrite() {
    let mut rig = Rig::new(
        (),
        |_: &mut (), _: Msg, _: &mut UpdateCtx<Msg>| {},
        |_: &(), ui: &mut Ui<Msg>| {
            ui.named("same", |ui| {
                ui.label("x").automation_id("same");
            });
        },
    );
    rig.view().unwrap();
    assert!(
        !rig.rt
            .diagnostics
            .contains(&UiDiagnostic::DuplicateAutomationId),
        "identical inner+outer ID must not flag a conflict"
    );
    let mut rig2 = Rig::new(
        (),
        |_: &mut (), _: Msg, _: &mut UpdateCtx<Msg>| {},
        |_: &(), ui: &mut Ui<Msg>| {
            ui.named("outer", |ui| {
                ui.label("x").automation_id("inner");
            });
        },
    );
    assert!(matches!(
        rig2.view(),
        Err(UiError::InvalidUi(UiDiagnostic::DuplicateAutomationId))
    ));
    assert!(
        rig2.rt
            .diagnostics
            .contains(&UiDiagnostic::DuplicateAutomationId)
    );
    assert!(
        rig2.root_children().is_empty(),
        "conflict rejects the whole staged transaction"
    );
}

/// D04 — a view rejected for an ID violation leaves the committed arena
/// AND mounted peers exactly as they were: the check runs before any
/// commit/peer mutation.
#[test]
fn automation_rejection_leaves_arena_and_peers_untouched() {
    let mut rig = Rig::new(
        (false, TextValue::new("initial")),
        |_: &mut (bool, TextValue), _: Msg, _: &mut UpdateCtx<Msg>| {},
        |s: &(bool, TextValue), ui: &mut Ui<Msg>| {
            ui.text_input(&s.1).automation_id("ed");
            ui.label("l").automation_id(if s.0 { "ed" } else { "lab" });
        },
    );
    rig.view().unwrap();
    let ids = rig.root_children();
    let ed = ids[0];
    let peer = rig.peer_id(ed).expect("editor peer mounted");
    let ops = rig.peer_ops(peer).len();
    rig.rt.state.0 = true;
    assert!(matches!(
        rig.view(),
        Err(UiError::InvalidUi(UiDiagnostic::DuplicateAutomationId))
    ));
    assert_eq!(rig.root_children(), ids, "arena unchanged");
    assert_eq!(rig.peer_id(ed), Some(peer), "same peer still mounted");
    assert_eq!(rig.peer_ops(peer).len(), ops, "no peer mutation");
    assert_eq!(
        rig.rt.diagnostics.last(),
        Some(&UiDiagnostic::DuplicateAutomationId)
    );
}

#[cfg(windows)]
#[test]
fn devctl_automation_id_reaches_uia_and_refreshes_without_replacement() {
    use crate::platform::win32::uia::{ChildBuild, UiaRoot};
    use windows::Win32::{Foundation::HWND, UI::Accessibility::*};
    use windows::core::Interface;
    let root = UiaRoot::new(HWND::default(), "ids").unwrap();
    let node = crate::NodeId {
        slot: 0,
        generation: 1,
    };
    let make = |id: &str| ChildBuild {
        automation_id: id.into(),
        id: node,
        name: "button".into(),
        ct: UIA_ButtonControlTypeId,
        localized: "Button",
        rect: UiaRect {
            left: 0.,
            top: 0.,
            width: 32.,
            height: 32.,
        },
        actionable: true,
        enabled: true,
        offscreen: false,
        selected: None,
        checked: None,
        expanded: None,
        peer_node: false,
        native: None,
    };
    root.rebuild(vec![make("sample.button")]).unwrap();
    let fragment: IRawElementProviderFragment = root.provider().cast().unwrap();
    let child = unsafe { fragment.Navigate(NavigateDirection_FirstChild) }.unwrap();
    let simple: IRawElementProviderSimple = child.cast().unwrap();
    let get = || {
        let v = unsafe { simple.GetPropertyValue(UIA_AutomationIdPropertyId) }.unwrap();
        uia_variant_string(&v)
    };
    assert_eq!(get(), "sample.button");
    root.rebuild(vec![make("sample.renamed")]).unwrap();
    assert_eq!(get(), "sample.renamed");
    root.close();
}
