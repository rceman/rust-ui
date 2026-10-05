# Repository instructions

## Architecture and scope

Product scope is canonical in `docs/PRODUCT_SCOPE.md`: rust-ui is desktop-only and fully desktop-responsive; mobile is out of scope (responsive != mobile). `docs/COMPONENT_SUPPORT.md` is the durable component support ledger and must be updated in every component milestone. UI milestones use the split review in `docs/REVIEW_AUTHORITY.md`.

The approved architecture baseline for the Windows spike is `29c67025dc88221899a8ab1b381f08491d44b85b` (the Astra documents define the provisional architecture; later baselines documented in task handoffs supersede older SHAs as authority). The current implementation/rework candidate is the branch HEAD. The owner approved a Windows implementation spike, not a final framework or a Mascot migration.

The current spike is Windows-only. Do not add other-platform backends or stubs, a custom editor, a browser runtime, compatibility shims, a mandatory async runtime, or CI. Native editable text must remain windowless RichEdit through `ITextHost` / `ITextServices`, with `TxDrawD2D`; HWND editor substitution is not an approved fallback.

### Platform contracts

rust-ui core semantics are platform-neutral; Windows is the current backend, not the semantic definition. Shared platform responsibilities have explicit semantic contracts documented in `docs/PLATFORM_CONTRACTS.md`, and each backend provides exactly one authoritative implementation. Consumers must not duplicate platform semantics; native handles/messages never leak into shared contracts (no `HWND`/`WM_*`/`RECT`/`WPARAM` in `src/` outside `platform/`). Validation scripts orchestrate only — Rust (`examples/native_probe.rs`, conformance tests in `src/tests.rs`) owns reusable native semantics. Do not implement macOS during the current Windows foundation milestone.

Keep native handles private. Application state changes in `update`; views stage a retained keyed transaction. Native node events and worker deliveries require generation fencing. Required native behavior must be validated on Windows, not inferred from fake-peer tests.

## Windows authority

The authoritative checkout is `W:\devin_folder\rust-ui`. The spike branch is `agent/windows-native-composer-contract-spike-v0.1-swe2`. Build, run, render, profile, review, commit, and push from this native Windows checkout. Do not open a PR for the spike.

## Verification

Available commands:

- `cargo check --examples`
- `cargo build --examples`
- `cargo test --lib` for deterministic core tests
- `cargo fmt --check`
- `cargo run --example composer` for the native executable

Use affected tests and short native smoke checks during iteration. A compiling native backend is not proof of input, IME, UIA, rendering, or idle behavior. Keep deterministic correctness, visual review, and performance measurements separate. Run the concise final measurement pass only after the candidate is frozen. Report unavailable keyboard/IME/display validation honestly.

The owner's Universal Gates 1–20 are authoritative and are canonically defined in this repository at `docs/QUALITY_GATES.md` — there is no Gate 21 and no other numbered gate taxonomy. `docs/QUALITY_GATES.md` is self-contained; rust-ui does not require any other local repository to know its gates. Formatting, tests, visual QA, native validation, performance checks, and evidence collection are verification mechanisms used to satisfy the gates, not additional numbered gates. The external Mascot repository may be consulted only as historical/reference implementation evidence where explicitly needed; Mascot-specific tooling and assets are outside this repository's change cone. Do not copy its public UI API or substitute an unrelated tool's checks for native rust-ui evidence.

Required spike findings belong in `docs/SPIKE_ARCHITECTURE_DEVIATIONS.md` and `docs/SPIKE_API_REVIEW.md`; use actual compiling consumer snippets and demonstrated deviations. Keep final evidence compact under `benchmark/results/windows-native-composer-spike-v0.1/`. Evidence must identify its exact code candidate. Do not report COMPLETE until applicable gates and required evidence pass; report concrete blockers instead of weakening contracts.
