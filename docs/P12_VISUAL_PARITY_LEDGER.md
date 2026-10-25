# P12 Visual Parity Discrepancy Ledger

Canonical inventory of native-vs-frozen-reference (`shadcn-gallery-v0.1`,
`base-nova`) discrepancies. Every entry carries a stable ID, expected vs
actual behavior, root-cause subsystem, verification approach and status.

- Base: `main` `873c9996a908d5307c364925bfec6b4c1c40eb54`
- Branch: `feat/p12-automated-visual-parity`
- Evidence root: `benchmark/results/p12-automated-visual-parity/`
- Contract authority: `reference/shadcn-gallery-v0.1/contract.json`
  (`contract/0.2`), 227 elements, 9 capture keys.

Status vocabulary: `OPEN` — defect confirmed, unfixed ·
`FIXED` — corrected and evidenced · `ACCEPTED` — approved difference ·
`DEFERRED` — documented, out of P12 scope · `NOISE` — documented
renderer-class difference (DirectWrite↔Chromium), not a defect.

## Severity vocabulary

- **S1** — blocking visual/semantic defect vs contract.
- **S2** — real difference, bounded (metrics/state/color).
- **S3** — cosmetic / tolerance-class residual.

---

## Typography & fonts

| ID | Specimen(s) | Expected | Actual | Root cause | Files | Sev | Verify | Status |
|---|---|---|---|---|---|---|---|---|
| P12-T1 | all editable peers (input.*, textarea.*, native-text.*) | Geist 400 | Segoe UI — `PeerConfig.face` hardcoded | peer config | `src/platform/win32/peer.rs` | S1 | rendered-font proof — Geist resolves in msftedit; ink w=74px ≈ contract 72 advance | **FIXED** |
| P12-T2 | input/textarea text em | font_size 14px + line_height 20px | `yHeight = line_twips·23/30` = 230tw ≈ 15.3px em — pitch↔em coupling under TXTBIT_D2DDWRITE (all para line-spacing controls inert; measured Segoe-era) | peer char-format calibration | `src/platform/win32/text/peer_format_create.rs` | S2 | measured: pitch is font-metric+leading (Geist 14px → ≈20px = contract); pf line-spacing channel proven INERT; yHeight now size-exact | **FIXED** |
| P12-T3 | peers without authored line_height | font 14px | `size_twips = 14·20 = 280tw` = 14pt = 18.67px — pt/px unit defect | peer config | `src/platform/win32/peer.rs` | S2 | measured: msftedit maps pt→DIP 1:1 under D2DDWRITE — 280tw = 14px em (was correct; NOT a defect) | **NOISE** — verified correct |
| P12-T4 | input.default.placeholder, textarea.*.placeholder, select placeholder value | Geist 14px muted | placeholder drawn via DirectWrite at `Exact(14dp)` while peer text = Segoe — cross-face metric mismatch on same control | separate draw path | `src/platform/win32/render/frame_editor.rs` | S2 | placeholder vs filled-specimen run metrics — recheck post-Geist (both paths now Geist 14) | OPEN |
| P12-T5 | typography.*, code specimens | Geist / Geist Mono | resolved via private registration — verify no silent fallback on parity-critical runs | registration vs proof | `src/platform/win32/fonts.rs`, `render/mod.rs:30,219` | S3 | font-proof per text-bearing node | OPEN |
| P12-T6 | DPI-dependent text positioning | scale-exact | `scale_factor` handled; verify 1.25× text runs land on snapped lattice | scale plumbing | render/peers | S3 | 1.25× capture vs canonical lattice | OPEN |
| P12-T7 | wrap/truncation | contract wraps | alert destructive uses authored `\n`; verify remaining wraps | text layout | ui recipes | S3 | per-element run-line comparison | OPEN |

## Input / Textarea / Field

| ID | Specimen(s) | Expected | Actual | Root cause | Files | Sev | Status |
|---|---|---|---|---|---|---|---|
| P12-I1 | input.default.* | text rect {x:10,y:5,h:18}, baseline 19 in 32px | baseline/pitch driven by T2 — verify after font fix | peer pitch + content insets | layout `editor_content_rect` | S2 | OPEN |
| P12-I2 | placeholder vertical/horizontal alignment | same run rect as filled value | separate draw at content-rect origin — verify vs filled runs | draw origin | `render/frame_editor.rs` | S2 | OPEN |
| P12-I3 | textarea.default.* | Geist 14px, pitch 20, 3-line 78dp specimen | line pitch via T2 + multiline clip band | peer + chrome | `builders_input.rs`, text/ | S2 | OPEN |
| P12-I4 | disabled states (input.default.disabled etc.) | contract disabled fg/bg/placeholder | reported mismatch — measure vs contract box/colors | recipe state | `src/style/` recipes | S2 | OPEN |
| P12-I5 | invalid + focused border/ring | destructive ring precedence | ring model approved pre-P12 — verify unchanged vs contract outline/ring parts | ring resolver | style/render | S3 | OPEN |
| P12-I6 | field label/description/error alignment | contract part rects | verify parts vs contract `parts` | field recipe | `src/ui/` field | S3 | OPEN |
| P12-I7 | caret + native selection | contract selection band colors | sel_bg/sel_fg authored from contract tokens; band blend verified pre-P12 — re-verify under Geist | peer selection path | `peer_selection_lifecycle.rs` | S2 | OPEN |

## Borders, focus & control states

| ID | Specimen(s) | Expected | Actual | Root cause | Files | Sev | Status |
|---|---|---|---|---|---|---|---|
| P12-B1 | independent border edges / bottom-border appearance | contract border_width[] per edge | unified 1dp border recipe — verify bottom edge on card footer / separator-adjacent controls | border patch model | style/render | S3 | OPEN |
| P12-B2 | corner radii | contract border_radius[] | r10 inputs, r14 cards, r6 controls — verify per element | recipes | style | S3 | OPEN |
| P12-B3 | focus-visible ring geometry | radius + 3px spread, contour-following | approved model — regression check only | ring draw | render | S3 | OPEN |
| P12-B4 | invalid-ring precedence | invalid wins over focus | `ring_color_role(invalid)` approved — regression check | ring resolver | style | S3 | OPEN |
| P12-B5 | hover/pressed/disabled colors | contract state colors | verify state recipe resolution | style recipes | style | S3 | OPEN |
| P12-B6 | staged vs real states | Presented ≠ runtime | `presented` resolved at paint only — approved | node model | node | S3 | ACCEPTED |

## Select / dropdowns / navigation

| ID | Specimen(s) | Expected | Actual | Root cause | Files | Sev | Status |
|---|---|---|---|---|---|---|---|
| P12-S1 | select trigger value/placeholder alignment | contract `value` part rect | value text via T1/T2 class or draw path — verify | select trigger | ui select | S2 | OPEN |
| P12-S2 | select popup content movement / positioning | anchored side=bottom offset 4, 208dp lane, clamp | reported movement — verify placement vs contract parts | popup placement | `resolve_popup_placement` | S2 | OPEN |
| P12-S3 | popup clipping / viewport bounds | flip+clamp | approved model — verify at compact heights | popup placement | layout | S3 | OPEN |
| P12-S4 | dropdown menu spacing/surface | contract 9dp separator zone, item spacing | quality-closure fixed heights — verify items | menu recipe | ui dropdown | S3 | OPEN |
| P12-S5 | tabs geometry + active/focus presentation | pill/line variants | +1.9px/tab documented DirectWrite class | tabs recipe | ui tabs | S3 | OPEN |
| P12-S6 | settings-nav wide/collapsed geometry | 256/48dp, contract items | clipping/overflow addressed pre-P12; collapsed-rail states re-verify | settings_nav | ui + gallery | S2 | OPEN |
| P12-S7 | light-dismiss + focus restore | Radix semantics | approved model — regression check | popup lifecycle | turn.rs/input_pointer.rs | S3 | OPEN |

## Hover & motion

| ID | Specimen(s) | Expected | Actual | Root cause | Files | Sev | Status |
|---|---|---|---|---|---|---|---|
| P12-M1 | hover fade-out symmetry | reference transitions (CSS) | reported abrupt/asymmetric hover leave — native has no transition engine; investigate enter/leave paint symmetry vs contract staged states | hot tracking + repaint | input_pointer + render | S2 | OPEN |
| P12-M2 | reduced-motion / idle repaints | no loops, no wakeups when static | verify no continuous scheduling | timers | win32 timers | S3 | OPEN |

## Deferred runtime observation

| ID | Specimen(s) | Expected | Actual | Root cause | Files | Sev | Status |
|---|---|---|---|---|---|---|---|
| P12-D1 | first-child Box under page.scroll/0 after Navigation visit | `part_key` reflects current page | reused Box keeps stale `part_key="list"` (devtools-visible; paint unaffected) | reconcile_nodes child-slot reuse without part_key rewrite | `src/runtime/reconcile_nodes.rs:163-206` | S2 | OPEN — evaluate parity impact, then focused fix if in-scope |

## Historical (previously fixed — regression watch only)

| ID | Item | Prior fix | Status |
|---|---|---|---|
| P12-H1 | multiline 3rd-line clipping | `line_height`→`line_twips`→yHeight calibration | HISTORICAL — recalibrate under Geist (T2) |
| P12-H2 | ring pillification | corner + 3px spread | ACCEPTED |
| P12-H3 | select/menu popup height clipping | `.gap(Space::None)` + 17dp separator zone | FIXED |
| P12-H4 | settings-nav appearance overflow | `Fill(1)` lane | FIXED |
| P12-H5 | invisible light selection | `win_bg` blend base | FIXED |
| P12-H6 | icon part edges / owner prefix | devtools parts fix | FIXED |
| P12-H7 | staged selection render | `apply_selection` forwarder | FIXED |
