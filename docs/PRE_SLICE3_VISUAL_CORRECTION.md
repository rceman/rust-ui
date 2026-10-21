# Pre-Slice-3 Visual Correction

Focused correction milestone on the published Slice-2 Gallery — closing
visual-contract gaps exposed by the post-Slice-2 full-page inventory.
**No Slice-3 components implemented** (Select/Tabs/Dropdown/SettingsNav/
Tooltip/Popover/Dialog remain unimplemented by design).

## Identity

- Base `origin/main`: `07a0782b26ab5c0f7abd6c6e29ba7a7b4a8f5c1a`
- Reviewed production SHA: `a2c9c52aa873cdf28ccc5f9f95c8503e2d71b6c1`
- Branch: `agent/pre-slice3-visual-correction`
- Evidence: `benchmark/results/shadcn-native-gallery-pre-slice3-correction/`

## Root causes and fixes

### 1. Multiline Native Text third-line clipping

The frozen `native-text.multiline.*` specimens are 360×78 with a 62dp
content region. RichEdit under `TXTBIT_D2DDWRITE` ignores every paragraph
line-spacing control (`EM_SETPARAFORMAT`/`PARAFORMAT2` `PFM_LINESPACING`,
TOM `ITextPara::SetLineSpacing` — all inert or `E_FAIL`) and derives the
line pitch from `CHARFORMAT.yHeight` alone. The default Segoe UI 14px
face yields ~25px pitch → 3 lines = 75px > 62dp → mid-glyph clip.

Fix: authored `line_height` (Dp) flows node → `PeerSpec` →
`PeerConfig.line_twips` → `CHARFORMAT.yHeight = line_twips * 23/30`
(calibrated on the real host; `EM_POSFROMCHAR` verifies drawn line
origins at exactly 0/20/40px for `dp(20)`). Honest note:
`EM_REQUESTRESIZE` stays stale in this mode; drawn pixel truth and
POSFROMCHAR are authoritative.

### 2. Visual-state staging (`Presented`)

`Node.presented` (bitflags HOVER/PRESSED/FOCUS_VISIBLE) is resolved as
`runtime || presented` for paint and state-dependent layout only. Runtime
focus, hit-testing, keyboard routing, UIA focus and devtools runtime
fields (`focused`/`hovered`/`pressed`) are untouched — the snapshot
exposes `presented` separately. Forced focus-visible specimens cannot
grant interactivity.

### 3. Ring geometry

Focus/invalid rings were previously drawn from a generic radius,
producing pillified outer contours. The shared rule is now:
outer corner radius = component corner radius + 3px spread (matches the
frozen `box-shadow` ring semantics) applied to every control family.
Ring precedence is centralized: `ring_color_role(invalid)` — the invalid
ring wins over focus-visible as a single ring (not stacked).

### 4. Staged native selection + contract band colors

UTF-16 `[start,end)` ranges on editor nodes are applied to the real
windowless RichEdit peer via TOM selection APIs on mount and after text
sync — renders without requiring OS focus. A missing
`PeerHandle::apply_selection` forwarder (trait default no-op) was the
live-path bug; fixed + regression tests `staged_selection_survives_first_activation`
and `selection_band_renders_contract_dark`. Selection colors carry the
frozen contract tokens; under `TXTBIT_D2DDWRITE` the band renders
`0.4·COLOR_HIGHLIGHT + 0.6·COLOR_WINDOW` (UI activation inert, verified),
so `COLOR_WINDOW`/`host.win_bg` = `#010101` — the black band lands
near-solid instead of being skipped (`highlight==window`) or washed to
`#9b9b9b`.

## Gates

- Linux: `cargo fmt --check` clean; `cargo check --locked --examples`;
  `cargo test --locked --lib` — **114/114**
- Windows default: `cargo.exe test --locked --lib` — **156/156**
- Windows devtools: `cargo.exe test --locked --lib --features devtools` — **185/185**
- `cargo.exe test --locked --test devctl_windows --features devtools` — **1/1**
- Resize sanity: `render_resize_max_us ≈ 399µs`, 0 text-layouts per
  resize step — Slice-2 bounded resize preserved.

## Reviews

- Astra (system): `PRE_SLICE3_VISUAL_CORRECTION_SYSTEM_APPROVED` on
  `a2c9c52aa873cdf28ccc5f9f95c8503e2d71b6c1` — round-1 findings
  (focus-repaint ordering, deviation-doc entry, builder docs, selection
  clamp/composition gate) all closed; residual items are cosmetic
  non-blockers.
- Argon (visual): `PRE_SLICE3_VISUAL_CORRECTION_VISUALLY_APPROVED` on
  `a2c9c52aa873cdf28ccc5f9f95c8503e2d71b6c1` — F-1 (invisible light
  selection) closed by the `win_bg` blend-base fix; all other checklist
  items stand.

## Statuses

- `PRE_SLICE3_VISUAL_CORRECTION_SYSTEM_APPROVED` — `a2c9c52aa873cdf28ccc5f9f95c8503e2d71b6c1` (Astra)
- `PRE_SLICE3_VISUAL_CORRECTION_VISUALLY_APPROVED` — `a2c9c52aa873cdf28ccc5f9f95c8503e2d71b6c1` (Argon)
