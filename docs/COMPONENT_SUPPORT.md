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

| Field | core | complete | yes | verified | - | - | yes | partial | group+parts | yes | VERIFIED | ui.field; vertical+horizontal, label/control/description/error parts. Labeled-by not wired (no infra). |
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
| Button | core | complete | yes | verified | - | - | partial | partial | foundation UIA | layout | VERIFIED | `ui.button`: all 6 variants (primary/secondary/outline/ghost/destructive/link) x normal/disabled/invalid, Xs/Sm/Md/Lg, inline `icon_start`/`icon_end` + a11y name. Geometry verified vs frozen contract (`benchmark/results/shadcn-native-gallery-slice1/`); icon-bearing buttons lay out icon+gap+label as one centered group (no overlap), labels nowrap, invalid state shows the destructive ring unfocused; raster deltas are documented DirectWrite↔Chromium text-measure noise. |
| IconButton | core | complete | yes | verified | - | - | partial | partial | foundation UIA | layout | VERIFIED | `ui.icon_button`: 5 variants x normal/disabled, 4 icon sizes (`size-icon-*`), decorative icon excluded from UIA name. Geometry exact (`dh=0` all sizes). |
| Card / Surface | core | complete | yes | verified | - | - | partial | partial | foundation UIA | layout | VERIFIED | `ui.card`: CardSize Default/Sm (16/12 card-spacing) x header/title/description/content/footer parts; `Card.width(dp)` fixed width. Outer + part geometry exact vs frozen (`card.default` 288×175, `card.default.size-sm` 288×152.25); footer `bg-muted/50` + border-t, `Justify::End` buttons; 1dp spread ring renders. |
| Separator | core | complete | yes | verified | - | - | yes | inert | separator role | yes | VERIFIED | ui.separator; orientation recipe. |
| Badge | core | complete | yes | verified | - | - | yes | inert | label | yes | VERIFIED | ui.badge; 6 variants. |
| Alert | core | complete | yes | verified | - | - | yes | inert | group+parts | yes | VERIFIED | ui.alert; default/destructive, icon/title/description parts. |
| Skeleton | core | complete | yes | verified | - | - | yes | inert | decorative | yes | VERIFIED | ui.skeleton; static only (reduced motion). |
| Kbd | core | complete | yes | verified | - | - | yes | inert | label | yes | VERIFIED | ui.kbd/kbd_group; single + group. |
| Typography (h1-h4, p, lead, small, muted, inline code, code block) | core | complete | yes | verified | - | - | partial | partial | foundation UIA | layout | VERIFIED | `ui.typography` + `ui.code_block`: Geist type scale (h1–h4, paragraph, lead, small, muted, inline-code), Geist font registered via DirectWrite custom collection. |
| Label | core | complete | yes | verified | - | - | yes | inert | label | yes | VERIFIED | ui.label; frozen states + sibling input composition. |
| Field | core | complete | yes | verified | - | - | yes | partial | group+parts | yes | VERIFIED | ui.field; vertical+horizontal, label/control/description/error parts. Labeled-by not wired (no infra). |
| Input | core | complete | yes | verified | - | - | yes | yes | foundation UIA | yes | VERIFIED | ui.input; Shadcn chrome on windowless RichEdit, placeholder/focus/disabled/invalid. |
| Textarea | core | complete | yes | verified | - | - | yes | yes | foundation UIA | yes | VERIFIED | ui.textarea; multiline chrome + states. |
| Native single-line text | core | complete | yes | verified | - | - | yes | yes | foundation UIA | yes | VERIFIED | ui.input/peer; windowless RichEdit, offscreen-blit composition. |
| Native multiline text | core | complete | yes | verified | - | - | yes | yes | foundation UIA | yes | VERIFIED | ui.textarea/peer; partial-clip band shows true doc rows. |
| Checkbox | core | complete | yes | verified | - | - | yes | yes | UIA CheckBox | yes | VERIFIED | ui.checkbox; app-model state, indicator/icon/label parts, Space/click. No indeterminate. |
| Radio Group | core | complete | yes | verified | - | - | yes | yes | UIA RadioButton | yes | VERIFIED | ui.radio_group/item; app-model single-selection. |
| Switch | core | complete | yes | verified | - | - | yes | yes | UIA CheckBox* | yes | VERIFIED | ui.switch; default+sm, thumb part. *Switch maps to CheckBox — no ToggleState infra. |
| Select | core | complete | no | - | - | - | - | - | - | layout | PLANNED | Trigger states + open list (side=bottom). Item-aligned popup mode deferred. |
| Tabs | core | complete | no | - | - | - | - | - | - | layout | PLANNED | default + line, horizontal. |
| Dropdown Menu | core | complete | no | - | - | - | - | - | - | - | PLANNED | Items, disabled, destructive, checkbox/radio items, shortcuts. No submenus. |
| SettingsNav (desktop settings sidebar pattern) | core | complete | no | - | - | - | - | - | - | modes | PLANNED | Wide + collapsed icon rail frozen; same automation ids across presentations. Not a commitment to the full shadcn Sidebar API. |
| Tooltip | core | complete | partial | foundation | - | - | - | - | - | - | FOUNDATION_AVAILABLE | Foundation button `.tooltip(..)`; shadcn tooltip surface/arrow not implemented. |
| Popover | core | complete | no | - | - | - | - | - | - | - | PLANNED | |
| Dialog | core | complete | no | - | - | - | - | - | - | - | PLANNED | |
| Gallery shell | core | complete | no | - | - | - | - | - | - | layout | PLANNED | Page tabs + Light/Dark/System switch for the native Gallery. |

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
