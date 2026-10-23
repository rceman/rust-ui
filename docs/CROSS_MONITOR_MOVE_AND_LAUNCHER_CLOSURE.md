# Cross-monitor held-move + window edge + launcher — P10 closure

## User reproduction

Held title-bar drag: monitor A → monitor B with the HWND partly
off-screen, then back to fully visible — WHILE the button stays down —
left black/stale rectangular regions and duplicated/clipped content.

## Root cause

`DefWindowProcW`'s modal move loop nests inside our `WM_NCLBUTTONDOWN`
dispatch → `in_dispatch` stays true for the entire gesture → every
arrival hits `reentrant()`. The P9 fix made `WM_SIZE` + `WM_PAINT`
(when a resize frame was pending) dispatch live — but a **move**
doesn't change the client size, so:

- `size_pending` stays false, `laid_client` stays current
- every `WM_PAINT` for a newly-exposed region hit the
  validate-and-discard leg → the exposed pixels never got a real frame
- `WM_MOVE` was queue-deferred (position history, post-release replay)
- `WM_DPICHANGED` was queue-deferred — fatal on a real cross-DPI
  crossing (DISPLAY2 100% → DISPLAY1 125%, traced mid-drag)

## Fix — session semantics (candidate `74570f5`)

- `sizing_session` Cell: armed by `WM_ENTERSIZEMOVE`, cleared by
  `WM_EXITSIZEMOVE` (reentrant + dispatch arms both). While armed,
  reentrant `WM_PAINT` dispatches live unconditionally — the OS's
  paint coalescing still bounds frame count.
- `WM_MOVE` (reentrant): collapses to a `move_pending` flag — zero
  queue growth during the drag (256-move test asserts len==0); one
  UIA refresh consumed at `WM_EXITSIZEMOVE`.
- `WM_DPICHANGED` (reentrant): dispatches live via the trampoline —
  the lparam RECT is borrowed but valid for the synchronous call.
- `WM_EXITSIZEMOVE` (reentrant): re-enters dispatch so the arm can
  consume `move_pending` + run the session-end UIA refresh once.

## Evidence

`benchmark/results/cross-monitor-held-move/` — trace (21 live PAINTs
in-session, live DPI transition, clean EXIT) + held-state screenshot
(full client painted mid-drag) + monitor topology. Regression test
`held_move_session_coalesces_and_arms` covers flag/coalesce semantics.

## Defect B — black window edges

Measured: window 980x1600, client 965x1562 → non-client insets
7L/30T/8R/8B px = the standard Win11 resizable frame geometry — the
visible strip is the **DWM-owned frame border**, not client padding.
Fixed by `DWMWA_BORDER_COLOR` synced to the window surface in
`sync_dark_caption` (dark `0x18181B` / light `0xFAFAFA`, COLORREF BGR).
No custom non-client frame added; older Windows ignores the attribute.

## Defect C — launcher speed + terminal flash

- Always built on every click + a `cargo.exe` console child surfaced a
  terminal under the hidden PowerShell.
- Now: build stamp `.rust-ui-gallery-build.json` (HEAD + profile) in
  the target dir — clean worktree + matching HEAD + binary present →
  launch directly (~1.4s measured, no cargo). Dirty worktree or HEAD
  mismatch → headless build via `ProcessStartInfo` (CreateNoWindow,
  UseShellExecute=false, async stdout/stderr — the sequential-ReadToEnd
  deadlock found and fixed during validation). Dirty builds never
  write a stamp. Failure → MessageBox tail; verified: no gallery on
  failure, no terminal on any path.

## Gates / reviews

Linux 123, Win lib 195 (incl. `held_move_session_coalesces_and_arms`),
devctl_windows 2/2. Astra + Argon verdicts recorded in the P10 report.
