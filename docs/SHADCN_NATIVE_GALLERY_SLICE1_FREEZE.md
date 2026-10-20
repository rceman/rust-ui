# Shadcn Native Gallery Slice 1 — Freeze / Publication Record

rust-ui is **desktop-only + fully desktop-responsive**, native Rust
(Win32 + Direct2D + DirectWrite + windowless RichEdit text). No browser,
DOM, WebView, JS runtime, or fallback UI stack. Slice 2 may build on
Slice-1 theme/layout/render/component foundations.

## Final identities

- **Production code SHA (under review):** `c8d8147f4c9a6c63a0fe9c0288f03d31ea2f40ea`
- **Publication HEAD:** `agent/shadcn-native-gallery-slice1-opus55` tip at publish time
  (docs/evidence commits on top of `c8d8147` — code unchanged since approval)
- **Visual approval (earlier full review):** `3e9ebad7feef30c517292a72b60f0d6a363d89f9`
  (`review/shadcn-native-gallery-slice1-visual-closure-argon`)
- **Visual delta approval (exact-SHA):** `f8d6bd0455c7d6d54b8dbf32108bb161a3a6bf55`
  (`review/shadcn-native-gallery-slice1-visual-delta-argon`)
- **System approval:** `03fe57cbe2860b6d1725def34bd267ca067c21bf`
  (`review/shadcn-native-gallery-slice1-system-closure-astra`)
- **Prior system review (rework source):** `fe55115037bbe60843866de33049c67f9f729eaa`
- **Base:** `8d3d27361071ee8915dd8f496ea98e8e9b005f0a`
- **Frozen reference publication:** `dc70f6644c0384170f3a6929eb2caaf33fc711a0`
- **Frozen reference payload:** `2e14e417a69ef8967b8fb17120282e3de6449fae`
  (tree `e0bd8eb16d38b778edab4d4ee23ca823696ceb43`) — unchanged
- **Devctl authority:** `cf8ab919b399ca4465a17710741f1c888b84cfa0`

## Final statuses

- `SHADCN_NATIVE_GALLERY_SLICE1_FROZEN`
- `SHADCN_NATIVE_GALLERY_SLICE1_SYSTEM_APPROVED`
- `SHADCN_NATIVE_GALLERY_SLICE1_VISUALLY_APPROVED`
- `SHADCN_NATIVE_GALLERY_SLICE1_PUBLISHED`
- `SLICE2_IMPLEMENTATION_CLEARED` (visual authority cleared Slice 2 to build on this)

## What shipped (Slice 1)

Reusable framework primitives — not Gallery mocks — exercised by
`examples/gallery` reproducing the frozen Shadcn reference:

- **Button** — `ui.button`: 6 variants × normal/disabled/invalid, Xs–Lg,
  `icon_start`/`icon_end`, invalid ring unfocused, nowrap centered
  icon+gap+label group.
- **IconButton** — `ui.icon_button`: 5 variants × normal/disabled, 4 icon
  sizes, decorative icon excluded from UIA name.
- **Card / Surface** — `ui.card`: `CardSize::Default`/`Sm`, header/title/
  description/content/footer parts, fixed `width(dp)`, footer `bg-muted/50`
  + border-t + `Justify::End` buttons, 1dp spread ring.
- **Typography** — `ui.typography` + `ui.code_block`: Geist type scale,
  DirectWrite custom collection, code-block `pre`/`pre.line[N]` parts.

Component ledger: all four promoted `IMPLEMENTED` → `VERIFIED` on the
approved evidence (`docs/COMPONENT_SUPPORT.md`).

## System closure content

- **S01** — `Win32_Graphics_Dwm` owned by base Windows deps; stale `Shadow`
  literals carry `spread`.
- **S02** — devtools emits one icon part per populated edge (single edge
  keeps canonical `icon`; both edges get `icon.start`/`icon.end`); owner
  `part_key` seeds descendant prefixes (`pre.line[N]`).
- **S03** — shared `crate::layout::justify_offset` drives Row and
  Column/Card; `Column::justify` no longer ignored; overflow clamps to 0,
  `Fill` consumes slack first, `Start` preserves all approved output.
- **S04** — non-finite `Shadow.spread` rejected before cache/distance math.
- **Fixture hygiene** — `devctl_probe` removed from `examples/`; internal
  `rust-ui-devctl-test-fixture` bin under `tests/support/` located by
  `CARGO_BIN_EXE_`; `examples/gallery` is the only UI acceptance app.

## Verification performed (see `SHADCN_NATIVE_GALLERY_SLICE1_OVERNIGHT_CLOSURE.md`)

- Linux/WSL: `cargo fmt --check` clean; `cargo check --locked --examples` PASS;
  `cargo test --locked --lib` 108/0.
- Windows (cargo.exe, NTFS target, default features): `check --locked --examples`
  PASS; `test --locked --lib` 147/0.
- Windows `--features devtools`: `check --locked --examples` PASS;
  `test --locked --lib` 176/0; `--test devctl_windows` PASS (3 cycles).
- Live Gallery devctl proof at scale 1.25: `icon-inline-end#icon` trailing edge,
  `typography.code-block` `pre`/`pre.line[1..3]`.
- Same-scale true-baseline snapshot diff vs `8d3d273`: 53 ids, 0 geometry
  diffs — pixel-path inert for all approved specimens.

## Intentionally remaining non-blocking follow-ups

- **V07** — `bg-clip-padding` / pressed +1dp crop nuance (known, accepted).
- **V09** — future scrollable page viewport requirement (out of Slice-1 scope).

Intermediate failures are recorded in the overnight closure report — no
retries or rework cycles are hidden.
