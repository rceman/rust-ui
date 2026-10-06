# rust-ui-devctl v0.1

Local, opt-in development tooling for the implement → inspect → interact →
WGC capture → compare loop. This tooling milestone precedes native Gallery;
it implements no Shadcn components, Gallery, MCP, web service or remote control.
The approved reference v0.1 remains immutable input.

## Build and feature boundary

On native Windows, from the repository root:

```powershell
cargo build --locked --features devtools --all-targets
$ctl = '.\target\debug\rust-ui-devctl.exe'
New-Item -ItemType Directory -Force .devctl | Out-Null
& $ctl launch --exe .\target\debug\examples\devctl_probe.exe --session .devctl\session.json --json
```

`devtools` enables the CLI, protocol/comparison helpers and Windows instrumentation.
Without it, there is no listener, session, capture, extra thread, network socket or
periodic tooling work. Even with the feature, an ordinary `App::run()` creates no
listener unless `launch` supplied the session credentials. An instrumented session
has one pipe worker, blocking on overlapped connection/I/O; no polling at idle.
The existing mailbox event wakes the existing guarded UI turn.

Automation IDs are ordinary optional node/accessibility metadata. Leaf builders
and actions accept `.automation_id("probe.button")`. Containers use:

```rust
ui.named("probe.surface", |ui| {
    ui.surface(Surface::new(), |ui| {
        ui.button("Count").automation_id("probe.button").on_press(|| Msg::Click);
    });
});
```

`named` requires exactly one staged child. IDs are semantic strings, not labels,
NodeIds, indices or paths. Keep them stable across keyed reconciliation. Empty,
control-bearing, oversized (>256 UTF-8 bytes) or duplicate IDs abort the staged
transaction in test/devtools builds before native mutation. IDs flow to retained
tree inspection, comparisons and Windows UIA AutomationId; devctl does not crawl UIA.

## Native agent workflow

```powershell
& $ctl wait-idle --session .devctl\session.json --json
& $ctl tree --session .devctl\session.json --json
& $ctl rect probe.button --session .devctl\session.json --json
& $ctl hover probe.button --session .devctl\session.json --json
& $ctl wait-idle --session .devctl\session.json --json
& $ctl screenshot --id probe.button --session .devctl\session.json --out .devctl\hover.png --json
& $ctl press probe.button --session .devctl\session.json --json
& $ctl screenshot --id probe.button --session .devctl\session.json --out .devctl\pressed.png --json
& $ctl release probe.button --session .devctl\session.json --json
& $ctl click probe.button --session .devctl\session.json --json
& $ctl focus probe.button --modality keyboard --session .devctl\session.json --json
& $ctl key Enter --session .devctl\session.json --json
& $ctl snapshot-layout --session .devctl\session.json --out .devctl\native.json --json
& $ctl screenshot --session .devctl\session.json --out .devctl\client.png --json
& $ctl compare-layout --reference .devctl\native.json --snapshot .devctl\native.json --json
& $ctl compare-image --reference-image .devctl\client.png --native-image .devctl\client.png --diff .devctl\self-diff.png --json
& $ctl compare-element --reference .devctl\native.json --snapshot .devctl\native.json --id probe.button --reference-image .devctl\client.png --native-image .devctl\client.png --out-dir .devctl\button --json
& $ctl shutdown --session .devctl\session.json --json
```

Launch accepts application arguments following `--`. Discovery returns opaque
session ID, owned child PID, HWND diagnostic identity, endpoint, protocol, scale
and client sizes. A private session file carries a separate 256-bit token.
Launching inherits no handles, so a probe cannot retain the CLI JSON output pipe.
One process/window per session is supported. Explicit shutdown waits for that
process to exit and deletes the receipt. After a crash, `cleanup --session ...`
removes a stale receipt only if its process is gone; it never kills another PID.

`hover`, `press`, `release` and `click` resolve the ID to a currently visible,
enabled hit owner, then use the real backend pointer/state/event machinery at
its center. No physical mouse movement or appearance mutation occurs. Press
persists until release. Focus currently supports keyboard modality; the current
foundation's focus ring follows focus (no new separate modality model is invented).

Supported keys: `Enter Space Tab Escape Left Up Right Down Backspace Delete Home
End`. Key down/up use the backend keyboard route, including native text handling.
OS modifier state applies, as it does to real input; release physical modifiers
for deterministic unmodified actions. Character typing, modifier chords, IME
synthesis, scroll-into-view and hidden targets are outside v0.1. Unsupported keys
fail explicitly. Input editing remains native RichEdit, with the application's
normal TextEdit acknowledgement. The probe's input label in a snapshot is the
retained committed TextValue, not an independent native read.

## Idle and presentation

`wait-idle` is an observed barrier, not a sleep. After a guarded UI turn and its
reentrant drain it requires: no queued mailbox/app events, deferred native work,
interaction dirty state, layout/paint dirty nodes, damage, invalidated client
paint, same-thread pump continuation or scheduled framework UI deadline. Then
`DwmFlush` completes the presentation barrier. It adds no timer or polling.
Every existing native dispatch may complete the pending barrier.

This is quiescence for work already admitted. It does not promise that a future
worker delivery or user input cannot change the UI afterward; future tasks and
OS caret blink are not frozen. Continuously animating applications can time out.
The timeout carries the last observed queue/dirty/deadline diagnostics. Default
client command deadline is 5000 ms (`--timeout 1..30000`); the UI budget reserves
250 ms for response delivery. Requests not yet dispatched when expired are skipped.
A timeout during an already executing mutation does not roll it back: inspect
state before retrying a non-idempotent action.

## Coordinates, snapshots and WGC

Snapshot schema: `rust-ui.devctl.snapshot-layout/0.1`. Tree order is retained child
order, not slot order. It contains client logical/physical dimensions, scale and
nodes with optional automation_id, kind, label, rect_dp, rect_px, visible, enabled,
focused, focus_visible, hovered, pressed, children. Labels are diagnostic prefixes
limited to 1024 characters. No pointer, timestamp, session nonce or NodeId is
included. Unchanged state serializes deterministically. Tree bounds: 8192 nodes,
128 levels, 4 MiB encoded response.

`ScaleFactor` means physical pixels per rust-ui logical unit. Windows derives it
through its existing dpi/96 adapter (96/120/144/192 → 1/1.25/1.5/2).
Physical rectangle edges use the shared rounding authority independently.
`rect_dp` and `rect_px` are **client-relative**, never screen/non-client coordinates.

Screenshot authority is exclusively **Windows Graphics Capture**. The CLI enables
PMv2 coordinates for capture, measures client origin through the existing Win32
space adapter, and compares the WGC item extent with DWM extended frame bounds.
It crops measured physical client bounds; a mismatch or geometry change fails.
There is no PrintWindow, BitBlt or other fallback. WinRT stays initialized for the
CLI process lifetime; frame/session/pool/D3D resources are released per capture.

Screenshot waits idle itself. `--id ID` crops the physical semantic rectangle;
`--pad 0..256` adds physical pixels, requiring the entire padded rectangle to be
visible. Partial, hidden or off-client targets fail `TARGET_NOT_VISIBLE`.
PNG output has a sibling `.capture.json` with backend, measured frame/client crop,
scale and sizes; there is no timestamp. PNG/JSON are published only after complete
writes. Capture can fail on unsupported Windows/session/GPU environments; it
returns `CAPTURE_FAILED`, never substitutes a different authority.

## Comparisons and frozen reference adapter

Comparators run offline, including on Linux without a Linux UI backend:

```powershell
& $ctl compare-layout --reference reference\shadcn-gallery-v0.1\contract.json --capture components/default --snapshot .devctl\native.json --id button.default --mode size-only --tolerance 0.25 --json
& $ctl compare-element --reference reference\shadcn-gallery-v0.1\contract.json --capture components/default --id button.default --snapshot .devctl\native.json --reference-image reference\shadcn-gallery-v0.1\screenshots\light\components.png --native-image .devctl\client.png --mode size-only --tolerance 0.25 --channel-tolerance 2 --out-dir .devctl\button --json
```

Use native IDs that match the frozen reference when implementing components.
The calibration probe deliberately has generic probe IDs and no Shadcn styling.
The read-only adapter consumes `contract/0.2` capture keys (`page/state`) directly;
there is no manually duplicated contract. Geometry is theme independent; choose
Light/Dark with the reference-image path. Full-document reference images need no
matching 14,000px native window: compare a visible semantic element.

Layout inputs can also both be native snapshots for calibration. Modes:
`absolute` (x/y/width/height) or `size-only` (width/height), plus visibility/presence.
No implicit relative alignment is applied. Output has per-ID expected/actual/delta,
explicit Dp tolerance and matched/missing/unexpected/failed summary. `--id` restricts
the comparison; unrelated gallery elements do not become missing targets.

Image comparison reports exact dimensions, mismatched pixels/percentage, max
channel delta, mean absolute channel error and a difference bounding box.
`--channel-tolerance 0..255` is explicit (default zero); raw channel statistics
are independent of that threshold. Diff is deterministic: red = amplified channel
error, magenta = a missing pixel on unequal extents. Statistics use a union canvas
with transparent absent samples; dimensions always fail when unequal. It never
claims perceptual equivalence or resizes glyphs to fake parity.

`compare-element` writes `reference.png`, `native.png`, `diff.png`, `comparison.json`.
Each image is cropped independently at its own authoritative rectangle. Reference
HTML is DPR1 CSS pixels; native is current host physical pixels. At 1.25 these can
legitimately differ: report the size mismatch, do not silently resample. A native
snapshot used as the reference uses its own rect_px for reference-image cropping.
Padding is physical pixels per image. Maximum decoded/diff image: 32 Mi pixels,
enough for the frozen v0.1 All page. Failed comparisons exit 6 with the full report.
Never place native/diff outputs into the frozen reference directory.

## Machine output and security

All commands accept `--json`; output is JSON even without it. Stdout contains only
one `{ok,result}` or `{ok,error}` document. Diagnostics use stderr. No prompts,
ANSI output, global SendInput, process-title scraping, arbitrary PID attach or
runtime shell/filesystem commands exist. Launch executes the explicitly supplied
local executable, without a shell. Store credentials in a user-private directory.
The pipe is local-only, session scoped, rejects remote clients and authenticates
version + independent token. This is not a security boundary against the same
Windows user or administrators with access to the receipt/process environment.

Exit codes: 0 success; 2 invalid CLI arguments; 3 missing session/target or target
not visible; 4 timeout; 5 capture failure; 6 comparison mismatch; 7 protocol,
unsupported operation, I/O or runtime error. See [protocol](DEVCTL_PROTOCOL.md)
for stable error codes and framing. Unknown/duplicate options fail instead of being
silently ignored. Output paths are local CLI inputs, not remote protocol commands.

## Verification and dependencies

```powershell
cargo fmt --check
cargo check --locked --all-targets
cargo test --locked --lib
cargo build --locked --features devtools --all-targets
cargo test --locked --features devtools --lib -- --test-threads=1
cargo test --locked --features devtools --test devctl_windows -- --nocapture
```

The Windows integration test uses the actual CLI, owned process, pipe, runtime and
WGC. It tests semantic effects, pressed/hover crops, committed native keyboard
edit, deterministic snapshots, comparator negative controls, authentication,
stale receipt cleanup, target handle counts and three complete shutdown cycles.
Build the probe before running that test. Portable helpers and semantic-ID tests
also run with `cargo test --locked --features devtools --lib` on Linux.

Direct additions are optional `serde` (derive/std), `serde_json` (std JSON) and
`png 0.17` (default features disabled). JSON libraries own framing data correctness;
a focused PNG codec avoids a second WIC/COM encoding/decoding lifecycle. The lock
pins their transitive compression/parser dependencies. Windows features are opt-in
Capture, Foundation events, D3D11/DirectX/DXGI, WinRT interop, DWM, pipes/overlapped
I/O/FileSystem and BCrypt. No CLI framework, async runtime, HTTP/image framework,
logging framework or CI was added. See current evidence for exact executed results.

The existing `native_probe_richedit_paints_text` parallel/DPI120 isolation defect
remains a separate foundation-hardening item. No related rendering/test change was
made here. Record a default parallel observation separately from serial results.
