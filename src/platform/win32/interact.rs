use super::*;

impl<S, M, U, V> Backend<S, M, U, V>
where
    M: 'static,
    U: Fn(&mut S, M, &mut UpdateCtx<'_, M>),
    V: Fn(&S, &mut crate::Ui<'_, '_, M>),
{
    /// Effective visual state = runtime interaction OR the node's staged
    /// `presented` flags — paint, ink and layout resolve through these so
    /// the two sources can never diverge. Devtools/UIA keep reporting the
    /// raw runtime fields (presented grants no real state).
    pub(crate) fn resolved_hot(&self, id: NodeId, n: &crate::node::Node) -> bool {
        crate::layout::presented_or(self.hot, id, n.presented.hover)
    }
    pub(crate) fn resolved_pressed(&self, id: NodeId, n: &crate::node::Node) -> bool {
        crate::layout::presented_or(self.pressed, id, n.presented.pressed)
    }
    pub(crate) fn resolved_focus_visible(&self, id: NodeId, n: &crate::node::Node) -> bool {
        crate::layout::presented_or(self.focus, id, n.presented.focus_visible)
    }

    /// The resolved interaction-state inputs — what the visual would be
    /// IF (pressed,hot,focus) held these values. Callers snapshot it
    /// BEFORE and AFTER flipping the state fields.
    pub(super) fn interact(&self, id: NodeId) -> InteractState {
        let p = self
            .rt
            .arena
            .get(id)
            .map(|n| n.presented)
            .unwrap_or_default();
        InteractState {
            pressed: self.pressed == Some(id) || p.pressed,
            hot: self.hot == Some(id) || p.hover,
            focus: self.focus == Some(id) || p.focus_visible,
        }
    }

    /// Resolve the node's effective visual under a hypothetical
    /// interaction state — the compare target for transition
    /// classification. Same recipe+patch+state chain the renderer runs.
    pub(super) fn resolved_at(
        &self,
        id: NodeId,
        st: InteractState,
    ) -> Option<crate::style::VisualStyle> {
        let n = self.rt.arena.get(id)?;
        let dark = self.rt.theme.dark;
        let forced = self.rt.forced_resolver();
        match &n.data {
            NodeData::Button {
                variant,
                size,
                style,
                disabled,
                invalid,
                selected,
                ..
            } => {
                let state = crate::style::StyleState::classify(*disabled, st.pressed, st.hot);
                let mut v = crate::style::resolve_button(
                    *variant, *size, style, state, st.focus, dark, *selected, *invalid,
                );
                crate::style::os_enforce_visual(&mut v, forced);
                Some(v)
            }
            NodeData::Action {
                style, disabled, ..
            } => {
                let mut b = style.resolve(*disabled, st.pressed, st.hot, st.focus);
                crate::style::os_enforce_box(&mut b, forced);
                Some(crate::style::VisualStyle {
                    box_style: b,
                    text_style: crate::style::TextStyle::default(),
                    focus_ring: crate::style::Color::Role(crate::theme::ColorRole::Focus),
                })
            }
            NodeData::Toggle {
                kind,
                size,
                checked,
                invalid,
                disabled,
                ..
            } => {
                let state = crate::style::StyleState::classify(*disabled, st.pressed, st.hot);
                let mut v = crate::style::resolve_toggle_visual(
                    *kind, *size, *checked, *invalid, state, st.focus, dark,
                );
                crate::style::os_enforce_visual(&mut v, forced);
                Some(v)
            }
            NodeData::Editor { patch, invalid, .. } => {
                let mut b = crate::style::resolve_text_input_chrome(patch);
                crate::style::os_enforce_box(&mut b, forced);
                Some(crate::style::VisualStyle {
                    box_style: b,
                    text_style: crate::style::TextStyle::default(),
                    focus_ring: if *invalid {
                        crate::style::Color::Role(crate::theme::ColorRole::DestructiveRing)
                    } else {
                        crate::style::Color::Role(crate::theme::ColorRole::Focus)
                    },
                })
            }
            _ => None,
        }
    }

    /// An interaction-state transition on `id` — classify by the ACTUAL
    /// old->new resolved visual difference, never by "some branch
    /// somewhere has metrics". Identical resolved output = no work;
    /// metric-bearing fields moving = LAYOUT|PAINT; paint-only = PAINT.
    pub(super) fn mark_state_dirty(&self, id: NodeId, old: InteractState) {
        let new = self.interact(id);
        let bits = match (self.resolved_at(id, old), self.resolved_at(id, new)) {
            (Some(a), Some(b)) => {
                let mut d = 0u8;
                // THE shared classifier — text metric diffs are LAYOUT,
                // never silently downgraded to paint
                crate::node::visual_dirty(&a, &b, self.rt.theme.dark, &mut d);
                d
            }
            (None, None) => 0b10, // unstyled node — repaint (unchanged rule)
            _ => 0b11,
        };
        self.state_dirty.set(self.state_dirty.get() | bits);
    }

    /// WM_MOUSEMOVE: hot tracking (Enter/Leave) + raw move to hovered peer.
    pub(crate) fn hover(&mut self, pos_px: space::ClientPhysicalPoint, wparam: usize) -> UiResult {
        let pos: Point = {
            let s = self.peer_ctx.scale.get();
            pos_px.0.logical(s)
        };
        self.mouse = pos;
        let hit = if self.native_capture.is_some() {
            self.native_capture
        } else {
            self.hit_test(pos)
        };
        if hit != self.hot {
            let old_hot = self.hot;
            let olds: Vec<(NodeId, InteractState)> = [old_hot, hit]
                .into_iter()
                .flatten()
                .map(|id| (id, self.interact(id)))
                .collect();
            if let Some(old) = old_hot
                && self.peer_for(old).is_none()
            {
                self.push_input(
                    old,
                    NodeEvent::Pointer(
                        PointerEvent {
                            position: pos,
                            button: None,
                            modifiers: modifiers_now(),
                        },
                        crate::node::PointerPhase::Leave,
                    ),
                )?;
                self.rt.sched_unarm_tooltip(old);
            }
            self.hot = hit;
            for (id, old) in olds {
                self.mark_state_dirty(id, old);
            }
            if let Some(id) = hit {
                if self.peer_for(id).is_none() {
                    self.push_input(
                        id,
                        NodeEvent::Pointer(
                            PointerEvent {
                                position: pos,
                                button: None,
                                modifiers: Modifiers::default(),
                            },
                            crate::node::PointerPhase::Enter,
                        ),
                    )?;
                }
                if self.rt.node_has_tooltip(id) {
                    self.rt.sched_arm_tooltip(id);
                }
            }
        }
        // hovered editor receives the raw move (hover states, drag select)
        // — peer-local = content-local via the shared transform, in px
        if let Some(id) = hit
            && self.peer_for(id).is_some()
        {
            let Some(origin) = self.peer_origin(id) else {
                return Err(crate::UiError::Platform(
                    "peer origin unrepresentable in physical px".into(),
                ));
            };
            let Some(lp) = origin
                .to_local(pos_px)
                .and_then(|l| space::try_lparam_px(l.0))
            else {
                return Err(crate::UiError::Platform(
                    "peer-local coordinate outside LPARAM range".into(),
                ));
            };
            // wparam carries the real MK_* flags (drag-select needs MK_LBUTTON)
            self.deliver_native(id, WM_MOUSEMOVE, wparam, lp)?;
            self.service_peer_events()?;
        }
        self.turn()
    }

    /// Pointer left the window — hot leaves, tooltip disarms.
    pub(crate) fn leave(&mut self) -> UiResult {
        if let Some(old) = self.hot.take() {
            // hover-left is a transition — the old hot=true resolved
            // visual may differ from the cleared state
            self.mark_state_dirty(
                old,
                InteractState {
                    pressed: self.pressed == Some(old),
                    hot: true,
                    focus: self.focus == Some(old),
                },
            );
            if self.peer_for(old).is_none() {
                self.push_input(
                    old,
                    NodeEvent::Pointer(
                        PointerEvent {
                            position: self.mouse,
                            button: None,
                            modifiers: modifiers_now(),
                        },
                        crate::node::PointerPhase::Leave,
                    ),
                )?;
            }
            self.rt.sched_unarm_tooltip(old);
        }
        self.turn()
    }
}
