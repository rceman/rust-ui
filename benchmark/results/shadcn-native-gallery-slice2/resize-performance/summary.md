# Window-resize freeze — investigation, root cause, fix

User-observed defect: while interactively resizing the native window the
application sometimes freezes/stalls. Measured on the REAL native
`examples/gallery` via devctl `resize` (SetWindowPos → the same synchronous
WM_SIZE path a drag step runs) — 124-step diagonal grow/shrink workload with
alternating-direction noise and small↔large jumps, Components page, Light
then Dark, scale 1.25.

## Instrumentation (added, kept, pull-based)

`RESIZE` static (`src/platform/win32/mod.rs`): atomic counters + a capped
8192-entry sample ring — never logs. Retrieved via devctl `perf`
(and reset via `perf-reset`). Every WM_SIZE/WM_PAINT delivery records a
`ResizeSample {kind, wm_size_us, resize_us, layout_us, uia_us, paint_us,
text_layouts, shadow_rasters}`.

## Root cause (measured, baseline.json)

Per WM_SIZE step the WndProc ran synchronously: `renderer.resize()` →
`relayout()` → `uia_refresh()` → `paint()`. With modal-loop delivery per
mouse-move this produced:

- **WM_SIZE handler: p50 29.3ms / p95 35.8ms / max 40.5ms**
- layout p50 15.2ms — dominated by **356 fresh `IDWriteTextLayout`
  creations per step** (47,824 total over 124 steps). `layout_for` had no
  cache; every Label/Button measure created a new COM layout.
- paint p50 13.8ms — the same measure churn again, plus…
- **216 paints per 124 steps (1.74×)** — the synchronous paint inside
  WM_SIZE defeated the OS's own WM_PAINT coalescing.
- UIA ≈ 0 (no client), render-target `Resize` ≈ 0.3ms, shadow rasters 0
  (card rects don't change size on window resize).

The concatenation — 29ms of synchronous work per mouse-move step plus
follow-up WM_PAINT deliveries — is the freeze.

## Fix (all three measured causes)

1. **Text-layout cache** (`render.rs`): bounded FIFO (512) `IDWriteTextLayout`
   cache keyed by `(text, style_sig{size,weight,family,line_height,
   underline,letter_spacing}, max_w, max_h)` — foreground excluded
   (paint-only). DIP/scale-free. Shared by measure and paint; COM clones.
   → text-layout creates/step: **356 → 0**.
2. **OS-coalesced paint**: WM_SIZE now does the cheap target `Resize` +
   `size_pending` + `InvalidateRect`. Layout/UIA/paint run inside the
   delivered WM_PAINT — the OS batches during the modal loop, so work runs
   once per **delivered frame**, never per mouse step. `WM_ERASEBKGND`
   returns 1 (no GDI flicker). Final frame guaranteed: trailing
   InvalidateRect → trailing WM_PAINT.
   → paints/step: **1.74× → 1.00×**, WM_SIZE handler: **29.3ms → 0.28ms**.

## Results

| metric | before | after (light) | after (dark) |
|---|---|---|---|
| WM_SIZE handler p50/p95/max | 29.3/35.8/40.5 ms | 0.28/0.37/0.53 ms | 0.29/0.37/0.44 ms |
| frame work p50/p95/p99/max | 29.3/35.8/38.1/40.5 ms | 16.3/22.7/25.5/27.2 ms | 16.8/22.3/25.4/27.7 ms |
| layout p50/max per frame | 15.2/18.9 ms | 0.95/1.6 ms | 0.95/1.8 ms |
| paint p50/max per frame | 13.8/24.5 ms | 15.3/26.3 ms | 15.8/26.8 ms |
| text-layout creates/frame | 356 | 0 | 0 |
| paints per resize step | 1.74× | 1.00× | 1.00× |

Acceptance: **no multi-hundred-ms or multi-second app-attributable stall**
(worst frame 27.2ms); **p95 app work/frame 22.7ms ≤ ~33ms target**. The
remaining ~15ms/frame paint is full-window software D2D rasterization — the
inherent floor, not the defect; bounded and smooth.

## Environment

Recorded in `environment.txt` — Windows host via WSL2 shell, DISPLAY2 at
1.25 scale, gallery.exe debug build, devctl `resize` driver.
