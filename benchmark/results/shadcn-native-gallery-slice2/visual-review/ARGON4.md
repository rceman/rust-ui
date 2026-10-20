# Argon independent visual re-review — Slice 2 candidate (fifth pass, FINAL)

Candidate: `d16f1b5b00b9ac1830181d9e949dcd0d54373a4a` on
`agent/shadcn-native-gallery-slice2-opus55`, worktree `rust-ui-slice2`.
PROMPT_ID: RUI_ARGON_P2-261020-190000. Re-review of the single ARGON3
blocker **N3 — placeholder text never renders** — plus a final sweep for
new defects.

## Method (own drive, fresh binary — stale-binary trap avoided)

- `taskkill gallery`; **deleted** `rust-ui-wsl-target\debug\examples\gallery.exe`,
  rebuilt @ `d16f1b5` (`CARGO_INCREMENTAL=0`, `--features devtools`). New exe
  timestamped 22:52:58 — confirmed fresh, not a no-op (previous was 22:42:28).
- Live session `argon4` (pid 22292): `click`/`focus`/`key`/`scroll`/`resize`/
  `snapshot-layout`/WGC `screenshot`. Windows 980×1600 & 980×640 (clip work),
  scale 1.25, Light + Dark (explicit `shell.theme.*` — `ThemeMode::System`
  resolves **dark** on this machine). Reference truth: repo
  `reference/shadcn-gallery-v0.1/` contract.json + `screenshots/{light,dark}/`.
- Raster proof by pixel histograms / `ImageChops.difference` / ASCII ink maps.
  Geometry re-checked vs contract for every bound element, all four pages.

## N3 — VERIFIED

The Editor arm (`render.rs:989-1013`) draws the staged `placeholder` string
muted inside `editor_content_rect` when `snapshot.committed.is_empty()`,
inside the per-node `ClipGuard`. All four contract specimens now render:

| Specimen | Light ink / color | Dark ink / color | Extent |
|---|---|---|---|
| `input.default.placeholder` | 270 px, **#737373** | 270 px, **#a1a1a1** | ~39 px text run (="Email") |
| `textarea.default.placeholder` | 1049 px, **#737373** | 1049 px, **#a1a1a1** | ~190 px run (="Type your message here.") |
| `native-text.single-line.placeholder` | 456 px, **#737373** | 456 px, **#a1a1a1** | ~88 px run (="Type here…") |
| `native-text.multiline.placeholder` | 456 px, **#737373** | 456 px, **#a1a1a1** | ~88 px run |

- **Muted role exact** — the rendered stem tone is `#737373` light /
  `#a1a1a1` dark = the **frozen `muted-foreground` token srgb verbatim**
  (`token-bindings` light `#737373`/dark `#a1a1a1`; `style.rs:1120-1121`
  resolves the same). Text uses the peer's 14dp font. ClearType fringes are
  the app-wide native text AA (same as every `DrawTextLayout` label — not a
  placeholder-specific artifact).
- **Gate is live both directions.** Drove a real fill in reverse: focused
  `input.default.filled` (seeded `team@example.com`), `Home` + `Delete ×20`
  → committed became `""` → the "Email" placeholder **appeared**, raster
  identical to `input.default.placeholder` (270 ink, same extent, same
  fringe signature; snapshot `label:''`). Per EN_CHANGE→`native_commit`→
  `on_edit`→restage, the same committed-is-empty gate hides the hint the
  moment a real character lands — symmetric and proven end-to-end.
- **No bleed/ghost.** `filled`/`disabled`/`invalid`/`readonly` editors (all
  stage the same `.placeholder(...)` but have non-empty committed text)
  render **only** their value — zero hint overdraw. E.g.
  `nt.single-line.filled` = dark `alice@example.com` only;
  `input.invalid` = `bad value` + ring only.
- **Focus-empty correct.** The emptied+focused `input.default.filled` shows
  the placeholder unchanged (interior pixel-identical to the unfocused
  placeholder; the only diff = the focus ring at the border, 61 px) —
  matches shadcn's placeholder-stays-on-focus-empty. The **caret is the
  real Win32 system caret** (host `TxCreateCaret/TxShowCaret/TxSetCaretPos`)
  drawn XOR'd by Windows *outside* the D2D frame — it cannot be occluded
  by the placeholder draw. Its glyph is not conclusively capturable via
  WGC on a non-OS-foreground window (blink/OS-focus bound); the caret
  pipeline itself was proven in ARGON3 by commit-true edits.
- **Clip clean.** Fully-clipped → `TARGET_NOT_VISIBLE`, zero paint.
  Partial-clip (element straddling viewport top after resize+scroll): only
  the in-clip band shows; hint text clipped at the clip edge, no bleed.

## Final sweep — no new defect

- **Geometry regression:** 110 contract-bound elements re-checked vs
  contract (components 52, forms 37, native-text 14, typography 7) —
  **0 height fails, 0 width fails** outside the known minors and the
  documented `shell.*`/`select.*`/`slider.*` scope-outs. Slice-1 + all
  previously-verified Slice-2 specimens preserved.
- **Stability:** ~40 devctl ops incl. focus/edit-delete, scroll, resize
  1600→640→1600, WGC captures — process alive, `wait-idle` clean
  (`DwmFlush` barrier, 0 damage pending). No crash.
- **ARGON3 minors still in tolerance, unchanged:**
  - **M-a** `*.disabled` dark text: native neutral-ink tones 91–251
    (dominant 251) on bg-fill 40 vs ref 61–86 on 25 — same measured range
    as ARGON3, unchanged. Root cause clarified below (N4): ref disabled
    editors are *empty + muted placeholder* at opacity .5; gallery's are
    *filled values* rendered at near-full fg. Still a minor.
  - **M-b** `alert.*#title` part = text-run width — unchanged
    part-semantics minor.
  - **M-c** `typography.small` w −5.07; `typography.inline-code`
    w −50.95 / h +2.03 — unchanged DirectWrite/mono metric class.

## NEW finding this pass — N4 (fidelity caveat, pre-existing, not a regression)

My ARGON3 report mis-cited the frozen strings (I wrote the *gallery's*
staged strings as the ref's). The **frozen recorded text** for these
specimens is different — the gallery composes its own strings:

| Specimen | Frozen recorded text | Gallery renders |
|---|---|---|
| `input.default.placeholder` | `"Placeholder text"` (~105dp) | `"Email"` (~31dp) |
| `textarea.default.placeholder` | `"Write something..."` (~104dp) | `"Type your message here."` (~152dp) |
| `native-text.*.placeholder` | `"Placeholder text"` | `"Type here…"` |
| `input/textarea.native.*.filled` | `"Filled value"`, `"A line of prose"`, `"First line…"`, `"Focused input"`, `"Read-only value"`… | `"team@example.com"`, `"alice@example.com"`, `"Line one…"`, `"Thanks for the update…"` |
| `*.disabled` (input/textarea/native) | **empty + muted placeholder** `"Disabled input"/"Disabled textarea"` | filled value + disabled |
| `field.*#control` | `"you@example.com"` placeholder (empty) | `"field@example.com"` value |
| `label.default` | label `"Label"` + input w/ `"With a label"` ph | label `"Email"` + empty box (no hint) |
| `checkbox.*` labels | `"checked"`/`"unchecked"`/`"disabled checked"`… | `"Checked"`/`"Unchecked"`/`"Disabled checked"`… (casing) |
| `field.horizontal` | `"Two-factor auth"`/`"Require a second factor…"` | `"Enable notifications"`/`"Receive product updates."` |
| `field.default.disabled` desc | `"Locked for this account."` | `"We'll only use this for notifications."` |

**Assessment.** This is a gallery-composition fidelity divergence, **not**
a platform defect: the element geometry/state/role are contract-exact and
every specimen demonstrates the correct visual *state*. The coverage
contract binds state-selectors→specimens + geometry + style
(`coverage.json` rule; `bindings_live` = style props) — it does **not**
declare verbatim strings. The gallery demonstrably *does* copy frozen
strings verbatim where the author copied them (alerts, kbd, card,
typography, field label/description/error, button/badge labels) — so the
editor/placeholder/label divergences look like an oversight, not intent,
and the raster on ~20 contract specimens is not what a pixel-diff vs the
frozen PNGs would expect. **Pre-existing**: identical content shipped in
every candidate since `e68f61c` — the seeded editors were accepted as
app-model content in ARGON1 (D5: "seeded `alice@example.com` absent")
through ARGON3; it is not a regression of `d16f1b5`. It surfaces now only
because the placeholder specimens' *entire* visible content is the string.

**Recommendation:** bounded `gallery.rs` pass to align staged
placeholder/seed/label strings to the contract's recorded text (the
largest single-specimen raster correction is making `.disabled` editors
empty-with-placeholder instead of filled, matching the frozen state-kind).
If instead the milestone rules specimen content to be app-authored and
free, that should be recorded as an explicit composition deviation —
either way it is adjudication, not a platform fix. I am reporting it as a
fidelity caveat rather than a blocker: the contract's normative surface is
state/geometry/role, and blocking the final pass on a pre-existing
composition class accepted in all prior passes would be inconsistent.

## Coverage

- placeholders 4/4 × L+D — render muted, token-exact color, gated,
  clipped, no bleed; focus-empty placeholder+ring verified.
- seeded editors (filled/disabled/invalid/readonly/field) × L+D — value
  only, no hint overdraw.
- geometry 110 elements × both pages — exact.
- live ops: click/focus/Home/Delete×20 (real commit→restage), scroll,
  scroll-into-view, resize, WGC — stable.
- Honest gaps unchanged: select/slider + `typography.{large,blockquote,
  list,table}` absent (reference-only); live char-typing unavailable (devctl
  `key` subset is Enter/Space/Tab/Escape/arrows/Backspace/Delete/Home/End —
  the gate was proven by the live empty→placeholder flip instead);
  `shell.*` icon-only theme chrome differs from ref shell; caret glyph
  visibility not conclusively capturable via WGC.

Evidence: `evidence4/` — `input-ph-{light,dark}.png`,
`textarea-ph-{light,dark}.png`, `nt-{sl,ml}-ph-{light,dark}.png`,
`input-filled-emptied.png` (the live gate flip),
`nt-sl-ph-partial.png` (clip), `cmp-*-ph.png` (native vs ref zoom strips),
`ref-*.png` frozen crops. Full captures `W:\devin_folder\argon4\`.

## VERDICT: VISUALLY_APPROVED

The sole ARGON3 blocker **N3 is fixed and verified** — the staged
placeholder now renders muted (`#737373`/`#a1a1a1` = the exact frozen
muted tokens) when the committed doc is empty, gated correctly in both
directions, clipped, on both themes, with no bleed over committed text.
The final sweep is clean: 110 contract elements geometry-exact, minors
unchanged, no crash. **N4** (specimen content strings vs frozen recorded
text) is reported for the lead to adjudicate — align the strings in
`gallery.rs` or record a content-composition deviation — and is not a
defect of the `d16f1b5` fix under review.
