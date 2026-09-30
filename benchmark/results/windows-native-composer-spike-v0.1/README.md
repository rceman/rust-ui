# Windows Native Composer Contract Spike v0.1 — evidence

**Candidate:** HEAD on `agent/windows-native-composer-contract-spike-v0.1-swe2`
**Environment:** Windows 11, 125% display scale (PerMonitorV2-aware), native `cargo build --example composer` debug build
**Capture method:** `PrintWindow` + `GetWindowRect` of the live composer window; UIA via `UIAutomationClient` managed client.

## Artifacts

- `composer-dark.png` — system/dark theme, all controls resolved (send shows the surgical red bottom border; custom action has typed shadow)
- `composer-light.png` — after `light` UIA-invoked `SetTheme(Light)` — editor text follows the resolved palette live (peers re-read `Theme.dark`); same layout, no remount
- `uia-tree.txt` — the raw-tree enumeration of the live window: label, three `RichEdit Control` Edit peers, every button, the custom action, the image semantics node
- `perf.txt` — idle process after 2.5s settle: `0.00% CPU` over a 3s window, `35.6 MB` working set, 20 threads (executor + timer + text-service workers)

## Verification status (this candidate)

- `cargo check --all-targets` — clean (warnings: unused/dead seams only)
- `cargo test --lib` — **69/69**
- `cargo build --example composer` — clean
- Native runtime — real Win32 window; windowless RichEdit peers with UIA providers; typed input lands; disabled nodes report `IsEnabled=false` and skip hit-test/focus; `ElementProviderFromPoint` hit-tests snapshot rects because msftedit's windowless provider reports an unusable bounds (Infinity) — deviation noted in `docs/SPIKE_ARCHITECTURE_DEVIATIONS.md`

## What is proven

- Retained + keyed staged UI tree: `ui.keyed`, `ui.group`, conditional sibling sections
- Windowless RichEdit through `ITextHost`/`ITextServices`/`TxDrawD2D` — real typing (`WM_CHAR`), focus, selection, IME message plumbing
- FAST/CUSTOM/SURGICAL styling: recipe default buttons, fully-styled action tile with `Shadow`, surgical `border_bottom` patch on `send`, live `TextStylePatch` restyle on a mounted editor
- Disabled = inert chrome: no hit-test target, no hover/press/focus, UIA `IsEnabled=false` + `IsKeyboardFocusable=false`
- Task/mailbox: `send -> stream chunks -> stop -> resend` — late chunks fenced by generation
- Theme: `Theme::resolve(mode, appearance)` — runtime flip is honored by paint and by every mounted peer
- Event-driven idle: 0% CPU at rest (no frame loop)
- DPI: PerMonitorV2 + `WM_DPICHANGED` — 125% rendering verified; DIP geometry stays correct

## Not validated (report honestly)

- IME composition window never appeared during this session — `WM_IME_*` messages are forwarded to the focused peer, but a real CJK/hangul IME interaction could not be exercised end-to-end
- `ElementProviderFromPoint` was validated by unit test only — the real `AutomationElement::FromPoint` call could not be made against a backgrounded window in this headless shell
- Single-display DPI coverage: 125% validated; 100/150/200 inferred through the same code path (DIP-first layout, `WM_DPICHANGED` re-layout + re-paint), not independently screenshotted
- multi-line editor height growth beyond the scratch activation rect is untested — `natural_size` activation latching is documented in the deviations doc
