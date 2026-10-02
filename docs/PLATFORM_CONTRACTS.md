# Platform Contracts — Canonical Architecture

This document defines how rust-ui splits *shared semantics* from *platform
implementations*. It is the authority for the cross-platform platform
boundary. Other architecture documents reference it; they do not redefine it.

> **Windows is the current implementation target, not the semantic
> definition of rust-ui.** A future macOS backend must satisfy the same
> semantic contracts — it is not implemented in the current milestone.

## The rule

```text
rust-ui shared semantic contracts
            ↓
one authoritative implementation per platform
            ↓
platform consumers
```

One semantic concept has one shared rust-ui contract. Each supported
platform has exactly one authoritative implementation of that contract.
Consumers use the contract/implementation and never independently reproduce
its semantics.

The defect class that forced this correction: `text.rs`, `window.rs`,
`uia.rs`, the native probe and PowerShell all knew pieces of the DPI
contract — the windowless RichEdit host space being *physical px at the
host DC's dpi* was discovered subsystem-by-subsystem, and a DPI-unaware
96-DPI probe masked it for months because `1 DIP == 1 px` numerically.

"Contract" is an ownership/semantic claim, not necessarily a `dyn` trait.
The compile target already selects the platform; representation is the
smallest correct Rust construct (typed modules, sealed traits, associated
types) with **no dynamic dispatch, registries, or DI machinery**.

## Coordinate contract (first implementation)

`src/geom.rs` is the shared geometry authority. Everything below lives
there — platform-neutral:

```text
Dp                    rust-ui logical layout/design unit (a pure scalar —
                      NOT a physical "1/96 inch" promise; the 96-DPI
                      baseline is Windows' own mapping rule and lives in
                      the Win32 adapter only)
PhysicalPx            physical scalar

Point/Size/Rect       logical-space point/size/rect (== Logical*)
PhysicalPoint/Size/Rect
                      physical-px point/size/rect (LTRB lattice)

ClientPhysicalPoint   marker — a window/surface client's own 0,0 space
ScreenPhysicalPoint   marker — global display space
PeerLocalPoint        marker — a peer surface's own 0,0 space

PeerOrigin            THE client-px <-> peer-local-px transform — one
                      snapped physical origin per (bounds, scale);
                      integer subtraction/addition only. Every consumer
                      (pointer, caret, host callbacks, probe) shares it.

ScaleFactor           px-per-logical-unit RATIO — platform-neutral,
                      positive and finite. `new`/`to_physical`/
                      `to_physical_f`/`to_logical`/`to_logical_f` are the
                      shared math; `scale_from_dpi`/`dpi_of` (the 96
                      baseline) exist ONLY in platform/win32/space.rs.

rounding policy       round-half-away-from-zero at logical→physical;
                      physical→logical is exact
```

Typed-space rules: logical values are `f32`, physical are `i32` at
boundaries; converting a `Rect` gives a `PhysicalRect`, never a
"Rect holding px". Client-space vs screen-space px are different types so
`ClientToScreen`'s input can't be silently substituted for a peer-local
point. Fractional-physical operations (`_f`) exist only for raster math
that must not snap mid-pipeline.

Windows realizes the contract in `platform/win32/space.rs` — a thin
adapter owning `RECT`/`RECTL`/`POINT`/`SIZE` conversions, `LPARAM`
packing, `ClientToScreen`/`ScreenToClient` (checked — native failure is
`None`, never an identity fallback), and the `dpi ↔ ratio` mapping
(`scale_from_dpi`/`dpi_of`). Any `* scale`/`dpi/96` outside `geom.rs`
(the ratio math) or `space.rs` (the DPI mapping) is a contract
violation, except documented API-required unit exceptions (e.g.
`TxGetExtent`'s HIMETRIC — 1/100 mm — computed locally at that call
site). Signed-16-bit LPARAM coordinates are range-checked
(`try_lparam_px` returns `None` out of range).

## Service inventory

Responsibilities actually present in the Windows backend today:

| Semantic area | Shared contract | Windows authority | Shared tests | Native tests |
|---|---|---|---|---|
| Geometry / DPI | `crate::geom` typed spaces + `ScaleFactor` | `platform::win32::space` (RECT/POINT seams, client↔screen) | `geometry_contract_*` conformance (96/120/144/192) | RichEdit px probe, UIA rect probe, `native_probe geometry` |
| Window | runtime window lifecycle + event delivery (rust-ui `Backend` trait) | `platform::win32::window` WndProc | runtime pump tests | live composer capture |
| Text services | `crate::node::TextPeer` (natural size, draw, committed-edit sink) | `platform::win32::text` (windowless RichEdit host, explicit `Activation` state machine + preserving scale relatch) | node/peer lifecycle + relatch-state tests | `native_probe_richedit_paints_text`, real IME (MEASURED via `native_probe ime`) |
| Input | `PointerEvent`/`KeyEvent`/`Modifiers` normalized events (modifiers read from real GetKeyState; `Key::Other` is platform-scoped raw VK) | `platform::win32::window` WM_* → semantic | pump/dispatch + submit-contract tests | real-key + real-IME acceptance |
| Accessibility | rust-ui semantic tree (role/name/bounds/state) | `platform::win32::uia` IUIAutomation provider | UIA-adjacent unit tests | `native_probe uia`, UIA probe |
| Clipboard | **deferred** — RichEdit owns Ctrl+X/C/V internally; no rust-ui consumer exists | — | — | — |
| Rendering | `BoxStyle`/shadow/fill/border/text semantics | `platform::win32::render` Direct2D | style/state tests | composer captures |
| Timers | scheduler deadlines | ONE shared `TimerPool` armed synchronously by `TxSetTimer` (FALSE on failure), routed through `platform::win32::window` `SetTimer`; collision-free ids + generation fencing | timer alloc/pool tests | idle/timeout evidence |

Deferred contracts are deferred *because no consumer exists* — the
documented rule is "no abstraction without a consumer".

## What may never leak upward

```text
HWND, WPARAM, LPARAM, WM_* message ids, RECT/POINT/RECTL,
ITextServices*, UIA provider objects, Gdi32 user32 symbols,
Win32 focus/capture quirks, himetric, charset conversions
```

These are implementation detail of the Windows authority. Shared
semantics name `ScreenPhysicalPoint`, `ScaleFactor`, `PointerEvent`,
`NodeEvent::Committed` — never their Win32 realization.

## Conformance hierarchy

```text
shared semantic authority   → deep/exhaustive focused tests (once)
platform adapter            → prove WHICH space/contract the native API speaks
consumer                    → representative integration only
freeze                      → broad acceptance over the agreed matrix
```

A Windows adapter test asserts "this API's bounds are physical px" — it
does not re-derive conversion math. `native_probe_richedit_paints_text`
is the canonical example: ink inside the requested island at all four
scales proves `TxDrawD2D`'s space, while `geometry_contract_*` proves the
math once.

Verification reuse: a pure shared-semantics change runs the conformance
suite; an adapter change runs its focused native test; only a freeze runs
the whole gate matrix. Full-system reruns are not a substitute for
dependency reasoning — and not skipped when a shared authority changed.

## Evidence classes

```text
PostMessage input / synthetic WM_DPICHANGED  → deterministic regression
real OS input / real monitor transition      → native acceptance
```

The Rust harness (`examples/native_probe.rs`) owns native semantics and
emits structured JSON evidence with non-zero exit on violated invariants.
PowerShell (`native.ps1`, `collect.ps1`) is orchestration only:
build/run/PID lifecycle/scenario sequencing/timeouts/artifact collection.
It must not own coordinate math, UIA semantics, or DPI rules.
