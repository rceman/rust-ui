    /// F05 — `ime_end` ordering against a REAL msftedit peer + real
    /// Runtime: drain notifications, ACKNOWLEDGE already-queued commits,
    /// then reconcile only the remaining divergence.
    ///
    /// Branch A (suppress=false): the final commit's Change notification
    /// survived composition and lands at the end — it must be pumped into
    /// the shared mirror BEFORE reconcile, or reconcile emits a duplicate
    /// commit that splits native/shared revisions.
    /// Branch B (suppress=true): the notification drained while composing
    /// was still marked — suppressed as preedit — so reconcile must
    /// reconstruct the divergence.
    ///
    /// Both assert: native text == shared committed text, peer revision
    /// == mirror revision, a stale proposal conflicts, and the NEXT
    /// genuine native edit is accepted.
    #[test]
    fn ime_end_commit_acknowledgement_both_branches() {
        for suppress in [false, true] {
            let (mut be, node) = ime_backend("abc");
            let peer = be.peer_for(node).unwrap();
            // PRECONDITION: composition pinned to this peer — and it must
            // stay pinned even if focus moves mid-composition
            be.focus = Some(node);
            be.ime_start();
            assert_eq!(be.ime_owner.get(), Some(Some(node)));
            be.focus = Some(NodeId {
                slot: 99,
                generation: 0,
            });
            assert_eq!(
                be.ime_target(),
                Some(node),
                "composition owner stays pinned over a focus move"
            );
            // a stale proposal queued on the pre-commit base
            be.rt.state.v.replace("proposal");
            be.rt.review_for_test().unwrap();
            assert!(be.rt.state.v.pending().is_some());
            // the final native commit lands in the service
            peer.borrow().native_write_for_test("abc+tail").unwrap();
            peer.borrow().emit_change_for_test();
            if suppress {
                // branch B — the notification drains while composing is
                // STILL marked: suppressed as preedit, mirror stays stale
                be.service_peer_events().unwrap();
                assert_eq!(
                    &*be.rt.state.v.text(),
                    "abc",
                    "suppressed commit must not reach the mirror"
                );
            }
            // ACTION: composition ends
            be.ime_end().unwrap();
            // POSTCONDITION — native == shared, revisions agree
            let peer_now = be.peer_for(node).unwrap();
            assert_eq!(
                peer_now.borrow().text().unwrap(),
                "abc+tail",
                "suppress={suppress}: native text must equal the commit"
            );
            assert_eq!(
                &*be.rt.state.v.text(),
                "abc+tail",
                "suppress={suppress}: shared TextValue must equal native"
            );
            let (_t, mirror_rev) = be.rt.committed_editor(node).unwrap();
            assert_eq!(
                mirror_rev,
                peer_now.borrow().peer_rev(),
                "suppress={suppress}: native/shared revisions must agree"
            );
            // the stale-base proposal CONFLICTED, never overwrote the commit
            assert!(
                !be.rt.state.conflicts.is_empty(),
                "suppress={suppress}: proposal on a stale base must conflict"
            );
            // the NEXT genuine native edit is accepted — no revision wedge
            peer_now
                .borrow()
                .native_write_for_test("abc+tail+more")
                .unwrap();
            peer_now.borrow().emit_change_for_test();
            be.service_peer_events().unwrap();
            be.rt.pump().unwrap();
            assert_eq!(
                &*be.rt.state.v.text(),
                "abc+tail+more",
                "suppress={suppress}: next edit must accept (no divergence)"
            );
            drop(peer_now);
            drop(peer);
            be.shutdown();
        }
    }

    /// D05 audit — the OTHER `set_focus` boundaries (UIA action, Tab
    /// traversal) must drain peer events before their turn too: a Change
    /// notification pending when focus moves was previously routed at the
    /// turn TAIL, leaving the edit stranded past the boundary.
    #[test]
    fn non_devctl_focus_paths_drain_peer_events_before_turn() {
        use windows::Win32::UI::Input::KeyboardAndMouse::VK_TAB;
        use windows::Win32::UI::WindowsAndMessaging::WM_KEYDOWN;
        for path in ["uia", "tab"] {
            let (mut be, node) = ime_backend("abc");
            let peer = be.peer_for(node).unwrap();
            peer.borrow().native_write_for_test("abc+native").unwrap();
            peer.borrow().emit_change_for_test();
            match path {
                "uia" => be.uia_action(node.slot, node.generation, true).unwrap(),
                _ => be.key_msg(WM_KEYDOWN, VK_TAB.0 as usize, 0).unwrap(),
            }
            assert_eq!(
                &*be.rt.state.v.text(),
                "abc+native",
                "{path}: the pending edit must apply at this boundary"
            );
            assert_eq!(
                be.peer_ctx.sink.lock().unwrap().len(),
                0,
                "{path}: no sink item may survive the boundary"
            );
            assert!(
                be.rt.events.is_empty(),
                "{path}: no queued event may survive the boundary"
            );
            drop(peer);
            be.shutdown();
        }
    }

    /// D05 audit — a primary pointer-Down on a NON-peer interactive node
    /// moves framework focus off a peer (KILLFOCUS is delivered); pending
    /// peer host events must be routed before that boundary's turn.
    #[test]
    fn semantic_pointer_down_drains_peer_events_before_turn() {
        use crate::platform::win32::space::ClientPhysicalPoint;
        let (mut be, ed, btn) = mixed_backend("abc");
        be.set_focus(Some(ed)).unwrap();
        be.service_peer_events().unwrap();
        be.turn().unwrap();
        let peer = be.peer_for(ed).unwrap();
        peer.borrow().native_write_for_test("abc+native").unwrap();
        peer.borrow().emit_change_for_test();
        // a primary Down on the semantic button — KILLFOCUS goes to the peer
        let r = be.rects.get(&btn).copied().unwrap();
        let s = be.peer_ctx.scale.get();
        let px = ClientPhysicalPoint(crate::geom::PhysicalPoint {
            x: s.to_physical(r.x + r.width / 2.),
            y: s.to_physical(r.y + r.height / 2.),
        });
        be.pointer(
            crate::node::PointerPhase::Down,
            px,
            Some(crate::event::PointerButton::Primary),
            0x0001, // MK_LBUTTON
        )
        .unwrap();
        assert_eq!(be.focus, Some(btn), "Down moves focus to the button");
        assert_eq!(
            &*be.rt.state.v.text(),
            "abc+native",
            "the pending edit must apply at this boundary"
        );
        assert_eq!(be.peer_ctx.sink.lock().unwrap().len(), 0);
        assert!(be.rt.events.is_empty());
        drop(peer);
        be.shutdown();
    }

    /// D05 — the focus boundary drains peer host events BEFORE the turn:
    /// a Change notification pending when `focus` arrives (EN_CHANGE posted
    /// mid-typing, the focus request lands first) was previously processed
    /// at the TAIL of the turn — after the pump — leaving its routed edit
    /// stranded in the sink: `native_notifications > 0` at wait-idle, and
    /// the edit applied only at the NEXT boundary.
    /// (`dev_command("focus")` itself needs a real HWND for dev_point; the
    /// boundary it calls — `focus_and_settle` — is the tested unit.)
    #[cfg(feature = "devtools")]
    #[test]
    fn devctl_focus_drains_peer_events_to_quiescence() {
        let (mut be, node) = ime_backend("abc");
        let peer = be.peer_for(node).unwrap();
        // a native edit notification is pending when the focus arrives
        peer.borrow().native_write_for_test("abc+native").unwrap();
        peer.borrow().emit_change_for_test();
        // the backend boundary the devctl `focus` command runs
        be.focus_and_settle(Some(node)).unwrap();
        assert_eq!(be.focus, Some(node));
        // nothing remains stranded in the peer's host queue
        assert!(
            peer.borrow().drain().unwrap().is_empty(),
            "focus boundary left host events queued"
        );
        // the pending edit was routed AND pumped this same boundary
        assert_eq!(&*be.rt.state.v.text(), "abc+native");
        // wait-idle's event queues see genuine quiescence (`damage`/
        // `pending_paint` are paint-lifecycle facets a headless backend
        // never clears — not the stranded-work signal under test)
        let (_idle, diag) = be.dev_idle_state();
        assert_eq!(diag["native_notifications"], 0, "{diag}");
        assert_eq!(diag["events"], false, "{diag}");
        assert_eq!(diag["reentrant"], 0, "{diag}");
        assert_eq!(diag["deferred_native"], 0, "{diag}");
        drop(peer);
        be.shutdown();
    }

    /// D05 — `leave` runs the WM_MOUSELEAVE path through the real devctl
    /// command: hot leaves and stays cleared after the turn settles.
    #[cfg(feature = "devtools")]
    #[test]
    fn devctl_leave_clears_hot_through_backend() {
        let (mut be, _node) = ime_backend("abc");
        be.hot = Some(_node);
        let v = be.dev_command("leave", &serde_json::json!({})).unwrap();
        assert_eq!(v["performed"], "leave");
        assert_eq!(be.hot, None);
        be.shutdown();
    }

    /// F08 — effective palette refresh compares RESOLVED OUTPUT. Two
    /// legs: a forced-colors flip moves the applied selection colors even
    /// when the theme is unchanged, and a same-descriptor palette drift
    /// (system colors moved, appearance/theme identical) still refreshes
    /// the mounted peer.
    #[test]
    fn effective_palette_refresh_compares_resolved_output() {
        fn cref(c: [f32; 4]) -> u32 {
            ((c[0] * 255.0 + 0.5) as u32)
                | (((c[1] * 255.0 + 0.5) as u32) << 8)
                | (((c[2] * 255.0 + 0.5) as u32) << 16)
        }
        let (mut be, node) = ime_backend("x");
        let peer = be.peer_for(node).unwrap();
        let before = peer.borrow().applied_colors_for_test();
        // LEG 1 — forced-colors flip: the REAL GetSysColor resolver path
        be.rt.set_appearance(Appearance {
            dark: false,
            forced_colors: true,
        });
        be.turn().unwrap();
        let after_flip = peer.borrow().applied_colors_for_test();
        let expected_sel = cref(crate::platform::win32::sys_color(
            crate::style::SystemColor::Highlight,
        ));
        assert_eq!(
            after_flip.1, expected_sel,
            "forced-colors flip must push the system selection colors"
        );
        assert_ne!(
            before, after_flip,
            "a palette flip must change applied peer colors"
        );
        // LEG 2 — same-descriptor palette drift: system colors move while
        // (appearance, theme) are byte-identical — the effective-output
        // compare must still push
        be.peer_ctx.sys_resolver.set(|_| [0.5, 0.25, 0.75, 1.0]);
        be.turn().unwrap();
        let after_drift = peer.borrow().applied_colors_for_test();
        assert_eq!(
            after_drift.1,
            cref([0.5, 0.25, 0.75, 1.0]),
            "same-descriptor palette drift must refresh applied sel colors"
        );
        assert_ne!(
            after_flip, after_drift,
            "descriptor-equal drift still moves the applied palette"
        );
        drop(peer);
        be.shutdown();
    }

    /// F09 — committed_ink is owned by successful paint. Covers:
    /// paint-only shadow transition (hover), move, remove, failed paint.
    #[test]
    fn committed_ink_commits_only_on_successful_paint() {
        use crate::style::{
            BoxStylePatch, ButtonStylePatch, Shadow, ShadowPatch, VisualStylePatch,
        };
        let sh = |dx: f32| Shadow {
            offset_x: crate::geom::Dp(dx),
            offset_y: crate::geom::Dp(2.0),
            blur_sigma: crate::geom::Dp(4.0),
            color: crate::style::Color::Role(crate::theme::ColorRole::Shadow),
            spread: crate::geom::Dp(0.0),
        };
        struct S {
            hover_shadow: bool,
            moved: bool,
            show: bool,
        }
        let s0 = S {
            hover_shadow: false,
            moved: false,
            show: true,
        };
        let app = App::new(
            s0,
            Box::new(|_: &mut S, _: Msg, _: &mut UpdateCtx<Msg>| {})
                as Box<dyn Fn(&mut S, Msg, &mut UpdateCtx<Msg>)>,
            Box::new(|s: &S, ui: &mut Ui<Msg>| {
                ui.column(
                    crate::Column::new().padding(if s.moved {
                        crate::Space::Lg
                    } else {
                        crate::Space::Xs
                    }),
                    |ui| {
                        if s.show {
                            let mut p = ButtonStylePatch::new();
                            if s.hover_shadow {
                                p = p.hover(VisualStylePatch {
                                    box_style: BoxStylePatch {
                                        shadow: ShadowPatch::Set(sh(8.0)),
                                        ..Default::default()
                                    },
                                    ..Default::default()
                                });
                            }
                            ui.button("b").style(p).on_press(press_msg);
                        }
                    },
                );
            }) as Box<dyn Fn(&S, &mut Ui<Msg>)>,
        );
        let peer_ctx = crate::platform::win32::PeerCtx::for_test().unwrap();
        let hwnd = probe_hwnd();
        peer_ctx.hwnd.set(hwnd);
        let pf = peer_ctx.make_factory();
        let rt = crate::app::runtime_for(
            app,
            Box::new(pf),
            Theme::light(),
            Appearance {
                dark: false,
                forced_colors: false,
            },
        );
        let mut be = crate::platform::win32::Backend::for_test(rt, peer_ctx).unwrap();
        be.hwnd = hwnd;
        be.turn().unwrap();
        be.relayout().unwrap();
        // the column wraps the button — resolve the BUTTON's NodeId
        let btn = {
            let col = be.rt.root_children()[0];
            let g = be.rt.arena.get(col).unwrap();
            let slot = g.children[0];
            NodeId {
                slot,
                generation: be.rt.arena.generation_of(slot),
            }
        };
        // first successful paint commits the initial ink
        be.paint_for_test(true).unwrap();
        let ink0 = be.committed_ink_map()[&btn];
        assert!(be.damage_rect().is_none(), "paint consumed the damage");

        // PAINT-ONLY SHADOW TRANSITION (hover) — the node's rect does not
        // move; only the resolved ink changes
        be.rt.state.hover_shadow = true;
        be.rt.review_for_test().unwrap();
        be.hot = Some(btn);
        be.relayout().unwrap();
        let d = be.damage_rect().expect("hover shadow must produce damage");
        assert_eq!(
            d.union(ink0),
            d,
            "damage must cover the OLD footprint, got {d:?} vs {ink0:?}"
        );
        // committed_ink still holds the OLD painted truth until paint
        assert_eq!(
            be.committed_ink_map()[&btn],
            ink0,
            "relayout must NOT replace the committed snapshot"
        );
        be.paint_for_test(true).unwrap();
        let ink1 = be.committed_ink_map()[&btn];
        assert_ne!(ink1, ink0, "hover shadow grows the committed footprint");

        // FAILED PAINT — proposed ink is computed + damaged, but the
        // committed snapshot must survive untouched and damage persists
        be.rt.state.hover_shadow = false;
        be.rt.review_for_test().unwrap();
        be.hot = None;
        be.relayout().unwrap();
        let r = be.paint_for_test(false);
        assert!(r.is_err(), "injected paint failure propagates");
        assert_eq!(
            be.committed_ink_map()[&btn],
            ink1,
            "a failed paint leaves the last committed snapshot intact"
        );
        assert!(
            be.damage_rect().is_some(),
            "a failed paint must NOT consume the accumulated damage"
        );
        // a successful paint then commits the pending change
        be.paint_for_test(true).unwrap();
        assert_eq!(be.committed_ink_map()[&btn], ink0);
        assert!(be.damage_rect().is_none());

        // MOVE — the old footprint is damaged even though the node lives
        be.rt.state.moved = true;
        be.rt.review_for_test().unwrap();
        be.relayout().unwrap();
        let d = be.damage_rect().expect("a move must damage");
        assert_eq!(d.union(ink0), d, "move damage covers the old rect");
        be.paint_for_test(true).unwrap();
        let ink2 = be.committed_ink_map()[&btn];
        assert_ne!(ink2, ink0, "moved node commits a new footprint");

        // REMOVE — the node's old ink enters damage; the committed entry
        // is pruned only after a successful paint
        be.rt.state.show = false;
        be.rt.review_for_test().unwrap();
        be.relayout().unwrap();
        let d = be.damage_rect().expect("removal must damage old ink");
        assert_eq!(
            d.union(ink2),
            d,
            "removal damage must cover the last painted footprint"
        );
        be.paint_for_test(true).unwrap();
        assert!(
            !be.committed_ink_map().contains_key(&btn),
            "removed node's committed ink is pruned at successful paint"
        );
        be.shutdown();
    }

