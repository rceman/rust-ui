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
            "wait-idle" => &["session", "timeout"],
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
                "reference-image",
                "native-image",
                "out-dir",
                "channel-tolerance",
                "tolerance",
                "mode",
                "pad",
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
fn expected(a: &Args) -> Result<BTreeMap<String, (Rect, bool)>> {
    let v = d::load_json(Path::new(a.required("reference")?))?;
    if v["schema"] == d::SNAPSHOT_SCHEMA {
        let s: Snapshot = serde_json::from_value(v).map_err(d::ioerr)?;
        Ok(s.index()?
            .into_iter()
            .map(|(id, (n, _))| (id, (n.rect_dp, n.visible)))
            .collect())
    } else {
        d::reference_rects(&v, a.required("capture")?)
    }
}
fn compare(a: &Args) -> Result<Value> {
    match a.command.as_str() {
        "compare-layout" => {
            let s = snapshot(a.required("snapshot")?)?;
            d::compare_layout(
                &expected(a)?,
                &s,
                a.number("tolerance", 0.)?,
                a.get("mode").unwrap_or("absolute"),
                a.id(),
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
            let s = snapshot(a.required("snapshot")?)?;
            let e = expected(a)?;
            let &(r, _) = e
                .get(id)
                .ok_or_else(|| Error::new("TARGET_NOT_FOUND", "reference ID"))?;
            let index = s.index()?;
            let n = index
                .get(id)
                .ok_or_else(|| Error::new("TARGET_NOT_FOUND", "native ID"))?
                .0;
            if !n.visible {
                return Err(Error::new("TARGET_NOT_VISIBLE", id));
            }
            let pad = a.number("pad", 0u32)?;
            let reference_data = d::load_json(Path::new(a.required("reference")?))?;
            let image_rect = if reference_data["schema"] == d::SNAPSHOT_SCHEMA {
                let ref_snapshot: Snapshot =
                    serde_json::from_value(reference_data).map_err(d::ioerr)?;
                ref_snapshot
                    .index()?
                    .get(id)
                    .ok_or_else(|| Error::new("TARGET_NOT_FOUND", "reference image ID"))?
                    .0
                    .rect_px
            } else {
                r
            };
            let (reference, reference_crop) =
                Image::load(Path::new(a.required("reference-image")?))?.crop(image_rect, pad)?;
            let (native, native_crop) =
                Image::load(Path::new(a.required("native-image")?))?.crop(n.rect_px, pad)?;
            let out = PathBuf::from(a.required("out-dir")?);
            std::fs::create_dir_all(&out).map_err(d::ioerr)?;
            reference.save(&out.join("reference.png"))?;
            native.save(&out.join("native.png"))?;
            let (image, diff) =
                d::compare_image(&reference, &native, a.number("channel-tolerance", 0u8)?)?;
            if let Some(diff) = diff {
                diff.save(&out.join("diff.png"))?
            }
            let layout = d::compare_layout(
                &e,
                &s,
                a.number("tolerance", 0.)?,
                a.get("mode").unwrap_or("absolute"),
                Some(id),
            )?;
            let result = json!({"id":id,"pass":layout["pass"]==true&&image["pass"]==true,"layout":layout,"image":image,"reference_crop":reference_crop,"native_crop":native_crop,"out_dir":out,"alignment":"independent semantic crops; no resize","padding_px":pad});
            d::write_json(&out.join("comparison.json"), &result)?;
            Ok(result)
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
            let (crop, crop_rect) = image.crop(r, a.number("pad", 0u32)?)?;
            image = crop;
            meta["element_crop"] = serde_json::to_value(crop_rect).unwrap();
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
