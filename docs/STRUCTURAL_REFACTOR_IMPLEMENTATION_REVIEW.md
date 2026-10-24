# Pre-P12 Structural Refactor — Independent Argon Implementation Review

- **Prompt ID:** `RUI_ARGON_P3-261024-100000`
- **Repository:** `rceman/rust-ui`
- **Production Base (`BASED_ON`):** `3344c0cf53800ca8935f6fc4c0a68ccf37be46f4`
- **Candidate Branch:** `refactor/pre-p12-structural-decomposition`
- **Candidate HEAD Reviewed (`CANDIDATE_HEAD`):** `19bd3c998830e8e426f7c1d434444b956a03e6eb`
- **Original Argon Plan (`ARGON_PLAN`):** `7922116deb12b31a7b42ebec17c4dbb89deb1f80` (`docs/STRUCTURAL_REFACTOR_PLAN.md`)
- **Review Branch:** `review/pre-p12-structural-refactor-argon`
- **Final Verdict:** `STRUCTURAL_REFACTOR_REWORK_REQUIRED`

---

## 1. Executive Summary

The candidate (`19bd3c998830e8e426f7c1d434444b956a03e6eb`) preserves 100% of runtime behavior, public API (`src/lib.rs` is byte-identical to `3344c0c`), Linux/Windows `--lib` test identity (`123/123` and `195/195` with `0` diff lines), P9/P10 modal resize/move/DPI reentrancy logic, and the `91f90f6` DWM cloak/reveal launch sequence. Its child-module decompositions of `src/platform/win32/`, `src/platform/win32/window/`, `src/platform/win32/layout/`, `src/platform/win32/devtools/`, `src/platform/win32/render/`, `src/node/`, `src/tasks/`, `src/layout/`, `src/devtools/`, `src/platform/win32/uia/`, and `examples/gallery/` are cohesive and well-structured.

However, independent verification uncovered **four concrete blockers (`F1`–`F4`)** that require a bounded rework pass before merge:

1. **`F1` (`BLOCKING` — Token Gate & `cargo fmt` Regression at `19bd3c9`):**
   - Commit `a857353` moved the `#[cfg(test)] mod tests` block (`115` lines) back into `src/platform/win32/devctl_client.rs` to restore `devtools::client::tests::*` paths, pushing `src/platform/win32/devctl_client.rs` to **`5,208` tokens (`20,833` bytes)** — so `./tools/check-file-tokens.sh` **exits `1` (`FAIL`)** at `19bd3c9`.
   - `cargo fmt --check` also **exits `1`** on `src/platform/win32/devctl_server.rs:221` (trailing blank line).
2. **`F2` (`BLOCKING` — Arbitrary `_a/_b/_c` & `1/2/3` Sharding in `src/ui/`, `src/style/`, `src/platform/win32/text/`, `src/runtime/`, and `examples/probe/`):**
   - While `include!` in `src/ui/mod.rs`, `src/style/mod.rs`, `src/platform/win32/text/mod.rs`, and `src/platform/win32/uia/mod.rs` is **accepted as a compilation mechanism** (preserving flat module scope and `#[implement]` COM bindings), the cut-points and filenames in `src/ui/`, `src/style/`, `src/platform/win32/text/`, `src/runtime/`, and `examples/probe/` are opaque token shards (`types_a..c`, `impl_a..c`, `builders..builders3`, `recipes1..3`, `peer_a..c`, `probe_a..c`).
   - Worse, in `src/ui/`, `builders.rs` defines `pub struct ButtonBuilder` + `impl ButtonBuilder` while `builders2.rs` starts with `impl Drop for ButtonBuilder`; and the bottom of `builders2.rs` defines `pub struct ActionBuilder` while `builders3.rs` starts with `impl ActionBuilder`. Splitting a struct and its `impl`/`Drop` across numbered files degrades maintainability.
3. **`F3` (`BLOCKING` — `tests/devctl_windows_b.rs` Breaks `cargo check --tests` and Drops P9 Resize Test from `--test devctl_windows`):**
   - `tests/devctl_windows_b.rs` was created as a separate top-level integration test crate **without `#![cfg(all(windows, feature = "devtools"))]`**, causing `cargo check --locked --tests` on Linux (and Windows without `--features devtools`) to fail with `12` compile errors.
   - Moving `native_resize_retained_layout_converges` into `tests/devctl_windows_b.rs` also duplicated 74 lines of harness helpers and silently dropped the P9 resize test from `cargo.exe test --locked --test devctl_windows --features devtools` (`1/1` instead of `2/2`).
4. **`F4` (`BLOCKING` — Missing Committed `after/` 12-State Equivalence Matrix & Stale `REPORT.md` / `token-after.txt`):**
   - `benchmark/results/pre-p12-structural-refactor/before/` has all 12 baseline `snapshot-layout` JSONs and 12 WGC PNGs (`{light,dark}` × `{components,forms,navigation,overlays,typography,native-text}`), but **no `after/` captures or 12-state comparison artifact exist**.
   - `REPORT.md` retains contradictory intermediate sections (`6 remainders`, `4 remainders`, `0 remainders`) and its footer still points to `R6_HEAD: e076350` / `REPORT_HEAD: e076350` instead of the final candidate SHA.

---

## 2. Full-Diff & Token Policy Assessment (Sections 4–6)

### 2.1 Full Diff (`3344c0cf53800ca8935f6fc4c0a68ccf37be46f4..19bd3c998830e8e426f7c1d434444b956a03e6eb`)
- `213 files changed, 79064 insertions(+), 39485 deletions(-)` (net delta includes committed baseline JSON/PNG artifacts under `benchmark/results/pre-p12-structural-refactor/before/` and `docs/STRUCTURAL_REFACTOR_PLAN.md`).
- **Non-move production code changes:** Restricted strictly to `mod` / `include!` / `#[path]` wiring, `pub(crate)` / `pub(super)` visibility adjustments on cross-file helpers, and one type annotation `let outcome: Result<Value>` in `src/bin/rust-ui-devctl.rs:418`. **Zero runtime, layout, rendering, text, or style logic was altered.**

### 2.2 Token Gate & Policy Files (`tools/check-file-tokens.sh`, `tools/token-budget-gate.sh`, `.devin/skills/token-budget/SKILL.md`, `AGENTS.md`)
- `tools/check-file-tokens.sh` properly excludes `! -path '*/docs/*'`.
- `AGENTS.md` correctly identifies `/home/therceman/git/rust-ui` as the sole authoritative source checkout and documents `W:\devin_folder\` as build/output target only.
- `.devin/skills/token-budget/SKILL.md` and `tools/token-budget-gate.sh` match the durable policy (`< 4,500` target, `< 5,000` ceiling, `docs/**` exempt, no stale one-off split maps).
- **Actual token-gate result on `19bd3c9`:** **FAIL (`exit 1`)** due to `src/platform/win32/devctl_client.rs` (`5,208` tok / `20,833` B). The next 7 largest authored files are between `4,531` and `4,988` tokens (`src/platform/win32/render/mod.rs` `4,988` tok, `src/platform/win32/devtools/parts.rs` `4,985` tok, `tests/devctl_windows/workflow.rs` `4,867` tok, `src/platform/win32/mod.rs` `4,803` tok, `src/style/recipes3.rs` `4,633` tok, `src/platform/win32/construct.rs` `4,612` tok, `src/tests/native_contracts_04.rs` `4,531` tok), and all remaining 174 authored files are `< 4,500` tokens.

---

## 3. Subsystem-by-Subsystem Architectural Assessment (Sections 7–18)

### 3.1 `src/tests/` (Section 7) — **ACCEPTED**
- Uses `include!` inside `src/tests/mod.rs` (and `src/tests/probes.rs`) so all 123 Linux tests and 195 Windows tests keep their exact historical test paths (`0` diff lines in both `test-identity-linux.diff` and `test-identity-windows.diff`, independently re-verified).

### 3.2 Win32 Core (`src/platform/win32/`), P9/P10 & `91f90f6` Cloak/Reveal (Sections 8–9) — **ACCEPTED**
- `struct Backend<S, M, U, V>` remains in `src/platform/win32/mod.rs` (`4,803` tok), with cohesive child modules: `construct.rs`, `peer.rs`, `peer_events.rs`, `peer_native.rs`, `turn.rs`, `paint_timers.rs`, `input_pointer.rs`, `input_key.rs`, `interact.rs`, `uia_bridge.rs`, `syscolor.rs`, and `run.rs`.
- `src/platform/win32/window/` (`mod.rs`, `wndproc.rs`, `wndproc_dispatch.rs`) keeps the entire P9/P10 modal loop (`WM_ENTERSIZEMOVE`/`WM_EXITSIZEMOVE`, live `WM_SIZE`, live in-session `WM_PAINT`, coalesced `WM_MOVE`, live `WM_DPICHANGED`) cohesive inside `wndproc.rs` (`2,799` tok) and `wndproc_dispatch.rs` (`3,369` tok).
- **Verified `91f90f6` preservation:** `DWMWA_CLOAK`, `DWMWA_TRANSITIONS_FORCEDISABLED`, `backend.committed_ink.borrow_mut().clear()`, `ShowWindow(SW_SHOW)` while cloaked, `WM_ERASEBKGND` `committed_ink.is_empty()` `DefWindowProcW` fallback in both `wndproc.rs` and `wndproc_dispatch.rs`, post-show synchronous `backend.paint()`, uncloak, second `backend.paint()`, `ValidateRect`, and `DwmFlush()` are 100% intact.

### 3.3 Renderer (`src/platform/win32/render/`) (Section 10) — **ACCEPTED**
- `struct Renderer` and DirectWrite caches stay in `render/mod.rs` (`4,988` tok), with child modules `brush.rs`, `frame.rs` (plus per-NodeData arm includes `frame_{label,icon,button,toggle,editor,custom}.rs` and `containers_{container,action,popup}.rs`), `guards.rs`, `icons.rs`, `shadow.rs`, and `tooltip.rs`.
- `TxDrawD2D` remains owned exclusively by `win32::text` (`WindowlessPeer::draw`), with zero circular dependency between `render` and `text`.

### 3.4 Native Text (`src/platform/win32/text/`) (Section 11) — **MECHANISM ACCEPTED, RENAME `peer_a..c` REQUIRED (`F2`)**
- Using `include!` inside `src/platform/win32/text/mod.rs` is justified by `#[implement(ITextHost2)]` COM macro scoping on `HostBox`.
- Moreover, the actual method boundaries inside `peer_a.rs`, `peer_b.rs`, and `peer_c.rs` **do** align with the Argon plan's 3 semantic groups (`peer_a.rs` = `PeerSurface` + `WindowlessPeer` struct + `apply_format`/`CHARFORMAT2W`/`PARAFORMAT2` + `create` + `send`; `peer_b.rs` = `text`/`selection_utf16`/`effective_geometry` + `draw` (`TxDrawD2D`) + `natural_size` (`EM_REQUESTRESIZE`) + `apply_bounds`; `peer_c.rs` = TOM selection `set_backward_selection_utf16`/`restore_selection_utf16` + activation + colors + `impl TextPeer` + `Drop`), **but** naming them `peer_a.rs`, `peer_b.rs`, `peer_c.rs` obscures those boundaries for P12.
- **Required correction:** Rename `peer_a.rs` → `peer_format_create.rs`, `peer_b.rs` → `peer_draw_measure.rs`, `peer_c.rs` → `peer_selection_lifecycle.rs`.

### 3.5 `src/ui/` and `src/style/` (Section 12) — **MECHANISM ACCEPTED, BOUNDARY & NAMING FIX REQUIRED (`F2`)**
- Using `include!` in `src/ui/mod.rs` and `src/style/mod.rs` is accepted because it keeps `crate::ui::*` and `crate::style::*` flat without `pub(super)` field sprawl.
- **Defects in `src/ui/`:**
  1. `builders.rs` defines `pub struct ButtonBuilder` + `impl ButtonBuilder`, while `builders2.rs` starts with `impl Drop for ButtonBuilder`.
  2. The bottom of `builders2.rs` defines `pub struct ActionBuilder`, while `builders3.rs` starts with `impl ActionBuilder`.
  3. All 11 included files use opaque names (`types_a..c`, `impl_a..c`, `builders..builders3`, `groups..groups2`).
- **Defects in `src/style/`:**
  1. `recipes1.rs`, `recipes2.rs`, `recipes3.rs` (`4,633` tok) use numbered names and scatter P12 form/nav/overlay recipes across `recipes1.rs` (`dropdown_*`, `settings_nav_*`, plus `BoxProps`, `StyleState`, `role_color`, `resolve_color`), `recipes2.rs` (`button_recipe` + `text_input_chrome_recipe`), `recipes3.rs` (`surface_recipe`, `card_*`, `label_recipe`, `badge_recipe`, `alert_*`, `resolve_toggle_visual`, `input_overlay_style`, `input_editor_chrome`, `field_*`, plus `dp_ok..text_ok`), and the tail of `validate.rs` (`select_content_patch`, `tabs_list_recipe`, `select_label_style`, `select_placeholder_patch`).
- See Section 5 below for the exact mechanical rename and cut-point table.

### 3.6 UIA (`src/platform/win32/uia/`) (Section 13) — **ACCEPTED**
- `mod.rs` + `include!` fragments (`site.rs`, `fragments.rs`, `editor.rs`, `fenced.rs`, `provider.rs`, `root.rs`, `uia_root.rs`) have clear semantic names and preserve COM `#[implement]` bindings, refcounts, and generation fences.

### 3.7 Runtime (`src/runtime/`) (Section 14) — **BOUNDARIES ACCEPTED, RENAME `impl_a..d` & PRUNE UNUSED IMPORTS REQUIRED (`F2`)**
- Uses real child `impl` modules where each file already owns one coherent responsibility (`impl_a` = `pump`/`review`/`commit_children`/`drain_dirty`; `impl_b` = `create_node`/`update_node`/`remove_node`; `impl_c` = `dispatch_event`/`deliver_envelope`/`emit_conflict*`; `impl_d` = peer entrypoints + scheduler + shutdown + test surface).
- Rename `impl_a.rs` → `pump_review.rs`, `impl_b.rs` → `reconcile_nodes.rs`, `impl_c.rs` → `dispatch_conflict.rs`, `impl_d.rs` → `peer_sched.rs`, and remove the ~65 copy-pasted unused imports in `src/runtime/*.rs`.

### 3.8 Devctl Client/Server (`src/platform/win32/devctl_client.rs` & `devctl_server.rs`) (Section 15) — **FIX TOKEN OVERFLOW & `cargo fmt` (`F1`)**
- The `#[path]` split between `devctl_client.rs` (CLI `Session`/`Launched` + `mod capture`) and `devctl_server.rs` (in-process pipe `Server`) is clean, but `devctl_client.rs` is at **`5,208` tokens** because `#[cfg(test)] mod tests` (`L529..644`) was moved back in `a857353`.
- Move `L529..644` into `src/platform/win32/devctl_client_tests.rs` and include it at the bottom of `devctl_client.rs` via `#[cfg(test)] include!("devctl_client_tests.rs");` (reducing `devctl_client.rs` to `~4,060` tok while preserving `devtools::client::tests::*`), and run `cargo fmt` to fix `devctl_server.rs:221`.

### 3.9 Gallery, Native Probe & `devctl_windows` (Sections 16, 19, 20)
- **Gallery (`examples/gallery.rs` + `examples/gallery/gallery_*_page.rs`):** **ACCEPTED.** Single authoritative Gallery implementation preserved, semantic page filenames used, and `CARGO_BIN_EXE_rust-ui-gallery` works.
- **Native Probe (`examples/native_probe.rs` + `examples/probe/probe_{a,b,c}.rs`):** Rename `probe_a.rs` → `input_window.rs`, `probe_b.rs` → `uia_inspect.rs`, `probe_c.rs` → `ime_japanese.rs`.
- **`tests/devctl_windows.rs` + `tests/devctl_windows_b.rs` (`F3`):** Delete `tests/devctl_windows_b.rs`, move `#[test] fn native_resize_retained_layout_converges()` (`L75..128`, `54` lines / `~480` tok) into `tests/devctl_windows/resize.rs`, and add `include!("devctl_windows/resize.rs");` at the bottom of `tests/devctl_windows.rs`.

---

## 4. Evidence Completeness & P9 / P10 / Cloak-Reveal Status (Sections 19–23)

1. **`REPORT.md` & `token-after.txt` (`F4`):** Must be updated so `REPORT.md` has a single authoritative final summary and footer pointing at the actual final candidate SHA, and `token-after.txt` reflects the clean exit-`0` token check.
2. **Missing `after/` 12-state equivalence evidence (`F4`):** Lead must capture the 12 post-refactor states (`{light,dark}` × `{components,forms,navigation,overlays,typography,native-text}`) into `benchmark/results/pre-p12-structural-refactor/after/` (or compare them directly against `before/` and commit the concise machine-readable diff summary in `equivalence.json` showing `0` unexplained layout rect delta and `0` deterministic mismatched pixels).
3. **P9 Held Resize (Section 21):** Structurally preserved and runtime-verified once `native_resize_retained_layout_converges` is restored to `--test devctl_windows` (`2/2`).
4. **P10 Cross-Monitor & Cloak/Reveal Honest Classification (Sections 22–23):** We explicitly choose **Option B** from Section 22: structural verbatim preservation + single-display DPI/move unit/probe tests are accepted for this zero-behavior refactor, provided `equivalence.json` and `REPORT.md` state explicitly that physical multi-monitor drag and multi-launch statistical flash observation were not re-executed in the single-display environment (`G19`).

---

## 5. Exact Rework Checklist for Lead

1. **Fix `F1` (`devctl_client.rs` token overflow + `cargo fmt`):**
   - Move `#[cfg(test)] mod tests { ... }` (`L529..644`) from `src/platform/win32/devctl_client.rs` into `src/platform/win32/devctl_client_tests.rs` and replace with `#[cfg(test)] include!("devctl_client_tests.rs");`.
   - Run `cargo fmt` (fixes `src/platform/win32/devctl_server.rs:221`).
2. **Fix `F3` (`tests/devctl_windows_b.rs`):**
   - Move `#[test] fn native_resize_retained_layout_converges()` (`L75..128` of `tests/devctl_windows_b.rs`) into `tests/devctl_windows/resize.rs`, add `include!("devctl_windows/resize.rs");` to `tests/devctl_windows.rs`, and `git rm tests/devctl_windows_b.rs`.
   - Verify `cargo check --locked --tests` passes on Linux and `cargo.exe test --locked --test devctl_windows --features devtools` runs **2/2** tests.
3. **Fix `F2` (Semantic filenames & whole-type boundaries in `ui/`, `style/`, `text/`, `runtime/`, `examples/probe/`):**
   - **`src/ui/`:** Move `impl Drop for ButtonBuilder` (`L1..88` of `builders2.rs`) to the bottom of `builders.rs`, move `pub struct ActionBuilder` (`L356..382` of `builders2.rs`) to the top of `builders3.rs`, and rename:
     - `types_a.rs` → `container_props.rs`
     - `types_b.rs` → `control_props.rs`
     - `types_c.rs` → `tx_staging.rs`
     - `impl_a.rs` → `ui_containers.rs`
     - `impl_b.rs` → `ui_primitives.rs`
     - `impl_c.rs` → `ui_controls.rs`
     - `builders.rs` → `builders_label_button.rs`
     - `builders2.rs` → `builders_text_custom.rs`
     - `builders3.rs` → `builders_action_popup.rs`
     - `groups.rs` → `scope_builders_nav.rs`
     - `groups2.rs` → `scope_builders_settings.rs`
     - `input.rs` → `builders_input.rs`
   - **`src/style/`:** Group recipes semantically and rename `recipes1..3.rs`:
     - `palette_tokens.rs` (`BoxProps`, `StyleState`, `role_color`, `resolve_color`, `SystemColor`, `system_slot`, `os_enforce_*`)
     - `recipes_button.rs` (`button_recipe`, `state_delta`, `resolve_button`, `selected_overlay`)
     - `recipes_surface_display.rs` (`surface_recipe`, `card_*`, `label_recipe`, `separator_recipe`, `badge_recipe`, `alert_*`, `skeleton_recipe`, `kbd_recipe`)
     - `recipes_forms_nav_overlays.rs` (`text_input_chrome_recipe`, `resolve_text_input_chrome`, `resolve_toggle_visual`, `input_overlay_style`, `input_editor_chrome`, `field_*`, `select_*`, `tabs_list_recipe`, `dropdown_*`, `settings_nav_*`)
     - `validate.rs` (`dp_ok..text_ok` + `box_style_ok..patch_touches_layout`)
   - **`src/platform/win32/text/`:** Rename `peer_a.rs` → `peer_format_create.rs`, `peer_b.rs` → `peer_draw_measure.rs`, `peer_c.rs` → `peer_selection_lifecycle.rs`.
   - **`src/runtime/`:** Rename `impl_a.rs` → `pump_review.rs`, `impl_b.rs` → `reconcile_nodes.rs`, `impl_c.rs` → `dispatch_conflict.rs`, `impl_d.rs` → `peer_sched.rs` (and prune unused copy-pasted imports).
   - **`examples/probe/`:** Rename `probe_a.rs` → `input_window.rs`, `probe_b.rs` → `uia_inspect.rs`, `probe_c.rs` → `ime_japanese.rs`.
4. **Fix `F4` (Evidence & `REPORT.md`):**
   - Capture/compare the 12 `after/` Gallery states (`{light,dark}` × `{components,forms,navigation,overlays,typography,native-text}`) against `before/` and record the 12-state layout/raster diff result in `equivalence.json`.
   - Clean up `REPORT.md` so it has one consistent final summary and footer matching the final commit SHA, and regenerate `token-after.txt` showing `0` violations from `./tools/check-file-tokens.sh`.
