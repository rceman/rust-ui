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

/// The renderer's `ColorRole -> D2D color` step delegates to the shared
/// resolver — there is ONE palette authority (`crate::style::role_color`);
/// a renderer may adapt format, never re-author semantic values.
fn role_color(role: crate::theme::ColorRole, dark: bool) -> D2D1_COLOR_F {
    let [r, g, b, a] = crate::style::role_color(role, dark);
    D2D1_COLOR_F { r, g, b, a }
}

// ---------------------------------------------------------------------------
// DirectWrite label measure — also feeds layout's natural sizes
// ---------------------------------------------------------------------------

struct Dwrite {
    factory: IDWriteFactory,
    formats: [Option<IDWriteTextFormat>; 3],
    /// (size_bits, weight_ordinal, family) -> format — Exact sizes land
    /// here, the fixed table covers the two base recipes (14pt normal /
    /// semibold). Bounded FIFO: arbitrary Exact sizes cannot grow it
    /// forever.
    cache: std::sync::Mutex<FormatCache>,
}

const FORMAT_CACHE_MAX: usize = 64;

#[derive(Default)]
struct FormatCache {
    map: std::collections::HashMap<(u32, u32, u8), IDWriteTextFormat>,
    order: std::collections::VecDeque<(u32, u32, u8)>,
}

impl FormatCache {
    fn get(&self, k: &(u32, u32, u8)) -> Option<IDWriteTextFormat> {
        self.map.get(k).cloned()
    }
    fn insert(&mut self, k: (u32, u32, u8), v: IDWriteTextFormat) {
        if self.map.contains_key(&k) {
            self.map.insert(k, v);
            return;
        }
        while self.map.len() >= FORMAT_CACHE_MAX {
            let Some(old) = self.order.pop_front() else {
                break;
            };
            self.map.remove(&old);
        }
        self.order.push_back(k);
        self.map.insert(k, v);
    }
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
            cache: std::sync::Mutex::new(FormatCache::default()),
        })
    })
    .as_ref()
    .map_err(|e| UiError::Platform(e.to_string()))
}

/// Canonical family name for a `FontFamily` slot — the bundled Geist
/// faces (privately registered, see `fonts.rs`). `None` = Segoe UI for
/// apps that never needed parity; the Gallery asserts `fonts::geist()`.
fn family_name(f: crate::theme::FontFamily) -> windows::core::PCWSTR {
    if super::fonts::geist().sans {
        match f {
            crate::theme::FontFamily::Sans => {
                windows::core::PCWSTR(super::fonts::SANS_FAMILY.as_ptr())
            }
            crate::theme::FontFamily::Mono => {
                windows::core::PCWSTR(super::fonts::MONO_FAMILY.as_ptr())
            }
        }
    } else {
        w!("Segoe UI")
    }
}

/// Resolved `TextStyle` -> DWrite format. `Body` = the theme base (14pt),
/// `Exact` = explicit logical dp (still OS/DPI scaled by the target).
/// Family + numeric weight ride the cache key — Geist's variable wght
/// axis covers 400–800 directly.
fn fmt_for(ts: &crate::style::TextStyle, _slot_hint: usize) -> UiResult<IDWriteTextFormat> {
    let d = dwrite()?;
    let size = match ts.size {
        crate::style::TextSize::Body => 14.0,
        crate::style::TextSize::Exact(dp) => dp.0.max(1.0),
    };
    let (weight, wkey) = match ts.weight {
        crate::style::TextWeight::Normal => (DWRITE_FONT_WEIGHT_NORMAL, 0u32),
        crate::style::TextWeight::Medium => (DWRITE_FONT_WEIGHT_MEDIUM, 1),
        crate::style::TextWeight::Semibold => (DWRITE_FONT_WEIGHT_SEMI_BOLD, 2),
        crate::style::TextWeight::Bold => (DWRITE_FONT_WEIGHT_BOLD, 3),
        crate::style::TextWeight::Extrabold => (DWRITE_FONT_WEIGHT_EXTRA_BOLD, 4),
    };
    let fam = match ts.font {
        crate::theme::FontFamily::Sans => 0u8,
        crate::theme::FontFamily::Mono => 1u8,
    };
    // base table covers (14pt, normal) and (14pt, semibold) for the
    // historical Segoe consumers — Geist misses it and goes through the
    // cache like every other (size, weight, family)
    if size == 14.0 && wkey <= 1 && fam == 0 && !super::fonts::geist().sans {
        return Ok(d.formats[wkey as usize].as_ref().unwrap().clone());
    }
    let key = (size.to_bits(), wkey, fam);
    if let Some(f) = d.cache.lock().unwrap().get(&key) {
        return Ok(f);
    }
    let collection = super::fonts::collection(&d.factory);
    let fmt = unsafe {
        d.factory
            .CreateTextFormat(
                family_name(ts.font),
                collection.as_ref(),
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

/// Build a text layout for `text` inside `max_w`, applying the resolved
/// style's line pitch (`line_height` -> CSS-style centered line box),
/// `letter_spacing` and `underline`. Measurements and paint share this —
/// they must never drift.
fn layout_for(
    text: &str,
    max_w: f32,
    max_h: f32,
    ts: &crate::style::TextStyle,
) -> UiResult<IDWriteTextLayout> {
    let d = dwrite()?;
    let fmt = fmt_for(ts, 0)?;
    let wide: Vec<u16> = text.encode_utf16().collect();
    unsafe {
        let lay = d
            .factory
            .CreateTextLayout(&wide, &fmt, max_w.max(1.0), max_h.max(1.0))
            .map_err(|e| UiError::Platform(format!("CreateTextLayout: {e}")))?;
        // line pitch — CSS line-height: the glyph band centers inside
        // the pitch, baseline = (pitch + ascent - descent)/2
        if let Some(lh) = ts.line_height {
            // GetLineMetrics wants a buffer sized for ALL wrapped lines —
            // an empty first call returns E_NOT_SUFFICIENT_BUFFER but fills
            // `count`. Only line[0] feeds the uniform baseline (every line
            // shares metrics), but the buffer must still fit `count`.
            let mut count = 0u32;
            let _ = lay.GetLineMetrics(None, &mut count);
            let mut line_metrics = vec![DWRITE_LINE_METRICS::default(); count.max(1) as usize];
            lay.GetLineMetrics(Some(&mut line_metrics), &mut count)
                .map_err(|e| UiError::Platform(format!("GetLineMetrics: {e}")))?;
            let ascent = line_metrics[0].baseline;
            let band = line_metrics[0].height;
            let descent = (band - ascent).max(0.0);
            let baseline = (lh.0 + ascent - descent) / 2.0;
            lay.SetLineSpacing(DWRITE_LINE_SPACING_METHOD_UNIFORM, lh.0, baseline.max(0.0))
                .map_err(|e| UiError::Platform(format!("SetLineSpacing: {e}")))?;
        }
        if ts.underline {
            lay.SetUnderline(
                true,
                DWRITE_TEXT_RANGE {
                    startPosition: 0,
                    length: wide.len() as u32,
                },
            )
            .map_err(|e| UiError::Platform(format!("SetUnderline: {e}")))?;
        }
        if ts.letter_spacing != 0.0
            && let Ok(lay1) = lay.cast::<IDWriteTextLayout1>()
        {
            lay1.SetCharacterSpacing(
                ts.letter_spacing,
                0.0,
                0.0,
                DWRITE_TEXT_RANGE {
                    startPosition: 0,
                    length: wide.len() as u32,
                },
            )
            .map_err(|e| UiError::Platform(format!("SetCharacterSpacing: {e}")))?;
        }
        Ok(lay)
    }
}

/// Measure text at a max width — natural (w,h) in DIP.
pub(crate) fn measure_text(
    text: &str,
    max_w: f32,
    ts: &crate::style::TextStyle,
) -> UiResult<(f32, f32)> {
    let lay = layout_for(text, max_w, f32::MAX, ts)?;
    unsafe {
        let mut m = DWRITE_TEXT_METRICS::default();
        lay.GetMetrics(&mut m)
            .map_err(|e| UiError::Platform(format!("GetMetrics: {e}")))?;
        Ok((m.width, m.height))
    }
}

/// The measure closure `layout` consumes.
pub(crate) fn measure_fn() -> impl FnMut(&str, f32, &crate::style::TextStyle) -> (f32, f32) {
    |text, w, ts| measure_text(text, w, ts).unwrap_or((0.0, 20.0))
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
    dpi: super::space::ScaleFactor,
    /// bumped every time the frame target is (re)created or its DPI
    /// changes — peer-compatible surfaces key on this so device loss or a
    /// DPI move deterministically invalidates every cached bitmap
    pub(crate) target_gen: std::cell::Cell<u64>,
    /// bounded shadow raster cache — dies with the target it paints into
    shadow_cache: RefCell<ShadowCache>,
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
            dpi: super::space::ScaleFactor::ONE,
            target_gen: std::cell::Cell::new(0),
            shadow_cache: RefCell::new(ShadowCache::default()),
            tip: RefCell::new(None),
            tip_fade_start: RefCell::new(None),
        })
    }

    pub(crate) fn set_dpi(&mut self, dpi: super::space::ScaleFactor) {
        if self.dpi == dpi {
            return;
        }
        self.dpi = dpi;
        if let Some(t) = &self.target {
            let d = super::space::dpi_of(dpi) as f32;
            unsafe { t.SetDpi(d, d) };
        }
        // peer surfaces + shadow rasters key on target DPI — a scale
        // change means every cached bitmap is stale; the generation bump
        // forces re-rasterization on next paint
        self.target_gen.set(self.target_gen.get() + 1);
        self.shadow_cache.borrow_mut().clear();
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
                dpiX: super::space::dpi_of(self.dpi) as f32,
                dpiY: super::space::dpi_of(self.dpi) as f32,
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
            // device/target recreation invalidates every peer surface and
            // shadow raster — compatible bitmaps die with their target
            self.target_gen.set(self.target_gen.get() + 1);
            self.shadow_cache.borrow_mut().clear();
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
        self.dpi = be.peer_ctx.scale.get();
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
        // resolved theme darkness — the app-selected mode, not raw OS state
        let dark = be.rt.theme.dark;
        let forced = be.rt.forced_resolver();
        let dpi = self.dpi;
        // COM add-ref as the interface we paint through — ends the &mut
        // self borrow so peer draws can read renderer state
        let target: ID2D1RenderTarget = self
            .ensure_target()?
            .cast()
            .map_err(|e| UiError::Platform(format!("target cast: {e}")))?;
        let _tgen = self.target_gen.get();
        unsafe {
            let d = super::space::dpi_of(dpi) as f32;
            target.SetDpi(d, d);
        }
        // forced colors route through the SAME resolver as every other
        // role consumer — the window clear is not an exempt path
        let bg = resolve_render(
            crate::style::Color::Role(crate::theme::ColorRole::Background),
            dark,
            forced,
        );
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
                right: r.x + r.width,
                bottom: r.y + r.height,
            };
            match &n.data {
                NodeData::Label {
                    text,
                    color_role,
                    wrap,
                    patch,
                    chrome,
                } => {
                    // resolved text style: recipe (role fg, body, normal)
                    // + the surgical patch (color_role already folded in)
                    let mut ts = crate::style::label_recipe();
                    ts.patch(patch);
                    // color_role is the recipe fg unless a patch set one
                    if patch.foreground.is_none() {
                        ts.foreground = crate::style::Color::Role(*color_role);
                    }
                    unsafe {
                        // framed chrome under the text — h2 rule line,
                        // inline-code surface
                        if let Some(c) = chrome {
                            paint_box(
                                &target,
                                &clip,
                                c,
                                dark,
                                forced,
                                dpi,
                                &self.shadow_cache,
                                self.target_gen.get(),
                            )?;
                        }
                        let ins = crate::layout::node_insets(
                            id,
                            n,
                            &crate::layout::Interaction {
                                dark,
                                ..Default::default()
                            },
                        );
                        let tx = text.as_ref();
                        let draw_max_w = if *wrap {
                            (r.width - ins.left.0 - ins.right.0).max(1.0)
                        } else {
                            f32::MAX
                        };
                        if let Ok(layout) = layout_for(tx, draw_max_w, r.height.max(1.0), &ts) {
                            let c = resolve_render(ts.foreground, dark, forced);
                            let brush = target.CreateSolidColorBrush(&c, None)?;
                            target.DrawTextLayout(
                                Vector2 {
                                    X: r.x + ins.left.0,
                                    Y: r.y + ins.top.0,
                                },
                                &layout,
                                &brush,
                                D2D1_DRAW_TEXT_OPTIONS_NONE,
                            );
                        }
                    }
                }
                NodeData::Icon { icon, size, color } => {
                    let c = resolve_render(*color, dark, forced);
                    let brush = unsafe { target.CreateSolidColorBrush(&c, None)? };
                    unsafe {
                        draw_icon(&target, &self.factory, icon, *size, &r, &brush)?;
                    }
                }
                NodeData::Button {
                    text,
                    variant,
                    style,
                    disabled,
                    size,
                    icon_start,
                    icon_end,
                    icon_only,
                    invalid,
                    motion: _,
                    tooltip: _,
                    accessible_name: _,
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
                        *invalid,
                    );
                    let bs = &vs.box_style;
                    let pressed = be.pressed == Some(id);
                    let ins = crate::layout::button_content_insets(&n.data, &vs);
                    // active:translate-y-px — the whole control shifts 1dp
                    let dy = if pressed && !*disabled { 1.0 } else { 0.0 };
                    let br = D2D_RECT_F {
                        left: r.x,
                        top: r.y + dy,
                        right: r.x + r.width,
                        bottom: r.y + r.height + dy,
                    };
                    unsafe {
                        // disabled:opacity-50 — the whole control alpha-
                        // fades; the layer wraps chrome + content
                        if *disabled {
                            target.PushLayer(
                                &D2D1_LAYER_PARAMETERS {
                                    contentBounds: D2D_RECT_F {
                                        left: f32::MIN,
                                        top: f32::MIN,
                                        right: f32::MAX,
                                        bottom: f32::MAX,
                                    },
                                    opacity: 0.5,
                                    ..Default::default()
                                },
                                None,
                            );
                        }
                        paint_box(
                            &target,
                            &br,
                            bs,
                            dark,
                            forced,
                            dpi,
                            &self.shadow_cache,
                            self.target_gen.get(),
                        )?;
                        // focus ring — the RESOLVED style's ring (3dp
                        // ring/50 band outside the border); enforcement
                        // supplies it for Focus-role colors too. The
                        // invalid state carries the same 3dp band in
                        // ring-destructive and does NOT require focus.
                        if (be.focus == Some(id) || *invalid) && !*disabled {
                            let rc = resolve_render(vs.focus_ring, dark, forced);
                            let rc_brush = target.CreateSolidColorBrush(&rc, None)?;
                            let rr = D2D1_ROUNDED_RECT {
                                rect: D2D_RECT_F {
                                    left: br.left - 1.5,
                                    top: br.top - 1.5,
                                    right: br.right + 1.5,
                                    bottom: br.bottom + 1.5,
                                },
                                radiusX: bs.radii.top_left.0 + 1.5,
                                radiusY: bs.radii.top_left.0 + 1.5,
                            };
                            let _ = target.DrawRoundedRectangle(&rr, &rc_brush, 3.0, None);
                        }
                        let fg = resolve_render(vs.text_style.foreground, dark, forced);
                        let fg_brush = target.CreateSolidColorBrush(&fg, None)?;
                        // icon content — the part rect is shared geometry
                        // with measure/devtools
                        let rect_dp = crate::geom::Rect {
                            x: r.x,
                            y: r.y + dy,
                            width: r.width,
                            height: r.height,
                        };
                        for (edge, icon) in [(false, *icon_start), (true, *icon_end)] {
                            if let Some(icon) = icon
                                && let Some(ir) =
                                    crate::layout::button_icon_rect(&n.data, &vs, &rect_dp, edge)
                            {
                                draw_icon_at(&target, &self.factory, &icon, &ir, &fg_brush)?;
                            }
                        }
                        if *icon_only
                            && let Some(icon) = icon_start.or(*icon_end)
                            && let Some(ir) =
                                crate::layout::button_icon_rect(&n.data, &vs, &rect_dp, false)
                        {
                            draw_icon_at(&target, &self.factory, &icon, &ir, &fg_brush)?;
                        }
                        if !*icon_only {
                            let wide: Vec<u16> = text.as_ref().encode_utf16().collect();
                            let lay = layout_for(
                                text.as_ref(),
                                (br.right - br.left - ins.left.0 - ins.right.0).max(1.0),
                                (br.bottom - br.top).max(1.0),
                                &vs.text_style,
                            )?;
                            lay.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
                            lay.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
                            // whitespace-nowrap — button labels are always
                            // single-line; a tight box must never wrap
                            lay.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
                            target.DrawTextLayout(
                                Vector2 {
                                    X: br.left + ins.left.0,
                                    Y: br.top,
                                },
                                &lay,
                                &fg_brush,
                                D2D1_DRAW_TEXT_OPTIONS_NONE,
                            );
                        }
                        if *disabled {
                            target.PopLayer();
                        }
                    }
                }
                NodeData::Editor {
                    patch, disabled, ..
                } => {
                    // authored chrome paints the frame (the peer draws its
                    // own text/selection inside the safety inset)
                    let mut chrome = super::layout::editor_chrome(patch);
                    crate::style::os_enforce_box(&mut chrome, forced);
                    unsafe {
                        paint_box(
                            &target,
                            &clip,
                            &chrome,
                            dark,
                            forced,
                            dpi,
                            &self.shadow_cache,
                            self.target_gen.get(),
                        )?;
                        // focus ENFORCEMENT — same independent ring layer
                        if be.focus == Some(id) && !*disabled {
                            paint_focus_ring(&target, &clip, dark, forced)?;
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
                        // `editor_content_rect` is THE pill→content
                        // transform — shared with layout/pointer/caret.
                        let c = super::layout::editor_content_rect(r, &chrome);
                        peer.borrow().draw(&target, c)?;
                    }
                }
                NodeData::Custom { render, .. } => {
                    let mut canvas = D2dCanvas {
                        target: &target,
                        dark,
                        forced,
                        geo_stack: Vec::new(),
                        path_geos: Vec::new(),
                    };
                    render.paint(
                        &mut canvas,
                        crate::geom::Rect {
                            x: r.x,
                            y: r.y,
                            width: r.width,
                            height: r.height,
                        },
                    );
                }
                NodeData::Container { kind, props, .. } => {
                    // painted box containers resolve to a BoxStyle and go
                    // through the shared painter — surface = recipe + patch,
                    // box_ = authored full style
                    if let Some(bs) = props.resolved_box(*kind) {
                        paint_box(
                            &target,
                            &clip,
                            &bs,
                            dark,
                            forced,
                            dpi,
                            &self.shadow_cache,
                            self.target_gen.get(),
                        )?;
                    }
                }
                NodeData::Action {
                    style, disabled, ..
                } => {
                    let bs = style.resolve(
                        *disabled,
                        be.pressed == Some(id),
                        be.hot == Some(id),
                        be.focus == Some(id),
                    );
                    unsafe {
                        paint_box(
                            &target,
                            &clip,
                            &bs,
                            dark,
                            forced,
                            dpi,
                            &self.shadow_cache,
                            self.target_gen.get(),
                        )?;
                        // focus ENFORCEMENT — the ring lives at paint, a
                        // layer consumer patches can never erase
                        if be.focus == Some(id) && !*disabled {
                            paint_focus_ring(&target, &clip, dark, forced)?;
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
    pub(crate) fn show_tooltip(
        &mut self,
        text: &str,
        anchor: HWND,
        scale: super::space::ScaleFactor,
    ) {
        let hwnd = match self.tip.borrow().as_ref() {
            Some(t) => t.hwnd,
            None => self.create_tip(text),
        };
        unsafe {
            let (w, h) = measure_text(
                text,
                400.0,
                &crate::style::TextStyle {
                    size: crate::style::TextSize::Exact(crate::geom::Dp(13.0)),
                    ..Default::default()
                },
            )
            .unwrap_or((80.0, 20.0));
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
                right: scale.to_physical(pw),
                bottom: scale.to_physical(ph),
            };
            let _ = AdjustWindowRect(&mut tip_rc, WS_POPUP, false);
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                x,
                y,
                scale.to_physical(pw),
                scale.to_physical(ph),
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
    target: &'a ID2D1RenderTarget,
    dark: bool,
    /// forced-colors resolver — canvas fill/text roles are NOT exempt
    forced: Option<&'a dyn Fn(crate::style::SystemColor) -> [f32; 4]>,
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
                Paint::FillRole(r) => {
                    resolve_render(crate::style::Color::Role(r), self.dark, self.forced)
                }
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
            let c = resolve_render(
                crate::style::Color::Role(run.color_role),
                self.dark,
                self.forced,
            );
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
/// Color resolution under forced colors: roles go through the platform's
/// system-color lookup; authored literals stay authored (an app that names
/// an exact RGBA under HC means that RGBA).
fn resolve_render(
    c: crate::style::Color,
    dark: bool,
    sys: Option<&dyn Fn(crate::style::SystemColor) -> [f32; 4]>,
) -> D2D1_COLOR_F {
    if let (crate::style::Color::Role(r), Some(f)) = (c, sys) {
        let [rr, gg, bb, aa] = f(crate::style::system_slot(r));
        return D2D1_COLOR_F {
            r: rr,
            g: gg,
            b: bb,
            a: aa,
        };
    }
    brush_color(c, dark)
}

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
) -> Result<ID2D1Geometry> {
    unsafe {
        let f = target.GetFactory()?;
        // proportional radius normalization — adjacent pairs sharing an
        // edge scale down by ONE factor (CSS rule); independent per-corner
        // clamps distort authored proportions
        let norm = radii.normalized(r.right - r.left, r.bottom - r.top);
        if let Some(crate::geom::Dp(u)) = norm.uniform() {
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
        let (tl, tr, br, bl) = (
            norm.top_left.0,
            norm.top_right.0,
            norm.bottom_right.0,
            norm.bottom_left.0,
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
    }
}

// ---------------------------------------------------------------------------
// bounded shadow cache — one shared raster budget across all shadows
// ---------------------------------------------------------------------------

/// one cached shadow bitmap; key = shape + sigma + color + scale + target
/// generation so DPI moves and device recreation re-rasterize
#[derive(Hash, Eq, PartialEq, Clone)]
struct ShadowKey {
    target_gen: u64,
    width: u32,
    height: u32,
    pad: u32,
    radii: [u32; 4], // normalized radii bit patterns (DIP)
    sigma: u32,
    spread: u32,
    color: u32,
    scale: u32,
    /// raster bakes the offset-exclusion — same mask CANNOT serve another
    /// offset (the exclusion window translates with it)
    offset_px: (i32, i32),
}

#[derive(Default)]
pub(crate) struct ShadowCache {
    map: std::collections::HashMap<ShadowKey, (ID2D1Bitmap, usize)>,
    order: std::collections::VecDeque<ShadowKey>,
    bytes: usize,
}

const SHADOW_CACHE_MAX_ENTRIES: usize = 48;
const SHADOW_CACHE_MAX_BYTES: usize = 32 * 1024 * 1024;
/// single shadow raster ceiling — checked, not assumed
const SHADOW_MAX_DIM: u32 = 4096;

impl ShadowCache {
    pub(crate) fn get(&self, k: &ShadowKey) -> Option<&ID2D1Bitmap> {
        self.map.get(k).map(|(b, _)| b)
    }
    pub(crate) fn insert(&mut self, k: ShadowKey, bmp: ID2D1Bitmap, bytes: usize) {
        // FIFO eviction under both bounds — cache is reuse, not storage
        while self.map.len() >= SHADOW_CACHE_MAX_ENTRIES
            || self.bytes + bytes > SHADOW_CACHE_MAX_BYTES
        {
            let Some(old) = self.order.pop_front() else {
                break;
            };
            if let Some((_, sz)) = self.map.remove(&old) {
                self.bytes = self.bytes.saturating_sub(sz);
            }
        }
        self.bytes += bytes;
        self.order.push_back(k.clone());
        self.map.insert(k, (bmp, bytes));
    }
    pub(crate) fn len(&self) -> usize {
        self.map.len()
    }
    /// target/device loss or DPI change — everything derived dies
    pub(crate) fn clear(&mut self) {
        self.map.clear();
        self.order.clear();
        self.bytes = 0;
    }
}

/// Outer shadow: rasterize the rounded-rect silhouette into a CPU alpha
/// mask at DEVICE pixels (dpi-aware), box-blur it (3 passes ~= Gaussian at
/// `blur_sigma`), subtract the solid interior so translucent fills never
/// expose shadow ink under the box, premultiply with the shadow color and
/// draw it as a bitmap offset by (dx, dy). Raster results are cached
/// bounded per renderer/target generation — the same resolved shadow on
/// consecutive frames does zero raster work.
pub(crate) fn draw_shadow(
    target: &ID2D1RenderTarget,
    r: &D2D_RECT_F,
    radii: &CornerRadii,
    shadow: &Shadow,
    dark: bool,
    dpi: super::space::ScaleFactor,
    cache: &RefCell<ShadowCache>,
    target_gen: u64,
) -> Result<()> {
    let sigma = shadow.blur_sigma.0.max(0.0);
    let spread = shadow.spread.0;
    let [sr, sg, sb, sa] = crate::style::resolve_color(shadow.color, dark);
    if sa <= 0.0 || !shadow.offset_x.0.is_finite() || !shadow.offset_y.0.is_finite() {
        return Ok(());
    }
    let einval =
        || windows::core::Error::from_hresult(windows::core::HRESULT(0x8007_0057u32 as i32));
    let scale = dpi;
    // CHECKED BEFORE ARITHMETIC — every float->usize conversion happens
    // only after the value proves finite and in-range; `2 * pad` and the
    // extent additions are checked_mul/checked_add, never wrapping.
    // pad covers the blur kernel AND any positive spread (the silhouette
    // dilates by `spread` before blurring); negative spread contracts
    // inward and needs no extra pad.
    let pad_dip = sigma * 3.0 + spread.max(0.0);
    if !pad_dip.is_finite() || pad_dip < 0.0 {
        return Err(einval().into());
    }
    let pad_px_f = scale.to_physical_f(pad_dip.max(1.0));
    let rw = (r.right - r.left).max(0.0);
    let rh = (r.bottom - r.top).max(0.0);
    let w_px_f = scale.to_physical_f(rw);
    let h_px_f = scale.to_physical_f(rh);
    let lim = SHADOW_MAX_DIM as f32;
    if ![pad_px_f, w_px_f, h_px_f]
        .iter()
        .all(|v| v.is_finite() && *v >= 0.0 && *v <= lim)
    {
        return Err(einval().into()); // unrepresentable raster — typed failure
    }
    let pad = pad_px_f.ceil() as usize;
    let w_px = w_px_f.ceil() as usize;
    let h_px = h_px_f.ceil() as usize;
    let Some(two_pad) = pad.checked_mul(2) else {
        return Err(einval().into());
    };
    let Some(w) = w_px.checked_add(two_pad) else {
        return Err(einval().into());
    };
    let Some(h) = h_px.checked_add(two_pad) else {
        return Err(einval().into());
    };
    if w == 0 || h == 0 {
        return Ok(()); // degenerate box — zero ink, not a failure
    }
    if w > SHADOW_MAX_DIM as usize || h > SHADOW_MAX_DIM as usize {
        return Err(einval().into()); // extent overflow is a defect
    }
    let Some(n_px) = w.checked_mul(h) else {
        return Err(einval().into());
    };
    let bytes = n_px.checked_mul(4).unwrap_or(usize::MAX);
    if bytes > SHADOW_CACHE_MAX_BYTES {
        return Err(windows::core::Error::from_hresult(windows::core::HRESULT(
            0x8007_0057u32 as i32,
        ))
        .into());
    }
    // normalized radii are part of the raster key — authored proportions
    let norm = radii.normalized(rw, rh);
    let cbits = |v: f32| (v.clamp(0.0, 1.0) * 255.0) as u32;
    // CHECKED offsets BEFORE integer arithmetic — `as i32` saturates and
    // a later `x + ox` would wrap/overflow; an offset larger than the max
    // raster dimension cannot influence the output anyway, so it is an
    // unrepresentable raster input, not a cast.
    let ox_f = scale.to_physical_f(shadow.offset_x.0);
    let oy_f = scale.to_physical_f(shadow.offset_y.0);
    if !(ox_f.is_finite() && oy_f.is_finite() && ox_f.abs() <= lim && oy_f.abs() <= lim) {
        return Err(einval().into());
    }
    // |offset| <= SHADOW_MAX_DIM, x,y <= w,h <= SHADOW_MAX_DIM — every
    // x+ox/y+oy below is safely inside i32
    let ox_key = ox_f.round() as i32;
    let oy_key = oy_f.round() as i32;
    let key = ShadowKey {
        target_gen,
        width: w as u32,
        height: h as u32,
        pad: pad as u32,
        radii: [
            norm.top_left.0.to_bits(),
            norm.top_right.0.to_bits(),
            norm.bottom_right.0.to_bits(),
            norm.bottom_left.0.to_bits(),
        ],
        sigma: sigma.to_bits(),
        spread: spread.to_bits(),
        color: (cbits(sa) << 24) | (cbits(sr) << 16) | (cbits(sg) << 8) | cbits(sb),
        scale: scale.0.to_bits(),
        offset_px: (ox_key, oy_key),
    };
    let bw = rw / 2.0;
    let bh = rh / 2.0;
    let (rtl, rtr, rbr, rbl) = (
        norm.top_left.0,
        norm.top_right.0,
        norm.bottom_right.0,
        norm.bottom_left.0,
    );
    if let Some(bmp) = cache.borrow().get(&key) {
        // cache hit — draw the rasterized shadow at its DIP footprint
        let dest = D2D_RECT_F {
            left: r.left + shadow.offset_x.0 - pad_dip,
            top: r.top + shadow.offset_y.0 - pad_dip,
            right: r.left + shadow.offset_x.0 - pad_dip + scale.to_logical_f(w as f32),
            bottom: r.top + shadow.offset_y.0 - pad_dip + scale.to_logical_f(h as f32),
        };
        unsafe {
            target.DrawBitmap(
                bmp,
                Some(&dest),
                1.0,
                D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                None,
            );
        }
        return Ok(());
    }

    // signed distance to the per-corner rounded rect — evaluated in DIP,
    // sampled at device-pixel density (1 DIP = `scale` px)
    let sd = |px: f32, py: f32| -> f32 {
        let x = scale.to_logical_f(px + 0.5) - pad_dip;
        let y = scale.to_logical_f(py + 0.5) - pad_dip;
        let rad = if y < bh {
            if x < bw { rtl } else { rtr }
        } else if x < bw {
            rbl
        } else {
            rbr
        };
        let qx = (x - bw).abs() - (bw - rad);
        let qy = (y - bh).abs() - (bh - rad);
        let ax = qx.max(0.0);
        let ay = qy.max(0.0);
        (ax * ax + ay * ay).sqrt() + qx.max(qy).min(0.0) - rad
    };
    // SOLID silhouette — kept for interior exclusion (UNEXPANDED box)
    let mut solid = vec![0f32; n_px];
    // MASK silhouette — the box DILATED by `spread`; blur then widens
    // from the expanded shape (CSS `0 0 0 Npx` ring = spread-only).
    let mut mask = vec![0f32; n_px];
    for y in 0..h {
        for x in 0..w {
            let d = sd(x as f32, y as f32);
            // AA edge half-width shrinks in DIP terms at high DPI —
            // `to_physical_f` keeps the sigma-kernel space in px
            solid[y * w + x] = (0.5 - scale.to_physical_f(d)).clamp(0.0, 1.0);
            mask[y * w + x] = (0.5 - scale.to_physical_f(d - spread)).clamp(0.0, 1.0);
        }
    }
    // 3 box blurs approximate a Gaussian of `sigma` DIP — kernel in px
    if sigma > 0.0 {
        let kr = (scale.to_physical_f(sigma) / 1.5).max(1.0) as usize;
        for _ in 0..3 {
            // horizontal
            let mut tmp = vec![0f32; n_px];
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
            let mut out = vec![0f32; n_px];
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
    // interior exclusion — the box paints UNSHIFTED at `r`; the shadow
    // bitmap lands at `r + offset`. A translucent box fill must not show
    // shadow ink through itself, so exclude the ORIGINAL silhouette:
    // mask-space pixel (x,y) maps to box-local (x - offset_px, y - offset_px).
    // bitmap pixel x lands at box-local (x + ox_px) once drawn at +offset —
    // the ORIGINAL silhouette covers mask positions (x+ox, y+oy), so the
    // exclusion samples solid at +offset, never -offset.
    let ox_px = ox_key;
    let oy_px = oy_key;
    let excl = |i: usize| -> f32 {
        let x = (i % w) as i32 + ox_px;
        let y = (i / w) as i32 + oy_px;
        if x < 0 || y < 0 || x >= w as i32 || y >= h as i32 {
            0.0
        } else {
            solid[y as usize * w + x as usize]
        }
    };
    for (i, a) in mask.iter_mut().enumerate() {
        *a = (*a - excl(i)).max(0.0) * sa;
    }
    // premultiplied BGRA
    let mut px = vec![0u32; n_px];
    for (i, a) in mask.iter().enumerate() {
        let a = a.clamp(0.0, 1.0);
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
            left: r.left + shadow.offset_x.0 - pad_dip,
            top: r.top + shadow.offset_y.0 - pad_dip,
            right: r.left + shadow.offset_x.0 - pad_dip + scale.to_logical_f(w as f32),
            bottom: r.top + shadow.offset_y.0 - pad_dip + scale.to_logical_f(h as f32),
        };
        target.DrawBitmap(
            &bmp,
            Some(&dest),
            1.0,
            D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
            None,
        );
        cache.borrow_mut().insert(key, bmp, bytes);
    }
    Ok(())
}

/// Paint one resolved `BoxStyle` at `r` — shadow under, fill, then per-side
/// borders (later sides own shared corners, T<R<B<L).
pub(crate) fn paint_box(
    target: &ID2D1RenderTarget,
    r: &D2D_RECT_F,
    style: &BoxStyle,
    dark: bool,
    forced: Option<&dyn Fn(crate::style::SystemColor) -> [f32; 4]>,
    dpi: super::space::ScaleFactor,
    shadow_cache: &RefCell<ShadowCache>,
    target_gen: u64,
) -> Result<()> {
    // OS enforcement — forced colors map roles to the real system
    // palette and suppress decorative shadow (shared authority)
    let mut enforced;
    let style = if forced.is_some() {
        enforced = *style;
        crate::style::os_enforce_box(&mut enforced, forced);
        &enforced
    } else {
        style
    };
    unsafe {
        if let Some(sh) = &style.shadow {
            draw_shadow(
                target,
                r,
                &style.radii,
                sh,
                dark,
                dpi,
                shadow_cache,
                target_gen,
            )?;
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
                    let c = resolve_render(side.color, dark, forced);
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
                // rounded silhouette (PushLayer geometric mask). The
                // `mask` geometry stays owned — the params borrow a clone
                // that is released right after PopLayer (ManuallyDrop in
                // the field type is an ABI detail, not a leak license).
                let mask = box_geometry(target, r, &style.radii)?;
                // RAII-paired layer: EVERY fallible allocation happens
                // BEFORE PushLayer — a `?` between push/pop would strand
                // the layer and leak the params' borrowed mask clone.
                // The corner partition is the DIAGONAL bisector: each side
                // is a trapezoid whose inner edge runs between the
                // adjacent inner corners — corner cells split on the
                // diagonal, so arbitrarily thick opposing sides can never
                // overlap.
                let (mut lw, mut tw, mut rw, mut bw) = (
                    style.border.left.width.0.max(0.0),
                    style.border.top.width.0.max(0.0),
                    style.border.right.width.0.max(0.0),
                    style.border.bottom.width.0.max(0.0),
                );
                // BOUNDED — opposing insets whose sum exceeds the box
                // extent would cross; scale both so they exactly fill
                // (degenerate inner corner at the midline, never overlap)
                let (w, h) = (r.right - r.left, r.bottom - r.top);
                if lw + rw > w && w > 0.0 {
                    let k = w / (lw + rw);
                    lw *= k;
                    rw *= k;
                }
                if tw + bw > h && h > 0.0 {
                    let k = h / (tw + bw);
                    tw *= k;
                    bw *= k;
                }
                let trapezoids: [[Vector2; 4]; 4] = [
                    // top side: outer top edge, inner edge inset by lw/rw
                    [
                        Vector2 {
                            X: r.left,
                            Y: r.top,
                        },
                        Vector2 {
                            X: r.right,
                            Y: r.top,
                        },
                        Vector2 {
                            X: r.right - rw,
                            Y: r.top + tw,
                        },
                        Vector2 {
                            X: r.left + lw,
                            Y: r.top + tw,
                        },
                    ],
                    // right side: outer right edge, inner inset by tw/bw
                    [
                        Vector2 {
                            X: r.right,
                            Y: r.top,
                        },
                        Vector2 {
                            X: r.right,
                            Y: r.bottom,
                        },
                        Vector2 {
                            X: r.right - rw,
                            Y: r.bottom - bw,
                        },
                        Vector2 {
                            X: r.right - rw,
                            Y: r.top + tw,
                        },
                    ],
                    // bottom side
                    [
                        Vector2 {
                            X: r.right,
                            Y: r.bottom,
                        },
                        Vector2 {
                            X: r.left,
                            Y: r.bottom,
                        },
                        Vector2 {
                            X: r.left + lw,
                            Y: r.bottom - bw,
                        },
                        Vector2 {
                            X: r.right - rw,
                            Y: r.bottom - bw,
                        },
                    ],
                    // left side
                    [
                        Vector2 {
                            X: r.left,
                            Y: r.bottom,
                        },
                        Vector2 {
                            X: r.left,
                            Y: r.top,
                        },
                        Vector2 {
                            X: r.left + lw,
                            Y: r.top + tw,
                        },
                        Vector2 {
                            X: r.left + lw,
                            Y: r.bottom - bw,
                        },
                    ],
                ];
                let sides = [
                    style.border.top,
                    style.border.right,
                    style.border.bottom,
                    style.border.left,
                ];
                let mut parts_v: Vec<(ID2D1SolidColorBrush, ID2D1PathGeometry)> = Vec::new();
                for (i, side) in sides.iter().enumerate() {
                    if side.width.0 <= 0.0 {
                        continue;
                    }
                    let c = resolve_render(side.color, dark, forced);
                    if c.a <= 0.0 {
                        continue;
                    }
                    let b = target.CreateSolidColorBrush(&c, None)?;
                    let geo = target.GetFactory()?.CreatePathGeometry()?;
                    {
                        let sink = geo.Open()?;
                        sink.SetFillMode(D2D1_FILL_MODE_WINDING);
                        let p = trapezoids[i];
                        sink.BeginFigure(p[0], D2D1_FIGURE_BEGIN_FILLED);
                        sink.AddLines(&[p[1], p[2], p[3]]);
                        sink.EndFigure(D2D1_FIGURE_END_CLOSED);
                        sink.Close()?;
                    }
                    parts_v.push((b, geo));
                }
                // the mask clone is acquired ONLY after every fallible
                // allocation — a `?` before this point can no longer
                // strand a ManuallyDrop addref
                let mut params = D2D1_LAYER_PARAMETERS {
                    contentBounds: D2D_RECT_F {
                        left: f32::MIN,
                        top: f32::MIN,
                        right: f32::MAX,
                        bottom: f32::MAX,
                    },
                    geometricMask: std::mem::ManuallyDrop::new(Some(mask.clone())),
                    maskAntialiasMode: D2D1_ANTIALIAS_MODE_ALIASED,
                    maskTransform: Matrix3x2::identity(),
                    opacity: 1.0,
                    opacityBrush: std::mem::ManuallyDrop::new(None),
                    layerOptions: D2D1_LAYER_OPTIONS_NONE,
                };
                target.PushLayer(&params, None);
                for (b, geo) in &parts_v {
                    target.FillGeometry(geo, b, None);
                }
                target.PopLayer();
                // release the params' clone now that the layer is popped
                std::mem::ManuallyDrop::drop(&mut params.geometricMask);
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

/// Required focus indicator — painted OUTSIDE the box in the Focus role.
/// This is the enforcement layer: it runs at render after every style
/// layer resolved, so no consumer patch can erase focus visibility.
unsafe fn paint_focus_ring(
    target: &ID2D1RenderTarget,
    r: &D2D_RECT_F,
    dark: bool,
    forced: Option<&dyn Fn(crate::style::SystemColor) -> [f32; 4]>,
) -> Result<()> {
    unsafe {
        let ring = target.CreateSolidColorBrush(
            &resolve_render(
                crate::style::Color::Role(crate::theme::ColorRole::Focus),
                dark,
                forced,
            ),
            None,
        )?;
        let outer = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: r.left - 2.0,
                top: r.top - 2.0,
                right: r.right + 2.0,
                bottom: r.bottom + 2.0,
            },
            radiusX: 7.0,
            radiusY: 7.0,
        };
        let og: ID2D1RoundedRectangleGeometry = target
            .GetFactory()?
            .CreateRoundedRectangleGeometry(&outer)?;
        target.DrawGeometry(&og, &ring, 1.5, None);
    }
    Ok(())
}

/// The DIP footprint a shadow's ink can reach — the box rect offset by
/// (dx,dy) and inflated by the blur pad (3σ covers the spread). This is
/// the per-node damage contribution for shadow changes: old and new rects
/// union through it.
pub(crate) fn shadow_ink_rect(
    r: &super::space::LogicalRect,
    s: &Shadow,
) -> super::space::LogicalRect {
    let pad = (s.blur_sigma.0.max(0.0) * 3.0).max(1.0);
    super::space::LogicalRect {
        x: r.x + s.offset_x.0 - pad,
        y: r.y + s.offset_y.0 - pad,
        width: r.width + 2.0 * pad,
        height: r.height + 2.0 * pad,
    }
}

fn resolve_color_f(c: crate::style::Color, dark: bool) -> D2D1_COLOR_F {
    let [r, g, b, a] = crate::style::resolve_color(c, dark);
    D2D1_COLOR_F { r, g, b, a }
}

// ---------------------------------------------------------------------------
// Lucide icons — stroke vector geometry, scaled from the 24x24 viewBox
// ---------------------------------------------------------------------------

/// Stroke an icon's `IconOp` list at a target rect. Geometry scales
/// `size/24`; stroke width follows Lucide (2 user units at 24) so a 16dp
/// icon strokes 1.33dp — matching the frozen specimens.
unsafe fn draw_icon_at(
    target: &ID2D1RenderTarget,
    factory: &ID2D1Factory,
    icon: &crate::icons::Icon,
    r: &crate::geom::Rect,
    brush: &ID2D1SolidColorBrush,
) -> UiResult {
    let scale = r.width / 24.0;
    let geo = factory.CreatePathGeometry()?;
    {
        let sink = geo.Open()?;
        let mut open = false;
        for op in icon.path() {
            match *op {
                crate::icons::IconOp::MoveTo(x, y) => {
                    if open {
                        sink.EndFigure(D2D1_FIGURE_END_OPEN);
                    }
                    sink.BeginFigure(
                        Vector2 {
                            X: r.x + x * scale,
                            Y: r.y + y * scale,
                        },
                        D2D1_FIGURE_BEGIN_HOLLOW,
                    );
                    open = true;
                }
                crate::icons::IconOp::LineTo(x, y) => sink.AddLine(Vector2 {
                    X: r.x + x * scale,
                    Y: r.y + y * scale,
                }),
                crate::icons::IconOp::QuadTo(cx, cy, x, y) => {
                    sink.AddQuadraticBezier(&D2D1_QUADRATIC_BEZIER_SEGMENT {
                        point1: Vector2 {
                            X: r.x + cx * scale,
                            Y: r.y + cy * scale,
                        },
                        point2: Vector2 {
                            X: r.x + x * scale,
                            Y: r.y + y * scale,
                        },
                    });
                }
                crate::icons::IconOp::CubicTo(cx1, cy1, cx2, cy2, x, y) => {
                    sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                        point1: Vector2 {
                            X: r.x + cx1 * scale,
                            Y: r.y + cy1 * scale,
                        },
                        point2: Vector2 {
                            X: r.x + cx2 * scale,
                            Y: r.y + cy2 * scale,
                        },
                        point3: Vector2 {
                            X: r.x + x * scale,
                            Y: r.y + y * scale,
                        },
                    });
                }
                crate::icons::IconOp::Close => {
                    if open {
                        sink.EndFigure(D2D1_FIGURE_END_CLOSED);
                        open = false;
                    }
                }
                crate::icons::IconOp::ArcTo {
                    rx,
                    ry,
                    x_axis_rotation,
                    large_arc,
                    sweep,
                    x,
                    y,
                } => sink.AddArc(&D2D1_ARC_SEGMENT {
                    point: Vector2 {
                        X: r.x + x * scale,
                        Y: r.y + y * scale,
                    },
                    size: D2D_SIZE_F {
                        width: rx * scale,
                        height: ry * scale,
                    },
                    rotationAngle: x_axis_rotation,
                    sweepDirection: if sweep {
                        D2D1_SWEEP_DIRECTION_CLOCKWISE
                    } else {
                        D2D1_SWEEP_DIRECTION_COUNTER_CLOCKWISE
                    },
                    arcSize: if large_arc {
                        D2D1_ARC_SIZE_LARGE
                    } else {
                        D2D1_ARC_SIZE_SMALL
                    },
                }),
                crate::icons::IconOp::Circle { cx, cy, r: rad } => {
                    if open {
                        sink.EndFigure(D2D1_FIGURE_END_OPEN);
                        open = false;
                    }
                    sink.BeginFigure(
                        Vector2 {
                            X: r.x + (cx + rad) * scale,
                            Y: r.y + cy * scale,
                        },
                        D2D1_FIGURE_BEGIN_HOLLOW,
                    );
                    // two 180° arcs make the circle without ellipse ops
                    sink.AddArc(&D2D1_ARC_SEGMENT {
                        point: Vector2 {
                            X: r.x + (cx - rad) * scale,
                            Y: r.y + cy * scale,
                        },
                        size: D2D_SIZE_F {
                            width: rad * scale,
                            height: rad * scale,
                        },
                        rotationAngle: 0.0,
                        sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                        arcSize: D2D1_ARC_SIZE_LARGE,
                    });
                    sink.AddArc(&D2D1_ARC_SEGMENT {
                        point: Vector2 {
                            X: r.x + (cx + rad) * scale,
                            Y: r.y + cy * scale,
                        },
                        size: D2D_SIZE_F {
                            width: rad * scale,
                            height: rad * scale,
                        },
                        rotationAngle: 0.0,
                        sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                        arcSize: D2D1_ARC_SIZE_LARGE,
                    });
                    sink.EndFigure(D2D1_FIGURE_END_CLOSED);
                }
                crate::icons::IconOp::RoundRect { x, y, w, h, r: rad } => {
                    if open {
                        sink.EndFigure(D2D1_FIGURE_END_OPEN);
                        open = false;
                    }
                    // separate geometry — a path sink can't round corners
                    let g2 = factory.CreateRoundedRectangleGeometry(&D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: r.x + x * scale,
                            top: r.y + y * scale,
                            right: r.x + (x + w) * scale,
                            bottom: r.y + (y + h) * scale,
                        },
                        radiusX: rad * scale,
                        radiusY: rad * scale,
                    })?;
                    target.DrawGeometry(&g2, brush, 2.0 * scale, None);
                }
            }
        }
        if open {
            sink.EndFigure(D2D1_FIGURE_END_OPEN);
        }
        sink.Close()?;
    }
    let style = factory.CreateStrokeStyle(
        &D2D1_STROKE_STYLE_PROPERTIES {
            startCap: D2D1_CAP_STYLE_ROUND,
            endCap: D2D1_CAP_STYLE_ROUND,
            dashCap: D2D1_CAP_STYLE_ROUND,
            lineJoin: D2D1_LINE_JOIN_ROUND,
            ..Default::default()
        },
        None,
    )?;
    target.DrawGeometry(&geo, brush, 2.0 * scale, &style);
    Ok(())
}

/// Standalone icon leaf — the icon fills its layout rect.
unsafe fn draw_icon(
    target: &ID2D1RenderTarget,
    factory: &ID2D1Factory,
    icon: &crate::icons::Icon,
    _size: crate::geom::Dp,
    r: &super::space::LogicalRect,
    brush: &ID2D1SolidColorBrush,
) -> UiResult {
    draw_icon_at(
        target,
        factory,
        icon,
        &crate::geom::Rect {
            x: r.x,
            y: r.y,
            width: r.width,
            height: r.height,
        },
        brush,
    )
}
