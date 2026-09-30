# Astra Handoff — rust-ui Architecture Concept

Work on:

`rceman/rust-ui`

Branch:

`agent/architecture-concept-astra`

This is a **docs-only architecture task**.

Do not implement rust-ui yet.

Do not migrate/copy the existing Mascot UI code into this repository.

The point of the task is to decide the programming model first, without being anchored by accidental details of the current Windows prototype.

Read first:

1. `README.md`
2. `docs/RUST_UI_ARCHITECTURE_CONCEPT_TASK.md`
3. `docs/MASCOT_PROVEN_CONTEXT.md`

Then research and design the concept described by the task.

You may inspect existing Rust UI frameworks and Vue/Svelte/Solid concepts for architectural ideas.

You may inspect `rceman/mascot` only when a concrete proven requirement needs clarification. Do not spend the task auditing or refactoring Mascot implementation code.

The central question is:

> What is the smallest, clearest, strongly typed native Rust UI programming model that gives application authors Vue/Svelte-like productivity and dynamic composition without importing browser/DOM/CSS/VDOM complexity?

Compare at least:

- fluent builder/listeners;
- typed message/update;
- immediate declarative;
- fine-grained signals;
- declarative macro/DSL;
- a minimal hybrid.

Be concrete. The final recommendation must include realistic Rust API examples, state/event ownership, async behavior, native TextInput integration, custom rendering extension points, layout, styling, motion and Windows/macOS backend boundaries.

Optimize for:

- low consumer boilerplate;
- low source/token cost;
- agent friendliness;
- deterministic semantics;
- narrow invalidation;
- no idle render loop;
- native text/input quality;
- testability;
- maintainable implementation complexity.

Do not optimize for:

- preserving current Mascot code;
- compatibility with an unreleased internal API;
- implementing every shadcn component;
- framework novelty.

Required deliverables are listed in `docs/RUST_UI_ARCHITECTURE_CONCEPT_TASK.md`.

Finish by committing and pushing documentation to the same branch.

Do not add implementation crates.

Do not add CI.

End the report with exactly one of:

`RUST_UI_ARCHITECTURE_CONCEPT_COMPLETE`

or

`RUST_UI_ARCHITECTURE_CONCEPT_BLOCKED: <reason>`
