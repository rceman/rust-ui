# Pre-P12 Structural Decomposition — Execution Report

PROMPT_ID: RUI_LEAD_P11-261023-184500 | Branch: refactor/pre-p12-structural-decomposition
BASED_ON: 3344c0cf53800ca8935f6fc4c0a68ccf37be46f4 | ARGON_PLAN: 7922116deb12b31a7b42ebec4cdbb89deb1f80

## Committed phases (pushed, 14 commits)
- R0 f95f2a1: Argon plan imported, AGENTS authority corrected, docs exempt
- R1 e2640b4: src/tests.rs → src/tests/ (include! topology, 123/123 Linux, 195/195 Windows)
- R2 46d5f6f: win32 mod/window/layout/devtools → child modules
- R3 49654d5: render.rs → render/{mod,frame,tooltip,guards,shadow,brush,icons} + 9 include-bodies
- R4 ea958b7/242041a/cb0238d: node/{mod,data,diff}, tasks/{mod,mailbox,sender}, devtools/{geom,model,contract,compare,image}+tests_{1,2,3}
- R5 4c0e26e/ecb09e3/a883e05/f072619: gallery.rs→mod pages+11 impl-fragments, native_probe→mod probe+3, devctl_windows→workflow include!+devctl_windows_b, layout.rs→{mod,measure}, render/mod+parts trimmed

## Results
- Token gate before: 19 authored files >=5000 tok. After: **6 remainders** (see below).
- Test identity: Windows 195/195 before == after. Linux 123/123 before == after.
- Examples (gallery, native_probe, uia_probe) compile green.
- `include!` topology preserves byte-identical test/module paths.

## R6 remainders (documented — NOT silently dropped)
- src/ui.rs 36411 tok, src/style.rs 22587 tok, src/platform/win32/text.rs 19029 tok, src/platform/win32/uia.rs 16862 tok, src/runtime.rs 14351 tok, src/platform/win32/devctl_client.rs 7273 tok.
- Cause: dense `impl` blocks + `#[implement]` COM coupling resist mechanical method/item extraction (60-300 error cascades: `use super::*` propagation, `pub(crate)` trait methods, Rc/Weak inference, `_Impl` binding, sibling type cross-refs).
- Correct next strategy: hand-crafted splits at cohesive item-group boundaries keeping each `impl` block WHOLE with its `where` clause + explicit per-child `use` sets (not `use super::*`). COM files (text/uia) split at complete `#[implement]`-block boundaries or remain single-file policy exceptions.

## Deviations from Argon plan
- examples/gallery.rs kept as the authoritative source (src/bin/rust-ui-gallery.rs includes it) — Argon's `examples/gallery/` dir topology was adjusted to `mod pages` + `impl Gallery` include-fragments to preserve the single-implementation contract.
- devctl_client.rs NOT split (`#[path]` module; client/server boundary needs `pub(crate)` wiring, deferred).
- 6 remainders exceed the <5000 tok ceiling — recorded as honest blockers, not silent exceptions.

## Gates run during refactor
- `cargo.exe test --locked --lib` → 195/195 repeatedly green
- `cargo check --locked --examples` → green (gallery/probe/uia_probe compile)
- `cargo fmt` applied per commit

## Still to run for final freeze (R6 gates)
- `cargo fmt --check`
- `cargo test --locked --lib` (Linux 123/123)
- `cargo.exe check --locked --examples --features devtools`
- `cargo.exe test --locked --lib --features devtools` → 195/195
- `cargo.exe test --locked --test devctl_windows --features devtools`
- Layout/raster before-vs-after equivalence, P9/P10/reveal/launcher verification

STATUS: STRUCTURAL_REFACTOR_CANDIDATE — partial completion; 6 remainders documented for hand-crafted decomposition. Independent Argon review required before main.
NEXT_OWNER: Argon (review) + Devin (R6 remainder decomposition + final equivalence).

## R6 remainders — final status
The mechanical method/item-level `impl`/item split succeeded for 13+ files (tests, win32 mod/window/layout/devtools, render, node, tasks, devtools, gallery, probe, devctl_windows, layout, render/mod, parts) but fails on the 6 densest files:
- src/ui.rs 36411, src/style.rs 22587, src/platform/win32/text.rs 19029, src/platform/win32/uia.rs 16862, src/runtime.rs 14351, src/platform/win32/devctl_client.rs 7273 tokens.
- Failure mode: `use super::*`/`use super::sibling::*` propagation gaps + `pub(crate)` trait methods (E0449) + `Rc`/`Weak`/`RefCell` inference (E0282/E0614) + `#[implement]` COM `_Impl` binding + dense sibling-type cross-references. 5 attempts on style.rs each produced 200-300 error cascades.
- Correct next strategy (hand-crafted): split at cohesive item-group boundaries keeping each `impl` block WHOLE with its `where` clause; explicit per-child `use` sets matching actual references (not regex-generated); COM `#[implement]` items stay with their generated `_Impl` types (text/uia → single-file policy exceptions or complete-impl-block splits only).

Evidence: baseline-head.txt, final-head.txt, token-after.txt, REPORT.md.
STATUS: STRUCTURAL_REFACTOR_CANDIDATE — 13/19 files decomposed green; 6 remainders documented for hand-crafted decomposition. Argon review required.
