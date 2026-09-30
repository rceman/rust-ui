# Repository instructions

## Architecture and scope

The Astra documents at architecture baseline `768eb172509a45ed15cdf5156fe57268ccaf8b08` define the provisional architecture. The owner approved a Windows implementation spike, not a final framework or a Mascot migration.

The current spike is Windows-only. Do not add other-platform backends or stubs, a custom editor, a browser runtime, compatibility shims, a mandatory async runtime, or CI. Native editable text must remain windowless RichEdit through `ITextHost` / `ITextServices`, with `TxDrawD2D`; HWND editor substitution is not an approved fallback.

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

The owner's Universal Gates 1–20 remain authoritative; their canonical source supplied for this task is `W:\devin_folder\mascot-ui-v01\docs\QUALITY_GATES.md`. Mascot-specific tooling and assets are outside this repository's change cone. Do not copy its public UI API or substitute an unrelated tool's checks for native rust-ui evidence.

Required spike findings belong in `docs/SPIKE_ARCHITECTURE_DEVIATIONS.md` and `docs/SPIKE_API_REVIEW.md`; use actual compiling consumer snippets and demonstrated deviations. Keep final evidence compact under `benchmark/results/windows-native-composer-spike-v0.1/`. Evidence must identify its exact code candidate. Do not report COMPLETE until applicable gates and required evidence pass; report concrete blockers instead of weakening contracts.
