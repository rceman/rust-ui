#![cfg(all(windows, feature = "devtools"))]
//! Real CLI + pipe + retained runtime + WGC acceptance. Build devctl_probe first.
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
fn count(s: &Value) -> String {
    let snapshot: Snapshot = serde_json::from_value(s.clone()).unwrap();
    snapshot.index().unwrap()["probe.label"].0.label.clone()
}
#[test]
fn native_agent_workflow_and_bounded_repetition() {
    let root = std::env::temp_dir().join(format!(
        "rust-ui-devctl-proof-{}",
        d::client::nonce().unwrap()
    ));
    std::fs::create_dir(&root).unwrap();
    let exe = Path::new(env!("CARGO_BIN_EXE_rust-ui-devctl"))
        .parent()
        .unwrap()
        .join("examples/devctl_probe.exe");
    assert!(
        exe.exists(),
        "build --features devtools --example devctl_probe first"
    );
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
        let initial = action(session, "tree", &[]);
        assert_eq!(count(&initial), "Clicks: 0");
        let snap: Snapshot = serde_json::from_value(initial).unwrap();
        assert_eq!(snap.index().unwrap().len(), 5);
        let rect = action(session, "rect", &["probe.button"]);
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
        action(session, "hover", &["probe.input"]);
        let baseline = root.join(format!("normal-{cycle}.png"));
        action(
            session,
            "screenshot",
            &["--id", "probe.button", "--out", baseline.to_str().unwrap()],
        );
        action(session, "hover", &["probe.button"]);
        action(session, "wait-idle", &[]);
        let hover = root.join(format!("hover-{cycle}.png"));
        action(
            session,
            "screenshot",
            &["--id", "probe.button", "--out", hover.to_str().unwrap()],
        );
        let before_handles = handles(launch["pid"].as_u64().unwrap() as u32);
        let normal = Image::load(&baseline).unwrap();
        let hovered = Image::load(&hover).unwrap();
        assert_eq!(
            d::compare_image(&normal, &hovered, 0).unwrap().0["pass"],
            false,
            "hover visual must differ"
        );
        action(session, "press", &["probe.button"]);
        action(session, "wait-idle", &[]);
        let pressed = root.join(format!("pressed-{cycle}.png"));
        action(
            session,
            "screenshot",
            &["--id", "probe.button", "--out", pressed.to_str().unwrap()],
        );
        assert_eq!(
            count(&action(session, "tree", &[])),
            "Clicks: 0",
            "press alone cannot invoke"
        );
        let p = Image::load(&pressed).unwrap();
        assert_eq!(
            d::compare_image(&hovered, &p, 0).unwrap().0["pass"],
            false,
            "pressed visual must differ"
        );
        action(session, "release", &["probe.button"]);
        assert_eq!(count(&action(session, "tree", &[])), "Clicks: 1");
        action(session, "click", &["probe.button"]);
        assert_eq!(count(&action(session, "tree", &[])), "Clicks: 2");
        action(
            session,
            "focus",
            &["probe.button", "--modality", "keyboard"],
        );
        action(session, "key", &["Enter"]);
        assert_eq!(count(&action(session, "tree", &[])), "Clicks: 3");
        action(session, "focus", &["probe.input"]);
        action(session, "key", &["End"]);
        action(session, "key", &["Backspace"]);
        let edited: Snapshot = serde_json::from_value(action(session, "tree", &[])).unwrap();
        assert_eq!(
            edited.index().unwrap()["probe.input"].0.label,
            "Calibration tex",
            "basic native key must reach committed application TextValue"
        );
        action(session, "focus", &["probe.button"]);
        action(session, "wait-idle", &[]);
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
            if n.automation_id.as_deref() == Some("probe.button") {
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
            "probe.button",
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
            "probe.button",
            "--out-dir",
            crops.to_str().unwrap(),
        ]);
        assert_eq!(comparison["pass"], true);
        assert!(crops.join("diff.png").exists());
        let after_handles = handles(launch["pid"].as_u64().unwrap() as u32);
        println!(
            "cycle {cycle}: scale={scale}, target handles before={before_handles} after={after_handles}"
        );
        // Focus/editing warms native resources. Measure capture growth only
        // after that warmup, with unchanged state and six fresh CLI captures.
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
        "native devctl workflow PASS; 3 complete launch/capture/shutdown cycles; outputs: {}",
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
