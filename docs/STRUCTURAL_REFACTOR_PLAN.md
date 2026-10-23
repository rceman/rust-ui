# Structural Refactor Plan — Pre-P11 Module Decomposition (`< 5000` Tokens/File)

## 1. Executive Intent & Authoritative Base

- **Repository:** `rceman/rust-ui`
- **Authoritative Base (`BASED_ON`):** `1e26aabe09b58f1cb7467921bcfd31ff9fbc82fb` (also compatible with subsequent `main` commits `4fb3f8e..3344c0c`)
- **Planning Branch:** `review/structural-refactor-plan-argon`
- **Target Milestone Preceded by This Refactor:** `SHADCN_AUTOMATED_VISUAL_PARITY_V1` (P11)

### Purpose
Decompose the **19 authored code files** currently exceeding the `5,000`-token (`20,000`-byte) per-file budget into cohesive, well-scoped Rust submodules (**111 target files**, every file targeting `< 4,500` tokens and strictly `< 5,000` tokens) with **zero behavioral, visual, public-API, or test-semantic change**.

### Core Design Principle: Parent-Declared Structs, Child `impl` Blocks
In Rust, child submodules (`mod child;`) have automatic visibility into private fields of types declared in their parent `mod.rs`, whereas sibling submodules (`backend.rs` vs `turn.rs`) cannot access each other's private fields without widening field visibility to `pub(super)` or `pub(crate)`.
Therefore, for every stateful multi-file subsystem (`win32`, `ui`, `runtime`, `render`, `text`, `uia`, `node`, `tasks`, `devtools`), **the primary struct/enum definitions remain in `mod.rs`** and coherent method/function groups move into child submodules. This avoids unnecessary visibility widening and preserves encapsulation (`G5`, `G6`, `G13`).

---

## 2. Token-Budget Policy & Exact `tools/check-file-tokens.sh` Rule

- **Hard Ceiling (authored code/config):** `< 5,000` tokens/file (`< 20,000` bytes under `tokens = bytes / 4`).
- **Planning Target:** `< 4,500` tokens/file (`< 18,000` bytes) to leave headroom for P11 additions.
- **Documentation Exemption (`docs/**`):** Exempt. The 11 `docs/*.md` files over `5,000` tokens (`STYLE_CUSTOMIZATION_MODEL.md`, `PLATFORM_FOUNDATION_*_REVIEW.md`, `API_EXAMPLES.md`, `SHADCN_REFERENCE_V0_1_FREEZE_REVIEW.md`, `QUALITY_GATES.md`, `SPIKE_IMPLEMENTATION_REVIEW.md`, `DEVCTL.md`) are single-purpose specifications or immutable historical review records referenced by SHA.

### Exact Script Change for Lead (Phase 0)
In `tools/check-file-tokens.sh`, add `! -path '*/docs/*'` to the `find` exclusions:

```bash
    ! -path '*/.git/*' \
    ! -path '*/target/*' \
    ! -path '*/target-*/*' \
    ! -path '*/.devctl/*' \
    ! -path '*/reference/*' \
    ! -path '*/benchmark/*' \
    ! -path '*/assets/*' \
    ! -path '*/docs/*' \
    ! -path '*/node_modules/*' \
```

---

## 3. Complete Inventory of Over-Budget Authored Code Files (19 Files)

Measured at `1e26aabe09b58f1cb7467921bcfd31ff9fbc82fb` (with `main` `3344c0c` delta noted for `win32/mod.rs` and `win32/window.rs`):

| # | Path | Tokens (`1e26aab` / `3344c0c`) | Bytes | Lines | Primary Responsibilities | Coupling / Risk | Target Module Count | Max Target File Tokens |
|---|---|---|---|---|---|---|---|---|
| 1 | `src/tests.rs` | `85,842` | `343,368` | `9,958` | Unit tests, fake peers, executor, Windows native probes, contract tests, Slice 1–3 recipe tests | Low (test-only), but naive 11-file split leaves 22k/18k files | **23 files** (`src/tests/*.rs`) | `~4,250` |
| 2 | `src/platform/win32/mod.rs` | `43,115` (`43,862`) | `172,460` | `4,145` (`4,208`) | `Backend`, `PeerCtx`, `PeerHandle`, `turn`, timers, paint, pointer/key/IME routing, UIA bridge, `run` loop | **Highest** (P9/P10 resize/move/DPI + reentrancy + DWM reveal) | **11 files** (`src/platform/win32/*.rs`) | `~4,370` |
| 3 | `src/ui.rs` | `36,411` | `145,647` | `4,094` | `CustomRender`/`Canvas`, container props, `Tx`/`UiFrame`/`Ui`, leaf builders, forms/nav/menus/overlays | Medium (public builder API + `Tx` staging internals) | **10 files** (`src/ui/*.rs`) | `~4,375` |
| 4 | `src/platform/win32/render.rs` | `26,493` | `105,972` | `2,639` | DirectWrite format/layout cache, `Renderer`, `draw_frame`, tooltip HWND, `D2dCanvas`, `ShadowCache`, `paint_box`, icons | High (`draw_frame` clip/layer guards + `ShadowCache` bounds) | **8 files** (`src/platform/win32/render/*.rs`) | `~4,350` |
| 5 | `examples/gallery.rs` | `24,065` | `96,262` | `2,058` | Canonical Shadcn Native Gallery state, shell header, Components/Forms/Nav/Overlays/NativeText/Typography pages | Medium (`src/bin/rust-ui-gallery.rs` `include!` dependency) | **7 files** (`examples/gallery/*.rs`) | `~4,280` |
| 6 | `src/style.rs` | `22,587` | `90,348` | `2,657` | `Color`, `Insets`/`CornerRadii`/`Border`/`Shadow`, `BoxStyle`, `TextStyle`, patches, `ActionStyle`, palette & component recipes | Medium (heavily touched in P11) | **6 files** (`src/style/*.rs`) | `~4,100` |
| 7 | `src/devtools/mod.rs` | `20,792` | `83,170` | `2,203` | Devctl wire protocol, `Snapshot`, `contract/0.2` adapter, `Image` + `area-box/1` + `compare_element`, embedded unit tests | Medium (53% is `#[cfg(test)] mod tests`) | **6 files** (`src/devtools/*.rs`) | `~4,200` |
| 8 | `src/platform/win32/text.rs` | `19,029` | `76,116` | `1,927` | `Msftedit` loader, `HostShared`/`TimerPool`, `HostBox` (`ITextHost2`), `WindowlessPeer`, `TxDrawD2D`, TOM selection, `TextPeer` | High (COM lifetime + P11 RichEdit typography/selection target) | **5 files** (`src/platform/win32/text/*.rs`) | `~4,200` |
| 9 | `src/platform/win32/uia.rs` | `16,862` | `67,449` | `1,734` | UIA `Snapshot`, `WindowlessSite`, `PaintFragment`, `EditorFragment`, `Fenced*` COM wrappers, `RootFragment`, `UiaRoot` | High (COM refcounting + generation fencing) | **5 files** (`src/platform/win32/uia/*.rs`) | `~3,910` |
| 10 | `src/runtime.rs` | `14,351` | `57,406` | `1,497` | `UpdateCtx`, `Runtime`, `pump`, `review`/reconcile (`create_node`/`update_node`/`remove_node`), event/conflict routing, peer entrypoints | High (reconcile + generation fencing) | **4 files** (`src/runtime/*.rs`) | `~4,160` |
| 11 | `examples/native_probe.rs` | `11,723` | `46,895` | `1,087` | External Win32/UIA/IME probe CLI for foundation verification | Low-Medium (Cargo `examples/` autodiscovery rule) | **3 files** (`examples/native_probe/*.rs`) | `~4,130` |
| 12 | `src/node.rs` | `11,099` | `44,396` | `1,201` | `KIND_*`, `PopupRole`, `SemanticRole`, `TextPeer`, `TextPeerSync`, `ContainerProps`, `NodeData`, `Node`, `dirty_diff` | Medium (`dirty_diff` drives layout/paint invalidation) | **3 files** (`src/node/*.rs`) | `~4,250` |
| 13 | `src/platform/win32/devtools.rs` | `10,649` | `42,597` | `1,021` | `dev_snapshot`, `dev_node`, `collect_parts`, `dev_command`, `service_devtools`, `dev_idle_state`, `toggle_part_rects` | Medium (used by all devctl E2E runs) | **3 files** (`src/platform/win32/devtools/*.rs`) | `~4,250` |
| 14 | `src/tasks.rs` | `9,942` | `39,769` | `1,093` | `Executor`, `WakerSet`, `CancelToken`, `Charge`, `Mailbox`, `TaskSender`, `UiProxy`, `TaskRegistry`, `CancelWatch` | Low-Medium (self-contained concurrency primitives) | **3 files** (`src/tasks/*.rs`) | `~3,950` |
| 15 | `src/platform/win32/window.rs` | `9,613` (`9,868`) | `38,453` | `908` (`928`) | `QueuedMsg`, `drain_queue`, `wndproc::trampoline`, `wndproc::reentrant`, `wndproc::dispatch`, `create`, `secondary_monitor_origin` | **High** (P9/P10 modal loop + reentrant queue) | **3 files** (`src/platform/win32/window/*.rs`) | `~4,100` |
| 16 | `src/platform/win32/layout.rs` | `9,350` | `37,402` | `943` | `natural` leaf/container measure, `LayoutCache::run`, `LayoutCache::walk`, `cross`/`cross_v`/`client_dip` | Medium-High (DIP layout + popup/scroll geometry) | **3 files** (`src/platform/win32/layout/*.rs`) | `~3,920` |
| 17 | `src/platform/win32/devctl_client.rs` | `7,273` | `29,092` | `834` | Out-of-process CLI `Session`/`Launched` client + in-process named-pipe `Server` + unit tests | Low-Medium (clean split at L511 between client and server) | **2 files** (`src/platform/win32/devctl_client/*.rs`) | `~3,780` |
| 18 | `src/layout.rs` | `7,008` | `28,033` | `755` | `waterfill`, `justify_offset`, `editor_chrome`, `node_insets`, `button_*`, `tab_*`, `popup_rect`, `toggle_*` | Low (pure stateless geometry functions) | **2 files** (`src/layout/*.rs`) | `~3,510` |
| 19 | `tests/devctl_windows.rs` | `6,073` | `24,292` | `624` | Windows devctl + WGC + Gallery E2E integration tests | Low-Medium (Cargo `tests/` autodiscovery rule) | **2 files** (`tests/devctl_windows.rs` + `tests/devctl_support/mod.rs`) | `~4,150` |

### Verification of Preliminary Findings (Section 5)
1. **Confirmed all 19 code files** from the preliminary scan. No additional non-doc files exceed `5,000` tokens (the next largest code file is `src/theme.rs` at `~3,400` tokens).
2. **Corrected 4 flaws in the preliminary proposal:**
   - **`src/tests.rs`:** The preliminary 10-file split was rejected because `native probes (Windows-only)` (`L3751..6201`, `~22,000` tok) and `mod native_contract_tests` (`L6202..8132`, `~18,000` tok) would still exceed the budget by `3.5x–4.5x`. Replaced with a verified **23-file split** where every file is `2,050–4,250` tokens.
   - **`src/platform/win32/mod.rs`:** The preliminary table omitted `L2062..3699` (`~17,000` tokens of input routing, focus/popup management, UIA bridge, DPI transition, and reentrant queue draining). Replaced with an **11-file split** keeping `struct Backend` in `win32/mod.rs`.
   - **`src/platform/win32/render.rs`:** The preliminary proposal suggested `render/richedit.rs` (`TxDrawD2D`), but `TxDrawD2D` actually lives in `src/platform/win32/text.rs` (`WindowlessPeer::draw`), while `render.rs` only calls `peer.borrow().draw(&target, ...)`. Adding `render/richedit.rs` would blur the renderer/text boundary. Instead, `draw_frame` is split cleanly into `render/frame.rs` (text/button/toggle/editor/custom arms) and `render/containers.rs` (container/action/popup arms + `paint_box` + rings).
   - **`examples/gallery.rs` (`include!` in `src/bin/rust-ui-gallery.rs`):** Simply adding `mod pages;` inside an `include!`-ed file breaks `rustc` path resolution when compiling `src/bin/rust-ui-gallery.rs`. Section 18 specifies the exact `[package]` / `[[bin]]` / `[[example]]` or `#[path = ...]` topology that preserves both `cargo run --example gallery` and `CARGO_BIN_EXE_rust-ui-gallery`.

---

## 4. Detailed Per-File Decomposition Plans (Sections 6–20)

### Section 6 — `src/tests.rs` (`85,842` tok, `9,958` lines → **23 files** under `src/tests/`)

`src/lib.rs` retains `#[cfg(test)] mod tests;`, which resolves to `src/tests/mod.rs`.
- **Shared test harness (`src/tests/mod.rs`):** Holds `FakePeer`, `PeerLog`, `PeerRec`, `PeerOp`, `Exec`, `TaskWake`, `TestExecutor`, `Msg`, and `Rig<S>` (`L1..375`, `~2,650` tok) with `pub(super)` visibility so all 22 sibling test modules import them via `use super::*;`.
- **Preserving test names & test identity (Section 25):**
  - Moving `#[test] fn foo()` into `src/tests/bar.rs` via `mod bar;` would change its Cargo test path from `tests::foo` to `tests::bar::foo` **unless** the child files are either included or re-exported, OR we note how Cargo lists test paths.
  - **Crucial insight for 100% identical `cargo test -- --list` output:** If `src/tests/mod.rs` includes each non-`mod` slice file via `include!("keys_reconcile.rs");` (or if `mod native_contract_tests` at `L6202..8132` is `mod native_contract_tests { include!("native_contracts_a.rs"); ... }`), then:
    1. Every single test name in `cargo test --locked --lib -- --list` stays **100% character-for-character identical** (`tests::<name>` and `tests::native_contract_tests::<name>`)!
    2. Zero `pub(super)` helper adjustments or duplicate `use` blocks are needed across the 22 files!
    3. Every file in `src/tests/*.rs` is an ordinary `.rs` file under `4,300` tokens checked by `tools/check-file-tokens.sh`!
  - Let's compare `include!` vs `mod` for `src/tests/`: using `include!("...")` inside `src/tests/mod.rs` (and inside `mod native_contract_tests` in `src/tests/mod.rs`) gives **exact `before == after` test-name set equality** on both Linux (`123` tests) and Windows (`240+` tests) without even a module-prefix diff. (Alternatively, standard `mod <name>;` with `use super::*;` is also valid if module-path prefixes `tests::<submod>::<fn>` are normalized by stripping the submodule segment when comparing `--list`, but `include!` in `src/tests/mod.rs` preserves verbatim test paths effortlessly.)

| Target File | Source Lines (`src/tests.rs`) | Est. Tokens | Test Groups / Contents | `cfg` Guards |
|---|---|---|---|---|
| `src/tests/mod.rs` | `L1..375` | `~2,850` | Imports, `PeerOp`, `PeerRec`, `PeerLog`, `FakePeer`, `Exec`, `TaskWake`, `TestExecutor`, `Msg`, `Rig<S>`, plus module/include wiring | `#[cfg(test)]` (from `lib.rs`) |
| `src/tests/keys_reconcile.rs` | `L376..773` | `~2,680` | Keys & diagnostics (`L376..511`) + Reconcile & slot generation (`L512..773`) | Portable |
| `src/tests/text_protocol_a.rs` | `L774..1195` | `~3,880` | Text protocol part 1: mount, programmatic `set_text`, native commit, conflict tombstones | Portable |
| `src/tests/text_protocol_b.rs` | `L1196..1609` | `~3,780` | Text protocol part 2: IME composition deferral, lease cell, selection, read-only/disabled | Portable |
| `src/tests/tasks_sched.rs` | `L1610..2070` | `~3,670` | Async tasks, `Mailbox`, `CancelToken`, scope lifting (`L1610..1956`) + Scheduler (`L1957..2070`) | Portable |
| `src/tests/sched_rework.rs` | `L2071..2529` | `~3,600` | Scheduler transitions/tooltips + rework round-2 regressions part 1 | Portable |
| `src/tests/regressions_a.rs` | `L2530..2953` | `~4,060` | Rework regressions part 2 (`DropPayload`, queue bounds, focus/capture) | Portable |
| `src/tests/regressions_b.rs` | `L2954..3410` | `~3,700` | Rework regressions part 3 (style patches, forced colors, damage union) | Portable |
| `src/tests/regressions_c.rs` | `L3411..3750` | `~2,820` | Rework regressions part 4 (layout waterfill, action semantics) | Portable |
| `src/tests/win32_probes_a.rs` | `L3751..4135` | `~3,650` | Windows native probes part 1: RichEdit D2D draw, scale matrix (`1.0..2.0`), live DPI relatch | `#[cfg(windows)]` |
| `src/tests/win32_probes_b.rs` | `L4136..4555` | `~3,950` | Windows native probes part 2: `F09/N04` shadow offset/spread & bounds, `F08` borders, `F07/N03` UIA teardown | `#[cfg(windows)]` |
| `src/tests/win32_probes_c.rs` | `L4556..4980` | `~3,890` | Windows native probes part 3: `F04` composition-deferred geometry, relatch, DIP↔px round-trip | `#[cfg(windows)]` |
| `src/tests/win32_probes_d.rs` | `L4981..5509` | `~4,120` | Windows native probes part 4: WndProc reentrancy, `drain_queue`, modal sizing/move loop | `#[cfg(windows)]` |
| `src/tests/win32_probes_e.rs` | `L5510..5938` | `~3,820` | Windows native probes part 5: `NodeDataLike`, peer events, IME owner pinning | `#[cfg(windows)]` |
| `src/tests/win32_probes_f.rs` | `L5939..6201` | `~2,350` | Windows native probes part 6: pre-contract native regressions | `#[cfg(windows)]` |
| `src/tests/native_contracts_a.rs` | `L6202..6690` | `~4,220` | `native_contract_tests` part 1: coordinate spaces, `PeerOrigin`, hit-testing | `#[cfg(windows)]` |
| `src/tests/native_contracts_b.rs` | `L6691..7180` | `~4,250` | `native_contract_tests` part 2: focus/capture/IME/UIA provider contracts | `#[cfg(windows)]` |
| `src/tests/native_contracts_c.rs` | `L7181..7660` | `~4,180` | `native_contract_tests` part 3: shadow cache, damage rect, wake/mailbox contracts | `#[cfg(windows)]` |
| `src/tests/native_contracts_d.rs` | `L7661..8132` | `~4,150` | `native_contract_tests` part 4: devctl focus/leave/peer-event order contracts | `#[cfg(windows)]` |
| `src/tests/slice1_recipes.rs` | `L8133..8671` | `~4,180` | Slice-1 visual/system recipes: Geist font, Button/IconButton/Card/Typography recipes | Portable + `#[cfg(windows)]` |
| `src/tests/slice2_forms_nav.rs` | `L8672..9120` | `~3,850` | `S03` Row/Column justify, `S02` devtools parts, Slice-2 Checkbox/Radio/Switch/Select/Tabs/Field | Portable + `#[cfg(windows)]` |
| `src/tests/slice3_overlays_a.rs` | `L9121..9528` | `~3,580` | Pre-Slice-3 presentation state, ring geometry, unfocused selection highlight | Portable + `#[cfg(windows)]` |
| `src/tests/slice3_overlays_b.rs` | `L9529..9958` | `~3,650` | Slice-3 Popover, Dialog, DropdownMenu, SettingsNav, keyboard nav cycling | Portable + `#[cfg(windows)]` |

---

### Section 7 & Section 8 — `src/platform/win32/mod.rs` (`43,115` / `43,862` tok → **11 files** under `src/platform/win32/`)

#### P9/P10 Sacred Behavior Ownership Map (Section 8)
To prevent fragmenting the P9 (live held resize) and P10 (held cross-monitor drag / DPI transition) state machines:
1. **`struct Backend<S, M, U, V>`** (`laid_client`, `size_pending`, `sizing_session`, `move_pending`, `in_dispatch`, `in_turn`, `reentrant_queue`, `dark_caption`, `committed_ink`) stays declared in **`src/platform/win32/mod.rs`**.
2. **`src/platform/win32/paint_timers.rs`** owns the entire **P9/P10 layout/paint convergence & DWM chrome seam**: `sync_dark_caption`, `resize_trace`, `relayout`, `relayout_inner` (which updates `laid_client`), `accumulate_ink_damage`, `ink_map`, `paint`, `paint_inner`, `paint_inner_impl`, `dpi_changed`, and `os_change`.
3. **`src/platform/win32/window/wndproc.rs`** (Section 9) owns the **WndProc modal-loop & reentrant routing seam**: `trampoline`, `reentrant` (live `WM_SIZE`, live in-session `WM_PAINT`, coalesced `WM_MOVE`, live `WM_DPICHANGED`, `WM_ENTERSIZEMOVE`/`WM_EXITSIZEMOVE`), and `dispatch`.

| Target File | Source Lines (`win32/mod.rs`) | Est. Tokens | Methods / Types Owned | Visibility Changes | Risk |
|---|---|---|---|---|---|
| `src/platform/win32/mod.rs` | `L1..239`, `L478..665` | `~3,850` | Submodule declarations, `WM_*` constants, `NativeSinkItem`, `PerfCounters`, `ResizeSample`, `ResizeCounters`, `PopupBook`, `InteractState`, `SubmitDecision`, `struct Backend<S,M,U,V>` definition | None (parent module; child modules access private `Backend` fields directly) | Low |
| `src/platform/win32/peer.rs` | `L240..477`, `L505..519` | `~2,650` | `PeerCtx`, `PeerHandle`, `impl TextPeer for PeerHandle`, `palette`, `modifiers_now` | `pub(super)` on `palette`, `modifiers_now` | Low |
| `src/platform/win32/construct.rs` | `L666..883`, `L3368..3705` | `~4,150` | `Backend::for_test`, `DeferredNative`, `MailboxEvent`, `WakeOutcome`, `WaitClass`, `init_wake_event`, `classify_wait`, `drain_reentrant`, `mailbox_wake`, `guarded_turn`, `mark_fatal`, `take_fatal`, `shutdown` | None (`impl Backend` in child module) | Medium (reentrant drain + wake) |
| `src/platform/win32/turn.rs` | `L926..1289`, `L3640..3699` | `~4,120` | `wake_pump`, `turn`, `turn_body` (sink drain, `rt.pump`, palette refresh, popup lifecycle bookkeeping, dirty classification), `apply_chrome`, `sanitize_focus` | None (`impl Backend` in child module) | High (commit/popup order) |
| `src/platform/win32/peer_events.rs` | `L1290..1757`, `L3500..3599` | `~4,280` | `deliver_native`, `enqueue_native`, `focused_target`, `send_focused`, `scroll_viewport_at`, `wheel`, `ime_target`, `send_ime`, `service_peer_events`, `invalidate_slot`, `native_commit`, `native_selection`, `peer_origin`, `apply_caret`, `ime_start`, `ime_end`, `reconcile_commit` | None (`impl Backend` in child module) | High (RichEdit/IME event ordering) |
| `src/platform/win32/paint_timers.rs` | `L898..925`, `L1758..2061`, `L2963..2972`, `L3338..3367` | `~3,850` | `sync_dark_caption` (DWM caption/border), `native_timer_fire`, `rearm_deadline`, `deadline_fire`, `resize_trace`, `relayout`, `relayout_inner`, `accumulate_ink_damage`, `ink_map`, `paint`, `paint_inner`, `paint_inner_impl`, `show_tooltip`, `hide_tooltip`, `os_change`, `dpi_changed` | None (`impl Backend` in child module) | **High** (P9/P10 convergence + DWM sync) |
| `src/platform/win32/input_pointer.rs` | `L2062..2413`, `L3260..3291` | `~3,680` | `inside_action`, `inside_subtree`, `modal_popup`, `topmost_popup`, `hit_test`, `hit_test_inner`, `pointer`, `push_input`, `editor_chrome_of`, `peer_for`, `update_cursor`, `peer_for_opt`, `pt_px`, `pt` | None (`impl Backend` in child module) | Medium-High |
| `src/platform/win32/input_key.rs` | `L2414..2731`, `L3292..3337` | `~3,620` | `key_msg`, `submit_decision`, `check_submit`, `focus_step`, `first_focusable`, `set_focus`, `focus_and_settle`, `focus_peer`, `char_msg` | None (`impl Backend` in child module) | Medium-High (D05 focus order) |
| `src/platform/win32/interact.rs` | `L2732..2962` | `~2,350` | `resolved_hot`, `resolved_pressed`, `resolved_focus_visible`, `interact`, `resolved_at`, `mark_state_dirty`, `hover`, `leave` | None (`impl Backend` in child module) | Low-Medium |
| `src/platform/win32/uia_bridge.rs` | `L2973..3259`, `L4029..4051` | `~3,300` | `uia_provider`, `uia_refresh`, `uia_refresh_impl`, `uia_action`, `semantic_uia_ct` | None (`impl Backend` in child module) | Medium |
| `src/platform/win32/run.rs` | `L3706..4028`, `L4052..4145` | `~3,450` | `OleGuard`, `run` (Backend construction, initial `guarded_turn`, atomic DWM cloak/reveal, `MsgWaitForMultipleObjectsEx` loop), `bail`, `os_appearance`, `sys_color`, `effective_reduced` | None | High (launch reveal + message loop) |

---

### Section 9 — `src/platform/win32/window.rs` (`9,613` / `9,868` tok → **3 files** under `src/platform/win32/window/`)

| Target File | Source Lines (`window.rs`) | Est. Tokens | Responsibilities | Visibility Changes |
|---|---|---|---|---|
| `src/platform/win32/window/mod.rs` | `L1..234`, `L828..908` | `~2,650` | Timer ID constants/helpers, `QueuedMsg`, `DrainOutcome`, `next_drain_item`, `drain_queue`, `focus_targeted`, `deferrable_arrival`, `create`, `secondary_monitor_origin` | Re-export `pub(crate) mod wndproc;` |
| `src/platform/win32/window/wndproc.rs` | `L235..483` | `~3,150` | `dispatch_owned`, `trampoline`, `reentrant` (all P9/P10 modal sizing/move/DPI live re-entry arms kept together in one file) | `pub(super) fn dispatch` called from `wndproc_dispatch.rs` |
| `src/platform/win32/window/wndproc_dispatch.rs` | `L484..826` | `~3,850` | `dispatch` (`WM_PUMP`, `WM_PAINT`, pointer/key/IME, `WM_ENTERSIZEMOVE`/`WM_EXITSIZEMOVE`, `WM_SIZE`, `WM_ERASEBKGND`, `WM_MOVE`, `WM_DPICHANGED`, `WM_SETTINGCHANGE`, `WM_TIMER`, `WM_GETOBJECT`, `WM_UIA_*`, `WM_NCDESTROY`) | `pub(super) fn dispatch` |

---

### Section 10 — `src/platform/win32/render.rs` (`26,493` tok → **8 files** under `src/platform/win32/render/`)

`struct Renderer` (and `Tip`, `ClipGuard`, `LayerGuard`) stays defined in `src/platform/win32/render/mod.rs` so all child modules can access `Renderer` fields (`factory`, `target`, `hwnd`, `dpi`, `target_gen`, `shadow_cache`, `tip`, `tip_fade_start`) without making them `pub`.

| Target File | Source Lines (`render.rs`) | Est. Tokens | Responsibilities |
|---|---|---|---|
| `src/platform/win32/render/mod.rs` | `L1..40`, `L357..524`, `L1394..1412` | `~2,250` | `Renderer` struct & lifecycle (`new`, `set_dpi`, `target_px`, `resize`, `ensure_target`, `draw`), `Tip`, `ClipGuard`, `LayerGuard` |
| `src/platform/win32/render/dwrite.rs` | `L41..356` | `~3,050` | `Dwrite`, `FormatCache`, `TextLayoutCache`, `fmt_for`, `layout_for`, `measure_text`, `measure_fn` |
| `src/platform/win32/render/frame_leaves.rs` | `L525..990` | `~4,350` | `Renderer::draw_frame` entry + `Label`, `Icon`, `Button`, and `Toggle` (`Checkbox`/`Switch`/`RadioItem`) paint arms |
| `src/platform/win32/render/frame_widgets.rs` | `L991..1258` | `~4,350` | `Renderer::draw_frame` `Editor` (`TxDrawD2D` scratch blit + placeholder), `Custom`, `Container`, `Action`, `Popup`, and `EndDraw` |
| `src/platform/win32/render/tooltip_canvas.rs` | `L1259..1393`, `L1413..1521` | `~2,450` | `show_tooltip`, `create_tip`, `hide_tooltip`, `tip_paint`, `D2dCanvas` (`Canvas` impl) |
| `src/platform/win32/render/shadow.rs` | `L1676..2030`, `L2403..2415` | `~2,950` | `ShadowKey`, `ShadowCache`, `draw_shadow` (SDF + `spread` + 3-pass box blur + interior exclusion), `shadow_ink_rect` |
| `src/platform/win32/render/box_rings.rs` | `L1522..1675`, `L2031..2402`, `L2416..2420` | `~4,250` | `resolve_render`, `brush_color`, `box_geometry`, `paint_box`, `shrink_radii`, `paint_focus_ring`, `paint_ring_color`, `paint_ring_color_width`, `inherited_action_ink`, `inherited_action_disabled` |
| `src/platform/win32/render/icons.rs` | `L2421..2639` | `~2,150` | `draw_icon_at`, `draw_icon` (typed Lucide vector path geometry) |

---

### Section 11 — `src/platform/win32/text.rs` (`19,029` tok → **5 files** under `src/platform/win32/text/`) — P11 Preparation

`Msftedit`, `HostEvent`, `HostShared`, `TimerPool`, `Activation`, `PeerShared`, `HostBox`, `PeerSurface`, `WindowlessPeer`, `NativeSend`, and `PeerConfig` are declared in `src/platform/win32/text/mod.rs` so child modules share full private field access without splitting the COM lifetime invariant.

| Target File | Source Lines (`text.rs`) | Est. Tokens | Responsibilities & P11 Readiness |
|---|---|---|---|
| `src/platform/win32/text/mod.rs` | `L1..309`, `L793..889`, `L1734..1769` | `~3,850` | `Msftedit` DLL/IID loader, `HostEvent`, `HostShared`, `TimerPool`, `PeerShared`, `HostBox`, `PeerSurface`, `WindowlessPeer` struct & accessors, `NativeSend`, `PeerConfig` |
| `src/platform/win32/text/host.rs` | `L310..792` | `~4,200` | `ComObjectInner` + `ITextHost_Impl` + `ITextHost2_Impl` on `HostBox` (`TxGetCharFormat`, `TxGetParaFormat`, `TxNotify`, `TxImmGetContext`, timers, caret, view inset) |
| `src/platform/win32/text/format_create.rs` | `L890..1220` | `~3,650` | **P11 Primary Target (Font/Format/Selection):** `apply_format` (`CHARFORMAT2W` / `PARAFORMAT2` / `EM_SETCHARFORMAT` / `EM_SETPARAFORMAT` / Geist font face & line-height twips), `WindowlessPeer::create`, `send`, `send_raw`, `text`, `story_len_utf16`, `selection_utf16`, `effective_geometry` |
| `src/platform/win32/text/draw_measure.rs` | `L1221..1488` | `~2,950` | **P11 Primary Target (D2D & Metrics):** `WindowlessPeer::draw` (`TxDrawD2D` into compatible D2D bitmap target), `natural_size` (`EM_REQUESTRESIZE`), `line_height`, `set_composing`, `activation_state`, `finish_pending_relatch`, `apply_bounds` |
| `src/platform/win32/text/selection_lifecycle.rs` | `L1489..1733`, `L1770..1927` | `~4,150` | **P11 Primary Target (Selection & Lifecycle):** `set_backward_selection_utf16` (TOM `ITextDocument` / `ITextSelection`), `set_selection_utf16`, `restore_selection_utf16`, `ensure_in_place`, `set_ui_active`, `effective_colors`, `set_colors`, `impl TextPeer for WindowlessPeer`, `Drop` |

---

### Section 12 — `src/platform/win32/uia.rs` (`16,862` tok → **5 files** under `src/platform/win32/uia/`)

| Target File | Source Lines (`uia.rs`) | Est. Tokens | Responsibilities |
|---|---|---|---|
| `src/platform/win32/uia/mod.rs` | `L1..180`, `L1341..1734` | `~4,150` | `Snapshot`, `VARIANT_MANUAL`, `WindowlessSite`, `ChildBuild`, `UiaRoot` (`rebuild`, `site`, `register_native`, `provider`, `Drop`) |
| `src/platform/win32/uia/paint_fragment.rs` | `L181..412` | `~2,550` | `ChildState`, `PaintFragment` (`IRawElementProviderSimple`, `IRawElementProviderFragment`, `IInvokeProvider`, `IToggleProvider`, `ISelectionItemProvider`, `IExpandCollapseProvider`) |
| `src/platform/win32/uia/editor_fragment.rs` | `L413..757` | `~3,350` | `EditorFragment` + `fence_provider_array` / `fence_range_array` |
| `src/platform/win32/uia/fenced_text_val.rs` | `L758..1080` | `~3,150` | `FencedText` (`ITextProvider`), `FencedValue` (`IValueProvider`), `FencedRange` (`ITextRangeProvider`) |
| `src/platform/win32/uia/fenced_prov_root.rs` | `L1081..1340` | `~2,650` | `FencedProvider` (`IRawElementProviderSimple`/`Fragment`) + `RootFragment` (`IRawElementProviderFragmentRoot`) |

---

### Section 13 — `src/ui.rs` (`36,411` tok → **10 files** under `src/ui/`)

`src/lib.rs` keeps `pub mod ui;` and `pub use ui::*;` (or its exact existing re-exports), so **zero public API paths change**. `StagedNode`, `UiFrame`, `Tx`, and `pub struct Ui` are declared in `src/ui/mod.rs` (with `pub use` re-exports of all child public types).

| Target File | Source Lines (`ui.rs`) | Est. Tokens | Responsibilities |
|---|---|---|---|
| `src/ui/mod.rs` | `L1..91`, `L669..1015` | `~4,150` | `CustomRender`, `Semantics`, `Role`, `Canvas`, `Path2d`, `Paint`, `TextRun`, `StagedNode`, `UiFrame`, `Tx`, `pub struct Ui`, submodule declarations & `pub use` re-exports |
| `src/ui/containers.rs` | `L92..471` | `~3,500` | `Row`, `Column`, `Stack`, `Surface`, `Card`, `Scroll`, `Separator`, `Alert`, `Skeleton` |
| `src/ui/widget_props.rs` | `L472..668` | `~1,950` | `Checkbox`, `Switch`, `RadioItemOpts`, `Field`, `Select`, `SelectChange`, `SelectItemOpts`, `DropdownMenu` |
| `src/ui/core_methods.rs` | `L1016..1494` | `~4,250` | `impl Ui`: static-identity containers (`row`, `column`, `stack`, `surface`, `card`, `scroll`, `separator`, ` badge`, `alert`, `skeleton`, `kbd`) + basic leaves (`label`, `button`, `text_input`, `text_area`, `custom`) |
| `src/ui/composition.rs` | `L1495..1836` | `~3,300` | `impl Ui`: `scope`, `keyed`, `box_`, `action`, `popup`, `typography`, `code_block`, `text`, `icon`, `icon_button`, `named`, `part`, `text_to_patch` |
| `src/ui/builders_leaf.rs` | `L1837..2335` | `~4,250` | `LabelBuilder`, `ButtonBuilder`, `TextInputBuilder` (up to `Drop`) |
| `src/ui/builders_custom_popup.rs` | `L2336..2892` | `~4,350` | `Drop for TextInputBuilder`, `CustomBuilder`, `ActionBuilder`, `PopupBuilder`, `TextBuilder`, `IconBuilder` |
| `src/ui/forms_api.rs` | `L2893..3307` | `~4,100` | `impl Ui` forms/controls (`checkbox`, `switch`, `radio_group`, `field`, `field_horizontal`, `input`, `textarea`, `select`) |
| `src/ui/nav_menus_api.rs` | `L3308..3703` | `~3,950` | `impl Ui` nav/overlays (`tabs`, `dropdown_menu`, `settings_nav`, `popover`, `dialog`, `alert_dialog`) + `RadioGroup`, `SelectItems`, `TabList`, `DropdownMenuItems`, `SettingsNavGroups` |
| `src/ui/input_builder.rs` | `L3704..4094` | `~3,550` | `SettingsNavItems`, `InputBuilder` + `Drop for InputBuilder` |

---

### Section 14 — `src/style.rs` (`22,587` tok → **6 files** under `src/style/`)

`src/lib.rs` keeps `pub mod style;` and `src/style/mod.rs` `pub use`s all child modules, keeping `crate::style::*` and `rust_ui::prelude::*` 100% unchanged.

| Target File | Source Lines (`style.rs`) | Est. Tokens | Responsibilities |
|---|---|---|---|
| `src/style/mod.rs` | `L1..395` | `~2,700` | `Color`, `Insets`, `CornerRadii`, `BorderSide`, `Border`, `Shadow`, `BoxStyle`, `TextWeight`, `TextSize`, `TextStyle`, `VisualStyle` |
| `src/style/patches.rs` | `L396..788` | `~3,550` | `BorderSidePatch`, `BorderPatch`, `CornerRadiiPatch`, `InsetsPatch`, `ShadowPatch`, `BoxStylePatch`, `TextStylePatch`, `TextInputStylePatch`, `VisualStylePatch` + `.patch()` methods |
| `src/style/state_action.rs` | `L789..1236` | `~3,850` | `StateStyles<P>`, `ButtonStylePatch`, `ActionStyle`, `Action`, `BoxProps`, `StyleState` |
| `src/style/recipes_button.rs` | `L1237..1815` | `~4,250` | Palette (`resolve_color`, translucent tokens), `button_recipe`, `resolve_button`, `state_delta`, `surface_recipe` |
| `src/style/recipes_card_form.rs` | `L1816..2238` | `~3,900` | `text_input_chrome_recipe`, `card_recipe`, `card_spacing`, `card_part_*`, `label_recipe`, `separator_recipe`, `badge_recipe`, `alert_recipe`, `skeleton_recipe`, `kbd_recipe`, `toggle_*`, `resolve_toggle_visual` |
| `src/style/recipes_overlay_os.rs` | `L2239..2657` | `~3,650` | `input_overlay_style`, `field_description_style`, `select_trigger_style`, `tab_*`, `popover_surface_style`, `dialog_surface_style`, `dropdown_*`, `settings_nav_*`, `SystemColor`, `os_enforce_*` |

---

### Section 15 — `src/runtime.rs`, `src/node.rs`, `src/tasks.rs`, `src/layout.rs`

#### 15.1 `src/runtime.rs` (`14,351` tok → **4 files** under `src/runtime/`)
`UpdateCtx` and `pub(crate) struct Runtime<S, M, U, V>` remain declared in `src/runtime/mod.rs`.

| Target File | Source Lines (`runtime.rs`) | Est. Tokens | Responsibilities |
|---|---|---|---|
| `src/runtime/mod.rs` | `L1..407` | `~4,050` | `UpdateCtx` (`spawn`, `cancel`, `scope`, theme/motion/focus), `struct Runtime`, `Runtime::new`, `pump`, `set_appearance`, `review` |
| `src/runtime/reconcile.rs` | `L408..895` | `~4,160` | `commit_children`, `drain_dirty`, `create_node`, `update_node`, `remove_node` |
| `src/runtime/dispatch_conflict.rs` | `L896..1219` | `~3,150` | `dispatch_event`, `deliver_envelope`, `emit_conflict_for_request`, `emit_conflict`, `edit_event`, `selection_event`, `composition_start/active/clear/end` |
| `src/runtime/sched_validate.rs` | `L1220..1497` | `~2,990` | `pump_sched`, `next_deadline`, `sched_arm_tooltip`, `set_motion_resolver`, `set_forced_resolver`, `shutdown`, test surface, `Drop`, `validate_bindings`, `validate_styles` |

#### 15.2 `src/node.rs` (`11,099` tok → **3 files** under `src/node/`)

| Target File | Source Lines (`node.rs`) | Est. Tokens | Responsibilities |
|---|---|---|---|
| `src/node/mod.rs` | `L1..392`, `L1174..1201` | `~4,150` | `KIND_*` constants, `PopupRole`, `SemanticRole`, `PeerSpec`, `TextPeer`, `NodeId`, `Presented`, `TextPeerSync`, `PeerDecision`, `NodeEvent`, `PointerPhase`, `QueuedEvent` |
| `src/node/data.rs` | `L393..677`, `L1104..1173` | `~3,150` | `ContainerProps`, `NodeData`, `Node` struct, `state_metric_affecting`, `box_dirty`, `visual_dirty`, `visual_clone`, `layout_clone`, `kind_tag`, `interactive`, `lease` |
| `src/node/dirty.rs` | `L678..1103` | `~3,800` | `dirty_diff` (retained node dirty-bit classifier) |

#### 15.3 `src/tasks.rs` (`9,942` tok → **3 files** under `src/tasks/`)

| Target File | Source Lines (`tasks.rs`) | Est. Tokens | Responsibilities |
|---|---|---|---|
| `src/tasks/mod.rs` | `L1..220` | `~2,100` | `Executor`, `TaskStartError`, `SendError`, `ProxySendError`, `WakerSet`, `CancelToken`, `CancelFuture`, `Charge`, `Envelope` |
| `src/tasks/mailbox.rs` | `L221..615` | `~3,650` | `MailboxState`, `PushOutcome`, `Mailbox` (bounded queue, wake callback, backpressure) |
| `src/tasks/registry.rs` | `L616..1093` | `~4,190` | `ChanCore`, `TaskSender<M>`, `Job<M>`, `SendFuture`, `UiProxy<M>`, `ScopePath`, `Registration`, `Dequeued`, `TaskRegistry`, `CancelWatch` |

#### 15.4 `src/layout.rs` (`7,008` tok → **2 files** under `src/layout/`)

| Target File | Source Lines (`layout.rs`) | Est. Tokens | Responsibilities |
|---|---|---|---|
| `src/layout/mod.rs` | `L1..349` | `~3,500` | `FillItem`, `waterfill`, `justify_offset`, `editor_chrome`, `editor_inset`, `editor_content_rect`, `Interaction`, `node_insets`, `icon_side_inset`, `button_icon_size`, `button_icon_gap`, `button_icon_rect`, `button_content_insets` |
| `src/layout/widgets.rs` | `L350..755` | `~3,510` | `select_chevron_rect`, `tab_*`, `settings_nav_*`, `popup_rect`, `ring_color_role`, `toggle_*` (`checkbox`, `radio`, `switch` geometry) |

---

### Section 16 — `src/devtools/mod.rs` (`20,792` tok → **6 files** under `src/devtools/`)

Over half of `src/devtools/mod.rs` (`L1131..2203`, `~10,500` tok) is its embedded `#[cfg(test)] mod tests`.

| Target File | Source Lines (`devtools/mod.rs`) | Est. Tokens | Responsibilities |
|---|---|---|---|
| `src/devtools/mod.rs` | `L1..360`, `L1128..1130` | `~3,300` | Protocol constants, `Request`, `Error`, `Response`, `Rect`, `PixelRect`, `snap_rect`, `PartNode`, `Presented`, `Node`, `SemanticKey`, `Snapshot`, `pub mod client;`, `pub use` re-exports |
| `src/devtools/contract.rs` | `L361..694` | `~3,550` | `ExpectedTarget`, `reference_targets` (`contract/0.2` parser), `TierScope`, `compare_layout` |
| `src/devtools/compare.rs` | `L695..1127` | `~3,950` | `ElementArtifacts`, `compare_element`, `Image` (PNG decode/encode, `crop`, `area-box/1` `canonicalize`, `compare_image`) |
| `src/devtools/tests_protocol_snap.rs` | `L1131..1566` | `~3,850` | Unit tests: protocol framing, `Snapshot::index`, D02 single coordinate snap authority |
| `src/devtools/tests_raster_contract.rs` | `L1567..1901` | `~3,250` | Unit tests: D01 `area-box/1` resampler + D03 `contract/0.2` adapter, tiers, parts |
| `src/devtools/tests_compare.rs` | `L1902..2203` | `~2,900` | Unit tests: offline `compare_element` pipeline & negative controls |

---

### Section 17 — `src/platform/win32/devtools.rs` (`10,649` tok) & `src/platform/win32/devctl_client.rs` (`7,273` tok)

#### 17.1 `src/platform/win32/devtools.rs` → **3 files** under `src/platform/win32/devtools/`

| Target File | Source Lines (`win32/devtools.rs`) | Est. Tokens | Responsibilities |
|---|---|---|---|
| `src/platform/win32/devtools/mod.rs` | `L1..355` | `~3,750` | `platform`, `clip_fully_excludes`, `dev_snapshot`, `dev_node` |
| `src/platform/win32/devtools/parts_target.rs` | `L356..509`, `L972..1021` | `~2,150` | `collect_parts`, `dev_target`, `dev_point`, `toggle_part_rects` |
| `src/platform/win32/devtools/commands.rs` | `L510..971` | `~4,750` (`~4,400` without trailing helpers) | `dev_command` (`handshake`, `tree`, `snapshot-layout`, `rect`, `hover`, `leave`, `press`, `release`, `click`, `focus`, `key`, `screenshot`, `shutdown`), `service_devtools`, `dev_idle_state`, `complete_devtools_idle` |

#### 17.2 `src/platform/win32/devctl_client.rs` → **2 files** (`devctl_client.rs` + `devctl_server.rs`)
In `src/devtools/mod.rs`, `#[cfg(windows)] #[path = "../platform/win32/devctl_client.rs"] pub mod client;` remains the entry module and delegates server internals to `mod devctl_server;`.

| Target File | Source Lines (`devctl_client.rs`) | Est. Tokens | Responsibilities |
|---|---|---|---|
| `src/platform/win32/devctl_client.rs` | `L1..510` | `~3,950` | `Handle`, pipe framing, receipt file, `Session`, `Launched`, `launch`, `session`, `shutdown`, `cleanup` |
| `src/platform/win32/devctl_server.rs` | `L511..834` | `~3,320` | `Admission`, `Envelope`, `Shared`, `Server` (in-process named-pipe listener thread), `mod capture;`, unit tests |

---

### Section 18 — `examples/gallery.rs` (`24,065` tok → **7 files** under `examples/gallery/`) & Resolving the `include!` Trap

#### How `examples/gallery` and `src/bin/rust-ui-gallery.rs` Currently Work
- `examples/gallery.rs` is auto-discovered by Cargo as `example "gallery"`.
- `src/bin/rust-ui-gallery.rs` contains `include!("../../examples/gallery.rs");` so `CARGO_BIN_EXE_rust-ui-gallery` is available to `tests/devctl_windows.rs` and `rust-ui-gallery-launcher.exe`.
- **Why naive `mod pages;` fails:** In Rust, `mod foo;` inside an `include!`-ed file resolves relative to the **including** file (`src/bin/foo.rs`), not `examples/foo.rs`.

#### Clean, Zero-Duplication Solution
Move `examples/gallery.rs` to **`examples/gallery/main.rs`** (Cargo natively discovers `examples/<name>/main.rs` as `example "<name>"`!) and update `src/bin/rust-ui-gallery.rs` to:

```rust
#[path = "../../examples/gallery/main.rs"]
mod gallery;

fn main() -> rust_ui::UiResult {
    gallery::main()
}
```
(With `pub(crate) fn main() -> UiResult` in `examples/gallery/main.rs` — or equivalently in `Cargo.toml` set `[[bin]] name = "rust-ui-gallery" path = "examples/gallery/main.rs"` and delete `src/bin/rust-ui-gallery.rs`!)
Setting `path = "examples/gallery/main.rs"` on `[[bin]] name = "rust-ui-gallery"` in `Cargo.toml` (or using `#[path = "../../examples/gallery/main.rs"] mod gallery;`) guarantees:
- `cargo check --examples` / `cargo run --example gallery` works,
- `cargo build --bin rust-ui-gallery` and `CARGO_BIN_EXE_rust-ui-gallery` work,
- Normal `mod components; mod forms; ...` inside `examples/gallery/main.rs` resolve cleanly to `examples/gallery/*.rs` in both builds!

| Target File | Source Lines (`examples/gallery.rs`) | Est. Tokens | Responsibilities |
|---|---|---|---|
| `examples/gallery/main.rs` | `L1..296`, `L1857..2058` | `~4,150` | `Page`, `Gallery` state, `Msg`, `update`, `view`, `header`, `stub_page`, `typography_page`, `card_parts`, `main` |
| `examples/gallery/components.rs` | `L297..634` | `~4,280` | `Gallery::components_page` (Button, IconButton, Card, Separator, Badge, Alert, Skeleton, Kbd) |
| `examples/gallery/forms_a.rs` | `L635..885` | `~3,150` | `Gallery::forms_page` part 1 (Checkbox, Switch, RadioGroup, Field specimens) |
| `examples/gallery/forms_b.rs` | `L886..1117` | `~2,900` | `Gallery::forms_page` part 2 (Input, Textarea, Select specimens) |
| `examples/gallery/navigation.rs` | `L1118..1428` | `~4,010` | `Gallery::navigation_page` (Tabs default/line, SettingsNav wide/collapsed rail) |
| `examples/gallery/overlays.rs` | `L1429..1691` | `~3,440` | `overlay_stage`, `Gallery::overlays_page` (Tooltip, Popover, Dialog, DropdownMenu) |
| `examples/gallery/native_text.rs` | `L1692..1856` | `~1,360` | `Gallery::native_text_page` (Windowless RichEdit single-line & multiline specimens) |

---

### Section 19 — `examples/native_probe.rs` (`11,723` tok → **3 files** under `examples/native_probe/`)

Move `examples/native_probe.rs` to `examples/native_probe/main.rs` so Cargo discovers a single `native_probe` example while helper modules live in `examples/native_probe/*.rs` without being treated as standalone examples.

| Target File | Source Lines (`native_probe.rs`) | Est. Tokens | Responsibilities |
|---|---|---|---|
| `examples/native_probe/main.rs` | `L1..292`, `L890..1087` | `~4,400` | Window lookup, geometry, mouse/key input injection (`click`, `type_text`, `dpi_changed`, `click_post`), `json_str`, CLI `main()` |
| `examples/native_probe/uia_probe.rs` | `L293..627` | `~4,230` | UIA tree/rect/enabled/count/invoke/value/label inspection helpers |
| `examples/native_probe/ime_probe.rs` | `L628..889` | `~3,090` | `ime_japanese`, `ime_japanese_inner` (IMM32 composition testing) |

---

### Section 20 — `tests/devctl_windows.rs` (`6,073` tok → **2 files**)

In Cargo, files in `tests/*.rs` are compiled as separate integration test crates, whereas subdirectories `tests/<subdir>/mod.rs` are **not** compiled as standalone crates.

| Target File | Source Lines (`tests/devctl_windows.rs`) | Est. Tokens | Responsibilities |
|---|---|---|---|
| `tests/devctl_support/mod.rs` | `L1..235` | `~2,150` | `Session` RAII wrapper, `devctl` CLI runner, JSON/snapshot helpers, `probe_session`, `gallery_session` |
| `tests/devctl_windows.rs` | `L236..624` | `~3,920` | `mod devctl_support;` + all `#[test]` integration tests (probe E2E, Gallery 3-cycle E2E, focus/leave, WGC compare-element, tier scope) |

---

### Section 21 — Temporary Files / Hygiene Audit

- **`LAUNCH_FLASH_PROBLEM.md`:**
  - Inspected `git status` and searched the entire repository (`grep` across all tracked files): **zero references** exist anywhere in code, scripts, or docs.
  - It is an untracked handoff scratch file created during the launch-flash investigation (which was fixed and pushed in `91f90f6`).
  - **Recommendation for Lead:** Delete untracked `LAUNCH_FLASH_PROBLEM.md` in Phase 0 (`rm -f LAUNCH_FLASH_PROBLEM.md`). (Left untouched on this planning branch per Section 34.)
- **Other temporary files:** None found in `/home/therceman/git/rust-ui` (`git status --ignored` shows only standard ignored directories).

---

## 5. Exact Refactor Phases & Commit Strategy (Sections 22–23)

Each phase is self-contained, independently compilable, and committed as its own checkpoint (`R1`–`R6`) for clean `git bisect` and rollback.

| Phase / Commit | Scope | Files Touched -> New Files Created | Risk Level | Verification Gate Before Commit | Rollback Checkpoint |
|---|---|---|---|---|---|
| **Phase 0 / `R0` (Baseline)** | Capture baseline test lists & visual/bounds baseline; delete untracked `LAUNCH_FLASH_PROBLEM.md`; add `! -path '*/docs/*'` to `tools/check-file-tokens.sh` | `tools/check-file-tokens.sh` (`1` file) | **Low** | `./tools/check-file-tokens.sh` reports exactly the 19 code files (`0` docs files) | `git checkout -- tools/check-file-tokens.sh` |
| **Phase 1 / `R1`** | Split `src/tests.rs` (`85,842` tok) into `src/tests/` | `src/tests.rs` -> `src/tests/mod.rs` + 22 slice files (`23` files) | **Low** | Linux `cargo test --locked --lib` + Windows `cargo.exe test --locked --lib --features devtools` + diff `--list` against baseline | `git reset --hard R0` |
| **Phase 2 / `R2`** | Split `src/platform/win32/window.rs`, `layout.rs`, `devtools.rs`, `devctl_client.rs`, and `mod.rs` | 5 files -> `22` files under `src/platform/win32/` | **Highest** (owns WndProc, P9/P10 resize/move/DPI, `Backend` turn/paint/run) | Linux check + Windows `cargo.exe test --locked --lib --features devtools` + Gallery launch/resize sanity | `git reset --hard R1` |
| **Phase 3 / `R3`** | Split `src/platform/win32/render.rs`, `text.rs`, and `uia.rs` | 3 files -> `18` files under `src/platform/win32/{render,text,uia}/` | **High** (D2D rendering, `TxDrawD2D`, COM `ITextHost2` & UIA fragments) | Windows `cargo.exe test --locked --lib --features devtools` (includes shadow, DWrite, RichEdit, and UIA native probes) | `git reset --hard R2` |
| **Phase 4 / `R4`** | Split platform-neutral core: `src/ui.rs`, `src/style.rs`, `src/runtime.rs`, `src/node.rs`, `src/tasks.rs`, `src/layout.rs`, `src/devtools/mod.rs` | 7 files -> `34` files under `src/{ui,style,runtime,node,tasks,layout,devtools}/` | **Medium** (public re-exports in `ui`/`style`/`devtools`) | Linux & Windows `cargo check --locked --examples --features devtools` + `cargo test --locked --lib --features devtools` | `git reset --hard R3` |
| **Phase 5 / `R5`** | Split `examples/gallery.rs`, `examples/native_probe.rs`, and `tests/devctl_windows.rs` | 3 files + `src/bin/rust-ui-gallery.rs` -> `12` files | **Low-Medium** (Cargo target discovery & `CARGO_BIN_EXE_rust-ui-gallery`) | `cargo.exe build --locked --features devtools --bin rust-ui-gallery --examples` + `cargo.exe test --locked --test devctl_windows --features devtools` | `git reset --hard R4` |
| **Phase 6 / `R6` (Freeze)** | Global token-budget gate + full test-name identity + visual/bounds/P9/P10 equivalence verification | Verification only (`0` code edits) | **Low** | `./tools/check-file-tokens.sh` exits `0` (`OK — every file under 5000 tokens`) + full Section 25–28 gates | `git reset --hard R5` |

---

## 6. Zero-Behavior-Change, Test Identity & Visual Equivalence Contracts (Sections 24–28)

### Section 24 — Zero-Behavior-Change Contract
No phase may alter:
- Public API signatures, type paths, or `rust_ui::prelude::*` exports
- Render output, component geometry, colors, fonts, selection, or motion
- Event ordering, focus, keyboard, pointer, or popup behavior
- Layout, P9 held border resize, P10 cross-monitor move/DPI transition, or DWM caption/border sync
- RichEdit (`ITextHost2`/`ITextServices2`/`TxDrawD2D`) or UIA provider semantics
- `rust-ui-devctl` protocol, Gallery specimen IDs, automation IDs, or test expectations

If a mechanical move uncovers a latent bug, **record it and do not fix it inside the refactor commits**.

### Section 25 — Test Identity Contract
1. **Baseline capture before Phase 1:**
   ```bash
   cargo test --locked --lib -- --list > /tmp/baseline_linux_lib_tests.txt
   powershell.exe -NoProfile -Command '$env:CARGO_TARGET_DIR="W:\devin_folder\rust-ui-slice3-target"; cargo.exe test --locked --lib --features devtools --manifest-path \\wsl.localhost\Ubuntu\home\therceman\git\rust-ui\Cargo.toml -- --list' > /tmp/baseline_win_lib_tests.txt
   ```
2. **Post-phase verification:**
   - Using `include!("...")` in `src/tests/mod.rs` (and `src/devtools/mod.rs` `mod tests`), `diff -u /tmp/baseline_linux_lib_tests.txt /tmp/after_linux_lib_tests.txt` and `diff -u /tmp/baseline_win_lib_tests.txt /tmp/after_win_lib_tests.txt` must be **empty (`0` diff lines, identical test names and counts)**.
   - If `mod <subfile>;` is used instead of `include!` for any test submodule, strip the single submodule path segment (`sed -E 's/^tests::[a-z0-9_]+::/tests::/'`) and assert `diff -u` is empty.

### Section 26 & 27 — Visual, Bounds & P9/P10 Runtime Equivalence Contract
1. **Bounds check equivalence:**
   - Run `python3 tools/bounds_check.py` (or devctl `snapshot-layout` on Light/Dark `components`, `forms`, `navigation`, `overlays`, `typography`, `native-text` pages) before Phase 1 and after Phase 5: all element `rect_dp` and `parts` rects must match with `0.0dp` delta.
2. **Visual screenshot equivalence:**
   - Capture WGC screenshots of the Gallery pages (`components`, `forms`, `navigation`, `overlays`, `typography`, `native-text`) in Light and Dark modes before Phase 1 and after Phase 5, and compare via `rust-ui-devctl compare-image --channel-tolerance 0`: `mismatched_pixels` must be `0`.
3. **P9/P10 & Launcher sanity (Section 27):**
   - Held border resize (`wide -> narrow -> wide` without mouse release): `laid_client` converges on every live `WM_PAINT`.
   - Held titlebar cross-monitor move (partially off-screen -> re-exposed): live in-session `WM_PAINT` repaints exposed region; `WM_EXITSIZEMOVE` refreshes UIA once.
   - `100% <-> 125%` `WM_DPICHANGED` transition: scale, D2D target DPI, layout, and RichEdit peers converge in one frame.
   - DWM dark caption/border (`0x18181B`) and cloak-reveal launch with zero white flash.
   - `rust-ui-gallery-launcher.exe` stamp fast-path and rebuild path succeed.

### Section 28 — Required Build / Test Gates
- **Linux:**
  ```bash
  cargo fmt --check
  cargo check --locked --examples
  cargo test --locked --lib
  ```
- **Windows (`CARGO_TARGET_DIR=W:\devin_folder\rust-ui-slice3-target`):**
  ```powershell
  cargo.exe check --locked --examples
  cargo.exe test --locked --lib --no-run
  cargo.exe test --locked --lib
  cargo.exe check --locked --examples --features devtools
  cargo.exe test --locked --lib --features devtools
  cargo.exe test --locked --test devctl_windows --features devtools
  ```
- **Token Gate:**
  ```bash
  ./tools/check-file-tokens.sh
  # Expected: OK — every file under 5000 tokens (exit 0)
  ```

---

## 7. Module-Visibility & Cycle / Coupling Analysis (Sections 29–30)

### Visibility Audit (Section 29)
- **Public API (`pub`):** **Zero new `pub` items.** All public types split into `src/ui/*.rs`, `src/style/*.rs`, `src/tasks/*.rs`, and `src/devtools/*.rs` are already `pub` today and are re-exported via `pub use` in their parent `mod.rs`.
- **Crate-Internal (`pub(crate)`):** Unchanged across crate-level modules.
- **Module-Internal (`pub(super)`):** Used only for helper functions shared between sibling submodules within the same parent directory (e.g., `window::wndproc::dispatch`, `render::dwrite::layout_for`, `render::box_rings::paint_box`, `render::shadow::draw_shadow`, `render::icons::draw_icon_at`, `style::recipes_button::resolve_color`).
- **Private Struct Fields:** Preserved as private (`field: Type`) on `Backend`, `Runtime`, `Ui`, `Tx`, `Renderer`, `WindowlessPeer`, and `UiaRoot` by keeping their `struct` definitions in the parent `mod.rs`.

### Acyclic Dependency Ownership (Section 30)
1. **`ui` ↔ `style`:** Acyclic. `style` defines `BoxStyle`, `TextStyle`, `VisualStyle`, `*Patch`, and component recipes; `ui` imports `crate::style::*` to stage nodes. `style` never imports `ui`.
2. **`win32::render` ↔ `win32::text`:** Acyclic. `render/frame_widgets.rs` calls `peer.borrow().draw(&target, rect, target_gen)` on `WindowlessPeer` (`win32::text`). `win32::text` never imports `win32::render` (it uses Direct2D `ID2D1RenderTarget` directly for `TxDrawD2D`).
3. **`win32::Backend` ↔ `win32::uia`:** Acyclic. `uia` defines `ChildBuild`, `Snapshot`, and `UiaRoot` without importing `Backend`; `win32/uia_bridge.rs` (`impl Backend`) builds `Vec<uia::ChildBuild>` from `Backend` state and calls `UiaRoot::rebuild`.
4. **`runtime` ↔ `node`:** Acyclic. `node` defines `Node`, `NodeData`, `TextPeer`, `TextPeerSync`, and `dirty_diff`; `runtime` imports `crate::node::*` to reconcile the arena and dispatch events.
5. **`devtools` (portable) ↔ `win32::devtools` (platform):** Acyclic. `src/devtools/` owns portable wire/snapshot/comparison types (`Request`, `Response`, `Snapshot`, `PartNode`, `Image`); `src/platform/win32/devtools/` populates `devtools::Snapshot` from `Backend`.

---

## 8. P11 Readiness Map (`SHADCN_AUTOMATED_VISUAL_PARITY_V1`) (Section 32)

After this structural refactor lands, the upcoming P11 automated visual parity work maps directly to dedicated, small (`< 4,350`-token) files:

| P11 Concern | Post-Refactor Authoritative Module Path |
|---|---|
| `TextStyle` & `TextStylePatch` | `src/style/mod.rs`, `src/style/patches.rs` |
| Input / Textarea / Select / Field chrome & overlay recipes | `src/style/recipes_card_form.rs`, `src/style/recipes_overlay_os.rs` |
| Native RichEdit peer creation, `CHARFORMAT2W`/`PARAFORMAT2` & Geist font face | `src/platform/win32/text/format_create.rs` |
| Native bundled font registration (`Geist` / `Geist Mono`) | `src/platform/win32/fonts.rs` (unchanged, `1,100` tok) |
| Native RichEdit `TxDrawD2D` rendering & `EM_REQUESTRESIZE` measurement | `src/platform/win32/text/draw_measure.rs` |
| Native RichEdit TOM selection (`ITextDocument` / `ITextSelection`) & colors | `src/platform/win32/text/selection_lifecycle.rs` |
| Renderer focus rings, invalid rings & `paint_box` borders | `src/platform/win32/render/box_rings.rs`, `src/platform/win32/render/frame_widgets.rs` |
| Motion / transition scheduler | `src/sched.rs` (unchanged, `2,400` tok) + `src/runtime/sched_validate.rs` |
| Devctl semantic tree, parts & style inspection | `src/platform/win32/devtools/mod.rs`, `src/platform/win32/devtools/parts_target.rs` |
| `compare-element` / `compare-layout` & `contract/0.2` adapter | `src/devtools/compare.rs`, `src/devtools/contract.rs` |
| Gallery specimen pages (`components`, `forms`, `navigation`, `overlays`, `native_text`) | `examples/gallery/{components,forms_a,forms_b,navigation,overlays,native_text}.rs` |

---

## 9. Universal Gates (1–20) Self-Review & Explicit Non-Goals (Sections 33–35)

- **G3 (Behavior Preservation):** Enforced by verbatim code movement, zero logic edits, test-list identity, and before/after screenshot + bounds comparison.
- **G4 & G5 (Scope & Minimal Design):** Only the 19 over-budget code files (plus `tools/check-file-tokens.sh` and `src/bin/rust-ui-gallery.rs` path wiring) are touched; parent-declared structs prevent `pub(super)` field sprawl.
- **G6, G7, G8, G10 (Ownership, Reuse, No Shims, Single Authority):** No duplicate Gallery binary, no compatibility shims, and P9/P10 modal resize/move/DPI logic remains cohesive in `window/wndproc.rs` and `win32/paint_timers.rs`.
- **G13, G16, G17, G18, G19, G20 (Blast Radius, Verification, Recovery, Identity, Honesty, Completeness):** 6 atomic phases (`R0`–`R6`) with per-phase Linux + Windows gates and exact token projections (`< 4,500` target, `< 5,000` ceiling across all 111 target files).
- **Explicit Non-Goals:** No P11 visual parity fixes, no RichEdit font/selection changes, no motion changes, no warning-cleanup sweep on unrelated files, and no public API changes during this refactor.
