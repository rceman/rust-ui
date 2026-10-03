# Independent final narrow Windows foundation closure review

**RUN_KEY:** `RUI-A-261003-2241`

**Role:** Astra Advisor

**Started:** 2026-10-03 23:51:18 UTC+03:00

**Verdict:** `REWORK_REQUIRED`

This checks only F01, F11 and F12/F13 against the previous freeze review.
The platform-contract architecture remains accepted. The remaining runtime
correction concerns terminal wake failure; queue teardown, ownerless IME,
verified IME engagement and the physical details layout have materially improved.
No broad architecture redesign or Component Gallery work is requested.

## Immutable identities

| Identity | SHA |
| --- | --- |
| Prior independent review / BASED_ON | `dc3a8615fdac5c9c334dc1c7cd26148a87c1bc15` |
| Frozen production/library implementation | `98c5614de498af3a9ece1930a2c78504b24e8f6c` |
| Harness/binary source | `9d78f127fe477557f73e3350088bc36e794c28d0` |
| Publication inspected | `a86dfe3917c338081a7518b808e0c4377f7b7b65` |

**The identity chain is valid.** Actual diffs show production queue/wake/IME
changes and focused tests at `98c5614`, together with the composition document
correction. The next commit changes only native_probe and collection sequencing.
Publication changes README, current details captures/rectangles, identity.txt and
SPIKE_API_REVIEW. No `src/`, manifest, lockfile or production-semantic change
occurs after `98c5614`.

The current receipt identifies:

```text
HEAD=9d78f127fe477557f73e3350088bc36e794c28d0
exe_sha256=33F2D67F9EEBE39579EFF8B88497F3B89BCEC24695C785A179A455FEA43BCD05
probe_sha256=318A725B7407E2CEDD948C1D246FD3D152EAC5187D669301FC9B3E22EE7FA5AA
run_utc=2026-10-03T20:22:38.3559983Z
```

Both executable hashes matched independently before and after the required
build/check commands. The focused native runs used identical copies outside
the repository. This establishes the declared source/binary relationship; it
does not retroactively identify every older artifact as a new capture.

The authoritative Windows checkout was clean at `a86dfe3` on the requested
branch. Previous independent review documents are unchanged by the candidate
and this review. Only this new document is committed and pushed. Production,
tests, harness, examples, evidence and earlier reviews are not modified. The
new review commit is identified separately in Git history and the execution
handoff; main is not merged.

## Independent verification

Native authority: `W:\devin_folder\rust-ui`, Windows 11 Home build 26200,
Rust 1.94.0. Portable tests: separate Linux checkout of `a86dfe3`, WSL2 kernel
`6.18.40.1-microsoft-standard-WSL2`, Rust 1.93.1.

| Executed check | Result / scope |
| --- | --- |
| Native Windows `cargo test --locked --lib` | PASS **100/100**, no ignored tests. |
| Linux `cargo test --locked --lib` | PASS **77/77**, no ignored tests. |
| Native Windows `cargo fmt --check` | PASS. |
| Native Windows `cargo check --locked --all-targets` | PASS, with warnings. |
| Native Windows `cargo build --locked --all-targets` | PASS, with warnings. |
| Focused queue/IME tests and production paths | Inspected and executed through the library suite; teardown injected during dispatch releases the mounted peer in one drain. |
| Native wake quota/timeout fixture | Successful fallback: one pump delivered. Terminal-failure case: zero pumps delivered after receiver recovery, error latch remains. Details below. |
| Two current real-IME runs | PASS: gained `ありがと`, mandatory submit counter unchanged at 0, native/app committed value agreement, following `Z` edit succeeds. |
| Current physical capture and UIA rectangles | PASS: unobscured caption, full draft-prefill\q and three separated labels. Fresh review-owned captures also inspected. |
| Identity and previous-review comparison | PASS: hashes, source chain, immutable previous review files. |

External fixtures, executable copies, logs and captures are under
`W:\devin_folder\rust-ui-final-approval-checks`. No repository tests were added
or changed. The wake fixture compiles the **verbatim production mailbox_wake
function** with the production `WM_APP + 7` message ID. Its minimal mailbox
observer records the failure latch; it does not execute full Backend/run-loop
code or model application envelopes. It exercises actual User32 queue quota,
synchronous-send success, timeout and receiver recovery on separate native
threads. The run-loop consequence is established separately from actual source.

The complete acceptance matrix and a new idle/resource smoke were not rerun.
The unchanged older performance files record 32.4 MiB, 17 threads, 30545 ms and
zero native timer fires. They are retained prior evidence for this review,
not newly reproduced measurements at `98c5614`. Fresh current proof is the
required builds/tests, focused IME/details and wake checks above.

## Closure matrix

```text
F01 PARTIALLY_CLOSED
F11 CLOSED
F12 CLOSED
F13 PARTIALLY_CLOSED
```

Previously closed F02–F10 and N01–N04 are not reopened. No concrete regression
requiring their reopening was found. No new architecture finding is created.

## F01 — queue and ownership closed; terminal wake progress remains open

### Closed corrections

Backend::drain_reentrant now carries one ordinary budget across merge passes.
Dispatch-time arrivals are merged before deciding whether to run mandatory
teardown, dispose closed-window work or post a continuation. A teardown arriving
in the live cell is selected in the same drain lifecycle, even after the ordinary
budget is spent. Late work after teardown is disposed without dispatch.

The committed regression reproduces the formerly failing mid-dispatch arrival
using the same atomic-close/mailbox-close/owned teardown legs. It runs actual
Backend shutdown and asserts the mounted peer released exactly once and the
queue emptied. The 138-ordinary-items trace still asserts exactly 128 ordinary
dispatches and disposal of ten remaining items; the earlier-delivery-error test
still proves teardown is not skipped. The original mandatory-cleanup blocker is
repaired.

IME ownership now distinguishes inactive, active-ownerless and active-owner(id)
using Option<Option<NodeId>>. ime_target and ime_end preserve the inner None.
Native send_ime therefore cannot transfer an ownerless composition to a later
focus. The focused ownerless regression passes, and generation checks remain in
the delivery path.

The mailbox callback releases mailbox state/wake locks before invoking the wake.
The new fallback uses scalar WM_PUMP and the existing WndProc ownership guard;
a same-thread reentrant entry queues owned work instead of forming a second
mutable Backend reference. No new framework mutex cycle or unguarded Backend
entry was found. The cross-thread fallback requests a 250 ms timeout with
SMTO_ABORTIFHUNG | SMTO_BLOCK. As documented by
[Microsoft](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendmessagetimeouta),
the same-queue case invokes the window procedure directly and does not enforce
that timeout; here its safety depends on the existing native-entry guard.

### Remaining MAJOR defect: both routes can fail on a live receiver

[mailbox_wake](https://github.com/rceman/rust-ui/blob/98c5614de498af3a9ece1930a2c78504b24e8f6c/src/platform/win32/mod.rs#L2409)
now tries PostMessageW, then SendMessageTimeoutW. Successful fallback genuinely
provides progress without another producer. But when both fail it only calls
wake_post_failed. That sets a flag and resets the coalescing edge. The failure
is read only at the beginning of a later Backend turn or after GetMessage has
already exited. The message loop checks take_fatal after ordinary dispatch,
not take_wake_failure.

Native fixture results, reproduced with actual queue saturation:

```text
receiver recovers during send wait:
  quota_items=10000, post_error=1816
  wake_ms=50, delivered_pumps=1, latch=false

receiver recovers after send timeout:
  quota_items=10000, post_error=1816
  wake_ms=261, delivered_pumps=0, latch=true
  receiver then drains all 10000 ordinary messages
```

The second receiver is alive and recovers; it is not permanently hung or
destroyed. The timeout does not leave a pump to be delivered after recovery.
In production those ordinary default messages need not invoke Backend::turn.
After processing them, the unchanged blocking run loop can wait again with
accepted mailbox work and an unread failure latch. The new run-exit check cannot
cause that exit. Retrying only on the next enqueue or observing the flag on
incidental UI input still violates the required progress contract.

The committed wake test covers successful PostMessage and failure on a destroyed
HWND. It checks the latch, rather than an observable terminal outcome on a live
receiver after recovery, so it misses this trace.

**Smallest remaining correction:** make terminal wake failure reach the existing
caller/run-loop error or cancellation path without another producer, enqueue or
incidental input. Prove the live-window quota → send-timeout → recovery trace
ends in delivery or an observable terminal error; it must not return to waiting
with stranded accepted work. Preserve the successful bounded fallback and fixed
queue/ownership paths. No continuous polling/render loop or new timer framework
is requested.

## F11 — CLOSED: verified engagement and current acceptance

The initial WM_INPUTLANGCHANGEREQUEST post is checked. A bounded retry/read-back
loop inspects GetKeyboardLayout for the composer's thread until Japanese langid
0x0411 is observed; absence becomes an error. A default IME window is required.
Open state is set and read back, and all required NATIVE|FULLSHAPE|ROMAN bits are
set and read back under bounded retries. The blind VK_DBE_HIRAGANA fallback is
removed. Verification failure cannot be accepted merely from a prior profile.

Both independent current-binary runs produced a nonempty newly gained
`ありがと` suffix, with submit count unchanged. The probe requires the app's
committed draft echo to equal the native provider value. After each run a normal
`Z` edit was observed in both native value and app echo. Repeating engagement
without assuming a pristine persisted profile also succeeded. The previously
closed deterministic preedit/acknowledgement proof remains applicable; this
review does not claim a separate live preedit timeline or monitor transition.

## F12 — CLOSED: actual physical evidence is usable

Current committed details-screen.png visibly shows the foreground composer,
caption, full draft-prefill\q and all three diagnostics. They are stacked with
clear vertical separation and no foreign-window occlusion of the composer.
The physical overlap from the older consumer has not recurred in these states.

Current recorded UIA rectangles are:

```text
turn        [481,667,690,690]
draft-echo  [481,695,655,719]
draft-edits [481,724,626,747]
```

All three exist, are distinct and strictly ordered, with gaps between adjacent
bottom/top coordinates. Current and fresh physical captures agree with the
layout account. PrintWindow remains explicitly non-authoritative for native
text fidelity; its comparison image is not substituted for physical proof.

## F13 — corrected canonical accounts; one evidence qualification remains

Current API candidate is now 98c5614, and 0405a80 is explicitly historical.
SPIKE_ARCHITECTURE_DEVIATIONS correctly describes deferring geometry during
composition, acknowledging queued native edits, reconciling remaining
divergence, resolving proposals and finally relatching. The binary receipt and
production/harness/publication distinction are valid. Counts 100/100 Windows
and 77/77 Linux are independently verified.

README now accurately distinguishes Rust platform/native assertions from
PowerShell orchestration and consumer-level assertions on probe output. It no
longer claims that every consumer assertion is inside Rust. Historical review
documents remain immutable.

**Remaining MINOR correction:** README still describes final-foundation.png as
regenerated on THIS candidate. That file and the idle/performance receipts are
unchanged from the prior bundle, whereas this publication establishes new
focused details evidence and the new binary receipt. No separate current
foundation/idle run receipt was supplied to substantiate their freshness.
Identical file content alone neither proves nor disproves a rerun. Treat those
artifacts as retained prior evidence unless the rerun is identified, and label
which scenarios are fresh versus reused. A request for that clarification was
left open during the review; no confirming receipt was available at completion.
This is an evidence-account correction, not a requirement to replay the broad
matrix or remeasure idle merely for formality.

## Gates 1–20, limited to these tracks

The canonical document still contains exactly twenty gates.

| Gate | Status | Reason |
| --- | --- | --- |
| 1 — Completeness | FAIL | Terminal wake progress and current/retained artifact qualification remain. |
| 2 — Approval boundaries | PASS | Accepted architecture retained; environmental limits explicit. |
| 3 — Contract preservation | FAIL | Both failed wake routes can leave accepted work stranded after receiver recovery. |
| 4 — Scope | PASS | Only the three requested tracks reviewed and one new artifact published. |
| 5 — Minimal design | PASS | Existing wake/error ownership can supply the bounded correction. |
| 6 — Authority/ownership | FAIL | Terminal wake failure is latched but lacks an independently reachable outcome. Queue and ownerless IME are repaired. |
| 7 — Reuse/duplication | PASS | Complete-queue policy and existing native-entry authority reused; harness boundary clarified. |
| 8 — Unrequested fallback | PASS | Blind IME toggle removed; bounded send fallback is explicitly within this task. |
| 9 — Deterministic contracts | FAIL | Live-receiver timeout/recovery can return to waiting without delivery/error. |
| 10 — Canonical truth | FAIL | Current-candidate capture freshness still needs substantiation or historical qualification. |
| 11 — Failure safety | FAIL | Terminal wake latch is not guaranteed to reach a failure path. |
| 12 — Boundedness/lifetime | PASS | Ordinary drain budget, mandatory teardown/disposal and bounded send timeout are retained; no new lifetime defect found in these corrections. |
| 13 — Dependencies/blast radius | PASS | No dependency or unrelated production expansion. |
| 14 — Redundant work/performance | PASS | No new idle loop; older smoke is not reported here as freshly measured. |
| 15 — Infrastructure boundary | PASS | Harness/consumer concepts remain outside library semantics. |
| 16 — Proof sufficiency | FAIL | Existing wake fixture misses the independently reproduced live-receiver terminal-failure branch. |
| 17 — Proportional recovery | PASS | Focused correction and evidence qualification suffice. |
| 18 — Identity/immutability | PASS | Exact source/binary hashes and publication scope verified; old reviews unchanged. |
| 19 — Artifact honesty | FAIL | New global identity does not substantiate freshness of retained foundation/idle receipts. |
| 20 — Systemic closure | FAIL | Successful fallback and dead-HWND latch test do not cover recovered live receivers after timeout. |

## Decision and bounded handoff

Only terminal mailbox-wake progress still requires runtime correction in this
review. Qualify retained evidence, or identify its current rerun receipt.
Preserve the repaired queue teardown, three-state IME ownership, verified input
engagement and usable physical capture. Run focused proof of changed paths;
another architecture redesign or full matrix replay is unnecessary.

Dead keys, physical monitor DPI transition, combined composition/physical scale
transition and foreground ElementProviderFromPoint remain environmental NOT RUN
items. None causes this rejection. It rests on an executed native failure trace
and its actual production error-path consequence.

The foundation is not ready to freeze or begin the next milestone. The accepted
architecture direction remains sound; ARCHITECTURE_BLOCKED is unwarranted.

SHADCN_COMPONENT_GALLERY_NOT_CLEARED

REWORK_REQUIRED
