impl WindowlessPeer {
    pub(crate) fn text(&self) -> UiResult<String> {
        unsafe {
            let mut b = BSTR::default();
            self.tx()
                .TxGetText(&mut b)
                .map_err(|e| UiError::Platform(format!("TxGetText: {e}")))?;
            let s = b.to_string();
            Ok(s)
        }
    }

    /// Selection as native UTF-16 (anchor, focus) char offsets — the
    /// direction is preserved via ITextDocument's selection flags, not
    /// EM_EXGETSEL's sorted cpMin/cpMax.
    /// whole-story length in UTF-16 units — clamp bound for staged ranges
    /// so an out-of-range author value can't fight read-back idempotence
    pub(crate) fn story_len_utf16(&self) -> UiResult<usize> {
        unsafe {
            let mut doc_raw: *mut c_void = std::ptr::null_mut();
            let vt = Interface::vtable(self.tx());
            (vt.base__.base__.QueryInterface)(
                self.tx().as_raw(),
                &ITextDocument::IID as *const GUID,
                &mut doc_raw,
            )
            .ok()
            .map_err(|e| UiError::Platform(format!("QI(ITextDocument): {e}")))?;
            let doc = ITextDocument::from_raw(doc_raw);
            // tomStory=6 is a UNIT (MoveEnd expands to the story end),
            // not a char position — Range(0,6) would mean "chars 0..6"
            let range = doc
                .Range(0, 0)
                .map_err(|e| UiError::Platform(format!("Range: {e}")))?;
            range
                .MoveEnd(6, 1)
                .map_err(|e| UiError::Platform(format!("MoveEnd(tomStory): {e}")))?;
            let end = range
                .GetEnd()
                .map_err(|e| UiError::Platform(format!("GetEnd: {e}")))?;
            Ok(end.max(0) as usize)
        }
    }

    pub(crate) fn selection_utf16(&self) -> UiResult<(usize, usize)> {
        unsafe {
            let mut doc_raw: *mut c_void = std::ptr::null_mut();
            let vt = Interface::vtable(self.tx());
            (vt.base__.base__.QueryInterface)(
                self.tx().as_raw(),
                &ITextDocument::IID as *const GUID,
                &mut doc_raw,
            )
            .ok()
            .map_err(|e| UiError::Platform(format!("QI(ITextDocument): {e}")))?;
            let doc = ITextDocument::from_raw(doc_raw);
            let sel: ITextSelection = doc
                .GetSelection()
                .map_err(|e| UiError::Platform(format!("GetSelection: {e}")))?;
            let flags = sel
                .GetFlags()
                .map_err(|e| UiError::Platform(format!("GetFlags: {e}")))?;
            let (cp_start, cp_end) = (
                sel.GetStart()
                    .map_err(|e| UiError::Platform(format!("GetStart: {e}")))?,
                sel.GetEnd()
                    .map_err(|e| UiError::Platform(format!("GetEnd: {e}")))?,
            );
            // tomSelStartActive: START is the focus (direction preserved —
            // EM_EXGETSEL's sorted cpMin/cpMax would lose it)
            let (anchor, focus) = if (flags & tomSelStartActive.0) != 0 {
                (cp_end, cp_start)
            } else {
                (cp_start, cp_end)
            };
            Ok((anchor as usize, focus as usize))
        }
    }

    /// Draw the peer's content directly into the window's D2D target.
    /// `bounds` is the peer's CONTENT rect in window DIP
    /// (`editor_content_rect` is the single shared transform). `lprcBounds`
    /// for `TxDrawD2D` is in the host's PHYSICAL-PIXEL space — msftedit
    /// divides by `dcDpi/96` internally, so `LogicalRect::rectl` performs THE
    /// conversion at this seam. The format space stays the peer's LOCAL
    /// client rect (px); one transform at the seam, nothing to cache.
    /// THE effective peer geometry — one authority for every consumer.
    /// While a composition-pending relatch is parked, `host.bounds` and
    /// `host.scale` still hold the OLD values — that IS the effective
    /// geometry msftedit renders in; pointer/caret/draw/measure must read
    /// it, never the pending new values.
    pub(crate) fn effective_geometry(&self) -> (LogicalRect, ScaleFactor) {
        let s = self.shared();
        (s.host.bounds, s.host.scale)
    }

    /// Draw the peer's content into the window's D2D target.
    /// `bounds` is the peer's CONTENT rect in window DIP; the peer draws
    /// where its effective geometry lives (`host.bounds`, unchanged while
    /// a composition relatch is parked) — the layout arg is advisory only.
    ///
    /// OFFSCREEN BLIT (the scratch compat-target path): `TxDrawD2D`
    /// renders the FULL document into this peer's OWN bitmap render
    /// target at surface-local `(0,0,w_px,h_px)` — a fresh per-peer
    /// target carries its own draw state, so it does not share whatever
    /// shared-target state made some peers emit zero pixels — then the
    /// frame `DrawBitmap`s the surface at the content rect. The blit is
    /// an ordinary draw op, so the frame's pushed axis-aligned clip cuts
    /// the visible band (a clip pushed around `TxDrawD2D` suppresses it
    /// entirely — deviation 1); a top-clipped editor therefore shows the
    /// MID-document rows that intersect the viewport, not the document
    /// head (deviation 12). The surface is reused across frames and
    /// recreated only when the peer's px size or the frame target's
    /// generation changes.
    pub(crate) fn draw(
        &self,
        rt: &ID2D1RenderTarget,
        _bounds: LogicalRect,
        target_gen: u64,
    ) -> UiResult<()> {
        self.ensure_in_place()?;
        let (bounds, sc) = self.effective_geometry();
        if !(bounds.width > 0.0
            && bounds.height > 0.0
            && bounds.x.is_finite()
            && bounds.y.is_finite())
        {
            return Ok(());
        }
        // surface extent = the client rect's snapped physical size — the
        // same edge-snapped mapping `TxGetClientRect` reports, so the
        // bitmap covers the format space 1:1
        let surface_px = LogicalRect::local(bounds.width, bounds.height).physical(sc);
        let w_px = surface_px.right - surface_px.left;
        let h_px = surface_px.bottom - surface_px.top;
        if w_px <= 0 || h_px <= 0 {
            return Ok(());
        }
        let surface = {
            let mut slot = self.surface.borrow_mut();
            let stale = slot.as_ref().map_or(true, |s| {
                s.target_gen != target_gen || s.w_px != w_px || s.h_px != h_px
            });
            if stale {
                let fmt = D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    // premultiplied alpha — the doc's TRANSPARENT backstyle
                    // must survive the blit; alpha-IGNORE would stamp an
                    // opaque black box over the painted pill chrome
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                };
                let created = unsafe {
                    rt.CreateCompatibleRenderTarget(
                        None,
                        Some(&D2D_SIZE_U {
                            width: w_px as u32,
                            height: h_px as u32,
                        }),
                        Some(&fmt),
                        D2D1_COMPATIBLE_RENDER_TARGET_OPTIONS_NONE,
                    )
                }
                .map_err(|e| UiError::Platform(format!("peer surface rendertarget: {e}")))?;
                *slot = Some(PeerSurface {
                    rt: created,
                    w_px,
                    h_px,
                    target_gen,
                });
            }
            slot.as_ref().unwrap().rt.clone()
        };
        unsafe {
            let up: ID2D1RenderTarget = surface
                .cast()
                .map_err(|e| UiError::Platform(format!("peer surface cast: {e}")))?;
            up.BeginDraw();
            up.Clear(Some(&D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            }));
            // `lprcBounds` is host-physical-px in the DRAW target's own
            // space — the document anchors at the surface origin and the
            // client rect (already px via TxGetClientRect) is the extent
            let mut rc = RECTL {
                left: 0,
                top: 0,
                right: w_px,
                bottom: h_px,
            };
            let drawn = self.tx().TxDrawD2D(&up, &mut rc, std::ptr::null_mut(), 0);
            let ended = up.EndDraw(None, None);
            drawn
                .and(ended)
                .map_err(|e| UiError::Platform(format!("TxDrawD2D(peer surface): {e}")))?;
            let bmp = surface
                .GetBitmap()
                .map_err(|e| UiError::Platform(format!("peer surface bitmap: {e}")))?;
            // The bitmap's true extent is the SNAPPED PHYSICAL-PIXEL
            // bounds — land it through the FRAME target's own dpi so the
            // ink sits exactly on the snapped px (identical to `bounds`
            // when the peer scale equals the frame dpi; the px-space
            // contract probe proves the mapping when they differ)
            let (mut fx, mut fy) = (0.0f32, 0.0f32);
            rt.GetDpi(&mut fx, &mut fy);
            let fsc = if fx.is_finite() && fx > 0.0 {
                fx / 96.0
            } else {
                sc.0
            };
            let px = bounds.physical(sc);
            let dest = D2D_RECT_F {
                left: px.left as f32 / fsc,
                top: px.top as f32 / fsc,
                right: px.right as f32 / fsc,
                bottom: px.bottom as f32 / fsc,
            };
            rt.DrawBitmap(
                &bmp,
                Some(&dest),
                1.0,
                D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
                None,
            );
        }
        Ok(())
    }

    /// Natural content size in DIP at the given layout width (REQRESIZE).
    /// msftedit formats in the client width, so the width must be current
    /// before measuring; a tall scratch bottom gives it room to report.
    pub(crate) fn natural_size(&self, width_dip: f32) -> UiResult<(f32, f32)> {
        {
            let mut s = self.shared_mut();
            // the effective geometry authority is `host.bounds` — while
            // COMPOSING (or a relatch already parked) it holds the old
            // space msftedit still renders in. Layout measures BEFORE
            // apply_bounds, so gating on `pending_relatch` alone installs
            // the freeze too late: a mid-composition width write would
            // corrupt host callbacks' coordinate space.
            if !self.composing.get() && self.pending_relatch.borrow().is_none() {
                s.host.bounds.width = width_dip;
                if s.host.bounds.height <= 0.0 {
                    // scratch height — the service reports natural extent
                    // via REQRESIZE regardless of clip height
                    s.host.bounds.height = 4000.0;
                }
            }
            // freshness: a stale extent must never masquerade as a fresh
            // measurement — reset before requesting so a dropped
            // notification is an error, not last pass's numbers
            s.host.natural = SIZE::default();
        }
        self.ensure_in_place()?;
        self.send(EM_REQUESTRESIZE, 0, 0)?;
        let sc = self.shared().host.scale;
        let px = self.shared().host.natural;
        if px.cy <= 0 {
            return Err(UiError::Platform(
                "EM_REQUESTRESIZE produced no fresh extent".into(),
            ));
        }
        // per-line metric from msftedit itself — cy / EM_GETLINECOUNT is
        // content-independent; layout's cap uses this, not a magic DIP.
        // Client units are px — `px_to_dip` is THE seam.
        if px.cy > 0
            && let Ok(ns) = self.send(EM_GETLINECOUNT, 0, 0)
        {
            self.line_h
                .set(sc.to_logical(px.cy) / (ns.lr.max(1) as f32));
        }
        Ok((sc.to_logical(px.cx), sc.to_logical(px.cy)))
    }

    /// measured single-line height (DIP); 0.0 until a `natural_size` pass
    /// latches it — layout falls back to its constant until then
    pub(crate) fn line_height(&self) -> f32 {
        self.line_h.get()
    }

    /// Global bounds (window DIP) + DPI scale — updates without recreate.
    /// The first non-empty bounds activate the text service (the format
    /// space latches at activation, so activating at mount — before layout —
    /// would latch a 0×0 space and draw nothing). The client rect is a live
    /// property (`TxGetClientRect` reads `host.bounds`) — never relatched,
    /// matching the proven mascot contract.
    /// Composition flag set/cleared by the backend's ime_start/ime_end.
    pub(crate) fn set_composing(&self, v: bool) {
        self.composing.set(v);
    }

    /// The activation state — a checked state machine, not a bool.
    pub(crate) fn activation_state(&self) -> Activation {
        self.activation.get()
    }

    /// Composition has ended — apply a deferred scale relatch now.
    /// The native transition runs with composition already cleared.
    pub(crate) fn finish_pending_relatch(&self) -> UiResult {
        let Some((bounds, scale)) = self.pending_relatch.borrow_mut().take() else {
            return Ok(());
        };
        self.apply_bounds(bounds, scale)
    }

    /// Global bounds (window DIP) + DPI scale — updates without recreate.
    /// The first non-empty bounds activate the text service (the format
    /// space latches at activation, so activating at mount — before layout —
    /// would latch a 0x0 space and draw nothing). The client rect is a live
    /// property (`TxGetClientRect` reads `host.bounds`) — never relatched,
    /// matching the proven mascot contract.
    ///
    /// RELATCH ORDERING (the correctness contract):
    /// 1. snapshot the selection BEFORE any native transition (checked —
    ///    a failed snapshot aborts the relatch rather than losing it);
    /// 2. deactivate under the CURRENT (old) latched space — the state
    ///    flags only move AFTER the native transition succeeds, and a
    ///    partial failure records the activation level actually reached;
    /// 3. only then are `host.bounds`/`host.scale` written — bookkeeping
    ///    never precedes the native transition;
    /// 4. reactivate, restore the selection (sorted extent + active end),
    ///    restore UI activation.
    /// While composition is active the whole relatch is DEFERRED —
    /// neither the latched space nor the bookkeeping is touched.
    pub(crate) fn apply_bounds(&self, bounds: LogicalRect, scale: ScaleFactor) -> UiResult {
        if self.composing.get() {
            // an active IME composition owns the effective geometry —
            // EVERY write defers whole (bounds AND scale together) until
            // ime_end: measurement, same-scale moves and scale relatch
            // all share the one frozen space msftedit still renders in
            *self.pending_relatch.borrow_mut() = Some((bounds, scale));
            return Ok(());
        }
        let prev = self.shared().host.scale;
        let needs_relatch = self.activation.get() != Activation::Inactive && prev != scale;
        if !needs_relatch {
            {
                let mut s = self.shared_mut();
                s.host.bounds = bounds;
                s.host.scale = scale;
            }
            return self.ensure_in_place();
        }
        // 1. selection snapshot — checked; abort rather than lose it
        let sel = self.selection_utf16()?;
        let was_ui = self.activation.get() == Activation::Ui;
        // 2. deactivate under the OLD latched space
        unsafe {
            if was_ui {
                self.tx()
                    .OnTxUIDeactivate()
                    .map_err(|e| UiError::Platform(format!("OnTxUIDeactivate: {e}")))?;
                self.activation.set(Activation::InPlace);
            }
            if let Err(e) = self.tx().OnTxInPlaceDeactivate() {
                // truth: native reached InPlace — record what happened
                self.activation.set(Activation::InPlace);
                return Err(UiError::Platform(format!("OnTxInPlaceDeactivate: {e}")));
            }
        }
        self.activation.set(Activation::Inactive);
        // 3. now the bookkeeping may move
        {
            let mut s = self.shared_mut();
            s.host.bounds = bounds;
            s.host.scale = scale;
        }
        // 4. reactivate + restore (checked)
        self.ensure_in_place()?;
        self.restore_selection_utf16(sel.0, sel.1)?;
        if was_ui {
            self.set_ui_active(true)?;
        }
        Ok(())
    }

}
