# rust-ui-devctl v0.1 implementation report

**RUST_UI_DEVCTL_V0_1_READY_FOR_NATIVE_GALLERY**

RUN_KEY: `RUI-W-261006-0850`.
Implementation: `6375a4e537f0ec396c92154738f7bfced4324c5c`.
Exact base: `dc70f6644c0384170f3a6929eb2caaf33fc711a0`.
Frozen reference payload: `2e14e417a69ef8967b8fb17120282e3de6449fae`.
Prior Rust production: `00dc29acfeb686d6a190d91624de7ab9a48e1e92`.
This report/evidence publication follows the implementation without altering it.

## Architecture and feature boundary

`devtools` is opt-in. Ordinary builds retain only optional semantic automation-ID
metadata and its UIA projection; they have no listener, session, capture, extra
thread or periodic tooling work. Feature-enabled apps also create no listener
unless explicitly launched with session credentials. No permanent daemon exists.

| Responsibility | Authority |
|---|---|
| Semantic identity | Retained node/staged transaction; builders and `Ui::named` |
| State, layout, interaction, native text | Existing runtime and guarded Win32 Backend |
| Protocol, deterministic snapshots, offline comparisons | `src/devtools/mod.rs` |
| Local named pipe and process/session ownership | `src/platform/win32/devctl_client.rs` |
| Runtime introspection/idle barrier | `src/platform/win32/devtools.rs` |
| WGC/client geometry/native capture resources | `src/platform/win32/devctl_capture.rs` |
| CLI orchestration and file outputs | `src/bin/rust-ui-devctl.rs` |

The transport reuses the checked Mailbox event wake and established native-entry
guard. It does not introduce another UI executor or cross-thread native-message
cascade. IPC admission serializes request publication with checked wake success;
a rejected request cannot later execute. Listener recreation failures reach the
existing terminal runtime path. All Win32/COM implementation stays under platform.

## CLI, protocol and automation IDs

Commands: `launch`, `wait-idle`, `tree`, `rect`, `hover`, `press`, `release`, `click`,
`focus`, `key`, `screenshot`, `snapshot-layout`, `compare-layout`, `compare-image`,
`compare-element`, `shutdown`; `cleanup` explicitly removes stale receipts.
Every command accepts `--json`; stdout contains one JSON document, even without
the switch. No interactive prompts or terminal progress are mixed into stdout.

`rust-ui.devctl/0.1` uses bounded 4 MiB length-prefixed UTF-8 JSON, request IDs,
command/arguments, version/authentication, and response result or typed error.
Local pipes reject remote clients. Independent 256-bit random session ID/token,
one instance, bounded 16-request UI handoff, checked framing and deadlines constrain
the endpoint. Credentials live in the explicit session receipt, not comparable
artifacts. One owned process/window per session; launch inherits no handles.

`.automation_id("probe.button")` and container `Ui::named` store optional semantic
strings. Empty, control-bearing, >256-byte or duplicate IDs reject a staged
test/devtools transaction before native mutation. Normal reconciliation retains
identity; IDs project to Windows UIA AutomationId and native retained tree lookup.
No label scraping, UIA self-crawl, exposed generational identity or DOM path is used.

Full CLI examples, error/exit codes and wire examples are in [DEVCTL.md](DEVCTL.md)
and [DEVCTL_PROTOCOL.md](DEVCTL_PROTOCOL.md). Exit classes are 0 success, 2 usage,
3 missing/not-visible, 4 timeout, 5 capture failure, 6 comparison mismatch and
7 protocol/unsupported/I/O/runtime failure.

## Wait-idle and semantic interaction

Idle requires no admitted mailbox/app events, reconciliation/state dirtiness,
native notifications/deferred/reentrant work, layout/paint dirtiness, damage,
invalidated client paint, pump continuation or scheduled framework deadline.
It completes after the guarded drain and a checked `DwmFlush` presentation barrier,
with a recheck afterward. It neither sleeps for an arbitrary interval nor polls.
Timeouts include last observed queue/dirty/deadline diagnostics.

Hover/press/release/click use existing pointer/hit/event/state machinery, with
strict enabled/visible/actual hit-owner lookup. Held press remains capturable;
release performs the real event. Keyboard focus and key down/up use the existing
backend/native route. Supported keys: Enter, Space, Tab, Escape, arrows, Backspace,
Delete, Home and End. Physical OS modifier state applies. Character synthesis,
modifier chords, IME automation and scroll-into-view are explicitly outside v0.1.
Future deliveries/OS caret are not frozen by an idle acknowledgement.

## WGC, coordinates and snapshot-layout

Only Windows Graphics Capture supplies screenshots; unsupported/failed capture
returns `CAPTURE_FAILED`, with no PrintWindow/BitBlt fallback. Free-threaded frame
pool, D3D11 staging texture and BGRA→RGBA copy have bounded frame/pixel budgets and
per-capture RAII cleanup. WinRT initialization remains alive for the CLI process
lifetime to prevent native capture cleanup racing module unload.

The CLI establishes PMv2 capture coordinates and measures GetClientRect,
client-to-screen and DWM extended frame geometry. It validates WGC extent and
unchanged geometry, then crops measured client pixels. Before/after retained
scene and successful-paint epoch checks reject a mutation (including changing
back) during acquisition. No guessed titlebar offset is used.

Scale is physical pixels per logical unit, using existing `geom::ScaleFactor` and
Win32 dpi conversion. Snapshot/rect coordinates are client-relative. Native tests
cover 96/120/144/192 DPI mappings; the actual host is 120 / 1.25. Actual client
capture is 962×853; button crop is 87×45 at (66,105). Hidden/off-client/padded-outside
targets fail rather than returning empty crops.

`rust-ui.devctl.snapshot-layout/0.1` records deterministic retained hierarchy,
semantic IDs, client sizes/scale, logical/physical rects and semantic visible,
enabled/focus/hover/press state. No pointer, nonce, timestamp or NodeId is included.
Budgets: 8192 nodes, depth 128, 4 MiB encoded response. The root/client logical
conversion explicitly uses the same scale authority and has a native consistency
assertion. Capture metadata carries geometry/backend, not a timestamp.

## Layout, image and element comparators

The read-only adapter consumes the frozen `contract/0.2` capture key (`page/state`)
and automation IDs directly. It creates no second maintained reference contract.
Layout compares presence/visibility and x/y/width/height with explicit Dp tolerance;
absolute and size-only modes are explicit. Results expose expected/actual/deltas
and matched/missing/unexpected/failed counts. Relative mode is not implemented.

PNG comparison reports dimensions, mismatched pixels/percentage, raw maximum
channel delta, mean absolute channel error and difference bbox, plus a deterministic
amplified diff. Explicit channel tolerance never hides raw statistics. Unequal
sizes fail and still generate a union-canvas diff; no image is rescaled and no
perceptual-equivalence claim is made. Decode/diff budget is 32 Mi pixels.

`compare-element` resolves each authoritative rect, crops the frozen full-page
reference and visible native client independently, and writes reference/native/diff
PNGs and geometry/image JSON. Full-page native stitching is unnecessary. Reference
DPR1 pixels and native host pixels can differ at scale 1.25; the result reports
that mismatch rather than manufacturing parity.

## Calibration probe and demonstrated agent workflow

`examples/devctl_probe.rs` uses existing generic primitives only, with IDs
`probe.root`, `probe.surface`, `probe.label`, `probe.button`, `probe.input`.
The native CLI integration independently drives three full sessions without a
physical mouse or GUI inspection requirement:

```powershell
cargo build --locked --features devtools --all-targets
$ctl = '.\target\debug\rust-ui-devctl.exe'
& $ctl launch --exe .\target\debug\examples\devctl_probe.exe --session .devctl\session.json --json
& $ctl wait-idle --session .devctl\session.json --json
& $ctl tree --session .devctl\session.json --json
& $ctl rect probe.button --session .devctl\session.json --json
& $ctl hover probe.button --session .devctl\session.json --json
& $ctl screenshot --id probe.button --session .devctl\session.json --out .devctl\hover.png --json
& $ctl press probe.button --session .devctl\session.json --json
& $ctl screenshot --id probe.button --session .devctl\session.json --out .devctl\pressed.png --json
& $ctl release probe.button --session .devctl\session.json --json
& $ctl snapshot-layout --session .devctl\session.json --out .devctl\native.json --json
& $ctl screenshot --session .devctl\session.json --out .devctl\client.png --json
& $ctl compare-layout --reference .devctl\native.json --snapshot .devctl\native.json --json
& $ctl compare-image --reference-image .devctl\client.png --native-image .devctl\client.png --diff .devctl\diff.png --json
& $ctl compare-element --reference .devctl\native.json --snapshot .devctl\native.json --id probe.button --reference-image .devctl\client.png --native-image .devctl\client.png --out-dir .devctl\button --json
& $ctl shutdown --session .devctl\session.json --json
```

Create `.devctl` before launch. Screenshot includes its own wait-idle. Representative
actual layout/image self controls:

```json
{"pass":true,"matched":5,"missing":0,"unexpected":[],"failed":0}
```

```json
{"mismatched_pixels":0,"max_channel_delta":0}
```

These are selected result fields, not a replacement wire schema. The negative
layout control changes only button x by +1 Dp: exit 6, reported delta +1. The image
control changes one pixel: exit 6, one mismatch, bbox (0,0,1,1). Element self fixture
writes both crops/diff and passes. Native actions produce Clicks 0→1→2→3 and accepted
RichEdit End/Backspace changes committed application text to `Calibration tex`.

## Dependencies, resources and security proofs

Added direct optional dependencies: serde 1 (derive, default std), serde_json 1
(default std), png 0.17 (default features disabled). Lockfile pins versions and
compression/parser transitives. PNG is a focused codec, avoiding extra WIC/COM
decode/encode ownership. Existing windows 0.62.2 receives only feature-gated capture,
D3D11/DirectX/DXGI, WinRT interop, DWM, pipes/I/O/file primitives and BCrypt APIs.
No Tokio, async/CLI/logging framework, HTTP server, image framework or CI was added.

The protocol exposes no shell command, arbitrary filesystem read, PID attach or
global SendInput. Launch uses the explicit executable without a shell; exclusive
receipt reservation prevents two launches overwriting ownership. The nonce is not
a protection against the same Windows user/admin reading its private credentials.
Overlapped I/O cancellation joins before freeing buffers; shutdown stops/joins the
pipe worker before backend destruction and waits owned process exit. Output files
are published after complete writes; session cleanup does not kill arbitrary PIDs.

In each final session, target handles warm from 225 to 233 and remain 233 across
six later captures. Each two-second idle sample observes zero CPU tick increase,
zero native timer fires and unchanged redraw counters. Three owned targets exit
and receipts/temp state are removed. These bounded smoke tests do not prove all
future GPU/COM leak or performance behavior.

## Final tests, identity and separate foundation issue

Exact-source native results: fmt/check/build PASS, Windows default **123/123**,
Windows devtools **139/139** (both parallel); actual CLI workflow **1/1**, three
sessions, **28.48 s**. Linux default **87/87**, devtools **98/98**, portable all-targets
check PASS. Count increments comprise automation-ID and UIA tests, eleven portable
devtools tests and five native transport/session tests; no existing tests disabled.
See [native evidence](../benchmark/results/rust-ui-devctl-v0.1/README.md) for logs,
binary identity, screenshots, snapshots and hashes.

The frozen reference tree is unchanged from `dc70f664`; all 18 canonical PNG blobs
are unchanged from approved `2e14e417`. No native Gallery, Shadcn component catalog,
MCP, web server, other-platform backend or main merge was started.

`native_probe_richedit_paints_text` parallel/DPI120 test isolation remains a separate
foundation issue. Final default/devtools observations passed, but no fix to that
pre-existing intermittent rendering test was made or claimed.

## Universal Gates 1–20

| Gate | Status | Milestone evidence |
|---|---|---|
| 1 Completeness | PASS | Full real CLI/probe interaction/capture/comparison/shutdown workflow |
| 2 Assumptions | PASS | Windows v0.1, bounded keys, keyboard focus and comparison modes documented |
| 3 Preservation | PASS | Default tests green; reference untouched; normal app instrumentation absent |
| 4 Scope | PASS | Generic devtool, probe, required IDs/UIA/docs/evidence only |
| 5 Minimal design | PASS | std CLI, one local worker, focused JSON/PNG, no framework/service |
| 6 Ownership | PASS | Retained tree, guarded backend, existing mailbox, Win32 capture/transport separation |
| 7 Reuse | PASS | Existing hit/key/focus/event, scale/space and native-entry authorities |
| 8 Fallback | PASS | WGC fails explicitly; no PrintWindow/BitBlt/global-input fallback |
| 9 Determinism | PASS | Stable snapshot ordering, explicit capture identity/tolerances, self controls |
| 10 Authority | PASS | DEVCTL/protocol/source agree; frozen reference read-only |
| 11 Failure/security | PASS | Nonce, bounded framing, cancellation/joins, admission, typed errors, owned child |
| 12 Bounds | PASS | Message/tree/depth/image/request/frame/queue budgets enforced |
| 13 Dependencies | PASS | Optional serde/serde_json/png and scoped existing Windows features |
| 14 Idle/work | PASS | Blocking waits; no tooling frame loop/polling; measured idle counters unchanged |
| 15 Infrastructure | PASS | Local transient session only; no remote/persistent daemon or CI |
| 16 Verification | PASS | Native 1.25 WGC workflow and negative controls; portable helper tests |
| 17 Recovery | PASS | Capture lifetime/DPI defects fixed at owning boundaries, no fallback added |
| 18 Identity | PASS | Final source and both binary hashes in receipt; evidence publication separate |
| 19 Honesty | PASS | Actual images inspected; exact pass counts and bounded observations, limits recorded |
| 20 Systemic audit | PASS | All commands share framing/lookup/errors, all Win APIs in platform, all captures WGC |

The implementation gate is achieved. Native Gallery work may be handed off after
the owner's review; this run ends with publication of tooling/evidence only.
