# Visual delta evidence — e49966d/8d3d273 baseline vs c8d8147

Production candidate: `c8d8147f4c9a6c63a0fe9c0288f03d31ea2f40ea`
Baseline (byte-identical render/layout code to approved `e49966d`): `8d3d27361071ee8915dd8f496ea98e8e9b005f0a`
Both captured live via rust-ui-devctl on `\\.\DISPLAY2` (portrait), scale 1.25, Light then Dark.

## Same-scale snapshot diff (Components page, Light)

53 common automation IDs, **0 geometry fields differ** (`rect_dp` x/y/w/h all identical).
The ONLY emitted-semantic delta: `button.default.icon-inline-end` now exposes the S02-fixed `icon` part at its true trailing position (baseline emitted none). All other `parts` arrays identical.

## Contract geometry spot-checks (compare-layout, tolerance 0.25)

| id | theme | result |
|---|---|---|
| card.default | light+dark | 288×175 exact, PASS |
| card.default.size-sm | light+dark | 288×152.25 exact, PASS |
| icon-button.default | light | 32×32 exact, PASS |
| typography.h1 | light | 720×96 exact, PASS |
| typography.code-block | light | 720×88 (contract 728×88) — same delta as approved e49966d evidence |
| typography.inline-code | light | 143.46×26.4 content-hugging — same as approved evidence |
| button.default / icon-inline-start / icon-inline-end | light+dark | wΔ 0.5–2.0dp — the same glyph-measure deltas present in the approved e49966d evidence (scale-1.0 captures) |

## Why no pixel-path delta

`git diff e49966d..c8d8147` touches only: Cargo feature ownership (build graph), devtools parts extraction (metadata, not paint), `justify_offset` (Row/Column policy; Gallery uses Start → offset 0 → identical origins), a non-finite `Shadow.spread` guard (Gallery spreads are finite), tests, and the devctl fixture relocation. No brush, recipe, text, or geometry path for any approved specimen changed.

Files: `snapshot-light-c8d8147.json`, `snapshot-dark-c8d8147.json`, `snapshot-light-base8d3d273.json`, `gallery-light-c8d8147.png`, `gallery-dark-c8d8147.png`.
