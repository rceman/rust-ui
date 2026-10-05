# Shadcn Gallery Reference v0.1 (candidate revision 2)

A deterministic, isolated HTML/CSS reference of the **current official
shadcn visual language**, frozen so that a later native rust-ui Gallery can
be measured against it. This is a visual reference + machine-readable
contract - **not** a rust-ui component implementation. Everything under
this directory is dev-only tooling (Node/Tailwind/Playwright); there is
zero rust-ui runtime or Cargo dependency.

**Status:** `reference_version: "0.1"`, `candidate_revision: 2` - a
pre-freeze candidate revision; the freeze policy ("any change requires a
reference_version bump") applies only after split review approval.

## Product scope

rust-ui is **desktop-only and fully desktop-responsive** - canonical in
`docs/PRODUCT_SCOPE.md`. Mobile is out of scope; "responsive" never means
mobile here. The `1440x1000` `DPR1` viewport is the deterministic canonical
**capture** viewport - not a fixed-size layout contract; the reference
content is ordinary document flow and adapts to other desktop widths.

Vendored upstream files are **provenance**, not the contract: the rust-ui
contract is a desktop subset/adaptation of them. Upstream mobile code stays
in vendored files untouched but is never transcribed (e.g. Sidebar mobile
`Sheet`/`offcanvas`/`useIsMobile` in `ui/sidebar.tsx`).

## Source-of-truth hierarchy

```
Official shadcn (ui.shadcn.com, upstream repo @ 295a1f1)
        v  vendored verbatim in vendor/shadcn/ (status per file in vendor/shadcn/UPSTREAM.md)
frozen HTML reference (this directory, index.html + src/)
        v  generated (scripts/*.js, deterministic)
tokens.json + coverage.json + token-bindings.json + contract.json
        v  measured
frozen screenshots (screenshots/{light,dark})
        v  compared against by
later native rust-ui implementation (devctl + contract.json)
```

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
- **Fonts: Geist + Geist Mono** (upstream canonical), vendored OFL variable
  woff2 in `vendor/geist/` (`geist@1.7.2`); committed so a fresh clone
  renders identically. Geist has a single `wght` axis (100-900) - no
  `opsz` optical-size axis, and no optical-sizing claim is made.
  Font proof is **exhaustive and fail-closed**: capture queries CDP
  `CSS.getPlatformFontsForNode` for every element with a non-whitespace
  direct text node and every input/textarea value/placeholder; an empty,
  error, or non-Geist (`isCustomFont:false`) result fails the capture.
  Counts and resolved names are recorded in `reference.json`.
- Base-UI transcription: DOM is transcribed from the upstream wrappers in
  `vendor/shadcn/apps/v4/registry/bases/base/ui/*.tsx` (element types,
  `data-slot`, `cn-*` classes, utility classes) plus the attributes Base UI
  1.6.0 renders (`data-checked`, `data-unchecked`, `data-disabled`,
  `data-active`, `data-open`, `data-side`/`data-align`, `aria-checked`,
  `aria-selected`, `aria-pressed`, `aria-invalid`, roles).
  Base-UI behavior is reproduced only to freeze visuals - no Base-UI runtime.
- Recipes are imported in `layer(base)` exactly like upstream
  `apps/v4/app/style-registry.css` - the components' own utility classes
  outrank the `.style-nova .cn-*` recipe hooks, matching installed
  components (transform-style-map inlines recipe classes the same way).
- Native Text specimens add `resize-none` to textareas - the browser UA
  resize grip is a browser artifact, not part of the future native RichEdit
  contract (Forms Textarea stays upstream-faithful).

## Implementation tiers

Every catalog specimen carries `tier` + `family` (propagated to
`contract.json`); the catalog `tier` field is the source of specimen tiers,
while `docs/COMPONENT_SUPPORT.md` remains the durable component support
ledger. Tier definitions live in `reference.json`/`coverage.json`:

- **core** - required for the first native rust-ui Gallery/component
  milestone; complete visually material desktop state/size coverage.
- **later** - planned desktop component, not in the first native milestone;
  representative coverage only.
- **reference-only** - retained visual coverage, no native promise.

`coverage.json` is the bounded upstream-selector-to-specimen table: for
every core family, each Nova recipe / wrapper state selector maps to a
specimen id, an `equivalent_to` id (with reason + measured equivalence), or
`n/a` with reason. `reference:check` fails if a mapped id is absent from the
contract or a core family lacks token bindings.

## Inventory

| Page | Contents |
|---|---|
| `all` | every specimen, grouped in fixed section order |
| `components` | Button (6 variants x normal/hover/pressed/focus-visible/disabled/invalid; sizes xs/sm/lg; inline-start + inline-end icon branches), Icon Button (5 variants x 5 states + sizes icon-xs/icon-sm/icon/icon-lg), Badge (6), Separator (h/v), Card (default/sm), Alert (default/destructive), Skeleton, Kbd (+group) |
| `forms` | Label, Input (placeholder/filled/focus-visible/disabled/invalid/invalid-focus-visible), Textarea (same), Checkbox (unchecked/checked/focus-visible/disabled/disabled-checked/invalid/invalid-checked), Radio Group (selected/unselected/focus-visible/disabled/invalid - both items and indicator keyed), Switch (unchecked/checked/focus-visible/disabled/disabled-checked/invalid + sm), Slider (single/range/disabled - later tier), Select trigger (placeholder/filled/focus-visible/disabled/invalid/size-sm), Field (normal/error/disabled/horizontal) |
| `navigation` | Settings Nav (sidebar-13 desktop subset; capture state `settings-nav-collapsed` = icon rail), Settings Nav Item (5 states), Tabs (default/line x 5 states), Breadcrumb (ellipsis + current), Pagination (prev/links/active/ellipsis/next), Toggle (2 variants x 5 states), Toggle Group (outline), Button Group (toolbar) |
| `typography` | h1-h4, p, lead, small, muted, inline code, code-block (docs `pre`/figure surface, Geist Mono, no highlighting) - core; large, blockquote, list - later; table - reference-only |
| `overlays` | Dialog, Alert Dialog, Popover, Tooltip (arrow), Dropdown Menu, Select open - each statically OPEN in its own stage; trigger/content carry separate ids |
| `native-text` | Input/Textarea recipes as the visual contract for future native RichEdit: single-line {placeholder, filled, focused, selection, disabled, readonly, invalid}; multiline {+ invalid}. `readonly` is a semantic state - visually identical to filled per upstream (equivalence recorded in coverage.json) |

## Exclusions

| Family | Reason |
|---|---|
| Accordion, Collapsible | disclosure mechanics; no distinct v0.1 visual contract needed |
| Aspect Ratio, Scroll Area, Resizable | layout/scroll plumbing, no recipe surface to freeze |
| Attachment, Avatar, Bubble, Message, Message Scroller | chat-domain components; Mascot-side, not gallery core |
| Calendar, Carousel, Chart, Data Table, Date Picker | heavy compound widgets; beyond v0.1 scope |
| Combobox, Command, Input OTP, Menubar, Navigation Menu, Context Menu | overlap/menu variants of the frozen Popover/Dropdown/Select patterns |
| Direction, Native Select | RTL/native-select primitives; not on the Nova visual path measured |
| Drawer, Sheet, Sonner/Toast, Empty, Item, Input Group, Progress, Spinner, Questionnaire, Marker | v0.1 scope cap - one representative per family axis was chosen instead |
| Checkbox indeterminate | no upstream Nova indeterminate visual; Base UI `data-indeterminate` unstyled - deferred |
| Sidebar mobile path (Sheet/offcanvas/useIsMobile) | rust-ui is desktop-only - vendored verbatim, never transcribed |
| Dropdown submenu surfaces | same content recipe; single bounded stage |
| Select `alignItemWithTrigger=true` | frozen in the `false` (side=bottom) mode shown in upstream select-example; item-aligned mode deferred |

## Semantic ID scheme

`<component>.<variant>[.<modifier>]*`, lowercase kebab segments,
dot-separated. States: `hover pressed focus-visible disabled checked
unchecked on off active inactive invalid invalid-focus-visible
disabled-checked invalid-checked placeholder filled focused selection
readonly open current`. Sizes: `size-xs size-sm size-lg size-icon
size-icon-xs size-icon-sm size-icon-lg`. Normal state omits the modifier.
SettingsNav items are addressed by destination, not presentation:
`settings-nav.<page>` (`settings-nav.general` ... `settings-nav.advanced`)
inside container `settings-nav.default`; the same ids are used in the wide
and the collapsed-rail capture states, so responsive presentations never
change automation identity.
Overlays split into `<id>.trigger` and `<id>.content`; content records the
trigger back-reference and upstream anchor (side/align/sideOffset) in
`contract.json`.

**Keyed parts (R03):** every `[data-part]` element carries a semantic key:
the chain of `data-part` ancestors plus optional `data-key` qualifiers -
e.g. `item[option-a].indicator`, `row[0].cell[1]`, `link[components]`,
`item[profile].icon`, `group[workspace]`. Never DOM paths. Generation fails
on duplicate keys inside an element; `reference:check` re-enumerates every
rendered `[data-part]` live and asserts one-to-one equality with the
exported contract parts.

IDs live in the DOM as `data-automation-id`; they are unique per page and
are exactly what a future rust-ui `.automation_id(...)` (UIA
`AutomationId`) will expose.

## Capture contract

- URL: `index.html?page=<id>&theme=light|dark|system&capture=1[&state=<capture-state>]`
- `capture=1` disables animations/transitions, hides the caret
  (`caret-color: transparent` + Playwright `caret: 'hide'`), uses
  `reducedMotion:'reduce'`; no timestamps, no randomness, no network.
- Canonical viewport 1440x1000 CSS px, `deviceScaleFactor 1`, zoom 100%;
  1 CSS px = 1 rust-ui logical `Dp`. Full-page screenshots.
- **Measurement order (R01):** font proof first (it mutates the DOM -
  `data-fontproof` marks + mirror nodes - and CDP-forced pseudo states do not
  survive DOM mutation; all marks are removed before forcing) -> forced
  pseudo states + real focus/selection -> settle (3x `requestAnimationFrame`
  + `document.fonts.ready`) -> measure geometry/text/parts/styles -> evaluate
  token bindings and pressed parity **in the same session/state** ->
  screenshot. The contract records the *painted* post-force state (pressed
  buttons really are 1px lower via `active:translate-y-px`); the recorded
  `bindings_live` / `proofs.pressed_pairs` in `contract.json` are read
  immediately before each PNG - they cannot describe a different state than
  the screenshot.
- **Pixel guard:** for every forced-state specimen (`hover`, `pressed`,
  `focus-visible`, `focused`, `invalid-focus-visible`) that has a same-size
  unforced twin, capture requires the screenshot crops (4px pad, absorbing
  the pressed +1px translate) to differ pixel-for-pixel. This catches the
  class of bug where a DOM mutation after forcing silently drops the forced
  styles: measurements and binding evals would still pass, but the PNG would
  be unstyled - and now fails the guard.
- Forced-state cells carry `data-force-state`; capture applies
  `CSS.forcePseudoState` via CDP so upstream CSS remains authoritative.
- Real focus+selection: `native-text` default state focuses
  `native-text.single-line.selection` (`setSelectionRange(0, 6)`);
  `multiline-selection` focuses `native-text.multiline.selection`
  (`setSelectionRange(22, 33)`). `settings-nav-collapsed` renders the
  sidebar icon rail (`--sidebar-width-icon: 3rem`, same automation ids).
- **Text measurement (R02):** `text_runs` are per meaningful leaf text run -
  one record per text node (rect, per-line rects, `baseline_y`, effective
  font family/size/weight/line-height, color). `input`/`textarea` values and
  placeholders have no DOM text nodes, so they are measured via a mirror
  element with identical computed box/font styles (browser editing behavior
  is not authority). Baseline definition:
  `baseline_y = line_top + (line_height - (fontBoundingBoxAscent +
  fontBoundingBoxDescent)) / 2 + fontBoundingBoxAscent`, with ascent/descent
  from canvas `measureText` on the run's effective Geist font. Every run is
  validated against the rendered pixels: ink inside the run rect (±1px) and
  the baseline inside the line's ink band. Limitations: ink validation is
  luminance-based (no OCR); ligatures/measurement of sub-1px ink may differ
  ±1px; Chromium font rasterization is the reference renderer, not the
  future native rasterizer.
- Screenshots: `screenshots/{light,dark}/<page>[--<state>].png`.
- **Theme modes (R07):** a single `matchMedia` listener bound once consults
  the current mode - System follows OS; Light/Dark pin regardless of later
  OS changes; System re-selects following. `reference:check` exercises the
  full sequence in one live document (emulated OS flips, clicks, navigation)
  and asserts the listener count stays exactly 1.
- Browser: bundled Chromium via `channel:'chromium'` (new headless mode).
  Deterministic rasterization flags: `--disable-gpu --force-color-profile=srgb
  --disable-lcd-text`.
- Layout: matrix sections render variants x forced states as framed grids;
  other sections use equal-width grid cells. `reference:check` enforces the
  class guard (every DOM class token must have a rule in
  `static/gallery.css`; exclusions: `cn-*` hooks, `group/*`/`peer/*`
  markers, `sr-only`, `dark`).
- **Flow margins (gallery-layout adaptation):** upstream applies prose-rhythm
  block margins in `typography-demo` (`mt-10`/`mt-8` on h2/h3) and on the
  per-variant specimens themselves (`mt-6`, `my-6`,
  `[&:not(:first-child)]:mt-6`, the code-block figure's
  `margin-top: calc(--spacing*6)`). Inside an isolated `specimen-frame` the
  outer block margin of the specimen root is zeroed
  (`src/gallery.css` `.specimen-frame > :first-child`/`> :last-child`) so the
  frame padding is balanced; every inner upstream rule is untouched. The
  upstream flow-margin contract is recorded as data in
  `tokens.json` `typography.flow_margins_px` for native flowing text.
- Sections are ordered core-tier first on every page; non-core sections show
  a `later`/`reference-only` outline badge next to the section title (the
  typography page mixes tiers, so its badge is per specimen caption).
- Install-time transforms emulated for our config (menuColor=default):
  `cn-menu-target`/`cn-menu-translucent` placeholders are REMOVED from menu
  and select content (transform-menu.ts); rtl=false -> no direction
  transform; `cn-font-heading` retained, resolving to `--font-sans` (Geist).
- Select open is frozen in the `alignItemWithTrigger=false` (side=bottom)
  mode shown in upstream select-example; the default item-aligned mode is
  deferred to a later reference version.
- Highlighted menu/select items are modelled with `data-highlighted` +
  forced `:focus` - Base UI moves real focus to the highlighted item, so
  the recipe's `focus:bg-accent focus:text-accent-foreground` applies.
- **Theme provenance (R08):** `vendor/shadcn/theme-neutral.css` is generated
  verbatim from vendored `apps/v4/registry/themes.ts` by `scripts/theme.js`
  (`reference:check` re-derives and compares byte-exact). Vars NOT in
  themes.ts (`--destructive-foreground`, `--surface*`, `--code*`,
  `--selection*`) live in `src/theme-local.css` with per-var upstream
  source (`apps/v4/app/globals.css`).

## Artifacts

- `static/gallery.css` - committed compiled Tailwind output.
- `contract.json` - `rust-ui.shadcn-reference.contract/0.1`: coordinate
  space, id vocabulary, pages, and per-element placements/box/text/
  `text_runs`/keyed `parts`. Light and dark geometry asserted identical.
- `tokens.json` - `rust-ui.shadcn-reference.tokens/0.1`: **frozen visual
  reference** - NOT the rust-ui runtime style authority (rust-ui's typed
  theme API stays canonical). Semantic colors as upstream oklch + derived
  sRGB+alpha (formula-only tokens resolved to concrete values), typography,
  spacing, radii, borders, shadows, control heights, focus ring, disabled.
- `coverage.json` - core-family upstream-selector coverage map (see
  Implementation tiers).
- `token-bindings.json` - `{automation_id, part?, capture_state?, theme,
  css_property, token_path}` rows binding core specimens' live computed
  values to `tokens.json` paths; `reference:check` compares them post-force
  (colors to sRGB+alpha at <=1/255 per channel and alpha <=0.005; lengths
  exact to 0.01px). Every core family has bindings.
- `reference.json` - freeze metadata: candidate revision, authority, style,
  fonts (+proof counts), tiers, viewport/platform, capture states, git bases.
- `screenshots/SHA256.json` - hashes for the determinism check.
- `vendor/geist/{LICENSE,UPSTREAM.md}` - Geist OFL + package provenance;
  `vendor/lucide/{LICENSE,UPSTREAM.md}` - lucide-static ISC + retained
  Feather MIT notices; `vendor/shadcn/UPSTREAM.md` - per-file
  verbatim/derived/local status.

## Reproduction

```bash
npm ci                     # dev tooling only (node_modules is gitignored)
npx playwright install chromium  # once per machine
npm run reference:capture  # regen theme/icons/tokens/coverage/bindings + css, capture
npm run reference:check    # validate everything below; invokes selftest
npm run reference:selftest # fault-injection: proves validators fail on wrong values
```

`reference:check` runs: version/schema validation (reference_version +
candidate_revision consistent across all committed JSON), theme derivation
byte-exactness, CSS rebuild identity, full re-capture byte-identity
(screenshots + contract), light/dark geometry parity, the R07 live
theme-listener sequence, exhaustive Geist font re-proof, R05 inline-start
icon branch proof, coverage resolution, token-binding live comparisons,
R01 pressed parity, R03 live part-key enumeration, attribution/vendor hash
checks, hygiene scans (no developer absolute paths, `.tmp-*`/debug files,
or http(s) URLs in render-time assets - SVG `xmlns` excepted), the legacy
token-closure map, and `reference:selftest` fault injection (backdrop alpha,
control height, padding, line-height, version, duplicate part key, missing
core state mapping). Non-zero exit on any failure.

## Future native comparison path

The native Gallery sets the same `data-automation-id` semantics through
rust-ui `.automation_id(...)`; `rust-ui-devctl` (`rect` / `hover` / `click` /
`focus` / `type` / `snapshot-layout` / `compare`) consumes `contract.json`
to compare native geometry/text/parts against these frozen placements.
