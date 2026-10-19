# Shadcn Native Gallery — Slice 1 Visual Rework Report

Worker run `RUI-W-261019-1440`, branch
`agent/shadcn-native-gallery-slice1-opus55`, authoritative checkout
`/home/therceman/git/rust-ui`. Base `8247984dfb7db0f7d94d2eb8f03fbc8910d04ef2`.
Closes the six blocking findings from the independent visual review
`75f24a6e2bd7a8b9f3d80cb13f9d23f4f3c651cf` on
`docs/SHADCN_NATIVE_GALLERY_SLICE1_VISUAL_REVIEW.md`.

No Slice 2 work. Review branch untouched. Frozen reference
(`reference/shadcn-gallery-v0.1`, publication `dc70f66`, payload `2e14e41`)
unmodified — verified via `git status -- reference/` (clean).

## V01 — Windows dark non-client title bar — **CLOSED**

Root cause: `DWMWA_USE_IMMERSIVE_DARK_MODE` was never set, so the caption
stayed light under a dark client.

Fix (platform boundary only): `Backend::sync_dark_caption(dark)` in
`src/platform/win32/mod.rs` calls
`DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, …)` once per
resolved-theme change on the existing HWND — no recreate, no custom-drawn
caption. Driven by the generic resolved `theme.dark` at the runtime pump;
Shadcn concepts stay out of platform code.

Proof (DwmGetWindowAttribute attr 20 + full-window `CopyFromScreen`
captures incl. non-client chrome):
- Dark / System→dark: `DWMDARK = 1`, dark caption over `#0a0a0a` client.
- Light: `DWMDARK = 0`, light caption over the light client.
- Runtime toggle `shell.theme.dark ↔ light` flips the existing caption.

## V02 — inline icon button layout — **CLOSED**

`button_content_insets` (`src/layout.rs`) now reserves
`border + reduced-side padding + icon_size + gap` on each icon-bearing
edge; the natural-width measure (`src/platform/win32/layout.rs`) is
`text_ink + insets` — one centered content group, no independent label
centering, no icon/label collision. Verified: `icon-inline-start`
`102.6×32`, `icon-inline-end` `101.7×32` (frozen `100.563/101.109×32`,
≤2dp glyph-measure noise), 16dp icon + 6dp gap, no overlap.

## V03 — invalid button ring — **CLOSED**

Paint gate `render.rs` changed to `(focus || invalid) && !disabled`, so
the outer destructive ring renders when unfocused, resolved per
variant/theme (`DestructiveRing` recipe: light `destructive/20`, dark
`destructive/40`). All six variants show the ring in both themes —
destructive-variant vs invalid-state distinction preserved.

## V04 — card outer ring / `Shadow.spread` — **CLOSED**

`draw_shadow` (`src/platform/win32/render.rs`) now honors `spread`:
positive spread expands the outer raster pad, joins `ShadowKey`, and the
mask builds from signed distance `d − spread` against the unexpanded
interior. The card's `0 0 0 1px` ring (`foreground/10`) now renders a
visible edge on light and dark — generic renderer semantics, no
card-specific special-casing.

## V05 — card structure / part parity — **CLOSED**

- `KIND_ROW` honors `props.justify` (`Start/Center/End`) — footer buttons
  right-align generically, not via Card coordinates.
- Card part recipes centralized in `src/style.rs` (`card_part_insets`,
  `card_title_style`, `card_content_style`, `card_footer_style`,
  `card_part_box`, `card_spacing`), consumed by `card_parts()` in
  `examples/gallery.rs`:
  - header `--card-spacing` horizontal insets, no default vertical pad
  - title `16/22` medium (Default) / `14/19.25` medium (Sm)
  - description muted; content `14/20` `CardForeground` at spacing inset
  - footer `bg-muted/50` + 1dp `border-t` + spacing padding, `Sm` buttons
    `Justify::End`
- Semantic parts `header`/`header.title`/`header.description`/`content`/
  `footer` preserved via nested `ui.part`.
- `card.default` `288×175`, `card.default.size-sm` `288×152.25` — both
  now exactly match frozen (previously −4.38 / +6.37).

## V06 — typography parity — **CLOSED**

- Specimen copy replaced with exact frozen text (all 10 specimens).
- `Typography::InlineCode` → `13px` mono semibold lh20 on muted chrome;
  content-hugging (`width(Content)` + a non-stretch row wrapper so the
  Stretch column can't inflate it). Measures `143.5×26.4`.
- `ui.code_block` — removed accidental `8dp` column pad/gap; `14/16`
  chrome insets own the geometry (`720×88`); mono `14/lh20`
  `CodeForeground` (`#a1a1a1` dark); parts `pre` + `pre.line[1..3]`.
- Content column is a real `720` box (wrapped in a non-stretch row so the
  page's `Stretch` column can't override `spec.width`); inner column
  `padding(None)`.
- **Systemic renderer bug fixed**: `layout_for`'s `GetLineMetrics` used a
  fixed 1-element buffer; any wrapped (2+ line) label errored
  `E_NOT_SUFFICIENT_BUFFER` → measured `(0,20)` and painted nothing. Now
  queries `count` then reads a sized buffer. Restores h1 (`720×96`, two
  lines), paragraph, and lead — the genuinely-wrapping specimens.

## V07 / V08 / V09 — non-blocking statuses

- **V07** (bg-clip / pressed +1dp): `UNCHANGED` — out of this rework's
  safe scope; still documented.
- **V08** (+4dp hardcoded slack): `INCIDENTALLY_CLOSED` — the `+4.0`
  measure slack is removed; width is `ink + insets` plus a documented 2dp
  ink-vs-layout epsilon (not per-label). Button labels also get
  `DWRITE_WORD_WRAPPING_NO_WRAP` (`whitespace-nowrap`) so a tight box can
  never wrap — the `"Size sm"` two-line defect.
- **V09** (oversized window / scroll): `UNCHANGED` — normal desktop
  window retained; scrollable page container not introduced in this
  bounded rework.

## Tests

5 focused recipe tests added (`src/tests.rs`, now 107 passing):
`button_content_insets_reserves_icon_space`,
`invalid_button_gets_destructive_ring_recipe`,
`card_recipe_shadow_has_unit_spread_ring`,
`card_part_recipes_match_frozen_contract`,
`inline_code_recipe_is_13px_and_wraps_content`.

## Gates

`cargo fmt --check` clean · `cargo check --examples` 0 errors ·
`cargo test --lib` 107/107. No CI.

## Evidence

`benchmark/results/shadcn-native-gallery-slice1/el-*` — updated
`reference.png`/`native-raw.png`/`native-canonical.png`/`diff.png`/
`comparison.json` for the affected specimens (`card.*`,
`button.default.icon-inline-*`, `button.default.invalid`,
`button.default.size-sm`, `typography.h1/.inline-code/.code-block`) via
`rust-ui-devctl compare-element --mode size-only --tolerance 0.25
--channel-tolerance 2`. Whole-window title-bar proof captured via native
screen capture (client + non-client) — devctl WGC stays component
authority. Ledger kept at `IMPLEMENTED` (not `VERIFIED`) — independent
visual closure decides that.

## Status

```text
V01 CLOSED · V02 CLOSED · V03 CLOSED · V04 CLOSED · V05 CLOSED · V06 CLOSED

SHADCN_NATIVE_GALLERY_SLICE1_READY_FOR_VISUAL_CLOSURE
```

(Not final visual approval — readiness for independent closure.)
