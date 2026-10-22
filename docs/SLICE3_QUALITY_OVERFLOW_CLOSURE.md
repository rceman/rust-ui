# Slice-3 quality / overflow / text-fidelity closure record

**Base main:** `d34253f` (post-Slice-3 publication, evidence-corrected)
**Production HEAD:** `38469bf` — layout corrections only (gallery field bounds, popup spacing recipe, separator bleed)
**Review closures:** Astra `SLICE3_QUALITY_OVERFLOW_SYSTEM_APPROVED` (bounded re-review after F1); Argon `SLICE3_TEXT_AND_OVERFLOW_VISUALLY_APPROVED` (37 PNGs, full-resolution pixel inspection)
**Evidence:** `benchmark/results/shadcn-native-gallery-slice3-quality-closure/`

## Root causes and fixes

| Defect | Root cause | Fix |
|---|---|---|
| SettingsNav Appearance controls off-viewport | `Field { width: Some(dp(640)) }` — a `Fixed` row ignores the pane's real width (~580 at 964w), pushing the trailing Select/Switches outside | `width: None` → `Fill(1)` row bounded by pane; content column `Fill(1)` shrink lane; spacer removed (`examples/gallery.rs`) |
| Select popup clipped in 360dp stage ("EU West" cut) | implicit `Space::Sm` (8dp) gap between every popup/group column child — content measured ~280dp+ | `.gap(Space::None)` on popup content + group columns (`src/ui.rs`) |
| Dropdown clipped in 464dp stage ("Panel right", "Delete workspace" cut) | same implicit-gap defect | same `.gap(Space::None)` → content 387dp, contract-exact |
| Select separator zone 9dp vs contract 17dp (Astra F1) | select separators sit between `p-1` groups: zone = 4+4+1+4+4; the `Space::Xs` wrapper modeled only `my-1` | wrapper `Space::Sm` (8+1+8=17); popup h=241 contract-exact; adjacent-group caveat documented in code comment |
| Select separator escaping popup rect | line width `pw+8` exceeded popup bounds | `w = popup_width` — bleed reaches the border, stays inside |

## Verification results

- **Text QA:** all six pages both themes — no clipped glyphs/descenders, no unwanted ellipsis/wrapping/overflow (Argon pixel inspection).
- **Full-page completeness:** true per-page client heights — components 1515, forms 1690 (all six Select specimens visible), navigation 1176, typography 775, overlays 1036, native-text 1048.
- **Compact 800dp:** forms/navigation/overlays + navigation-collapsed usable, no horizontal overflow, expanded+collapsed SettingsNav verified.
- **Popup contract:** select h=241 inside 360 stage (bottom 418<468); menu h=387 inside 464 stage (bottom 996<1004); all items/shortcuts/destructive rows visible.
- **Bounds QA:** `tools/bounds_check.py` — 0 violations across 20 snapshots (client containment, popup-child containment, zero-size controls).
- **Transcript:** `interaction/devctl-final-clean.txt` — zero unexpected FAIL (tab pointer-focus via `press-at`; parts carry no automation_id).
- **Resize sanity:** 3 bounded resize+wait-idle cycles, no freeze/loop (`resize-sanity/`).

## Gates

Linux: fmt clean, `check --locked --examples` clean, lib 123/123.
Windows: lib 165/165, devtools lib 194/194, `devctl_windows` 1/1 (~40s).

## Launcher

- Desktop shortcut: `rust-ui Gallery.lnk` → `tools/windows/run-gallery.ps1`.
- Builds authoritative `/home/therceman/git/rust-ui` via native cargo.exe into `W:\devin_folder\rust-ui-gallery-target`, launches `rust-ui-gallery.exe`; build failure shows the error — no stale launch. Validated end-to-end. See `docs/MANUAL_GALLERY_TESTING.md`.

## Known residual advisories (non-blocking)

- Menu separator zone (9dp) unchanged and contract-exact, though not re-demonstrated in a bounds dump (menu closed in fullpage snapshots — open state proven by `states/*/dropdown-menu.open.png` + transcript).
- No unit test yet pins popup-height geometry (bounds QA covers it in evidence).
- `parts` rects aren't walked by `bounds_check.py`; `"text-input"`/`"action"` kinds aren't in its ZERO_SIZE set.
