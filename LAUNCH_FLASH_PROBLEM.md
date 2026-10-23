# Problem brief — intermittent white flash at Gallery launch (Win32 + Direct2D)

Handoff document for a stronger model. Repo: `github.com:rceman/rust-ui`, checkout
`/home/therceman/git/rust-ui` (WSL) — native Windows build via `cargo.exe`.

## Symptom

User reports a brief **white** flash when opening the Gallery — currently ~1/5 of
launches. Three fix attempts landed; each reduced but did not eliminate it.

## Environment

- Windows 11, dark OS theme (user is in dark mode).
- Gallery = `examples/gallery.rs` compiled as `src/bin/rust-ui-gallery.rs`, default
  **dark** theme, Direct2D/DirectWrite renderer, single `WS_OVERLAPPEDWINDOW` HWND.
- Launch path: Desktop shortcut → `rust-ui-gallery-launcher.exe` (GUI-subsystem
  Rust launcher, `W:\devin_folder\rust-ui-gallery-target\debug\`) → stamp fast-path
  or headless cargo build → spawns `rust-ui-gallery.exe` with `CREATE_NO_WINDOW`.
  Console/terminal flashes are already ruled out (launcher is an exe, all children
  are `CREATE_NO_WINDOW`).
- Build: `cargo.exe build --locked --features devtools --bin rust-ui-gallery` with
  `CARGO_TARGET_DIR=W:\devin_folder\rust-ui-slice3-target` (sources via
  `\\wsl.localhost\Ubuntu\...` UNC).

## What was already tried (all on `main`)

| Commit | Change | Result |
|---|---|---|
| `7bd9017` | `hbrBackground = CreateSolidBrush(surface-matched)` at `WNDCLASSW` reg (`window.rs:851`) — dark `0x18181B` / light `0xFAFAFA` via `os_appearance()` | flash persists |
| `7a555b7` | Deferred `ShowWindow`: HWND created hidden (no `WS_VISIBLE`), `run()` calls `ShowWindow(SW_SHOW)` only **after** the first `guarded_turn()` paints+presents (`mod.rs:3931-3942`); `window.rs:882` no longer shows | flash persists, still intermittent |
| `1be5332` | `DWMWA_TRANSITIONS_FORCEDISABLED` (attr 3) + `DwmFlush()` after `ShowWindow` at the reveal (`mod.rs:3941-3954`) | user still reports flash (~1/5) |

## Relevant architecture (exact code)

### Reveal sequence — `src/platform/win32/mod.rs` `run()` ~3929-3954

```rust
if let Err(e) = backend.guarded_turn() {   // initial view→layout→paint→present
    return bail(&mut backend, e);
}
unsafe {
    let transitions_off: BOOL = BOOL(1);
    let _ = DwmSetWindowAttribute(hwnd, DWMWINDOWATTRIBUTE(3), ...);  // transitions off
    let _ = ShowWindow(hwnd, SW_SHOW);                              // reveal
    let _ = DwmFlush();
}
```

### Paint dispatch — `src/platform/win32/window.rs`

`dispatch_owned` normal path (~501): `WM_PAINT` → `ValidateRect` then runs a real
`be.paint()`. The **reentrant** arm (~421) discards via `ValidateRect` unless
`size_pending || stale || sizing_session`.

`paint_inner_impl` (mod.rs ~1989) paints unconditionally once reached; `paint()`
→ `renderer.draw()` → D2D draw + `EndDraw` + swapchain present.

### Non-client frame

`sync_dark_caption` (mod.rs ~898-923): `DWMWA_USE_IMMERSIVE_DARK_MODE` +
`DWMWA_BORDER_COLOR` (attr 34, surface-matched). Called inside `turn()` ~996 —
runs inside the initial `guarded_turn`, i.e. while the window is still **hidden**.

## Leading hypothesis (unverified)

The first frame is **presented while the HWND is hidden** — DWM may discard a
frame for an occluded/invisible window, so at `ShowWindow` the swapchain has no
composited content; the client shows unfilled (white) for ~1 frame until the
post-reveal `WM_PAINT` lands. Intermittency = scheduling race between the
discard and the reveal paint. Supporting logic:

- Class-brush erase now shows **dark** surface, yet the flash is still **white**
  → the flash is probably not `WM_ERASEBKGND`/class brush.
- If this holds, the fix is: keep the reveal hidden OR after `ShowWindow` force
  a synchronous real paint (`UpdateWindow` → `WM_PAINT` → normal dispatch paints)
  then `DwmFlush`. `UpdateWindow` was **not** tried yet — the only post-show step
  today is `DwmFlush`.

## Alternative suspects (unverified)

1. **Non-client frame flashes light** — `sync_dark_caption` applies attrs to a
   hidden window; DWM may compose the first visible frame with default attrs
   before honoring them. Test: is the white region the **caption/borders** or
   the **client**? (Ask user / frame-capture.) If caption: re-assert attrs
   right after `ShowWindow`.
2. **Present-while-hidden discarded** (above) — then the reveal-time paint is
   the first real frame; ensure it runs synchronously (`UpdateWindow`) before
   returning to the loop.
3. **Async present** — DXGI flip present queued; first scan-out still empty.
   May need present-after-visible or a second present post-show.
4. **Another window** — a second HWND (devtools pipe host? popup? richedit host
   window?) briefly created/shown white. Check `CreateWindowExW` callsites.
5. **Launcher-side flash** — believed ruled out (GUI exe, `CREATE_NO_WINDOW`
   children) but confirm by launching `rust-ui-gallery.exe` directly and
   comparing flash rate.

## Suggested next steps

1. **Instrument the reveal** (env-gated like `RUI_RESIZE_TRACE` — see
   `size_seq`/`resize_trace_path` fields): log timestamps of `guarded_turn`
   end, `ShowWindow`, first post-show `WM_PAINT` dispatch (sequence + whether it
   painted or validate-discarded), `Present` return codes,
   `IsWindowVisible(hwnd)` at paint time. This directly tests hypothesis #2.
2. **Cheap candidate fix** to try first: after `ShowWindow` add
   `UpdateWindow(hwnd)` (synchronous `WM_PAINT` → real paint on a now-visible
   window) then `DwmFlush`. Possibly also re-assert `sync_dark_caption` after
   the show for suspect #1.
3. **Capture the flash**: `rust-ui-devctl` WGC screenshot in a tight loop during
   launch, or `GetClientRect`/`PrintWindow` probes, to identify whether the
   white region is client or non-client.
4. If still flashing after a verified synchronous post-show paint, the
   remaining lever is `DXGI` present flags / disabling window frame during the
   first N ms, or `SW_SHOWNORMAL`/`SW_SHOWNA` variants.

## Caveats / constraints

- `create()` is generic over `<S,M,U,V>`; `run()` is in `mod.rs` (backend), the
  WndProc/`trampoline`/`reentrant` live in `window.rs` (`wndproc` mod).
- `WM_ERASEBKGND` returns `LRESULT(1)` (custom erase suppressed) — symmetric in
  both dispatch and reentrant arms.
- Linux host has no display — all real verification must run on Windows
  (`powershell.exe` one-liners from WSL, or the devctl harness).
- Build stamps: `W:\devin_folder\rust-ui-gallery-target\.rust-ui-gallery-build`
  (line-based head/profile/bin_mtime/bin_size). HEAD changes force a rebuild on
  the user's next shortcut launch — they always see the latest `main`.
