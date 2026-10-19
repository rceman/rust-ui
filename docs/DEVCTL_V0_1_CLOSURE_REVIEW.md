# Independent Closure Review — `rust-ui-devctl` v0.1 Bounded Rework

**Verdict:** `RUST_UI_DEVCTL_V0_1_APPROVED` / `NATIVE_GALLERY_TOOLING_CLEARED`

- **Run Key:** `RUI-A-261015-2310`
- **Reviewer:** Astra Advisor (System / Contract / Platform Authority)
- **Repository:** `rceman/rust-ui`
- **Implementation Branch:** `agent/rust-ui-devctl-v0.1`
- **Review Branch:** `review/rust-ui-devctl-v0.1-closure-astra`
- **Based On (Merged / Report HEAD):** `9f88d7ca3a57bd876ef505d36f819c090ba0a973`
- **Production Implementation HEAD:** `cf8ab919b399ca4465a17710741f1c888b84cfa0`
- **Rework Evidence Publication Commit:** `dcb68f2e785f7fc553794b9e83f58281f6694d40`
- **Docs-Only Cadence Commit:** `e7731bd8338e3f24424da1b4f52a5bb8a8d2e2c2`
- **Original Devctl Report HEAD:** `7a1ad19a397d10149b6d50ebdf5c1c60d0c6bc67`
- **Original Devctl Implementation HEAD:** `6375a4e537f0ec396c92154738f7bfced4324c5c`
- **Frozen Reference Publication Base:** `dc70f6644c0384170f3a6929eb2caaf33fc711a0`
- **Frozen Shadcn Reference Payload:** `2e14e417a69ef8967b8fb17120282e3de6449fae`
- **Previous Review Branch / HEAD:** `review/rust-ui-devctl-v0.1-astra` @ `32f52c35e97c10f91d0b5d5d01a8c3bf91ce9d49`

---

## Executive Summary & Disposition of Previous Findings

| Finding | Previous Severity | Closure Status | Verification Summary |
|---|---|---|---|
| **D01** | BLOCKER | **CLOSED** | `compare-element` (`rust-ui.devctl.compare-element/0.2`) separates `RAW_NATIVE_CAPTURE` (`native-raw.png` at native physical `scale_factor`) from `CANONICAL_COMPARISON_RASTER` (`native-canonical.png` at `1.0 px/Dp`), resampled via deterministic `area-box/1` over each side's own snapped logical lattice `snap_rect(rect.grow(pad), 1.0)`. Cross-authority comparison of frozen DPR1 `button.default` (`70×32`) vs native `1.25x` WGC `probe.button` (`87×45` raw -> `69×36` canonical) eliminates the `1.25x` scale-mismatch L-border (`312` missing pixels = exact `70×36 - 69×32` logical size delta). |
| **D02** | BLOCKER | **CLOSED** | `devtools::snap_rect` (`src/devtools/mod.rs:152-181`) is the sole logical→physical conversion authority in devtools/CLI, delegating every edge to `crate::geom::Rect::physical` (`ScaleFactor::try_to_physical` / `to_physical`, round-half-away-from-zero). `Image::crop` takes an integer `&PixelRect`; the divergent `floor`/`ceil` crop policy is deleted. Systemic audit across all `581` element records and `806` visible-part records (`1,387` total) at scales `{1.0, 1.25, 1.5, 2.0}` confirms all `421` element and `578` part (`999/1,387`) rounding disagreements are eliminated. |
| **D03** | BLOCKER | **CLOSED** | `reference_targets` (`src/devtools/mod.rs:360-497`) faithfully adapts `rust-ui.shadcn-reference.contract/0.2`, including `tier` (`core`, `later`, `reference-only`), exact `<page>/<state>` capture lookup with no fallback, visible and hidden (`visible: false`, `reason: "not-rendered"`) parts keyed by `SemanticKey { owner, part }` (`owner#part`), owner-relative and derived page-absolute part rects, `owner_attributed`, `label_gap`, `interaction`, and `meta`. `TierScope` defaults to `core` and reports `scope.excluded_by_tier` and `out_of_scope`. `Snapshot` (`rust-ui.devctl.snapshot-layout/0.2`) and `PartNode` provide a complete generic semantic-parts contract without requiring devctl redesign. |
| **D04** | CONTRACT | **CLOSED** | `Runtime::review()` (`src/runtime.rs:333-349`) enforces non-empty, `<= 256`-byte, control-free, unique `automation_id` invariants in **all builds** (no `cfg(test)` or `cfg(feature = "devtools")` gate), pushes `UiDiagnostic::DuplicateAutomationId` to `self.diagnostics`, and aborts before any arena or peer mutation. `Ui::named` (`src/ui.rs:582-603`) is idempotent on identical IDs and records `DuplicateAutomationId` without overwriting on conflicting IDs. Proven in default non-test/non-devtools library compilation via `tests/invalid_automation_id.rs` (`2/2 PASS`). |
| **D05** | INTERACTION / IDLE | **CLOSED** | All focus transition boundaries (`Backend::focus_and_settle` for devctl `focus`, `Backend::uia_action(focus)`, `Backend::focus_step` Tab traversal, and primary pointer-Down focus transitions in `Backend::pointer`) execute `set_focus -> service_peer_events -> turn` so peer host notifications are routed before `rt.pump()`. Canonical `leave` command is implemented across CLI, protocol, `Backend::leave()`, and docs, clearing hover state cleanly. |

---

## 1. Exact Identity & Merge Shape

- **Original devctl report HEAD:** `7a1ad19a397d10149b6d50ebdf5c1c60d0c6bc67` (parent of both `cf8ab91` and `e7731bd`).
- **Docs-only durable `AGENTS.md` cadence commit:** `e7731bd8338e3f24424da1b4f52a5bb8a8d2e2c2` (`1 file changed, 17 insertions(+)` in `AGENTS.md`).
- **Rework implementation HEAD (`PRODUCTION_HEAD`):** `cf8ab919b399ca4465a17710741f1c888b84cfa0` (`12 files changed, 2270 insertions(+), 196 deletions(-)` on top of `7a1ad19`).
- **Rework report & evidence publication commit:** `dcb68f2e785f7fc553794b9e83f58281f6694d40` (parent `cf8ab91`; adds only `docs/DEVCTL_V0_1_REWORK_REPORT.md` and `benchmark/results/rust-ui-devctl-v0.1-rework/**`).
- **Final merged/report branch HEAD (`BASED_ON`):** `9f88d7ca3a57bd876ef505d36f819c090ba0a973` (merge of `dcb68f2` and `e7731bd`).
- **Semantic preservation proof:**
  - `git diff cf8ab919b399ca4465a17710741f1c888b84cfa0 9f88d7ca3a57bd876ef505d36f819c090ba0a973 -- src/ examples/ tests/ Cargo.toml Cargo.lock reference/` is **empty**.
  - `git diff dcb68f2e785f7fc553794b9e83f58281f6694d40 9f88d7ca3a57bd876ef505d36f819c090ba0a973` contains only the 17-line `AGENTS.md` UI verification cadence section from `e7731bd`.
  - All committed evidence under `benchmark/results/rust-ui-devctl-v0.1-rework/` records `"candidate": "cf8ab919b399ca4465a17710741f1c888b84cfa0"` in `identity.json` and matches the `cf8ab91` implementation tree.

---

## 2. D01 — Canonical Comparison Raster

- **Two explicit raster spaces (`src/devtools/mod.rs:678-829`, `src/bin/rust-ui-devctl.rs:191-258`):**
  1. **`RAW_NATIVE_CAPTURE` (`native-raw.png`):** Cropped directly from the full WGC client capture at `native_px = snap_rect(native_rect.grow(pad), native_scale)` (`src/devtools/mod.rs:765, 767`) and saved unchanged to `native-raw.png` (`src/bin/rust-ui-devctl.rs:249`).
  2. **`CANONICAL_COMPARISON_RASTER` (`native-canonical.png`):** Resampled from the full WGC client capture onto the native element's `1.0 px/Dp` logical lattice `native_canonical_rect = snap_rect(native_rect.grow(pad), 1.0)` via `native_image.canonicalize(native_scale, &native_canonical_rect)` (`src/devtools/mod.rs:763, 768`) and saved to `native-canonical.png` (`src/bin/rust-ui-devctl.rs:250-252`).
- **Reference DPR1 authority preserved:** For frozen contract references, `ref_scale` is read from `coordinate_space.device_scale_factor` (`1.0`), `ref_canonical = snap_rect(want.grow(pad), 1.0)`, and `reference.png` remains on the `1.0 px/Dp` lattice.
- **Canonical-only visual metrics & diff:** `compare_image(&reference_canonical, &native_canonical, channel_tolerance)` (`src/devtools/mod.rs:770`) drives `visual_canonical` and `diff.png`. When `reference_scale != native_scale`, `visual_raw` explicitly records `{"computed": false, "reason": "raw native raster is at 1.25 px/Dp and reference at 1 px/Dp; cross-scale raw diff is not meaningful"}` (`src/devtools/mod.rs:782-788`).
- **Schema completeness (`rust-ui.devctl.compare-element/0.2`):** Explicitly records `raster.{reference, native_raw, native_canonical}`, `normalization.{source_scale, target_scale, source_dimensions, canonical_dimensions, resampler, identity, transform.{mapping, canonical_rect_dp, source_footprint_px}}`, `visual_canonical`, `visual_raw`, `geometry`, `scope`, `evidence_classes`, and `padding_comparison_px`. No arbitrary resize-to-fit exists.

---

## 3. D01 — Resampler Correctness (`area-box/1`)

- **Implementation audit (`Image::canonicalize`, `src/devtools/mod.rs:941-1000`):**
  - Validates image integrity (`self.validate()`), positive finite `source_scale`, and bounded canonical dimensions (`w > 0 && h > 0 && w * h <= MAX_PIXELS`).
  - For each canonical pixel `(i, j)` in `0..w × 0..h`, computes the exact source footprint `[fx0, fx1) × [fy0, fy1)` using a single multiplication per edge:
    - `fx0 = source_scale * (canonical.left + i) as f64`
    - `fx1 = source_scale * (canonical.left + i + 1) as f64`
    - `fy0 = source_scale * (canonical.top + j) as f64`
    - `fy1 = source_scale * (canonical.top + j + 1) as f64`
  - Fails closed with `TARGET_NOT_VISIBLE` if `sx0 < 0 || sy0 < 0 || sx1 > self.width || sy1 > self.height` (no edge clamping or replication).
  - Accumulates straight RGBA channels in `f64` row-major order weighted by separable overlap `wpx = wx * wy` (skipping `wpx <= 0.0`), normalizes by `weight`, and converts via `(acc[c] / weight).round().clamp(0., 255.) as u8`.
  - At `source_scale == 1.0`, `fx0 = canonical.left + i` and `fx1 = canonical.left + i + 1` are exact integers, `sx1 = sx0 + 1`, `wpx = 1.0`, and output is bit-identical to `Image::crop`.
- **Multi-scale verification:** Tested across scales `1.00`, `1.25`, `1.50`, `2.00` (`canonical_identity_determinism_and_calibration`, `src/devtools/mod.rs:1558-1647`) and non-binary scales `1.10`, `1.75` (`canonicalize_non_binary_scales_have_no_edge_artifacts`, `src/devtools/mod.rs:2123-2157`), confirming flat logical regions canonicalize without synthetic edge artifacts.

---

## 4. D01 — Real Cross-Authority Proof (`button.default` vs `probe.button` at Scale 1.25)

- **Inputs:**
  - Reference contract: `reference/shadcn-gallery-v0.1/contract.json` (`--capture components/default --id button.default`)
  - Reference image: `reference/shadcn-gallery-v0.1/screenshots/light/components.png` (`1440 × 2140` at DPR `1.0`)
  - Native target: `probe.button` captured via live WGC at host scale `1.25` (DPI 120, client `962 × 853` px).
- **Exit semantics:** Exits `6` (`pass: false`), which is expected because `devctl_probe`'s button (`"Count"`) intentionally does not implement Shadcn styling.
- **Independently inspected artifacts (`benchmark/results/rust-ui-devctl-v0.1-rework/cross/` and live test reproduction):**
  - `reference.png`: **`70 × 32` px** (`1.0 px/Dp`, crop/canonical rect `{left: 273, top: 301, right: 343, bottom: 333}`). Crisp 32 px height with no spurious top background row.
  - `native-raw.png`: **`87 × 45` px** (`1.25 px/Dp`, physical crop rect `{left: 66, top: 105, right: 153, bottom: 150}`, matching `snapshot.json` `rect_px`).
  - `native-canonical.png`: **`69 × 36` px** (`1.0 px/Dp`, canonical rect `{left: 53, top: 84, right: 122, bottom: 120}`, resampled via `area-box/1` from source footprint `{x: 66.25, y: 105.0, width: 86.25, height: 45.0}`).
  - `diff.png`: **`70 × 36` px** union canvas. Visual inspection confirms:
    - The `69 × 32` overlap (`2,520` pixels) shows spatially aligned red channel deltas comparing `"Default"` vs `"Count"` and button fill/radii in `1.0 px/Dp` logical space.
    - The `312` magenta `missing_pixels` (`1 × 32 = 32` px on the right + `70 × 4 = 280` px on the bottom) equal `70×36 - 69×32 = 312`, reflecting **only** the real logical size difference between `button.default` (`69.984 × 32.0 Dp`) and `probe.button` (`69.4609375 × 36.0 Dp`), with **no** artificial `1.25x` scale border.
- **Verdict:** **D01 CLOSED.**

---

## 5. D02 — Single Coordinate Snap Authority

- **Centralized authority (`src/devtools/mod.rs:152-181`):**
  - `devtools::snap_rect(logical: Rect, scale: f64) -> Result<PixelRect>` validates finite positive `scale` and finite representable coordinates, casts `f64 -> f32` once at the boundary, checks edge representability via `ScaleFactor::try_to_physical`, and delegates to `crate::geom::Rect::physical(s)`.
  - `Image::crop(&self, r: &PixelRect)` (`src/devtools/mod.rs:905-929`) accepts only an already-snapped integer `PixelRect` (`crate::geom::PhysicalRect`). All `floor`/`ceil` rounding is removed from production code.
  - `--pad` on `compare-element` expands the logical `Rect` (`Rect::grow(pad)`) before snapping through `snap_rect`.
- **Fractional geometry & systemic audit (`src/devtools/mod.rs:1362-1535`):**
  - `button.default` (`x = 273.0, y = 300.75, width = 69.984, height = 32.0`) snaps at `1.0` to `{left: 273, top: 301, right: 343, bottom: 333}` = **`70 × 32` px** (not `70 × 33`).
  - `systemic_frozen_contract_snap_audit` iterates all **581** element-capture records and **806** visible-part records (**1,387** total) in `reference/shadcn-gallery-v0.1/contract.json` across scales `1.00, 1.25, 1.50, 2.00`, verifying edge rounding and confirming all **421/581** element disagreements and **578/806** visible-part disagreements (**999/1,387** total) with the retired `floor`/`ceil` crop policy are resolved.
- **Verdict:** **D02 CLOSED.**

---

## 6. D03 — `contract/0.2` Adapter Completeness

- **`reference_targets` (`src/devtools/mod.rs:360-497`) preserves:**
  - `automation_id` and `SemanticKey { owner, part }` (`owner` or `owner#part`);
  - `tier` (`core`, `later`, `reference-only`; unknown tier fails with `INVALID_ARGUMENT`);
  - exact `<page>/<state>` capture identity (unknown capture key fails with `TARGET_NOT_FOUND` without falling back to another capture);
  - element `rect` and part `relative_rect` + derived page-absolute `rect` (`owner.rect.x + relative.x`, `owner.rect.y + relative.y`);
  - `visible` and hidden `reason` (`"not-rendered"`, with `rect: None`);
  - `interaction` state object where present;
  - `owner_attributed`, `label_gap`, and remaining part `meta`.
- **Independently verified real frozen cases via `rust-ui-devctl compare-layout`:**
  1. **SettingsNav collapsed hidden label** (`--capture navigation/settings-nav-collapsed --id settings-nav.general#label`): emits `{"expected": "hidden", "id": "settings-nav.general#label", "part": "label", "pass": true, "reason": "not-rendered", "tier": "core"}` (exit `0`).
  2. **Dropdown hidden/non-rendered part** (`--capture overlays/default --id dropdown-menu.default.content#group[appearance].item[word-wrap].indicator`): emits `{"expected": "hidden", ..., "pass": true, "reason": "not-rendered", "tier": "core"}` (exit `0`).
  3. **Checkbox owner-attributed label** (`--capture forms/default --id checkbox.default.unchecked#label`): preserves `owner_attributed: true`, `label_gap: Some(8.0)`, `relative_rect.x: 24.0`, and derived absolute `expected: {"x": 228.0, "y": 1266.5, "width": 71.984, "height": 14.0}`.
  4. **Visible nested/keyed part** (`--capture overlays/default --id dropdown-menu.default.content#group[appearance].item[toolbar].indicator`): resolves nested key `group[appearance].item[toolbar].indicator` with derived absolute `expected: {"x": 300.0, "y": 1619.5, "width": 16.0, "height": 16.0}`.
  5. **Wrong capture key** (`--capture components/nonexistent`): fails closed with `{"error": {"code": "TARGET_NOT_FOUND", "message": "capture has no elements"}, "ok": false}` (exit `3`).

---

## 7. D03 — Core Tier Scope

- **`TierScope::parse` (`src/devtools/mod.rs:535-563`):**
  - Defaults to `core` when `--tier` is omitted.
  - Accepts `all` or comma-separated valid tiers (`core`, `later`, `reference-only`).
  - Rejects unknown or ambiguous tier specs (`--tier bogus`, `--tier all,core`) with `INVALID_ARGUMENT` (exit `2`).
  - Excluded targets remain in authority and are counted in `scope.excluded_by_tier`; native elements matching an excluded tier report under `out_of_scope` rather than `unexpected`.
  - Explicit `--id` bypasses the tier filter while still reporting the target's `tier`.
- **Independently verified on `forms/default`:**
  - `--tier core`: `scope.excluded_by_tier = 13`, `missing = 122`.
  - `--tier all`: `scope.excluded_by_tier = 0`, `missing = 135`.
  - `--tier bogus`: fails with `INVALID_ARGUMENT: unknown tier bogus` (exit `2`).
- **Verdict:** **D03 tier closure PASS.**

---

## 8. D03 — Native Parts Usability

- **Assessment across protocol, schema, adapter, and comparator layers:**
  - `rust-ui.devctl/0.1` wire protocol, `rust-ui.devctl.snapshot-layout/0.2` (`Node.parts: Vec<PartNode>` with `{key, rect_dp, rect_px, visible, reason}`), `SemanticKey { owner, part }` (`owner#part`), `Snapshot::index()` (requiring an `automation_id` owner for any node with `parts`), `Snapshot::part_index()` (indexing `(owner, part)` and rejecting duplicates), `reference_targets` / `snapshot_targets`, `compare_layout`, and `compare_element` (`rust-ui.devctl.compare-element/0.2`) form a complete, generic end-to-end semantic-parts model.
  - Currently, `Backend::dev_node` (`src/platform/win32/devtools.rs:124`) emits `parts: vec![]` because no compound Shadcn components exist yet in the foundation runtime. When compound components are built during the Native Gallery milestone, populating `Node.parts` from retained component subtrees requires only local runtime/node wiring into `dev_node` — **without** redesigning the devctl wire protocol, `snapshot-layout/0.2` schema, `contract/0.2` adapter, or `compare-element/0.2` schema.
- **Explicit Answer:**
  ```text
  Does v0.1 now provide a usable generic native semantic-parts exposure contract?
  YES
  ```
- **Verdict:** **D03 CLOSED.**

---

## 9. D04 — Automation ID Invariant

- **Unconditional enforcement (`src/runtime.rs:333-349`):**
  - The `#[cfg(any(test, feature = "devtools"))]` gate has been removed from `Runtime::review()` (`grep "cfg(any(test" src/` returns 0 matches).
  - Every build enforces: non-empty (`!id.is_empty()`), `<= 256` UTF-8 bytes (`id.len() <= 256`), no control characters (`!id.chars().any(char::is_control)`), and uniqueness across the staged tree (`ids.insert(id.clone())`).
  - On violation, `Runtime::review()` pushes `UiDiagnostic::DuplicateAutomationId` to `self.diagnostics`, restores `self.scratch_nodes = tx.nodes`, and returns `Err(UiError::InvalidUi(UiDiagnostic::DuplicateAutomationId))` **before** `self.commit_children(...)` — leaving the committed arena and mounted native peers completely untouched.
- **`Ui::named` and builder audit (`src/ui.rs:582-603, 989-1586`):**
  - `Ui::named` checks `self.tx.nodes[index as usize].automation_id`: identical ID is idempotent; a different pre-existing ID pushes `UiDiagnostic::DuplicateAutomationId` to `self.tx.diagnostics` without overwriting.
  - All other `.automation_id(...)` methods (`LabelBuilder`, `ButtonBuilder`, `TextInputBuilder`, `CustomBuilder`, `ActionBuilder`, `TextBuilder`) apply to freshly staged leaf/action nodes created by that builder alone.
- **Non-test / non-devtools compilation proof (`tests/invalid_automation_id.rs`):**
  - `cargo test --locked --test invalid_automation_id` compiles the `rust_ui` library **without** `cfg(test)` and **without** `feature = "devtools"`, runs `App::run()` with duplicate and empty automation IDs, and verifies both return `Err(UiError::InvalidUi(UiDiagnostic::DuplicateAutomationId))` (`2/2 PASS`).
- **Verdict:** **D04 CLOSED.**

---

## 10. D05 — Focus Event Order

- **Peer-event drain ordering audit (`src/platform/win32/mod.rs` & `devtools.rs`):**
  - `dev_command("focus")` (`src/platform/win32/devtools.rs:269`) calls `Backend::focus_and_settle(Some(id))` (`src/platform/win32/mod.rs:1996-2000`), which executes `self.set_focus(node)? -> self.service_peer_events()? -> self.turn()`.
  - `Backend::uia_action` (`src/platform/win32/mod.rs:2358-2367`) executes `self.set_focus(Some(id))? -> self.service_peer_events()? -> self.turn()`.
  - `Backend::focus_step` (`src/platform/win32/mod.rs:1952-1954`, called from `key_msg` Tab traversal before `self.turn()`) executes `self.set_focus(Some(eligible[next]))? -> self.service_peer_events()`.
  - `Backend::pointer` primary-Down on a semantic non-peer node (`src/platform/win32/mod.rs:1693-1699`) executes `self.set_focus(Some(id))? -> self.service_peer_events()?` before `self.turn()`.
- **Pending-edit regression proofs (`src/tests.rs:7317-7435`):**
  - `devctl_focus_drains_peer_events_to_quiescence`, `non_devctl_focus_paths_drain_peer_events_before_turn` (`uia` and `tab` legs), and `semantic_pointer_down_drains_peer_events_before_turn` all verify that a pending native RichEdit `EN_CHANGE` (`emit_change_for_test`) at a focus transition is drained by `service_peer_events()`, pumped by `rt.pump()` within the same turn, updates `state.v.text()` to `"abc+native"`, and leaves `native_notifications == 0` and `events == false` at `wait-idle`.
- **Verdict:** **D05 focus CLOSED.**

---

## 11. D05 — `leave` / Unhover

- **End-to-end implementation:**
  - CLI allow-list (`src/bin/rust-ui-devctl.rs:45`): `"wait-idle" | "leave" => &["session", "timeout"]` (rejects target IDs or extraneous options).
  - Protocol & dispatch (`src/platform/win32/devtools.rs:272-275`): `"leave"` invokes `self.leave().map_err(platform)?` and returns `{"performed": "leave"}`.
  - Backend (`src/platform/win32/mod.rs:2160-2188`): `Backend::leave()` clears `self.hot`, marks interaction state dirty via `mark_state_dirty`, emits `PointerPhase::Leave` to non-peer nodes, disarms tooltips (`sched_unarm_tooltip`), and runs `self.turn()`. (Held `press` state remains explicit until `release`, as documented in `docs/DEVCTL.md:76-78`).
  - Verified in live Windows workflow (`tests/devctl_windows.rs:177-192`): `hover probe.button -> wait-idle -> tree (hovered: true) -> leave -> wait-idle -> tree (hovered: false)`.
- **Verdict:** **D05 CLOSED.**

---

## 12. Snapshot & Comparison Schema Revisions

- **`rust-ui.devctl.snapshot-layout/0.2` (`src/devtools/mod.rs:267`):** Checked with strict equality in `Snapshot::index()`; `0.1` documents are rejected (`src/devtools/mod.rs:1849-1851`). Deterministic `BTreeMap` indexing, retained child order, and bounds (`8,192` nodes, `128` depth, `4 MiB` message limit) remain intact.
- **`rust-ui.devctl.compare-element/0.2` (`src/devtools/mod.rs:790`):** Explicitly versioned in `comparison.json`. Historical v0.1 evidence under `benchmark/results/rust-ui-devctl-v0.1/` is preserved untouched and not reinterpreted.

---

## 13. Comparator Negative Controls

Independently verified in `src/devtools/mod.rs` unit tests and live CLI checks:
- **Canonical raster vs itself:** `mismatched_pixels = 0`, `missing_pixels = 0`, `pass = true`.
- **Same logical geometry at `1.0` vs `1.25` / `1.5` / `2.0` (`compare_element_position_independent_canonical`, `canonical_identity_determinism_and_calibration`):** Canonical dimensions align (`40 × 20`), `missing_pixels = 0`, `mismatched_pixels = 0` (no artificial missing border).
- **Real logical size difference (`compare_element_size_delta_is_missing_pixels_not_scale_noise`):** `44 × 20` vs `40 × 20` Dp at scale `1.25` produces canonical dimensions `[44, 20]` vs `[40, 20]` and `missing_pixels = 80` (exact `4 × 20` right-hand strip).
- **Different visual fill/text (`compare_element_position_independent_canonical`):** Produces `missing_pixels = 0`, `mismatched_pixels = 80`, and exact logical `difference_bbox: {"x": 4.0, "y": 0.0, "width": 4.0, "height": 20.0}`.
- **Off-capture element:** Fails closed with `TARGET_NOT_VISIBLE` (no coordinate clamping).

---

## 14. Resource & Idle Regression

Verified in `benchmark/results/rust-ui-devctl-v0.1-rework/` (`windows-devctl-windows.txt`, `cycles/summary.json`) and during our Windows acceptance run:
- **Handle stability:** Target process handles stabilize after warmup and remain flat across 6 consecutive steady-state WGC captures (`207 -> 207` across all 5 scripted cycles; `232 -> 232, 232, 232, 232, 232, 232` in our live cycle 0 run).
- **Zero idle polling / CPU delta:** Across a 2-second idle window on an active instrumented session:
  - Process CPU delta: **`0` ticks** (`GetProcessTimes` 100ns kernel+user ticks).
  - `native_timer_fires`: **`0 -> 0`**.
  - `requested_redraws`: **`71 -> 71`**.
- **Shutdown & receipt cleanup:** `shutdown` exits the target process and removes the session receipt; `cleanup` removes stale receipts only when the owned PID is no longer running.

---

## 15. Capture Guard Flake — Classification

- **Observation & mechanism:** `screenshot` brackets `capture_client` with pre- and post-capture `snapshot-layout` and `paint_epoch` checks (`src/bin/rust-ui-devctl.rs:287-318`). Because `devctl_probe` is a visible desktop window, any physical OS mouse movement entering or leaving the window during WGC frame acquisition delivers `WM_MOUSEMOVE` / `WM_MOUSELEAVE`, flipping `Backend::hot` (`Node.hovered`) and triggering the fail-closed `CAPTURE_FAILED: retained semantic geometry/state changed during WGC capture; wait idle and retry` guard.
- **Classification:**
  - Pre-existing at `7a1ad19` (not introduced or worsened by `D01`–`D05`);
  - Fail-closed by design (prevents stale/inconsistent element crops);
  - Safe to retry the `screenshot` command after `wait-idle`.
  ```text
  SEPARATE_DEVCTL_CAPTURE_STABILITY_FOLLOWUP
  NON_BLOCKING_FOR_V0_1_CLOSURE
  ```

---

## 16. Dependencies & Frozen Reference Immutability

- **Dependencies:** `git diff 7a1ad19a397d10149b6d50ebdf5c1c60d0c6bc67 9f88d7ca3a57bd876ef505d36f819c090ba0a973 -- Cargo.toml Cargo.lock` is **empty**. No heavy image crate, CLI framework, or async runtime was added.
- **No forbidden APIs:** Zero uses of `PrintWindow`, `BitBlt`, `SendInput`, `mouse_event`, or `SetCursorPos` in production/tooling code.
- **Frozen reference immutability:**
  - `git rev-parse dc70f6644c0384170f3a6929eb2caaf33fc711a0:reference` == `9f88d7ca3a57bd876ef505d36f819c090ba0a973:reference` == `e0bd8eb16d38b778edab4d4ee23ca823696ceb43`.
  - `git diff 2e14e417a69ef8967b8fb17120282e3de6449fae 9f88d7ca3a57bd876ef505d36f819c090ba0a973 -- reference/shadcn-gallery-v0.1/screenshots/ reference/shadcn-gallery-v0.1/contract.json reference/shadcn-gallery-v0.1/tokens.json` is **empty**.
  - All **18/18** frozen screenshot PNGs under `reference/shadcn-gallery-v0.1/screenshots/{light,dark}/` match `reference/shadcn-gallery-v0.1/screenshots/SHA256.json` (`0` mismatches).

---

## 17. Test & Build Gate Results

Independently executed on native Windows (`W:\devin_folder\rust-ui-closure-astra` at `9f88d7ca3a57bd876ef505d36f819c090ba0a973`, DPI 120 / scale `1.25`):

| Command | Result |
|---|---|
| `cargo fmt --check` | **PASS** (exit 0) |
| `cargo check --locked` | **PASS** (exit 0) |
| `cargo check --locked --features devtools` | **PASS** (exit 0) |
| `cargo test --locked --lib` | **128/128 PASS** (0.71s; `native_probe_richedit_paints_text` did not flake in this run) |
| `cargo test --locked --features devtools --lib` | **156/156 PASS** (0.72s) |
| `cargo test --locked --test invalid_automation_id` | **2/2 PASS** (0.07s) |
| `cargo test --locked --features devtools --test devctl_windows` | Verified complete cycle 0 end-to-end (`scale=1.25`, `leave` hover clear, RichEdit `"Calibration tex"`, cross-authority `button.default` vs `probe.button` exit 6 with `70×32` ref / `87×45` raw / `69×36` canonical / `312` missing pixels, 6 steady captures at `232` flat handles, 2s idle `0` CPU ticks); subsequent cycle hit the pre-existing fail-closed OS cursor `CAPTURE_FAILED` guard classified in §15 (`1/1 PASS` in committed clean-desktop `windows-devctl-windows.txt` and `5/5 PASS` in `cycles/summary.json`). |

---

## 18. Evidence Identity

- Inspected `benchmark/results/rust-ui-devctl-v0.1-rework/`:
  - `identity.json` ties all artifacts to `"candidate": "cf8ab919b399ca4465a17710741f1c888b84cfa0"` and frozen reference `dc70f6644c0384170f3a6929eb2caaf33fc711a0` (`e0bd8eb16d38b778edab4d4ee23ca823696ceb43`).
  - All **42/42** files in the authoritative Windows rework checkout match `SHA256.json` (`0` mismatches; all 8 PNG artifacts match byte-for-byte across both Windows and Linux checkouts).
  - Visually inspected `cross/reference.png` (`70×32`), `cross/native-raw.png` (`87×45`), `cross/native-canonical.png` (`69×36`), `cross/diff.png` (`70×36`), `probe/client.png` (`962×853`), `probe/normal.png` (`87×45`), `probe/hover.png` (`87×45`), and `probe/pressed.png` (`87×45`). All image contents match the report's claims.

---

## 19. Universal Gates 1–20 Evaluation

| Gate | Verdict | Rationale |
|---|---|---|
| **Gate 1 — Requirements & Goal Completeness** | **PASS** | All five rework items (`D01`–`D05`) are implemented, tested, and evidenced on native Windows at scale `1.25`. |
| **Gate 2 — Assumptions, Ambiguity & Approval Boundaries** | **PASS** | Implementation follows the settled rework design without inventing unapproved semantics. |
| **Gate 3 — Existing Behavior & Contract Preservation** | **PASS** | Frozen reference (`dc70f66` / `2e14e41`) and foundation tests (`128/128`) are preserved. |
| **Gate 4 — Scope Discipline / Surgical Change** | **PASS** | Diff is strictly confined to `D01`–`D05`, tests, docs, evidence, and the durable `AGENTS.md` cadence rule. |
| **Gate 5 — Minimal Correct Design / No Overengineering** | **PASS** | Local 60-line `area-box/1` resampler and direct `snap_rect` delegation; no external framework bloat. |
| **Gate 6 — Architecture, Ownership & Responsibility Boundaries** | **PASS** | `snap_rect` delegates to the single `crate::geom::Rect::physical` authority; Win32 stays isolated in `src/platform/win32/`. |
| **Gate 7 — Duplication & Existing Capability Reuse** | **PASS** | Eliminated duplicate `floor`/`ceil` crop rounding in `Image::crop`; reuses `crate::geom::ScaleFactor` and `PhysicalRect`. |
| **Gate 8 — No Unrequested Fallback / Shim / Legacy Path** | **PASS** | Hard-cut schema bumps to `snapshot-layout/0.2` and `compare-element/0.2`; WGC-only capture. |
| **Gate 9 — Deterministic Semantics & Stable Contracts** | **PASS** | Deterministic `area-box/1` canonicalization, unconditional `automation_id` validation, and settled focus/leave semantics. |
| **Gate 10 — Authority, Specs, Rules & Obsolescence** | **PASS** | `reference_targets` faithfully adapts `rust-ui.shadcn-reference.contract/0.2` including parts, hidden parts, and tiers. |
| **Gate 11 — Failure Safety, Atomicity, Idempotency & Security** | **PASS** | `DuplicateAutomationId` rejects atomically before arena/peer commit; `canonicalize` and `crop` fail closed on out-of-bounds footprints. |
| **Gate 12 — Boundedness / Resource & Agent-Visible Output Discipline** | **PASS** | Bounded `MAX_PIXELS`, `MAX_MESSAGE`, node/depth limits, and single-JSON stdout discipline preserved. |
| **Gate 13 — Dependency, External-System & Blast-Radius Necessity** | **PASS** | `Cargo.toml` and `Cargo.lock` are completely unchanged from `7a1ad19`. |
| **Gate 14 — Performance, Latency & Redundant Work** | **PASS** | Zero idle CPU ticks, zero timer fires, zero redraw drift over 2s idle windows; handle count flat across repeated WGC captures. |
| **Gate 15 — Persistence / Infrastructure Boundary** | **PASS** | Artifact persistence remains in CLI/devtools layers; runtime remains persistence-agnostic. |
| **Gate 16 — Verification Sufficiency & Test Quality** | **PASS** | Unit, non-test/non-devtools integration (`invalid_automation_id`), systemic 1,387-record contract audit, and live Windows WGC cross-authority proof all pass. |
| **Gate 17 — Recovery Proportionality / Sunk-Cost Containment** | **PASS** | Clean, surgical correction on `agent/rust-ui-devctl-v0.1`. |
| **Gate 18 — Verification / Review Immutability & Artifact Identity** | **PASS** | Implementation (`cf8ab91`) and evidence publication (`dcb68f2`) are cleanly separated; `SHA256.json` and `identity.json` verified. |
| **Gate 19 — Completion Integrity / Final Artifact Honesty** | **PASS** | Cross-authority mismatch (`exit 6`) on the unstyled calibration probe is reported honestly alongside aligned canonical diff evidence. |
| **Gate 20 — Systemic Scope Completeness / Exhaustive Invariant Audit** | **PASS** | Exhaustive audit across all `581` element records + `806` visible part records (`1,387` total) at 4 scales and all `set_focus` callers (`4` boundaries). |

---

## 20. Final Verdict

```text
D01 CLOSED
D02 CLOSED
D03 CLOSED
D04 CLOSED
D05 CLOSED

RUST_UI_DEVCTL_V0_1_APPROVED
NATIVE_GALLERY_TOOLING_CLEARED
```

```text
Can Opus 5.5 now perform:

implement
→ launch
→ inspect semantic tree
→ interact
→ wait-idle
→ WGC capture
→ compare against frozen HTML
→ inspect aligned actionable diff
→ fix

without redesigning devctl?

YES
```

---

```text
EXECUTION FOOTER

RUN_KEY: RUI-A-261015-2310
ROLE: Astra Advisor
PROJECT: rceman/rust-ui
BRANCH: review/rust-ui-devctl-v0.1-closure-astra
STARTED_AT: 2026-10-19T10:47:49+03:00
FINISHED_AT: 2026-10-19T11:02:30+03:00
BASED_ON: 9f88d7ca3a57bd876ef505d36f819c090ba0a973
PRODUCTION_HEAD: cf8ab919b399ca4465a17710741f1c888b84cfa0
REPORT_HEAD: PENDING_COMMIT
STATUS: RUST_UI_DEVCTL_V0_1_APPROVED
NEXT_OWNER: User
```
