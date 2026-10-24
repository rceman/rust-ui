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
include!("devctl_windows/workflow.rs");
include!("devctl_windows/resize.rs");
