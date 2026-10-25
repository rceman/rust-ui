use super::*;

pub(crate) struct PeerCtx {
    pub lib: Arc<Msftedit>,
    /// owning window — assigned on WM_CREATE (peers may mount before it)
    pub hwnd: std::cell::Cell<HWND>,
    pub scale: std::cell::Cell<space::ScaleFactor>,
    /// interaction state at layout-run time — state style branches can
    /// consume content insets, so layout resolves the live state
    pub hot: std::cell::Cell<Option<NodeId>>,
    pub pressed: std::cell::Cell<Option<NodeId>>,
    pub focus: std::cell::Cell<Option<NodeId>>,
    pub sink: Arc<Mutex<Vec<NativeSinkItem>>>,
    pub registry: Arc<Mutex<HashMap<u32, RcWeak<RefCell<WindowlessPeer>>>>>,
    /// THE armed-native-timer authority — live owners + tombstones
    /// (stale WM_TIMER fencing). The host writes on TxSetTimer/
    /// TxKillTimer; the backend routes WM_TIMER through `owner()`.
    pub timer_pool: Arc<Mutex<crate::platform::win32::text::TimerPool>>,
    pub(super) next_id: AtomicU64,
    /// live theme colors for peer creation (updated on theme switch)
    pub colors: std::cell::RefCell<(Appearance, Theme)>,
    /// THE system-color authority for the peer palette path — one fn so
    /// a test can move the RESOLVED palette under identical descriptors
    pub sys_resolver: std::cell::Cell<fn(crate::style::SystemColor) -> [f32; 4]>,
}

impl PeerCtx {
    #[cfg(test)]
    pub(crate) fn for_test() -> UiResult<Rc<PeerCtx>> {
        Ok(Rc::new(PeerCtx {
            lib: Msftedit::load()?,
            hwnd: std::cell::Cell::new(HWND::default()),
            scale: std::cell::Cell::new(space::ScaleFactor::ONE),
            hot: std::cell::Cell::new(None),
            pressed: std::cell::Cell::new(None),
            focus: std::cell::Cell::new(None),
            sink: Arc::new(Mutex::new(Vec::new())),
            registry: Arc::new(Mutex::new(HashMap::new())),
            timer_pool: Arc::new(Mutex::new(crate::platform::win32::text::TimerPool::new())),
            next_id: AtomicU64::new(1),
            sys_resolver: std::cell::Cell::new(sys_color),
            colors: std::cell::RefCell::new((
                Appearance {
                    dark: false,
                    forced_colors: false,
                },
                Theme::light(),
            )),
        }))
    }

    /// The `peer_factory` closure passed into `Runtime::new`. The ctx is
    /// UI-thread-only (peers are windowless COM objects) — the registry is
    /// a routing index of Weak handles, not a second owner.
    pub(crate) fn make_factory(
        self: &Rc<PeerCtx>,
    ) -> impl Fn(crate::node::PeerSpec) -> UiResult<Box<dyn crate::node::TextPeer>> + 'static {
        let ctx = self.clone();
        move |spec| {
            let id = ctx.next_id.fetch_add(1, Ordering::SeqCst);
            let (appearance, theme) = ctx.colors.borrow().clone();
            let resolver = ctx.sys_resolver.get();
            let (_fg, sel_bg, sel_fg) = palette(&theme, &appearance, &resolver);
            // forced colors resolve roles through the OS system palette —
            // creation-time fg is no exception
            let fg_resolved = match (spec.foreground, appearance.forced_colors) {
                (crate::style::Color::Role(r), true) => resolver(crate::style::system_slot(r)),
                (c, _) => crate::style::resolve_color(c, theme.dark),
            };
            // capability boundary: the peer receives ONLY the resolved
            // foreground + an optional exact line box — font
            // face/size/weight/selection stay OS-owned. The AUTHORED
            // `Color` is stored — roles re-resolve on theme flips;
            // literals are invariant under resolve_color.
            let cfg = PeerConfig {
                multiline: spec.multiline,
                // disabled peers mount read-only — inert until enabled
                read_only: spec.read_only || spec.disabled,
                // parity: the bundled Geist (privately registered via
                // fonts.rs) is the frozen reference face; Segoe remains
                // only where Geist never registered (non-parity apps).
                face: if super::fonts::geist().sans {
                    "Geist".into()
                } else {
                    "Segoe UI".into()
                },
                // 14pt in twips → 14 DIP em under TXTBIT_D2DDWRITE
                // (measured: msftedit maps pt→DIP 1:1, so 280tw renders
                // a 14px font — the contract font_size — NOT 18.67px)
                size_twips: (14.0f32 * 20.0) as i32,
                // 1 dip = 15 twips (twips are absolute — the authored
                // DIP count lands exact at any scale)
                line_twips: spec
                    .line_height
                    .map(|d| (d.0 * 15.0).round() as i32)
                    .unwrap_or(0),
                fg: fg_resolved,
                fg_authored: spec.foreground,
                sel_bg,
                sel_fg,
                bold: false,
            };
            let peer = WindowlessPeer::create(
                id,
                &ctx.lib,
                ctx.hwnd.get(),
                ctx.scale.get(),
                &cfg,
                ctx.sink.clone(),
                ctx.timer_pool.clone(),
            )?;
            let handle = Rc::new(RefCell::new(peer));
            Ok(Box::new(PeerHandle {
                peer: handle,
                ctx: ctx.clone(),
                node_slot: std::cell::Cell::new(u32::MAX),
            }) as Box<dyn crate::node::TextPeer>)
        }
    }
}

/// The `TextPeer` object the retained node actually owns — a lightweight
/// handle onto `Rc<RefCell<WindowlessPeer>>`; the backend registry holds a
/// Weak reference to the same cell for event routing.
pub(crate) struct PeerHandle {
    peer: Rc<RefCell<WindowlessPeer>>,
    ctx: Rc<PeerCtx>,
    node_slot: std::cell::Cell<u32>,
}

impl crate::node::TextPeer for PeerHandle {
    fn peer_id(&self) -> u64 {
        self.peer.borrow().peer_id()
    }
    fn initialize(
        &mut self,
        text: &str,
        revision: crate::text::TextRevision,
        binding: crate::text::BindingToken,
    ) -> UiResult {
        self.peer.borrow_mut().initialize(text, revision, binding)
    }
    fn set_text(
        &mut self,
        text: &str,
        base: crate::text::TextRevision,
        requested: crate::text::TextRevision,
    ) -> UiResult {
        self.peer.borrow_mut().set_text(text, base, requested)
    }
    fn release(&mut self) {
        self.peer.borrow_mut().release();
        let slot = self.node_slot.get();
        if slot != u32::MAX {
            self.ctx.registry.lock().unwrap().remove(&slot);
        }
    }
    /// Authored `foreground` changed on a mounted editor — the ONLY style
    /// routed into the peer. Resolved against the live appearance, pushed
    /// over all content via the char format.
    fn apply_foreground(&mut self, fg: crate::style::Color) {
        let (app, theme) = self.ctx.colors.borrow().clone();
        // forced colors: roles resolve through the OS system palette —
        // the same resolver every render consumer uses
        let resolver = self.ctx.sys_resolver.get();
        let c = match (fg, app.forced_colors) {
            (crate::style::Color::Role(r), true) => resolver(crate::style::system_slot(r)),
            (c, _) => crate::style::resolve_color(c, theme.dark),
        };
        let cref = windows::Win32::Foundation::COLORREF(
            ((c[0] * 255.0) as u32)
                | (((c[1] * 255.0) as u32) << 8)
                | (((c[2] * 255.0) as u32) << 16),
        );
        self.peer.borrow_mut().apply_format(cref, fg);
    }
    /// Read-only flips ride into the native service — ES_READONLY blocks
    /// edits at the msftedit level, not just the input router. The runtime
    /// passes the composed value (`read_only || disabled`).
    fn set_read_only(&mut self, ro: bool) {
        self.peer.borrow().set_read_only(ro);
    }
    /// Staged selection — forwarded to the real peer; without this the
    /// trait default no-ops and staged ranges never reach msftedit.
    fn apply_selection(&mut self, sel: Option<(usize, usize)>) -> UiResult {
        crate::node::TextPeer::apply_selection(&mut *self.peer.borrow_mut(), sel)
    }
    fn line_twips_latched(&self) -> Option<i32> {
        Some(self.peer.borrow().line_twips_latched())
    }
    fn attach(&mut self, node: NodeId) {
        self.node_slot.set(node.slot);
        self.peer.borrow_mut().attach(node);
        self.ctx
            .registry
            .lock()
            .unwrap()
            .insert(node.slot, Rc::downgrade(&self.peer));
    }
}

/// Theme palette handed to peers (fg/selection colors as `COLORREF` floats).
/// `sys` is THE system-color authority — the palette's resolved output
/// moves with it even when (appearance, theme) are identical.
pub(crate) fn palette(
    theme: &Theme,
    appearance: &Appearance,
    sys: &dyn Fn(crate::style::SystemColor) -> [f32; 4],
) -> ([f32; 4], [f32; 4], [f32; 4]) {
    // forced colors: the peer's selection palette follows the OS system
    // colors — the same resolver every other consumer uses
    if appearance.forced_colors {
        return (
            if theme.dark {
                [0.94, 0.94, 0.94, 1.0]
            } else {
                [0.09, 0.09, 0.11, 1.0]
            },
            sys(crate::style::SystemColor::Highlight),
            sys(crate::style::SystemColor::HighlightText),
        );
    }
    // frozen contract selection tokens — NOT the OS default: light
    // selection=#000 on #fff text, dark=#e5e5e5 on #171717
    if theme.dark {
        (
            [0.94, 0.94, 0.94, 1.0],
            [0.898, 0.898, 0.898, 1.0],
            [0.09, 0.09, 0.09, 1.0],
        )
    } else {
        (
            [0.09, 0.09, 0.11, 1.0],
            [0.0, 0.0, 0.0, 1.0],
            [1.0, 1.0, 1.0, 1.0],
        )
    }
}

// ---------------------------------------------------------------------------
// the backend object — one per run loop
// ---------------------------------------------------------------------------
