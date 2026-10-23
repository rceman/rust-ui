    /// F01 — the IME delivery owner is the PINNED composition peer, not
    /// current focus: COMPOSITION/ENDCOMPOSITION must resolve to it.
    #[test]
    fn ime_delivery_targets_pinned_owner() {
        struct S;
        let mut rig = Rig::new(
            S,
            |_: &mut S, _: Msg, _: &mut UpdateCtx<Msg>| {},
            |_: &S, ui: &mut Ui<Msg>| {
                ui.label("x");
            },
        );
        rig.view().unwrap();
        let Rig { rt, .. } = rig;
        let mut be = crate::platform::win32::Backend::for_test(
            rt,
            crate::platform::win32::PeerCtx::for_test().unwrap(),
        )
        .unwrap();
        let a = NodeId {
            slot: 7,
            generation: 1,
        };
        let b = NodeId {
            slot: 8,
            generation: 1,
        };
        // focus on B, no composition pinned -> IME resolves focus
        be.focus = Some(b);
        assert_eq!(be.ime_target(), Some(b));
        // composition pinned to A -> focus move MUST NOT redirect
        be.ime_owner.set(Some(Some(a)));
        assert_eq!(be.ime_target(), Some(a));
    }

    /// F01 — an explicitly OWNERLESS composition start (the capture said
    /// "no owner") stays ownerless through delivery and end bookkeeping:
    /// it must never latch onto a peer that gains focus later.
    #[test]
    fn ownerless_ime_stays_ownerless() {
        struct S;
        let mut rig = Rig::new(
            S,
            |_: &mut S, _: Msg, _: &mut UpdateCtx<Msg>| {},
            |_: &S, ui: &mut Ui<Msg>| {
                ui.label("x");
            },
        );
        rig.view().unwrap();
        let Rig { rt, .. } = rig;
        let mut be = crate::platform::win32::Backend::for_test(
            rt,
            crate::platform::win32::PeerCtx::for_test().unwrap(),
        )
        .unwrap();
        let b = NodeId {
            slot: 8,
            generation: 1,
        };
        // START captured with explicitly NO owner -> ownerless pin
        be.ime_owner.set(Some(None));
        // a peer gains focus mid-composition — the ownerless composition
        // must NOT be acquired by it
        be.focus = Some(b);
        assert_eq!(
            be.ime_target(),
            None,
            "ownerless composition never resolves to a later focus"
        );
        // delivery goes nowhere — the focused peer receives nothing
        be.send_ime(WM_IME_COMPOSITION, 0, 0).unwrap();
        // end bookkeeping releases the ownerless pin; no peer acquires it
        be.ime_end().unwrap();
        assert_eq!(
            be.ime_owner.get(),
            None,
            "end bookkeeping clears the ownerless pin to inactive"
        );
        // after it ends, IME traffic resolves through focus again (the
        // turn inside ime_end cleared the unfocused dead slot — re-set it)
        be.focus = Some(b);
        assert_eq!(be.ime_target(), Some(b));
    }

    /// F01 owner discriminator — a replayed message with an explicitly
    /// absent arrival owner must NOT fall through to current focus; a
    /// captured owner replays to it; live dispatch resolves current.
    #[test]
    fn replay_owner_is_discriminated() {
        // the discriminator IS Option<Option<NodeId>> on the cell —
        // the resolution contract: captured = Some(c) -> c governs
        // (None means no-owner), None = live -> current focus
        let a = NodeId {
            slot: 1,
            generation: 0,
        };
        let b = NodeId {
            slot: 2,
            generation: 0,
        };
        let resolve =
            |captured: Option<Option<NodeId>>, live: Option<NodeId>| captured.unwrap_or(live);
        // replay with captured owner A -> A even though focus moved to B
        assert_eq!(resolve(Some(Some(a)), Some(b)), Some(a));
        // replay that captured NO owner -> None, NOT the new focus B
        assert_eq!(resolve(Some(None), Some(b)), None);
        // live dispatch -> current focus
        assert_eq!(resolve(None, Some(b)), Some(b));
        assert_eq!(resolve(None, None), None);
    }

    /// The timer pool allocator: collision-free while live, honest None at
    /// capacity, reuse only after free — and never modulo-wrap onto live.
    #[test]
    fn native_timer_pool_is_the_armed_authority() {
        use windows::Win32::UI::WindowsAndMessaging::KillTimer;
        let pool = std::sync::Arc::new(std::sync::Mutex::new(TimerPool::new()));
        let nid = NodeId {
            slot: 1,
            generation: 1,
        };

        let t1 = pool.lock().unwrap().alloc().unwrap();
        pool.lock().unwrap().arm(t1, nid, 7);
        let t2 = pool.lock().unwrap().alloc().unwrap();
        pool.lock().unwrap().arm(t2, nid, 8);
        assert_ne!(t1, t2);
        assert!(t1 >= NATIVE_TIMER_BASE && t1 < NATIVE_TIMER_BASE + NATIVE_TIMER_CAP);
        assert_eq!(pool.lock().unwrap().owner(t1), Some((nid, 7)));

        // kill -> tombstone: the id stays resolved to NO live owner and
        // is never reallocated — a queued stale WM_TIMER can't bind anew
        pool.lock().unwrap().kill(t1);
        assert_eq!(pool.lock().unwrap().owner(t1), None);
        assert!(!pool.lock().unwrap().live_ids().contains(&t1));
        let t3 = pool.lock().unwrap().alloc().unwrap();
        assert_ne!(t3, t1, "a tombstoned id is never reused");
        pool.lock().unwrap().arm(t3, nid, 9);
        assert_eq!(pool.lock().unwrap().owner(t3), Some((nid, 9)));

        // capacity exhaustion is honest — fill every live slot
        let mut pool = TimerPool::new();
        let nid2 = NodeId {
            slot: 2,
            generation: 1,
        };
        let mut armed = 0usize;
        while let Some(t) = pool.alloc() {
            pool.arm(t, nid2, armed as u32);
            armed += 1;
        }
        assert_eq!(armed, NATIVE_TIMER_CAP);
        assert!(pool.alloc().is_none());
        let _ = KillTimer;
    }
