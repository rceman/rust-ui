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
& $ctl launch --exe .\target\debug\rust-ui-gallery.exe --session .devctl\session.json --json
```

`devtools` enables the CLI, protocol/comparison helpers and Windows instrumentation.
Without it, there is no listener, session, capture, extra thread, network socket or
periodic tooling work. Even with the feature, an ordinary `App::run()` creates no
listener unless `launch` supplied the session credentials. An instrumented session
has one pipe worker, blocking on overlapped connection/I/O; no polling at idle.
The existing mailbox event wakes the existing guarded UI turn.

Automation IDs are ordinary optional node/accessibility metadata. Leaf builders
and actions accept `.automation_id("button.default")`. Containers use:

```rust
ui.named("card.default", |ui| {
    ui.surface(Surface::new(), |ui| {
        ui.button("Default").automation_id("button.default").on_press(|| Msg::Invoke);
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
& $ctl rect button.default --session .devctl\session.json --json
& $ctl hover button.default --session .devctl\session.json --json
& $ctl wait-idle --session .devctl\session.json --json
& $ctl screenshot --id button.default --session .devctl\session.json --out .devctl\hover.png --json
& $ctl press button.default --session .devctl\session.json --json
& $ctl screenshot --id button.default --session .devctl\session.json --out .devctl\pressed.png --json
& $ctl release button.default --session .devctl\session.json --json
& $ctl click button.default --session .devctl\session.json --json
& $ctl focus button.default --modality keyboard --session .devctl\session.json --json
& $ctl key Enter --session .devctl\session.json --json
& $ctl snapshot-layout --session .devctl\session.json --out .devctl\native.json --json
& $ctl screenshot --session .devctl\session.json --out .devctl\client.png --json
& $ctl compare-layout --reference .devctl\native.json --snapshot .devctl\native.json --json
& $ctl compare-image --reference-image .devctl\client.png --native-image .devctl\client.png --diff .devctl\self-diff.png --json
& $ctl compare-element --reference .devctl\native.json --snapshot .devctl\native.json --id button.default --reference-image .devctl\client.png --native-image .devctl\client.png --out-dir .devctl\button --json
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
persists until release. `leave` takes no target: it runs the backend's
WM_MOUSELEAVE path — pointer left the window, hot state clears. Focus
currently supports keyboard modality; the current
foundation's focus ring follows focus (no new separate modality model is invented).

Supported keys: `Enter Space Tab Escape Left Up Right Down Backspace Delete Home
End`. Key down/up use the backend keyboard route, including native text handling.
OS modifier state applies, as it does to real input; release physical modifiers
for deterministic unmodified actions. Character typing, modifier chords, IME
synthesis and hidden targets are outside v0.1. Unsupported keys
fail explicitly. Input editing remains native RichEdit, with the application's
normal TextEdit acknowledgement. An editor's label in a snapshot is the
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
client RPC deadline is 5000 ms (`--timeout 1..30000`); the UI budget reserves
250 ms for response delivery. Requests not yet dispatched when expired are skipped.
A timeout during an already executing mutation does not roll it back: inspect
state before retrying a non-idempotent action.

## Coordinates, snapshots and WGC

Snapshot schema: `rust-ui.devctl.snapshot-layout/0.2` (checked with strict
equality; 0.1 documents are rejected). Tree order is retained child
order, not slot order. It contains client logical/physical dimensions, scale and
nodes with optional automation_id, kind, label, rect_dp, rect_px, visible, enabled,
focused, focus_visible, hovered, pressed, parts, children. `parts` is additive and
omitted when empty: each part is `{key, rect_dp, rect_px, visible, reason?}` with
rects in the same client-relative absolute space as `rect_dp`, addressed as
`owner#part` (a `SemanticKey`, never a flattened fake automation id). The v0.1
runtime emits no parts — no compound components exist yet; synthetic snapshots
exercise the mechanism. Duplicate `(owner, part)` keys or parts on a node without
an automation id fail `INVALID_ARGUMENT`. Labels are diagnostic prefixes
limited to 1024 characters. No pointer, timestamp, session nonce or NodeId is
included. Unchanged state serializes deterministically. Tree bounds: 8192 nodes,
128 levels, 4 MiB encoded response.

`ScaleFactor` means physical pixels per rust-ui logical unit. Windows derives it
through its existing dpi/96 adapter (96/120/144/192 → 1/1.25/1.5/2).
Physical rectangle edges use the shared rounding authority independently.
`rect_dp` and `rect_px` are **client-relative**, never screen/non-client coordinates.

**One snap authority.** Every devctl logical→physical conversion goes through
`devtools::snap_rect`, which delegates each *edge* to
`crate::geom::Rect::physical`/`ScaleFactor` (round half away from zero; width is
`right-left`, never `round(width*scale)`), casting f64→f32 exactly once and
rejecting non-finite/non-positive scales and unrepresentable edges. No other
rounding exists in devtools or the CLI: `Image::crop` takes an already-snapped
integer `PixelRect`, and `--pad N` in `compare-element` grows the *logical* rect
by N Dp on each side *before* snapping (N comparison pixels at scale 1.0).
The retired floor-origin/ceil-edge policy disagreed with the authority on
421 of 581 frozen element-capture records (999 of 1387 including visible
parts), e.g. `button.default` at y=300.75 h=32 snapped to 301..333 = 32 px.
`screenshot --id --pad` keeps its existing meaning: physical pixels grown on
the already-snapped `rect_px`.

Screenshot authority is exclusively **Windows Graphics Capture**. The CLI enables
PMv2 coordinates for capture, measures client origin through the existing Win32
space adapter, and compares the WGC item extent with DWM extended frame bounds.
It crops measured physical client bounds; a mismatch or geometry change fails.
There is no PrintWindow, BitBlt or other fallback. WinRT stays initialized for the
CLI process lifetime; frame/session/pool/D3D resources are released per capture.

Screenshot waits idle itself and checks unchanged retained semantic geometry/state
before/after WGC, and rejects an intervening successfully painted frame (including
a semantic state changing and changing back). Native caret/preedit pixels are not
part of the retained semantic
scene check. A changed scene fails instead of producing a stale element crop.
Each RPC uses the requested deadline; the composite screenshot additionally has
a five-second WGC frame bound and bounded PNG work.
Screenshot `--id ID` crops the physical semantic rectangle;
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
& $ctl compare-element --reference reference\shadcn-gallery-v0.1\contract.json --capture components/default --id button.default --native-id button.default --snapshot .devctl\native.json --reference-image reference\shadcn-gallery-v0.1\screenshots\light\components.png --native-image .devctl\client.png --mode size-only --tolerance 0.25 --channel-tolerance 2 --out-dir .devctl\button --json
```

Use native IDs that match the frozen reference when implementing components;
Gallery specimens intentionally carry the contract's automation ids.
The read-only adapter consumes `contract/0.2` capture keys (`page/state`) directly;
there is no manually duplicated contract. Geometry is theme independent; choose
Light/Dark with the reference-image path. Full-document reference images need no
matching 14,000px native window: compare a visible semantic element.

**Targets and tier scope.** The adapter emits one target per element
(`owner`) and one per capture part (`owner#part`); part x/y are relative to
the owner's rect and are exposed absolute. Hidden parts
(`{"visible":false,"reason":"not-rendered"}`) are explicit targets with no
rect: they pass when the native part is absent or not visible, and fail with
their reason when a native part renders. Visible parts compare rect like
elements. Targets carry the element tier (`core`/`later`/`reference-only`);
unknown tiers fail `INVALID_ARGUMENT`. `--tier` selects the scope (`all` or a
comma-separated subset, default `core`): out-of-scope targets are excluded
from missing accounting but counted under `scope.excluded_by_tier`, and
native ids matching them list under `out_of_scope`, not `unexpected`. An
explicit `--id` (`owner` or `owner#part`) bypasses the tier filter and still
reports `tier` per row. `--native-id` lets `compare-element` bind a reference
id to a differently named native element (no fuzzy matching).

Layout inputs can also both be native snapshots for calibration. Modes:
`absolute` (x/y/width/height) or `size-only` (width/height), plus visibility/presence.
No implicit relative alignment is applied. Output has per-ID expected/actual/delta,
explicit Dp tolerance and matched/missing/unexpected/out_of_scope/failed summary.
`--id` restricts the comparison; unrelated gallery elements do not become
missing targets.

**Two raster spaces.** `RAW_NATIVE_CAPTURE` is the unaltered WGC raster at
the native `scale_factor`, saved as evidence and never overwritten by
normalized data. `CANONICAL_COMPARISON_RASTER` is 1.0 comparison pixel per
logical Dp (aligned with the frozen DPR1 HTML), derived deterministically by
resampler `area-box/1`: for each canonical pixel (i,j) of that side's own
canonical rect `snap_rect(element_logical_rect.grow(pad), 1.0)` — the
reference and the native element each have their own canonical lattice, so a
position difference never mis-samples the native raster — the source box
`[s*(C.left+i), s*(C.left+i+1)) × [s*(C.top+j), s*(C.top+j+1))` in the raw
raster is averaged with exact fractional-coverage area weights (separable,
straight sRGB-encoded RGBA channels, f64 row-major accumulation, round half
up, clamp 0..255). Both canonical rasters align at their own element's
top-left; a logical size difference then shows as honest missing pixels on
the union canvas, never as scale noise. `source = canonical * source_scale`; no
resize-until-dimensions-match exists. At s==1.0 the output is bit-identical
to a plain crop. A footprint reaching outside the raw capture fails
`TARGET_NOT_VISIBLE` (no edge clamping). `compare-image` is unchanged: a
same-lattice raw compare of whole images with no crop or canonicalization.
Image comparison reports exact dimensions, mismatched pixels/percentage, max
channel delta, mean absolute channel error and a difference bounding box.
`--channel-tolerance 0..255` is explicit (default zero); raw channel statistics
are independent of that threshold. Diff is deterministic: red = amplified channel
error, magenta = a missing pixel on unequal extents. Statistics use a union canvas
with transparent absent samples; dimensions always fail when unequal. It never
claims perceptual equivalence.

`compare-element` writes `reference.png`, `native-raw.png`,
`native-canonical.png`, `diff.png`, `comparison.json` (schema
`rust-ui.devctl.compare-element/0.2`). `--native-image` must be a FULL raw WGC
client capture whose dimensions equal the snapshot `client_px` (else
`INVALID_ARGUMENT`); `--reference-image` for a contract reference must equal
the contract `pages[].document` of the capture's page, and for a snapshot
reference must equal that snapshot's `client_px` (its `scale_factor` then
drives the same canonicalization, identity at 1.0). `diff.png` and pixel
metrics compare the reference canonical raster against the native CANONICAL
raster — the actionable signal — while `native-raw.png` stays unaltered.
`comparison.json` fields: `schema`; `id` (reference key) / `native_id`;
`pass` (geometry row AND canonical-visual pass; `false` still exits 6 with
the full report); `scope {tiers, tier_of_target}`; `geometry` (the
`compare-layout` row for this target — the contract/snapshot authority);
`raster` — `reference` `{file, scale_px_per_dp, crop_rect_px,
canonical_rect_dp, dimensions}`, `native_raw` `{file, scale_px_per_dp,
crop_rect_px, dimensions}`, `native_canonical` `{file, scale_px_per_dp: 1.0,
canonical_rect_dp, dimensions}`;
`normalization {source_scale, target_scale, source_dimensions,
canonical_dimensions, resampler:"area-box/1", identity,
transform {mapping, canonical_rect_dp, source_footprint_px}}` — the native
canonical lattice and its exact f64 footprint in source px; `visual_canonical`
(compare-image metrics, canonical space); `visual_raw` (`computed:false` with
the cross-scale reason, or real metrics when both sides share a scale);
`evidence_classes` — `geometry` and `whole_component_raster` are `measured`;
`flat_fills`, `borders`, `radii`, `icons`, `text_bounds_baseline` and
`glyph_raster` are `not_measured` (Chromium and DirectWrite/D2D glyph pixels
are not expected to be byte-identical; geometry is contract authority, the
canonical raster is a visual signal); `padding_comparison_px`. Maximum
decoded/diff image: 32 Mi pixels, enough for the frozen v0.1 All page.
Failed comparisons exit 6 with the full report.
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
WGC against `examples/gallery` — rust-ui's real native UI acceptance application,
reached through the `rust-ui-gallery` [[bin]] alias (the same compilation unit;
`CARGO_BIN_EXE_` covers bin targets only). It tests semantic effects, pressed/hover
crops, committed native keyboard edit, deterministic snapshots, comparator
negative controls, authentication, stale receipt cleanup, target handle counts
and three complete shutdown cycles. No synthetic devctl UI exists; the launch
target builds automatically and reaches the test through `CARGO_BIN_EXE_`.
Portable helpers and semantic-ID tests
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
