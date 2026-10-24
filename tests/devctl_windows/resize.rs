#[test]
fn native_resize_retained_layout_converges() {
    let exe = Path::new(env!("CARGO_BIN_EXE_rust-ui-gallery")).to_path_buf();
    assert!(exe.exists(), "Gallery launch target must exist");
    let file = std::env::temp_dir().join(format!(
        "rust-ui-resize-{}.json",
        d::client::nonce().unwrap()
    ));
    let _owner = Session { file: file.clone() };
    let session = file.to_str().unwrap();
    good(&[
        "launch",
        "--exe",
        exe.to_str().unwrap(),
        "--session",
        session,
    ]);
    action(session, "wait-idle", &[]);
    // window px, not client — successive steps mimic a drag's coalesced
    // WM_SIZE deliveries; each must re-lay out to the live client
    for (w, h) in [(1400, 1600), (964, 1600), (1500, 1700), (900, 1561)] {
        action(
            session,
            "resize",
            &["--width", &w.to_string(), "--height", &h.to_string()],
        );
        action(session, "wait-idle", &[]);
        let s = snap(&action(session, "snapshot-layout", &[]));
        let client = s.client_dp;
        assert_eq!(
            s.root.rect_dp.width, client.width,
            "root width must track client {w}x{h}"
        );
        assert_eq!(
            s.root.rect_dp.height, client.height,
            "root height must track client {w}x{h}"
        );
        // deepest laid-out edge must reach the live client — a stale
        // narrow layout leaves content ending short of the right edge
        fn right(n: &d::Node) -> f64 {
            let mut m = n.rect_dp.x + n.rect_dp.width;
            for c in &n.children {
                m = m.max(right(c));
            }
            m
        }
        let edge = right(&s.root);
        assert!(
            (edge - client.width).abs() < 2.0,
            "deepest edge {edge} must reach client width {} at {w}x{h}",
            client.width
        );
    }
}
