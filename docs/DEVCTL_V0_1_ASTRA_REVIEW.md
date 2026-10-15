# Independent System / Contract / Platform Review — `rust-ui-devctl` v0.1

**Verdict:** `RUST_UI_DEVCTL_V0_1_REWORK_REQUIRED`

- **Run Key:** `RUI-A-261006-1250`
- **Reviewer:** Astra Advisor (System / Contract / Platform Authority)
- **Repository:** `rceman/rust-ui`
- **Candidate Branch:** `agent/rust-ui-devctl-v0.1`
- **Report / Publication HEAD:** `7a1ad19a397d10149b6d50ebdf5c1c60d0c6bc67`
- **Production Implementation HEAD:** `6375a4e537f0ec396c92154738f7bfced4324c5c`
- **Exact Base:** `dc70f6644c0384170f3a6929eb2caaf33fc711a0`
- **Frozen Shadcn Reference Payload:** `2e14e417a69ef8967b8fb17120282e3de6449fae`
- **Prior Rust Production Foundation:** `00dc29acfeb686d6a190d91624de7ab9a48e1e92`

---

## Executive Answer to the Central Question

> **Is `rust-ui-devctl` v0.1 genuinely trustworthy and sufficient for Opus 5.5 to implement native `rust-ui` Gallery against the frozen HTML reference without manual GUI inspection?**

**No — `RUST_UI_DEVCTL_V0_1_REWORK_REQUIRED`.**

The local session architecture, opt-in feature boundary, named-pipe framing/authentication, guarded `Mailbox` wake admission, `wait-idle` presentation barrier (`DwmFlush` + recheck), WGC-only capture pipeline, resource cleanup, and dependency scoping are well-designed and verified on native Windows (DPI 120 / scale `1.25`).

However, **`rust-ui-devctl` v0.1 fails its core cross-authority comparison mission**: comparing native `rust-ui` components rendered on the Windows host (`scale_factor = 1.25`) against the frozen DPR1 HTML reference (`reference/shadcn-gallery-v0.1/`). Specifically:

1. **`D01` (Blocker — Cross-Scale Visual Comparison Failure, §§9, 14, 21):** At host scale `1.25`, `compare-element` crops the frozen DPR1 reference PNG at `1.0 px/Dp` and the native WGC capture at `1.25 px/Dp`, then overlays the unscaled `1.0x` crop onto the top-left `(0, 0)` corner of the `1.25x` crop without scale normalization or a canonical `1.0x` comparison raster. The right 20% and bottom 20% of the native crop (~36% of all pixels) are unconditionally flagged as `missing_pixels` (solid magenta L-band in `diff.png`), and the remaining 64% compares spatially misaligned coordinates (`(x, y)` Dp in reference vs `(0.8x, 0.8y)` Dp in native). `compare-element` permanently exits `6` (`pass: false`, `dimensions_match: false`) with non-actionable diffs. In the committed evidence (`benchmark/results/rust-ui-devctl-v0.1/probe/element/comparison.json`), `compare-element` was only ever executed as a native self-comparison (`client-0.png` against itself), never against a frozen HTML reference PNG.
2. **`D02` (Blocker — Split Rounding Policy / Fractional-Dp Crop Off-by-One/Two Errors, §§8, 9, 14):** `crate::geom::ScaleFactor::to_physical` (`round()` half-away-from-zero per edge via `Rect::physical`) is the single coordinate-rounding authority in `rust-ui` and is used by `dev_node` (`src/platform/win32/devtools.rs:67`) to pre-round native `rect_px`. In contrast, `Image::crop` (`src/devtools/mod.rs:336-339`) implements a second, incompatible outward-expansion policy (`floor()` on `left`/`top`, `ceil()` on `right`/`bottom`). Because `compare-element` passes fractional reference `rect`s from `contract.json` directly into `Image::crop`, **421 out of 581 (72.5%)** element-capture records in `reference/shadcn-gallery-v0.1/contract.json` suffer a **1 px or 2 px width/height mismatch and up to 1 px origin shift even at scale `1.00` (DPR 1)** when native and reference logical `rect_dp` are identical (e.g., `button.default` at `y = 300.75, height = 32.0` crops to `70 × 33` instead of `70 × 32`; `badge.outline` at `x = 692.938, width = 54.109, height = 20` crops to `56 × 20` instead of `54 × 20`).
3. **`D03` (Blocker — Incomplete Frozen Contract Adapter for Parts, Hidden Parts & Tiers, §§11, 12):** `reference_rects` (`src/devtools/mod.rs:184-207`) reads only top-level `el["automation_id"]` and `cap["rect"]` with a hardcoded `(rect, true)` visibility tuple, completely ignoring `cap["parts"]` (820 part entries across all captures). Consequently, all **14 hidden-part records** (`{"visible": false, "reason": "not-rendered"}` in `navigation/settings-nav-collapsed` and `dropdown-menu.default.content`) and owner-attributed part geometry (such as `checkbox.*` `parts.label` with `label_gap: 8`, where `cap.rect` is only the `16×16` box) are invisible to `compare-layout`. Moreover, `reference_rects` does not filter or distinguish `tier` (`core` vs `later` / `reference-only`), so full-capture layout comparisons on pages with non-core specimens unconditionally fail with `missing > 0`.
4. **`D04` (Contract / Build-Mode Consistency — Unconditional API vs Conditional Validation, §§2, 3):** `.automation_id(...)`, `Ui::named(...)`, `UiDiagnostic::DuplicateAutomationId`, and UIA `UIA_AutomationIdPropertyId` projection are compiled unconditionally in default builds, but validation of empty, `>256`-byte, control-character, and duplicate automation IDs in `Runtime::view` (`src/runtime.rs:333`) is gated behind `#[cfg(any(test, feature = "devtools"))]`, omits `self.diagnostics.push(UiDiagnostic::DuplicateAutomationId)`, and allows `Ui::named` (`src/ui.rs:475`) to silently overwrite an inner builder's `.automation_id(...)`.
5. **`D05` (Interaction / Idle Completeness — `focus` Peer Event Drain & Missing `leave`/`unhover`, §§5, 6):** `dev_command("focus", ...)` (`src/platform/win32/devtools.rs:267-268`) calls `self.set_focus(Some(id))` then `self.turn()` without calling `self.service_peer_events()` before `self.turn()` (unlike `pointer()` and `key_msg()`), so any peer `HostEvent::SelChange`/`Change` drained at the end of `turn_body()` (`src/platform/win32/mod.rs:864`, after `rt.pump()`) remains unpumped in `self.rt.events`. Additionally, there is no `leave` / `unhover` command to clear hot state without hovering a second interactive control.

---

## Detailed Audit by Review Area

### 1. Exact Identity & Publication Separation

- **Base SHA:** `dc70f6644c0384170f3a6929eb2caaf33fc711a0` (verified parent of the `agent/rust-ui-devctl-v0.1` implementation chain `d6da076 -> 5125877 -> 752751c -> 6375a4e`).
- **Implementation SHA:** `6375a4e537f0ec396c92154738f7bfced4324c5c` (`20 files changed, 3828 insertions(+), 5 deletions(-)` against `dc70f664`).
- **Report / Publication SHA:** `7a1ad19a397d10149b6d50ebdf5c1c60d0c6bc67` (`36 files changed, 10231 insertions(+)` against `6375a4e5`).
- **Separation proof:** `git diff 6375a4e537f0ec396c92154738f7bfced4324c5c 7a1ad19a397d10149b6d50ebdf5c1c60d0c6bc67 -- src/ examples/ tests/ Cargo.toml Cargo.lock` is **empty**. `7a1ad19` adds only `.gitattributes`, `docs/DEVCTL_V0_1_IMPLEMENTATION_REPORT.md`, and `benchmark/results/rust-ui-devctl-v0.1/*`.
- **Frozen reference immutability:** `git diff dc70f6644c0384170f3a6929eb2caaf33fc711a0 7a1ad19a397d10149b6d50ebdf5c1c60d0c6bc67 -- reference/` is **empty**, and all 18 canonical screenshot PNG blobs under `reference/shadcn-gallery-v0.1/screenshots/{light,dark}/` plus `contract.json` and `tokens.json` are byte-identical to `2e14e417a69ef8967b8fb17120282e3de6449fae`.

### 2. Devtools Opt-In Boundary

- **Feature gating:** `src/lib.rs:9-10` gates `pub mod devtools` on `#[cfg(feature = "devtools")]`. `src/platform/win32/mod.rs:50-51, 434-437, 2564-2571, 2687-2688, 3015-3024` gates `mod devtools`, `Backend::devtools`, `Backend::paint_epoch`, `service_devtools()`, `complete_devtools_idle()`, and `Server::start(...)` on `#[cfg(feature = "devtools")]`.
- **Normal builds (`default = []`):** Have zero pipe listener, zero devctl worker thread, zero WGC/D3D11/WinRT capture code, zero `serde`/`serde_json`/`png` dependencies, and zero periodic polling or frame loops.
- **Feature-enabled builds without `launch`:** `Server::start` (`src/platform/win32/devctl_client.rs:560-562`) returns `Ok(None)` immediately when `RUST_UI_DEVCTL_SESSION` is absent from the environment, spawning no thread and opening no pipe.
- ** Semantic metadata in normal runtime:** `Node::automation_id: Option<Rc<str>>` (`src/node.rs:371`) and UIA `UIA_AutomationIdPropertyId` (`src/platform/win32/uia.rs:244, 449`) remain in default builds. This is lightweight (`Option<Rc<str>>` is 16 bytes per node) and architecturally justified for UIA, subject to finding **`D04`** below.

### 3. Semantic Automation Identity

- **Identity separation:** `automation_id` is stored as `Option<Rc<str>>` on `StagedNode` and `Node`, separate from generational `NodeId { slot, generation }` and reconciliation `ChildKey`.
- **Reconciliation survival:** In `Runtime::update_node` (`src/runtime.rs:699-703`), `automation_id` updates in place on the surviving `NodeId` and sets `DIRTY_SEMANTICS` when changed, refreshing `ChildState::automation_id` in `UiaRoot::rebuild` (`src/platform/win32/uia.rs:1428`) without replacing the UIA provider.
- **Centralized lookup in devtools:** `dev_target` (`src/platform/win32/devtools.rs:126-148`) and `Snapshot::index` (`src/devtools/mod.rs:150-182`) both resolve automation IDs against the retained `Runtime` arena / snapshot without label scraping or UIA self-crawling.
- **Defect `D04` (Build-mode divergence, missing diagnostic push, and `Ui::named` overwrite):**
  1. In `Runtime::view` (`src/runtime.rs:333-347`), the preflight validation rejecting empty (`""`), `>256`-byte, control-character, or duplicate `automation_id`s is gated on `#[cfg(any(test, feature = "devtools"))]`. Because `.automation_id(...)`, `Ui::named(...)`, `UiDiagnostic::DuplicateAutomationId`, and UIA projection are unconditional public contracts, a default production build (`cargo build`) silently accepts invalid and duplicate `automation_id`s and exposes them to Windows UIA, while `cargo test` and `--features devtools` reject the exact same `view()` transaction with `Err(UiError::InvalidUi(UiDiagnostic::DuplicateAutomationId))`.
  2. Unlike every other `UiDiagnostic` branch in `Runtime::view` (`src/runtime.rs:324, 329, 354, 370`), the `DuplicateAutomationId` branch at `src/runtime.rs:343` does not push to `self.diagnostics`.
  3. In `Ui::named` (`src/ui.rs:466-476`), `self.tx.nodes[index as usize].automation_id = Some(Rc::from(id))` unconditionally overwrites `automation_id` even if the single child inside `draw` already set `.automation_id(...)`, silently discarding the inner ID instead of flagging `InvalidComposition` or `DuplicateAutomationId`.

### 4. Protocol & Local Session Model

- **Wire framing (`src/devtools/mod.rs:78-103`, `src/platform/win32/devctl_client.rs:120-175`):** `rust-ui.devctl/0.1` uses a 4-byte little-endian length prefix followed by UTF-8 JSON, capped at `MAX_MESSAGE = 4 MiB` before payload allocation. `#[serde(deny_unknown_fields)]` is enforced on `Request`. Over-budget responses degrade cleanly to a `MESSAGE_TOO_LARGE` error envelope (`devctl_client.rs:153-158`).
- **Pipe security & local-only boundary (`devctl_client.rs:176-194, 210-240`):** Named pipe `\\.\pipe\rust-ui-devctl-<64-hex-session-id>` uses `FILE_FLAG_FIRST_PIPE_INSTANCE`, `max_instances = 1`, and `PIPE_REJECT_REMOTE_CLIENTS`. Session ID and session token are independent 256-bit values from `BCryptGenRandom`.
- **Overlapped I/O safety (`devctl_client.rs:43-70`):** `pending()` cancels timed-out or stopped overlapped operations via `CancelIoEx` and waits for completion via `GetOverlappedResult(..., true)` before the stack `OVERLAPPED` and buffer go out of scope.
- **No unsafe capabilities:** The protocol exposes only `handshake`, `tree`, `snapshot-layout`, `rect`, `wait-idle`, `hover`, `press`, `release`, `click`, `focus`, `key`, and `shutdown`. There is no shell execution, arbitrary file read, arbitrary PID attach, or remote socket. `launch` spawns the explicit executable via `CreateProcessW` with `bInheritHandles = false` and an atomic `create_new(true)` session receipt reservation; `cleanup` checks `PROCESS_SYNCHRONIZE` / `WaitForSingleObject(..., 0)` and removes the receipt only after process exit.

### 5. `wait-idle` Correctness

- **Implementation (`src/platform/win32/devtools.rs:383-458`, `mod.rs:2560-2573`, `window.rs:298-300`):**
  - `dev_idle_state()` checks all 11 runtime/backend work indicators: `mailbox == 0`, `!events`, `reentrant == 0`, `deferred_native == 0`, `native_notifications == 0` (`peer_ctx.sink`), `state_dirty == 0`, `!dirty_layout_or_paint` (across all arena slots), `!pending_paint` (`GetUpdateRect(hwnd, None, false)`), `!damage`, `!scheduled_deadline` (`rt.next_deadline()`), and `!pump_queued`.
  - `complete_devtools_idle()` runs after `guarded_turn()` (post-`drain_reentrant()`) and at the end of `wndproc::trampoline` (after every `WM_PAINT`, `WM_TIMER`, `WM_PUMP`, etc.). When `dev_idle_state()` reports `idle == true`, it executes `DwmFlush()` under `in_dispatch = true`, drains any reentrant arrivals produced during `DwmFlush()`, and re-evaluates `dev_idle_state()` (`still_idle`) before replying to the pending `wait-idle` request.
- **Screenshot consistency guard (`src/bin/rust-ui-devctl.rs:238-276`):** `screenshot` runs `wait-idle`, records pre-capture `handshake` (`paint_epoch`) and `snapshot-layout`, performs WGC capture, and verifies both post-capture `snapshot-layout` equality and unchanged `paint_epoch` (incremented on every `Backend::paint()` in `src/platform/win32/mod.rs:1478-1484`).
- **Edge case `D05a` (`focus` peer events at end of `turn_body`):** See §6 below — `dev_command("focus", ...)` calls `self.set_focus(Some(id))` then `self.turn()` without calling `self.service_peer_events()` prior to `self.turn()`, unlike `pointer()` (`mod.rs:1690`) and `key_msg()` (`mod.rs:1825`).

### 6. Semantic Interaction Fidelity

- **Route sharing (`src/platform/win32/devtools.rs:214-298`):**
  - `hover`, `press`, `release`, and `click` resolve the target via `dev_target` and `dev_point`, verifying `n.interactive()`, positive rect inside client physical bounds, and `self.hit_test(logical) == Some(id)`, then invoke the real `Backend::hover` and `Backend::pointer(Down / Up)` methods without OS-global `SendInput`.
  - Held `press` sets real `Backend::pressed = Some(id)` without firing `NodeEvent::Press`; `release` clears `pressed` and emits `NodeEvent::Press`.
  - `key` maps the 12-key v0.1 subset (`Enter`, `Space`, `Tab`, `Escape`, `Left`, `Up`, `Right`, `Down`, `Backspace`, `Delete`, `Home`, `End`) to `Backend::key_msg(WM_KEYDOWN, ...)` + `Backend::key_msg(WM_KEYUP, ...)`, exercising the real `msftedit` windowless RichEdit peer and `TextEdit` commit path.
- **Defect `D05` (Two interaction gaps relevant to Gallery testing):**
  1. **Missing `service_peer_events()` before `turn()` in `focus` (`devtools.rs:267-268`):** `self.set_focus(Some(id))` sends `WM_KILLFOCUS` / `WM_SETFOCUS` to windowless RichEdit peers via `deliver_native`, which queues `HostEvent`s on the peer. Because `dev_command("focus")` calls `self.turn()` immediately without calling `self.service_peer_events()` first, those `HostEvent`s are only drained at line 864 of `turn_body()` — *after* `self.rt.pump()` (line 730) and `self.paint()` (line 862) have already run. If a peer emits `HostEvent::SelChange` or `HostEvent::Change` during that late drain, `self.rt.events` becomes non-empty after `rt.pump()` with no `wake_pump()` call, leaving unpumped events that can stall `wait-idle`. Calling `self.service_peer_events()` before `self.turn()` inside `set_focus` (or in `dev_command("focus")` and `WM_SETFOCUS`/`WM_KILLFOCUS`) closes this gap.
  2. **No `leave` / `unhover` command:** `dev_point` rejects non-interactive targets (`probe.root`, `probe.surface`, `probe.label`) with `NOT_SUPPORTED`, and there is no `leave` or `unhover` command to invoke `Backend::leave()`. In `tests/devctl_windows.rs:131`, the test had to hover `probe.input` (`action(session, "hover", &["probe.input"])`) as a workaround to clear hover from `probe.button`. When testing Gallery components in isolation or verifying hover-exit transitions, an explicit `leave` / `unhover` command is needed.

### 7. Windows Graphics Capture (WGC) Authority

- **Sole authority (`src/platform/win32/devctl_capture.rs:1-274`):** Exclusively uses `Windows.Graphics.Capture` (`IGraphicsCaptureItemInterop::CreateForWindow`, `Direct3D11CaptureFramePool::CreateFreeThreaded`, `GraphicsCaptureSession`). There is no `PrintWindow`, `BitBlt`, or synthetic raster fallback anywhere in the codebase; unsupported or occluded/minimized windows fail closed with `CAPTURE_FAILED` (exit code 5).
- **Geometry & epoch validation:** Establishes `DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2` with an RAII restore guard (`DpiContext`), measures `(GetClientRect, client_to_screen(0,0), DwmGetWindowAttribute(DWMWA_EXTENDED_FRAME_BOUNDS))` before and after capture, rejects any geometry change during capture, verifies WGC `item.Size()` and `frame.ContentSize()` match the DWM extended frame bounds, maps the D3D11 staging texture with an RAII `Unmap` guard (`Mapped`) honoring `mapped.RowPitch`, converts `B8G8R8A8` to `RGBA8`, and crops to the exact physical client rect `(origin.x - bounds.left, origin.y - bounds.top, client.width, client.height)`.
- **WinRT / D3D11 lifecycle:** `CaptureRuntime` keeps `RoInitialize(RO_INIT_MULTITHREADED)` alive for the `rust-ui-devctl` CLI process lifetime (`src/bin/rust-ui-devctl.rs:327-331`), while `Capture::drop` removes the `FrameArrived` handler token and closes both `GraphicsCaptureSession` and `Direct3D11CaptureFramePool` on every capture.

### 8. DPI & Coordinate Model — **BLOCKER `D02`**

- **Native coordinate conversion (`src/geom.rs`, `src/platform/win32/space.rs`, `src/platform/win32/devtools.rs`):**
  - `ScaleFactor::from_dpi` maps `96 -> 1.00`, `120 -> 1.25`, `144 -> 1.50`, `192 -> 2.00`.
  - `Rect::physical(scale)` converts logical `Dp` rects to physical pixel rects by applying `ScaleFactor::to_physical` (`(logical * scale.0).round() as i32`, round-half-away-from-zero) independently to `left`, `top`, `right`, and `bottom`.
  - `dev_node` (`src/platform/win32/devtools.rs:67, 111-116`) computes `rect_px` using `r.physical(self.peer_ctx.scale.get())`, so native `rect_px` values in `Snapshot` are integer physical pixel rects derived from the `.round()` authority.
- **Defect `D02` — Divergent `floor`/`ceil` Rounding Policy in `Image::crop` (`src/devtools/mod.rs:331-352`) and Unrounded Reference Rects in `compare-element` (`src/bin/rust-ui-devctl.rs:180-195`):**
  - Instead of using `crate::geom::Rect::physical` (`round()`), `Image::crop` implements a second, conflicting rounding rule on `devtools::Rect`:
    ```rust
    let x = r.x.floor() - pad as f64;
    let y = r.y.floor() - pad as f64;
    let right = (r.x + r.width).ceil() + pad as f64;
    let bottom = (r.y + r.height).ceil() + pad as f64;
    ```
  - When `Image::crop` is called on a native `rect_px` (which `dev_node` already snapped to integers via `.round()`), `floor()` and `ceil()` are no-ops.
  - However, when `compare-element` crops the frozen reference PNG (`src/bin/rust-ui-devctl.rs:190-193`), it passes the raw fractional CSS-px `Rect` from `contract.json` directly into `Image::crop` without converting through `Rect::physical(ScaleFactor::ONE)`!
  - Out of **581** total element-capture records in `reference/shadcn-gallery-v0.1/contract.json`, **480** have fractional coordinates (`x`, `y`, `width`, or `height`), and **421 out of 581 (72.5%)** produce a **1 px or 2 px crop width/height error (and up to 1 px origin shift) between `Image::crop` (`floor`/`ceil`) and `Rect::physical(ScaleFactor::ONE)` (`round`) even at scale `1.00` (DPR 1)!**
  - Concrete examples verified against `reference/shadcn-gallery-v0.1/contract.json`:
    - `button.default` (`components/default`, `rect = { x: 273.0, y: 300.75, width: 69.984, height: 32.0 }`):
      - `Rect::physical(ScaleFactor::ONE)` (`round`): `left = 273`, `top = 301`, `right = 343`, `bottom = 333` -> **`70 × 32` px** (preserving the exact `32 Dp` button height).
      - `Image::crop` (`floor`/`ceil`): `left = 273`, `top = floor(300.75) = 300`, `right = ceil(342.984) = 343`, `bottom = ceil(332.75) = 333` -> **`70 × 33` px** (+1 px spurious height, including 1 row of background above the button).
    - `badge.outline` (`components/default`, `rect = { x: 692.938, y: 1728.0, width: 54.109, height: 20.0 }`):
      - `Rect::physical(ScaleFactor::ONE)` (`round`): `left = round(692.938) = 693`, `right = round(747.047) = 747` -> **`54 × 20` px**.
      - `Image::crop` (`floor`/`ceil`): `left = floor(692.938) = 692`, `right = ceil(747.047) = 748` -> **`56 × 20` px** (+2 px spurious width, pulling in almost a full pixel of background on both left and right edges).

### 9. CRITICAL: Frozen DPR1 Reference vs Native DPI 1.25 — **BLOCKER `D01`**

- **Mandatory Question:**
  > *At host scale 1.25, can `compare-element` currently produce a meaningful pixel-level visual comparison between the frozen DPR1 reference crop and native WGC crop?*
- **Explicit Answer: NO.**
- **Independent Live Proof at Host Scale 1.25:**
  Executing `compare-element` against `reference/shadcn-gallery-v0.1/contract.json` (`--capture components/default --id button.default --reference-image reference/shadcn-gallery-v0.1/screenshots/light/components.png`) and the native WGC capture on this host (`scale_factor = 1.25`) produces:
  ```json
  {
    "alignment": "independent semantic crops; no resize",
    "id": "button.default",
    "image": {
      "actual": [87, 45],
      "expected": [70, 33],
      "dimensions": [87, 45],
      "dimensions_match": false,
      "mismatched_pixels": 3915,
      "missing_pixels": 1605,
      "mismatch_percent": 100.0,
      "max_channel_delta": 255,
      "mean_absolute_channel_error": 81.13231162196679,
      "difference_bbox": { "x": 0.0, "y": 0.0, "width": 87.0, "height": 45.0 },
      "pass": false
    },
    "pass": false
  }
  ```
- **Why this is a Blocker for Native Gallery Handoff:**
  1. Even when a native `rust-ui` component has the **exact** logical `Dp` dimensions of the frozen HTML specimen (e.g., `69.984 × 32.0 Dp` for `button.default`), its native physical crop at host scale `1.25` is `87 × 40` px while the DPR1 reference crop is `70 × 32` px (or `70 × 33` px with `D02`).
  2. `compare_image` places the `1.0x` reference crop at `(0, 0)` of the `1.25x` union canvas without any scale transform. Every pixel `(x, y)` in the overlap compares reference logical coordinate `(x, y)` Dp against native logical coordinate `(x / 1.25, y / 1.25) = (0.8x, 0.8y)` Dp — comparing the wrong spatial locations of the component against each other.
  3. The right 20% and bottom 20% of the `1.25x` native crop (~36% of the union canvas area) have no corresponding pixel in the `1.0x` reference crop and are unconditionally marked as `missing_pixels` (rendered as a solid magenta L-border in `diff.png`).
  4. `dimensions_match` is permanently `false`, `difference_bbox` is permanently the entire native image, `mismatch_percent` is dominated by scale divergence, and `diff.png` is visually unusable for diagnosing background/border/radius/icon/text parity.
- **Required Architectural Resolution for `D01`:**
  - The frozen HTML reference (`reference/shadcn-gallery-v0.1/`) must remain strictly read-only.
  - `rust-ui-devctl` must provide a deterministic, mathematically grounded cross-scale visual comparison path between native captures (at `scale_factor`, e.g. `1.25`) and the frozen DPR1 (`1.0 px/Dp`) reference, while preserving raw native evidence.
  - Concretely, `compare-element` (and/or `screenshot`) must distinguish:
    - **`RAW_NATIVE_CAPTURE`:** the unscaled physical-pixel WGC crop (`native.png`, at `scale_factor`), preserved unmodified as ground-truth native rendering evidence.
    - **`CANONICAL_COMPARISON_RASTER`:** a deterministic `1.0 px/Dp` comparison raster aligned to the logical `Dp` coordinate grid (`Rect::physical(ScaleFactor::ONE)`), produced either by:
      - **(Option B — Explicit Deterministic Scale Normalization):** mapping the native physical crop (or sampling the native client capture over the element's logical `Dp` box using `scale_factor`) onto the target `1.0 px/Dp` logical pixel grid (`width_1x = round(r_dp.x + r_dp.width) - round(r_dp.x)`, `height_1x = round(r_dp.y + r_dp.height) - round(r_dp.y)`) via an explicitly declared, deterministic resampling filter (e.g., exact area-averaging / box-integration or bilinear/Mitchell filter in sRGB/linear space, recorded in `comparison.json` as `normalization: { mode, native_scale_factor, reference_dpr, resampler }`), saving `native.raw.png` (or `native.png`) alongside `native.normalized_1x.png` and `diff.png`; **or**
      - **(Option A / C — Canonical 1.0x Native Comparison Surface):** if and only if WGC + D2D can render/capture at effective `1.0` scale without introducing a second non-production renderer or violating the WGC-only capture rule (note: on a single-monitor Windows host configured at 125% DPI, `CreateForWindow` WGC captures at the monitor's DWM composition scale `1.25`, so Option B is always required whenever the host monitor scale is not `1.0`).
  - **Realistic visual-evidence expectations across DirectWrite (1.25 -> 1.0x) vs Chromium (DPR 1):**
    - Once normalized to the shared `1.0 px/Dp` logical grid (and with `D02` fixed so both sides use `ScaleFactor::to_physical` edge rounding), flat surface fills, borders, corner radii, padding/margins, control bounds, icon placement, and text bounding boxes align spatially to within `±1 px` anti-aliasing edges.
    - Glyph stems and subpixel/grayscale anti-aliasing fringes between DirectWrite and Chromium FreeType/Skia will still exhibit localized channel deltas along glyph/icon contours; therefore `comparison.json` should report both exact raw statistics on the normalized `1.0x` canvas and thresholded mismatch metrics (with explicit `--channel-tolerance`), without claiming bit-exact cross-engine glyph identity.

### 10. `snapshot-layout` Correctness

- **Schema & determinism (`src/devtools/mod.rs:124-182`, `src/platform/win32/devtools.rs:14-125`):**
  - Emits `schema: "rust-ui.devctl.snapshot-layout/0.1"`, `scale_factor`, `client_dp`, `client_px`, and `root`.
  - Traversal follows `n.children` (retained child order, not arena slot order).
  - Excludes unstable runtime data (no `NodeId`, raw pointers, timestamps, or session tokens).
  - Enforces bounds: `MAX_NODES = 8192`, `MAX_DEPTH = 128`, label prefix capped at `1024` chars, encoded frame capped at `4 MiB`.
  - Captures committed state (`visible`, `enabled`, `focused`, `focus_visible`, `hovered`, `pressed`, `rect_dp`, `rect_px`).

### 11. Frozen Contract Adapter — **BLOCKER `D03`**

- **Direct read of `contract/0.2` (`src/devtools/mod.rs:184-207`):**
  - Verifies `$schema == "rust-ui.shadcn-reference.contract/0.2"`.
  - Indexes `el["captures"].get(capture)` by exact `<page>/<state>` key; unknown capture keys return `TARGET_NOT_FOUND` with no silent fallback.
- **Defect `D03` — Ignored `parts`, Hidden Parts (`visible: false`), and Milestone Tiers:**
  1. **`cap["parts"]` is completely ignored:** `reference_rects` only reads `el["automation_id"]` and `cap["rect"]`, inserting `(rect, true)` with a hardcoded `true` boolean. In `contract/0.2`, sub-element parts (820 records across all captures) live in `cap["parts"]`, including:
     - All **14 hidden-part records** (`{"visible": false, "reason": "not-rendered"}`):
       - `navigation/settings-nav-collapsed`: `settings-nav.default` parts `sidebar.header`, `sidebar.nav.group[workspace].group-label`, `sidebar.nav.group[assistant].group-label`, `sidebar.nav.group[system].group-label`, and `settings-nav.{general,appearance,agents,integrations,voice,advanced}` part `label`.
       - `all/default` and `overlays/default`: `dropdown-menu.default.content` parts `group[appearance].item[word-wrap].indicator` and `group[panel-position].item[panel-right].indicator`.
     - **Owner-attributed parts:** e.g., `checkbox.default.checked` (and all 7 `checkbox.*` specimens), where `cap["rect"]` is only the `16 × 16` checkbox control box and the label (`width: 55.672, height: 14, label_gap: 8, owner_attributed: true`) is stored under `cap["parts"]["label"]`.
     - Because `reference_rects` never reads `cap["parts"]` and hardcodes `visible = true`, `compare-layout` cannot verify that collapsed `SettingsNav` hides its header/group/item labels, cannot verify part geometry (`<element_id>.<part_key>`), and treats any native automation IDs assigned to parts as `unexpected` failures in full-page comparisons.
  2. **No `tier` filtering (`core` vs `later` / `reference-only`):** `contract.json` contains `later` and `reference-only` elements on `all/default` (24 non-core), `navigation/default` (14), `navigation/settings-nav-collapsed` (14), `forms/default` (3), `overlays/default` (3), and `typography/default` (4). Because `reference_rects` loads all elements in a capture regardless of `el["tier"]`, running `compare-layout` on a page capture without `--id` during the `core` Gallery milestone will always fail with `missing > 0` for unimplemented `later`/`reference-only` specimens.

### 12. Layout Comparator

- **Behavior (`src/devtools/mod.rs:208-258`):**
  - Explicitly validates `tolerance.is_finite() && tolerance >= 0.0` and `mode ∈ {"absolute", "size-only"}`. Unsupported modes (such as `"relative"`) fail closed with `INVALID_ARGUMENT`.
  - Self-comparison returns `pass: true, matched: 5, missing: 0, unexpected: [], failed: 0`.
  - `+1 Dp` mutation on `probe.button.x` in `absolute` mode exits `6` with exact `delta.x = 1.0`.
  - Missing target ID returns `TARGET_NOT_FOUND` (exit `3`) when queried via `--id`, or increments `missing` and sets `pass: false` (exit `6`) in full-snapshot comparison.
  - Note: For comparing internal part layout of a component rendered at a different window `(x, y)` offset than the HTML document flow, `size-only` checks only `width`/`height` and ignores relative `(x - parent.x, y - parent.y)` placement; supporting element-relative part offsets (in conjunction with `D03`) or an explicit `relative`/part check will make layout verification of composite controls (`SettingsNav`, `Tabs`, `Checkbox` + label, `Field`) much stronger.

### 13. Image Comparator

- **Behavior (`src/devtools/mod.rs:373-434`):**
  - Validates non-zero image dimensions and `width * height <= MAX_PIXELS` (`32 Mi` pixels, sufficient for `all.png` at `1440 × 14062 = 20.25M` pixels).
  - Self-comparison produces `mismatched_pixels: 0, max_channel_delta: 0, mean_absolute_channel_error: 0.0, difference_bbox: null`.
  - Single-pixel `+1` mutation produces `mismatched_pixels: 1, max_channel_delta: 1, difference_bbox: {x: 0, y: 0, width: 1, height: 1}` and exits `6`.
  - Raw statistics (`max_channel_delta`, `mean_absolute_channel_error`) are computed before applying `tolerance`, so `--channel-tolerance` never masks raw channel deltas.
  - No perceptual-equivalence claims are made.

### 14. Element Comparator — **BLOCKERS `D01` & `D02`**

- **End-to-end flow (`src/bin/rust-ui-devctl.rs:161-215`):**
  - Resolves `id` in both reference and native snapshot, verifies native `n.visible`, crops both images, writes `reference.png`, `native.png`, `diff.png`, and `comparison.json` atomically via `atomic_write`, and combines layout + image pass states.
- **Verdict on `compare-element` readiness for Opus's native Gallery work:**
  - **NOT READY (`RUST_UI_DEVCTL_V0_1_REWORK_REQUIRED`).** Due to **`D01`** (no scale normalization or canonical `1.0x` comparison raster between DPR1 reference crops and `1.25x` native WGC crops) and **`D02`** (`Image::crop` using `floor`/`ceil` on fractional reference rects vs `ScaleFactor::to_physical` `.round()` on native rects), `compare-element` cannot produce a valid or actionable pixel comparison against `reference/shadcn-gallery-v0.1/` on this host.

### 15. Calibration Probe Independent Reproduction

- Independently executed on the authoritative Windows checkout (`W:\devin_folder\rust-ui` at `7a1ad19` / `6375a4e`, DPI 120, scale `1.25`):
  - `cargo build --locked --features devtools --all-targets`: **PASS**.
  - Live CLI sequence (`launch` -> `wait-idle` -> `tree` -> `rect probe.button` -> `hover` -> `wait-idle` -> `screenshot --id probe.button` -> `press` -> `wait-idle` -> `screenshot --id probe.button` -> `tree` -> `release` -> `click` -> `wait-idle` -> `snapshot-layout` -> `screenshot` -> `compare-layout` -> `compare-image` -> `compare-element` -> `shutdown`): **PASS**.
  - Verified `probe.button` rect at scale `1.25`: `rect_dp = (53, 83.62109375, 69.4609375, 36)`, `rect_px = (66, 105, 87, 45)`.
  - Verified held `press` state: `pressed: true` in `tree`, `Clicks: 0` before release, distinct `normal.png` / `hover.png` / `pressed.png` button crops (`87 × 45` px), `Clicks: 1` after `release`, `Clicks: 2` after `click`, `Clicks: 3` after keyboard `focus` + `Enter`, and committed RichEdit text `"Calibration tex"` after `focus probe.input` + `End` + `Backspace`.
  - Process exited cleanly on `shutdown` and removed its session receipt.

### 16. Resource Lifecycle

- Independently ran `cargo test --locked --features devtools --test devctl_windows -- --nocapture` (3 complete launch / interaction / 10-WGC-capture / 2s-idle / shutdown / cleanup cycles, 27.39s):
  - Cycle 0: target handles `205 -> 213` after focus/RichEdit warmup, then **`213, 213, 213, 213, 213, 213`** across 6 consecutive steady-state WGC captures (zero handle growth).
  - Cycle 1: target handles `199 -> 207`, then **`207` flat** across 6 consecutive WGC captures.
  - Cycle 2: target handles `199 -> 207`, then **`207` flat** across 6 consecutive WGC captures.
  - All child processes exited and all temporary directories and session receipts were cleaned up.

### 17. Idle-Performance Boundary

- Across all 3 cycles in our independent run (and in the worker's receipt), a 2-second idle window on an active instrumented session recorded:
  - Process CPU delta: **`0` ticks** (`GetProcessTimes` 100ns kernel+user ticks).
  - `native_timer_fires`: **`0 -> 0`**.
  - `requested_redraws`: **`71 -> 71`** (unchanged).
- Code inspection confirms the pipe thread blocks on `WaitForMultipleObjects(&[ov.hEvent, stop], false, INFINITE)` during `ConnectNamedPipe` (`devctl_client.rs:205`) and the UI thread blocks on `MsgWaitForMultipleObjectsEx(..., INFINITE, ...)` (`mod.rs:3038`).

### 18. Dependencies

- `Cargo.toml` additions are minimal and properly scoped as `optional = true` under `devtools`:
  - `serde = { version = "1", features = ["derive"], optional = true }`
  - `serde_json = { version = "1", optional = true }`
  - `png = { version = "0.17", default-features = false, optional = true }`
  - `windows = "0.62.2"` feature expansions (`Graphics_Capture`, `Graphics_DirectX_Direct3D11`, `Win32_Graphics_Direct3D11`, `Win32_Graphics_Direct3D`, `Win32_Graphics_Dxgi`, `Win32_Graphics_Dwm`, `Win32_System_WinRT*`, `Win32_System_Pipes`, `Win32_Storage_FileSystem`, `Win32_System_IO`, `Win32_Security_Cryptography`, `Foundation`) are all gated under the `devtools` feature.
- No async runtime (Tokio), HTTP server, CLI parser crate, logging framework, or heavyweight image crate was added.

### 19. Platform Ownership & Architecture

- All Win32/WinRT/D3D11/pipe/process code resides under `src/platform/win32/{devtools,devctl_client,devctl_capture}.rs`.
- No `HWND`, `WM_*`, `RECT`, or `WPARAM` types leaked into shared core modules (`src/lib.rs`, `src/node.rs`, `src/runtime.rs`, `src/ui.rs`, `src/devtools/mod.rs`).
- Core runtime does not know about Shadcn; only the offline comparator helper `reference_rects` in `src/devtools/mod.rs` reads `rust-ui.shadcn-reference.contract/0.2`.

### 20. Existing Foundation Regression

- Independently executed on Windows (`rustc 1.94.0`):
  - `cargo fmt --check`: **PASS**.
  - `cargo test --locked --lib` (parallel): **122/123 PASS**; the sole failure is the pre-existing intermittent `tests::native_probe_richedit_paints_text` parallel DPI-120 test isolation issue (`[probe] dc_dpi=120 dc_scale=1.25 ... scale 1: text must render ink inside the bounds, got 0`).
  - `cargo test --locked --lib -- --test-threads=1` (serial): **123/123 PASS** (1.95s).
  - `cargo test --locked --features devtools --lib -- --test-threads=1` (serial): **139/139 PASS** (1.92s).
  - `cargo test --locked --features devtools --test devctl_windows -- --nocapture`: **1/1 PASS** (27.39s).
- The `native_probe_richedit_paints_text` parallel DPI-120 isolation issue is pre-existing from `00dc29a` and unrelated to `devctl`.

### 21. Evidence Honesty

- All 33 files listed in `benchmark/results/rust-ui-devctl-v0.1/SHA256.json` were independently hashed and match their recorded SHA-256 digests (`0` mismatches).
- Visual inspection of `probe/client-0.png` (`962 × 853`), `probe/normal-0.png` (`87 × 45`), `probe/hover-0.png` (`87 × 45`), and `probe/pressed-0.png` (`87 × 45`) confirms genuine WGC captures with distinct button states and committed text `"Calibration tex"`.
- **Overclaim (`D01` / Gate 19):** The report headlines `RUST_UI_DEVCTL_V0_1_READY_FOR_NATIVE_GALLERY`, but `probe/element/comparison.json` only tested `compare-element` as a self-comparison (`snapshot-0.json` and `client-0.png` against themselves), and `reference-adapter.json` only tested `compare-layout` on a missing ID (`button.default` against `devctl_probe`). The worker never executed `compare-element` between a frozen DPR1 reference PNG and a native `1.25x` WGC capture, which immediately fails with 100% pixel mismatch and a 1,605-pixel missing-pixel border.

---

## 22. Universal Gates 1–20 Evaluation

| Gate | Verdict | Rationale |
|---|---|---|
| **Gate 1 — Requirements & Goal Completeness** | **FAIL** | `D01` (`compare-element` unusable across DPR1 reference vs native 1.25x capture) and `D03` (`reference_rects` ignores `parts` and hidden parts `visible: false`). |
| **Gate 2 — Assumptions, Ambiguity & Approval Boundaries** | **FAIL** | Worker assumed reporting a raw dimension mismatch at scale 1.25 satisfied the `compare-element` parity workflow without surfacing that it blocks visual comparison on the 1.25x host (`D01`). |
| **Gate 3 — Existing Behavior & Contract Preservation** | **PASS** | Frozen reference (`2e14e41` / `dc70f66`) is untouched; foundation tests pass (`123/123` serial). |
| **Gate 4 — Scope Discipline / Surgical Change** | **PASS** | Changes are confined to `devtools`, `automation_id`/UIA plumbing, calibration probe, docs, and evidence. |
| **Gate 5 — Minimal Correct Design / No Overengineering** | **PASS** | Focused std CLI, single overlapped pipe worker, no daemon/framework bloat. |
| **Gate 6 — Architecture, Ownership & Responsibility Boundaries** | **FAIL** | `D02`: `Image::crop` introduces a second rounding policy (`floor`/`ceil`) instead of routing through the single `geom::ScaleFactor::to_physical` (`round`) authority. |
| **Gate 7 — Duplication & Existing Capability Reuse** | **FAIL** | `D02`: `Image::crop` duplicates/contradicts `Rect::physical` edge snapping. |
| **Gate 8 — No Unrequested Fallback / Shim / Legacy Path** | **PASS** | WGC-only capture and backend-only input routing; no `PrintWindow`, `BitBlt`, or `SendInput` fallback. |
| **Gate 9 — Deterministic Semantics & Stable Contracts** | **FAIL** | `D01` (unusable cross-scale element comparison), `D02` (72.5% of reference rects differ by 1–2 px between `floor`/`ceil` and `round` even at scale 1.00), `D04` (`DuplicateAutomationId` enforced only in `test`/`devtools` builds), `D05` (`focus` peer events & missing `leave`). |
| **Gate 10 — Authority, Specs, Rules & Obsolescence** | **FAIL** | `D03`: `contract/0.2` authority includes `parts` (with `visible: false` hidden parts and owner-attributed checkbox labels) and `tier`, which `reference_rects` ignores. |
| **Gate 11 — Failure Safety, Atomicity, Idempotency & Security** | **PASS** | Atomic file writes, bounded frames, `CancelIoEx` + join before drop, checked wake admission, process ownership guards. |
| **Gate 12 — Boundedness / Resource & Agent-Visible Output Discipline** | **PASS** | Bounded 4 MiB messages, 32 Mi-pixel images, 8,192 nodes, 128 depth, 16-request queue, single JSON stdout document. |
| **Gate 13 — Dependency, External-System & Blast-Radius Necessity** | **PASS** | Only optional `serde`, `serde_json`, `png` (`default-features = false`), and scoped `windows` features under `devtools`. |
| **Gate 14 — Performance, Latency & Redundant Work** | **PASS** | Zero idle CPU ticks, zero timer fires, zero redraw drift over 2s idle windows; full 3-cycle workflow completes in 27.4s. |
| **Gate 15 — Persistence / Infrastructure Boundary** | **PASS** | Session receipt and artifact paths are owned by the CLI layer; runtime stays persistence-agnostic. |
| **Gate 16 — Verification Sufficiency & Test Quality** | **FAIL** | `compare-element` was only tested via native-to-native self-comparison (`D01`), missing both the 1.25x vs DPR1 failure (`D01`) and the fractional-rect `floor`/`ceil` vs `round` crop bug (`D02`). |
| **Gate 17 — Recovery Proportionality / Sunk-Cost Containment** | **PASS** | Clean modular implementation; rework is localized to `src/devtools/mod.rs`, `src/bin/rust-ui-devctl.rs`, `src/platform/win32/devtools.rs`, and `src/runtime.rs`. |
| **Gate 18 — Verification / Review Immutability & Artifact Identity** | **PASS** | Implementation (`6375a4e`) and evidence publication (`7a1ad19`) are cleanly separated; `SHA256.json` and `identity.json` match committed artifacts. |
| **Gate 19 — Completion Integrity / Final Artifact Honesty** | **FAIL** | Claiming `RUST_UI_DEVCTL_V0_1_READY_FOR_NATIVE_GALLERY` when `compare-element` cannot compare any native control at host scale 1.25 against the frozen DPR1 HTML reference is stronger than the evidence (`D01`). |
| **Gate 20 — Systemic Scope Completeness / Exhaustive Invariant Audit** | **FAIL** | `D02` violates the repository's systemic single-coordinate-authority invariant (`geom::ScaleFactor::to_physical` vs `Image::crop` `floor`/`ceil`). |

---

## Required Bounded Rework Before Native Gallery Handoff

1. **`D01` — Actionable DPR1 Reference ↔ Native Host DPI Visual Comparison in `compare-element`:**
   - Preserve the raw native physical crop (`RAW_NATIVE_CAPTURE`, at `scale_factor`) as an unscaled artifact.
   - When the reference image is at DPR 1 (`1.0 px/Dp`) and the native capture is at `scale_factor != 1.0` (e.g., `1.25`), deterministically produce a `CANONICAL_COMPARISON_RASTER` at `1.0 px/Dp` aligned to the element's logical `Dp` bounds (`Rect::physical(ScaleFactor::ONE)`), using a declared, deterministic resampler tied mathematically to `scale_factor`, `reference_dpr`, and logical `Dp` coordinates.
   - Emit separate files in `--out-dir` (e.g., `reference.png`, `native.raw.png` / `native.png`, `native.normalized.png` when scaled, and `diff.png` computed in the canonical `1.0 px/Dp` comparison space) and record the exact normalization metadata in `comparison.json`.
   - Prove `compare-element` with an automated test and committed evidence comparing a real `1.25x` native capture against a `1.0x` reference crop.
2. **`D02` — Unify Crop Rounding Policy on `crate::geom::ScaleFactor::to_physical` (`round`):**
   - Eliminate the divergent `floor()`/`ceil()` edge expansion in `Image::crop` (or ensure reference `Dp` rects from `contract.json` are converted to pixel rects through `crate::geom::Rect::physical(ScaleFactor::ONE)` — i.e., `.round()` per edge — before cropping, matching `dev_node`'s `r.physical(scale)`).
   - Add unit tests covering fractional `Dp` rects (such as `y = 300.75, height = 32.0` and `x = 692.938, width = 54.109, height = 20.0`) across scales `1.00, 1.25, 1.50, 2.00` to verify zero off-by-one/two crop dimension discrepancies.
3. **`D03` — Complete `contract/0.2` Adapter Coverage (`parts`, `visible: false`, and `tier`):**
   - Extend the frozen contract adapter (`reference_rects` / `compare_layout`) so that `cap["parts"]` (including the 14 `{"visible": false, "reason": "not-rendered"}` hidden parts in `navigation/settings-nav-collapsed` and `dropdown-menu.default.content`, and owner-attributed parts like `checkbox.*` `label`) are addressable and validated (e.g., as `<element_id>.<part_key>` when present/queried, and verifying `visible: false` parts are absent or hidden in the native snapshot).
   - Support tier-aware layout comparison (defaulting to or supporting `--tier core` for page-level `compare-layout`) so `later` and `reference-only` specimens do not cause false `missing` failures during the `core` Gallery milestone.
4. **`D04` — Unconditional `automation_id` Validation & Diagnostic Recording:**
   - Remove `#[cfg(any(test, feature = "devtools"))]` from the `DuplicateAutomationId` check in `Runtime::view` (`src/runtime.rs:333`) so default builds and `devtools`/test builds enforce identical `automation_id` validity and uniqueness invariants.
   - Push `UiDiagnostic::DuplicateAutomationId` to `self.diagnostics` before returning `Err(UiError::InvalidUi(UiDiagnostic::DuplicateAutomationId))`.
   - In `Ui::named` (`src/ui.rs:466-476`), reject (with `DuplicateAutomationId` or `InvalidComposition`) rather than silently overwriting a child node that already has an `automation_id`.
5. **`D05` — `focus` Peer Event Drain & `leave`/`unhover` Command:**
   - Ensure `set_focus` / `dev_command("focus", ...)` drains `self.service_peer_events()?` before `self.turn()` completes (or re-pumps when `service_peer_events()` enqueues runtime events) so native peer notifications triggered by `WM_SETFOCUS`/`WM_KILLFOCUS` never sit unpumped in `self.rt.events`.
   - Add a `leave` (or `unhover`) devctl command that routes `Backend::leave()` so agents can deterministically clear hover state without needing a second dummy interactive control.
