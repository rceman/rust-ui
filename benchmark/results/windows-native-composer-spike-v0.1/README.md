# Windows Native Composer Contract Spike v0.1 — evidence

**Candidate:** `0d0e020a788e918734ee1c7bdbb20e0d52e43ae9` on `agent/windows-native-composer-contract-spike-v0.1-swe2` (post-`PLATFORM_FOUNDATION_REVIEW` rework — review artifact `1603547c`).
**Environment:** Windows 11, monitor at 125% scale (PerMonitorV2), native `cargo build --examples` debug build.
**Identity receipt:** `identity.txt` — source SHA + composer.exe SHA256 + native_probe.exe SHA256 + run UTC, written per collection pass.
**Capture method:** `PrintWindow` of the live composer window (window-level raster pipeline under evidence — does NOT exercise a separate presentation path); UIA through `UIAutomationClient`; ALL native semantics through `examples/native_probe.rs` (JSON-emitting Rust harness, non-zero exit on violated invariants). PowerShell (`native.ps1`, `collect.ps1`) is orchestration only — build/run/PID lifecycle/scenario sequencing/artifacts; no coordinate math, UIA interpretation, input packing, or DPI rules remain in `.ps1`.

## Artifacts

- `composer-dark.png` / `composer-light.png` / `theme-back-dark.png` — theme states; `final-foundation.png` — final candidate (verified visually: all rows inside pills, editor text aligned inside its chrome)
- `smoke.json` — PMv2-verified geometry receipt (awareness asserted = `pmv2`, dpi, window/client px)
- `uia-tree.txt` — live UIA tree with physical screen-px bounds (draft, body, three rows, all buttons)
- `typing.png`/`unicode.png`/`selection.png`/`undo.png`/`readonly.png` (+`.txt`) — typed value, unicode, Ctrl+A replace, Ctrl+Z undo chain, staged read-only rejection
- `ime.json`/`ime.png`/`ime.txt` — real MS-IME (TSF) hiragana: `before` snapshot + `gained` committed suffix + `no_submit` assertion — preedit cannot pass the check
- `disabled.txt`/`disabled-mid.png` — send disabled + stop enabled mid-flight via live `IUIAutomationElement::CurrentIsEnabled`
- `multiline.png`/`multiline.txt` — 6 Shift+Enter newlines; body grew 57px → 134px (max_lines bound caps at ~4 visible lines)
- `reorder-before/after/final.txt` + `reorder.png` — reorder/remove/recreate with live tree diffs
- `send-mid.png`/`send-done.png`/`send-stop.txt` — send → mid-flight disabled → stop → resend → done
- `dpi-96/120/144/192.png`+`.txt` — synthetic `WM_DPICHANGED` per scale with READ-BACK assertion (window px == 500×470 Dp × dpi/96, exact at all four)
- `scale.txt`/`scale-100.png`/`scale-1000.png` — 100/1000-row mount + UIA child counts (121/1021) + settle timing
- `perf.txt` + `perf-counters.txt` — 30 s idle: `idle_cpu_pct_30s=0`, 34.6 MiB WS, 18 threads; counters written at CLEAN shutdown (`uptime_ms=30568`, `requested_redraws=81`, `caret_redraws=82`, `native_timer_fires=0`) — proves event-driven idle AND graceful teardown emission
- `identity.txt` — run receipt

## Verification status (this candidate)

- `cargo test --lib` (Windows) — **85/85**
- `cargo test --locked --lib` (WSL2 Ubuntu, real Linux run) — **76/76** portable suite
- `cargo build --examples` — clean; `cargo fmt --check` — clean
- `native_probe` subcommands exercised live: `geometry`, `uia`, `uia-rect`, `uia-enabled`, `uia-count`, `invoke`, `click-named`, `type` (posted), `key-post`, `value`, `dpichange`, `ime`

## What is proven (measured)

- Retained + keyed staged UI tree; windowless RichEdit via `ITextHost2`/`ITextServices2`/`TxDrawD2D`
- **Coordinate contract:** `crate::geom` owns semantic spaces (`ScaleFactor` = platform-neutral px/Dp ratio, `PeerOrigin` single snapped transform, `ClientPhysicalPoint`/`ScreenPhysicalPoint`/`PeerLocalPoint` markers); `platform/win32/space.rs` owns all Win32 conversions (RECT/POINT/LPARAM packing, checked `ClientToScreen`/`ScreenToClient`, the `dpi ↔ ratio` mapping)
- **Native entry:** all WndProc→Backend entries pass one ownership guard; reentrant callbacks enqueue OWNED payloads (`WM_DPICHANGED` copies the RECT; `WM_NCDESTROY` atomically closes + queues full cleanup through the drain); deferred peer delivery is generation-checked with explicit continuation
- `native_probe_richedit_paints_text` — PMv2, ink inside the requested island at 96/120/144/192 with zero stray pixels, plus live scale-change leg; the F04 relatch regression proves text/selection/undo preserved and measurement refreshed across scale change
- Real SendInput keystrokes; mid-text click places caret at the clicked offset
- **Real IME:** committed hiragana extends the prior value (`gained` must contain kana AND preserve `before`); `no_submit` asserted — preedit cannot produce a pass
- Synthetic `WM_DPICHANGED` via `SendMessageW` with read-back size assertion — regression evidence only
- UIA bounds in physical screen px; `IsEnabled=false` for disabled; order/name/focus-change notifications
- Timers: `TxSetTimer` arms synchronously through one shared `TimerPool` (honest FALSE on failure, collision-free IDs, generation-fenced stale WM_TIMER)
- Forced colors: roles map to real `GetSysColor` RGB through the shared `SystemColor` slots; `ReducedMotion::System` re-resolves on `WM_SETTINGCHANGE`
- Theme flip honored live including authored role foregrounds re-resolving on peers
- Idle ~0% CPU, event-driven; clean shutdown emits perf counters (teardown evidence)

## Honest limits

- **Real monitor DPI transition** — NOT RUN; the display stayed at 125%. Synthetic `WM_DPICHANGED` + adapter-unit coverage is recorded; a real monitor/scale move remains a distinct acceptance class.
- **Dead keys** — NOT RUN; en-US has none and no international layout is installed.
- **IME composition across scale relatch** — NOT RUN as a combined scenario; composition state preservation is covered by the committed/selection/undo legs and documented in deviations §3.
- `ElementProviderFromPoint` — unit-tested; real `FromPoint` against the foreground window not exercised.
- Caret visibility in `PrintWindow` captures is unreliable (XOR caret); caret geometry validated via click→insert.
- `requested_redraws`/`caret_redraws` counters are demand-driven repaint counts, not a resource-leak audit; no fake-peer cycle test is presented as native lifetime proof.
