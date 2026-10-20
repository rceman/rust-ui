# Shadcn Native Gallery — Slice 1 parity evidence

Candidate: `c8d8147f4c9a6c63a0fe9c0288f03d31ea2f40ea`
(branch `agent/shadcn-native-gallery-slice1-opus55`, authoritative checkout
`/home/therceman/git/rust-ui`)

Status: `SHADCN_NATIVE_GALLERY_SLICE1_PUBLISHED`

Prior identities — rework base `8247984`, production `cbb39d31985567861a68fec39f22b95728310e36`,
independent visual review `75f24a6e2bd7a8b9f3d80cb13f9d23f4f3c651cf`.

Native capture: `examples/gallery` on the win32 backend, light theme
(`shell.theme.light`), client 964×861 @ scale 1.0 on the secondary monitor.
Reference: `reference/shadcn-gallery-v0.1/contract.json` +
`screenshots/light/components.png` (frozen).

## Method

`rust-ui-devctl compare-element --mode size-only --tolerance 0.25
--channel-tolerance 2` per specimen — geometry (contract authority) plus
canonical raster. One `el-<id>/` dir per element: `reference.png`,
`native-raw.png`, `native-canonical.png`, `comparison.json`, `diff.png`.

## Findings (geometry, `size-only`)

| specimen | width Δ | height Δ | note |
|---|---|---|---|
| all `button.*` | +0.4 … +4.0 | 0.0 | text-measure noise (below) |
| all `button.default.size-*` | +0.0 … +1.3 | 0.0 | heights exact after row-align fix |
| all `icon-button.*` | 0.0 | 0.0 | exact |
| `card.default` | 0.0 | 0.0 | fixed — V05 card part recipes |
| `card.default.size-sm` | 0.0 | 0.0 | fixed — V05 card part recipes |

- **Height** is exact for every size specimen — the `cross_v` row-align
  fix (`src/platform/win32/layout.rs`) removed a stretch-to-max bug; all
  `size-*` / `size-icon-*` specimens report `dh = 0.0`.
- **Width** deltas are +0.4 … +4.0 dp on text-bearing elements — the known
  DirectWrite↔Chromium variable-font (Geist) measure difference, not a
  layout defect. Empty icon buttons measure 0-width-delta.
- **Raster** is a visual signal (Chromium↔D2D glyph pixels differ by
  design); `compare-element` marks `whole_component_raster` measured but
  glyphs `not_measured`.

## Status

`SHADCN_NATIVE_GALLERY_SLICE1_PUBLISHED` — code frozen at `c8d8147`, system
and visual authorities both approved the exact SHA, evidence committed.
See `docs/SHADCN_NATIVE_GALLERY_SLICE1_FREEZE.md`.
