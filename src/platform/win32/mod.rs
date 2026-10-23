//! Windows-only native backend: a real Win32 message loop + windowless
//! RichEdit text peers + a software-capable Direct2D/DirectWrite presenter.
//!
//! Structure: `window` owns HWND/WndProc plumbing, `text` owns the msftedit
//! host/peer, `layout` computes DIP rects, `render` paints the frame,
//! `uia` exposes the accessibility tree. The retained core stays generic —
//! this module only consumes its pub(crate) seams.

#![cfg(windows)]

#[cfg(feature = "devtools")]
mod devtools;
pub(crate) mod fonts;
pub(crate) mod layout;
pub(crate) mod render;
pub(crate) mod space;
pub(crate) mod text;
pub(crate) mod uia;
pub(crate) mod window;

mod construct;
mod input_key;
mod input_pointer;
mod interact;
mod paint_timers;
mod peer;
mod peer_events;
mod peer_native;
mod run;
mod syscolor;
mod turn;
mod uia_bridge;

pub(crate) use construct::{MailboxEvent, WaitClass, WakeOutcome, classify_wait, init_wake_event};
pub(crate) use peer::{PeerCtx, PeerHandle, palette};
pub(crate) use run::run;
pub(crate) use syscolor::{os_appearance, sys_color};

pub(crate) use layout::LayoutCache;
pub(crate) use render::Renderer;
pub(crate) use space::LogicalRect;
pub(crate) use text::{HostEvent, Msftedit, PeerConfig, WindowlessPeer};

pub(crate) use std::cell::RefCell;
pub(crate) use std::collections::HashMap;
pub(crate) use std::ffi::c_void;
pub(crate) use std::rc::Rc;
pub(crate) use std::rc::Weak as RcWeak;
pub(crate) use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
pub(crate) use std::sync::{Arc, Mutex};

use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Dwm::{
    DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWINDOWATTRIBUTE, DwmSetWindowAttribute,
};
use windows::Win32::System::Threading::{
    CREATE_EVENT, CreateEventExW, EVENT_ALL_ACCESS, INFINITE, SetEvent,
};

use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::*;

use crate::event::{Key, KeyEvent, Modifiers, PointerButton, PointerEvent, ScrollOffset};
use crate::geom::{Point, Visibility};
use crate::node::{NodeData, NodeEvent, QueuedEvent};
use crate::runtime::UpdateCtx;
use crate::sched::SchedEvent;
use crate::text::{EditOrigin, TextEdit, TextSelection};
use crate::theme::{Appearance, ReducedMotion, SubmitPolicy, Theme};

use crate::{App, NodeId, UiError, UiResult};

/// One routed native-to-runtime semantic event (bounded sink; the backend
/// drains it before each pump, never synchronously inside COM/WndProc).
pub(crate) enum NativeSinkItem {
    Edit { node: NodeId, edit: TextEdit },
    Selection { node: NodeId, sel: TextSelection },
}

/// Backend-side counters the lead's perf recipe can sample (not yet wired to
/// a report — measurement comes later).
pub(crate) struct PerfCounters {
    pub start: std::time::Instant,
    pub requested_redraws: AtomicU64,
    /// framework-driven animation steps vs native-driven redraws
    pub animation_callbacks: AtomicU64,
    pub animation_timer_fires: AtomicU64,
    pub native_caret_redraws: AtomicU64,
    pub native_timer_fires: AtomicU64,
    /// EN_CHANGE notifications suppressed while a composition was active —
    /// preedit leaking into the committed channel is a defect we count,
    /// not a value we route
    pub preedit_commits_suppressed: AtomicU64,
    /// IME-end reconcile emitted a commit — the final EN_CHANGE arrived
    /// while the composing flag was still up and had to be reconstructed
    /// from peer-vs-mirror divergence.
    pub ime_reconciled_commits: AtomicU64,
}

impl PerfCounters {
    pub(crate) fn new() -> Self {
        PerfCounters {
            start: std::time::Instant::now(),
            requested_redraws: AtomicU64::new(0),
            animation_callbacks: AtomicU64::new(0),
            animation_timer_fires: AtomicU64::new(0),
            native_caret_redraws: AtomicU64::new(0),
            native_timer_fires: AtomicU64::new(0),
            preedit_commits_suppressed: AtomicU64::new(0),
            ime_reconciled_commits: AtomicU64::new(0),
        }
    }
}

/// One instrumented WM_SIZE dispatch — phase split plus per-event
/// resource-creation deltas. Bounded by `RESIZE_SAMPLE_CAP`.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "devtools", derive(serde::Serialize))]
pub(crate) struct ResizeSample {
    /// 0 = WM_SIZE delivery, 1 = WM_PAINT delivery
    pub kind: u8,
    pub wm_size_us: u64,
    pub resize_us: u64,
    pub layout_us: u64,
    pub uia_us: u64,
    pub paint_us: u64,
    pub text_layouts: u64,
    pub shadow_rasters: u64,
}

/// The resize-path report — atomic counters (every caller) plus a capped
/// ring of per-WM_SIZE samples for percentile analysis. Counters are plain
/// atomics: near-free in production, never logs; retrieved only through the
/// devctl `perf` pull.
pub(crate) struct ResizeCounters {
    pub events: AtomicU64,
    pub layout_runs: AtomicU64,
    pub layout_us: AtomicU64,
    pub layout_max_us: AtomicU64,
    pub paint_runs: AtomicU64,
    pub paint_us: AtomicU64,
    pub paint_max_us: AtomicU64,
    pub uia_runs: AtomicU64,
    pub uia_us: AtomicU64,
    pub uia_max_us: AtomicU64,
    pub resize_runs: AtomicU64,
    pub resize_us: AtomicU64,
    pub resize_max_us: AtomicU64,
    pub text_layout_creates: AtomicU64,
    pub shadow_rasters: AtomicU64,
    pub wm_size_us: AtomicU64,
    pub wm_size_max_us: AtomicU64,
    pub samples: std::sync::Mutex<std::collections::VecDeque<ResizeSample>>,
}

/// Bounded sample ring — worst case ~8192 × ~64B.
pub(crate) const RESIZE_SAMPLE_CAP: usize = 8192;

impl ResizeCounters {
    const fn new() -> Self {
        use std::sync::atomic::AtomicU64 as A;
        ResizeCounters {
            events: A::new(0),
            layout_runs: A::new(0),
            layout_us: A::new(0),
            layout_max_us: A::new(0),
            paint_runs: A::new(0),
            paint_us: A::new(0),
            paint_max_us: A::new(0),
            uia_runs: A::new(0),
            uia_us: A::new(0),
            uia_max_us: A::new(0),
            resize_runs: A::new(0),
            resize_us: A::new(0),
            resize_max_us: A::new(0),
            text_layout_creates: A::new(0),
            shadow_rasters: A::new(0),
            wm_size_us: A::new(0),
            wm_size_max_us: A::new(0),
            samples: std::sync::Mutex::new(std::collections::VecDeque::new()),
        }
    }
    pub(crate) fn bump(c: &AtomicU64, sum: &AtomicU64, max: &AtomicU64, us: u64) {
        c.fetch_add(1, Ordering::Relaxed);
        sum.fetch_add(us, Ordering::Relaxed);
        max.fetch_max(us, Ordering::Relaxed);
    }
    pub(crate) fn record(&self, s: ResizeSample) {
        Self::bump(
            &self.events,
            &self.wm_size_us,
            &self.wm_size_max_us,
            s.wm_size_us,
        );
        let mut ring = self.samples.lock().unwrap();
        if ring.len() == RESIZE_SAMPLE_CAP {
            ring.pop_front();
        }
        ring.push_back(s);
    }
    #[cfg(feature = "devtools")]
    pub(crate) fn snapshot(&self) -> serde_json::Value {
        let r = |a: &AtomicU64| a.load(Ordering::Relaxed);
        let samples: Vec<ResizeSample> = self.samples.lock().unwrap().iter().copied().collect();
        serde_json::json!({
            "wm_size_events": r(&self.events),
            "wm_size_us": r(&self.wm_size_us),
            "wm_size_max_us": r(&self.wm_size_max_us),
            "layout_runs": r(&self.layout_runs),
            "layout_us": r(&self.layout_us),
            "layout_max_us": r(&self.layout_max_us),
            "paint_runs": r(&self.paint_runs),
            "paint_us": r(&self.paint_us),
            "paint_max_us": r(&self.paint_max_us),
            "uia_runs": r(&self.uia_runs),
            "uia_us": r(&self.uia_us),
            "uia_max_us": r(&self.uia_max_us),
            "render_resizes": r(&self.resize_runs),
            "render_resize_us": r(&self.resize_us),
            "render_resize_max_us": r(&self.resize_max_us),
            "text_layout_creates": r(&self.text_layout_creates),
            "shadow_rasters": r(&self.shadow_rasters),
            "samples": samples,
        })
    }
    pub(crate) fn reset(&self) {
        let r = |a: &AtomicU64| a.store(0, Ordering::Relaxed);
        r(&self.events);
        r(&self.layout_runs);
        r(&self.layout_us);
        r(&self.layout_max_us);
        r(&self.paint_runs);
        r(&self.paint_us);
        r(&self.paint_max_us);
        r(&self.uia_runs);
        r(&self.uia_us);
        r(&self.uia_max_us);
        r(&self.resize_runs);
        r(&self.resize_us);
        r(&self.resize_max_us);
        r(&self.text_layout_creates);
        r(&self.shadow_rasters);
        r(&self.wm_size_us);
        r(&self.wm_size_max_us);
        self.samples.lock().unwrap().clear();
    }
}

/// Process-wide resize instrumentation — atomics in a `static`, so the
/// free-function hot paths (`layout_for`, `draw_shadow`) can count without
/// plumbing a handle through the renderer.
pub(crate) static RESIZE: ResizeCounters = ResizeCounters::new();

/// Shared context the `peer_factory` hands to each `PeerHandle` — the
/// registry is a ROUTING INDEX (weak handles), not a second owner: the
/// authoritative peer lives inside the retained node's `peer` field.

pub(crate) const WM_PUMP: u32 = WM_APP + 7;
/// generation-fenced UIA actions routed through the UI thread
pub(crate) const WM_UIA_PRESS: u32 = WM_APP + 8;
pub(crate) const WM_UIA_FOCUS: u32 = WM_APP + 9;

/// One live popup's bookkeeping — the pre-open focus owner so a close
/// restores focus truthfully.
struct PopupBook {
    id: NodeId,
    focus_return: Option<NodeId>,
    focus_pending: bool,
    /// monotonic open order — a same-commit multi-close restores the
    /// DEEPEST layer's return target deterministically
    open_seq: u64,
}

/// Interaction state snapshot for transition classification.
#[derive(Copy, Clone, PartialEq)]
struct InteractState {
    pressed: bool,
    hot: bool,
    focus: bool,
}

/// The Enter-fork decision — Submit (semantic only, peer never sees the
/// key) or Edit (delivered to the peer, never also submits).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum SubmitDecision {
    Submit,
    Edit,
}

/// THE Win32 modifier read — real keyboard state at event time, the one
/// place `GetKeyState` is consulted for semantic events.

pub(crate) struct Backend<S, M, U, V>
where
    M: 'static,
    U: Fn(&mut S, M, &mut UpdateCtx<'_, M>),
    V: Fn(&S, &mut crate::Ui<'_, '_, M>),
{
    pub rt: crate::runtime::Runtime<S, M, U, V>,
    pub hwnd: HWND,
    /// last applied non-client dark appearance — `DWMWA_USE_IMMERSIVE_
    /// DARK_MODE` is synced when the resolved theme.light/dark changes
    /// (incl. System-driven), never by recreating the window
    pub dark_caption: std::cell::Cell<Option<bool>>,
    pub peer_ctx: Rc<PeerCtx>,
    pub renderer: RefCell<Renderer>,
    /// laid-out rects in DIP, keyed by live NodeId — pruned on remove
    pub rects: HashMap<NodeId, LogicalRect>,
    /// depth-first paint/hit order (cached each layout pass)
    pub order: Vec<NodeId>,
    /// per-node scroll-viewport clip (DIP), populated for descendants of
    /// KIND_SCROLL containers — paint pushes it (peers included), hit-test
    /// rejects outside it, focus skips the fully excluded, devtools and
    /// UIA report the same visibility/bounds truth
    pub clips: HashMap<NodeId, LogicalRect>,
    pub focus: Option<NodeId>,
    pub hot: Option<NodeId>,
    pub pressed: Option<NodeId>,
    /// richedit-owned capture (TxSetCapture) vs framework press capture
    pub native_capture: Option<NodeId>,
    pub mouse: Point,
    /// armed/deadline timers owned by the loop (distinct from peer timers)
    pub deadline_timer: Option<std::time::Instant>,
    // (armed-timer state lives in peer_ctx.timer_pool — the ONE authority;
    // the host arms via SetTimer synchronously and this backend routes
    // WM_TIMER through it — no mirror map here)
    /// currently-shown tooltip node (overlay window managed by renderer)
    pub tooltip_for: Option<NodeId>,
    /// UIA root — built on the first WM_GETOBJECT
    pub uia: Option<uia::UiaRoot>,
    pub counters: Arc<PerfCounters>,
    /// ended by WM_DESTROY — the pump exits
    pub closed: Arc<AtomicBool>,
    /// last fatal backend error surfaced from a WndProc
    fatal: Option<crate::UiError>,
    /// `turn()` is executing — native reentrancy (e.g. `SetFocus` inside
    /// `OnTxUIActivate`) must queue another pump instead of recursing
    in_turn: std::cell::Cell<bool>,
    /// reentrant WndProc arrivals — owned payloads ONLY (scalars or a
    /// copied RECT for WM_DPICHANGED; no borrowed pointers). Drained
    /// FIFO at the end of the outer dispatch — the ONE deferred lane.
    pub(crate) reentrant_queue:
        std::cell::RefCell<std::collections::VecDeque<crate::platform::win32::window::QueuedMsg>>,
    /// WndProc dispatch holds the backend — the trampoline checks this
    /// THROUGH THE ROUTE POINTER before forming `&mut Backend`, so a
    /// synchronous native callback can never create overlapping access
    pub(crate) in_dispatch: std::cell::Cell<bool>,
    /// one coalesced `WM_PUMP` outstanding for a deferred turn
    pump_queued: std::cell::Cell<bool>,
    /// native messages deferred while their target peer was borrowed —
    /// FIFO; delivered at the top of the next `service_peer_events`
    deferred_native: std::collections::VecDeque<DeferredNative>,
    /// interaction-state change (hot/pressed/focus) with no consumer
    /// handler — classified: bit0 relayout (a metric-bearing branch
    /// transitioned), bit1 repaint; consumed by turn_body
    state_dirty: std::cell::Cell<u8>,
    /// a WM_SIZE resized the target but layout/paint have not yet run —
    /// coalesced into the next delivered WM_PAINT (OS-batched during the
    /// modal sizing loop) so one frame's work runs per paint delivery,
    /// never per mouse-move step
    size_pending: std::cell::Cell<bool>,
    /// sizing-session trace sink — RUI_RESIZE_TRACE=<path> enables a
    /// generation log (SIZE/PAINT sequence vs client/target/laid sizes)
    /// used to pin live-drag generation loss; None in normal runs
    resize_trace_path: Option<std::path::PathBuf>,
    /// WM_SIZE arrival ordinal during a sizing session — trace only
    size_seq: std::cell::Cell<u64>,
    /// client DIP size the last relayout actually laid out — WM_PAINT
    /// re-verifies the live client against it so any resize whose
    /// WM_SIZE/coalesce path never reached a relayout still converges
    /// instead of leaving the retained tree at a stale width
    laid_client: std::cell::Cell<(f32, f32)>,
    /// inside a WM_ENTERSIZEMOVE..WM_EXITSIZEMOVE modal loop (border-drag
    /// OR title-bar move) — reentrant WM_PAINT must run live for the whole
    /// session: a move can expose previously off-screen client pixels even
    /// though size/layout are unchanged
    pub(crate) sizing_session: std::cell::Cell<bool>,
    /// coalesced WM_MOVE pending — during a held title-bar drag every move
    /// is reentrant; the latest position wins, a single UIA refresh runs
    /// at session end rather than replaying the full move history
    pub(crate) move_pending: std::cell::Cell<bool>,
    /// replay-owner discriminator: `None` = live dispatch (resolve current
    /// focus); `Some(c)` = replaying — `c` is the captured arrival owner,
    /// `None` inside means EXPLICITLY no owner (never fall through)
    arrival_focus: std::cell::Cell<Option<Option<NodeId>>>,
    /// the peer that received WM_IME_STARTCOMPOSITION — composition owns
    /// it through END, even if focus moved meanwhile
    /// None = no active composition; `Some(None)` = composition START was
    /// captured with NO owner — stays ownerless through delivery, end
    /// bookkeeping and relatch (never falls through to a later-focused
    /// peer); `Some(Some(id))` = pinned composition owner
    pub(crate) ime_owner: std::cell::Cell<Option<Option<NodeId>>>,
    /// a reentrant push exceeded REENTRANT_QUEUE_CAP — surfaced as
    /// QueueOverflow by the next drain (typed failure, never silent loss)
    queue_overflowed: std::cell::Cell<bool>,
    /// pending UTF-16 lead surrogate awaiting its trail unit (WM_CHAR
    /// pairing for the public Key::Char route)
    pending_lead_surrogate: std::cell::Cell<Option<u16>>,
    /// accumulated ink damage since the last committed paint — union of
    /// every paint-dirty node's OLD committed ink and NEW ink (a moved or
    /// removed shadow must damage where it used to be). Cleared by paint().
    damage: std::cell::Cell<Option<LogicalRect>>,
    /// node -> the ink footprint it committed at the LAST paint — the old
    /// half of the damage union
    committed_ink: std::cell::RefCell<HashMap<NodeId, LogicalRect>>,
    /// the effective colors LAST pushed to mounted peers — per-peer
    /// (fg, sel_bg, sel_fg) resolved output keyed by NodeId. Compared
    /// against a fresh resolution every turn: descriptor-equal
    /// GetSysColor drift still refreshes.
    applied_palette: std::cell::RefCell<Option<Vec<(NodeId, ([f32; 4], [f32; 4], [f32; 4]))>>>,
    /// live popup bookkeeping — slot -> focus to restore when the popup
    /// unmounts. Populated/cleared in turn_body between dirty drain and
    /// focus sanitize.
    open_popups: HashMap<u32, PopupBook>,
    /// next popup open sequence — books restore deepest-layer-first
    popup_seq: u64,
    #[cfg(feature = "devtools")]
    devtools: Option<crate::devtools::client::Server>,
    /// frame counter — monotonic per committed paint; kept unconditional so
    /// the env-gated sizing-session trace compiles in every feature set
    paint_epoch: std::cell::Cell<u64>,
}

fn modifiers_now() -> Modifiers {
    unsafe {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            GetKeyState, VK_CONTROL, VK_LWIN, VK_MENU, VK_SHIFT,
        };
        let down = |vk: u16| GetKeyState(vk as i32) < 0;
        Modifiers {
            shift: down(VK_SHIFT.0),
            ctrl: down(VK_CONTROL.0),
            alt: down(VK_MENU.0),
            logo: down(VK_LWIN.0) || down(windows::Win32::UI::Input::KeyboardAndMouse::VK_RWIN.0),
        }
    }
}

enum DeferredNative {
    /// a whole WM_* triple — both parameters are scalars for this msg
    Message {
        node: NodeId,
        msg: u32,
        wparam: usize,
        lparam: isize,
    },
}

fn deferrable(msg: u32) -> bool {
    matches!(
        msg,
        WM_SETFOCUS
            | WM_KILLFOCUS
            | WM_CHAR
            | WM_SYSCHAR
            | WM_KEYDOWN
            | WM_KEYUP
            | WM_SYSKEYDOWN
            | WM_SYSKEYUP
            | WM_MOUSEMOVE
            | WM_LBUTTONDOWN
            | WM_LBUTTONUP
            | WM_LBUTTONDBLCLK
            | WM_RBUTTONDOWN
            | WM_RBUTTONUP
            | WM_MBUTTONDOWN
            | WM_MBUTTONUP
            | WM_MOUSEWHEEL
            | WM_IME_STARTCOMPOSITION
            | WM_IME_ENDCOMPOSITION
            | WM_IME_COMPOSITION
            | WM_IME_NOTIFY
            | WM_TIMER
    )
}
