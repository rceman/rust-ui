# rust-ui component support ledger

Canonical answer to "what does rust-ui actually support today?". Update this
file in every component milestone. Specimen identity lives in the reference
catalog (`reference/shadcn-gallery-v0.1/src/catalog.js`, `tier` field); this
ledger summarizes support status and must not drift from it.

- **rust-ui scope: desktop only** (`PRODUCT_SCOPE.md`).
- **Desktop responsiveness: supported / required.**
- **Mobile platforms / product behavior: intentionally out of scope.**

Visual authority: shadcn `base-nova` reference v0.1 is FROZEN
(`reference/shadcn-gallery-v0.1/`; see `SHADCN_REFERENCE_V0_1_FREEZE.md`).
Reference completeness stays tier-specific below; freezing the reference
does not change Native API, platform, parity or implementation status.

## Vocabulary

| Field | Values |
|---|---|
| Tier | `core` (first native Gallery milestone) - `later` (planned desktop component, not first milestone) - `reference-only` (visual coverage, no native promise) |
| Reference | `none` - `partial` - `representative` (later-tier: representative states only) - `complete` (all visually material desktop states frozen) |
| Native API | `no` - `partial` (foundation primitive exists, not the shadcn component contract) - `yes` |
| Windows / macOS / Linux | `-` none - `foundation` (backend primitive exists, unverified vs reference) - `verified` |
| Visual / Interaction parity | `-` not measured - `partial` - `verified` (compared against the frozen reference) |
| Accessibility | `-` none - `foundation UIA` (Windows UIA provider exists for the primitive) - `verified` |
| Desktop-responsive | `-` not yet - `layout` (participates in Content/Fixed/Fill + min/max layout) - `modes` (compact/wide/collapsed presentations) |
| Status | `REFERENCE_ONLY` - `PLANNED` - `FOUNDATION_AVAILABLE` - `PARTIAL` - `IMPLEMENTED` - `VERIFIED` - `DEFERRED` |

`FOUNDATION_AVAILABLE` = a platform-foundation primitive exists (approved
Windows foundation `00dc29a`) but the shadcn visual/state contract is not
implemented or measured.

The `Desktop-responsive` column ("responsive layout target" - `layout`,
`modes` planned modes) describes intended target capability unless the
`Status` column says otherwise; no `PLANNED` / `REFERENCE_ONLY` /
`DEFERRED` row may read as shipped.

## Core

| Component / Pattern | Tier | Reference | Native API | Windows | macOS | Linux | Visual parity | Interaction parity | Accessibility | Desktop-responsive | Status | Notes |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Button | core | complete | yes | verified | - | - | verified | verified | foundation UIA | layout | VERIFIED | `ui.button`: all 6 variants (primary/secondary/outline/ghost/destructive/link) x normal/disabled/invalid, Xs/Sm/Md/Lg, inline `icon_start`/`icon_end` + a11y name. Geometry verified vs frozen contract (`benchmark/results/shadcn-native-gallery-slice1/`); icon-bearing buttons lay out icon+gap+label as one centered group (no overlap), labels nowrap, invalid state shows the destructive ring unfocused; raster deltas are documented DirectWrite↔Chromium text-measure noise. Forced `Presented` staging renders the full 6-cell state grid (hover/pressed/focus-visible) without runtime state; focus/invalid rings follow component contour + 3px spread, invalid wins. |
| IconButton | core | complete | yes | verified | - | - | verified | verified | foundation UIA | layout | VERIFIED | `ui.icon_button`: 5 variants x normal/disabled, 4 icon sizes (`size-icon-*`), decorative icon excluded from UIA name. Geometry exact (`dh=0` all sizes). Forced `Presented` staging renders hover/pressed/focus-visible cells. |
| Card / Surface | core | complete | yes | verified | - | - | verified | verified | foundation UIA | layout | VERIFIED | `ui.card`: CardSize Default/Sm (16/12 card-spacing) x header/title/description/content/footer parts; `Card.width(dp)` fixed width. Outer + part geometry exact vs frozen (`card.default` 288×175, `card.default.size-sm` 288×152.25); footer `bg-muted/50` + border-t, `Justify::End` buttons; 1dp spread ring renders. |
| Separator | core | complete | yes | verified | - | - | yes | inert | separator role | yes | VERIFIED | ui.separator; orientation recipe. |
| Badge | core | complete | yes | verified | - | - | yes | inert | label | yes | VERIFIED | ui.badge; 6 variants. |
| Alert | core | complete | yes | verified | - | - | yes | inert | group+parts | yes | VERIFIED | ui.alert; default/destructive, icon/title/description parts. |
| Skeleton | core | complete | yes | verified | - | - | yes | inert | decorative | yes | VERIFIED | ui.skeleton; static only (reduced motion). |
| Kbd | core | complete | yes | verified | - | - | yes | inert | label | yes | VERIFIED | ui.kbd/kbd_group; single + group. |
| Typography (h1-h4, p, lead, small, muted, inline code, code block) | core | complete | yes | verified | - | - | verified | verified | foundation UIA | layout | VERIFIED | `ui.typography` + `ui.code_block`: Geist type scale (h1–h4, paragraph, lead, small, muted, inline-code), Geist font registered via DirectWrite custom collection. |
| Label | core | complete | yes | verified | - | - | yes | inert | label | yes | VERIFIED | ui.label; frozen states + sibling input composition. |
| Field | core | complete | yes | verified | - | - | yes | verified | group+parts | yes | VERIFIED | ui.field; vertical+horizontal, label/control/description/error parts. Labeled-by not wired (no infra). |
| Input | core | complete | yes | verified | - | - | yes | yes | foundation UIA | yes | VERIFIED | ui.input; Shadcn chrome on windowless RichEdit, placeholder/focus/disabled/invalid; staged `Presented` focus-visible + invalid composition. |
| Textarea | core | complete | yes | verified | - | - | yes | yes | foundation UIA | yes | VERIFIED | ui.textarea; multiline chrome + states; staged `Presented` focus-visible + invalid composition, native staged UTF-16 selection, authored `line_height` pitch calibration (frozen 3-line 78dp specimen exact). |
| Native single-line text | core | complete | yes | verified | - | - | yes | yes | foundation UIA | yes | VERIFIED | ui.input/peer; windowless RichEdit, offscreen-blit composition; staged UTF-16 selection renders without runtime focus. |
| Native multiline text | core | complete | yes | verified | - | - | yes | yes | foundation UIA | yes | VERIFIED | ui.textarea/peer; partial-clip band shows true doc rows. |
| Checkbox | core | complete | yes | verified | - | - | yes | yes | UIA CheckBox | yes | VERIFIED | ui.checkbox; app-model state, indicator/icon/label parts, Space/click. No indeterminate. Staged `Presented` focus-visible ring follows control contour. |
| Radio Group | core | complete | yes | verified | - | - | yes | yes | UIA RadioButton | yes | VERIFIED | ui.radio_group/item; app-model single-selection. Staged `Presented` focus-visible ring follows item contour. |
| Switch | core | complete | yes | verified | - | - | yes | yes | UIA CheckBox* | yes | VERIFIED | ui.switch; default+sm, thumb part. *Switch maps to CheckBox — no ToggleState infra. Staged `Presented` focus-visible ring follows control contour. |
| Select | core | complete | yes | verified | - | - | verified | verified | UIA ComboBox + List/ListItem | layout | VERIFIED | ui.select: combobox trigger (value/placeholder, chevron, invalid/disabled/focus-visible) + anchored PopupRole::Listbox popup (side=bottom, offset 4, 208dp lane) with label/item/separator parts. Focus enters the listbox on open (selected option first), arrows/Home/End move via nav_target, Enter/Space picks, Escape/outside-press dismisses; closing restores the trigger. Item-aligned popup mode deferred. |
| Tabs | core | complete | yes | verified | - | - | verified | verified | UIA Tab + TabItem | layout | VERIFIED | ui.tabs: list chrome (muted pill / line lane) + keyed tab triggers; click selects, ArrowLeft/Right move focus AND activate (Radix automatic activation), Home/End jump, disabled skipped. Active chip = bg + shadow-sm; line variant paints the ::after underline marker. |
| Dropdown Menu | core | complete | yes | verified | - | - | verified | verified | UIA Menu/MenuItem | - | VERIFIED | ui.dropdown_menu: MenuButton trigger + anchored PopupRole::Menu popup (side=bottom, align=start, offset 4, 192dp lane) with label/item/separator/shortcut/indicator parts; normal/disabled/destructive/checkbox/radio item kinds, arrows/Home/End via nav_target (disabled skipped), Enter/Space activates, Escape/outside-press dismisses, focus restores to trigger. Checkbox/radio mutate app model via typed DropdownMenuChange. No submenus. |
| SettingsNav (desktop settings sidebar pattern) | core | complete | yes | verified | - | - | verified | verified | UIA List/ListItem | modes | VERIFIED | ui.settings_nav + ui.settings_nav_item — frozen sidebar-13 desktop subset only: wide 256dp + collapsed 48dp icon rail, keyed groups with labels, same automation ids across presentations, active/hover/focus-visible/disabled states. Not the full shadcn Sidebar API; no mobile/drawer/Sheet behavior. |
| Tooltip | core | complete | partial | foundation | - | - | - | - | - | - | FOUNDATION_AVAILABLE | Foundation button `.tooltip(..)`; shadcn tooltip surface/arrow not implemented. |
| Popover | core | complete | partial | foundation | - | - | - | - | - | - | PLANNED | Generic popup layer shipped (`ui.popover`): anchored placement (side/align/offset, flip+clamp), light-dismiss + Escape, modal scrim + hit/focus/scroll gate, focus_on_open/restore, UIA Pane/Menu/List. The shadcn Popover component contract stays PLANNED. |
| Dialog | core | complete | no | - | - | - | - | - | - | - | PLANNED | |
| Gallery shell | core | complete | yes | verified | - | - | - | - | - | - | PARTIAL | Native Gallery acceptance shell shipped — nav rail, Light/Dark/System theme switch, per-page routing, RUI_GALLERY_PAGE boot. Full frozen-catalog page completeness lands with remaining component slices. |

## Later

| Component / Pattern | Tier | Reference | Native API | Windows | macOS | Linux | Visual parity | Interaction parity | Accessibility | Desktop-responsive | Status | Notes |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Slider | later | representative | no | - | - | - | - | - | - | - | DEFERRED | single, range, disabled. |
| Toggle | later | representative | no | - | - | - | - | - | - | - | DEFERRED | |
| Toggle Group | later | representative | no | - | - | - | - | - | - | - | DEFERRED | |
| Button Group | later | representative | no | - | - | - | - | - | - | - | DEFERRED | |
| Breadcrumb | later | representative | no | - | - | - | - | - | - | - | DEFERRED | |
| Pagination | later | representative | no | - | - | - | - | - | - | - | DEFERRED | |
| Alert Dialog | later | representative | no | - | - | - | - | - | - | - | DEFERRED | |
| Typography (large, blockquote, list) | later | representative | no | - | - | - | - | - | - | - | DEFERRED | |
| Full desktop Sidebar framework (header/footer, groups, badges, submenus, nested groups, floating/inset, left/right, keyboard toggle, resizable rail) | later | none | no | - | - | - | - | - | - | modes | DEFERRED | Desktop only; shadcn mobile Sidebar behavior is never in scope. |

## Reference-only / deferred

| Component / Pattern | Tier | Reference | Native API | Windows | macOS | Linux | Visual parity | Interaction parity | Accessibility | Desktop-responsive | Status | Notes |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Typography table | reference-only | representative | no | - | - | - | - | - | - | - | REFERENCE_ONLY | |
| Calendar, Date Picker | reference-only | none | no | - | - | - | - | - | - | - | DEFERRED | Add only with a concrete Mascot/agent use case. |
| Combobox, Command | reference-only | none | no | - | - | - | - | - | - | - | DEFERRED | |
| Sheet, Toast | reference-only | none | no | - | - | - | - | - | - | - | DEFERRED | Sheet is not a mobile-navigation substitute in rust-ui. |
| Other shadcn families (Accordion, Avatar, Carousel, Chart, Data Table, Menubar, Navigation Menu, Context Menu, Input OTP, Progress, Spinner, ...) | reference-only | none | no | - | - | - | - | - | - | - | DEFERRED | See reference README exclusions. |
