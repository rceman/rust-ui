//! Direct2D/DirectWrite presenter: one HWND render target (software-
//! RichEdit content via `TxDrawD2D`, a `Canvas` impl for custom nodes, and
//! the tooltip overlay as a real nonactivating top-level HWND.
//!
//! `TxDrawD2D` draws axis-aligned rectangular-clip text into the same target
//! — live text is never scaled/animated/alpha-blended.

pub(super) use std::cell::RefCell;
pub(super) use std::sync::OnceLock;

pub(super) use windows::Win32::Foundation::*;
pub(super) use windows::Win32::Graphics::Direct2D::Common::*;
pub(super) use windows::Win32::Graphics::Direct2D::*;
pub(super) use windows::Win32::Graphics::DirectWrite::*;
pub(super) use windows::Win32::Graphics::Dxgi::Common::*;
pub(super) use windows::Win32::Graphics::Gdi::*;
pub(super) use windows::Win32::System::LibraryLoader::GetModuleHandleW;
pub(super) use windows::Win32::UI::WindowsAndMessaging::*;
pub(super) use windows::core::*;
pub(super) use windows_numerics::{Matrix3x2, Vector2};

pub(super) use crate::geom::Point;
pub(super) use crate::node::NodeData;
pub(super) use crate::runtime::UpdateCtx;
pub(super) use crate::ui::{Canvas, Paint, PathOp, TextRun};
pub(super) use crate::{NodeId, UiError, UiResult};

pub(super) use super::{Backend, LogicalRect};

const FONT: &str = "Segoe UI";

/// The renderer's `ColorRole -> D2D color` step delegates to the shared
/// resolver — there is ONE palette authority (`crate::style::role_color`);
/// a renderer may adapt format, never re-author semantic values.
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

pub(crate) fn dwrite() -> UiResult<&'static Dwrite> {
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
pub(crate) fn family_name(f: crate::theme::FontFamily) -> windows::core::PCWSTR {
    match f {
        crate::theme::FontFamily::Sans if super::fonts::geist().sans => {
            windows::core::PCWSTR(super::fonts::SANS_FAMILY.as_ptr())
        }
        crate::theme::FontFamily::Mono if super::fonts::geist().mono => {
            windows::core::PCWSTR(super::fonts::MONO_FAMILY.as_ptr())
        }
        _ => w!("Segoe UI"),
    }
}

/// Layout-affecting signature of a resolved `TextStyle` — everything that
/// can change `IDWriteTextLayout` shaping/metrics. `foreground` is excluded
/// (paint-only). Shared by the format cache key and the layout cache key.
pub(crate) fn style_sig(ts: &crate::style::TextStyle) -> (u32, u32, u8, u32, u8, u32) {
    let size = match ts.size {
        crate::style::TextSize::Body => 14.0f32.to_bits(),
        crate::style::TextSize::Exact(dp) => dp.0.max(1.0).to_bits(),
    };
    let wkey = match ts.weight {
        crate::style::TextWeight::Normal => 0u32,
        crate::style::TextWeight::Medium => 1,
        crate::style::TextWeight::Semibold => 2,
        crate::style::TextWeight::Bold => 3,
        crate::style::TextWeight::Extrabold => 4,
    };
    let fam = match ts.font {
        crate::theme::FontFamily::Sans => 0u8,
        crate::theme::FontFamily::Mono => 1,
    };
    (
        size,
        wkey,
        fam,
        ts.line_height.map(|d| d.0.to_bits()).unwrap_or(0),
        ts.underline as u8,
        ts.letter_spacing.to_bits(),
    )
}

/// `(text, style_sig, max_w, max_h)` — full inputs of `layout_for`.
type LayoutKey = (String, u32, u32, u8, u32, u8, u32, u32, u32);

/// Bounded `IDWriteTextLayout` reuse: measure + paint create a fresh COM
/// layout per call otherwise — ~350 creations per resize step on the
/// Gallery. Layout inputs are DIP/scale-free so no scale belongs in the
/// key. FIFO-bounded at 512; COM clones share the same underlying object.
struct TextLayoutCache {
    map: std::collections::HashMap<LayoutKey, IDWriteTextLayout>,
    order: std::collections::VecDeque<LayoutKey>,
}

impl TextLayoutCache {
    const CAP: usize = 512;
    fn new() -> Self {
        TextLayoutCache {
            map: std::collections::HashMap::new(),
            order: std::collections::VecDeque::new(),
        }
    }
    fn get(&self, k: &LayoutKey) -> Option<IDWriteTextLayout> {
        self.map.get(k).cloned()
    }
    fn insert(&mut self, k: LayoutKey, v: IDWriteTextLayout) {
        if self.map.contains_key(&k) {
            return;
        }
        if self.order.len() == Self::CAP {
            if let Some(old) = self.order.pop_front() {
                self.map.remove(&old);
            }
        }
        self.order.push_back(k.clone());
        self.map.insert(k, v);
    }
}

static TEXT_LAYOUTS: std::sync::LazyLock<std::sync::Mutex<TextLayoutCache>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(TextLayoutCache::new()));

/// Resolved `TextStyle` -> DWrite format. `Body` = the theme base (14pt),
/// `Exact` = explicit logical dp (still OS/DPI scaled by the target).
/// Family + numeric weight ride the cache key — Geist's variable wght
/// axis covers 400–800 directly.
pub(crate) fn fmt_for(
    ts: &crate::style::TextStyle,
    _slot_hint: usize,
) -> UiResult<IDWriteTextFormat> {
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
pub(crate) fn layout_for(
    text: &str,
    max_w: f32,
    max_h: f32,
    ts: &crate::style::TextStyle,
) -> UiResult<IDWriteTextLayout> {
    let (size, wkey, fam, lh, ul, ls) = style_sig(ts);
    let key = (
        text.to_string(),
        size,
        wkey,
        fam,
        lh,
        ul,
        ls,
        max_w.to_bits(),
        max_h.to_bits(),
    );
    if let Some(l) = TEXT_LAYOUTS.lock().unwrap().get(&key) {
        return Ok(l);
    }
    super::RESIZE
        .text_layout_creates
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
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
        TEXT_LAYOUTS.lock().unwrap().insert(key, lay.clone());
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

    /// current framebuffer size in physical px — resize-generation probe
    /// for the sizing-session trace
    pub(crate) fn target_px(&self) -> (u32, u32) {
        self.target
            .as_ref()
            .map(|t| {
                let s = unsafe { t.GetPixelSize() };
                (s.width, s.height)
            })
            .unwrap_or((0, 0))
    }

    pub(crate) fn resize(&mut self) {
        let t0 = std::time::Instant::now();
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
        super::ResizeCounters::bump(
            &super::RESIZE.resize_runs,
            &super::RESIZE.resize_us,
            &super::RESIZE.resize_max_us,
            t0.elapsed().as_micros() as u64,
        );
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
}

mod brush;
pub(crate) mod frame;
mod guards;
mod icons;
mod shadow;
mod tooltip;

pub(crate) use brush::{
    paint_box, paint_focus_ring, paint_ring_color, paint_ring_color_width, role_color, shrink_radii,
};
pub(crate) use guards::{ClipGuard, D2dCanvas, LayerGuard};
pub(crate) use icons::{
    draw_icon, draw_icon_at, inherited_action_disabled, inherited_action_ink, resolve_color_f,
    shadow_ink_rect,
};
pub(crate) use shadow::{ShadowCache, ShadowKey, draw_shadow};
