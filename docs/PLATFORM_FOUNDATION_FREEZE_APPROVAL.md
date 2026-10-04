# Independent Windows foundation freeze approval review

**RUN_KEY:** `RUI-A-261004-1250`

**Role:** Astra Advisor

**Started:** 2026-10-04 12:49:59 UTC+03:00

**Verdict:** `REWORK_REQUIRED`

This reviews only the remaining F01 and F13 findings from the authoritative
previous review. The platform-contract architecture remains accepted. There
is no architecture redesign, Component Gallery work or main merge.

## Immutable identities and scope

| Identity | SHA |
| --- | --- |
| Prior independent review / BASED_ON | `38727ed916452b18a949c5f745fedaac9c7a00a7` |
| Frozen production implementation | `d410e393d9d3cd0a3b5c0af997e9b6d8431a280b` |
| Evidence/docs publication inspected | `15d5b4a4ccd04ce4d4d6d6b4c524411424121160` |

The identity chain is valid. The production commit changes win32/mod.rs,
win32/window.rs and focused library tests. The publication commit changes only
evidence and documentation: no production, tests, harness, manifest or lockfile
changes follow the frozen implementation. The README's older harness SHA
`9d78f127fe477557f73e3350088bc36e794c28d0` identifies the unchanged harness
implementation; the current binaries are built from the complete `d410e39`
source snapshot, as the receipt records.

The authoritative Windows checkout was clean on the requested branch at
`15d5b4a`. The current receipt records:

```text
HEAD=d410e393d9d3cd0a3b5c0af997e9b6d8431a280b
exe_sha256=ACB787B150BC82013F885BB17E89AE700B36BB64FC4D43908E438E2383095AB6
probe_sha256=FB6AF595629B79AF0B9E06AB4D29828DA8304ADA208DFFB15AA8E26FE2BA0EBD
run_utc=2026-10-04T09:46:24.4898401Z
```

Both executable hashes matched independently before and after the review's
build/check commands. Publication changes ten evidence/docs files. Changed
captures and idle receipts, the current source receipt and explicit provenance
statement support the declared collection relationship. An unchanged image's
Git blob does not establish that it was not recollected; identical output may
be regenerated. Conversely, the receipt is not a retrospective source identity
for every retained artifact.

Only this new review document is created, committed and pushed. No previous
review, implementation, test, harness or evidence file is changed by this review.
The pushed review SHA is recorded separately in the execution handoff.

## Independent verification

Native authority: `W:\devin_folder\rust-ui`, Windows 11 Home build 26200,
Rust 1.94.0. Portable tests: separate checkout of `15d5b4a` under WSL2 Linux,
kernel `6.18.40.1-microsoft-standard-WSL2`, Rust 1.93.1.

| Independently executed check | Result |
| --- | --- |
| Windows `cargo test --locked --lib` | PASS: **102/102**, no ignored tests. |
| Linux `cargo test --locked --lib` | PASS: **77/77**, no ignored tests. |
| Native Windows `cargo fmt --check` | PASS, exit 0. |
| Native Windows `cargo check --locked --all-targets` | PASS, exit 0; warnings remain. |
| Native Windows `cargo build --locked --all-targets` | PASS, exit 0; warnings remain. |
| Source/publication and binary identity audit | PASS. |
| Current committed physical details capture | Inspected: caption and full draft-prefill\q visible; three details labels visibly separated, no foreign-window occlusion. |
| Current final-foundation capture | Inspected; no visible label overlap. PrintWindow remains non-authoritative for native text fidelity. |

Windows logs are in the native user's TEMP directory as
`rust-ui-freeze-approval-tests.log`, `rust-ui-freeze-approval-check.log` and
`rust-ui-freeze-approval-build.log`. The Linux log is
`/tmp/rust-ui-freeze-approval-linux.log`.

The current idle smoke, send/IME/details collection and complete native scenario
matrix are recorded worker evidence, not independently replayed in this review.
No live-window resource exhaustion causing native SetTimer to fail was induced.
The all-three-fail conclusion below is a production control-flow audit against
the documented native API contract, not a claim of an independently reproduced
live-window timer-allocation failure.

## Closure matrix

```text
F01 PARTIALLY_CLOSED
F13 CLOSED
```

Previously closed F02–F12 and N01–N04 are not reopened. No concrete regression
requiring their reopening was found within this closure surface.

## F01 — timer recovery improves progress; terminal failure remains unowned

### What is closed or preserved

The production wake authority now tries PostMessageW, then a synchronous send
with a 250 ms timeout, then SetTimer with a 50 ms recovery interval. The reserved
recovery ID is distinct from the deadline ID and native peer timer namespace.
The recovery WM_TIMER handler calls KillTimer before the real Backend turn.
No recovery timer is armed during normal successful posted wakes, and no
permanent polling loop was introduced.

`mailbox_wake_survives_queue_saturation_then_recovery` passed independently. It
uses a real receiver thread/window and GetMessage loop, accepts one envelope,
fills the posted queue, forces the send timeout, asserts TimerArmed and observes
the envelope drained after receiver recovery without another producer/enqueue
or incidental UI input. It also checks no wake-failure latch remains.

The proof's exact boundary matters: that receiver's custom WndProc drains via
Mailbox::pop directly. It does not execute Backend::turn or the production run
loop. The separately passing `mailbox_recovery_timer_dispatches_a_turn` manually
dispatches a recovery WM_TIMER through the actual owned dispatcher with a test
Backend and proves one application update. It is not the same saturated-queue
native trace. These tests support the individual legs; they do not reproduce
the specifically requested combined native-loop/Backend success trace.

The existing mid-drain destruction, more-than-128 ordinary items followed by
teardown, earlier-error followed by teardown, and ownerless IME regressions
remain in the passing native suite. Their ownership/drain code is unchanged by
this correction; spot inspection found no regression.

### Smallest remaining failing invariant

**Accepted work still has no guaranteed progress or terminal observability when
all three native wake routes fail.** The production trace is:

```text
one envelope is accepted
PostMessageW fails
SendMessageTimeoutW fails
SetTimer returns zero
Mailbox::wake_post_failed sets wake_failed and clears pending_wake
mailbox_wake returns Latched; the installed Fn() callback ignores that return
receiver recovers and exhausts ordinary messages
no recovery timer or pump is owned
receiver can remain in GetMessage with accepted work and an unread latch
```

The only wake-latch readers are Backend::turn_body and the code after the
GetMessage loop exits. Neither operation is made reachable by the all-failed
branch. Resetting pending_wake permits a future enqueue to retry; it does not
provide progress without that future enqueue. No terminal close, independently
observable completion/error or guaranteed wait interruption is established.

Microsoft documents a zero SetTimer return as failure to create a timer; it
does not define that result as proof of window destruction or run-loop exit.
Therefore the destroyed-HWND test cannot establish the needed universal
terminal-window implication. See the
[SetTimer contract](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-settimer).
That contract also specifies calling-thread ownership of the associated window;
the inline producer wake callback does not establish that ownership. The native
fixture's successful cross-thread arm on this machine is measured behavior,
not a documented guarantee that the final route cannot fail on a live receiver.

WakeOutcome labels accurately describe posting, completed sending, arming,
latching and skipping. Latched is not success: its consumer is a void callback,
and storing the flag does not make the failure observable to the sleeping
receiver or producer. The dead-HWND test explicitly reads the flag itself;
that is not the production loop's terminal observability proof.

The idle receipt reports `native_timer_fires=0`; that counter covers native peer
timer dispatch, not recovery WM_TIMER dispatch. It is consistent with normal
idle behavior, but alone cannot prove no recovery timer remained armed. The
focused tests do not count late recovery messages or prove a single Backend
turn in the combined saturated-queue scenario. No additional independent
duplicate-delivery defect is asserted from that proof gap.

### Bounded correction required before freeze

Keep the existing accepted architecture and wake authority. Make the
all-three-fail branch own a guaranteed terminal outcome or reachable progress
without another enqueue/input; an unread latch is insufficient. Establish the
native API ownership contract for the recovery route. Add focused proof of the
all-failed live-receiver case and the requested real GetMessage/Backend timer
recovery trace, including one delivery, cleared latch and one-shot cleanup.
This is completion of F01, not a request for a new timer framework.

## F13 — current/retained provenance is materially corrected

The README now explicitly separates fresh `send-*`, perf receipts,
final-foundation, IME and details evidence from retained smoke/typing/Unicode,
selection/undo/readonly/disabled/multiline/reorder/UIA/DPI/scale/theme artifacts.
Retained evidence is identified as prior-candidate, with historical provenance
in Git. It no longer implies those artifacts were all freshly measured on the
current candidate. The API review's current candidate is `d410e39`.

The updated identity hashes match the actual binaries. The changed physical
capture is unobscured and shows separated labels; its UIA receipt has strictly
ordered, nonoverlapping vertical extents. PrintWindow is explicitly classified
as non-authoritative for native text fidelity. Historical chronology remains.

The fresh perf files record zero idle CPU, **32.3 MiB**, **17 threads**, and
`uptime_ms=30574`, with `native_timer_fires=0` and no animation timer fires.
The clean-shutdown receipt demonstrates counter emission, not a general leak
proof. This review does not convert that one smoke into universal performance
or resource guarantees.

**MINOR, nonblocking documentation discrepancy:** README's perf row still
quotes `uptime_ms=30545` instead of the current file's `30574`. Correct that
literal when next updating the evidence account. The 29 ms discrepancy does
not invalidate the explicit source/binary provenance or change the approximately
30.5-second idle result; it is not retained as a material F13 blocker.

## Universal Gates 1–20: affected closure surface

Exactly twenty canonical gates remain. PASS here is scoped to this closure
surface and preserved prior decisions, not a new broad architecture review.

| Gate | Status | Evidence / remaining condition |
| --- | --- | --- |
| 1 — Completeness | FAIL | F01's all-three-fail progress/terminal invariant remains. |
| 2 — Approval boundaries | PASS | Accepted architecture and environmental limits preserved. |
| 3 — Contract preservation | FAIL | Accepted mailbox work can still become stranded after all wake routes fail. |
| 4 — Scope discipline | PASS | Only F01/F13 reviewed; only a new review artifact published. |
| 5 — Minimal design | PASS | Bounded correction in existing wake/error authority suffices. |
| 6 — Authority/ownership | FAIL | Final wake failure owns no reachable progress/terminal outcome. |
| 7 — Reuse/duplication | PASS | Existing wake, drain and native-entry authorities retained. |
| 8 — Unrequested fallback | PASS | Recovery route is explicitly requested in this task. |
| 9 — Stable semantics | FAIL | Failure branch still depends on future input/producer/exit to be observed. |
| 10 — Canonical truth | PASS | Material current/retained provenance and API identity corrected; minor numeric typo disclosed. |
| 11 — Failure safety | FAIL | Latched failure does not itself interrupt or terminate the receiver wait. |
| 12 — Boundedness | PASS | One reserved timer slot, bounded send, existing drain bounds; no new unbounded allocation found. |
| 13 — Dependencies | PASS | No new dependency or unrelated expansion. |
| 14 — Redundant work | PASS | No permanent idle recovery polling; receipt counter scope is qualified above. |
| 15 — Infrastructure boundary | PASS | Harness/consumer behavior remains outside public library semantics. |
| 16 — Proof sufficiency | FAIL | All-three-fail and combined native-loop/Backend success proof are incomplete. |
| 17 — Proportional recovery | PASS | Focused wake correction and tests suffice; no architecture redesign. |
| 18 — Identity/immutability | PASS | Exact candidate, executable hashes and docs-only publication verified; old reviews unchanged. |
| 19 — Completion honesty | FAIL | Universal wake closure cannot be claimed from the current latch and split tests; provenance itself is corrected. |
| 20 — Systemic completeness | FAIL | The last failure branch remains outside the guaranteed progress/terminal invariant. |

## Freeze decision

The architecture direction is sound, and F13's material provenance correction
is accepted. F01 still needs its final failure outcome and focused integration
proof. Windows foundation freeze and Component Gallery are not cleared by this
review. Unrelated environmental NOT RUN items do not cause this decision.

SHADCN_COMPONENT_GALLERY_NOT_CLEARED

REWORK_REQUIRED
