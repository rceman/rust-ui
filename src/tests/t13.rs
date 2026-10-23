#[test]
fn staged_selection_reaches_peer() {
    struct St {
        tv: TextValue,
    }
    let mut rig = Rig::new(
        St {
            tv: TextValue::new("Select this phrase"),
        },
        |_, _, _| {},
        |s, ui| {
            ui.input(&s.tv).selection(0, 6);
        },
    );
    rig.view().unwrap();
    let stack = rig.rt.arena.get(rig.root_children()[0]).unwrap();
    let leaf = crate::NodeId {
        slot: stack.children[0],
        generation: rig.rt.arena.generation_of(stack.children[0]),
    };
    // the selection lands in the retained Editor data …
    match &rig.rt.arena.get(leaf).unwrap().data {
        NodeData::Editor { selection, .. } => assert_eq!(*selection, Some((0, 6))),
        _ => panic!("expected editor leaf under the input stack"),
    }
    // … and is pushed to the peer (mount-time apply)
    let pid = rig.peer_id(leaf).expect("editor peer mounted");
    assert!(
        rig.peer_ops(pid)
            .iter()
            .any(|op| matches!(op, PeerOp::SetSelection(0, 6))),
        "peer must receive the staged selection: {:?}",
        rig.peer_ops(pid)
    );
    assert_eq!(rig.peer_rec(pid).sel, Some((0, 6)));
}

/// Presented flags are presentation-only — they never revive
/// interactivity. A disabled button stays inert under `.presented(..)`.
#[test]
fn presented_does_not_grant_interactivity() {
    use crate::node::Presented;
    let mut rig = Rig::new(
        (),
        |_, _, _| {},
        |_, ui| {
            ui.button("x").disabled(true).presented(Presented::HOVER);
        },
    );
    rig.view().unwrap();
    let id = rig.root_children()[0];
    let n = rig.rt.arena.get(id).unwrap();
    assert_eq!(n.presented, Presented::HOVER);
    assert!(
        !n.interactive(),
        "presented.hover must not make a disabled node interactive"
    );
}

/// The frozen contract's exact 20dp line box: `.line_height(dp(20))`
/// reaches the peer as `line_twips` and msftedit's D2DDWRITE layout
/// honors it via the calibrated yHeight (it ignores every paragraph
/// line-spacing control — EM_SETPARAFORMAT / TxGetParaFormat / TOM /
/// RTF all measured inert). PIXEL TRUTH: the doc must draw 3 bands at
/// 20px pitch fully inside the 62dp content rect. (EM_REQUESTRESIZE is
/// stale by platform quirk — it still reports the natural 75 — so the
/// assertion is on the drawn framebuffer, not the reported extent.)
#[test]
#[cfg(target_os = "windows")]
fn multiline_exact_line_spacing() {
    let lib = crate::platform::win32::Msftedit::load().expect("msftedit");
    let hwnd = probe_hwnd();
    use crate::platform::win32::space::{LogicalRect, ScaleFactor};
    let cfg = || crate::platform::win32::PeerConfig {
        multiline: true,
        read_only: false,
        face: "Segoe UI".into(),
        size_twips: 280,
        line_twips: 300,
        fg: [1., 1., 1., 1.],
        fg_authored: crate::style::Color::rgba(200, 30, 30, 255),
        sel_bg: [0.2, 0.4, 0.8, 1.],
        sel_fg: [1., 1., 1., 1.],
        bold: false,
    };
    let sink = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut peer = crate::platform::win32::WindowlessPeer::create(
        1,
        &lib,
        hwnd,
        ScaleFactor(1.0),
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
        "First line of a note.\nSecond line with more text.\nThird line closes it out.",
        crate::text::TextRevision::mint(),
        binding,
    )
    .expect("init");
    peer.apply_bounds(
        LogicalRect {
            x: 0.,
            y: 0.,
            width: 340.,
            height: 62.,
        },
        ScaleFactor(1.0),
    )
    .expect("bounds");
    // live mount order: the backend pushes the resolved foreground over
    // all content (and again on theme flips) — the pushed CHARFORMAT
    // carries host.cf.yHeight, so any service-side charformat writeback
    // would leak into the drawn pitch
    peer.apply_format(
        windows::Win32::Foundation::COLORREF(0x00FFFFFF),
        crate::style::Color::rgba(255, 255, 255, 255),
    );
    let fb = draw_peer_dib(&peer);
    let ink = ink_rows(&fb, 800, 62);
    let bands = ink_bands(&ink);
    assert_eq!(
        bands.len(),
        3,
        "3 lines must draw as 3 ink bands, got {bands:?}"
    );
    // pitch = band-top deltas ≈ 20px (measured 5,25,45 on this host)
    let pitch = (bands[2].0 - bands[0].0) as f32 / 2.0;
    assert!(
        (pitch - 20.0).abs() <= 1.5,
        "drawn line pitch must be ≈20px, got {pitch} (bands {bands:?})"
    );
    let last = ink.iter().rposition(|&c| c > 0).unwrap_or(0);
    assert!(
        last <= 61,
        "all glyph ink must fit the 62dp content rect, last ink row {last}"
    );
    // honest record: REQRESIZE stays stale (~75) — drawn truth is the gate
    if let Ok((_, h)) = peer.natural_size(340.0) {
        println!("note: stale EM_REQUESTRESIZE extent h={h} (drawn pitch is authoritative)");
    }
}

/// (windows diag) draw the activated peer through the production
/// TxDrawD2D→bitmap→DrawBitmap path into a black 800x600 DIB and return
/// the BGRA framebuffer.
#[cfg(windows)]
fn draw_peer_dib(peer: &crate::platform::win32::WindowlessPeer) -> Vec<u8> {
    draw_peer_dib_on(peer, [0., 0., 0., 1.])
}

/// draw_peer_dib with an explicit clear color — a dark band must be
/// distinguished from the canvas, not just the glyph ink
#[cfg(windows)]
fn draw_peer_dib_on(peer: &crate::platform::win32::WindowlessPeer, clear: [f32; 4]) -> Vec<u8> {
    use crate::platform::win32::space::LogicalRect;
    use windows::Win32::Foundation::RECT;
    use windows::Win32::Graphics::Direct2D::Common::*;
    use windows::Win32::Graphics::Direct2D::*;
    use windows::Win32::Graphics::Dxgi::Common::*;
    use windows::Win32::Graphics::Gdi::*;
    use windows::core::Interface;
    const FBW: i32 = 800;
    const FBH: i32 = 600;
    unsafe {
        let screen = GetDC(None);
        let memdc = CreateCompatibleDC(Some(screen));
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut bmi = BITMAPINFO::default();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = FBW;
        bmi.bmiHeader.biHeight = -FBH;
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB.0;
        let hbmp =
            CreateDIBSection(Some(memdc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0).expect("dib");
        SelectObject(memdc, HGDIOBJ(hbmp.0));
        let d2d: ID2D1Factory =
            D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None).expect("d2d");
        let props = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_SOFTWARE,
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_IGNORE,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            ..Default::default()
        };
        let dcrt = d2d.CreateDCRenderTarget(&props).expect("dc rt");
        dcrt.BindDC(
            memdc,
            &RECT {
                left: 0,
                top: 0,
                right: FBW,
                bottom: FBH,
            },
        )
        .expect("BindDC");
        let rt: ID2D1RenderTarget = dcrt.cast().expect("cast");
        rt.BeginDraw();
        rt.Clear(Some(&D2D1_COLOR_F {
            r: clear[0],
            g: clear[1],
            b: clear[2],
            a: clear[3],
        }));
        peer.draw(
            &rt,
            LogicalRect {
                x: 0.,
                y: 0.,
                width: 340.,
                height: 200.,
            },
            0,
        )
        .expect("draw");
        rt.EndDraw(None, None).expect("end");
        std::slice::from_raw_parts(bits as *const u8, (FBW * FBH * 4) as usize).to_vec()
    }
}

/// (windows diag) per-row ink counts: pixels brighter than the black
/// clear (glyph fg is white) inside the peer's content columns.
#[cfg(windows)]
fn ink_rows(frame: &[u8], fbw: usize, rows: usize) -> Vec<usize> {
    (0..rows)
        .map(|y| {
            (0..340usize)
                .filter(|&x| {
                    let o = (y * fbw + x) * 4;
                    frame[o] > 96 || frame[o + 1] > 96 || frame[o + 2] > 96
                })
                .count()
        })
        .collect()
}

/// (windows diag) contiguous ink runs (>=1 empty row breaks a band) —
/// (top, bottom_inclusive, ink_rows).
#[cfg(windows)]
fn ink_bands(ink: &[usize]) -> Vec<(usize, usize, usize)> {
    let mut bands = Vec::new();
    let mut cur: Option<(usize, usize, usize)> = None;
    for (y, &c) in ink.iter().enumerate() {
        if c > 0 {
            match &mut cur {
                Some((_, b, n)) => {
                    *b = y;
                    *n += 1;
                }
                None => cur = Some((y, y, 1)),
            }
        } else if let Some(b) = cur.take() {
            bands.push(b);
        }
    }
    if let Some(b) = cur.take() {
        bands.push(b);
    }
    bands
}

/// A staged selection survives the real mount order: the runtime applies
/// it while the peer is still Inactive (before the first measure/bounds
/// activates the service), and it must still be there — AND still paint —
/// after activation. Regression cover for the PeerHandle `apply_selection`
/// forwarder the live path once lacked.
#[test]
#[cfg(target_os = "windows")]
fn staged_selection_survives_first_activation() {
    let lib = crate::platform::win32::Msftedit::load().expect("msftedit");
    let hwnd = probe_hwnd();
    use crate::platform::win32::space::{LogicalRect, ScaleFactor};
    let cfg = || crate::platform::win32::PeerConfig {
        multiline: true,
        read_only: false,
        face: "Segoe UI".into(),
        size_twips: 280,
        line_twips: 300,
        fg: [1., 1., 1., 1.],
        fg_authored: crate::style::Color::rgba(200, 30, 30, 255),
        sel_bg: [0.2, 0.4, 0.8, 1.],
        sel_fg: [1., 1., 1., 1.],
        bold: false,
    };
    let sink = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut peer = crate::platform::win32::WindowlessPeer::create(
        1,
        &lib,
        hwnd,
        ScaleFactor(1.0),
        &cfg(),
        sink,
        std::sync::Arc::new(std::sync::Mutex::new(
            crate::platform::win32::text::TimerPool::new(),
        )),
    )
    .expect("peer create");
    let binding = crate::text::BindingToken::mint();
    // runtime mount order: initialize (TxSetText) then the staged
    // selection — all BEFORE the peer sees its first non-empty bounds
    crate::node::TextPeer::initialize(
        &mut peer,
        "First line of a note.\nSecond line with more text.\nThird line closes it out.",
        crate::text::TextRevision::mint(),
        binding,
    )
    .expect("init");
    crate::node::TextPeer::apply_selection(&mut peer, Some((22, 33))).expect("staged sel");
    // layout: natural_size activates in-place, then real bounds land
    peer.natural_size(340.0).expect("natural_size");
    peer.apply_bounds(
        LogicalRect {
            x: 0.,
            y: 0.,
            width: 340.,
            height: 62.,
        },
        ScaleFactor(1.0),
    )
    .expect("bounds");
    assert_eq!(
        peer.selection_utf16().ok(),
        Some((22, 33)),
        "staged selection must survive mount-time activation"
    );

    // ---- the unfocused staged range still paints selection-colored ink
    use windows::Win32::Foundation::RECT;
    use windows::Win32::Graphics::Direct2D::Common::*;
    use windows::Win32::Graphics::Direct2D::*;
    use windows::Win32::Graphics::Dxgi::Common::*;
    use windows::Win32::Graphics::Gdi::*;
    use windows::core::Interface;
    const FBW: i32 = 800;
    const FBH: i32 = 600;
    let (bits, dcrt) = unsafe {
        let screen = GetDC(None);
        let memdc = CreateCompatibleDC(Some(screen));
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut bmi = BITMAPINFO::default();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = FBW;
        bmi.bmiHeader.biHeight = -FBH;
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB.0;
        let hbmp =
            CreateDIBSection(Some(memdc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0).expect("dib");
        SelectObject(memdc, HGDIOBJ(hbmp.0));
        let d2d: ID2D1Factory =
            D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None).expect("d2d");
        let props = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_SOFTWARE,
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_IGNORE,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            ..Default::default()
        };
        let dcrt = d2d.CreateDCRenderTarget(&props).expect("dc rt");
        dcrt.BindDC(
            memdc,
            &RECT {
                left: 0,
                top: 0,
                right: FBW,
                bottom: FBH,
            },
        )
        .expect("BindDC");
        (bits, dcrt)
    };
    let rt: ID2D1RenderTarget = dcrt.cast().expect("cast");
    let island = LogicalRect {
        x: 0.,
        y: 0.,
        width: 340.,
        height: 62.,
    };
    unsafe {
        rt.BeginDraw();
        rt.Clear(Some(&D2D1_COLOR_F {
            r: 1.,
            g: 1.,
            b: 1.,
            a: 1.,
        }));
    }
    peer.draw(&rt, island, 0).expect("draw");
    unsafe { rt.EndDraw(None, None).expect("end") };
    let data = unsafe { std::slice::from_raw_parts(bits as *const u8, (FBW * FBH * 4) as usize) };
    // band = 0.4·sel_bg + 0.6·win_bg under D2DDWRITE → a (0.2,0.4,0.8)
    // highlight over win_bg #010101 lands ≈ (21,42,83) — dark blue, not
    // the saturated source color
    let mut blue = 0usize;
    for y in 0..FBH {
        for x in 0..FBW {
            let o = (y * FBW + x) as usize * 4;
            let (b, r) = (data[o] as i32, data[o + 2] as i32);
            if b > 60 && b > r + 20 {
                blue += 1;
            }
        }
    }
    assert!(
        blue > 300,
        "staged selection must paint selection-colored ink, got {blue}px"
    );
}
