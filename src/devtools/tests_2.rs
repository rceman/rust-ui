use super::*;

    #[test]
    fn systemic_frozen_contract_snap_audit() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("reference/shadcn-gallery-v0.1/contract.json");
        let v = load_json(&path).unwrap();
        assert_eq!(v["$schema"], CONTRACT_SCHEMA);
        let mut records = Vec::new();
        for el in v["elements"].as_array().unwrap() {
            for (_k, cap) in el["captures"].as_object().unwrap() {
                let rect: Rect = serde_json::from_value(cap["rect"].clone()).unwrap();
                records.push((rect, true));
                for (_pk, part) in cap["parts"].as_object().into_iter().flatten() {
                    if part["visible"].as_bool().unwrap_or(true) {
                        records.push((
                            Rect {
                                x: rect.x + part["x"].as_f64().unwrap(),
                                y: rect.y + part["y"].as_f64().unwrap(),
                                width: part["width"].as_f64().unwrap(),
                                height: part["height"].as_f64().unwrap(),
                            },
                            false,
                        ));
                    }
                }
            }
        }
        assert_eq!(
            records.iter().filter(|r| r.1).count(),
            581,
            "element-capture records"
        );
        assert_eq!(
            records.iter().filter(|r| !r.1).count(),
            806,
            "visible-part records"
        );
        let mut disagree = 0usize;
        let mut disagree_elements = 0usize;
        for &(r, is_element) in &records {
            let old = old_crop_edges(r);
            let new = px(&snap_rect(r, 1.0).unwrap());
            if old != new {
                disagree += 1;
                if is_element {
                    disagree_elements += 1;
                }
            }
            for s in [1.0, 1.25, 1.5, 2.0] {
                let p = snap_rect(r, s).unwrap();
                // Edge snap spec, restated independently of the impl.
                assert_eq!(i64::from(p.left), (r.x * s).round() as i64);
                assert_eq!(i64::from(p.right), ((r.x + r.width) * s).round() as i64);
                // Canonical lattice (scale 1.0) is the reference-crop space;
                // integer-size-in-Dp rects keep an integer px size only when
                // width*scale is itself integral.
                let c = snap_rect(r, 1.0).unwrap();
                if (r.width * s).fract() == 0. {
                    assert_eq!(i64::from(p.right) - i64::from(p.left), (r.width * s) as i64);
                }
                if (r.height * s).fract() == 0. {
                    assert_eq!(
                        i64::from(p.bottom) - i64::from(p.top),
                        (r.height * s) as i64
                    );
                }
                assert!(c.right >= c.left && c.bottom >= c.top);
            }
        }
        // Measured on this checkout: 421/581 element records disagree with
        // the retired floor/ceil policy (the independent review's figure);
        // including the 806 visible-part records it is 999/1387.
        assert_eq!(disagree_elements, 421, "element old-vs-new disagreements");
        assert_eq!(disagree, 999, "element+part old-vs-new disagreements");
    }
    // --- D01: two raster spaces + area-box/1 ----------------------------
    /// Flat logical regions: `colors` columns of `cell_dp` Dp, `rows_dp` Dp
    /// tall, rasterized at `scale` px/Dp. Region boundaries land on integer
    /// source pixels for the tested scales.
    fn flat_regions(scale: f64, cell_dp: u32, rows_dp: u32, colors: &[[u8; 4]]) -> Image {
        let cell = (cell_dp as f64 * scale).round() as usize;
        let rows = (rows_dp as f64 * scale).round() as usize;
        let w = cell * colors.len();
        let mut rgba = vec![0; w * rows * 4];
        for y in 0..rows {
            for x in 0..w {
                let c = colors[x / cell];
                rgba[(y * w + x) * 4..(y * w + x) * 4 + 4].copy_from_slice(&c);
            }
        }
        Image {
            width: w as u32,
            height: rows as u32,
            rgba,
        }
    }
    #[test]
    fn canonical_identity_determinism_and_calibration() {
        let colors = [
            [200, 0, 0, 255],
            [0, 200, 0, 255],
            [0, 0, 200, 255],
            [200, 200, 200, 255],
        ];
        // Region boundaries at Dp multiples of 4 (s=1.25 -> 5px), 2 (s=1.5
        // -> 3px), 1 (s=2.0 -> 2px) keep canonical footprints inside a single
        // region; canonicalization is then pixel-exact.
        for &(s, cell_dp) in &[(1.25f64, 4u32), (1.5, 2), (2.0, 1)] {
            let reference = flat_regions(1.0, cell_dp, 4, &colors);
            let native = flat_regions(s, cell_dp, 4, &colors);
            let c = PixelRect {
                left: 0,
                top: 0,
                right: (colors.len() as u32 * cell_dp) as i32,
                bottom: 4,
            };
            let canonical = native.canonicalize(s, &c).unwrap();
            assert_eq!((canonical.width, canonical.height), (c.right as u32, 4));
            let (v, _) = compare_image(&reference, &canonical, 0).unwrap();
            assert_eq!(v["missing_pixels"], 0, "no L-border/clamped edge");
            assert_eq!(v["mismatched_pixels"], 0, "s={s} calibration");
            // Deterministic: byte-equal on re-run.
            assert_eq!(canonical.rgba, native.canonicalize(s, &c).unwrap().rgba);
        }
        // s == 1.0 must be BIT-IDENTICAL to a plain crop.
        let img = flat_regions(1.0, 4, 4, &colors);
        let c = PixelRect {
            left: 2,
            top: 1,
            right: 11,
            bottom: 3,
        };
        assert_eq!(
            img.canonicalize(1.0, &c).unwrap().rgba,
            img.crop(&c).unwrap().rgba
        );
        // Same canonical raster -> zero diff; one pixel -> exact diff + bbox.
        let mut altered = img.clone();
        altered.rgba[(2 * 16 + 5) * 4] = 9;
        let canon = img.canonicalize(1.0, &c).unwrap();
        let changed = altered.canonicalize(1.0, &c).unwrap();
        let (v, _) = compare_image(&canon, &changed, 0).unwrap();
        assert_eq!(v["mismatched_pixels"], 1);
        assert_eq!(
            v["difference_bbox"],
            json!({"x":3.,"y":1.,"width":1.,"height":1.})
        );
        // Style-different native, identical logical geometry: the diff bbox
        // is exactly the differing logical region in canonical space.
        let mut different = colors;
        different[1] = [255, 255, 0, 255];
        let styled = flat_regions(1.25, 4, 4, &different);
        let canon = styled
            .canonicalize(
                1.25,
                &PixelRect {
                    left: 0,
                    top: 0,
                    right: 16,
                    bottom: 4,
                },
            )
            .unwrap();
        let (v, _) = compare_image(&img, &canon, 0).unwrap();
        assert_eq!(v["missing_pixels"], 0);
        assert_eq!(v["mismatched_pixels"], 16);
        assert_eq!(
            v["difference_bbox"],
            json!({"x":4.,"y":0.,"width":4.,"height":4.})
        );
        // Footprint outside the raw capture fails closed.
        assert_eq!(
            img.canonicalize(
                1.25,
                &PixelRect {
                    left: 0,
                    top: 0,
                    right: 16,
                    bottom: 4
                }
            )
            .err()
            .unwrap()
            .code,
            "TARGET_NOT_VISIBLE"
        );
    }
    // --- D03: contract adapter, tiers, parts ----------------------------
    #[test]
    fn tier_scope_parse_and_filter() {
        assert_eq!(TierScope::parse(None).unwrap().tiers, ["core"]);
        assert!(TierScope::parse(Some("all")).unwrap().all);
        assert!(
            TierScope::parse(Some("core,later"))
                .unwrap()
                .includes(Some("later"))
        );
        assert!(
            !TierScope::parse(Some("core"))
                .unwrap()
                .includes(Some("later"))
        );
        assert!(TierScope::parse(Some("core")).unwrap().includes(None));
        assert!(TierScope::parse(Some("bogus")).is_err());
        assert!(TierScope::parse(Some("all,core")).is_err());
    }
    fn contract_fixture() -> Value {
        json!({"$schema":CONTRACT_SCHEMA,"elements":[
          {"automation_id":"checkbox.default.unchecked","tier":"core",
           "captures":{"forms/default":{"rect":{"x":204,"y":1265.5,"width":16,"height":16},
             "parts":{"label":{"x":24,"y":1,"width":71.984,"height":14,
               "owner_attributed":true,"label_gap":8,"color":"#000"},
                      "ghost":{"visible":false,"reason":"not-rendered"}},
             "interaction":{"focused":true,"selection":[0,6]}}}},
          {"automation_id":"later.widget","tier":"later",
           "captures":{"forms/default":{"rect":{"x":0,"y":0,"width":10,"height":10},"parts":{}}}},
          {"automation_id":"other.widget","tier":"later",
           "captures":{"forms/default":{"rect":{"x":0,"y":20,"width":10,"height":10},"parts":{}}}}
        ]})
    }
    #[test]
    fn adapter_parts_tiers_hidden_and_absolute_rects() {
        let t = reference_targets(&contract_fixture(), "forms/default").unwrap();
        // Element + two parts of the checkbox + two later elements.
        assert_eq!(t.len(), 5);
        let label = &t[&SemanticKey::parse("checkbox.default.unchecked#label").unwrap()];
        // Part x/y are owner-relative; absolute rect derives from the owner.
        assert_eq!(label.rect.unwrap().x, 228.);
        assert_eq!(label.relative_rect.unwrap().x, 24.);
        assert!(label.owner_attributed);
        assert_eq!(label.label_gap, Some(8.));
        let ghost = &t[&SemanticKey::parse("checkbox.default.unchecked#ghost").unwrap()];
        assert!(!ghost.visible && ghost.rect.is_none());
        assert_eq!(ghost.reason.as_deref(), Some("not-rendered"));
        let el = &t[&SemanticKey::parse("checkbox.default.unchecked").unwrap()];
        assert_eq!(
            el.interaction,
            Some(json!({"focused":true,"selection":[0,6]}))
        );
        // Wrong capture key: exact lookup, no fallback.
        assert_eq!(
            reference_targets(&contract_fixture(), "forms/pressed")
                .unwrap_err()
                .code,
            "TARGET_NOT_FOUND"
        );
        // Unknown tier fails closed.
        let mut bad = contract_fixture();
        bad["elements"][0]["tier"] = json!("bogus");
        assert_eq!(
            reference_targets(&bad, "forms/default").unwrap_err().code,
            "INVALID_ARGUMENT"
        );
    }
    #[test]
    fn compare_layout_parts_hidden_and_out_of_scope() {
        let targets = reference_targets(&contract_fixture(), "forms/default").unwrap();
        // Native: checkbox owner only, no parts; later.widget present;
        // other.widget absent from contract perspective is fine — it IS in
        // the expected map (later tier, out of scope under default core).
        let mut s = fixture();
        s.root.automation_id = Some("checkbox.default.unchecked".into());
        s.root.rect_dp = Rect {
            x: 204.,
            y: 1265.5,
            width: 16.,
            height: 16.,
        };
        s.root.children = vec![Node {
            automation_id: Some("later.widget".into()),
            kind: "custom".into(),
            label: String::new(),
            rect_dp: Rect {
                x: 0.,
                y: 0.,
                width: 10.,
                height: 10.,
            },
            rect_px: Rect {
                x: 0.,
                y: 0.,
                width: 13.,
                height: 13.,
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
        }];
        let core = TierScope::parse(None).unwrap();
        let v = compare_layout(&targets, &s, 0., "absolute", None, &core).unwrap();
        // core rows: element + label + ghost; later targets excluded/counted.
        assert_eq!(v["scope"]["excluded_by_tier"], 2);
        assert_eq!(v["out_of_scope"], json!(["later.widget"]));
        assert_eq!(v["unexpected"], json!([]));
        // label is a visible expected part absent natively -> missing.
        assert_eq!(v["missing"], 1);
        // ghost expected hidden and absent natively -> its row passes.
        let ghost_row = v["comparisons"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == "checkbox.default.unchecked#ghost")
            .unwrap();
        assert_eq!(ghost_row["pass"], true);
        // Hidden expected but rendered natively -> explicit failure.
        s.root.parts = vec![PartNode {
            key: "ghost".into(),
            rect_dp: Rect {
                x: 0.,
                y: 0.,
                width: 4.,
                height: 4.,
            },
            rect_px: Rect {
                x: 0.,
                y: 0.,
                width: 5.,
                height: 5.,
            },
            visible: true,
            reason: None,
        }];
        let v = compare_layout(&targets, &s, 0., "absolute", None, &core).unwrap();
        let ghost_row = v["comparisons"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == "checkbox.default.unchecked#ghost")
            .unwrap();
        assert_eq!(ghost_row["pass"], false);
        assert_eq!(ghost_row["reason"], "not-rendered");
        // Visible part compares like an element (absolute rect derivation).
        s.root.parts = vec![PartNode {
            key: "label".into(),
            rect_dp: Rect {
                x: 228.,
                y: 1266.5,
                width: 71.984,
                height: 14.,
            },
            rect_px: Rect {
                x: 285.,
                y: 1583.,
                width: 90.,
                height: 18.,
            },
            visible: true,
            reason: None,
        }];
        let v = compare_layout(
            &targets,
            &s,
            0.,
            "absolute",
            Some("checkbox.default.unchecked#label"),
            &core,
        )
        .unwrap();
        assert_eq!(v["pass"], true);
        assert_eq!(v["comparisons"].as_array().unwrap().len(), 1);
        // Explicit --id bypasses the tier filter.
        let v = compare_layout(&targets, &s, 0., "absolute", Some("other.widget"), &core).unwrap();
        assert_eq!(v["missing"], 1);
        // Duplicate (owner,part) rejected.
        s.root.parts.push(PartNode {
            key: "label".into(),
            rect_dp: Rect {
                x: 0.,
                y: 0.,
                width: 1.,
                height: 1.,
            },
            rect_px: Rect {
                x: 0.,
                y: 0.,
                width: 1.,
                height: 1.,
            },
            visible: true,
            reason: None,
        });
        assert!(s.part_index().is_err());
        // Snapshot 0.2 strictness: the old schema string is rejected.
        s.schema = "rust-ui.devctl.snapshot-layout/0.1".into();
        assert!(s.index().is_err());
        assert!(snapshot_targets(&s).is_err());
    }
    #[test]
    fn semantic_key_parse_display() {
        let k = SemanticKey::parse("owner#group[a].item[b].icon").unwrap();
        assert_eq!(k.owner, "owner");
        assert_eq!(k.part.as_deref(), Some("group[a].item[b].icon"));
        assert_eq!(k.display(), "owner#group[a].item[b].icon");
        assert!(SemanticKey::parse("owner").unwrap().part.is_none());
        for bad in ["", "#", "owner#", "#part", "a#b#c"] {
            assert!(SemanticKey::parse(bad).is_err(), "{bad}");
        }
    }
