use super::super::*;
use super::guards::{box_geometry, brush_color, resolve_render};
use super::*;
use crate::style::{CornerRadii, Shadow};

/// one cached shadow bitmap; key = shape + sigma + color + scale + target
/// generation so DPI moves and device recreation re-rasterize
#[derive(Hash, Eq, PartialEq, Clone)]
pub(crate) struct ShadowKey {
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
    dpi: super::super::space::ScaleFactor,
    cache: &RefCell<ShadowCache>,
    target_gen: u64,
) -> Result<()> {
    let sigma = shadow.blur_sigma.0.max(0.0);
    let spread = shadow.spread.0;
    let [sr, sg, sb, sa] = crate::style::resolve_color(shadow.color, dark);
    if sa <= 0.0
        || !shadow.offset_x.0.is_finite()
        || !shadow.offset_y.0.is_finite()
        || !shadow.spread.0.is_finite()
    {
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

    // cache miss — the SDF+blur re-raster below is the expensive resize step
    super::super::RESIZE
        .shadow_rasters
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

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
