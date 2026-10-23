//! rust-ui-gallery-launcher — the Desktop-shortcut target for the Gallery.
//!
//! A GUI-subsystem binary: no console is ever created (the terminal flash
//! that powershell.exe -WindowStyle Hidden and wscript-wrapped console
//! children could not avoid). It reproduces the script contract:
//!
//!   fast path  -> stamp matches HEAD on a clean worktree + binary
//!                 identity (mtime+size) -> launch the Gallery instantly
//!   otherwise  -> headless cargo build (CREATE_NO_WINDOW), write the
//!                 stamp on a clean tree, then launch
//!   failure    -> native MessageBox with the cargo error tail; nothing
//!                 launches
//!
//! The authoritative source is /home/therceman/git/rust-ui resolved via
//! `wsl.exe wslpath`; output lives in W:\devin_folder\rust-ui-gallery-target.

#![windows_subsystem = "windows"]

use std::io::Write;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const REPO_WSL: &str = "/home/therceman/git/rust-ui";
const TARGET_DIR: &str = "W:\\devin_folder\\rust-ui-gallery-target";
const PROFILE_ID: &str = "debug-devtools";

fn run(exe: &str, args: &[&str], cwd: &str) -> Option<(i32, String)> {
    let out = Command::new(exe)
        .args(args)
        .current_dir(cwd)
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .ok()?;
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    Some((out.status.code().unwrap_or(-1), text))
}

fn launch(exe: &Path) -> bool {
    Command::new(exe)
        .current_dir(TARGET_DIR)
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .spawn()
        .is_ok()
}

fn fail(msg: &str) -> ! {
    unsafe {
        MessageBoxW(
            None,
            &windows::core::HSTRING::from(msg),
            &windows::core::HSTRING::from("rust-ui Gallery - launch failed"),
            MB_OK | MB_ICONERROR,
        );
    }
    std::process::exit(1);
}

fn stamp_path() -> PathBuf {
    Path::new(TARGET_DIR).join(".rust-ui-gallery-build")
}

/// Line-based stamp: head=<sha>\nprofile=<id>\nbin_mtime=<ticks>\nbin_size=<n>
fn read_stamp() -> std::collections::HashMap<String, String> {
    std::fs::read_to_string(stamp_path())
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.split_once('=').map(|(k, v)| (k.into(), v.into())))
        .collect()
}

fn main() {
    let bin = Path::new(TARGET_DIR).join("debug\\rust-ui-gallery.exe");

    let Some((_, wslpath_out)) = run("wsl.exe", &["wslpath", "-w", REPO_WSL], TARGET_DIR) else {
        fail(&format!("'wsl wslpath -w {REPO_WSL}' failed"));
    };
    let repo_win = wslpath_out.lines().next().unwrap_or("").trim().to_string();
    if repo_win.is_empty() || !Path::new(&repo_win).join("Cargo.toml").exists() {
        fail(&format!(
            "authoritative repo not reachable - wslpath returned '{repo_win}'"
        ));
    }

    let head = run(
        "wsl.exe",
        &["-e", "git", "-C", REPO_WSL, "rev-parse", "HEAD"],
        TARGET_DIR,
    )
    .map(|(_, o)| o.trim().to_string())
    .unwrap_or_default();
    let dirty = run(
        "wsl.exe",
        &["-e", "git", "-C", REPO_WSL, "status", "--porcelain"],
        TARGET_DIR,
    )
    .map(|(_, o)| !o.trim().is_empty())
    .unwrap_or(true);

    // fast path: clean tree + matching HEAD + pinned binary identity
    let s = read_stamp();
    if let Ok(meta) = bin.metadata() {
        let ok = !dirty
            && !head.is_empty()
            && s.get("head").map(|v| v.as_str()) == Some(head.as_str())
            && s.get("profile").map(|v| v.as_str()) == Some(PROFILE_ID)
            && s.get("bin_size").and_then(|v| v.parse::<u64>().ok()) == Some(meta.len());
        // mtime compare keeps sub-second writes honest
        let mt = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        if ok && s.get("bin_mtime").and_then(|v| v.parse::<u64>().ok()) == Some(mt) && launch(&bin)
        {
            return;
        }
    }

    // headless cargo build via --manifest-path (UNC cwd is not allowed)
    let manifest = format!("{repo_win}\\Cargo.toml");
    let args = [
        "build",
        "--locked",
        "--features",
        "devtools",
        "--bin",
        "rust-ui-gallery",
        "--manifest-path",
        &manifest,
    ];
    let mut cmd = Command::new("cargo.exe");
    cmd.args(&args)
        .current_dir(TARGET_DIR)
        .env("CARGO_TARGET_DIR", TARGET_DIR)
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let out = match cmd.output() {
        Ok(o) => o,
        Err(e) => fail(&format!("cargo.exe failed to start: {e}")),
    };
    if !out.status.success() {
        let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
        text.push_str(&String::from_utf8_lossy(&out.stderr));
        let tail: Vec<&str> = text.lines().collect();
        let start = tail.len().saturating_sub(12);
        fail(&format!(
            "cargo build failed (exit {})\n\n{}",
            out.status.code().unwrap_or(-1),
            tail[start..].join("\n")
        ));
    }

    if !dirty && !head.is_empty() && bin.exists() {
        if let Ok(meta) = bin.metadata() {
            let mt = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            let _ = std::fs::File::create(stamp_path()).map(|mut f| {
                let _ = write!(
                    f,
                    "head={head}\nprofile={PROFILE_ID}\nbin_mtime={mt}\nbin_size={}\n",
                    meta.len()
                );
            });
        }
    } else {
        let _ = std::fs::remove_file(stamp_path());
    }

    if !bin.exists() || !launch(&bin) {
        fail(&format!("launch failed - {}", bin.display()));
    }
}
