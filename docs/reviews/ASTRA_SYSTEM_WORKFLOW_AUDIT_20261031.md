# RUST_UI_ASTRA_SYSTEM_AND_WORKFLOW_AUDIT

**PROMPT_ID:** `rust-ui-astra-SYSTEM-AUDIT-20261031-114600`

**Role:** Independent systems architecture reviewer — Astra Advisor

**Project:** `rceman/rust-ui`

**Audit mode:** Read-only systems/workflow audit; subsequent documentation publication explicitly authorized by the owner.

**Verdict:** **CRITICAL_BLOCKERS** for P12 acceptance, not for the accepted architecture direction.

**Next owner:** rust-ui Planner

**Merge authority:** NONE

| Identity | Value |
|---|---|
| Audited branch | `feat/p12-automated-visual-parity` |
| Verified audit base | `cb6d888207321a2b724e68c302175661245ee00e` |
| Verified remote main | `873c9996a908d5307c364925bfec6b4c1c40eb54` |
| Latest commit touching production source/examples/Cargo manifests | `0db6e33127ed963dedc4b409de2a471dc7642289` |
| Audit started | `2026-10-31 13:02:04 UTC+03:00` |
| Audit completed | `2026-10-31 13:54:57 UTC+03:00` |
| Documentation publication branch | `review/astra-system-workflow-audit-20261031` |

The remote was checked twice during the audit and still matched the supplied P12 baseline. No newer published P0/P1 correction appeared. The active Worker's later changes are **not reviewed or accepted here**. Source, tests, configuration, policies, reference artifacts, and existing evidence were not modified. No GUI application or new screenshot sweep was launched. The publication commit containing this report is documentation-only and is not a new runtime candidate.

The accompanying [Engineering Workflow v2](PROPOSED_ENGINEERING_WORKFLOW_V2.md) is a **proposal requiring Planner authorization**, not an automatically adopted repository policy.

## A. Executive verdict

The architecture remains a sound foundation. The principal problem is that verification increasingly proves isolated mechanisms or settled states while completion reports imply broader user-visible correctness.

P12 should **not merge at the audited SHA**. The immediate blockers are:

1. Theme changes do not reliably invalidate the complete client surface. The reported theme-ordering defect is present in the published code.
2. Bounded rendering assumes framebuffer preservation without establishing it. The HWND render target uses `D2D1_PRESENT_OPTIONS_NONE`; device-loss retry also preserves the old partial damage scope.
3. Field correction has an unresolved authority conflict. Upstream specifies conditional start alignment, but frozen v0.1 actually records centered Switch geometry.
4. Motion guarantees exceed their implementation and evidence. Active reversals resample the clock rather than use the last successfully displayed value; omitted transition families contradict the vendored recipes.
5. P12 evidence cannot independently substantiate several VERIFIED/ACCEPTED claims. Field raster coverage, higher-DPI acceptance, comparison totals, and artifact identity need correction.

The highest-value workflow change is **one verified capture per state, many offline comparisons**. Most required infrastructure exists. The project needs a small checked capture driver, trustworthy receipts, targeted invariant tests, and narrower acceptance runs—not another framework or more review rounds.

Confidence is high in source-level findings and artifact inconsistencies, and moderate in runtime impact beyond the user's observations. Current Windows behavior was not reproduced and Rust suites were not rerun in this audit.

## B. Verified project state

### Independently executed inspection

- Checked local status, branch identity and remote refs with Git. The working tree was clean at audit start and finish.
- Read runtime reconciliation, theme propagation, Win32 turn/paint/presentation, transitions, RichEdit draw/measurement, UIA refresh, scheduler/mailbox, devtools capture/comparison, Gallery Field composition, test workflow, governance scripts and current evidence.
- Ran `tools/check-file-tokens.sh`: exit 0.
- Exercised the existing checker against temporary files outside the repository: 19,999 bytes passed; 20,000 bytes failed with exit 1. No test/source files were added.
- Compared retained P11 before/after PNG and layout bytes: 12/12 PNG pairs and 12/12 layout JSON pairs were equal.
- Parsed and counted P12 artifacts and recomputed the documented subset's size deltas.
- Inspected the existing Forms Light screenshot; no new visual capture was made.
- Checked the frozen reference diff against main: empty.
- Consulted Microsoft's Direct2D presentation/resize documentation for the native content-retention contract.

### Substantiated project progress

P11's structural-preservation evidence is substantial: the retained `before_clean` and `after` artifacts independently match, and [Argon's final approval](../STRUCTURAL_REFACTOR_FINAL_APPROVAL.md) identifies the reviewed candidate and code SHA. This does not transfer P11's visual equivalence result to P12.

The current [Gallery binary](../../src/bin/rust-ui-gallery.rs) includes the actual Gallery source. [Native integration tests](../../tests/devctl_windows.rs) launch `CARGO_BIN_EXE_rust-ui-gallery`, not a synthetic UI. The RichEdit DIB regression's host in [probes.rs](../../src/tests/probes.rs#L117) no longer uses `WS_VISIBLE`.

The retained generational arena, keyed reconciliation, native RichEdit, UIA, bounded mailbox/task infrastructure, and blocking wait remain present. Devctl and its capture/IPC dependencies remain behind `devtools`; ordinary application startup does not start its listener. P12 has not changed `reference/shadcn-gallery-v0.1/`.

### Reported results and limits

Linux 126/126, Windows devtools 198/198, and Windows default 169/169 appear in P12 documentation. The P12 evidence directory does not contain exact-candidate test logs or a binary/source identity receipt sufficient to bind those executions independently to the audited executable. They remain **reported results**, not an audit PASS. No current Rust suite result is invented here.

[COMPONENT_SUPPORT.md](../COMPONENT_SUPPORT.md) still identifies unfinished scope: Shadcn Tooltip is foundation-only, Popover is partial/planned, Dialog is planned, and the Gallery shell is partial. Historical component approval does not mean the entire native catalog or final P12 parity is complete.

The main evidence sources are [P12 ledger](../P12_VISUAL_PARITY_LEDGER.md), [capture/runtime audit](../P12_CAPTURE_AND_RUNTIME_AUDIT.md), [P12 evidence README](../../benchmark/results/p12-automated-visual-parity/README.md), and [motion evidence](../../benchmark/results/p12-automated-visual-parity/motion/README.md). Their contradictions are evaluated below rather than inherited as PASS labels.

## C. Architecture assessment

### Preserve the accepted ownership model

| Responsibility | Current authority |
|---|---|
| State mutation and staged reconciliation | Runtime |
| Node lifetime and stale identity rejection | Generational arena |
| Typed recipes and effective visual classification | Shared style/node code |
| Logical geometry and component geometry | Shared geometry/layout |
| Native input, RichEdit, HWND, UIA and presentation | Win32 backend |
| Cross-thread delivery | Mailbox and owned waitable event |
| Development inspection | Retained tree through devtools |
| Visual target | Frozen reference |
| Visual judgment | Opus |
| Systems/contracts/performance judgment | Astra |

There is no justification here for a replacement renderer, CSS runtime, mandatory async runtime, browser-based native UI, or reopening the foundation architecture. Windows remains the current platform realization of shared semantic contracts.

### Theme atomicity: correct direction, incomplete boundary

The published sequence is explicit:

- [pump_review.rs:249](../../src/runtime/pump_review.rs#L249) commits children.
- [reconcile_nodes.rs:196](../../src/runtime/reconcile_nodes.rs#L196) classifies visual changes using the existing theme.
- [pump_review.rs:262](../../src/runtime/pump_review.rs#L262) installs the staged theme afterward.

Unchanged role-backed nodes can avoid paint classification despite changing appearance. Full-client damage on a genuine effective-palette change is appropriate: background and gaps between nodes change too. Dirtying all live nodes is useful, but should not be the sole indirect mechanism for global surface invalidation.

The bounded correction should establish these invariants:

1. Validate against the staged theme; style preflight currently also receives the old `self.theme.dark`.
2. Commit palette identity once. A rejected transaction must not publish a new palette/displayed state. Structural/native commit failure retains the existing terminal teardown policy.
3. Compare effective palette output, not only Theme/Appearance descriptors. System colors can change under identical descriptors.
4. Synchronize RichEdit foreground and selection colors before presenting surrounding chrome in the new palette.
5. Damage the complete client once for a global palette change, including background/inter-node surfaces. Keep ordinary hover/focus/animation damage local.
6. Rebase transition/display caches so concrete old-palette colors cannot contaminate the first new-theme frame.
7. Commit displayed truth only after successful paint. Failure must not consume pending palette work or publish false displayed values.
8. Preserve HWND/peer identity, composition, selection, popup state and focus.

The effective peer-palette comparison in [turn.rs:71](../../src/platform/win32/turn.rs#L71) is useful and should remain. DWM caption synchronization already avoids window recreation, but non-client DWM updates and D2D client presentation are separate operations; source ordering is not proof of compositor-level atomicity.

Required focused regression coverage: first successful post-switch presentation in both directions, rapid reversal, System change, same-descriptor system-palette change, open popup/focused editor, and injected paint failure. A settled `wait-idle` screenshot cannot prove this invariant.

### Render-target content lifetime: additional immediate correction

[render/mod.rs:474](../../src/platform/win32/render/mod.rs#L474) creates the HWND target with `D2D1_PRESENT_OPTIONS_NONE`. [frame.rs:41](../../src/platform/win32/render/frame.rs#L41) then repaints only damage and skips other nodes. This requires untouched framebuffer contents to remain valid.

Microsoft documents NONE as discarding the frame after presentation, while RETAIN_CONTENTS preserves it. Partial redraw currently lacks the platform guarantee it assumes. [Microsoft: D2D1_PRESENT_OPTIONS](https://learn.microsoft.com/en-us/windows/win32/api/d2d1/ne-d2d1-d2d1_present_options)

Target-loss retry creates a new target but retries with existing partial damage. In addition, `Resize` is called from `WM_PAINT`; Direct2D specifies undefined backbuffer contents after Resize even when retention is enabled. [Microsoft: ID2D1HwndRenderTarget::Resize](https://learn.microsoft.com/en-us/windows/win32/direct2d/id2d1hwndrendertarget-resize)

Smallest correction: establish retained-content semantics for ordinary bounded painting; require full-client initialization after target creation, actual resize and recreation; avoid unnecessary same-size Resize calls; propagate resize failure.

This is a proven API-contract mismatch. It is **not proven to be the specific cause of the user's black rectangle**. Treat it as part of the immediate rendering correctness surface, not as an invitation to redesign the renderer.

### Field: native intent and frozen authority conflict

The native horizontal Field hardcodes `Align::Center` in [ui_controls.rs:379](../../src/ui/ui_controls.rs#L379). The [vendored upstream field.tsx](../../reference/shadcn-gallery-v0.1/vendor/shadcn/apps/v4/registry/bases/base/ui/field.tsx) includes conditional start alignment when FieldContent exists.

However, the frozen local [catalog.js:432](../../reference/shadcn-gallery-v0.1/src/catalog.js#L432) emits `flex-row items-center` without that conditional selector. Its [contract.json](../../reference/shadcn-gallery-v0.1/contract.json) confirms the centered arrangement.

| Measurement | Frozen reference | Committed native Forms snapshot |
|---|---:|---:|
| Content height | 37 dp | 37 dp |
| Switch top relative to content | Centered arrangement | approximately +9.304 dp |
| Thumb top relative to Field | 10.484 dp | approximately 10.492 dp |
| Description top relative to Field | 16 dp | 18 dp |

The native Switch offset is real and substantially matches the frozen centered target. A start-alignment correction follows the upstream conditional rule and user intent, but changes that frozen relationship. The description has a separate +2 dp internal spacing discrepancy.

Planner must authorize a narrowly documented native deviation or future reference revision. Do not silently alter frozen v0.1 or label a top-aligned result exact frozen parity. Any correction belongs in Field composition/semantics; generic Row alignment remains unchanged.

Validate `field.default`, `.invalid`, `.disabled` and `field.horizontal` through owner-relative part positions and visible real-Gallery crops. Outer dimensions alone are insufficient.

### Motion and successful displayed-state ownership

[transitions.rs](../../src/platform/win32/transitions.rs) has three concrete limitations:

- Active reversal computes a starting value with `trans_ease(..., now)`, rather than the last successfully presented value.
- `displayed_*` mutates `shown` during drawing, before EndDraw succeeds.
- Eligibility excludes editors, checkbox and Select-family controls, although [vendored Nova CSS](../../reference/shadcn-gallery-v0.1/vendor/shadcn/apps/v4/registry/styles/style-nova.css) includes `transition-colors` for Checkbox, Input, Textarea and Select trigger.

Scheduler/easing tests prove useful primitives but not these rendering invariants. Use one frame timestamp, retain successful displayed channels, and commit them at successful presentation. Define eligible properties from the accepted component contract. Button/switch endpoint success cannot establish complete motion parity.

### Other architecture observations

Generational caches, retained peer ownership, the owned mailbox event, and bounded teardown routes remain valuable. No new failure in those previously approved authorities was established here. P12's `part_key` assignment repairs a concrete semantic metadata problem; it should remain covered by focused reconciliation tests.

Large backend responsibility and repeated arena traversal are candidates for measured improvements, not evidence that the architecture must be replaced. Preserve shared classification/geometry helpers when extracting small modules; avoid creating second semantic authorities inside devtools or scripts.

## D. Runtime performance findings

### Retained measurements, not current-P12 results

[Historical resize evidence](../../benchmark/results/shadcn-native-gallery-slice2/resize-performance/summary.md) records:

| Metric | Historical baseline | Historical corrected Light |
|---|---:|---:|
| WM_SIZE handler p50 | 29.3 ms | 0.28 ms |
| Frame work p50 / p95 / p99 | 29.3 / 35.8 / 38.1 ms | 16.3 / 22.7 / 25.5 ms |
| Layout p50 | 15.2 ms | 0.95 ms |
| Paint p50 | 13.8 ms | 15.3 ms |
| Text-layout creations | Hundreds per step | 0 per frame after warmup |
| Paints per resize step | 1.74 | 1.00 |

These are historical debug-build measurements at scale 1.25, not measurements of `cb6d888`. The environment receipt is incomplete for exact modern binary attribution. The claim that approximately 15 ms software painting is an inherent floor is not established by those observations.

[Earlier devctl evidence](../../benchmark/results/rust-ui-devctl-v0.1/README.md) has source/hash-bound short idle and handle observations. They establish historical behavior, not current P12 resource usage.

### Current structural findings

- Blocking idle is preserved by `MsgWaitForMultipleObjectsEx(..., INFINITE, ...)` in [run.rs](../../src/platform/win32/run.rs#L309). No permanent rendering loop was found.
- Scheduler work is demand-driven, with transition deadlines and eventual quiescence.
- Format/text-layout caches are bounded; the text-layout cache caps entries at 512.
- [Shadow cache](../../src/platform/win32/render/shadow.rs) is bounded by 48 entries, 32 MiB and maximum raster dimensions.
- [Peer drawing](../../src/platform/win32/text/peer_draw_measure.rs#L99) reuses bitmap allocation but clears/rerenders the peer when drawn. Allocation reuse is not cached text-content rendering.
- Paint still constructs a whole ink map, compares damage, clones paint order and traverses nodes. Small damage does not imply O(damaged nodes) CPU cost.
- Interaction transitions can begin with a full paint: state dirty bits request paint without necessarily adding local ink damage; `damage=None` means full-frame rendering.
- UIA refresh is requested on paint work in [turn.rs](../../src/platform/win32/turn.rs#L305). With an active UIA root, color-only animation can unnecessarily rebuild semantic snapshots.
- [Resize diagnostics](../../src/platform/win32/mod.rs#L137) record a mutex-protected 8192-entry ring in ordinary builds; only JSON export is feature-gated. The ring can retain approximately 512 KiB. It is not merely a few atomics.
- [handshake_json](../../src/platform/win32/devtools/service.rs#L96), also used by `epoch`, builds a full semantic snapshot to obtain window metadata. This is avoidable overhead for timing samples.

These are source-backed cost mechanisms, not measured current latency. Optimize after measuring; do not introduce a new incremental-layout framework speculatively.

### NOT_MEASURED for the current candidate

Startup/time to first visible frame; input-to-present percentiles; first theme-frame latency; ordinary-interaction frame distributions; full versus bounded damage area/frequency; allocation pressure; current working/private memory; sustained handle/bitmap/COM/GPU growth; default/release versus devtools overhead.

### Minimal reproducible baseline proposal

After correctness fixes freeze, run one three-run campaign using identical binary hashes, font assets, display, viewport and workload. Record OS/CPU/GPU/refresh rate, actual scale, rustc, profile/features and whether a UIA client is attached.

Each run: launch real Gallery; record process creation and first successful presentation; warm Components; execute 100 alternating hover/leave actions, 20 Light/Dark switches, the existing resize workload, 30 seconds idle, and memory/handle observations before shutdown. Use optimized devtools instrumentation plus a default-feature launch/idle control. Never compare debug and release as equivalent.

Existing command shapes:

```powershell
& $ctl launch --exe $gallery --session $session --json
& $ctl wait-idle --session $session --json
& $ctl perf-reset --session $session --json
& $ctl hover button.default --session $session --json
& $ctl wait-idle --session $session --json
& $ctl leave --session $session --json
& $ctl click shell.theme.dark --session $session --json
& $ctl wait-idle --session $session --json
& $ctl click shell.theme.light --session $session --json
& $ctl wait-idle --session $session --json
& $ctl perf --session $session --out $perfOutput --json
& $ctl shutdown --session $session --json
```

These commands are a proposed workload, **not executed results**. Launch must use the frozen tested executable, not an implicit rebuild. Always use a finally/RAII cleanup path.

Current `resize --width/--height` passes outer-window physical pixels to SetWindowPos in [devtools/parts.rs:324](../../src/platform/win32/devtools/parts.rs#L324), while DEVCTL.md describes client Dp. Until reconciled, record returned client dimensions rather than assuming requested dimensions describe the client.

Existing counters do not provide input-to-present latency or all per-frame distributions. Extend the existing bounded sampler minimally with frame cause, palette revision, damage area and successful-present timing. Separate CLI/RPC elapsed time from runtime latency. Report sample counts, p50/p95/p99, maximum and run-to-run spread; establish thresholds from a baseline and product requirements, not arbitrary new limits.

## E. Bug and regression risk register

| Finding | Severity / priority | Evidence, root cause and subsystem | Recommended action / validation |
|---|---|---|---|
| SYS-01 Theme invalidation | Critical / P0 | Runtime commits under old theme; unchanged role-backed nodes may remain clean. See C. | Complete atomic effective-palette correction; prove first successful frame, both directions, System changes, peers, reversals and failure paths. |
| SYS-02 Framebuffer preservation | Critical / P0 | Partial rendering uses PRESENT_OPTIONS_NONE; recreation retains partial damage; Resize invalidates contents. See C and Microsoft contracts. | Establish retention/full initialization. Native regression must verify untouched regions through local repaint and recovery. |
| SYS-03 Field authority and spacing | Major / P1 | Centered native geometry matches frozen thumb position but conflicts with upstream conditional rule; description +2 dp. | Planner resolves authority. Field-only correction; four specimens, owner-relative parts and visible Gallery raster. |
| SYS-04 Motion continuity/coverage | Major / P1 | Active reversal resamples time; shown cache commits before presentation; recipe families omitted. | Successful-frame channel ownership and explicit property coverage; deterministic interruption/failure checks plus bounded native proof. |
| SYS-05 Evidence attribution | Major / P1 | No P12 binary receipt; committed comparison inputs do not reconstruct some reports. | Bind screenshot/snapshot/comparison by hashes; recollect only missing or incompatible states. |
| SYS-06 Premature status labels | Major / P1 | Field hidden; T6 unmeasured; zero-height claim contradicted by inline-code result. | Correct current statuses, preserve history; evidence or authorized deviation required for approval. |
| SYS-07 Typography residuals | Major / P1 | Tabs approximately -9.98 dp, small -5.07 dp, inline-code -40.65 dp width and +2.025 dp height. | Verify resolved face/weight/advance/baseline on identical strings and inputs. Fix or approve individually justified deviations. |
| SYS-08 Capture cost/state preconditions | Moderate / P2 | Prior environment loss duplicated pages; E2E makes 33 capture requests with up to 3 attempts each. | Assert state, reuse captures, use campaign retry budget, separate functional/resource loops. |
| SYS-09 Redundant hot-path/diagnostic work | Moderate / P2 | Full snapshot per epoch, default-build sample ring, whole ink traversal, paint-triggered UIA refresh. | Measure first; gate detailed diagnostics and remove demonstrated redundant work. |
| SYS-10 File-budget scope gaps | Moderate / P2 | Python/extensionless scripts omitted, staged type changes omitted, hook local. | Share scope, cover authored exceptions/type changes, verify activation at handoff. No unauthorized CI. |
| SYS-11 Tooling documentation drift | Moderate / P2 | DEVCTL says no compound parts/Gallery; resize units differ; audit says both resolved and OPEN. | Correct current-state docs from implementation/evidence; do not edit immutable historical reviews. |

The inline-code explanation that the reference mono stack differs from bundled Geist Mono is unsupported: the frozen contract explicitly names Geist Mono, 14 px, weight 600. Registration success alone is not per-run proof of the actual face used for every glyph. Font advance, wrapping and baseline differences must not be silently categorized as renderer noise.

Popup focus/clipping, native selection, high-DPI text and compact desktop behavior have historical tests/approvals. Their current P12 status requires the relevant final-candidate evidence; this audit neither invents new failures in those systems nor converts old PASS results into new ones.

## F. File-size governance assessment

Current implemented scope passes the hard rule `bytes < 20000`. Boundary behavior was independently exercised.

| File | Bytes | Headroom |
|---|---:|---:|
| `src/platform/win32/render/mod.rs` | 19,934 | 66 |
| `src/platform/win32/mod.rs` | 19,707 | 293 |
| `src/platform/win32/devtools/parts.rs` | 19,553 | 447 |
| `tests/devctl_windows/workflow.rs` | 19,471 | 529 |
| `src/platform/win32/construct.rs` | 18,639 | 1,361 |
| `src/bin/rust-ui-devctl.rs` | 18,160 | 1,840 |

The [hook](../../.githooks/pre-commit) is activated in this checkout through `.git/config`. It inspects staged blob sizes, handles rename destinations, and rejects the exact boundary. The [full checker](../../tools/check-file-tokens.sh) uses pipefail. This is materially better than advisory size reminders.

Remaining gaps:

- Both implementations omit `.py`; `tools/bounds_check.py` is authored code.
- Extensionless authored scripts such as `.githooks/pre-commit` are omitted.
- The hook's `--diff-filter=AMR` excludes type-change entries.
- Scope predicates are duplicated between hook and checker.
- `core.hooksPath` is checkout-local; activation elsewhere is not verified.
- Whole `reference/` and `benchmark/` exclusions include authored scripts, not just generated/vendor data. Document those actual exemptions accurately.

P11's include-based splits preserve behavior and identities but do not always create independent module boundaries. Frame fragments depend on surrounding locals; UI/style fragments share namespaces. This creates indirection, not proof that a new mass refactor is warranted.

Split near-limit files when relevant work next touches them. Prefer cohesive typed helpers/child modules. Retain include fragments where preserving test identity or a small render arm is justified. Future PR/server enforcement is an owner-authorized proposal only; the current no-CI constraint remains intact.

## G. Capture and verification cost audit

### Launch/capture paths

| Path | Purpose and audit conclusion |
|---|---|
| Gallery binary / Gallery example | Real native visual and interaction authority; one source compilation unit. |
| devctl CLI | Console orchestration, WGC and offline comparison; no separate visual specimen. |
| Rust Gallery launcher / PowerShell launcher / VBS wrapper | Developer convenience paths; both build/launch implementations need consistent identity and environment handling. |
| `tests/devctl_windows*` | Real Gallery E2E and resize verification. Cleanup uses session shutdown guards. |
| Hidden RichEdit DIB host | Subsystem regression; WS_VISIBLE removed. Preserve it. |
| Native/UIA probes | On-demand subsystem semantics; do not substitute for Gallery visual acceptance. |
| Composer / historical benchmark scripts | Historical or example workflows, not current Gallery parity authority. |

No current Cargo target or acceptance route for the old synthetic devctl UI fixture was found. An obsolete output binary, if retained on disk, is build debris rather than a reason to delete native subsystem tests. Keep devctl itself.

### Actual integration cost

[workflow.rs](../../tests/devctl_windows/workflow.rs) performs normal/hover/pressed captures, full-client/repeat captures, and six additional resource captures per cycle: **11 x 3 = 33 requests**. [The CLI](../../src/bin/rust-ui-devctl.rs#L287) permits three attempts per request, so the upper acquisition count is 99 in a retry-heavy run. The audit document's approximately 15 count omits the six-per-cycle repetition block.

Each [WGC acquisition](../../src/platform/win32/devctl_capture.rs#L95) creates a D3D11 device/context, frame pool and session. Offline element comparisons do not need another acquisition.

The reported earlier approximately 250 duplicate-state images out of 258 are consistent with the environment-propagation failure described in the repository. The current committed 12 PNG hashes are distinct; that duplicate burst was not found in the retained subset.

### What the committed P12 bundle actually contains

- 12 PNGs: six pages x two themes.
- 103 comparison JSON files, including a generic `comparison.json`.
- 100 wrapper result records containing geometry.
- 60/100 geometry records within 0.25 dp for width and height, independently recalculated.
- 102 wrapper records have `ok: false`, including strict comparison mismatches and missing-target errors; the remaining generic `comparison.json` has no outer `ok` field. These are not 103 equivalent visual defects.
- No P12 executable/source identity receipt or test logs.

The reported 87 captures / 351 comparisons / 215 size passes may describe a larger working collection, but that collection is not reconstructible from the committed bundle.

Concrete inconsistencies:

- [forms-light.json](../../benchmark/results/p12-automated-visual-parity/captures/forms-light.json) marks all four Fields invisible. `field.default` is at y approximately 1316, below the paired 1036-pixel image.
- [Its comparison](../../benchmark/results/p12-automated-visual-parity/comparisons/light-forms-field.default.json) describes a raster crop below that image's extent. Current [compare_element](../../src/devtools/compare.rs#L173) would reject the supplied hidden target. The report therefore used different inputs or provenance from the committed pair.
- Inline-code height differs by approximately 2.025 dp in both themes, contradicting the README's zero-height-delta claim.
- Comparison rows use tolerance zero; the 0.25-dp size-pass summary is a separate derived assessment, not those commands' PASS result.
- T6 is marked ACCEPTED while the README explicitly says no capture above scale 1.0 was available.
- The capture/runtime audit calls the contour issue resolved in one section and OPEN in its final results.
- Motion's claimed approximately 12 ms sampling actually has representative consecutive gaps around 55–63 ms. Those short series show activity and settling, not precise first-frame or reversal continuity.

Absolute HTML document positions and native Gallery positions need not match. Use explicit size-only and owner-relative part comparisons for components, and absolute mode only where the two layouts intentionally correspond. Raw zero-tolerance glyph differences are signals for analysis, not automatic geometry failure excuses or automatic visual approval.

### Cache, evidence and retry recommendations

Use a small file manifest, not a database/service. Capture identity includes source SHA, clean source state, Gallery/devctl executable hashes, features/profile, reference hash, actual page/state/theme, effective palette revision, client Dp/px size, scale, scroll/popups/interactions, font hashes/resolution, OS/rendering environment and relevant flags.

Each record binds PNG and snapshot hashes, successful paint epoch/capture timing, state preconditions, command, attempt count and result. Each comparison binds both inputs, semantic ID/part, mode, tolerances and comparator version.

Reuse identical captures for offline comparisons. Changed comparator/tolerance requires comparison only. Changed executable/rendering environment means old images remain historical baselines. Documentation-only reuse requires explicit unchanged-payload identity. Retain failure/review crops preferentially rather than duplicating every source image.

Verify actual Gallery page and effective theme before capture. Unexpected identical semantic fingerprints across supposedly different pages must stop the campaign. Keep Gallery assertions in the driver, not runtime-specific application commands.

Use one end-to-end screenshot deadline and a campaign retry budget. Record transient causes/attempts. Do not blindly repeat timed-out non-idempotent interactions. The current per-RPC timeout can multiply across one screenshot's several RPCs and retries.

Use these evidence statuses explicitly: **PASS**, **FAIL**, **NOT_MEASURED**, **UNAVAILABLE**, **DEFERRED**, **APPROVED_DEVIATION**. Deviations require reason and approving authority. Missing evidence is never PASS.

## H. Proposed engineering workflow v2

This is a proposal, not a new gate taxonomy or accepted policy. Operational detail is in [PROPOSED_ENGINEERING_WORKFLOW_V2.md](PROPOSED_ENGINEERING_WORKFLOW_V2.md).

| Level | Trigger / owner | Required work | Reuse / optional work | Capture budget / failure behavior |
|---|---|---|---|---|
| A: inner loop | Bounded source edit / Worker | Affected compilation and deterministic tests; diff inspection | Reuse unchanged evidence during iteration; no routine whole suites | 0 WGC. Escalate only for concrete native uncertainty. |
| B: focused integration | Coherent fix or native diagnosis / Worker | Affected real Gallery page/states, semantic assertions, relevant native probe | Reuse same-build captures; no unrelated pages | Normally <=6 static captures per correction set; separately bounded motion sequence if justified. Stop on wrong state or missing target. |
| C: cached comparison | Valid capture exists / evidence owner | Offline size/part/text/raster comparisons and explicit statuses | Reuse original PNG/snapshot for every crop | 0 new WGC. Missing inputs are NOT_MEASURED, not a capture loop. |
| D: frozen acceptance | Lead freezes implementation / acceptance owner | Applicable format/build, Linux/Windows, Gallery/native, DPI/resize, theme/motion, resource/idle and budget checks | Reuse B/C evidence for same final binary; rerun affected checks after corrections | Plan approximately 16–24 unique static states; resource/motion budgets separate. Stop on systematic identity/state failures. |
| E: independent review | Candidate and manifest ready / Opus and Astra | Separate visual and systems decisions | Inspect immutable evidence; reproduce disputed/high-risk claims only | 0 captures by default; state the unanswered invariant before adding captures. |

Procedure: Lead defines invariant/owned files/proof; Worker completes coherent implementation; A then focused B; C enumerates actual outcomes; Lead freezes candidate; D runs once; failures form one bounded correction set; independent E reviews the exact final identity; Planner decides acceptance/merge.

Run full devctl functionality once. Preserve resource evidence with minimal additional lifecycle sessions rather than rerunning every semantic/comparator assertion. A proposed equivalent is 13 nominal captures: 11 in the functional/resource session plus one in each of two additional launch/shutdown sessions.

Motion should use bounded native frame sampling instead of repeated CLI process launches. First-frame theme proof must not rely on the current screenshot command's initial wait-idle barrier, which deliberately observes the settled state.

Planner owns scope, authority conflicts and acceptance. Lead owns decomposition, file ownership, freeze and acceptance coordination. Workers own bounded implementation. Astra owns systems/contracts/performance; Opus owns visuals/feel. One writer per mutable file set; parallelize disjoint work only.

Handoffs contain task/invariant, base SHA, owned files, changes, checks actually run, evidence/binary identity, open failures/deviations, stop condition and next owner. Carry finding IDs into correction tasks. Avoid reloading the entire review history or reopening proven non-defects without a concrete regression.

## I. Prioritized implementation backlog

### P0 — immediate correctness

- Finish active effective-palette transaction and complete-client invalidation.
- Establish framebuffer retention and full initialization after creation/resize/loss.
- Prove first post-switch presentation in both directions, not only endpoints.

Coordinate these as one rendering correction set. Do not assign a competing Worker to the same runtime/render files.

### P1 — before P12 acceptance

- Resolve Field authority, then fix the authorized alignment and internal spacing.
- Correct successful-present transition state and verify reversal/failure behavior.
- Reconcile motion-family coverage with the frozen recipes or explicit approved scope.
- Resolve material typography/font residuals individually.
- Obtain direct higher-DPI final-candidate evidence; classify unavailable cases honestly.
- Correct unsupported VERIFIED/ACCEPTED statements.
- Publish paired and hashed captures/snapshots/comparisons, including visible Fields.
- Obtain independent final Opus and Astra approvals.

### P2 — workflow/quality improvements

- Checked capture plan, actual-state preconditions, cache keys and manifest.
- Separate functional/resource/offline loops and bound retries.
- Lightweight epoch/frame sampling through existing instrumentation.
- Correct resize units and stale current docs.
- Shared file-budget scope, Python/extensionless/type-change coverage.
- Gate detailed diagnostic collection and profile redundant UIA/traversal work.

### P3 — future optimization

- Optimize traversal/allocations only after profiling.
- Add byte-aware text/peer cache accounting if larger workloads demonstrate need.
- Extract reusable transition policy when another component/backend needs it.
- Consider owner-authorized PR/server enforcement later; no CI implementation now.

## J. Proposed changes to repository instructions

No policy changes were applied. Suggested minimal additions to AGENTS.md:

> Before a page/state capture, verify the actual retained Gallery page, effective theme, interaction state, viewport and scale. A requested environment variable or output filename is not proof of running state.

> Capture once per verified binary/environment/state identity and reuse that image and paired snapshot for offline comparisons. Record attempts; stop on repeated unexpected semantic-state equality.

> Theme, motion, damage and presentation changes require a first-successful-presentation invariant test. Settled screenshots do not substitute for that proof.

> Acceptance evidence identifies source, executable hashes, features/profile, input hashes, command and result. Prior-candidate results remain historical.

> The task owner declares a bounded capture plan before native verification. Whole-suite and whole-Gallery repetition belongs at coherent acceptance boundaries.

> Verify core.hooksPath when establishing a Worker checkout. The staged hook and full-tree checker share the authored-file scope.

Recommended script/doc changes: retain the necessary orchestration from the transient sweep driver in one maintained script; assert running state and explicit WSL forwarding; persist input hashes/modes; provide owner-relative comparison using existing geometry; correct capture counts/current statuses/resize documentation. Do not change frozen reference artifacts or historical independent reviews.

### Universal Gates 1–20 assessment

This is an audit assessment, not certification of unexecuted acceptance checks.

| Gate | Assessment |
|---|---|
| 1 Completeness | P12 remains incomplete: theme, Field disposition, motion and evidence. |
| 2 Assumptions/approval | Field authority conflict needs explicit Planner disposition. |
| 3 Contract preservation | First-frame and framebuffer contracts require correction. |
| 4 Scope | Accepted Windows/native/reference boundaries remain appropriate. |
| 5 Minimal design | Bounded fixes are sufficient; no replacement framework needed. |
| 6 Ownership | Palette/presentation/displayed-state ownership needs tightening. |
| 7 Reuse | Keep shared geometry/classification; centralize capture and budget predicates. |
| 8 Fallbacks | No renderer replacement or new fallback justified. |
| 9 Determinism | First-frame and motion guarantees not fully proven. |
| 10 Authority | Current labels/documentation contradict artifacts in identified places. |
| 11 Failure safety | Resize/recreation and failed-paint displayed-state boundaries need work. |
| 12 Boundedness | Useful queue/cache limits exist; capture budgets and diagnostics need refinement. |
| 13 Dependencies | Inspected runtime/tooling isolation is appropriate. |
| 14 Performance | Redundant verification demonstrated; current runtime metrics insufficient. |
| 15 Persistence | No new database, daemon or central service needed. |
| 16 Verification | Targeted presentation and internal-part proof missing despite broad reported tests. |
| 17 Recovery | Batch known fixes; avoid whole-sweep repetition and unrelated refactors. |
| 18 Identity | P12 binary/input provenance incomplete. |
| 19 Honesty | Correct unsupported completion/acceptance labels and aggregate claims. |
| 20 Systemic scope | Fix the rendering invariant across paths, not one control or screenshot. |

There is no Gate 21.

## K. Final handoff to Planner

**Top five findings:** theme ordering/global damage; framebuffer preservation/recovery; Field authority conflict; motion displayed-state/coverage gaps; expensive yet incomplete evidence attribution.

**Immediate actions:** keep the active Worker on the coordinated rendering correction; include target lifetime and first-presentation proof; resolve Field authority explicitly; require paired/hash-bound affected-state evidence; correct current statuses before final review.

**Lead implementation:** bounded P0/P1 fixes followed by checked capture orchestration and offline reuse, using existing runtime/scheduler/devctl/comparison infrastructure.

**Preserve:** native Win32/D2D/DWrite/RichEdit architecture, generational identity, typed styles, frozen reference, desktop responsiveness, zero-idle objective, and independent visual/systems review ownership.

**Approvals:** Opus for final visual fidelity/feel; Astra for rendering contracts/failure semantics/performance/evidence; Planner/User for Field disposition and merge authorization.

**Merge recommendation: DO NOT MERGE P12 at `cb6d888207321a2b724e68c302175661245ee00e`.** Review the corrected frozen candidate. This audit does not accept future Worker changes.

### Publication verification

The isolated review branch starts exactly at the audited P12 SHA. Its only additions are this report and the explicitly proposed workflow document. All A–K sections and local file links were checked; `tools/check-file-tokens.sh` passed. Runtime tests and screenshot collection were not repeated for this documentation-only publication. The active P12 checkout and remote were rechecked before publication and still identified `cb6d888207321a2b724e68c302175661245ee00e` with a clean local worktree.

The containing Git commit identifies the report revision; it does not replace the audit base or production-source identity in the opening table. The final publication handoff supplies the verified remote commit and report URL.

```text
AUDIT_VERDICT: CRITICAL_BLOCKERS
NEXT_OWNER: rust-ui Planner
MERGE_AUTHORITY: NONE
```
