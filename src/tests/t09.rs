#[test]
fn shadow_patch_is_atomic_set_remove() {
    use crate::style::*;
    let mut b = BoxStyle::new();
    assert_eq!(b.shadow, None);
    let sh = Shadow {
        color: Color::rgb(0, 0, 0),
        offset_x: crate::geom::Dp(0.0),
        offset_y: crate::geom::Dp(4.0),
        blur_sigma: crate::geom::Dp(8.0),
        spread: crate::geom::Dp(0.0),
    };
    b.patch(&BoxStylePatch {
        shadow: ShadowPatch::Set(sh),
        ..Default::default()
    });
    assert_eq!(b.shadow, Some(sh));
    // Unchanged never erases
    b.patch(&BoxStylePatch::default());
    assert_eq!(b.shadow, Some(sh));
    // Remove forces none
    b.patch(&BoxStylePatch {
        shadow: ShadowPatch::Remove,
        ..Default::default()
    });
    assert_eq!(b.shadow, None);
}

/// `ui.action` with decorative content stages cleanly; nested actionable
/// children are a structural diagnostic, never staged.
#[test]
fn action_stages_and_rejects_nested_actionable() {
    use crate::style::Action;
    struct S;
    let mut rig = Rig::new(
        S,
        |_: &mut S, _: Msg, _: &mut UpdateCtx<Msg>| {},
        |_: &S, ui: &mut Ui<Msg>| {
            ui.action(Action::new().label("tile"), |ui| {
                ui.label("decorative");
            })
            .on_press(|| Msg::Press);
        },
    );
    rig.view().unwrap();
    // the action node committed with its child
    let kinds: Vec<u8> = {
        let root = rig.rt.root;
        let rn = rig.rt.arena.get(root).unwrap();
        rn.children
            .iter()
            .map(|&s| {
                let g = rig.rt.arena.generation_of(s);
                rig.rt
                    .arena
                    .get(crate::NodeId {
                        slot: s,
                        generation: g,
                    })
                    .unwrap()
                    .kind_tag()
            })
            .collect()
    };
    assert_eq!(kinds, vec![crate::node::KIND_ACTION]);

    // nested button inside an action -> InvalidComposition abort
    let mut rig2 = Rig::new(
        S,
        |_: &mut S, _: Msg, _: &mut UpdateCtx<Msg>| {},
        |_: &S, ui: &mut Ui<Msg>| {
            ui.action(Action::new().label("bad"), |ui| {
                ui.button("nested").on_press(|| Msg::Press);
            })
            .on_press(|| Msg::Press);
        },
    );
    let r = rig2.view();
    assert!(
        matches!(
            r,
            Err(crate::UiError::InvalidUi(
                crate::UiDiagnostic::InvalidComposition
            ))
        ),
        "{r:?}"
    );
}

/// Patch removal on a later view pass restores recipe values — the node is
/// the SAME retained node (no remount) and the committed style is the new
/// declaration, not patch history.
#[test]
fn patch_removal_restores_recipe_without_remount() {
    use crate::style::*;
    struct S {
        patched: bool,
    }
    let mut rig = Rig::new(
        S { patched: true },
        |_: &mut S, _: Msg, _: &mut UpdateCtx<Msg>| {},
        |s: &S, ui: &mut Ui<Msg>| {
            let b = ui
                .button("send")
                .variant(crate::theme::ButtonVariant::Primary);
            if s.patched {
                b.style(
                    ButtonStylePatch::new()
                        .border_bottom_width(crate::geom::dp(2.0))
                        .border_bottom_color(Color::rgb(255, 0, 0)),
                )
                .on_press(|| Msg::Press);
            } else {
                b.on_press(|| Msg::Press);
            }
        },
    );
    rig.view().unwrap();
    let node_id = {
        let root = rig.rt.root;
        let rn = rig.rt.arena.get(root).unwrap();
        let slot = rn.children[0];
        crate::NodeId {
            slot,
            generation: rig.rt.arena.generation_of(slot),
        }
    };
    // pass 1: patch committed
    {
        let n = rig.rt.arena.get(node_id).unwrap();
        let NodeDataLike { patch: p } = button_style_of(n);
        assert_eq!(
            p.styles.base.box_style.border.bottom.width,
            Some(crate::geom::Dp(2.0))
        );
    }
    // pass 2: patch removed — same node, fresh declaration
    rig.rt.state.patched = false;
    rig.view().unwrap();
    let n2 = rig.rt.arena.get(node_id).unwrap();
    assert_eq!(button_style_of(n2).patch, Default::default());
}

struct NodeDataLike {
    patch: crate::style::ButtonStylePatch,
}

fn button_style_of(n: &crate::node::Node) -> NodeDataLike {
    match &n.data {
        crate::node::NodeData::Button { style, .. } => NodeDataLike { patch: *style },
        _ => panic!("expected button"),
    }
}

#[cfg(windows)]
#[test]
fn rid_safearray_roundtrip() {
    use windows::Win32::System::Ole::*;
    use windows::Win32::System::Variant::VT_I4;
    use windows::Win32::UI::Accessibility::UiaAppendRuntimeId;
    unsafe {
        eprintln!("UiaAppendRuntimeId = {UiaAppendRuntimeId}");
        let sa = SafeArrayCreateVector(VT_I4, 0, 3);
        let vals = [UiaAppendRuntimeId as i32, 7, 99];
        for (i, v) in vals.iter().enumerate() {
            let r =
                SafeArrayPutElement(sa, &(i as i32), v as *const i32 as *const core::ffi::c_void);
            eprintln!("put {i} -> {r:?}");
        }
        let lb = SafeArrayGetLBound(sa, 1).unwrap();
        let ub = SafeArrayGetUBound(sa, 1).unwrap();
        for i in lb..=ub {
            let mut out = 0i32;
            SafeArrayGetElement(sa, &i, &mut out as *mut i32 as *mut core::ffi::c_void).unwrap();
            eprintln!("rid[{i}] = {out}");
        }
    }
}

/// P6 lifecycle: a retained provider reference must stay live across
/// reorder/disable for the same NodeId, die on remove, stay dead after a
/// same-slot new-generation recreate, and everything dies on close.
#[cfg(windows)]
#[test]
fn uia_retained_provider_lifecycle() {
    use crate::platform::win32::uia::{ChildBuild, UiaRoot};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Accessibility::*;
    use windows::core::Interface;

    let root = UiaRoot::new(HWND::default(), "t").expect("root");
    let mk = |slot: u32, generation: u64, name: &str, enabled: bool| ChildBuild {
        automation_id: String::new(),
        id: crate::NodeId { slot, generation },
        name: name.into(),
        ct: UIA_ButtonControlTypeId,
        localized: "Button",
        rect: UiaRect {
            left: 0.0,
            top: 0.0,
            width: 10.0,
            height: 10.0,
        },
        actionable: true,
        enabled,
        offscreen: false,
        selected: None,
        checked: None,
        expanded: None,
        peer_node: false,
        native: None,
    };
    let simple_of = |f: &IRawElementProviderFragment| -> IRawElementProviderSimple {
        f.cast().expect("simple")
    };

    // mount A,B,C
    root.rebuild(vec![
        mk(0, 0, "a", true),
        mk(1, 0, "b", true),
        mk(2, 0, "c", true),
    ])
    .unwrap();
    let rf: IRawElementProviderFragment = root.provider().cast().unwrap();
    let b = unsafe {
        rf.Navigate(NavigateDirection_FirstChild)
            .unwrap()
            .Navigate(NavigateDirection_NextSibling)
            .unwrap()
    }; // slot 1
    assert_eq!(
        uia_variant_string(&unsafe { simple_of(&b).GetPropertyValue(UIA_NamePropertyId).unwrap() }),
        "b"
    );

    // reorder C,A,B — same NodeIds survive; retained B must still be live
    root.rebuild(vec![
        mk(2, 0, "c", true),
        mk(0, 0, "a", true),
        mk(1, 0, "b", true),
    ])
    .unwrap();
    assert!(unsafe { simple_of(&b).GetPropertyValue(UIA_NamePropertyId) }.is_ok());

    // disable B — the retained fragment must report enabled=false live
    root.rebuild(vec![
        mk(2, 0, "c", true),
        mk(0, 0, "a", true),
        mk(1, 0, "b", false),
    ])
    .unwrap();
    let en = unsafe {
        simple_of(&b)
            .GetPropertyValue(UIA_IsEnabledPropertyId)
            .unwrap()
    };
    assert!(unsafe { !en.Anonymous.Anonymous.Anonymous.boolVal.as_bool() });
    // disabled node cannot take focus
    assert!(
        unsafe { b.SetFocus() }.is_err(),
        "disabled must reject SetFocus"
    );

    // remove B — retained reference dies permanently
    root.rebuild(vec![mk(2, 0, "c", true), mk(0, 0, "a", true)])
        .unwrap();
    assert!(unsafe { simple_of(&b).GetPropertyValue(UIA_NamePropertyId) }.is_err());

    // same-slot new-generation recreate — old B ref must NOT resolve to
    // the replacement; the new provider answers
    root.rebuild(vec![
        mk(2, 0, "c", true),
        mk(0, 0, "a", true),
        mk(1, 1, "b2", true),
    ])
    .unwrap();
    assert!(unsafe { simple_of(&b).GetPropertyValue(UIA_NamePropertyId) }.is_err());
    let last = unsafe { rf.Navigate(NavigateDirection_LastChild).unwrap() };
    assert_eq!(
        uia_variant_string(&unsafe {
            simple_of(&last)
                .GetPropertyValue(UIA_NamePropertyId)
                .unwrap()
        }),
        "b2"
    );

    // close — everything dies
    root.close();
    assert!(unsafe { simple_of(&last).GetPropertyValue(UIA_NamePropertyId) }.is_err());
    assert!(unsafe { rf.Navigate(NavigateDirection_FirstChild) }.is_err());
}

/// P8: style preflight — non-finite/negative/invalid authored values are
/// rejected at commit BEFORE any native mutation.
#[test]
fn invalid_style_rejected_at_commit() {
    use crate::geom::Dp;
    let mut rig = Rig::new(
        (),
        |_: &mut (), _m: Msg, _u: &mut UpdateCtx<Msg>| {},
        |_: &(), ui| {
            let mut p = crate::style::ButtonStylePatch::new();
            p.styles.base.box_style.padding.left = Some(Dp(f32::NAN));
            ui.button("b").style(p);
        },
    );
    match rig.view() {
        Err(crate::UiError::InvalidUi(crate::UiDiagnostic::InvalidStyle)) => {}
        r => panic!("expected InvalidStyle, got {r:?}"),
    }
    // negative border width also rejects
    let mut rig2 = Rig::new(
        (),
        |_: &mut (), _m: Msg, _u: &mut UpdateCtx<Msg>| {},
        |_: &(), ui| {
            let mut p = crate::style::ButtonStylePatch::new();
            p.styles.base.box_style.border.top.width = Some(Dp(-1.0));
            ui.button("b").style(p);
        },
    );
    match rig2.view() {
        Err(crate::UiError::InvalidUi(crate::UiDiagnostic::InvalidStyle)) => {}
        r => panic!("expected InvalidStyle, got {r:?}"),
    }
    // zero/nonpositive exact text size rejects
    let mut rig3 = Rig::new(
        (),
        |_: &mut (), _m: Msg, _u: &mut UpdateCtx<Msg>| {},
        |_: &(), ui| {
            ui.label("x")
                .style(crate::style::TextStylePatch::new().size(Dp(0.0)));
        },
    );
    match rig3.view() {
        Err(crate::UiError::InvalidUi(crate::UiDiagnostic::InvalidStyle)) => {}
        r => panic!("expected InvalidStyle, got {r:?}"),
    }
}

/// P8: proportional radius normalization — adjacent pairs sharing an edge
/// scale by ONE factor so authored proportions survive; oversized pairs
/// never exceed the edge.
#[test]
fn radii_normalize_proportionally() {
    use crate::geom::Dp;
    use crate::style::CornerRadii;
    // tl=40 tr=80 on a 60-wide rect — pair sum 120 > 60 -> factor 0.5
    // applied to BOTH (and every other pair's factor too)
    let r = CornerRadii {
        top_left: Dp(40.0),
        top_right: Dp(80.0),
        bottom_right: Dp(0.0),
        bottom_left: Dp(0.0),
    };
    let n = r.normalized(60.0, 100.0);
    assert!((n.top_left.0 - 20.0).abs() < 1e-4);
    assert!((n.top_right.0 - 40.0).abs() < 1e-4);
    // ratio preserved
    assert!((n.top_right.0 / n.top_left.0 - 2.0).abs() < 1e-4);
    // within-bounds radii pass through untouched
    let small = CornerRadii::all(Dp(4.0));
    assert_eq!(small.normalized(100.0, 100.0), small);
}

/// P8: repeated `.style()` / state setters merge fieldwise — a later call
/// must not erase earlier `Some` fields.
#[test]
fn repeated_style_setters_merge_fieldwise() {
    use crate::geom::Dp;
    use crate::style::*;
    // ButtonStylePatch hover: two calls merge
    let p = ButtonStylePatch::new()
        .hover(VisualStylePatch {
            box_style: BoxStylePatch {
                background: Some(Color::rgb(1, 1, 1)),
                ..Default::default()
            },
            ..Default::default()
        })
        .hover(VisualStylePatch {
            text_style: TextStylePatch {
                foreground: Some(Color::rgb(2, 2, 2)),
                ..Default::default()
            },
            ..Default::default()
        });
    let h = p.styles.hover.unwrap();
    assert_eq!(h.box_style.background, Some(Color::rgb(1, 1, 1)));
    assert_eq!(h.text_style.foreground, Some(Color::rgb(2, 2, 2)));
    // ActionStyle state setters merge
    let a = ActionStyle::new(BoxStyle::new())
        .hover(BoxStylePatch {
            padding: crate::style::InsetsPatch {
                left: Some(Dp(3.0)),
                ..Default::default()
            },
            ..Default::default()
        })
        .hover(BoxStylePatch {
            background: Some(Color::rgb(9, 9, 9)),
            ..Default::default()
        });
    let h = a.hover.unwrap();
    assert_eq!(h.padding.left, Some(Dp(3.0)));
    assert_eq!(h.background, Some(Color::rgb(9, 9, 9)));
    // resolved action keeps BOTH merged fields
    let r = a.resolve(false, false, true, false);
    assert_eq!(r.padding.left, Dp(3.0));
    assert_eq!(r.background, Color::rgb(9, 9, 9));
}

/// P8: dirty classification compares RESOLVED output — a patch whose net
/// effect equals the current resolved style produces zero dirty work;
/// shadow-only changes are paint, never layout.
#[test]
fn resolved_dirty_classification() {
    use crate::geom::Dp;
    use crate::node::NodeData;
    use crate::style::*;
    const LAYOUT: u8 = 0b0000_0001;
    const PAINT: u8 = 0b0000_0010;
    let surface = |patch: BoxStylePatch| NodeData::Container {
        kind: crate::node::KIND_SURFACE,
        props: crate::node::ContainerProps {
            gap: None,
            align: None,
            justify: None,
            padding: None,
            patch,
            full: None,
            card_size: None,
            scroll: Dp::default(),
        },
    };
    // same authored descriptor -> no dirty
    let p = BoxStylePatch::new();
    assert_eq!(
        crate::node::dirty_diff(&surface(p), &surface(p), false, None),
        0
    );
    // a patch authoring the SAME resolved value as the recipe -> no dirty
    let mut same = BoxStylePatch::new();
    same.padding.top = Some(Dp(12.0));
    same.padding.right = Some(Dp(12.0));
    same.padding.bottom = Some(Dp(12.0));
    same.padding.left = Some(Dp(12.0));
    assert_eq!(
        crate::node::dirty_diff(&surface(p), &surface(same), false, None),
        0,
        "patch resolving to identical output must be a no-op"
    );
    // shadow-only change -> PAINT, never LAYOUT
    let mut sh = BoxStylePatch::new();
    sh.shadow = ShadowPatch::Set(Shadow {
        color: Color::rgb(0, 0, 0),
        offset_x: Dp(0.0),
        offset_y: Dp(2.0),
        blur_sigma: Dp(4.0),
        spread: Dp(0.0),
    });
    let d = crate::node::dirty_diff(&surface(p), &surface(sh), false, None);
    assert_eq!(d & PAINT, PAINT);
    assert_eq!(d & LAYOUT, 0, "shadow-only must not request layout");
    // border WIDTH change -> LAYOUT; border COLOR change -> PAINT only
    let mut bw = BoxStylePatch::new();
    bw.border.top.width = Some(Dp(3.0));
    assert_eq!(
        crate::node::dirty_diff(&surface(p), &surface(bw), false, None),
        LAYOUT | PAINT
    );
    let mut bc = BoxStylePatch::new();
    bc.border.top.color = Some(Color::rgb(9, 9, 9));
    assert_eq!(
        crate::node::dirty_diff(&surface(p), &surface(bc), false, None),
        PAINT,
        "color-only border change is paint-only"
    );
    // a forced-colors resolver stub — the semantic mapping under test is
    // that enforcement rewrites roles to the resolver's concrete answer
    let forced_stub = |_slot: crate::style::SystemColor| [0.11, 0.11, 0.11, 1.0];
    // forced-colors suppresses a set shadow -> resolved equal -> no work
    assert_eq!(
        crate::node::dirty_diff(&surface(p), &surface(sh), false, Some(&forced_stub)),
        0,
        "shadow change under forced-colors resolves to identical output"
    );
    // F08: two DIFFERENT authored roles resolving to the same concrete
    // RGBA under the live theme are a strict no-op — authored identity
    // alone must not force work. Card and Background both resolve to
    // #FFFFFF light; under dark they diverge (#171717 vs #0A0A0A) — the
    // resolved comparison must follow the theme.
    {
        let mut rc = BoxStylePatch::new();
        rc.background = Some(crate::style::Color::Role(crate::theme::ColorRole::Card));
        let mut lit = BoxStylePatch::new();
        lit.background = Some(crate::style::Color::Role(
            crate::theme::ColorRole::Background,
        ));
        assert_eq!(
            crate::node::dirty_diff(&surface(rc), &surface(lit), false, None),
            0,
            "two roles resolving to identical concrete RGBA -> zero work"
        );
        let d = crate::node::dirty_diff(&surface(rc), &surface(lit), true, None);
        assert_eq!(
            d & PAINT,
            PAINT,
            "theme flip makes the same authored pair diverge -> paint"
        );
    }
}
