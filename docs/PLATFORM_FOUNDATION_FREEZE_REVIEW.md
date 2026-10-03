# Independent narrow Windows foundation freeze review

**RUN_KEY:** `RUI-A-261003-1443`

**Role:** Astra Advisor

**Started:** 2026-10-03 14:53:30 UTC+03:00

**Verdict:** `REWORK_REQUIRED`

This reviews closure of the remaining findings in the authoritative prior
approval review. The platform-contract architecture remains accepted. The
remaining corrections belong to the existing queue, wake and verification
owners; they do not justify an architecture redesign.

## Identity chain and review scope

| Identity | Exact SHA |
| --- | --- |
| Prior independent review / BASED_ON | `a09c284349db661b5093533c255c046fe1db3df9` |
| Last non-test production/library semantic change | `b99d55893c8b6a3b0b585645bb34f31dc6cc747f` |
| Harness/binary source snapshot | `4e1aa6bb504c2ebc5aca74f04e7c73a37cfbae65` |
| Publication reviewed | `2f4410881c0ef238ce60c6d2d3f6c0d154dc7a56` |

**The identity chain is valid, with the declared test-only exception and a
formatting exception.** Git diffs show that `b99d558` to `4e1aa6` changes only
`examples/native_probe.rs` and benchmark scripts/evidence. The latter snapshot
contains the target-thread IME engagement harness and details scenario.
`4e1aa6` to `2f44108` changes README, receipts/captures, two formatting-only
expressions in `examples/native_probe.rs`, and one `#[cfg(windows)]` restoration
on `src/tests.rs::probe_wndproc`. It does not change production/library
semantics, Cargo manifests or dependencies. Thus publication is evidence plus
formatting and test gating, rather than literally evidence files only.

Linux verification below applies to **publication `2f44108`**, including the
restored test gating. It is not a claim that the ungated Linux tests at the
binary-source snapshot ran successfully.

The receipt records:

```text
HEAD=4e1aa6bb504c2ebc5aca74f04e7c73a37cfbae65
exe_sha256=9C310FF514E8CCF433E55CBFC6602926D14643396E900DDA38ED22A047E33443
probe_sha256=132E71E81E8E16D87FF8DED2B3FCE916A669767BCFE63E9C33760632DE006182
run_utc=2026-10-02T20:55:44.0990193Z
```

Both executable hashes independently matched before build checks. Identical
copies were retained outside the repository for the focused native run. The
receipt identifies source and binaries; it does not certify every accompanying
claim or screenshot. Neither later publication change introduces a second
production implementation candidate.

The authoritative Windows checkout was clean at `2f44108` on the requested
branch. Git comparison confirms all four previous independent review documents
are unchanged since `a09c284`. This review publishes only this new document.
No production, tests, harness, examples, evidence or previous review files are
modified by the publication. No Component Gallery work or main merge occurred.
The containing review commit is identified in Git history and the execution
handoff, separately from production and evidence identities.

## Independently executed checks

Native authority: `W:\devin_folder\rust-ui`, Microsoft Windows 11 Home,
build 26200, Rust 1.94.0. Portable verification: detached `2f44108` checkout,
WSL2 Linux `6.18.40.1-microsoft-standard-WSL2`, Rust 1.93.1.

| Check | Result and limits |
| --- | --- |
| Native Windows `cargo test --locked --lib` | PASS **97/97**, no ignored tests. Inspected focused closure assertions. |
| Linux `cargo test --locked --lib` | PASS **77/77**, no ignored tests. |
| Native Windows `cargo fmt --check` | PASS. |
| Native Windows `cargo check --locked --all-targets` | PASS, with existing warnings. |
| Native Windows `cargo build --locked --all-targets` | PASS, with warnings. |
| External native Backend regression | **FAIL:** teardown arriving during the drain remains queued; closed=true, one message remains, mounted peer release count=0. |
| Additional native provider-array lifecycle assertions | PASS: real nonempty provider and range arrays; retained pattern, ranges, enclosing provider and extracted provider all return `UIA_E_ELEMENTNOTAVAILABLE` after removal, same-slot recreation and close. |
| Verbatim `json_str` helper, strict JSON round-trip | PASS for backslash-q, quotes, backslash, LF, CR, tab, every U+0000–001F control, kana and supplementary Unicode. |
| Fresh receipt-bound composer/probe run | PMv2 geometry asserted; 120 DPI / scale 1.25. Three details labels exist at distinct, vertically ordered, nonoverlapping UIA rectangles. |
| Fresh physical-screen and PrintWindow captures | Actual foreground composer has full caption/prefill and separated details labels. Published physical image has an occlusion problem described under F12. |
| Focused real-IME acceptance | Newly gained `ありがと`, native/app TextValue agreement, submit count unchanged at 0; following `Z` edit observed by app echo. This run does not establish the missing layout/mode verification branches under F11. |

External regression work used a separate source copy under
`W:\devin_folder\rust-ui-freeze-checks\source`. Only its review tests were
extended. The repository implementation was not edited. The teardown fixture
runs real Backend dispatch/drain and checks a mounted fake peer's release. Its
callback injects exactly the atomic-close/mailbox-close/owned teardown arrival
used by reentrant WndProc; it does **not** claim a real DestroyWindow call during
that callback. The additional UIA assertions extend the actual native RichEdit
fixture, using genuine COM objects and actual SAFEARRAY calls.

Review-owned captures and logs are outside the repository at
`W:\devin_folder\rust-ui-freeze-checks`, including `details-screen.png`,
`details-pw.png`, `ime-screen.png`, `details.json`, `drain-edge.log` and
`provider-extra.log`. Owned composer processes were gracefully closed.

The complete **17-scenario matrix** and **30.5-second idle run** remain
Agent Worker recorded evidence, not independently replayed in full. Current
files record `idle_cpu_pct_30s=0`, **32.4 MiB** working set, **17 threads**,
`uptime_ms=30545`, `requested_redraws=83`, `caret_redraws=78`, and
`native_timer_fires=0`. README's approximately 32 MiB account is consistent.
These show one idle measurement and clean counter emission, not leak freedom.

## Closure matrix

| Item | Status | Closure / smallest remaining invariant |
| --- | --- | --- |
| F01 | PARTIALLY_CLOSED | Merge/drain arrivals made during dispatch before deciding cleanup or continuation; guarantee progress after a failed mailbox wake without needing another enqueue; preserve an explicitly ownerless composition pin. |
| F04 | CLOSED | Measurement-first and same-scale bounds changes freeze the whole effective geometry during composition. |
| F05 | CLOSED | Queued final edits are acknowledged before reconciliation; both native notification branches preserve text/revision agreement, conflict stale proposals and accept the next edit. |
| F07 | CLOSED | Nonempty provider-array ABI path is exercised; independently extended native assertions verify retained references across all three lifecycle boundaries. |
| F08 | CLOSED | Applied mounted-peer palette compares actual effective foreground/selection output, including equal-descriptor system palette drift. |
| F09 | CLOSED | Only successful paint replaces committed ink; failed paint preserves prior ink and damage; focused hover/move/remove proof is meaningful. |
| F11 | PARTIALLY_CLOSED | Check target layout activation and requested conversion/open mode; fail explicitly when engagement cannot be verified instead of retaining the blind-toggle route. |
| F12 | PARTIALLY_CLOSED | Replace or accurately withdraw the occluded published physical-screen proof; the real layout overlap is fixed and independently verified. |
| F13 | PARTIALLY_CLOSED | Update current API candidate and composition preservation narrative; make capture/harness ownership claims match the actual evidence and scripts. |
| N03 | CLOSED | Direct-interface SAFEARRAY insertion/ownership remains correct; nonempty provider/range paths and exact stale-reference HRESULTs independently pass. |

F02, F03, F06, F10, N01, N02 and N04 are not reopened. No concrete regression
was found requiring their reopening. No new architecture findings are added.

## Remaining findings

### F01 — BLOCKER: arrivals during drain still escape mandatory cleanup

The original **138 ordinary messages followed by destruction** trace is
repaired. `next_drain_item` sweeps the backlog independently of the ordinary
budget; shared-policy tests assert exactly 128 ordinary dispatches, disposal
of the remaining ten and survival of an earlier delivery error. The real
Backend regression verifies actual mounted-peer release and queue disposal.
Painted-key replay now uses captured ownership, including captured None.
Native IME delivery now uses the pinned peer when the pin contains a node.

However, [Backend::drain_reentrant](https://github.com/rceman/rust-ui/blob/b99d55893c8b6a3b0b585645bb34f31dc6cc747f/src/platform/win32/mod.rs#L2317)
moves the queue into a local variable. Dispatch-time arrivals enter the live
cell. After the local drain returns, those arrivals are appended, but the
cleanup/continuation decision still uses **out.remainder computed before that
append**. Mandatory teardown in the live cell is not scanned in that pass.

Independently reproduced result:

```text
initial queue: one replayed painted Enter
update callback: reentrant-style close + enqueue WM_NCDESTROY
one Backend drain: Ok(())
closed=true
remaining=1
mounted_peer_releases=0
```

No fatal error forces bail in this trace. Closed suppresses the pump wake;
cleanup/PostQuitMessage can remain stranded. The same stale remainder decision
can omit a continuation for ordinary arrivals generated while dispatching.
Compute cleanup/progress from the **complete queue**, including new arrivals,
while preserving the ordinary budget and closed-window disposal contract.

Two progress/ownership edges also remain:

- [Mailbox wake failure](https://github.com/rceman/rust-ui/blob/b99d55893c8b6a3b0b585645bb34f31dc6cc747f/src/tasks.rs#L254)
  now latches an error and releases the coalesced edge, which is an improvement.
  But only the next enqueue retries; only a later Backend turn reads the error.
  A single failed post does not itself guarantee either event. The blocking
  GetMessage loop can eventually sleep with queued work and an unread latch.
  Give the existing wake/error path a guaranteed progress outcome without a
  second producer or incidental UI input. This is source/failure-path analysis,
  not a claim that a native PostMessage fault was injected in the acceptance run.
- `ime_owner: Option<NodeId>` still conflates inactive composition with an
  explicitly ownerless START. `ime_target` and `ime_end` fall back to current
  focus when it is None. An ownerless captured start can therefore acquire a
  different peer for subsequent composition traffic. Preserve the active/no-owner
  distinction through native delivery and end bookkeeping, just as ordinary
  replay already does. The existing pin test covers Some(A), not this edge.

These are remaining F01 ownership/progress defects, not reasons to redesign
the accepted platform contracts.

### F11 — MAJOR: target-thread IME engagement is not fully verified

Full JSON escaping is now applied to dynamic receipt strings in value, UIA
names/tree, actions and operational errors. Backslash-q independently
round-trips through the live provider/probe. Human CLI usage diagnostics are
separate from JSON data receipts.

The new harness discovers/requests a Japanese HKL and sets conversion mode on
the target default IME window. But [the activation post](https://github.com/rceman/rust-ui/blob/4e1aa6bb504c2ebc5aca74f04e7c73a37cfbae65/examples/native_probe.rs#L683)
discards PostMessageW's result and substitutes a sleep for read-back of the
**target thread's actual keyboard layout**. The mode check accepts any result
with NATIVE set; it does not establish the requested hiragana/open state.
If no default IME window is available, the code still uses the **blind
VK_DBE_HIRAGANA toggle** that the correction was intended to eliminate.

The nonempty newly gained kana, mandatory unchanged submit counter and real
application TextValue agreement prevent the old acceptance false-pass. They
do not prove deterministic input-mode engagement independent of persisted
TIP state. Check the request, target layout and required mode/open state, with
bounded engagement and an explicit failure when verification is unavailable.
No new keyboard framework is needed.

### F12 — MAJOR evidence correction: overlap fixed, published screen obscured

Composer now stacks the three details labels inside a public Column within
Surface. Published UIA rectangles are distinct and ordered. Fresh native
physical/UIA verification independently confirms the fix:

```text
turn        [257,443,466,466]
draft-echo  [257,471,431,495]
draft-edits [257,500,402,523]
```

Full caption and draft-prefill are visible on the foreground physical screen;
the labels do not overlap. The owner's text-over-text concern is resolved in
this tested details state. This is not a blanket acceptance of every future
text/layout combination.

README correctly downgrades PrintWindow to a window/state receipt and explicitly
excludes native text fidelity. That classification closes the former capture
misinterpretation. Yet current **details-screen.png** is obscured below the
editor by another application/desktop view and a large black region. The three
claimed details labels are not visible in it. The paired PrintWindow image
shows the labels but cannot substitute for the declared physical-display proof.
Replace the obscured physical artifact with an unobscured foreground capture,
or explicitly mark it unusable and remove the associated physical-fidelity
claim. Do not change the already repaired consumer layout again.

### F13 — MINOR canonical truth, with the F12 evidence qualification

No canonical document under `docs/` changed between the prior review and this
publication. Two previously identified current-state statements remain:

- `SPIKE_API_REVIEW.md` still says **Candidate 0405a80** without a historical label.
- `SPIKE_ARCHITECTURE_DEVIATIONS.md` section 3 still describes composition
  surviving deactivate/reactivate because COM identity is retained. Current
  production instead defers geometry during composition, acknowledges queued
  edits, reconciles remaining divergence, resolves proposals and then relatches.

Correct those present-state accounts while preserving historical chronology.
Receipt identities, current test counts and approximately stated performance
values are otherwise consistent when the Linux publication gating is qualified.

**NOTE:** `collect.ps1::S-Details` itself compares UIA rectangle bottoms/tops and
asserts the value round-trip. These are visible representative consumer checks;
README's blanket assertion that PowerShell contains no UIA interpretation or
assertion logic is too broad. Accurately scope that account, or place the
reusable rectangle assertion in the Rust harness under the existing documented
boundary. This does not call for another platform abstraction or Gate 21.

## Closed corrections: implementation evidence

- **F04/F05:** natural_size now gates on composing before any pending bounds
  request exists. apply_bounds parks same-scale moves as well as scale changes.
  Draw, pointer/caret mapping and host callbacks consume the effective host
  snapshot. The real-order measurement-first regression passes. ime_end now
  services events, pumps queued edits, compares against the updated mirror,
  pumps reconciliation, resolves proposals and finishes relatch. The real
  msftedit/runtime regression covers survived and suppressed notifications,
  native/shared revision agreement, stale-proposal conflict and the next edit.
- **F07/N03:** the new mandatory array fixture constructs a two-element
  SAFEARRAY from genuine native providers, extracts a retained provider and
  destroys the array. pub(crate) fence_provider_array is a narrow crate-private
  test seam. ABI insertion uses interface pointer values; array AddRefs and
  local wrappers drop normally. Guard disarm transfers arrays, not compensating
  interface leaks. The independent extra assertions cover pattern, both ranges,
  enclosing provider and array provider with exact unavailable HRESULTs at
  removal/recreation/close, filling the committed fixture's shorter assertion
  coverage without editing it. No escaped-reference regression was found.
- **F08:** turn_body compares each mounted peer's resolved foreground,
  selection background and selection foreground. Descriptor-equal palette
  changes are detected. The focused test actually checks applied native colors
  under forced colors and a same-descriptor resolver change. Creation,
  apply_foreground and refresh consume the same installed system resolver.
  Shared visual_dirty remains the interaction classification authority.
- **F09:** relayout only accumulates old/proposed damage. paint_inner commits
  ink and consumes damage only after Renderer::draw succeeds; EndDraw failure
  propagates, with bounded target-loss retry. The test checks hover shadow,
  failure, move and removal against prior successful footprints. This proves
  retained bookkeeping without relying on full-frame clear. Offset/sign/cache
  and representability protections remain intact. No ancestor-clip feature is
  invented to expand this narrow closure surface.

## Gates 1–20, reassessed only for this closure surface

The canonical document still defines exactly Gates 1 through 20.

| Gate | Status | Reason |
| --- | --- | --- |
| 1 — Completeness | FAIL | Mandatory ownership/progress and current evidence corrections remain. |
| 2 — Assumptions/approval boundaries | PASS | Architecture retained; measured and unavailable acceptance distinguished. |
| 3 — Contract preservation | FAIL | Drain-time arrivals and failed wake lack universal cleanup/progress. |
| 4 — Scope | PASS | Review and required corrections remain in the named finding families. |
| 5 — Minimal design | PASS | Existing queue/wake/harness owners suffice. |
| 6 — Ownership/authority | FAIL | Local/live queue split and ownerless IME pin remain incomplete. |
| 7 — Reuse/semantic duplication | PASS | Geometry, shared visual classification, palette and paint authorities retained. Consumer assertion location is qualified under F13. |
| 8 — Unrequested fallback | FAIL | IME engagement retains the blind-toggle fallback instead of verified engagement or failure. |
| 9 — Deterministic contracts | FAIL | Drain progress and input-mode engagement still have unchecked branches. |
| 10 — Canonical truth | FAIL | Current API candidate and composition account remain stale. |
| 11 — Failure safety | FAIL | Teardown can remain queued; failed mailbox wake requires an incidental future event. |
| 12 — Boundedness/lifetime | FAIL | Budgets are bounded, but mandatory native resource cleanup can remain stranded. |
| 13 — Dependencies/blast radius | PASS | No unrelated dependency or platform expansion. |
| 14 — Performance/redundant work | PASS | No new idle loop or unrelated optimization requirement; recorded smoke qualified. |
| 15 — Infrastructure boundary | PASS | Reference-app observability remains outside public library semantics. |
| 16 — Verification sufficiency | FAIL | Drain-time arrival counterexample fails; target layout/mode read-back remains incomplete. |
| 17 — Proportional recovery | PASS | Bounded corrections remain feasible without redesign. |
| 18 — Identity/immutability | PASS | Hashes match; exact production/harness/publication distinction and test-gating exception verified; previous reviews unchanged. |
| 19 — Artifact honesty | FAIL | Obscured physical proof, stale current docs and overbroad harness account. |
| 20 — Systemic closure | FAIL | Original passing trace does not cover arrivals produced during the drain itself. |

## Smallest correction set before freeze

1. Finish F01 for the complete local/live queue, including teardown arriving
   during dispatch, ordinary-arrival continuation, earlier errors and closed
   remainder disposal. Preserve explicit ownerless composition. Guarantee the
   mailbox failure outcome without a second enqueue. Add focused proof of these
   exact traces; retain the repaired 138-message test.
2. Verify target-thread Japanese layout activation and the required IME mode/open
   state. Remove blind mode-toggle acceptance when verification is unavailable.
3. Replace or withdraw the obscured physical capture; correct current API and
   composition narratives and precisely describe consumer assertion ownership.

No additional broad architecture review or full matrix replay is required merely
for formality. Changed owners need focused proof, then correctly bound current
receipts. Already closed findings need not be reworked.

Dead keys, physical monitor DPI transition, the combined real composition/monitor
transition and foreground ElementProviderFromPoint remain environmental NOT RUN
items. None is a blocker merely for being unavailable. Rejection here rests on
concrete queue/progress defects and incomplete current-state proof/claims.

The Windows foundation is not ready to freeze. The post-foundation Component
Gallery is not cleared. The accepted architecture direction remains sound;
ARCHITECTURE_BLOCKED is unwarranted.

SHADCN_COMPONENT_GALLERY_NOT_CLEARED

REWORK_REQUIRED
