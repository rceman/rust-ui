# Independent final narrow Windows foundation review

**Date:** 2026-10-02. **Verdict:** `REWORK_REQUIRED`.

This reviews the remaining findings from the previous closure review. The
platform-contract architecture remains sound. Several bounded corrections are
effective, but native progress, composition ordering, representability,
accessibility arrays, effective styles, damage and verification still have
concrete defects. Passing tests do not close paths their assertions omit.

## Immutable identities

| Item | Exact identity |
| --- | --- |
| Repository | `rceman/rust-ui` |
| Branch | `agent/windows-native-composer-contract-spike-v0.1-swe2` |
| Previous closure review commit | `ab031a7fef101d55cc3f0dec2a472ffc87a65f4d` |
| Last production implementation | `4a464d6002e7d9b905cb528adb5f00069f483520` |
| Verification/harness source snapshot | `c18ebcab2fea605e94a3a1f58415c5b2f1d1831e` |
| Evidence publication snapshot | `a2e948570acb2f601f7f1888156cfcd078f9d6fc` |

**The identity chain is valid.** Inspection of the Git diffs confirms no `src/`,
Cargo manifest or lockfile changes after `4a464d6`. The intervening changes
update `examples/native_probe.rs` and benchmark verification/evidence files.
The `c18ebca` to `a2e9485` diff changes only `identity.txt` and `smoke.json`.
Publication HEAD is therefore not another production implementation candidate.

The receipt identifies the harness/source snapshot whose production sources
are those of `4a464d6`:

```text
HEAD=c18ebcab2fea605e94a3a1f58415c5b2f1d1831e
exe_sha256=8794CE08FF39E8507F472ACFF5D498A5EC9E56BEFA7E1C56BBE3CB63285FBCCB
probe_sha256=73848100F63D02D21A5559BB8A341B3A59FDF0AC3BBF5D0EE51E0CE073B3794D
run_utc=2026-10-02T08:47:00.3041732Z
```

Native Windows `Get-FileHash` independently matched both existing executable
hashes, before and after the review checks. This establishes receipt/binary
consistency; it does not certify every acceptance claim. Later receipt-only
publication cannot change the binary semantics.

Both earlier review documents remain immutable. Their SHA256 values before
publication are:

```text
PLATFORM_FOUNDATION_REVIEW.md
c05368739931dced9407fbb6fceec84e3baccfc83161d365eb9607bbda33b21f
PLATFORM_FOUNDATION_CLOSURE_REVIEW.md
7701046047f96bf94a18c4e80b61e34cb14798bda3d6c728e1d51face2709721
```

This new document is the sole review publication change. Production,
verification code and evidence are unchanged. No Gallery work or main merge
was performed.

## Review environment and independent checks

Native review and publication used the authoritative checkout
`W:\devin_folder\rust-ui`, initially clean at `a2e9485`, Windows 11 Home,
native Rust 1.94.0. Additional source inspection used a detached Linux
checkout of that same snapshot. Linux checks used WSL2,
`6.18.40.1-microsoft-standard-WSL2`, Rust 1.93.1.

| Independently executed check | Result / limits |
| --- | --- |
| Linux `cargo test --locked --lib` | PASS, **76/76**. |
| Native Windows `cargo test --locked --lib` | PASS, **86/86**. Native assertions were also inspected. |
| `cargo fmt --check`, Linux and native Windows | PASS. |
| Native Windows `cargo check --locked --all-targets` | PASS, with warnings. No inference from cross-compilation. |
| Current geometry source in an external scratch executable | Fractional-origin, negative-point and checked integer add/sub cases pass; the `2^31` conversion counterexample below reproduces. |
| Isolated native OLE SAFEARRAY ABI experiment | Direct `VT_UNKNOWN` pointer insertion correctly AddRefs/releases. The extra-indirection call shape used in production exits with **0xC0000005** (`-1073741819`). This isolates the API misuse; it is not a production UIA crash replay. |
| Fresh review-owned composer/probe smoke | PMv2 asserted, actual DPI **120**, scale **1.25**, window **980×900 physical px**, draft UIA value `draft-prefill`. Temporary UIA and PrintWindow captures made outside the repository; owned process closed afterward. |
| Fresh default UIA tree | No `turn ...` counter label exists while details are hidden. |
| Actual visual artifacts | Inspected `final-foundation.png`, `composer-dark.png`, `typing.png`, `ime.png`, plus a fresh native PrintWindow capture. See F12. |
| Receipt decoding | Identity, IME, geometry and performance files decode as UTF-8, including BOM where present; checked JSON receipts parse. |

The **15 native collection scenarios** and **30-second performance run** are
reported/recorded evidence, not independently replayed as a complete matrix.
This review independently executed native library tests and the focused smoke
above. No new repository tests were added. External review experiments are
identified separately from production-path execution.

## Closure matrix

| Finding | Status | Smallest remaining defect |
| --- | --- | --- |
| F01 | PARTIALLY_CLOSED | Preserve an explicitly absent arrival owner, use that owner for IME/focus routes, check every continuation post, and complete queued destruction beyond the drain budget. |
| F03 | PARTIALLY_CLOSED | Reject `2^31` at the checked float-to-i32 boundary and enforce representability when constructing peer origins. |
| F04 | PARTIALLY_CLOSED | Make composition deferral preserve one effective peer geometry and use the actual composition owner through completion. |
| F05 | PARTIALLY_CLOSED | Acknowledge the final queued native edit before resolving a pending TextValue proposal. |
| F06 | PARTIALLY_CLOSED | Surface framework deadline arming failure or provide guaranteed recovery; truthful `None` alone can leave scheduled work asleep. |
| F07 | PARTIALLY_CLOSED | Correct nonempty SAFEARRAY insertion/COM reference ownership and stale range-argument fencing; exercise real escaped children. |
| F08 | PARTIALLY_CLOSED | Share text-metric damage classification and apply system-aware colors to live native foreground/selection consumers. |
| F09 | PARTIALLY_CLOSED | Check offset-coordinate arithmetic and retain actual paint/state/clip ink footprints after every committed paint. |
| F11 | PARTIALLY_CLOSED | Require observable counter and application committed state, check SendInput success, and escape all JSON string contents. |
| F12 | PARTIALLY_CLOSED | Remove unsupported proof claims and reconcile the historical final image with current cropped-text captures. |
| F13 | PARTIALLY_CLOSED | Correct current candidate identities, obsolete CLI name and the composition-preservation account. |
| N01 | CLOSED | Backward-selection preconditions and exact directional restoration are now asserted non-vacuously. |
| N02 | CLOSED | Repeated logical timer rearm reuses its registration; tombstones prevent stale tick redirection. |
| N03 | PARTIALLY_CLOSED | Guard transfer is repaired, but nonempty array insertion is unsafe and provider entries retain an extra reference. |
| N04 | CLOSED | Exclusion sign and offset-bearing cache identity are repaired; ordinary ±x native pixel regressions pass. |

F02 and F10 remain closed. No regression in platform-neutral scale meaning or
portable test compilation was found. Their architecture is not reopened.

## Findings and bounded corrections

### F01 — BLOCKER: progress and ownership are not universal

[Native entry and drain](https://github.com/rceman/rust-ui/blob/4a464d6002e7d9b905cb528adb5f00069f483520/src/platform/win32/mod.rs#L2101)
now guard the initial turn and `bail`/DestroyWindow; copied DPI payloads remain
owned. Queue capacity **256** is independent of drain budget **128**. A drain
of exactly 128 items with nothing left correctly succeeds. Delivery errors are
recorded while draining continues, and the outer continuation checks posting
failure. `bail` explicitly calls shutdown even when its drain fails.

Remaining concrete paths:

1. `focused_target()` uses `arrival_focus.get().or(self.focus)`. A replay whose
   captured owner was **None** falls through to a newly focused peer. Live
   dispatch and replay-with-no-owner need distinct representation. Furthermore,
   `focus_targeted` omits WM_SETFOCUS, WM_KILLFOCUS and WM_MOUSEWHEEL although
   their dispatch uses `send_focused`.
2. `ime_start` and `ime_end` use current `self.focus`, not the replay's original
   peer. Native IME delivery can target A while composing flags, conflict
   resolution and pending relatch affect newly focused B.
3. `turn`, `enqueue_native` and the inner `service_peer_events` continuation
   still ignore `PostMessageW` results after setting `pump_queued`. The checked
   outer drain does not repair these paths. A failed wake can suppress future
   wakes and strand owned work.
4. Reentrant WM_NCDESTROY sets `closed=true`, detaches the WndProc route and
   appends cleanup behind existing queued items. If cleanup lies beyond item
   128, a successful first drain leaves it pending. Continuation is suppressed
   because the window is closed; full shutdown/PostQuitMessage has not run.
   There need be no earlier error to invoke `bail`. The run loop can wait with
   mandatory cleanup stranded. Continuing after an earlier delivery error is
   useful, but does not make this success-path destruction trace safe.

The new queue regression checks constants and message classification, not
these drain/continuation/destruction traces. Retain the existing ownership
design, introduce an explicit replay-owner discriminator, use one checked wake
operation, and finish mandatory teardown independently of ordinary drain
budget/closed-window posting. Add focused exact-128, greater-than-128,
post-failure, no-owner, focus-change and destruction-after-error assertions.

### F03 — MAJOR: integer arithmetic repaired; float boundary still saturates

[PeerOrigin](https://github.com/rceman/rust-ui/blob/4a464d6002e7d9b905cb528adb5f00069f483520/src/geom.rs#L242)
now uses checked subtraction/addition. The prior `scale=1.25`, origin `0.4`,
client px `1` case returns local `0`; negative inputs are preserved. Overflow
edges return None. Production and native_probe signed-16 LPARAM packing use
the checked authority, accepting -32768 through 32767 and rejecting outsiders.

However, `ScaleFactor::try_to_physical` compares an f32 with
`i32::MAX as f32`, which equals **2147483648**, not 2147483647. Independent
execution of the exact current module produced:

```text
ScaleFactor::ONE.try_to_physical(2147483648.0)
observed: Some(2147483647)
required: None
```

This is a saturating cast presented as successful checked conversion.
`PeerOrigin::from_logical` also still constructs through unchecked
`Rect::physical`, despite its comment promising None for unrepresentable
bounds. Use an exact range check and enforce checked origin construction at
the native seam. No coordinate architecture redesign is needed.

### F04 / N01 — MAJOR remaining composition geometry; backward selection CLOSED

[Relatch implementation](https://github.com/rceman/rust-ui/blob/4a464d6002e7d9b905cb528adb5f00069f483520/src/platform/win32/text.rs#L1173)
now snapshots before deactivation, changes geometry only afterward, reactivates
the same service, restores sorted TOM extent and then the active end, and
restores UI activation when previously UI-active. HRESULT failures are honored
and reached activation states are recorded. Fresh natural measurement resets
its extent and errors if no fresh result arrives. Multiline growth is asserted;
max-lines/scroll acceptance is recorded, not rerun as a complete scenario here.

The native regression first proves nonempty `anchor > focus`, available undo,
known UI activation and committed text. After relatch it checks identical text,
exact anchor/focus pair and direction, undo availability and UI activation.
That closes N01. It checks EM_CANUNDO, not an executed undo/redo round trip;
this report does not promote availability into measured redo preservation.

Deferring scale relatch during composition is a sound native strategy in
principle. Its current implementation does not yet preserve its own boundary:

- The composing flag can belong to the wrong peer under deferred delivery
  (F01). Consequently the actual IME owner can still be deactivated.
- While `apply_bounds` saves pending bounds/scale and keeps host geometry old,
  global `peer_ctx.scale` and layout `rects` already become new. Pointer/caret
  transforms use the new values, whereas host client/screen callbacks use the
  old host snapshot. For an editor at logical x=40, changing scale 1 to 1.5
  makes those origins 60 and 40 physical px during deferral.
- `natural_size` still writes host width while composition is active, and
  drawing combines new supplied layout bounds with the old host scale. Thus
  the promised whole-geometry deferral is not consistently consumed.
- No focused test drives `set_composing(true)` through pending geometry and
  `finish_pending_relatch` with those concurrent consumers.

Keep one effective peer geometry while a newer geometry is pending; have
pointer, caret, host callbacks, drawing and measurement consume that snapshot.
Execute pending relatch only after the owning composition's final committed
state is acknowledged (F05). This deterministic state-machine proof can close
the issue without requiring a physical monitor transition during live IME.

### F05 — MAJOR: final native edit is enqueued before proposal resolution, but not acknowledged

[IME end](https://github.com/rceman/rust-ui/blob/4a464d6002e7d9b905cb528adb5f00069f483520/src/platform/win32/mod.rs#L2169)
now clears composition flags before draining peer notifications. Checked
selection conversion replaces the old end/zero fallbacks, and preedit
selection notifications are suppressed. Painted character input and supported
system-key routes are now explicit. Platform-scoped opaque `Key::Other`
remains acceptable; portable consumers use normalized keys.

The canonical `SubmitPolicy::Enter` truth table is now:

| Editor | Enter | Shift+Enter |
| --- | --- | --- |
| Single-line | Submit | Submit, no newline |
| Multiline | Submit | Edit/newline |

The explicit keydown fork avoids delivering a submitting keydown as editing.
Supported normal/deferred routes were inspected; real IME no-submit proof is
still incomplete for the reasons under F11.

The final-commit conflict order remains wrong. `native_commit` calls
`Runtime::edit_event`, which **queues** an edit. `ime_end` then immediately
calls `Runtime::composition_end` before `turn` runs `Runtime::pump` and
`TextPeerSync::accept_native` advances the shared committed revision.

A concrete trace is: pending proposal base R; final IME edit base R/result N;
native edit enqueued; proposal resolution still sees R and applies P; pump
later rejects the native edit because its base R no longer matches P. The
final committed edit can lose to a proposal that should have conflicted.
Draining host notifications is not acknowledgement of runtime edit events.

Process the owning final native edit into committed runtime/TextValue state
before proposal resolution and relatch. Assert that a proposal based on the
pre-composition revision conflicts after a final native commit, while preedit
never commits and commit Enter never submits. This is a deterministic ordering
defect, not an unavailable environmental acceptance requirement.

### F06 / N02 — MAJOR remaining framework failure; native timer identity CLOSED

TimerPool rearm now reuses a logical timer's registration. Its bounded
tombstone/non-reuse strategy prevents queued stale WM_TIMER from binding to
a future owner during the window lifetime. Native arming/capacity failures
return FALSE; ticks remain periodic; kill/release/Drop/window shutdown clean
peer registrations. Exhaustion of the bounded lifetime ID budget is honestly
reported rather than silently recycling stale IDs. N02 is closed.

[Framework deadline arming](https://github.com/rceman/rust-ui/blob/4a464d6002e7d9b905cb528adb5f00069f483520/src/platform/win32/mod.rs#L1028)
now checks SetTimer and truthfully leaves `deadline_timer=None` on failure.
However, `rearm_deadline` returns no error and guarantees no retry; its comment
depends on an unrelated next turn. If this was the only pending wake, the
message loop sleeps with required scheduled work still pending. Surface a
typed arming failure through the caller's existing error path or provide a
bounded guaranteed recovery. This is the remaining F06 correction, not a
request for a new timer framework.

### F07 / N03 — BLOCKER: nonempty SAFEARRAY calls are unsafe

[Native fences](https://github.com/rceman/rust-ui/blob/4a464d6002e7d9b905cb528adb5f00069f483520/src/platform/win32/uia.rs#L594)
now cover FragmentRoot, enclosing providers, patterns and returned ranges.
The guard/disarm change correctly keeps output arrays alive through transfer
and destroys owned arrays on fallible exits. Known text/value patterns are
wrapped; unsupported patterns do not escape by a generic delegation.

Both `fence_range_array` and `fence_provider_array` nevertheless pass
`&ptr as *const _ as *const c_void` to SafeArrayPutElement on VT_UNKNOWN arrays.
VT_UNKNOWN insertion expects the **interface pointer itself**, without
another level of indirection, and AddRefs it. This is specified by Microsoft's
[SafeArrayPutElement contract](https://learn.microsoft.com/en-us/windows/win32/api/oleauto/nf-oleauto-safearrayputelement).
Passing the address of the pointer makes native code treat a stack address as
the COM interface. The isolated native ABI experiment independently reproduced
access violation 0xC0000005 with that call shape; the direct-pointer control
correctly increased and then released the interface reference.

After that argument is fixed, `std::mem::forget(fenced)` in the provider-array
loop also leaks its original reference: SafeArrayPutElement adds an array-owned
reference; it does not consume the wrapper's existing owned reference. Let that
owned interface drop normally. Disarming the array guard and forgetting an
interface reference are different ownership operations.

Other narrow gaps:

- `unwrap_range` retrieves a fenced argument's native range through
  `inner_raw` without checking that argument's generation/liveness. Receiver
  methods check the receiver, which does not prove a retained dead argument
  may be used with a live receiver. Fence the argument before unwrapping.
- The fixture now retains a real msftedit pattern, document range and enclosing
  provider and asserts invalidation on removal/recreation. `GetChildren` may
  return null/empty, and no child provider is extracted, retained or queried.
  It therefore does not establish the claimed child-provider lifecycle.
  The enclosing-provider assertion also permits either of two calls to fail,
  rather than requiring the specifically supported call to be fenced.

Exercise nonempty selection/range and provider arrays, success transfer,
failure cleanup and actual child-reference retention across removal/recreation/
close. Keep the existing wrapper architecture; fix its ABI and ownership.

### F08 — MAJOR: state metrics and live system colors still bypass shared semantics

[Interaction classification](https://github.com/rceman/rust-ui/blob/4a464d6002e7d9b905cb528adb5f00069f483520/src/platform/win32/mod.rs#L1690)
now compares the effective old and new state, which fixes the previous
any-metric-branch approximation. Same effective output yields zero work, and
box metric changes yield layout plus paint. Opposing oversized border widths
are proportionally normalized, preventing crossing strip geometry.

Text metric changes still produce paint alone. The backend calls `box_dirty`
and treats every remaining text difference as paint, while the existing shared
`node::visual_dirty` correctly marks text size/weight as layout plus paint.
Reuse that authoritative classifier for hover/pressed/focus transitions.
This is an actual duplicate semantic rule with different results.

Window clear, canvas fill/text and focus ring now use the system-aware resolver.
Native foreground does not: `WindowlessPeer::set_colors` ignores the supplied
foreground and resolves its authored role through ordinary `resolve_color`.
`PeerHandle::apply_foreground` follows the ordinary resolver as well.
Selection colors use system slots at palette construction, but live OS changes
can fail to propagate: `os_change` writes the new appearance/theme cell before
`turn_body`'s peer-color update, which runs only when the theme differs. An
appearance/system-palette change with unchanged theme therefore leaves mounted
peers on old effective foreground/selection colors.

Use the effective system-aware role resolver for peer foreground and update
mounted peer palettes whenever their effective output changes. Preserve the
distinction between authored roles and concrete effective values. ReducedMotion
System refresh is present; it does not repair these color paths.

### F09 / N04 — MAJOR remaining offset bounds and retained damage; sign/key CLOSED

[Shadow raster](https://github.com/rceman/rust-ui/blob/4a464d6002e7d9b905cb528adb5f00069f483520/src/platform/win32/render.rs#L1080)
now maps a mask pixel to original-box space with **x+offset_x, y+offset_y**.
That is the correct sign for excluding the original interior when the mask is
drawn at the authored offset. Cache identity includes both baked offset axes.
The independently executed native regression covers sigma-zero ±x interior
exclusion and side placement. ±y and combined offsets follow the same
inspected axis-symmetric formula; no separate native pixel execution for those
cases is claimed here. N04's sign/cache defect is closed.

Sigma/padding/dimensions and allocation extents are now checked before arithmetic;
large-sigma failure is tested. The border ManuallyDrop mask clone is acquired
after fallible brush/geometry preparation and released after PopLayer, closing
the earlier failure-path leak.

Offset arithmetic is still unchecked. Public Shadow validation accepts any
finite offset. `ox_key`/`oy_key` use saturating float-to-i32 casts, followed by
unchecked `pixel_x + ox_key` and `pixel_y + oy_key`. At scale 1, a finite offset
of `2147483648.0` becomes i32::MAX; adding pixel 1 overflows in debug and wraps
in release. Check representability and additions or reject an unrepresentable
raster before entering the loop.

Old/new move and removal damage unions are improved, but `committed_ink` is
updated only during relayout, not after a paint-only change. Its Action styles
resolve with hot/pressed/focus all false, and it does not retain ancestor clip
state. A paint-only shadow expansion can be drawn without updating the saved
footprint; a later shrink/removal cannot reliably invalidate the actual old
ink. Save effective painted ink and relevant clip state at the successful
paint boundary and union the actual previous/new footprints. Full-frame clear
currently masks these omissions and is not retained-damage proof.

### F11 — MAJOR: native harness ownership improved; IME assertions can still false-pass

[Harness](https://github.com/rceman/rust-ui/blob/c18ebcab2fea605e94a3a1f58415c5b2f1d1831e/examples/native_probe.rs#L496)
now owns DPI derivation, native action, physical geometry read-back/assertion
and checked pointer packing. PowerShell orchestrates the real `dpichange`
command and no longer computes semantic DPI expectations. The editor argument
is honored. The IME check requires a newly gained, nonempty suffix with kana;
it no longer searches the entire after value. UTF-8 transport/receipt encoding
repairs are effective for the checked committed files.

However:

- `turn_counter(...).unwrap_or(0)` permits a missing counter before and after
  to pass as unchanged zero. The default composer hides that label inside
  `show_details`; S-Ime does not open details. The fresh default native UIA
  tree independently confirms no label. The committed `submits:0` receipt
  therefore does not prove absence of submit.
- SendInput return counts are discarded throughout the IME action. Newly
  gained kana is useful acceptance evidence, but not an assertion that every
  required native input operation succeeded.
- After is read from the native provider; no observable application committed
  TextValue assertion establishes native/core agreement or preedit exclusion.
- IME/value JSON formatting only escapes quotes (and interpolates keys raw).
  Backslashes and control characters, including multiline values, can make
  output invalid JSON despite UTF-8 correctness. Use complete JSON string
  escaping for all public harness string results.

Require the counter's presence and a known baseline, assert input success,
observe the application's committed value and proposal behavior, and serialize
valid JSON. Do not weaken the gained-suffix check.

### F12 — MAJOR proof/visual honesty; identity, UTF-8 and performance numbers repaired

The immutable receipt chain and binary hashes are valid. `perf.txt` and
`perf-counters.txt` match README: idle CPU 0%, 32.3 MiB working set, 17 threads;
uptime 30544 ms, requested/caret redraws 78, native timer fires 0. Clean-shutdown
counter emission is teardown evidence. It is not a provider/peer resource-leak
test, and README now explicitly avoids claiming leak freedom. This review did
not independently repeat the 30-second run.

Current IME receipt decodes/parses correctly and gained text contains newly
committed kana. Its no-submit claim remains unsupported (F11). README also
overstates universal native entry, relatch preservation, metric transitions,
child-provider invalidation and all checked shadow arithmetic in ways directly
contradicted above.

The original text-above-pill contradiction is absent in the inspected
`final-foundation.png`; rows are inside chrome. But that file last changed in
**afe5bfc**, the previous implementation evidence, and current collection never
writes that filename. README still labels it the final candidate without
historical qualification.

More significantly, current `composer-dark.png` and `typing.png` visibly omit
the `composer spike` label and crop the draft prefix to `efill` /
`efillhello world`, although their value receipts contain the full text.
An independent fresh native PrintWindow capture reproduced the missing label
and `efill` crop while the current native UIA value was `draft-prefill`.
`ime.png` shows the full prefix and label. This is an actual inconsistency in
the claimed current capture pipeline. The experiment does not establish
whether the same defect occurs on the physical display; no such inference is
made. Explain/correct the rendering or capture boundary and publish consistent
current visual proof. A historical good image cannot substitute for it.

### F13 — MINOR canonical staleness, with material proof corrections under F04/F12

No files under `docs/` changed between the previous review commit and the
publication snapshot. Concrete current-document corrections still needed:

- `SPIKE_API_REVIEW.md` identifies candidate **0405a80** as current.
- `PLATFORM_CONTRACTS.md` names `native_probe uia-tree`; the real command is
  `uia`.
- `SPIKE_ARCHITECTURE_DEVIATIONS.md` still says composition survives relatch
  because the same COM service is retained, describing unconditional
  deactivate/reactivate rather than composition-active geometry deferral.
- Evidence README's Candidate **e561c6e** does not identify the receipt source
  snapshot **c18ebca** or explain its relationship to production **4a464d6**.
- The historical `final-foundation.png` and unsupported measured-proof claims
  require F12 corrections.

Current shared ScaleFactor/Dp wording is appropriately platform-neutral;
earlier bitmap rendering is clearly historical; the obsolete one-activation
rule is superseded. Keep that historical chronology. Correct present identity,
CLI and proof statements without rewriting the immutable reviews.

## New concrete observations within the remaining scope

- **BLOCKER — F07/N03:** incorrect VT_UNKNOWN pointer shape on nonempty array
  insertion, independently confirmed against native OLE. The guard fix alone
  cannot make this path safe.
- **MAJOR — F12:** current PrintWindow proof crops a full native value and omits
  a semantic label; independently reproduced. Rendering versus capture cause
  remains to be resolved, rather than asserted from a historical screenshot.

These are concrete defects on the requested closure surface, not architecture
redesign findings or stylistic renumbering.

## Environmental NOT RUN and residual risk

| Item | Disposition |
| --- | --- |
| Dead keys | NOT RUN. Not independently a blocker: normalized character/surrogate routes plus related real-key/native-text proof bound this environmental case. |
| Physical monitor DPI transition | NOT RUN. Not independently a blocker: deterministic mapping, native relatch and synthetic DPI geometry assertions provide relevant adapter proof. They do not imply the physical move was measured. |
| Real IME composition during physical scale transition | NOT RUN. Not required merely for formality. Composition-active deferral can establish the invariant deterministically, but the actual owner/geometry/commit state machine still violates it (F01/F04/F05). Correct and test those paths; an unavailable physical combined run is not the reason for rejection. |
| Foreground ElementProviderFromPoint | NOT RUN. Not independently a blocker given focused hit-test authority and finite native UIA geometry. Separate escaped-reference defects remain F07. |

## Gates 1–20, limited to this closure surface

The canonical taxonomy contains exactly **20 gates**. These statuses evaluate
the corrected foundation, not whether the review document was written.

| Gate | Status | Reason |
| --- | --- | --- |
| 1 — Requirements/completeness | FAIL | Material closure defects remain. |
| 2 — Assumptions/approval boundaries | PASS | Approved architecture retained; unavailable environmental cases are explicitly bounded. |
| 3 — Contract preservation | FAIL | Deferred ownership, final composition commit, effective geometry/style and damage invariants remain violated. |
| 4 — Scope | PASS | Corrections stay within prior findings; no Gallery or product expansion. |
| 5 — Minimal design | PASS | Existing owners can implement the bounded fixes; no replacement architecture needed. |
| 6 — Authority/ownership | FAIL | Replay/composition ownership and pending/effective geometry diverge. |
| 7 — Semantic duplication/reuse | FAIL | Interaction text-metric classification duplicates and disagrees with the shared classifier; native effective colors bypass shared enforcement. |
| 8 — Unrequested fallback/legacy | PASS | No new fallback/backend/editor path introduced. |
| 9 — Deterministic stable contracts | FAIL | Representability, stale range arguments, final-edit ordering and timer failure progress remain incomplete. |
| 10 — Canonical authority/status | FAIL | Current identity, command and preservation accounts remain stale. |
| 11 — Failure safety | FAIL | Unsafe COM array insertion, ignored wake failures and silent deadline arming failure. |
| 12 — Boundedness/resources | FAIL | Provider reference leak, offset arithmetic overflow and stranded mandatory teardown. Outer queue and timer bounds themselves are improved. |
| 13 — Dependencies/blast radius | PASS | No unrelated dependency/runtime/platform expansion found. |
| 14 — Redundant work/performance | PASS | Prior repeated-timer and any-branch state-work defects are corrected; no new permanent framework loop found. This is not leak freedom or a broad performance guarantee. |
| 15 — Infrastructure boundary | PASS | Normal consumers retain the approved boundary. |
| 16 — Proof sufficiency/reuse | FAIL | Composition deferral and nonempty escaped arrays lack proof; IME counter can false-pass. Portable/native suites are independently verified. |
| 17 — Proportional recovery | PASS | Narrow corrections remain feasible in existing authorities. |
| 18 — Immutability/identity | PASS | Valid production/harness/publication chain, matching executable hashes, immutable prior reviews. |
| 19 — Artifact honesty | FAIL | Unsupported proof claims, historical image labeled current and contradictory current visual artifacts. |
| 20 — Systemic invariant audit | FAIL | Selected positive assertions do not close all owner, continuation, native-state, effective-style and failure routes. |

## Smallest correction set before freeze

1. **Native entry/progress:** explicit arrival-owner replay state; consistent
   IME/focus ownership; checked wake scheduling everywhere; mandatory cleanup
   independent of ordinary queue drain budget. Assert the exact boundary traces.
2. **Geometry/composition:** exact checked representability; one effective peer
   geometry during pending relatch; acknowledge final IME edit before resolving
   pending proposal and relatching. Exercise these deterministic combined paths.
3. **Timers/UIA:** propagate framework deadline arming failure; correct direct
   interface-pointer array insertion and reference balancing; reject dead
   range arguments; retain/query nonempty escaped children/ranges in fixtures.
4. **Style/Shadow:** reuse the shared visual damage classifier; live system-aware
   native colors; checked offset arithmetic; commit actual state/clip-dependent
   ink after paint, with old/new damage assertions.
5. **Harness/evidence/docs:** mandatory observable no-submit and application
   committed-state assertions; checked input success; complete JSON escaping;
   resolve current capture discrepancy; correct current identity/CLI/proof
   wording and label historical visuals. Preserve both earlier reviews.

No complete native matrix replay is required merely to repeat unaffected
proof. Run the focused corrected cases, then publish honest evidence tied to
the resulting immutable production and harness identities.

The Windows foundation is not ready to freeze or merge toward main. The
post-foundation Shadcn Component Gallery milestone is not cleared. The platform
contract direction remains accepted; `ARCHITECTURE_BLOCKED` is unwarranted.

SHADCN_COMPONENT_GALLERY_NOT_CLEARED

REWORK_REQUIRED
