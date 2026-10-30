# P12-M1 — Event-driven motion evidence (time-sampled)

`motion_series.json` — devctl `epoch` payloads sampled ~12ms across each
flight. The `epoch` handler (`devtools/service.rs:96 handshake_json`) returns
`paint_epoch`, `counters.animation_timer_fires`/`animation_callbacks`,
`transitions` (live count), `trans_detail` (`transitions.rs:205 trans_probe`),
and `reduced` (reduced-motion flag).

## Mechanism

`src/platform/win32/transitions.rs` — per-node `Trans{from_*,to_*,start}`
where `from_*` is the last DISPLAYED value (interruption-safe: a mid-flight
reversal continues from the current interpolation). Channels: visual
(colors/fills), `s` scalar (switch-thumb / tabs-line), `p` pressed-offset,
`o` disabled-opacity, `r` ring presence. `ease_css` (`sched.rs:316`) = the
frozen `cubic-bezier(0.4,0,0.2,1)` material curve over the 150ms deadline.
Only transition-carrying kinds animate (cn-button variants, cn-tabs-trigger,
cn-switch); editors/nav/checkbox snap. The shared deadline timer fires only
while a flight is live — no animation work while static.

## Measured flights

| Case | Evidence |
|---|---|
| `button.hover-enter` | `frac 0.632→0.981`, `animation_timer_fires +7`, `callbacks +6` — intermediate eased frames, bounded scheduler |
| `button.press-release` | `frac 0.437→0.981`, `atf +7`, `cb +15` — press/release channels interpolate |
| `switch.default.unchecked` toggle | `s:[0→1]` thumb scalar `frac 0.426@48ms→0.945@108ms`, `r:[0→1]` ring fade, `atf +3/60ms`; concurrent `slot5 r:[1→0]` fade-out (focus transfer, interruption-safe) |

All flights converge to `transitions:0` — the scheduler quiesces at the
endpoint (no continuous timer). `reduced:false` in these runs; the
reduced-motion path snaps `frac` to target (no flight) per `sched.reduced`.

## Classification

**P12-M1 FIXED** — event-driven eased transitions with bounded invalidation;
intermediate frames proven via time-sampled `epoch`, not `capture=1` alone.
Final visual acceptance belongs to Opus 5.5.
