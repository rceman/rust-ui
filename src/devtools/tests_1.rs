use super::*;

    use super::*;
    fn fixture() -> Snapshot {
        let r = Rect {
            x: 10.,
            y: 20.,
            width: 30.,
            height: 40.,
        };
        Snapshot {
            schema: SNAPSHOT_SCHEMA.into(),
            scale_factor: 1.25,
            client_dp: Rect {
                x: 0.,
                y: 0.,
                width: 100.,
                height: 100.,
            },
            client_px: Rect {
                x: 0.,
                y: 0.,
                width: 125.,
                height: 125.,
            },
            root: Node {
                automation_id: Some("sample.button".into()),
                kind: "button".into(),
                label: "test \"\\\n\r\t é".into(),
                rect_dp: r,
                rect_px: Rect {
                    x: 13.,
                    y: 25.,
                    width: 37.,
                    height: 50.,
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
    #[test]
    fn protocol_round_trip_and_json_escaping() {
        let r = Request {
            protocol: PROTOCOL.into(),
            token: "nonce".into(),
            id: 42,
            command: "tree".into(),
            arguments: json!({"s":"\"\\\n\r\t\u{1}日本"}),
        };
        let encoded = encode(&r).unwrap();
        let got: Request = read_frame(&mut &encoded[..]).unwrap();
        assert_eq!(got.arguments, r.arguments);
        assert_eq!(got.id, 42);
    }
    #[test]
    fn oversized_and_malformed_frames_fail() {
        assert_eq!(
            read_frame::<_, Request>(&mut &((MAX_MESSAGE + 1) as u32).to_le_bytes()[..])
                .unwrap_err()
                .code,
            "MESSAGE_TOO_LARGE"
        );
        assert!(decode::<Request>(b"{}").is_err());
        assert!(decode::<Value>(b"{wrong}").is_err());
    }
    #[test]
    fn response_error_code_preserved() {
        let r = Response::from_result(9, Err(Error::new("TARGET_NOT_FOUND", "probe")));
        assert!(!r.ok);
        assert_eq!(r.error.unwrap().code, "TARGET_NOT_FOUND");
        assert!(r.result.is_none());
    }
    #[test]
    fn deterministic_snapshot_and_strict_duplicates() {
        let mut s = fixture();
        assert_eq!(
            serde_json::to_vec(&s).unwrap(),
            serde_json::to_vec(&s.clone()).unwrap()
        );
        let clone = s.root.clone();
        s.root.children.push(clone);
        assert!(s.index().is_err());
        s.root.children.clear();
        s.scale_factor = f64::NAN;
        assert!(s.index().is_err());
    }
    #[test]
    fn layout_self_and_one_dp_mutation() {
        let mut s = fixture();
        let expected = snapshot_targets(&s).unwrap();
        let core = TierScope::parse(None).unwrap();
        assert_eq!(
            compare_layout(&expected, &s, 0., "absolute", None, &core).unwrap()["pass"],
            true
        );
        s.root.rect_dp.x += 1.;
        let result = compare_layout(&expected, &s, 0., "absolute", None, &core).unwrap();
        assert_eq!(result["pass"], false);
        assert_eq!(result["comparisons"][0]["delta"]["x"], 1.);
        assert!(compare_layout(&expected, &s, 0., "absolute", Some("missing"), &core).is_err());
        assert!(compare_layout(&expected, &s, f64::NAN, "absolute", None, &core).is_err());
    }
    #[test]
    fn image_self_and_one_pixel_bbox() {
        let a = Image {
            width: 3,
            height: 2,
            rgba: vec![0; 24],
        };
        let mut b = a.clone();
        assert_eq!(compare_image(&a, &b, 0).unwrap().0["mismatched_pixels"], 0);
        b.rgba[(1 * 3 + 2) * 4] = 255;
        let (v, diff) = compare_image(&a, &b, 0).unwrap();
        assert_eq!(v["mismatched_pixels"], 1);
        assert_eq!(
            v["difference_bbox"],
            json!({"x":2.,"y":1.,"width":1.,"height":1.})
        );
        assert_eq!(v["max_channel_delta"], 255);
        assert_eq!(diff.unwrap().rgba[(1 * 3 + 2) * 4], 255);
    }
    #[test]
    fn crop_visible_bounds_and_padding() {
        let a = Image {
            width: 3,
            height: 2,
            rgba: vec![128; 24],
        };
        let r = snap_rect(
            Rect {
                x: 1.,
                y: 0.,
                width: 2.,
                height: 2.,
            },
            1.0,
        )
        .unwrap();
        let b = a.crop(&r).unwrap();
        assert_eq!((b.width, b.height), (2, 2));
        assert_eq!(r.left, 1);
        // Padding grows the LOGICAL rect before snapping (compare-element),
        // or the snapped px rect directly (screenshot evidence crop).
        let grown = snap_rect(
            Rect {
                x: 1.,
                y: 0.,
                width: 2.,
                height: 2.,
            }
            .grow(1.),
            1.0,
        )
        .unwrap();
        assert_eq!(
            a.crop(&grown).err().unwrap().code,
            "TARGET_NOT_VISIBLE",
            "padded crop outside image must fail closed"
        );
        assert_eq!(
            a.crop(&grow_px(r, 1)).err().unwrap().code,
            "TARGET_NOT_VISIBLE"
        );
    }
    #[test]
    fn png_round_trip() {
        let a = Image {
            width: 3,
            height: 2,
            rgba: vec![128; 24],
        };
        let p = std::env::temp_dir().join(format!("rust-ui-devctl-png-{}.png", std::process::id()));
        a.save(&p).unwrap();
        let b = Image::load(&p).unwrap();
        std::fs::remove_file(p).unwrap();
        assert_eq!(a.rgba, b.rgba);
    }
    #[test]
    fn frozen_reference_adapter_uses_capture_identity() {
        let v = json!({"$schema":"rust-ui.shadcn-reference.contract/0.2","elements":[{"automation_id":"button.default","tier":"core","captures":{"components/default":{"rect":{"x":1,"y":2,"width":30,"height":32},"parts":{}},"components/pressed":{"rect":{"x":1,"y":3,"width":30,"height":32},"parts":{}}}}]});
        let key = SemanticKey::parse("button.default").unwrap();
        assert_eq!(
            reference_targets(&v, "components/pressed").unwrap()[&key]
                .rect
                .unwrap()
                .y,
            3.
        );
        assert!(reference_targets(&v, "missing/default").is_err());
    }
    #[test]
    fn unequal_dimensions_produce_diff_without_resizing() {
        let a = Image {
            width: 2,
            height: 2,
            rgba: vec![255; 16],
        };
        let b = Image {
            width: 1,
            height: 2,
            rgba: vec![255; 8],
        };
        let (v, diff) = compare_image(&a, &b, 0).unwrap();
        assert_eq!(v["dimensions_match"], false);
        assert_eq!(v["missing_pixels"], 2);
        assert_eq!(
            v["difference_bbox"],
            json!({"x":1.,"y":0.,"width":1.,"height":2.})
        );
        assert_eq!(diff.unwrap().width, 2);
    }
    #[test]
    fn image_invalid_extent_rejected() {
        assert!(
            compare_image(
                &Image {
                    width: 0,
                    height: 0,
                    rgba: vec![]
                },
                &Image {
                    width: 0,
                    height: 0,
                    rgba: vec![]
                },
                0
            )
            .is_err()
        );
        assert!(
            Image {
                width: 3,
                height: 3,
                rgba: vec![0; 4]
            }
            .validate()
            .is_err()
        );
    }
    // --- D02: single snap authority -------------------------------------
    fn px(r: &PixelRect) -> (i64, i64, i64, i64) {
        (
            i64::from(r.left),
            i64::from(r.top),
            i64::from(r.right),
            i64::from(r.bottom),
        )
    }
    #[test]
    fn snap_rect_edge_semantics() {
        // Integer edges stay exact at every scale.
        let r = Rect {
            x: 10.,
            y: 20.,
            width: 30.,
            height: 40.,
        };
        for s in [1.0, 1.25, 1.5, 2.0] {
            let p = snap_rect(r, s).unwrap();
            assert_eq!(px(&p).0, (10. * s).round() as i64);
            assert_eq!(px(&p).1, (20. * s).round() as i64);
            // Width is right-left of snapped edges, never round(width*s) —
            // x=10,w=30 at s=1.25: 12.5->13 and 50->50 gives 37, not 38.
            assert_eq!(
                px(&p).2 - px(&p).0,
                ((40. * s).round() - (10. * s).round()) as i64
            );
        }
        // Fractional origin/size: each edge rounds half away from zero —
        // the width is never rounded directly.
        let f = snap_rect(
            Rect {
                x: 300.75,
                y: 0.25,
                width: 32.5,
                height: 32.75,
            },
            1.0,
        )
        .unwrap();
        assert_eq!(px(&f), (301, 0, 333, 33));
        // Frozen button.default (y=300.75, h=32) snaps to 301..333 = 32 px.
        let b = snap_rect(
            Rect {
                x: 0.,
                y: 300.75,
                width: 120.,
                height: 32.,
            },
            1.0,
        )
        .unwrap();
        assert_eq!((b.top, b.bottom), (301, 333));
        // Fractional right/bottom edges round independently.
        let e = snap_rect(
            Rect {
                x: 0.4,
                y: 0.6,
                width: 9.1,
                height: 9.9,
            },
            1.0,
        )
        .unwrap();
        assert_eq!(px(&e), (0, 1, 10, 11));
        // Near-zero origin still snaps, but a rect outside the image fails
        // bounds when cropped.
        let small = Image {
            width: 4,
            height: 4,
            rgba: vec![0; 64],
        };
        let neg = snap_rect(
            Rect {
                x: -1.,
                y: 0.,
                width: 2.,
                height: 2.,
            },
            1.0,
        )
        .unwrap();
        assert_eq!(neg.left, -1);
        assert_eq!(small.crop(&neg).err().unwrap().code, "TARGET_NOT_VISIBLE");
        assert_eq!(snap_rect(r, 0.).unwrap_err().code, "INVALID_ARGUMENT");
        assert_eq!(snap_rect(r, f64::NAN).unwrap_err().code, "INVALID_ARGUMENT");
        assert!(
            snap_rect(
                Rect {
                    x: 0.,
                    y: 0.,
                    width: f64::INFINITY,
                    height: 1.
                },
                1.0
            )
            .is_err()
        );
    }
    /// The previous crop policy (floor origin/ceil far edge), kept here only
    /// to audit the frozen contract against the new authority.
    fn old_crop_edges(r: Rect) -> (i64, i64, i64, i64) {
        (
            r.x.floor() as i64,
            r.y.floor() as i64,
            (r.x + r.width).ceil() as i64,
            (r.y + r.height).ceil() as i64,
        )
    }
