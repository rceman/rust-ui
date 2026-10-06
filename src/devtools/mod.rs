//! Opt-in development protocol and deterministic comparison helpers.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    path::Path,
};
pub const PROTOCOL: &str = "rust-ui.devctl/0.1";
pub const MAX_MESSAGE: usize = 4 * 1024 * 1024;
pub const MAX_PIXELS: usize = 32 * 1024 * 1024;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub protocol: String,
    pub token: String,
    pub id: u64,
    pub command: String,
    pub arguments: Value,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Error {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}
impl Error {
    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }
    pub fn new(code: &str, message: impl ToString) -> Self {
        Self {
            code: code.into(),
            message: message.to_string(),
            details: None,
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub protocol: String,
    pub id: u64,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Error>,
}
impl Response {
    pub fn from_result(id: u64, r: Result<Value>) -> Self {
        match r {
            Ok(v) => Self {
                protocol: PROTOCOL.into(),
                id,
                ok: true,
                result: Some(v),
                error: None,
            },
            Err(e) => Self {
                protocol: PROTOCOL.into(),
                id,
                ok: false,
                result: None,
                error: Some(e),
            },
        }
    }
}
pub fn encode<T: Serialize>(v: &T) -> Result<Vec<u8>> {
    let bytes = serde_json::to_vec(v).map_err(|e| Error::new("PROTOCOL_ERROR", e))?;
    if bytes.len() > MAX_MESSAGE {
        return Err(Error::new("MESSAGE_TOO_LARGE", "4 MiB limit"));
    }
    let mut out = (bytes.len() as u32).to_le_bytes().to_vec();
    out.extend(bytes);
    Ok(out)
}
pub fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    if bytes.len() > MAX_MESSAGE {
        return Err(Error::new("MESSAGE_TOO_LARGE", "4 MiB limit"));
    }
    serde_json::from_slice(bytes).map_err(|e| Error::new("PROTOCOL_ERROR", e))
}
pub fn read_frame<R: Read, T: serde::de::DeserializeOwned>(r: &mut R) -> Result<T> {
    let mut n = [0; 4];
    r.read_exact(&mut n).map_err(ioerr)?;
    let n = u32::from_le_bytes(n) as usize;
    if n > MAX_MESSAGE {
        return Err(Error::new("MESSAGE_TOO_LARGE", "frame exceeds 4 MiB"));
    }
    let mut b = vec![0; n];
    r.read_exact(&mut b).map_err(ioerr)?;
    decode(&b)
}
pub fn ioerr(e: impl ToString) -> Error {
    Error::new("IO_ERROR", e)
}
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
impl Rect {
    pub fn valid(self) -> bool {
        [self.x, self.y, self.width, self.height]
            .iter()
            .all(|x| x.is_finite())
            && self.width >= 0.
            && self.height >= 0.
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub automation_id: Option<String>,
    pub kind: String,
    pub label: String,
    pub rect_dp: Rect,
    pub rect_px: Rect,
    pub visible: bool,
    pub enabled: bool,
    pub focused: bool,
    pub focus_visible: bool,
    #[serde(default)]
    pub hovered: bool,
    #[serde(default)]
    pub pressed: bool,
    pub children: Vec<Node>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub schema: String,
    pub scale_factor: f64,
    pub client_dp: Rect,
    pub client_px: Rect,
    pub root: Node,
}
pub const SNAPSHOT_SCHEMA: &str = "rust-ui.devctl.snapshot-layout/0.1";
impl Snapshot {
    pub fn index(&self) -> Result<BTreeMap<String, (&Node, Option<&Node>)>> {
        if self.schema != SNAPSHOT_SCHEMA
            || !self.scale_factor.is_finite()
            || self.scale_factor <= 0.
        {
            return Err(Error::new("INVALID_ARGUMENT", "snapshot schema/scale"));
        }
        let mut out = BTreeMap::new();
        let mut count = 0;
        fn visit<'a>(
            n: &'a Node,
            p: Option<&'a Node>,
            o: &mut BTreeMap<String, (&'a Node, Option<&'a Node>)>,
            depth: usize,
            count: &mut usize,
        ) -> Result<()> {
            *count += 1;
            if depth > 128 || *count > 8192 || !n.rect_dp.valid() || !n.rect_px.valid() {
                return Err(Error::new("INVALID_ARGUMENT", "snapshot bounds"));
            }
            if let Some(id) = &n.automation_id {
                if o.insert(id.clone(), (n, p)).is_some() {
                    return Err(Error::new("INVALID_ARGUMENT", "duplicate automation ID"));
                }
            }
            for ch in &n.children {
                visit(ch, Some(n), o, depth + 1, count)?
            }
            Ok(())
        }
        visit(&self.root, None, &mut out, 0, &mut count)?;
        Ok(out)
    }
}
pub fn reference_rects(v: &Value, capture: &str) -> Result<BTreeMap<String, (Rect, bool)>> {
    if v["$schema"] != "rust-ui.shadcn-reference.contract/0.2" {
        return Err(Error::new("INVALID_ARGUMENT", "reference contract schema"));
    }
    let mut out = BTreeMap::new();
    for el in v["elements"]
        .as_array()
        .ok_or_else(|| Error::new("INVALID_ARGUMENT", "elements missing"))?
    {
        if let Some(cap) = el["captures"].get(capture) {
            let id = el["automation_id"]
                .as_str()
                .ok_or_else(|| Error::new("INVALID_ARGUMENT", "reference ID"))?;
            let rect: Rect = serde_json::from_value(cap["rect"].clone()).map_err(ioerr)?;
            if !rect.valid() || out.insert(id.into(), (rect, true)).is_some() {
                return Err(Error::new("INVALID_ARGUMENT", "reference geometry/ID"));
            }
        }
    }
    if out.is_empty() {
        return Err(Error::new("TARGET_NOT_FOUND", "capture has no elements"));
    }
    Ok(out)
}
pub fn compare_layout(
    expected: &BTreeMap<String, (Rect, bool)>,
    actual: &Snapshot,
    tolerance: f64,
    mode: &str,
    id: Option<&str>,
) -> Result<Value> {
    if !tolerance.is_finite() || tolerance < 0. || !matches!(mode, "absolute" | "size-only") {
        return Err(Error::new("INVALID_ARGUMENT", "tolerance/mode"));
    }
    let nodes = actual.index()?;
    let mut rows = Vec::new();
    let mut missing = 0;
    let mut failed = 0;
    for (key, (want, visible)) in expected {
        if id.is_some_and(|id| id != key) {
            continue;
        }
        let Some((node, _parent)) = nodes.get(key) else {
            missing += 1;
            rows.push(json!({"id":key,"pass":false,"missing":true}));
            continue;
        };
        let got = node.rect_dp;
        let delta = Rect {
            x: got.x - want.x,
            y: got.y - want.y,
            width: got.width - want.width,
            height: got.height - want.height,
        };
        let pass = (*visible == node.visible)
            && delta.width.abs() <= tolerance
            && delta.height.abs() <= tolerance
            && (mode == "size-only" || (delta.x.abs() <= tolerance && delta.y.abs() <= tolerance));
        if !pass {
            failed += 1
        }
        rows.push(json!({"id":key,"expected":want,"actual":got,"delta":delta,"tolerance":tolerance,"pass":pass}));
    }
    if rows.is_empty() {
        return Err(Error::new("TARGET_NOT_FOUND", "ID absent from expected"));
    }
    let unexpected: Vec<_> = nodes
        .keys()
        .filter(|k| !expected.contains_key(*k) && id.is_none())
        .cloned()
        .collect();
    Ok(
        json!({"mode":mode,"pass":missing==0&&failed==0&&unexpected.is_empty(),"matched":rows.len()-missing,"missing":missing,"unexpected":unexpected,"failed":failed,"comparisons":rows}),
    )
}
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
    pub fn crop(&self, r: Rect, pad: u32) -> Result<(Self, Rect)> {
        self.validate()?;
        if !r.valid() || pad > 256 {
            return Err(Error::new("INVALID_ARGUMENT", "crop rect/padding"));
        }
        let x = r.x.floor() - pad as f64;
        let y = r.y.floor() - pad as f64;
        let right = (r.x + r.width).ceil() + pad as f64;
        let bottom = (r.y + r.height).ceil() + pad as f64;
        if r.width <= 0.
            || r.height <= 0.
            || x < 0.
            || y < 0.
            || right > self.width as f64
            || bottom > self.height as f64
        {
            return Err(Error::new(
                "TARGET_NOT_VISIBLE",
                "full crop including padding must be visible",
            ));
        }
        let (x, y, w, h) = (x as u32, y as u32, (right - x) as u32, (bottom - y) as u32);
        let mut rgba = Vec::with_capacity(w as usize * h as usize * 4);
        for yy in y..y + h {
            let start = ((yy * self.width + x) * 4) as usize;
            rgba.extend(&self.rgba[start..start + w as usize * 4]);
        }
        Ok((
            Self {
                width: w,
                height: h,
                rgba,
            },
            Rect {
                x: x as f64,
                y: y as f64,
                width: w as f64,
                height: h as f64,
            },
        ))
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
#[cfg(windows)]
#[path = "../platform/win32/devctl_client.rs"]
pub mod client;

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Snapshot {
        let r = Rect {
            x: 10.,
            y: 20.,
            width: 30.,
            height: 40.,
        };
        Snapshot {
            schema: SNAPSHOT_SCHEMA.into(),
            scale_factor: 1.25,
            client_dp: Rect {
                x: 0.,
                y: 0.,
                width: 100.,
                height: 100.,
            },
            client_px: Rect {
                x: 0.,
                y: 0.,
                width: 125.,
                height: 125.,
            },
            root: Node {
                automation_id: Some("probe.button".into()),
                kind: "button".into(),
                label: "test \"\\\n\r\t é".into(),
                rect_dp: r,
                rect_px: Rect {
                    x: 13.,
                    y: 25.,
                    width: 37.,
                    height: 50.,
                },
                visible: true,
                enabled: true,
                focused: false,
                focus_visible: false,
                hovered: false,
                pressed: false,
                children: vec![],
            },
        }
    }
    #[test]
    fn protocol_round_trip_and_json_escaping() {
        let r = Request {
            protocol: PROTOCOL.into(),
            token: "nonce".into(),
            id: 42,
            command: "tree".into(),
            arguments: json!({"s":"\"\\\n\r\t\u{1}日本"}),
        };
        let encoded = encode(&r).unwrap();
        let got: Request = read_frame(&mut &encoded[..]).unwrap();
        assert_eq!(got.arguments, r.arguments);
        assert_eq!(got.id, 42);
    }
    #[test]
    fn oversized_and_malformed_frames_fail() {
        assert_eq!(
            read_frame::<_, Request>(&mut &((MAX_MESSAGE + 1) as u32).to_le_bytes()[..])
                .unwrap_err()
                .code,
            "MESSAGE_TOO_LARGE"
        );
        assert!(decode::<Request>(b"{}").is_err());
        assert!(decode::<Value>(b"{wrong}").is_err());
    }
    #[test]
    fn response_error_code_preserved() {
        let r = Response::from_result(9, Err(Error::new("TARGET_NOT_FOUND", "probe")));
        assert!(!r.ok);
        assert_eq!(r.error.unwrap().code, "TARGET_NOT_FOUND");
        assert!(r.result.is_none());
    }
    #[test]
    fn deterministic_snapshot_and_strict_duplicates() {
        let mut s = fixture();
        assert_eq!(
            serde_json::to_vec(&s).unwrap(),
            serde_json::to_vec(&s.clone()).unwrap()
        );
        let clone = s.root.clone();
        s.root.children.push(clone);
        assert!(s.index().is_err());
        s.root.children.clear();
        s.scale_factor = f64::NAN;
        assert!(s.index().is_err());
    }
    #[test]
    fn layout_self_and_one_dp_mutation() {
        let mut s = fixture();
        let expected = s
            .index()
            .unwrap()
            .into_iter()
            .map(|(k, (v, _))| (k, (v.rect_dp, v.visible)))
            .collect();
        assert_eq!(
            compare_layout(&expected, &s, 0., "absolute", None).unwrap()["pass"],
            true
        );
        s.root.rect_dp.x += 1.;
        let result = compare_layout(&expected, &s, 0., "absolute", None).unwrap();
        assert_eq!(result["pass"], false);
        assert_eq!(result["comparisons"][0]["delta"]["x"], 1.);
        assert!(compare_layout(&expected, &s, 0., "absolute", Some("missing")).is_err());
        assert!(compare_layout(&expected, &s, f64::NAN, "absolute", None).is_err());
    }
    #[test]
    fn image_self_and_one_pixel_bbox() {
        let a = Image {
            width: 3,
            height: 2,
            rgba: vec![0; 24],
        };
        let mut b = a.clone();
        assert_eq!(compare_image(&a, &b, 0).unwrap().0["mismatched_pixels"], 0);
        b.rgba[(1 * 3 + 2) * 4] = 255;
        let (v, diff) = compare_image(&a, &b, 0).unwrap();
        assert_eq!(v["mismatched_pixels"], 1);
        assert_eq!(
            v["difference_bbox"],
            json!({"x":2.,"y":1.,"width":1.,"height":1.})
        );
        assert_eq!(v["max_channel_delta"], 255);
        assert_eq!(diff.unwrap().rgba[(1 * 3 + 2) * 4], 255);
    }
    #[test]
    fn crop_visible_bounds_and_padding() {
        let a = Image {
            width: 3,
            height: 2,
            rgba: vec![128; 24],
        };
        let (b, r) = a
            .crop(
                Rect {
                    x: 1.,
                    y: 0.,
                    width: 2.,
                    height: 2.,
                },
                0,
            )
            .unwrap();
        assert_eq!((b.width, b.height), (2, 2));
        assert_eq!(r.x, 1.);
        assert_eq!(a.crop(r, 1).err().unwrap().code, "TARGET_NOT_VISIBLE");
        assert!(a.crop(r, 257).is_err());
    }
    #[test]
    fn png_round_trip() {
        let a = Image {
            width: 3,
            height: 2,
            rgba: vec![128; 24],
        };
        let p = std::env::temp_dir().join(format!("rust-ui-devctl-png-{}.png", std::process::id()));
        a.save(&p).unwrap();
        let b = Image::load(&p).unwrap();
        std::fs::remove_file(p).unwrap();
        assert_eq!(a.rgba, b.rgba);
    }
    #[test]
    fn frozen_reference_adapter_uses_capture_identity() {
        let v = json!({"$schema":"rust-ui.shadcn-reference.contract/0.2","elements":[{"automation_id":"button.default","captures":{"components/default":{"rect":{"x":1,"y":2,"width":30,"height":32}},"components/pressed":{"rect":{"x":1,"y":3,"width":30,"height":32}}}}]});
        assert_eq!(
            reference_rects(&v, "components/pressed").unwrap()["button.default"]
                .0
                .y,
            3.
        );
        assert!(reference_rects(&v, "missing/default").is_err());
    }
    #[test]
    fn unequal_dimensions_produce_diff_without_resizing() {
        let a = Image {
            width: 2,
            height: 2,
            rgba: vec![255; 16],
        };
        let b = Image {
            width: 1,
            height: 2,
            rgba: vec![255; 8],
        };
        let (v, diff) = compare_image(&a, &b, 0).unwrap();
        assert_eq!(v["dimensions_match"], false);
        assert_eq!(v["missing_pixels"], 2);
        assert_eq!(
            v["difference_bbox"],
            json!({"x":1.,"y":0.,"width":1.,"height":2.})
        );
        assert_eq!(diff.unwrap().width, 2);
    }
    #[test]
    fn image_invalid_extent_rejected() {
        assert!(
            compare_image(
                &Image {
                    width: 0,
                    height: 0,
                    rgba: vec![]
                },
                &Image {
                    width: 0,
                    height: 0,
                    rgba: vec![]
                },
                0
            )
            .is_err()
        );
        assert!(
            Image {
                width: 3,
                height: 3,
                rgba: vec![0; 4]
            }
            .validate()
            .is_err()
        );
    }
}
