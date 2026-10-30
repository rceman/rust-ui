use super::*;

impl<S, M, U, V> Backend<S, M, U, V>
where
    M: 'static,
    U: Fn(&mut S, M, &mut UpdateCtx<'_, M>),
    V: Fn(&S, &mut crate::Ui<'_, '_, M>),
{
    /// THE checked wake — every deferred-work continuation routes here.
    /// `pump_queued` is set ONLY when the post actually lands; a failed
    /// post rolls the flag back and surfaces a typed error — the caller's
    /// fatal path propagates it. Guaranteed-progress semantics are real.
    /// Sync the top-level non-client caption to the resolved dark
    /// appearance via `DWMWA_USE_IMMERSIVE_DARK_MODE`. Receives a generic
    /// resolved light/dark bool — no Shadcn/theme types cross this
    /// platform boundary. Only re-issues the attribute on change.
    pub(super) fn sync_dark_caption(&self, dark: bool) {
        if self.hwnd.0.is_null() || self.dark_caption.get() == Some(dark) {
            return;
        }
        let val: BOOL = BOOL::from(dark);
        unsafe {
            let _ = DwmSetWindowAttribute(
                self.hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &val as *const _ as *const c_void,
                std::mem::size_of::<BOOL>() as u32,
            );
            // the ~7px DWM-owned resize frame edge reads as a black strip
            // around left/right/bottom — synchronize it with the resolved
            // window surface (Shadcn zinc dark 0x18181B / light 0xFAFAFA,
            // COLORREF is 0x00BBGGRR). Older Windows ignores the attribute.
            const DWMWA_BORDER_COLOR: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(34);
            let border: u32 = if dark { 0x001B1818 } else { 0x00FAFAFA };
            let _ = DwmSetWindowAttribute(
                self.hwnd,
                DWMWA_BORDER_COLOR,
                &border as *const _ as *const c_void,
                std::mem::size_of::<u32>() as u32,
            );
        }
        self.dark_caption.set(Some(dark));
    }

    /// `WM_TIMER` on a peer-owned id: forward into the peer through the
    /// delivery contract, drain the host events the callback produced.
    /// `WM_TIMER` on a peer-owned id: forward into the peer through the
    /// delivery contract, drain the host events the callback produced.
    /// Periodic: the native timer stays armed until msftedit kills it —
    /// a tick does NOT consume the registration.
    pub(crate) fn native_timer_fire(&mut self, tid: usize) -> UiResult {
        let Some((node, nid)) = self.peer_ctx.timer_pool.lock().unwrap().owner(tid) else {
            return Ok(()); // stale/tombstoned timer id — reject
        };
        self.counters
            .native_timer_fires
            .fetch_add(1, Ordering::Relaxed);
        self.deliver_native(node, WM_TIMER, nid as usize, 0)?;
        self.service_peer_events()?;
        Ok(())
    }

    /// The single framework deadline timer — armed to the sched's earliest
    /// deadline; killed when there's no demand (no persistent polling).
    /// Rearm the framework deadline. A FAILED `SetTimer` is surfaced as a
    /// typed error — silently storing `None` leaves scheduled work asleep
    /// with no guarantee another turn ever runs.
    pub(crate) fn rearm_deadline(&mut self) -> UiResult {
        const DEADLINE_ID: usize = 1;
        let next = self.rt.next_deadline();
        match (self.deadline_timer, next) {
            (Some(_), Some(n)) => {
                let due = n
                    .saturating_duration_since(std::time::Instant::now())
                    .as_millis() as u32;
                let armed = unsafe {
                    let _ = KillTimer(Some(self.hwnd), DEADLINE_ID);
                    SetTimer(Some(self.hwnd), DEADLINE_ID, due.max(1), None) != 0
                };
                if !armed {
                    self.deadline_timer = None;
                    return Err(UiError::Platform(
                        "SetTimer(deadline) failed — scheduled work would sleep".into(),
                    ));
                }
                self.deadline_timer = Some(n);
            }
            (Some(_), None) => {
                unsafe {
                    let _ = KillTimer(Some(self.hwnd), DEADLINE_ID);
                }
                self.deadline_timer = None;
            }
            (None, Some(n)) => {
                let due = n
                    .saturating_duration_since(std::time::Instant::now())
                    .as_millis() as u32;
                let armed =
                    unsafe { SetTimer(Some(self.hwnd), DEADLINE_ID, due.max(1), None) != 0 };
                if !armed {
                    return Err(UiError::Platform(
                        "SetTimer(deadline) failed — scheduled work would sleep".into(),
                    ));
                }
                self.deadline_timer = Some(n);
            }
            (None, None) => {}
        }
        Ok(())
    }

    /// The deadline timer fired — drain sched work (frames + chrome) once.
    pub(crate) fn deadline_fire(&mut self) -> UiResult {
        self.deadline_timer = None;
        self.counters
            .animation_timer_fires
            .fetch_add(1, Ordering::Relaxed);
        self.turn()
    }

    // ----- layout / paint ---------------------------------------------------

    /// Relayout the whole tree — rect cache over LIVE NodeIds only.
    /// HWND-gated: a synchronous WM_SIZE inside CreateWindowExW arrives
    /// before the peer ctx has a window — layout defers to the first real
    /// turn rather than failing on a null HWND.
    /// append one line to the sizing-session trace when RUI_RESIZE_TRACE
    /// is set — append-open per line keeps the sink cheap at drag rates
    pub(crate) fn resize_trace(&self, line: String) {
        if let Some(p) = &self.resize_trace_path {
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open(p)
            {
                use std::io::Write;
                let _ = writeln!(f, "{line}");
            }
        }
    }

    pub(crate) fn relayout(&mut self) -> UiResult {
        let t = std::time::Instant::now();
        let r = self.relayout_inner();
        ResizeCounters::bump(
            &RESIZE.layout_runs,
            &RESIZE.layout_us,
            &RESIZE.layout_max_us,
            t.elapsed().as_micros() as u64,
        );
        r
    }

    pub(super) fn relayout_inner(&mut self) -> UiResult {
        if self.peer_ctx.hwnd.get().is_invalid() {
            return Ok(());
        }
        // state branches can consume content insets — expose the live
        // interaction state so layout resolves the same style the paint does
        self.peer_ctx.hot.set(self.hot);
        self.peer_ctx.pressed.set(self.pressed);
        self.peer_ctx.focus.set(self.focus);
        let mut layout = LayoutCache::new(self.peer_ctx.scale.get());
        let (rects, order, clips) = layout.run(&mut self.rt, &self.peer_ctx)?;
        // record the client this layout actually solved — only after a
        // successful run, so a failed relayout can't mark a stale layout
        // as current for the WM_PAINT stale check
        if let Ok(sz) = layout::client_dip(self.peer_ctx.hwnd.get(), self.peer_ctx.scale.get()) {
            self.laid_client.set(sz);
        }
        // prune stale ids — backend cache holds live NodeIds only
        self.rects = rects;
        self.order = order;
        self.clips = clips;
        // Damage contract: compute the PROPOSED ink for the new layout and
        // union old-committed + proposed into `damage` — but do NOT
        // install it. `committed_ink` is owned exclusively by successful
        // paint completion; a relayout that never reaches a working paint
        // must not replace the last-painted truth.
        let proposed = self.ink_map();
        self.accumulate_ink_damage(&proposed);
        Ok(())
    }

    /// Union the OLD committed ink and a NEW proposed map into `damage`
    /// for every changed/moved/removed id — and brand-new nodes too.
    /// Reads, never writes, the committed snapshot.
    pub(super) fn accumulate_ink_damage(&self, new_ink: &HashMap<NodeId, LogicalRect>) {
        let ink = self.committed_ink.borrow();
        for (&id, &old) in ink.iter() {
            let dr = match new_ink.get(&id) {
                Some(&new) if new != old => old.union(new),
                Some(_) => continue,
                None => old,
            };
            let acc = self.damage.get().map(|d| d.union(dr)).unwrap_or(dr);
            self.damage.set(Some(acc));
        }
        for (&id, &new) in new_ink.iter() {
            if !ink.contains_key(&id) {
                let acc = self.damage.get().map(|d| d.union(new)).unwrap_or(new);
                self.damage.set(Some(acc));
            }
        }
    }

    /// THE committed-ink authority — per live node, the footprint a paint
    /// actually deposits: node rect UNION resolved-state shadow ink. The
    /// resolution runs through `resolved_at` with the LIVE interaction
    /// state — a hover/pressed-driven shadow change produces real
    /// old/new footprints, not an all-false approximation. The node rect
    /// IS the paint clip (this renderer clips per node rect), so the
    /// recorded footprint is already clip-bounded.
    pub(super) fn ink_map(&self) -> HashMap<NodeId, LogicalRect> {
        let mut m: HashMap<NodeId, LogicalRect> = HashMap::new();
        for (&id, &r) in self.rects.iter() {
            m.insert(id, self.ink_of(id, r));
        }
        m
    }

    /// One node's ink footprint — layout rect UNION resolved shadow ink
    /// (same resolution path as ink_map: interactive first, then the
    /// non-interactive chrome kinds resolved_at does not cover).
    pub(super) fn ink_of(&self, id: NodeId, r: LogicalRect) -> LogicalRect {
        let shadow = self
            .resolved_at(id, self.interact(id))
            .and_then(|v| v.box_style.shadow)
            .or_else(|| {
                // editors resolve chrome (non-interactive) — the
                // resolved_at path does not cover them
                match self.rt.arena.get(id).map(|n| &n.data) {
                    Some(crate::node::NodeData::Editor { patch, .. }) => {
                        layout::editor_chrome(patch).shadow
                    }
                    Some(crate::node::NodeData::Container { kind, props }) => {
                        props.resolved_box(*kind).and_then(|b| b.shadow)
                    }
                    Some(crate::node::NodeData::Popup { props, .. }) => props
                        .resolved_box(crate::node::KIND_POPUP)
                        .and_then(|b| b.shadow),
                    _ => None,
                }
            });
        match shadow {
            Some(sh) => r.union(render::shadow_ink_rect(&r, &sh)),
            None => r,
        }
    }

    /// Mark one node's full ink footprint as damaged — the OLD committed
    /// footprint union the current one (covers a moved/shrunk/removed
    /// shadow). Used for paint-classified changes and chrome steps
    /// (transition repaints) where no ink-map diff exists to catch.
    pub(super) fn mark_ink_damage(&self, id: NodeId) {
        let Some(r) = self.rects.get(&id).copied() else {
            return;
        };
        let ink = self.ink_of(id, r);
        let dr = self
            .committed_ink
            .borrow()
            .get(&id)
            .copied()
            .map(|old| old.union(ink))
            .unwrap_or(ink);
        let acc = self.damage.get().map(|d| d.union(dr)).unwrap_or(dr);
        self.damage.set(Some(acc));
    }

    /// Present the frame — D2D target, axis-aligned clips, chrome paint.
    /// `committed_ink` is owned EXCLUSIVELY by successful paint
    /// completion: the proposed map is computed against the visuals this
    /// paint resolved (live interact state), damage unions old+new, and
    /// only a draw that SUCCEEDED replaces the painted snapshot. A failed
    /// draw leaves the previous committed ink and the damage intact.
    pub(crate) fn paint(&mut self) -> UiResult {
        self.paint_inner(|be| be.renderer.borrow_mut().draw(be))
    }

    pub(super) fn paint_inner(&mut self, draw: impl FnOnce(&Self) -> UiResult) -> UiResult {
        let t = std::time::Instant::now();
        let r = self.paint_inner_impl(draw);
        ResizeCounters::bump(
            &RESIZE.paint_runs,
            &RESIZE.paint_us,
            &RESIZE.paint_max_us,
            t.elapsed().as_micros() as u64,
        );
        r
    }

    pub(super) fn paint_inner_impl(&mut self, draw: impl FnOnce(&Self) -> UiResult) -> UiResult {
        if self.hwnd.0.is_null() {
            return Ok(());
        }
        let new_ink = self.ink_map();
        self.accumulate_ink_damage(&new_ink);
        draw(&*self)?;
        // commit boundary — ONLY here does proposed ink become the
        // last-painted snapshot, and damage is consumed
        *self.committed_ink.borrow_mut() = new_ink;
        self.damage.set(None);
        #[cfg(feature = "devtools")]
        self.paint_epoch.set(
            self.paint_epoch
                .get()
                .checked_add(1)
                .ok_or_else(|| UiError::Platform("devtools paint epoch exhausted".into()))?,
        );
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn paint_for_test(&mut self, draw_ok: bool) -> UiResult {
        self.paint_inner(|_| {
            if draw_ok {
                Ok(())
            } else {
                Err(UiError::Platform("injected paint failure".into()))
            }
        })
    }

    /// The accumulated damage union the next paint will clip to — read
    /// by the renderer for bounded invalidation (and by tests).
    pub(crate) fn damage_rect(&self) -> Option<LogicalRect> {
        self.damage.get()
    }

    #[cfg(test)]
    pub(crate) fn committed_ink_map(&self) -> HashMap<NodeId, LogicalRect> {
        self.committed_ink.borrow().clone()
    }

    // ----- tooltip ------------------------------------------------------------

    /// Tooltip semantic delay elapsed — the runtime control property fired;
    /// show the owned nonactivating overlay at the node's rect.
    pub(super) fn show_tooltip(&mut self, node: NodeId) {
        if !self.rt.arena.is_live(node) {
            return;
        }
        let text = {
            let n = self.rt.arena.get(node).unwrap();
            match &n.data {
                NodeData::Button {
                    tooltip: Some(t), ..
                } => t.to_string(),
                _ => return,
            }
        };
        self.tooltip_for = Some(node);
        let _ = text; // renderer consumes on paint
        // the overlay shows after the semantic delay — reduce instant-fires
        // it at the same deadline
        self.renderer
            .borrow_mut()
            .show_tooltip(&text, self.hwnd, self.peer_ctx.scale.get());
    }

    pub(crate) fn hide_tooltip(&mut self) {
        self.tooltip_for = None;
        self.renderer.borrow_mut().hide_tooltip();
    }

    // ----- input routing -----------------------------------------------------

    /// `WM_SETTINGCHANGE` — resnapshot OS appearance AND re-resolve an
    /// authored `ReducedMotion::System` (the SPI value may have flipped
    /// without a theme restage).
    pub(crate) fn os_change(&mut self) -> UiResult {
        let appearance = os_appearance();
        self.rt.set_appearance(appearance);
        self.rt.refresh_reduced();
        *self.peer_ctx.colors.borrow_mut() = (appearance, self.rt.theme.clone());
        self.turn()
    }

    /// DPI changed — scale update + full damage, no recreate
    pub(crate) fn dpi_changed(&mut self, dpi: u32) -> UiResult {
        let scale = space::scale_from_dpi(dpi);
        self.peer_ctx.scale.set(scale);
        self.renderer.borrow_mut().set_dpi(scale);
        self.relayout()
    }
}
