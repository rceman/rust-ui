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

**Resolution (implemented):** each peer renders into its own
`ID2D1BitmapRenderTarget` at LOCAL `(0,0,w,h)` — anchoring inside a
peer-owned surface is exact — and the frame blits the bitmap at the
global content rect via `DrawBitmap`. `TxGetViewInset` stays zero; the
padding between pill chrome and text lives in the blit destination rect
(content rect = pill rect minus insets).

**Contract implication:** a native text peer is a *leaf surface*, not an
inline draw — the compositor positions its rectangle; the peer's internal
origin is always `(0,0)`. This is also the shape a future
DirectComposition visual-per-peer path wants.

## 3. Activation latches the format space; peers get one activation

`OnTxInPlaceActivate` binds the format space **once**. Re-activating at a
different rect does not reliably re-anchor content, so peers activate
lazily — at the first non-empty bounds — and never deactivate/re-activate
on size changes (the mascot reference contract). `natural_size` activates
with a tall scratch height because `EM_REQUESTRESIZE` requires an
activated service to report; the lazy activation is therefore
load-bearing for measurement, not a defect.

**Contract implication:** an editor's activation is not "create-time"; it
is "first real bounds". Any implementation detail that relies on
create-time activation is wrong.

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
