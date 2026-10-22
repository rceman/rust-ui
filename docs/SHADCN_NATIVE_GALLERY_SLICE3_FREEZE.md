# Shadcn Native Gallery — Slice 3 freeze record

**Candidate SHA:** `79816a4` on `agent/shadcn-native-gallery-slice3-opus55`
**Based on:** `4e380975e8b544781bb6fcfd939a0e76fafe3105` (published pre-Slice-3 `main`)
**Frozen reference:** `reference/shadcn-gallery-v0.1/` (unchanged — payload `2e14e417`)
**Reviews:** Astra **SYSTEM_APPROVED** (round 3, advisories landed `79816a4`); Argon **VISUALLY_APPROVED** (structural findings fixed `6909d99`) + **`SLICE3_LIGHT_THEME_VISUAL_EVIDENCE_CLEARED`** (post-publication correction closure, see below)

## Post-publication evidence correction (RUI_LEAD_P6-261022-151300)

The originally published `visual/light/*` set was **invalid/mislabeled**: `evidence-slice3.ps1`'s capture cycle clicked `shell.theme.dark` for dark passes but clicked *nothing* for light passes — the Gallery boots `ThemeMode::System` and the host OS is dark, so every "light" PNG rendered dark. Root cause is a **capture-automation defect only** — manual `click shell.theme.light` verification proves Light resolves correctly (page `#fff`, primary `#171717`, sidebar `#fafafa`); no production code changed (`79816a4` stands).

- Corrected captures: all `visual/fullpages/light/*` (6) + `visual/states/light/*` (8) recaptured with explicit `shell.theme.light` clicks and asserted devctl results; `states/{light,dark}/{select,dropdown-menu}.open` recaptured at current overlay-stage geometry (884×360/464 — the stale dark set held pre-stage trigger-only crops); `fullpages/dark/overlays.png` relocated from a path-collapsed file after Argon found the canonical file stale. Flat-name accident files deleted.
- Capture-order note: the theme click is an outside press that dismisses only the topmost light-dismiss popup (the menu) — `select.open` captures legitimately show the listbox still open; `dropdown-menu.open` re-clicks the trigger before capture.
- Original invalid set is superseded by `5e3261c`/`0211dbe` on `main`; the mistake is preserved in history, not rewritten.
- Fresh bounded Argon review on the corrected set: light genuineness, Slice-3 light parity vs contract, Slice-1/2 light sanity, stage chrome — **`SLICE3_LIGHT_THEME_VISUAL_EVIDENCE_CLEARED`** on `0211dbe`.
- Ledger reconciliation: Slice-3 rows now carry `Windows=verified`, `Visual/Interaction parity=partial` matching the VERIFIED convention; `Gallery shell` `PLANNED`→`PARTIAL` (shell shipped as the devctl acceptance app; full catalog completeness lands with later slices).

## Post-publication quality/overflow closure (RUI_LEAD_P7-261022-180000)

A deeper inspection found real layout defects missed by earlier visual review:

- **SettingsNav overflow**: `Field.width = Fixed(640)` pushed Appearance-pane controls off-viewport → `Fill`-bounded row + `Fill(1)` content lane (gallery recipe, no production semantics change).
- **Select/Dropdown stage clipping**: implicit `Space::Sm` gaps between popup/group column children inflated select (~280) and menu (~483) contents past the frozen 360/464 stages → `.gap(Space::None)`; select content now 241 and menu 387, both contract-exact.
- **Separator zone**: select separator models the full 17dp frozen zone (group `p-1` + `my-1` + line + `my-1` + `p-1`); dropdown keeps contract 9dp. Separator line width `pw+8`→`pw` (stays inside popup rect).
- **Tooling**: `tools/bounds_check.py` overflow QA — 0 violations / 20 snapshots.
- **Evidence**: `benchmark/results/shadcn-native-gallery-slice3-quality-closure/` — true per-page fullpage heights (forms 1690 includes all six select specimens), compact-800 QA both themes, clean devctl transcript, resize sanity.
- **Reviews:** Astra `SLICE3_QUALITY_OVERFLOW_SYSTEM_APPROVED` @ `38469bf`; Argon `SLICE3_TEXT_AND_OVERFLOW_VISUALLY_APPROVED`.
- **Launcher:** Desktop `rust-ui Gallery.lnk` → `tools/windows/run-gallery.ps1` builds+launches authoritative source; documented in `docs/MANUAL_GALLERY_TESTING.md`.
- Full record: `docs/SLICE3_QUALITY_OVERFLOW_CLOSURE.md`.

## Scope delivered

| Area | Items |
|---|---|
| Popup foundation | `ui.popover` — KIND_POPUP=18, anchored placement (side/align/offset, flip+viewport-clamp, scroll-clip escape), deferred paint-last/hit-first order, outside-press + Escape → `OpenChange(false)` (topmost layer only), `focus_on_open` into content + focus restore, modal scrim + hit/focus/scroll gate, nested popups at arbitrary depth, deterministic multi-close restore (`open_seq`) |
| Select | `ui.select` — ComboBox trigger + Listbox popup, groups/labels/separator/disabled/selected/highlighted parts, focus enters listbox on open (selected option), arrow/Home/End nav, Enter/Space pick→close→value→focus restore, Escape/outside dismiss |
| Dropdown Menu | `ui.dropdown_menu` — MenuButton trigger + Menu popup; normal/disabled/destructive/shortcut/checkbox/radio/label/separator items, arrow/Home/End nav with disabled-skip, Escape/outside dismiss + focus restore |
| Tabs | `ui.tabs` — default muted-pill + line underline variants, keyed triggers, click→focus+activate, arrow auto-activate (Radix automatic), Home/End, roving TabList |
| SettingsNav | `ui.settings_nav`/`item` — wide 256dp + collapsed 48dp rail, groups+labels, icon+label items, active/hover/focus-visible/disabled states, content pane |
| Gallery | Navigation + Overlays pages (frozen `.overlay-stage` chrome on select/menu specimens), Forms select-trigger specimens, `RUI_GALLERY_PAGE` boot-page env, `RUI_SETTINGS_NAV_COLLAPSED` rail env |
| devctl | `press-at`/`scroll`/`scroll-into-view`/`resize`/`perf`/`perf-reset` verbs documented; screenshot bounded retry (`CAPTURE_FAILED`-only, 3 attempts); `Toggle.checked` exposed in snapshots |
| Icons | +LogOut, Trash2, User, Settings, Palette, Bot, Mic, Plug, SlidersHorizontal; figure-op guards in `draw_icon_at` |

## Key implementation notes

- **Popup deferred layout** — popup nodes defer to an end-of-run placement pass: anchor resolved by automation id against committed rects (intersected with the anchor's scroll clip), `resolve_popup_placement` is the shared platform-neutral policy. Children lay out into content-minus-insets; a `nested` worklist drains popups staged inside popups to arbitrary depth. Modal takes the viewport rect (scrim), non-modal the content rect; both land last in `order` = paint-last/hit-first.
- **Popup lifecycle bookkeeping** — `open_popups: slot → PopupBook {id, focus_return, focus_pending, open_seq}` reconciled each turn between dirty-drain and `sanitize_focus`: replaced slots carry the displaced `focus_return`; vanished books restore newest-layer-first (deterministic); new books insert *after* restore and record the post-restore owner; `focus_pending` resolves in paint order after relayout so the topmost popup deterministically wins.
- **Semantic surface** — `SemanticRole::{ComboBox,Option,MenuButton,MenuItem,MenuItemCheckbox,MenuItemRadio,TabList,Tab,SettingsNavItem}`; `selected`/`checked`/`expanded` flow through `ChildBuild` to UIA SelectionItem/Toggle/ExpandCollapse and devctl snapshots; tab items are contract parts (`owner#list.tab[key]`).
- **Icon pipeline** — `draw_icon_at` guards every figure-continuation op (`LineTo`/`QuadTo`/`CubicTo`/`ArcTo`) behind an open figure; `Icon::Palette` begins with a real `MoveTo` (a leading `ArcTo` was a D2DERR_WRONG_STATE crash).
- **Stage chrome** — `overlay_stage` reproduces the frozen `.overlay-stage` panel (r14, `bg-muted/40` via new `Muted40` role, 1dp `foreground/10` ring, fixed demo height); contract element bounds = stage box.
- **Determinism** — `open_seq` makes multi-open/multi-close ordering commit-deterministic; new-book insertion iterates sorted slots; `topmost_popup` ranks by `open_seq`.

## Devctl fixture retirement (addendum RUI_LEAD_P5-261022-124754)

| Item | Result |
|---|---|
| Deleted files | `tests/support/devctl_fixture.rs` (67 lines), its `[[bin]]` Cargo target `rust-ui-devctl-test-fixture` |
| New launch mechanism | `tests/devctl_windows.rs` launches `env!("CARGO_BIN_EXE_rust-ui-gallery")`; `src/bin/rust-ui-gallery.rs` is `include!("../../examples/gallery.rs")` — one authoritative Gallery compilation unit, no second UI |
| Gallery IDs used | `shell.root`, `shell.nav.{components,forms,navigation,typography,overlays,native-text}`, `button.default`, `input.default.filled`, `checkbox.default.unchecked`, `input.default.disabled`, page specimens |
| Protocol tests retained | launch, authenticated session, handshake, wait-idle, tree, rect, TARGET_NOT_FOUND, token/protocol rejection, shutdown, cleanup |
| Application E2E retained | hover/leave/press/release/click on `button.default` (runtime state, no `Presented`), focus + Space toggles `checkbox.default.unchecked` (`checked` snapshots), `shell.nav.forms` click → real `Msg::SetPage` retained-tree swap, native RichEdit editing on `input.default.filled` → committed TextValue, snapshot-layout determinism, screenshot, compare-layout (1dp mutation), compare-image (1-px mutation), compare-element (frozen reference ↔ native `button.default`), capture backend metadata |
| 3-cycle result | 3 sequential launch→capture→shutdown cycles, fresh session/temp root per cycle — **1/1 PASS, 39.0s** |
| Resource/idle result | handles ≤ steady+2 across 6 warm captures/cycle; 2s idle CPU + counters-equality asserted; `native_timer_fires` counter checked; clean shutdown + stale-receipt cleanup per cycle |
| Search proof | `rg 'rust-ui-devctl-test-fixture\|devctl_fixture\|probe\.(button\|input\|label\|surface\|root)\|Fixture text'` → matches only in immutable historical docs (`DEVCTL_V0_1_*`, `SHADCN_NATIVE_GALLERY_SLICE1_*`) — zero live source/test/example/Cargo references |

### Terminal answers (addendum § terminal criteria)

1. `tests/support/devctl_fixture.rs` absent — **yes**.
2. Cargo target `rust-ui-devctl-test-fixture` absent — **yes**.
3. All live fixture identifiers absent — **yes** (historical docs only).
4. `tests/devctl_windows.rs` launches Gallery — **yes** (`CARGO_BIN_EXE_rust-ui-gallery`).
5. Real Gallery IDs for interaction proof — **yes** (`button.default`, `checkbox.default.unchecked`, `shell.nav.forms`, `input.default.filled`).
6. Real Gallery native editor flows — **yes** (`input.default.filled` RichEdit commit).
7. Snapshot/layout/image/element comparisons preserved — **yes**.
8. Resource/idle/boundedness checks preserved — **yes**.
9. Fixture-specific exact-node/count/counter assumptions removed — **yes** (no `Clicks:`, no `== 5` node count).
10. One authoritative Gallery implementation — **yes** (`include!` same compilation unit).

Astra explicitly answered the 8 topology questions YES (fixture removed-not-renamed; real Gallery E2E; protocol/app separation; runtime-not-Presented states; deterministic Cargo discovery; launch isolation; boundedness preserved; single implementation).

## Gate evidence (candidate `79816a4`)

- Linux: `cargo fmt --check` clean; `cargo check --locked` clean; `cargo test --locked --lib` **123/123**.
- Windows native: `cargo test --locked --lib` **165/165**; `--features devtools --lib` **194/194**; `cargo check --locked --all-targets --features devtools` clean; `cargo test --locked --test devctl_windows --features devtools` **1/1 (~39s)**.
- Interaction transcript: `benchmark/results/shadcn-native-gallery-slice3/interaction/devctl-slice3-verify.txt`.
- Visual: `benchmark/results/shadcn-native-gallery-slice3/visual/` — 6 pages × 2 themes fullpage + 8 state crops × 2 themes; resize-sanity 3× resize+wait-idle = 130ms wall.

## Invariants held

- Frozen reference untouched; Slice-1/2 specimen geometry preserved.
- No Slice-4 components (Tooltip/Popover/Dialog absent as components; the generic `ui.popover` layer is the shared foundation they will consume).
- Platform neutrality: zero `HWND`/`WM_*`/`RECT`/`WPARAM` outside `src/platform/`; `resolve_popup_placement`/`nav_target`/`node_insets`/`presented_or` shared.
- No forbidden tech; windowless RichEdit preserved; no synthetic devctl UI.

## Deviations / known limitations

- `SPIKE_ARCHITECTURE_DEVIATIONS.md` entries stand; no new deviations.
- Tab-list width +1.9px/tab and select option interior ±1px are the documented DirectWrite↔Chromium measure/raster class (Argon findings 3-4).
- `settings-nav.default` ids the sidebar only (256/48); the content pane is a sibling (contract-faithful id mapping).
- Reference pages host out-of-slice specimens (breadcrumb/pagination/toggle-group/button-group on Navigation; dialog/alert-dialog/popover/tooltip stages on Overlays) — later slices.
- `ui.popover` inside an `ui.action` draw closure is inert by design (documented constraint; shipped composites anchor siblings).
