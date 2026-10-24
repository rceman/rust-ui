# Pre-P12 Structural Refactor — Argon Independent Delta Review

- **Project:** `rceman/rust-ui`
- **Review Base:** `9a88af28f873d41e98f6f33c423d97f750b60ff3`
- **Corrected Candidate Reviewed:** `92821c7962e4d95d9fad02b2f7e79a8446a96372`
- **Verified Code SHA:** `bd4d8c5a47e4b3fafbbb1876cd83d0bd8e248bbb`
- **Review Branch:** `review/pre-p12-structural-refactor-delta-argon`
- **Final Verdict:** `STRUCTURAL_REFACTOR_EVIDENCE_REWORK_REQUIRED`

---

## 1. Structural Corrections Verification (`F1`–`F3`, Public API & Test Identity) — **PASS**

Inspected full diff `19bd3c998830e8e426f7c1d434444b956a03e6eb..92821c7962e4d95d9fad02b2f7e79a8446a96372` (`dd25b48`, `4f745c0`, `86feffe`, `bd4d8c5`, `92821c7`):

1. **`F1` (`devctl_client.rs` token overflow & formatting) — CLOSED:**
   - `dd25b48` extracted the `#[cfg(test)] mod tests` body from `src/platform/win32/devctl_client.rs` into `src/platform/win32/devctl_client_tests.rs` (`1,139` tok) via `include!("devctl_client_tests.rs");`.
   - `src/platform/win32/devctl_client.rs` is now `4,078` tokens (`16,312` bytes), and the largest authored file in the entire repository is `src/platform/win32/render/mod.rs` at `4,988` tokens (`19,955` bytes < `20,000` bytes).
   - Historical test paths `devtools::client::tests::*` remain byte-identical (`195/195` on Windows), and `cargo fmt --check` exits `0`.
2. **`F2` (Semantic filenames & whole-type boundaries in `ui/`, `style/`, `text/`, `runtime/`, `examples/probe/`) — CLOSED:**
   - `4f745c0` moved `impl Drop for ButtonBuilder` into `src/ui/builders_label_button.rs` (`3,654` tok) and moved `pub struct ActionBuilder` into `src/ui/builders_action_popup.rs` (`3,581` tok), eliminating the mid-type split across files.
   - All numbered/lettered shards in `src/ui/`, `src/style/`, `src/platform/win32/text/`, `src/runtime/`, and `examples/probe/` were renamed and grouped into cohesive semantic files (`container_props.rs`, `control_props.rs`, `tx_staging.rs`, `ui_containers.rs`, `ui_primitives.rs`, `ui_controls.rs`, `builders_{label_button,text_custom,action_popup,input}.rs`, `scope_builders_{nav,settings}.rs`, `palette_tokens.rs`, `recipes_{button,surface_display,forms_nav_overlays}.rs`, `peer_{format_create,draw_measure,selection_lifecycle}.rs`, `pump_review.rs`, `reconcile_nodes.rs`, `dispatch_conflict.rs`, `peer_sched.rs`, `input_window.rs`, `uia_inspect.rs`, `ime_japanese.rs`), and unused copy-pasted imports in `src/runtime/` were pruned.
3. **`F3` (`tests/devctl_windows` topology) — CLOSED:**
   - `tests/devctl_windows_b.rs` was removed and `native_resize_retained_layout_converges` was moved into `tests/devctl_windows/resize.rs` (`480` tok) included by `tests/devctl_windows.rs`.
   - `cargo.exe test --locked --test devctl_windows --features devtools` runs `2/2` tests, and Linux `cargo check --locked --examples` + `cargo test --locked --lib` (`123/123`) pass cleanly.
4. **Public API & Test Identity — PASS:**
   - `src/lib.rs` and `rust_ui::prelude::*` (11 items) have `0` diff lines against `3344c0cf53800ca8935f6fc4c0a68ccf37be46f4`.
   - Linux (`123/123`) and Windows (`195/195`) unit test lists have `0` diff lines against baseline.

---

## 2. Git Pre-Commit Gate & Token-Checker Audit — **2 SMALL SCRIPT FIXES REQUIRED**

Inspected `.githooks/pre-commit`, `tools/check-file-tokens.sh`, `.devin/skills/token-budget/SKILL.md`, and `AGENTS.md`:

- **Verified good:**
  - `.devin/hooks.v1.json` and `tools/token-budget-gate.sh` are removed.
  - `.githooks/pre-commit` inspects staged index blobs via `git diff-index --cached -z --name-status --diff-filter=AMR` and `git cat-file -s ":$p"`, handles root commits, NUL-delimited filenames, renames (`R*`), and fails closed (`exit 1`) on any Git error or `size >= 20000`.
  - `tools/check-file-tokens.sh` uses `set -o pipefail` and `-size "+19999c"` so a `20,000`-byte (`5,000`-token) file fails.
- **Two minor script defects to fix:**
  1. **`.githooks/pre-commit` line `48` is missing `*/target-*/*`:** `tools/check-file-tokens.sh` excludes `! -path '*/target-*/*'`, but `.githooks/pre-commit` line `48` only excludes `*/target/*`. Add `*/target-*/*|` to `.githooks/pre-commit:48` for exact exclusion parity.
  2. **`tools/check-file-tokens.sh` line `43` prints `OK — every file under 0 tokens` when clean:** In `awk -v max="$max_bytes"`, `lim=int(max/4)` is only set inside the per-row `{ ... }` block (`L38`), which never runs when `find` emits `0` rows, leaving `lim` uninitialized (`0`) in `END`. Move `lim=int(max/4)` to `BEGIN { lim=int(max/4) }` (or compute `int(max/4)` in `END`) so it prints `OK — every file under 5000 tokens`.

---

## 3. 12-State Equivalence Analysis (`before/` vs `after/`) — **CLASSIFICATION `A` (CAPTURE-PROTOCOL MISMATCH; DETERMINISTIC RECAPTURE REQUIRED)**

Independently compared all 12 `before/*.json` vs `after/*.json` layout snapshots and all 12 `before/{light,dark}/*.png` vs `after/{light,dark}/*.png` WGC screenshots pixel-by-pixel.

### 3.1 Root Cause of All Observed Layout & Pixel Differences (`Classification A`)
Zero production runtime, layout, rendering, or input lines changed between `3344c0c` and `bd4d8c5`. Every difference between `before/` and `after/` is caused by **`after/` using a different devctl interaction sequence than `before/`**:

1. **Why all 12 JSONs and all 12 PNGs differ in the header band (`y = 45..74`, `2,529–3,603` mismatched pixels):**
   - In **`before/`**, for every state the capture script clicked `shell.nav.<page>` **first** and clicked `shell.theme.{light,dark}` **last**. Consequently, in all 12 `before/*.json` files:
     - `shell.theme.{light,dark}` has `focused: true, hovered: true` (rendering the 3dp focus ring + hover fill around the Light/Dark theme button at `x = 560..640, y = 45..74`), and
     - `shell.nav.<page>` has `focused: false`.
   - In **`after/`**, the capture script clicked `shell.theme.{light,dark}` **first** and clicked `shell.nav.<page>` **last**. Consequently, in all 12 `after/*.json` files:
     - `shell.nav.<page>` has `focused: true` (rendering the 3dp focus ring around the active page tab at `x = 83..540, y = 45..74`), and
     - `shell.theme.{light,dark}` has `focused: false, hovered: false`.
   - **Crucial pixel proof below the header (`y >= 80`):** Across **10 of the 12 states** (`light` and `dark` for `components`, `forms`, `navigation`, `typography`, and `native-text`), there are **`0` mismatched pixels anywhere below `y = 80`** (`100%` byte-identical page content rasterization).
2. **Why `light/overlays` and `dark/overlays` differ in `select.default.open` (`expanded: true` in `before/` vs `expanded: false` in `after/`, `52,916` / `52,858` px in `y = 177..424`):**
   - In `examples/gallery.rs:247`, Gallery boots with `select_open: true`.
   - When on `Page::Overlays`, `select.default.open.content` is mounted with `dismiss_light: true`. Any subsequent pointer `Down` outside the select popup (such as clicking `shell.nav.overlays` or `shell.theme.{light,dark}` while already on `Page::Overlays`) triggers `Backend::pointer` Radix light-dismiss (`src/platform/win32/input_pointer.rs:176-191`), setting `select_open = false`.
   - In `before/`, `overlays` was captured without a post-mount outside click that dismissed `select_open` (or `select.default.open.trigger` was re-clicked to open before theme selection), whereas in `after/` clicking in the single-session sweep light-dismissed `select.default.open`.
3. **Why `light/forms` (`label.default`) and `light/overlays` (`select.default.open`) carried `part_key = "list"` in `after/`:**
   - In `after/`, `Page::Navigation` (whose first child under `page.scroll/0` is `tabs.default`'s unkeyed `Box` with `ui.part("list", ...)`) was visited **before** `Page::Forms` and `Page::Overlays` in the same Gallery process. When switching pages, `Runtime::update_node` (`src/runtime/reconcile_nodes.rs:163-206`, unchanged from `3344c0c`) reused the first child `Box` slot without overwriting `n.part_key`. (Note: this is pre-existing runtime behavior and must **not** be modified in this refactor; capturing in the same page order / fresh launch as `before/` eliminates the capture-state discrepancy.)

### 3.2 Required Deterministic Recapture (`F4` Completion)
Per Section 3, because the differences are **Classification A** (reproducible capture-sequence mismatches rather than refactor regressions), Lead must:
1. Re-run the `after/` capture using the **exact same navigation/click sequence as `before/`**:
   - Visit pages in the order `components -> forms -> navigation -> typography -> native-text` (or fresh-launch per page) and for each state click `shell.nav.<page>` **first** and `shell.theme.{light,dark}` **last** so `shell.theme.{light,dark}` has `focused: true, hovered: true` and `shell.nav.<page>` has `focused: false`.
   - For `light/overlays` and `dark/overlays`, ensure `select.default.open` is open (`expanded: true`) and `shell.theme.{light,dark}` is the last clicked/focused control (`focused: true, hovered: true`), matching `before/{light,dark}-overlays.json`.
   - *(Alternative if `before/`'s exact script is no longer available: run the single deterministic per-page capture script identically against both `3344c0c` (`before/`) and `bd4d8c5` (`after/`) so all 12 `before/*.json` vs `after/*.json` and `before/*/*.png` vs `after/*/*.png` are captured under identical focus/hover/expanded conditions.)*
2. Update `benchmark/results/pre-p12-structural-refactor/equivalence.json` and `REPORT.md` with the measured 12/12 layout and raster comparison results, explicitly distinguishing measured pixel/geometry equality from any inferred non-regression.

---

## 4. P9 / P10 & Launcher Preservation — **PASS**

- **No runtime changes in `19bd3c9..92821c7`:** Confirmed via `git diff` that `dd25b48..92821c7` did not touch any P9 (`window/wndproc.rs`, `paint_timers.rs`), P10 (`WM_DPICHANGED`, `WM_MOVE`), DWM cloak/reveal (`run.rs`), or launcher (`src/bin/rust-ui-gallery-launcher.rs`) logic.
- **P9 held resize:** Verified `2/2` in `tests/devctl_windows`.
- **P10 cross-monitor:** Structural-only preservation in the single-display environment remains accepted as honesty-compliant (`G19`).

---

## 5. Smallest Exact Correction Set Before Approval

1. **`.githooks/pre-commit` (`L48`):** Add `*/target-*/*|` to the path exclusion `case` pattern so it matches `tools/check-file-tokens.sh`.
2. **`tools/check-file-tokens.sh` (`L37-43`):** Initialize `BEGIN { lim=int(max/4) }` in the `awk` script so the 0-violation message prints `OK — every file under 5000 tokens` instead of `0 tokens`.
3. **Deterministic 12-state `after/` recapture (`benchmark/results/pre-p12-structural-refactor/`):** Recapture `after/` matching `before/`'s exact focus/hover (`shell.theme.{light,dark}` clicked last) and `select.default.open` (`expanded: true`) conditions (or recapture both `before/` at `3344c0c` and `after/` at the final code SHA with the identical deterministic script), then update `equivalence.json` and `REPORT.md`.

---

## Verdict

```text
STRUCTURAL_REFACTOR_EVIDENCE_REWORK_REQUIRED
```
