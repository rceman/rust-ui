# Windows Native Composer Contract Spike v0.1 — evidence

**Candidate:** branch `agent/windows-native-composer-contract-spike-v0.1-swe2`, final SHA in commit trailer.
**Environment:** Windows 11, monitor at 125% scale (PerMonitorV2), native `cargo build --examples` debug build.
**Capture method:** `PrintWindow` of the live composer window; UIA through `UIAutomationClient`; native semantics through `examples/native_probe.rs` (JSON-emitting Rust harness). PowerShell (`native.ps1`, `collect.ps1`) is orchestration only — build/run/PID lifecycle/scenario sequencing/artifacts.

## Artifacts

- `composer-dark.png` / `composer-light.png` — theme states; `final-foundation.png` — final candidate after the platform-contract migration (all rows inside pills, draft peer shows the real IME + pointer evidence inline)
- `evidence-final-geometry.json` — PMv2 physical-px window/client geometry (dpi=120, scale=1.25)
- `evidence-final-uia.json` — UIA tree with physical-px bounding rects (draft, three rows, all buttons)
- `evidence-final-value.json` — draft committed text: `draft-prefillacceptありがと` — real SendInput keystrokes and real MS-IME hiragana composition+commit
- `send-mid.png` / `send-done.png` — stream chunks, send disabled + stop enabled mid-flight, re-enabled at completion
- `typing.png`, `unicode.png`, `readonly.png`, `selection.png`, `undo.png` (+`.txt`) — peer input/selection/undo evidence
- `uia-tree.txt`, `live-tree.txt`, `disabled.txt` — tree/enablement evidence
- `perf.txt` — idle: ~0% CPU, bounded threads

## Verification status (this candidate)

- `cargo test --lib` — **81/81** (includes the `geometry_contract_*` conformance suite at 96/120/144/192 DPI and the PMv2 RichEdit paint probe with a live scale-change leg)
- `cargo build --examples` — clean; `cargo fmt --check` — clean
- `native_probe geometry|uia|click|type|ime|value|dpichange` — all exit 0 on this candidate

## What is proven (measured)

- Retained + keyed staged UI tree, windowless RichEdit via `ITextHost2`/`ITextServices2`/`TxDrawD2D`
- **Coordinate contract:** logical DIP retained side, physical px at the Win32/RichEdit/UIA seam — typed in `crate::geom` (`ScaleFactor`, `Logical*`/`Physical*` spaces, round-half-away), Win32 seams in `platform/win32/space.rs`; probes/harness reuse the same authority
- `native_probe_richedit_paints_text` — PMv2-aware, ink inside the requested island at 96/120/144/192 with zero stray pixels, plus the live scale-change leg (`apply_bounds` A→B → ink inside the B island, no ghost at A)
- Real SendInput keystrokes reach the focused peer; mid-text click places the caret at the clicked glyph (insert at measured px offset)
- **Real IME:** MS-IME (TSF) hiragana composition + Enter commit lands `か`, `よ`, `ありがと` in the draft editor — measured, not synthetic
- Synthetic `WM_DPICHANGED` via `SendMessageW` (PostMessage refuses cross-process pointer lparams): the handler applies the suggested rect and relayouts — regression evidence only
- UIA bounding rects in physical screen px for every control; disabled nodes report `IsEnabled=false`
- Theme flip honored live; idle at ~0% CPU; event-driven (no frame loop)

## Honest limits

- **Real monitor DPI transition** — not exercised; the display stayed at 125%. Synthetic `WM_DPICHANGED` proves the adapter path only; a real monitor/scale move is still required for full native acceptance of the transition.
- **Dead keys** — the en-US layout has no dead keys and no international layout is installed; real dead-key acceptance could not be exercised. Real CJK IME composition is validated instead.
- `ElementProviderFromPoint` — unit-tested; a real `FromPoint` probe against the foregrounded window is not exercised in this pass.
- Caret visibility in `PrintWindow` captures is unreliable (screen-level XOR caret); caret geometry is validated semantically via click→insert-position.
- Multi-line growth beyond the scratch activation extent is covered by the REQRESIZE path but not by a dedicated tall-document capture.
