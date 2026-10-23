#[cfg(windows)]
fn uia_variant_string(v: &windows::Win32::System::Variant::VARIANT) -> String {
    // VARIANT(vt=VT_BSTR) -> String
    unsafe {
        let inner = &*(v as *const _ as *const windows::Win32::System::Variant::VARIANT);
        let _ = inner;
        // bstrVal lives in the nested anonymous union
        let bstr: &windows::core::BSTR =
            std::mem::transmute(&v.Anonymous.Anonymous.Anonymous.bstrVal);
        bstr.to_string()
    }
}

/// UIA: painted fragments must survive rebuild — previously `rebuild` kept
/// only `Weak`s and dropped the strong refs at loop end, so every painted
/// child died instantly and `Navigate` could never produce it.
#[cfg(windows)]
#[test]
fn uia_children_survive_rebuild() {
    use crate::platform::win32::uia::{ChildBuild, UiaRoot};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Accessibility::*;
    use windows::core::Interface;

    let root = UiaRoot::new(HWND::default(), "t").expect("root");
    let mk = |slot: u32, name: &str, ct: UIA_CONTROLTYPE_ID, loc: &'static str, act| ChildBuild {
        automation_id: String::new(),
        id: crate::NodeId {
            slot,
            generation: 0,
        },
        name: name.into(),
        ct,
        localized: loc,
        rect: UiaRect {
            left: 0.0,
            top: 0.0,
            width: 10.0,
            height: 10.0,
        },
        actionable: act,
        enabled: true,
        offscreen: false,
        selected: None,
        checked: None,
        expanded: None,
        peer_node: false,
        native: None,
    };
    root.rebuild(vec![
        mk(0, "lbl", UIA_TextControlTypeId, "Text", false),
        mk(1, "btn", UIA_ButtonControlTypeId, "Button", true),
    ])
    .expect("rebuild");

    let rf: IRawElementProviderFragment = root.provider().cast().expect("root frag");
    // first child = the label — upgrade through the weak must succeed
    let first = unsafe {
        rf.Navigate(NavigateDirection_FirstChild)
            .expect("first child")
    };
    // sibling navigation label -> button
    let second = unsafe {
        first
            .Navigate(NavigateDirection_NextSibling)
            .expect("next sibling")
    };
    let simple: IRawElementProviderSimple = second.cast().expect("simple");
    unsafe {
        let name = simple.GetPropertyValue(UIA_NamePropertyId).expect("name");
        assert_eq!(uia_variant_string(&name), "btn");
        let ct = simple
            .GetPropertyValue(UIA_ControlTypePropertyId)
            .expect("ct");
        // vt = VT_I4; lVal inside the union
        let lval = ct.Anonymous.Anonymous.Anonymous.lVal;
        assert_eq!(lval, UIA_ButtonControlTypeId.0);
        // actionable -> InvokePattern present; label -> absent
        simple
            .GetPatternProvider(UIA_InvokePatternId)
            .expect("button must expose invoke");
        let lbl: IRawElementProviderSimple = first.cast().expect("lbl simple");
        assert!(
            lbl.GetPatternProvider(UIA_InvokePatternId).is_err(),
            "label must not expose invoke"
        );
    }
    // focus tracking: set_focus marks the owner; GetFocus resolves it
    root.set_focus(Some(crate::NodeId {
        slot: 1,
        generation: 0,
    }));
    let rroot: IRawElementProviderFragmentRoot = root.provider().cast().expect("rroot");
    unsafe {
        let f = rroot.GetFocus().expect("focus must resolve");
        let fs: IRawElementProviderSimple = f.cast().expect("fs");
        assert_eq!(
            uia_variant_string(&fs.GetPropertyValue(UIA_NamePropertyId).unwrap()),
            "btn"
        );
    }
}

/// `ElementProviderFromPoint` must hit-test against the SNAPSHOT rect —
/// native editor providers report an unusable BoundingRectangle
/// (msftedit returns Infinity on windowless sites), so the root carries
/// the authoritative rect per child.
#[cfg(windows)]
#[test]
fn uia_element_from_point_uses_snapshot_rect() {
    use crate::platform::win32::uia::{ChildBuild, UiaRoot};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Accessibility::*;
    use windows::core::Interface;

    let root = UiaRoot::new(HWND::default(), "t").expect("root");
    let mk = |slot: u32, name: &str, rect: UiaRect, native: Option<IRawElementProviderFragment>| {
        ChildBuild {
            automation_id: String::new(),
            id: crate::NodeId {
                slot,
                generation: 0,
            },
            name: name.into(),
            ct: if native.is_some() {
                UIA_EditControlTypeId
            } else {
                UIA_ButtonControlTypeId
            },
            localized: "t",
            rect,
            actionable: native.is_none(),
            enabled: true,
            offscreen: false,
            selected: None,
            checked: None,
            expanded: None,
            peer_node: native.is_some(),
            native,
        }
    };
    // a real msftedit provider cannot be built in-process without the
    // peer machinery — emulate "broken native rect" with a painted frag
    // whose own rect disagrees: a native slot's rect is what's used
    let native_like = None;
    root.rebuild(vec![
        mk(
            0,
            "btn",
            UiaRect {
                left: 10.0,
                top: 10.0,
                width: 50.0,
                height: 20.0,
            },
            native_like.clone(),
        ),
        mk(
            1,
            "ed",
            UiaRect {
                left: 10.0,
                top: 40.0,
                width: 200.0,
                height: 30.0,
            },
            None,
        ),
    ])
    .expect("rebuild");
    let rroot: IRawElementProviderFragmentRoot = root.provider().cast().expect("rroot");
    unsafe {
        // inside the button's rect
        let hit = rroot
            .ElementProviderFromPoint(20.0, 15.0)
            .expect("hit in btn");
        let s: IRawElementProviderSimple = hit.cast().expect("simple");
        assert_eq!(
            uia_variant_string(&s.GetPropertyValue(UIA_NamePropertyId).unwrap()),
            "btn"
        );
        // inside the editor's rect — second child wins (topmost order)
        let hit2 = rroot
            .ElementProviderFromPoint(50.0, 55.0)
            .expect("hit in ed");
        let s2: IRawElementProviderSimple = hit2.cast().expect("simple");
        assert_eq!(
            uia_variant_string(&s2.GetPropertyValue(UIA_NamePropertyId).unwrap()),
            "ed"
        );
        // outside everything
        assert!(
            rroot.ElementProviderFromPoint(500.0, 500.0).is_err(),
            "a point outside all children must not fabricate a hit"
        );
    }
}

/// A disabled control must surface as non-enabled + non-focusable in UIA
/// — the assistive-tech contract that pairs with hit-test/dispatch gating.
#[cfg(windows)]
#[test]
fn uia_disabled_node_reports_not_enabled() {
    use crate::platform::win32::uia::{ChildBuild, UiaRoot};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Accessibility::*;
    use windows::core::Interface;

    let root = UiaRoot::new(HWND::default(), "t").expect("root");
    let mk = |generation: u64, enabled: bool| ChildBuild {
        automation_id: String::new(),
        id: crate::NodeId {
            slot: 0,
            generation,
        },
        name: "off".into(),
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
    root.rebuild(vec![mk(0, false)]).expect("rebuild");
    let rf: IRawElementProviderFragment = root.provider().cast().expect("root frag");
    let btn = unsafe { rf.Navigate(NavigateDirection_FirstChild).expect("child") };
    let s: IRawElementProviderSimple = btn.cast().expect("simple");
    unsafe {
        let en = s
            .GetPropertyValue(UIA_IsEnabledPropertyId)
            .expect("enabled");
        // VT_BOOL: VARIANT_TRUE is nonzero
        let b = en.Anonymous.Anonymous.Anonymous.boolVal.0;
        assert_eq!(b, 0, "disabled node must report IsEnabled=false");
        let f = s
            .GetPropertyValue(UIA_IsKeyboardFocusablePropertyId)
            .expect("focusable");
        assert_eq!(
            f.Anonymous.Anonymous.Anonymous.boolVal.0, 0,
            "disabled node must not be keyboard-focusable"
        );
    }
    // dead generation fences every accessor
    root.rebuild(vec![mk(1, true)]).expect("rebuild2");
    unsafe {
        assert!(
            s.GetPropertyValue(UIA_NamePropertyId).is_err(),
            "stale fragment must not answer after remint"
        );
    }
}

/// The spike scenario: Send A streams chunks -> Stop A -> Send B -> a late
/// in-flight A chunk must not contaminate B; Stop must not block resend.
#[test]
fn send_stop_resend_late_chunks_fenced() {
    struct S {
        got: Vec<u32>,
        busy: bool,
    }
    // channel so the test can fire a stale envelope later — a "late A chunk"
    let late: Arc<Mutex<Option<crate::TaskSender<Msg>>>> = Arc::new(Mutex::new(None));
    let late2 = late.clone();
    let mut rig = Rig::new(
        S {
            got: vec![],
            busy: false,
        },
        move |s: &mut S, m: Msg, cx: &mut UpdateCtx<Msg>| match m {
            Msg::Kick => {
                if s.busy {
                    return;
                }
                s.busy = true;
                let l = late2.clone();
                let _ = cx.spawn("stream", move |job| {
                    *l.lock().unwrap() = Some(job.sender().clone());
                    async move {
                        let _ = job.send(Msg::Chunk(1)).await;
                        std::future::pending::<()>().await;
                    }
                });
            }
            Msg::Press => {
                cx.cancel("stream");
                s.busy = false;
            }
            Msg::Chunk(c) => s.got.push(c),
            _ => {}
        },
        |_: &S, _: &mut Ui<Msg>| {},
    );
    rig.view().unwrap();
    // Send A — task A registers and emits chunk 1
    rig.rt.push_msg(Msg::Kick);
    rig.pump().unwrap();
    rig.exec.poll();
    rig.pump().unwrap();
    assert_eq!(rig.rt.state.got, vec![1]);
    assert!(rig.rt.state.busy);
    // Stop A — busy clears; the resend path is legal again
    rig.rt.push_msg(Msg::Press);
    rig.pump().unwrap();
    assert!(!rig.rt.state.busy);
    let stale_sender = late.lock().unwrap().take().expect("A sender");
    // Send B — same key, fresh generation
    rig.rt.push_msg(Msg::Kick);
    rig.pump().unwrap();
    assert!(rig.rt.state.busy);
    rig.exec.poll(); // B sends chunk 1, parks
    rig.pump().unwrap();
    assert_eq!(rig.rt.state.got, vec![1, 1]);
    // late A chunk: the fenced token's send must fail, nothing contaminates
    let stale = stale_sender.clone();
    let rt = tokio_block(async move { stale.send(Msg::Chunk(99)).await });
    assert!(rt.is_err(), "fenced token send must fail, got {rt:?}");
    rig.pump().unwrap();
    assert_eq!(rig.rt.state.got, vec![1, 1]);
}

/// Minimal block-on for the test sender future — the send resolves
/// immediately on a fenced token.
fn tokio_block<F: std::future::Future>(fut: F) -> F::Output {
    use std::sync::Arc;
    use std::task::{Context, Poll, Wake};
    struct W;
    impl Wake for W {
        fn wake(self: Arc<Self>) {}
    }
    let waker = std::task::Waker::from(Arc::new(W));
    let mut cx = Context::from_waker(&waker);
    let mut fut = std::pin::pin!(fut);
    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(v) => return v,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

// ---------------------------------------------------------------------------
// styling — the documented surgical invariant + resolution ordering
// ---------------------------------------------------------------------------

#[test]
fn surgical_patch_changes_only_authored_fields() {
    use crate::style::*;
    use crate::theme::{ButtonVariant, ControlSize};
    // the mandatory case from STYLE_CUSTOMIZATION_MODEL.md
    let patch = ButtonStylePatch::new()
        .border_bottom_width(crate::geom::dp(2.0))
        .border_bottom_color(Color::rgb(255, 0, 0));
    for &dark in &[false, true] {
        for state in [
            StyleState::Normal,
            StyleState::Hover,
            StyleState::Pressed,
            StyleState::Disabled,
        ] {
            for &focus in &[false, true] {
                let base = resolve_button(
                    ButtonVariant::Primary,
                    ControlSize::Md,
                    &ButtonStylePatch::new(),
                    state,
                    focus,
                    dark,
                    false,
                    false,
                );
                let patched = resolve_button(
                    ButtonVariant::Primary,
                    ControlSize::Md,
                    &patch,
                    state,
                    focus,
                    dark,
                    false,
                    false,
                );
                // exactly two fields may differ
                let (a, b) = (base.box_style, patched.box_style);
                assert_eq!(
                    a.background, b.background,
                    "background must not change ({state:?} focus={focus} dark={dark})"
                );
                assert_eq!(a.radii, b.radii, "radii must not change");
                assert_eq!(a.padding, b.padding, "padding must not change");
                assert_eq!(a.shadow, b.shadow, "shadow must not change");
                assert_eq!(a.border.top, b.border.top, "top border unchanged");
                assert_eq!(a.border.left, b.border.left, "left border unchanged");
                assert_eq!(a.border.right, b.border.right, "right border unchanged");
                assert_eq!(b.border.bottom.width, crate::geom::Dp(2.0));
                assert_eq!(b.border.bottom.color, Color::rgb(255, 0, 0));
                assert_eq!(base.text_style, patched.text_style, "text style unchanged");
            }
        }
    }
}

#[test]
fn patch_merge_and_resolution_order() {
    use crate::style::*;
    use crate::theme::{ButtonVariant, ControlSize};
    // consumer base applies across ALL states (it sits over recipe state)
    let patch = ButtonStylePatch::new().background(Color::rgb(1, 2, 3));
    for state in [StyleState::Normal, StyleState::Hover, StyleState::Pressed] {
        let v = resolve_button(
            ButtonVariant::Primary,
            ControlSize::Md,
            &patch,
            state,
            false,
            false,
            false,
            false,
        );
        assert_eq!(v.box_style.background, Color::rgb(1, 2, 3), "{state:?}");
    }
    // consumer state patch lands on top of that state's resolved values
    let mut p = ButtonStylePatch::new().background(Color::rgb(1, 2, 3));
    let mut hover = VisualStylePatch::new();
    hover.box_style.background = Some(Color::rgb(9, 9, 9));
    p = p.hover(hover);
    let v = resolve_button(
        ButtonVariant::Primary,
        ControlSize::Md,
        &p,
        StyleState::Hover,
        false,
        false,
        false,
        false,
    );
    assert_eq!(v.box_style.background, Color::rgb(9, 9, 9));
    // normal keeps the consumer base
    let v = resolve_button(
        ButtonVariant::Primary,
        ControlSize::Md,
        &p,
        StyleState::Normal,
        false,
        false,
        false,
        false,
    );
    assert_eq!(v.box_style.background, Color::rgb(1, 2, 3));
    // focus overlay: recipe focus border + consumer focus patch on top
    let mut p2 = ButtonStylePatch::new();
    let mut fv = VisualStylePatch::new();
    fv.box_style.background = Some(Color::rgb(7, 7, 7));
    p2 = p2.focus_visible(fv);
    let v = resolve_button(
        ButtonVariant::Primary,
        ControlSize::Md,
        &p2,
        StyleState::Normal,
        true,
        false,
        false,
        false,
    );
    assert_eq!(v.box_style.background, Color::rgb(7, 7, 7));
    assert!(
        v.box_style.border.top.width.0 > 0.0,
        "recipe focus ring present"
    );
}
