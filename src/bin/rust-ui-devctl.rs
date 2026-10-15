//! Small explicit CLI: session actions or deterministic offline comparators.
use rust_ui::devtools::{self as d, Error, Image, Rect, Result, Snapshot};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
struct Args {
    command: String,
    options: BTreeMap<String, String>,
    position: Vec<String>,
    app_args: Vec<String>,
}
impl Args {
    fn parse() -> Result<Self> {
        let mut it = std::env::args().skip(1);
        let command = it.next().ok_or_else(|| {
            Error::new("INVALID_ARGUMENT", "command required; see docs/DEVCTL.md")
        })?;
        let (mut options, mut position, mut app_args) =
            (BTreeMap::<String, String>::new(), Vec::new(), Vec::new());
        while let Some(s) = it.next() {
            if s == "--" {
                app_args.extend(it);
                break;
            }
            if s == "--json" {
                continue;
            }
            if let Some(k) = s.strip_prefix("--") {
                let v = it.next().ok_or_else(|| {
                    Error::new("INVALID_ARGUMENT", format!("--{k} requires a value"))
                })?;
                if options.insert(k.into(), v).is_some() {
                    return Err(Error::new("INVALID_ARGUMENT", "duplicate option"));
                }
            } else {
                position.push(s)
            }
        }
        let allowed: &[&str] = match command.as_str() {
            "launch" => &["exe", "session"],
            "shutdown" | "cleanup" => &["session"],
            "tree" | "snapshot-layout" => &["session", "timeout", "out"],
            "wait-idle" | "leave" => &["session", "timeout"],
            "rect" | "hover" | "press" | "release" | "click" => &["session", "timeout", "id"],
            "focus" => &["session", "timeout", "id", "modality"],
            "key" => &["session", "timeout", "key"],
            "screenshot" => &["session", "timeout", "id", "out", "pad"],
            "compare-layout" => &[
                "reference",
                "capture",
                "snapshot",
                "id",
                "tolerance",
                "mode",
                "tier",
            ],
            "compare-image" => &[
                "reference-image",
                "native-image",
                "channel-tolerance",
                "diff",
            ],
            "compare-element" => &[
                "reference",
                "capture",
                "snapshot",
                "id",
                "native-id",
                "reference-image",
                "native-image",
                "out-dir",
                "channel-tolerance",
                "tolerance",
                "mode",
                "pad",
                "tier",
            ],
            _ => {
                return Err(Error::new(
                    "INVALID_ARGUMENT",
                    "unknown command; see docs/DEVCTL.md",
                ));
            }
        };
        if let Some(k) = options.keys().find(|k| !allowed.contains(&k.as_str())) {
            return Err(Error::new(
                "INVALID_ARGUMENT",
                format!("unknown --{k} for {command}"),
            ));
        }
        if position.len() > 1
            || (!position.is_empty() && (!allowed.contains(&"id") && !allowed.contains(&"key")))
            || (!position.is_empty() && (options.contains_key("id") || options.contains_key("key")))
            || (!app_args.is_empty() && command != "launch")
        {
            return Err(Error::new(
                "INVALID_ARGUMENT",
                "unexpected/ambiguous positional arguments",
            ));
        }
        Ok(Self {
            command,
            options,
            position,
            app_args,
        })
    }
    fn get(&self, k: &str) -> Option<&str> {
        self.options.get(k).map(String::as_str)
    }
    fn required(&self, k: &str) -> Result<&str> {
        self.get(k)
            .ok_or_else(|| Error::new("INVALID_ARGUMENT", format!("--{k} required")))
    }
    fn number<T: std::str::FromStr>(&self, k: &str, default: T) -> Result<T> {
        self.get(k).map_or(Ok(default), |s| {
            s.parse()
                .map_err(|_| Error::new("INVALID_ARGUMENT", format!("invalid --{k}")))
        })
    }
    fn id(&self) -> Option<&str> {
        self.get("id")
            .or_else(|| self.position.first().map(String::as_str))
    }
}
fn snapshot(p: &str) -> Result<Snapshot> {
    serde_json::from_value(d::load_json(Path::new(p))?).map_err(d::ioerr)
}
/// Reference side of a comparison: frozen `contract/0.2` (needs `--capture`)
/// or a native snapshot used as a calibration reference.
fn expected(
    a: &Args,
) -> Result<(
    BTreeMap<d::SemanticKey, d::ExpectedTarget>,
    Option<Snapshot>,
    Value,
)> {
    let v = d::load_json(Path::new(a.required("reference")?))?;
    if v["schema"] == d::SNAPSHOT_SCHEMA {
        let s: Snapshot = serde_json::from_value(v.clone()).map_err(d::ioerr)?;
        let t = d::snapshot_targets(&s)?;
        Ok((t, Some(s), v))
    } else {
        Ok((d::reference_targets(&v, a.required("capture")?)?, None, v))
    }
}
/// Integrality-checked f64 -> pixel rect for already-physical rects
/// (`rect_px`, client bounds). Logical rects must go through `snap_rect`.
fn exact_px(r: Rect) -> Result<d::PixelRect> {
    let edge = |v: f64| -> Result<i32> {
        if !v.is_finite()
            || v.fract() != 0.
            || v < i64::from(i32::MIN) as f64
            || v > i64::from(i32::MAX) as f64
        {
            return Err(Error::new("INVALID_ARGUMENT", "non-integral pixel rect"));
        }
        Ok(v as i32)
    };
    Ok(d::PixelRect {
        left: edge(r.x)?,
        top: edge(r.y)?,
        right: edge(r.x + r.width)?,
        bottom: edge(r.y + r.height)?,
    })
}
fn compare(a: &Args) -> Result<Value> {
    match a.command.as_str() {
        "compare-layout" => {
            let s = snapshot(a.required("snapshot")?)?;
            d::compare_layout(
                &expected(a)?.0,
                &s,
                a.number("tolerance", 0.)?,
                a.get("mode").unwrap_or("absolute"),
                a.id(),
                &d::TierScope::parse(a.get("tier"))?,
            )
        }
        "compare-image" => {
            let x = Image::load(Path::new(a.required("reference-image")?))?;
            let y = Image::load(Path::new(a.required("native-image")?))?;
            let (v, diff) = d::compare_image(&x, &y, a.number("channel-tolerance", 0u8)?)?;
            if let (Some(p), Some(image)) = (a.get("diff"), diff) {
                image.save(Path::new(p))?
            }
            Ok(v)
        }
        "compare-element" => {
            let id = a
                .id()
                .ok_or_else(|| Error::new("INVALID_ARGUMENT", "--id required"))?;
            let key = d::SemanticKey::parse(id)?;
            let native_key = d::SemanticKey::parse(a.get("native-id").unwrap_or(id))?;
            let tiers = d::TierScope::parse(a.get("tier"))?;
            let s = snapshot(a.required("snapshot")?)?;
            let (targets, reference_snapshot, reference_data) = expected(a)?;
            let reference_image = Image::load(Path::new(a.required("reference-image")?))?;
            let native_image = Image::load(Path::new(a.required("native-image")?))?;
            // The reference raster's declared space: snapshot client_px, or
            // the frozen contract page document at coordinate_space scale.
            let (ref_scale, ref_dims) = if let Some(rs) = &reference_snapshot {
                (rs.scale_factor, (rs.client_px.width, rs.client_px.height))
            } else {
                let page = a.required("capture")?.split('/').next().unwrap_or_default();
                let document = reference_data["pages"]
                    .as_array()
                    .and_then(|pages| {
                        pages
                            .iter()
                            .find(|p| p["id"].as_str() == Some(page))
                            .map(|p| p["document"].clone())
                    })
                    .ok_or_else(|| {
                        Error::new("INVALID_ARGUMENT", "contract page document missing")
                    })?;
                (
                    reference_data["coordinate_space"]["device_scale_factor"]
                        .as_f64()
                        .ok_or_else(|| {
                            Error::new("INVALID_ARGUMENT", "contract device_scale_factor")
                        })?,
                    (
                        document["width"].as_f64().unwrap_or(f64::NAN),
                        document["height"].as_f64().unwrap_or(f64::NAN),
                    ),
                )
            };
            let artifacts = d::compare_element(
                &targets,
                &key,
                &reference_image,
                ref_scale,
                ref_dims,
                &s,
                &native_image,
                &native_key,
                &tiers,
                a.number("pad", 0u32).map(f64::from)?,
                a.number("tolerance", 0.)?,
                a.get("mode").unwrap_or("absolute"),
                a.number("channel-tolerance", 0u8)?,
            )?;
            let out = PathBuf::from(a.required("out-dir")?);
            std::fs::create_dir_all(&out).map_err(d::ioerr)?;
            artifacts.reference_crop.save(&out.join("reference.png"))?;
            artifacts.native_raw.save(&out.join("native-raw.png"))?;
            artifacts
                .native_canonical
                .save(&out.join("native-canonical.png"))?;
            if let Some(diff) = &artifacts.diff {
                diff.save(&out.join("diff.png"))?
            }
            d::write_json(&out.join("comparison.json"), &artifacts.result)?;
            Ok(artifacts.result)
        }
        _ => Err(Error::new("NOT_SUPPORTED", &a.command)),
    }
}
#[cfg(windows)]
fn native(a: &Args) -> Result<Value> {
    use d::client as c;
    if a.command == "launch" {
        return c::launch(
            Path::new(a.required("exe")?),
            &a.app_args,
            Path::new(a.required("session")?),
        );
    }
    let path = Path::new(a.required("session")?);
    let session = c::session(path)?;
    if a.command == "cleanup" {
        return c::cleanup(path);
    }
    if a.command == "shutdown" {
        return c::shutdown(&session, path);
    }
    let timeout = a.number("timeout", 5000u32)?;
    if a.command == "screenshot" {
        c::request(&session, "wait-idle", Value::Null, timeout)?;
        let state = c::request(&session, "handshake", Value::Null, timeout)?;
        let hwnd = state["hwnd"]
            .as_u64()
            .ok_or_else(|| Error::new("PROTOCOL_ERROR", "handshake HWND"))?;
        let scene = c::request(&session, "snapshot-layout", Value::Null, timeout)?;
        let rect = if let Some(id) = a.id() {
            let snapshot: Snapshot = serde_json::from_value(scene.clone()).map_err(d::ioerr)?;
            let index = snapshot.index()?;
            let n = index
                .get(id)
                .ok_or_else(|| Error::new("TARGET_NOT_FOUND", id))?
                .0;
            Some(json!({"visible":n.visible,"rect_px":n.rect_px}))
        } else {
            None
        };
        let (mut image, mut meta) = c::capture_client(hwnd as usize)?;
        if scene != c::request(&session, "snapshot-layout", Value::Null, timeout)? {
            return Err(Error::new(
                "CAPTURE_FAILED",
                "retained semantic geometry/state changed during WGC capture; wait idle and retry",
            ));
        }
        let after = c::request(&session, "handshake", Value::Null, timeout)?;
        let before_epoch = state["paint_epoch"]
            .as_u64()
            .ok_or_else(|| Error::new("PROTOCOL_ERROR", "missing paint epoch"))?;
        let after_epoch = after["paint_epoch"]
            .as_u64()
            .ok_or_else(|| Error::new("PROTOCOL_ERROR", "missing paint epoch"))?;
        if before_epoch != after_epoch {
            return Err(Error::new(
                "CAPTURE_FAILED",
                "a native frame was painted during WGC acquisition; wait idle and retry",
            ));
        }
        meta["stable_retained_scene"] = json!(true);
        if let Some(r) = rect {
            if r["visible"] != true {
                return Err(Error::new("TARGET_NOT_VISIBLE", "target hidden"));
            }
            let r: Rect = serde_json::from_value(r["rect_px"].clone()).map_err(d::ioerr)?;
            let pad = a.number("pad", 0u32)?;
            if pad > 256 {
                return Err(Error::new("INVALID_ARGUMENT", "--pad exceeds 256"));
            }
            // rect_px is already snapped physical px; --pad here is physical
            // pixels (element evidence crop, not the comparison raster).
            let px = d::grow_px(exact_px(r)?, i64::from(pad));
            image = image.crop(&px)?;
            meta["element_crop"] =
                json!({"left":px.left,"top":px.top,"right":px.right,"bottom":px.bottom});
            meta["automation_id"] = json!(a.id());
        } else if a.get("pad").is_some() {
            return Err(Error::new("INVALID_ARGUMENT", "--pad needs --id"));
        }
        meta["scale_factor"] = state["scale_factor"].clone();
        meta["width"] = json!(image.width);
        meta["height"] = json!(image.height);
        meta["client_dp"] = state["client_dp"].clone();
        meta["client_px"] = state["client_px"].clone();
        let path = Path::new(a.required("out")?);
        image.save(path)?;
        d::write_json(&path.with_extension("capture.json"), &meta)?;
        return Ok(meta);
    }
    let args = json!({"id":a.id(),"key":a.get("key").or_else(||a.position.first().map(String::as_str)),"modality":a.get("modality")});
    let v = c::request(&session, &a.command, args, timeout)?;
    if let Some(out) = a.get("out") {
        d::write_json(Path::new(out), &v)?
    }
    Ok(v)
}
fn run() -> Result<Value> {
    let a = Args::parse()?;
    if a.command.starts_with("compare-") {
        return compare(&a);
    }
    #[cfg(windows)]
    {
        native(&a)
    }
    #[cfg(not(windows))]
    {
        Err(Error::new(
            "NOT_SUPPORTED",
            "native sessions require Windows; offline comparisons are portable",
        ))
    }
}
fn main() {
    // Keep WinRT initialized for the CLI process lifetime: capture callback
    // cleanup must not race unloading GraphicsCapture.dll during PNG encoding.
    #[cfg(windows)]
    let _capture_runtime = if std::env::args().nth(1).as_deref() == Some("screenshot") {
        d::client::CaptureRuntime::initialize().map(Some)
    } else {
        Ok(None)
    };
    #[cfg(windows)]
    let outcome = match &_capture_runtime {
        Ok(_) => run(),
        Err(e) => Err(e.clone()),
    };
    #[cfg(not(windows))]
    let outcome = run();
    let (result, code) = match outcome {
        Ok(v) => {
            let mismatch = v.get("pass") == Some(&Value::Bool(false));
            (
                json!({"ok":!mismatch,"result":v}),
                if mismatch { 6 } else { 0 },
            )
        }
        Err(e) => {
            let code = match e.code.as_str() {
                "INVALID_ARGUMENT" => 2,
                "TARGET_NOT_FOUND" | "TARGET_NOT_VISIBLE" | "SESSION_NOT_FOUND" => 3,
                "TIMEOUT" => 4,
                "CAPTURE_FAILED" => 5,
                "COMPARE_MISMATCH" => 6,
                _ => 7,
            };
            (json!({"ok":false,"error":e}), code)
        }
    };
    println!("{}", serde_json::to_string(&result).unwrap());
    std::process::exit(code)
}
