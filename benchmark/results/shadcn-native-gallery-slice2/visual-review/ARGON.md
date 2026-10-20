# Argon independent visual review — Slice 2 candidate (FRESH, second pass)

Candidate: `3bb426ee131559dcbe9e2ffcf0fa562ba3816b09` on
`agent/shadcn-native-gallery-slice2-opus55`, worktree `rust-ui-slice2`.
PROMPT_ID: RUI_ARGON_P2-261020-161000.

A prior ARGON.md existed here for `eaf7ce1` — this report is my own fresh
evidence at `3bb426e` (the prior file's D1–D7 findings are independently
re-confirmed; it also reported app crashes I did not reproduce — see Notes).

## Method (my own drive, not the submitted capture set)

- Binary audit first: the shipped `W:\devin_folder\rust-ui-wsl-target\debug\
  examples\gallery.exe` (Oct 20 19:42) was **stale** — `strings` showed zero
  slice-2 ids (`badge.default`, `native-text.*`, `switch.*` all absent), nav
  had Forms/NativeText disabled. It ran slice-1 code with only the taller
  window. I rebuilt it from the worktree (`CARGO_INCREMENTAL=0`,
  `CARGO_TARGET_DIR=W:\devin_folder\rust-ui-wsl-target`, `--features devtools
  --example gallery`); cargo fingerprint had to be forced (`touch` +
  deleted stale exe) — it no-op'd on the first attempt. Fresh exe 19:50
  now contains all slice-2 ids. **The prior 147-PNG evidence set was
  captured before that stale rebuild, so it is not proof of this binary.**
- Live drive: `rust-ui-devctl` session `argon3` — `click`/`focus`/`key`/
  `scroll`/`scroll-into-view`/`resize`/`snapshot-layout`/WGC `screenshot`.
- Comparison: `compare-element --mode size-only --tolerance 0.5
  --channel-tolerance 24` for every element on every implemented page vs
  `reference/shadcn-gallery-v0.1/contract.json` + frozen page PNGs
  (`W:\devin_folder\rui-ref2`), Light and Dark. ~100 element comparisons.
- Windows sized 948–1960 dp to expose every specimen (scroll is broken, see
  D1, so reachability required window resize).
- Text/glyph raster differences (DirectWrite vs Chromium) are treated as
  signal-only; geometry is the contract authority.

## MAJOR defects

| # | Where | Finding |
|---|-------|---------|
| **D1** | **Scroll is dead (all pages)** | `scroll`/`scroll-into-view` report success (`{"scrolled":30}` / `{"scrolled":16}`) and a scrollbar lane appears (viewport width 900→884), but **content never translates**: `button.default` stays at y=116 after ±30/±40/scroll-into-view, kbd stays clipped. The offset never reaches layout. Verified at 900 and 1522 dp windows, light and dark. Below-fold specimens are **unreachable** — this also makes "scroll clipping at edges" unverifiable as designed and is a functional blocker for the page viewport feature. |
| **D2** | `page.scroll` viewport underfill + content tail clip | The scroll node caps at its measured content height instead of filling the window — Components viewport 913 dp in a 1522 dp client, Typography 533 dp, Forms 1085 dp — and its measure **under-measures the laid-out content by ~16 dp**: the last row is clipped mid-row with hundreds of dp of dead window below (`kbd.default`/`kbd.group` slivered at y≈1009–1013 on Components; `typography.code-block` fully invisible on Typography). Evidence: `evidence/kbd-clipped-deadspace.png`. |
| **D3** | `separator.vertical` | Contract 1×64; native reports **1×0, visible:false** — the vertical separator collapses to zero height and renders nothing. `compare-element` → `TARGET_NOT_VISIBLE`. (Horizontal separator is pixel-exact.) |
| **D4** | Disabled toggles render wrong | Light: `checkbox.default.disabled` shows stray dark + solid-blue `(10,66,156)` artifacts instead of a faint unchecked box; `checkbox.default.disabled-checked` loses the gray `(138,138,138)` checked fill entirely (white box + stray pale pixels); `switch.default.disabled-checked` is **fully invisible** (all-white 32×18). Dark: same pattern — `switch.default.disabled` all-bg invisible, `checkbox.default.disabled(-checked)` stray light/orange/blue pixels. 93% and 57% raster mismatch. Evidence: `evidence/forms-{light,dark}-*/`. |
| **D5** | `native-text` disabled/readonly/invalid editors paint no text | Light: `native-text.{single-line,multiline}.{readonly,disabled}` = 0 text pixels (seeded `alice@example.com` / `Line one…` absent — only border+bg paint). Dark additionally: `multiline.disabled`, `single-line.disabled`, `*.invalid` (ring paints, "bad value/body" text absent) = 0 text px. The sibling **forms-page** `input/textarea.disabled` DO render text — so this is specific to the native-text page editors. Filled/focused/placeholder/selection editors render text correctly both themes. |
| **D6** | `field.*` +18…+20 dp height + illegal inner padding | `field.default/.invalid/.disabled` 288×103/102/103 vs frozen 288×83/82/83; `field.horizontal` 288×55 vs 288×37 — `ui.field` adds ~8dp padding the contract forbids (parts at rel x=8 vs frozen x=0 edge-to-edge), and in `field.horizontal` the switch overflows the field box right edge by ~8dp (frozen thumb rel x=271 inside). geo FAIL all 4, both themes. |
| **D7** | `alert.*` heights short / description wrap | `alert.default` 320×58 vs 60 (−2); `alert.destructive` 320×58 vs 80 (−22): the 2-line frozen description renders ONE line — pixel crop shows title + single desc line, second line absent. Both themes, evidence `evidence/components-light-alert_destructive/`. |
| **D8** | Forms rows overflow horizontally, unreachably | Radio row needs ~1914 dp (`radio-group.default.disabled` at x=1552) and field row ~1264 dp (`field.horizontal` x=960) — inside any normal-width client they're clipped invisible at the right edge; the frozen reference wraps these specimens. Combined with D1 they are unreachable at 948–1440 dp widths. |
| **D9** | `skeleton.default` element rect | Contract 224×40; native element **884×40** (row-stretch) — parts (avatar 40², lines 168/112) are correct, the outer wrapper takes row width not content. geo FAIL both themes (dW=+1656 vs the 224 frozen). |

## MINOR / watch

- `badge.*` −1.76 dp width on all 6 variants (DirectWrite measures "Badge"
  narrower; cosmetic but uniform — exceeds the 0.5 tol).
- `kbd.default` −1.99, `kbd.group` −8.46 width; kbd dark raster 79%
  mismatch — the cap bg covers ~⅓ of the cap vs frozen fill (dark theme).
- `typography.inline-code` −50.95 dp width (+2 height) — chip much narrower
  than frozen (mono metric + padding class, needs a look).
- Slice-1 `button.*` width deltas ±0.5–2.5 dp (same text-metric class as
  slice-1 accepted evidence; flagged where >0.5 tol). Card/icon-button/
  separator.horizontal/h1/h2/p/lead all pass cleanly.
- `switch.default.invalid` ring renders saturated red where frozen shows a
  soft pink ring (alpha/intensity class).
- **Transient:** one session (stale binary) devolved into every
  `snapshot-layout` failing `INVALID_ARGUMENT "snapshot bounds"` after
  launch+screenshot — some node held an invalid rect. Not reproduced on the
  fresh binary (10-step resize sweep clean), but worth watching — an
  invalid-rect path exists.
- **Selection state cannot be synthesized**: devctl v0.1 has no modifier
  chords/drag — `native-text.*.selection` specimens can only be checked for
  text rendering (they render text), not a live selection. Honest gap.
- Prior review noted app crashes during devctl drives — not reproduced in
  my session, but the snapshot-bounds event above may be related.

## Verified good

- Geometry PASS: all `input.default.*`, `textarea.default.*`, checkbox/
  switch/radio element boxes, `card.*`, `icon-button.*`, `label.default`,
  `separator.horizontal`, `typography.{h1..h4,p,lead,small,muted}` — sizes
  exact vs contract in both themes.
- States work: `click` flips `checkbox.default.unchecked`→checked
  (evidence `checkbox-clicked-flip.png`); keyboard `focus` sets
  focused/focus_visible=true; invalid rings paint (red border on
  `checkbox.default.invalid`, `input.default.invalid`); radio/switch/
  checkbox pre-staged checked states render.
- `native-text` element boxes are contract-exact (360×32/64/78); filled/
  focused/placeholder text renders; `multiline.filled` 3-line 78dp correct.
- Theme toggle light/dark both resolve; resize relayouts correctly
  (multi-step sweep, no artifacts; viewport bug D2 aside).
- Slice-1 surfaces unchanged in kind: cards parts, icon-button grid, button
  heights all exact.

## Element coverage

| Page | Elements compared (L+D) | Geo PASS | Defects |
|------|------------------------|----------|---------|
| components | ~52 ×2 | most | D2,D3,D7,D9,badge/kbd widths, kbd-dark raster |
| forms | 38 ×2 | 34/38 | D4,D6,D8 |
| native-text | 14 ×2 | 14/14 geo | D5 (text paint) |
| typography | 6 ×2 (spots) | 5/6 | D2 clip, inline-code width |
| states | focus/click live-driven | — | ring paints; flips work |

## VERDICT: NEEDS_REWORK

Blockers: **D1** (scroll offset never applies — page content unreachable),
**D2** (viewport underfill + tail clip — default page clips its last row),
**D3** (separator.vertical invisible), **D4** (disabled-toggle state
rendering), **D5** (native-text disabled/readonly/invalid paint no text),
**D6/D7** (field/alert frozen geometry misses), **D8** (rows clipped off
right edge), **D9** (skeleton element rect). Each is independently
reproduced in both themes where applicable; evidence crops under
`benchmark/results/shadcn-native-gallery-slice2/visual-review/evidence/`
(native-canonical.png / reference.png / diff.png / comparison.json per
element) plus the full `W:\devin_folder\argon3\` capture set.
