# Shadcn Native Gallery — Slice 2 freeze record

**Candidate SHA:** `cc90ad2971ba27e71d67e54efd64fd311675d113`
**Based on:** `a7beab1e633ca890471cb02f9e98b02f56614d61` (published Slice 1 `main`)
**Frozen reference:** `reference/shadcn-gallery-v0.1/` (unchanged — payload `2e14e417`)
**Branch:** `agent/shadcn-native-gallery-slice2-opus55`

## Scope delivered

| Area | Items |
|---|---|
| Static | `ui.separator`, `ui.badge` (6 variants), `ui.alert` (default/destructive + icon/title/description parts), `ui.skeleton`, `ui.kbd`, `ui.kbd_group` |
| Forms | `ui.label`, `ui.field` (vertical/horizontal + label/control/description/error parts), `ui.input`, `ui.textarea` (Shadcn chrome on windowless RichEdit), `ui.checkbox`, `ui.radio_group`/`item`, `ui.switch` (default/sm + thumb) |
| Native text | single-line + multiline Shadcn integration on the same windowless RichEdit peer (`ITextHost`/`ITextServices`/`TxDrawD2D`) |
| Shared | `KIND_SCROLL` reusable page viewport; Gallery Forms + Native Text pages; resize-freeze fix |

## Key implementation notes

- **Resize freeze fixed + measured** — WM_SIZE handler 29.3ms → 0.28ms
  (103×), text-layout creates 356 → 0/step, paints coalesced 1.74× → 1×,
  p95 frame work 22.7ms ≤ 33ms. Evidence: `benchmark/results/
  shadcn-native-gallery-slice2/resize-performance/`.
- **Scroll viewport** — app-owned Dp offset (state in the model);
  children stack unbounded, rects stored translated by the clamped
  applied offset; a clips map confines paint (RAII Push/PopAxisAlignedClip),
  hit-test, peer geometry, focus eligibility, devtools visibility, and UIA
  bounds (`rects ∩ clips` + `UIA_IsOffscreen`).
- **Offscreen peer blit** — every RichEdit peer draws `TxDrawD2D` into a
  cached per-peer `ID2D1BitmapRenderTarget`, then `DrawBitmap`s into the
  frame. Fixed the positional shared-target zero-pixel quirk AND made the
  pushed clip confine peers (top-clip band shows true mid-document rows —
  `SPIKE_ARCHITECTURE_DEVIATIONS.md` entry 12 resolved).
- **Toggle leaf** (`KIND_TOGGLE`) — checkbox/radio/switch share one
  painted leaf; checked state is app-model (Msg), Space/Enter→Press,
  UIA CheckBox/RadioButton, synthetic `indicator`/`thumb`/`label` parts.
- **Input/Textarea** — two-node KIND_STACK: borderless editor leaf at the
  full element rect + transparent overlay carrying the 1dp border, r10,
  invalid `InputDestructive`, disabled `InputDisabled`. Chrome never
  disturbs the peer.
- **Placeholder** — staged hint drawn muted over the empty doc.

## Invariants held

- Frozen reference untouched; Slice-1 specimen geometry preserved
  (w/h exact on sampled specimens; accepted ±0.5–3.5dp text-metric class).
- No Slice-3 components (Select/Tabs/Dropdown/SettingsNav/Tooltip/Popover/
  Dialog absent); no `devctl_probe` public example; fixtures under tests/.
- No forbidden tech; windowless RichEdit internals preserved.

## Deviations / known limitations

- `SPIKE_ARCHITECTURE_DEVIATIONS.md` #1–#12 (entry 12 resolved-by-blit).
- Switch maps to UIA `CheckBox` — no ToggleState/SelectionItem infra.
- Alert destructive description uses an explicit `\n` at the frozen wrap
  (DirectWrite advance fits one line at 274dp where Chromium wraps).
- Placeholder is a separate muted draw, not a RichEdit feature.
- Argon N4: gallery staged strings were aligned to frozen `text_inventory`.
- Unverifiable by devctl: live text selection (no modifier-drag cmd);
  real drag-resize is proven via `devctl resize` exercising the same
  synchronous WM_SIZE path.

## Gates (all green at candidate)

- Linux: `cargo fmt --check`, `cargo check --locked --examples`,
  `cargo test --locked --lib` (108/108)
- Windows default: `cargo.exe test --locked --lib` (147/147)
- Windows devtools: `cargo.exe test --locked --lib --features devtools`
  (176/176) + `devctl_windows` integration (1/1) + native-text regression
- Resize: baseline→final evidence dir (status RESIZE_PERFORMANCE_REGRESSION_CLEARED)

## Reviews

- **Astra system review** — `SYSTEM_APPROVED` (findings M1/M2 + m1–m6
  reworked; rework verified; deviation #12 honesty confirmed).
- **Argon visual review** — `VISUALLY_APPROVED` (D1–D9 + N1/N2/N3
  reworked across 4 passes; all verified).
