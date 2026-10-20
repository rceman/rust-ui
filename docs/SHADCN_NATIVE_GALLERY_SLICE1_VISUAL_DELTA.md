# Shadcn Native Gallery Slice 1 — Exact-SHA Visual Delta Review (Argon)

Independent delta-closure review. I did not write the code under review.
Scope: did `e49966d → c8d8147` change any approved visual output path, and
does the compact evidence support `VISUALLY_APPROVED` for this exact code.

## Identity

- Candidate production code: `c8d8147f4c9a6c63a0fe9c0288f03d31ea2f40ea`
- Evidence-bearing HEAD: `af2740d56718dbd701060153178fab4cac866d77` (evidence-only commit on top of c8d8147; `git diff c8d8147..af2740d` touches only `benchmark/results/…/visual-delta-c8d8147/`)
- Approved visual baseline: `e49966d529e6da5c2cacd4d88c3048dd58f8e640`
- Prior visual approval report: `3e9ebad7feef30c517292a72b60f0d6a363d89f9`
- System approval: `03fe57cbe2860b6d1725def34bd267ca067c21bf`
- Review worktree: `review/shadcn-native-gallery-slice1-visual-delta-argon` @ `af2740d`
- Diff range audited: `e49966d..c8d8147` — commits `0ebc88d`, `8d3d273`, `c8aa537`, `c8d8147` (ancestry verified via `git merge-base --is-ancestor`)

## 1. Diff audit — every changed file classified

`git diff e49966d..c8d8147 --name-status` = 16 entries, all audited hunk-by-hunk:

| File | Change | Pixel/layout-path verdict for approved specimens |
|---|---|---|
| `Cargo.toml` | `Win32_Graphics_Dwm` moved from `devtools` feature list to base `windows` dep features; `devctl_probe` example deleted, `rust-ui-devctl-test-fixture` bin added (`tests/support/devctl_fixture.rs`) | INERT — build-graph ownership only; no source semantics |
| `src/layout.rs` | new `justify_offset(justify, inner_main, used)`: `slack=(inner-used).max(0)` → Start 0 / Center slack/2 / End slack | INERT for approved output — pure function, only observable via non-Start justify (see §2) |
| `src/platform/win32/layout.rs` | `walk` → `pub(crate)`; KIND_ROW inline justify match replaced by `justify_offset` (identical semantics); KIND_COLUMN\|KIND_CARD gains `y = inner.y + justify_offset(props.justify…)` (Column::justify was silently ignored before) | INERT for approved output — Row byte-equivalent refactor; Column/Card offset is 0 for every container that exists (§2) |
| `src/platform/win32/devtools.rs` | `dev_node` → `pub(crate)`; owner `part_key` seeds `prefix` for `collect_parts` (`pre.line[N]` composition); icon-part emission per populated edge (`icon` for single edge at its true edge, `icon.start`/`icon.end` for both-edge; guard `has_icon` unchanged) | INERT — devtools SNAPSHOT metadata only; `dev_node`/`collect_parts` feed the devctl tree JSON, never paint or layout |
| `src/platform/win32/render.rs` | `draw_shadow` early-return adds `!shadow.spread.0.is_finite()` | INERT — fires only on NaN/±∞ spread; all Gallery/theme shadow spreads are finite constants |
| `src/tests.rs` | `spread: Dp(0.0)` added to 4 stale `Shadow` literals in Windows-cfg tests (the field already existed — literals were stale); 3 new focused tests (`justify_offset` policy, Column/Row walk justify, devtools icon-edge + prefix parts) | TEST-ONLY |
| `tests/devctl_windows.rs` | fixture binary path via `CARGO_BIN_EXE_rust-ui-devctl-test-fixture`; label expectation `Calibration tex` → `Fixture tex`; comments | TEST-ONLY |
| `tests/support/devctl_fixture.rs` | new self-gated test fixture binary (was `examples/devctl_probe.rs`) | TEST-ONLY — not the acceptance app |
| `examples/devctl_probe.rs` | deleted | TEST-ONLY removal — Gallery is `examples/gallery.rs`, untouched |
| `docs/DEVCTL.md`, `docs/COMPONENT_SUPPORT.md` | fixture-name docs; ledger note updates | DOCS |
| `benchmark/…/INDEX.md`, `titlebar/{proof.txt,window-*.png}` | evidence additions/status at `e49966d` (V01 titlebar proof; INDEX → READY_FOR_VISUAL_CLOSURE) | EVIDENCE — not production code |

No other production-code change exists in the range. `examples/gallery.rs`,
`src/style.rs`, `src/theme.rs`, `src/geom.rs`, `src/typography.rs`, `src/ui.rs`,
all recipe/brush/font/measure code: **untouched**.

## 2. `justify_offset` semantics — verified, not assumed

- `src/layout.rs:88-96`: `slack = (inner_main - used).max(0.0)`; `Start ⇒ 0` unconditionally (not merely when slack=0). Any container without an explicit non-Start justify places children at identical origins/sizes to the pre-delta code.
- KIND_ROW (`src/platform/win32/layout.rs:531-538`): the previous inline `match props.justify … { Start ⇒ 0, Center ⇒ slack/2, End ⇒ slack }` is verbatim the new shared call — provably equivalent arithmetic on identical inputs.
- KIND_COLUMN | KIND_CARD (`layout.rs:479-486`): new offset `inner.y + justify_offset(props.justify.unwrap_or_default(), inner.height, used)`. `props.justify` is `Option<Justify>` (`src/node.rs:291`); default `None` → `Start` → offset 0.
- Who can set `justify`? Only `Column/Row::justify(...)` builders and `Card::props()` — and `Card::props()` (`src/ui.rs:266-279`) leaves `justify: None`. `grep -rn '\.justify(' src/ examples/` returns exactly ONE call: `examples/gallery.rs:396 .justify(Justify::End)` on the card-footer **Row** — and Rows already honored justify pre-delta, so no behavior change there either.
- Consequence: every Column/Card/Row rect in the Gallery is bit-identical to `e49966d`. The only behavioral delta is for hypothetical non-Start Columns — none exist in the approved surface or anywhere in `src/`.

## 3. Frozen reference

`git diff dc70f6644c0384170f3a6929eb2caaf33fc711a0..HEAD -- reference/` is empty. Frozen contract untouched.

## 4. Independent snapshot-diff verification (recomputed, not trusting DELTA.md)

Parsed `snapshot-light-c8d8147.json` vs `snapshot-light-base8d3d273.json`
myself — full recursive tree walk comparing every field, every
`rect_dp`/`rect_px` (x/y/w/h), every `parts` array element-wise, and
children recursively:

- `schema`, `scale_factor` (1.25 both), `client_dp`, `client_px` identical.
- **53 automation IDs, identical sets and identical tree order.**
- **Exactly 2 diffs in the entire tree:**
  1. `button.default.icon-inline-end` `parts: [] → ["icon"]` — the documented S02 icon-edge fix (baseline emitted no icon part because the old code always probed the leading edge; `button_icon_rect(…, false)` returns `None` for a trailing-only button).
  2. `shell.nav.components` `hovered: false → true` — capture-time cursor state, not geometry and not a code output path (DELTA.md says the icon part is "the ONLY emitted-semantic delta" — strictly there are two; the hover flag is an input-state artifact of the live capture, benign).
- **0 geometry fields differ** — every `rect_dp` and `rect_px` in the tree is byte-identical. All other `parts` arrays identical.

Icon-part placement verified against the owner rect:
`button.default.icon-inline-end` owner `rect_dp = {x:387.28, y:298, w:101.72, h:32}` → right edge ≈489; `icon` part `rect_dp = {x:463.997, y:306, w:16, h:16}` → trailing edge inside right padding (border ~1 + inset ~9); `rect_px = {x:580, y:383, w:20, h:20}` = ×1.25 exact. `button.default.icon-inline-start` keeps its leading-edge `icon` part at `x:281.69` (owner `x:272.69` + 9 inset) — identical in both snapshots.

`typography.code-block` is **not** in any delta snapshot — all three capture
the Components page only (53 IDs; the code block lives on the Typography
page). Verified statically instead: `ui.code_block` (`src/ui.rs:1130-1143`)
stages `part("pre")` on the chrome box — the node that also receives the
`typography.code-block` automation_id via `ui.named` — plus `part("line[i]")`
on each of the Gallery's 3 lines. With owner-prefix seeding (`devtools.rs:111-132`,
`collect_parts` composes `prefix.join(".")`), the owner emits exactly
`["pre", "pre.line[1]", "pre.line[2]", "pre.line[3]"]` — matching both the
documented contract in `src/ui.rs:1104-1106` ("`pre.line[N]` the 1-based code
lines") and the new `devtools_parts_expose_populated_icon_edges_and_owner_prefix`
test assertion. Metadata-only either way; no pixel path involved.

## 5. PNG inspection

`gallery-light-c8d8147.png` / `gallery-dark-c8d8147.png` (962×853 RGBA both —
matches `client_px`). Decoded and inspected: coherent gallery structure
(header band, left nav column, button/IconButton grid, card row at bottom);
no blank/corrupt regions. Inter-PNG diff = 6,441 px confined to bbox
x:104–831, y:56–93 — the header band (hover/focus-ring state differences
consistent with the snapshot `hovered`/`focused` flags).

## 6. Evidence assessment — including defects found (adversarial notes)

1. **The "light" PNG is actually a dark-theme render.** Corners/center =
   (10,10,10) = `#0a0a0a` dark client; mean RGB ≈ 28.7 (dark PNG ≈ 28.4). The
   approved `titlebar/window-light.png` proves a true light client is white
   (center = 255,255,255). The light snapshot's focus state corroborates:
   `shell.theme.light` was never clicked (`focused:false`; focus sits on
   `shell.nav.components`), while the dark snapshot shows `shell.theme.dark`
   focused+hovered. The Gallery defaults to ThemeMode::System which resolved
   dark in that environment (per `titlebar/titlebar-proof.txt`). So the
   "light" capture is System-resolved-dark, and DELTA.md's "Light then Dark"
   label is inaccurate — no light-theme raster exists in the delta evidence.
   **Impact assessment: not a hiding place for a regression.** The diff
   contains zero theme/brush/style code, so a light-rendering bug cannot
   have been introduced; the snapshot comparison remains a valid
   same-state diff (rect_dp is theme-independent); and the approved e49966d
   evidence already covered light-theme raster. Defect is documentation
   accuracy, recommended for correction, non-blocking.
2. Cross-scale honesty: prior `el-*`/`titlebar` evidence is
   `scale_px_per_dp = 1.0`; delta captures are 1.25. PNG byte-compare across
   them is not apples-to-apples and was not used — dp-geometry is the basis.
3. `parts` key-set change beyond the icon addition: none in the captured
   surface. The `pre.line[N]` prefix composition is a metadata correction to
   the documented contract; both-edge `icon.start`/`icon.end` keys can only
   appear on a both-icon button, which the Gallery does not contain
   (`icon-inline-start` uses `.icon_start` only; `icon-inline-end` uses
   `.icon_end` only; icon_buttons are `icon_only` → unchanged `icon` key).
4. `reference/` frozen: verified empty diff (§3).
5. No required visual re-verification is outstanding: the range's only
   render-code touch is the non-finite `Shadow.spread` guard, unreachable by
   Gallery constants.

## Verdict

The `e49966d → c8d8147` production delta is **pixel-path inert** for every
approved specimen: the only functional layout change (Column/Card justify)
produces offset 0 for every container that exists; the Row path is a
same-arithmetic refactor; devtools changes are snapshot metadata; the render
guard fires only on non-finite input. My independent tree-wide snapshot diff
confirms 0 geometry deltas and exactly the documented `icon-inline-end#icon`
part addition (plus one benign capture-time hover flag). Evidence defects
noted above are accuracy/labeling issues in DELTA.md, not visual regressions.

`EXACT_SHA_VISUAL_DELTA_CLEARED`
`SHADCN_NATIVE_GALLERY_SLICE1_VISUALLY_APPROVED` — candidate
`c8d8147f4c9a6c63a0fe9c0288f03d31ea2f40ea`

## EXECUTION FOOTER

- PROMPT_ID: RUI_ARGON_P1-261020-003000
- ROLE: Argon Reviewer
- PROJECT: rceman/rust-ui
- BRANCH: review/shadcn-native-gallery-slice1-visual-delta-argon
- STARTED_AT: 2026-10-20 10:41:00 +0300
- FINISHED_AT: 2026-10-20 11:13:00 +0300
- BASED_ON: 8d3d27361071ee8915dd8f496ea98e8e9b005f0a
- PRODUCTION_HEAD: c8d8147f4c9a6c63a0fe9c0288f03d31ea2f40ea
- REPORT_HEAD: PENDING_COMMIT_SHA
- STATUS: EXACT_SHA_VISUAL_DELTA_CLEARED + SHADCN_NATIVE_GALLERY_SLICE1_VISUALLY_APPROVED (c8d8147)
- NEXT_OWNER: Lead
