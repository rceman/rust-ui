// ======================== native probes (Windows-only) =====================

#[cfg(windows)]
thread_local! {
    static PROBE_HWND: std::cell::RefCell<windows::Win32::Foundation::HWND> =
        std::cell::RefCell::new(windows::Win32::Foundation::HWND::default());
}

#[cfg(windows)]
/// A real hidden host HWND for native-peer tests — msftedit's
/// TxGetDC/scale plumbing needs a live window, not a null handle.
#[cfg(windows)]
fn probe_hwnd() -> windows::Win32::Foundation::HWND {
    use windows::Win32::Foundation::*;
    use windows::Win32::UI::WindowsAndMessaging::*;
    use windows::core::*;
    unsafe {
        let _ = windows::Win32::System::Ole::OleInitialize(None);
        let _ = windows::Win32::System::Com::CoInitializeEx(
            None,
            windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
        );
        let cls = w!("rustui_probe_px");
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpszClassName: cls,
            hInstance: windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
                .unwrap()
                .into(),
            lpfnWndProc: Some(probe_wndproc),
            ..Default::default()
        };
        let _ = RegisterClassExW(&wc);
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            cls,
            w!("probe"),
            WS_OVERLAPPEDWINDOW,
            0,
            0,
            800,
            500,
            None,
            None,
            None,
            None,
        )
        .expect("probe hwnd")
    }
}

#[cfg(windows)]
unsafe extern "system" fn probe_wndproc(
    hwnd: windows::Win32::Foundation::HWND,
    msg: u32,
    wp: windows::Win32::Foundation::WPARAM,
    lp: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    windows::Win32::UI::WindowsAndMessaging::DefWindowProcW(hwnd, msg, wp, lp)
}

/// Deterministic offscreen regression for the windowless-RichEdit
/// coordinate contract. msftedit's `TxDrawD2D` host space is PHYSICAL
/// PIXELS at the host DC's dpi — the service divides `lprcBounds` by
/// `dcDpi/96` to reach the target's logical units. A DPI-UNAWARE host
/// makes px==DIP numerically, which is exactly how the translate-up
/// defect shipped: this probe therefore runs under per-monitor-v2 thread
/// awareness (the production model) and measures the real DC dpi instead
/// of assuming 96.
///
/// Per peer scale in {1.0, 1.25, 1.5, 2.0} the probe mounts text at
/// create time (the pre-mount path that regressed), applies DIP bounds,
/// and draws onto a DC render target bound to an in-memory DIB. The DIB
/// framebuffer is px-exact: ink must land at `DIP * scale` and only there.
/// Asserts: ink inside the requested island, zero glyph pixels outside it
/// (no duplicate/offset copy), and scale-invariant natural height at the
/// host's real DC scale.
#[cfg(windows)]
#[test]
fn native_probe_richedit_paints_text() {
    use windows::Win32::Foundation::*;
    use windows::Win32::Graphics::Direct2D::Common::*;
    use windows::Win32::Graphics::Direct2D::*;
    use windows::Win32::Graphics::Dxgi::Common::*;
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::System::Com::*;
    use windows::Win32::System::Ole::OleInitialize;
    use windows::core::*;

    unsafe {
        let _ = OleInitialize(None);
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        // production runs PMv2 — the probe MUST match or the px contract
        // degenerates to px==DIP and the regression stays invisible
        let _ = windows::Win32::UI::HiDpi::SetThreadDpiAwarenessContext(
            windows::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
        );
    }

    let lib = crate::platform::win32::Msftedit::load().expect("msftedit");

    // real host hwnd — TxGetDC returns its DC, which is where the service
    // reads its dpi; created AFTER the thread is PMv2-aware
    let hwnd = unsafe {
        use windows::Win32::UI::WindowsAndMessaging::*;
        let cls = w!("rustui_probe_px");
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpszClassName: cls,
            hInstance: windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
                .unwrap()
                .into(),
            lpfnWndProc: Some(probe_wndproc),
            ..Default::default()
        };
        let _ = RegisterClassExW(&wc);
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            cls,
            w!("probe"),
            WS_OVERLAPPEDWINDOW,
            0,
            0,
            800,
            500,
            None,
            None,
            None,
            None,
        )
        .expect("hwnd")
    };

    let dc_dpi = unsafe {
        let dc = GetDC(Some(hwnd));
        let d = GetDeviceCaps(Some(dc), LOGPIXELSY);
        ReleaseDC(Some(hwnd), dc);
        d
    };
    let dc_scale = crate::platform::win32::space::scale_from_dpi(dc_dpi as u32).0;
    eprintln!("[probe] dc_dpi={dc_dpi} dc_scale={dc_scale}");

    // in-memory framebuffer: DIB + memory DC + D2D DC render target.
    // target dpi = the real DC dpi so framebuffer px == host px ==
    // DIP * scale — positions land exactly, no window-frame bleed.
    const FBW: i32 = 800;
    const FBH: i32 = 600;
    let (memdc, bits, dcrt) = unsafe {
        let screen = GetDC(None);
        let memdc = CreateCompatibleDC(Some(screen));
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut bmi = BITMAPINFO::default();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = FBW;
        bmi.bmiHeader.biHeight = -FBH; // top-down
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
            dpiX: dc_dpi as f32,
            dpiY: dc_dpi as f32,
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
        (memdc, bits, dcrt)
    };
    let rt: ID2D1RenderTarget = dcrt.cast().expect("cast");
    let data = unsafe { std::slice::from_raw_parts(bits as *const u8, (FBW * FBH * 4) as usize) };

    let cfg = || crate::platform::win32::PeerConfig {
        multiline: true,
        read_only: false,
        face: "Segoe UI".into(),
        size_twips: 280,
        line_twips: 0,
        fg: [1.0, 1.0, 1.0, 1.0],
        fg_authored: crate::style::Color::rgba(200, 30, 30, 255),
        sel_bg: [0.2, 0.4, 0.8, 1.0],
        sel_fg: [1.0, 1.0, 1.0, 1.0],
        bold: false,
    };
    use crate::platform::win32::space::{LogicalRect, ScaleFactor};
    let island_dip = LogicalRect {
        x: 34.0,
        y: 167.0,
        width: 336.0,
        height: 22.0,
    };

    // ---- draw-position matrix across all four canonical scales ---------
    let mut nat_dip_at_dc = None;
    for scale in [1.0f32, 1.25, 1.5, 2.0] {
        let sink = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut peer = crate::platform::win32::WindowlessPeer::create(
            1,
            &lib,
            hwnd,
            ScaleFactor(scale),
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
            "Hello",
            crate::text::TextRevision::mint(),
            binding,
        )
        .expect("initialize");
        // app order: measure (scratch extent, activates), then real bounds
        let (_, h_dip) = peer.natural_size(400.0).expect("natural_size");
        peer.apply_bounds(island_dip, ScaleFactor(scale)).unwrap();
        // typed-after-mount must land identically to mount-time text
        let _ = peer.send(0x0007 /*WM_SETFOCUS*/, 0, 0);
        for c in " xy".encode_utf16() {
            let _ = peer.send(0x0102 /*WM_CHAR*/, c as usize, 0);
        }
        let _ = peer.drain();

        unsafe {
            rt.BeginDraw();
            rt.Clear(Some(&D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            }));
        }
        peer.draw(&rt, island_dip, 0, 1.0).expect("TxDrawD2D");
        unsafe {
            rt.EndDraw(None, None).expect("EndDraw");
        }

        // island in framebuffer px — ink must sit inside it; THE layer
        // performs the same conversion the peer applied
        let island_px = island_dip.physical(ScaleFactor(scale));
        let (ix0, iy0) = (island_px.left, island_px.top);
        let (ix1, iy1) = (island_px.right, island_px.bottom);
        let mut inside = 0usize;
        let mut outside = 0usize;
        for y in 0..FBH {
            for x in 0..FBW {
                let o = (y * FBW + x) as usize * 4;
                let white = data[o] > 180 && data[o + 1] > 180 && data[o + 2] > 180;
                if !white {
                    continue;
                }
                if x >= ix0 && x < ix1 && y >= iy0 && y < iy1 {
                    inside += 1;
                } else {
                    outside += 1;
                }
            }
        }
        eprintln!(
            "[probe] scale={scale} island={ix0},{iy0}-{ix1},{iy1} inside={inside} outside={outside} nat_h={h_dip:.1}"
        );
        assert!(
            inside > 50,
            "scale {scale}: text must render ink inside the bounds, got {inside}"
        );
        assert_eq!(
            outside, 0,
            "scale {scale}: glyph ink outside the island — duplicate/offset copy"
        );
        if (scale - dc_scale).abs() < 0.01 {
            // natural size is only meaningful when the claimed scale equals
            // the host DC's real scale — the service's px space IS the DC's
            nat_dip_at_dc = Some(h_dip);
        }
    }
    let _ = include!("probes_block_00.rs");
    let _ = include!("probes_block_01.rs");
    let _ = include!("probes_block_02.rs");
    let _ = include!("probes_block_03.rs");
    let _ = include!("probes_block_04.rs");
    let _ = include!("probes_block_05.rs");
    // ---- round-trip: DIP -> px -> DIP --------------------------------
    // at the DC's real scale a DIP measurement survives the px seam:
    // natural height of one 14pt line with view insets is ~25 DIP at the
    // Segoe UI default — assert the sane band, not the magic number.
    if let Some(h) = nat_dip_at_dc {
        assert!(
            (20.0..=32.0).contains(&h),
            "single-line natural height ~25 DIP at dc scale, got {h}"
        );
    }

    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::*;
        let _ = DeleteDC(memdc);
        let _ = DestroyWindow(hwnd);
    }
}
