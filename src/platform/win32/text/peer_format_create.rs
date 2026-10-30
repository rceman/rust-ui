struct PeerSurface {
    rt: ID2D1BitmapRenderTarget,
    w_px: i32,
    h_px: i32,
    /// renderer `target_gen` this surface was created for
    target_gen: u64,
}

/// One windowless RichEdit instance — the `TextPeer` implementation the
/// backend's `peer_factory` produces. UI-thread only.
pub(crate) struct WindowlessPeer {
    tx: Option<ITextServices2>,
    host: *mut HostBox,
    /// opaque peer id for the runtime's logging/diagnostics
    id: u64,
    /// owning node (generation-checked when routing events back)
    node: NodeId,
    /// the peer mirror revision: last applied/committed revision the CORE
    /// agreed on — native edits tag their base with it
    peer_rev: Cell<TextRevision>,
    /// mount binding — tags every emitted edit/conflict
    binding: Option<BindingToken>,
    /// ui-side sink the backend drains each turn (node->event)
    sink: std::sync::Arc<std::sync::Mutex<Vec<super::NativeSinkItem>>>,
    /// multiline vs single-line init
    multiline: bool,
    /// authored line box in twips as latched at create — snapshot introspection
    line_twips: i32,
    /// THE TextServices activation state — NOT one boolean (F04):
    /// InPlace = formatted against the px client space (measure/draw);
    /// Ui = additionally UI-active — focus owner only (msftedit allows a
    /// single UI-active control per container).
    activation: Cell<Activation>,
    /// IME composition owns this peer's UI activation — a scale relatch
    /// (deactivate + TOM mutation) is DEFERRED until composition ends
    /// rather than interrupting marked input
    pub(crate) composing: Cell<bool>,
    /// bounds/scale waiting on composition end — the latched format space
    /// must stay internally consistent, so BOTH move together, never
    /// piecemeal before the native transition
    pending_relatch: std::cell::RefCell<Option<(LogicalRect, ScaleFactor)>>,
    /// msftedit-derived single-line height in DIP — `natural_size` reports
    /// cy = line_count × line; dividing by EM_GETLINECOUNT gives the real
    /// metric regardless of content. 0 = unmeasured.
    line_h: Cell<f32>,
    /// lazily-created composition surface (`TxDrawD2D` target + blit
    /// source) — one per peer, reused across frames, resized with the
    /// content rect and invalidated by frame-target generation
    surface: RefCell<Option<PeerSurface>>,
}

impl WindowlessPeer {
    fn shared(&self) -> std::cell::Ref<'_, PeerShared> {
        unsafe { (*self.host).state.borrow() }
    }
    fn shared_mut(&self) -> std::cell::RefMut<'_, PeerShared> {
        unsafe { (*self.host).state.borrow_mut() }
    }
    fn tx(&self) -> &ITextServices2 {
        self.tx.as_ref().unwrap()
    }

    /// owning node id (set by `attach` after mount)
    pub(crate) fn node(&self) -> NodeId {
        self.node
    }
    pub(crate) fn node_opt(&self) -> Option<NodeId> {
        (self.node.slot != u32::MAX).then_some(self.node)
    }
    /// the peer mirror — last core-agreed revision for native base tags
    pub(crate) fn peer_rev(&self) -> TextRevision {
        self.peer_rev.get()
    }
    pub(crate) fn binding(&self) -> Option<BindingToken> {
        self.binding
    }
    /// the core accepted a native commit — advance the mirror
    pub(crate) fn record_commit(&self, rev: TextRevision) {
        self.peer_rev.set(rev);
    }
    /// exported windowless-accessibility IID (None when msftedit lacks it)
    pub(crate) fn windowless_acc_iid(&self) -> Option<windows::core::GUID> {
        self.shared().lib.iid_windowless_acc
    }

    /// Re-style live content: patch the host default char format and push
    /// it over ALL existing text. The stack copy is what EM_SETCHARFORMAT
    /// consumes — the host borrow never crosses a `send` (msftedit calls
    /// back into the host synchronously).
    /// Push the resolved editable foreground over ALL content. The only
    /// style a native peer accepts — size/weight/face stay OS-owned.
    /// `authored` is the consumer's `Color` — roles re-resolve on theme
    /// flips; concrete literals are invariant by construction.
    pub(crate) fn apply_format(&mut self, fg: COLORREF, authored: crate::style::Color) {
        let cf = {
            let mut sh = self.shared_mut();
            sh.host.fg_authored = authored;
            sh.host.cf.crTextColor = fg;
            sh.host.fg = fg;
            *sh.host.cf
        };
        let _ = self.send(EM_SETCHARFORMAT, SCF_ALL as usize, &cf as *const _ as isize);
        self.apply_font_tom();
    }

    /// Push the configured face+size through TOM — the only channel
    /// msftedit honors for font metrics under TXTBIT_D2DDWRITE
    /// (EM_SETCHARFORMAT's szFaceName/yHeight are ignored there — the
    /// range font is the live authority). Re-applied after every text
    /// load because TxSetText resets the story to the default font.
    pub(crate) fn apply_font_tom(&self) {
        let (face_w, size_pt) = {
            let sh = self.shared();
            (sh.host.cf.szFaceName, sh.host.cf.yHeight as f32 / 20.0)
        };
        unsafe {
            let mut doc_raw: *mut c_void = std::ptr::null_mut();
            let vt = Interface::vtable(self.tx());
            if (vt.base__.base__.QueryInterface)(
                self.tx().as_raw(),
                &ITextDocument::IID as *const GUID,
                &mut doc_raw,
            )
            .is_err()
            {
                return;
            }
            let doc = ITextDocument::from_raw(doc_raw);
            let Ok(range) = doc.Range(0, 0) else { return };
            let Ok(_) = range.MoveEnd(6, 1) else { return };
            let Ok(font) = range.GetFont() else { return };
            let name: Vec<u16> = face_w
                .iter()
                .take_while(|c| **c != 0)
                .copied()
                .collect();
            let _ = font.SetName(&windows::core::BSTR::from_wide(&name));
            let _ = font.SetSize(size_pt);
        }
    }

    /// Build the host + text services for one peer.
    pub(crate) fn create(
        id: u64,
        lib: &Arc<Msftedit>,
        hwnd: HWND,
        scale: ScaleFactor,
        cfg: &PeerConfig,
        sink: std::sync::Arc<std::sync::Mutex<Vec<super::NativeSinkItem>>>,
        timer_pool: std::sync::Arc<std::sync::Mutex<TimerPool>>,
    ) -> UiResult<WindowlessPeer> {
        let mut face = [0u16; 32];
        for (i, c) in cfg.face.encode_utf16().take(31).enumerate() {
            face[i] = c;
        }
        let shared = PeerShared {
            lib: lib.clone(),
            host: HostShared {
                hwnd,
                bounds: LogicalRect::default(),
                scale,
                bits: TXTBIT_WORDWRAP
                    | TXTBIT_AUTOWORDSEL
                    | TXTBIT_DISABLEDRAG
                    | TXTBIT_D2DDWRITE
                    | TXTBIT_D2DPIXELSNAPPED
                    | if cfg.multiline { TXTBIT_MULTILINE } else { 0 }
                    | if cfg.read_only { TXTBIT_READONLY } else { 0 },
                natural: SIZE::default(),
                cf: Box::new(CHARFORMATW {
                    cbSize: std::mem::size_of::<CHARFORMATW>() as u32,
                    dwMask: CFM_ALL,
                    // CFE_AUTOCOLOR: draw-time theme recoloring without
                    // rewriting character formats on every view
                    dwEffects: CFE_EFFECTS(CFE_AUTOCOLOR.0 | if cfg.bold { CFE_BOLD.0 } else { 0 }),
                    // Font size stays authored-exact (`size_twips` =
                    // contract font px): under TXTBIT_D2DDWRITE the
                    // line pitch historically derived from yHeight,
                    // which conflated glyph size with line spacing and
                    // rendered ~9% oversized text at lh20. The exact
                    // pitch is now requested through the initial
                    // PARAFORMAT2 line-spacing fields below (measured
                    // live on this host; see the pf comment) — yHeight
                    // never carries the pitch again.
                    yHeight: cfg.size_twips,
                    yOffset: 0,
                    crTextColor: colorref(cfg.fg),
                    bCharSet: DEFAULT_CHARSET,
                    bPitchAndFamily: 0,
                    szFaceName: face,
                }),
                pf: Box::new({
                    let mut p = PARAFORMAT2::default();
                    p.Base.cbSize = std::mem::size_of::<PARAFORMAT2>() as u32;
                    // Measured live (Geist, 96dpi, P12): the initial-pf
                    // PFM_LINESPACING/rule-4 exact channel is INERT too —
                    // dyLineSpacing 360 still pitched at ~20px. Pitch is
                    // font-metric + msftedit leading only, so it now
                    // follows the exact 14px face (≈20px) with no hack.
                    p
                }),
                fg: colorref(cfg.fg),
                fg_authored: cfg.fg_authored,
                sel_bg: colorref(cfg.sel_bg),
                sel_fg: colorref(cfg.sel_fg),
                win_bg: COLORREF(0x010101),
                caret_pos: POINT::default(),
                caret_size: SIZE::default(),
                caret_shown: false,
                caret_created: false,
                armed_timers: std::collections::BTreeMap::new(),
                node: None,
                timer_pool: timer_pool.clone(),

                events: Vec::new(),
                read_only: cfg.read_only,
                overflow: false,
            },
        };
        let host = Box::into_raw(Box::new(HostBox {
            host: unsafe { &*(&VT_HOST2 as *const ITextHost2_Vtbl as *const ITextHost_Vtbl) },
            host2: &VT_HOST2,
            refs: Cell::new(1),
            state: RefCell::new(shared),
        }));
        // RAII: if CreateTextServices or the QI fails, the host box (and
        // its owned `unk`) must not leak — the guard owns it until the
        // peer is fully constructed
        struct HostGuard(*mut HostBox);
        impl Drop for HostGuard {
            fn drop(&mut self) {
                unsafe {
                    drop(Box::from_raw(self.0));
                }
            }
        }
        let guard = HostGuard(host);
        unsafe {
            let mut punk: *mut c_void = std::ptr::null_mut();
            (lib.create_text_services)(std::ptr::null_mut(), host as *mut c_void, &mut punk)
                .ok()
                .map_err(|e| UiError::Platform(format!("CreateTextServices: {e}")))?;
            let unk = IUnknown::from_raw(punk);
            let mut tx2_raw: *mut c_void = std::ptr::null_mut();
            (Interface::vtable(&unk).QueryInterface)(
                unk.as_raw(),
                &lib.iid_text_services2 as *const GUID,
                &mut tx2_raw,
            )
            .ok()
            .map_err(|e| UiError::Platform(format!("QI(ITextServices2): {e}")))?;
            let tx2 = ITextServices2::from_raw(tx2_raw);
            let mut res = LRESULT(0);
            // plain-text mode, event mask, no URL detect, bounded length
            let _ = tx2.TxSendMessage(
                EM_SETTEXTMODE,
                WPARAM(TM_PLAINTEXT.0 as usize),
                LPARAM(0),
                &mut res,
            );
            let _ = tx2.TxSendMessage(
                EM_SETEVENTMASK,
                WPARAM(0),
                LPARAM(
                    (ENM_CHANGE
                        | ENM_REQUESTRESIZE
                        | ENM_SELCHANGE
                        | ENM_KEYEVENTS
                        | ENM_MOUSEEVENTS) as isize,
                ),
                &mut res,
            );

            let _ = tx2.TxSendMessage(EM_AUTOURLDETECT, WPARAM(0), LPARAM(0), &mut res);
            let _ = tx2.TxSendMessage(EM_LIMITTEXT, WPARAM(4096), LPARAM(0), &mut res);
            // NOTE: no OnTxInPlaceActivate yet — activation latches the
            // format space; peers mount before layout, so activation waits
            // for the first non-empty bounds (see ensure_activated)
            let peer = WindowlessPeer {
                tx: Some(tx2),
                host,
                id,
                line_twips: cfg.line_twips,
                node: NodeId {
                    slot: u32::MAX,
                    generation: u64::MAX,
                },
                peer_rev: Cell::new(TextRevision::default()),
                binding: None,
                sink,
                multiline: cfg.multiline,
                activation: Cell::new(Activation::Inactive),
                composing: Cell::new(false),
                pending_relatch: std::cell::RefCell::new(None),
                line_h: Cell::new(0.0),
                surface: RefCell::new(None),
            };
            std::mem::forget(guard);
            Ok(peer)
        }
    }

    /// Forward a window message (already in the peer's local DIP space for
    /// pointer coords) into the text service. Host callbacks run INSIDE
    /// this call — no `RefCell` borrow may be held across it. The HRESULT
    /// is captured raw (the safe binding drops it) — it's the documented
    /// processed/fallback signal.
    pub(crate) fn send(&self, msg: u32, wparam: usize, lparam: isize) -> UiResult<NativeSend> {
        // focus arrival is the ONLY driver of UI activation — the backend
        // owns focus; a measured/drawn peer is never implicitly UI-active
        if msg == WM_SETFOCUS {
            self.set_ui_active(true)?;
        }
        let out = self.send_raw(msg, wparam, lparam);
        if msg == WM_KILLFOCUS {
            self.set_ui_active(false)?;
        }
        out
    }

    fn send_raw(&self, msg: u32, wparam: usize, lparam: isize) -> UiResult<NativeSend> {
        unsafe {
            let mut res = LRESULT(0);
            let vtbl = Interface::vtable(self.tx());
            let hr = (vtbl.base__.TxSendMessage)(
                self.tx().as_raw(),
                msg,
                WPARAM(wparam),
                LPARAM(lparam),
                &mut res,
            );
            if !hr.is_ok() && hr != S_FALSE {
                return Err(UiError::Platform(format!("TxSendMessage {msg:#x}: {hr:?}")));
            }
            Ok(NativeSend { hr, lr: res.0 })
        }
    }

}
