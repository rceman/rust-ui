    {
        use crate::platform::win32::uia::{ChildBuild, UiaRoot};
        use windows::Win32::UI::Accessibility::*;
        let sink = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut peer = crate::platform::win32::WindowlessPeer::create(
            21,
            &lib,
            hwnd,
            ScaleFactor(1.0),
            &cfg(),
            sink,
            std::sync::Arc::new(std::sync::Mutex::new(
                crate::platform::win32::text::TimerPool::new(),
            )),
        )
        .expect("peer create");
        let binding = crate::text::BindingToken::mint();
        crate::node::TextPeer::initialize(
            &mut peer,
            "abc",
            crate::text::TextRevision::mint(),
            binding,
        )
        .expect("initialize");
        peer.apply_bounds(island_dip, ScaleFactor(1.0)).unwrap();
        let node = crate::NodeId {
            slot: 31,
            generation: 7,
        };
        crate::node::TextPeer::attach(&mut peer, node);

        // the real msftedit provider for this peer
        let iid = peer.windowless_acc_iid().expect("acc iid");
        let root = UiaRoot::new(HWND::default(), "t").expect("root");
        let acc: IRicheditWindowlessAccessibility =
            unsafe { windows::core::Interface::from_raw(peer.query_iid(&iid).expect("qi")) };
        let site = root.site(node).expect("site");
        let prov = unsafe { acc.CreateProvider(&site) }.expect("provider");
        let frag: IRawElementProviderFragment = prov.cast().expect("frag");
        root.rebuild(vec![ChildBuild {
            automation_id: String::new(),
            id: node,
            name: "ed".into(),
            ct: UIA_EditControlTypeId,
            localized: "Edit",
            rect: UiaRect {
                left: 0.0,
                top: 0.0,
                width: 10.0,
                height: 10.0,
            },
            actionable: false,
            enabled: true,
            offscreen: false,
            selected: None,
            checked: None,
            expanded: None,
            peer_node: true,
            native: Some(frag.clone()),
        }])
        .expect("rebuild");

        // retain every escape path: the text pattern, a range from it,
        // the enclosing element, and a children array entry
        let rf: IRawElementProviderFragment = root.provider().cast().unwrap();
        let ed = unsafe { rf.Navigate(NavigateDirection_FirstChild).unwrap() };
        let es: IRawElementProviderSimple = ed.cast().unwrap();
        let text_pat: ITextProvider = unsafe {
            es.GetPatternProvider(UIA_TextPatternId)
                .unwrap()
                .cast()
                .unwrap()
        };
        let range = unsafe { text_pat.DocumentRange().unwrap() };
        let enclosing = unsafe { range.GetEnclosingElement().unwrap() };
        let kids = unsafe { range.GetChildren().unwrap() }; // may be null/empty
        // NONEMPTY range array — the selection always has >=1 range;
        // extract one element and retain it past teardown
        let sel_sa = unsafe { text_pat.GetSelection().unwrap() };
        assert!(!sel_sa.is_null(), "GetSelection must return an array");
        let (lo, hi) = unsafe {
            (
                windows::Win32::System::Ole::SafeArrayGetLBound(sel_sa, 1).unwrap(),
                windows::Win32::System::Ole::SafeArrayGetUBound(sel_sa, 1).unwrap(),
            )
        };
        assert!(hi >= lo, "selection array must be nonempty");
        let mut praw: *mut core::ffi::c_void = std::ptr::null_mut();
        unsafe {
            windows::Win32::System::Ole::SafeArrayGetElement(
                sel_sa,
                &lo,
                &mut praw as *mut _ as *mut core::ffi::c_void,
            )
            .unwrap()
        };
        assert!(!praw.is_null());
        let sel_range: ITextRangeProvider = unsafe { windows_core::Interface::from_raw(praw) };
        unsafe {
            windows::Win32::System::Ole::SafeArrayDestroy(sel_sa);
        }
        // a retained CHILD provider — GetChildren may be empty for plain
        // text; when nonempty extract+retain a real element
        let mut retained_child: Option<IRawElementProviderSimple> = None;
        if !kids.is_null() {
            let (klo, khi) = unsafe {
                (
                    windows::Win32::System::Ole::SafeArrayGetLBound(kids, 1).unwrap(),
                    windows::Win32::System::Ole::SafeArrayGetUBound(kids, 1).unwrap(),
                )
            };
            if khi >= klo {
                let mut kraw: *mut core::ffi::c_void = std::ptr::null_mut();
                unsafe {
                    windows::Win32::System::Ole::SafeArrayGetElement(
                        kids,
                        &klo,
                        &mut kraw as *mut _ as *mut core::ffi::c_void,
                    )
                    .unwrap()
                };
                if !kraw.is_null() {
                    retained_child = Some(unsafe { windows_core::Interface::from_raw(kraw) });
                }
            }
        }
        // live sanity: the range reads text while the node is live
        let txt = unsafe { range.GetText(-1).unwrap() };
        assert!(txt.len() >= 3, "range must read committed text while live");
        // a live retained range/array element answers while live
        let _ = unsafe { sel_range.GetText(-1).unwrap() };
        if let Some(c) = &retained_child {
            let _ = unsafe { c.GetPropertyValue(UIA_NamePropertyId) }; // may err legitimately
        }

        // REQUIRED NONEMPTY PROVIDER ARRAY — drive fence_provider_array
        // with a real array holding genuine native provider references
        // (the peer's own provider + its enclosing element). The fixture
        // FAILS unless the array is nonempty AND at least one element is
        // extracted — conditional extraction is the vacuous proof this
        // corrects.
        let prov_simple: IRawElementProviderSimple = frag.cast().expect("prov simple");
        let enc_simple: IRawElementProviderSimple = enclosing.cast().expect("enc simple");
        let in_sa = unsafe {
            windows::Win32::System::Ole::SafeArrayCreateVector(
                windows::Win32::System::Variant::VT_UNKNOWN,
                0,
                2,
            )
        };
        assert!(!in_sa.is_null(), "input provider array must allocate");
        for (i, p) in [&prov_simple, &enc_simple].iter().enumerate() {
            let idx = i as i32;
            unsafe {
                windows::Win32::System::Ole::SafeArrayPutElement(
                    in_sa,
                    &idx,
                    windows_core::Interface::as_raw(*p) as *const core::ffi::c_void,
                )
                .expect("put native provider element");
            }
        }
        let fenced_sa =
            unsafe { crate::platform::win32::uia::fence_provider_array(in_sa, &root.snap, node) }
                .expect("fence_provider_array must succeed");
        assert!(!fenced_sa.is_null(), "fenced provider array required");
        let (flo, fhi) = unsafe {
            (
                windows::Win32::System::Ole::SafeArrayGetLBound(fenced_sa, 1).unwrap(),
                windows::Win32::System::Ole::SafeArrayGetUBound(fenced_sa, 1).unwrap(),
            )
        };
        assert_eq!(fhi - flo + 1, 2, "fenced array must carry both elements");
        // extract and RETAIN a real child provider from the array
        let mut craw: *mut core::ffi::c_void = std::ptr::null_mut();
        unsafe {
            windows::Win32::System::Ole::SafeArrayGetElement(
                fenced_sa,
                &flo,
                &mut craw as *mut _ as *mut core::ffi::c_void,
            )
            .expect("child extraction must not be skipped");
        }
        assert!(!craw.is_null(), "extracted child element must exist");
        let arr_child: IRawElementProviderSimple =
            unsafe { windows_core::Interface::from_raw(craw) };
        unsafe {
            windows::Win32::System::Ole::SafeArrayDestroy(fenced_sa);
        }
        // the extracted provider answers while its node is live
        assert!(
            unsafe { arr_child.GetPropertyValue(UIA_ControlTypePropertyId) }.is_ok(),
            "extracted provider must answer while live"
        );

        // REMOVE the node — every retained native object must die
        root.rebuild(vec![]).unwrap();
        assert!(
            unsafe { es.GetPropertyValue(UIA_NamePropertyId) }.is_err(),
            "escaped fragment must die on removal"
        );
        assert!(
            unsafe { range.GetText(-1) }.is_err(),
            "retained range must die on removal"
        );
        assert!(
            unsafe { text_pat.DocumentRange() }.is_err(),
            "retained pattern must die on removal"
        );
        assert!(
            unsafe { enclosing.GetPropertyValue(UIA_NamePropertyId) }.is_err()
                && unsafe { enclosing.GetPatternProvider(UIA_TextPatternId) }.is_err(),
            "escaped enclosing provider must fail on BOTH supported calls"
        );
        assert!(
            unsafe { sel_range.GetText(-1) }.is_err(),
            "a range extracted from a retained selection array must die"
        );
        assert!(
            unsafe { arr_child.GetPropertyValue(UIA_ControlTypePropertyId) }.is_err(),
            "a provider extracted from a nonempty array must die on removal"
        );
        if let Some(c) = &retained_child {
            assert!(
                unsafe { c.GetPropertyValue(UIA_NamePropertyId) }.is_err(),
                "a retained child provider must die on removal"
            );
        }
        if !kids.is_null() {
            unsafe {
                windows::Win32::System::Ole::SafeArrayDestroy(kids);
            }
        }
        // RECREATE same slot, NEW generation — retained refs stay dead
        let node2 = crate::NodeId {
            slot: 31,
            generation: 8,
        };
        root.rebuild(vec![ChildBuild {
            automation_id: String::new(),
            id: node2,
            name: "ed2".into(),
            ct: UIA_EditControlTypeId,
            localized: "Edit",
            rect: UiaRect {
                left: 0.0,
                top: 0.0,
                width: 10.0,
                height: 10.0,
            },
            actionable: false,
            enabled: true,
            offscreen: false,
            selected: None,
            checked: None,
            expanded: None,
            peer_node: true,
            native: None,
        }])
        .unwrap();
        assert!(
            unsafe { range.GetText(-1) }.is_err(),
            "stale gen still fenced"
        );

        // CLOSE — nothing answers
        root.close();
        assert!(unsafe { es.GetPropertyValue(UIA_NamePropertyId) }.is_err());
        drop(acc);
    }
