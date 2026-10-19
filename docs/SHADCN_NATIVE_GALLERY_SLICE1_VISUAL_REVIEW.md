# Independent Visual / UX Closure Review — Native Shadcn Gallery Slice 1

**Verdict:** `SHADCN_NATIVE_GALLERY_SLICE1_VISUAL_REWORK_REQUIRED`

- **Run Key:** `RUI-O-261019-1335`
- **Reviewer:** Opus 5.5 Reviewer (Visual / UX / Component Fidelity Authority)
- **Repository:** `rceman/rust-ui`
- **Implementation Branch:** `agent/shadcn-native-gallery-slice1-opus55`
- **Review Branch:** `review/shadcn-native-gallery-slice1-visual-opus55`
- **Based On (Branch HEAD):** `8247984dfb7db0f7d94d2eb8f03fbc8910d04ef2`
- **Production Implementation Candidate:** `cbb39d31985567861a68fec39f22b95728310e36`
- **Frozen Reference Publication Base:** `dc70f6644c0384170f3a6929eb2caaf33fc711a0`
- **Frozen Visual / Reference Payload:** `2e14e417a69ef8967b8fb17120282e3de6449fae`
- **Approved Devctl Tooling:** `cf8ab919b399ca4465a17710741f1c888b84cfa0` (closure review `e8ab6f36ed36a82e776999d27577c66fa4e899f4`)

---

## 1. Executive Summary

An independent visual and geometric review of `agent/shadcn-native-gallery-slice1-opus55` (`8247984` / code `cbb39d3`) was conducted against the frozen Shadcn `base-nova` / `neutral` reference (`reference/shadcn-gallery-v0.1/contract.json`, `tokens.json`, and canonical Light/Dark screenshots) using both the committed evidence in `benchmark/results/shadcn-native-gallery-slice1/` and one coherent live Windows acceptance pass across **Light**, **Dark**, and **System** modes.

Several important improvements from intermediate builds are confirmed:
- The `KIND_ROW` vertical cross-axis alignment fix (`cross_v` in `src/platform/win32/layout.rs:599-610`) resolved the row stretch bug; all `Button` (`xs=24`, `sm=28`, `default=32`, `lg=36`) and `IconButton` (`icon-xs=24`, `icon-sm=28`, `icon=32`, `icon-lg=36`) specimens now have exact heights (`dh = 0.0`).
- Bundled `Geist` (`Geist-Variable.ttf`) and `Geist Mono` (`GeistMono-Variable.ttf`) are privately registered and active via DirectWrite (`src/platform/win32/fonts.rs`).
- `Button` disabled states (`disabled:opacity-50` via D2D `PushLayer(opacity: 0.5)`), `Destructive` Light/Dark tint and text colors (`#e7000b` on `#e7000b/10` in Light; `#ff6467` on `#ff6467/20` in Dark), `Ghost`/`Link` vertical rhythm, and `IconButton` square sizing/centering look visually coherent and faithful to the Nova reference.

However, **six blocking visual/fidelity defects (`V01`–`V06`)** remain in the Slice-1 candidate and must be corrected before scaling these patterns to Slice 2:
1. **`V01` (`BLOCKING`) — Windows Dark-mode non-client title bar remains stark white** (`DWMWA_USE_IMMERSIVE_DARK_MODE` is never set; `DwmGetWindowAttribute` returns `0` in Dark mode).
2. **`V02` (`BLOCKING`) — Inline-icon buttons (`button.default.icon-inline-start` & `button.default.icon-inline-end`) collide text into the icon** because `button_content_insets` omits `icon_size + icon_gap` (`16 + 6 = 22dp`), causing DirectWrite to center `"Add item"` and `"Continue"` across the full button width right over the icon (`3dp`–`4dp` overlap).
3. **`V03` (`BLOCKING`) — Unfocused `button.*.invalid` specimens omit the required `3dp` outer destructive ring** (`aria-invalid:ring-3 aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40`), rendering only a `1dp` red border because `render.rs:598` gates the ring on `be.focus == Some(id)` instead of `be.focus == Some(id) || *invalid`.
4. **`V04` (`BLOCKING`) — `Card` outer `ring-1 ring-foreground/10` edge ring is completely invisible** because `draw_shadow` (`src/platform/win32/render.rs:1240-1501`) ignores `shadow.spread: Dp(1.0)` and zeroes out the mask when `blur_sigma == 0` and `offset == (0,0)`, leaving Light cards invisible white-on-white.
5. **`V05` (`BLOCKING`) — `Card` (`card.default` and `card.default.size-sm`) internal part layout, insets, typography, footer chrome, and button alignment materially diverge from the frozen reference** (`header` has `8dp` all-around padding instead of `16dp`/`12dp` horizontal-only; `title` is `14px` regular instead of `16px`/`14px` medium; `content` is at `x=0` with `0dp` horizontal padding and wrong `MutedForeground` color; `footer` lacks `bg-muted/50` fill, `1dp` `border-t`, and `16dp`/`12dp` padding; `Cancel`/`Save` buttons are left-aligned at `x=8` instead of right-aligned because `KIND_ROW` ignores `Justify::End`).
6. **`V06` (`BLOCKING`) — `Typography` page specimen fidelity defects:** `typography.inline-code` stretches across the full `868dp` window width instead of hugging its text; `typography.code-block` doubles its padding (`24dp/22dp`) and inserts an `8dp` gap between code lines (`height = 120dp` vs `88dp`) due to an unconfigured inner `Column::new()`, uses `#fafafa` instead of `#a1a1a1` in Dark mode, and emits `line[N]` instead of `pre.line[N]`; and all 9 non-code-block `typography.*` specimens use non-canonical text strings instead of the frozen `contract.json` specimen copy.

---

## 2. Explicit Disposition of Owner-Observed Visual Concerns (`A`–`J`)

| Concern | Status | Visual / Geometric Evidence |
|---|---|---|
| **A. Windows dark-mode non-client title bar** | **FINDING (`V01` BLOCKING)** | In Dark mode (`shell.theme.dark`), the client area is `#0a0a0a` while the native Windows DWM caption/title bar remains pure white (`#ffffff`). `DwmGetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE)` returns `0` (`FALSE`) in `System`, `Light`, and `Dark` modes; `DwmSetWindowAttribute` for `DWMWA_USE_IMMERSIVE_DARK_MODE` is never called anywhere in `src/platform/win32/`. |
| **B. Button typography / vertical alignment** | **PASS (text-only)** | Geist Sans `14px` (`12.8px` sm, `12px` xs) weight `500` (`DWRITE_FONT_WEIGHT_MEDIUM`) is active and vertically centered via `DWRITE_PARAGRAPH_ALIGNMENT_CENTER` inside `24/28/32/36dp` button boxes. (Horizontal centering on inline-icon buttons is broken — see **H / `V02`**). |
| **C. Disabled-state consistency** | **PASS** | All 6 `Button` variants and all 5 `IconButton` variants apply whole-control `opacity: 0.5` (`PushLayer` in `src/platform/win32/render.rs:570-584`) over each variant's own base recipe (`Primary`, `Secondary`, `Outline`, `Ghost`, `Destructive`, `Link`), preserving variant differentiation in both Light and Dark modes. |
| **D. Invalid-state border/ring treatment** | **FINDING (`V03` BLOCKING)** | All 6 unfocused `button.*.invalid` specimens render **only** a `1dp` red border (`#e7000b` in Light, `#ff6467 @ 50%` in Dark) and **completely omit** the required `3dp` outer `ring-destructive/20` (dark `/40`) halo (`box_shadow: 0 0 0 3px` in `contract.json`) because `render.rs:598` only draws `vs.focus_ring` when `be.focus == Some(id)`. |
| **E. Destructive button tone/weight** | **PASS** | Matches frozen Nova tokens in both Light (`bg: #e7000b @ 10%`, `fg: #e7000b`, `hover: #e7000b @ 20%`) and Dark (`bg: #ff6467 @ 20%`, `fg: #ff6467`, `hover: #ff6467 @ 30%`). Soft tinted pill with crisp red label/icon, not a heavy solid burgundy fill. |
| **F. Ghost/Link vertical rhythm** | **PASS** | `button.ghost` and `button.link` sit on the exact `32.0dp` row height and share baseline alignment with sibling buttons; `button.link` underlines cleanly on hover. |
| **G. `xs`/`sm`/`lg` size consistency** | **PASS** | After the `cross_v` fix in `src/platform/win32/layout.rs:599-610`, `button.default.size-xs` (`24dp`), `size-sm` (`28dp`), `button.default` (`32dp`), and `size-lg` (`36dp`) all measure `dh = 0.0dp` with proper radii (`8/8/10/10dp`) and font scale (`12/12.8/14/14px`). |
| **H. Inline icon size/gap/alignment** | **FINDING (`V02` BLOCKING)** | In `button.default.icon-inline-start` (`"Add item"`) and `button.default.icon-inline-end` (`"Continue"`), the `16×16dp` Lucide vector icon is placed at the correct edge (`x=9` / `x=78.7`), but the text layout box (`render.rs:641-652`) is not inset by `icon_size + icon_gap` (`22dp`). The centered text collides into the icon (`3dp` overlap into `ArrowLeftCircle` on `"Add item"`; `4dp` overlap into `ArrowRight` on `"Continue"`). |
| **I. IconButton optical centering/stroke/size** | **PASS (with `V07` note)** | All 5 variants (`default`, `secondary`, `outline`, `ghost`, `destructive`) and 4 sizes (`icon-xs` `24×24`, `icon-sm` `28×28`, `icon` `32×32`, `icon-lg` `36×36`) have exact `0.0dp` width/height deltas, centered `12/14/16/16dp` Lucide `ArrowRight` vector strokes (`2 * size / 24` stroke width, round caps/joins). |
| **J. Row/section spacing and overall Gallery polish** | **FINDING (`V04`, `V05`, `V06` BLOCKING)** | While the top button grid is clean, the `Card` section at the bottom of `Components` is visually broken (invisible card border/ring, unindented `x=0` content text, unstyled left-aligned footer buttons) and the `Typography` page has a full-width stretched `inline-code` bar, a bloated `code-block`, and non-canonical specimen text. |

---

## 3. Detailed Findings (`V01`–`V09`)

### `V01` (`BLOCKING`) — Windows Dark-Mode Non-Client Title Bar Remains Light

- **Scope:** Section 4A, Section 5 (`WINDOWS TITLE BAR — REQUIRED`), Section 17, Section 18.
- **Observation:**
  - When the Gallery resolves to Dark mode (`shell.theme.dark`, or `shell.theme.system` when Windows is in Dark mode), the D2D client area clears to `#0a0a0a` (`ColorRole::Background`), but the native Win32 top-level title/caption bar remains **stark white (`#ffffff`)**.
  - Live Win32 query `DwmGetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE /* 20 */, ...)` during our Windows acceptance pass returned:
    - `System` mode: `0` (`FALSE`)
    - `Light` mode: `0` (`FALSE`)
    - `Dark` mode: `0` (`FALSE`)
  - Full-window screen capture of the native `HWND` in Dark mode (`window-full-dark-components.png`) confirms the white non-client title bar above the dark `#0a0a0a` client area.
- **Root Cause:**
  - `src/platform/win32/window.rs` (`create_window`) and `src/platform/win32/mod.rs` / `render.rs` never call `DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &dark_bool, size_of::<BOOL>())` on window creation or when `be.rt.theme.dark` changes.
- **Required Correction:**
  - Synchronize `DWMWA_USE_IMMERSIVE_DARK_MODE` on the top-level `HWND` with the resolved `be.rt.theme.dark` state on initial creation and whenever `be.rt.theme.dark` transitions across `Light`, `Dark`, and `System` modes.

---

### `V02` (`BLOCKING`) — Inline-Icon Button Text Overlaps / Collides With the Icon

- **Scope:** Section 4H, Section 10 (`INLINE ICON BUTTONS`).
- **Specimens Affected:** `button.default.icon-inline-start` (`"Add item"`), `button.default.icon-inline-end` (`"Continue"`), and any `ui.button` using `.icon_start(..)` or `.icon_end(..)`.
- **Frozen Reference Authority (`contract.json`):**
  - `button.default.icon-inline-start` (`width = 100.563dp, height = 32dp`):
    - `parts.icon`: `{x: 9, y: 8, width: 16, height: 16}` (`1dp` border + `8dp` reduced start padding)
    - `text` (`"Add item"`): `{x: 31, y: 7, width: 58.563, height: 18}` (`9 + 16 icon + 6 gap = 31dp`)
  - `button.default.icon-inline-end` (`width = 101.109dp, height = 32dp`):
    - `text` (`"Continue"`): `{x: 11, y: 7, width: 59.109, height: 18}` (`1dp` border + `10dp` start padding)
    - `parts.icon`: `{x: 76.109, y: 8, width: 16, height: 16}` (`11 + 59.109 + 6 gap = 76.109dp`)
- **Actual Native Rendering (`el-button.default.icon-inline-start/native-canonical.png`, `el-button.default.icon-inline-end/native-canonical.png`, `light-components.png`, `dark-components.png`):**
  - `button_content_insets` (`src/layout.rs:298-327`) returns `left = 9.0, right = 11.0` for `icon-inline-start` and `left = 11.0, right = 9.0` for `icon-inline-end` — it does **not** add `button_icon_size(size) + button_icon_gap(size)` (`16 + 6 = 22dp`) to the icon-bearing side.
  - `src/platform/win32/render.rs:640-656` then creates a centered `IDWriteTextLayout` of width `br.width - ins.left - ins.right` at `X = br.left + ins.left`:
    - For `"Add item"` (`104.59dp` wide): text is centered across `x = 9..93.59dp`, placing `"Add item"` at `x ≈ 22.0..80.6dp` while the `ArrowLeftCircle` icon occupies `x = 9..25dp` (**`3dp` horizontal collision** of `"A"` into the circle icon).
    - For `"Continue"` (`103.72dp` wide): text is centered across `x = 11..94.72dp`, placing `"Continue"` at `x ≈ 23.0..82.7dp` while the `ArrowRight` icon occupies `x = 78.72..94.72dp` (**`4dp` horizontal collision** of `"e"` into the arrow shaft).
- **Required Correction:**
  - Inset the button text layout rect on the icon-bearing side(s) by `button_icon_size(size) + button_icon_gap(size)` so the text occupies the remaining text slot (`x = 31dp` for `icon_start` at `Md`, ending `6dp` before `icon_end`).

---

### `V03` (`BLOCKING`) — Unfocused `invalid` Buttons Omit the Required 3dp Outer Destructive Ring

- **Scope:** Section 4D, Section 7 (`BUTTON STATES`), Section 18 (`DARK THEME DETAILS`).
- **Specimens Affected:** `button.default.invalid`, `button.secondary.invalid`, `button.outline.invalid`, `button.ghost.invalid`, `button.destructive.invalid`, `button.link.invalid` in both Light and Dark modes.
- **Frozen Reference Authority (`style-nova.css:150`, `contract.json` `button.*.invalid`):**
  - `.cn-button` specifies `aria-invalid:border-destructive dark:aria-invalid:border-destructive/50 aria-invalid:ring-3 aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40`.
  - In `contract.json`, unfocused `button.default.invalid` and `button.destructive.invalid` carry both the `1px` destructive border **and** `box_shadow: oklab(0.577 0.217662 0.112464 / 0.2) 0px 0px 0px 3px` (`#e7000b @ 20%` in Light, `#ff6467 @ 40%` in Dark).
- **Actual Native Rendering (`light-btn-default-invalid.png`, `dark-btn-default-invalid.png`, `light-components.png`, `dark-components.png`):**
  - `src/style.rs:1480-1484` sets `v.focus_ring = Color::Role(ColorRole::DestructiveRing)` when `invalid == true`, but `src/platform/win32/render.rs:598` only draws `vs.focus_ring` when `be.focus == Some(id) && !*disabled`.
  - Consequently, unfocused invalid buttons render with **only a 1dp red border** and **no 3dp outer destructive ring**, making `button.*.invalid` look like an ordinary button with a thin red border.
- **Required Correction:**
  - Paint the `3dp` outer ring (`vs.focus_ring`) whenever `(be.focus == Some(id) || *invalid) && !*disabled`.

---

### `V04` (`BLOCKING`) — `Card` Outer `ring-1 ring-foreground/10` Edge Ring Is Completely Invisible

- **Scope:** Section 15 (`CARD`), Section 18 (`DARK THEME DETAILS`).
- **Specimens Affected:** `card.default`, `card.default.size-sm` in Light and Dark modes.
- **Frozen Reference Authority (`contract.json` lines 642, 898; `tokens.json` `foreground-10`):**
  - `card.default` and `card.default.size-sm` carry a `1px` outer edge ring (`ring-1 ring-foreground/10`: `#0a0a0a @ 10%` in Light, `#fafafa @ 10%` in Dark) around the `14dp`-radius card container.
- **Actual Native Rendering (`el-card.default/native-canonical.png`, `el-card.default.size-sm/native-canonical.png`, `light-components.png`, `dark-components.png`):**
  - `card_recipe` (`src/style.rs:1532-1552`) sets `border: Border::default()` and `shadow: Some(Shadow { color: Color::Role(ColorRole::Foreground10), offset_x: Dp(0.0), offset_y: Dp(0.0), blur_sigma: Dp(0.0), spread: Dp(1.0) })`.
  - In `src/platform/win32/render.rs:1240-1501`, `draw_shadow` **never reads `shadow.spread`**. Because `blur_sigma == 0.0` and `offset == (0, 0)`, `mask` is identical to `solid`, and line `1458` (`*a = (*a - excl(i)).max(0.0) * sa`) subtracts `solid` from `solid`, producing **all-zero alpha (`0.0`)**.
  - As a result, the Card's `1dp` edge ring is never painted: in Light mode (`#ffffff` card on `#ffffff` background), the card boundary is completely invisible; in Dark mode (`#171717` card on `#0a0a0a` background), the `ring-foreground/10` edge ring is absent.
- **Required Correction:**
  - Honor `shadow.spread` in `draw_shadow` (expanding the outer silhouette by `spread` while excluding only the unexpanded `solid` box interior) or stroke the `1dp` ring geometry directly.

---

### `V05` (`BLOCKING`) — `Card` Internal Part Geometry, Insets, Typography, Footer Chrome, and Button Alignment Materially Diverge From Frozen Authority

- **Scope:** Section 4J, Section 15 (`CARD`), Section 16 (`CARD SEMANTIC PARTS AS VISUAL AID`).
- **Specimens Affected:** `card.default` (`dh = -4.38dp`: `170.62dp` actual vs `175.00dp` expected) and `card.default.size-sm` (`dh = +6.37dp`: `158.62dp` actual vs `152.25dp` expected).
- **Part-by-Part Comparison (Live Devctl Snapshot vs Frozen `contract.json`):**

| Card / Part | Frozen Reference (`contract.json` rel Dp) | Native Actual (`snapshot.json` rel Dp) | Root Cause of Discrepancy |
|---|---|---|---|
| `card.default` | `0, 0, 288 × 175` | `0, 0, 288 × 170.62` (`dh = -4.38`) | Sum of `header` (`+12.62dp`) and `footer` (`-17.00dp`) vertical errors below. |
| `card.default#header` | `x: 0, y: 16, w: 288, h: 46` | `x: 0, y: 16, w: 288, h: 58.62` | `Column::new().gap(Space::Xs)` in `examples/gallery.rs:282` inherits `Column::default()`'s `padding: Space::Sm` (`8dp` on all 4 sides), adding `16dp` of unwanted vertical padding (`8dp` top + `8dp` bottom) and `8dp` horizontal padding instead of `px: 16dp, py: 0dp`. |
| `card.default#header.title` | `x: 16, y: 16, w: 256, h: 22` (`16px` Medium `lh=22`) | `x: 8, y: 24, w: 58.92, h: 18.62` (`14px` Normal) | Authored as `ui.label("Card Title")` (`14px` weight `400`) instead of `16px` weight `500` `lh=22px`, and inset by `8dp` instead of `16dp`. |
| `card.default#header.description` | `x: 16, y: 42, w: 256, h: 20` | `x: 8, y: 46.62, w: 191.67, h: 20` | Inset by `8dp` (`x=8`) instead of `16dp` (`x=16`) and pushed down by `header`'s `8dp` top padding. |
| `card.default#content` | `x: 0, y: 78, w: 288, h: 20` (text at `x: 16`, color `CardForeground` `#0a0a0a`) | `x: 0, y: 90.62, w: 288, h: 20` (text at **`x: 0`**, color `MutedForeground` `#737373`) | Bare `ui.typography(Typography::Muted, ...)` has **`0dp` horizontal padding** (text is flush against the outer left edge of the card!) and uses `MutedForeground` instead of `CardForeground`. |
| `card.default#footer` | `x: 0, y: 114, w: 288, h: 61` (`bg-muted/50`, `1dp` `border-t`, `16dp` padding, buttons right-aligned at `x: 159.8..261`) | `x: 0, y: 126.62, w: 288, h: 44` (transparent bg, no `border-t`, `8dp` padding, buttons **left-aligned** at `x: 8..133`) | Authored as a bare `Row::new().gap(Space::Sm).justify(Justify::End)`: lacks `bg-muted/50` + `border-t` + `16dp` padding, and `KIND_ROW` in `src/platform/win32/layout.rs:506-547` ignores `props.justify`, leaving `Cancel`/`Save` left-aligned. |
| `card.default.size-sm` | `0, 0, 288 × 152.25` (`header` `x=12, y=12, h=43.25`, `title` `14px` Medium `lh=19.25`, `content` text `x=12`, `footer` `y=99.25, h=53` with `12dp` pad + `1dp` border-t) | `0, 0, 288 × 158.62` (`dh = +6.37`; `header` still `h=58.62` at `x=8`, `content` at `x=0`, `footer` still `h=44` at `x=8`) | Only the outer card top padding (`12dp`) and part gap (`12dp`) change for `CardSize::Sm`; all interior parts in `examples/gallery.rs:310-338` are identical to `card.default`. |

---

### `V06` (`BLOCKING`) — Typography Page Specimen Fidelity (`inline-code`, `code-block`, and Specimen Copy)

- **Scope:** Section 14 (`GEIST TYPOGRAPHY`), Section 17, Section 18.
- **Observed Defects (`light-typography.png`, `dark-typography.png`, `light-typography-snapshot.json`):**
  1. **`typography.inline-code` stretches across the full window width (`868 × 26.4dp` vs frozen `194.41 × 24.38dp`):**
     - `typography_page` (`examples/gallery.rs:354`) uses `Column::new().gap(Space::Lg).align(Align::Stretch)`. Because `Label` with `chrome: Some(..)` accepts the stretched `inner.width` from its parent column, `typography.inline-code` paints a full-width `868dp` gray bar across the page instead of an inline-sized code pill hugging its text.
     - In addition, `Typography::InlineCode` (`src/typography.rs:93`) sets `size: Dp(14.0)` whereas `tokens.json:573-578` specifies `13px` (`text-[0.8125rem]`) for `typography.inline-code`.
  2. **`typography.code-block` (`src/ui.rs:1085-1117`) has doubled padding, `8dp` inter-line gaps, wrong Dark foreground role, and flat part keys:**
     - `ui.code_block` nests `ui.column(Column::new(), ...)` inside `ui.box_(chrome)`. Because `Column::new()` defaults to `padding: Space::Sm (8dp)` and `gap: Space::Sm (8dp)` on top of `chrome`'s `padding: 14dp/16dp`, the code lines sit at `x = 24dp, y = 22, 50, 78dp` and total height is **`120.0dp`** instead of the frozen **`88.0dp`** (`x = 16dp, y = 14, 34, 54dp` with `0dp` inter-line gap).
     - In Dark mode (`dark-typography.png`), `ui.code_block` uses `TextStyle::default()` (`ColorRole::Foreground` `#fafafa`) instead of `ColorRole::SurfaceForeground` / `ColorRole::CodeForeground` (`#a1a1a1`, `tokens.json:327-339`).
     - Semantic parts on `typography.code-block` emit `pre`, `line[1]`, `line[2]`, `line[3]` instead of `pre`, `pre.line[1]`, `pre.line[2]`, `pre.line[3]`.
  3. **All 9 non-code-block `typography.*` specimens in `examples/gallery.rs:357-389` use placeholder text instead of frozen `contract.json` specimen copy:**
     - `typography.h1`: `"Design is intelligence made visible"` (`1` line, `48dp` tall) vs frozen `"Taxing Laughter: The Joke Tax Chronicles"` (`2` lines, `96dp` tall in a `720dp` column).
     - `typography.h2`: `"Heading 2 with a border rule"` vs frozen `"The People of the Kingdom"`.
     - `typography.h3`: `"Heading 3"` vs frozen `"The Joke Tax"`.
     - `typography.h4`: `"Heading 4"` vs frozen `"People stopped telling jokes"`.
     - `typography.p`: `"The king's pawn..."` vs frozen `"The king, seeing how much happier his subjects were, realized the error of his ways and repealed the joke tax."`.
     - `typography.lead`: `"A lead paragraph..."` vs frozen `"A modal dialog that interrupts the user with important content and expects a response."`.
     - `typography.small`: `"Small caption text"` vs frozen `"Email address"`.
     - `typography.muted`: `"Muted helper text"` vs frozen `"Enter your email address."`.
     - `typography.inline-code`: `"rust-ui --install"` vs frozen `"npx shadcn@latest init"`.

---

### `V07` (`NON_BLOCKING`) — Button `bg-clip-padding` (`1dp` Transparent Border Inset) & `pressed` Translate Clipping

- **Scope:** Section 6 (`BUTTON VISUAL FIDELITY`), Section 7 (`BUTTON STATES`), Section 11 (`ICONBUTTON`).
- **Observation:**
  - Frozen `.cn-button` (`style-nova.css:150`) uses `border: 1px solid transparent; background-clip: padding-box`, so buttons with a transparent border (`Default`, `Secondary`, `Ghost`, `Destructive`) paint their background inside the `1px`-inset padding box (`30×30` fill inside a `32×32` border box), and buttons with a translucent border (`Outline` and `invalid` in Dark mode) do not bleed background fill underneath the `1px` border stroke.
  - `paint_box` (`src/platform/win32/render.rs:1540`) fills the full outer `border-box` geometry `r` before stroking the border. This accounts for the `1px` perimeter ring in `compare-element` diffs (`el-button.default/diff.png`, `el-icon-button.default/diff.png`) and slightly alters translucent border contrast in Dark mode.
  - Additionally, when a button is `pressed` (`active:translate-y-px`), `render.rs:560` shifts the painted rect `br` down by `+1.0dp`, so `screenshot --id` (which crops to the unshifted layout rect `r`) clips the bottom `1px` of the shifted button.

---

### `V08` (`NON_BLOCKING`) — Text-Bearing Button Width Deltas (`-0.50dp … +2.95dp` on Non-Icon Buttons)

- **Scope:** Section 13 (`WIDTH DELTAS`).
- **Observation:**
  - Across text-only buttons (`button.default` `+2.51dp`, `button.secondary` `+0.43dp`, `button.outline` `+2.95dp`, `button.ghost` `+2.33dp`, `button.destructive` `-0.50dp`, `button.link` `+2.08dp`, `size-xs` `+0.01dp`, `size-sm` `+0.53dp`, `size-lg` `+1.28dp`), approximately `+2.0dp` of the positive width bias comes from `src/platform/win32/layout.rs:168`:
    ```rust
    let w = tw + ins.left.0 + ins.right.0 + icon_w(icon_start) + icon_w(icon_end) + 4.0;
    ```
    Since `ins.left.0 + ins.right.0` (`src/layout.rs:312-316`) **already includes** the `2.0dp` border width (`1dp` left + `1dp` right) plus `20.0dp` padding (`10dp` left + `10dp` right), the `+ 4.0` adds a full `4.0dp` of unconditional width slack on top of DirectWrite's `GetMetrics().width`.
  - Visually, the extra `~1–2dp` per side on text buttons does not distort button hierarchy, so it is classified `NON_BLOCKING`, though reducing the extra slack in `layout.rs:168` would tighten width parity.

---

### `V09` (`NON_BLOCKING`) — Gallery Shell Heading & Viewport Scrollability

- **Scope:** Sections 19, 20, 21, 25 (`GALLERY PAGE ARCHITECTURE`, `CURRENT WINDOW SIZE`, `HEADER / NAVIGATION POLISH`, `RESPONSIVENESS`).
- **Observation:**
  - At the default `964 × 861` client viewport (`980 × 900` outer window), the Gallery navbar (`All`, `Components`, `Forms`, `Navigation`, `Typography`, `Overlays`, `Native Text`, plus `Light`/`Dark`/`System` icon buttons) and all Slice-1 `Components` (`y = 48..715dp`) and `Typography` specimens fit cleanly without clipping.
  - Unimplemented Slice-1 pages (`All`, `Forms`, `Navigation`, `Overlays`, `Native Text`) are honestly disabled in the navbar (`!p.implemented()`).
  - Note for future slices: `examples/gallery.rs` does not yet wrap page content in a scroll container if the window is resized below `~730dp` height.

---

## 4. Component Support Ledger (`docs/COMPONENT_SUPPORT.md`) Check

- Confirmed that `docs/COMPONENT_SUPPORT.md` marks `Button`, `IconButton`, `Card / Surface`, and `Typography` as `IMPLEMENTED` with `Visual parity: partial` and `Interaction parity: partial` (not `VERIFIED`), which remains honest while Slice-1 visual rework is pending.

---

## 5. Final Verdict & Mandatory Questions

```text
SHADCN_NATIVE_GALLERY_SLICE1_VISUAL_REWORK_REQUIRED
```

```text
Is this Slice-1 implementation visually trustworthy enough
to use its theme/component/rendering patterns as the foundation
for the rest of the frozen native Shadcn Gallery?

NO
```

Blocking findings `V01`, `V02`, `V03`, `V04`, `V05`, and `V06` must be resolved before Slice-1 visual approval and before starting Slice 2.

---

```text
EXECUTION FOOTER

RUN_KEY: RUI-O-261019-1335
ROLE: Opus 5.5 Reviewer
PROJECT: rceman/rust-ui
BRANCH: review/shadcn-native-gallery-slice1-visual-opus55
STARTED_AT: 2026-10-19T14:12:14+03:00
FINISHED_AT: 2026-10-19T14:32:00+03:00
BASED_ON: 8247984dfb7db0f7d94d2eb8f03fbc8910d04ef2
PRODUCTION_HEAD: cbb39d31985567861a68fec39f22b95728310e36
REPORT_HEAD: PENDING_COMMIT
STATUS: SHADCN_NATIVE_GALLERY_SLICE1_VISUAL_REWORK_REQUIRED
NEXT_OWNER: User
```
