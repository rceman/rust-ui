# Shadcn Native Gallery Slice 1 — Overnight Autonomous Closure

`PROMPT_ID: RUI_LEAD_P1-261019-223000` — took Slice 1 from
`SHADCN_NATIVE_GALLERY_SLICE1_READY_FOR_VISUAL_CLOSURE` all the way through
system rework, independent review, publication, devctl fixture hygiene, and
`main` integration. This is the auditable record; the canonical freeze
record is `docs/SHADCN_NATIVE_GALLERY_SLICE1_FREEZE.md`.

## Initial identities

- Base: `8d3d27361071ee8915dd8f496ea98e8e9b005f0a` (docs-only descendant of
  approved `e49966d529e6da5c2cacd4d88c3048dd58f8e640`)
- Branch: `agent/shadcn-native-gallery-slice1-opus55`
- Prior system review `fe55115037bbe60843866de33049c67f9f729eaa` →
  `SHADCN_NATIVE_GALLERY_SLICE1_SYSTEM_REWORK_REQUIRED` (findings S01–S04)

## Fixes applied

- **S01** `Cargo.toml`: `windows/Win32_Graphics_Dwm` moved from
  `[features].devtools` → `[target."cfg(windows)".dependencies].windows.features`;
  5 stale Windows `Shadow{}` literals in `src/tests.rs` given `spread: Dp(0.0)`.
- **S02** `src/platform/win32/devtools.rs`: `dev_node` emits one icon part per
  populated edge via shared `button_icon_rect` (single edge keeps canonical
  `icon`; both-edge → `icon.start`/`icon.end`; icon-only → centered `icon`);
  owner `part_key` seeds `collect_parts` prefix → `pre.line[N]`.
- **S03** `src/layout.rs` + `src/platform/win32/layout.rs`: shared
  `justify_offset(justify, inner_main, used)` policy consumed by both
  `KIND_ROW` and `KIND_COLUMN|KIND_CARD` — `Column::justify` no longer a
  silent no-op. Overflow clamps slack to 0; `Fill` consumes slack first;
  `Start` → offset 0 (approved output unchanged).
- **S04** `src/platform/win32/render.rs`: `!shadow.spread.0.is_finite()` added
  to `draw_shadow`'s early guard — non-finite spread can never reach cache
  dimensions / distance-field / blur math.

## devctl fixture migration

- `examples/devctl_probe.rs` deleted → `tests/support/devctl_fixture.rs`
- New internal `[[bin]] rust-ui-devctl-test-fixture` (self-gated
  `cfg(all(windows, feature="devtools"))`, stub `fn main(){}` otherwise so
  `CARGO_BIN_EXE_` resolves on every configuration)
- `tests/devctl_windows.rs` locates it via
  `env!("CARGO_BIN_EXE_rust-ui-devctl-test-fixture")`; asserts on the
  renamed draft text
- `docs/DEVCTL.md` updated — no instructions to build/launch a probe app;
  `examples/gallery` remains the sole UI acceptance application

## Gate results

- Linux/WSL: `cargo fmt --check` PASS; `cargo check --locked --examples` PASS;
  `cargo test --locked --lib` **108/0**
- Windows default (`cargo.exe`, NTFS target): `check --locked --examples` PASS;
  `test --locked --lib` **147/0**
- Windows `--features devtools`: `check --locked --examples` PASS;
  `test --locked --lib` **176/0** (incl. new `devtools_parts_*`,
  `column_and_row_justify_*`, `justify_offset_*` tests);
  `--test devctl_windows` PASS (3 complete launch/capture/shutdown cycles)
- Live Gallery proof (scale 1.25): `button.default.icon-inline-end#icon`
  resolves at trailing edge; `typography.code-block` →
  `pre, pre.line[1], pre.line[2], pre.line[3]`

## Independent reviews (fresh contexts, exact SHA)

- **Astra** `RUI_ASTRA_P1-261019-234500` → report `86510af` initially
  `REWORK_REQUIRED` (new finding C01: fmt regression in `devctl_windows.rs`),
  fixed by `c8d8147` → closure report `03fe57cbe2860b6d1725def34bd267ca067c21bf`
  = `SHADCN_NATIVE_GALLERY_SLICE1_SYSTEM_APPROVED` + `SLICE1_PUBLICATION_CLEARED`
- **Argon** `RUI_ARGON_P1-261020-003000` → report
  `f8d6bd0455c7d6d54b8dbf32108bb161a3a6bf55` =
  `EXACT_SHA_VISUAL_DELTA_CLEARED` + `SHADCN_NATIVE_GALLERY_SLICE1_VISUALLY_APPROVED`

## Visual-delta methodology

Same-scale (1.25) live snapshot diff of true `8d3d273` baseline vs `c8d8147`:
53 automation IDs, **0 geometry diffs**; only deltas = S02 `icon` part addition
(baseline emitted none at end edge) + one capture-time `hovered` flag. Pixel
path provably inert — no brush/recipe/text/geometry change. Compact evidence
committed under `benchmark/results/shadcn-native-gallery-slice1/visual-delta-c8d8147/`.

## Retries / intermediate failures (not hidden)

- Two transient `CAPTURE_FAILED` runs of `devctl_windows` (scene changed
  during WGC — environment race; third run fully green, reproducible).
- Astra C01 `fmt` regression (bin rename landed after my earlier `cargo fmt`)
  — fixed by `c8d8147`, independently re-verified.
- Argon noted the first delta evidence commit mislabeled a dark PNG as
  "light" and lacked a typography snapshot — corrected by `edf81a8`
  (true light render + `snapshot-typography-*` files added).
- Two invalid "baseline" capture attempts (WSL `/tmp` UNC not reachable from
  Windows → PowerShell ran in the main worktree) before the true `8d3d273`
  baseline was built from an NTFS copy — then the snapshot diff was honest.

## Final identities

- Final code SHA: `c8d8147f4c9a6c63a0fe9c0288f03d31ea2f40ea`
- Final publication SHA: this commit's parent chain tip on
  `agent/shadcn-native-gallery-slice1-opus55`
- Final `main` SHA: recorded at merge time (post-merge sanity confirms
  `origin/main` == local `main`)

## Component ledger

Button, IconButton, Card/Surface, Typography → `VERIFIED`
(`docs/COMPONENT_SUPPORT.md`). macOS/Linux cells unchanged (`-`); interaction
parity stays `partial`; accessibility stays `foundation UIA` — no overclaim.

## Remaining non-blocking follow-ups

- V07 `bg-clip-padding` / pressed +1dp crop (known, accepted at e49966d)
- V09 future scrollable page viewport requirement

No Slice 2 code was started. Frozen reference `reference/shadcn-gallery-v0.1/`
unchanged throughout (verified `git diff dc70f66..HEAD -- reference/` empty).
