# Pre-P12 Structural Refactor — Final Report

- PROMPT_ID: RUI_LEAD_P11-261023-184500
- ROLE: Lead
- PROJECT: rceman/rust-ui
- BRANCH: refactor/pre-p12-structural-decomposition
- BASED_ON: 3344c0cf53800ca8935f6fc4c0a68ccf37be46f4
- ARGON_PLAN: 7922116deb12b31a7b42ebec17c4dbb89deb1f80
- ARGON_REVIEW: 9a88af28f873d41e98f6f33c423d97f750b60ff3
- VERIFIED_CODE_SHA: a56f727301f399fefcc86f155d7d8840e7b970bc

## Result — STRUCTURAL_REFACTOR_CORRECTED_CANDIDATE_READY_FOR_ARGON_REVIEW

All 19 god-files decomposed; the Argon independent review's F1–F4 blockers
are corrected; a local Git pre-commit hook now enforces the token budget at
commit time.

### Token gate
- `tools/check-file-tokens.sh` → **exit 0, 0 authored files >= 5000 tok**
  (was 19). Exactly-20000-byte files now FAIL (strict-> to +N-1 fix);
  `set -o pipefail` so a broken pipeline can't false-PASS.
- `.githooks/pre-commit` blocks any STAGED authored blob >= 20000 B.
  Activate per-checkout: `git config --local core.hooksPath .githooks`
  (not propagated by clone; `--no-verify` can bypass; no server-side gate).

### Decomposition (see docs/STRUCTURAL_REFACTOR_PLAN.md + Argon review)
- tests/, win32 mod/window/layout/devtools/render, node/tasks/devtools/
  layout/runtime → cohesive child modules.
- gallery/native_probe/devctl_windows → mod/impl/include! fragments.
- devctl_client → client+server `#[path]` siblings.
- style/ui/text/uia → mod + include! item sequences (flat crate::X::*).
- F2 renames: ui/{container_props,control_props,tx_staging,ui_containers,
  ui_primitives,ui_controls,builders_{label_button,text_custom,
  action_popup},scope_builders_{nav,settings},builders_input};
  style/{palette_tokens,recipes_button,recipes_surface_display,
  recipes_forms_nav_overlays,validate}; text/peer_{format_create,
  draw_measure,selection_lifecycle}; runtime/{pump_review,
  reconcile_nodes,dispatch_conflict,peer_sched}; probe/{input_window,
  uia_inspect,ime_japanese}. ButtonBuilder Drop + ActionBuilder cohesive.
- F3: native_resize_retained_layout_converges → tests/devctl_windows/
  resize.rs (devctl_windows integration 2/2); devctl_windows_b.rs removed.

### Equivalence (evidence in this dir)
- Test identity: Linux 123/123, Windows 195/195, devctl integration 2/2 —
  test-identity-{linux,windows}.diff = 0 lines (IDENTICAL).
- Public API: lib.rs `pub` items + `rust_ui::prelude::*` (11 items)
  IDENTICAL — 0 new/removed/changed paths.
- Deterministic 12-state clean-room comparison (equivalence.json):
  **12/12 PASS — 0 semantic tree diffs, 0 geometry diffs, 0 part
  visibility diffs, 0 mismatched pixels** on every state. Identical
  capture script run against BEFORE `3344c0c` (before_clean/) and AFTER
  branch HEAD (after/) in back-to-back fresh Gallery processes
  (RUI_GALLERY_PAGE boot page + resize + click shell.theme.<t> last +
  wait-idle). Original before/ retained as historical evidence.
- Same-SHA same-session repeat capture: 0 mismatched pixels — protocol
  is deterministic.
- P9 held resize: green (2/2 devctl test). P10 cross-monitor: structurally
  preserved — physical multi-monitor runtime testing unavailable in the
  single-display environment; code-path preservation independently
  accepted by Argon.
- Cloak/reveal + launcher: preserved — behavioral code moved verbatim;
  devctl E2E launch green.

### Commits (R0–R6 + corrections)
R0 f95f2a1 | R1 e2640b4 | R2 46d5f6f | R3 49654d5 | R4 cb0238d |
R5 a883e05 | R6 19bd3c9 | F1 dd25b48 | F2+F3 4f745c0 | E 86feffe | F bd4d8c5 | D-evidence 92821c7 | delta-scripts a56f727 | final-evidence <this commit>

### Deviations from Argon plan
- `include!` item-sequence topology for the 6 densest files (mechanical
  method-level split produced 60–300 error cascades).
- devctl_client → client+server `#[path]` siblings (Argon: child modules).
- Git pre-commit hook replaces the Devin Stop hook (durable, per-checkout,
  no LLM dependency).
- Delta-review script fixes: `.githooks/pre-commit` gained `*/target-*/*`
  exclusion parity; checker prints `5000 tokens` on a clean tree.
- Deferred observation (pre-existing, NOT introduced/fixed by this
  refactor): unkeyed Box `part_key` reuse (`"list"`) after visiting
  Navigation in a shared Gallery process — fresh-process-per-state
  capture eliminates it from evidence; runtime semantics unchanged.

### Intentionally deferred bugs
None introduced. All known P12 visual differences (Input/placeholder
alignment, disabled colors, Segoe-vs-Geist, selection rendering, Textarea
typography, Select movement, SettingsNav clipping, hover fade-out)
preserved untouched — P12 owns them.

STATUS: STRUCTURAL_REFACTOR_FINAL_CANDIDATE_READY_FOR_ARGON_APPROVAL
NEXT_OWNER: Argon
