# Post-Slice-2 Full-Page Gallery Inventory

Complete visual inventory of the published `rceman/rust-ui` Gallery after Slice 2,
captured for Slice-3 planning. **Evidence-capture only** — no fixes, no redesign,
no Slice-3 work.

## Identity

- Repository: `rceman/rust-ui`
- Production SHA captured: `07a0782b26ab5c0f7abd6c6e29ba7a7b4a8f5c1a` (`origin/main`)
- Application: `examples/gallery` (the real native acceptance app — not the devctl
  fixture)
- Source checkout built: `/home/therceman/git/rust-ui-evidence` (worktree at the
  exact SHA above), built with `cargo.exe --features devtools --locked` on native
  Windows. **Zero source changes** for capture.

## Environment

- OS: Windows NT 10.0.26200 (Windows 11)
- Display: `\\.\DISPLAY2` — 1440×2560 vertical (secondary) monitor; primary is
  `\\.\DISPLAY1` 3072×1728
- DPI: 96 → scale factor **1.0** (1 dp = 1 px)
- Capture backend: `Windows.Graphics.Capture` via devctl `screenshot`
  (client-content PNGs, client-relative pixels); `PrintWindow(PW_RENDERFULLCONTENT)`
  for the two full-window captures

## Window / client geometry

- Outer window: **980 × 1600 px** at (-1392, 48) on DISPLAY2
- Client area: **964 × 1561 px** (964 × 1561 dp @ scale 1.0)
- One common size for all pages: **YES**
- Every implemented page fits without scrolling: **YES** — verified via
  `snapshot-layout`: no `page.scroll` descendant with `height > 0` is clipped or
  extends below the viewport on any captured page (deepest content bottom
  ≈ 1530 dp < 1561 dp client height)
- Scroll offset during capture: top (0) — no scrolling performed
- Stitched screenshots: **NONE** — every image is a single native capture

## Pages

Implemented pages captured (per `examples/gallery.rs` `Page` dispatch — only these
four route to real renderers):

| Page | Nav id | Light | Dark | Content bottom (dp) |
|---|---|---|---|---|
| Components | `shell.nav.components` | `light/components.png` | `dark/components.png` | ~1131 (col h 1031) |
| Forms | `shell.nav.forms` | `light/forms.png` | `dark/forms.png` | ~1468 |
| Typography | `shell.nav.typography` | `light/typography.png` | `dark/typography.png` | ~750 |
| Native Text | `shell.nav.native-text` | `light/native-text.png` | `dark/native-text.png` | ~1035 |

Stub/unimplemented pages intentionally **omitted** (route to `stub_page`, no real
content): `All`, `Navigation`, `Overlays`. Their nav buttons are disabled in the U
I.

Representative full native window (title bar + non-client): `window/light.png`,
`window/dark.png` (Components page shown).

## Capture-quality checklist

| Capture | Result |
|---|---|
| Light Components | PASS |
| Dark Components | PASS |
| Light Forms | PASS |
| Dark Forms | PASS |
| Light Typography | PASS |
| Dark Typography | PASS |
| Light Native Text | PASS |
| Dark Native Text | PASS |
| Window Light | PASS |
| Window Dark | PASS |

All PNGs are original full-resolution captures; no resizing/downsampling/JPEG.

## Observed anomalies (recorded, NOT fixed)

1. **Non-client title bar stays dark in Light theme** — `window/light.png` shows a
   dark title bar ("rust-ui Gallery") identical to `window/dark.png`. The app does
   not appear to drive `DWMWA_USE_IMMERSIVE_DARK_MODE` per-theme; it follows the
   system dark mode. (titlebar mismatch)
2. **Native Text: two multiline specimens show a partially clipped third text
   line** — the "First line of a note. / Second line with more text." textareas
   (`native-text` page, ~y=570 and ~y=755 on light) show a third line's glyph tops
   cut off at the textarea's bottom content edge. Same in dark. Visible mid-glyph
   clipping, not an empty band.
3. **`focus-visible` specimens look identical to default in static captures** —
   e.g. Forms "Focused input"/"Focused textarea" rows and the `*.focus-visible`
   input/switch/checkbox specimens show no visible focus ring difference at rest.
   (Whether the ring only renders during real keyboard focus is not determinable
   from a static capture.)
4. **`window/*.png` has a ~4px dark edge ring** — `PrintWindow` renders the DWM
   border/shadow region as near-black; capture artifact of the full-window path,
   not UI content.
5. **Components: a bare "Badge" text label and an empty badge row region** appear
   below the badge variants row (~y=600 in `light/components.png`) — section
   label/specimen spacing quirk, captured as-is.
6. Disabled nav buttons (`All`, `Navigation`, `Overlays`) render enabled-looking
   text color on hover-none state; they are `enabled:false` in devtools (verified
   in snapshot) — purely informational.

## Artifact publication

- Evidence branch: `evidence/post-slice2-fullpage-gallery` (this commit)
- GitHub Actions artifact `rust-ui-post-slice2-gallery-fullpages`: **the
  repository has no GitHub Actions/workflow configuration at all** (no
  `.github/`, no `.yml`), and the task forbids adding temporary CI. Per mission
  §17 the Actions-artifact publication is reported UNAVAILABLE via existing
  infrastructure. Fallback: release `evidence-post-slice2-fullpage-gallery`
  carries `rust-ui-post-slice2-gallery-fullpages.zip` (asset id 661736151,
  704527 bytes, all PNGs + this README) —
  https://github.com/rceman/rust-ui/releases/tag/evidence-post-slice2-fullpage-gallery
