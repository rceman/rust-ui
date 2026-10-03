# Spike architecture deviations — composer contract v0.1

**Candidate:** final working tree on `agent/windows-native-composer-contract-spike-v0.1-swe2`
**Baseline:** Astra architecture `29c67025dc88221899a8ab1b381f08491d44b85b`

Deviations discovered while implementing the Windows backend against the
approved architecture. Each entry is a real, demonstrated behavior of the
native platform that the documented design did not fully predict — these
constrain what the final API must promise.

## 1. `PushAxisAlignedClip` must not wrap peer draws

**Deviant:** the renderer wraps retained drawing in a
`PushAxisAlignedClip`/`PopAxisAlignedClip` pair around each painted node;
wrapping that clip around a windowless-RichEdit `TxDrawD2D` call produces
**zero output** (the call returns `Ok`, nothing paints).

**Root cause:** msftedit's D2D path uses its own device-context state and
ignores the caller's pushed axis-aligned clip; the clip rect intersects the
peer's formatted region and discards everything.

**Implementation consequence:** `NodeData::Editor` draws without a pushed
clip; confinement comes from the peer's own view bounds. This is
observably correct — text paints inside its pill, not outside.

**Contract implication:** the public contract must say "native peers draw
in their own bounds; compositor clips don't reach them" — i.e. peers are
final leaf surfaces, not clip children.

## 2. `TxDrawD2D` anchors unreliably into a shared hwnd target — peers render to their own bitmap

`ITextServices2::TxDrawD2D` takes `lprcBounds` as the destination rect.
Empirically the bounds rect DOES translate the anchor (moving `top` moves
the drawn text) but the content lands offset from it — by roughly one
line height above `bounds.top` — for peers whose format space was latched
by measurement before real layout bounds existed. Neither scroll messages
(`EM_SETSCROLLPOS`, `EM_LINESCROLL`), `EM_SETRECTNP`, re-activation, nor
`TxGetViewInset` reliably reset that anchor.

**Resolution (current — supersedes the earlier peer-bitmap path):** the
peer's format space is latched in HOST PHYSICAL PX (deviation 10) and
`TxDrawD2D` draws directly into the shared frame target at the peer's
physical-px bounds; the logical->px boundary crossing happens once via
`PeerOrigin`. The earlier peer-local-bitmap compositor worked around the
wrong unit, not the anchor; it is removed. `TxGetViewInset` stays zero;
padding between pill chrome and text lives in the bounds inset.

**Contract implication:** a native text peer is a *leaf surface*, not an
inline draw — the compositor positions its rectangle; the peer's internal
origin is always `(0,0)`. This is also the shape a future
DirectComposition visual-per-peer path wants.

## 3. Activation latches the format space — updated by the F04 rework

`OnTxInPlaceActivate` binds the format space in the host's **physical
px** units — the scale relatch defect (ink drawn one row displaced after
a scale change) proved the latched view does NOT follow the DIP bounds.

**Current contract (supersedes the earlier "one activation forever"
rule):** activation is an explicit three-state machine
(`Inactive`/`InPlace`/`Ui` — in-place activation covers measure/draw;
UI activation is focus-owned). A peer activates lazily at first
non-empty bounds and, on a **scale change** outside composition,
deactivates and re-activates at the new px mapping — the native editing
state (committed text, directional selection, undo history) survives
because the `ITextServices` COM object is retained; the selection
anchor/focus pair is additionally snapshotted and restored because
deactivation is free to collapse it. **During an active composition a
bounds/scale change is instead DEFERRED** (`pending_relatch`): every
consumer — measure, draw, pointer/caret mapping, host callbacks — keeps
reading the one effective geometry actually painted by the native peer,
and no deactivation runs mid-composition. When `WM_IME_ENDCOMPOSITION`
arrives, the peer drains its notifications, queued native edits are
acknowledged into the shared mirror, remaining divergence is reconciled
and acknowledged, stale proposals are resolved against the true
committed revision, and only THEN does `finish_pending_relatch` apply
the deferred geometry (composition state survives because no relatch
ran during it). `natural_size` activates with a tall scratch height
because `EM_REQUESTRESIZE` requires an activated service and reports the
natural extent through the host callback; a request that produces no
fresh notification is an error, never stale success.

**Contract implication:** an editor's activation is not "create-time";
it is "first real bounds", and scale transitions re-latch explicitly.

## 4. Synchronous reentrancy: `send` must defer through the backend

`peer.borrow().send(WM_*)` enters msftedit; host callbacks
(`TxSetFocus`/`TxInvalidateRect`) synchronously re-enter the backend
(`WM_SETFOCUS` → `be.turn()`) while the `RefCell` borrow is still held —
a `RefCell` panic if a second borrow is attempted.

**Consequence:** all peer sends funnel through `Backend::send_native`,
which guards `in_turn` and defers via a `deferred_native` queue; deferred
deliveries happen in `service_peer_events` with the same `NodeId`
generation check. `WM_KEYDOWN`, `WM_CHAR`, `WM_LBUTTON*` (and the other
message classes that reach a peer) all go through this path.

**Contract implication:** "the platform layer forwards raw Win32 messages
to peers" is not the contract — the contract is "peer delivery is
generation-checked and reentrancy-safe", which the deferred queue proves.

## 5. Native UIA providers are strong, not weak

`UiaRoot::rebuild` originally stored `IRawElementProviderFragment` via
`.downgrade()` for every child — classic COM objects (the msftedit
provider) do not implement `IWeakReferenceSource`; `.downgrade()` returns
`E_NOINTERFACE`, which silently collapsed the whole child list.

**Consequence:** the live child order holds `Weak` for generated
`PaintFragment`s and strong `IRawElementProviderFragment` for native
peers; both are generation-fenced through `NodeId.generation`.

**Contract implication:** "providers are weakly held" is only true for
Rust-generated fragments; the stable contract is "providers are
generation-fenced" — lifetime mechanism is an implementation detail.

## 6. Hit-test excludes disabled nodes, not just event dispatch

The original contract gated semantic `NodeEvent` dispatch on
`n.interactive()` — but a disabled button still received `hover`/`pressed`
state because `hit_test` returned it as a target. Disabled nodes are
**not** hit-test results; they render, they lay out, they do not accept
pointer state.

**Consequence:** `hit_test` returns `None` for
`Button`/`Editor`/`Action`/`Custom` nodes that are not `interactive()`;
disabled nodes are inert chrome.

**Contract implication:** disabled is a *pointer/focus/invoke* absence,
not a visual attribute; the renderer still draws the disabled recipe.

## 7. `Theme.dark` — resolved, not ambient

`Appearance.dark` reflects the OS mode; `Theme.dark` reflects the
application's resolved mode (`Light`/`Dark`/`System`). When the owner
requests `light` while the OS is `dark`, `Appearance.dark` stays true and
the whole UI stayed dark — the original bug.

**Consequence:** every consumer that chooses palette or resolves a role
color reads `theme.dark` (stored in the shared `peer_ctx.colors` cell).
`Appearance` only informs `ThemeMode::System` resolution, never direct
render choice.

**Contract implication:** `ui.theme(Theme::resolve(mode, appearance))` is
the single application entry point; the backend owns the OS→theme
resolution path only for `ThemeMode::System` and OS changes.

## 8. `natural_size` measures against an activated service

`EM_REQUESTRESIZE` returns garbage until the text service is in-place
activated — the peer's scratch 4000-DIP-tall activation rect is real, not
a hack. `natural_size` therefore:
1. updates `host.bounds.width` to the laid-out width;
2. activates if not yet activated (scratch bottom = natural height);
3. sends `EM_REQUESTRESIZE` and reads the reported natural size.

**Contract implication:** measurement is a native peer concern, not a
pure-DWrite concern; the peer must exist before layout can ask for its
natural height.

## 9. Peers mount inside `CreateWindowExW` with no hwnd

The first `turn()` runs while `CreateWindowExW` is still on the stack
(WM_CREATE/WM_SIZE dispatch synchronously inside the call), so peers mount
with `host.hwnd == HWND::default()`. With an invalid hwnd,
`ClientToScreen` inside msftedit silently fails, `IRicheditWindowless
Accessibility::CreateProvider` yields fragments that report `Infinity`
bounding rectangles, and IME/caret popup placement is wrong.

**Consequence:** after the window exists, the backend repairs every
already-mounted peer via `set_hwnd` (registry walk, generation-safe). The
fix restores both the D2D draw path and real editor bounding rects in the
UIA tree.

**Contract implication:** peer creation must tolerate a temporarily
invalid host hwnd; the backend owns the repair pass — consumers of
`peer_factory` never see it.

## 10. msftedit's host space is physical px, not DIP — probe environment hid it

`TxDrawD2D`'s `lprcBounds`, `TxGetClientRect`, caret coords and pointer
lparams are all interpreted by msftedit in the host DC's physical-pixel
units. Under a Per-Monitor-V2 process at 125% the service divides the
bounds by `dcDpi/96`; a DPI-unaware 96-DPI probe sees px==DIP and the
contract looks like "everything is DIP". The symptom was mount-time-text
peers drawing exactly one row above their pill — `arg/scale` on screen.

**Consequence:** the coordinate contract is now typed and single-sourced —
`crate::geom` owns `ScaleFactor`/`Logical*`/`Physical*` spaces and the
rounding policy; `platform/win32/space.rs` owns the Win32 seams; every
host callback converts explicitly at the boundary. The regression probe
runs PMv2 at 96/120/144/192.

**Contract implication:** documented in `docs/PLATFORM_CONTRACTS.md` and
institutionalized via Gate 16 environment-equivalence + Gate 20
duplicated-authority audit rules.

## 11. Synthetic `PostMessageW` cannot deliver `WM_DPICHANGED`

`WM_DPICHANGED` carries a `const RECT*` lparam; `PostMessageW` refuses
cross-process pointer parameters (`0x80070487`). The earlier
VirtualAllocEx + WriteProcessMemory + PostMessage approach in the
evidence harness silently no-opped — `Out-Null` swallowed the return.

**Consequence:** `native_probe.exe dpichange` uses `SendMessageW` — the
window manager marshals the suggested RECT for this known system message.
Verified: the composer applied a 750x750-px suggested rect at synthetic
144 DPI and relaid out under `ScaleFactor(1.5)`.

**Contract implication:** synthetic WM_DPICHANGED = deterministic
regression evidence only; a real monitor transition remains a distinct
acceptance class (recorded in PLATFORM_CONTRACTS.md).
