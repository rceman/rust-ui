# Shadcn Native Gallery — Slice 3 quality/overflow closure evidence

Production candidate: quality-closure HEAD on `main` (layout fixes only —
SettingsNav field bounds, popup spacing, separator bleed); base Slice-3
production `79816a4`.
Frozen reference: `reference/shadcn-gallery-v0.1/` @ `2e14e417a69ef8967b8fb17120282e3de6449fae` (unmodified).
Capture host: native Windows 11 VM, scale 1.0, WGC, explicit `shell.theme.*` clicks, `wait-idle` barriers, asserted devctl results.

## Layout

- `fullpages/{light,dark}/<page>.png` — true full-page captures at per-page client heights measured from deepest content bounds (no below-fold specimens):
  components 1515, forms 1690 (includes all six Select trigger specimens), navigation 1176, typography 775, overlays 1036, native-text 1048, all at 964dp client width.
- `compact-800/{light,dark}/` — compact-desktop QA at 800dp client: forms, navigation, overlays, navigation-collapsed (48dp rail).
- `states/{light,dark}/` — stage-bound crops: `select.open` (884×360), `dropdown-menu.open` (884×464), `settings-nav.expanded`/`collapsed`, `tabs.*.active`, `button.default`, `card.default`.
- `bounds/report.json` — `tools/bounds_check.py` over all 20 snapshot-layout dumps: 0 violations (client containment, popup-child containment, zero-size controls).
- `interaction/devctl-final-clean.txt` — clean end-to-end devctl transcript; no unexpected FAIL.
- `resize-sanity/` — bounded resize timing.

## Corrections proven by this set

- SettingsNav Appearance fields now respect pane width — Theme Select / Animations / Compact switches visible inside the pane (was `Fixed(640)` overflow).
- Select popup fits its frozen 360dp stage at contract-exact 241dp content height; Dropdown fits 464dp at contract-exact 387dp — implicit `Space::Sm` container gaps removed from popup/group columns; select separator wrapper models the full 17dp frozen zone (group p-1 + `my-1` + line + `my-1` + group p-1), dropdown separator keeps the contract 9dp `my-1` zone.
- Every fullpage genuinely includes its whole implemented page.