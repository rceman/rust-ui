# Pre-Slice-3 Visual Correction — Full-Page Evidence

Corrected full-page captures of the native Gallery after the
`PRE_SLICE3_VISUAL_CORRECTION` milestone. Compare against
`benchmark/results/shadcn-native-gallery-post-slice2-fullpages/` (the
pre-correction inventory) and `reference/shadcn-gallery-v0.1/` (frozen
authority).

## Identity

- Repository: `rceman/rust-ui`
- Production SHA captured: `a2c9c52aa873cdf28ccc5f9f95c8503e2d71b6c1` (`agent/pre-slice3-visual-correction`,
  on top of `07a0782b26ab5c0f7abd6c6e29ba7a7b4a8f5c1a` = published `origin/main`)
- Application: `examples/gallery` built with `cargo.exe --features devtools
  --locked` on native Windows.
- Evidence branch: `agent/pre-slice3-visual-correction` (correction + evidence)

## Environment

- OS: Windows NT 10.0.26200 (Windows 11)
- Display: `\\.\DISPLAY2` — 1440×2560 vertical (secondary); primary
  `\\.\DISPLAY1` 3072×1728
- DPI: 96 → scale factor **1.0** (1 dp = 1 px)
- Capture backend: `Windows.Graphics.Capture` via devctl `screenshot`
  (client-content PNGs); `PrintWindow(PW_RENDERFULLCONTENT)` for the two
  full-window captures

## Window / client geometry

- Outer window: **980 × 1600 px** on DISPLAY2
- Client area: **964 × 1561 px** (964 × 1561 dp @ scale 1.0)
- One common size for all pages: **YES**
- Every implemented page fits without scrolling: **YES**
- Scroll offset: top (0). Stitched screenshots: **NONE**

## Pages

| Page | Nav id | Light | Dark |
|---|---|---|---|
| Components | `shell.nav.components` | `light/components.png` | `dark/components.png` |
| Forms | `shell.nav.forms` | `light/forms.png` | `dark/forms.png` |
| Typography | `shell.nav.typography` | `light/typography.png` | `dark/typography.png` |
| Native Text | `shell.nav.native-text` | `light/native-text.png` | `dark/native-text.png` |

Stub pages (`All`, `Navigation`, `Overlays`) intentionally omitted — they
route to `stub_page`. `window/light.png` + `window/dark.png` show the
complete native window including title bar (Native Text page shown).

## Corrected items verified in these captures

- **Multiline Native Text 3-line specimens** — all three lines fully
  rendered, no glyph clipping, both themes. Root cause: fixed-height 78dp
  specimens drew at RichEdit's natural ~25px pitch (75px of content in a
  62dp content rect). Fix: authored `.line_height(dp(20))` →
  `PeerConfig.line_twips` → calibrated `CHARFORMAT.yHeight` under
  `TXTBIT_D2DDWRITE` (paragraph line-spacing is inert in that mode);
  live-doc `EM_POSFROMCHAR` confirms line origins at y = 0/20/40 px.
- **Visual-state staging (`Presented`)** — forced hover / pressed /
  focus-visible specimens resolve as `runtime || presented` for paint and
  state-dependent layout only. Runtime focus, hit-testing, UIA focus and
  devtools runtime fields are unchanged. Multiple focus-visible specimens
  render simultaneously with zero OS focus.
- **Button / IconButton state grids** — all six cells distinct per variant:
  normal, hover, pressed (1px `translate-y`), focus-visible (ring),
  disabled, invalid (destructive ring).
- **Input / Textarea / Checkbox / Radio / Switch focus-visible** —
  independently staged on Forms + Native Text pages.
- **Contour-following focus/invalid rings** — outer radius derived from
  the component's own corner radius + 3px spread (no pillified rings).
  Invalid ring takes precedence over focus-visible (single ring,
  destructive/20%-style color).
- **Native selection specimens** — single-line `Select` and multiline
  `Second line` ranges visibly highlighted via the real windowless
  RichEdit peer (UTF-16 staged selection), without stealing OS focus.
  Dark theme uses the light-neutral selection colors.

## Gates (this candidate)

- Linux: `cargo fmt --check` clean; `cargo check --locked --examples`;
  `cargo test --locked --lib` — **114/114**
- Windows default: `cargo.exe test --locked --lib` — **156/156**
- Windows devtools: `cargo.exe test --locked --lib --features devtools` —
  **185/185**
- Windows `cargo.exe test --locked --test devctl_windows --features devtools`
  — **1/1**
- Bounded resize sanity: `render_resize_max_us ≈ 399µs` across 3 resize
  steps, 0 text-layouts per resize — Slice-2 resize fix preserved.

## Known capture artifacts / honest notes

- `window/*.png` shows a ~4px dark edge ring — `PrintWindow` frame
  artifact, not a rendering defect. Title bar follows app theme
  (light/dark) correctly. The staged selection also renders in classic
  OS highlight blue under `PrintWindow`'s re-render path — a known
  PrintWindow divergence, not the D2DDWRITE blend output.
- The native Gallery is intentionally more compact than the HTML frozen
  catalog (section cards/specimen grids are a deferred layout decision —
  out of scope for this correction).
- `EM_REQUESTRESIZE` (natural extent) stays stale under `D2DDWRITE`; the
  drawn pixel truth and `EM_POSFROMCHAR` positions are authoritative —
  recorded in `docs/SPIKE_ARCHITECTURE_DEVIATIONS.md`.
- Dark-theme selection bands show a ~1px brighter strip at the band's
  left edge (caret/AA at the selection anchor) — present pre- and
  post-fix; cosmetic.
