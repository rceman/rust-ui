# Independent closure-only Windows foundation review

**RUN_KEY:** `RUI-A-261004-1459`

**Role:** Astra Advisor

**Started:** 2026-10-04 15:02:00 UTC+03:00

**Verdict:** `REWORK_REQUIRED`

The filename is the requested review destination, not an approval verdict.
This reviews only F01's replacement mailbox wake authority. The waitable-event
direction is sound; its production acceptance, failure and lifetime contracts
are incomplete. F13 remains CLOSED. No other finding is reopened and no broad
architecture redesign is requested.

## Immutable identities

| Identity | SHA |
| --- | --- |
| Prior review / BASED_ON | `8554f31f7b3e40b91c713d9fa08f2af2c50f35fa` |
| Frozen production implementation | `a5ad4ed065d11a482a8588706162c91241212c58` |
| Publication inspected | `5ccb2d1a2b12d0bb6608735d057a81e266582486` |

The identity chain is valid. Production changes Cargo.toml, win32/mod.rs,
win32/window.rs and focused tests. Publication changes six evidence/docs files
only; no source, harness, tests, manifest or lockfile changes follow production.
The older harness implementation SHA in README identifies unchanged harness
code; the complete binary source snapshot is a5ad4ed, as identity.txt records.

```text
HEAD=a5ad4ed065d11a482a8588706162c91241212c58
exe_sha256=564DE5015CF84EBFC7CC1B28127704D3A8D66433A022B15C7C6E7101ADBB6865
probe_sha256=15BA7D10F4276562DBC9BC45F26B292064BE8E8C81DF4BF33F41F266BC1FC56D
run_utc=2026-10-04T10:24:51.7737380Z
```

Both executable hashes matched independently before and after the required
build/check commands. The native checkout was clean at the publication SHA on
the requested branch. Previous independent reviews remain unchanged. Only this
new review document is committed and pushed; its SHA is recorded separately in
the execution handoff. No implementation, tests, harness or evidence are changed.

## Independently executed verification

Native authority: `W:\devin_folder\rust-ui`, Windows 11 Home build 26200,
Rust 1.94.0. Linux: separate checkout of 5ccb2d1, WSL2 kernel
`6.18.40.1-microsoft-standard-WSL2`, Rust 1.93.1.

| Check | Result |
| --- | --- |
| Windows `cargo test --locked --lib` | PASS **104/104**, exit 0; none ignored. |
| Linux `cargo test --locked --lib` | PASS **77/77**, exit 0; none ignored. |
| Windows `cargo fmt --check` | PASS, exit 0. |
| Windows `cargo check --locked --all-targets` | PASS, exit 0; warnings remain. |
| Windows `cargo build --locked --all-targets` | PASS, exit 0; warnings remain. |
| Focused native quota/event, failure and coalescing tests | Executed in the native library suite and their assertions inspected. |
| Triggering sync and async send failure counterexample | Independently reproduced on Windows; both report success after their envelopes are dropped. |
| Identity/provenance spot-check | PASS, with minor stale numeric prose disclosed below. |

The review-owned native fixture is outside the repository under
`W:\devin_folder\rust-ui-approved-review-checks\RUI-A-261004-1459`.
It imports the actual production tasks.rs and key.rs directly, and executes a
verbatim extraction of production mailbox_wake with a deliberately invalid
native event handle. It installs that callback into the real Mailbox seam,
then calls public UiProxy::try_send and TaskSender::send. It does not run the
full application wait loop; that loop's handling is audited from production
source. No repository test was added or modified.

Build/test logs are in the native user's TEMP directory with the prefix
`rust-ui-approved-review-`; the Linux log is
`/tmp/rust-ui-approved-review-linux.log`. The current idle/send/foundation smoke
is worker-recorded evidence, not independently replayed. No visual/IME matrix
replay was required or performed. No native allocation failure or scheduling
race is claimed to have been experimentally induced where the review below
uses source-level failure/interleaving analysis instead.

## Closure

```text
F01 PARTIALLY_CLOSED
```

F13 remains CLOSED. Previously closed F02–F12 and N01–N04 are not reopened.
The concrete new native-entry/cleanup regressions described below are within
F01's newly introduced wake/run-loop surface.

## F01: smallest remaining invariant

**A successful triggering send still establishes neither durable delivery nor
a guaranteed observable terminal rejection.** The event removes the old
message-queue capacity dependency, but the complete acceptance/wake/wait/close
boundary does not yet enforce this invariant.

### Single event authority: the successful mechanism is sound

Production creates one unnamed, initially nonsignaled, auto-reset event before
installing the producer callback. The same event participates in
MsgWaitForMultipleObjectsEx with INFINITE, QS_ALLINPUT and MWMO_INPUTAVAILABLE.
A signal remains available until consumed by the single waiting UI thread;
normal idle is a blocking wait, with no polling interval or mailbox timer.

The cross-thread PostMessageW/SendMessageTimeoutW/recovery-timer cascade and
MAILBOX_WAKE_TIMER dispatch are removed. Retained same-thread WM_PUMP
continuations serve native deferred dispatch; they are not the producer wake
mechanism. Event signalling does not use the posted-message quota. These are
appropriate corrections to the old failure class.

The native quota regression passes: a real receiver thread's posted queue is
filled, one envelope is queued, the event wakes MsgWaitForMultipleObjectsEx
without another producer/input, and the envelope is popped. Its precise scope
is smaller than the production integration claim: it invokes the wake helper
separately after try_send, without installing the real callback, and drains
Mailbox::pop rather than Backend::turn. The five-message coalescing test proves
FIFO delivery through one manually invoked test Backend turn; no event callback
or event wait is installed in that test. The idle test proves an unsignaled
native wait blocks for its finite test timeout. These are useful focused proofs,
but not proof of every production acceptance/failure/lifetime leg.

### Acceptance linearization: independently reproduced successful-send/drop

Mailbox::try_push and push_or_wait linearize insertion under the state lock,
release it, call void poke_ui, then unconditionally return Ok/Pushed. The
installed callback ignores WakeOutcome. On SetEvent failure mailbox_wake closes
the mailbox, drops pending envelopes, wakes waiters, then stores a failure latch.
There is no propagation to the send that invoked this callback.

The native review fixture produced:

```text
sync triggering=Ok(()) future=Err(Closed) closed=true backlog=0 failure_latched=true
async triggering=Ready(Ok(())) future=Ready(Err(Closed)) closed=true backlog=0 failure_latched=true
```

Thus the triggering send receives success for an envelope that has already
been discarded before that send returns. Later Closed results do not repair
that acknowledgement. An intentionally asynchronous terminal-runtime contract
would require a guaranteed observable run result; the latch does not provide
that guarantee because its readers remain turn_body and run-loop exit, neither
of which the failing signal makes reachable.

The committed failure test queues successfully without an installed callback,
then calls mailbox_wake_for_test separately and checks the latch/future sends.
It does not observe the triggering producer's result or a guaranteed runtime
termination. Mailbox::close drains registered waiters outside its lock, but the
native failure test does not assert an actual registered waiter's wake and
terminal poll. Its comments overstate integration closure.

### Every SetEvent call: continuation bypasses the failure authority

The repository-wide source audit finds exactly two production mailbox SetEvent
calls, both in win32/mod.rs:

| Call | Actual failure policy |
| --- | --- |
| mailbox_wake, line 2441 | Check result, close/drop mailbox, latch failure; triggering send still reports success. |
| run-loop backlog continuation, line 2949 | `let _ = SetEvent(wake_event)`; ignore result entirely. |

Runtime::pump also requests continuation through Mailbox::poke_ui; that does
not justify the extra unchecked call. A remaining backlog with failed
continuation must reach the same checked terminal authority, not silently lose
its only signal. Producer-side mailbox closure and ignored UI-side failure are
not one semantic failure contract.

### Wait results: broken authority can spin instead of terminating

The loop distinguishes only WAIT_OBJECT_0. Every other value enters the same
PeekMessage sweep and next wait:

| Wait result | Current behavior |
| --- | --- |
| WAIT_OBJECT_0 | Run Backend turn, then sweep queued messages. |
| WAIT_OBJECT_0 + 1 | Sweep queue; appropriate normal input path. |
| WAIT_FAILED | No error capture, typed failure or teardown; repeat wait. |
| Unexpected result | Also silently treated as message readiness. |

If the wait repeatedly fails, the loop can repeatedly call it without blocking,
without running the turn that reads the latch, and without deterministic cleanup.
The API explicitly documents WAIT_FAILED and GetLastError. This is a concrete
missing branch, not a demand for environmental acceptance. See
[Microsoft's wait contract](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-msgwaitformultipleobjectsex).
The loop must classify its actual legal outcomes and make failed/invalid wait
results typed terminal exits through owned cleanup.

### Handle lifetime: detachment does not quiesce cloned callbacks

poke_ui clones Arc<dyn Fn()> under the wake mutex, releases that mutex and
invokes the clone. close removes only the stored callback. WakeEventGuard then
closes a raw HANDLE independently of any already admitted callback. The callback
captures a usize, not an owner keeping the kernel handle alive.

The permitted interleaving is:

```text
producer clones callback and enters mailbox_wake
producer observes closed=false, then is descheduled before SetEvent
UI shutdown marks closed, detaches callback and closes the event handle
producer resumes SetEvent using the old raw handle
```

The closed flag is a check, not a lifetime pin; the wake mutex is no longer held.
A stale callback can therefore call a closed or reused handle. Detach-before-
CloseHandle is necessary but insufficient without ownership or quiescence of
in-flight signalers. This interleaving is source-established, not experimentally
claimed as a forced handle-reuse run. SetEvent's documented failure/result
contract is at
[Microsoft SetEvent](https://learn.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-setevent).

### New ingress and initialization must retain native ownership/cleanup

The event-ready branch calls backend.turn directly at line 2927 without setting
in_dispatch or draining reentrant arrivals under the established entry contract.
The initial turn and WndProc paths do set that flag. The trampoline checks only
in_dispatch before forming a mutable Backend reference; in_turn is not its
ownership guard. A synchronous native callback during the new direct turn can
therefore form a second mutable Backend reference. The new wake ingress must
use the same guard/drain authority. This is a concrete F01 ownership regression,
not a reopening of unrelated architecture.

CreateEventExW is called after HWND creation. Its error uses `?` at line 2845,
bypassing bail/DestroyWindow/shutdown. No Backend Drop implementation destroys
the live HWND or detaches its stored Backend pointer. An event-creation failure
can therefore return with an HWND retaining a pointer to the dropped Backend.
That new failure path must use the existing owned teardown contract.

### Smallest bounded correction set

Complete the existing event authority rather than replacing the architecture:

- Define and enforce the triggering send's failure acknowledgement, or prove
  guaranteed runtime-level terminal observability for every admitted send.
- Route all signal failures and failed/unexpected waits through that one typed
  terminal policy, with deterministic cleanup; no ignored continuation result.
- Keep the event alive for all admitted callbacks, then detach/close without
  the raw-handle race. Preserve native-entry guarding/draining for event turns
  and teardown for event-creation failure.

Focused proof must observe triggering sync/async outcomes, pending disposition,
registered waiter wake/Closed, real runtime termination on a broken authority,
backlog signal failure, callback/close interleaving and event-entry reentrancy.
The successful quota/coalescing/idle legs should remain covered. No new public
application abstraction, general timer framework or broad native matrix is needed.

## F13: provenance spot-check, no material reopening

README separates current a5ad4ed send/idle/foundation receipts, immediately
prior d410e39 IME/details evidence, and older retained prior-candidate artifacts.
The identity binds the current source and verified executable hashes. Publication
is evidence/docs only; the API review points to a5ad4ed. PrintWindow remains
non-authoritative for native text fidelity. No false current-source provenance
claim requiring F13 reopening was found.

Actual current perf files record zero idle CPU, 32.2 MiB, **16 threads** and
`uptime_ms=30536`, with zero native/animation timer fires. These are recorded
worker smoke values, not new independent performance measurements or a leak
proof. No periodic mailbox timer or self-polling exists in the healthy loop.
The unhandled WAIT_FAILED path is separately a failure-path spin risk.

MINOR numeric prose remains: README's perf row still quotes 17 threads and
30545 ms. Use 16 and 30536 when updating that account. This is a bounded stale
numeric description, not a material source/binary provenance regression, and
does not reopen F13.

## Universal Gates 1–20, scoped to F01

Exactly twenty canonical gates remain. Preserved PASS decisions below do not
constitute another broad architecture review.

| Gate | Status | Closure evidence / exact remaining condition |
| --- | --- | --- |
| 1 — Completeness | FAIL | Triggering-send/durable-wake/terminal-outcome invariant remains incomplete. |
| 2 — Approval boundaries | PASS | Accepted architecture and review-only scope retained. |
| 3 — Contract preservation | FAIL | Successful send/drop and new unguarded event entry violate existing contracts. |
| 4 — Scope discipline | PASS | F01 only; F13 provenance spot-check; one new review artifact. |
| 5 — Minimal design | PASS | Waitable-event direction is sound; bounded completion of its boundaries suffices. |
| 6 — Authority/ownership | FAIL | Raw callback handle lifetime and event Backend ingress are not owned. |
| 7 — Reuse/duplication | FAIL | Producer and backlog SetEvent failures follow different semantic policies. |
| 8 — Unrequested fallback | PASS | Old cascade removed; intentional event replacement, no compatibility fallback. |
| 9 — Stable semantics | FAIL | Triggering sends can report success after wake failure discards their work. |
| 10 — Canonical truth | PASS | Current source/publication identity and retained evidence distinction preserved; minor numbers disclosed. |
| 11 — Failure safety | FAIL | Wait failure is unhandled; creation failure bypasses cleanup; callback close race remains. |
| 12 — Boundedness/lifetime | FAIL | Failed wait may loop indefinitely; handle/native-window lifetime is not universally safe. |
| 13 — Dependencies | PASS | Existing Windows crate feature enables the event API; no unrelated dependency expansion. |
| 14 — Redundant work/performance | FAIL | Healthy idle blocks, but broken wait can spin instead of terminating. |
| 15 — Infrastructure boundary | PASS | Kernel wake mechanism remains private to the backend. |
| 16 — Proof sufficiency | FAIL | Current tests omit triggering acknowledgement, admitted-callback close and actual terminal run outcome. |
| 17 — Proportional recovery | PASS | Focused event-boundary correction/tests suffice; architecture redesign unnecessary. |
| 18 — Identity/immutability | PASS | Frozen source, binaries and docs-only publication verified; earlier reviews unchanged. |
| 19 — Completion honesty | FAIL | Claimed accepted-but-unwakeable impossibility is contradicted by the native send fixture. |
| 20 — Systemic completeness | FAIL | Send, continuation, wait, callback lifetime and ingress are not yet one complete authority contract. |

## Decision

The new event is the right kind of independent blocking wake primitive. It does
not yet make successful acceptance without durable delivery or observable
terminal rejection impossible. F01 remains open; the foundation cannot freeze
and Component Gallery is not cleared. This is implementation rework within the
accepted direction, not ARCHITECTURE_BLOCKED. Unrelated environmental NOT RUN
items do not determine this verdict.

SHADCN_COMPONENT_GALLERY_NOT_CLEARED

REWORK_REQUIRED
