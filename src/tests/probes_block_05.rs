    {
        let sink = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut peer = crate::platform::win32::WindowlessPeer::create(
            11,
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
            "State",
            crate::text::TextRevision::mint(),
            binding,
        )
        .expect("initialize");
        peer.apply_bounds(island_dip, ScaleFactor(1.0)).unwrap();
        // focus -> UI-active — the precondition is ASSERTED, not assumed
        peer.send(0x0007 /*WM_SETFOCUS*/, 0, 0).unwrap();
        assert_eq!(
            peer.activation_state(),
            crate::platform::win32::text::Activation::Ui,
            "peer must be UI-active before the relatch leg"
        );
        for c in " XY".encode_utf16() {
            peer.send(0x0102 /*WM_CHAR*/, c as usize, 0).unwrap();
        }
        // BACKWARD selection established through TOM — the same
        // directional API the restore path uses (SetStart/SetEnd collapse
        // is precisely why sorted-extent + active-end restore exists).
        let text_before = peer.text().expect("text before");
        let len = text_before.encode_utf16().count();
        assert!(len >= 4, "precondition: enough text to select");
        peer.set_backward_selection_utf16(len - 1, len - 3)
            .expect("establish backward selection");
        let (anchor, focus) = peer.selection_utf16().expect("sel before");
        // NON-VACUOUS PRECONDITIONS — the test proves nothing unless the
        // backward selection and undo history genuinely exist
        assert!(
            focus < anchor,
            "precondition: backward selection (anchor={anchor} focus={focus}) not established"
        );
        assert_ne!(anchor, focus, "precondition: selection must be nonempty");
        let can_undo = peer
            .send(0x00C6 /*EM_CANUNDO*/, 0, 0)
            .map(|s| s.lr != 0)
            .unwrap_or(false);
        assert!(can_undo, "precondition: undo history must exist");

        peer.apply_bounds(island_dip, ScaleFactor(1.5)).unwrap();

        assert_eq!(peer.text().unwrap(), text_before, "text lost on relatch");
        let (a2, f2) = peer.selection_utf16().unwrap();
        assert_eq!(
            (a2, f2),
            (anchor, focus),
            "directional selection/extent lost on relatch"
        );
        assert!(f2 < a2, "direction must still be backward");
        assert_eq!(
            peer.activation_state(),
            crate::platform::win32::text::Activation::Ui,
            "UI activation not restored"
        );
        let can_undo_after = peer.send(0x00C6, 0, 0).map(|s| s.lr != 0).unwrap_or(false);
        assert!(can_undo_after, "undo history lost across relatch");
        // a fresh measurement must produce a fresh result (not stale state)
        let (_, h2) = peer.natural_size(400.0).expect("fresh natural_size");
        assert!(h2 > 0.0, "fresh measurement returned empty extent");
        // multiline growth: Enter arrives as VK_RETURN keydown (plain-text
        // mode filters bare CR chars)
        for _ in 0..12 {
            peer.send(0x0100, 0x0D /*VK_RETURN*/ as usize, 0).unwrap();
            peer.send(0x0101, 0x0D, 0).unwrap();
            peer.send(0x0102, 'x' as usize, 0).unwrap();
        }
        let (_, grown) = peer.natural_size(300.0).expect("grown measure");
        assert!(grown > h2, "multiline growth not reflected in measurement");
    }
