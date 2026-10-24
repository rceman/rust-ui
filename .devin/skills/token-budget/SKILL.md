---
name: token-budget
description: File-size gate — run after every task/campaign/goal completes. Verifies every source file stays under 5000 tokens (tokens = bytes/4 = 20000 bytes) and refactors oversized files via mechanical module splits.
---

# Token Budget Gate

**Budget: 5000 tokens per file.** Estimate = `bytes / 4` → **20000 byte limit**.

Enforced locally by `.githooks/pre-commit` (staged authored blob >=
20000 bytes blocks the commit; activate per-checkout with
`git config --local core.hooksPath .githooks`). `tools/check-file-tokens.sh`
is the manual full-tree gate — run it at every task/campaign/goal
completion boundary, and work this procedure when it reports violations:

## 1. Measure

```bash
tools/check-file-tokens.sh            # exit 1 + FAIL list (worst first)
tools/check-file-tokens.sh src 20000  # scope/override if needed
```

Only text sources count (`.rs`/`.md`/`.toml`/scripts); `benchmark/`,
`reference/`, `.devctl/`, `assets/` binaries are excluded by design.

## 2. Pick the worst offender — refactor mechanically

**`docs/**` is exempt** (single-purpose specs + immutable review records).

Splits are **moves, not redesigns**: relocate contiguous marked sections
into a `<name>/` module dir. Prefer **parent-declared types + child impl
modules** (`struct X` stays in `mod.rs`, method groups move to
`mod.rs`'s children so fields stay private). For test files use
`include!("slice.rs")` inside the parent `mod.rs` so `cargo test --list`
paths stay byte-identical.

Procedure for each file >=5000 tokens (work through at a coherent
task/campaign/goal completion boundary):

1. Read the file's `// =====`/`//!` section markers and type/impl layout.
2. Choose cohesive responsibility boundaries (never a stale filename map).
3. Move sections to `<stem>/<child>.rs`; keep `pub(crate)`/`pub(super)`
   minimal; add `pub use` re-exports in `mod.rs` for public API stability.
4. Verify: `cargo fmt --check`, `cargo check --locked --examples`,
   `cargo test --locked --lib`.
5. Re-run `tools/check-file-tokens.sh` until exit 0 — target <4500/file.
6. If a mechanical move uncovers a latent bug, record it — do NOT fix it
   inside the refactor.

## 3. Re-run until green (or report the honest blocker)

The Git pre-commit hook blocks any staged oversized authored file
immediately; the full-tree checker catches anything staged before the
hook was activated. Acknowledge consciously deferred files to the user
instead of letting the gate nag silently.
