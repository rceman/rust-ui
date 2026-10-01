# Independent Windows platform/foundation follow-up review

**Reviewed candidate:** `cd150e4ebe120b21fbd4517fb6efcfc8360e69d4`  
**Repository / branch:** `rceman/rust-ui` / `agent/windows-native-composer-contract-spike-v0.1-swe2`  
**Approved architecture baseline:** `29c67025dc88221899a8ab1b381f08491d44b85b`  
**Previous review:** `docs/SPIKE_IMPLEMENTATION_REVIEW.md`, reviewing `5a93faf647ff8d7e2eaaca16f7ffd816fcc9884a`  
**Review date:** 2026-10-01

## Decision

The shared semantic contract direction is sound, and the rework contains substantial real corrections. The frozen candidate still has material implementation and verification defects. It is not ready for foundation freeze, merge clearance, or the post-foundation Shadcn Component Gallery milestone.

The architecture does not need replacement. The required work is a bounded correction of existing authorities, native lifecycle paths, and evidence tooling. Passing Windows tests would not override the defects below.

## Review method and limits

The candidate was inspected in an isolated detached worktree at the exact SHA above. All source links in this report are pinned to that SHA. Production implementation, tests, and canonical documents were not modified. This review document is the only repository file added by this review; no commit, push, merge, or gallery/application implementation was performed.

Read: `AGENTS.md`, `QUALITY_GATES.md`, `PLATFORM_CONTRACTS.md`, `BACKEND_ARCHITECTURE.md`, `NATIVE_CONTROL_BOUNDARY.md`, `PROGRAMMING_MODEL.md`, `STATE_AND_EVENTS.md`, `STYLE_CUSTOMIZATION_MODEL.md`, both spike review/deviation documents, the previous implementation review, composer/native-probe consumers, and the evidence bundle. Inspected the retained runtime, event/task/style/geometry code and the Win32 window, text, layout, rendering, input and accessibility paths.

This reviewer ran on Linux, not the designated native Windows checkout `W:\devin_folder\rust-ui`. No fresh Windows execution, IME session, physical DPI move, or COM teardown measurement is claimed. Repository evidence is assessed separately from independently reproduced checks.

| Independent check | Result |
|---|---|
| `cargo fmt --check` | PASS |
| `cargo check --locked --all-targets --target x86_64-pc-windows-gnu` | PASS, with warnings; compilation only |
| `cargo check --locked --examples` on Linux | PASS, with warnings |
| `cargo test --locked --lib` on Linux | FAIL to compile: four unresolved references to Windows-gated portable helpers |
| Standalone probe importing the candidate's actual `src/geom.rs` | Confirms pointer/client conversion disagreement: local pixel `1` versus `0` |
| Windows `81/81` library tests | Reported, not independently reproduced |
| Representative actual rendered output | Inspected `final-foundation.png`; it contradicts the accompanying visual claim |

There are 81 `#[test]` declarations in `src/tests.rs`; this is consistent with the reported Windows count. It does not establish that the final identified binary passed them.

Severity meanings: **BLOCKER** prevents safe foundation use; **MAJOR** is a material contract or proof defect requiring correction before freeze; **MINOR** is a bounded documentation correction; **NOTE** records an accepted limitation or positive result. Finding IDs below are references, not another numbered gate system.

## Prior-review disposition

| Prior area | Status | Rechecked result |
|---|---|---|
| WndProc/native reentrancy | PARTIALLY_RESOLVED | Guard now precedes the WndProc mutable reference, but initial native entry remains unguarded and reentrant pointer messages are reposted. F01. |
| Native dispatch ordering/results | PARTIALLY_RESOLVED | Peer FIFO, capacity, generation checks and raw HRESULT/LRESULT capture added. The outer repost path and ignored errors still defeat the complete contract. F01/F05. |
| RichEdit geometry/rendering | PARTIALLY_RESOLVED | Direct physical-pixel `TxDrawD2D` replaces the bitmap workaround; resize and scale relatching added. Origin-rounding inconsistency and contradictory final capture remain. F03/F04/F12. |
| HWND repair / initial layout | RESOLVED | Layout and paint are HWND-gated; mounted peers receive the valid handle before the explicit initial turn. This is a reasonable private lifecycle detail. Dispatch ownership remains a separate defect. |
| IME/input | PARTIALLY_RESOLVED | IME boundaries reach RichEdit and native key interpretation precedes submit. Real kana evidence exists. Preedit, scale-change composition, modifiers, drag and submit contracts remain insufficiently established. F04/F05/F11. |
| Native timers | PARTIALLY_RESOLVED | Namespace routing and NodeId ownership/pruning repaired. Failure, periodicity and ID reuse remain defective. F06. |
| UIA generation/liveness | PARTIALLY_RESOLVED | Generation-bearing native registry, stable live child state and normal-close disconnection added. Native patterns escape the fence; geometry/notification and teardown paths remain incomplete. F07. |
| Focus/hit ownership | PARTIALLY_RESOLVED | Disabled eligibility, focus/capture sanitation and enclosing Action ownership improved. CustomRender hit geometry is still ignored, and input normalization is incomplete. F05. |
| Style guarantees | PARTIALLY_RESOLVED | Capability-limited editor patch, numeric preflight, proportional radii, merge fixes and independent focus ring added. Concrete equality, active-state layout and OS enforcement are incomplete. F08. |
| Shadow/resource bounds | PARTIALLY_RESOLVED | Cache limits and target-generation invalidation added. Zero-blur/offset masks, checked extents and old/new damage are still incorrect. F09. |
| Resource lifetime/leaks | PARTIALLY_RESOLVED | Normal-path mask release and bounded format cache repaired. Native failure-path leaks and real teardown proof remain. F07/F09/F12. |
| Test portability | PARTIALLY_RESOLVED | Native UIA tests now guarded. Existing portable helper failure remains, with another portable helper newly Windows-gated. F10. |
| Evidence integrity | PARTIALLY_RESOLVED | Finite UIA labels/rectangles and explicit NOT RUN limits improve honesty. Identity, API/deviation descriptions, visual claim and harness consistency remain material defects. F11/F12/F13. |

No prior native acceptance result is labeled independently revalidated by this Linux review.

## Significant findings

### F01 — BLOCKER: native entry and deferred delivery are not a complete safety contract

The new [WndProc guard](https://github.com/rceman/rust-ui/blob/cd150e4ebe120b21fbd4517fb6efcfc8360e69d4/src/platform/win32/window.rs#L67) fixes the original unconditional second mutable reference during ordinary dispatch. Two paths still violate its intended invariant:

- The [reentrant branch](https://github.com/rceman/rust-ui/blob/cd150e4ebe120b21fbd4517fb6efcfc8360e69d4/src/platform/win32/window.rs#L105) reposts `WM_DPICHANGED` with the original borrowed `RECT*` LPARAM. This is not a copied scalar payload. An asynchronous pointer-bearing system message is forbidden; its failure is ignored. Microsoft's [PostMessageW contract](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-postmessagew) explicitly excludes pointer parameters for asynchronous messages below `WM_USER`.
- The [explicit initial `backend.turn()` and error-path `DestroyWindow`](https://github.com/rceman/rust-ui/blob/cd150e4ebe120b21fbd4517fb6efcfc8360e69d4/src/platform/win32/mod.rs#L2054) execute while a mutable backend operation is active without setting `in_dispatch`. A synchronous native callback can therefore bypass the guard and construct another mutable backend reference. `in_turn` does not guard reference construction.

The scalar repost path also forms a second delivery mechanism. It appends nested messages behind already queued input and resolves the focused peer at redelivery rather than capturing the original NodeId. A delayed focus/key/character message can therefore acquire a different owner. FIFO within the backend's peer queue does not prove ordering across this outer window queue.

[`deliver_native` and its bounded FIFO](https://github.com/rceman/rust-ui/blob/cd150e4ebe120b21fbd4517fb6efcfc8360e69d4/src/platform/win32/mod.rs#L554) are real improvements. However, `send_focused`, deferred draining, character, pointer, hover, focus and timer paths discard delivery errors. Queue overflow is typed at insertion and then silently lost at many callers. Successful HRESULT/LRESULT capture likewise does not establish universal fallback/result semantics when most routes discard the outcome.

**Required correction:** one ownership guard covering every native-entering backend operation; message-class rules distinguishing owned payloads, synchronous results and coalescible work; preserved target identity and defined ordering; checked posting/delivery errors; and an explicit continuation when a bounded drain leaves work. Teardown must run full cleanup after a reentrant `WM_NCDESTROY`, not only detach and quit. Add focused nested-callback, focus-change, pointer-payload, overflow and failure-path proof.

### F02 — MAJOR: the shared scale contract still defines Windows DPI semantics

[`ScaleFactor`](https://github.com/rceman/rust-ui/blob/cd150e4ebe120b21fbd4517fb6efcfc8360e69d4/src/geom.rs#L126) is publicly described as `dpi / 96` with an allegedly universal 96-DPI baseline. It exports `from_dpi` and `dpi`; [PLATFORM_CONTRACTS](https://github.com/rceman/rust-ui/blob/cd150e4ebe120b21fbd4517fb6efcfc8360e69d4/docs/PLATFORM_CONTRACTS.md#L37) defines Dp as `1/96 inch` and mandates that DPI conversion live in shared geometry.

The underlying multiplication/division is reusable. `ScaleFactor(2.0)` already permits a backend to supply a ratio directly, so macOS implementation is not mathematically impossible. The normative public meaning nevertheless makes Windows' logical-DPI convention the shared semantic definition. A backing scale is not physical display density. Apple's [coordinate/backing guidance](https://developer.apple.com/library/archive/documentation/GraphicsAnimation/Conceptual/HighResolutionOSX/APIs/APIs.html) distinguishes points, backing pixels and per-object native conversions, and does not define backing scale as physical DPI.

**Required correction:** define ScaleFactor as positive finite physical pixels per rust-ui logical unit. Keep Windows DPI-to-ratio and ratio-to-D2D-DPI conversion, including `96`, in the Win32 authority. Describe Dp as a logical layout unit, without a physical-inch promise. Shared conformance tests should test ratios and rounding; Windows adapter tests should test Windows DPI mapping. A macOS backend can then supply its native backing ratio without inventing DPI values. No new trait or dispatch framework is needed.

### F03 — MAJOR: typed geometry does not yet establish one peer-origin transform

The distinction between logical f32 geometry and physical integer edges is useful. Rect edges round independently, and signed native mouse coordinates are decoded. Client/screen marker wrappers improve some seams. The remaining bypasses are consequential:

- [Pointer conversion](https://github.com/rceman/rust-ui/blob/cd150e4ebe120b21fbd4517fb6efcfc8360e69d4/src/platform/win32/mod.rs#L1180) converts client pixels to logical coordinates, subtracts the logical content origin and rounds again. [Text client/screen callbacks](https://github.com/rceman/rust-ui/blob/cd150e4ebe120b21fbd4517fb6efcfc8360e69d4/src/platform/win32/text.rs#L389) and caret placement add/subtract the separately rounded physical origin. These are different rules.
- Reproduction using the actual `geom.rs`: scale `1.25`, logical origin `0.4`, client pixel `1`. Pointer conversion yields `round((1/1.25 - 0.4)*1.25) = 1`; subtracting the snapped origin yields `1 - round(0.4*1.25) = 0`. Half-away rounding is not translation invariant. This is a confirmed one-pixel disagreement, not a speculative future optimization.
- No distinct peer-local coordinate type prevents substituting a window-client point. Public raw fields and unmarked PhysicalPoint remain easy escape paths.
- [UIA construction](https://github.com/rceman/rust-ui/blob/cd150e4ebe120b21fbd4517fb6efcfc8360e69d4/src/platform/win32/mod.rs#L1604) and [probe clicking](https://github.com/rceman/rust-ui/blob/cd150e4ebe120b21fbd4517fb6efcfc8360e69d4/examples/native_probe.rs#L91) independently perform client/screen conversion instead of using the Win32 authority. The authority's [conversion wrappers](https://github.com/rceman/rust-ui/blob/cd150e4ebe120b21fbd4517fb6efcfc8360e69d4/src/platform/win32/space.rs#L81) ignore native failure and return apparently valid coordinates.

**Required correction:** one typed window-client ↔ peer-local transform using an explicitly snapped origin, consumed by pointer, caret, host callbacks and native tests. Route screen conversion through one checked adapter. Define representability/failure behavior for invalid scales, out-of-range conversions and packed signed-16-bit LPARAM coordinates. Test fractional and negative origins, edge ties, scale transitions and rect edges.

The documented HIMETRIC conversion in `TxGetExtent` is a justified Win32 API exception. Fractional raster conversions are also justified. Neither permits independent origin or screen-space semantics in consumers. The documentation's claim that all platforms' native accessibility/display coordinates are physical pixels must also be narrowed to the Windows realization; native coordinate units belong to each adapter.

### F04 — MAJOR: scale relatching is a rendering correction with unproven native-state preservation

[`apply_bounds`](https://github.com/rceman/rust-ui/blob/cd150e4ebe120b21fbd4517fb6efcfc8360e69d4/src/platform/win32/text.rs#L975) retains the same ITextServices instance and reactivates it when scale changes. This is a plausible response to latched physical view state. It does not resend text, which is positive.

However, it updates host bounds/scale **before** deactivation, unconditionally performs UI activation on every measured peer, ignores activation results and marks the peer activated regardless of success. There is no explicit distinction between in-place activation and UI/focus ownership. Microsoft's [in-place deactivation contract](https://learn.microsoft.com/en-us/windows/win32/api/textserv/nf-textserv-itextservices-ontxinplacedeactivate) distinguishes those states and allows only one UI-active control at a time.

The [live-scale regression leg](https://github.com/rceman/rust-ui/blob/cd150e4ebe120b21fbd4517fb6efcfc8360e69d4/src/tests.rs#L3332) checks ink in the new island and no stray pixels. It does not inspect committed text, directional selection, caret, composition, undo or focus before/after relatching. Retaining the COM instance is insufficient proof that deactivation preserves those states. This review does not claim a reproduced composition-loss incident; the sensitive native transition is unproven.

[`natural_size`](https://github.com/rceman/rust-ui/blob/cd150e4ebe120b21fbd4517fb6efcfc8360e69d4/src/platform/win32/text.rs#L943) still uses a 4000-unit scratch height, ignores EM_REQUESTRESIZE failure and accepts the previously stored natural extent without proving a fresh resize notification. A bounded scratch rectangle is reasonable, but stale/failed measurement must not appear successful. Tall multiline growth remains explicitly unvalidated.

**Required correction:** explicit activation/focus ownership, checked activation and fresh measurement results, a safe old-to-new coordinate transition, and native tests preserving text, selection, caret, undo and focused/unfocused ownership. Exercise active composition during scale change or establish a documented implementation strategy that preserves it. Validate multiline growth, its max-lines cap and scrolling beyond the scratch-format scenario.

### F05 — MAJOR: shared input and hit semantics are still incompletely realized

IME start/end are now forwarded and key interpretation precedes submit. These repairs are real. The following implementation paths still disagree with shared semantics:

- [Semantic pointer/key events](https://github.com/rceman/rust-ui/blob/cd150e4ebe120b21fbd4517fb6efcfc8360e69d4/src/platform/win32/mod.rs#L1338) use `Modifiers::default()` even when modifiers are held. Named keys are normalized, but `Key::Other` carries an undocumented raw Windows virtual-key value; `Key::Char` is not supplied to painted consumers through the character path.
- [WM_MOUSEMOVE](https://github.com/rceman/rust-ui/blob/cd150e4ebe120b21fbd4517fb6efcfc8360e69d4/src/platform/win32/window.rs#L192) discards WPARAM, and [hover forwarding](https://github.com/rceman/rust-ui/blob/cd150e4ebe120b21fbd4517fb6efcfc8360e69d4/src/platform/win32/mod.rs#L1532) sends zero flags. This loses `MK_LBUTTON` during drag selection. System-key, right-button and double-click classes listed as reentrantly routable are not equivalently delivered by normal dispatch.
- [`check_submit_override`](https://github.com/rceman/rust-ui/blob/cd150e4ebe120b21fbd4517fb6efcfc8360e69d4/src/platform/win32/mod.rs#L1398) declares that consumed multiline Enter both inserts a newline and submits. The canonical [submit contract](https://github.com/rceman/rust-ui/blob/cd150e4ebe120b21fbd4517fb6efcfc8360e69d4/docs/NATIVE_CONTROL_BOUNDARY.md#L139) describes Shift+Enter as newline and plain Enter as submit. The implementation's additional editing side effect is not an approved semantic correction.
- [`hit_test`](https://github.com/rceman/rust-ui/blob/cd150e4ebe120b21fbd4517fb6efcfc8360e69d4/src/platform/win32/mod.rs#L1093) honors enclosing Action ownership but never invokes `CustomRender::hit_test`. An actionable custom node receives rectangular hits regardless of its public hit-test result.
- Native committed-change routing has no explicit preedit suppression at [`native_commit`](https://github.com/rceman/rust-ui/blob/cd150e4ebe120b21fbd4517fb6efcfc8360e69d4/src/platform/win32/mod.rs#L780). Whether native notifications guarantee committed-only snapshots needs a focused assertion, not the source comment. Selection read/conversion failures are replaced with `(0,0)` or end-of-text, contrary to the checked-boundary contract.

**Required correction:** one Win32 input normalization authority, preserved native metadata and supported message routing, the approved submit/edit distinction, actual custom hit testing, checked selection conversion, and assertions that IME preedit never reaches committed TextValue or triggers Submit.

Disabled pointer/focus/invoke eligibility is an appropriate universal rust-ui contract. Read-only is separately pushed into RichEdit. Those policies should be retained and consumed consistently by pointer, keyboard and UIA routes.

### F06 — MAJOR: native timer ownership still reports success for failure and reuses live identities

[Timer handling](https://github.com/rceman/rust-ui/blob/cd150e4ebe120b21fbd4517fb6efcfc8360e69d4/src/platform/win32/mod.rs#L876) now records the owning NodeId and prunes dead peers, fixing the prior namespace/owner defect. Remaining failures:

- A full table silently drops the request; SetTimer's zero/failure result is ignored. [TxSetTimer](https://github.com/rceman/rust-ui/blob/cd150e4ebe120b21fbd4517fb6efcfc8360e69d4/src/platform/win32/text.rs#L350) nevertheless returns TRUE and records the requested timer in a separate host set. Microsoft's [TxSetTimer contract](https://learn.microsoft.com/en-us/windows/win32/api/textserv/nf-textserv-itexthost-txsettimer) requires FALSE on failure.
- `native_timer_fire` kills/removes the timer after its first tick. This is an undocumented one-shot conversion of a native periodic timer; there is no proof that RichEdit always requests another tick. Win32 timers notify on each elapsed interval until destroyed. See [Microsoft timer operations](https://learn.microsoft.com/en-us/windows/win32/winmsg/about-timers).
- [`native_timer_id`](https://github.com/rceman/rust-ui/blob/cd150e4ebe120b21fbd4517fb6efcfc8360e69d4/src/platform/win32/window.rs#L29) wraps modulo 4096 without checking existing IDs. One long-lived timer plus enough other registrations can replace its native timer and map entry. Delayed WM_TIMER messages also have no original generation once an ID is reused.

**Required correction:** one authoritative timer state, honest request/failure handling, correct repeated-tick semantics, collision-free identities and stale-message behavior, and tests for long-lived timers, churn, capacity, native failure, unmount and close. Demand-driven native caret/edit timers are legitimate; a zero framework idle loop does not justify altering their semantics.

### F07 — MAJOR: UIA wrappers improve liveness but do not fence all native references

The generation-keyed native registry, persistent child state, explicit labels, enabled/focus properties, sibling order and normal-close table clearing are genuine improvements. Final JSON includes finite editor rectangles and correct row labels.

Remaining issues:

- [`EditorFragment::GetPatternProvider`](https://github.com/rceman/rust-ui/blob/cd150e4ebe120b21fbd4517fb6efcfc8360e69d4/src/platform/win32/uia.rs#L387) returns the native pattern object directly. A client can cache native Text/Value patterns or ranges and bypass the wrapper's generation check after removal. `FragmentRoot` also delegates to the native provider without checking liveness. The windowless site's parent traversal lacks a live check.
- [`rebuild`](https://github.com/rceman/rust-ui/blob/cd150e4ebe120b21fbd4517fb6efcfc8360e69d4/src/platform/win32/uia.rs#L729) recognizes additions/removals but not reorder-only structure changes. Name/bounds are updated silently. A newly created root does not initialize its focus snapshot from the backend's existing focus.
- Cached child screen rectangles are refreshed by turns, but [WM_SIZE/WM_DPICHANGED](https://github.com/rceman/rust-ui/blob/cd150e4ebe120b21fbd4517fb6efcfc8360e69d4/src/platform/win32/window.rs#L282) directly relayout/paint without UIA refresh. WM_MOVE is not handled. A client retaining its provider can see stale bounds after window movement, resize or scale change. Refresh errors are discarded.
- Snapshot → strong child provider → snapshot forms a cycle while live. Normal `close()` breaks it, but reentrant NCDESTROY skips full shutdown and there is no equivalent Drop cleanup. This is a concrete cleanup gap; a measured retained-window leak is not claimed.

**Required correction:** generation/liveness must reach cached native patterns and text ranges, or native disconnection must be proven to invalidate them. Fence meaningful traversal/root operations, initialize and update focus/geometry, report reordered children, propagate refresh failures, and guarantee cycle-breaking cleanup on every exit. Native retained-pattern/range and real-provider teardown tests are needed; painted-fragment tests do not prove RichEdit provider lifetime.

### F08 — MAJOR: one typed style vocabulary exists, but its semantic authority and damage guarantees are incomplete

FAST recipes, public CUSTOM primitives and sparse SURGICAL patches do share the principal `BoxStyle`/`VisualStyle` representation and box painter. `TextInputStylePatch` correctly limits native text styling to chrome plus foreground. State patch merging, omission/removal and proportional radii have received useful focused tests. The bottom-only Primary button override is fieldwise: the width and red color do not replace unrelated recipe fields before OS enforcement. No backend import is required in the consumer.

Material remaining defects:

- [`style::role_color`](https://github.com/rceman/rust-ui/blob/cd150e4ebe120b21fbd4517fb6efcfc8360e69d4/src/style.rs#L868) and [`render::role_color`](https://github.com/rceman/rust-ui/blob/cd150e4ebe120b21fbd4517fb6efcfc8360e69d4/src/platform/win32/render.rs#L35) independently contain the complete palette. This is duplicated semantic authority, even though the current values mostly agree.
- Dirty comparison still compares role-bearing authored-style values, not concrete resolved colors. For example, light Background and AccentForeground resolve to the same RGBA but remain different Color variants. The documented concrete-value no-op contract is not implemented.
- [`turn_body`](https://github.com/rceman/rust-ui/blob/cd150e4ebe120b21fbd4517fb6efcfc8360e69d4/src/platform/win32/mod.rs#L492) relayouts and paints whenever any app message was processed (`needs_layout || updated`). Thus identical style/state traversal and shadow-only app changes still perform layout/native measurement despite the unit dirty bits.
- Hover/press/focus updates set paint dirtiness without classifying active-state metric changes. Padding/border/text-size state patches can paint against stale layout. The tests comparing dirty flags do not exercise this live backend transition.
- Per-side border corners use full-height left/right strips, not the approved corner bisector. Very thick opposing strips can overlap. This differs from the canonical geometry contract.
- Forced-colors enforcement only removes shadows; the hardcoded painted palette is not replaced by actual system colors. ReducedMotion::System is resolved initially but a staged System theme is treated as non-reduced by the shared runtime, and OS changes do not consistently re-resolve it.
- Native foreground ownership treats every role except Foreground as an explicit fixed color. An authored MutedForeground/Accent role is resolved once and then retained across theme changes. Resolved Theme.dark is used by principal render paths, but arbitrary role-backed peer foreground does not remain live.

**Required correction:** one concrete resolver/palette authority, backend work driven by actual resolved differences including interaction state, canonical side-corner geometry, real OS/accessibility enforcement and consistent role re-resolution. Preserve the successful sparse surgical model; no CSS or private second component API is needed.

### F09 — MAJOR: Shadow remains bounded in intent, but zero-blur, offset exclusion and extent safety are wrong

The public representation remains one outer descriptor (`color`, signed offsets, nonnegative `blur_sigma`), shared by built-in and public box surfaces. No general effects system or shadow-specific idle demand was added. The retained cache has 48-entry / 32-MiB limits, a 4096-pixel dimension limit and target-generation invalidation.

The [raster implementation](https://github.com/rceman/rust-ui/blob/cd150e4ebe120b21fbd4517fb6efcfc8360e69d4/src/platform/win32/render.rs#L1055) still violates the approved primitive:

- At sigma zero, `mask` equals `solid`; subtracting solid clears the entire shadow, including a visibly offset sharp shadow.
- Interior exclusion occurs before translating the bitmap by the shadow offset. Its hole is translated with the shadow, instead of excluding the original unshifted box interior. A translucent box can therefore show shadow ink inside its own interior.
- `2 * pad` and width/height addition happen before checked dimension/allocation guards. A finite, accepted `f32::MAX` sigma can produce infinite derived padding, saturate its integer cast and overflow this arithmetic. Over-budget paths silently return success without the requested shadow, rather than the required typed failure.
- The damage accumulator inspects the newly committed node/rect. It does not retain old shadow ink and old clipping for removal, movement or ancestor clip changes. Full-frame clearing currently masks stale pixels; it does not implement the documented old/new damage contract.

**Required correction:** checked derived extents before allocation/native mutation, typed resource failure, offset-aware exclusion of the original interior, visible sigma-zero shadows, and old/new ink accounting with their respective clips. Validate pixels and damage through the renderer, not only descriptor formulas.

Failure cleanup also remains incomplete: `CreateTextServices`/QI failure after raw HostBox allocation leaks the host, and brush creation failure after PushLayer can bypass PopLayer and the ManuallyDrop mask release. Use narrow RAII cleanup for those existing resources. The successful normal-path release correction should be retained.

### F10 — MAJOR: portable semantic tests still do not compile

Independently reproduced `cargo test --locked --lib` on Linux fails with:

```text
src/tests.rs:3752  tokio_block not found
src/tests.rs:4025  NodeDataLike not found; button_style_of not found
src/tests.rs:4035  button_style_of not found
```

The callers are portable tests, while [`tokio_block`](https://github.com/rceman/rust-ui/blob/cd150e4ebe120b21fbd4517fb6efcfc8360e69d4/src/tests.rs#L3759), NodeDataLike and button_style_of have `#[cfg(windows)]`. The latter defect existed before; the task-cancellation helper adds another instance. The helper's name does not represent a Tokio dependency.

**Required correction:** remove platform gating from genuinely portable helpers; retain gating on native tests. Run the shared headless suite on Linux and the focused native suite on Windows. Cross-target `cargo check` does not replace either execution result.

### F11 — MAJOR: proof reuse and the Rust/PowerShell ownership split are not implemented end to end

The new Rust probe is valuable, but [native.ps1](https://github.com/rceman/rust-ui/blob/cd150e4ebe120b21fbd4517fb6efcfc8360e69d4/benchmark/results/windows-native-composer-spike-v0.1/native.ps1#L94) still packs pointer LPARAM, injects UTF-16 characters and modifier keys, performs UIA traversal/pattern/property interpretation and converts UIA screen centers to client coordinates. PowerShell is not orchestration-only as its canonical contract and evidence README claim.

There is also a definite integration failure: [Post-DpiChanged](https://github.com/rceman/rust-ui/blob/cd150e4ebe120b21fbd4517fb6efcfc8360e69d4/benchmark/results/windows-native-composer-spike-v0.1/native.ps1#L130) invokes `native_probe dpi-changed <HWND> <PID> <dpi>`. The [actual CLI](https://github.com/rceman/rust-ui/blob/cd150e4ebe120b21fbd4517fb6efcfc8360e69d4/examples/native_probe.rs#L345) accepts `dpichange <window-title> <dpi>`. The DPI scenario cannot execute that documented contract. The multiline script searches for Edit while the recorded native body provider is Document.

Probe assertions also remain weaker than reported proof:

- Setting PMv2 ignores failure, and the queried awareness context is discarded rather than asserted/recorded.
- Geometry/UIA commands emit snapshots, often defaulting failed properties; they do not assert the claimed invariants. Synthetic dpichange sends the message and returns success without read-back assertions.
- IME acceptance checks whether the final draft contains any kana, without recording its initial value. A draft already containing kana can pass after no new composition. Input acceptance counts, preedit/commit separation and no-submit behavior are not asserted.
- The RichEdit test varies peer scale at one actual host/DC DPI. Natural height is checked only where the requested scale matches that DC. This is useful adapter/placement coverage; it is not four independent native DPI/font-metric environments.

**Required correction:** move the remaining native semantics/assertions into the existing Rust harness and consume production authorities; keep PowerShell responsible for sequencing, process lifetime, timeouts and collection. Fix its command interface. Record and assert awareness/environment, input success, before/after state and invariant-specific outcomes. Golden expected literals/property tests are legitimate; reproducing the production conversion or input model in another consumer is not.

### F12 — MAJOR: final artifact identity and visual/performance claims are not freeze-quality evidence

The [evidence README](https://github.com/rceman/rust-ui/blob/cd150e4ebe120b21fbd4517fb6efcfc8360e69d4/benchmark/results/windows-native-composer-spike-v0.1/README.md) names a final SHA in a commit trailer, but `cd150e4` has no such identity trailer. [`identity.txt`](https://github.com/rceman/rust-ui/blob/cd150e4ebe120b21fbd4517fb6efcfc8360e69d4/benchmark/results/windows-native-composer-spike-v0.1/identity.txt) identifies `f2afd84220d9f1f59235119c530cf46b5ad3dbca`, an earlier binary hash and an 08:25 run. The final JSON files were committed in `6e45113`; they contain no build source SHA, executable hash, run timestamp or complete environment receipt tying their runtime to the frozen candidate. Git artifact history establishes when files were committed, not which binary produced them.

I inspected the actual [final-foundation.png](https://github.com/rceman/rust-ui/blob/cd150e4ebe120b21fbd4517fb6efcfc8360e69d4/benchmark/results/windows-native-composer-spike-v0.1/final-foundation.png). Row text visibly sits above the corresponding outlined pill positions, including row 1. The README says “all rows inside pills.” That claim is contradicted by the submitted capture. This does not identify whether the remaining defect is production drawing, capture DPI/context, or stale/wrong-state evidence. It does prevent using this artifact as positive geometry acceptance. Recapture with recorded equivalent awareness and compare an actual screen capture if PrintWindow changes the native path.

Additional inconsistencies:

- `SPIKE_API_REVIEW.md` still identifies `89b39f1` and demonstrates editor TextStylePatch size/weight plus live EM_SETCHARFORMAT application. The actual consumer now uses capability-limited TextInputStylePatch. This is a material false API description.
- Deviation 2 still says peer-local bitmap rendering is implemented. The final code draws directly into the frame target. Deviation 3's one-activation account omits scale deactivation/reactivation. Other old entries retain obsolete method/ownership descriptions.
- The duplicated send artifact row is corrected. Finite editor rectangles are now recorded; an old Infinity limitation is not the final UIA result.
- `perf.txt` is unchanged since `5a93faf`: idle CPU 0%, working set 36.2 MiB, 22 threads, scale 1.25. It is historical smoke evidence, not a fresh performance result for the substantially changed native lifecycle.
- The current idle script waits 30 seconds but force-kills the process, so the clean-shutdown RUI_PERF counter dump cannot be collected as intended. No committed native counter/resource-baseline artifact proves teardown or zero framework wakeups. The 100-cycle test uses fake peers, not real HWND/COM/GDI/D2D accounting. Updated Send/Stop/resend scripting is not accompanied by identified final cancellation/resend artifacts.

**Required correction:** regenerate a source/binary-identified evidence bundle after the last implementation change; provide test/build logs, awareness/host/input metadata, actual reviewed visuals, corrected compiling API documentation and native idle/teardown counters. A receipt may name the exact implementation source commit when the later commit only adds evidence, but must prove the source tree/binary match. Preserve older smoke artifacts only as explicitly historical evidence.

### F13 — MINOR: bounded canonical status corrections

PLATFORM_CONTRACTS still lists real IME and real-key acceptance as pending, while final evidence reports measurements. Programming/native/style/backend documents retain concept-only/no-implementation wording; the style test section still says none executed. These statuses need a clear split between normative contract, implemented subset and outstanding proof.

The pending wording alone would be a bounded correction. Combined with the wrong API and rendering descriptions in F12, canonical documentation is materially misleading and Gate 19 fails. Do not silently turn unreceipted measurements into final acceptance while updating these labels.

## Single semantic authority audit

| Concept | Actual authority / remaining duplication | Assessment |
|---|---|---|
| Logical ↔ physical scaling | Shared geom methods reused widely; Windows DPI baseline defined in core | Arithmetic centralized, platform meaning leaked. F02. |
| Window-client ↔ peer-local origin | Pointer subtracts in logical space; caret/text callbacks subtract/add snapped physical origin | Competing semantics with a reproduced mismatch. F03. |
| Client ↔ screen | space.rs wrappers plus raw calls in backend, Rust probe and PowerShell | Consumers bypass authority and hide native failure. F03/F11. |
| Logical layout / native content inset | Pure layout and inset policy lives with Win32 layout/native measurement | Extract existing portable policy or expose one core owner; a future backend should supply measurements, not redefine Fill/inset semantics. |
| Input normalization / focus / hit ownership | Public events and Node::interactive exist; backend independently supplies default modifiers, VK numbers, action/focus eligibility | Contract is explicit but realization incomplete. F05. |
| Text state | Core committed revision/proposal mirror, native selection/undo/composition | Appropriate ownership; native scale activation and committed-only notification proof incomplete. F04/F05. |
| Accessibility state | Arena semantics plus UIA live snapshot/cache | Derived snapshot is legitimate; escaped native providers and missed lifecycle refresh are not. F07. |
| Timer ownership | Framework scheduler deadlines; host requested-ID set; backend native timer map | Framework/native separation is legitimate; conflicting native armed/success state is not. F06. |
| Palette/style resolution | Core palette plus full renderer palette and native palette policy | Concrete duplication; platform conversion should consume the shared result. F08. |
| Native resource accounting | COM/GDI/D2D owners and bounded caches, fake-peer lifecycle test, shutdown counter hook | Native lifetime/failure accounting and committed proof incomplete. F09/F12. |
| Validation harness | Rust native probe plus PowerShell native math/UIA/input implementation | Migration incomplete; independent semantics remain. F11. |

The audit finds multiple instances of the same duplication class. Corrections must rerun this bounded inventory and eliminate unexplained competing authorities; another isolated DPI call-site fix would not satisfy Gates 6/7/20.

## Architecture deviations: every recorded entry

| Entry | Follow-up assessment |
|---|---|
| 1. No pushed clip around peer draw | ACCEPT AS PLATFORM IMPLEMENTATION DETAIL, provided the peer's own bounds and allowed ancestor clipping remain correct. Final native confinement evidence still needs correction. |
| 2. Peer-local bitmap rendering | Historical workaround, superseded in code. The final foundation uses direct physical-pixel TxDrawD2D; update the deviation. No per-peer bitmap cache remains to approve. |
| 3. Lazy one-time activation | Lazy in-place activation is reasonable; unconditional UI activation and the new scale relatch require rework/proof. F04. |
| 4. Native reentrancy queue | REWORK BEFORE FOUNDATION. The principle is sound; two queue/entry mechanisms still disagree on safety and ownership. F01. |
| 5. Strong native UIA providers | Strong ownership is appropriate for classic COM; generation/liveness must cover native patterns and every teardown path. F07. |
| 6. Disabled excluded from hit testing | ACCEPT AND UPDATE ARCHITECTURE CONTRACT. Universal inert pointer/focus/invoke eligibility is sound; native and semantic consumers must agree. |
| 7. Resolved Theme.dark | ACCEPT AS PLATFORM IMPLEMENTATION DETAIL for principal palette ownership. Role-backed peer foreground and OS-policy resolution still need F08. |
| 8. Activated RichEdit measurement | TEMPORARY SPIKE COMPROMISE until fresh-result/error and multiline-growth proof. F04. |
| 9. HWND repair after creation | ACCEPT AS PLATFORM IMPLEMENTATION DETAIL with the current layout/paint gating and repair before normal native geometry work. No consumer leakage. |
| 10. Physical-pixel RichEdit host boundary | Correct Windows adapter realization; shared scale meaning and remaining duplicate origin conversion need F02/F03. Do not call the single-DC test four measured host-DPI environments. |
| 11. Synchronous synthetic WM_DPICHANGED | Correct direction for the probe's borrowed message payload. Its PowerShell caller is incompatible, and production reentrant dispatch still reposts the pointer. F01/F11. |

The native island remains a principled boundary: native editing state stays in RichEdit and rust-ui positions an axis-aligned rectangle. DirectComposition per peer could realize the same semantic boundary later. This candidate does not prove that future compositor, and its obsolete bitmap description must not be frozen as the current implementation.

## Native acceptance and remaining validation

| Area | What the evidence/code actually establishes | Foundation consequence |
|---|---|---|
| 96/120/144/192 geometry | Shared conversion tests and a native paint test varying peer ScaleFactor on one actual host/DC | Valuable focused coverage; not four independent actual monitor/DC-DPI captures. Label accurately and prove relevant native metric equivalence. |
| Actual 125% environment | Final geometry JSON records DPI 120, scale 1.25; finite UIA screen rectangles exist | Recorded measurement, not fresh reviewer execution; source/binary receipt needed. |
| Live scale change | Ink-only A→B native regression in test source | Does not prove text/selection/undo/focus/composition preservation. F04 is a freeze blocker. |
| Synthetic WM_DPICHANGED | Rust helper uses SendMessageW with an owned local RECT; reported suggested-rect resize | Deterministic regression, not real monitor acceptance; script broken and final read-back receipt absent. |
| Caret / pointer | Reported click→mid-text insertion; code preserves down/up metadata | Some functional evidence; fractional-origin and drag paths remain incorrect. F03/F05. |
| Selection / undo | Stored scenario artifacts and native selection-reading code | Useful earlier evidence; scale preservation and checked conversion still unproved. |
| Real IME | Real SendInput/MS-IME driver and final kana in the draft | Improved beyond synthetic WM_CHAR. Needs identified before/after composition/commit/no-submit proof; current test can falsely pass. |
| Dead keys | **NOT RUN** | Acceptable explicitly retained validation item for this environment; not by itself an architecture blocker. |
| Real physical monitor DPI transition | **NOT RUN** | Acceptable remaining native acceptance item only after deterministic state-preservation and adapter/environment proof are sufficient. Current F04 prevents that clearance. |
| UIA | Finite bounds, labels, enabled states and native editor providers recorded | Does not prove retained-pattern liveness, move/resize refresh, FromPoint acceptance or teardown. F07. |
| Idle/resources/teardown | Historical 36.2-MiB smoke; event-driven loop and bounded caches in source | No final native resource-baseline or clean counter receipt. F12. |
| Tall multiline growth | Explicitly not captured beyond scratch path | Validate supported growth/cap/scroll behavior; do not relabel inferred coverage as measured. |

Neither dead keys nor the unavailable physical monitor transition automatically requires redesign or indefinite architectural blocking. The absence of adequate composition/native-state preservation proof at a transition newly introduced by this candidate is material. Real IME must be more than an already-kana-containing final buffer before foundation approval.

## API, retained runtime, resource and scope conclusions

**API ergonomics — NOTE:** FAST remains concise: `ui.button(...).on_press(...)` and chained native text builders. CUSTOM is available through public BoxStyle, ActionStyle, Canvas and CustomRender primitives. SURGICAL bottom-border customization is practical and fieldwise. TextValue acknowledgement/conflict handling is additional ceremony with a clear ownership purpose; keyed closures and task factories are understandable. Consumer code contains no HWND, COM, backend registry or native-message concepts. ScaleFactor's exported DPI semantics and the raw-key meaning are the exceptions needing contract correction, not a reason to redesign application composition.

**Retained/keyed/events/tasks — NOTE:** the staged arena, generation fencing, explicit scopes and keyed peer identity remain a reasonable foundation. Shared task limits are eight active registrations, 64 mailbox entries and 64 blocked send waiters; generic message payload bytes remain the consumer's responsibility. Cancellation fences at send/dequeue and spawn failure is now surfaced by the example. No mandatory async runtime was added. The fake-peer lifecycle tests are useful for shared ownership, but cannot certify native resources or current Windows input behavior.

**Idle/resources — NOTE:** the blocking GetMessage loop and mailbox wake seam support an event-driven framework. There is no permanent framework render loop or shadow frame/timer demand in the inspected code. Removing per-peer bitmaps eliminates the earlier peer-cache generation problem. DirectWrite and shadow caches have explicit limits. Existing native timer and failure-cleanup defects must be corrected; one historical perf smoke cannot establish final leak freedom, stable working set or thread requirements. No optimization is requested merely because it could be done later.

**Scope — NOTE:** no CSS runtime/parser, selector metadata, macOS/Linux backend, Mascot product implementation, plugin/scripting engine, browser layout engine, mandatory Tokio or hidden CI dependency was introduced. Simple Rows/Columns/Fill and proportional corner normalization are typed layout/geometry policy, not a browser engine. Evidence hooks remain development concerns. No AppCommand/AppEvent, reference application or gallery work was started during this review.

## Future macOS paper test

Application state/messages, committed TextValue, scoped events, keyed identity and public component/style vocabulary can be realized by another backend without changing their public meaning. Native text, clipboard editing, focus, accessibility APIs and native rendering are legitimate platform realizations.

A backend author using AppKit would currently have to reinterpret the documented 96-DPI/inch meaning, reconcile native point/backing spaces with the universal physical-screen claim, assign a new meaning to raw Key::Other values, and reproduce Win32-owned logical inset/layout or accessibility eligibility policy. Those are leaked or incompletely owned semantics. F02/F03/F05/F07/F08 and extraction of existing portable policy remove them without implementing macOS or introducing architecture-only traits.

After those corrections, macOS can provide its native backing conversion, native text and accessibility implementations while consuming the same rust-ui semantics. The intended boundary is achievable; it is not yet fully established at this SHA.

## Universal Gates 1–20

The canonical document contains exactly Gates 1–20. Gate 6 contains single semantic authority/platform ownership; Gate 7 contains semantic duplication; Gate 9 requires stable platform-neutral semantics; Gate 16 contains environment equivalence and proof reuse; Gate 20 requires the systemic authority audit. Their wording is strengthened as requested. Implementation compliance is assessed separately below.

| Gate | Assessment | Evidence / reason |
|---|---|---|
| 1 Requirements/completeness | FAIL | Required native ownership, style and final proof remain incomplete. |
| 2 Assumptions/approval | FAIL | Multiline newline+submit and universal Windows DPI meaning are not established approved semantic corrections. |
| 3 Contract preservation | FAIL | Input, Shadow and state/work guarantees differ from canonical contracts. |
| 4 Scope discipline | PASS | Changes remain within the Windows spike and its contract/evidence correction. |
| 5 Minimal correct design | FAIL | Existing narrow ownership fixes are incomplete; no speculative framework is needed. |
| 6 Authority/platform ownership | FAIL | F02/F03/F06/F08/F11. |
| 7 Semantic duplication | FAIL | Peer origins, palettes, screen/input/UIA script logic. |
| 8 No obsolete/fallback paths | FAIL | Old executable native-semantic scripting remains alongside the replacement harness. |
| 9 Stable platform-neutral semantics | FAIL | Shared 96-DPI meaning, raw key meaning and inconsistent normalization. |
| 10 Canonical authority/obsolescence | FAIL | API/deviation/status descriptions conflict with current code. |
| 11 Failure safety | FAIL | Borrowed pointer repost, ignored native failures, unchecked Shadow extents and cleanup gaps. |
| 12 Boundedness | FAIL | Caches/queues are bounded, but ID collisions, unsafe extent arithmetic and silent capacity failure remain. |
| 13 Dependency necessity | PASS | Added native API capability is relevant; no unwanted runtime/framework introduced. |
| 14 Performance/redundant work | FAIL | Every processed app update forces layout/paint; final measurements are missing. |
| 15 Infrastructure boundary | PASS | Normal consumers/core do not own checkout, capture or evidence orchestration. Development hooks are explicit. |
| 16 Verification/proof reuse | FAIL | Portable tests fail; native state transitions unproved; awareness/host matrix and harness assertions insufficient. |
| 17 Recovery proportionality | PASS | Existing focused authorities can be corrected; no architecture replacement is justified. |
| 18 Immutable identity | FAIL | Candidate was held immutable for review, but evidence-to-final-binary identity is incomplete. |
| 19 Artifact honesty | FAIL | Visual claim contradicted, API/render descriptions obsolete, resource smoke historical. |
| 20 Systemic audit | FAIL | Multiple competing semantic authorities remain; the table above identifies the bounded correction surface. |

PASS denotes the inspected changed cone, not independently certified Windows execution or remote authoritative-checkout operations.

## Smallest bounded correction set before another review

1. **Native ownership/delivery:** complete the existing guard and message-class contract across initial turn, native callbacks, failure and teardown; remove borrowed-pointer repost; preserve generation/ordering and propagate errors. F01.
2. **Shared geometry/adapter:** make scale a validated ratio, move Windows DPI mapping to Win32, centralize snapped peer-origin and checked screen conversions, and route runtime/probe consumers through them. Extract existing pure logical layout/inset policy rather than redefine it in a future backend. F02/F03.
3. **Text/input:** separate in-place/UI/focus activation, prove all native state across scale relatching, fix normalization/drag/submit/custom hits and checked committed selection, and validate fresh native measurement plus multiline cap/scrolling. F04/F05.
4. **Native timers:** one accurate owner, honest failure/capacity behavior, periodic delivery where required, collision-free identity and stale-message proof. F06.
5. **Accessibility:** cover escaped patterns/ranges, live bounds/focus/order and all cleanup paths; test real native retained references. F07.
6. **Styling/render resources:** consume one concrete palette/resolver; honor no-op/paint/layout classification and active state, theme roles and OS policy; correct border corners, zero/offset Shadow masks, checked bounds and old/new damage; close native allocation/layer failure paths. F08/F09.
7. **Verification:** fix portable helper gating; finish the Rust-native/PowerShell-orchestration cutover and CLI; assert source-relevant native outcomes and environment equivalence. Run focused shared/adapter regressions and final native acceptance. F10/F11.
8. **Freeze evidence/docs:** update actual compiling API and superseded deviations, resolve the visibly contradictory capture, collect final identified tests/visuals/idle/native teardown evidence, and correct statuses while retaining NOT RUN items. F12/F13.

The next review should inspect the corrected immutable source, focused regression results and a final acceptance receipt. It should not require another architecture redesign or implementation of a future platform. Dead keys and a physical monitor move may remain explicitly unrun if the corrected contract and deterministic/native state-preservation evidence justify that bounded environmental limitation.

**Use as next-milestone base:** this SHA can be the starting checkout for the bounded repairs, but it has no foundation clearance for the Shadcn Component Gallery. **Merge toward main:** not cleared at this SHA; review the corrected candidate and identified evidence first.

REWORK_REQUIRED
