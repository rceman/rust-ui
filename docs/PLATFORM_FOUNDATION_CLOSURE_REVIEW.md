# Independent Windows foundation closure review

**Review date:** 2026-10-02. **Verdict:** `REWORK_REQUIRED`.

This reviews closure of F01–F13, not a replacement architecture. The shared
platform-contract direction remains sound. The corrected implementation has
materially improved, but its remaining native ownership, state, timer,
accessibility and rendering defects prevent foundation freeze.

## Immutable identities and scope

| Item | Exact identity |
| --- | --- |
| Repository / branch | `rceman/rust-ui` / `agent/windows-native-composer-contract-spike-v0.1-swe2` |
| Previous independent review commit | `1603547c2d9238014ed519404535828dd4b7dfe5` |
| Previous implementation reviewed | `cd150e4ebe120b21fbd4517fb6efcfc8360e69d4` |
| Corrected frozen production implementation | `0d0e020a788e918734ee1c7bdbb20e0d52e43ae9` |
| Evidence/documentation snapshot reviewed | `afe5bfc7c5367e40db15e0fc0692b03756647b76` |

`git diff 0d0e020a... afe5bfc7...` contains only files under
`benchmark/results/windows-native-composer-spike-v0.1/`, including its README.
There are no intervening production, example, dependency or test changes.
`afe5bfc7...` is therefore an evidence snapshot over the single implementation
candidate, not another implementation candidate.

The previous `docs/PLATFORM_FOUNDATION_REVIEW.md` is byte-identical between
`1603547...` and `afe5bfc7...` and remains untouched by this review. This new
document is the only publication change. Production and evidence are unchanged;
Component Gallery work and merging `main` are outside this review.

## Review environment and independently executed checks

Source inspection used a detached checkout of `afe5bfc7...`. Native verification
and publication used the clean authoritative checkout
`W:\devin_folder\rust-ui`, on the requested branch at that same HEAD.
The review host is WSL2 Linux `6.18.40.1-microsoft-standard-WSL2`, x86_64,
Rust/Cargo 1.93.1. Windows interop reaches Windows 11 Home, native Rust 1.94.0.

| Independent check | Result and scope |
| --- | --- |
| `cargo test --locked --lib`, Linux | PASS, **76/76**; no Linux UI backend needed. |
| `cargo test --locked --lib`, native Windows | PASS, **85/85**, including RichEdit painting/relatch and UIA lifecycle tests. Assertions were also inspected; execution does not establish assertions the tests omit. |
| `cargo fmt --check` | PASS. |
| `cargo check --locked --all-targets --target x86_64-pc-windows-gnu`, Linux | PASS, with warnings. This is compilation, not Windows execution. |
| Exact candidate geometry module, external scratch executable | Prior fractional counterexample now produces local `(0,0)` and round-trips correctly. A separate representability case reproduces unchecked integer overflow in `PeerOrigin::to_local`. No repository tests or source were added. |
| Native binary hashes | Both existing Windows example binaries match `identity.txt`, independently computed with `Get-FileHash`. |
| Fresh native composer/probe smoke | PMv2 asserted; actual window DPI 120; 24 UIA children; draft value `draft-prefill`. Synthetic 144-DPI message changed the window to 750×705 physical pixels; actual monitor DPI remained 120. Draft UIA rectangle remained finite: `[51,120,699,182]`. |
| Fresh graceful close of that review-owned process | Exit 0; counters emitted to a temporary file: `uptime_ms=1115 requested_redraws=81 animation_callbacks=0 animation_timer_fires=0 caret_redraws=82 native_timer_fires=0`. This was a short lifecycle smoke, not another 30-second performance run or leak test. |
| Evidence inspection | Parsed identity, geometry and performance receipts; inspected actual `final-foundation.png` and `ime.png`; audited script/CLI compatibility and artifact history/encoding. |

Temporary verification logs were outside the repository. The first native test
wrapper stopped on a compiler warning because Windows PowerShell treated native
stderr as an error; the corrected wrapper subsequently completed all 85 tests.
That wrapper issue is not a library failure.

**Reported, inspected, but not independently replayed:** the full collection
matrix, real SendInput/IME acceptance, the 30-second idle measurement, all four
synthetic DPI captures, multiline/scroll interaction, Send→Stop→resend, and
100/1000-row collection scenarios. The native test's four requested scales were
independently executed, but are not four physical monitor environments.

## F01–F13 disposition

| Finding | Status | Smallest remaining defect |
| --- | --- | --- |
| F01 | PARTIALLY_CLOSED | Guard error-path native entry; bind and bound the outer owned-message queue; check continuation failures and guarantee teardown despite drain errors. |
| F02 | CLOSED | Core scale meaning and Windows mapping are now correctly separated. |
| F03 | PARTIALLY_CLOSED | Use the checked packing authority in the probe and define checked representability at peer-local and logical/physical boundaries. |
| F04 | PARTIALLY_CLOSED | Restore backward selection correctly; make relatch state/failure ordering safe; establish composition preservation independently of retaining the COM object. |
| F05 | PARTIALLY_CLOSED | Finish the existing character/system-input routes, checked selection/composition boundary, and single-line Shift+Enter contract. |
| F06 | PARTIALLY_CLOSED | Rearm existing peer timers without orphaning prior IDs; prevent stale-ID redirection; make release and framework arming failure agree with native state. |
| F07 | PARTIALLY_CLOSED | Close native provider escape paths and SAFEARRAY ownership; prove retained native patterns/ranges, not only painted fragments. |
| F08 | PARTIALLY_CLOSED | Apply system-role enforcement to every role consumer and classify actual interaction-state differences. |
| F09 | PARTIALLY_CLOSED | Correct offset exclusion/cache identity, checked extent arithmetic, committed old/new damage, and the remaining mask failure leak. |
| F10 | CLOSED | Portable helpers compile and execute; Linux 76/76 and native Windows 85/85 independently pass. |
| F11 | PARTIALLY_CLOSED | Remove remaining PowerShell DPI assertions and unchecked probe packing; make IME success require newly committed kana and observable no-submit/committed-value invariants. |
| F12 | PARTIALLY_CLOSED | Identity and final visual are corrected; reconcile performance prose, readable Unicode receipts, historical artifacts and the remaining unsupported proof claims. |
| F13 | PARTIALLY_CLOSED | Correct/label stale API identity and canonical status/proof descriptions; retain explicitly historical design prose. |

### F01 — BLOCKER: native entry and deferred ownership

The initial turn is now guarded. Reentrant `WM_DPICHANGED` copies its RECT into
owned storage; the borrowed pointer is no longer asynchronously reposted.
Reentrant `WM_NCDESTROY` closes/detaches immediately and queues full shutdown.
Peer delivery carries NodeId, checks generation, propagates delivery errors and
posts continuation when its bounded drain leaves work. These are real repairs.

Remaining defects are visible in
[window.rs](https://github.com/rceman/rust-ui/blob/0d0e020a788e918734ee1c7bdbb20e0d52e43ae9/src/platform/win32/window.rs#L43)
and [backend entry/drain](https://github.com/rceman/rust-ui/blob/0d0e020a788e918734ee1c7bdbb20e0d52e43ae9/src/platform/win32/mod.rs#L1896):

- `bail(&mut Backend, ...)` calls `DestroyWindow` without entering the native
  ownership guard. Its synchronous WndProc can construct another mutable backend
  reference during that backend operation. The initial-turn repair does not
  close this error path (`mod.rs:2196`).
- Outer `QueuedMsg` stores no arrival-time NodeId/generation. Deferred character,
  key and peer-focus delivery resolves `self.focus` later. The inner peer queue's
  generation fence cannot preserve an owner the outer queue never captured.
- Outer pushes have no capacity check. The 128-iteration drain bounds execution,
  not allocation. It returns `QueueOverflow` after the 128th dispatch without
  checking whether the queue has just become empty; remaining ordinary work has
  no continuation. A queued destruction marker is also bypassed if an earlier
  delivery errors or exhausts that drain.
- Pump/continuation `PostMessageW` failures remain ignored, including after
  setting `pump_queued=true`. Required progress can be recorded as scheduled
  when no wake was delivered.

The owned-queue design is appropriate. It needs one complete entry/exit contract,
saved peer identity, enqueue capacity, checked progress and unconditional teardown;
no new application event architecture is required.

### F02 — CLOSED: platform-neutral scale

`ScaleFactor` now means physical pixels per rust-ui logical unit;
`new` checks positive finite ratios. `Dp` has no physical-inch guarantee.
`from_dpi`/`dpi` have left core; `scale_from_dpi` and `dpi_of`, including Windows'
96 baseline, reside in `win32/space.rs`. The public contract agrees. A macOS
backend can directly supply a backing ratio such as 2.0.

This closes the previous architectural leakage. Unchecked representability is a
separate F03 issue. The old introductory geometry comment generalizing native
physical-pixel spaces is stale wording, not a surviving Windows definition of
`ScaleFactor`.

### F03 — MAJOR: snapped origin repaired; range boundary incomplete

Pointer, hover, caret and RichEdit screen/client callbacks now use the snapped
`PeerOrigin`. The exact previous case, scale 1.25, logical origin 0.4, client px 1,
now yields peer-local px 0. Independent scratch execution imported the actual
candidate geometry module and verified its round trip. Checked client/screen
adapter failures and signed-16 LPARAM packing are also present.

However, `PeerOrigin::to_local`/`to_client` use unchecked i32 subtraction/addition.
For origin `i32::MIN` and client x=1, the former panics in a debug build; release
arithmetic can wrap. Logical conversions saturate float-to-integer casts without
a failure contract. These are representability defects, not evidence of another
fractional-origin mismatch under normal coordinates.

[Probe `click_post`](https://github.com/rceman/rust-ui/blob/0d0e020a788e918734ee1c7bdbb20e0d52e43ae9/examples/native_probe.rs#L218)
still masks coordinates with `0xffff` instead of consuming `try_lparam_px`.
It silently truncates coordinates outside the packing range. Export/use the
existing checked helper at this dev seam and check unrepresentable geometry;
do not introduce another conversion authority.

HIMETRIC in the RichEdit adapter and fractional raster math remain justified,
narrow native exceptions. Negative and fractional coordinates themselves are
supported; overflow handling remains the incomplete part.

### F04 — MAJOR: explicit activation helps, but preservation is not proven

`Inactive`/`InPlace`/`Ui` now distinguish measurement/drawing from focus-owned UI
activation. Activation failures propagate. Fresh measurement clears the stored
extent and rejects a request without a new positive extent. The native regression
now measures multiline growth; the recorded composer grows to its max-lines
visible cap. Those previous measurement defects are substantially corrected.

[Scale relatch](https://github.com/rceman/rust-ui/blob/0d0e020a788e918734ee1c7bdbb20e0d52e43ae9/src/platform/win32/text.rs#L1092)
still changes bounds/scale before deactivation and silently ignores a failed
selection snapshot via `.ok()`. A partially failed UI/in-place deactivation can
also leave the bookkeeping state ahead of the native transition.

More concretely, selection restoration calls `SetStart(anchor)` then
`SetEnd(focus)`. For a backward selection such as anchor 7/focus 5, TOM collapses
the range to 5/5; changing `tomSelStartActive` afterward cannot restore its extent.
Microsoft explicitly documents this collapse in
[ITextRange::SetEnd](https://learn.microsoft.com/en-us/windows/win32/api/tom/nf-tom-itextrange-setend).
Restore the sorted range and then its active-end direction, with checked offsets.

The passing native relatch leg sends VK_SHIFT messages directly to the service
without establishing OS modifier state. It never asserts a nonempty backward
selection was obtained. It compares `EM_CANUNDO` booleans without requiring undo
to exist or exercising undo/redo; caret and focus preservation are not asserted.
The test therefore does not prove everything its comment and README claim.

**Active composition across relatch:** retaining the COM instance proves object
identity, not preservation of the external IME/TSF focus/context lifetime through
UI deactivation and TOM selection mutation. There is no composition-aware
relatch guard or documented native guarantee establishing that invariant.
Committed-text/selection/undo checks cannot substitute for it. This is an
unresolved native-state contract, not a reproduced composition-loss incident.
It can close with a narrowly justified native strategy plus focused proof; the
specific unavailable environmental scenario need not be mandatory if equivalent
proof establishes the composition ownership boundary.

### F05 — MAJOR: input repairs leave semantic gaps

Real modifiers and drag-button metadata are now supplied. `CustomRender::hit_test`
is actually called; disabled interactive nodes remain universally inert.
TextArea Enter/Shift+Enter have an explicit mutually exclusive submit/edit fork.
The platform-scoped `Key::Other` is acceptable as an opaque escape outside the
portable vocabulary: a future backend need not manufacture Windows VK values.
Portable application logic should use the normalized variants. A complete new
keyboard abstraction is unnecessary.

Remaining gaps in [input/commit routing](https://github.com/rceman/rust-ui/blob/0d0e020a788e918734ee1c7bdbb20e0d52e43ae9/src/platform/win32/mod.rs#L845):

- `native_selection` still falls back to `text.len()` on failed conversions,
  and does not gate selection notifications during composition against the
  committed snapshot. `native_commit` has checked conversion; this other path
  does not share that guarantee.
- `WM_IME_ENDCOMPOSITION` delivers/drains peer notifications **before** clearing
  the composing flag. `native_commit` discards every notification while that
  flag is set. `ime_end` then resolves queued proposals without an explicit
  final committed read-back. A final commit emitted during that drain can be
  discarded as preedit. Define/prove the final-commit order; native UIA text
  alone does not establish app `TextValue` agreement.
- `WM_CHAR` has only a native-peer route; the public `Key::Char` semantic route
  is absent for painted/custom handlers. System-key classes are listed as
  deferrable but have no corresponding normal native dispatch branch. Finish
  the supported routes or explicitly narrow their shared contract.
- Single-line Shift+Enter follows the generic Shift→Edit branch, whereas the
  canonical boundary says plain Enter submits even with Shift on a single-line
  input. Normal submit is also decided before native interpretation, conflicting
  with the canonical account. Align the implementation and its policy without
  introducing double submit/newline effects.

The inspected real-IME image visibly contains hiragana. It does not establish
preedit suppression, final app-value synchronization or absence of submit.

### F06 — MAJOR: timer identity and armed-state authority remain incomplete

`TxSetTimer` now arms synchronously and returns FALSE on capacity/native failure.
Live IDs are collision-free and ticks remain periodic until killed. Native
Windows allocator tests pass. The framework deadline route kills its timer on
fire; inspection found no unconditional framework idle loop.

[Host timer arming](https://github.com/rceman/rust-ui/blob/0d0e020a788e918734ee1c7bdbb20e0d52e43ae9/src/platform/win32/text.rs#L392)
allocates a fresh ID for every request. Repeating the same `(node, idtimer)`
overwrites `armed_timers[idtimer]` without killing/removing the old native timer
and pool entry. Both registrations can keep firing, and `TxKillTimer` finds only
the latest. This contradicts the backend comment claiming rescheduling of the
same Win32 ID.

The normal `TextPeer::release` nulls `host`; explicit per-host timer cleanup
exists only in `Drop` when `host` is non-null. Native teardown may call timer
cancellation, but the framework has no authoritative release cleanup for the
orphan above. Window shutdown does clear the pool, which is a useful backstop.

Modulo ID recycling after cancellation also allows an already posted old tick
to resolve to a newly allocated owner. The message has only the reused Win32 ID;
checking the *new* map's NodeId cannot recover the old generation. Microsoft's
[KillTimer contract](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-killtimer)
states that queued WM_TIMER messages remain.

Finally, `rearm_deadline` ignores `SetTimer` failure while storing `Some(deadline)`.
Repair repeated arming, stale-token reuse, shared release cleanup and checked
framework arming. No additional general timer framework is needed.

### F07 — MAJOR: native references still escape, and range arrays leak

Stable live child state, generation-bearing native lookup, move/resize/DPI
refresh, focus initialization and root cycle-breaking improve the implementation.
Text/value patterns and most range-returning methods now have explicit fences.
The retained-provider lifecycle test genuinely proves painted fragment liveness
through reorder/disable/remove/recreate/close.

[Native wrappers](https://github.com/rceman/rust-ui/blob/0d0e020a788e918734ee1c7bdbb20e0d52e43ae9/src/platform/win32/uia.rs#L552)
still have escape paths: `EditorFragment::FragmentRoot` delegates without its
live check; range `GetEnclosingElement` returns the raw native simple provider;
`GetChildren` returns unfenced provider arrays. Clients can retain these and
obtain native patterns outside the rust-ui fence. An equivalent native
disconnection guarantee for these references is not established.

`fence_range_array` allocates a replacement SAFEARRAY but never destroys the
owned array returned by the native provider. Failure after allocating the output
can also leak it. There is additionally no focused proof that methods receiving
another range correctly unwrap a fenced argument before delegating to RichEdit.

The lifecycle test sets `native: None`; it does not exercise escaped RichEdit
patterns, ranges or enclosing providers. Close the escape/array ownership paths
and extend the existing native fixture to retain those actual objects through
removal and close. Successful root disconnection alone is insufficient proof.

### F08 — MAJOR: shared style authority exists; enforcement/work are incomplete

The full duplicate renderer palette is gone: it delegates to `style.rs`.
Authored role identity is distinct from concrete equality. Surgical per-side
patching, radii/insets, arbitrary RGBA, state patches, patch removal and typed
Shadow share the public representation. The existing surgical test passes;
there is no private second built-in style vocabulary. The `0d0e020a...` ordering
fix consumes state dirty bits before deciding layout, closing that specific bug.
`ReducedMotion::System` now re-resolves after OS setting changes.

The forced-colors resolver is not consumed universally. Window clearing,
CustomCanvas role fill/text and focus rings call the ordinary dark/light role
resolver. Native foreground updates call `resolve_color(authored, dark)` and
the peer selection palette ignores forced colors. These paths can disagree with
system-enforced surfaces/text. See
[render consumers](https://github.com/rceman/rust-ui/blob/0d0e020a788e918734ee1c7bdbb20e0d52e43ae9/src/platform/win32/render.rs#L347)
and [peer colors](https://github.com/rceman/rust-ui/blob/0d0e020a788e918734ee1c7bdbb20e0d52e43ae9/src/platform/win32/text.rs#L1222).

Interaction work uses `any_metric_branch`, rather than the actual old/new
resolved state: pressing can demand layout because an unrelated hover branch
has metrics, and a visually unchanged transition still demands paint. Compare
the effective transition using the existing resolver/classifier. Equal staged
updates avoid layout/paint, although `updated` still rebuilds UIA.

Side-border painting now partitions corners with trapezoids. Its claim that
arbitrarily thick opposing sides cannot overlap is not justified when summed
insets exceed the box extent; the inner corners cross. Bound/normalize that case
and test the renderer, rather than changing public border semantics.

### F09 — MAJOR: Shadow pixels, cache identity, bounds and damage

The one outer Shadow descriptor remains narrow: color, offsets and sigma, no
layout mutation, idle timer or hidden general effects. The renderer has a
48-entry/32-MiB cache and target-generation invalidation. Oversized raster
requests now return an error instead of silently succeeding. HostBox creation/QI
has a cleanup guard, and fallible border allocations precede PushLayer.

Four correctness gaps remain in
[shadow raster/cache](https://github.com/rceman/rust-ui/blob/0d0e020a788e918734ee1c7bdbb20e0d52e43ae9/src/platform/win32/render.rs#L989)
and [damage bookkeeping](https://github.com/rceman/rust-ui/blob/0d0e020a788e918734ee1c7bdbb20e0d52e43ae9/src/platform/win32/mod.rs#L497):

1. A mask pixel is drawn at `r + offset - pad + pixel`. Original-interior
   exclusion must sample the original solid at `pixel + offset`, but the code
   samples `pixel - offset`. At sigma 0, width 10 and x-offset +2, this leaves
   ink on the translated left strip inside the box instead of the outer right
   strip. The corrected zero-blur path is still geometrically wrong.
2. `ShadowKey` omits offsets although the baked exclusion now depends on them.
   Different-offset shadows can reuse the wrong cached hole.
3. `2 * pad` and extent additions remain unchecked **before** dimension/budget
   guards. A permitted finite `f32::MAX` sigma makes derived padding overflow;
   an error after that arithmetic is too late.
4. `committed_ink` is overwritten during relayout and removed IDs are pruned
   before paint. Paint-only shadow changes do not commit their new footprint
   there. Actual hot/pressed/focus branches and old/new ancestor clips are not
   preserved. Full-frame clearing masks this; it does not prove retained damage.

Also, border `D2D1_LAYER_PARAMETERS` creates a ManuallyDrop mask clone before
fallible brush/geometry allocation. An early `?` still leaks that clone even
though no layer has yet been pushed. Move the ownership acquisition after the
fallible work or guard it. Preserve the successful PushLayer/PopLayer repair.

These are defects in the existing primitive and lifecycle, not requests for
future optimization or additional effects.

### F10 — CLOSED: portable tests

The previously Windows-gated portable helpers are corrected. Both independently
executed suites pass. Native tests stay gated, and Linux runs do not manufacture
a Linux UI backend. No mandatory Tokio or hidden CI dependency was introduced.

### F11 — MAJOR: harness authority and assertion quality

PowerShell's former input/UIA implementation is substantially removed. The
actual `dpichange <window> <dpi>` CLI is now invoked correctly, and named body
lookup is fixed. Rust geometry asserts/records PMv2; the fresh native smoke
independently exercised that assertion.

However, [S-Dpi](https://github.com/rceman/rust-ui/blob/afe5bfc7c5367e40db15e0fc0692b03756647b76/benchmark/results/windows-native-composer-spike-v0.1/collect.ps1#L153)
still computes `500 * dpi / 96`, physical dimensions and their tolerance in
PowerShell. The Rust `dpichange` command only sends/waits; it does not own the
read-back assertion. Probe pointer packing also bypasses the checked production
authority. Scenario sequencing/capture/lifecycle are legitimate orchestration;
these conversions/assertions are not.

[IME acceptance](https://github.com/rceman/rust-ui/blob/0d0e020a788e918734ee1c7bdbb20e0d52e43ae9/examples/native_probe.rs#L475)
now records `before` and `gained`, but counts kana in **after**, not in **gained**.
If kana already exists and input does nothing, `after == before` passes. SendInput
acceptance counts are ignored; the post-read is hardcoded to `draft` instead of
the requested editor. `no_submit:true` is printed without measuring submit.
The prefix test cannot prove no-submit: the actual composer's Send handler leaves
the draft intact while starting its effect. A native provider's preedit/value
also does not prove committed application `TextValue` agreement.

Require a nonempty new committed kana suffix, assert accepted input and the
relevant app/preedit/submit outcomes, and place DPI read-back in the Rust command.
Use valid JSON string escaping for values, including multiline text. These are
bounded improvements to the existing harness, not a new verification framework.

### F12 — identity and visual corrected; remaining evidence honesty

The receipt contains all requested fields, and actual Windows files independently
match its hashes:

```text
source: 0d0e020a788e918734ee1c7bdbb20e0d52e43ae9
composer SHA256: 1D603D649E863E53FCB053BFD62197B22BEACA448EA83C7595CE4F39721D2733
native_probe SHA256: 8F61130482039B5E86146CBE94018BDE982D0D1FBF106F8F469A4D74B8E223DF
run UTC: 2026-10-01T19:57:30.0359253Z
```

`smoke.json` records PMv2, DPI 120 and scale 1.25. The actual final screenshot
now places row text inside its pills; the prior contradictory capture is
resolved. The IME image visibly shows `draft-prefillありがと`.

**MINOR:** README performance numbers are stale. Actual files say **32.1 MiB,
17 threads**, uptime **30535**, redraw/caret **78/78**; README says 34.6 MiB,
18 threads, 30568 and 81/82. Actual cumulative counters have zero animation
callbacks/timer fires and zero native timer fires. The scripts now close
gracefully, and independent smoke confirmed counter emission. This is not a
native resource-leak baseline or an interval-separated frame count. One smoke
does not establish performance for every workload.

**MINOR:** `ime.json`, `ime.txt` and `unicode.txt` cannot be decoded as UTF-8.
The JSON has no declared encoding. This undermines machine-readable Unicode
proof; it does not show that the native editor failed to render Unicode.
Use an explicit consistent encoding. Byte-identical artifacts may have been
recollected, so unchanged Git history alone does not prove a stale run.

The older `evidence-final-geometry/uia/value.json` files are not generated by the
current collector and retain the prior collection's history; label them
historical. The README's fresh identity is valid, but its assertions that all
native semantics left PowerShell, gained kana is required, and relatch proves
composition preservation are contradicted by F04/F11. Those are **MAJOR** proof
overclaims, not cosmetic filenames. Repair assertion quality before refreshing
the final acceptance account. Idle, normal teardown and leak freedom are three
different claims.

### F13 — MINOR canonical corrections, plus the material proof claims above

Core scale meaning, current direct TxDrawD2D, three activation states and editor
`TextInputStylePatch` are now correctly described. Real IME/key inventory is no
longer marked pending. Deviation 2 explicitly marks peer-local bitmap rendering
superseded; that historical explanation is acceptable.

`SPIKE_API_REVIEW.md` still identifies `0405a80`, rather than the frozen source
or an explicitly historical API snapshot. The inventory names nonexistent
`native_probe uia-tree` where the CLI is `uia`. Canonical documents still contain
concept-only/no-tests-executed status language without a clear link separating
original design exercises from today's implementation/proof status. Keep their
historical signatures/chronology; add accurate current status and identity.

Composition-preservation and universal harness/enforcement claims need correction
against the implementation, not wording that simply reclassifies unproven work
as passed.

## Newly discovered material regressions within the existing finding scope

These are not a new architecture taxonomy or renamed style preferences:

| Item | Severity | Concrete defect / existing scope |
| --- | --- | --- |
| N01 | MAJOR | Newly added directional selection restoration collapses backward ranges. F04. |
| N02 | MAJOR | Fresh per-request timer allocation orphans an earlier timer on repeated native ID. F06. |
| N03 | MAJOR | New range fencing fails to release the native SAFEARRAY it replaces. F07. |
| N04 | MAJOR | Offset-dependent exclusion is cached without offset identity; its sampling sign is also wrong. F09. |

No new evidence establishes that the platform-contract direction is fundamentally
unsound. Remaining defects can be corrected within its existing owners.

## Single-authority audit and future-platform paper test

| Concept / claimed authority | Independent disposition |
| --- | --- |
| Logical↔physical — `geom::ScaleFactor` | Single ratio/rounding authority is present. F02 closed; checked representability remains F03. |
| DPI↔ratio, client↔screen — `win32/space.rs` | Correct narrow Win32 mapping and checked OS conversion. Probe packing and PowerShell DPI assertions remain competing consumer implementations. |
| Client↔peer-local — `geom::PeerOrigin` | Normal pointer/caret/host paths agree and old fractional counterexample is fixed. Range checks remain. |
| Logical layout/insets — `layout.rs` | Shared inset/content layout is reused by the Win32 layout adapter; native measurement is legitimate adapter work. No second browser layout machinery found. |
| Input — Win32 dispatch/normalization | Modifier and hit ownership repairs are real. Normal/deferred dispatch and committed/selection/submit boundaries remain incomplete. |
| Committed/native text — `win32/text.rs` plus shared `TextPeerSync` | Shared revisions/proposals versus native editing state is a legitimate split, not duplicated text ownership. Relatch and final-composition synchronization still need proof/correction. |
| Accessibility — `win32/uia.rs` | Derived semantic snapshot is legitimate. Escaped native objects/array ownership defeat complete liveness. |
| Native timers — shared pool | One routing pool exists, but local lookup, global registration and native armed state still diverge. Framework scheduler policy is appropriately separate; native arming failure must be checked. |
| Palette/style — `style.rs` | One palette/model exists. System enforcement is bypassed by some consumers; interaction work classification still guesses from any branch. |
| Forced colors/system — semantic slots + platform resolver | Slot mapping and GetSysColor adapter are appropriate. Apply them through all role consumers. |
| Native resources — render/text/UIA owners | Bounded renderer caches are present. Native array, mask and timer lifetime defects remain; performance counters do not account for them. |
| Validation/native semantics — `native_probe.rs` | Direction is correct, but unchecked packing, weak IME assertions and PowerShell DPI rules leave the claimed cutover incomplete. |

Window lifecycle and event delivery can remain typed modules; no symmetric trait
framework is needed. macOS can realize the same logical geometry, text revision,
input, accessibility and rendering contracts using native backing scale and its
own API units. It does not need Windows DPI, VK meaning for portable keys, a
second public event model or a second accessibility semantic tree. Clipboard is
explicitly deferred because no shared consumer exists; RichEdit owns its native
editing clipboard path. Current defects are in Windows realization/verification,
not a reason to replace the approved semantic direction.

The proof hierarchy remains credible in design: deep shared authority tests,
focused native adapters, representative consumers, broad freeze acceptance.
The code must finish consuming those authorities and assert the actual invariant
at each layer. Repeating a formula in a script is not proof reuse.

## Environmental NOT RUN disposition

| Case | Residual-risk judgment |
| --- | --- |
| Dead keys | NOT RUN. Not independently a freeze blocker: Unicode/native character delivery and real IME provide related evidence. Keep layout-specific acceptance outstanding; do not call it PASS. |
| Physical monitor DPI transition | NOT RUN. Not independently a blocker once F03/F04 are correct: ratio/origin conformance, native raster matrix and synthetic DPI are useful substitutes for the bounded contract. They do not measure a physical move or four monitor DPIs. |
| Active IME during scale relatch | NOT RUN. The existing substitute is insufficient: COM identity and committed state do not prove external composition/focus lifetime across UI deactivation and selection mutation. Close that precise invariant with a justified strategy and focused native/contract proof; do not fail merely for lacking this particular environmental recording. |
| Foreground ElementProviderFromPoint | NOT RUN. Not independently a blocker: focused snapshot hit tests and finite native UIA bounds bound routing risk. Separate retained native provider lifetime defects remain F07. |

Fresh native tests and smoke do not silently convert any of these to PASS.

## Universal Gates 1–20

`docs/QUALITY_GATES.md` contains exactly Gates 1 through 20. Their strengthened
6/7/9/16/20 meanings are retained. These statuses assess implementation closure,
not merely completion of this review or Agent Worker's labels.

| Gate | Status | Evidence / reason |
| --- | --- | --- |
| 1 — Requirements/completeness | FAIL | Material F01/F03–F09/F11 remain; closure is incomplete. |
| 2 — Assumptions/approval boundaries | PASS | Architecture/scope are approved and environmental NOT RUN limits are explicit. Unsupported proof is separately rejected below. |
| 3 — Contract preservation | FAIL | Backward selection, repeated timers and shadow cache/pixels violate retained native/style contracts. |
| 4 — Scope | PASS | Rework traces to prior findings; no Component Gallery/product expansion found. |
| 5 — Minimal design | PASS | Existing narrow modules/typed contracts remain appropriate; no speculative platform framework is needed. |
| 6 — Authority/ownership | FAIL | Native entry, timer/native-state ownership and verification boundary are incomplete. |
| 7 — Semantic duplication/reuse | FAIL | Probe packing and PowerShell DPI semantics still bypass production authorities. |
| 8 — Unrequested fallback/legacy paths | PASS | Current renderer uses direct TxDrawD2D; no active alternate editor or bitmap workaround. Historical prose is distinguishable. |
| 9 — Stable deterministic semantics | FAIL | Deferred ownership, native selection/composition, timer reuse and representability are not yet universal guarantees. |
| 10 — Canonical authority/status | FAIL | API/source/status and proof accounts need the F12/F13 corrections. |
| 11 — Failure safety | FAIL | Unguarded error-path native entry, ignored wake/arming failures and resource cleanup holes. |
| 12 — Boundedness | FAIL | Outer queue lacks enqueue capacity; shadow extents overflow before guards; timer/array leaks remain. |
| 13 — Dependencies/blast radius | PASS | No mandatory runtime, CSS/browser/plugin/other-platform implementation or hidden CI expansion. |
| 14 — Redundant work/performance | FAIL | Repeated native timer requests can create continuing redundant callbacks; state classification schedules unnecessary layout/paint. No generic future optimization demanded. |
| 15 — Infrastructure boundaries | PASS | Native handles and evidence mechanics remain outside normal shared consumer flows. |
| 16 — Proof sufficiency/reuse | FAIL | Passing suites do not establish omitted directional selection, undo/redo, composition or native range-lifetime assertions; harness false-positive remains. |
| 17 — Recovery proportionality | PASS | Bounded correction within current owners remains feasible; no restart/redesign justified. |
| 18 — Immutability/identity | PASS | One frozen source, evidence-only successor, matching native hashes, unchanged historical review; no production/evidence mutation during review. |
| 19 — Artifact honesty | FAIL | README proof/performance discrepancies and canonical/encoding/historical-label corrections remain. |
| 20 — Systemic invariant completeness | FAIL | Authority matrix audit still finds explained but unresolved violations; selected positive tests do not prove the global claims. |

## Smallest correction set before another closure review

1. **Native entry:** guard `bail` and every native-entering backend operation;
   preserve original peer/generation in outer deferred input; bound enqueue;
   continue bounded drains correctly; check wake failure and run shutdown even
   when a drain fails. Add focused nested/error/destruction/owner-change proof.
2. **Geometry:** route probe packing through the checked authority and reject
   unrepresentable transformations before wrapping/saturating valid-looking data.
3. **Native text/input:** restore sorted selection plus direction; check snapshots
   and transition state; prove real range/caret/focus/undo/redo preservation;
   establish composition-aware relatch and final committed-value synchronization;
   eliminate selection fallbacks; align existing submit/character/system routes.
4. **Timers:** reuse or replace a peer's existing native registration correctly;
   reject old queued tokens after reuse; share deterministic release cleanup;
   honor framework arming failure. Test repeated IDs, cancellation/reuse and
   long-lived periodic behavior, not just allocator capacity.
5. **Accessibility:** fence every escaped provider/range or demonstrate native
   disconnection equivalence; own/destroy replaced arrays and failure output;
   test retained actual RichEdit patterns/ranges across removal/close and range
   operations involving wrapped arguments.
6. **Style/rendering:** finish system-role enforcement and actual transition work
   classification; handle oversized opposing borders; correct shadow exclusion
   and cache key; check arithmetic before allocation; retain old/new painted ink
   and clips until damage is accumulated; guard the mask clone on failures.
7. **Harness/evidence/docs:** move DPI assertion into Rust, use checked geometry,
   make IME proof require new committed kana and observable app/no-submit state;
   refresh source/binary-identified affected acceptance after fixes; reconcile
   canonical statuses, performance prose, encoding and historical labels. Keep
   unavailable environmental cases explicitly NOT RUN.

Preserve the fixes already proven: neutral scale semantics, snapped-origin math,
direct native text rendering, fresh measurement, portable tests, dirty-bit
ordering, typed shared styling and valid artifact identity. Corrections above
are required before freeze; they do not authorize Gallery, application-shell,
AppCommand or Mascot migration work.

The branch can remain the rework base, but `0d0e020a...` is not cleared as the
frozen foundation or for merging toward `main`. After corrections and focused
closure proof, the same platform-contract direction can support the next milestone.

SHADCN_COMPONENT_GALLERY_NOT_CLEARED

REWORK_REQUIRED
