    {
        let sink = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut peer = crate::platform::win32::WindowlessPeer::create(
            13,
            &lib,
            hwnd,
            ScaleFactor(1.0),
            &cfg(),
            sink.clone(),
            std::sync::Arc::new(std::sync::Mutex::new(
                crate::platform::win32::text::TimerPool::new(),
            )),
        )
        .expect("peer create");
        crate::node::TextPeer::initialize(
            &mut peer,
            "abc",
            crate::text::TextRevision::mint(),
            crate::text::BindingToken::mint(),
        )
        .expect("initialize");
        let old_b = island_dip;
        peer.apply_bounds(old_b, ScaleFactor(1.0)).unwrap();
        // PRECONDITION: composing
        peer.set_composing(true);
        // ACTION: a scale relatch arrives mid-composition
        let new_b = crate::geom::Rect {
            x: 40.0,
            y: 20.0,
            width: 200.0,
            height: 30.0,
        };
        peer.apply_bounds(new_b, ScaleFactor(1.5)).unwrap();
        // POSTCONDITION: the effective geometry is STILL the old space —
        // host callbacks, pointer mapping and draw all agree
        let (eb, es) = peer.effective_geometry();
        assert_eq!(es, ScaleFactor(1.0), "scale must remain old while pending");
        assert_eq!(eb.x, old_b.x);
        assert_eq!(eb.y, old_b.y);
        // and a measure pass under pending geometry must NOT corrupt it:
        // natural_size at the NEW width leaves effective bounds untouched
        let _ = peer.natural_size(999.0).unwrap();
        let (eb2, _) = peer.effective_geometry();
        assert_eq!(
            eb2.width, old_b.width,
            "pending relatch: width write was parked"
        );

        // REAL ORDER — measure BEFORE the bounds/scale request: with
        // composition active, natural_size(new_width) runs first and the
        // effective geometry is already frozen; the later apply_bounds
        // parks, never retroactively widening the space the service used
        let mut peer2 = crate::platform::win32::WindowlessPeer::create(
            14,
            &lib,
            hwnd,
            ScaleFactor(1.0),
            &cfg(),
            sink.clone(),
            std::sync::Arc::new(std::sync::Mutex::new(
                crate::platform::win32::text::TimerPool::new(),
            )),
        )
        .expect("peer create");
        crate::node::TextPeer::initialize(
            &mut peer2,
            "abc",
            crate::text::TextRevision::mint(),
            crate::text::BindingToken::mint(),
        )
        .expect("initialize");
        peer2.apply_bounds(island_dip, ScaleFactor(1.0)).unwrap();
        peer2.set_composing(true);
        // measure at the NEW width first — host geometry must NOT absorb it
        let _ = peer2.natural_size(999.0).unwrap();
        let (eb4, es4) = peer2.effective_geometry();
        assert_eq!(
            eb4.width, island_dip.width,
            "measure-first ordering: new width must not leak into the frozen space"
        );
        assert_eq!(es4, ScaleFactor(1.0));
        // THEN the bounds/scale request arrives — defers whole
        peer2.apply_bounds(new_b, ScaleFactor(2.0)).unwrap();
        let (eb5, es5) = peer2.effective_geometry();
        assert_eq!(eb5.x, island_dip.x);
        assert_eq!(es5, ScaleFactor(1.0));
        // composition ends — the parked relatch applies atomically
        peer2.set_composing(false);
        peer2.finish_pending_relatch().unwrap();
        let (eb6, es6) = peer2.effective_geometry();
        assert_eq!(es6, ScaleFactor(2.0));
        assert_eq!(eb6.x, new_b.x);
        crate::node::TextPeer::release(&mut peer2);
        // composition ends -> the pending relatch applies atomically
        peer.set_composing(false);
        peer.finish_pending_relatch().unwrap();
        let (eb3, es3) = peer.effective_geometry();
        assert_eq!(es3, ScaleFactor(1.5));
        assert_eq!(eb3.x, new_b.x);
        crate::node::TextPeer::release(&mut peer);
    }
