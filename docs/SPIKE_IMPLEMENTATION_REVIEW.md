REWORK_REQUIRED

The spike demonstrates a real Windows implementation and a viable native leaf-surface approach. It does **not yet prove the approved foundation contract**: native dispatch safety, UIA lifetime, timer routing, DPI/resize behavior, and several styling guarantees need implementation corrections.

**1. Exact reviewed HEAD**

Reviewed `5a93faf647ff8d7e2eaaca16f7ffd816fcc9884a` against approved baseline `29c67025dc88221899a8ab1b381f08491d44b85b`, using an isolated checkout. The six principal architecture documents remain unchanged from that baseline.

Independent checks:

- Windows GNU target: `cargo check --locked --all-targets --target x86_64-pc-windows-gnu` passed.
- Linux examples check and `cargo fmt --check` passed.
- Linux `cargo test --locked --lib` failed compilation because Windows UIA tests lack platform guards and a portable patch-removal test references Windows-gated helpers. See [test declarations](https://github.com/rceman/rust-ui/blob/5a93faf647ff8d7e2eaaca16f7ffd816fcc9884a/src/tests.rs#L3377).
- The reported Windows **69/69** was not independently reproduced: this review environment had no native Windows execution capability.

No code or documentation was modified during the review.

**2. Architecture deviations classification**

Every entry in [SPIKE_ARCHITECTURE_DEVIATIONS.md](https://github.com/rceman/rust-ui/blob/5a93faf647ff8d7e2eaaca16f7ffd816fcc9884a/docs/SPIKE_ARCHITECTURE_DEVIATIONS.md) is classified below.

| Entry | Classification | Assessment |
|---|---|---|
| 1. No pushed clip around native drawing | ACCEPT AS PLATFORM IMPLEMENTATION DETAIL | Avoiding that clip during `TxDrawD2D` is reasonable. The final bitmap must still obey applicable compositor clipping and native-island overlap rules. The proposed blanket exemption is too broad. |
| 2. Peer-local bitmap rendering | REWORK BEFORE FOUNDATION | The boundary is principled; its current coordinate, DPI, device-loss, and resource handling is incomplete. |
| 3. One lazy activation | TEMPORARY SPIKE COMPROMISE | Lazy in-place activation is reasonable. Scratch activation, unconditional UI activation, and ignored activation errors do not establish the claimed durable lifecycle. |
| 4. Deferred native delivery | REWORK BEFORE FOUNDATION | The implementation does not establish complete reentrancy safety, ordering, overflow handling, or universal generation fencing. |
| 5. Strong native UIA providers | REWORK BEFORE FOUNDATION | Strong references are appropriate for classic COM providers. The actual registry lacks generation identity and unmount cleanup. |
| 6. Disabled nodes excluded from hit testing | ACCEPT AND UPDATE ARCHITECTURE CONTRACT | This is an appropriate universal pointer/focus/invoke policy. Existing focus and native input paths do not yet enforce it completely. |
| 7. Resolved `Theme.dark` ownership | ACCEPT AS PLATFORM IMPLEMENTATION DETAIL | Correct ownership model and consistent principal palette selection. Some theme application behavior still needs correction. |
| 8. Activated native measurement | TEMPORARY SPIKE COMPROMISE | Native measurement is appropriate. The scratch path has insufficient error handling and no validated multiline growth contract. |
| 9. Post-creation HWND repair | REWORK BEFORE FOUNDATION | The repair explains the improved recorded rectangles. It does not make window-dependent activity before a valid HWND a safe lifecycle. |

**3. RichEdit bitmap-rendering verdict**

**A principled native-island boundary, implemented incompletely.**

RichEdit continues to own text editing; the compositor positions a peer-owned surface. That shape is compatible with a future visual-per-peer DirectComposition implementation, although this spike does not prove that future implementation.

Concrete problems:

- The bitmap reuse check uses logical size only. It omits DPI and parent target/device generation; DPI is assigned only when creating the bitmap. See [bitmap creation and reuse](https://github.com/rceman/rust-ui/blob/5a93faf647ff8d7e2eaaca16f7ffd816fcc9884a/src/platform/win32/text.rs#L875).
- Drawing places text inside a **6dp horizontal / 5dp vertical inset**, while layout assigns the outer rectangle to the host. Pointer and caret transforms also use the outer origin. Visible text, native coordinates, and measurement therefore lack one consistent transform. See [editor drawing](https://github.com/rceman/rust-ui/blob/5a93faf647ff8d7e2eaaca16f7ffd816fcc9884a/src/platform/win32/render.rs#L448), [peer bounds](https://github.com/rceman/rust-ui/blob/5a93faf647ff8d7e2eaaca16f7ffd816fcc9884a/src/platform/win32/layout.rs#L468), and [pointer conversion](https://github.com/rceman/rust-ui/blob/5a93faf647ff8d7e2eaaca16f7ffd816fcc9884a/src/platform/win32/mod.rs#L895).
- `Renderer::resize` exists but is not called by `WM_SIZE`. Target-loss recovery recreates the frame target without invalidating peer surfaces.
- `dpi_changed` relayouts but does not itself request the promised final repaint. `SetWindowPos` can synchronously cause painting before the scale update.
- One retained bitmap per peer bounds the number of surfaces. Allocation dimensions and aggregate bytes lack checked limits.
- Transparent bitmap clearing and alpha composition need reconciliation with the approved opaque native editing boundary.

Static screenshots and finite UIA rectangles do not establish caret, selection, IME positioning, resize, or DPI correctness.

**4. Native reentrancy verdict**

**Rework required; the documented “all sends funnel through the backend” claim is false.**

The most serious issue is the WndProc boundary: [the trampoline](https://github.com/rceman/rust-ui/blob/5a93faf647ff8d7e2eaaca16f7ffd816fcc9884a/src/platform/win32/window.rs#L27) constructs `&mut Backend` before checking any reentrancy state. A synchronous native callback can enter that boundary while the outer backend operation remains active. `in_turn` does not resolve that mutable-aliasing hazard.

The delivery mechanism also has functional gaps:

- [send_native](https://github.com/rceman/rust-ui/blob/5a93faf647ff8d7e2eaaca16f7ffd816fcc9884a/src/platform/win32/mod.rs#L414) uses an immutable `try_borrow`; nested immutable sends can still enter RichEdit.
- Deferred messages are silently discarded at capacity.
- New direct sends can overtake pending deferred sends.
- Queue insertion does not independently establish continued draining.
- Deferred draining, hover, timers, formatting, and measurement contain direct sends outside the stated common guard.
- `TxSendMessage` discards its HRESULT, losing processing/fallback information described by [Microsoft’s contract](https://learn.microsoft.com/en-us/windows/win32/api/textserv/nf-textserv-itextservices-txsendmessage).

Generation checking during deferred draining is useful, but insufficient.

This should become an explicit private backend contract covering admissible deferred messages, owned payloads, ordering, synchronous results, overflow, progress, and teardown. Pointer-bearing or result-sensitive messages cannot safely be deferred as arbitrary integer tuples.

Native input needs correction too: composition start/end are consumed without forwarding them to RichEdit, and submit is decided **before** native key interpretation without checking composition state. Original key-message metadata and pointer button/modifier flags are also lost.

Related lifecycle defects are foundation blockers: `fatal` is recorded but never consumed, and the initial-turn error path can drop the backend while the live HWND retains its route pointer. See [run/error lifecycle](https://github.com/rceman/rust-ui/blob/5a93faf647ff8d7e2eaaca16f7ffd816fcc9884a/src/platform/win32/mod.rs#L1567).

**5. UIA verdict**

**Real provider integration is demonstrated; lifetime and live-state correctness are not.**

Mixed strong/weak ownership is reasonable. Its generation implementation is defective:

- [Native providers are cached by slot alone](https://github.com/rceman/rust-ui/blob/5a93faf647ff8d7e2eaaca16f7ffd816fcc9884a/src/platform/win32/uia.rs#L442).
- The registry is not pruned on peer removal.
- Rebuilding can associate an old native provider with a new generation occupying that slot.
- Native windowless-site navigation lacks a live-generation/closed check.

UIA rebuilding occurs through `WM_GETOBJECT`, rather than being maintained after commits, geometry changes, removal, and enabled-state changes. Clients retaining existing providers can therefore observe stale names, bounds, order, or enabled properties. Previously returned painted fragments also retain their old property values when the same node generation remains live.

Additional gaps:

- UIA focus dispatch checks liveness but not current enabled/focus eligibility.
- Native editor labels are not applied: the recorded names remain `"RichEdit Control"`.
- No focus/property/structure notification implementation was found.
- Native provider disconnection and externally retained COM references need explicit teardown validation.

The final tree records **four finite editor rectangles**. That proves an improvement over the earlier Infinity result, not complete accessibility behavior or leak safety.

**6. Styling-model verdict**

**The shared typed model is substantially present, but its approved guarantees are incomplete.**

Painted buttons, actions, boxes, and surfaces use shared typed values and painting machinery. FAST/CUSTOM/SURGICAL are real API paths.

The exact surgical invariant is supported: the bottom-border patch changes only its selected fields after recipe/state resolution. The implementation and test cover unrelated box/text values across themes and interaction states. See [button resolution](https://github.com/rceman/rust-ui/blob/5a93faf647ff8d7e2eaaca16f7ffd816fcc9884a/src/style.rs#L988).

Required corrections remain:

- No `InvalidStyle` preflight validates finite numbers, nonnegative geometry, positive text size, or allocation arithmetic before native mutation.
- Per-side border strips overlap at corners rather than following the approved nonoverlapping corner partition.
- Nonuniform radii are independently clamped rather than proportionally normalized.
- Consumer border patches can erase the recipe focus border; required focus visibility is not enforced independently afterward.
- Repeated `ActionStyle` state setters and editor `.style(...)` replace patches instead of consistently merging sparse fields.
- Dirty comparisons operate on authored descriptions rather than concrete resolved values.
- Button style changes receive paint-only classification even when padding/text changes affect measurement; shadow-only box/action changes can request layout. Blanket relayout on every application update masks some classification mistakes and defeats the promised no-op behavior.
- Forced-colors/high-contrast/text-scale enforcement is incomplete.

Native editor chrome is a separate hardcoded radius/inset/border path. The approved `TextInputStylePatch` capability boundary is absent; public editable-content size/weight patches were introduced instead. That is an **unlisted architecture deviation**, requiring restoration of the approved boundary or an explicitly reviewed contract change.

Theme ownership itself is correct: principal consumers use resolved `Theme.dark`; ambient appearance informs System resolution. However, theme switching pushes a generic foreground into every peer, overwriting explicit foreground customization, and simultaneous theme/style updates can apply using the previous palette.

**7. Shadow verdict**

**Correct public scope and representation; incomplete implementation contract.**

`Shadow` has the approved four fields, and `ShadowPatch` supports atomic Set/Remove/Unchanged behavior. Public primitives and painted built-ins share it. There is no hidden general effects subsystem or shadow timer.

[Shadow rendering](https://github.com/rceman/rust-ui/blob/5a93faf647ff8d7e2eaaca16f7ffd816fcc9884a/src/platform/win32/render.rs#L918) nevertheless:

- Reallocates masks, blur buffers, pixels, and a bitmap on every paint; the required bounded cache is absent.
- Uses unchecked dimensions/products without byte limits.
- Rasterizes at one pixel per DIP rather than the specified DPI-aware device-pixel footprint.
- Does not exclude the original rounded interior; translucent fills expose interior shadow.
- Does not maintain the promised old/new clipped ink damage union.
- Skips alpha-zero drawing without normalizing equivalent resolved styles.
- Can trigger layout through style dirty classification.

Full-frame repaint hides some damage errors in this opaque harness. It does not prove the specified damage contract.

**8. Retained/keyed/event/task verdict**

**The retained core is credible; its guarantees do not yet extend through every backend subsystem.**

The implementation has genuine staged reconciliation, typed scoped keys, generational node identity, editor binding/conflict state, and removal cleanup. Task/mailbox limits, registration fencing, cancellation, bounded sweeps, and completion reaping are substantive implementations.

Core generation discipline is stronger than the UIA and native timer implementations described above.

Other functional gaps:

- Hover, pressed, and focus changes do not reliably mark paint damage when no application handler runs. Built-in chrome must update independently of consumer observation handlers.
- `sanitize_focus` exists but is unused. Already-focused disabled/removed nodes and native capture need commit-time cleanup.
- UIA can focus a disabled node, and native character delivery does not universally recheck interactivity.
- Public `read_only` is not propagated into native behavior.
- Decorative custom children can become hit targets inside an Action, preventing the Action from receiving the press. Action nesting validation also misses actionable custom children.

Disabled exclusion is implemented in shared hit testing and semantic dispatch, so it is intended framework behavior. The remaining paths must enforce that universal contract.

**9. API ergonomics verdict**

**Generally concise and understandable; behavior and error handling need correction.**

Normal FAST usage is concise. CUSTOM composition is available through public primitives. SURGICAL border changes are practical. `TextValue`, explicit conflicts, keyed rows, and typed jobs are understandable.

No HWND, COM, native generation queue, or backend handle leaks into ordinary consumer code.

The example’s supplied executor adds substantial setup code compared with the short architecture snippets, but follows the approved owner-supplied executor model.

A concrete consumer bug exists: [Send handling](https://github.com/rceman/rust-ui/blob/5a93faf647ff8d7e2eaaca16f7ffd816fcc9884a/examples/composer.rs#L195) sets `busy` before spawning and ignores the spawn result. Executor rejection or capacity failure can leave the application busy without a task. The example must handle that result.

The native style capability mismatch and Action hit ownership problems also prevent the compiling API from proving all promised consumer behavior.

**10. Idle/resource verdict**

**An event-driven framework is present; resource and idle approval remain unsupported.**

The framework blocks in `GetMessage` and has a demand-driven deadline timer. No permanent framework render loop or mandatory Tokio runtime was found.

Concrete resource defects:

- Native timers are allocated starting around `101`, while [WndProc routing](https://github.com/rceman/rust-ui/blob/5a93faf647ff8d7e2eaaca16f7ffd816fcc9884a/src/platform/win32/window.rs#L193) only recognizes IDs at or above `0x40000000` and subtracts that base. Requested timers therefore do not reach their intended handler.
- [Timer registrations](https://github.com/rceman/rust-ui/blob/5a93faf647ff8d7e2eaaca16f7ffd816fcc9884a/src/platform/win32/mod.rs#L652) store slots without generations, allocate additional IDs for repeated requests, and lack peer-unmount cleanup.
- Per-side border painting moves an owned COM geometry into `ManuallyDrop` and never releases it. See [mask ownership](https://github.com/rceman/rust-ui/blob/5a93faf647ff8d7e2eaaca16f7ffd816fcc9884a/src/platform/win32/render.rs#L1103). This leaks a reference on each relevant paint; [Rust documents](https://doc.rust-lang.org/std/mem/struct.ManuallyDrop.html) that this wrapper suppresses automatic destruction.
- The process-global DWrite format cache retains every historical exact-size/weight combination without eviction.
- Peer surfaces and shadow transient buffers lack adequate allocation bounds.

The final perf artifact records **36.2 MB, 22 threads, and rounded 0% CPU over three seconds**. It establishes one debug-process smoke observation. It does not establish zero requested redraws, timer correctness, leak freedom, thread attribution, or general efficiency.

**11. Evidence integrity findings**

These fail completion/artifact-honesty gates and must be corrected:

| Finding | Actual final-tree evidence |
|---|---|
| API review identifies `89b39f1` | Stale candidate identity. |
| Evidence identifies “HEAD”; deviations identify “final working tree” | No immutable build/source/binary identity recorded. |
| Send artifact row duplicated | Confirmed duplication. |
| README describes three native editors | Final UIA tree contains four. |
| README still describes Infinity bounds | Final tree records finite rectangles; comments retain obsolete rationale too. |
| README says 35.6 MB / 20 threads | Final `perf.txt` says 36.2 MB / 22 threads. |
| Live Stop → resend claimed | Capture script invokes Send and waits for completion; it does not invoke Stop or resend. |
| Clean teardown implicitly treated as demonstrated | Capture script force-kills both sessions. |
| Zero idle loop claimed from smoke | Three-second CPU sample does not satisfy the counter-based idle gate. |
| All IME messages described as forwarded | Start/end composition are not forwarded. |

The [capture script](https://github.com/rceman/rust-ui/blob/5a93faf647ff8d7e2eaaca16f7ffd816fcc9884a/benchmark/results/windows-native-composer-spike-v0.1/collect.ps1#L63) and [README](https://github.com/rceman/rust-ui/blob/5a93faf647ff8d7e2eaaca16f7ffd816fcc9884a/benchmark/results/windows-native-composer-spike-v0.1/README.md#L16) support Send → completion and static UIA/theme snapshots. Unit cancellation tests provide separate evidence; they cannot substitute for the claimed live sequence.

These inconsistencies do not establish fabrication. They do prevent accepting the current completion claims.

**12. Remaining validation gaps**

**Real end-to-end IME must validate before foundation approval.** It is an explicit baseline gate, and current composition/submit code has concrete defects. Validate candidate confirmation without submit, committed versus preedit notifications, queued replacements/conflicts, selection/undo preservation, and removal during composition.

DPI coverage must remain precise:

- **125%:** exercised in the recorded smoke environment.
- **100/150/200%:** not independently measured.
- **Multiple displays:** not exercised.

The same code path is inferred coverage, not measured coverage. Current cache/resize defects make that distinction material.

Further required evidence includes:

- Real multiline TextArea growth, shrink, wrapping, width changes, and `max_lines`; the example uses single-line inputs.
- Native typing, selection, undo, Unicode/RTL, pointer drag, caret, read-only, and disabled transitions.
- Narrator traversal/activation, text patterns, live enabled/focus/bounds changes, and retained stale-provider references.
- Actual Stop → resend, keyed focus retention, removal/recreation, and graceful close with queued work.
- Thirty-second idle counters and 100 mount/reorder/remove cycles returning live resource counts to baseline.
- Recorded startup/binary/dependency/allocation and 100/1000-row measurements required by the baseline.

The baseline also says the opaque starter harness does not complete its remaining Windows layering gates. Transparent/borderless adjacent-editor evidence is absent.

**13. Exact corrections required before foundation freeze**

1. **Repair native dispatch safety:** guard ownership before creating backend references; centralize native entry; define FIFO/progress/error behavior and safe deferred payload classes.
2. **Repair window lifecycle:** supply a valid HWND before window-dependent peer activity; surface fatal errors; destroy/detach the window route safely on every failure path.
3. **Repair native text semantics:** forward required IME boundaries, decide submit after native consumption, preserve message metadata, implement read-only/enabled behavior, and complete caret cleanup.
4. **Repair native rendering:** use one content-coordinate transform, resize the frame target, invalidate peer surfaces on DPI/device changes, repaint after DPI changes, and enforce checked allocation and opaque-island constraints.
5. **Repair timers:** consistent ID routing, generation-bearing ownership, correct repeated-request semantics, and cancellation on unmount/close.
6. **Repair UIA:** generation-keyed provider registration, unmount/disconnection cleanup, live property/order updates and notifications, correct labels/focus eligibility, and stale-reference teardown.
7. **Complete style/Shadow contracts:** preflight validation, shared native chrome capability, independent focus enforcement, merge/equality/damage rules, corner geometry, OS enforcement, bounded shadow caching, interior exclusion, and clipped old/new damage.
8. **Repair interaction/resource defects:** paint interaction state without app handlers; sanitize focus/capture; preserve Action hit ownership; release mask COM references; bound retained format resources.
9. **Repair example and test portability:** handle spawn failure; correctly guard Windows tests and portable helpers.
10. **Regenerate honest evidence after the last code change:** immutable source/binary identity, build/test logs, corrected documentation, and the missing native acceptance measurements.

**14. Safe base for the next reference-application experiment**

**No foundation clearance for `5a93faf`.** Hold the planned consumer/reference experiment until these implementation corrections and native gates pass. No AppCommand/application-shell work was started during this review.

**15. Safe to merge toward `main`**

**Not at this HEAD.** Merge clearance requires a new identified candidate containing the implementation corrections, completed native validation, and consistent artifacts. That corrected candidate should receive a follow-up review before merging.

RUST_UI_WINDOWS_SPIKE_IMPLEMENTATION_REVIEW_COMPLETE
