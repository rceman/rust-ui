# Proposed Engineering Workflow v2

**Status: PROPOSAL — requires rust-ui Planner authorization.** This document changes no current policy, acceptance requirement, runtime behavior, or review authority. Universal Gates 1–20 remain the sole gate taxonomy.

**Source:** [Astra systems and workflow audit](ASTRA_SYSTEM_WORKFLOW_AUDIT_20261031.md), based on `cb6d888207321a2b724e68c302175661245ee00e`. The proposal applies to incremental component work as well as P12. It does not approve P12 or future implementation changes.

## Operating rule

Implement a coherent correction, prove the affected invariant, capture each necessary state once, and reuse its paired image/snapshot for offline comparisons. Freeze the implementation before broad acceptance. Independent reviewers inspect that same candidate and evidence revision.

Keep native Win32/D2D/DWrite/RichEdit architecture, the frozen reference, desktop responsiveness, generational identity, typed style authority, and event-driven idle behavior unchanged. The real `rust-ui-gallery.exe` remains the visual acceptance application. Hidden/offscreen subsystem probes remain valid for native contracts; they do not substitute for Gallery acceptance.

## Start a bounded task

Lead supplies a short task record:

```text
TASK / FINDING_IDS:
BASE_SHA:
INVARIANT / OBSERVED_FAILURE:
OWNED_FILES:
EXPECTED_CHANGE:
PROOF_REQUIRED:
CAPTURE_PLAN / BUDGET:
STOP_CONDITION:
NEXT_OWNER:
```

Assign one writer per mutable file set. Parallelize only disjoint work with explicit interface ownership. A correction to theme transactions and a correction to render-target lifetime should be coordinated when they touch the same rendering invariant; two Workers should not independently rewrite those files.

The task states what would falsify its claim. “Settled screenshot looks correct” does not establish theme atomicity; “outer rectangle matches” does not establish internal alignment. Preserve closed findings unless a concrete regression changes their evidence.

## Level A — inner development loop

**Trigger/owner:** bounded implementation edits; Worker.

**Required:** inspect the diff; compile affected targets; run affected deterministic tests; check file size before a near-budget file grows. Choose tests by the changed authority, not by superficial filename similarity. For documentation-only changes, check links, identities, diff scope and wording instead of rerunning native tests.

**Optional:** a narrowly scoped static analysis or small native contract probe when shared tests cannot exercise the changed platform behavior.

**Skip/reuse:** no routine WGC, full Gallery sweep, full native suite, or complete reference regeneration. Existing evidence can guide implementation but cannot certify a changed binary.

**Artifacts:** concise command/result notes and the relevant regression assertion. Do not commit every intermediate build log.

**Budget:** zero WGC captures. A concrete native uncertainty escalates to B; it does not justify repeatedly running D.

**Stop:** finish the coherent implementation and the known failure set before expensive acceptance. Escalate an authority conflict or frozen-reference defect to Planner; do not silently change the reference.

## Level B — focused integration

**Trigger/owner:** a coherent fix needs native proof, or a specific observed defect cannot be diagnosed statically; Worker under Lead's plan.

**Required:** the affected real Gallery page, interaction states and semantic/part assertions. Run a native subsystem probe only where it tests a distinct contract such as IME ownership, UIA lifetime or device-loss recovery.

**Optional:** one comparison at a second DPI or compact viewport when the change crosses those boundaries. Motion and first-presentation changes need bounded frame evidence, not a larger collection of settled screenshots.

**Skip/reuse:** unrelated pages and already proven same-build states. Use one capture for all element crops and comparisons within it.

**Artifacts:** paired PNG/snapshot and a receipt binding their identity, plus focused assertions. Store failure crops when they explain the defect; avoid copying the full image into every element directory.

**Budget:** normally no more than six unique static captures per coherent correction set. Lead can justify a larger budget by enumerating distinct invariants. A bounded motion/frame sequence has a separate stated duration and maximum sample count; it is not disguised as hundreds of static states.

**Stop:** missing target, wrong running page/theme, invisible target or identity mismatch stops that capture plan. Do not continue producing misleading filenames. Batch the observed failure set before rerunning.

## Level C — cached visual comparison

**Trigger/owner:** a valid paired capture exists; assigned evidence owner, normally the Worker.

**Required:** run geometry and raster comparisons offline with explicit modes and tolerances. For Field and similar composite controls, compare owner-relative parts as well as outer size. Compare absolute positions only where the reference and native composition intentionally share that coordinate frame.

**Optional:** focused text/edge crops for interpretation by Opus. Raw pixel differences are measurements, not a claim of perceptual equivalence or a reason to dismiss font-advance differences.

**Skip/reuse:** all native launches and WGC for existing valid inputs. A comparator or tolerance change reruns the comparison, not capture. A failed geometry comparison does not itself require recapturing an unchanged state.

**Artifacts:** input hashes, semantic ID/part, comparison mode, tolerance, command/tool identity, expected/actual/delta, status and useful diff/crops. Summaries must derive from those records and distinguish size agreement from complete visual parity.

**Budget:** zero new WGC. Missing inputs become NOT_MEASURED and a specific B request, never an automatic unlimited capture loop.

**Stop:** reject invisible/out-of-image crops and unpaired snapshots. Invalid provenance is not a visual FAIL or PASS; fix the evidence input first.

## Level D — frozen candidate acceptance

**Trigger/owner:** Lead declares a cohesive implementation frozen; one acceptance owner coordinates execution.

**Required:** applicable format/build, Linux core and Windows native tests; actual Gallery/devctl integration; affected DPI/resize, IME/UIA/RichEdit, theme/motion, resource/idle checks; file-budget check; representative reference comparisons; all applicable Universal Gates 1–20. Record exact commands, features and candidate identity. Historical test counts do not transfer to the new executable.

**Reuse:** B/C artifacts can satisfy acceptance only when they bind the final binary and matching environment/state. An unchanged documentation commit may reuse binary evidence if payload equivalence is explicit. Different source/binary results remain historical. Unaffected native contract coverage may be reused where its proof boundary and environment equivalence are documented, rather than replaying every environmental scenario.

**Optional:** extra DPI, window-size, resource or frame measurements for unresolved risks. Unavailable environmental cases retain their honest status and invariant-level risk assessment.

**Capture planning:** approximately 16–24 unique static Gallery states is a starting planning envelope, not a completeness limit or new acceptance threshold. Enumerate required invariants first; explain additions. The resource workflow has its own budget: replace three repetitions of the full functional sequence with one functional session and two minimal launch/capture/shutdown sessions where equivalent. With the current eleven-capture functional workload, this proposes thirteen nominal captures rather than thirty-three. Preserve relevant assertions; do not simply delete leak coverage. Motion frames are separately bounded and should use native frame instrumentation rather than a fresh CLI process for each sample.

**Artifacts:** one immutable evidence manifest, test results, capture/comparison summaries, representative runtime metrics, exceptions and open failures. Include measurement sample counts and distributions; do not manufacture performance thresholds before establishing a baseline.

**Failure behavior:** stop on systemic state/identity errors. Gather the coherent known failure set, fix it, rerun affected checks, then perform the required final confirmation. The existing cadence remains: complete implementation, full applicable verification, fix failures together, repeat until green. This proposal narrows redundant work; it does not waive that acceptance gate.

## Level E — independent reviews

**Trigger/owners:** final candidate and evidence are available; Opus reviews visual quality, Astra reviews systems/contracts/performance/evidence, Planner owns acceptance and merge authorization.

**Required:** both reviews name the exact candidate and evidence revision. Findings identify the violated invariant, concrete evidence, minimal correction and closure proof. A new user-visible failure takes precedence over a stale PASS label.

**Optional:** reproduce disputed or high-risk claims with a stated unanswered question. Reuse trustworthy retained evidence; do not automatically replay acceptance as a second full campaign.

**Budget:** zero new captures by default. Additional captures require a small enumerated plan tied to the disputed finding, not a new broad discovery sweep.

**Artifacts/stop:** review artifact and focused closure matrix. Do not reopen unrelated proven non-defects during closure. Changed production after review requires review of the relevant delta; publication-only commits retain separate implementation/evidence identities.

## Capture reuse and identity

Use an ordinary versioned JSON manifest beside existing evidence. No database, daemon or control service is necessary. Keep authentication tokens, transient session IDs and absolute developer paths out of retained records.

The capture key must include or reference:

- Source SHA and clean/dirty source identity; Gallery and devctl executable hashes; profile and feature set.
- Reference version/content identity and comparator identity where comparison results are cached.
- Actual Gallery page, requested/effective theme, interaction state, popup/scroll state and target IDs.
- Logical and physical client size, scale, font assets/resolution, OS/rendering environment and materially relevant flags.
- Capture semantics: settled state versus a specified successful presentation epoch. A paint epoch by itself is not a globally unique cache key.

The receipt binds PNG and semantic snapshot hashes to this key. The comparison receipt binds its two inputs, mode, tolerances and result. Keep capture timestamps as provenance, outside deterministic snapshot equality. Persist actual measurement values rather than unsupported “verified” prose.

Example shape, illustrative rather than a required new schema:

```json
{
  "source_sha": "<full source SHA>",
  "gallery_sha256": "<binary hash>",
  "devctl_sha256": "<binary hash>",
  "profile": "<actual build profile>",
  "features": ["devtools"],
  "reference": "<frozen payload identity>",
  "environment": "environment.json",
  "state": {
    "page": "forms",
    "theme": "light",
    "capture_kind": "settled",
    "preconditions": "<checked semantic assertions>"
  },
  "png_sha256": "<image hash>",
  "snapshot_sha256": "<snapshot hash>",
  "command": ["<exact executable>", "<exact arguments>"],
  "attempts": 1,
  "status": "PASS"
}
```

Before capture, verify the actual page and theme from retained state and identifying semantic targets. Environment variables and filenames are requests, not observations. Keep these Gallery-specific assertions in the orchestration layer rather than adding product-specific runtime commands.

Compare semantic fingerprints before acquiring another image. Unexpected identical state across supposedly different pages stops the plan. Identical images in states that intentionally render identically are valid; this is not a blanket hash-uniqueness requirement.

A changed binary normally creates a new capture identity. Reuse across documentation-only publication is justified by unchanged implementation/binary identity, not by assuming the UI probably looks the same. A changed font, DPI or rendering environment also invalidates canonical capture reuse for that environment.

## Retry, resource and measurement discipline

Give one screenshot operation an end-to-end deadline encompassing its RPCs and retries. Record attempts and the transient failure reason. Apply a campaign-wide retry budget so three per-command attempts cannot silently multiply a sweep. Never blindly retry a timed-out click or other non-idempotent mutation; first observe whether its effect occurred.

Preserve explicit launch/shutdown/resource tests but separate them from functional and offline comparator tests. Count launches, WGC requests/attempts, unique states, cache hits, unexpected semantic duplicates and total wall time. These counters reveal cost without excessive logs or a new service.

For rendering changes, distinguish requested style, submitted draw and successfully presented frame. Theme acceptance requires the first successful post-switch presentation to have one coherent palette across background, painted nodes and native peers. The current screenshot command's wait-idle barrier cannot prove this transient invariant by itself.

Use existing frame/timing instrumentation for bounded measurement, with present success, cause and damage information where needed. Start with three repeated runs of the same frozen executable/environment; include warm interaction, theme switches, resize and thirty-second idle. Record p50/p95/p99 where sample counts make them meaningful, plus counts and ranges. The primary audit specifies the minimal workloads and existing command paths. Baseline first; agree performance thresholds afterward.

## Reporting and escalation

Allowed evidence outcomes: PASS, FAIL, NOT_MEASURED, UNAVAILABLE, DEFERRED and APPROVED_DEVIATION. A deviation names its reason and approving authority. Missing target/capture is not PASS; dimension agreement is not raster or motion parity.

Use concise checkpoints that answer what changed, what was actually proved, what remains and who owns it. Do not create a new top-level goal for every checkpoint or resend the entire historical review. Carry stable finding IDs and link immutable evidence instead.

Escalate to Planner for scope/authority conflicts, a frozen-reference defect, acceptance-policy changes or a newly discovered systemic blocker. Lead can autonomously sequence ordinary bounded fixes and tests within authorized scope. Reviewers do not become competing implementation Workers.

Before a merge decision, the handoff contains:

```text
IMPLEMENTATION_SHA:
EVIDENCE_SHA / MANIFEST:
BINARY_IDENTITIES / ENVIRONMENT:
COMPLETED:
CHECKS_EXECUTED:
CAPTURES: planned / unique / attempts / reused
OPEN_FAILURES / NOT_MEASURED / APPROVED_DEVIATIONS:
VISUAL_REVIEW:
SYSTEMS_REVIEW:
MERGE_RECOMMENDATION:
NEXT_OWNER:
```

## Minimal adoption sequence

Planner can authorize the capture preconditions/manifest and focused workflow first. Lead should implement them in existing tooling, correct current evidence labels, then evaluate observed time/capture reductions. Follow with the shared file-budget scope and lightweight timing improvements when relevant. Do not launch a second refactor campaign, new numbered gate system, new CI, or a general evidence platform.

Until authorized, this document remains a proposal. Existing repository instructions and approved architecture retain authority.
