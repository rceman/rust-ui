# Cross-monitor held-move — P10 evidence

User defect: held title-bar drag across monitors with part of the HWND
temporarily off-screen left black/stale rectangles while still held.

## Trace (after/ — env `RUI_RESIZE_TRACE`)

`trace/session-trace.log` — one `WM_ENTERSIZEMOVE..WM_EXITSIZEMOVE`
session recorded during the real drag (SendInput title-bar hold):
- 21 live `PAINT` dispatches inside the session — each newly-exposed
  region repainted while the button was still held (previously
  ValidateRect-discarded)
- live `WM_DPICHANGED` mid-drag: DISPLAY2 (100%) -> DISPLAY1 (125%),
  client 964x1561 -> 1207x1953 px, laid DIP 965.6x1562.4 stable
- `EXITSIZEMOVE` consumed the coalesced `move_pending` once — zero
  queued WM_MOVE position history

## Held capture

`after/held-crossmonitor-dark.png` — captured WHILE the mouse button
was still held, window fully re-exposed on DISPLAY1 after the
off-screen phase: full client painted, no black blocks, no duplicated
or half-drawn content, DirectWrite-crisp text.

## Code candidate

`74570f5` (+ test) — `sizing_session`/`move_pending` session policy in
`src/platform/win32/{mod,window}.rs`; `DWMWA_BORDER_COLOR` sync;
`tools/windows/run-gallery.ps1` fast-path.
