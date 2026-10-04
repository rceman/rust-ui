# Independent final F01 Windows foundation closure review

**RUN_KEY:** `RUI-A-261004-1714`

**Role:** Astra Advisor

**Started:** 2026-10-04 17:21:31 UTC+03:00

**Verdict:** `REWORK_REQUIRED`

This reviews only the acceptance/signal/wait/lifetime surface changed for F01.
The platform-contract architecture remains accepted. F13 and all other F/N
findings remain CLOSED. No Component Gallery work, broad foundation review,
implementation change or main merge is performed.

## Immutable identities

| Identity | SHA |
| --- | --- |
| Prior review / BASED_ON | `ac6bb2d7276d64b4398ad89354a13856f4c046be` |
| Frozen production implementation | `eaf5a566b0a72e5e3e3a0e25644acc6190f4f3b6` |
| Publication inspected | `0f28fdef82978357a729a8efa6a8855c9cf8babe` |

The identity chain is valid. Production changes win32/mod.rs, runtime.rs,
tasks.rs and focused tests. Publication changes seven evidence/docs files only;
no production, tests, harness, manifest or lockfile changes follow production.
The native checkout was clean at the publication SHA on the requested branch.

The receipt records:

```text
HEAD=eaf5a566b0a72e5e3e3a0e25644acc6190f4f3b6
exe_sha256=347CDB25C5594770431FE7D6BF6D3F543DA151A28612DCD4AE47E9DE6619CAA8
probe_sha256=16687D45C6CAFBD226A928F42245B23E08897C532533AC7A616BB28119B4D78E
run_utc=2026-10-04T14:13:54.3378160Z
```

Both executable hashes matched independently before and after the required
checks. Previous reviews are unchanged. Only this new review document is
created, committed and pushed; the review SHA is recorded separately in the
execution handoff. No repository implementation, tests, harness or evidence
are modified by this review.

## Independent verification

Native authority: `W:\devin_folder\rust-ui`, Windows 11 Home build 26200,
Rust 1.94.0. Linux: separate checkout of 0f28fde under WSL2 kernel
`6.18.40.1-microsoft-standard-WSL2`, Rust 1.93.1.

| Independently executed check | Result |
| --- | --- |
| Windows `cargo test --locked --lib` | PASS **111/111**, exit 0, no ignored tests. |
| Linux `cargo test --locked --lib` | PASS **77/77**, exit 0, no ignored tests. |
| Native Windows `cargo fmt --check` | PASS, exit 0. |
| Native Windows `cargo check --locked --all-targets` | PASS, exit 0; warnings remain. |
| Native Windows `cargo build --locked --all-targets` | PASS, exit 0; warnings remain. |
| Installed-seam lifecycle/send overlap fixture | Reproduced successful sync/async sends followed by payload discard after the overlapping wake fails. |
| Identity and provenance spot-check | PASS; F13 remains CLOSED. |

The focused Windows fixture imports the actual production tasks.rs and key.rs
from the authoritative checkout. Its installed boolean wake callback pauses
on a channel, then returns false: the same failure outcome used by the candidate's
installed-seam failure tests. It adds no repository test or implementation seam.
It is under `W:\devin_folder\rust-ui-final-freeze-checks\RUI-A-261004-1714`
as pending-edge-race.rs/.exe/.log.

This is deterministic failure injection at the real Mailbox callback boundary.
It does not claim that SetEvent was observed failing on a valid Arc-owned kernel
handle, nor that the complete native application loop was fault-injected. The
shared boolean signal contract declares this failure outcome and the candidate
claims its acceptance semantics cover it; the fixture tests that actual contract.
Native kernel-event success, lifetime and wait-classifier tests were independently
executed through the library suite. Current send/idle/foundation collection and
the worker's three consecutive green runs remain reported evidence rather than
claims that this review replayed those exact collection/run sequences. No broad
visual/IME matrix replay was required or performed.

Windows logs use the native TEMP prefix `rust-ui-final-freeze-`; the Linux log is
`/tmp/rust-ui-final-freeze-linux.log`.

## Closure

```text
F01 PARTIALLY_CLOSED
```

The remaining invariant is narrow: **a send coalesced against an in-flight
lifecycle wake can return success before durable wake establishment, then lose
its payload when that signal fails, without a guaranteed observable terminal
runtime outcome.** The other concrete corrections from the previous review are
accepted as described below; no unrelated finding is reopened.

## Corrections actually closed

### Direct triggering sends and rollback

With no pending edge, try_push and push_or_wait now hold the mailbox state lock
across insertion and signal result. False rolls back pop_back while that lock
still excludes every producer and consumer: the triggering envelope remains
exactly the tail and cannot be dequeued between insertion and rollback.
The direct installed-seam tests assert Err(ProxySendError::Wake) and the real
TaskSender async Err(SendError::Wake). These pass independently.

The failed envelope is returned by ownership, remaining queued envelopes are
detached under the same lock, and drops/waiter wakes occur after unlock. UiProxy
releases its returned envelope once. SendFuture reclaims the payload/Charge and
releases the Charge before reporting Wake; the payload remains normally owned
by the future until drop. No unsafe duplication or false pending count was found
in that direct rollback path. Future sends see Closed.

The intended linearization point is now insertion plus successful signal result
(or an already established coalesced edge), before releasing the state lock.
That point is sound for the direct callback-success/failure path, but the
pending-edge shortcut still admits an unconfirmed lifecycle result.

### Single native signal, wait and event-lifetime authorities

The repository-wide audit finds one production SetEvent call, in
MailboxEvent::signal, and one production MsgWaitForMultipleObjectsEx site.
The ignored direct backlog SetEvent call is gone. The run-loop backlog uses
Mailbox::poke_ui and bails on its false result. Runtime/lifecycle/install callers
also use that seam; their pending-edge concurrency is the residual defect below.
The cross-thread post/send/timer cascade remains removed. Same-thread WM_PUMP
serves the distinct reentrant/deferred-native continuation contract.

Wait results are explicit: event, queue input, WAIT_FAILED with last_os_error,
and unexpected result. Both error classes route to typed bail/teardown. The
classifier proof passes; the previous failed-wait spin is repaired. With one
actual handle, classification matches the complete production wait domain.

Arc<MailboxEvent> now owns CloseHandle. The run loop, installed callback and
already-cloned callbacks retain strong ownership. Detachment cannot invalidate
an admitted signaler. The deterministic test holds an actual cloned callback
across mailbox detach and dropping the loop's Arc, signals successfully and
observes the native event; final callback drop releases the last owner. There
is one event Drop authority and no production raw-owner duplication.

### Native entry, initialization and parked waiter behavior

Initial and event-driven turns now call guarded_turn, which sets in_dispatch,
runs turn, clears the flag and drains reentrant arrivals on success. Run errors
enter bail, whose destruction/drain/shutdown path owns mandatory cleanup.
The trampoline's existing flag check therefore prevents a second direct mutable
Backend entry during a synchronous callback. The focused test asserts delivery
and drainage of prequeued owned native work; it is not itself a new synchronous
WndProc injection, but the inspected helper and existing trampoline authority
establish the required routing. No new ownership regression remains here.

Event creation now precedes window::create and the HWND's Backend route pointer.
The injected init helper returns typed Platform error; source ordering proves
that failure occurs before a live HWND exists. The old dangling-window failure
path is removed; normal Arc/COM field destruction owns already-created resources.

The full-mailbox waiter test registers a genuine parked async send, closes the
mailbox through failed installation signalling and polls the awakened executor.
It asserts zero live tasks. The public outcome is terminal task closure:
CancelWatch sees mailbox Closed and completes without resuming the inner send
body; dropping that body drops SendFuture and unregisters its waiter. A retained
TaskSender observes is_closed and subsequent sends report Closed. This is a
valid non-stranding task contract; an inner Err(Closed) acknowledgement is not
required for that already parked task.

## One remaining failure: an unconfirmed lifecycle edge is treated as durable

signal_ok swaps pending_wake to true before invoking its callback. A second
caller seeing true immediately returns success. Producer push paths hold state,
but poke_ui does not hold that authority across its callback/result. Charge
release, CancelWatch completion and install/backlog pulses can use poke_ui.
Their in-flight result is consequently not serialized with new send acceptance.

The independently reproduced trace uses one lifecycle poke and one envelope:

```text
lifecycle poke sets pending_wake=true
installed callback begins, pauses before returning its signal result
producer locks state, inserts its envelope
signal_ok sees pending_wake=true and returns true without signalling/waiting
producer reports Ok; its envelope is queued
lifecycle callback returns false
poke_ui closes the mailbox and drops the accepted envelope, then latches failure
```

Actual Windows fixture output:

```text
direct sync triggering=Err(Wake)
sync coalesced_send=Ok(()) closed=true backlog=0 dropped=1 failure_latched=true
async coalesced_send=Ready(Ok(())) closed=true backlog=0 dropped=1 failure_latched=true
```

Each affected payload drops exactly once; the defect is acknowledgement and
terminal observability, not double drop or retained queue growth. The state
lock prevents competing queue insertion during a direct send's own callback,
but cannot protect against a lifecycle callback operating outside that lock.
An edge whose callback has not yet succeeded is not proof of durable signalling.

Later Closed results and a stored latch do not compensate for this coalesced
send's success. The lifecycle caller is not necessarily the UI thread: a worker's
Charge release or completion can invoke it. Its false result is ignored by those
callers. The latch readers still require a Backend turn or loop exit. This path
does not signal a separate terminal wake or otherwise establish that those readers
will run. Correct WAIT_FAILED classification handles a failing wait; it is not
a proof that every false signal result must also break or terminate the wait.

No new architectural failure is asserted. This is the same acceptance/wake
invariant that the correction claims to close, now exposed at the explicitly
requested lifecycle/coalescing boundary.

### Smallest correction before freeze

Make all signal-state transitions participating in send acceptance share the
same authority: pending/in-flight cannot count as confirmed durable wake before
its result is known. Serialize or resolve lifecycle/install signal results with
coalesced acceptance, so failed establishment rejects affected sends or has a
guaranteed observable terminal outcome covering them. Add the overlapping
lifecycle-poke/sync-send and lifecycle-poke/async-send regressions. Preserve the
correct event ownership, wait classification and native-entry fixes; no further
backend architecture replacement or broad defect-discovery cycle is needed.

## Healthy paths and evidence scope

The native queue fixture now caps successful posts at 512. It is a bounded
busy-queue/event mechanism exercise, not measured queue-quota exhaustion.
That reduction is acceptable: kernel signalling is independently checked and
its production wake authority contains no posted-message call. Its old test
header still says saturated-to-quota; that wording should match the bounded
exercise when the focused regression is updated. No exhaustion stress is demanded.

The FIFO/coalescing, idle blocking, send-after-close, mandatory mid-drain
teardown and ownerless IME cases remain in the passing suite. The five-envelope
FIFO test invokes one test Backend turn; it is not a count of kernel event pulses.
Healthy production idle uses INFINITE without a mailbox timer or polling loop.

F13 remains CLOSED. Current identity and API candidate identify eaf5a56;
publication is evidence/docs only. README distinguishes current wake-build
receipts from retained prior-candidate IME/details and older artifacts. Recorded
current idle values are zero CPU, 32.3 MiB, 17 threads, 30555 ms and zero native
and animation timer fires. These are worker smoke receipts, not a general leak
proof or a new independently reproduced idle run. Minor stale numeric/comment
wording does not reopen material provenance findings.

## Universal Gates 1–20: F01 closure surface

Exactly twenty canonical gates remain. PASS is scoped to this changed surface
and preserved prior decisions, not a renewed broad foundation review.

| Gate | Status | Evidence / exact residual condition |
| --- | --- | --- |
| 1 — Completeness | FAIL | Lifecycle/coalesced acceptance still permits success followed by silent discard. |
| 2 — Approval boundaries | PASS | Accepted architecture and narrow review-only scope preserved. |
| 3 — Contract preservation | FAIL | Coalesced sends lack the required durable-wake/terminal acknowledgement. |
| 4 — Scope discipline | PASS | F01 only; provenance spot-check; one new review artifact. |
| 5 — Minimal design | PASS | Existing event and state authority need bounded coordination, not redesign. |
| 6 — Authority/ownership | FAIL | Queue acceptance lock does not own lifecycle signal completion. Event handle ownership itself is repaired. |
| 7 — Reuse/duplication | PASS | One native SetEvent, one wait and shared checked continuation seam. |
| 8 — Unrequested fallback | PASS | No old cascade or compatibility fallback reintroduced. |
| 9 — Stable semantics | FAIL | Pending-before-confirmation is treated as successful durable establishment. |
| 10 — Canonical truth | PASS | Current source/binary/provenance identity is accurate; narrow wording limits disclosed. |
| 11 — Failure safety | FAIL | Coalesced accepted work can be dropped with only an unread lifecycle latch. |
| 12 — Boundedness/lifetime | PASS | State bounds retained; wait spin and raw-handle/native-window lifetime failures repaired. |
| 13 — Dependencies | PASS | No unrelated dependency expansion. |
| 14 — Redundant work/performance | PASS | Healthy idle blocks; failed waits now terminate; no periodic mailbox work. |
| 15 — Infrastructure boundary | PASS | Native event remains private to the backend. |
| 16 — Proof sufficiency | FAIL | Isolated failure tests miss the independently reproduced in-flight lifecycle/send overlap. |
| 17 — Proportional recovery | PASS | One bounded coordination correction and focused overlap proofs suffice. |
| 18 — Identity/immutability | PASS | Frozen source and binaries verified; previous reviews unchanged. |
| 19 — Completion honesty | FAIL | Universal durable-acceptance claim is contradicted by the shared installed-seam fixture. |
| 20 — Systemic completeness | FAIL | Lifecycle and producer acceptance are not yet one complete atomic failure boundary. |

## Verdict

The systemic event/ownership corrections are materially improved and largely
accepted. F01 remains open solely at lifecycle-signal/coalesced acceptance.
That bounded invariant must close before Windows foundation freeze or Component
Gallery clearance. No unrelated environmental NOT RUN item causes this decision.

SHADCN_COMPONENT_GALLERY_NOT_CLEARED

REWORK_REQUIRED
