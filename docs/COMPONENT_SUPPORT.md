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
| Button | core | complete | partial | foundation | - | - | - | - | foundation UIA | layout | FOUNDATION_AVAILABLE | Foundation `ui.button`: Primary/Secondary/Ghost/Destructive, Sm/Md/Lg, disabled. Missing vs reference: outline, link, xs, invalid, inline icons. |
| IconButton | core | complete | no | - | - | - | - | - | - | layout | PLANNED | Reference: 5 variants x 5 states + 4 icon sizes. |
| Card / Surface | core | complete | partial | foundation | - | - | - | - | - | layout | FOUNDATION_AVAILABLE | Foundation `ui.surface`; Card header/content/footer composition not implemented. |
| Separator | core | complete | no | - | - | - | - | - | - | layout | PLANNED | |
| Badge | core | complete | no | - | - | - | - | - | - | layout | PLANNED | 6 variants. |
| Alert | core | complete | no | - | - | - | - | - | - | layout | PLANNED | default, destructive. |
| Skeleton | core | complete | no | - | - | - | - | - | - | layout | PLANNED | Static only (reduced motion). |
| Kbd | core | complete | no | - | - | - | - | - | - | - | PLANNED | single, group. |
| Typography (h1-h4, p, lead, small, muted, inline code, code block) | core | complete | partial | foundation | - | - | - | - | foundation UIA | layout | FOUNDATION_AVAILABLE | Foundation text/label nodes; Geist type scale not implemented. |
| Label | core | complete | partial | foundation | - | - | - | - | foundation UIA | layout | FOUNDATION_AVAILABLE | Foundation label text; field association not implemented. |
| Field | core | complete | no | - | - | - | - | - | - | layout | PLANNED | vertical (default/invalid/disabled) + horizontal settings row. |
| Input | core | complete | partial | foundation | - | - | - | - | foundation UIA | layout | FOUNDATION_AVAILABLE | Built on native single-line text; shadcn chrome/states not implemented. |
| Textarea | core | complete | partial | foundation | - | - | - | - | foundation UIA | layout | FOUNDATION_AVAILABLE | Built on native multiline text. |
| Native single-line text | core | complete | partial | foundation | - | - | - | - | foundation UIA | layout | FOUNDATION_AVAILABLE | Windowless RichEdit `ui.text_input` (placeholder, read-only, disabled, IME). Reference defines chrome/metrics only; editing semantics are platform authority. |
| Native multiline text | core | complete | partial | foundation | - | - | - | - | foundation UIA | layout | FOUNDATION_AVAILABLE | Windowless RichEdit `ui.text_area`. |
| Checkbox | core | complete | no | - | - | - | - | - | - | - | PLANNED | No indeterminate (no Nova visual). |
| Radio Group | core | complete | no | - | - | - | - | - | - | layout | PLANNED | |
| Switch | core | complete | no | - | - | - | - | - | - | - | PLANNED | default + sm. |
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
