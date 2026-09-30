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
use crate::node::{
    KIND_SURFACE, NodeData,
};
use crate::runtime::UpdateCtx;
use crate::theme::{ButtonVariant, ControlSize};
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
        (Focus, false) => [0.35, 0.35, 0.40, 1.0],
        (Focus, true) => [0.65, 0.65, 0.70, 1.0],
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
        })
    })
    .as_ref()
    .map_err(|e| UiError::Platform(e.to_string()))
}

/// Measure text at a max width — natural (w,h) in DIP.
pub(crate) fn measure_text(text: &str, max_w: f32, _size: f32) -> UiResult<(f32, f32)> {
    let d = dwrite()?;
    let fmt = d.formats[0].as_ref().unwrap();
    let wide: Vec<u16> = text.encode_utf16().collect();
    unsafe {
        let lay = d
            .factory
            .CreateTextLayout(&wide, fmt, max_w.max(1.0), f32::MAX)
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
                    ..
                } => {
                    let fmt = dwrite()?.formats[0].as_ref().unwrap();
                    let wide: Vec<u16> = text.as_ref().encode_utf16().collect();
                    if let Ok(layout) = unsafe {
                        dwrite()?.factory.CreateTextLayout(
                            &wide,
                            fmt,
                            if *wrap { r.w.max(1.0) } else { f32::MAX },
                            r.h.max(1.0),
                        )
                    } {
                        let c = role_color(*color_role, dark);
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
                    style: _,
                    disabled,
                    size,
                    motion: _,
                    tooltip: _,
                } => {
                    let h = match size {
                        ControlSize::Sm => 28.0f32,
                        ControlSize::Md => 36.0,
                        ControlSize::Lg => 44.0,
                    };
                    let by = r.y + (r.h - h).max(0.0f32) / 2.0;
                    let br = D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: r.x,
                            top: by,
                            right: r.x + r.w,
                            bottom: (by + h).min(r.y + r.h),
                        },
                        radiusX: 6.0,
                        radiusY: 6.0,
                    };
                    let (fill, fg, border) = match variant {
                        ButtonVariant::Primary => (
                            role_color(crate::theme::ColorRole::Accent, dark),
                            role_color(crate::theme::ColorRole::AccentForeground, dark),
                            None,
                        ),
                        ButtonVariant::Secondary => (
                            role_color(crate::theme::ColorRole::Muted, dark),
                            role_color(crate::theme::ColorRole::Foreground, dark),
                            Some(role_color(crate::theme::ColorRole::Border, dark)),
                        ),
                        ButtonVariant::Ghost => (
                            role_color(crate::theme::ColorRole::Background, dark),
                            role_color(crate::theme::ColorRole::Foreground, dark),
                            None,
                        ),
                        ButtonVariant::Destructive => (
                            role_color(crate::theme::ColorRole::Destructive, dark),
                            role_color(crate::theme::ColorRole::AccentForeground, false),
                            None,
                        ),
                    };
                    let disabled_dim = if *disabled { 0.45 } else { 1.0 };
                    unsafe {
                        let fill_brush = target.CreateSolidColorBrush(
                            &D2D1_COLOR_F {
                                a: fill.a * disabled_dim,
                                ..fill
                            },
                            None,
                        )?;
                        let geo: ID2D1RoundedRectangleGeometry =
                            target.GetFactory()?.CreateRoundedRectangleGeometry(&br)?;
                        target.FillGeometry(&geo, &fill_brush, None);
                        if let Some(bc) = border {
                            let bb = target.CreateSolidColorBrush(&bc, None)?;
                            target.DrawGeometry(&geo, &bb, 1.0, None);
                        }
                        // focus ring — real Focus role, drawn when focused
                        if be.focus == Some(id) {
                            let ring = target.CreateSolidColorBrush(
                                &role_color(crate::theme::ColorRole::Focus, dark),
                                None,
                            )?;
                            let outer = D2D1_ROUNDED_RECT {
                                rect: D2D_RECT_F {
                                    left: br.rect.left - 2.0,
                                    top: br.rect.top - 2.0,
                                    right: br.rect.right + 2.0,
                                    bottom: br.rect.bottom + 2.0,
                                },
                                radiusX: 7.0,
                                radiusY: 7.0,
                            };
                            let og: ID2D1RoundedRectangleGeometry = target
                                .GetFactory()?
                                .CreateRoundedRectangleGeometry(&outer)?;
                            target.DrawGeometry(&og, &ring, 1.5, None);
                        }
                        let wide: Vec<u16> = text.as_ref().encode_utf16().collect();
                        let fmt = dwrite()?.formats[0].as_ref().unwrap();
                        let lay = dwrite()?.factory.CreateTextLayout(
                            &wide,
                            fmt,
                            br.rect.right - br.rect.left - 16.0,
                            h.max(1.0),
                        )?;
                        let mut tm = DWRITE_TEXT_METRICS::default();
                        lay.GetMetrics(&mut tm)?;
                        let fg_brush = target.CreateSolidColorBrush(
                            &D2D1_COLOR_F {
                                a: fg.a * disabled_dim,
                                ..fg
                            },
                            None,
                        )?;
                        lay.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
                        lay.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
                        target.DrawTextLayout(
                            Vector2 {
                                X: br.rect.left + 8.0,
                                Y: br.rect.top,
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
                NodeData::Container { kind, .. } => {
                    if *kind == KIND_SURFACE {
                        unsafe {
                            let geo: ID2D1RoundedRectangleGeometry = target
                                .GetFactory()?
                                .CreateRoundedRectangleGeometry(&D2D1_ROUNDED_RECT {
                                    rect: clip,
                                    radiusX: 8.0,
                                    radiusY: 8.0,
                                })?;
                            let m = target.CreateSolidColorBrush(
                                &role_color(crate::theme::ColorRole::Muted, dark),
                                None,
                            )?;
                            target.FillGeometry(&geo, &m, None);
                            let bc = target.CreateSolidColorBrush(
                                &role_color(crate::theme::ColorRole::Border, dark),
                                None,
                            )?;
                            target.DrawGeometry(&geo, &bc, 1.0, None);
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
