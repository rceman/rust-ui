# rust-ui-devctl v0.1 correction — rework report

- Implementation (`PRODUCTION_HEAD`): `cf8ab919b399ca4465a17710741f1c888b84cfa0` (branch `agent/rust-ui-devctl-v0.1`)
- Report (`REPORT_HEAD`): `<set by publication commit>`
- Design authority: `W:\devin_folder\devctl-rework-design.md`
- Frozen reference: `dc70f6644c0384170f3a6929eb2caaf33fc711a0` (tree `reference/` = `e0bd8eb16d38b778edab4d4ee23ca823696ceb43`)
- Evidence bundle: `benchmark/results/rust-ui-devctl-v0.1-rework/` (all artifacts produced from a **fresh clone** of the candidate at host scale 1.25 / DPI 120)
- STATUS: `RUST_UI_DEVCTL_V0_1_READY_FOR_CLOSURE_REVIEW`

## D01 CANONICAL COMPARISON RASTER

CLOSED.

Two raster spaces are now explicit in `compare-element`
(`src/bin/rust-ui-devctl.rs` → `d::compare_element` at `src/devtools/mod.rs:691`):

- **Raw space** — element-relative crop at each side's own capture scale.
  `native-raw.png` is `Image::crop` of `snap_rect(native_rect.grow(pad), s)`
  against the full client capture; `native-raw.png` is never overwritten with
  normalized data (`mod.rs:764-768`). `reference.png` likewise at the
  contract's 1.0 px/Dp.
- **Canonical space** — `native-canonical.png` is the raw raster resampled by
  `Image::canonicalize` (`mod.rs:941`, `area-box/1` — exact area-weighted box
  filter over the source footprint `s*C`; footprint edges computed as
  `s*(left+i)`/`s*(left+i+1)`, zero-overlap source pixels skipped) onto the
  canonical lattice `C = snap_rect(native_rect.grow(pad), 1.0)`.
  `reference_canonical` uses `snap_rect(want.grow(pad), 1.0)`. Each side
  canonicalizes by **its own** logical rect (the H1-rework fix): equal logical
  sizes give equal canonical dimensions; a logical size delta shows as honest
  `missing_pixels` at the right/bottom — never scale noise.
- `s == 1.0` is bit-identical to a plain crop (`canonicalize`
  footprints land on integer boundaries; proven by
  `compare_element_position_independent_canonical` and `run-100` sample:
  `normalization.identity:true`, `mismatched_pixels:0`).
- `compare_image`/`diff` run reference-vs-**canonical** only; `pass=false`
  still exits 6 (CLI exit map `rust-ui-devctl.rs:398-410`).
- `comparison.json` schema `rust-ui.devctl.compare-element/0.2`
  (`mod.rs:790`): per-side `scale_px_per_dp`/`crop_rect_px`/`canonical_rect_dp`/
  `dimensions`, `normalization.{source_scale,target_scale,source_dimensions,
  canonical_dimensions,resampler,identity,transform{canonical_rect_dp,
  source_footprint_px}}`, `visual_canonical`, `visual_raw`
  (`computed:false` + reason when scales differ — see RAW VS CANONICAL),
  `evidence_classes`, `geometry`, `scope`.
- Dimension guards fail closed with `INVALID_ARGUMENT`: native-image dims must
  equal snapshot `client_px`; reference-image dims must equal the contract page
  document dims (contract path) or the reference snapshot `client_px`.
- Tests: `canonicalize_non_binary_scales_have_no_edge_artifacts` (s = 1.1, 1.75:
  flat region canonicalizes exactly — no spurious edge pixels),
  `compare_element_position_independent_canonical` (ref at (100,50)@1.0 vs
  native at (24,12)@1.25 → equal canonical dims, missing=0, mismatched=0 —
  position does not matter, only content; a style-different native yields a diff
  bbox exactly equal to the logical region),
  `compare_element_size_delta_is_missing_pixels_not_scale_noise` (44×20 vs 40×20
  → missing=80, exactly the 4×20 logical strip; off-capture footprint →
  `TARGET_NOT_VISIBLE`, no clamping).

## D02 COORDINATE SNAP AUTHORITY

CLOSED.

`snap_rect` (`src/devtools/mod.rs:152`) is the single authority for
logical→physical rect conversion and delegates to `crate::geom::Rect::physical`
(edge-`round` at the given scale). `Image::crop` takes an integer `PixelRect`
(no floor/ceil anywhere in the pipeline). `--pad` on `compare-element` grows the
**logical** rect before snapping.

Systemic audit against the real frozen `contract.json`
(`systemic_frozen_contract_snap_audit`, `mod.rs:1463`): iterates all **581**
element-capture records plus **806** visible-part absolute rects (1387 total);
for each, restates the snap spec independently
(`round(edge*s)`, integer-Dp sizes keep exact integer px when `w*s`/`h*s` are
integral) at scales {1.0, 1.25, 1.5, 2.0} and counts disagreements with the
retired floor/ceil crop policy. Measured, asserted in the test and reproduced by
an independent scratch recount with Rust half-away-from-zero rounding
(`benchmark/.../systemic-audit-counts.txt`):

- element records: **421/581 disagree** (matches the independent review's figure)
- visible-part records: **578/806 disagree**
- total: **999/1387**

## D03 CONTRACT/0.2 ADAPTER

CLOSED.

`reference_targets` (`mod.rs:360`) reads the frozen `contract/0.2`: exact
`--capture <page>/<capture_state>` capture records, `tier`/`visible`/`reason`
propagated (14 hidden parts become explicit non-visible targets),
element-relative part rects (`part` key `owner#part` via
`SemanticKey{owner,part}` / `display()`/`parse()`), `owner_attributed` and
`label_gap` forwarded into `meta`. `snapshot_targets` (`mod.rs:501`) treats a
snapshot-layout/0.2 document as its own authority with `tier:null` (no tier
authority in snapshots → always in scope). `TierScope::parse` (`mod.rs:535`)
defaults to `core`; `all`/csv accepted; unknown tier → `INVALID_ARGUMENT`;
out-of-scope rows are emitted with `pass:null` + `reason:"excluded by --tier"`
and counted under `scope.excluded_by_tier`.

`snapshot-layout` schema is bumped to `rust-ui.devctl.snapshot-layout/0.2`
(`mod.rs:267`): the only change is the additive `parts: []` field on nodes
(`Node.parts`, `mod.rs:217`; native runtime emits none —
`src/platform/win32/devtools.rs` `dev_node`); strict parse rejects 0.1 docs.
`--native-id` selects the native target independently of `--id`; both accept
`owner#part` (snapshot `part_index` rejects duplicate `(owner,part)` keys with
`INVALID_ARGUMENT`). `compare_layout(..., tiers)` reports
`scope.excluded_by_tier`/`out_of_scope`.

## D04 AUTOMATION-ID INVARIANT

CLOSED.

`Runtime::review()` (`src/runtime.rs:333-349`) validates automation IDs in
**every build** — the `cfg(any(test, feature="devtools"))` gate is removed.
Valid IDs are non-empty, ≤ 256 bytes, contain no control characters, and are
unique across the staged tree. A rejection pushes
`UiDiagnostic::DuplicateAutomationId` into `self.diagnostics`, restores
`self.scratch_nodes = tx.nodes`, and returns `Err(UiError::InvalidUi(..))` —
all **before any commit** (the committed arena and peers are untouched; proven
by `automation_rejection_leaves_arena_and_peers_untouched`).

`Ui::named` (`src/ui.rs:582-606`): same ID is idempotent; a different ID
produced inside `draw` pushes a transaction `DuplicateAutomationId` diagnostic
and does **not** overwrite the existing name. The other `.automation_id`
setters in `ui.rs` were audited: all write onto a fresh staged node (builder
holds `automation_id: Option`, applied once at finalize) — `named` is the only
setter that can meet a pre-existing ID.

Unit tests (`src/tests.rs`): `automation_id_invariants_reject_in_all_builds`
(empty / >256 bytes / control char — each atomic + diagnostic recorded),
`automation_duplicate_aborts_and_semantic_id_survives_reconcile`,
`named_id_conflict_is_diagnostic_not_overwrite`,
`automation_rejection_leaves_arena_and_peers_untouched`.

**Build-independence evidence.** `src/tests.rs` is compiled under `cfg(test)`
and cannot prove the gate is gone; two independent proofs are provided:
(1) `grep cfg src/runtime.rs` — the only remaining `#[cfg(test)]`s are
test-helper fns; the check block has none; `grep cfg(any(test` across `src/`
→ 0 hits. (2) `tests/invalid_automation_id.rs` — an integration test compiled
against the library **without** `cfg(test)`/`devtools`: the public API *does*
permit observation (`App::run` creates a real window; the first `review()`
fails → `bail` → `Err(InvalidUi(DuplicateAutomationId))`). 2/2 pass.

**Mutation evidence.** With the check stubbed out, three tests failed
(`automation_id_invariants_reject_in_all_builds`,
`automation_duplicate_aborts_and_semantic_id_survives_reconcile`,
`automation_rejection_leaves_arena_and_peers_untouched`); restored → all pass.
(`named_id_conflict` still passes under the mutation — its rejection rides the
transaction-diagnostics preflight, which is correct behavior.)

## D05 INTERACTION / IDLE

CLOSED.

- `focus` drains peer events at the backend boundary via
  `Backend::focus_and_settle` (`src/platform/win32/mod.rs:1996`):
  `set_focus → service_peer_events → turn`. `dev_command("focus")` calls it
  (`src/platform/win32/devtools.rs`).
- `leave` — exact spelling — calls `Backend::leave()` (`mod.rs:2160`, clears
  `hot`/`pressed`/capture and disarms the tooltip owner) and returns
  `{"performed":"leave"}`; CLI `leave --session <f> [--timeout ms]` accepts no
  id/args (allow-list `rust-ui-devctl.rs`). Documented in `docs/DEVCTL.md` and
  `docs/DEVCTL_PROTOCOL.md`.
- `set_focus` audit: `point_at` peer-Down already ended
  `deliver_native → service_peer_events → turn`; **fixed callers** —
  semantic pointer-Down (`mod.rs:1694`), `focus_step` traversal, and
  `uia_action(focus)` — each `set_focus → service_peer_events → turn`, each
  justified by a demonstrated RED→GREEN (see below). `u.set_focus` sites
  (UIA-provider bookkeeping) are a different object's method — not callers.

**Defect model (settled in H2):** `turn_body` already calls
`service_peer_events` at its tail, so events drained there arrive **after**
the pump — Change/SelChange → `emit Edit` → a sink notification left for the
**next** boundary: `wait-idle` observes `native_notifications>0` and the edit
applies a boundary late. Plain WM_SETFOCUS alone emits only
Caret/Invalidate host events (no sink items), so the RED reproduces the real
mid-typing race with a pending RichEdit EN_CHANGE (`emit_change_for_test`).

- RED (unfixed `set_focus→turn`, `devctl_focus_drains_peer_events_to_quiescence`):
  `left:"abc" right:"abc+native"` — the pending edit stranded past the
  boundary. GREEN after the drain.
- `non_devctl_focus_paths_drain_peer_events_before_turn`: RED on the UIA leg
  (same `left/right`), GREEN after — covers `uia_action` + `focus_step`.
- `semantic_pointer_down_drains_peer_events_before_turn` (added in H3a per
  review): editor focused + pending EN_CHANGE → primary pointer-Down on a
  semantic button (KILLFOCUS to the peer) → RED `"abc" vs "abc+native"` on the
  unfixed line → GREEN restored. All four fixed callers are now
  test-demonstrated.
- Lifecycle regression in `tests/devctl_windows.rs` (additive):
  `hover probe.button → wait-idle → tree hovered:true → leave → wait-idle →
  tree hovered:false` — verified in every workflow run (and in the 5-cycle
  scripted workflow: `hovered True` after hover, `False` after `leave`,
  `probe.input.label == "Calibration tex"` after `focus`/`End`/`Backspace`).

## CROSS-AUTHORITY HTML↔NATIVE PROOF

Observed on the fresh clone at host scale 1.25 (evidence `cross/` + transcripts):

```
compare-element --reference contract.json --capture components/default
  --id button.default --native-id probe.button
  --reference-image .../screenshots/light/components.png
  --native-image <client.png> --snapshot <snapshot.json> --out-dir cross/
→ exit 6, pass:false, schema rust-ui.devctl.compare-element/0.2
```

| facet | observed |
|---|---|
| `raster.reference` | dims **70×32** on the 1.0 lattice, canonical `{273,301,343,333}` |
| `raster.native_raw` | scale **1.25**, dims **87×45** == snapshot `rect_px` (physical evidence, unnormalized) |
| `raster.native_canonical` | scale **1.0**, dims **69×36** == `snap_rect(native logical, 1.0)` (`{53,84,122,120}`) |
| `normalization` | `source_scale` 1.25, `resampler` **area-box/1**, `identity` false, footprint `{x:66.25, y:105, w:86.25, h:45}` |
| `visual_canonical` | `missing_pixels` **312** = 70×36 − 69×32 — exactly the union−intersection of the canonical dims, i.e. produced **solely by the logical size delta**, no scale artifact; `mismatched` 2520 |
| `visual_raw` | `computed:false` (cross-scale raw diff not meaningful) |
| `geometry` | `delta {x:−220, y:−217.13, w:−0.52, h:+4}` → `pass:false` (absolute mode — the probe places the element at its own position) |
| `compare-layout --reference contract.json` | exit 6 — every core element `missing:true` (the probe tree genuinely lacks them; honest negative) |

**This is NOT a visual PASS** and does not claim glyph raster identity: the
probe button's own styling differs from the shadcn reference across the entire
overlap (all 2520 intersection pixels mismatch). What the run proves is the
*evidence model*: reference stays on the 1.0 contract lattice, native evidence
is produced in physical px at the true host scale, the canonical raster is a
deterministic area-weighted resample of it, and every missing/diff pixel is
attributable to a real logical difference rather than rasterization noise.

## RAW VS CANONICAL ARTIFACTS

- Canonical lattice is **per-element**: each side snaps its OWN grown logical
  rect (`C_ref = snap(want.grow(pad),1.0)`, `C_nat = snap(native.grow(pad),1.0)`);
  canonical rasters align at each element's own top-left. A logical size delta
  → dimension mismatch + missing pixels at right/bottom (honest); equal sizes →
  equal dims.
- `native-raw.png` is `snap_rect(native_rect, s)` — the raw crop; the canonical
  raster is sampled from the **full raw capture** over the footprint `s*C`.
  The two rects can differ by a sub-pixel at the edges (raw snaps the true
  logical rect at scale s; canonical edges `round` at 1.0 — e.g.
  `{66,105,153,150}` vs footprint `{66.25,105,86.25,45}`) — by design: raw is
  unaltered physical evidence, canonical is the comparable raster.
- `visual_raw` is computed only when `reference_scale == native_scale`
  (snapshot-vs-snapshot calibration); otherwise `computed:false` + reason.
- `pass:false` always exits 6 — a failing comparison is evidence, not an error.
- Schema bumps: `snapshot-layout/0.1 → 0.2` (additive `parts`), and
  `compare-element` `comparison.json` → `0.2` (two raster spaces). The old 0.1
  evidence under `benchmark/results/rust-ui-devctl-v0.1/` remains the
  historical record and is not reinterpreted.

## TEST RESULTS

All run in the fresh clone `devctl-fresh-cf8ab91` at `cf8ab91`, host scale 1.25
(raw outputs under `benchmark/results/rust-ui-devctl-v0.1-rework/`):

| Command | Result |
|---|---|
| `cargo fmt --check` | exit 0 |
| `cargo check --locked` | exit 0 |
| `cargo check --locked --features devtools` | exit 0 (also `--all-targets` clean on the source worktree) |
| `cargo test --locked --lib` (parallel) | **128/128** |
| `cargo test --locked --lib -- --test-threads=1` | **128/128** |
| `cargo test --locked --features devtools --lib` (parallel) | **156/156** |
| `cargo test --locked --features devtools --lib -- --test-threads=1` | **156/156** |
| `cargo test --locked --test invalid_automation_id` | **2/2** |
| `cargo test --locked --features devtools --test devctl_windows` | **1/1** — 3 launch/capture/shutdown cycles, all assertions |
| `cargo test ... --lib systemic_frozen_contract_snap_audit` | 1/1 (counts independently reproduced: 421+578=999/1387) |
| Linux/WSL (cargo 1.93.1, content-identical copy) | `cargo test --locked --lib` **90/90**; `--features devtools --lib` **111/111**; `cargo check --locked --features devtools --bins` exit 0 |

**Known flake — status as observed:** `tests::native_probe_richedit_paints_text`
(host DPI 120, parallel-only) did **not** trigger in either parallel run of this
gate (it had failed 1/154 in an earlier parallel run during development). It
remains intermittent and unfixed per scope; it passes serially and alone.

**CAPTURE_FAILED guard flake:** one occurrence in 17 devctl_windows runs (the
fail-closed "retained scene changed during WGC capture" guard). 8/8 current +
8/8 baseline `7a1ad19` runs passed during characterization — cause not proven;
analysis shows a real OS pointer message (`WM_MOUSEMOVE`/`WM_MOUSELEAVE` →
`hot` → `hovered`) reaching the shown probe window mid-capture is the only
verified-reachable mechanism, and the susceptibility predates this rework
(`flake-summary.txt`). No retry was added (out of scope); recommended
follow-up: a bounded wait-idle→re-snapshot→re-capture retry inside the
`screenshot` arm — transient input produces a one-shot mismatch while a
genuinely mutating scene keeps mismatching.

## RESOURCE / IDLE REGRESSION

- `devctl_windows` per cycle: target handles 199→207 after warm-up, then
  **stable across 6 steady captures** (207, no growth); 2 s idle CPU delta
  **0**; `native_timer_fires` 0/0, `requested_redraws` 71/71 frozen across
  the idle window — no regression.
- Scripted 5-cycle workflow (`cycles/summary.json`): probe handles
  **207→207** before/after captures every cycle; 0 guard events; no
  `devctl_probe`/`rust-ui-devctl` processes and no session receipt files left
  after the final cycle (verified by `tasklist` + filesystem check).

## DEPENDENCY AUDIT

`git diff 7a1ad19 cf8ab91 -- Cargo.toml Cargo.lock` is **empty** — no new
dependencies, no version changes. Forbidden-API check on added lines vs
`7a1ad19`: no `PrintWindow`/`BitBlt`/`SendInput`/`mouse_event`/`SetCursorPos` —
all capture evidence is `Windows.Graphics.Capture` + D2D.

## FROZEN REFERENCE IMMUTABILITY

`git rev-parse cf8ab91:reference` == `dc70f66:reference` ==
`e0bd8eb16d38b778edab4d4ee23ca823696ceb43`; `git diff dc70f66..cf8ab91 --
reference` empty; all **18/18** PNGs under
`reference/shadcn-gallery-v0.1/screenshots/{light,dark}/` hash-match the frozen
`SHA256.json` (verified in the fresh clone, `frozen-reference.txt`). The
reference was consumed strictly read-only.
