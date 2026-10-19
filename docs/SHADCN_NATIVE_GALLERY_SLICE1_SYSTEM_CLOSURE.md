# System / Contract / Platform Closure Review — Shadcn Native Gallery Slice 1

Independent closure review of the four blocking findings (`S01`–`S04`) plus
devctl fixture hygiene reported by the prior Astra system review. Reviewer did
not write this code. Verification was performed by direct diff/code inspection
at the reviewed HEAD plus re-execution of every cheap Linux gate; Windows-gated
claims are assessed as evidence statements and checked for structural
plausibility (imports, cfgs, env plumbing, case matrices).

## 1. Identity & Immutability

- **Repository:** `rceman/rust-ui`
- **Review Worktree (WSL):** `/home/therceman/git/rust-ui-slice1-astra-closure`
- **Implementation / Publication Branch:** `agent/shadcn-native-gallery-slice1-opus55`
- **Review Branch:** `review/shadcn-native-gallery-slice1-system-closure-astra`
- **`BASED_ON`:** `8d3d27361071ee8915dd8f496ea98e8e9b005f0a`
- **`PRODUCTION_HEAD` (code under review):** `c8aa5371f039bcb4a9b84dfa4aad61f0edfa2f78`
- **Prior System Review:** `fe55115037bbe60843866de33049c67f9f729eaa`
  (`SHADCN_NATIVE_GALLERY_SLICE1_SYSTEM_REWORK_REQUIRED`, findings `S01`–`S04`)
- **Independent Visual Approval:** `3e9ebad7feef30c517292a72b60f0d6a363d89f9`
  (`SHADCN_NATIVE_GALLERY_SLICE1_VISUALLY_APPROVED`)
- **Frozen Reference Publication:** `dc70f6644c0384170f3a6929eb2caaf33fc711a0`
- **Delta reviewed:** `git diff e49966d529e6da5c2cacd4d88c3048dd58f8e640..HEAD`
  (16 files, +479/−105)

### Immutability

- `git diff dc70f6644c0384170f3a6929eb2caaf33fc711a0..HEAD -- reference/` is
  **empty**; `reference/` tree at HEAD resolves to
  `e0bd8eb16d38b778edab4d4ee23ca823696ceb43`, identical to the frozen
  publication. **Frozen reference untouched — PASS.**
- Worktree `git status` clean at HEAD; the only delta between `8d3d273` and
  `HEAD` is the single closure commit `c8aa537`.

## 2. Per-Finding Verification

### `S01` — Default Windows build/tests compile without devtools — **CLOSED**

Evidence:
- `Cargo.toml:50` — `"Win32_Graphics_Dwm"` is now the last entry of
  `[target."cfg(windows)".dependencies].windows.features`, i.e. owned by the
  base Windows dependency set.
- `Cargo.toml:8–25` — `[features].devtools` no longer lists
  `windows/Win32_Graphics_Dwm` (removed; confirmed by diff and by grep: the
  only `Dwm` occurrence in `Cargo.toml` is line 50).
- `src/platform/win32/mod.rs:35` unconditionally imports
  `windows::Win32::Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute}`;
  that import is now satisfied by default Windows features — the previous
  `E0432` under default `cargo.exe check --examples` is structurally resolved.
- Stale `Shadow {}` literals: every `Shadow` struct literal in `src/` now
  carries `spread`. Windows-gated test sites updated:
  `src/tests.rs:4135`, `4246`, `4272`, `4295`, `5323`, `5767`, `7518`
  (`spread: crate::geom::Dp(0.0)` / `spread: Dp(0.0)`). The intentional
  `spread: Dp(1.0)` at `src/style.rs:1549` is the frozen card ring-1 recipe,
  not a stale literal.
- Worker-reported Windows evidence (`check --locked --examples` PASS default
  and `--features devtools`; `test --locked --lib` 147/0 and 176/0) is not
  re-executable on Linux but is plausible: the dependency/feature graph and
  field initializers are the entire mechanism of the prior `E0432`/`E0063`
  failures, and both are corrected.
- Genericity: the fix is dependency/feature ownership plus field updates —
  no Gallery/ID special-casing.

### `S02` — Devtools semantic parts: populated icon edges + owner part prefix — **CLOSED**

Evidence (`src/platform/win32/devtools.rs`):
- **Owner prefix (`devtools.rs:109–131`):** `let mut prefix = Vec::new();` is
  created before the owner-part push; when `n.part_key = Some(pk)` the owner
  part is emitted **and** `prefix.push(pk.to_string())` seeds the descendant
  scope before `self.collect_parts(id, &mut prefix, &mut parts)`
  (`devtools.rs:131`). `collect_parts` (`devtools.rs:223–258`) emits
  `prefix.join(".")`, so `pre` + `line[N]` children compose
  `pre.line[1]`, `pre.line[2]`, `pre.line[3]` — matching `src/ui.rs` and the
  frozen `contract.json` keys for `typography.code-block`.
- **Icon edges (`devtools.rs:155–185`):** the hardcoded
  `button_icon_rect(&n.data, &vs, &r, false)` is replaced by an `emit_icon`
  closure over the shared `crate::layout::button_icon_rect` authority
  (`src/layout.rs:261–310`). Case matrix verified against `button_icon_rect`
  semantics and the paint path (`src/platform/win32/render.rs:630–645`, which
  iterates `[(false, icon_start), (true, icon_end)]`):
  - `icon_only` → `emit_icon(false, "icon")` → canonical `icon`, centered
    rect, identical to paint.
  - both edges → `icon.start` + `icon.end`, deterministic; no populated edge
    can disappear.
  - single start → `emit_icon(false, "icon")` → canonical `icon`.
  - single end → `emit_icon(icon_start.is_none() == true, "icon")` →
    `button_icon_rect(..., true)` resolves the trailing edge → canonical
    `icon` (this was the reported `TARGET_NOT_FOUND` hole; now closed).
  - no icons → `button_icon_rect` returns `None` (`side.is_none() &&
    !icon_only`) → no part emitted.
- Frozen-contract compatibility: Gallery populates only single-edge inline
  icons (`examples/gallery.rs:229–235`) and icon-only buttons, so the
  canonical `icon` key is preserved for every contracted lookup; the
  `icon.start`/`icon.end` keys only appear for both-edge buttons that do not
  exist in `contract.json`.
- Test coverage: `devtools_parts_expose_populated_icon_edges_and_owner_prefix`
  (`src/tests.rs:8683–8775`, `#[cfg(all(windows, feature = "devtools"))]`)
  drives `Backend::dev_node` (widened to `pub(crate)`, `devtools.rs:43`) and
  asserts leading `icon` on the start edge, trailing `icon` resolved on the
  end edge (`x > 60`), `icon.start`/`icon.end` for both-edge, and
  `["pre", "pre.line[1]", "pre.line[2]", "pre.line[3]"]` exactly.
- Worker-reported live devctl proof (`icon-inline-start#icon` x≈281.7,
  `icon-inline-end#icon` x≈464.0, `pre.line[1..3]` at scale 1.25) is
  consistent with the code path verified above.

### `S03` — `Column::justify` honored via shared `justify_offset` policy — **CLOSED**

Evidence:
- `src/layout.rs:90–96` — new `pub fn justify_offset(justify, inner_main,
  used) -> f32` in platform-neutral core: `slack = (inner_main -
  used).max(0.0)`; `Start → 0.0`, `Center → slack/2`, `End → slack`. No
  `HWND`/`WM_*`/native types — layering per `docs/PLATFORM_CONTRACTS.md`.
- `src/platform/win32/layout.rs:474–486` — `KIND_COLUMN | KIND_CARD` now
  computes `used = Σsizes + gap·(n−1)` and starts
  `y = inner.y + crate::layout::justify_offset(props.justify
  .unwrap_or_default(), inner.height, used)`. `Column::justify`
  (`src/ui.rs:152–168`) is no longer silently ignored.
- `src/platform/win32/layout.rs:525–538` — `KIND_ROW` consumes the same
  function (the previous inline `match` was refactored without semantic
  change — identical formula, identical clamp).
- Invariants: `Start → 0.0` preserves the approved Gallery output (Gallery's
  only `justify` use is `Row::justify(Justify::End)` on the footer,
  `examples/gallery.rs:396` — Row behavior is pixel-identical). Overflow
  (`used > inner`) clamps `slack` to `0` → Start-equivalent placement, never
  negative origins. `Fill` children consume the axis inside `waterfill`
  before `justify_offset` runs → `slack = 0` → zero offset.
- Tests: `justify_offset_is_shared_row_column_policy` (`src/tests.rs:8526`,
  cross-platform — the +1 Linux test) covers Start/Center/End, overflow and
  exact-fit clamps, and Fill-consumes-axis. `column_and_row_justify_
  position_children_on_main_axis` (`src/tests.rs:8547`, `#[cfg(windows)]`)
  walks the real `LayoutCache::walk` (widened to `pub(crate)`,
  `win32/layout.rs:381`): Column ys `[0,20,40]`/`[120,140,160]`/
  `[240,260,280]`, overflow `[0,20,40]`; Row xs symmetric.
- Genericity: single arithmetic policy in shared code consumed by both axis
  branches — exactly what the finding required; no specimen special-casing.

### `S04` — Non-finite `Shadow.spread` guarded — **CLOSED**

Evidence (`src/platform/win32/render.rs:1264–1269`):
- The early guard is now
  `sa <= 0.0 || !offset_x.is_finite() || !offset_y.is_finite() ||
  !spread.is_finite()` → `Ok(())` no-op, matching the existing invalid-offset
  policy.
- Ordering verified: the guard precedes every `spread` consumer —
  `pad_dip = sigma*3 + spread.max(0.0)` (`render.rs:1280`), raster-dimension
  checks (`1284–1315`), `ShadowKey { spread: spread.to_bits() }`
  (`render.rs:1352`), and the signed-distance field `d - spread`
  (`render.rs:1414`). `NaN`/`±∞` spread can no longer reach cache identity,
  dimension math, blur sizing, or the distance field (the prior hole was
  `(f32::NAN).max(0.0) == 0.0` bypassing `pad_dip.is_finite()`).

## 3. Devctl Fixture Hygiene — **VERIFIED CLEAN**

- `examples/devctl_probe.rs` **deleted**; `examples/` retains only real apps:
  `composer.rs`, `gallery.rs`, `native_probe.rs`, `uia_probe.rs`.
- Fixture is a `[[bin]]` target `rust-ui-devctl-test-fixture`
  (`Cargo.toml:40–42`) at `tests/support/devctl_fixture.rs` — internal test
  infrastructure, no `required-features`; source self-gates on
  `cfg(all(windows, feature = "devtools"))` with a stub `main()` otherwise
  (`devctl_fixture.rs:10–67`), so Cargo always supplies
  `CARGO_BIN_EXE_rust-ui-devctl-test-fixture`.
- `tests/devctl_windows.rs:89` locates the binary via
  `env!("CARGO_BIN_EXE_rust-ui-devctl-test-fixture")` (whole test already
  `#![cfg(all(windows, feature = "devtools"))]`, so `env!` resolution is
  sound). Fixture body uses generic `probe.*` IDs — no Shadcn coupling.
- `docs/DEVCTL.md` updated: launch example now names
  `rust-ui-devctl-test-fixture.exe`, describes it as the internal test
  fixture that builds automatically via `CARGO_BIN_EXE_` (`DEVCTL.md:16`,
  `174`, `296–297`); it is not presented as a product app/example.
- Historical references to `devctl_probe` remain only in frozen evidence
  (`docs/DEVCTL_V0_1_IMPLEMENTATION_REPORT.md`,
  `docs/DEVCTL_V0_1_REWORK_REPORT.md`, `benchmark/results/rust-ui-devctl-
  v0.1-rework/*`) — correctly untouched historical record.

## 4. Gate Re-Execution (this worktree, Linux)

| Gate command | Result |
|---|---|
| `cargo check --locked --examples` | **PASS** — 0 errors; only pre-existing cfg dead-code warnings (untouched files) |
| `cargo test --locked --lib` | **PASS** — 108 passed / 0 failed (107 prior + new `justify_offset_is_shared_row_column_policy`; Windows-only tests cfg'd out, matching worker's 147/176 Windows counts) |
| `cargo fmt --check` | **FAIL — 1 hunk** (`tests/devctl_windows.rs:90`, see `C01`) |
| `git diff dc70f66..HEAD -- reference/` | empty — frozen reference intact |

Windows-only gates (`cargo.exe check`/`test` default + devtools, live devctl
semantic proof, 3-cycle integration) are evidence statements — structurally
consistent with the code as verified, not re-runnable here.

## 5. New Blocking Finding

### `C01` (`BLOCKING`, trivially bounded) — `cargo fmt --check` fails at HEAD; "fmt clean" evidence claim does not reproduce

- **Location:** `tests/devctl_windows.rs:90` —
  `assert!(exe.exists(), "internal devctl test fixture binary must exist");`
  (a 76-char single-line `assert!` with a message argument).
- **Reproduction:** `cargo fmt --check` exits 1 at `c8aa537` on
  rustfmt 1.8.0-stable — verified identically on both installed toolchains
  (`1.93.0`, `1.93.1`) and on all editions (2015/2021/2024) via a minimal
  repro: current rustfmt block-formats `assert!(cond, "msg")`. The base
  file at `8d3d273` (multi-line `assert!`) was fmt-clean — the violation was
  **introduced by the commit under review** when the assert was collapsed.
- **Why it blocks:** `cargo fmt --check` is a listed verification command for
  this repository (AGENTS.md) and was explicitly claimed clean in the
  worker's evidence — the claim is falsifiable at HEAD. Under Gates 16/19 a
  candidate whose listed gates do not all pass cannot be certified complete.
- **Required fix:** run `cargo fmt` (exactly one hunk reformats; no semantic
  change), then re-state the fmt gate honestly. Nothing else is required —
  all four prior findings are genuinely closed.

## 6. Universal Gates (1–20) Evaluation

| Gate | Status | Notes |
|---|---|---|
| **G1–G2** Scope & Architecture | **PASS** | Desktop-only Windows spike; policy in shared `src/layout.rs`, consumption in `platform/`; no native-type leaks; no forbidden backends/shims added. |
| **G3** Behavior Preservation | **PASS** | `Justify::Start` → 0 offset (Gallery pixels preserved); Row formula unchanged; canonical `icon` key for all contracted lookups; `spread = 0` shadow path unchanged. |
| **G4–G8** Contracts, Ownership & Reuse | **PASS** | `justify_offset` is the single shared Row/Column policy; icon parts reuse `button_icon_rect` shared with paint; no duplication of platform semantics. |
| **G9–G10** Deterministic Semantics | **PASS** | Column/Card now honor `justify`; devtools parts deterministic (`icon`, `icon.start`/`icon.end`, `pre.line[N]`); no populated edge can silently disappear. |
| **G11–G12** Failure Safety & Boundedness | **PASS** | Non-finite spread → typed no-op before all consumers; existing raster/byte bounds unchanged. |
| **G13** Dependency & Feature Scope | **PASS** | `Win32_Graphics_Dwm` owned by base Windows deps; devtools feature no longer carries runtime-required features; fixture bin self-gates. |
| **G14–G15** Performance & Event-Driven | **PASS** | No new loops/polling; part emission is on-demand devtool path. |
| **G16** Verification & Test Gate | **FAIL (`C01`)** | `cargo fmt --check` fails at HEAD (1 hunk); new targeted tests exist and pass where compilable (Linux 108/0; Windows claims plausible). |
| **G17–G18** Recovery & Artifact Identity | **PASS** | Frozen `reference/` untouched; evidence identifies exact candidate. |
| **G19** Completion Integrity | **FAIL (`C01`)** | The "cargo fmt --check clean" evidence claim is not reproducible at HEAD; all other claimed Linux/Windows evidence is consistent with verified code. |
| **G20** Systemic Completeness | **PASS** (given C01 fixed) | The four prior blockers closed generically; fixture hygiene complete; remaining defect is a single formatting hunk. |

## 7. Final Verdict

`S01`–`S04` and devctl fixture hygiene are **genuinely closed** — the fixes
are generic, correctly layered, contract-compatible, and covered by new
deterministic tests. No visual rework is implied (Start→0 keeps approved
pixels; parts/spread/Dwm changes do not alter rendering).

However, the candidate is not certifiable: `cargo fmt --check` — a listed
repository gate explicitly claimed clean — fails at HEAD on a one-line
regression introduced by the very commit under review. Approving would
certify a gate that falsifiably fails.

```text
SHADCN_NATIVE_GALLERY_SLICE1_SYSTEM_REWORK_REQUIRED
```

Remaining work is a single mechanical step: run `cargo fmt` (one hunk at
`tests/devctl_windows.rs:90`), confirm `cargo fmt --check` exits 0, and
re-issue the gate evidence. On completion, the closure evidence above
stands — no further system review deltas are anticipated.

```text
EXECUTION FOOTER

PROMPT_ID: RUI_ASTRA_P1-261019-234500
ROLE: Astra Reviewer
PROJECT: rceman/rust-ui
BRANCH: review/shadcn-native-gallery-slice1-system-closure-astra
STARTED_AT: 2026-10-19T23:47:00+03:00
FINISHED_AT: 2026-10-20T00:06:00+03:00
BASED_ON: 8d3d27361071ee8915dd8f496ea98e8e9b005f0a
PRODUCTION_HEAD: c8aa5371f039bcb4a9b84dfa4aad61f0edfa2f78
REPORT_HEAD: PENDING_COMMIT_SHA
STATUS: SHADCN_NATIVE_GALLERY_SLICE1_SYSTEM_REWORK_REQUIRED
NEXT_OWNER: Lead
```
