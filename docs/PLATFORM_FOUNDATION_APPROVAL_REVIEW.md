# Independent narrow Windows foundation approval review

**RUN_KEY:** `RUI-A-261002-1704`

**Role:** Astra Advisor

**Started:** 2026-10-02 17:09:06 UTC+03:00

**Verdict:** `REWORK_REQUIRED`

This checks closure of the findings in `PLATFORM_FOUNDATION_FINAL_REVIEW.md`.
The platform-contract architecture remains accepted. This is not a redesign
or a Component Gallery review. The remaining defects can be corrected in the
existing owners, but the production candidate is not ready to freeze.

## Immutable identities and publication scope

| Identity | SHA |
| --- | --- |
| Authoritative previous independent review / BASED_ON | `dd6ebdc536e2837072a236d1343376f1b3e1c791` |
| Last production/library semantic change | `11c308ebce19bae5499bfbd8b1b1a0bcddbad0ca` |
| Harness/example/docs source snapshot | `3eaadfbf03d81ad017cff72d40cbc6647b4e677e` |
| Evidence publication reviewed | `08f45cf8a44a5b3569ac6610bad03c11a14fc5eb` |

**The identity chain is valid.** Actual Git diffs show no `src/`, Cargo manifest,
lockfile or other library-semantic changes after `11c308e`. The changes through
`3eaadfb` affect the native probe's bounded UIA invoke retry, composer fake-effect
timing, platform documentation and collection scripts/README. Those alter the
reference consumer and verification, not the production library contract.
`3eaadfb` to `08f45cf` changes only files in the evidence directory, including
receipts, screenshots and measured text outputs. It is evidence publication.

Current `identity.txt` records:

```text
HEAD=3eaadfbf03d81ad017cff72d40cbc6647b4e677e
exe_sha256=50FD020FEFC5AE75BC2F33D9B115AFF2EBF75EBBAFE25439AFA9213853CFD07B
probe_sha256=0364A2CE58E0EAC2D205C2D0A3162C1DDF3948A267BDCE8728F84CAE6FD95B6E
run_utc=2026-10-02T12:37:34.9867962Z
```

Native Windows `Get-FileHash` independently matched both executable hashes.
The receipt correctly binds the binaries to the harness snapshot over the
frozen library implementation. Source identity is not a certification of
every assertion or screenshot in the bundle.

The authoritative Windows checkout was clean on the requested branch at
`08f45cf`. Only this new document is committed by the review. Production,
harness, examples, evidence and all three earlier reviews remain unchanged.
No Component Gallery work or main merge was performed. The pushed review
commit is identified separately by Git history and the final execution footer.

## Independent verification and environment

Native verification/publication used `W:\devin_folder\rust-ui`, Windows 11
Home, Rust 1.94.0. Linux verification used a detached checkout of `08f45cf`,
WSL2 Linux 6.18.40.1-microsoft-standard-WSL2, Rust 1.93.1.

| Executed independently | Result / scope |
| --- | --- |
| Native Windows `cargo test --locked --lib` | PASS **89/89**. |
| Linux `cargo test --locked --lib` | PASS **77/77**. |
| Native Windows `cargo fmt --check` | PASS. |
| Native Windows `cargo check --locked --all-targets` | PASS, with warnings. |
| Exact current geometry module, external scratch executable | PASS for signed limits, adjacent f32 values, rejected `2^31`, checked origin construction and the fractional-origin counterexample. |
| Verbatim production drain-selection helper, isolated scratch fixture | **Counterexample reproduced:** 138 ordinary items followed by destruction consumes 128, leaves 11, and does not select destruction. This isolates the production queue policy; it does not claim execution of full Backend shutdown. |
| Verbatim shared native-edit acceptance method, isolated scratch fixture | Validates the source-derived duplicate-reconciliation trace: shared revision 2/native revision 3; the following native edit is rejected. This is not a real RichEdit notification-timing replay. |
| Fresh native composer/probe smoke | PMv2 asserted; actual DPI 120, scale 1.25, window 980×900 physical px. Owned processes closed after checks. |
| Focused real-IME command, with details expanded | Command succeeds with required counter, newly gained kana and application/native-value agreement. A subsequent typed `Z` is observed by the app echo in this run. This does not exercise the alternative final-notification trace under F05. |
| Focused native JSON check | Typed `\q`; `native_probe value` emits invalid JSON. Strict parser rejects the unescaped backslash. |
| Current visual evidence and fresh captures | Inspected all four requested PNGs. Fresh PrintWindow captures still crop text. A separate physical-screen crop confirms three details labels overlap. |
| Evidence receipts | Identity/IME/geometry/performance text decodes as UTF-8; checked committed JSON receipts parse. |

The complete **16-scenario matrix** and **30.5-second idle run** are recorded
Agent Worker evidence, not independently replayed in full. The focused checks
above were run because the remaining findings depend on them. Scratch checks
and captures were written outside the repository; no tests or source were
added to the production checkout.

## Closure matrix

| Item | Status | Smallest remaining correction |
| --- | --- | --- |
| F01 | PARTIALLY_CLOSED | Deliver destruction behind any budget-exhausted ordinary remainder, preserve replay ownership through painted keys and pinned IME delivery, and check the remaining mailbox wake path. |
| F03 | CLOSED | Exact float/integer bounds and checked PeerOrigin construction are corrected. |
| F04 | PARTIALLY_CLOSED | Preserve effective width during measurement before pending relatch is installed; use pinned IME ownership consistently. |
| F05 | PARTIALLY_CLOSED | Avoid issuing reconciliation against a mirror whose final native edit is still queued, which can split native/shared revisions. |
| F06 | CLOSED | Framework SetTimer failure now propagates through the existing error path. |
| F07 | PARTIALLY_CLOSED | Add nonvacuous nonempty native provider/child-array retention and lifecycle proof. |
| F08 | PARTIALLY_CLOSED | Remove appearance-cache pre-seeding and refresh effective system colors when palette output changes. |
| F09 | PARTIALLY_CLOSED | Keep committed_ink exclusively at successful paint boundaries and exercise real retained-damage transitions. |
| F11 | PARTIALLY_CLOSED | Apply complete JSON escaping to all emitters, including value and names, not only IME output. |
| F12 | PARTIALLY_CLOSED | Fix the details-label overlap and publish reliable, accurately classified current visual/performance evidence. |
| F13 | PARTIALLY_CLOSED | Correct current API identity, composition-preservation narrative and evidence identity/performance account. |
| N03 | PARTIALLY_CLOSED | ABI/reference fixes are correct; the nonempty native provider-array path remains unexercised by the fixture. |

F02, F10, N01, N02 and N04 remain closed. No concrete regression requiring
their reopening was found.

## Closure findings

### F01 — BLOCKER: destruction still stops behind ordinary remainder

[Queue authority](https://github.com/rceman/rust-ui/blob/11c308ebce19bae5499bfbd8b1b1a0bcddbad0ca/src/platform/win32/window.rs#L65)
selects a teardown item without budget only when it is already at the queue
front. It does not find teardown behind an unprocessed ordinary item.

The new regression appends destruction after exactly 128 ordinary items. That
case works: after consuming 128, teardown is at the front. The requested
greater-than-budget case does not:

```text
queue: 138 ordinary items, then WM_NCDESTROY
ordinary_consumed=128
remaining=11
destruction_delivered=false
```

This independently exercises the verbatim `next_drain_item` implementation.
In production, reentrant destruction has already set `closed=true` and detached
the WndProc route. `wake_pump` then declines to post because the window is
closed. Full shutdown/PostQuitMessage stays queued; there need be no preceding
delivery error to invoke `bail`. The original blocker remains.

Select/complete mandatory destruction independently of ordinary budget, then
explicitly dispose of the closed window's ordinary remainder. Test actual
cleanup execution and remainder disposition, not merely a `tore=true` variable
after selecting a marker. Include 128 ordinary items plus further ordinary
items before destruction, and the earlier-error case.

Other ownership/progress corrections are real but incomplete:

- `Option<Option<NodeId>>` correctly distinguishes captured None from live
  focus. Focus/wheel/keyboard/character classes now capture arrival ownership.
  However, the painted-key branch in `key_msg` still uses `self.focus`, bypassing
  `focused_target`. A captured-ownerless Enter can press the newly focused
  button; a captured painted owner A can be redirected to B.
- `ime_owner` pins composition bookkeeping, but native WM_IME_COMPOSITION/
  END delivery still uses `send_focused`, which does not consume `ime_owner`.
  After a focus change, the native message and composition-ending bookkeeping
  can affect different peers. Pin the native delivery as well as the flags.
- Backend deferred continuations now use a checked wake and roll back
  `pump_queued` on failure. The cross-thread mailbox wake installed in `run`
  still discards `PostMessageW` failure. It needs an explicit failure/progress
  path; the claim that every deferred-work wake is checked is not yet true.

The error-recording post in `mark_fatal` is separately followed by the run
loop's safe-point fatal check; it should not be confused with proof of the
mailbox callback's failure behavior.

### F03 — CLOSED: representability is now exact

[ScaleFactor and PeerOrigin](https://github.com/rceman/rust-ui/blob/11c308ebce19bae5499bfbd8b1b1a0bcddbad0ca/src/geom.rs#L161)
reject the positive upper bound exclusively and accept the negative bound
inclusively. Independent execution confirms:

| Input at scale 1 | Result |
| --- | --- |
| `2147483648.0`, also `i32::MAX as f32` | None |
| Adjacent f32 below `2^31`: 2147483520 | Some(2147483520) |
| Adjacent f32 above `2^31` | None |
| `i32::MIN as f32` | Some(i32::MIN) |
| Adjacent f32 below the negative limit | None |
| Adjacent f32 toward zero from the negative limit | Representable success |

PeerOrigin construction now returns Option and uses checked conversion.
Callers handle failure; integer add/sub and signed-16 LPARAM packing remain
checked. The prior fractional-origin example still yields local zero. This
closes F03 without changing the accepted platform-neutral scale meaning.

### F04 — MAJOR: effective snapshot is shared after parking, but measurement precedes parking

[Effective peer geometry](https://github.com/rceman/rust-ui/blob/11c308ebce19bae5499bfbd8b1b1a0bcddbad0ca/src/platform/win32/text.rs#L1104)
is now read by draw, pointer/caret transforms and native host callbacks. The
native test proves that once pending relatch exists, a measurement at a new
requested width does not mutate the old effective width. Applying the pending
geometry afterward is checked. These correct important earlier paths.

Actual layout orders operations differently: `LayoutCache::run` first calls
`natural(...)`, which calls peer `natural_size(new_width)`, and only later
walks nodes and calls `apply_bounds(new_bounds, new_scale)`. `natural_size`
freezes width only if `pending_relatch` is already Some. On the first scale
layout during active composition that option is still None, so new width is
written into old host geometry before deferral is established.

Freeze the composing peer's effective measurement geometry before a new-scale
layout measures it, or otherwise gate that write on the composition/transition
state. Test the production order: composition active, measure new width first,
then request new bounds/scale. The current regression reverses those last two
operations. Pinned native IME delivery also needs the F01 correction.

This is a deterministic state-machine issue. A real physical monitor transition
during live IME is not required merely for formality.

### F05 — MAJOR: acknowledgement order improves, but reconciliation can double-emit

[IME end/reconciliation](https://github.com/rceman/rust-ui/blob/11c308ebce19bae5499bfbd8b1b1a0bcddbad0ca/src/platform/win32/mod.rs#L2250)
now clears composing, drains notifications, compares actual peer text with the
shared committed mirror, pumps edits, resolves proposals and finally relatches.
This repairs a final notification previously suppressed while composing.
The shared regression proves an explicitly enqueued final edit wins over a
stale-base proposal when acknowledged first. The fresh real-IME command also
observed application/native agreement on its executed path.

The alternative final-notification path still splits revisions:

| Step | Shared mirror | Native peer |
| --- | --- | --- |
| Before final drain | R | R |
| `native_commit` queues final E1(base R, result N1) | R, E1 not pumped | N1 recorded |
| `reconcile_commit` sees old mirror and queues E2(base R, result N2) | R, two queued edits | N2 recorded |
| Pump accepts E1, rejects E2's stale base R | N1 | N2 |
| Next genuine native edit uses base N2 | Rejected against N1 | Native text continues changing |

The isolated verbatim `TextPeerSync::accept_native` check confirms that final
rejection. This is a source-derived reachable notification trace, not a claim
that the fresh real-IME run reproduced that timing.

Acknowledge already queued final native edits before deciding reconciliation,
then reconcile only actual remaining divergence, acknowledge that result,
resolve proposals and relatch. Assert both endings: final change suppressed
while composing, and final change queued after clearing. Require native/shared
revision agreement and acceptance of the next edit, as well as TextValue and
stale-proposal outcomes. The current fake-peer test never invokes reconciliation.

### F06 — CLOSED: deadline failure is propagated

`rearm_deadline` now returns UiResult, reports SetTimer failure and records only
the native armed state. `turn_body` propagates the error; the run loop's existing
fatal/teardown path handles it. Scheduled work is no longer silently left to
wait for an unrelated turn. No new timer framework was introduced. The already
closed native timer rearm/tombstone behavior remains unchanged.

### F07 / N03 — MAJOR proof gap: ABI and reference ownership fixed; child array remains optional

[Both native array wrappers](https://github.com/rceman/rust-ui/blob/11c308ebce19bae5499bfbd8b1b1a0bcddbad0ca/src/platform/win32/uia.rs#L594)
now pass the interface pointer itself to SafeArrayPutElement(VT_UNKNOWN).
Local wrappers drop normally after the array obtains its independent AddRef.
Input/output guards clean fallible exits; disarming transfers the output array
without prematurely destroying it. The provider `mem::forget` leak is removed.
These match the [native API ownership contract](https://learn.microsoft.com/en-us/windows/win32/api/oleauto/nf-oleauto-safearrayputelement).
The earlier access-violation call shape is corrected in production.

Stale fenced range arguments are now checked before native unwrapping. The
independently executed Windows fixture forces a nonempty GetSelection array,
extracts a real native range, destroys the array and verifies retained range
invalidation. This exercises the corrected range-array ABI path successfully.

Provider/child-array proof is still conditional: `GetChildren` may be null or
empty; extracting a child and checking it occurs only inside optional branches.
No assertion requires a child or any nonempty provider array. The test also
does not assert every retained pattern/range/enclosing/child across all of
removal, recreation and live close. The README's blanket child-lifetime claim
therefore remains unsupported.

Add a nonvacuous fixture exercising the actual nonempty provider-array helper
with real native provider references, retain an extracted child and assert
removal/recreation/close invalidation. Require the relevant preconditions. This
is a bounded verification correction; no new production ABI defect or leak in
the repaired insertion code is alleged.

### F08 — MAJOR: classifier repaired; live palette refresh still bypassed

Win32 interaction transitions now call shared `node::visual_dirty`. Text
size/weight correctly yield layout plus paint; effective equality yields no
work. Foreground at creation and authored-foreground updates now use system
slots under forced colors. Native `set_colors` can resolve the same slots.

The live path still prevents that refresh: `os_change` writes the new
`(appearance, theme)` into `peer_ctx.colors` before calling `turn`. The new
whole-pair comparison in `turn_body` then compares equal and skips mounted peer
colors on an appearance-only change with unchanged Theme. This is the earlier
cache-ordering bug, still present.

Additionally, an effective GetSysColor palette can change while all Appearance
flags and Theme are unchanged. Comparing only those descriptors cannot detect
changed concrete colors. Refresh/compare effective palette outputs on the OS
notification, without pre-seeding the old applied palette. Exercise forced-color
flip and same-descriptor palette change. Keep the shared classifier fix.

### F09 — MAJOR remaining paint-state contract; offset arithmetic repaired

Shadow offsets are now checked finite and limited before casts/additions. With
both offsets and raster coordinates bounded by SHADOW_MAX_DIM, additions fit
i32. Finite unrepresentable offsets fail before arithmetic. The sigma guards,
bounded cache, mask ownership and already closed sign/cache-key fixes remain.

`paint` now computes live interaction-resolved ink and installs it after
successful drawing, which repairs paint-only/state-shadow commits. However,
`relayout` still independently assigns `*committed_ink = new_ink` before any
successful paint. It still replaces/prunes the claimed last-painted snapshot
at layout time. Adding a second paint assignment does not remove that mutation.

Keep the committed snapshot exclusive to successful paint; relayout may union
proposed damage against it without replacing it. Add focused assertions for
paint-only and hover/pressed/focus shadow changes, movement/removal and failed
paint. There are no direct committed_ink/damage regressions in the current test
file. No new ancestor-clipping feature is required: validate clip changes only
where this renderer actually supports them. Full-frame clear is not the proof.

### F11 — MAJOR: mandatory signals repaired; complete escaping is not applied everywhere

The counter is mandatory before/after; details are explicitly expanded. Newly
gained suffix must be nonempty and contain kana, SendInput counts in the IME
operation are checked, and `draft-echo` comes from the app's `draft.text()`.
It is genuine shared TextValue observation, not another native read. The
focused native command independently passed these assertions for draft.
Preedit suppression remains a production composing-state gate; the native
driver does not separately sample app state during each preedit step.

`json_str` correctly handles quotes, backslashes, newline, CR, tab, control
characters and Unicode, but only IME output uses it. Value, UIA names, named
actions and other outputs still use partial escaping/raw interpolation.
The independently reproduced ASCII example is:

```text
native value: draft-prefill\q
emitted: {"kind":"value","evidence":"acceptance","name":"draft","value":"draft-prefill\q"}
strict parser: Invalid \escape
```

Use the complete serializer for every emitted string. This is a harness fix,
not a public rust-ui API change. The real IME acceptance is currently tied to
the draft application's committed echo; do not infer equivalent app-state
observation for every other editor.

### F12 — MAJOR: settle does not resolve capture; physical-screen overlap confirmed

Requested current images were inspected directly:

| Artifact | Observed |
| --- | --- |
| composer-dark.png | Caption and full draft-prefill visible. |
| final-foundation.png | Caption/full draft visible; now regenerated by a foundation scenario. |
| typing.png | Caption absent; draft prefix still cropped to efillhello world. |
| ime.png | Caption absent; prefix cropped; three details labels overlap. |

Fresh review-owned PrintWindow captures using the current 350 ms settle still
show cropped `efill` and absent caption. A separate PMv2 physical-screen crop
shows the caption and full draft text correctly, but independently confirms
the details-label overlap. Thus neither issue is closed by claiming a timing
settle: PrintWindow can disagree with the physical display, and the label
overlap exists on the physical display itself.

The same three UIA labels occupy exactly the same screen rectangle in the
fresh capture:

```text
turn 0 | busy false | notice -       [311,497,567,520]
draft-echo draft-prefill\q          [311,497,567,520]
draft-edits 2 last ...              [311,497,567,520]
```

Composer places the three labels directly inside Surface, whose children
overlay. Arrange those reference-consumer labels vertically using existing
public layout primitives. This is a bounded example correction, not a change
to the framework's accepted Surface semantics.

Reviewer-owned screenshot: `W:\devin_folder\rust-ui-approval-checks\details-screen.png`.
It was captured in response to the owner's request and is outside the commit.
The already published `ime.png` also visibly supports the overlap finding.

PrintWindow is currently insufficient as sole text-fidelity proof. Document its
observed limitation and use truthful current physical-display evidence where
needed, or establish a reliable capture boundary. Do not claim the cropped
captures are physically displayed text defects without the separate check.

Resource receipt values are fresh and native_timer_fires is zero. But README
still reports the previous 32.3 MiB/17 threads/30544 ms, whereas current files
record **32.1 MiB/16 threads/30542 ms**. Counters show graceful emission, not
leak freedom; that honest distinction is retained. The full idle measurement
was inspected, not independently repeated.

### F13 — MINOR current documentation, plus F12 proof honesty

The platform inventory now correctly names `native_probe uia`. Historical
bitmap and one-time-activation accounts need not be rewritten.

Current staleness remains:

- `SPIKE_API_REVIEW.md` still identifies **0405a80** as its current candidate.
- `SPIKE_ARCHITECTURE_DEVIATIONS.md` describes composition surviving scale
  deactivate/reactivate because COM identity is retained, omitting actual
  composition deferral, reconciliation and acknowledgement ordering.
- README identifies production 11c308e and receipt source 3eaadfb indirectly,
  but should explicitly distinguish those from publication 08f45cf.
- Current performance values, capture limitation and blanket provider/queue
  proof claims need the corrections described above.

Keep clearly historical chronology and all prior independent reviews
immutable. Correct present-state identity and demonstrated behavior only.

## Environmental limits

Dead keys, physical monitor DPI transition and foreground
ElementProviderFromPoint acceptance remain NOT RUN. The combined physical
IME/monitor transition is not claimed. None is a rejection merely for an
unavailable environment. Related deterministic/native proof can bound those
cases once the actual state-machine and ownership defects above are corrected.
Recorded or inferred code-path coverage is not relabeled measured coverage.

## Gates 1–20, limited to the remaining closure surface

The canonical document still contains exactly Gates 1 through 20.

| Gate | Status | Closure reason |
| --- | --- | --- |
| 1 — Completeness | FAIL | Material closure findings remain. |
| 2 — Assumptions/approval boundaries | PASS | Accepted architecture retained; environmental limits explicit. |
| 3 — Contract preservation | FAIL | Teardown, replay ownership, final revision agreement and committed paint snapshot remain incomplete. |
| 4 — Scope | PASS | Review and corrections remain within the requested finding families. |
| 5 — Minimal design | PASS | Existing owners can supply the bounded corrections. |
| 6 — Authority/ownership | FAIL | Pinned IME delivery and native/shared revision acknowledgement still diverge. |
| 7 — Semantic duplication/reuse | PASS | Shared visual classification and checked geometry authorities are reused; no replacement platform policy requested. Ordering defects are assessed under 6/9. |
| 8 — Unrequested fallback | PASS | No new framework/editor/platform fallback introduced. |
| 9 — Stable deterministic contracts | FAIL | Ownership/progress, effective geometry and final-commit branches are not universal. |
| 10 — Canonical authority/status | FAIL | API identity and current-state preservation accounts remain stale. |
| 11 — Failure safety | FAIL | Mandatory cleanup can remain queued; unchecked mailbox wake and premature paint snapshot remain. SAFEARRAY ABI failure is repaired. |
| 12 — Boundedness/lifetime | FAIL | Queue/cache arithmetic bounds improve, but closed-window mandatory resource cleanup can remain stranded. |
| 13 — Dependencies/blast radius | PASS | No unrelated production expansion found. |
| 14 — Redundant work/performance | PASS | No new permanent framework loop or repeated-timer regression found; smoke is not a universal performance guarantee. |
| 15 — Infrastructure boundary | PASS | Reference observability remains outside the public library. |
| 16 — Verification/proof reuse | FAIL | Teardown test misses the reproducing trace, child-array proof is optional, reconciliation branch and damage transitions lack focused proof. |
| 17 — Proportional recovery | PASS | Narrow corrections are feasible; redesign is unwarranted. |
| 18 — Immutability/identity | PASS | Exact production/harness/publication chain, matching hashes, immutable earlier reviews. |
| 19 — Artifact honesty | FAIL | Capture contradiction, overlap, stale resource values and overbroad proof labels. |
| 20 — Systemic closure | FAIL | Selected passing cases do not close the complete requested owner/progress/acknowledgement paths. |

## Bounded correction set

1. Finish teardown across an intervening ordinary remainder; use the captured
   owner for painted keys and pinned owner for native IME; give mailbox wake
   failure an explicit progress/error path. Execute the full cleanup regression.
2. Freeze effective composing geometry before new-width measurement; acknowledge
   queued final edits before reconciliation; prove both final-notification
   branches, stale-proposal conflict and acceptance of the following edit.
3. Execute actual nonempty native provider/child-array lifecycle assertions.
   Preserve the corrected ABI, references, range arguments and array guards.
4. Refresh live effective system palettes without pre-seeding the applied cache;
   keep committed ink exclusive to successful paint and assert damage transitions.
5. Apply full JSON serialization to all harness string outputs, arrange details
   labels with existing layout primitives, and publish honest current captures,
   resource values and canonical identities/semantics.

Run focused affected proof, then publish receipts tied to the resulting frozen
library and harness snapshots. Unaffected architecture or full matrix proof
does not need to be redesigned or replayed merely for formality.

The Windows foundation is not ready for freeze or merge toward main. The
post-foundation milestone remains uncleared. `ARCHITECTURE_BLOCKED` is not
justified: the architecture direction is still sound.

SHADCN_COMPONENT_GALLERY_NOT_CLEARED

REWORK_REQUIRED
