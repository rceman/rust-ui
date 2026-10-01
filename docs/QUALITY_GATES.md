# Mascot Universal Task Completion and Review Gates

**Canonical authority:** project-wide Mascot engineering policy  
**Status:** active

This file contains the canonical Mascot Universal Task Completion and Review Gates 1-20.

These twenty gates are the only repository-wide completion/review gate taxonomy. Project/task specializations may strengthen them and define executable checks, but MUST NOT create a competing numbered gate system.

## GATE 1 — Requirements & Goal Completeness

Every explicit requirement and approved clarification MUST be implemented and verified.

Translate the requested outcome into concrete verifiable goals before coding. Re-read the authoritative Task/request and current Planner documents after implementation rather than relying on the initial prompt or an early summary.

For Mascot this includes, as applicable:

- source/runtime behavior;
- rig/art requirements;
- animation ranges and clips;
- native-platform behavior;
- visual evidence;
- performance/iteration requirements;
- agent-authoring/tooling requirements;
- final authoritative Git status/diff/commit/push requirements.

FAIL when only a helper or partial lifecycle is proven; required states, error/default/recovery paths, animation cases, evidence, performance checks or later clarifications are missing; or completion has no specific verification path.

Evidence MUST map:

    requirement -> implementation -> proof

## GATE 2 — Assumptions, Ambiguity & Approval Boundaries

Material assumptions and ambiguity MUST be surfaced before semantics are chosen.

If multiple reasonable interpretations exist, present them instead of silently choosing one. If a materially simpler interpretation/solution exists, surface it. If product/art/architecture ownership is unclear, stop and request the required decision.

Do not invent product semantics, visual language, compatibility obligations, animation behavior, runtime ownership or external integration contracts merely to keep moving.

FAIL if implementation guessed or hid a material uncertainty.

Small implementation details inside an already-approved design may be resolved locally when they do not alter user-visible semantics or architecture.

## GATE 3 — Existing Behavior & Contract Preservation

Behavior outside approved change scope MUST remain unchanged unless the difference is a necessary demonstrated consequence of the Task.

Inspect the authoritative baseline for every affected existing semantic path before changing it.

Preserve unaffected:

- CLI behavior;
- runtime semantics;
- public/native interaction behavior;
- rig coordinate conventions;
- animation semantics;
- asset identity/style;
- evidence format;
- build/test behavior;
- platform behavior.

A new test that merely rewrites the expected value is not authorization for a behavior change.

This gate does **not** require backward compatibility for superseded internal formats. Internal hard-cut migration is governed by Gate 8 and the repository hard-cut policy.

## GATE 4 — Scope Discipline / Surgical Change

Every changed line/file MUST trace to:

- an explicit requirement;
- a necessary implementation;
- a necessary test;
- required generated consequence/evidence;
- required authoritative documentation/contract update;
- unavoidable dependency consequence.

FAIL for:

- adjacent cleanup;
- opportunistic refactors;
- unrelated formatting;
- broad dependency refresh;
- unrelated generated churn;
- cosmetic edits outside task scope;
- unrelated dead-code removal.

If a systemic audit under Gate 20 proves the defect class is broader than the initial file set, the expanded scope is justified by that audit and MUST be reported.

## GATE 5 — Minimal Correct Design / No Overengineering

Implement the smallest design that fully satisfies current requirements and established boundaries.

FAIL for:

- speculative frameworks without present ownership value;
- one-use abstractions that add indirection;
- unrequested flexibility/configurability;
- unnecessary managers/strategies/adapters;
- large general-purpose systems where a narrow project-owned primitive suffices;
- material complexity without correctness, reuse, performance or ownership benefit.

Review MUST ask whether a materially simpler solution satisfies the same contract.

For Mascot, "minimal" does not mean ad-hoc one-off scripts when the workflow is explicitly intended to be reused by agents. Durable authoring/QA tooling is justified when it eliminates repeated implementation/token cost.

## GATE 6 — Architecture, Ownership & Responsibility Boundaries

Responsibilities MUST remain with the correct owner and stay cohesive.

Current intended boundaries include:

- animation semantics/data -> `mascot-animation`;
- Windows rendering/composition -> Windows renderer layer;
- agent/developer authoring and QA orchestration -> `mascotctl` / project-owned tooling;
- canonical art/rig data -> versioned asset data;
- runtime-generated outline/shadow -> renderer/compositor, not baked art;
- native editable text -> native platform text stack when product UI work begins;
- Git authority for a Windows task -> the authoritative checkout named by its handoff;
- native Windows build/run/render/perf -> native Windows environment.

FAIL for:

- God crates/modules/tools;
- hidden orchestration in unrelated infrastructure;
- duplicated ownership;
- renderer code owning animation authoring policy;
- art tooling owning runtime semantics;
- product runtime carrying development-only video/QA dependencies;
- architecture drift from approved boundaries.

## GATE 7 — Duplication & Existing Capability Reuse

Before adding material logic, inspect the affected cone for an existing owner/capability.

Reuse suitable:

- animation types;
- renderer primitives;
- asset decoders;
- QA/artifact-analysis utilities;
- image/render intermediates;
- CLI parsers;
- validation helpers;
- test utilities;
- timing/profiling helpers.

FAIL for:

- duplicate business/animation rules;
- multiple canonical clip representations;
- a second rig parser;
- repeated image decode/render passes that can be shared;
- re-creating scratch crop/zoom/montage logic instead of using project tooling;
- materially equivalent implementations without justification.

## GATE 8 — No Unrequested Fallback / Shim / Legacy Path

Do not add fallbacks, compatibility shims, aliases, dual read/write/configuration sources, old/new routing, heuristic legacy interpretation, weaker-authority recovery or executable obsolete paths unless explicitly required and owner-approved.

Mascot internal formats are **hard-cut by default**.

A real compatibility boundary must be explicitly named, for example:

- released/public API;
- persisted user data that must be read;
- external third-party consumer;
- plugin/provider protocol outside this repository;
- supported released product version.

Without such a boundary:

- migrate;
- update all in-repo consumers;
- remove old parser/loader/data path;
- remove compatibility tests;
- delete the shim.

For the current clip-authoring migration, the intended final state is:

    canonical authoring format: mascot-clips/0.3
    supported authoring format: mascot-clips/0.3
    executable mascot-clips/0.2 compatibility: none

Historical docs/evidence may mention 0.2.

## GATE 9 — Deterministic Semantics & Stable Contracts

Every public/cross-boundary operation MUST have one deterministic semantic meaning and one stable contract.

For Mascot this includes:

- rig coordinate spaces;
- pivot/origin meaning;
- positive rotation direction;
- safe rotation ranges;
- clip key/easing semantics;
- compact authoring expansion;
- root mirror behavior;
- runtime outline/shadow semantics;
- artifact detector severity/reason codes;
- CLI output/exit semantics;
- gate receipt structure.

Inputs/outputs MUST use canonical shared naming and scalar semantics where authority defines them.

FAIL for:

- ambiguous coordinates;
- unstable schema/types;
- hidden modes;
- duplicated semantic fields;
- two canonical authoring sources;
- caller guesswork;
- tool output that changes meaning depending on undocumented environment state.

## GATE 10 — Authority, Specs, Rules & Obsolescence

Before coding and again during review, resolve current authoritative:

- `AGENTS.md`;
- task handoff;
- rig/runtime spec;
- product UI/animation direction;
- agent-authoring format policy;
- visual-QA policy;
- quality gates;
- approved cutovers;
- already-existing implementations;
- superseding work.

Do not repair, optimize, expand tests for or preserve a surface already replaced/retiring unless bounded migration safety or owner approval requires it.

If two docs conflict, resolve the authority/supersession rather than implementing both.

## GATE 11 — Failure Safety, Atomicity, Idempotency & Security

Review every changed/new:

- error path;
- retry;
- file mutation;
- generated artifact write;
- subprocess call;
- sync operation;
- cache mutation;
- external/native API call;
- multi-side-effect boundary.

Validate before invalid durable mutation.

For authoring/QA tooling:

- partial generation MUST NOT masquerade as complete evidence;
- failed renders/encodes MUST produce non-zero failure;
- stale cache MUST NOT be treated as fresh evidence;
- source/evidence synchronization, when a task actually requires it, MUST NOT overwrite unrelated authoritative changes;
- generated receipts MUST identify the exact candidate/input identity;
- retryable subprocess operations MUST avoid duplicate/corrupt output.

Keep secrets/sensitive data out of logs/evidence.

Security controls MUST correspond to a demonstrated trust boundary and threat model. Do not add credential ceremony or privilege layers without concrete risk reduction.

## GATE 12 — Boundedness / Resource & Agent-Visible Output Discipline

All loops, retries, recursion, scans, render ranges, frame counts, subprocesses, concurrency, retained frame history, logs, result sets, caches and model-visible output MUST be safely bounded, streamed or paginated as appropriate.

Agent-visible output MUST be semantically minimal.

Prefer commands such as:

    mascotctl clip show blink
    mascotctl rig joint neck

over dumping an entire rig/clip library.

Normal status should omit:

- repeated request context;
- duplicate paths;
- giant normalized JSON;
- raw frame lists when a concise suspect summary suffices;
- null/default filler;
- verbose diagnostic detail unrelated to the next action.

Detailed evidence can live in files/JSON artifacts.

FAIL for token spam, unbounded frame retention, accidental giant stdout or unnecessary repeated context.

## GATE 13 — Dependency, External-System & Blast-Radius Necessity

Every added/widened:

- Rust crate;
- Python package;
- native library;
- binary tool;
- subprocess;
- media encoder;
- runtime;
- service;
- network dependency

MUST be materially necessary.

Review:

- reverse dependency cone;
- binary/runtime footprint;
- build/test cost;
- deployment coupling;
- dev-only vs production ownership.

Examples:

- development-only `ffmpeg` is acceptable for evidence encoding when kept out of the production runtime;
- a browser engine/game engine/general-purpose UI framework is not acceptable merely to simplify the animation lab;
- image-analysis Python may be acceptable if clearly owned as dev tooling and materially simpler than duplicating it in Rust.

FAIL for unnecessary dependency expansion or production coupling to development-only tooling.

## GATE 14 — Performance, Latency & Redundant Work

The change MUST NOT introduce or leave a material performance regression or unnecessary repeated work in the affected path.

Inspect:

- algorithmic complexity;
- repeated image decoding;
- duplicate frame rendering;
- repeated art decomposition;
- process-per-frame/subprocess amplification;
- unnecessary Cargo rebuilds for data-only edits;
- unnecessary rerender of unchanged clips;
- avoidable synchronous I/O;
- excessive allocations/memory growth;
- lock contention;
- token/output overhead.

Current authoring-tool targets are defined in `docs/MASCOT_AGENT_AUTHORING_FORMAT.md`.

Project policy:

- routine operation >10 s -> profile before completion;
- routine operation >30 s -> performance defect/blocker unless explicitly justified;
- routine 30-120 s authoring commands are not acceptable merely because they eventually succeed.

Render frames once and reuse them for:

- artifact analysis;
- contact sheets;
- zoom evidence;
- video encoding.

Performance-sensitive native runtime measurements MUST remain separate from deterministic correctness tests.

## GATE 15 — Persistence / Infrastructure Boundary

Persistence, filesystem layout, cache mechanics, evidence storage and platform infrastructure MUST remain behind explicit typed/use-case-oriented boundaries where they are not themselves the task domain.

Animation/runtime logic MUST NOT depend directly on:

- task-specific Git checkout/sync mechanics;
- evidence directory layout;
- ffmpeg command construction;
- Windows working-copy location;
- cache filesystem internals.

The product runtime MUST NOT know developer evidence/QA storage mechanics.

CLI/orchestration layers may own those infrastructure concerns.

FAIL when domain/runtime code becomes coupled to physical persistence or dev-environment mechanics without explicit architectural approval.

## GATE 16 — Verification Sufficiency & Test Quality

Verification MUST prove changed semantics rather than merely execute code.

Bugs SHOULD use:

    RED -> minimal fix -> GREEN -> regression proof

where practical.

Features MUST map acceptance criteria to focused automated/runtime evidence.

For the current animation/rig work, verification may include:

- parser/expansion unit tests;
- rig validation;
- runtime animation tests;
- deterministic static stress renders;
- all-frame artifact analysis;
- zoomed visual inspection;
- committed MP4/contact-sheet evidence;
- native Windows performance measurements.

Existing relevant tests MUST remain green.

Never weaken:

- assertions;
- safe ranges;
- detector thresholds;
- visual gates;
- tests;
- gate definitions

merely to obtain green.

A passing alpha guard alone does not prove visual quality.

### Environment equivalence for platform-dependent verification

A test or probe that validates platform-dependent behavior must reproduce every platform/environment property that is semantically relevant to the behavior being tested. A PASS obtained under materially different process, DPI, locale, input, timing, graphics, accessibility, or OS semantics is not sufficient evidence for the production contract.

For DPI-sensitive/native UI behavior specifically:

    production process DPI-awareness
    ==
    native validation/probe DPI-awareness

unless the test intentionally exercises a different awareness mode and states that explicitly.

For geometry/rendering tests, the harness must explicitly establish and record:

    process DPI-awareness mode
    logical coordinate unit
    native coordinate unit
    effective DPI / scale
    expected conversion boundary

Do not rely on accidental equivalence such as:

    96 DPI
    -> 1 DIP == 1 physical px

because this can mask unit/scale defects that appear at:

    125%
    150%
    200%

A geometry probe that passes only because it runs DPI-unaware at 96 DPI must not be treated as evidence for a Per-Monitor-V2 production process.

Where applicable, deterministic coverage should include representative scales such as:

    96 DPI   / 100%
    120 DPI  / 125%
    144 DPI  / 150%
    192 DPI  / 200%

The exact matrix may be reduced only when the tested contract is mathematically proven scale-independent and the reduction is documented.

## GATE 17 — Recovery Proportionality / Sunk-Cost Containment

Repair of stale, rejected, obsolete, contaminated or ambiguous execution state MUST NOT cost/risk more than bounded salvage into a clean lane plus cleanup.

Explicitly compare repair cost/risk against clean salvage/restart.

Mascot examples:

- do not preserve a bad v0.1 rig decomposition merely because effort was spent on it;
- do not stack adapters around an obsolete clip format;
- do not keep corrupted/generated evidence if deterministic regeneration is cheaper;
- do not keep patching a broken working copy when a clean clone from the authoritative remote is safer.

After repeated failures of the same local approach, prefer a bounded reset/reconstruction rather than sunk-cost continuation.

## GATE 18 — Verification / Review Immutability & Artifact Identity

All mutating:

- formatting;
- code generation;
- art generation/reconstruction;
- clip compilation/migration;
- evidence generation;
- dependency installation/update;
- autofix

MUST occur before authoritative final verification.

Capture exact candidate identity and relevant content/input hashes where practical.

Run final verification against that materially frozen candidate.

If final verification or review mutates source, rig data, clips or renderer behavior, previous verification/evidence is stale and MUST be rerun.

For generated visual evidence, the receipt MUST tie evidence to the exact:

- source HEAD/tree;
- rig data;
- clip data;
- renderer/tool version;
- relevant generation settings.

Any unaccounted artifact mutation during authoritative verification is FAIL.

## GATE 19 — Completion Integrity / Final Artifact Honesty

Before declaring completion, inspect:

- final code;
- final rig data;
- final clips;
- final generated evidence;
- final videos/contact sheets;
- final QA JSON;
- final performance report;
- final documentation;
- final Git state.

Every changed file MUST be justified.

Known relevant failures, unresolved visual artifacts or contract mismatches MUST NOT be hidden behind a green summary.

For visual work, Lead/agent MUST inspect representative **actual rendered outputs**, not only schemas, logs or detector summaries.

For platform-specific workflows, completion also requires:

- the task's designated authoritative Git checkout is used;
- `git status` and the complete diff are reviewed there;
- commit and push are performed from that authoritative checkout;
- the authoritative task branch is pushed;
- the authoritative worktree is clean and equal to the expected origin state.

When a task explicitly designates a native Windows clone as Git authority, do not add an unnecessary WSL copy/sync round-trip.

Use COMPLETE only when the final artifact honestly satisfies the declared contract.

## GATE 20 — Systemic Scope Completeness / Exhaustive Invariant Audit

Automatically applicable when the Task/change claims a:

- cross-cutting;
- subsystem-wide;
- project-wide;
- repo-wide;
- migration;
- hard-cut;
- replacement;
- prohibition;
- sole-authority;
- exhaustive invariant.

A systemic claim requires systemic evidence; examples cannot prove a global invariant.

Before completion:

1. define the invariant and bounded surface;
2. enumerate candidate violations with independent discovery methods;
3. classify every candidate;
4. fix the complete required set;
5. rerun the same audit;
6. demonstrate zero unexplained candidates.

After the second finding of the same architectural defect class in one implementation/review cycle, STOP whack-a-mole correction and perform this systemic audit before further local fixes.

### Test-environment divergence is a systemic defect class

When a defect reveals that a test environment differed materially from production, do not fix only the single failing test.

Audit all validation paths for the same hidden assumption.

For this class of issue, check all native probes/scenarios that depend on:

    DPI
    coordinate transforms
    device pixels vs logical units
    font/text metrics
    surface dimensions
    pointer coordinates
    caret/selection geometry
    IME/candidate positioning
    UIA bounds

If one harness had an incorrect DPI-awareness assumption, verify the others instead of assuming the problem is local.

The corrective action is complete only when:

    production invariant
    test-harness invariant
    evidence metadata

agree.

Current examples that trigger Gate 20:

- "0.2 compatibility is completely removed";
- "all required clips use compact 0.3 authoring";
- "no required visual artifact remains inside declared safe ranges";
- "all required clips have committed video/contact-sheet evidence";
- "no recurring authoring workflow remains scratch-only";
- "static runtime has no continuous frame loop";
- "every required animation is checked frame-by-frame";
- "no routine authoring command exceeds the performance policy without explanation";
- "no native geometry probe relies on implicit 96-DPI/DPI-unaware equivalence to production".

---

## Mascot specializations

These are specializations of Gates 1-20, not additional gates.

### Visual rig / animation QA

The canonical visual-QA implementation is defined in:

- `docs/MASCOT_AGENT_AUTHORING_VISUAL_QA.md`
- `docs/MASCOT_RIG_RUNTIME_SPEC_V0.2.md`

Visual completion requires both automated detection and actual rendered-output review.

Automatic detection should identify, rank and provide zoom evidence for likely:

- gaps;
- dark slivers;
- black wedges;
- double outlines;
- detached components;
- topology anomalies;
- shading discontinuities;
- exposed cut edges;
- z-order/coverage anomalies.

ERROR findings MUST be fixed before pass. WARN findings MUST be reviewed and either fixed or narrowly suppressed with committed rationale/data.

### Agent authoring / token efficiency

The canonical compact authoring and tool-performance requirements are defined in:

- `docs/MASCOT_AGENT_AUTHORING_FORMAT.md`

Agent-facing data and CLI output must remain compact enough that repeated development does not waste context/tokens.

### Native Windows validation

For tasks whose handoff requires native Windows validation:

- the task handoff names the authoritative Git checkout;
- native Windows is the build/run/render/performance environment;
- when Windows Git/SSH is available and the task designates Windows authority, clone/fetch/commit/push directly on Windows;
- no CI substitution;
- avoid unnecessary cross-environment source-copy/sync pipelines.

#### DPI-awareness equivalence (Gate 16 specialization)

The production application runs Per-Monitor-V2 DPI-aware. Any native probe or scenario intended to validate production geometry/input/rendering must establish equivalent DPI-awareness before exercising the contract — for a test binary, `SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)` is sufficient. A probe that runs DPI-unaware at 96 DPI sees `px == DIP` numerically and cannot detect physical-pixel contract defects.

#### Regression rationale — why this rule exists

A RichEdit `TxDrawD2D` geometry defect was hidden because the offline probe ran DPI-unaware at 96 DPI, where px and DIP were numerically equal.

The real Per-Monitor-V2 application at 125% exposed the incorrect DIP/physical-pixel COM-boundary assumption: msftedit's host coordinate space is physical pixels at the host DC's dpi, so DIP bounds rendered ~one row above their intended position (each `lprcBounds` value divided by `dcDpi/96`).

This is a reusable engineering rule, not merely historical evidence: match the harness's environment to the production contract before trusting a geometry PASS.

### Executable gate mapping

`mascotctl` SHOULD expose project-owned commands that implement these twenty gates without inventing a second gate taxonomy.

Examples:

    mascotctl gate review
    mascotctl gate task

A task receipt MUST report each Gate 1-20 as:

    PASS / FAIL / N/A

with concise evidence/reference.

Lower-level commands such as:

    mascotctl format
    mascotctl check
    mascotctl test
    mascotctl qa stress
    mascotctl qa animations
    mascotctl perf

are verification mechanisms used as evidence for one or more universal gates; they are **not** separate numbered gates.

## Universal evidence and reporting

The completion report records every Gate 1-20 as PASS/FAIL/N/A plus concise evidence/reference.

Detailed command output may remain in project-native evidence artifacts.

N/A requires a concrete changed-cone rationale.

Mascot-specific specialization only strengthens these twenty gates and never creates another gate taxonomy.
