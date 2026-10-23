    {
        use crate::platform::win32::render::{ShadowCache, draw_shadow};
        use crate::style::CornerRadii;
        let cache = RefCell::new(ShadowCache::default());
        // draw coordinates are DIP — the framebuffer is device px at
        // dc_dpi: convert the box to px for ink assertions
        let dcs = dc_dpi as f32 / 96.0;
        let box_px = D2D_RECT_F {
            left: 60.0,
            top: 60.0,
            right: 140.0,
            bottom: 110.0,
        };
        let box_dev = D2D_RECT_F {
            left: box_px.left * dcs,
            top: box_px.top * dcs,
            right: box_px.right * dcs,
            bottom: box_px.bottom * dcs,
        };
        let shadow = |dx: f32, dy: f32| crate::style::Shadow {
            color: crate::style::Color::rgba(0, 0, 0, 255),
            blur_sigma: crate::geom::Dp(0.0),
            offset_x: crate::geom::Dp(dx),
            offset_y: crate::geom::Dp(dy),
            spread: crate::geom::Dp(0.0),
        };
        let case = |dx: f32, dy: f32| -> (usize, usize) {
            unsafe {
                rt.BeginDraw();
                // WHITE clear — alpha is IGNORE-mode on a DC target;
                // black shadow ink reads as dark pixels
                rt.Clear(Some(&D2D1_COLOR_F {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                }));
            }
            draw_shadow(
                &rt,
                &box_px,
                &CornerRadii::default(),
                &shadow(dx, dy),
                false,
                ScaleFactor(1.0),
                &cache,
                1,
            )
            .expect("draw_shadow");
            unsafe {
                rt.EndDraw(None, None).expect("end");
            }
            // count ink inside vs outside the box footprint — the
            // interior test shrinks 2px so D2D's linear-sampling edge
            // bleed can't masquerade as interior ink
            let mut inside = 0usize;
            let mut outside = 0usize;
            for y in 20..180i32 {
                for x in 20..220i32 {
                    let p = &data[(y * FBW + x) as usize * 4..];
                    let ink = p[0] < 200 || p[1] < 200 || p[2] < 200;
                    if ink {
                        let in_box = x as f32 >= box_dev.left + 2.0 * dcs
                            && (x as f32) < box_dev.right - 2.0 * dcs
                            && y as f32 >= box_dev.top + 2.0 * dcs
                            && (y as f32) < box_dev.bottom - 2.0 * dcs;
                        if in_box {
                            inside += 1;
                        } else {
                            outside += 1;
                        }
                    }
                }
            }
            (inside, outside)
        };
        let (in_pos, out_pos) = case(12.0, 0.0);
        assert_eq!(in_pos, 0, "+x shadow must not paint inside the box");
        assert!(out_pos > 0, "+x shadow produced no ink at all");
        // the ink must be on the RIGHT side — sample the strips
        let (in_neg, out_neg) = case(-12.0, 0.0);
        assert_eq!(in_neg, 0, "-x shadow must not paint inside the box");
        assert!(out_neg > 0, "-x shadow produced no ink");
        // which side is the ink on? column histogram for +x and -x
        let side = |dx: f32, dy: f32| -> (i64, i64) {
            unsafe {
                rt.BeginDraw();
                rt.Clear(Some(&D2D1_COLOR_F {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                }));
            }
            draw_shadow(
                &rt,
                &box_px,
                &CornerRadii::default(),
                &shadow(dx, dy),
                false,
                ScaleFactor(1.0),
                &cache,
                1,
            )
            .unwrap();
            unsafe {
                rt.EndDraw(None, None).unwrap();
            }
            let mut left = 0i64;
            let mut right = 0i64;
            for y in 20..180i32 {
                for x in 20..220i32 {
                    let p = &data[(y * FBW + x) as usize * 4..];
                    if p[0] < 200 || p[1] < 200 || p[2] < 200 {
                        if (x as f32) < (box_dev.left + box_dev.right) / 2.0 {
                            left += 1;
                        } else {
                            right += 1;
                        }
                    }
                }
            }
            (left, right)
        };
        let (l_pos, r_pos) = side(12.0, 0.0);
        assert!(r_pos > l_pos, "+offset shadow must sit right of the box");
        let (l_neg, r_neg) = side(-12.0, 0.0);
        assert!(l_neg > r_neg, "-offset shadow must sit left of the box");

        // oversized sigma/extent is a typed failure, never a wrap
        let huge = crate::style::Shadow {
            color: crate::style::Color::rgba(0, 0, 0, 255),
            blur_sigma: crate::geom::Dp(1.0e9),
            offset_x: crate::geom::Dp(0.0),
            offset_y: crate::geom::Dp(0.0),
            spread: crate::geom::Dp(0.0),
        };
        assert!(
            draw_shadow(
                &rt,
                &box_px,
                &CornerRadii::default(),
                &huge,
                false,
                ScaleFactor(1.0),
                &cache,
                1
            )
            .is_err(),
            "oversized sigma must be a typed failure"
        );
        // unrepresentable OFFSET — finite-but-huge would saturate `as i32`
        // and overflow `x + ox`; rejected BEFORE any integer arithmetic.
        // Non-finite offsets are degenerate (zero ink), consistent with
        // the sa<=0 handling — they return Ok without raster work.
        for bad in [2147483648.0f32, 50000.0] {
            let b = crate::style::Shadow {
                color: crate::style::Color::rgba(0, 0, 0, 255),
                blur_sigma: crate::geom::Dp(0.0),
                offset_x: crate::geom::Dp(bad),
                offset_y: crate::geom::Dp(0.0),
                spread: crate::geom::Dp(0.0),
            };
            assert!(
                draw_shadow(
                    &rt,
                    &box_px,
                    &CornerRadii::default(),
                    &b,
                    false,
                    ScaleFactor(1.0),
                    &cache,
                    1
                )
                .is_err(),
                "finite unrepresentable offset {bad} must be a typed failure"
            );
        }
        for degenerate in [f32::INFINITY, f32::NAN] {
            let b = crate::style::Shadow {
                color: crate::style::Color::rgba(0, 0, 0, 255),
                blur_sigma: crate::geom::Dp(0.0),
                offset_x: crate::geom::Dp(degenerate),
                offset_y: crate::geom::Dp(0.0),
                spread: crate::geom::Dp(0.0),
            };
            unsafe { rt.BeginDraw() };
            assert!(
                draw_shadow(
                    &rt,
                    &box_px,
                    &CornerRadii::default(),
                    &b,
                    false,
                    ScaleFactor(1.0),
                    &cache,
                    1
                )
                .is_ok(),
                "non-finite offset {degenerate} is zero-ink, not a crash"
            );
            unsafe { rt.EndDraw(None, None).unwrap() };
        }
        // offset is part of the raster identity — +12 and -12 produced
        // TWO distinct cached masks (the +12 mask cannot serve -12)
        let _ = cache.borrow().len();
        assert!(
            cache.borrow().len() >= 2,
            "different offsets must not share a raster"
        );
    }
