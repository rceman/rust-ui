# Independent Windows foundation freeze verdict

**RUN_KEY:** `RUI-A-261004-2033`

**Role:** Astra Advisor

**Started:** 2026-10-04 22:00:55 UTC+03:00

**Verdict:** `FOUNDATION_APPROVED`

This is a closure-only review of F01 against the last authoritative review.
The previous lifecycle-signal/send-acceptance counterexample is closed. The
accepted platform-contract direction is preserved. F13 and all other previously
closed F/N findings remain CLOSED; no concrete regression caused by this change
requires reopening them.

## Immutable identities and review scope

| Identity | SHA |
| --- | --- |
| Prior review / BASED_ON | `607b5dd2dcaab76eae684e16049d3ccffdbcccff` |
| Frozen production implementation | `00dc29acfeb686d6a190d91624de7ab9a48e1e92` |
| Publication inspected | `a668dec3c2ac951d12b0969f1145dca7fc2b7d3a` |

The chain is valid. Production changes tasks.rs, runtime.rs, win32/mod.rs and
focused tests. Publication changes only the evidence README and SPIKE_API_REVIEW;
no implementation, tests, harness, manifest or lockfile changes follow production.
The authoritative Windows checkout was clean on the requested branch at a668dec.

Only this new review document is created, committed and pushed. No previous
review, implementation, tests, harness or collection evidence is modified.
The pushed review SHA is recorded separately in the execution handoff. No main
merge or Component Gallery implementation is performed.

## Independently executed checks

Native authority: `W:\devin_folder\rust-ui`, Windows 11 Home build 26200,
Rust 1.94.0. Linux: separate checkout of a668dec under WSL2 kernel
`6.18.40.1-microsoft-standard-WSL2`, Rust 1.93.1.

| Check | Result |
| --- | --- |
| Windows `cargo test --locked --lib` | PASS **121/121**, no ignored tests; initial and final independent suite runs passed. |
| Linux `cargo test --locked --lib` | PASS **86/86**, no ignored tests. |
| Native Windows `cargo fmt --check` | PASS, exit 0. |
| Native Windows `cargo check --locked --all-targets` | PASS, exit 0; existing warnings remain. |
| Native Windows `cargo build --locked --all-targets` | PASS, exit 0; existing warnings remain. |
| Independent installed-seam overlap fixture | PASS: sync/async failure and success, blocked acceptance before callback completion, one callback, one payload drop, exactly-once delivery on success. |
| Focused destructor, startup, replacement, consumed-edge and native tests | Executed in the library suites and assertions inspected. |
| Source/publication/provenance audit | PASS; no current-build collection claim for retained receipts. |

The counts are explained by nine new portable regressions and one new native
real-event consumed-edge test: Linux 77 -> 86; Windows 111 -> 121. The old queue
saturation test was renamed and corrected to busy-message-queue coverage, with
no net count change. No test was disabled or ignored to obtain those counts.

The review-owned fixture is outside the repository under
`W:\devin_folder\rust-ui-freeze-verdict-checks\RUI-A-261004-2033` as
lifecycle-closure.rs/.exe/.log. It imports the actual production tasks.rs and
key.rs from the native checkout. `--cfg test` exposes only the existing
read-only mutex-observation helper used by this fixture. It installs a paused
boolean callback in the real Mailbox seam and uses actual UiProxy::try_send
and TaskSender::send. It is protocol-level deterministic failure injection;
it does not claim that SetEvent failed on a legitimately live native handle.
The real event, wait and lifetime proofs are separately native suite tests.

Windows build/test logs use the native TEMP prefix `rust-ui-freeze-verdict-`;
the portable log is `/tmp/rust-ui-freeze-verdict-linux.log`. Current review-build
executable hashes are:

```text
composer.exe     DA8B0AC4B014138DCE15E15414BF3D24B0DB7913453C3AB71D972AA4FFA3E255
native_probe.exe B173758A218CDF46A40D25375A20DFF541DFA8C1CC6A12E6D17E687A88D6EC47
```

These identify this review's build from the inspected source-equivalent
publication checkout; they do not rewrite or supersede retained collection
identity.txt. The worker's earlier intermittent parallel RichEdit ink failures
are disclosed in the evidence README. This review's native runs passed the
existing native_probe_richedit_paints_text assertion. No reproducible current
failure or causal relation to F01 was demonstrated; no rendering finding is
reopened. No broad visual/IME matrix or new idle smoke was replayed.

## Closure

```text
F01 CLOSED
```

There is no remaining correction required within the reviewed F01 surface.

## Wake-state authority and acceptance linearization

MailboxState's single mutex owns queue, waiters, closed, callback, WakeState and
the failure latch. The old independent pending_wake atomic and signal_ok shortcut
are absent. Task cancellation/completion flags and Charge counters remain
separate task-lifecycle data; they do not publish wake establishment.

| State | Meaning verified in production |
| --- | --- |
| Uninstalled { owed } | Explicit startup obligation; no delivered signal is claimed. |
| Idle | Installed authority with no outstanding confirmed edge. |
| Confirmed | The callback has already returned true while the same mutex is held. |
| Terminal | Acceptance is closed; callback is detached; no revival. |

Signal invocation remains inside the state lock, and Confirmed is assigned only
after callback success. Concurrent producers cannot observe the in-flight Idle
state, insert work or report success while the callback result remains unknown.
On false, terminal_locked completes closure/detachment under that same lock
before any blocked producer can acquire it.

For an installed authority, successful acceptance linearizes at insertion plus
successful signalling or reuse of Confirmed, before releasing the state lock.
If the producer itself establishes a failed edge, its exact tail envelope is
rolled back under the same lock and it receives typed Wake. Consumers cannot
pop it between insertion and rollback. Ownership moves, Charge release and
payload/waiter cleanup remain outside the lock; no duplicate envelope, double
drop or false pending count was found in these paths. Explicit normal task
cancellation/generation/close contracts still govern subsequent delivery.

Pre-install acceptance is the expressly selected Option B: recording the queued
payload and Uninstalled { owed: true } under the lock. This is a startup
obligation, not an installed durable signal. install_wake synchronously resolves
that obligation and returns Err(Wake) on failed establishment. Windows run checks
that result and returns typed Platform failure through owned bail/teardown after
HWND creation. wake_seen changes only Confirmed -> Idle; it cannot erase an
Uninstalled obligation. The pre-install success/failure and lifecycle-only owed
regressions execute and pass.

## Exact prior overlap: independently closed in both directions

The native review fixture pauses a lifecycle callback before returning its
result, starts a concurrent producer and checks both absence of an early result
and ownership of the state mutex. Releasing false or true produces:

```text
sync success=false result=Err(Closed)
sync success=true  result=Ok(())
async success=false result=Ready(Err(Closed))
async success=true  result=Ready(Ok(()))
```

All four cases report blocked_before_release=true, callback_calls=1 and
payload_drops=1. Failure leaves the queue empty and future sends Closed; neither
sync nor async sends reports success. Async polls terminate rather than leaving
a parked future. Success reuses only the published Confirmed edge, retains the
exact payload until dequeue and delivers it once without a second callback.
The old successful-send-then-drop counterexample is no longer reproducible.

The four portable overlap tests independently assert those same meaningful
outcomes, including payload identity, exact drop counts and post-terminal sends.
This is synchronization proof, not merely testing that failures set a flag.

## Lock discipline, lifecycle and replacement

There is one production-installed wake callback outside tests: Windows run's
closure reads the atomic backend/window liveness state and signals its
Arc-owned MailboxEvent. It does not access MailboxState, call Mailbox, wake user
waiters, synchronously dispatch WndProc or drop a captured reentrant object.
The kernel signal is compatible with holding the mailbox mutex. No reverse
mailbox lock acquisition exists in that production callback.

MailboxState::signal temporarily clones the stored callback; its local Arc drop
is nonfinal because the stored callback still owns it under the lock. Replacement
and terminal detachment move owning Arcs out; their final destruction, queued
Envelope/Charge drops and waiter wakes occur after unlocking. The captured-
destructor regression asserts a free lock and actually re-enters mailbox queries
on both failed installation and replacement. Charge purge/close and registered
waiter proofs also remain green; no lock-order cycle was found.

Charge::drop, task completion, installation, runtime and bounded backlog all use
this same signal authority. Even where a worker lifecycle caller ignores the
local boolean, false has already installed Terminal under the mutex: concurrent
and future producers cannot return success behind that failure. UI-owned runtime
continuation now propagates false as typed error, rather than waiting for an
unreachable later observation. Failure latches survive routine close.

Replacing a callback does not inherit an old Confirmed result. install_wake
records the obligation, switches to Idle, invokes the new callback and confirms
only its successful result. A queued backlog or startup debt similarly forces
the new pulse. Old callback destruction is outside the mutex. Installing with
nothing owed and an empty queue does not introduce an idle signal.

## Native edge consumption and preserved backend contracts

The real-event test consumes the auto-reset event while logical state is still
Confirmed, then sends before wake_seen. That send coalesces; the active UI turn
owns its upcoming sweep. Runtime::pump calls wake_seen before bounded queue
consumption, so pre-observation work is included. A producer arriving after
wake_seen sees Idle and establishes a fresh native edge. The test asserts
callback counts, native signal observation and exact-once FIFO values across
both boundaries and a repeated cycle. Bounded remainder uses the same authority;
no pulse loss or permanent idle polling was found.

There is one production SetEvent site in MailboxEvent::signal and one native
MsgWaitForMultipleObjectsEx site. Failed and unexpected wait results remain
typed terminal exits through bail; there is no ignored direct SetEvent or old
mailbox post/send/timer cascade. Same-thread WM_PUMP remains a distinct checked
reentrant/deferred-native mechanism. Event-ready turns retain guarded_turn and
mandatory teardown; ownerless IME behavior is unaffected.

Arc event ownership still covers run-loop, installed and admitted callbacks,
with CloseHandle only on final ownership release. Initialization still precedes
HWND route ownership. These previously accepted fixes are preserved.

The native queue test now correctly describes bounded busy posted-message
traffic, up to 512 successful posts, and asserts nonempty traffic. The actual
try_send establishes the owned event through the installed seam. This proves
independence from ordinary queue load; it is not quota-exhaustion evidence and
no resource-exhaustion stress is required.

## F13 spot-check and approval limits

F13 remains CLOSED. Current candidate and regression verification identify
00dc29a. Publication is documentation only. identity.txt correctly remains the
receipt for the retained eaf5a56 collection and its older binary hashes; it is
explicitly not this candidate's receipt. README separately identifies the older
a5ad4ed foundation image, d410e39 IME/details and earlier retained artifacts.
No current synchronization-build idle/IME/visual collection is falsely claimed.
Those historical receipts are not converted into fresh measurements by this
review's successful tests/builds.

Approval means the Windows foundation may freeze, the accepted shared-platform
contract direction stands and the post-foundation Shadcn Component Gallery may
begin. It does not claim macOS completion, complete future component coverage,
new environmental acceptance or immunity from future performance regressions.
Previously documented environmental NOT RUN limitations remain scoped as before;
they do not block this completed F01 closure.

## Universal Gates 1–20: affected closure surface

Exactly twenty canonical gates remain. The preserved PASS decisions are scoped
to this final closure and prior accepted findings, not a new broad review.

| Gate | Status | Evidence |
| --- | --- | --- |
| 1 — Completeness | PASS | Exact failing lifecycle/send overlap closes for sync and async. |
| 2 — Approval boundaries | PASS | Narrow closure-only scope and explicit approval limits retained. |
| 3 — Contract preservation | PASS | Direct Wake failure, terminal concurrent sends and normal delivery preserved. |
| 4 — Scope discipline | PASS | One review artifact; no implementation or other milestone work. |
| 5 — Minimal design | PASS | Existing state mutex and event authority complete the correction. |
| 6 — Authority/ownership | PASS | One mutex owns wake completion and acceptance; Arc owns the native event. |
| 7 — Reuse/duplication | PASS | One signal, one wait and one terminal mailbox transition. |
| 8 — Unrequested fallback | PASS | No old cascade, shim or new fallback. |
| 9 — Stable semantics | PASS | Confirmed follows actual callback success; startup obligation is explicit. |
| 10 — Canonical truth | PASS | Current regression identity and retained receipt provenance are distinguished. |
| 11 — Failure safety | PASS | Rollback/terminal state precedes competing acceptance; destruction is post-unlock. |
| 12 — Boundedness/lifetime | PASS | Existing queue/waiter bounds, bounded sweeps and owned handle lifetime preserved. |
| 13 — Dependencies | PASS | No dependency/infrastructure expansion in this correction. |
| 14 — Redundant work/performance | PASS | Confirmation coalesces; empty installation does not pulse; healthy idle blocks. |
| 15 — Infrastructure boundary | PASS | Platform-native event remains private; shared semantics stay in MailboxState. |
| 16 — Proof sufficiency | PASS | Independent overlap fixture, 121 native/86 portable tests and focused edge/destructor proofs. |
| 17 — Proportional recovery | PASS | Bounded F01 correction; no architecture redesign or unrelated rendering investigation. |
| 18 — Identity/immutability | PASS | Exact frozen/publication identities verified; prior reviews remain immutable. |
| 19 — Completion honesty | PASS | Approval rests on current tests; retained captures and intermittent history qualified. |
| 20 — Systemic completeness | PASS | Acceptance, lifecycle, install, consumption, replacement and terminal paths share the state authority. |

SHADCN_COMPONENT_GALLERY_CLEARED

FOUNDATION_APPROVED
