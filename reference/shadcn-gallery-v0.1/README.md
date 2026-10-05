# Shadcn Gallery Reference v0.1

A deterministic, isolated HTML/CSS reference of the **current official
shadcn visual language**, frozen so that a later native rust-ui Gallery can
be measured against it. This is a visual reference + machine-readable
contract - **not** a rust-ui component implementation. Everything under
this directory is dev-only tooling (Node/Tailwind/Playwright); there is
zero rust-ui runtime or Cargo dependency.

## Source-of-truth hierarchy

```
Official shadcn (ui.shadcn.com, upstream repo @ 295a1f1)
        v  vendored verbatim in vendor/shadcn/
frozen HTML reference (this directory, index.html + src/)
        v  generated
tokens.json + contract.json
        v  measured
frozen screenshots (screenshots/{light,dark})
        v  compared against by
later native rust-ui implementation (devctl + contract.json)
```

Any change to the frozen artifacts requires a `reference_version` bump.

## Authority

- Site: https://ui.shadcn.com/ - reviewed 2026-10-05
- Upstream: `github.com/shadcn-ui/ui` @ `295a1f114a138f23b5dfee0e0c6812394dfeb90c`
- Style: **base-nova** - Base UI library, Nova style, `neutral` base color,
  Lucide icons, radius `default` (`--radius: 0.625rem`), menuAccent `subtle`,
  menuColor `default`.
- Why Nova and not Vega: `component-preview` renders `base-nova` by default,
  `packages/shadcn/src/preset/defaults.ts` lists `nova` first, and
  /docs/theming documents `"style": "base-nova"`. The components.json docs
  still say `new-york` - stale; Vega is the legacy New York look.
- Deviation: Nova's canonical font is Geist; rust-ui deliberately renders
  this reference in **Segoe UI Variable** (system-installed), mono in
  **Cascadia Mono**. Resolved platform families are asserted at capture
  (CDP `CSS.getPlatformFontsForNode`) and recorded in `reference.json`.
- Base-UI transcription: DOM is transcribed from the upstream wrappers in
  `vendor/shadcn/apps/v4/registry/bases/base/ui/*.tsx` (element types,
  `data-slot`, `cn-*` classes, utility classes) plus the attributes Base UI
  1.6.0 renders (`data-checked`, `data-unchecked`, `data-indeterminate`,
  `data-disabled`, `data-active`, `data-open`, `data-side`/`data-align`,
  `aria-checked`, `aria-selected`, `aria-pressed`, `aria-invalid`, roles).
  Base-UI behavior is reproduced only to freeze visuals - no Base-UI runtime.
- Recipes are imported in `layer(base)` exactly like upstream
  `apps/v4/app/style-registry.css` - the components' own utility classes
  outrank the `.style-nova .cn-*` recipe hooks, matching installed
  components (transform-style-map inlines recipe classes the same way).
- Native Text specimens add `resize-none` to textareas - the browser UA
  resize grip is a browser artifact, not part of the future native RichEdit
  contract (Forms Textarea stays upstream-faithful).

## Inventory

| Page | Contents |
|---|---|
| `all` | every specimen, grouped in fixed section order |
| `components` | Button (6 variants x normal/hover/pressed/focus-visible/disabled; sizes xs/sm/lg/icon-xs/icon-sm/icon/icon-lg; inline-start icon), Badge (6), Separator (h/v), Card (default/sm), Alert (default/destructive), Skeleton, Kbd (+group) |
| `forms` | Label, Input (5 states), Textarea (5), Checkbox (unchecked/checked/disabled/invalid), Radio Group (4), Switch (4 + sm), Slider (single/range/disabled), Select trigger (6), Field (normal/error/disabled) |
| `navigation` | Tabs (default/line x 5 states), Breadcrumb (ellipsis + current), Pagination (prev/links/active/ellipsis/next), Toggle (2 variants x 5 states), Toggle Group (outline), Button Group (toolbar) |
| `typography` | h1-h4, p, blockquote, list, inline code, lead, large, small, muted, table |
| `overlays` | Dialog, Alert Dialog, Popover, Tooltip (arrow), Dropdown Menu, Select open - each statically OPEN in its own stage; trigger/content carry separate ids |
| `native-text` | Input/Textarea recipes as the visual contract for future native RichEdit: single-line {placeholder, filled, focused, selection, disabled, readonly, invalid}; multiline {placeholder, filled, focused, selection, disabled, readonly}. `readonly` is a semantic state - visually identical to filled per upstream (no distinct recipe) |

## Exclusions

| Family | Reason |
|---|---|
| Accordion, Collapsible | disclosure mechanics; no distinct v0.1 visual contract needed |
| Aspect Ratio, Scroll Area, Resizable | layout/scroll plumbing, no recipe surface to freeze |
| Attachment, Avatar, Bubble, Message, Message Scroller | chat-domain components; Mascot-side, not gallery core |
| Calendar, Carousel, Chart, Data Table, Date Picker | heavy compound widgets; beyond v0.1 scope |
| Combobox, Command, Input OTP, Menubar, Navigation Menu, Context Menu | overlap/menu variants of the frozen Popover/Dropdown/Select patterns |
| Direction, Native Select | RTL/native-select primitives; not on the Nova visual path measured |
| Drawer, Sheet, Sidebar, Sonner/Toast, Empty, Item, Input Group, Progress, Spinner, Questionnaire, Marker | v0.1 scope cap - one representative per family axis was chosen instead |
| Checkbox indeterminate | no upstream Nova indeterminate visual; Base UI `data-indeterminate` unstyled - deferred |

## Semantic ID scheme

`<component>.<variant>[.<modifier>]*`, lowercase kebab segments,
dot-separated. States: `hover pressed focus-visible disabled checked
unchecked indeterminate on off active inactive invalid placeholder filled
focused selection readonly open current`. Sizes: `size-xs size-sm size-lg
size-icon size-icon-xs size-icon-sm size-icon-lg`. Normal state omits the
modifier (`button.default`, `button.outline.hover`, `checkbox.default.checked`).
Overlays split into `<id>.trigger` and `<id>.content`; content records the
trigger back-reference and upstream anchor (side/align/sideOffset) in
`contract.json`. IDs live in the DOM as `data-automation-id`; they are never
DOM paths and are unique per page. These ids are exactly what a future
rust-ui `.automation_id(...)` (UIA `AutomationId`) will expose.

## Capture contract

- URL: `index.html?page=<id>&theme=light|dark|system&capture=1[&state=<capture-state>]`
- `capture=1` disables animations/transitions, hides the caret
  (`caret-color: transparent` + Playwright `caret: 'hide'`), uses
  `reducedMotion:'reduce'`; no timestamps, no randomness, no network.
- Canonical viewport 1440x1000 CSS px, `deviceScaleFactor 1`, zoom 100%;
  1 CSS px = 1 rust-ui logical `Dp`. Full-page screenshots.
- Forced-state cells carry `data-force-state`; capture applies
  `CSS.forcePseudoState` via CDP so upstream CSS remains authoritative
  (no duplicated hover styling). Tailwind's `@media (hover:hover)` hover
  variants verified to apply under forced hover.
- Real focus+selection: one element holds focus. `native-text` default
  capture state focuses `native-text.single-line.selection` with
  `setSelectionRange(0, 6)`; capture state `multiline-selection` focuses
  `native-text.multiline.selection` with `setSelectionRange(22, 33)`.
- Screenshots: `screenshots/{light,dark}/<page>[--<state>].png` for all 7
  pages + the multiline-selection state. System mode follows
  `prefers-color-scheme` live; `reference:check` proves System -> dark/light
  under emulated schemes.
- Browser: bundled Chromium via `channel:'chromium'` (new headless mode -
  `chromium_headless_shell` does not paint form-control text selection).
  Deterministic rasterization flags: `--disable-gpu --force-color-profile=srgb
  --disable-lcd-text` (without them, subpixel-AA/GPU raster wobble produces
  stray pixel diffs on tall pages).
- Layout: matrix sections (Button, Toggle) render variants x forced states as
  a framed grid with muted row/column headers; other sections use equal-width
  grid cells (inline `grid-template-columns` - Tailwind never sees
  template-built class names). `reference:check` enforces this: every class
  token on rendered DOM must have a rule in `static/gallery.css` (exclusions:
  `cn-*` upstream hooks, `group/*`/`peer/*` markers, `sr-only`, `dark`).
- Install-time transforms emulated for our config (menuColor=default):
  `cn-menu-target`/`cn-menu-translucent` placeholders are REMOVED from menu
  and select content (transform-menu.ts) - menus render opaque `bg-popover`
  and destructive items render `text-destructive`. (The translucent
  menuColor variant would instead neutralize them to accent-foreground.)
  rtl=false -> no direction transform; iconLibrary=lucide -> no-op;
  `cn-font-heading` retained, resolving to font-sans.
- Select open is frozen in the `alignItemWithTrigger=false` (side=bottom)
  mode shown in upstream `select-example`; the default item-aligned mode is
  deferred to a later reference version.
- Highlighted menu/select items are modelled with `data-highlighted` +
  forced `:focus` - Base UI moves real focus to the highlighted item, so
  the recipe's `focus:bg-accent focus:text-accent-foreground` applies.

## Artifacts

- `static/gallery.css` - committed compiled Tailwind output (reference
  renders without rebuilding).
- `contract.json` - `rust-ui.shadcn-reference.contract/0.1`: coordinate
  space, id vocabulary, pages, and per-element placements/box/text/parts.
  Light and dark geometry are asserted identical; one placement set stored.
  No colors (colors live in `tokens.json`).
- `tokens.json` - `rust-ui.shadcn-reference.tokens/0.1`: frozen visual
  tokens (semantic colors light+dark as upstream oklch + derived sRGB,
  translucent usages, typography, spacing, radii, borders, shadows, control
  heights, focus ring, disabled). Explicitly NOT the rust-ui runtime style
  authority - the rust-ui typed theme API stays canonical.
- `reference.json` - freeze metadata: authority, review date, style,
  fonts, viewport/platform, theme modes, capture states, git bases,
  devctl bridge note.
- `screenshots/SHA256.json` - hashes for the determinism check.

## Reproduction

```bash
npm ci                     # dev tooling only (node_modules is gitignored)
npm run reference:capture  # build CSS + icons, capture screenshots, emit contract.json/reference.json
npm run reference:check    # validate: rebuild identical, re-capture byte-identical,
                           # light/dark geometry identity, system-mode, fonts, token closure
npm run reference:tokens   # regenerate tokens.json
```

`reference:check` rebuilds the CSS to a temp dir and asserts it is
byte-identical to the committed file, re-captures and asserts screenshots
and `contract.json` are byte-identical, re-proves font resolution and
system-theme behavior, and checks token closure (every measured radius/
border-width/font-size/font-weight/control height maps to a tokens.json
value). Non-zero exit on any failure.

## Future native comparison path

The native Gallery sets the same `data-automation-id` semantics through
rust-ui `.automation_id(...)`; `rust-ui-devctl` (`rect` / `hover` / `click` /
`focus` / `type` / `snapshot-layout` / `compare`) consumes `contract.json`
to compare native geometry/text/parts against these frozen placements.
