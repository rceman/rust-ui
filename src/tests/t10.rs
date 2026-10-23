/// P11: bounded lifecycle — 100 mount/reorder/remove/recreate cycles must
/// release each peer exactly once and return live node/peer counts to
/// baseline (no retained generation keeps accumulating).
#[test]
fn hundred_mount_remove_cycles_reclaim_all() {
    struct S {
        v: TextValue,
        mounted: bool,
    }
    let mut rig = Rig::new(
        S {
            v: TextValue::new("x"),
            mounted: true,
        },
        |_: &mut S, _m: Msg, _cx: &mut UpdateCtx<Msg>| {},
        |s: &S, ui: &mut Ui<Msg>| {
            ui.group("root", |ui| {
                if s.mounted {
                    ui.group("ed", |ui| {
                        ui.text_input(&s.v).on_edit(Msg::Edited);
                    });
                }
            });
        },
    );
    rig.view().unwrap();
    let baseline_peers = rig
        .peers
        .recs
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .len();
    for i in 0..100 {
        rig.rt.state.mounted = i % 2 == 0;
        rig.view().unwrap();
        rig.pump().unwrap();
    }
    // odd i unmounts, even remounts — end with the editor mounted
    rig.rt.state.mounted = true;
    rig.view().unwrap();
    rig.pump().unwrap();
    let recs = rig.peers.recs.lock().unwrap_or_else(|e| e.into_inner());
    let mounts = recs.len() - baseline_peers;
    let releases: u32 = recs.values().map(|r| r.released).sum();
    // every created peer released exactly once; at most one remains live
    assert!(mounts >= 48, "expected >=48 mounts, got {mounts}");
    assert!(
        mounts as u32 - releases <= 1,
        "live peers = {} — must be <= 1",
        mounts as u32 - releases
    );
    assert!(
        recs.values().all(|r| r.released <= 1),
        "no peer released twice"
    );
    // slots/generations stay bounded — a freed slot is reusable, not grown
    assert!(rig.rt.arena.slot_len() < 64, "arena must recycle slots");
}

// ============================================================================
// Platform-contract geometry conformance — docs/PLATFORM_CONTRACTS.md
// ============================================================================
// The SHARED semantic authority is `crate::geom` — these tests prove the
// conversion mathematics ONCE for every backend, at every canonical scale.
// Windows adapter tests (probe, UIA) separately prove *which* space a given
// native API speaks; they never re-derive this math.

#[test]
fn geometry_contract_scale_identity() {
    use crate::geom::ScaleFactor;
    // shared semantics: positive finite px-per-Dp ratios — no OS baseline
    for want in [1.0f32, 1.25, 1.5, 2.0] {
        let s = ScaleFactor::new(want).expect("valid ratio");
        assert!((s.0 - want).abs() < 1e-6);
        assert!(s.valid());
    }
    // invalid ratios are refused — a zero/negative/NaN scale can never
    // smuggle broken geometry into the shared model
    for bad in [0.0f32, -1.25, f32::NAN, f32::INFINITY] {
        assert!(ScaleFactor::new(bad).is_none());
        assert!(!ScaleFactor(bad).valid());
    }
}

/// Win32 adapter contract: monitor DPI <-> ratio mapping lives in
/// `platform::win32::space` — the shared layer never sees `96`. This is a
/// Windows-adapter test (portable ratio tests live in geom).
#[cfg(windows)]
#[test]
fn win32_scale_from_dpi_mapping() {
    use crate::platform::win32::space::{dpi_of, scale_from_dpi};
    for (dpi, want) in [(96u32, 1.0f32), (120, 1.25), (144, 1.5), (192, 2.0)] {
        let s = scale_from_dpi(dpi);
        assert!((s.0 - want).abs() < 1e-6, "dpi {dpi}");
        assert_eq!(dpi_of(s), dpi, "round-trip dpi {dpi}");
    }
}

#[test]
fn geometry_contract_logical_to_physical() {
    use crate::geom::ScaleFactor;
    // exactly-representable conversions are exact at every scale
    for &(ratio, px96) in &[(1.0f32, 96i32), (1.25, 120), (1.5, 144), (2.0, 192)] {
        let s = ScaleFactor(ratio);
        assert_eq!(s.to_physical(96.0), px96, "96dp@x{ratio}");
        assert_eq!(s.to_physical(0.0), 0);
        assert_eq!(s.to_physical(-8.0), -((8.0 * s.0).round()) as i32);
    }
    // rounding policy: round-half-away-from-zero — the ONLY policy
    let s = ScaleFactor::ONE;
    assert_eq!(s.to_physical(0.49), 0);
    assert_eq!(s.to_physical(0.5), 1);
    assert_eq!(s.to_physical(0.51), 1);
    assert_eq!(s.to_physical(-0.5), -1); // half-away, not half-even
    // 1.25x: 0.4dp -> 0.5px -> rounds to 1 (not 0)
    assert_eq!(ScaleFactor(1.25).to_physical(0.4), 1);
    assert_eq!(ScaleFactor(1.25).to_physical(1.6), 2); // 2.0 exact
    assert_eq!(ScaleFactor(1.5).to_physical(2.5), 4); // 3.75 -> 4
    // EXACT representability boundary — the review's counterexample:
    // 2^31 f32 is representable as f32 but NOT as i32; the checked path
    // must reject it rather than saturate to i32::MAX.
    assert_eq!(s.try_to_physical(2147483648.0), None);
    assert_eq!(s.try_to_physical(-2147483648.0), Some(i32::MIN));
    assert_eq!(s.try_to_physical(2147483520.0), Some(2147483520)); // max good f32
    assert_eq!(s.try_to_physical(f32::NAN), None);
    assert_eq!(s.try_to_physical(f32::INFINITY), None);
    assert_eq!(s.try_to_physical(f32::NEG_INFINITY), None);
}

#[test]
fn geometry_contract_round_trip() {
    use crate::geom::ScaleFactor;
    // logical -> px -> logical stays within half a physical pixel of the
    // source value (the maximum rounding error, by construction)
    for &ratio in &[1.0f32, 1.25, 1.5, 2.0] {
        let s = ScaleFactor(ratio);
        let tol = 0.5 / s.0 + 1e-4;
        let mut v = -32.0f32;
        while v <= 1024.0 {
            let rt = s.to_logical(s.to_physical(v));
            assert!(
                (rt - v).abs() <= tol,
                "round-trip @x{ratio}: {v} -> {} -> {rt}",
                s.to_physical(v)
            );
            v += 0.1;
        }
    }
}

#[test]
fn geometry_contract_rect_conversion() {
    use crate::geom::{Point, Rect, ScaleFactor};
    for &ratio in &[1.0f32, 1.25, 1.5, 2.0] {
        let s = ScaleFactor(ratio);
        let r = Rect {
            x: 34.0,
            y: 167.0,
            width: 336.0,
            height: 22.0,
        };
        let p = r.physical(s);
        // edges snap independently — width is never derived from two
        // rounded edges inside the impl
        assert_eq!(p.left, s.to_physical(34.0));
        assert_eq!(p.right, s.to_physical(370.0));
        let back = p.logical(s);
        let tol = 0.5 / s.0 + 1e-4;
        assert!((back.x - 34.0).abs() <= tol);
        assert!((back.y - 167.0).abs() <= tol);
        assert!((back.right() - 370.0).abs() <= 2.0 * tol);
    }
    // zero-size and negative-origin rects survive
    let z = Rect::local(0.0, 0.0).physical(ScaleFactor(1.25));
    assert_eq!(z.size().w, 0);
    let neg = Rect {
        x: -20.0,
        y: -8.0,
        width: 10.0,
        height: 4.0,
    }
    .physical(ScaleFactor(1.5));
    assert!(neg.left < 0 && neg.top < 0, "screen px may be negative");

    // geometry helpers shared with the retained side
    let r = Rect {
        x: 10.0,
        y: 10.0,
        width: 40.0,
        height: 20.0,
    };
    assert!(r.contains(Point { x: 10.0, y: 10.0 }));
    assert!(!r.contains(Point { x: 50.0, y: 10.0 })); // exclusive edge
    let u = r.union(Rect {
        x: 0.0,
        y: 0.0,
        width: 5.0,
        height: 5.0,
    });
    assert_eq!((u.x, u.y), (0.0, 0.0));
    assert_eq!(u.right(), 50.0);
}

/// F03 regression — the reviewer's reproduction. A peer at fractional
/// logical origin under a fractional scale must resolve ONE snapped
/// physical origin; pointer/caret/host callbacks share it. The bug was
/// two competing models giving different answers at the same pixel.
#[test]
fn peer_origin_single_snapped_transform() {
    use crate::geom::{
        ClientPhysicalPoint, PeerLocalPoint, PeerOrigin, PhysicalPoint, Rect, ScaleFactor,
    };
    // scale=1.25, logical origin 0.4 -> snapped physical origin:
    // round-half-away(0.4 * 1.25) = round(0.5) = 1px
    let o = PeerOrigin::from_logical(
        Rect {
            x: 0.4,
            y: 0.4,
            width: 100.0,
            height: 20.0,
        },
        ScaleFactor(1.25),
    )
    .expect("representable bounds snap");
    assert_eq!(o.0, PhysicalPoint { x: 1, y: 1 });
    // client px 1 -> peer-local 0 under the SHARED snapped transform
    // (the alternate "subtract-then-rescale" path answered 1 — removed)
    assert_eq!(
        o.to_local(ClientPhysicalPoint(PhysicalPoint { x: 1, y: 1 })),
        Some(PeerLocalPoint(PhysicalPoint { x: 0, y: 0 }))
    );
    // round-trip is exact in integer px
    let c = ClientPhysicalPoint(PhysicalPoint { x: 42, y: 7 });
    assert_eq!(o.to_local(c).and_then(|l| o.to_client(l)), Some(c));
    // negative peer-local (pointer above/left of the surface) is legal
    let neg = o.to_local(ClientPhysicalPoint(PhysicalPoint { x: -3, y: 0 }));
    assert_eq!(neg.map(|l| l.0), Some(PhysicalPoint { x: -4, y: -1 }));
    assert_eq!(
        neg.and_then(|l| o.to_client(l)).map(|c| c.0),
        Some(PhysicalPoint { x: -3, y: 0 })
    );
    // representability is CHECKED — extremes are None, never wrapped
    let omin = PeerOrigin(PhysicalPoint {
        x: i32::MIN,
        y: i32::MIN,
    });
    assert_eq!(
        omin.to_local(ClientPhysicalPoint(PhysicalPoint { x: 1, y: 0 })),
        None
    );

    // unrepresentable origin coordinates -> construction fails (None),
    // never a silently saturated PeerOrigin
    assert_eq!(
        PeerOrigin::from_logical(
            Rect {
                x: 2147483648.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
            ScaleFactor(1.0),
        ),
        None
    );
    let omax = PeerOrigin(PhysicalPoint {
        x: i32::MAX,
        y: i32::MAX,
    });
    assert_eq!(
        omax.to_client(PeerLocalPoint(PhysicalPoint { x: 1, y: 0 })),
        None
    );
    assert_eq!(ScaleFactor(1.5).try_to_physical(1e20), None);
    assert_eq!(ScaleFactor(1.5).try_to_physical(f32::NAN), None);
    assert_eq!(ScaleFactor(1.5).try_to_physical(64.0), Some(96));
    // another fractional case at a different scale — origin 8.2dp @1.5
    let o2 = PeerOrigin::from_logical(
        Rect {
            x: 8.2,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        },
        ScaleFactor(1.5),
    )
    .expect("representable");
    assert_eq!(o2.0.x, 12); // 8.2 * 1.5 = 12.3 -> 12
}

#[test]
fn geometry_contract_scale_change_is_pure() {
    use crate::geom::{Rect, ScaleFactor};
    // a scale change is a pure function of (logical, scale) — no hidden
    // latching; identical inputs always produce identical px geometry
    let r = Rect {
        x: 12.0,
        y: 3.0,
        width: 100.0,
        height: 40.0,
    };
    let at1 = r.physical(ScaleFactor(1.0));
    let at2 = r.physical(ScaleFactor(2.0));
    assert_eq!(at2.left, at1.left * 2);
    assert_eq!(at1, r.physical(ScaleFactor(1.0)));
    // fractional coords round *after* scaling — deterministic, not
    // commutable: 12.5dp is 13px@96 but 25px@192 (not 26)
    let f = Rect {
        x: 12.5,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    assert_eq!(f.physical(ScaleFactor(1.0)).left, 13);
    assert_eq!(f.physical(ScaleFactor(2.0)).left, 25);
}

// ===========================================================================
// F01/F06 regressions — native entry classes + the timer pool authority
// ===========================================================================
