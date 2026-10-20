# Argon independent visual re-review — Slice 2 candidate (third pass)

Candidate: `0920dbbb6a683e8dc5605f5bb91270243f35b489` on
`agent/shadcn-native-gallery-slice2-opus55`, worktree `rust-ui-slice2`.
PROMPT_ID: RUI_ARGON_P2-261020-170000. Re-review of the rework for my prior
D1–D9 + minors (ARGON.md @ `3bb426e`).

## Method (own drive, fresh binary)

- Rebuilt `gallery.exe` from the worktree @ `0920dbb`
  (`CARGO_INCREMENTAL=0`, `CARGO_TARGET_DIR=W:\devin_folder\rust-ui-wsl-target`,
  `--features devtools`). Fingerprint no-op build — exe 20:20 matches the
  20:22 commit (build-then-commit); not stale.
- Live session `argon2r`: `click`/`focus`/`key`/`scroll`/`scroll-into-view`/
  `resize`/`snapshot-layout`/WGC `screenshot`; window 980×1600 (client
  964×1561) and 980×700/760 (client 964×661/721), Light + Dark.
- Contract authority: `reference/shadcn-gallery-v0.1/contract.json` +
  frozen PNGs (`W:\devin_folder\rui-ref2`). Size-only geometry vs
  `rect_dp` from snapshots; raster checked by direct pixel histograms +
  side-by-side crops (evidence2/).
- Systemic-change regression sweep: `natural()` now applies the node's own
  LayoutSpec and `Stretch` honors authored `Fixed` — re-sampled every
  implemented page's element rects in both themes.

## Per-finding verdicts

| # | Verdict | Evidence |
|---|---------|----------|
| **D1** scroll dead | **VERIFIED** | 980×700 client (964×661): `scroll --id page.scroll --dy 300` → all 90 content rects translated exactly −300 (`button.default` y116→−184, `kbd.*` 1095→795); 51 `visible` flips (button.default true→false above fold, `card.default` false→true into view). `scroll-into-view kbd.group` → `{"scrolled":186}`, lands flush at viewport bottom (y=609, bottom 629=viewport edge). Load-bearing scroll works. |
| **D2** underfill + tail clip | **VERIFIED** | 964×1561: `page.scroll` = (32,100,900,1429) — fills the column leftover (root column now `Fill(1)`). Content ends y≈1115; kbd row fully painted (20dp, not slivered); nothing clipped; the region below content inside the viewport is uniform page bg (correct for a fill viewport — the old defect was window dead space + clipped tail, both gone). |
| **D3** separator.vertical | **VERIFIED** | (347.5,640,**1×64**, visible:true) both themes — contract 1×64 exact. |
| **D4** disabled toggles | **PARTIAL — still defect** | PushLayer bounds fix works: `checkbox.disabled-checked` gray fill restored (139,139,139 light / 120,120,120 dark); `switch.disabled` + `switch.disabled-checked` render (track+thumb) both themes. **BUT** `checkbox.default.disabled` still shows stray dark + navy `(10,66,156)` + bright-blue/orange specks inside the 16×16 box (light AND dark). Root cause identified (N1 below). |
| **D5** native-text state editors paint no text | **STILL DEFECT — broader than diagnosed** | `*.readonly` and `*.invalid` editors paint **zero text** on native-text AND forms pages, both themes, even after a forced relayout. `field.*#control` inputs (flagless, seeded `field@example.com`) are **also empty** — the mechanism is not flag-bound. `input/textarea.focus-visible` render text squished into a ~1–2px band at the element top (degenerate peer band). `single-line.placeholder` ~empty (~14px vs ref's 124 muted px) while `multiline.placeholder` renders. `*.disabled` empty is **consistent with the frozen ref** (ref disabled inputs also show no text). Model holds the text (snapshot labels correct; peer draw/bounds-side failure). Refs: ref `readonly` 286 px / `invalid` 212 px vs native 0. |
| **D6** field.* geometry | **VERIFIED** | `field.default` 288×83, `.invalid` 288×82, `.disabled` 288×83, `field.horizontal` 288×37 — all exact; parts at rel x=0 (edge-to-edge); `field.horizontal#thumb` inside box (right edge 639 < 640). |
| **D7** alert.* geometry | **VERIFIED** | `alert.default` 320×60, `alert.destructive` 320×80 exact; destructive renders title + **two** description lines (3 text bands y950/972/992). |
| **D8** row overflow | **VERIFIED** | `chunks(2)` wrap: all radio/field specimen boxes inside 964 client (max x+w = 784 < 900 viewport). |
| **D9** skeleton.default | **VERIFIED** | 224×40 (was 884×40) — `natural()`+Stretch-honors-Fixed works. |
| **Crash** | **not reproduced** | 3× (full-client WGC screenshot → scroll +40 → resize → screenshot → resize) on native-text with 2 partially-clipped editors (top + bottom edge). Process stayed alive (pid 19288). Consistent with rework's bounded claim. |

## NEW defects found this pass

| # | Where | Finding |
|---|-------|---------|
| **N1** | Forms checkbox row | **Specimen labels collide.** 7 labeled checkboxes at 32dp pitch but labels measure ~50–90dp (`checkbox.default.focus-visible#label` = x136→213.57). Every label overflows its 16dp element box into the next siblings: all 7 labels paint on top of each other in the same y-band → illegible glyph pile; and because unchecked/disabled box interiors are transparent, the overlapped text shows **inside** the boxes — the D4 specks in `checkbox.default.disabled` (and left-edge specks in `.invalid`). Ref: each frozen checkbox+label specimen is ~300dp apart. Fix class: give each specimen a sized cell (like radio's 360dp boxes) or content-hug pitch ≥ box+label. Both themes. |
| **N2** | `typography.small` | **Regression, core tier.** Native 720×14 vs contract **91.9×18** — the label stretches to the 720 column width (missing the content-hug row wrapper that `inline-code` got) and under-measures height by 4dp. Possibly introduced/never-compared before — core-tier contract miss either way. |

## Minors re-check

- `kbd` dark raster: cap now fills its box (was ~⅓ coverage) — **improved**;
  glyph ink still sparser than ref (79%→~closer), still a soft mismatch.
- `switch.default.invalid` ring: saturated red → **softened** muted dark-red
  band (light: pale-pink + thin edge; dark: (108,46,47)/(151,73,75)) —
  acceptable, ref's dark invalid switch is near-invisible anyway.
- `typography.inline-code` −50.95w/+2.02h — **unchanged** (still narrow).
- `badge.*` −1.76, `button.*` ±0.5–2.5 widths — same accepted DirectWrite-vs-
  Chromium text-metric class. `kbd.group` improved −8.46→−3.48.
- `radio-group.*#item[option-*]` parts now carry ids — resolved.
- No `text-input` devctl cmd → live selection still unverifiable (honest gap,
  unchanged).

## Regression sweep vs the systemic `natural()`/LayoutSpec change

Geometry (size-only, 0.5dp tol) re-sampled on all implemented pages, both
themes where relevant:

| Page | Elements | Height fails | Width fails | Notes |
|------|----------|--------------|-------------|-------|
| components (light) | ~46 staged | **0** | text-metric class only (badge −1.76, buttons ≤2.5, kbd) | D3/D7/D9 exact; h1..p unchanged |
| forms (light) | 37 staged | **0** | **0** | all inputs/toggles/radio/fields exact; field fixes verified |
| native-text (light) | 14 | **0** | **0** | all 360×32/64/78 exact |
| typography (light) | 10 staged | small −4 | small +628 (N2), inline-code −50.95, code-block −8 (known container-adapt) | h1–h4/p/lead/muted exact |
| absent-honest | select.*/slider.*/typography.{large,blockquote,list,table} | — | — | tier later/reference-only, not staged |

Net: the LayoutSpec change landed cleanly — the only fresh geometry miss is
`typography.small` (N2); every height on components/forms/native-text is
exact. Slice-1 specimens (button/icon-button/card/separator/kbd/typography
heads) preserved.

## Element coverage

| Page | Checked | Geo PASS | Open issues |
|------|---------|----------|-------------|
| components L+D | ~46 ×2 | all but text-metric widths | kbd-dark raster (minor) |
| forms L+D | 37 ×2 | 37/37 | N1 label collision + D4 residual, D5 editors |
| native-text L+D | 14 ×2 | 14/14 | D5 text paint |
| typography | 10 spots | 8/10 | N2 small, inline-code |
| live | scroll/scroll-into-view/resize/focus/click/key | — | all verified; Delete into invalid editor |

Evidence: `evidence2/` (checkbox-row collision, disabled native-vs-ref zooms,
nt readonly/invalid native-vs-ref, typography-small, kbd dark) + full capture
set `W:\devin_folder\argon2r\`.

## VERDICT: NEEDS_REWORK

Blockers:
1. **D5** — `readonly`/`invalid` editors paint no seeded text (both themes,
   forms + native-text + field controls; `focus-visible` squished to a 1–2px
   band). Model has the text; peer draw/bounds side fails. The rework's
   "resolved by scroll fix" diagnosis was wrong — all these editors are
   fully visible in an unfilled viewport and still render nothing.
2. **N1/D4-residual** — checkbox specimen labels collide (32dp pitch vs
   ~90dp labels): illegible text pile + bleed-through artifacts inside
   `checkbox.default.disabled` (both themes) — the D4 symptom persists for
   the unchecked-disabled case.
3. **N2** — `typography.small` core-tier geometry miss (720×14 vs 91.9×18),
   same content-hug class as the still-open `inline-code` minor.

Non-blocking but noted: kbd dark raster still thinner than ref; single-line
placeholder ~empty (multi-line renders); `*.disabled` textlessness matches
the frozen reference (not a defect).
