# Shadcn Native Gallery — Slice 2 closure report

**Production head:** `cc90ad2971ba27e71d67e54efd64fd311675d113`
**Based on:** `a7beab1e633ca890471cb02f9e98b02f56614d61`
**Status:** `SHADCN_NATIVE_GALLERY_SLICE2_PUBLISHED`

## Delivered

Complete Slice 2 of the Shadcn Native Gallery: Separator, Badge, Alert,
Skeleton, Kbd, Label, Field, Input, Textarea, Checkbox, Radio Group,
Switch, native single-line + multiline Shadcn integration, reusable
scrollable page viewport, Forms + Native Text Gallery pages — and the
window-resize freeze investigation + fix.

## Evidence index (`benchmark/results/shadcn-native-gallery-slice2/`)

- `resize-performance/` — `baseline.json` (pre-fix), `final-light.json`,
  `final-dark.json`, `final-forms.json`, `environment.txt`, `summary.md`
- `slice1-regression/` — specimen-size-preservation proof + RESULT.md
- `visual/` — 147-PNG Light+Dark specimen set (pre-rework captures)
- `visual-review/` — Argon reports ARGON.md, ARGON2.md, ARGON3.md,
  ARGON4.md + evidence/ (rework cycle through approval)

## Verification

| Gate | Result |
|---|---|
| `cargo fmt --check` | clean |
| `cargo check --locked --examples` | green |
| `cargo test --locked --lib` (linux) | 108/108 |
| `cargo.exe test --locked --lib` (win, default) | 147/147 |
| `cargo.exe test --locked --lib --features devtools` | 176/176 |
| `devctl_windows` integration | 1/1 |
| Resize | WM_SIZE 29.3→0.28ms; frame p95 22.7≤33ms — CLEARED |
| Slice-1 regression | specimen sizes preserved |

## Review outcomes

- **Astra** `SYSTEM_APPROVED` — initial NEEDS_REWORK (M1 UIA phantom
  bounds/IsOffscreen, M2 clipped-editor band, m1–m6) → all reworked and
  verified; deviation #12 recorded honestly.
- **Argon** `VISUALLY_APPROVED` — four passes: D1–D9, N1/N2, N3 → all
  reworked and verified; placeholder + offscreen-blit fixes landed; a
  process-death report was not reproduced across bounded attempts.

## Honest limitations (do not overclaim)

- Switch → UIA `CheckBox` (no ToggleState infra).
- `alert.destructive` desc uses a forced `\n` for the frozen 2-line box.
- Placeholder is a separate muted draw (not a RichEdit feature).
- Live text selection and real drag-resize not directly scriptable by
  devctl — resize proven via the same synchronous WM_SIZE path.
- The earlier devctl-reported crash near clipped editors was not
  reproduced post-blit (bounded attempts); noted as investigated.

## Statuses

`SHADCN_NATIVE_GALLERY_SLICE2_FROZEN` · `SHADCN_NATIVE_GALLERY_SLICE2_SYSTEM_APPROVED` · `SHADCN_NATIVE_GALLERY_SLICE2_VISUALLY_APPROVED` · `RESIZE_PERFORMANCE_REGRESSION_CLEARED` · `SHADCN_NATIVE_GALLERY_SLICE2_PUBLISHED`
