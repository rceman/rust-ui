use super::*;

/// Per-node transition state — the backend's half of the `Scheduler`
/// transition contract. The scheduler owns the clock (150ms steps via the
/// shared deadline timer); this map owns WHAT is being interpolated: the
/// resolved visual plus the scalar channels the frozen recipes animate —
/// switch-thumb position, tabs-line indicator opacity, button pressed
/// offset and the disabled opacity layer.
///
/// `from_*` is the last DISPLAYED value (interruption-safe: a reversal
/// mid-flight continues from the current interpolation, never the
/// origin), `to_*` the resolved target. Channels a kind does not use sit
/// at equal from/to — the lerp is a no-op.
#[derive(Copy, Clone)]
pub(crate) struct Trans {
    from_v: crate::style::VisualStyle,
    to_v: crate::style::VisualStyle,
    /// kind scalar — Switch thumb / TabLine indicator
    from_s: f32,
    to_s: f32,
    /// pressed offset — `active:translate-y-px` on cn-button
    from_p: f32,
    to_p: f32,
    /// disabled layer opacity — `disabled:opacity-50`
    from_o: f32,
    to_o: f32,
    /// focus/invalid ring presence — the box-shadow ring fades in/out
    /// under `transition-all` (CSS animates the shadow alpha)
    from_r: f32,
    to_r: f32,
    start: std::time::Instant,
}

/// The per-channel value the last paint actually deposited — the `from`
/// source for every new transition AND the settled target after one
/// ends. Only channels the node's paint path reports exist.
#[derive(Default)]
pub(crate) struct Shown {
    v: Option<crate::style::VisualStyle>,
    s: Option<f32>,
    p: Option<f32>,
    o: Option<f32>,
    r: Option<f32>,
}

impl<S, M, U, V> Backend<S, M, U, V>
where
    M: 'static,
    U: Fn(&mut S, M, &mut UpdateCtx<'_, M>),
    V: Fn(&S, &mut crate::Ui<'_, '_, M>),
{
    /// Is this node's reference recipe transition-carrying?
    /// `transition-all` on cn-button + cn-tabs-trigger, `transition-all`
    /// + `transition-transform` on cn-switch. Checkbox/radio items,
    /// cn-select parts, nav/menu actions and editors carry NONE — they
    /// snap in the reference and snap here. A Button additionally honors
    /// the authored `motion` opt-out (`ui.button().motion(None)`).
    pub(super) fn transition_eligible(&self, id: NodeId) -> bool {
        match self.rt.arena.get(id).map(|n| &n.data) {
            Some(NodeData::Button {
                variant, motion, ..
            }) => {
                motion.is_some()
                    && matches!(
                        variant,
                        crate::theme::ButtonVariant::Primary
                            | crate::theme::ButtonVariant::Secondary
                            | crate::theme::ButtonVariant::Outline
                            | crate::theme::ButtonVariant::Ghost
                            | crate::theme::ButtonVariant::Destructive
                            | crate::theme::ButtonVariant::Link
                            | crate::theme::ButtonVariant::Tab
                            | crate::theme::ButtonVariant::TabLine
                    )
            }
            Some(NodeData::Toggle { kind, .. }) => *kind == crate::theme::ToggleKind::Switch,
            _ => false,
        }
    }

    /// The resolved per-channel targets for the node's CURRENT state.
    fn transition_targets(
        &self,
        id: NodeId,
    ) -> Option<(crate::style::VisualStyle, f32, f32, f32, f32)> {
        let n = self.rt.arena.get(id)?;
        let st = self.interact(id);
        let v = self.resolved_at(id, st)?;
        let disabled = match &n.data {
            NodeData::Button { disabled, .. } | NodeData::Toggle { disabled, .. } => *disabled,
            _ => false,
        };
        // ring presence — (focus-visible OR unconditional invalid) AND
        // enabled, matching the painter's gate; the alpha channel fades,
        // the resolved color comes from `focus_ring` (destructive when
        // invalid)
        let ring = match &n.data {
            NodeData::Button { invalid, .. } | NodeData::Toggle { invalid, .. } => {
                ((st.focus || *invalid) && !disabled) as u8 as f32
            }
            _ => (st.focus && !disabled) as u8 as f32,
        };
        let (s, p) = match &n.data {
            NodeData::Toggle { checked, .. } => ((*checked) as u8 as f32, 0.0),
            NodeData::Button {
                selected, variant, ..
            } => {
                let s = if *variant == crate::theme::ButtonVariant::TabLine {
                    (*selected) as u8 as f32
                } else {
                    0.0
                };
                (s, st.pressed as u8 as f32)
            }
            _ => (0.0, 0.0),
        };
        Some((v, s, p, if disabled { 0.5 } else { 1.0 }, ring))
    }

    /// An animatable change arrived (interact-state or DIRTY_VALUE data
    /// flip): begin a 150ms eased transition on eligible nodes whose
    /// displayed output differs from the resolved target — otherwise
    /// cancel any stale entry so the node snaps. Reduced-motion and
    /// forced-colors skip animation entirely (instant settle).
    pub(super) fn begin_transition(&mut self, id: NodeId) {
        let Some((to_v, to_s, to_p, to_o, to_r)) = self.transition_targets(id) else {
            return;
        };
        if !self.transition_eligible(id)
            || self.rt.sched.reduced
            || self.rt.forced_resolver().is_some()
        {
            self.trans.borrow_mut().remove(&id.slot);
            self.rt.sched.cancel_transition(id);
            return;
        }
        let now = std::time::Instant::now();
        // displayed-from: a live transition interpolates at `now`; the
        // shown cache holds the last painted channel values otherwise
        let (fv, fs, fp, fo, fr) = {
            let tr = self.trans.borrow();
            match tr.get(&id.slot) {
                Some(t) => {
                    let e = trans_ease(*t, now);
                    let dark = self.rt.theme.dark;
                    (
                        lerp_visual(&t.from_v, &t.to_v, e, dark),
                        lerp(t.from_s, t.to_s, e),
                        lerp(t.from_p, t.to_p, e),
                        lerp(t.from_o, t.to_o, e),
                        lerp(t.from_r, t.to_r, e),
                    )
                }
                None => {
                    let sh = self.shown.borrow();
                    let s = sh.get(&id.slot);
                    (
                        s.and_then(|x| x.v).unwrap_or(to_v),
                        s.and_then(|x| x.s).unwrap_or(to_s),
                        s.and_then(|x| x.p).unwrap_or(to_p),
                        s.and_then(|x| x.o).unwrap_or(to_o),
                        s.and_then(|x| x.r).unwrap_or(to_r),
                    )
                }
            }
        };
        if fv == to_v && fs == to_s && fp == to_p && fo == to_o && fr == to_r {
            self.trans.borrow_mut().remove(&id.slot);
            self.rt.sched.cancel_transition(id);
            return;
        }
        self.trans.borrow_mut().insert(
            id.slot,
            Trans {
                from_v: fv,
                to_v,
                from_s: fs,
                to_s,
                from_p: fp,
                to_p,
                from_o: fo,
                to_o,
                from_r: fr,
                to_r,
                start: now,
            },
        );
        let _ = self
            .rt
            .sched
            .start_transition(id, crate::theme::MotionToken::Hover, now);
    }

    /// Settle: the scheduler's `TransitionDone` — the entry drops; the
    /// next paint resolves the instant target.
    pub(super) fn end_transition(&self, node: NodeId) {
        self.trans.borrow_mut().remove(&node.slot);
    }

    /// Remove every trace of a node — unmount, hide, or reclassify.
    pub(super) fn prune_transition(&self, slot: u32) {
        self.trans.borrow_mut().remove(&slot);
        self.shown.borrow_mut().remove(&slot);
    }

    /// The painted `VisualStyle` for this frame — the eased interpolation
    /// while a transition runs, else the instant target. Records what the
    /// frame shows so the next transition can continue from it.
    pub(crate) fn displayed_visual(
        &self,
        id: NodeId,
        to_v: crate::style::VisualStyle,
    ) -> crate::style::VisualStyle {
        let dark = self.rt.theme.dark;
        let now = std::time::Instant::now();
        let t = self.trans.borrow().get(&id.slot).copied();
        let v = match t {
            Some(t) if t.to_v == to_v => {
                let frac = trans_frac(t, now);
                if frac < 1.0 {
                    lerp_visual(&t.from_v, &t.to_v, crate::sched::ease_css(frac), dark)
                } else {
                    self.trans.borrow_mut().remove(&id.slot);
                    to_v
                }
            }
            Some(_) => {
                // target drifted mid-flight — snap to the live resolve
                self.trans.borrow_mut().remove(&id.slot);
                to_v
            }
            None => to_v,
        };
        self.shown.borrow_mut().entry(id.slot).or_default().v = Some(v);
        v
    }

    /// Painted kind-scalar (Switch thumb / TabLine indicator) for this
    /// frame — `to_s` is the resolved target (0|1); interrupted flights
    /// continue from their interpolated value.
    pub(crate) fn displayed_scalar(&self, id: NodeId, to_s: f32) -> f32 {
        let s = self.channel(id, to_s, |t| (t.from_s, t.to_s));
        self.shown.borrow_mut().entry(id.slot).or_default().s = Some(s);
        s
    }

    /// Painted pressed-offset channel (cn-button `active:translate-y-px`).
    pub(crate) fn displayed_pressed(&self, id: NodeId, to_p: f32) -> f32 {
        let p = self.channel(id, to_p, |t| (t.from_p, t.to_p));
        self.shown.borrow_mut().entry(id.slot).or_default().p = Some(p);
        p
    }

    /// Painted disabled-opacity channel (`disabled:opacity-50`).
    pub(crate) fn displayed_opacity(&self, id: NodeId, to_o: f32) -> f32 {
        let o = self.channel(id, to_o, |t| (t.from_o, t.to_o));
        self.shown.borrow_mut().entry(id.slot).or_default().o = Some(o);
        o
    }

    /// Painted ring-presence channel — the focus-visible/invalid ring
    /// alpha (0 hidden, 1 shown) eased like the CSS box-shadow fade.
    pub(crate) fn displayed_ring(&self, id: NodeId, to_r: f32) -> f32 {
        let r = self.channel(id, to_r, |t| (t.from_r, t.to_r));
        self.shown.borrow_mut().entry(id.slot).or_default().r = Some(r);
        r
    }

    /// Shared scalar-channel step: eased `lerp(from,to)` while the node's
    /// transition targets `to`, else `to` (snap or settled).
    fn channel(&self, id: NodeId, to: f32, pick: impl Fn(Trans) -> (f32, f32)) -> f32 {
        match self.trans.borrow().get(&id.slot).copied() {
            Some(t) => {
                let (from, tgt) = pick(t);
                let frac = trans_frac(t, std::time::Instant::now());
                if (tgt - to).abs() < 1e-6 && frac < 1.0 {
                    lerp(from, tgt, crate::sched::ease_css(frac))
                } else {
                    to
                }
            }
            None => to,
        }
    }
}

fn trans_frac(t: Trans, now: std::time::Instant) -> f32 {
    (now.saturating_duration_since(t.start).as_secs_f32() * 1000.0
        / crate::sched::TRANSITION_MS as f32)
        .min(1.0)
}

fn trans_ease(t: Trans, now: std::time::Instant) -> f32 {
    crate::sched::ease_css(trans_frac(t, now))
}

fn lerp(a: f32, b: f32, e: f32) -> f32 {
    a + (b - a) * e
}

/// Lerp one authored color: resolve both ends to sRGB, mix channels.
/// Role resolution happens per step so a theme flip mid-flight recolors
/// both endpoints instead of stranding the captured palette.
fn lerp_color(
    a: crate::style::Color,
    b: crate::style::Color,
    e: f32,
    dark: bool,
) -> crate::style::Color {
    let [ar, ag, ab, aa] = crate::style::resolve_color(a, dark);
    let [br, bg, bb, ba] = crate::style::resolve_color(b, dark);
    crate::style::Color::Rgba {
        r: (lerp(ar, br, e) * 255.0).round().clamp(0.0, 255.0) as u8,
        g: (lerp(ag, bg, e) * 255.0).round().clamp(0.0, 255.0) as u8,
        b: (lerp(ab, bb, e) * 255.0).round().clamp(0.0, 255.0) as u8,
        a: (lerp(aa, ba, e) * 255.0).round().clamp(0.0, 255.0) as u8,
    }
}

fn lerp_dp(a: crate::geom::Dp, b: crate::geom::Dp, e: f32) -> crate::geom::Dp {
    crate::geom::Dp(lerp(a.0, b.0, e))
}

fn lerp_side(
    a: crate::style::BorderSide,
    b: crate::style::BorderSide,
    e: f32,
    dark: bool,
) -> crate::style::BorderSide {
    crate::style::BorderSide {
        width: lerp_dp(a.width, b.width, e),
        color: lerp_color(a.color, b.color, e, dark),
    }
}

/// `transition-all` over the resolved `BoxStyle` — every numeric and
/// color channel interpolates; a `None` shadow treats as the zero
/// shadow so appear/disappear fades rather than popping.
fn lerp_box(
    a: &crate::style::BoxStyle,
    b: &crate::style::BoxStyle,
    e: f32,
    dark: bool,
) -> crate::style::BoxStyle {
    const ZERO: crate::style::Shadow = crate::style::Shadow {
        color: crate::style::Color::Rgba {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        },
        offset_x: crate::geom::Dp(0.0),
        offset_y: crate::geom::Dp(0.0),
        blur_sigma: crate::geom::Dp(0.0),
        spread: crate::geom::Dp(0.0),
    };
    let (sa, sb) = (a.shadow.unwrap_or(ZERO), b.shadow.unwrap_or(ZERO));
    crate::style::BoxStyle {
        background: lerp_color(a.background, b.background, e, dark),
        border: crate::style::Border {
            top: lerp_side(a.border.top, b.border.top, e, dark),
            right: lerp_side(a.border.right, b.border.right, e, dark),
            bottom: lerp_side(a.border.bottom, b.border.bottom, e, dark),
            left: lerp_side(a.border.left, b.border.left, e, dark),
        },
        radii: crate::style::CornerRadii {
            top_left: lerp_dp(a.radii.top_left, b.radii.top_left, e),
            top_right: lerp_dp(a.radii.top_right, b.radii.top_right, e),
            bottom_right: lerp_dp(a.radii.bottom_right, b.radii.bottom_right, e),
            bottom_left: lerp_dp(a.radii.bottom_left, b.radii.bottom_left, e),
        },
        padding: crate::style::Insets {
            top: lerp_dp(a.padding.top, b.padding.top, e),
            right: lerp_dp(a.padding.right, b.padding.right, e),
            bottom: lerp_dp(a.padding.bottom, b.padding.bottom, e),
            left: lerp_dp(a.padding.left, b.padding.left, e),
        },
        shadow: Some(crate::style::Shadow {
            color: lerp_color(sa.color, sb.color, e, dark),
            offset_x: lerp_dp(sa.offset_x, sb.offset_x, e),
            offset_y: lerp_dp(sa.offset_y, sb.offset_y, e),
            blur_sigma: lerp_dp(sa.blur_sigma, sb.blur_sigma, e),
            spread: lerp_dp(sa.spread, sb.spread, e),
        }),
    }
}

/// Lerp the full resolved visual. Non-interpolatable channels (font,
/// weight, underline, letter-spacing, line-height) take the target —
/// CSS animates only computed-value-continuous properties.
fn lerp_visual(
    a: &crate::style::VisualStyle,
    b: &crate::style::VisualStyle,
    e: f32,
    dark: bool,
) -> crate::style::VisualStyle {
    let mut text_style = b.text_style;
    text_style.foreground = lerp_color(a.text_style.foreground, b.text_style.foreground, e, dark);
    crate::style::VisualStyle {
        box_style: lerp_box(&a.box_style, &b.box_style, e, dark),
        text_style,
        focus_ring: lerp_color(a.focus_ring, b.focus_ring, e, dark),
    }
}
