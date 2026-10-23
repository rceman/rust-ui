    {
        let (a, b) = (1.25f32, 2.0f32); // real-DC-adjacent -> 200%
        let sink = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut peer = crate::platform::win32::WindowlessPeer::create(
            9,
            &lib,
            hwnd,
            ScaleFactor(a),
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
            "Ghost",
            crate::text::TextRevision::mint(),
            binding,
        )
        .expect("initialize");
        peer.apply_bounds(island_dip, ScaleFactor(a)).unwrap();
        unsafe {
            rt.BeginDraw();
            rt.Clear(Some(&D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            }));
        }
        peer.draw(&rt, island_dip, 0).expect("draw@A");
        unsafe {
            rt.EndDraw(None, None).expect("EndDraw");
        }
        // count ink at A's island — this stays (framebuffer accumulates)
        let ia = island_dip.physical(ScaleFactor(a));
        let mut ink_a = 0usize;
        for y in ia.top..ia.bottom {
            for x in ia.left..ia.right {
                let o = (y * FBW + x) as usize * 4;
                if data[o] > 180 {
                    ink_a += 1;
                }
            }
        }
        assert!(ink_a > 40, "live-change leg: no ink at scale A");

        // now the scale change — same island DIP, new scale
        peer.apply_bounds(island_dip, ScaleFactor(b)).unwrap();
        unsafe {
            rt.BeginDraw();
            rt.Clear(Some(&D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            }));
        }
        peer.draw(&rt, island_dip, 0).expect("draw@B");
        unsafe {
            rt.EndDraw(None, None).expect("EndDraw");
        }
        let ib = island_dip.physical(ScaleFactor(b));
        let (mut in_b, mut stray) = (0usize, 0usize);
        for y in 0..FBH {
            for x in 0..FBW {
                let o = (y * FBW + x) as usize * 4;
                if data[o] <= 180 {
                    continue;
                }
                if x >= ib.left && x < ib.right && y >= ib.top && y < ib.bottom {
                    in_b += 1;
                } else {
                    stray += 1;
                }
            }
        }
        eprintln!("[probe] live-scale {a}->{b}: in_b={in_b} stray={stray}");
        assert!(in_b > 40, "no ink inside the B-scaled island");
        assert_eq!(stray, 0, "scale change left ghost ink at stale coordinates");
    }
