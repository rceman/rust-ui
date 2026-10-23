impl Msftedit {
    /// Load msftedit + resolve entry points. `OleInitialize` is the caller's
    /// responsibility (per UI thread via the backend's RAII guard).
    pub(crate) fn load() -> UiResult<Arc<Msftedit>> {
        unsafe {
            let module = LoadLibraryExW(w!("msftedit.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32)
                .map_err(|e| UiError::Platform(format!("LoadLibraryExW(msftedit.dll): {e}")))?;
            let data_iid = |name: &std::ffi::CStr| -> UiResult<GUID> {
                let p = GetProcAddress(module, PCSTR(name.as_ptr() as *const u8));
                if p.is_none() {
                    return Err(UiError::Platform(format!("missing export {name:?}")));
                }
                Ok(*(p.unwrap() as *const GUID))
            };
            let create = GetProcAddress(module, s!("CreateTextServices"));
            let create =
                create.ok_or_else(|| UiError::Platform("no CreateTextServices export".into()))?;
            Ok(Arc::new(Msftedit {
                module,
                create_text_services: std::mem::transmute::<*const u8, CreateTextServicesFn>(
                    create as *const u8,
                ),
                iid_text_services: data_iid(c"IID_ITextServices")?,
                iid_text_services2: data_iid(c"IID_ITextServices2")?,
                iid_text_host: data_iid(c"IID_ITextHost")?,
                iid_text_host2: data_iid(c"IID_ITextHost2")?,
                iid_windowless_acc: data_iid(c"IID_IRicheditWindowlessAccessibility").ok(),
            }))
        }
    }
}

// ---------------------------------------------------------------------------
// host events the peer drains after every text-services call
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HostEvent {
    /// text services invalidated its view
    Invalidate,
    /// EN_CHANGE: committed content changed
    Change,
    /// EN_SELCHANGE: selection changed
    SelChange,
    /// richedit wants mouse capture set/released
    Capture(bool),
    /// caret geometry or visibility changed (system caret)
    Caret,
    /// richedit wants focus moved to the owner window
    GrabFocus,
}

/// Everything `TxGet*` reads back from the host.
pub(crate) struct HostShared {
    /// owning window (IME context, timers, capture)
    pub hwnd: HWND,
    /// global bounds in window DIP (for ScreenToClient conversions)
    pub bounds: LogicalRect,
    /// px-per-DIP — the ONLY scale authority (see space.rs)
    pub scale: ScaleFactor,
    /// property bits advertised via TxGetPropertyBits
    pub bits: u32,
    /// latest EN_REQUESTRESIZE size — client units are PHYSICAL px
    /// (the host space; see space.rs); divide by scale for DIP
    pub natural: SIZE,
    pub cf: Box<CHARFORMATW>,
    /// v2 because the host default carries line spacing (v1 has no
    /// dyLineSpacing/bLineSpacingRule fields); msftedit reads by cbSize
    pub pf: Box<PARAFORMAT2>,
    pub fg: COLORREF,
    /// the AUTHORED foreground (Color, not a resolved COLORREF) — theme
    /// flips re-resolve roles through it; a literal `Color::Rgba` is
    /// invariant under resolve and so correctly survives flips
    pub fg_authored: crate::style::Color,
    pub sel_bg: COLORREF,
    pub sel_fg: COLORREF,
    /// COLOR_WINDOW — the selection band's blend base under D2DDWRITE
    /// (band = 0.4·sel_bg + 0.6·win_bg). Near-black so the contract's
    /// dark selection tokens render near-solid; must stay ≠ sel_bg or
    /// msftedit skips the band (TXTBACK_TRANSPARENT → never painted)
    pub win_bg: COLORREF,
    /// system-caret state recorded from Tx*Caret calls
    pub caret_pos: POINT,
    pub caret_size: SIZE,
    pub caret_shown: bool,
    pub caret_created: bool,
    /// richedit-requested timers armed by THIS host (richedit id ->
    /// win32 id) — KillTimer/Drop resolve through it; the armed win32
    /// ids live in the shared `timer_pool` (existence + routing owner)
    pub armed_timers: std::collections::BTreeMap<u32, usize>,
    /// this host's node — assigned at attach; timer routing needs it
    pub node: Option<crate::NodeId>,
    /// THE armed-timer authority — shared `TimerPool` (live + tombstones)
    pub timer_pool: std::sync::Arc<std::sync::Mutex<TimerPool>>,
    /// bounded host-event log — same 128 bound as the runtime event queue;
    /// overflow is a QueueOverflow failure, not a silent drop
    pub events: Vec<HostEvent>,
    /// event buffer overflowed — next drain returns QueueOverflow
    pub overflow: bool,
    /// the host's read-only state gate
    pub read_only: bool,
}

impl HostShared {
    /// bounded push — an overflow is flagged, not silently dropped; the next
    /// `drain` surfaces it as `QueueOverflow`
    fn ev(&mut self, e: HostEvent) {
        if self.events.len() >= crate::event::EVENT_QUEUE_CAP {
            self.overflow = true;
            return;
        }
        self.events.push(e);
    }
}

fn bools(v: bool) -> BOOL {
    BOOL::from(v)
}

/// THE native-timer authority — win32 id -> live owner, or a tombstone.
/// `Tombstone` exists because KillTimer leaves any already-queued
/// WM_TIMER deliverable (MS contract): an id is NEVER reallocated while
/// tombstoned, so a stale tick resolves to a dead entry and can never be
/// redirected at a different timer's owner.
#[derive(Clone, Copy)]
pub(crate) enum TimerState {
    Live(crate::NodeId, u32),
    Tombstone,
}

pub(crate) struct TimerPool {
    map: std::collections::HashMap<usize, TimerState>,
    /// monotonic allocation cursor — ids are allocated, never reused
    next: usize,
}

impl TimerPool {
    pub(crate) fn new() -> Self {
        TimerPool {
            map: std::collections::HashMap::new(),
            next: 0,
        }
    }

    /// Allocate a collision-free win32 timer id; `None` when the
    /// namespace is full (honestly reported to msftedit).
    pub(crate) fn alloc(&mut self) -> Option<usize> {
        use super::window::{NATIVE_TIMER_BASE, NATIVE_TIMER_CAP};
        for _ in 0..NATIVE_TIMER_CAP {
            let tid = NATIVE_TIMER_BASE + (self.next % NATIVE_TIMER_CAP);
            self.next = self.next.wrapping_add(1);
            if !self.map.contains_key(&tid) {
                return Some(tid);
            }
        }
        None
    }

    pub(crate) fn arm(&mut self, tid: usize, node: crate::NodeId, idtimer: u32) {
        self.map.insert(tid, TimerState::Live(node, idtimer));
    }

    /// Kill: the id becomes a tombstone — a queued stale WM_TIMER still
    /// resolves here and resolves to NO owner.
    pub(crate) fn kill(&mut self, tid: usize) {
        self.map.insert(tid, TimerState::Tombstone);
    }

    /// The live owner of `tid`, if any — tombstones/stale ids are None.
    pub(crate) fn owner(&self, tid: usize) -> Option<(crate::NodeId, u32)> {
        match self.map.get(&tid) {
            Some(TimerState::Live(node, idtimer)) => Some((*node, *idtimer)),
            _ => None,
        }
    }

    /// Currently live win32 ids (tombstones excluded).
    pub(crate) fn live_ids(&self) -> Vec<usize> {
        self.map
            .iter()
            .filter_map(|(t, s)| matches!(s, TimerState::Live(..)).then_some(*t))
            .collect()
    }

    pub(crate) fn clear(&mut self) {
        self.map.clear();
    }
}

/// Explicit TextServices activation state — in-place activation (draw/
/// measure) and UI activation (focus-owned UI: caret, selection visibility,
/// IME target) are SEPARATE contracts; a measured peer is never implicitly
/// UI-active.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum Activation {
    Inactive,
    InPlace,
    Ui,
}

/// Peer-visible state the host callbacks need — the msftedit reference is
/// kept per peer so the DLL ref guard outlives every COM object the peer
/// built.
pub(crate) struct PeerShared {
    pub lib: Arc<Msftedit>,
    pub host: HostShared,
}

/// COM object handed to `CreateTextServices`.
#[repr(C)]
pub(crate) struct HostBox {
    host: &'static ITextHost_Vtbl,
    host2: &'static ITextHost2_Vtbl,
    refs: Cell<u32>,
    state: RefCell<PeerShared>,
}

// Single-inheritance layout: slot 0 carries the full ITextHost2 vtbl with
// OFFSET 0 so both ITextHost and ITextHost2 pointers are base+0.
static VT_HOST2: ITextHost2_Vtbl = ITextHost2_Vtbl::new::<HostBox, 0>();

