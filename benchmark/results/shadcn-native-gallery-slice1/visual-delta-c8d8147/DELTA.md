# Visual delta evidence — e49966d/8d3d273 baseline vs c8d8147

Production candidate: `c8d8147f4c9a6c63a0fe9c0288f03d31ea2f40ea`
Baseline (byte-identical render/layout code to approved `e49966d`): `8d3d27361071ee8915dd8f496ea98e8e9b005f0a`
Captured live via rust-ui-devctl on `\\.\DISPLAY2` (portrait), scale 1.25. Gallery defaults to System theme; `shell.theme.light`/`shell.theme.dark` were clicked explicitly for the Light/Dark captures.

## Same-scale snapshot diff (Components page, Light)

53 common automation IDs, **0 geometry fields differ** (`rect_dp`/`rect_px` all identical between baseline and candidate).

The emitted-semantic deltas are exactly:
- `button.default.icon-inline-end` gains `parts: ["icon"]` at its true trailing position (x≈463.997 for owner x≈387.28 w≈101.72) — this IS the S02 fix; baseline emitted no icon part there (hardcoded leading edge returned `None`).
- one benign `hovered` flag difference on `shell.nav.components` — capture-time cursor state, not rendering.

`button.default.icon-inline-start` emits `icon` identically in both (leading edge, x≈281.7).

## Live semantic-part proof (candidate, devctl session)

- `button.default.icon-inline-end` → `parts: ["icon"]` at trailing edge
- `typography.code-block` → `parts: ["pre","pre.line[1]","pre.line[2]","pre.line[3]"]` (see `snapshot-typography-light-c8d8147.json`)

## Contract geometry spot-checks (compare-layout, tolerance 0.25)

| id | theme | result |
|---|---|---|
| card.default | light+dark | 288×175 exact, PASS |
| card.default.size-sm | light+dark | 288×152.25 exact, PASS |
| icon-button.default | light | 32×32 exact, PASS |
| typography.h1 | light | 720×96 exact, PASS |
| typography.code-block | light | 720×88 (contract 728×88) — same delta as approved e49966d evidence |
| typography.inline-code | light | 143.46×26.4 content-hugging — same as approved evidence |
| button.default / icon-inline-start / icon-inline-end | light+dark | wΔ 0.5–2.0dp — the same glyph-measure deltas present in the approved e49966d evidence |

Note on scales: the committed `el-*` evidence was captured at scale 1.0 (`native_raw.scale_px_per_dp` = 1.0); these delta captures are at 1.25, so PNG bytes are not comparable across the two sets — dp geometry is the delta authority here (verified byte-identical).

## Why no pixel-path delta

`git diff e49966d..c8d8147` touches only: Cargo feature ownership (build graph), devtools parts extraction (metadata, not paint), `justify_offset` (Row/Column policy; Gallery uses Start → offset 0 → identical origins), a non-finite `Shadow.spread` guard (Gallery spreads are finite), tests, and the devctl fixture relocation. No brush, recipe, text, or geometry path for any approved specimen changed. The only `.justify()` call in the repo is `Justify::End` on a **Row** in `examples/gallery.rs:396`, and Rows already honored justify before — zero visual delta.

Files: `snapshot-light-c8d8147.json`, `snapshot-dark-c8d8147.json`, `snapshot-light-base8d3d273.json`, `snapshot-typography-light-c8d8147.json`, `snapshot-typography-dark-c8d8147.json`, `gallery-light-c8d8147.png` (true light render), `gallery-dark-c8d8147.png`.
