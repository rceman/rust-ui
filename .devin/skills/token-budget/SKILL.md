---
name: token-budget
description: File-size gate — run after every task/campaign/goal completes. Verifies every source file stays under 5000 tokens (tokens = bytes/4 = 20000 bytes) and refactors oversized files via mechanical module splits.
---

# Token Budget Gate

**Budget: 5000 tokens per file.** Estimate = `bytes / 4` → **20000 byte limit**.

Runs automatically at every stop via `.devin/hooks.v1.json` →
`tools/token-budget-gate.sh`. When invoked, work this procedure:

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
task/campaign/goal completion boundary — the Stop hook is only a
reminder, not permission to hijack an unrelated task):

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

The Stop hook re-flags remaining violations on the next turn end —
acknowledge consciously deferred files to the user instead of letting the
gate nag silently.
