use super::*;


    // --- compare_element pipeline (offline, portable) --------------------
    fn target_at(rect: Rect) -> ExpectedTarget {
        ExpectedTarget {
            tier: Some("core".into()),
            capture: None,
            rect: Some(rect),
            relative_rect: None,
            visible: true,
            reason: None,
            owner_attributed: false,
            label_gap: None,
            interaction: None,
            meta: BTreeMap::new(),
        }
    }
    /// `w`x`h` image of `bg` with the element's `rect` (snapped at `scale`)
    /// filled by `colors` in columns of `cell_dp` logical Dp.
    fn element_image(
        w: u32,
        h: u32,
        rect: Rect,
        scale: f64,
        cell_dp: u32,
        colors: &[[u8; 4]],
        bg: [u8; 4],
    ) -> Image {
        let mut rgba = vec![0; w as usize * h as usize * 4];
        for i in 0..w as usize * h as usize {
            rgba[i * 4..i * 4 + 4].copy_from_slice(&bg);
        }
        // Fill the full source-pixel FOOTPRINT extent (floor/ceil of the
        // scaled edges), not just the snapped rect: a fractional logical edge
        // still contributes its flat colour to the boundary source pixels it
        // overlaps, so canonicalization yields the exact flat colour.
        let x0 = (rect.x * scale).floor() as i64;
        let x1 = ((rect.x + rect.width) * scale).ceil() as i64;
        let y0 = (rect.y * scale).floor() as i64;
        let y1 = ((rect.y + rect.height) * scale).ceil() as i64;
        for sy in y0.max(0)..y1.min(i64::from(h)) {
            for sx in x0.max(0)..x1.min(i64::from(w)) {
                let u = ((sx as f64 + 0.5) / scale - rect.x) / f64::from(cell_dp);
                let c = colors[(u.floor() as usize).max(0) % colors.len()];
                let i = ((sy as usize * w as usize) + sx as usize) * 4;
                rgba[i..i + 4].copy_from_slice(&c);
            }
        }
        Image {
            width: w,
            height: h,
            rgba,
        }
    }
    fn native_snapshot(rect: Rect, scale: f64, client_px: (u32, u32)) -> Snapshot {
        let px = snap_rect(rect, scale).unwrap();
        Snapshot {
            schema: SNAPSHOT_SCHEMA.into(),
            scale_factor: scale,
            client_dp: Rect {
                x: 0.,
                y: 0.,
                width: client_px.0 as f64 / scale,
                height: client_px.1 as f64 / scale,
            },
            client_px: Rect {
                x: 0.,
                y: 0.,
                width: client_px.0 as f64,
                height: client_px.1 as f64,
            },
            root: Node {
                automation_id: Some("probe.el".into()),
                kind: "button".into(),
                label: "probe".into(),
                rect_dp: rect,
                rect_px: Rect {
                    x: px.left as f64,
                    y: px.top as f64,
                    width: (px.right - px.left) as f64,
                    height: (px.bottom - px.top) as f64,
                },
                visible: true,
                enabled: true,
                focused: false,
                focus_visible: false,
                hovered: false,
                pressed: false,
                presented: None,
                selection: None,
                line_height: None,
                line_twips: None,
                selected: None,
                checked: None,
                expanded: None,
                parts: vec![],
                children: vec![],
            },
        }
    }
    /// Ten distinct column colours — one per 4-Dp column of a 40-Dp element,
    /// so a per-column mutation lands in exactly one logical region.
    const PALETTE: [[u8; 4]; 10] = [
        [200, 0, 0, 255],
        [0, 200, 0, 255],
        [0, 0, 200, 255],
        [200, 200, 200, 255],
        [90, 30, 30, 255],
        [30, 90, 30, 255],
        [30, 30, 90, 255],
        [150, 120, 30, 255],
        [30, 150, 120, 255],
        [120, 30, 150, 255],
    ];
    #[test]
    fn compare_element_position_independent_canonical() {
        let want = Rect {
            x: 100.,
            y: 50.,
            width: 40.,
            height: 20.,
        };
        let at = Rect {
            x: 24.,
            y: 12.,
            width: 40.,
            height: 20.,
        };
        let mut targets = BTreeMap::new();
        targets.insert(SemanticKey::parse("ref.el").unwrap(), target_at(want));
        let core = TierScope::parse(None).unwrap();
        let key = SemanticKey::parse("ref.el").unwrap();
        let native_key = SemanticKey::parse("probe.el").unwrap();
        // Element content is identical in logical space although it sits at
        // a different position in a differently scaled raster.
        let reference = element_image(160, 100, want, 1.0, 4, &PALETTE, [255; 4]);
        let native_img = element_image(125, 80, at, 1.25, 4, &PALETTE, [255; 4]);
        let snap = native_snapshot(at, 1.25, (125, 80));
        let r = compare_element(
            &targets,
            &key,
            &reference,
            1.0,
            (160., 100.),
            &snap,
            &native_img,
            &native_key,
            &core,
            0.,
            0.,
            "absolute",
            0,
        )
        .unwrap();
        assert_eq!(r.result["visual_canonical"]["missing_pixels"], 0);
        assert_eq!(r.result["visual_canonical"]["mismatched_pixels"], 0);
        assert_eq!(
            r.result["raster"]["native_canonical"]["canonical_rect_dp"],
            json!({"left":24,"top":12,"right":64,"bottom":32})
        );
        assert_eq!(
            r.result["raster"]["reference"]["canonical_rect_dp"],
            json!({"left":100,"top":50,"right":140,"bottom":70})
        );
        // Style-different native, identical logical geometry: the diff is
        // exactly the differing logical region in canonical space.
        let mut different = PALETTE;
        different[1] = [255, 255, 0, 255];
        let styled = element_image(125, 80, at, 1.25, 4, &different, [255; 4]);
        let r = compare_element(
            &targets,
            &key,
            &reference,
            1.0,
            (160., 100.),
            &snap,
            &styled,
            &native_key,
            &core,
            0.,
            0.,
            "absolute",
            0,
        )
        .unwrap();
        assert_eq!(r.result["visual_canonical"]["missing_pixels"], 0);
        assert_eq!(
            r.result["visual_canonical"]["difference_bbox"],
            json!({"x":4.,"y":0.,"width":4.,"height":20.})
        );
        assert_eq!(r.result["visual_canonical"]["mismatched_pixels"], 80);
    }
    #[test]
    fn compare_element_size_delta_is_missing_pixels_not_scale_noise() {
        let want = Rect {
            x: 100.,
            y: 50.,
            width: 40.,
            height: 20.,
        };
        let at = Rect {
            x: 100.,
            y: 50.,
            width: 44.,
            height: 20.,
        };
        let mut targets = BTreeMap::new();
        targets.insert(SemanticKey::parse("ref.el").unwrap(), target_at(want));
        let core = TierScope::parse(None).unwrap();
        let key = SemanticKey::parse("ref.el").unwrap();
        let native_key = SemanticKey::parse("probe.el").unwrap();
        let reference = element_image(160, 100, want, 1.0, 4, &PALETTE, [255; 4]);
        let native_img = element_image(200, 100, at, 1.25, 4, &PALETTE, [255; 4]);
        let snap = native_snapshot(at, 1.25, (200, 100));
        let r = compare_element(
            &targets,
            &key,
            &reference,
            1.0,
            (160., 100.),
            &snap,
            &native_img,
            &native_key,
            &core,
            0.,
            0.,
            "absolute",
            0,
        )
        .unwrap();
        // 44 vs 40 logical width: canonical dims differ by exactly the
        // logical strip; missing pixels are the 4x20 right-hand strip only.
        assert_eq!(
            r.result["raster"]["native_canonical"]["dimensions"],
            json!([44, 20])
        );
        assert_eq!(r.result["visual_canonical"]["missing_pixels"], 80);
        assert_eq!(r.result["visual_canonical"]["mismatched_pixels"], 80);
        assert_eq!(r.result["geometry"]["delta"]["width"], 4.);
        assert_eq!(r.result["geometry"]["pass"], false);
        // A native footprint reaching outside the raw capture fails closed.
        let off = Rect {
            x: 500.,
            y: 0.,
            width: 40.,
            height: 20.,
        };
        let off_snap = native_snapshot(off, 1.25, (200, 100));
        let e = compare_element(
            &targets,
            &key,
            &reference,
            1.0,
            (160., 100.),
            &off_snap,
            &native_img,
            &native_key,
            &core,
            0.,
            0.,
            "absolute",
            0,
        )
        .err()
        .unwrap();
        assert_eq!(e.code, "TARGET_NOT_VISIBLE");
    }
    #[test]
    fn canonicalize_non_binary_scales_have_no_edge_artifacts() {
        // s*(C.left+i) as one product keeps footprints exact at non-binary
        // scales: a flat region resamples to the exact flat colour with no
        // missing pixels and no out-of-raster pixel.
        for &s in &[1.1f64, 1.75] {
            let mut rgba = vec![0; 220 * 220 * 4];
            for p in rgba.chunks_exact_mut(4) {
                p.copy_from_slice(&[77, 77, 77, 255]);
            }
            let img = Image {
                width: 220,
                height: 220,
                rgba,
            };
            let c = snap_rect(
                Rect {
                    x: 10.,
                    y: 10.,
                    width: 40.,
                    height: 20.,
                },
                1.0,
            )
            .unwrap();
            let canonical = img.canonicalize(s, &c).unwrap();
            assert!((canonical.width == 40) && (canonical.height == 20));
            assert!(
                canonical
                    .rgba
                    .chunks_exact(4)
                    .all(|p| p == [77, 77, 77, 255]),
                "s={s} produced a non-flat pixel"
            );
        }
    }
