    {
        use crate::platform::win32::render::{ShadowCache, paint_box};
        let cache = RefCell::new(ShadowCache::default());
        let mut bs = crate::style::BoxStyle::default();
        bs.background = crate::style::Color::rgba(30, 30, 30, 255);
        // top+bottom each exceed the whole 20px height
        for side in [&mut bs.border.top, &mut bs.border.bottom] {
            side.width = crate::geom::Dp(40.0);
            side.color = crate::style::Color::rgba(200, 30, 30, 255);
        }
        let r = D2D_RECT_F {
            left: 10.0,
            top: 10.0,
            right: 110.0,
            bottom: 30.0,
        };
        unsafe {
            rt.BeginDraw();
            rt.Clear(Some(&D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            }));
        }
        paint_box(&rt, &r, &bs, false, None, ScaleFactor(1.0), &cache, 9).expect("paint_box");
        unsafe {
            rt.EndDraw(None, None).unwrap();
        }
        // every painted pixel is inside the box — clamped insets cannot
        // push ink outside the outer edge
        let mut stray = 0usize;
        for y in 0..60i32 {
            for x in 0..130i32 {
                let p = &data[(y * FBW + x) as usize * 4..];
                let red = p[0] > 150 && p[1] < 80 && p[2] < 80; // BGRA: red channel
                if red
                    && !(x as f32 >= r.left - 1.0
                        && (x as f32) < r.right + 1.0
                        && y as f32 >= r.top - 1.0
                        && (y as f32) < r.bottom + 1.0)
                {
                    stray += 1;
                }
            }
        }
        assert_eq!(stray, 0, "oversized borders bled outside the box");
    }
