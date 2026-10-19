# Independent Visual / UX Closure Review — Native Shadcn Gallery Slice 1 (`V01`–`V06`)

## 1. Review Identity, Scope & Immutability

- **Repository:** `rceman/rust-ui`
- **Authoritative Source Checkout (WSL):** `/home/therceman/git/rust-ui`
- **Dedicated Review Worktree (WSL):** `/home/therceman/git/rust-ui-slice1-closure-argon`
- **Implementation Branch:** `agent/shadcn-native-gallery-slice1-opus55`
- **Review Branch:** `review/shadcn-native-gallery-slice1-visual-closure-argon`
- **Reviewed Publication / Evidence HEAD (`BASED_ON`):** `8d3d27361071ee8915dd8f496ea98e8e9b005f0a`
- **Production Implementation Candidate (`PRODUCTION_HEAD`):** `e49966d529e6da5c2cacd4d88c3048dd58f8e640`
- **Previous Implementation Reviewed:** `8247984dfb7db0f7d94d2eb8f03fbc8910d04ef2` (production `cbb39d31985567861a68fec39f22b95728310e36`)
- **Previous Independent Visual Review:** `75f24a6e2bd7a8b9f3d80cb13f9d23f4f3c651cf` (`review/shadcn-native-gallery-slice1-visual-opus55`, status `SHADCN_NATIVE_GALLERY_SLICE1_VISUAL_REWORK_REQUIRED`)
- **Frozen Shadcn Reference Publication:** `dc70f6644c0384170f3a6929eb2caaf33fc711a0`
- **Frozen Visual / Reference Payload:** `2e14e417a69ef8967b8fb17120282e3de6449fae`
- **Approved `rust-ui-devctl` v0.1:** implementation `cf8ab919b399ca4465a17710741f1c888b84cfa0`, closure review `e8ab6f36ed36a82e776999d27577c66fa4e899f4` (`RUST_UI_DEVCTL_V0_1_APPROVED` / `NATIVE_GALLERY_TOOLING_CLEARED`)

### Commit & Reference Immutability Verification

1. **History from previous review base `8247984dfb7db0f7d94d2eb8f03fbc8910d04ef2` to `8d3d27361071ee8915dd8f496ea98e8e9b005f0a`:**
   - `e49966d529e6da5c2cacd4d88c3048dd58f8e640` — `slice1: V01-V06 visual rework — title-bar theme, button/card/typography parity` (production implementation + updated `el-*` comparison artifacts + rework report)
   - `0ebc88d5c2b93849c549a0082caeb789e923508b` — `slice1: docs/evidence hygiene — candidate id, card/button notes, V01 titlebar proof` (docs/evidence-only)
   - `8d3d27361071ee8915dd8f496ea98e8e9b005f0a` — `slice1: fix stale INDEX status — READY_FOR_VISUAL_CLOSURE` (docs/evidence-only)
2. **Post-production diff check (`git diff --stat e49966d529e6da5c2cacd4d88c3048dd58f8e640 8d3d27361071ee8915dd8f496ea98e8e9b005f0a`):**
   - Touches only `benchmark/results/shadcn-native-gallery-slice1/INDEX.md`, `benchmark/results/shadcn-native-gallery-slice1/titlebar/*`, and `docs/COMPONENT_SUPPORT.md`. Zero source/implementation files were modified after `e49966d529e6da5c2cacd4d88c3048dd58f8e640`.
3. **Frozen reference immutability:**
   - `git rev-parse dc70f6644c0384170f3a6929eb2caaf33fc711a0:reference` and `git rev-parse 8d3d27361071ee8915dd8f496ea98e8e9b005f0a:reference` both resolve to `e0bd8eb16d38b778edab4d4ee23ca823696ceb43`.

---

## 2. Independent Verification of Blocking Findings (`V01`–`V06`)

### `V01` — Native Windows Dark-Mode Non-Client Title Bar (`CLOSED`)

- **Previous Defect:** In Dark mode (`#0a0a0a` client surface), the native Windows top-level non-client title/caption bar remained stark white (`#ffffff`) because `DWMWA_USE_IMMERSIVE_DARK_MODE` was never set on the `HWND`.
- **Rework Implementation (`src/platform/win32/mod.rs:360-401, 765-767`):**
  - `Backend` tracks `dark_caption: Cell<Option<bool>>` and calls `Backend::sync_dark_caption(dark)` during `turn()` using the generic resolved `self.rt.theme.dark` boolean.
  - `sync_dark_caption` invokes `DwmSetWindowAttribute(self.hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &val, size_of::<BOOL>())` on the existing `HWND` only when the resolved light/dark state changes — without recreating the window, without custom-drawn caption chrome, and without leaking Shadcn-specific types into platform code.
- **Committed Evidence & Live Independent Verification:**
  - Committed full-window captures (`benchmark/results/shadcn-native-gallery-slice1/titlebar/window-light.png`, `window-dark.png`, `window-system.png`) and `titlebar-proof.txt` were inspected directly.
  - Independently verified on live `examples/gallery` (`HWND = 2360526`, client `964.0 × 861.0` @ scale `1.0`):
    - **System mode (initial launch, system dark):** `DwmGetWindowAttribute(hwnd, 20)` = `1`; full-window capture confirms dark native caption over `#0a0a0a` dark client.
    - **Light mode (`click shell.theme.light`):** `DwmGetWindowAttribute(hwnd, 20)` = `0`; full-window capture confirms light native caption over `#ffffff` light client.
    - **Dark mode (`click shell.theme.dark`):** `DwmGetWindowAttribute(hwnd, 20)` = `1`; full-window capture confirms dark native caption over `#0a0a0a` dark client on the existing `HWND`.
- **Verdict:** **`V01 CLOSED`**

---

### `V02` — Inline-Icon Button Content Grouping & Single-Line Labels (`CLOSED`)

- **Previous Defect:** `button_content_insets` (`src/layout.rs`) did not reserve `icon_size + icon_gap` (`16 + 6 = 22dp`) on the icon-bearing side, causing `render.rs` to center the button label across the full button width and collide `"Add item"` / `"Continue"` into their Lucide icons. Additionally, `"Size sm"` wrapped onto two lines when laid out in a tight box.
- **Rework Implementation (`src/layout.rs:240-278`, `src/platform/win32/layout.rs:157-166`, `src/platform/win32/render.rs:645-667`):**
  - `button_content_insets` now reserves `border + icon_side_inset(pad) + button_icon_size(size) + button_icon_gap(size)` (`1 + 8 + 16 + 6 = 31dp` for `ControlSize::Md`) on each icon-bearing edge.
  - Both natural-width measurement (`tw + ins.left + ins.right + 2.0`) and `DrawTextLayout` placement (`X: br.left + ins.left.0`, width `br.width - ins.left - ins.right`) share `button_content_insets`, forming one centered `[16dp icon + 6dp gap + label]` content group.
  - `render.rs:657` applies `lay.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)` (`whitespace-nowrap`) so button labels never wrap onto multiple lines.
- **Committed Evidence & Live Independent Verification:**
  - `button.default.icon-inline-start`: native `102.59 × 32.00dp` vs frozen `100.563 × 32dp` (`dh = 0.0`, `dw = +2.03dp` DirectWrite glyph advance); icon part at `rel_x = 9.0dp, rel_y = 8.0dp, 16×16dp`, label starts at `x = 31.0dp` (`6dp` clear gap, zero overlap).
  - `button.default.icon-inline-end`: native `101.72 × 32.00dp` vs frozen `101.109 × 32dp` (`dh = 0.0`, `dw = +0.61dp`); canonical dimensions `102 × 32` match frozen `102 × 32` (`missing_pixels = 0`), with `"Continue"` and the trailing `ArrowRight` icon cleanly separated by `6dp`.
  - `button.default.size-sm`: native `67.43 × 28.00dp` vs frozen `68.906 × 28dp` (`dh = 0.0`); canonical dimensions `68 × 28` match frozen `68 × 28` (`missing_pixels = 0`), rendering `"Size sm"` on a single centered line in both Light and Dark modes.
- **Verdict:** **`V02 CLOSED`**

---

### `V03` — Invalid Button Outer Destructive Ring (`CLOSED`)

- **Previous Defect:** `src/platform/win32/render.rs` gated the `3dp` outer ring on `be.focus == Some(id) && !*disabled`, so unfocused `button.*.invalid` specimens only rendered a `1dp` red border and omitted the required `3dp` `ring-destructive/20` (Light) / `ring-destructive/40` (Dark) outer halo.
- **Rework Implementation (`src/platform/win32/render.rs:602-619`):**
  - The ring paint condition is now `(be.focus == Some(id) || *invalid) && !*disabled`.
  - `resolve_button` (`src/style.rs`) assigns `ColorRole::DestructiveRing` (`#e7000b @ 20%` in Light, `#ff6467 @ 40%` in Dark) whenever `invalid == true`, independent of focus.
- **Committed Evidence & Live Independent Verification:**
  - Inspected `el-button.default.invalid/native-canonical.png` and live Light/Dark captures (`components_light.png`, `components_dark.png`).
  - All 6 unfocused invalid specimens (`button.default.invalid`, `button.secondary.invalid`, `button.outline.invalid`, `button.ghost.invalid`, `button.destructive.invalid`, `button.link.invalid`) visibly render both the `1dp` destructive border (`#e7000b` Light / `#ff6467 @ 50%` Dark) and the `3dp` outer destructive ring in both Light and Dark themes.
  - `button.destructive` (soft translucent red fill, no border, no outer ring) remains visually distinct from `button.default.invalid` (primary fill + destructive border + `3dp` destructive outer ring) and `button.destructive.invalid` (destructive fill + destructive border + `3dp` destructive outer ring).
- **Verdict:** **`V03 CLOSED`**

---

### `V04` — Card Outer `1dp` Ring / Generic `Shadow.spread` Rendering (`CLOSED`)

- **Previous Defect:** `card_recipe` specified the `ring-1 ring-foreground/10` perimeter ring via `Shadow { blur_sigma: 0.0, offset: (0,0), spread: Dp(1.0) }`, but `draw_shadow` in `src/platform/win32/render.rs` ignored `shadow.spread` and subtracted `solid` from `solid` (`alpha = 0`), leaving Cards with no visible outer boundary.
- **Rework Implementation (`src/platform/win32/render.rs:1190-1425`):**
  - `ShadowKey` now includes `spread: u32` (`spread.to_bits()`).
  - `pad_dip` expands by `sigma * 3.0 + spread.max(0.0)`.
  - `solid` records the unexpanded interior silhouette (`0.5 - scale.to_physical_f(d)`), while `mask` records the spread-dilated silhouette (`0.5 - scale.to_physical_f(d - spread)`) prior to optional Gaussian box-blur passes and interior exclusion (`mask * (1.0 - solid)`).
  - This is a generic renderer correction for `Shadow.spread` rather than Card-specific paint code.
- **Committed Evidence & Live Independent Verification:**
  - Inspected `el-card.default/native-canonical.png`, `el-card.default.size-sm/native-canonical.png`, `el-card.default/diff.png`, and live Light/Dark captures.
  - Both `card.default` and `card.default.size-sm` display a crisp, continuous `1dp` `foreground/10` rounded outer ring around the `12dp` (`Default`) and `8dp` (`Sm`) card corners in both Light and Dark themes.
- **Verdict:** **`V04 CLOSED`**

---

### `V05` — Card Part Composition, Typography, Insets & Footer Layout Parity (`CLOSED`)

- **Previous Defect:** `card.default` (`dh = -4.38dp`) and `card.default.size-sm` (`dh = +6.37dp`) suffered from multiple part-level errors: `header` inherited `8dp` all-around `Column` padding (`x = 8` instead of `16`/`12`, `+16dp` height), `title` used `14px` regular `ui.label` instead of `16px/22` and `14px/19.25` medium, `content` had `0dp` horizontal inset (`x = 0`) and `MutedForeground` color, and `footer` lacked `bg-muted/50` + `1dp` `border-t` while `Cancel`/`Save` were left-aligned because `KIND_ROW` ignored `Justify::End`.
- **Rework Implementation (`src/style.rs:1534-1605`, `src/platform/win32/layout.rs:515-527`, `examples/gallery.rs:367-412`):**
  - `KIND_ROW` in `src/platform/win32/layout.rs` now honors `props.justify` (`Justify::Start`, `Justify::Center`, `Justify::End`) by offsetting `x` by `0.0`, `slack / 2.0`, or `slack`.
  - Centralized Card part recipes in `src/style.rs` (`card_part_insets`, `card_part_box`, `card_title_style`, `card_content_style`, `card_footer_style`) and `Space::None` (`0dp`) container padding eliminate accidental `8dp` container padding inheritance.
- **Committed Evidence & Live Independent Verification (`components_light.json` vs `contract.json`):**

| Card / Part | Native Relative Rect (`x, y, w × h` dp) | Frozen Reference Relative Rect (`x, y, w × h` px) | Parity |
|---|---|---|---|
| **`card.default`** | `0.0, 0.0, 288.0 × 175.0` | `0, 0, 288 × 175` | **Exact (`dw = 0.0, dh = 0.0`)** |
| `card.default#header` | `0.0, 16.0, 288.0 × 46.0` | `0, 16, 288 × 46` | **Exact (`dh = 0.0`)** |
| `card.default#header.title` | `16.0, 16.0, 69.58 × 22.0` | `16, 16, 70.64 × 22` | **Exact pos & `22dp` lh (`16px` Medium)** |
| `card.default#header.description` | `16.0, 42.0, 191.67 × 20.0` | `16, 42, 194.25 × 20` | **Exact pos & `20dp` lh (`MutedForeground`)** |
| `card.default#content` | `0.0, 78.0, 288.0 × 20.0` (text at `x = 16.0`) | `0, 78, 288 × 20` (text at `x = 16`) | **Exact pos, `20dp` lh & `CardForeground`** |
| `card.default#footer` | `0.0, 114.0, 288.0 × 61.0` | `0, 114, 288 × 61` | **Exact (`bg-muted/50` + `1dp border-t` + right-aligned `Cancel`/`Save`)** |
| **`card.default.size-sm`** | `0.0, 0.0, 288.0 × 152.25` | `0, 0, 288 × 152.25` | **Exact (`dw = 0.0, dh = 0.0`)** |
| `card.default.size-sm#header` | `0.0, 12.0, 288.0 × 43.25` | `0, 12, 288 × 43.25` | **Exact (`dh = 0.0`)** |
| `card.default.size-sm#header.title` | `12.0, 12.0, 60.88 × 19.25` | `12, 12, 62.94 × 19.25` | **Exact pos & `19.25dp` lh (`14px` Medium)** |
| `card.default.size-sm#header.description` | `12.0, 35.25, 191.67 × 20.0` | `12, 35.25, 194.25 × 20` | **Exact pos & `20dp` lh (`MutedForeground`)** |
| `card.default.size-sm#content` | `0.0, 67.25, 288.0 × 20.0` (text at `x = 12.0`) | `0, 67.25, 288 × 20` (text at `x = 12`) | **Exact pos, `20dp` lh & `CardForeground`** |
| `card.default.size-sm#footer` | `0.0, 99.25, 288.0 × 53.0` | `0, 99.25, 288 × 53` | **Exact (`bg-muted/50` + `1dp border-t` + right-aligned `Cancel`/`Save`)** |

- **Verdict:** **`V05 CLOSED`**

---

### `V06` — Typography Hierarchy, Multi-Line Wrapping, Copy & Code Recipes (`CLOSED`)

- **Previous Defect:**
  1. `layout_for` in `src/platform/win32/render.rs` passed a 1-element `[DWRITE_LINE_METRICS::default(); 1]` buffer to `GetLineMetrics`, which failed with `E_NOT_SUFFICIENT_BUFFER` whenever any text wrapped onto 2+ lines (`h1`, `p`, `lead`).
  2. `typography.inline-code` stretched to the full column width instead of hugging its content and used `14dp` instead of `13px`.
  3. `typography.code-block` nested a default `Column::new()` (`8dp` padding + `8dp` gap) inside its `14dp/16dp` chrome (`height = 120dp` vs `88dp`) and used `#fafafa` (`Foreground`) instead of `#a1a1a1` (`CodeForeground`) in Dark mode.
  4. Non-code specimens used placeholder strings instead of the frozen `contract.json` specimen copy.
- **Rework Implementation (`src/platform/win32/render.rs:204-219`, `src/typography.rs:78-87`, `src/ui.rs:1088-1142`, `examples/gallery.rs:304-364`):**
  - `layout_for` now queries `lay.GetLineMetrics(None, &mut count)` first and allocates `vec![DWRITE_LINE_METRICS::default(); count.max(1) as usize]`, fixing multi-line `line_height` measurement and rendering across the entire backend.
  - `Typography::InlineCode` now uses `13.0dp` Geist Mono Semibold `lh20` with `Length::Content` width inside a non-stretching row wrapper.
  - `ui.code_block` uses `ColorRole::Code` surface, `ColorRole::CodeForeground` text (`#a1a1a1` in Dark mode), and `Column::new().padding(Space::None).gap(Space::None)` inside the `14dp/16dp` chrome (`height = 88.0dp`).
  - All 10 `typography.*` specimens in `examples/gallery.rs` use the exact frozen copy inside a `720dp` column.
- **Committed Evidence & Live Independent Verification (`typography_light.png`, `typography_dark.png`, `typography_light.json`):**
  - `typography.h1` (`"Taxing Laughter: The Joke Tax Chronicles"`): `720.0 × 96.0dp` (frozen `720 × 96`), wraps cleanly onto 2 lines in Light and Dark.
  - `typography.h2` (`"The People of the Kingdom"`): `720.0 × 45.0dp` (frozen `720 × 45`), with `1dp` bottom border rule and `8dp` bottom padding.
  - `typography.h3` (`"The Joke Tax"`): `720.0 × 32.0dp` (frozen `720 × 32`).
  - `typography.h4` (`"People stopped telling jokes"`): `720.0 × 28.0dp` (frozen `720 × 28`).
  - `typography.p` (`"The king, seeing how much happier his subjects were, realized the error of his ways and repealed the joke tax."`): `720.0 × 56.0dp` (frozen `720 × 56`), wraps cleanly onto 2 lines.
  - `typography.lead` (`"A modal dialog that interrupts the user with important content and expects a response."`): `720.0 × 56.0dp` (frozen `720 × 56`), wraps cleanly onto 2 lines in `MutedForeground`.
  - `typography.small` (`"Email address"`): `14.0dp` height (`14px` Medium `lh14`).
  - `typography.muted` (`"Enter your email address."`): `720.0 × 20.0dp` (frozen `720 × 20`).
  - `typography.inline-code` (`"npx shadcn@latest init"`): content-hugging `143.46 × 26.40dp` pill in `13px` Geist Mono Semibold on muted chrome.
  - `typography.code-block`: `720.0 × 88.0dp` (`dh = 0.0` vs frozen `88dp`), with semantic parts `pre` (`0, 0, 720×88`) and `line[1..3]` at `rel=(16, 14, h=20)`, `(16, 34, h=20)`, `(16, 54, h=20)` rendered in `#a1a1a1` (`CodeForeground`) on `#1f1f1f` (`Code`) in Dark mode.
- **Verdict:** **`V06 CLOSED`**

---

## 3. Non-Blocking Items (`V07`–`V09`) & Build Hygiene Note

- **`V07` (`NON_BLOCKING`, unchanged):** `paint_box` fills the outer `border-box` rather than `padding-box` (`bg-clip-padding`), and `pressed` (`+1dp` translate-y) shifts `1dp` beyond the static layout crop. Visually minor (`1dp` subpixel edge blend); remains non-blocking.
- **`V08` (`NON_BLOCKING`, incidentally closed):** Hardcoded `+4.0dp` button width slack was tightened to `+2.0dp` uniform ink-vs-layout epsilon plus `DWRITE_WORD_WRAPPING_NO_WRAP`, eliminating both excessive width slack and tight-box label wrapping.
- **`V09` (`NON_BLOCKING`, unchanged):** Slice-1 pages (`Components` and `Typography`) fit comfortably within the normal `964 × 861` desktop viewport (`y_max = 719dp` on `Components`, `y_max = 737dp` on `Typography`) and pass the bounded `820 × 800` desktop resize check without clipping. A scrollable page viewport container remains a known follow-up when taller catalog pages are populated.
- **Non-Blocking Build / Test Hygiene Note for Slice 2 (`NON_BLOCKING`):**
  - In `Cargo.toml`, `"windows/Win32_Graphics_Dwm"` is currently listed under the `devtools` feature (`Cargo.toml:17`) rather than `[target."cfg(windows)".dependencies].windows.features` (`Cargo.toml:43`), and 5 legacy `Shadow { ... }` struct literals inside `#[cfg(target_os = "windows")]` tests in `src/tests.rs` (lines `4130, 4240, 4265, 4287, 7509`) omit `spread: Dp(0.0)`. Moving `Win32_Graphics_Dwm` into the base Windows dependency features and adding `spread: Dp(0.0)` to those 5 Windows-only test literals should be cleaned up in the next implementation pass so non-`devtools` Windows builds and Windows `cargo test --lib` compile without flags. This does not affect any visual, layout, or runtime behavior of Slice 1.

---

## 4. Regression & Parity Summary Across Slice 1

- **Button regression pass:** All 6 variants (`default`, `secondary`, `outline`, `ghost`, `destructive`, `link`), 6 states (`normal`, `hover`, `pressed`, `focus-visible`, `disabled`, `invalid`), 4 sizes (`xs` `24dp`, `sm` `28dp`, `default` `32dp`, `lg` `36dp`), and both inline-icon specimens (`icon-inline-start`, `icon-inline-end`) were verified in Light and Dark modes. Live `hover` -> `press` -> `release` -> `leave` -> `focus` transitions on `button.default` and `button.destructive` settle cleanly with no stale hover or press state.
- **IconButton regression pass:** All 5 variants × normal/disabled and all 4 square sizes (`24×24`, `28×28`, `32×32`, `36×36`) maintain exact `0.0dp` width/height deltas and crisp optical centering.
- **Card & parts:** Both `card.default` (`288 × 175`) and `card.default.size-sm` (`288 × 152.25`) match the frozen contract in outer dimensions, `1dp` outer spread ring, part insets, typography hierarchy, muted footer surface + top border, and right-aligned `Cancel`/`Save` buttons.
- **Typography:** All 10 core recipes (`h1`–`h4`, `p`, `lead`, `small`, `muted`, `inline-code`, `code-block`) match frozen specimen copy, font family (`Geist` / `Geist Mono`), line heights, wrapping, and Light/Dark surface & foreground tokens.
- **Gallery shell & responsiveness:** Page navigation tabs, active tab pill, and `Light`/`Dark`/`System` theme buttons operate cleanly, synchronize the native Windows title bar immediately on the existing `HWND`, and remain unclipped across normal desktop resizing.
- **Component Support Ledger (`docs/COMPONENT_SUPPORT.md`):** Remains honestly at `IMPLEMENTED` pending post-review promotion.

---

## 5. Final Verdict

```text
V01 CLOSED
V02 CLOSED
V03 CLOSED
V04 CLOSED
V05 CLOSED
V06 CLOSED

SHADCN_NATIVE_GALLERY_SLICE1_VISUALLY_APPROVED
SLICE2_VISUAL_IMPLEMENTATION_CLEARED
```

```text
Is the Slice-1 native implementation now visually trustworthy enough
to use its theme, typography, Button/IconButton, Card, layout and rendering
patterns as the foundation for the remaining native Shadcn Gallery?

YES
```

```text
EXECUTION FOOTER

RUN_KEY: RUI-R-261019-2040
ROLE: Argon Reviewer
PROJECT: rceman/rust-ui
BRANCH: review/shadcn-native-gallery-slice1-visual-closure-argon
STARTED_AT: 2026-10-19T20:36:56+03:00
FINISHED_AT: 2026-10-19T20:51:15+03:00
BASED_ON: 8d3d27361071ee8915dd8f496ea98e8e9b005f0a
PRODUCTION_HEAD: e49966d529e6da5c2cacd4d88c3048dd58f8e640
REPORT_HEAD: PENDING_COMMIT_SHA
STATUS: SHADCN_NATIVE_GALLERY_SLICE1_VISUALLY_APPROVED
NEXT_OWNER: User
```
