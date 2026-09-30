//! Direct2D/DirectWrite presenter: one HWND render target (software-
//! compatible, opaque), theme chrome for labels/buttons/surfaces, windowless
//! RichEdit content via `TxDrawD2D`, a `Canvas` impl for custom nodes, and
//! the tooltip overlay as a real nonactivating top-level HWND.
//!
//! `TxDrawD2D` draws axis-aligned rectangular-clip text into the same target
//! — live text is never scaled/animated/alpha-blended.

use std::cell::RefCell;
use std::sync::OnceLock;

use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::*;
use windows_numerics::Vector2;

use crate::geom::Point;
use crate::node::NodeData;
use crate::runtime::UpdateCtx;
use crate::ui::{Canvas, Paint, PathOp, TextRun};
use crate::{UiError, UiResult};

use super::Backend;

const FONT: &str = "Segoe UI";

/// shadcn neutral palette (light / dark) per ColorRole — the renderer's
/// Theme/ColorRole conversion layer.
fn role_color(role: crate::theme::ColorRole, dark: bool) -> D2D1_COLOR_F {
    use crate::theme::ColorRole::*;
    let v = match (role, dark) {
        (Background, false) => [0.980, 0.980, 0.980, 1.0],
        (Background, true) => [0.043, 0.043, 0.043, 1.0],
        (Foreground, false) => [0.090, 0.090, 0.090, 1.0],
        (Foreground, true) => [0.929, 0.929, 0.929, 1.0],
        (Muted, false) => [0.961, 0.961, 0.961, 1.0],
        (Muted, true) => [0.149, 0.149, 0.149, 1.0],
        (MutedForeground, false) => [0.451, 0.451, 0.451, 1.0],
        (MutedForeground, true) => [0.639, 0.639, 0.639, 1.0],
        (Accent, false) => [0.094, 0.094, 0.106, 1.0], // primary = inverse fg
        (Accent, true) => [0.980, 0.980, 0.980, 1.0],
        (AccentForeground, false) => [0.980, 0.980, 0.980, 1.0],
        (AccentForeground, true) => [0.094, 0.094, 0.106, 1.0],
        (Border, false) => [0.898, 0.898, 0.898, 1.0],
        (Border, true) => [0.180, 0.180, 0.180, 1.0],
        (Destructive, _) => [0.863, 0.149, 0.149, 1.0],
        (DestructiveForeground, false) => [0.980, 0.980, 0.980, 1.0],
        (DestructiveForeground, true) => [0.980, 0.980, 0.980, 1.0],
        (Focus, false) => [0.35, 0.35, 0.40, 1.0],
        (Focus, true) => [0.65, 0.65, 0.70, 1.0],
        (Shadow, false) => [0.0, 0.0, 0.0, 0.16],
        (Shadow, true) => [0.0, 0.0, 0.0, 0.35],
    };
    D2D1_COLOR_F {
        r: v[0],
        g: v[1],
        b: v[2],
        a: v[3],
    }
}

// ---------------------------------------------------------------------------
// DirectWrite label measure — also feeds layout's natural sizes
// ---------------------------------------------------------------------------

struct Dwrite {
    factory: IDWriteFactory,
    formats: [Option<IDWriteTextFormat>; 3],
    /// (size_bits, weight_ordinal) -> format — Exact sizes land here, the
    /// fixed table covers the two base recipes (14pt normal / semibold)
    cache: std::sync::Mutex<std::collections::HashMap<(u32, u32), IDWriteTextFormat>>,
}

fn dwrite() -> UiResult<&'static Dwrite> {
    static D: OnceLock<UiResult<Dwrite>> = OnceLock::new();
    D.get_or_init(|| unsafe {
        let factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)
            .map_err(|e| UiError::Platform(format!("DWriteCreateFactory: {e}")))?;
        let mk = |factory: &IDWriteFactory, size: f32, bold: bool| -> UiResult<IDWriteTextFormat> {
            factory
                .CreateTextFormat(
                    w!("Segoe UI"),
                    None,
                    if bold {
                        DWRITE_FONT_WEIGHT_SEMI_BOLD
                    } else {
                        DWRITE_FONT_WEIGHT_NORMAL
                    },
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    size,
                    w!("en-us"),
                )
                .map_err(|e| UiError::Platform(format!("CreateTextFormat: {e}")))
        };
        let f0 = mk(&factory, 14.0, false)?;
        let f1 = mk(&factory, 14.0, true)?;
        Ok(Dwrite {
            factory,
            formats: [Some(f0), Some(f1), None],
            cache: std::sync::Mutex::new(std::collections::HashMap::new()),
        })
    })
    .as_ref()
    .map_err(|e| UiError::Platform(e.to_string()))
}

/// Resolved `TextStyle` -> DWrite format. `Body` = the theme base (14pt),
/// `Exact` = explicit logical dp (still OS/DPI scaled by the target).
fn fmt_for(ts: &crate::style::TextStyle, _slot_hint: usize) -> UiResult<IDWriteTextFormat> {
    let d = dwrite()?;
    let size = match ts.size {
        crate::style::TextSize::Body => 14.0,
        crate::style::TextSize::Exact(dp) => dp.0.max(1.0),
    };
    let wkey = match ts.weight {
        crate::style::TextWeight::Normal => 0u32,
        crate::style::TextWeight::Medium => 1,
        crate::style::TextWeight::Bold => 2,
    };
    // base table covers (14pt, normal) and (14pt, semibold)
    if size == 14.0 && wkey <= 1 {
        return Ok(d.formats[wkey as usize].as_ref().unwrap().clone());
    }
    let key = (size.to_bits(), wkey);
    if let Some(f) = d.cache.lock().unwrap().get(&key) {
        return Ok(f.clone());
    }
    let weight = match ts.weight {
        crate::style::TextWeight::Normal => DWRITE_FONT_WEIGHT_NORMAL,
        crate::style::TextWeight::Medium => DWRITE_FONT_WEIGHT_SEMI_BOLD,
        crate::style::TextWeight::Bold => DWRITE_FONT_WEIGHT_BOLD,
    };
    let fmt = unsafe {
        d.factory.CreateTextFormat(
            w!("Segoe UI"),
            None,
            weight,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            size,
            w!("en-us"),
        )
        .map_err(|e| UiError::Platform(format!("CreateTextFormat: {e}")))?
    };
    d.cache.lock().unwrap().insert(key, fmt.clone());
    Ok(fmt)
}

/// Measure text at a max width — natural (w,h) in DIP.
pub(crate) fn measure_text(text: &str, max_w: f32, size: f32) -> UiResult<(f32, f32)> {
    let d = dwrite()?;
    let ts = crate::style::TextStyle {
        size: if (size - 14.0).abs() < f32::EPSILON {
            crate::style::TextSize::Body
        } else {
            crate::style::TextSize::Exact(crate::geom::Dp(size))
        },
        ..Default::default()
    };
    let fmt = fmt_for(&ts, 0)?;
    let wide: Vec<u16> = text.encode_utf16().collect();
    unsafe {
        let lay = d
            .factory
            .CreateTextLayout(&wide, &fmt, max_w.max(1.0), f32::MAX)
            .map_err(|e| UiError::Platform(format!("CreateTextLayout: {e}")))?;
        let mut m = DWRITE_TEXT_METRICS::default();
        lay.GetMetrics(&mut m)
            .map_err(|e| UiError::Platform(format!("GetMetrics: {e}")))?;
        Ok((m.width, m.height))
    }
}

/// The measure closure `layout` consumes.
pub(crate) fn measure_fn() -> impl FnMut(&str, f32, f32) -> (f32, f32) {
    |text, w, size| measure_text(text, w, size).unwrap_or((0.0, 20.0))
}

// ---------------------------------------------------------------------------
// tooltip — real nonactivating top-level HWND, shadcn inverse chrome
// ---------------------------------------------------------------------------

static TIP_CLASS_REGISTERED: OnceLock<()> = OnceLock::new();

struct Tip {
    hwnd: HWND,
    target: Option<ID2D1HwndRenderTarget>,
    factory: Option<ID2D1Factory>,
    text: String,
}

/// All rendering state — recreated only on target loss; text peers and the
/// retained model are unaffected.
pub(crate) struct Renderer {
    factory: ID2D1Factory,
    target: Option<ID2D1HwndRenderTarget>,
    hwnd: HWND,
    dpi: f32,
    tip: RefCell<Option<Tip>>,
    /// tooltip fade clock — 150ms chrome fade (chrome only, never text)
    tip_fade_start: RefCell<Option<std::time::Instant>>,
}

impl Renderer {
    pub(crate) fn new() -> UiResult<Renderer> {
        let factory: ID2D1Factory = unsafe {
            D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)
                .map_err(|e| UiError::Platform(format!("D2D1CreateFactory: {e}")))?
        };
        Ok(Renderer {
            factory,
            target: None,
            hwnd: HWND::default(),
            dpi: 96.0,
            tip: RefCell::new(None),
            tip_fade_start: RefCell::new(None),
        })
    }

    pub(crate) fn set_dpi(&mut self, dpi: f32) {
        self.dpi = dpi;
        if let Some(t) = &self.target {
            unsafe { t.SetDpi(dpi, dpi) };
        }
    }

    pub(crate) fn resize(&mut self) {
        if self.hwnd.is_invalid() {
            return;
        }
        let mut rc = RECT::default();
        unsafe {
            let _ = GetClientRect(self.hwnd, &mut rc);
        }
        if let Some(t) = &self.target {
            unsafe {
                let _ = t.Resize(&windows::Win32::Graphics::Direct2D::Common::D2D_SIZE_U {
                    width: (rc.right - rc.left).max(1) as u32,
                    height: (rc.bottom - rc.top).max(1) as u32,
                });
            }
        } else {
            let _ = self.ensure_target();
        }
    }

    fn ensure_target(&mut self) -> UiResult<&ID2D1HwndRenderTarget> {
        if self.target.is_some() {
            return Ok(self.target.as_ref().unwrap());
        }
        unsafe {
            let mut rc = RECT::default();
            let _ = GetClientRect(self.hwnd, &mut rc);
            let props = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_SOFTWARE,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_IGNORE,
                },
                dpiX: self.dpi,
                dpiY: self.dpi,
                ..Default::default()
            };
            let hp = D2D1_HWND_RENDER_TARGET_PROPERTIES {
                hwnd: self.hwnd,
                pixelSize: windows::Win32::Graphics::Direct2D::Common::D2D_SIZE_U {
                    width: (rc.right - rc.left).max(1) as u32,
                    height: (rc.bottom - rc.top).max(1) as u32,
                },
                presentOptions: D2D1_PRESENT_OPTIONS_NONE,
            };
            let t = self
                .factory
                .CreateHwndRenderTarget(&props, &hp)
                .map_err(|e| UiError::Platform(format!("CreateHwndRenderTarget: {e}")))?;
            self.target = Some(t);
            Ok(self.target.as_ref().unwrap())
        }
    }

    /// Full frame paint — walks the layout's depth order.
    pub(crate) fn draw<S, M, U, V>(&mut self, be: &Backend<S, M, U, V>) -> UiResult
    where
        M: 'static,
        U: Fn(&mut S, M, &mut UpdateCtx<'_, M>),
        V: Fn(&S, &mut crate::Ui<'_, '_, M>),
    {
        self.hwnd = be.hwnd;
        self.dpi = be.peer_ctx.scale.get() * 96.0;
        // target-loss -> recreate only the renderer (peers/model preserved);
        // retry only on a genuine device/target loss
        for attempt in 0..2 {
            match self.draw_frame(be) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    self.target = None;
                    let lost = matches!(&e, UiError::Platform(m) if m.contains("RECREATE_TARGET"));
                    if attempt == 1 || !lost {
                        return Err(e);
                    }
                }
            }
        }
        Ok(())
    }

    fn draw_frame<S, M, U, V>(&mut self, be: &Backend<S, M, U, V>) -> UiResult
    where
        M: 'static,
        U: Fn(&mut S, M, &mut UpdateCtx<'_, M>),
        V: Fn(&S, &mut crate::Ui<'_, '_, M>),
    {
        self.hwnd = be.hwnd;
        let dark = be.rt.appearance().dark;
        let dpi = self.dpi;
        let target = self.ensure_target()?;
        unsafe {
            target.SetDpi(dpi, dpi);
        }
        let bg = role_color(crate::theme::ColorRole::Background, dark);
        unsafe {
            target.BeginDraw();
            target.Clear(Some(&bg));
        }
        let order = be.order.clone();
        for id in order {
            let Some(r) = be.rects.get(&id).copied() else {
                continue;
            };
            let Some(n) = be.rt.arena.get(id) else {
                continue;
            };
            let clip = D2D_RECT_F {
                left: r.x,
                top: r.y,
                right: r.x + r.w,
                bottom: r.y + r.h,
            };
            match &n.data {
                NodeData::Label {
                    text,
                    color_role,
                    wrap,
                    patch,
                } => {
                    // resolved text style: recipe (role fg, body, normal)
                    // + the surgical patch (color_role already folded in)
                    let mut ts = crate::style::label_recipe();
                    ts.patch(patch);
                    // color_role is the recipe fg unless a patch set one
                    if patch.foreground.is_none() {
                        ts.foreground = crate::style::Color::Role(*color_role);
                    }
                    let wide: Vec<u16> = text.as_ref().encode_utf16().collect();
                    let wide: Vec<u16> = if wide.is_empty() { vec![0x20] } else { wide };
                    // size/weight map onto the DWrite format table
                    let fi = match ts.weight {
                        crate::style::TextWeight::Normal => 0,
                        crate::style::TextWeight::Medium => 1,
                        crate::style::TextWeight::Bold => 2,
                    };
                    if let Ok(layout) = unsafe {
                        dwrite()?.factory.CreateTextLayout(
                            &wide,
                            &fmt_for(&ts, fi)?,
                            if *wrap { r.w.max(1.0) } else { f32::MAX },
                            r.h.max(1.0),
                        )
                    } {
                        let c = resolve_color_f(ts.foreground, dark);
                        let brush = unsafe { target.CreateSolidColorBrush(&c, None)? };
                        unsafe {
                            target.DrawTextLayout(
                                Vector2 { X: r.x, Y: r.y },
                                &layout,
                                &brush,
                                D2D1_DRAW_TEXT_OPTIONS_NONE,
                            );
                        }
                    }
                }
                NodeData::Button {
                    text,
                    variant,
                    style,
                    disabled,
                    size,
                    motion: _,
                    tooltip: _,
                } => {
                    // deterministic chain: recipe -> consumer patch -> state
                    let state = crate::style::StyleState::classify(
                        *disabled,
                        be.pressed == Some(id),
                        be.hot == Some(id),
                    );
                    let vs = crate::style::resolve_button(
                        *variant,
                        *size,
                        style,
                        state,
                        be.focus == Some(id),
                        dark,
                    );
                    let bs = &vs.box_style;
                    let pad_l = bs.padding.left.0;
                    let pad_r = bs.padding.right.0;
                    let br = D2D_RECT_F {
                        left: r.x,
                        top: r.y,
                        right: r.x + r.w,
                        bottom: r.y + r.h,
                    };
                    unsafe {
                        paint_box(target, &br, bs, dark)?;
                        let wide: Vec<u16> = text.as_ref().encode_utf16().collect();
                        let fmt = fmt_for(&vs.text_style, 0)?;
                        let lay = dwrite()?.factory.CreateTextLayout(
                            &wide,
                            &fmt,
                            (br.right - br.left - pad_l - pad_r).max(1.0),
                            (br.bottom - br.top).max(1.0),
                        )?;
                        let fg = resolve_color_f(vs.text_style.foreground, dark);
                        let fg_brush = target.CreateSolidColorBrush(&fg, None)?;
                        lay.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
                        lay.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
                        target.DrawTextLayout(
                            Vector2 {
                                X: br.left + pad_l,
                                Y: br.top,
                            },
                            &lay,
                            &fg_brush,
                            D2D1_DRAW_TEXT_OPTIONS_NONE,
                        );
                    }
                }
                NodeData::Editor { .. } => {
                    // border + content — the peer draws its own text/selection
                    unsafe {
                        let bc = target.CreateSolidColorBrush(
                            &role_color(crate::theme::ColorRole::Border, dark),
                            None,
                        )?;
                        let geo: ID2D1RoundedRectangleGeometry = target
                            .GetFactory()?
                            .CreateRoundedRectangleGeometry(&D2D1_ROUNDED_RECT {
                                rect: clip,
                                radiusX: 6.0,
                                radiusY: 6.0,
                            })?;
                        if be.focus == Some(id) {
                            let ring = target.CreateSolidColorBrush(
                                &role_color(crate::theme::ColorRole::Focus, dark),
                                None,
                            )?;
                            target.DrawGeometry(&geo, &ring, 1.5, None);
                        } else {
                            target.DrawGeometry(&geo, &bc, 1.0, None);
                        }
                    }
                    // NOTE: no axis-aligned clip around the peer draw —
                    // msftedit's D2D path does not honor a pushed clip
                    // (text silently fails to land). The peer's own view
                    // rect confines its output.
                    if clip.right > clip.left
                        && clip.bottom > clip.top
                        && let Some(peer) = be.peer_for(id)
                    {
                        peer.borrow()
                            .draw(
                                target,
                                (r.x + 6.0, r.y + 5.0, r.x + r.w - 6.0, r.y + r.h - 5.0),
                            )?;
                    }
                }
                NodeData::Custom { render, .. } => {
                    let mut canvas = D2dCanvas {
                        target,
                        dark,
                        geo_stack: Vec::new(),
                        path_geos: Vec::new(),
                    };
                    render.paint(
                        &mut canvas,
                        crate::geom::Rect {
                            x: r.x,
                            y: r.y,
                            width: r.w,
                            height: r.h,
                        },
                    );
                }
                NodeData::Container { kind, props, .. } => {
                    // painted box containers resolve to a BoxStyle and go
                    // through the shared painter — surface = recipe + patch,
                    // box_ = authored full style
                    if let Some(bs) = props.resolved_box(*kind) {
                        unsafe {
                            paint_box(target, &clip, &bs, dark)?;
                        }
                    }
                }
                NodeData::Action {
                    style,
                    disabled,
                    ..
                } => {
                    let bs = style.resolve(
                        *disabled,
                        be.pressed == Some(id),
                        be.hot == Some(id),
                        be.focus == Some(id),
                    );
                    unsafe {
                        paint_box(target, &clip, &bs, dark)?;
                        // recipe focus ring — real Focus role outside the box
                        if be.focus == Some(id) && !*disabled {
                            let ring = target.CreateSolidColorBrush(
                                &role_color(crate::theme::ColorRole::Focus, dark),
                                None,
                            )?;
                            let outer = D2D1_ROUNDED_RECT {
                                rect: D2D_RECT_F {
                                    left: clip.left - 2.0,
                                    top: clip.top - 2.0,
                                    right: clip.right + 2.0,
                                    bottom: clip.bottom + 2.0,
                                },
                                radiusX: 7.0,
                                radiusY: 7.0,
                            };
                            let og: ID2D1RoundedRectangleGeometry = target
                                .GetFactory()?
                                .CreateRoundedRectangleGeometry(&outer)?;
                            target.DrawGeometry(&og, &ring, 1.5, None);
                        }
                    }
                }
                _ => {}
            }
        }
        match unsafe { target.EndDraw(None, None) } {
            Err(e) if e.code() == D2DERR_RECREATE_TARGET => {
                Err(UiError::Platform("D2DERR_RECREATE_TARGET".into()))
            }
            Err(e) => Err(UiError::Platform(format!("EndDraw: {e}"))),
            Ok(()) => Ok(()),
        }
    }

    // ---- tooltip overlay -------------------------------------------------

    /// Semantic tooltip fired — show the owned nonactivating overlay near
    /// the anchor rect. Chrome fade 150ms unless reduced-motion.
    pub(crate) fn show_tooltip(&mut self, text: &str, anchor: HWND, scale: f32) {
        let hwnd = match self.tip.borrow().as_ref() {
            Some(t) => t.hwnd,
            None => self.create_tip(text),
        };
        unsafe {
            let (w, h) = measure_text(text, 400.0, 13.0).unwrap_or((80.0, 20.0));
            let (pw, ph) = (w + 16.0, h + 10.0);
            // position: centered under the anchor's first editor rect —
            // callers pass the node rect through anchor client space; we
            // place it just below the rect in screen space
            let mut rc = RECT::default();
            let _ = GetClientRect(anchor, &mut rc);
            let mut pt = POINT { x: 0, y: rc.bottom };
            let _ = ClientToScreen(anchor, &mut pt);
            let x = pt.x;
            let y = pt.y + 4;
            let mut tip_rc = RECT {
                left: 0,
                top: 0,
                right: (pw * scale) as i32,
                bottom: (ph * scale) as i32,
            };
            let _ = AdjustWindowRect(&mut tip_rc, WS_POPUP, false);
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                x,
                y,
                (pw * scale) as i32,
                (ph * scale) as i32,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
            let _ = InvalidateRect(Some(hwnd), None, false);
        }
        *self.tip_fade_start.borrow_mut() = Some(std::time::Instant::now());
    }

    fn create_tip(&self, text: &str) -> HWND {
        TIP_CLASS_REGISTERED.get_or_init(|| unsafe {
            let inst = GetModuleHandleW(None).unwrap();
            let wc = WNDCLASSW {
                lpfnWndProc: Some(tip_wndproc),
                hInstance: HINSTANCE(inst.0),
                lpszClassName: w!("RustUiTooltip"),
                ..Default::default()
            };
            let _ = RegisterClassW(&wc);
        });
        unsafe {
            let inst = GetModuleHandleW(None).unwrap();
            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_LAYERED,
                w!("RustUiTooltip"),
                PCWSTR::null(),
                WS_POPUP,
                0,
                0,
                0,
                0,
                None,
                None,
                Some(inst.into()),
                None,
            )
            .unwrap_or_default();
            *self.tip.borrow_mut() = Some(Tip {
                hwnd,
                target: None,
                factory: Some(self.factory.clone()),
                text: text.to_string(),
            });
            hwnd
        }
    }

    pub(crate) fn hide_tooltip(&mut self) {
        if let Some(t) = self.tip.borrow().as_ref() {
            unsafe {
                let _ = ShowWindow(t.hwnd, SW_HIDE);
            }
        }
    }

    /// Tooltip paint (its own target — inverse shadcn chrome).
    pub(crate) fn tip_paint(hwnd: HWND) {
        unsafe {
            let _ = ValidateRect(Some(hwnd), None);
        }
    }
}

unsafe extern "system" fn tip_wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            // minimal chrome: inverse neutral bg + fg text via GDI text — the
            // tip is chrome-only (never a native text island)
            unsafe {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rc = RECT::default();
                let _ = GetClientRect(hwnd, &mut rc);
                let bg = CreateSolidBrush(COLORREF(0x00232323)); // near-black
                FillRect(hdc, &rc, bg);
                let _ = DeleteObject(bg.into());
                let _ = SetTextColor(hdc, COLORREF(0x00fafafa));
                let _ = SetBkMode(hdc, TRANSPARENT);
                let _ = EndPaint(hwnd, &ps);
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wp, lp) },
    }
}

// ---------------------------------------------------------------------------
// Canvas — custom node paint over the D2D target
// ---------------------------------------------------------------------------

struct D2dCanvas<'a> {
    target: &'a ID2D1HwndRenderTarget,
    dark: bool,
    /// built geometries kept alive for the paint call's borrow scope
    geo_stack: Vec<ID2D1PathGeometry>,
    path_geos: Vec<ID2D1Geometry>,
}

impl Canvas for D2dCanvas<'_> {
    fn path(&mut self, path: &crate::ui::Path2d, paint: Paint) {
        unsafe {
            let Ok(factory) = self.target.GetFactory() else {
                return;
            };
            let Ok(geo) = factory.CreatePathGeometry() else {
                return;
            };
            let Ok(sink) = geo.Open() else {
                return;
            };
            sink.SetFillMode(D2D1_FILL_MODE_WINDING);
            let mut fig_open = false;
            for op in &path.ops {
                match op {
                    PathOp::MoveTo(p) => {
                        if fig_open {
                            sink.EndFigure(D2D1_FIGURE_END_OPEN);
                        }
                        sink.BeginFigure(Vector2 { X: p.x, Y: p.y }, D2D1_FIGURE_BEGIN_FILLED);
                        fig_open = true;
                    }
                    PathOp::LineTo(p) => {
                        if fig_open {
                            sink.AddLine(Vector2 { X: p.x, Y: p.y });
                        }
                    }
                    PathOp::QuadTo(c, p) => {
                        if fig_open {
                            sink.AddQuadraticBezier(&D2D1_QUADRATIC_BEZIER_SEGMENT {
                                point1: Vector2 { X: c.x, Y: c.y },
                                point2: Vector2 { X: p.x, Y: p.y },
                            });
                        }
                    }
                    PathOp::Close => {
                        if fig_open {
                            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
                            fig_open = false;
                        }
                    }
                }
            }
            if fig_open {
                sink.EndFigure(D2D1_FIGURE_END_OPEN);
            }
            if sink.Close().is_err() {
                return;
            }
            let color = match paint {
                Paint::FillRole(r) => role_color(r, self.dark),
                Paint::Rgba(r, g, b, a) => D2D1_COLOR_F {
                    r: r as f32 / 255.0,
                    g: g as f32 / 255.0,
                    b: b as f32 / 255.0,
                    a: a as f32 / 255.0,
                },
            };
            if let Ok(brush) = self.target.CreateSolidColorBrush(&color, None) {
                self.target.FillGeometry(&geo, &brush, None);
            }
        }
    }
    fn text(&mut self, run: &TextRun, origin: Point) {
        let Ok(d) = dwrite() else { return };
        let Some(fmt) = d.formats[0].as_ref() else {
            return;
        };
        let wide: Vec<u16> = run.text.as_ref().encode_utf16().collect();
        unsafe {
            let Ok(lay) = d.factory.CreateTextLayout(&wide, fmt, f32::MAX, f32::MAX) else {
                return;
            };
            let c = role_color(run.color_role, self.dark);
            if let Ok(brush) = self.target.CreateSolidColorBrush(&c, None) {
                self.target.DrawTextLayout(
                    Vector2 {
                        X: origin.x,
                        Y: origin.y,
                    },
                    &lay,
                    &brush,
                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                );
            }
        }
    }
}
// ---------------------------------------------------------------------------
// paint_box — the shared box painter (background + per-corner radii +
// per-side borders + one outer shadow). Every painted component resolves to
// a `BoxStyle` and lands here — no private parallel renderers.
// ---------------------------------------------------------------------------

use crate::style::{BoxStyle, CornerRadii, Shadow};
use windows_numerics::Matrix3x2;

/// Resolve an authored `Color` into the target's float4.
fn brush_color(c: crate::style::Color, dark: bool) -> D2D1_COLOR_F {
    let [r, g, b, a] = crate::style::resolve_color(c, dark);
    D2D1_COLOR_F { r, g, b, a }
}

/// Rounded-rect geometry with per-corner radii. Uniform radii use the D2D
/// rounded-rect geometry; mixed corners build an edge+arc path.
unsafe fn box_geometry(
    target: &ID2D1RenderTarget,
    r: &D2D_RECT_F,
    radii: &CornerRadii,
) -> Result<ID2D1Geometry> { unsafe {
    let f = target.GetFactory()?;
    if let Some(crate::geom::Dp(u)) = radii.uniform() {
        let g = f.CreateRoundedRectangleGeometry(&D2D1_ROUNDED_RECT {
            rect: *r,
            radiusX: u,
            radiusY: u,
        })?;
        return Ok(g.cast()?);
    }
    let g = f.CreatePathGeometry()?;
    let sink = g.Open()?;
    sink.SetFillMode(D2D1_FILL_MODE_WINDING);
    let hw = (r.right - r.left) / 2.0;
    let hh = (r.bottom - r.top) / 2.0;
    let (tl, tr, br, bl) = (
        radii.top_left.0.min(hw).min(hh),
        radii.top_right.0.min(hw).min(hh),
        radii.bottom_right.0.min(hw).min(hh),
        radii.bottom_left.0.min(hw).min(hh),
    );
    sink.BeginFigure(
        Vector2 {
            X: r.left + tl,
            Y: r.top,
        },
        D2D1_FIGURE_BEGIN_FILLED,
    );
    sink.AddLine(Vector2 {
        X: r.right - tr,
        Y: r.top,
    });
    if tr > 0.0 {
        sink.AddArc(&D2D1_ARC_SEGMENT {
            point: Vector2 {
                X: r.right,
                Y: r.top + tr,
            },
            size: D2D_SIZE_F {
                width: tr,
                height: tr,
            },
            rotationAngle: 0.0,
            sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
            arcSize: D2D1_ARC_SIZE_SMALL,
        });
    }
    sink.AddLine(Vector2 {
        X: r.right,
        Y: r.bottom - br,
    });
    if br > 0.0 {
        sink.AddArc(&D2D1_ARC_SEGMENT {
            point: Vector2 {
                X: r.right - br,
                Y: r.bottom,
            },
            size: D2D_SIZE_F {
                width: br,
                height: br,
            },
            rotationAngle: 0.0,
            sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
            arcSize: D2D1_ARC_SIZE_SMALL,
        });
    }
    sink.AddLine(Vector2 {
        X: r.left + bl,
        Y: r.bottom,
    });
    if bl > 0.0 {
        sink.AddArc(&D2D1_ARC_SEGMENT {
            point: Vector2 {
                X: r.left,
                Y: r.bottom - bl,
            },
            size: D2D_SIZE_F {
                width: bl,
                height: bl,
            },
            rotationAngle: 0.0,
            sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
            arcSize: D2D1_ARC_SIZE_SMALL,
        });
    }
    sink.AddLine(Vector2 {
        X: r.left,
        Y: r.top + tl,
    });
    if tl > 0.0 {
        sink.AddArc(&D2D1_ARC_SEGMENT {
            point: Vector2 {
                X: r.left + tl,
                Y: r.top,
            },
            size: D2D_SIZE_F {
                width: tl,
                height: tl,
            },
            rotationAngle: 0.0,
            sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
            arcSize: D2D1_ARC_SIZE_SMALL,
        });
    }
    sink.EndFigure(D2D1_FIGURE_END_CLOSED);
    sink.Close()?;
    Ok(g.cast()?)
}}

/// Outer shadow: rasterize the rounded-rect silhouette into a CPU alpha
/// mask, box-blur it (3 passes ~= Gaussian at `blur_sigma`), premultiply
/// with the shadow color and draw it as a bitmap offset by (dx, dy). This
/// is a real blur — no D2D1.1 device-context effect dependency.
fn draw_shadow(
    target: &ID2D1RenderTarget,
    r: &D2D_RECT_F,
    radii: &CornerRadii,
    shadow: &Shadow,
    dark: bool,
) -> Result<()> {
    let sigma = shadow.blur_sigma.0.max(0.0);
    let [sr, sg, sb, sa] = crate::style::resolve_color(shadow.color, dark);
    if sa <= 0.0 {
        return Ok(());
    }
    // rasterization scale: 1 DIP = 1 mask pixel (D2D upscales for the DIP
    // target — the blur is in logical space per the contract)
    let pad = (sigma * 3.0).ceil().max(1.0);
    let w = (r.right - r.left).ceil() as usize + 2 * pad as usize;
    let h = (r.bottom - r.top).ceil() as usize + 2 * pad as usize;
    if w == 0 || h == 0 {
        return Ok(());
    }
    let bw = (r.right - r.left).max(0.0) / 2.0;
    let bh = (r.bottom - r.top).max(0.0) / 2.0;
    let (rtl, rtr, rbr, rbl) = (
        radii.top_left.0.min(bw).min(bh),
        radii.top_right.0.min(bw).min(bh),
        radii.bottom_right.0.min(bw).min(bh),
        radii.bottom_left.0.min(bw).min(bh),
    );
    // signed distance to the per-corner rounded rect (mask space)
    let sd = |px: f32, py: f32| -> f32 {
        // pixel coords are mask-local: box spans [pad, pad+bw*2]×[pad, pad+bh*2]
        let x = px - pad;
        let y = py - pad;
        let rad = if y < bh {
            if x < bw { rtl } else { rtr }
        } else if x < bw {
            rbl
        } else {
            rbr
        };
        // rounded-box sdf (standard)
        let qx = (x - bw).abs() - (bw - rad);
        let qy = (y - bh).abs() - (bh - rad);
        let ax = qx.max(0.0);
        let ay = qy.max(0.0);
        (ax * ax + ay * ay).sqrt() + qx.max(qy).min(0.0) - rad
    };
    let mut mask = vec![0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let d = sd(x as f32 + 0.5, y as f32 + 0.5);
            // coverage: inside d<0; 0.5-px AA edge
            mask[y * w + x] = (0.5 - d).clamp(0.0, 1.0);
        }
    }
    // 3 box blurs approximate a Gaussian of `sigma`; kernel radius r ~ sigma
    if sigma > 0.0 {
        let kr = (sigma / 1.5).max(1.0) as usize; // 3 passes of r≈σ/1.5 ≈ σ
        for _ in 0..3 {
            // horizontal
            let mut tmp = vec![0f32; w * h];
            for y in 0..h {
                let mut acc = 0f32;
                for x in 0..w + 2 * kr {
                    if x < w + kr {
                        acc += if x < w { mask[y * w + x] } else { 0.0 };
                    }
                    if x >= 2 * kr + 1 {
                        acc -= mask[y * w + (x - 2 * kr - 1)];
                    }
                    if x >= kr {
                        let ox = x - kr;
                        if ox < w {
                            tmp[y * w + ox] = acc / (2 * kr + 1) as f32;
                        }
                    }
                }
            }
            // vertical
            let mut out = vec![0f32; w * h];
            for x in 0..w {
                let mut acc = 0f32;
                for y in 0..h + 2 * kr {
                    if y < h + kr {
                        acc += if y < h { tmp[y * w + x] } else { 0.0 };
                    }
                    if y >= 2 * kr + 1 {
                        acc -= tmp[(y - 2 * kr - 1) * w + x];
                    }
                    if y >= kr {
                        let oy = y - kr;
                        if oy < h {
                            out[oy * w + x] = acc / (2 * kr + 1) as f32;
                        }
                    }
                }
            }
            mask = out;
        }
    }
    // premultiplied BGRA
    let mut px = vec![0u32; w * h];
    for (i, a) in mask.iter().enumerate() {
        let a = (*a).clamp(0.0, 1.0) * sa;
        let r8 = (sr * a * 255.0) as u32;
        let g8 = (sg * a * 255.0) as u32;
        let b8 = (sb * a * 255.0) as u32;
        let a8 = (a * 255.0) as u32;
        px[i] = (a8 << 24) | (r8 << 16) | (g8 << 8) | b8;
    }
    unsafe {
        let bmp = target.CreateBitmap(
            D2D_SIZE_U {
                width: w as u32,
                height: h as u32,
            },
            Some(px.as_ptr() as *const _),
            (w * 4) as u32,
            &D2D1_BITMAP_PROPERTIES {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                ..Default::default()
            },
        )?;
        let dest = D2D_RECT_F {
            left: r.left + shadow.offset_x.0 - pad,
            top: r.top + shadow.offset_y.0 - pad,
            right: r.left + shadow.offset_x.0 - pad + w as f32,
            bottom: r.top + shadow.offset_y.0 - pad + h as f32,
        };
        target.DrawBitmap(&bmp, Some(&dest), 1.0, D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, None);
    }
    Ok(())
}

/// Paint one resolved `BoxStyle` at `r` — shadow under, fill, then per-side
/// borders (later sides own shared corners, T<R<B<L).
fn paint_box(
    target: &ID2D1RenderTarget,
    r: &D2D_RECT_F,
    style: &BoxStyle,
    dark: bool,
) -> Result<()> {
    unsafe {
        if let Some(sh) = &style.shadow {
            draw_shadow(target, r, &style.radii, sh, dark)?;
        }
        let bg = resolve_color_f(style.background, dark);
        let geo = box_geometry(target, r, &style.radii)?;
        if bg.a > 0.0 {
            let b = target.CreateSolidColorBrush(&bg, None)?;
            target.FillGeometry(&geo, &b, None);
        }
        if style.border.any() {
            if let Some(side) = style.border.uniform() {
                if side.width.0 > 0.0 {
                    let c = brush_color(side.color, dark);
                    if c.a > 0.0 {
                        let b = target.CreateSolidColorBrush(&c, None)?;
                        // midline stroke: inset by half the width so the
                        // stroke lands exactly inside the outer edge
                        let mid = box_geometry(
                            target,
                            &D2D_RECT_F {
                                left: r.left + side.width.0 / 2.0,
                                top: r.top + side.width.0 / 2.0,
                                right: r.right - side.width.0 / 2.0,
                                bottom: r.bottom - side.width.0 / 2.0,
                            },
                            &shrink_radii(&style.radii, side.width.0 / 2.0),
                        )?;
                        target.DrawGeometry(&mid, &b, side.width.0, None);
                    }
                }
            } else {
                // non-uniform: each side painted as a strip clipped to the
                // rounded silhouette (PushLayer geometric mask)
                let mask = box_geometry(target, r, &style.radii)?;
                let params = D2D1_LAYER_PARAMETERS {
                    contentBounds: D2D_RECT_F {
                        left: f32::MIN,
                        top: f32::MIN,
                        right: f32::MAX,
                        bottom: f32::MAX,
                    },
                    geometricMask: std::mem::ManuallyDrop::new(Some(mask)),
                    maskAntialiasMode: D2D1_ANTIALIAS_MODE_ALIASED,
                    maskTransform: Matrix3x2::identity(),
                    opacity: 1.0,
                    opacityBrush: std::mem::ManuallyDrop::new(None),
                    layerOptions: D2D1_LAYER_OPTIONS_NONE,
                };
                target.PushLayer(&params, None);
                let strips = [
                    (
                        style.border.top,
                        D2D_RECT_F { left: r.left, top: r.top, right: r.right, bottom: r.top + style.border.top.width.0 },
                    ),
                    (
                        style.border.right,
                        D2D_RECT_F { left: r.right - style.border.right.width.0, top: r.top, right: r.right, bottom: r.bottom },
                    ),
                    (
                        style.border.bottom,
                        D2D_RECT_F { left: r.left, top: r.bottom - style.border.bottom.width.0, right: r.right, bottom: r.bottom },
                    ),
                    (
                        style.border.left,
                        D2D_RECT_F { left: r.left, top: r.top, right: r.left + style.border.left.width.0, bottom: r.bottom },
                    ),
                ];
                for (side, sr) in strips {
                    if side.width.0 <= 0.0 {
                        continue;
                    }
                    let c = brush_color(side.color, dark);
                    if c.a <= 0.0 {
                        continue;
                    }
                    let b = target.CreateSolidColorBrush(&c, None)?;
                    target.FillRectangle(&sr, &b);
                }
                target.PopLayer();
            }
        }
    }
    Ok(())
}

fn shrink_radii(r: &CornerRadii, d: f32) -> CornerRadii {
    CornerRadii {
        top_left: crate::geom::Dp((r.top_left.0 - d).max(0.0)),
        top_right: crate::geom::Dp((r.top_right.0 - d).max(0.0)),
        bottom_right: crate::geom::Dp((r.bottom_right.0 - d).max(0.0)),
        bottom_left: crate::geom::Dp((r.bottom_left.0 - d).max(0.0)),
    }
}

fn resolve_color_f(c: crate::style::Color, dark: bool) -> D2D1_COLOR_F {
    let [r, g, b, a] = crate::style::resolve_color(c, dark);
    D2D1_COLOR_F { r, g, b, a }
}
