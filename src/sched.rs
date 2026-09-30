use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::event::FrameTime;
use crate::node::NodeId;
use crate::theme::MotionToken;

pub(crate) const FRAME_STEP: Duration = Duration::from_millis(16);
pub(crate) const TRANSITION_MS: u64 = 150;
pub(crate) const TOOLTIP_DELAY: Duration = Duration::from_millis(500);

/// Narrow per-window scheduler — chrome interactions only, NOT a general
/// animation engine. One earliest deadline; no demand => no deadline, no
/// timer callback, no redraw request. Native caret blink is excluded.
pub(crate) struct Scheduler {
    transitions: HashMap<u32, Transition>,
    frames: HashMap<u32, FrameDemand>,
    tooltips: HashMap<u32, Tooltip>,
    pub(crate) reduced: bool,
}

struct Transition {
    node: NodeId,
    token: MotionToken,
    start: Instant,
    /// the next step tick's index (each FRAME_STEP, clipped at the end)
    next_tick: u32,
}

struct FrameDemand {
    node: NodeId,
    /// `next_frame=None` + suspended means parked by hidden/remove —
    /// a Reduce->NoPreference flip must NOT resume these
    suspended: bool,
    /// `None` while suspended or policy-parked — first delta after resume ~0
    next_frame: Option<Instant>,
    last: Option<Instant>,
}

struct Tooltip {
    node: NodeId,
    /// semantic show deadline — a one-shot timer, kept under Reduce
    show_at: Instant,
    fired: bool,
}

/// Chrome/scheduler events returned to the backend by `poll`.
#[derive(Debug, Clone)]
pub(crate) enum SchedEvent {
    /// a transition frame step — backend repaints chrome interpolation
    TransitionStep {
        node: NodeId,
        token: MotionToken,
        elapsed: Duration,
    },
    /// settled at the end (or instantly under Reduce/policy flip)
    TransitionDone { node: NodeId, token: MotionToken },
    /// custom-node frame request — routed to the node's `on_frame`
    Frame { node: NodeId, time: FrameTime },
    /// semantic tooltip show — fires after the delay even under Reduce
    /// (instant, without the fade/scale animation)
    TooltipShow { node: NodeId },
}

impl Scheduler {
    /// Transition demand begins — 150 ms, stepped at FRAME_STEP.
    /// Under Reduce there is no transition: the state settles instantly and
    /// the caller gets `TransitionDone` immediately (zero animation work).
    pub(crate) fn start_transition(
        &mut self,
        node: NodeId,
        token: MotionToken,
        now: Instant,
    ) -> Option<SchedEvent> {
        if self.reduced {
            return Some(SchedEvent::TransitionDone { node, token });
        }
        self.transitions.insert(
            node.slot,
            Transition {
                node,
                token,
                start: now,
                next_tick: 1,
            },
        );
        None
    }

    /// Arm the semantic tooltip delay (one-shot). Under Reduce the delay is
    /// still semantic — the tooltip shows instantly once due, without the
    /// fade/scale animation. Reduce never removes the tooltip itself.
    pub(crate) fn arm_tooltip(&mut self, node: NodeId, now: Instant) {
        self.tooltips.insert(
            node.slot,
            Tooltip {
                node,
                show_at: now + TOOLTIP_DELAY,
                fired: false,
            },
        );
    }

    /// Custom-node frame demand. A repeated `on=true` is a NO-OP — the due
    /// time and delta clock are preserved, so an unchanged re-view can never
    /// re-arm instant work. Suspension lives on the entry, not in `on`.
    pub(crate) fn frame_demand(&mut self, node: NodeId, on: bool, now: Instant) {
        if on {
            self.frames.entry(node.slot).or_insert(FrameDemand {
                node,
                suspended: false,
                next_frame: Some(now),
                last: None,
            });
        } else {
            self.frames.remove(&node.slot);
        }
    }

    /// Unarm the tooltip delay (pointer leave/hide).
    pub(crate) fn cancel_tooltip(&mut self, node: NodeId) {
        self.tooltips.remove(&node.slot);
    }

    /// Hide/unmount-ish suspension: transitions and tooltips are cancelled;
    /// frame demand is parked (delta resets — resume restarts the clock).
    /// `suspended` is the entry's own flag, orthogonal to the reduced
    /// policy, so a policy flip can never un-hide a parked demand.
    pub(crate) fn set_suspended(&mut self, node: NodeId, suspended: bool, now: Instant) {
        self.transitions.remove(&node.slot);
        self.tooltips.remove(&node.slot);
        if let Some(f) = self.frames.get_mut(&node.slot) {
            f.suspended = suspended;
            if suspended {
                f.next_frame = None;
                f.last = None;
            } else if f.next_frame.is_none() {
                f.next_frame = Some(now); // delta resets on resume
            }
        }
    }

    /// Node removed: every demand cancels.
    pub(crate) fn cancel_node(&mut self, node: NodeId) {
        self.transitions.remove(&node.slot);
        self.frames.remove(&node.slot);
        self.tooltips.remove(&node.slot);
    }

    /// Effective reduced-motion change — mid-transition clears the demand
    /// and settles instantly; custom frame demand suspends (resumes with a
    /// reset delta when the policy lifts).
    pub(crate) fn set_reduced(&mut self, reduced: bool, now: Instant) -> Vec<SchedEvent> {
        if self.reduced == reduced {
            return Vec::new();
        }
        self.reduced = reduced;
        let mut out = Vec::new();
        if reduced {
            for (_, t) in self.transitions.drain() {
                out.push(SchedEvent::TransitionDone {
                    node: t.node,
                    token: t.token,
                });
            }
            for f in self.frames.values_mut() {
                f.next_frame = None;
                f.last = None;
            }
        } else {
            // policy lifted — resume only non-parked demand
            for f in self.frames.values_mut() {
                if !f.suspended && f.next_frame.is_none() {
                    f.next_frame = Some(now); // delta resets on resume
                }
            }
        }
        out
    }

    /// The earliest deadline across all demand — None when idle (no timer,
    /// no redraw request).
    pub(crate) fn next_deadline(&self) -> Option<Instant> {
        let mut best: Option<Instant> = None;
        let mut track = |t: Option<Instant>| {
            if let Some(t) = t {
                best = Some(best.map_or(t, |b| b.min(t)));
            }
        };
        if !self.reduced {
            for t in self.transitions.values() {
                let end = t.start + Duration::from_millis(TRANSITION_MS);
                track(Some((t.start + FRAME_STEP * t.next_tick).min(end)));
            }
            for f in self.frames.values() {
                if !f.suspended {
                    track(f.next_frame);
                }
            }
        }
        // tooltips keep their semantic delay under Reduce — they just show
        // instantly when due instead of animating
        for t in self.tooltips.values() {
            if !t.fired {
                track(Some(t.show_at));
            }
        }
        best
    }

    pub(crate) fn is_idle(&self) -> bool {
        self.next_deadline().is_none()
    }

    pub(crate) fn has_frame_demand(&self) -> bool {
        self.frames.values().any(|f| f.next_frame.is_some())
    }

    /// Poll due work — called only when `next_deadline` says so. Emits at
    /// most one transition step per active transition per call.
    pub(crate) fn poll(&mut self, now: Instant) -> Vec<SchedEvent> {
        let mut out = Vec::new();
        // transitions (skipped entirely under Reduce — no animation work)
        if !self.reduced {
            let slots: Vec<u32> = self.transitions.keys().copied().collect();
            for slot in slots {
                let Some(t) = self.transitions.get_mut(&slot) else {
                    continue;
                };
                let end = t.start + Duration::from_millis(TRANSITION_MS);
                let step_at = (t.start + FRAME_STEP * t.next_tick).min(end);
                if now >= step_at {
                    let elapsed = now - t.start;
                    if elapsed >= Duration::from_millis(TRANSITION_MS) || now >= end {
                        out.push(SchedEvent::TransitionDone {
                            node: t.node,
                            token: t.token,
                        });
                        self.transitions.remove(&slot);
                    } else {
                        out.push(SchedEvent::TransitionStep {
                            node: t.node,
                            token: t.token,
                            elapsed,
                        });
                        t.next_tick = (elapsed.as_millis() / 16) as u32 + 1;
                    }
                }
            }
            // custom frame demand — one frame per active demand per tick
            let fslots: Vec<u32> = self.frames.keys().copied().collect();
            for slot in fslots {
                if let Some(f) = self.frames.get_mut(&slot)
                    && !f.suspended
                    && let Some(next) = f.next_frame
                    && now >= next
                {
                    let delta = f.last.map_or(Duration::ZERO, |l| now - l);
                    f.last = Some(now);
                    f.next_frame = Some(now + FRAME_STEP);
                    out.push(SchedEvent::Frame {
                        node: f.node,
                        time: FrameTime {
                            delta,
                            absolute: now,
                        },
                    });
                }
            }
        }
        // tooltips — one-shot semantic delay, fires under Reduce too
        let tslots: Vec<u32> = self.tooltips.keys().copied().collect();
        for slot in tslots {
            if let Some(t) = self.tooltips.get_mut(&slot)
                && !t.fired
                && now >= t.show_at
            {
                t.fired = true;
                out.push(SchedEvent::TooltipShow { node: t.node });
                self.tooltips.remove(&slot);
            }
        }
        out
    }

    /// Full clear — window teardown.
    pub(crate) fn reset(&mut self) {
        self.transitions.clear();
        self.frames.clear();
        self.tooltips.clear();
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Scheduler {
            transitions: HashMap::new(),
            frames: HashMap::new(),
            tooltips: HashMap::new(),
            reduced: false,
        }
    }
}
