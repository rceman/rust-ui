# P12 — Automated Visual Parity Closure

**Branch:** `feat/p12-automated-visual-parity`
**Baseline:** `873c9996a908d5307c364925bfec6b4c1c40eb54` (main, P11 structural refactor — P12 start)
**Candidate:** `9a58eeb` (signed-margin finiteness + probe/audit/boot-fix chain)
**Reference (frozen):** `reference/shadcn-gallery-v0.1` — immutable `contract.json` + `screenshots/{light,dark}/*.png`

## Verification matrix (R3)

| Gate | Result |
|---|---|
| `cargo fmt --check` | PASS |
| `cargo check --locked --examples` | PASS |
| `cargo test --lib` | **126/126 PASS** |
| `tools/check-file-tokens.sh` | PASS (every file < 5000 tokens) |
| Windows build (UNC manifest, `--features devtools`) | PASS |

## P12-M1 — Native motion parity (event-driven transitions)

Frozen reference: 150ms, `cubic-bezier(0.4,0,0.2,1)`, `transition-all/-colors/-transform/-opacity` per the staged recipes.

**Implementation:** `src/platform/win32/transitions.rs` — per-node `Trans` channels (visual + switch-thumb scalar + pressed-displacement + disabled-opacity + ring-alpha); `ease_css` solver (Newton+bisection) in `sched.rs`; reuses deadline-timer → `WM_TIMER` → `turn()` → `paint()`; `displayed_*` lerps consumed at paint; interruption-safe via last-painted `shown` cache; `DIRTY_VALUE` on checked/selected/disabled/invalid flips. No continuous idle animation — scheduler work stops at settle.

**Time-sampled evidence** (devctl `epoch`/`trans_detail` handshake, avoids screenshot races):

```
hover -> trans_detail.frac: 0.453 -> 0.899 -> 0.988 -> []  (settled ~150ms)
animation_timer_fires/callbacks advance ONLY while animating
paint_epoch advances during animation, FLAT at idle (M2 idle-zero proven)
```

**Reduced motion (M2):** `RUI_REDUCED_MOTION=1` → `reduced=true`, `transitions=0`, instant snap, flat epochs. Both halves verified.

## Geometry matrix (real Geist + TOM font, both themes)

Corrected capture sweep v6 — `RUI_GALLERY_PAGE` now crosses WSL→Windows via
`WSLENV` (an earlier run mislabeled all pages as Components content);
boot-time regressions that killed Forms/Typography launches are fixed
(`53e7d3e` read-only TOM font, `9a58eeb` signed-margin finiteness). 87
native captures across all 7 pages × light+dark + collapsed-nav.

351 `compare-element` runs; **215/351 size-PASS at tolerance 0.25**
(`dh=0` on every compared element — all height deltas are width-only).

**Verified EXACT (`dw=0 dh=0`)** — `typography.code-block` 728 (the
`margin_x(-4)` breakout + 4-space indent), all `input.*`/`textarea.*`
variants @360px (disabled/filled/focus-visible/invalid/placeholder, both
themes), `settings-nav-item.*` 208×32, `radio-group.*` 360×40,
`settings-nav.default` collapsed rail 48px. `TARGET_NOT_FOUND` elements
resolve to `DEFERRED` (out-of-scope) specimens — Slider/Toggle/
Toggle-Group/Breadcrumb/Pagination.

### Remaining deviations (text-metric / mono-face class)

| Element | Delta | Class |
|---|---|---|
| `tabs.default`/`tabs.line` | −9.98 w | text-metric (~3px/tab advance) after S5 row-padding fix |
| ~~`typography.code-block`~~ | 0.0 w | **FIXED** — `margin_x(-4)` breakout + indent, exact at 728 |
| `typography.small` | −5.07 w | text-metric |
| `typography.inline-code` | −40.6 w | mono-face advance (ref `font-mono` stack ≠ bundled Geist Mono); recipe size corrected 13→14 |
| `button.destructive` | −2.5 w | text-metric |
| `select.default.*` trigger | −1.52 w | text-metric |
| `button.*` variants | ±0.5–1.3 w | sub-dp text-metric |

## Fonts (T5)

`AddFontResourceExW` cannot load bundled TTFs from a `\\wsl.localhost` 9P share — fonts must sit next to the exe (`assets/fonts/geist` under the binary dir). With private registration: `geist{sans:true,mono:true}` (handshake exposes it).

Under `TXTBIT_D2DDWRITE` the `CHARFORMAT` `szFaceName`/`yHeight` are inert — editable peers render via `ITextDocument → ITextRange → ITextFont::SetName+SetSize` (`apply_font_tom`). Geist/Geist Mono render at the authored 14px; `input.default.filled` ink 67px vs contract 72.19 advance.

## Rings / borders / states

- **B1/B2** — geometry 0.00 exact (card, alert, separator, checkbox, switch, textarea, field, settings-nav-item).
- **B3** — focus ring `oklab(0.708)/0.5` 3px (gray 207) contract-exact.
- **B4** — invalid-ring precedence: invalid-focus-visible shows red ring, not gray.
- **B5** — state colors contract-faithful: default `(23,23,23)`, hover `(69,69,69)`, disabled `(139,139,139)`, destructive `(252,229,230)`, outline white, secondary/ghost hovers.
- **I5** — invalid `(250,204,206)` ring + `(231,0,11)` border = contract `oklch` exact.
- **T4** — muted placeholder `(115,115,115)` at ~10px pad.
- **S6** — settings-nav items 240/208×32 expanded, 32×32 collapsed.
- **S1/S3** — select content 208×241, items 200×28, offset −49.1 vs contract −48.3.
- **S4/S7** — live select keyboard pick + popup close + focus restore verified.

## Accepted/documented

- **T6** — DPI-dependent positioning unexercisable at `scale_factor=1.0` on this box (no >1.0 display); mapping code unchanged, no >1.0 capture evidence.
- **I1/I2** — ~1px baseline/placeholder residual (S3-class, accepted).
- **I7** — selection `[0,6]` staged-presented unfocused (RichEdit hides selection w/o focus); caret/selection verified live.
- **I4** — disabled appearance accepted.

## Ledger

See `docs/P12_VISUAL_PARITY_LEDGER.md` for the per-item verdicts.
