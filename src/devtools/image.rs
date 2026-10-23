use super::*;
use serde_json::{Value, json};

#[derive(Clone)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}
impl Image {
    pub fn load(p: &Path) -> Result<Self> {
        let file = std::fs::File::open(p).map_err(ioerr)?;
        let mut decoder = png::Decoder::new(file);
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        decoder.set_limits(png::Limits {
            bytes: MAX_PIXELS * 8,
        });
        let mut reader = decoder.read_info().map_err(ioerr)?;
        let info = reader.info();
        let count = (info.width as usize)
            .checked_mul(info.height as usize)
            .filter(|n| *n <= MAX_PIXELS)
            .ok_or_else(|| Error::new("INVALID_ARGUMENT", "image pixel budget"))?;
        let mut buf = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).map_err(ioerr)?;
        let b = &buf[..info.buffer_size()];
        let mut rgba = Vec::with_capacity(count * 4);
        match info.color_type {
            png::ColorType::Rgba => rgba.extend(b),
            png::ColorType::Rgb => {
                for p in b.chunks_exact(3) {
                    rgba.extend([p[0], p[1], p[2], 255])
                }
            }
            png::ColorType::Grayscale => {
                for v in b {
                    rgba.extend([*v, *v, *v, 255])
                }
            }
            png::ColorType::GrayscaleAlpha => {
                for p in b.chunks_exact(2) {
                    rgba.extend([p[0], p[0], p[0], p[1]])
                }
            }
            _ => return Err(Error::new("INVALID_ARGUMENT", "PNG color type")),
        }
        Ok(Self {
            width: info.width,
            height: info.height,
            rgba,
        })
    }
    pub fn validate(&self) -> Result<()> {
        let n = (self.width as usize)
            .checked_mul(self.height as usize)
            .filter(|n| *n > 0 && *n <= MAX_PIXELS)
            .ok_or_else(|| Error::new("INVALID_ARGUMENT", "image extent"))?;
        if self.rgba.len() != n * 4 {
            return Err(Error::new("INVALID_ARGUMENT", "RGBA length"));
        }
        Ok(())
    }
    pub fn save(&self, p: &Path) -> Result<()> {
        self.validate()?;
        atomic_write(p, |file| {
            let mut encoder = png::Encoder::new(file, self.width, self.height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .map_err(ioerr)?
                .write_image_data(&self.rgba)
                .map_err(ioerr)
        })
    }
    /// Crop at an INTEGER pixel rect (a `snap_rect`/`grow_px` result). There
    /// is no float->int rounding here: `left/top/right/bottom` are already the
    /// snapped edges. The whole rect must lie inside the image.
    pub fn crop(&self, r: &PixelRect) -> Result<Self> {
        self.validate()?;
        let (l, t) = (i64::from(r.left), i64::from(r.top));
        let (w, h) = px_dims(r);
        if w <= 0
            || h <= 0
            || l < 0
            || t < 0
            || i64::from(r.right) > i64::from(self.width)
            || i64::from(r.bottom) > i64::from(self.height)
        {
            return Err(Error::new("TARGET_NOT_VISIBLE", "crop outside image"));
        }
        let (x, y, w, h) = (l as u32, t as u32, w as u32, h as u32);
        let mut rgba = Vec::with_capacity(w as usize * h as usize * 4);
        for yy in y..y + h {
            let start = ((yy * self.width + x) * 4) as usize;
            rgba.extend(&self.rgba[start..start + w as usize * 4]);
        }
        Ok(Self {
            width: w,
            height: h,
            rgba,
        })
    }
    /// `area-box/1`: resample a RAW raster at `source_scale` px/Dp onto the
    /// CANONICAL_COMPARISON_RASTER lattice — one 1.0-px/Dp pixel per logical
    /// Dp of `canonical` (a `snap_rect(.., 1.0)` result). Canonical pixel
    /// (i,j) covers the source box `[s*(C.left+i), s*(C.left+i+1))` x
    /// `[s*(C.top+j), s*(C.top+j+1))`; each channel is the exact
    /// fractional-coverage area average of overlapping source pixels
    /// (separable weights, straight sRGB-encoded RGBA channels, f64
    /// accumulation in row-major order, round half up, clamp 0..255). A
    /// footprint reaching outside the source fails TARGET_NOT_VISIBLE (no
    /// edge clamping/replication). At `source_scale == 1.0` footprints are
    /// exact source pixels and the output is bit-identical to `crop`.
    pub fn canonicalize(&self, source_scale: f64, canonical: &PixelRect) -> Result<Image> {
        self.validate()?;
        if !source_scale.is_finite() || source_scale <= 0. {
            return Err(Error::new("INVALID_ARGUMENT", "source scale"));
        }
        let (w, h) = px_dims(canonical);
        if w <= 0 || h <= 0 || w as u64 * h as u64 > MAX_PIXELS as u64 {
            return Err(Error::new("INVALID_ARGUMENT", "canonical extent"));
        }
        let (w, h) = (w as u32, h as u32);
        let mut rgba = vec![0; w as usize * h as usize * 4];
        for j in 0..i64::from(h) {
            for i in 0..i64::from(w) {
                // s * (C.left + i): one product per edge so non-binary scales
                // cannot fabricate a zero-overlap boundary source pixel.
                let fx0 = source_scale * (i64::from(canonical.left) + i) as f64;
                let fx1 = source_scale * (i64::from(canonical.left) + i + 1) as f64;
                let fy0 = source_scale * (i64::from(canonical.top) + j) as f64;
                let fy1 = source_scale * (i64::from(canonical.top) + j + 1) as f64;
                let (sx0, sx1) = (fx0.floor() as i64, fx1.ceil() as i64);
                let (sy0, sy1) = (fy0.floor() as i64, fy1.ceil() as i64);
                if sx0 < 0 || sy0 < 0 || sx1 > i64::from(self.width) || sy1 > i64::from(self.height)
                {
                    return Err(Error::new(
                        "TARGET_NOT_VISIBLE",
                        "canonical footprint outside raw capture",
                    ));
                }
                let mut acc = [0f64; 4];
                let mut weight = 0f64;
                for sy in sy0..sy1 {
                    let wy = fy1.min((sy + 1) as f64) - fy0.max(sy as f64);
                    for sx in sx0..sx1 {
                        let wx = fx1.min((sx + 1) as f64) - fx0.max(sx as f64);
                        let wpx = wx * wy;
                        if wpx <= 0. {
                            continue;
                        }
                        let base = ((sy as usize * self.width as usize) + sx as usize) * 4;
                        for c in 0..4 {
                            acc[c] += f64::from(self.rgba[base + c]) * wpx;
                        }
                        weight += wpx;
                    }
                }
                if weight <= 0. {
                    return Err(Error::new("TARGET_NOT_VISIBLE", "empty footprint"));
                }
                let out = ((j as usize * w as usize) + i as usize) * 4;
                for c in 0..4 {
                    rgba[out + c] = (acc[c] / weight).round().clamp(0., 255.) as u8;
                }
            }
        }
        Ok(Image {
            width: w,
            height: h,
            rgba,
        })
    }
}
pub fn compare_image(a: &Image, b: &Image, tolerance: u8) -> Result<(Value, Option<Image>)> {
    a.validate()?;
    b.validate()?;
    let w = a.width.max(b.width);
    let h = a.height.max(b.height);
    let total = w as u64 * h as u64;
    if total > MAX_PIXELS as u64 {
        return Err(Error::new(
            "INVALID_ARGUMENT",
            "difference canvas pixel budget",
        ));
    }
    let (mut count, mut missing, mut max, mut sum) = (0u64, 0u64, 0u8, 0u64);
    let mut bbox: Option<(u32, u32, u32, u32)> = None;
    let mut heat = Vec::with_capacity(total as usize * 4);
    for y in 0..h {
        for x in 0..w {
            let pixel = |image: &Image| -> Option<[u8; 4]> {
                if x < image.width && y < image.height {
                    let i = (y as usize * image.width as usize + x as usize) * 4;
                    Some(image.rgba[i..i + 4].try_into().unwrap())
                } else {
                    None
                }
            };
            let pa = pixel(a);
            let pb = pixel(b);
            let absent = pa.is_none() || pb.is_none();
            let p = pa.unwrap_or([0; 4]);
            let q = pb.unwrap_or([0; 4]);
            let mut peak = 0;
            for i in 0..4 {
                let delta = p[i].abs_diff(q[i]);
                peak = peak.max(delta);
                sum += u64::from(delta);
            }
            max = max.max(peak);
            if absent {
                missing += 1;
                heat.extend([255, 0, 255, 255]);
            } else {
                heat.extend([peak.saturating_mul(4), 0, 0, 255]);
            }
            if absent || peak > tolerance {
                count += 1;
                bbox = Some(match bbox {
                    None => (x, y, x + 1, y + 1),
                    Some((l, t, r, b)) => (l.min(x), t.min(y), r.max(x + 1), b.max(y + 1)),
                });
            }
        }
    }
    let dimensions = a.width == b.width && a.height == b.height;
    Ok((
        json!({"pass":dimensions&&count==0,"dimensions_match":dimensions,"expected":[a.width,a.height],"actual":[b.width,b.height],"dimensions":[w,h],"tolerance":tolerance,"mismatched_pixels":count,"missing_pixels":missing,"pixel_domain":"union canvas; absent samples transparent; missing pixels always mismatch","mismatch_percent":count as f64*100./total as f64,"max_channel_delta":max,"mean_absolute_channel_error":sum as f64/(total*4) as f64,"difference_bbox":bbox.map(|(l,t,r,b)|Rect{x:l as f64,y:t as f64,width:(r-l) as f64,height:(b-t) as f64})}),
        Some(Image {
            width: w,
            height: h,
            rgba: heat,
        }),
    ))
}
pub fn load_json(path: &Path) -> Result<Value> {
    let f = std::fs::File::open(path).map_err(ioerr)?;
    let mut bytes = Vec::new();
    f.take(MAX_MESSAGE as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(ioerr)?;
    decode(&bytes)
}
// Publish only complete artifacts. A failed encode/write preserves an existing
// destination and the temporary file is removed on every failure.
fn atomic_write(path: &Path, write: impl FnOnce(std::fs::File) -> Result<()>) -> Result<()> {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let temp = path.with_extension(format!("devctl-{}-{n}.tmp", std::process::id()));
    struct Temp(std::path::PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp)
        .map_err(ioerr)?;
    let guard = Temp(temp.clone());
    write(file)?;
    std::fs::rename(&temp, path).map_err(ioerr)?;
    drop(guard);
    Ok(())
}
pub fn write_json(path: &Path, v: &impl Serialize) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(v).map_err(ioerr)?;
    if bytes.len() > MAX_MESSAGE {
        return Err(Error::new("MESSAGE_TOO_LARGE", "JSON output exceeds 4 MiB"));
    }
    atomic_write(path, |mut f| f.write_all(&bytes).map_err(ioerr))
}
