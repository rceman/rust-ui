# P12 — Capture & Runtime Audit

Task: `rust-ui-lead-P12-20261030-130100` — evidence hygiene + runtime audit.
Authoritative checkout: `/home/therceman/git/rust-ui` @ `d72010e` (branch `feat/p12-automated-visual-parity`).

## 1. Single-Input window — confirmed identity

**Source:** `src/tests/probes.rs:80–139` — `native_probe_richedit_paints_text`.

The probe created a Win32 `CreateWindowExW` host with `WS_OVERLAPPEDWINDOW | WS_VISIBLE` (800×500, title `"probe"`) to supply the DPI authority the windowless RichEdit service reads via `TxGetDC`. The window was never a render target — the test renders into an in-memory `CreateDIBSection` framebuffer through `TxDrawD2D`. `WS_VISIBLE` only placed a persistent window on the desktop.

**Fix (committed `d72010e`):** dropped `WS_VISIBLE`. The non-visible host still reports the same `GetDeviceCaps(LOGPIXELSY)` and the DIB paint is unchanged — `native_probe_richedit_paints_text` passes and the full native suite is 198/198. No test semantics weakened: HWND/DPI behavior preserved, only desktop visibility removed.

## 2. GUI launch-path inventory

| Path | Window | Classification | Cleanup |
|---|---|---|---|
| `src/tests/probes.rs` `native_probe_richedit_paints_text` | `WS_OVERLAPPEDWINDOW` (was `|WS_VISIBLE` → now hidden) | RichEdit DIB regression | `DestroyWindow` at scope end |
| `src/bin/rust-ui-gallery.rs` → `examples/gallery.rs` | visible app window (platform `run.rs` `ShowWindow`) | **Visual/interaction acceptance target** | devctl `shutdown` / process exit |
| `src/bin/rust-ui-devctl.rs` | none (console) | orchestration | n/a |
| `src/bin/rust-ui-gallery-launcher.rs` | spawns gallery | launcher shim | child exit |
| `examples/native_probe.rs`, `examples/uia_probe.rs` | tool windows | subsystem probes (run on demand) | process exit |
| `examples/composer.rs` | visible app window | interactive example | process exit |
| `tests/devctl_windows*.rs` | launches real `CARGO_BIN_EXE_rust-ui-gallery` | E2E acceptance (3-cycle) | per-cycle `shutdown` |
| `tools/windows/run-gallery.ps1` | spawns gallery | dev launcher | child exit |

No stale `rust-ui-devctl-test-fixture.exe` is launched by the acceptance path — `devctl_windows` binds `CARGO_BIN_EXE_rust-ui-gallery`. The fixture binary that exists on disk is a headless console target for devctl protocol smoke, not a visual specimen; it is not in the acceptance path and was not revived.

## 3. Screenshot / WGC counts

- **devctl E2E test:** `for cycle in 0..3` — 3 Gallery sessions × ~5 `screenshot` calls (normal/hover/pressed/client/repeat) + `snapshot-layout` + `compare-*` ≈ **15 real captures** for the handle-growth regression. Bounded, intentional.
- **`benchmark/results/p12-automated-visual-parity/README.md` "102 compare-element runs":** these are **offline crop comparisons** against a handful of captured PNGs — not 102 WGC operations. `compare-element` reads an already-captured `--native-image` and crops the element rect.
- **Redundant burst (the user's observation):** an earlier P12 sweep produced **~258 PNGs**, but `RUI_GALLERY_PAGE` never crossed the WSL→Windows interop env boundary — every session booted the default **Components** page, so ~250 of those were duplicate captures of identical state under mislabeled filenames. Root cause: missing `WSLENV` forwarding (fixed in the corrected sweep), plus a `open()` path bug that scattered result debris.

**Duplicate-capture count (this P12 sweep):** ~250 of 258 (only the real components captures were unique state).

## 4. Gallery-only visual acceptance

- Visual parity evidence derives from the real `rust-ui-gallery.exe` (`examples/gallery.rs`), staged per page via `RUI_GALLERY_PAGE` and themed via `shell.theme.{light,dark,system}`.
- No synthetic UI fixture stands in for acceptance; `devctl_windows` exercises the Gallery binary directly.
- env propagation for staging is via `WSLENV` when launched from WSL interop (Windows-side env inheritance is fine — `launch_process` builds the block from `vars_os`).

## 5. Capture policy (established)

Static parity captures once per unique `(build SHA, exe, page, theme, state, viewport, DPI, font env)`; reuse one screenshot for many offline `compare-element` crops. Re-capture only on a relevant build/state/env change, an unstable capture, or an explicit reproducibility check. Bounded retry for transient `CAPTURE_FAILED`; motion uses an explicitly bounded time-sample plan, not static screenshots.

## 6. Components glyph-fringe artifact

Pixel sampling of `components-light.png` (1:1, 964×1561) over the text-dense nav region measures **4.06% high-chroma pixels** — the DirectWrite ClearType subpixel signature (colored edges on glyph boundaries). That fringe class is expected on a real-display WGC capture at 1:1 and is not, by itself, a defect.

The user-reported "dark ghosted/offset contours near the active pill" cannot be verdicted on this capture — it predates the env fix and the bounded-invalidation rework, and a clean 1:1 reference-matched capture is required to distinguish ClearType/capture-scale from a real invalidation/layer-dup defect. A fresh verified capture comes from the corrected sweep; the item stays **OPEN** pending that.

## 7. Changes made

- `d72010e` — drop `WS_VISIBLE` from the RichEdit DIB probe (removes the user-visible window; native 198/198 green).
- `53e7d3e` — `apply_font_tom` lifts `TXTBIT_READONLY` for the TOM font push (read-only/disabled input stories rejected `SetName` `ACCESS_DENIED` → Gallery exited `code=1` on the Forms page). `FORMS` now boots.
- Corrected sweep driver (`/tmp/p12sweep.py`): `WSLENV` env forwarding, bounded `launch` retry for the flaky `SESSION_NOT_FOUND` race, per-tag filenames, stale-code re-read, result writes to `/mnt/w/` paths.

## 8. Results

- Visible-probe root cause: `probes.rs` `WS_VISIBLE` — **removed** (`d72010e`).
- WGC operations: bounded (devctl E2E ≈15; parity captures per unique state).
- Redundant captures: ~250 duplicate-state PNGs traced to the interop env gap — root cause fixed, not tolerated.
- Gallery acceptance: enforced (real `rust-ui-gallery` only).
- Visual artifact: ClearType-class at 4.06% chroma; ghosted-contour item **OPEN** pending fresh capture.
- Tests: native 198/198 devtools, 169/169 default, Linux 126/126 — green.
