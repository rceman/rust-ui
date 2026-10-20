# Slice-1 regression sample — Slice-2 candidate 8f3f71b

compare-layout vs frozen `reference/shadcn-gallery-v0.1/contract.json`,
absolute mode, tol 0.25dp. Specimens: button.default, icon-inline-end,
icon-button.default, card.default, card.default.size-sm, typography.h1,
typography.code-block — Light + Dark.

## Result: PRESERVED

Every sampled Slice-1 specimen's measured size is IDENTICAL to the
frozen contract (height delta 0.0 everywhere; widths exact except the
±0.5dp glyph-measure noise already present in approved Slice-1
evidence). icon-button 32×32, card 288×175 / 288×152.25 exact.

x/y deltas are large and EXPECTED: the Components page now interleaves
the frozen Slice-2 specimens (badges, separators) in contract order and
lives inside `page.scroll`, so absolute positions differ — that is page
composition, not specimen regression.

`typography.code-block` width 720 vs 728: stretch-width element
adapting to the scroll viewport's inner width (same mechanism as h1 —
all stretch specimens report 720 = the new content width). A stretch
element tracking its container is correct behavior, not a size
regression.

Snapshots in this dir are the post-change capture sets used for the
comparisons.
