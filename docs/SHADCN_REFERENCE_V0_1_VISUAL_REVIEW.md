# Shadcn Reference v0.1 — Visual / UI Freeze Review

| | |
|---|---|
| Run key | `RUI-O-261005-1604` |
| Role | Opus 5.5 Reviewer (UI design / visual authority) |
| Candidate | `2fe97eaf5438ea928a85224956248f859d6ac464` (`agent/shadcn-gallery-reference-v0.1-opus55`) |
| Review branch | `review/shadcn-reference-v0.1-visual-opus55` (from exact candidate HEAD) |
| Rust production | `00dc29acfeb686d6a190d91624de7ab9a48e1e92` (not touched) |
| Authority | shadcn Base UI · Nova · neutral · Lucide · Geist Sans / Geist Mono · `shadcn-ui/ui@295a1f1` |
| Scope | Visual / UI only. Contract integrity, determinism, provenance, licences and ledger truth are Astra's review. |

**Result: `VISUAL_REFERENCE_REWORK_REQUIRED`**. Five bounded findings (V01–V05) remain. Everything else reviewed is visually ready.

Independence note: the same model session that authored the candidate wrote this review, in the reviewer role. To offset that, every finding below rests on rendered pixels, live computed styles, or the pinned upstream source, not on memory of the authoring intent.

## 1. Method

- **Inventory confirmed: 18 screenshots.** Light and Dark × {all, components, forms, navigation, navigation--settings-nav-collapsed, typography, overlays, native-text, native-text--multiline-selection}.
- **Every page inspected full height in both themes.** Single pages were tiled at 1:1. The 14,563 px All page was checked at 1:3 side by side, with 1:1 crops wherever something looked off.
- **Zoomed crops (×2–×4) for fine detail:**
  - checkbox mark and radio dot centring;
  - switch thumbs;
  - inline-start/end button icons;
  - Dialog footer edge;
  - Native Text selection;
  - SettingsNav rows.
- **Pixel comparisons:**
  - Input/Textarea invalid vs invalid + focus-visible;
  - overlay surface vs stage vs page luminance.
- **Live, non-forced interaction (Playwright, same capture flags):**
  - real hover on a SettingsNav item, and keyboard Tab focus;
  - shell theme buttons: Dark (holds under OS light) → System (follows OS light/dark), with one listener;
  - Field label computed styles in both themes;
  - SettingsNav and gallery at desktop widths 1280 / 1100 / 960 / 820.
- **Upstream comparison:** the vendored Nova recipes (`style-nova.css`), plus the Base UI component and example sources for Button, Field, Dropdown Menu, Switch and Sidebar.

## 2. Findings requiring rework

### V01 — SettingsNav: the first row collides with the header separator

- **VISIBLE DEFECT:** In the content pane, the "Theme" row sits directly on the header separator: the row's top edge is 0 px below it. Every other separator-to-row gap is 12 px. The "Theme" label touches the rule, and the "System" select trigger starts about 3 px under it. The rows read as top-heavy, and the header block looks glued to the first row.
  - Contract parts: `content.separator[header]` y = 80, height 1; `content.field[theme]` y = 81.
  - For comparison: `separator[row-1]` y = 130 → `field[animations]` y = 143.
- **WHY IT MATTERS:** SettingsNav is CORE, and this content pane is the canonical settings-page composition a native implementation will copy. A 0 px vs 12 px rhythm forces the implementer to guess which value is intended.
- **WHERE:** `navigation` and `navigation--settings-nav-collapsed`, Light and Dark, `settings-nav.default` → `content.separator[header]` / `content.field[theme]`. Reproduces live at every desktop width checked.
- **EXPECTED VISUAL DIRECTION:** Use one vertical rhythm for the header separator and the row separators: the same gap above and below each rule, so the first row sits exactly like rows 2 and 3.

### V02 — Field: the invalid and disabled label treatments do not follow Nova

- **VISIBLE DEFECT:** In both themes the "Email" label renders in plain foreground at full opacity in all three specimens: `field.default`, `field.default.invalid` and `field.default.disabled`.
- **What upstream does:** Nova ties both treatments to the Field container. React serialises `<Field data-invalid>` / `<Field data-disabled>` (used in `field-example.tsx` and `checkbox-example.tsx`) as `="true"`.
  - Invalid: `.cn-field { data-[invalid=true]:text-destructive }` turns the label destructive.
  - Disabled: `.cn-field-label { group-data-[disabled=true]/field:opacity-50 }` dims the label to 50%.
- **Live probe:**
  - The invalid specimen carries no `data-invalid` at all.
  - The disabled specimen carries a bare `data-disabled=""`, which does not match `data-[disabled=true]`.
  - Setting `="true"` gives a label colour of `oklch(0.577 0.245 27.325)` (destructive) for invalid, and a label opacity of 0.5 for disabled.
- **WHY IT MATTERS:** Field is CORE. As captured, a native implementation would leave invalid and disabled labels unstyled, which is a direct Nova mismatch in two material states.
- **WHERE:** `forms` and `all`, Light and Dark, `field.default.invalid` and `field.default.disabled` (label part).
- **EXPECTED VISUAL DIRECTION:**
  - Invalid field: label in destructive colour. The description stays muted and the error stays destructive, as upstream.
  - Disabled field: label at 50% opacity, alongside the already-correct disabled input.

### V03 — Dropdown Menu: the checkbox and radio groups are run together without upstream grouping

- **VISIBLE DEFECT:** The open menu lists, in one undivided run:
  - Profile, Settings, Team members, Billing (disabled);
  - Show toolbar ✓, Word wrap;
  - Panel left ✓, Panel right.

  There is no separator or group label between these three blocks. Nova draws checkbox items and radio items with the same right-aligned `CheckIcon` indicator (`cn-dropdown-menu-item-indicator`, `right-2`). The screenshot therefore gives no visual way to tell the toggle group from the single-choice group, or either from the action items above them.
- **What upstream does:** Its examples always wrap these in labelled groups divided by separators: `DropdownMenuWithCheckboxes` uses "Appearance", `DropdownMenuWithRadio` uses "Panel Position".
- **WHY IT MATTERS:** Dropdown Menu is CORE. The composite specimen is the single frozen menu target, and as composed it reads as one flat list of unrelated rows. This is the kind of state appearance that forces a native implementation to guess.
- **WHERE:** `overlays` and `all`, Light and Dark, `dropdown-menu.default.content`, between `item[billing]` → `item[toolbar]` and `item[word-wrap]` → `item[panel-left]`.
- **EXPECTED VISUAL DIRECTION:** Follow the upstream grouping: separator + label ("Appearance"-style) before the checkbox items, and separator + label ("Panel Position"-style) before the radio items. Keep the existing items and states.

### V04 — All page: the selection specimens render without their frozen state

- **VISIBLE DEFECT:** On `all.png` (both themes), `native-text.single-line.selection` shows plain "Select this phrase" with no focus ring and no selection highlight. On `native-text.png` the same ID shows the focus ring plus the "Select" highlight. `native-text.multiline.selection` likewise shows no selection on All. One semantic ID shows two different visual states across canonical screenshots.
- **WHY IT MATTERS:** Native Text selection appearance is one of the few things this reference owns for the future RichEdit controls. A canonical screenshot that shows the selection specimen unselected is a wrong visual target for anyone reading the All page.
- **WHERE:** `all`, Light and Dark: `native-text.single-line.selection` (around y = 13,933) and `native-text.multiline.selection` (around y = 14,228).
- **EXPECTED VISUAL DIRECTION:** Each specimen should look the same wherever it appears. Either render the frozen selection state on the All page too, or mark these two specimens there clearly as "state shown on Native Text" instead of silently rendering them unselected.

### V05 — Dark overlay stages erase popover, menu and select elevation

- **VISIBLE DEFECT:** The overlay stage is gallery chrome (`.overlay-stage`, `bg-muted/40`). In Dark it resolves to RGB 21, against popover-surface RGB 23, so Popover, Dropdown and Select-open content barely lift off their stage: only the 10% foreground ring separates them.
- **How this differs from real use:** the page background is RGB 10. On it, the same surfaces are clearly raised; that is how the Nova dark surface hierarchy is meant to read.
- **Light is fine:** stage 251 vs surface 255, plus a visible `shadow-md`.
- **WHY IT MATTERS:** Overlay separation and dark surface hierarchy are explicit review criteria. As captured, the Dark overlays understate the surface step a native implementation must reproduce, and Light and Dark convey different elevation intent.
- **WHERE:** `overlays` and `all`, Dark: `popover.default`, `dropdown-menu.default`, `select.default.open` (the Tooltip and Dialog stages are unaffected).
- **EXPECTED VISUAL DIRECTION:** In Dark, the stage behind anchored overlays should not compete with the overlay surface. It should read like the app background, so the popover/menu surface step is as visible as it is in real use. The change is limited to gallery chrome; the component recipes stay unchanged.

## 3. Reviewed and visually ready

| Area | Verdict |
|---|---|
| **Geist typography** | Correct families everywhere; mono only on code, Kbd, captions and shortcuts. h1 48 px extrabold, h2 30 px with rule, h3 24, h4 20 semibold; body 16 at `leading-7`; lead 20 muted; small 14 medium; muted 14; inline code semibold mono chip. No clipping, bad wrapping or baseline drift seen. Inside each typography frame the top and bottom spacing is now even (24 px). |
| **Code block** | Docs surface, radius, 16/14 padding, no clipping at either edge, ligatures off. Good in both themes. |
| **Button** | Six variants × normal / hover / pressed / focus-visible / disabled / invalid all visibly distinct where upstream differs. Labels centred. Pressed sits 1 px lower. 3 px focus ring. Disabled at 50%. Size row xs / sm / default / lg is proportionate. |
| **R05 inline-start** | The `arrow-left-circle` icon (the upstream example's icon) sits 9 px from the outer edge (1 px border + 8 px `pl-2`), mirrored by inline-end. Vertically centred, Light and Dark. Visually closed. |
| **IconButton** | Arrow centred in all four sizes and all states; disabled and destructive treatments read correctly. |
| **Badge, Separator, Card, Alert, Skeleton, Kbd** | Faithful to Nova. Card footer band, alert icon alignment and the destructive alert colour are correct. Ghost and link badges render identically, which is faithful to upstream. |
| **Input, Textarea** | Placeholder, filled, focus (ring border + 3 px ring/50), disabled (input/50 fill) and invalid are all distinct. Invalid vs invalid + focus-visible match pixel for pixel (≤ 2 levels at one subpixel, light and dark); this is correct, because the `aria-invalid` ring and border override the focus colours in the Nova recipe. No redesign is warranted. |
| **Checkbox** | Check mark centred in the 16 px box. Checked, disabled, disabled-checked, invalid (destructive border), invalid-checked (primary fill + destructive ring) and focus-visible are all correct. |
| **Radio Group** | Dot centred. Selected, unselected, focus (on option B), disabled; invalid keeps primary on the checked item and destructive on the unchecked one, as upstream. |
| **Switch** | The earlier defect is gone. The thumb travels 14 px (default) and 10 px (sm). The disabled specimen is unchecked and the disabled-checked specimen is checked. Dark thumbs are foreground (unchecked) and primary-foreground (checked). The light unchecked track is pale, as in upstream Nova. |
| **Select** | Trigger placeholder, filled, focus, disabled, invalid and sm are correct; the chevron is aligned. The open list shows label, highlighted, selected ✓, disabled, separator and a second group, anchored 4 px below its trigger. |
| **Tabs** | Default and line variants in active, inactive, hover, focus-visible and disabled; the line indicator is placed correctly. |
| **Tooltip, Popover, Dialog** | Tooltip is inverted with its arrow centred. Popover padding, radius and ring are correct. Dialog surface, close button, `bg-muted/50` footer with its top border, blurred backdrop: Light good, and Dark good apart from V05, which does not affect the Dialog stage. |
| **Native Text** | Chrome, padding, placeholder, readonly (= filled, as documented), disabled and invalid are correct. Selection is a solid highlight hugging the line box (black in Light, `0.922` in Dark). Judged as chrome only; editing behaviour remains platform authority. |
| **SettingsNav (apart from V01)** | Clear group labels, Lucide icons, readable active page (accent fill + medium weight). Real hover and 2 px keyboard focus ring work live and match the forced specimens. Disabled is dimmed. The collapsed icon rail keeps the active accent, centres the icons and keeps the group gaps as upstream does. |
| **Light forced states** | The hover, pressed and focus-visible states that were missing from earlier Light captures are present on every relevant specimen. |
| **Light/Dark parity** | Both themes are equally finished. Destructive and invalid treatments, muted text, borders and focus rings stay visible in Dark. The only parity gap is V05. |
| **Desktop responsiveness** | Content uses ordinary flow; nothing is fixed to 1440. SettingsNav stays coherent at 1280, 1100, 960 and 820: nav fixed at 16 rem, content column shrinking without overlap, and the collapsed rail available as the compact presentation. No mobile behaviour is required or present. |

## 4. Non-blocking observations (no rework required)

- **N1 — Dark text looks heavier.** Captures use grayscale antialiasing (`--disable-lcd-text`), which makes light-on-dark Geist look about a weight heavier than the same weight in Light. Geometry is unaffected. Native comparison should judge weight from the typography tokens, not from ink density in the Dark PNGs.
- **N2 — The gallery itself overflows below about 1000 px.** The shell header and the six-column Button/Toggle matrices produce horizontal scroll (`scrollWidth` 996 at 820 / 900 px viewports). The canonical freeze is a single 1440 viewport and these are gallery layouts, not component contracts. The future native Gallery shell should define its own compact header and matrix wrapping as a desktop-responsive presentation.
- **N3 — Inconsistent capitalisation.** Checkbox specimen labels mix "Focus visible" with lowercase "unchecked / checked / disabled / invalid". Cosmetic copy only.
- **N4 — Icon/label mismatch.** The inline-start specimen pairs the upstream back-arrow icon with the label "Add item". The geometry is right; a label such as "Back" would read better.
- **N5 — Hover and active share a fill.** In Light, SettingsNav hover and active use the same very subtle `sidebar-accent` fill (upstream Nova), so the active page is mainly marked by `font-medium`. A native implementation must keep that weight change.

## 5. Result

```text
VISUAL_REFERENCE_REWORK_REQUIRED
```

Bounded rework: V01 (SettingsNav row rhythm), V02 (Field invalid/disabled labels), V03 (Dropdown grouping), V04 (All-page selection specimens), V05 (Dark overlay stage). After a corrected candidate, only these items and their Light/Dark screenshots need visual re-review. Nothing in this review asks for native implementation or a change to the Rust foundation.
