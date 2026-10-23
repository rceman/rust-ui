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

Splits are **moves, not redesigns**: relocate contiguous marked sections
into a `<name>/` module dir, keep `pub(crate)` visibility, re-export where
needed. Standard targets (worst-first):

| File | Split |
|---|---|
| `src/tests.rs` | `src/tests/{mod,fake_peers,executor,node,runtime,style,layout,win32_reentrancy,devctl,uia,text}.rs` |
| `src/platform/win32/mod.rs` | `win32/{backend,peer,popup,syscolor,run,turn}.rs` |
| `src/ui.rs` | `ui/{mod,semantics,container,text,button,form,nav,overlay,canvas}.rs` |
| `src/platform/win32/render.rs` | `render/{mod,dwrite,chrome,richedit,canvas,tooltip,shadow}.rs` |
| `examples/gallery.rs` | `examples/gallery/{mod,pages/*,state,nav}.rs` (keep `include!` wiring working) |
| `src/style.rs` | `style/{mod,color,geometry,box_style,text_style,visual_style,patch}.rs` |
| `src/devtools/mod.rs` | `devtools/{mod,codec,rect,compare}.rs` |
| `src/platform/win32/text.rs` | `text/{mod,host,msftedit,d2d,ime,measure}.rs` |
| `src/platform/win32/uia.rs` | `uia/{mod,provider,events,rects,richedit_peer}.rs` |
| `src/runtime.rs` | `runtime/{mod,transaction,focus,schedule,pump}.rs` |
| `src/platform/win32/window.rs` | `window/{mod,wndproc,messages}.rs` |

Docs over 5000: prefer splitting a `docs/<topic>/` index+sections only
when a doc is actively edited; review/freeze artifacts may stay (note the
exemption choice to the user rather than splitting blindly).

## 3. Verify after every split

```bash
cargo fmt --check && cargo check --locked --examples && cargo test --locked --lib
```

Keep each moved section byte-identical — no "cleanup while moving"
without explicit ask.

## 4. Re-run until green (or report the honest blocker)

The Stop hook will re-flag remaining violations on the next turn end —
acknowledge consciously deferred files to the user instead of letting the
gate nag silently.
