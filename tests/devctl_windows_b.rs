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
fn native_resize_retained_layout_converges() {
    let exe = Path::new(env!("CARGO_BIN_EXE_rust-ui-gallery")).to_path_buf();
    assert!(exe.exists(), "Gallery launch target must exist");
    let file = std::env::temp_dir().join(format!(
        "rust-ui-resize-{}.json",
        d::client::nonce().unwrap()
    ));
    let _owner = Session { file: file.clone() };
    let session = file.to_str().unwrap();
    good(&[
        "launch",
        "--exe",
        exe.to_str().unwrap(),
        "--session",
        session,
    ]);
    action(session, "wait-idle", &[]);
    // window px, not client — successive steps mimic a drag's coalesced
    // WM_SIZE deliveries; each must re-lay out to the live client
    for (w, h) in [(1400, 1600), (964, 1600), (1500, 1700), (900, 1561)] {
        action(
            session,
            "resize",
            &["--width", &w.to_string(), "--height", &h.to_string()],
        );
        action(session, "wait-idle", &[]);
        let s = snap(&action(session, "snapshot-layout", &[]));
        let client = s.client_dp;
        assert_eq!(
            s.root.rect_dp.width, client.width,
            "root width must track client {w}x{h}"
        );
        assert_eq!(
            s.root.rect_dp.height, client.height,
            "root height must track client {w}x{h}"
        );
        // deepest laid-out edge must reach the live client — a stale
        // narrow layout leaves content ending short of the right edge
        fn right(n: &d::Node) -> f64 {
            let mut m = n.rect_dp.x + n.rect_dp.width;
            for c in &n.children {
                m = m.max(right(c));
            }
            m
        }
        let edge = right(&s.root);
        assert!(
            (edge - client.width).abs() < 2.0,
            "deepest edge {edge} must reach client width {} at {w}x{h}",
            client.width
        );
    }
}
