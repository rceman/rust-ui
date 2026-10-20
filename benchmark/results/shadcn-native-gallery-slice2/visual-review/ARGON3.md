# Argon independent visual re-review — Slice 2 candidate (fourth pass)

Candidate: `3c5c3b0c4183526a345f913db84b16192b900c76` on
`agent/shadcn-native-gallery-slice2-opus55`, worktree `rust-ui-slice2`.
PROMPT_ID: RUI_ARGON_P2-261020-183000. Second re-review of the rework for my
prior ARGON2 findings (D5, N1, N2) **plus the new offscreen-blit peer path**
that resolved the M2 top-clip deviation.

## Method (own drive, fresh binary — stale-binary trap avoided)

- `taskkill gallery`; **deleted** `rust-ui-wsl-target\debug\examples\gallery.exe`,
  rebuilt @ `3c5c3b0` (`CARGO_INCREMENTAL=0`, `--features devtools`). New exe
  timestamped at build time — confirmed fresh, not a no-op.
- Live session `argon3r` (pid 20680): `click`/`focus`/`key`/`scroll`/
  `scroll-into-view`/`resize`/`snapshot-layout`/WGC `screenshot`.
  Windows 980×1600 (client 964×1561), 980×760/700/500/520 for clip work,
  scale 1.0, Light + Dark. Reference truth: `W:\devin_folder\rui-ref2`
  contract.json + frozen PNGs.
- Raster proof by pixel histograms / `ImageChops.difference` on same-doc rows.
  Geometry re-sampled for every contract-bound native element in all four
  implemented pages, both themes (size-only, 0.5dp tol).

## Per-finding verdicts

| # | Verdict | Evidence |
|---|---------|----------|
| **D5** editors paint no text | **VERIFIED** | Every seeded editor now renders text. native-text 14/14: filled/focused/selection/readonly ≈386–665 ink px, invalid 259–271, disabled muted ~490/348. forms 12/12: input/textarea all states 259–929. field.\*#control 3/3 render 564 px (flagless `field@example.com`). No last-3-per-group blanks left. **Both themes.** Disabled correctly renders *muted* text — matches the frozen ref (I re-measured: ref disabled = 446–563 px muted text; ARGON2's "ref disabled is empty" was wrong). Focus-visible no longer squished to a 1–2px band — full text. |
| **N1** checkbox label collision | **VERIFIED** | Specimens now on 144dp pitch in 4-col chunks (48/192/336/480). Labels end before next cell (max label end x≈557 vs next at 624/row-wrap). Disabled labels dim to grays (132–211). Disabled + disabled-checked box interiors clean — the D4 navy/dark specks are **gone** (unchecked-disabled = pure bg, disabled-checked keeps gray fill). Both themes. |
| **N2** typography.small | **VERIFIED** | Now **86.86×18** (was 720×14) — content-hug row wrapper restored, height exact vs contract 18. Residual: width −5.07dp vs contract 91.94 (DirectWrite-vs-Chromium metric, borderline just past the ±3.5 class). `inline-code` content-hugged (143.46 not 720) — still −50.95w/+2.03h, persistent mono-advance minor (see M-below). |
| **M2/blit regression** | **VERIFIED** | See dedicated section — all sub-checks pass. |

## M2 / offscreen-blit verification (the new render path)

`TxDrawD2D` → per-peer `ID2D1BitmapRenderTarget` (full content rect, premult,
transparent clear) → `DrawBitmap` into frame under the ClipGuard.

- **Partial-clip band = true mid-document.** `multiline.focused` top-clipped
  (band y100–137, doc rows ~27–64): band pixels **pixel-identical** to the same
  doc rows in an unclipped `multiline.readonly` (y354–391) — **0 px diff**.
  vs a doc-head crop: **1173 px diff**. Shows "Line two.", not "Line one."
- **Caret maps true offsets.** Clicked the clipped `multiline.filled` band
  (center→doc row ~30, line2) → Delete removed the **line2→3 break** — caret at
  end of line2, correct. The clipped `focused` click landed at end-of-line1 —
  that point was in the inter-line gap and snapped to the nearest char edge
  (legitimate). Full pipeline verified end-to-end: click→focus→Home→Delete on a
  clipped editor committed (`alice`→`lice`).
- **Fully-clipped → nothing painted.** Editors with `visible:false` and
  rect fully outside the clip leave zero ink; the region below the viewport
  clip edge is clean (y429–461, no bleed).
- **Band position correct**, not shifted to band-top (text at doc-consistent
  rows). No banding / halo / alpha-tint / offset / bleed / resize-ghost
  artifacts — premult blit renders clean text on white and disabled-gray.
- **No crash** through ~13 cycles of clipped-editor scroll + resize + WGC
  screenshot; process stayed alive/idle (the ARGON crash territory).
- **Dark theme**: clipped band shows the same mid-doc rows (y109–118 ink) —
  the blit is theme-agnostic.

## Regression sweep (size-only vs contract, every native element, both themes)

| Page | Specimens | Height fails | Width fails |
|------|-----------|--------------|-------------|
| components | ~52 | **0** | only accepted text-metric (button ±0.5–2.5, badge −1.76, kbd −1.99/−3.48) |
| forms | 37 | **0** | **0** |
| native-text | 14 | **0** | **0** |
| typography | 10 | 0 | small −5.07, inline-code −50.95, code-block −8 (known container-adapt) |

Identical deltas light and dark (geometry is theme-independent). Slice-1
specimens (button/icon-button/card/separator/kbd/badge/alert/skeleton) and all
previously-passing Slice-2 specimens preserved — the blit change touched only
editor peers, and all heights are exact.

## NEW findings this pass

| # | Where | Finding | Severity |
|---|-------|---------|----------|
| **N3** | `input.default.placeholder`, `textarea.default.placeholder`, `native-text.single-line.placeholder`, `native-text.multiline.placeholder` | **Placeholder text never renders — 0 non-white px inside all four, both themes** — vs frozen ref ~540 px muted "Email"/"Type your message here." Root cause: the `placeholder` field is staged into `NodeData::Editor` but **no platform code consumes/draws it** (verified absent since slice-1 — pre-existing platform gap, **not** a blit regression). The blit fix *exposed* it: peers now faithfully render their own (empty) docs; under the old positional bug some placeholder editors appeared to show text (foreign doc content — the "placeholder renders" note in ARGON1/2 was that artifact). These are contract-bound **core** specimens whose placeholder is their only content. Fix path is bounded (render the staged string muted when the doc is empty). | **Defect — needs fix or formal deferral** |
| **M-a** | `*.disabled` editors, **dark only** | Native renders noticeably brighter disabled text (~130–251) on brighter bg (40) vs ref's heavily-dimmed (~64–81 on 25). Text renders (D5 satisfied) but the dark disabled-dim is weaker than frozen. Light is close (241 vs 249 bg). | minor |
| **M-b** | `alert.*#title` part | Part width = text-run (59.96/148.78) vs contract's 274 full-block width. Part-semantics difference (native emits text-run bounds), pre-existing — `devtools.rs` untouched by this rework. | minor / part-semantics |
| **M-c** | `typography.small` w −5.07; `typography.inline-code` w −50.95/+2.03h | DirectWrite/mono-advance metric class; inline-code persistent since first pass (same font size, ~25% narrower mono advance). | minor |

Element chrome/borders/layout on the N3 specimens are all contract-exact —
only the placeholder hint text is absent.

## Coverage

- native-text 14 editors × L+D — geometry exact, text renders (except
  placeholders, N3), clip/caret/blit verified.
- forms 37 × L+D — geometry exact, editors render, checkbox row fixed.
- components ~52 × L+D — heights exact; widths in accepted text-metric class.
- typography 10 × L+D — N2 fixed; small/inline-code/code-block residuals above.
- live ops: scroll (app-owned offset, correct max clamp — earlier "no-ops" were
  my own >max deltas, not a bug), scroll-into-view, resize, click, focus, key,
  WGC — all stable, no crash.
- Honest gaps unchanged: select/slider/typography.{large,blockquote,list,table}
  absent (tier later/reference-only); live selection unverifiable (no text-input
  devctl cmd); shell.* chrome differs from ref shell (icon-only theme buttons).

Evidence: `evidence3/` (cb-row light/dark, clip-band mid-doc proofs,
placeholder natives + ref, inline-code native/ref, readonly/disabled zooms) +
full captures `W:\devin_folder\argon3r\`.

## VERDICT: NEEDS_REWORK

Everything reworked is verified clean — **D5, N1, N2, and the M2/blit path all
pass, the regression sweep is clean, and there is no crash.** One new defect
stands between this and approval:

1. **N3 — placeholder text never renders** on all four placeholder specimens
   (`input`/`textarea`/`native-text.single-line`/`native-text.multiline`),
   both themes. Contract-bound core elements whose placeholder hint is their
   entire visible content — empty vs the frozen ref's ~540 px muted text.
   Pre-existing platform gap (the staged `placeholder` field is never drawn),
   newly exposed by the blit fix's faithful per-peer rendering. Bounded fix:
   draw the staged placeholder muted when the document is empty.

   *Framing for the lead:* this is not a rework regression — the rework is
   clean. It is a real frozen-truth miss on core specimens. If the milestone
   consciously defers native placeholder rendering, it should be recorded as a
   deviation/partial rather than shipping the specimens silently empty.

Non-blocking minors: M-a dark disabled-dim, M-b alert title part-width, M-c
small/inline-code metric residuals.
