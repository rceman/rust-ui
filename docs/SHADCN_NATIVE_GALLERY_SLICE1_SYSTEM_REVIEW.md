# Final System / Contract / Platform Closure Review — Shadcn Native Gallery Slice 1

## 1. Identity & Immutability Verification

- **Repository:** `rceman/rust-ui`
- **Authoritative Source Checkout (WSL):** `/home/therceman/git/rust-ui`
- **Dedicated System Review Worktree (WSL):** `/home/therceman/git/rust-ui-slice1-system-astra`
- **Implementation / Publication Branch:** `agent/shadcn-native-gallery-slice1-opus55`
- **Review Branch:** `review/shadcn-native-gallery-slice1-system-astra`
- **Reviewed Publication / Evidence HEAD (`BASED_ON`):** `8d3d27361071ee8915dd8f496ea98e8e9b005f0a`
- **Production Implementation Under Review (`PRODUCTION_HEAD`):** `e49966d529e6da5c2cacd4d88c3048dd58f8e640`
- **Previous Production Candidate:** `8247984dfb7db0f7d94d2eb8f03fbc8910d04ef2` (`cbb39d31985567861a68fec39f22b95728310e36`)
- **Independent Visual Closure (`review/shadcn-native-gallery-slice1-visual-closure-argon`):** `3e9ebad7feef30c517292a72b60f0d6a363d89f9` (`SHADCN_NATIVE_GALLERY_SLICE1_VISUALLY_APPROVED`, `SLICE2_VISUAL_IMPLEMENTATION_CLEARED`)
- **Frozen Reference Publication:** `dc70f6644c0384170f3a6929eb2caaf33fc711a0` (payload `2e14e417a69ef8967b8fb17120282e3de6449fae`)
- **Approved `rust-ui-devctl` Implementation:** `cf8ab919b399ca4465a17710741f1c888b84cfa0`

### History & Reference Immutability

1. Commits after `e49966d529e6da5c2cacd4d88c3048dd58f8e640` (`0ebc88d5c2b93849c549a0082caeb789e923508b` and `8d3d27361071ee8915dd8f496ea98e8e9b005f0a`) touch only `benchmark/results/shadcn-native-gallery-slice1/INDEX.md`, `benchmark/results/shadcn-native-gallery-slice1/titlebar/*`, and `docs/COMPONENT_SUPPORT.md`.
2. `reference/shadcn-gallery-v0.1/` is bit-for-bit identical to `dc70f6644c0384170f3a6929eb2caaf33fc711a0:reference` (`e0bd8eb16d38b778edab4d4ee23ca823696ceb43`).

---

## 2. Section-by-Section System / Contract / Platform Audit

### Section 2 — Generic Row Justification (`src/geom.rs`, `src/ui.rs`, `src/platform/win32/layout.rs`)

- **Public enum (`src/geom.rs:382-387`):** `pub enum Justify` defines `Start` (default), `Center`, and `End`. No `SpaceBetween` or other unimplemented enum variant exists on `Justify`.
- **`KIND_ROW` main-axis calculation (`src/platform/win32/layout.rs:501-553`):**
  - `gap` (`props.gap.map(space_px).unwrap_or(0.0)`) is subtracted once before `waterfill(inner.width - gap * (n - 1), &items)` and summed once in `used = sizes.iter().sum::<f32>() + gap * (n - 1)`.
  - `slack = (inner.width - used).max(0.0)` clamps negative free space to `0.0`, preventing negative offsets on overflow.
  - `Justify::Start => 0.0`, `Justify::Center => slack / 2.0`, `Justify::End => slack` shift the starting `x` inside `inner` (which already accounts for `node_insets` padding/border once), while `cross_v` computes `(h, y)` independently on the cross axis.
  - `Content`, `Fixed`, and `Fill` items interact coherently: unconstrained `Fill` items consume remaining width in `waterfill` (`slack = 0.0`), while `Content` / `Fixed` / max-capped `Fill` items leave `slack >= 0.0` for `Justify`.
- **Systemic gap identified (`S03`):** `pub struct Column` (`src/ui.rs:133-169`) also exposes `pub fn justify(mut self, j: Justify) -> Self` and stages `justify: Some(self.justify)` into `ContainerProps`, but `KIND_COLUMN | KIND_CARD` (`src/platform/win32/layout.rs:460-500`) silently ignores `props.justify` (`let mut y = inner.y;`). Because `Column::justify` is part of the public API, it must either apply the symmetric vertical `slack` offset in `KIND_COLUMN | KIND_CARD` or not be silently ignored.

---

### Section 3 — `Shadow.spread` Generic Rendering (`src/platform/win32/render.rs:1186-1519`)

- **Cache identity (`ShadowKey`, `render.rs:1186-1199, 1348`):** `spread: u32` (`spread.to_bits()`) is included in `ShadowKey` alongside `target_gen`, `width`, `height`, `pad`, `radii`, `sigma`, `color`, `scale`, and `offset_px`. Different spread values never alias in `ShadowCache`.
- **Zero-spread preservation:** When `spread == 0.0`, `pad_dip = sigma * 3.0`, `d - spread == d`, and `mask` equals `solid` prior to the 3-pass box blur, preserving prior `spread = 0` behavior.
- **Positive spread & corner radii:** `pad_dip = sigma * 3.0 + spread.max(0.0)` expands the raster pad by `spread`, and `mask[y * w + x] = (0.5 - scale.to_physical_f(d - spread)).clamp(0.0, 1.0)` dilates the signed-distance field by `spread` DIP before blurring and before subtracting the unexpanded interior `solid` (`(*a - excl(i)).max(0.0) * sa`).
- **Resource bounds:** `pad_px_f`, `w_px_f`, `h_px_f` are checked against `0.0..=SHADOW_MAX_DIM` (`4096`), `checked_mul` / `checked_add` guard `w`, `h`, `n_px`, and `bytes <= SHADOW_CACHE_MAX_BYTES` (`32 MiB`).
- **Non-finite `spread` edge case (`S04`):** Line `1264` checks `!shadow.offset_x.0.is_finite() || !shadow.offset_y.0.is_finite()`, but does not check `!shadow.spread.0.is_finite()`. Because `(f32::NAN).max(0.0)` evaluates to `0.0` in Rust IEEE-754 semantics, `spread = f32::NAN` bypasses the `pad_dip.is_finite()` check at line `1277` and produces `NaN` in `d - spread` at line `1410`. Adding `!spread.is_finite()` to the early guard at line `1264` (or `einval` check) closes this hole.

---

### Section 4 — DirectWrite `GetLineMetrics` Fix (`src/platform/win32/render.rs:204-220`)

- **Implementation:**
  ```rust
  let mut count = 0u32;
  let _ = lay.GetLineMetrics(None, &mut count);
  let mut line_metrics = vec![DWRITE_LINE_METRICS::default(); count.max(1) as usize];
  lay.GetLineMetrics(Some(&mut line_metrics), &mut count)
      .map_err(|e| UiError::Platform(format!("GetLineMetrics: {e}")))?;
  ```
- **Assessment:** Placed at the generic `layout_for` boundary shared by `measure_fn()` and `Renderer::paint`. Properly queries `count` via the two-call DirectWrite pattern, floors allocation at `count.max(1) as usize` (safe for 0/1/multi-line text), and propagates any second-call `HRESULT` failure as `UiError::Platform`. Single-line labels are unaffected; wrapped labels (`h1`, `p`, `lead`) no longer fail with `E_NOT_SUFFICIENT_BUFFER`.

---

### Section 5 — Windows DWM Caption Appearance (`src/platform/win32/mod.rs:367, 684-701, 767`)

- **Platform boundary:** `Backend::sync_dark_caption(&self, dark: bool)` takes only the generic resolved `self.rt.theme.dark` boolean during `Backend::turn()`. No Shadcn or Gallery types cross into `src/platform/win32/`.
- **Idempotence & lifecycle:** `dark_caption: Cell<Option<bool>>` caches the last applied state on `Backend` so `DwmSetWindowAttribute(self.hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, ...)` is called on the existing `HWND` only when `dark` changes (initial creation, `UpdateCtx::set_theme`, or `WM_SETTINGCHANGE` `ImmersiveColorSet`).
- **Failure safety:** `let _ = DwmSetWindowAttribute(...);` ignores unsupported/older OS failures without aborting `turn()` or client rendering.
- **Feature-gate defect (`S01`):** `src/platform/win32/mod.rs:35` unconditionally imports `windows::Win32::Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute}` in normal runtime code, but `"windows/Win32_Graphics_Dwm"` is still only enabled under `[features].devtools` (`Cargo.toml:17`) and is missing from `[target."cfg(windows)".dependencies].windows.features` (`Cargo.toml:43`). See **`S01` (`BLOCKING`)** below.

---

### Section 6 — Button Intrinsic Measurement (`src/layout.rs:215-336`, `src/platform/win32/layout.rs:129-166`, `src/platform/win32/render.rs:645-667`)

- **Shared inset authority:** `button_content_insets` (`src/layout.rs:300-336`) computes `border + icon_side_inset(pad) + button_icon_size(size) + button_icon_gap(size)` on each icon-bearing side (`icon_start` and/or `icon_end`), and `border + pad` on text-only sides.
- **Coherence across variants/states/sizes:** Both `natural` (`tw + ins.left + ins.right + 2.0` in `src/platform/win32/layout.rs:157-165`) and `Renderer::paint` (`X: br.left + ins.left.0`, width `br.width - ins.left - ins.right` in `src/platform/win32/render.rs:647-666`) consume `button_content_insets`. Works for text-only, `icon_start`, `icon_end`, both icons, and all `ControlSize` variants (`Xs`, `Sm`, `Md`, `Lg`).
- **`NO_WRAP` scoping:** `lay.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)` is applied only on the button's `IDWriteTextLayout` in `render.rs:657` (`measure` already passes `max_w = f32::MAX` at `layout.rs:158`), so generic `NodeData::Label` wrapping is unaffected.

---

### Section 7 — Card Part Recipes / Ownership (`src/style.rs:1530-1605`, `examples/gallery.rs:367-412`)

- **Recipe ownership:** `card_recipe`, `card_spacing`, `card_part_insets`, `card_part_box`, `card_title_style`, `card_content_style`, and `card_footer_style` live in `src/style.rs` (and are re-exported in `src/lib.rs::prelude`). `Space::None` (`0dp`) lives in `src/theme.rs`.
- **No reference-coordinate hacks:** `examples/gallery.rs` composes `card.default` and `card.default.size-sm` using those generic recipes plus `Justify::End` on the footer `Row`, with zero hardcoded pixel coordinates.

---

### Section 8 — Typography Generic API (`src/typography.rs`, `src/ui.rs:1086-1144`, `examples/gallery.rs:304-364`)

- **Generic API vs specimen copy:** `src/typography.rs` and `Ui::typography` / `Ui::code_block` (`src/ui.rs:1086-1144`) contain only generic style/chrome/layout recipes (`FontFamily::Sans` / `Mono`, `TextSize::Exact`, `TextWeight`, `line_height`, `letter_spacing`, `Length::Content` for `InlineCode`). Frozen specimen strings live exclusively in `examples/gallery.rs`.
- **Semantic part defect in `code_block` (`S02`):** `Ui::code_block` (`src/ui.rs:1130-1143`) stages `self.part("pre", ...)` on the outer `box_` and `ui.part("line[N]", ...)` on each line, documenting that the emitted part keys are `pre` and `pre.line[N]`. However, when `ui.named("typography.code-block", ...)` attaches the `automation_id` to that same outer `box_`, `src/platform/win32/devtools.rs:109-128` drops the owner's `part_key` (`"pre"`) when recursing into `self.collect_parts(id, &mut Vec::new(), &mut parts)`, emitting `"line[1..3]"` instead of `"pre.line[1..3]"`. See **`S02` (`BLOCKING`)** below.

---

### Section 9 — Devtools / Automation Regression (`src/platform/win32/devtools.rs:107-173`)

- Inspecting `src/platform/win32/devtools.rs` revealed two concrete semantic-part regressions introduced in Slice 1 (`S02`):
  1. **Trailing-icon Button part omitted (`devtools.rs:151`):** Calls `crate::layout::button_icon_rect(&n.data, &vs, &r, false)` with `icon_end` hardcoded to `false`. For `button.default.icon-inline-end` (`icon_start: None, icon_end: Some(Icon::ArrowRight)`), `button_icon_rect(..., false)` returns `None`, so no `"icon"` part is emitted in `snapshot-layout` (`parts: None` / `[]`), and `compare-element` / `compare-layout` on `button.default.icon-inline-end#icon` fails with `TARGET_NOT_FOUND`.
  2. **Owner `part_key` prefix dropped in `collect_parts` (`devtools.rs:109-128`):** When a node with `automation_id.is_some()` also carries `part_key = Some(pk)` (as in `typography.code-block` where the outer `box_` carries both `automation_id = "typography.code-block"` and `part_key = "pre"`), `dev_node` pushes `pk` (`"pre"`) as a part, but then calls `self.collect_parts(id, &mut Vec::new(), &mut parts)` with an empty `prefix` instead of `vec![pk.to_string()]`. Child parts are therefore emitted as `line[1..3]` instead of `pre.line[1..3]`, breaking `contract.json` lookup for `typography.code-block#pre.line[1..3]`.

---

### Section 10 — Default Feature / Dependency Hygiene (`Cargo.toml`, `src/tests.rs`)

- **Important Fact-Check Against `8d3d27361071ee8915dd8f496ea98e8e9b005f0a` (`PRODUCTION_HEAD: e49966d529e6da5c2cacd4d88c3048dd58f8e640`):**
  - In Argon's visual closure review (`3e9ebad7feef30c517292a72b60f0d6a363d89f9`), Argon noted that moving `windows/Win32_Graphics_Dwm` into `[target."cfg(windows)".dependencies]` and adding `spread: Dp(0.0)` to 5 Windows-only `Shadow` literals in `src/tests.rs` **had not yet been done** in `e49966d529e6da5c2cacd4d88c3048dd58f8e640` / `8d3d27361071ee8915dd8f496ea98e8e9b005f0a`.
  - Inspecting `Cargo.toml` and `src/tests.rs` at `8d3d27361071ee8915dd8f496ea98e8e9b005f0a` confirms that both defects are **still present in the codebase**:
    1. `Cargo.toml:17` lists `"windows/Win32_Graphics_Dwm"` only under `[features].devtools`, while `Cargo.toml:43` (`[target."cfg(windows)".dependencies].windows.features`) omits `"Win32_Graphics_Dwm"`.
    2. `src/tests.rs` lines `4130`, `4240`, `4265`, `4287`, and `7509` (inside `#[cfg(all(test, target_os = "windows"))]`) still construct `Shadow { color, blur_sigma, offset_x, offset_y }` without `spread`.
  - Consequently, on Windows:
    - `cargo.exe check --locked --examples` (default features, without `--features devtools`) **fails to compile** with `error[E0432]: unresolved import windows::Win32::Graphics::Dwm`.
    - `cargo.exe test --locked --lib` (even with `--features devtools`) **fails to compile** with 5 `error[E0063]: missing field spread in initializer of style::Shadow` errors.
  - Under Astra's system/contract authority (Sections 10, 11, 13, and Universal Gates `G13`, `G16`, `G20`), normal Windows builds must not depend on `--features devtools` to compile, and the Windows `--lib` unit test target must compile and pass. This is recorded as **`S01` (`BLOCKING`)**.

---

### Section 11 — Event-Driven / Performance Invariant

- **Runtime loop:** `Backend::sync_dark_caption` runs only inside `Backend::turn()` and short-circuits via `self.dark_caption.get() == Some(dark)`. Zero timers, polling loops, or continuous redraws were added.
- **Font registration:** `fonts::collection` uses `OnceLock` (`register_once`), registering bundled Geist fonts once per process.
- **Shadow cache:** Bounded at `48` entries / `32 MiB` / `4096px` max dimension, cleared on device loss or DPI change.
- **Devtools coupling:** Once `Win32_Graphics_Dwm` is moved from `[features].devtools` to `[target."cfg(windows)".dependencies].windows.features` (`S01`), the Gallery runtime has zero dependency on `devtools`.

---

### Section 12 — Test Coverage Assessment

- The 5 new unit tests in `src/tests.rs:8392-8514` (`button_content_insets_reserves_icon_space`, `invalid_button_gets_destructive_ring_recipe`, `card_recipe_shadow_has_unit_spread_ring`, `card_part_recipes_match_frozen_contract`, `inline_code_recipe_is_13px_and_wraps_content`) test pure style/inset recipes on Linux/Windows.
- However, because the Worker only ran `cargo test --lib` on WSL Linux, the 5 broken `Shadow` struct literals in the Windows-gated `#[cfg(all(test, target_os = "windows"))]` tests (`src/tests.rs:4130, 4240, 4265, 4287, 7509`) went unnoticed, and there was no unit test covering:
  - `devtools.rs` semantic part extraction for trailing-icon buttons (`icon_end`) or root-part owners (`code_block` `pre.line[N]`),
  - `KIND_ROW` / `KIND_COLUMN` `Justify::{Start, Center, End}` layout positioning on Windows.

---

### Section 13 — Build / Test Gate Execution Results

1. **WSL Linux (`cargo fmt --check`, `cargo check --locked --examples`, `cargo test --locked --lib`):**
   - `cargo fmt --check`: **PASS** (clean)
   - `cargo check --locked --examples`: **PASS** (`0 errors`, `#[cfg(target_os = "windows")]` inactive on Linux)
   - `cargo test --locked --lib`: **PASS** (`107 passed; 0 failed`)
2. **Windows Native Authority (`cargo.exe` against `/home/therceman/git/rust-ui-slice1-system-astra`):**
   - `cargo.exe check --locked --examples` (default features): **FAIL (`1 error`)**
     ```text
     error[E0432]: unresolved import `windows::Win32::Graphics::Dwm`
       --> src\platform\win32\mod.rs:35:31
     ```
   - `cargo.exe check --locked --examples --features devtools`: **PASS (`0 errors`)**
   - `cargo.exe test --locked --lib --features devtools --no-run`: **FAIL (`5 errors`)**
     ```text
     error[E0063]: missing field `spread` in initializer of `style::Shadow` (src\tests.rs:4130)
     error[E0063]: missing field `spread` in initializer of `style::Shadow` (src\tests.rs:4240)
     error[E0063]: missing field `spread` in initializer of `style::Shadow` (src\tests.rs:4265)
     error[E0063]: missing field `spread` in initializer of `style::Shadow` (src\tests.rs:4287)
     error[E0063]: missing field `spread` in initializer of `style::Shadow` (src\tests.rs:7509)
     ```

---

## 3. Summary of System Findings (`S01`–`S04`)

### `S01` (`BLOCKING`) — Default Windows Build Fails Without `--features devtools` and Windows `cargo test --lib` Fails to Compile

- **Location:** `Cargo.toml:17, 43` and `src/tests.rs:4130, 4240, 4265, 4287, 7509`
- **Root Cause:**
  1. `src/platform/win32/mod.rs:35` unconditionally imports `windows::Win32::Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute}`, but `"windows/Win32_Graphics_Dwm"` is only enabled under `[features].devtools` (`Cargo.toml:17`) and is missing from `[target."cfg(windows)".dependencies].windows.features` (`Cargo.toml:43`). Normal Windows builds (`cargo.exe check --examples`, `cargo.exe build --example gallery`) fail with `E0432`.
  2. Five `Shadow { ... }` struct literals inside `#[cfg(all(test, target_os = "windows"))]` in `src/tests.rs` (`4130, 4240, 4265, 4287, 7509`) were not updated when `pub spread: Dp` was added to `Shadow`, causing `cargo.exe test --lib` on Windows to fail with 5 `E0063` errors.
- **Required Fix:**
  1. Add `"Win32_Graphics_Dwm"` to `[target."cfg(windows)".dependencies].windows.features` in `Cargo.toml` (and remove the redundant entry from `[features].devtools`).
  2. Add `spread: crate::geom::Dp(0.0)` to the 5 legacy `Shadow` struct initializers in `src/tests.rs` (`4130, 4240, 4265, 4287, 7509`) and verify `cargo.exe check --locked --examples` and `cargo.exe test --locked --lib` pass on Windows.

---

### `S02` (`BLOCKING`) — Devtools Semantic Part Exposure Drops Trailing-Icon Button Parts and Root-Part Owner Prefixes

- **Location:** `src/platform/win32/devtools.rs:108-172`
- **Root Cause:**
  1. **Trailing-icon Button part omitted (`devtools.rs:151`):** `dev_node` calls `crate::layout::button_icon_rect(&n.data, &vs, &r, false)` with `icon_end` hardcoded to `false`. When a button has `icon_start: None` and `icon_end: Some(...)` (e.g. `button.default.icon-inline-end`), `button_icon_rect(..., false)` returns `None`, so no `"icon"` part is emitted in `snapshot-layout` (`parts: None`), and `compare-element` / `compare-layout` on `button.default.icon-inline-end#icon` fails with `TARGET_NOT_FOUND`.
  2. **Owner `part_key` prefix dropped (`devtools.rs:109-128`):** When an automation-ID owner node also carries `part_key = Some(pk)` (as in `typography.code-block`, where `ui.named("typography.code-block", ...)` and `self.part("pre", ...)` attach to the same outer `box_`), `dev_node` pushes `PartNode { key: "pre", ... }` at line `110`, but then calls `self.collect_parts(id, &mut Vec::new(), &mut parts)` at line `128` with an empty prefix instead of `vec![pk.to_string()]`. Child parts are emitted as `"line[1]"`, `"line[2]"`, `"line[3]"` instead of `"pre.line[1]"`, `"pre.line[2]"`, `"pre.line[3]"` (contradicting `src/ui.rs:1106`, `docs/SHADCN_NATIVE_GALLERY_SLICE1_REWORK_REPORT.md:85`, and `reference/shadcn-gallery-v0.1/contract.json`).
- **Required Fix:**
  1. In `src/platform/win32/devtools.rs:151`, pass `let icon_end = !*icon_only && icon_start.is_none() && icon_end.is_some();` (or emit icon parts for all populated icon edges consistently with `render.rs:630`).
  2. In `src/platform/win32/devtools.rs:109-128`, initialize the prefix passed to `self.collect_parts` with `n.part_key`:
     ```rust
     let mut prefix = Vec::new();
     if let Some(pk) = &n.part_key {
         ...
         prefix.push(pk.to_string());
     }
     self.collect_parts(id, &mut prefix, &mut parts);
     ```

---

### `S03` (`BLOCKING`) — `Column::justify` Public API Silently Ignored in `KIND_COLUMN | KIND_CARD` Layout

- **Location:** `src/ui.rs:136, 152-168` and `src/platform/win32/layout.rs:460-500`
- **Root Cause:** `Column` exposes `pub fn justify(mut self, j: Justify) -> Self` and stages `justify: Some(self.justify)` into `ContainerProps`, identical to `Row::justify`. Slice 1 implemented `props.justify` in `KIND_ROW` (`src/platform/win32/layout.rs:515-527`), but left `KIND_COLUMN | KIND_CARD` (`src/platform/win32/layout.rs:460-500`) hardcoded to `let mut y = inner.y;`. Per Section 2 ("Do not accept silently ignored public variants"), `KIND_COLUMN | KIND_CARD` must apply the symmetric main-axis `slack` offset (`Justify::Start => 0.0`, `Justify::Center => slack / 2.0`, `Justify::End => slack`).
- **Required Fix:** Apply the same 8-line `used` / `slack` / `props.justify` offset calculation to `y` in the `KIND_COLUMN | KIND_CARD` branch of `src/platform/win32/layout.rs:474`.

---

### `S04` (`NON_BLOCKING`) — `draw_shadow` Non-Finite `spread` Guard

- **Location:** `src/platform/win32/render.rs:1264`
- **Observation:** `draw_shadow` checks `!shadow.offset_x.0.is_finite() || !shadow.offset_y.0.is_finite()` at line `1264`, but does not check `!shadow.spread.0.is_finite()`. Because `(f32::NAN).max(0.0)` evaluates to `0.0` in Rust, `spread = f32::NAN` passes `pad_dip.is_finite()` at line `1277` and produces `NaN` in `d - spread` at line `1410`. Adding `!spread.is_finite()` to the early guard at line `1264` is a 1-line defensive hardening.

---

## 4. Universal Gates (1–20) Evaluation

| Gate | Status | Notes |
|---|---|---|
| **G1–G2** Scope & Architecture | **PASS** | Desktop-only, Windows-first native backend; no forbidden dependencies or platform leaks into `src/` outside `platform/`. |
| **G3** Behavior Preservation | **PASS** | `spread = 0`, `Justify::Start`, single-line text, and text-only buttons preserve prior behavior. |
| **G4–G8** Contracts, Ownership & Reuse | **PASS** | Generic DWM bool boundary, centralized Card/Button/Typography recipes in `src/style.rs` / `src/layout.rs` / `src/typography.rs`. |
| **G9–G10** Deterministic Semantics | **FAIL (`S02`, `S03`)** | `devtools.rs` drops `icon_end` button part and `code_block` `pre.` part prefix; `KIND_COLUMN` ignores `Column::justify`. |
| **G11–G12** Failure Safety & Boundedness | **PASS** (`S04` note) | `GetLineMetrics` two-call allocation and `ShadowCache` (`48` entries / `32 MiB` / `4096px`) are bounded. |
| **G13** Dependency & Feature Scope | **FAIL (`S01`)** | `Win32_Graphics_Dwm` is gated under `devtools` instead of base Windows dependencies, breaking default Windows builds. |
| **G14–G15** Performance & Event-Driven | **PASS** | Zero polling loops or continuous redraws; DWM attribute synced only on resolved `dark` change. |
| **G16** Verification & Test Gate | **FAIL (`S01`)** | Windows `cargo.exe check --examples` (default features) and `cargo.exe test --lib` fail to compile due to `Win32_Graphics_Dwm` and 5 un-updated `Shadow` test literals. |
| **G17–G20** Evidence & Systemic Completeness | **FAIL (`S01`–`S03`)** | Linux-only `cargo check`/`cargo test` masked Windows-gated compile failures and `devtools.rs` part key bugs. |

---

## 5. Final Verdict

```text
SHADCN_NATIVE_GALLERY_SLICE1_SYSTEM_REWORK_REQUIRED
```

```text
Can Slice 2 safely build on the generic layout/render/text/platform/component
changes introduced by Slice 1 without first reworking their system contracts?

NO
```

*(Once the bounded fixes in `S01`–`S03` — moving `Win32_Graphics_Dwm` to base Windows features in `Cargo.toml`, adding `spread: Dp(0.0)` to the 5 Windows-only `Shadow` test literals in `src/tests.rs`, fixing `icon_end` and owner `part_key` prefix propagation in `src/platform/win32/devtools.rs`, and honoring `props.justify` in `KIND_COLUMN | KIND_CARD` in `src/platform/win32/layout.rs` — are applied, no visual rework is needed as none of these changes alter the rendered pixels approved by Argon.)*

```text
EXECUTION FOOTER

RUN_KEY: RUI-A-261019-2215
ROLE: Astra Advisor
PROJECT: rceman/rust-ui
BRANCH: review/shadcn-native-gallery-slice1-system-astra
STARTED_AT: 2026-10-19T22:06:02+03:00
FINISHED_AT: 2026-10-19T22:16:30+03:00
BASED_ON: 8d3d27361071ee8915dd8f496ea98e8e9b005f0a
PRODUCTION_HEAD: e49966d529e6da5c2cacd4d88c3048dd58f8e640
REPORT_HEAD: PENDING_COMMIT_SHA
STATUS: SHADCN_NATIVE_GALLERY_SLICE1_SYSTEM_REWORK_REQUIRED
NEXT_OWNER: User
```
