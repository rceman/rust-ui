//! native_probe — repository-local native Windows harness.
//!
//! Boundary: PowerShell owns orchestration (build/run/PID lifecycle/scenario
//! sequencing/artifacts); THIS tool owns native semantics — physical-pixel
//! geometry, real input, DPI transitions, UIA comparison, and real IME.
//!
//! Every subcommand prints one JSON evidence object to stdout and exits
//! non-zero on a failed invariant. Evidence classes:
//!   - "regression"  — synthetic/sent messages (deterministic)
//!   - "acceptance"  — real OS behavior (SendInput / real IME / real DPI)
//!
//! All coordinates are PHYSICAL PIXELS — this process is Per-Monitor-V2,
//! matching the production composer's awareness model (Gate 16).

#[cfg(windows)]
mod probe {
    include!("probe/probe_a.rs");
    include!("probe/probe_b.rs");
    include!("probe/probe_c.rs");
}
#[cfg(windows)]
/// Complete JSON string escaping — quotes, backslashes, control chars and
/// newlines all escape; used by EVERY string the harness emits.
pub fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(windows)]
fn main() {
    use probe::*;
    set_pmv2();
    // UTF-8 stdout — kana/unicode values must serialize correctly through
    // the PowerShell capture pipe; the OEM codepage mangles them
    unsafe {
        windows::Win32::System::Console::SetConsoleOutputCP(65001);
    }
    let args: Vec<String> = std::env::args().collect();
    let usage = concat!(
        "usage: native_probe <sub> <window> [args...]
",
        "  window = title substring | 0xHWND | decimal HWND
",
        "  subs: geometry uia uia-rect uia-enabled uia-count value invoke
",
        "        click click-post click-named type type-post type-post-hex key-post
",
        "        dpichange ime"
    );
    if args.len() < 3 {
        eprintln!("{usage}");
        std::process::exit(2);
    }
    let hwnd = match resolve_window(&args[2]) {
        Ok(h) => h,
        Err(e) => {
            eprintln!(
                "{{\"error\":{}}}",
                crate::json_str(&format!("window '{}' not found: {e}", args[2]))
            );
            std::process::exit(2);
        }
    };
    let res: std::result::Result<String, windows::core::Error> = match args[1].as_str() {
        "geometry" => geometry(hwnd),
        "uia" => uia_tree(hwnd),
        "click" => {
            let cx: i32 = args[3].parse().unwrap_or(0);
            let cy: i32 = args[4].parse().unwrap_or(0);
            click(hwnd, cx, cy).map(|_| {
                format!(
                    "{{\"kind\":\"click\",\"evidence\":\"acceptance\",\"client_px\":[{cx},{cy}]}}"
                )
            })
        }
        "type" => type_text(hwnd, &args[3]).map(|_| {
            format!(
                "{{\"kind\":\"type\",\"evidence\":\"acceptance\",\"text\":{}}}",
                crate::json_str(&args[3])
            )
        }),
        "click-post" => {
            let cx: i32 = args[3].parse().unwrap_or(0);
            let cy: i32 = args[4].parse().unwrap_or(0);
            click_post(hwnd, cx, cy).map(|_| {
                format!(
                    "{{\"kind\":\"click-post\",\"evidence\":\"regression\",\"client_px\":[{cx},{cy}]}}"
                )
            })
        }
        "type-post" => type_post(hwnd, &args[3]).map(|_| {
            format!(
                "{{\"kind\":\"type-post\",\"evidence\":\"regression\",\"text\":{}}}",
                crate::json_str(&args[3])
            )
        }),
        // unicode payload as HEX-encoded UTF-8 — argv is unsafe for
        // non-ASCII text under Windows PowerShell 5.1 (native argv is
        // ANSI-encoded AND a BOM-less .ps1 is parsed as ANSI); hex is
        // ASCII-safe end to end
        "type-post-hex" => {
            let hexs = args[3].trim();
            let mut bytes = Vec::with_capacity(hexs.len() / 2);
            let mut ok = true;
            for pair in hexs.as_bytes().chunks(2) {
                match std::str::from_utf8(pair)
                    .ok()
                    .and_then(|h| u8::from_str_radix(h, 16).ok())
                {
                    Some(b) => bytes.push(b),
                    None => {
                        ok = false;
                        break;
                    }
                }
            }
            match String::from_utf8(bytes) {
                Ok(text) if ok => type_post(hwnd, &text).map(|_| {
                    "{\"kind\":\"type-post\",\"evidence\":\"regression\",\"via\":\"hex\"}"
                        .to_string()
                }),
                _ => Err(windows::core::Error::new(
                    windows::Win32::Foundation::E_FAIL.into(),
                    "type-post-hex: malformed hex or non-UTF-8 payload",
                )),
            }
        }
        "key-post" => {
            let vk: u32 = args[3]
                .trim_start_matches("0x")
                .parse()
                .or_else(|_| u32::from_str_radix(args[3].trim_start_matches("0x"), 16))
                .unwrap_or(0);
            let ctrl = args.get(4).is_some_and(|a| a == "ctrl" || a == "1");
            let shift = args.get(4).is_some_and(|a| a == "shift" || a == "2")
                || args.get(5).is_some_and(|a| a == "shift");
            key_post(hwnd, vk, ctrl, shift).map(|_| {
                format!("{{\"kind\":\"key-post\",\"evidence\":\"regression\",\"vk\":{vk}}}")
            })
        }
        "foreground" => {
            // raise + TOPMOST so a physical screen capture is guaranteed
            // unobscured by foreign windows (evidence honesty — F12)
            foreground(hwnd);
            let _ = unsafe {
                windows::Win32::UI::WindowsAndMessaging::SetWindowPos(
                    hwnd,
                    Some(windows::Win32::UI::WindowsAndMessaging::HWND_TOPMOST),
                    0,
                    0,
                    0,
                    0,
                    windows::Win32::UI::WindowsAndMessaging::SWP_NOMOVE
                        | windows::Win32::UI::WindowsAndMessaging::SWP_NOSIZE,
                )
            };
            Ok(r#"{"kind":"foreground","ok":true}"#.to_string())
        }
        "uia-rect" => uia_rect(hwnd, &args[3]),
        "uia-rect-prefix" => uia_rect_prefix(hwnd, &args[3]),
        "uia-enabled" => uia_enabled(hwnd, &args[3]),
        "uia-count" => uia_count(hwnd),
        "invoke" => invoke(hwnd, &args[3]),
        "click-named" => click_named(hwnd, &args[3]),
        "value" => uia_value(hwnd, &args[3]).map(|v| {
            format!(
                "{{\"kind\":\"value\",\"evidence\":\"acceptance\",\"name\":{},\"value\":{}}}",
                crate::json_str(&args[3]),
                v.map(|s| crate::json_str(&s)).unwrap_or("null".into())
            )
        }),
        "dpichange" => {
            let dpi: u32 = args[3].parse().unwrap_or(96);
            let pid = unsafe {
                let mut p = 0u32;
                windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(
                    hwnd,
                    Some(&mut p),
                );
                p
            };
            dpi_changed(hwnd, pid, dpi)
                .map(|_| "{\"kind\":\"dpichange\",\"evidence\":\"regression\"}".to_string())
        }
        "ime" => ime_japanese(hwnd, &args[3], &args[4]),
        other => {
            eprintln!("unknown subcommand '{other}'\n{usage}");
            std::process::exit(2);
        }
    };
    match res {
        Ok(json) => {
            println!("{json}");
        }
        Err(e) => {
            eprintln!("{{\"error\":{}}}", crate::json_str(&format!("{e}")));
            std::process::exit(1);
        }
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("native_probe is Windows-only");
    std::process::exit(2);
}
