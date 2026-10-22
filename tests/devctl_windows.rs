#![cfg(all(windows, feature = "devtools"))]
//! Real CLI + pipe + retained runtime + WGC protocol integration — run
//! end-to-end against `examples/gallery`, rust-ui's single real native
//! UI acceptance application (launched through the `rust-ui-gallery`
//! [[bin]] alias, the same compilation unit, so `CARGO_BIN_EXE_`
//! resolves deterministically). There is no synthetic devctl UI
//! fixture: every assertion below exercises durable Gallery semantics.
use rust_ui::devtools::{self as d, Image, Snapshot};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::Command,
};
fn cli(args: &[&str]) -> (i32, Value) {
    let out = Command::new(env!("CARGO_BIN_EXE_rust-ui-devctl"))
        .args(args)
        .arg("--json")
        .output()
        .unwrap();
    let v = serde_json::from_slice(&out.stdout).unwrap_or_else(|_| {
        panic!(
            "invalid CLI JSON: {:?}; stderr={}",
            out.stdout,
            String::from_utf8_lossy(&out.stderr)
        )
    });
    (out.status.code().unwrap_or(-1), v)
}
fn good(args: &[&str]) -> Value {
    let (code, v) = cli(args);
    assert_eq!(code, 0, "{v}");
    v["result"].clone()
}
struct Session {
    file: PathBuf,
}
impl Drop for Session {
    fn drop(&mut self) {
        if self.file.exists() {
            let _ = cli(&["shutdown", "--session", self.file.to_str().unwrap()]);
        }
    }
}
fn action(session: &str, command: &str, more: &[&str]) -> Value {
    let mut args = vec![command, "--session", session];
    args.extend_from_slice(more);
    good(&args)
}
fn snap(v: &Value) -> Snapshot {
    serde_json::from_value(v.clone()).unwrap()
}
fn cpu_ticks(pid: u32) -> u64 {
    use windows::Win32::{
        Foundation::{CloseHandle, FILETIME},
        System::Threading::*,
    };
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).unwrap();
        let (mut created, mut exited, mut kernel, mut user) = (
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
        );
        GetProcessTimes(h, &mut created, &mut exited, &mut kernel, &mut user).unwrap();
        CloseHandle(h).unwrap();
        let ticks = |t: FILETIME| (u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime);
        ticks(kernel) + ticks(user)
    }
}
fn handles(pid: u32) -> u32 {
    use windows::Win32::{Foundation::CloseHandle, System::Threading::*};
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).unwrap();
        let mut count = 0;
        GetProcessHandleCount(h, &mut count).unwrap();
        CloseHandle(h).unwrap();
        count
    }
}
#[test]
fn native_agent_workflow_and_bounded_repetition() {
    let root = std::env::temp_dir().join(format!(
        "rust-ui-devctl-proof-{}",
        d::client::nonce().unwrap()
    ));
    std::fs::create_dir(&root).unwrap();
    // the real Gallery application — `rust-ui-gallery` compiles
    // examples/gallery.rs verbatim; Cargo-managed exe discovery, no
    // filesystem guessing
    let exe = Path::new(env!("CARGO_BIN_EXE_rust-ui-gallery")).to_path_buf();
    assert!(exe.exists(), "Gallery launch target must exist");
    for cycle in 0..3 {
        let file = root.join(format!("session-{cycle}.json"));
        let owner = Session { file: file.clone() };
        let session = file.to_str().unwrap();
        let launch = good(&[
            "launch",
            "--exe",
            exe.to_str().unwrap(),
            "--session",
            session,
        ]);
        assert_eq!(launch["protocol"], d::PROTOCOL);
        assert!(launch["window"]["paint_epoch"].as_u64().is_some());
        let scale = launch["window"]["scale_factor"].as_f64().unwrap();
        assert!(scale > 0.);
        action(session, "wait-idle", &[]);
        // Gallery boots on the Components page — stable, deterministic
        // initial state; the semantic index must expose durable IDs and
        // must NOT expose other pages' specimens
        let initial = action(session, "tree", &[]);
        let initial_snap = snap(&initial);
        let index = initial_snap.index().unwrap();
        for id in ["shell.root", "shell.nav.forms", "button.default"] {
            assert!(index.contains_key(id), "{id} must be in the tree");
        }
        assert!(
            !index.contains_key("input.default.filled"),
            "Forms specimens are not mounted on the Components page"
        );
        let rect = action(session, "rect", &["button.default"]);
        let r = rect["rect_dp"].clone();
        assert_eq!(
            rect["rect_px"]["x"].as_f64().unwrap(),
            rust_ui::ScaleFactor::new(scale as f32)
                .unwrap()
                .to_physical(r["x"].as_f64().unwrap() as f32) as f64
        );
        let (code, error) = cli(&["rect", "not-a-semantic-id", "--session", session]);
        assert_eq!(code, 3);
        assert_eq!(error["error"]["code"], "TARGET_NOT_FOUND");
        let mut wrong = d::client::session(&file).unwrap();
        wrong.token = "0".repeat(64);
        assert_eq!(
            d::client::request(&wrong, "tree", Value::Null, 5000)
                .unwrap_err()
                .code,
            "PROTOCOL_MISMATCH"
        );
        // runtime pointer proof — button.default carries no Presented
        // override, so hover/press visuals and the `hovered` field are
        // REAL runtime state
        let baseline = root.join(format!("normal-{cycle}.png"));
        action(
            session,
            "screenshot",
            &[
                "--id",
                "button.default",
                "--out",
                baseline.to_str().unwrap(),
            ],
        );
        action(session, "hover", &["button.default"]);
        action(session, "wait-idle", &[]);
        let hovered: Snapshot = snap(&action(session, "tree", &[]));
        assert_eq!(
            hovered.index().unwrap()["button.default"].0.hovered,
            true,
            "hover must report runtime hot"
        );
        let hover = root.join(format!("hover-{cycle}.png"));
        action(
            session,
            "screenshot",
            &["--id", "button.default", "--out", hover.to_str().unwrap()],
        );
        let before_handles = handles(launch["pid"].as_u64().unwrap() as u32);
        let normal = Image::load(&baseline).unwrap();
        let hovered_img = Image::load(&hover).unwrap();
        assert_eq!(
            d::compare_image(&normal, &hovered_img, 0).unwrap().0["pass"],
            false,
            "runtime hover visual must differ"
        );
        action(session, "press", &["button.default"]);
        action(session, "wait-idle", &[]);
        let pressed = root.join(format!("pressed-{cycle}.png"));
        action(
            session,
            "screenshot",
            &["--id", "button.default", "--out", pressed.to_str().unwrap()],
        );
        let p = Image::load(&pressed).unwrap();
        assert_eq!(
            d::compare_image(&hovered_img, &p, 0).unwrap().0["pass"],
            false,
            "runtime pressed visual must differ"
        );
        action(session, "release", &["button.default"]);
        action(session, "hover", &["button.default"]);
        action(session, "leave", &[]);
        action(session, "wait-idle", &[]);
        let cleared: Snapshot = snap(&action(session, "tree", &[]));
        assert_eq!(
            cleared.index().unwrap()["button.default"].0.hovered,
            false,
            "leave must clear runtime hot"
        );
        // snapshot determinism for unchanged application state
        let snapshot = root.join(format!("snapshot-{cycle}.json"));
        let v = action(
            session,
            "snapshot-layout",
            &["--out", snapshot.to_str().unwrap()],
        );
        assert_eq!(
            v,
            action(session, "snapshot-layout", &[]),
            "snapshot must be deterministic"
        );
        let full = root.join(format!("client-{cycle}.png"));
        let meta = action(session, "screenshot", &["--out", full.to_str().unwrap()]);
        assert_eq!(meta["capture_backend"], "Windows.Graphics.Capture");
        assert_eq!(meta["stable_retained_scene"], true);
        let image = Image::load(&full).unwrap();
        assert_eq!(
            image.width as f64,
            meta["client_px"]["width"].as_f64().unwrap()
        );
        assert_eq!(
            image.height as f64,
            meta["client_px"]["height"].as_f64().unwrap()
        );
        assert!(image.rgba.chunks_exact(4).any(|p| p != &image.rgba[..4]));
        let repeat = root.join(format!("repeat-{cycle}.png"));
        let again = action(session, "screenshot", &["--out", repeat.to_str().unwrap()]);
        assert_eq!(
            (again["width"].clone(), again["height"].clone()),
            (meta["width"].clone(), meta["height"].clone())
        );
        let s = snapshot.to_str().unwrap();
        assert_eq!(
            good(&["compare-layout", "--reference", s, "--snapshot", s])["pass"],
            true
        );
        let mut changed: Snapshot = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(changed.root.rect_dp, changed.client_dp);
        assert_eq!(changed.root.rect_px, changed.client_px);
        fn bump(n: &mut d::Node) -> bool {
            if n.automation_id.as_deref() == Some("button.default") {
                n.rect_dp.x += 1.;
                return true;
            }
            n.children.iter_mut().any(bump)
        }
        assert!(bump(&mut changed.root));
        let mutation = root.join(format!("changed-{cycle}.json"));
        d::write_json(&mutation, &changed).unwrap();
        let (code, result) = cli(&[
            "compare-layout",
            "--reference",
            s,
            "--snapshot",
            mutation.to_str().unwrap(),
            "--id",
            "button.default",
        ]);
        assert_eq!(code, 6);
        assert_eq!(result["result"]["comparisons"][0]["delta"]["x"], 1.);
        let img = full.to_str().unwrap();
        assert_eq!(
            good(&[
                "compare-image",
                "--reference-image",
                img,
                "--native-image",
                img
            ])["mismatched_pixels"],
            0
        );
        let mut pixel = image.clone();
        pixel.rgba[0] = pixel.rgba[0].wrapping_add(1);
        let pixel_file = root.join(format!("pixel-{cycle}.png"));
        pixel.save(&pixel_file).unwrap();
        let (code, result) = cli(&[
            "compare-image",
            "--reference-image",
            img,
            "--native-image",
            pixel_file.to_str().unwrap(),
        ]);
        assert_eq!(code, 6);
        assert_eq!(result["result"]["mismatched_pixels"], 1);
        assert_eq!(
            result["result"]["difference_bbox"],
            json!({"x":0.,"y":0.,"width":1.,"height":1.})
        );
        let crops = root.join(format!("crops-{cycle}"));
        let comparison = good(&[
            "compare-element",
            "--reference",
            s,
            "--snapshot",
            s,
            "--reference-image",
            img,
            "--native-image",
            img,
            "--id",
            "button.default",
            "--out-dir",
            crops.to_str().unwrap(),
        ]);
        assert_eq!(comparison["pass"], true);
        assert!(crops.join("diff.png").exists());
        // Cross-authority: the FROZEN shadcn contract's button.default vs
        // the live Gallery specimen of the SAME semantic id — the real
        // reference-vs-native raster pipeline. The verdict is not pinned
        // (font/native raster deltas are documented); the contract is
        // the schema, artifacts, and normalization model.
        let reference_dir =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("reference/shadcn-gallery-v0.1");
        let cross = root.join(format!("cross-{cycle}"));
        let (code, cmp) = cli(&[
            "compare-element",
            "--reference",
            reference_dir.join("contract.json").to_str().unwrap(),
            "--capture",
            "components/default",
            "--id",
            "button.default",
            "--native-id",
            "button.default",
            "--reference-image",
            reference_dir
                .join("screenshots/light/components.png")
                .to_str()
                .unwrap(),
            "--native-image",
            img,
            "--snapshot",
            s,
            "--out-dir",
            cross.to_str().unwrap(),
        ]);
        assert!(code == 0 || code == 6, "{cmp}");
        let cmp = &cmp["result"];
        assert_eq!(cmp["schema"], "rust-ui.devctl.compare-element/0.2");
        println!(
            "cycle {cycle} frozen-vs-native button.default pass={}",
            cmp["pass"]
        );
        for f in [
            "reference.png",
            "native-raw.png",
            "native-canonical.png",
            "diff.png",
            "comparison.json",
        ] {
            assert!(cross.join(f).exists(), "{f} artifact missing");
        }
        // reference evidence stays on the contract's 1.0 lattice
        assert_eq!(cmp["raster"]["reference"]["dimensions"], json!([70, 32]));
        let v_snap = snap(&v);
        let button = &v_snap.index().unwrap()["button.default"].0;
        let raw_dims: Vec<i64> = cmp["raster"]["native_raw"]["dimensions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d.as_i64().unwrap())
            .collect();
        assert_eq!(
            cmp["raster"]["native_raw"]["scale_px_per_dp"]
                .as_f64()
                .unwrap(),
            scale
        );
        assert_eq!(
            raw_dims,
            vec![button.rect_px.width as i64, button.rect_px.height as i64],
            "native-raw dims must equal the physical rect_px when scale != 1"
        );
        let canonical_w = (button.rect_dp.x + button.rect_dp.width).round() as i64
            - button.rect_dp.x.round() as i64;
        let canonical_h = (button.rect_dp.y + button.rect_dp.height).round() as i64
            - button.rect_dp.y.round() as i64;
        let canonical_dims: Vec<i64> = cmp["raster"]["native_canonical"]["dimensions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d.as_i64().unwrap())
            .collect();
        assert_eq!(
            cmp["raster"]["native_canonical"]["scale_px_per_dp"]
                .as_f64()
                .unwrap(),
            1.0
        );
        assert_eq!(canonical_dims, vec![canonical_w, canonical_h]);
        assert_eq!(
            cmp["normalization"]["source_scale"].as_f64().unwrap(),
            scale
        );
        assert_eq!(cmp["normalization"]["resampler"], "area-box/1");
        if scale != 1.0 {
            assert_eq!(
                cmp["visual_raw"]["computed"], false,
                "cross-scale raw diff is not meaningful"
            );
        }
        let (rw, rh) = (70i64, 32i64);
        let (nw, nh) = (canonical_dims[0], canonical_dims[1]);
        let expected_missing = rw.max(nw) * rh.max(nh) - rw.min(nw) * rh.min(nh);
        assert_eq!(
            cmp["visual_canonical"]["missing_pixels"].as_i64().unwrap(),
            expected_missing,
            "missing pixels must equal union-intersection of canonical dims"
        );
        // real application-state transition: devctl click → Press →
        // Msg::SetPage → retained tree swap — Forms specimens appear
        action(session, "click", &["shell.nav.forms"]);
        action(session, "wait-idle", &[]);
        let forms = action(session, "tree", &[]);
        let forms_snap = snap(&forms);
        let forms_index = forms_snap.index().unwrap();
        for id in [
            "input.default.filled",
            "checkbox.default.unchecked",
            "shell.nav.forms",
        ] {
            assert!(
                forms_index.contains_key(id),
                "{id} must exist after SetPage(Forms)"
            );
        }
        // invoke boundary: pointer-down alone must not toggle — the app
        // model only changes on release; Toggle state is now observable
        // through the snapshot `checked` field
        action(session, "scroll-into-view", &["checkbox.default.unchecked"]);
        let before = snap(&action(session, "tree", &[]));
        assert_eq!(
            before.index().unwrap()["checkbox.default.unchecked"]
                .0
                .checked,
            Some(false)
        );
        action(session, "press", &["checkbox.default.unchecked"]);
        action(session, "wait-idle", &[]);
        let mid = snap(&action(session, "tree", &[]));
        assert_eq!(
            mid.index().unwrap()["checkbox.default.unchecked"].0.checked,
            Some(false),
            "pointer-down alone cannot invoke the toggle"
        );
        action(session, "release", &["checkbox.default.unchecked"]);
        action(session, "wait-idle", &[]);
        let toggled = snap(&action(session, "tree", &[]));
        assert_eq!(
            toggled.index().unwrap()["checkbox.default.unchecked"]
                .0
                .checked,
            Some(true),
            "release must invoke: app model checked=true"
        );
        // keyboard activation — focus + Space toggles it back off
        action(
            session,
            "focus",
            &["checkbox.default.unchecked", "--modality", "keyboard"],
        );
        action(session, "key", &["Space"]);
        let untoggled = snap(&action(session, "tree", &[]));
        assert_eq!(
            untoggled.index().unwrap()["checkbox.default.unchecked"]
                .0
                .checked,
            Some(false),
            "Space on a focused toggle must invoke"
        );
        // native editing proof — real editor focus, raw keys to
        // msftedit, committed TextValue reaches the snapshot label
        action(session, "scroll-into-view", &["input.default.filled"]);
        let seeded = snap(&action(session, "tree", &[]));
        assert_eq!(
            seeded.index().unwrap()["input.default.filled"].0.label,
            "Filled value"
        );
        action(
            session,
            "focus",
            &["input.default.filled", "--modality", "keyboard"],
        );
        action(session, "key", &["End"]);
        action(session, "key", &["Backspace"]);
        let edited: Snapshot = snap(&action(session, "tree", &[]));
        assert_eq!(
            edited.index().unwrap()["input.default.filled"].0.label,
            "Filled valu",
            "basic native key must reach committed application TextValue"
        );
        // resource boundedness — same invariants as before: capture
        // growth is bounded after warmup, idle is silent
        let after_handles = handles(launch["pid"].as_u64().unwrap() as u32);
        println!(
            "cycle {cycle}: scale={scale}, target handles before={before_handles} after={after_handles}"
        );
        let steady = after_handles;
        for i in 0..6 {
            action(session, "screenshot", &["--out", repeat.to_str().unwrap()]);
            let observed = handles(launch["pid"].as_u64().unwrap() as u32);
            println!("cycle {cycle} steady capture {i}: handles={observed}");
            assert!(
                observed <= steady + 2,
                "capture-dependent handle growth after warmup"
            );
        }
        let current = d::client::session(&file).unwrap();
        let before_idle = d::client::request(&current, "handshake", Value::Null, 5000).unwrap();
        assert!(
            before_idle["counters"]["native_timer_fires"]
                .as_u64()
                .is_some()
        );
        let cpu_before = cpu_ticks(current.pid);
        std::thread::sleep(std::time::Duration::from_secs(2));
        let idle_cpu = cpu_ticks(current.pid) - cpu_before;
        println!("cycle {cycle} idle 2s CPU delta (100ns ticks)={idle_cpu}");
        let after_idle = d::client::request(&current, "handshake", Value::Null, 5000).unwrap();
        println!(
            "cycle {cycle} 2s idle counters: before={} after={}",
            before_idle["counters"], after_idle["counters"]
        );
        assert_eq!(
            before_idle["counters"], after_idle["counters"],
            "idle introduced timer/redraw activity"
        );
        let retained = d::client::session(&file).unwrap();
        action(session, "shutdown", &[]);
        assert!(!file.exists());
        d::write_json(&file, &retained).unwrap();
        assert_eq!(
            good(&["cleanup", "--session", session])["stale_session_removed"],
            true
        );
        assert!(!file.exists());
        drop(owner);
    }
    println!(
        "native devctl workflow PASS; 3 complete launch/capture/shutdown cycles on examples/gallery; outputs: {}",
        root.display()
    );
    // Evidence can be retained explicitly; default test cleans all temporary state.
    if let Ok(out) = std::env::var("RUST_UI_DEVCTL_PROOF_OUT") {
        let out = PathBuf::from(out);
        std::fs::create_dir_all(&out).unwrap();
        for entry in std::fs::read_dir(&root).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_file()
                && entry.file_name().to_string_lossy().contains("-0.")
            {
                std::fs::copy(entry.path(), out.join(entry.file_name())).unwrap();
            }
        }
        let dir = out.join("element");
        std::fs::create_dir_all(&dir).unwrap();
        for entry in std::fs::read_dir(root.join("crops-0")).unwrap() {
            let entry = entry.unwrap();
            if entry.path().extension().is_some_and(|s| s == "png") {
                std::fs::copy(entry.path(), dir.join(entry.file_name())).unwrap();
            }
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}
