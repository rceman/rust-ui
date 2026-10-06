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
pub(crate) mod layout;
pub(crate) mod render;
pub(crate) mod space;
pub(crate) mod text;
pub(crate) mod uia;
pub(crate) mod window;

pub(crate) use layout::LayoutCache;
pub(crate) use render::Renderer;
pub(crate) use space::LogicalRect;
pub(crate) use text::{HostEvent, Msftedit, PeerConfig, WindowlessPeer};

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::rc::Rc;
use std::rc::Weak as RcWeak;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use windows::Win32::Foundation::*;
use windows::Win32::System::Threading::{
    CREATE_EVENT, CreateEventExW, EVENT_ALL_ACCESS, INFINITE, SetEvent,
};

use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::*;

use crate::event::{Key, KeyEvent, Modifiers, PointerButton, PointerEvent};
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

/// Shared context the `peer_factory` hands to each `PeerHandle` — the
/// registry is a ROUTING INDEX (weak handles), not a second owner: the
/// authoritative peer lives inside the retained node's `peer` field.
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
    next_id: AtomicU64,
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
            // foreground — font face/size/weight/selection stay OS-owned.
            // The AUTHORED `Color` is stored — roles re-resolve on theme
            // flips; literals are invariant under resolve_color.
            let cfg = PeerConfig {
                multiline: spec.multiline,
                // disabled peers mount read-only — inert until enabled
                read_only: spec.read_only || spec.disabled,
                face: "Segoe UI".into(),
                size_twips: (14.0f32 * 20.0) as i32,
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
    if theme.dark {
        (
            [0.94, 0.94, 0.94, 1.0],
            [0.17, 0.36, 0.72, 1.0],
            [1.0, 1.0, 1.0, 1.0],
        )
    } else {
        (
            [0.09, 0.09, 0.11, 1.0],
            [0.23, 0.47, 0.85, 1.0],
            [1.0, 1.0, 1.0, 1.0],
        )
    }
}

/// Same-thread continuation: a reentrant/deferred native callback posts
/// this so the deferred work runs through the guarded turn path — it is
/// NOT the cross-thread mailbox authority (the owned `MailboxEvent`
/// handles producers on other threads).
pub(crate) const WM_PUMP: u32 = WM_APP + 7;
/// generation-fenced UIA actions routed through the UI thread
pub(crate) const WM_UIA_PRESS: u32 = WM_APP + 8;
pub(crate) const WM_UIA_FOCUS: u32 = WM_APP + 9;

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

// ---------------------------------------------------------------------------
// the backend object — one per run loop
// ---------------------------------------------------------------------------

pub(crate) struct Backend<S, M, U, V>
where
    M: 'static,
    U: Fn(&mut S, M, &mut UpdateCtx<'_, M>),
    V: Fn(&S, &mut crate::Ui<'_, '_, M>),
{
    pub rt: crate::runtime::Runtime<S, M, U, V>,
    pub hwnd: HWND,
    pub peer_ctx: Rc<PeerCtx>,
    pub renderer: RefCell<Renderer>,
    /// laid-out rects in DIP, keyed by live NodeId — pruned on remove
    pub rects: HashMap<NodeId, LogicalRect>,
    /// depth-first paint/hit order (cached each layout pass)
    pub order: Vec<NodeId>,
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
    #[cfg(feature = "devtools")]
    devtools: Option<crate::devtools::client::Server>,
    #[cfg(feature = "devtools")]
    paint_epoch: std::cell::Cell<u64>,
}

#[cfg(test)]
impl<S, M, U, V> Backend<S, M, U, V>
where
    M: 'static,
    U: Fn(&mut S, M, &mut UpdateCtx<'_, M>),
    V: Fn(&S, &mut crate::Ui<'_, '_, M>),
{
    /// Headless backend for queue/teardown/ownership tests — wraps a
    /// caller-built Runtime with a NULL HWND (no window, no presenter
    /// target). `drain_reentrant`, `shutdown` and the owner pins run the
    /// real production paths.
    pub(crate) fn for_test(
        rt: crate::runtime::Runtime<S, M, U, V>,
        peer_ctx: Rc<PeerCtx>,
    ) -> UiResult<Self> {
        Ok(Backend {
            #[cfg(feature = "devtools")]
            devtools: None,
            #[cfg(feature = "devtools")]
            paint_epoch: std::cell::Cell::new(0),
            rt,
            hwnd: HWND::default(),
            peer_ctx,
            renderer: RefCell::new(Renderer::new()?),
            rects: HashMap::new(),
            order: Vec::new(),
            focus: None,
            hot: None,
            pressed: None,
            native_capture: None,
            mouse: Point { x: 0.0, y: 0.0 },
            deadline_timer: None,
            tooltip_for: None,
            uia: None,
            counters: Arc::new(PerfCounters::new()),
            closed: Arc::new(AtomicBool::new(false)),
            fatal: None,
            in_turn: std::cell::Cell::new(false),
            reentrant_queue: std::cell::RefCell::new(std::collections::VecDeque::new()),
            arrival_focus: std::cell::Cell::new(None),
            ime_owner: std::cell::Cell::new(None),
            queue_overflowed: std::cell::Cell::new(false),
            pending_lead_surrogate: std::cell::Cell::new(None),
            in_dispatch: std::cell::Cell::new(false),
            pump_queued: std::cell::Cell::new(false),
            deferred_native: std::collections::VecDeque::new(),
            state_dirty: std::cell::Cell::new(0),
            damage: std::cell::Cell::new(None),
            committed_ink: std::cell::RefCell::new(HashMap::new()),
            applied_palette: std::cell::RefCell::new(None),
        })
    }
}

/// The deferred-native-delivery contract (private to this backend).
///
/// WHAT may defer: only `DeferredNative::Message` — a (node, msg, wparam,
/// lparam) tuple whose parameters are PURE COPYABLE SCALARS for that
/// message id (verified by `deferrable(msg)`): focus, char, key, pointer,
/// IME, wheel, and peer timer messages. No message carrying a pointer, a
/// borrowed lifetime, or a required synchronous return may ever enter
/// this queue — such sends must execute synchronously or fail.
///
/// ORDER: strict FIFO. While the queue is non-empty EVERY subsequent
/// delivery enqueues behind it — a live send may never overtake a queued
/// one. The queue drains in full at the head of `service_peer_events`.
///
/// OVERFLOW: bounded at `EVENT_QUEUE_CAP`; a push past capacity returns
/// `UiError::QueueOverflow` to the caller — nothing is silently dropped.
///
/// PROGRESS: enqueueing while no turn is active posts `WM_PUMP`, so a
/// deferred message always reaches `service_peer_events` without relying
/// on an unrelated later message.
///
/// FENCING: each entry carries the full `NodeId` (slot+generation); a
/// drained entry whose slot now holds a different generation is dropped.
///
/// TEARDOWN: `shutdown` clears the queue — a closed backend drops pending
/// deliveries deterministically.
#[derive(Debug)]
enum DeferredNative {
    /// a whole WM_* triple — both parameters are scalars for this msg
    Message {
        node: NodeId,
        msg: u32,
        wparam: usize,
        lparam: isize,
    },
}

/// The set of messages whose wparam/lparam are plain scalars — the only
/// ones permitted in the deferred queue.
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

/// Owned kernel wake event — the HANDLE lives exactly as long as the
/// last `Arc<MailboxEvent>`: the run loop holds one, the installed wake
/// callback holds one, and any in-flight signaler cloned from the
/// callback holds one until its call returns. `CloseHandle` can never
/// precede the last legitimate signaler — a detached callback that was
/// already cloned still signals a VALID handle.
pub(crate) struct MailboxEvent(HANDLE);
// kernel event handles are safe to signal/wait across threads
unsafe impl Send for MailboxEvent {}
unsafe impl Sync for MailboxEvent {}

impl MailboxEvent {
    /// Auto-reset event — the sole cross-thread mailbox wake authority.
    pub(crate) fn create() -> windows::core::Result<Self> {
        unsafe { CreateEventExW(None, PCWSTR::null(), CREATE_EVENT(0), EVENT_ALL_ACCESS.0) }
            .map(MailboxEvent)
    }

    /// Signal the event — the ONLY primitive behind the mailbox wake.
    pub(crate) fn signal(&self) -> bool {
        unsafe { SetEvent(self.0).is_ok() }
    }

    /// test seam: wrap an arbitrary raw handle (e.g. a never-valid one)
    /// to exercise the SetEvent-failure terminal contract
    #[cfg(test)]
    pub(crate) fn from_raw(h: HANDLE) -> Self {
        MailboxEvent(h)
    }

    /// test seam: the raw handle for wait assertions
    #[cfg(test)]
    pub(crate) fn raw(&self) -> HANDLE {
        self.0
    }
}

impl Drop for MailboxEvent {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// Wake-authority creation for `run()` — returns the typed platform
/// failure `run` propagates. Called BEFORE `window::create`, so a
/// failure unwinds with no HWND, no installed route pointer, and only
/// the not-yet-live Backend to drop — nothing leaks or dangles.
pub(crate) fn init_wake_event(
    raw: windows::core::Result<MailboxEvent>,
) -> UiResult<Arc<MailboxEvent>> {
    raw.map(Arc::new)
        .map_err(|e| UiError::Platform(format!("mailbox wake event: {e}")))
}

/// Where a `mailbox_wake` attempt landed — the signaled event, the
/// closed short-circuit, or the terminal latch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WakeOutcome {
    /// the mailbox wake event was signaled — the run loop's
    /// MsgWaitForMultipleObjectsEx is already waiting on it
    Signaled,
    /// window already closed — no wake attempted (counts as a placed
    /// wake for the callback contract: mailbox teardown owns the rest)
    Skipped,
    /// SetEvent itself failed — the wake channel is terminally dead;
    /// the CALLER (mailbox signal path) applies the terminal contract
    Latched,
}

/// Classification of a `MsgWaitForMultipleObjectsEx` result — every
/// legal result is an explicit branch; anything else (incl. WAIT_FAILED
/// and undocumented codes) is a typed terminal platform error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WaitClass {
    /// WAIT_OBJECT_0 — the mailbox wake event fired
    Event,
    /// WAIT_OBJECT_0 + handle_count — the message queue holds new input
    Input,
}

pub(crate) fn classify_wait(wait: WAIT_EVENT, handle_count: usize) -> UiResult<WaitClass> {
    if wait == WAIT_OBJECT_0 {
        return Ok(WaitClass::Event);
    }
    if wait.0 == WAIT_OBJECT_0.0 + handle_count as u32 {
        return Ok(WaitClass::Input);
    }
    if wait == WAIT_FAILED {
        return Err(UiError::Platform(format!(
            "MsgWaitForMultipleObjectsEx WAIT_FAILED: {}",
            std::io::Error::last_os_error()
        )));
    }
    Err(UiError::Platform(format!(
        "MsgWaitForMultipleObjectsEx returned undocumented result {wait:?}"
    )))
}

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
    pub(crate) fn wake_pump(&self) -> UiResult {
        if self.hwnd.0.is_null() || self.closed.load(Ordering::SeqCst) || self.pump_queued.get() {
            return Ok(()); // nothing to wake, or already armed
        }
        self.pump_queued.set(true);
        let posted =
            unsafe { PostMessageW(Some(self.hwnd), WM_PUMP, WPARAM(0), LPARAM(0)).is_ok() };
        if !posted {
            self.pump_queued.set(false);
            return Err(UiError::Platform("PostMessageW(WM_PUMP) failed".into()));
        }
        Ok(())
    }

    /// One semantic turn: drain the peer sink into the runtime, pump
    /// (bounded), drain the scheduler, apply dirty layout/paint, present if
    /// needed. WndProc + WM_PUMP both funnel here.
    ///
    /// Reentrancy: `OnTxUIActivate` inside `apply_bounds`/`draw` can call
    /// `TxSetFocus` → synchronous `WM_SETFOCUS` → `turn()`. A nested turn
    /// would run view()/layout while the outer one is mid-layout, so a
    /// reentrant call instead posts one coalesced `WM_PUMP`; the deferred
    /// turn runs the body after the outer frame unwinds — the transition
    /// is deferred, never dropped.
    pub(crate) fn turn(&mut self) -> UiResult {
        if self.in_turn.get() {
            // a failed wake surfaces — never "scheduled" silently
            self.wake_pump()?;
            return Ok(());
        }
        self.in_turn.set(true);
        let r = self.turn_body();
        self.in_turn.set(false);
        r
    }

    fn turn_body(&mut self) -> UiResult {
        // a pump post may still be in flight after this turn settles — it
        // clears the flag so a later reentrant call re-arms the post
        self.pump_queued.set(false);
        // safe-point: a cross-thread mailbox wake whose signal failed is a
        // broken progress guarantee — surface typed, same as wake_pump
        if self.rt.mailbox.take_wake_failure() {
            return Err(UiError::Platform("mailbox wake signal failed".into()));
        }
        // peer-emitted semantic events (ack edits, selections)
        let items: Vec<NativeSinkItem> = std::mem::take(&mut *self.peer_ctx.sink.lock().unwrap());
        for it in items {
            match it {
                NativeSinkItem::Edit { node, edit } => {
                    self.rt.edit_event(node, edit)?;
                }
                NativeSinkItem::Selection { node, sel } => {
                    self.rt.selection_event(node, sel)?;
                }
            }
        }
        let updated = self.rt.pump()?;
        // peer-creation spec stays current — new peers mount with the
        // live (appearance, theme)
        *self.peer_ctx.colors.borrow_mut() = (self.rt.appearance(), self.rt.theme.clone());
        // EFFECTIVE palette refresh — compare the RESOLVED color output
        // peers would apply, never the descriptors: a GetSysColor change
        // under identical (appearance, theme) is real palette drift, and
        // pre-seeding an applied-cache with descriptors would hide it.
        {
            let appearance = self.rt.appearance();
            let dark = self.rt.theme.dark;
            let forced = appearance.forced_colors;
            let sys = self.peer_ctx.sys_resolver.get();
            let (_fg, sel_bg, sel_fg) = palette(&self.rt.theme, &appearance, &sys);
            let resolver = forced.then_some(&sys as &dyn Fn(crate::style::SystemColor) -> [f32; 4]);
            // per-peer effective tuple — keyed by full NodeId so a same-
            // slot recreate compares fresh
            let peers: Vec<(u32, Rc<RefCell<WindowlessPeer>>)> = self
                .peer_ctx
                .registry
                .lock()
                .unwrap()
                .iter()
                .filter_map(|(slot, w)| w.upgrade().map(|p| (*slot, p)))
                .collect();
            let current: Vec<(NodeId, ([f32; 4], [f32; 4], [f32; 4]))> = peers
                .iter()
                .map(|(_, p)| {
                    let p = p.borrow();
                    (p.node(), p.effective_colors(sel_bg, sel_fg, dark, resolver))
                })
                .collect();
            let mut applied = self.applied_palette.borrow_mut();
            if applied.as_ref() != Some(&current) {
                for (_, p) in &peers {
                    p.borrow().set_colors(sel_bg, sel_fg, dark, resolver);
                }
                *applied = Some(current);
            }
        }
        // scheduler due work: frames -> events, chrome -> render side
        let chrome = self.rt.pump_sched(std::time::Instant::now())?;
        for ev in &chrome {
            self.apply_chrome(ev);
        }
        // dirty classification -> layout / paint work
        let dirty = self.rt.drain_dirty();
        // commit boundary: a removed/disabled focus or capture owner must
        // be resolved BEFORE layout/paint use stale state
        self.sanitize_focus()?;
        let mut needs_layout = !self.rects.is_empty() && self.order.is_empty();
        let mut needs_paint = !chrome.is_empty();
        let mut needs_uia = false;
        for (id, d) in dirty {
            if d & crate::runtime::Runtime::<S, M, U, V>::DIRTY_LAYOUT != 0 {
                needs_layout = true;
            }
            if d & crate::runtime::Runtime::<S, M, U, V>::DIRTY_SEMANTICS != 0 {
                needs_uia = true;
            }
            if d & crate::runtime::Runtime::<S, M, U, V>::DIRTY_PAINT != 0 {
                needs_paint = true;
                // ink-damage union — the node's committed rect plus any
                // shadow footprint; cumulative between paints. (The window
                // still repaints fully today, but the union is the promised
                // damage contract and is tested.)
                if let Some(r) = self.rects.get(&id).copied() {
                    // OLD committed ink (where the node last painted —
                    // covers a moved/shrunk/removed shadow) UNION new ink
                    let mut dr = self
                        .committed_ink
                        .borrow()
                        .get(&id)
                        .copied()
                        .map(|old| old.union(r))
                        .unwrap_or(r);
                    let sh = match self.rt.arena.get(id).map(|n| &n.data) {
                        Some(crate::node::NodeData::Container { kind, props }) => props
                            .resolved_box(*kind)
                            .and_then(|b| b.shadow.map(|s| (b, s))),
                        Some(crate::node::NodeData::Action {
                            style, disabled, ..
                        }) => {
                            let b = style.resolve(*disabled, false, false, false);
                            b.shadow.map(|s| (b, s))
                        }
                        Some(crate::node::NodeData::Editor { patch, .. }) => {
                            let c = layout::editor_chrome(patch);
                            c.shadow.map(|s| (c, s))
                        }
                        _ => None,
                    };
                    if let Some((_b, s)) = sh {
                        let ink = super::win32::render::shadow_ink_rect(&r, &s);
                        dr = dr.union(ink);
                    }
                    let acc = self.damage.get().map(|d| d.union(dr)).unwrap_or(dr);
                    self.damage.set(Some(acc));
                }
            }
        }
        if self.rects.is_empty() {
            needs_layout = true;
        }
        // built-in chrome state transitions feed the SAME classification —
        // a hover/pressed change that alters metric-bearing fields must
        // reach layout; a color-only transition only repaints
        {
            let d = self.state_dirty.replace(0);
            if d & 0b01 != 0 {
                needs_layout = true;
            }
            if d & 0b10 != 0 {
                needs_paint = true;
            }
        }
        // layout is driven by the CLASSIFIED work, not by "any message
        // arrived" — an update whose resolved output is unchanged does no
        // layout and no paint
        if needs_layout {
            self.relayout()?;
            needs_paint = true;
            needs_uia = true;
        }
        // live UIA state follows the committed tree — external clients see
        // name/enabled/bounds/order changes without a fresh WM_GETOBJECT.
        // `updated` still counts: focus/press semantics ride app messages
        // that need not flip a dirty bit.
        if needs_uia || updated || needs_paint {
            self.uia_refresh()?;
        }
        if needs_paint {
            self.paint()?;
        }
        self.service_peer_events()?;
        self.rearm_deadline()?;
        Ok(())
    }

    /// Chrome events from the scheduler — tooltip show, transition steps —
    /// become render-side state (the runtime emits no frames for these).
    fn apply_chrome(&mut self, ev: &SchedEvent) {
        match ev {
            SchedEvent::TooltipShow { node } => {
                self.show_tooltip(*node);
            }
            SchedEvent::TransitionStep { node, .. } | SchedEvent::TransitionDone { node, .. } => {
                // hover/focus chrome animation — invalidate the node rect
                if let Some(r) = self.rects.get(node).copied() {
                    self.counters
                        .animation_callbacks
                        .fetch_add(1, Ordering::Relaxed);
                    let _ = r;
                }
            }
            _ => {}
        }
    }

    /// The ONE backend→peer native delivery funnel — see `DeferredNative`
    /// for the contract. Returns the message LRESULT for the synchronous
    /// path; a queued delivery returns 0 (deferred messages are
    /// fire-and-forget by contract — senders needing a result must be on
    /// the synchronous path).
    ///
    /// Errors: `QueueOverflow` when the deferred queue is full;
    /// `UiError::Platform` when a non-deferrable message hits a borrowed
    /// peer (contract violation — such messages may never sit in the
    /// queue).
    /// The ONE backend→peer native delivery funnel — see `DeferredNative`
    /// for the contract. `Ok(Some(send))` = executed synchronously with the
    /// real `TxSendMessage` outcome (HRESULT consumption + LRESULT);
    /// `Ok(None)` = queued (deferred sends are fire-and-forget by contract
    /// — callers needing a result are on the synchronous path and treat
    /// `None` as "availability unknown").
    ///
    /// Errors: `QueueOverflow` when the deferred queue is full;
    /// `UiError::Platform` when a non-deferrable message hits a borrowed
    /// peer (contract violation — such messages may never sit in the
    /// queue).
    pub(crate) fn deliver_native(
        &mut self,
        id: NodeId,
        msg: u32,
        wparam: usize,
        lparam: isize,
    ) -> UiResult<Option<text::NativeSend>> {
        // FIFO: a live delivery may never overtake a queued one — while
        // anything is pending, enqueue behind it
        if !self.deferred_native.is_empty() {
            return self.enqueue_native(id, msg, wparam, lparam);
        }
        // NOTE: no peer_for() — its generation filter borrows the cell and
        // would panic during a live borrow; the deferred delivery runs the
        // slot lookup then instead
        let peer = self
            .peer_ctx
            .registry
            .lock()
            .unwrap()
            .get(&id.slot)
            .and_then(|w| w.upgrade());
        let Some(peer) = peer else {
            return Ok(None); // fenced: dead slot — nothing to deliver to
        };
        match peer.try_borrow() {
            Ok(p) if p.node() == id => {
                // msftedit runs host callbacks inside the send — SetFocus,
                // timers, invalidation all re-enter the WndProc
                // synchronously; in_turn keeps a nested turn() deferred
                // until this send unwinds (restore, not clear — the send
                // may itself run inside an outer turn)
                let was = self.in_turn.replace(true);
                let r = p.send(msg, wparam, lparam);
                self.in_turn.set(was);
                r.map(Some)
                    .map_err(|e| UiError::Platform(format!("peer send msg={msg:#x}: {e}")))
            }
            Ok(_) => Ok(None), // fenced: slot holds a different generation
            Err(_) => self.enqueue_native(id, msg, wparam, lparam),
        }
    }

    /// FIFO enqueue with progress + overflow semantics per the contract.
    fn enqueue_native(
        &mut self,
        id: NodeId,
        msg: u32,
        wparam: usize,
        lparam: isize,
    ) -> UiResult<Option<text::NativeSend>> {
        if !deferrable(msg) {
            return Err(UiError::Platform(format!(
                "native msg {msg:#x} is not deferrable (pointer or sync result)"
            )));
        }
        if self.deferred_native.len() >= crate::event::EVENT_QUEUE_CAP {
            return Err(UiError::QueueOverflow);
        }
        self.deferred_native.push_back(DeferredNative::Message {
            node: id,
            msg,
            wparam,
            lparam,
        });
        // guaranteed progress: outside a turn nothing else will reach
        // service_peer_events — arm one pump (coalesced); a failed post
        // is a typed failure, not silent starvation
        if !self.in_turn.get() {
            self.wake_pump()?;
        }
        Ok(None)
    }

    /// Raw window message to the focused peer (IME, focus, wheel routing).
    /// Delivery failures (queue overflow, contract violation) propagate —
    /// callers must not silently drop them.
    /// Focus resolution: `Some(c)` arrival capture = replay — `c` governs
    /// (and `None` inside it means NO owner at arrival — do not fall
    /// through to whatever is focused now); `None` = live dispatch →
    /// current focus. Generation fencing still applies at `peer_for`.
    fn focused_target(&self) -> Option<NodeId> {
        self.arrival_focus.get().unwrap_or(self.focus)
    }
    pub(crate) fn send_focused(&mut self, msg: u32, wparam: usize, lparam: isize) -> UiResult {
        if let Some(id) = self.focused_target() {
            self.deliver_native(id, msg, wparam, lparam)?;
        }
        Ok(())
    }

    /// IME delivery owner — the pinned composition peer while a
    /// composition is live; the focus-captured target otherwise. An
    /// explicitly OWNERLESS start (`Some(None)`) resolves to None — it
    /// must never latch onto whatever peer happens to gain focus later.
    /// (STARTCOMPOSITION establishes the pin, so it resolves through the
    /// arrival snapshot before `ime_owner` is set.)
    pub(crate) fn ime_target(&self) -> Option<NodeId> {
        match self.ime_owner.get() {
            Some(owner) => owner,
            None => self.focused_target(),
        }
    }

    /// Native IME traffic goes to the composition OWNER — a focus change
    /// mid-composition must not redirect preedit/candidate handling or
    /// the ending commit to another peer.
    pub(crate) fn send_ime(&mut self, msg: u32, wparam: usize, lparam: isize) -> UiResult {
        if let Some(id) = self.ime_target() {
            self.deliver_native(id, msg, wparam, lparam)?;
        }
        Ok(())
    }

    /// Drain every live peer's host-event queue: caret/capture/timer/change
    /// notifications msftedit posted while a call ran.
    pub(crate) fn service_peer_events(&mut self) -> UiResult {
        // deferred native sends first — FIFO drain; the borrow that blocked
        // them is unwound by the time a turn calls this. Sends can re-queue
        // (a reentrant delivery while the peer is still borrowed), so the
        // pass is bounded at the queue cap — anything still pending drains
        // on the next service call.
        let mut budget = crate::event::EVENT_QUEUE_CAP;
        while budget > 0
            && let Some(item) = self.deferred_native.pop_front()
        {
            budget -= 1;
            let DeferredNative::Message {
                node,
                msg,
                wparam,
                lparam,
            } = item;
            let peer = self
                .peer_ctx
                .registry
                .lock()
                .unwrap()
                .get(&node.slot)
                .and_then(|w| w.upgrade())
                .filter(|p| p.borrow().node() == node);
            if let Some(peer) = peer {
                // generation-checked; the send's host callbacks may defer
                // further messages — they enqueue behind, drained next pass.
                // Delivery failure surfaces — a silently dropped message is
                // a lost input, not a transient skip.
                let was = self.in_turn.replace(true);
                let r = peer.borrow().send(msg, wparam, lparam);
                self.in_turn.set(was);
                r?;
            }
        }
        // bounded drain left work — arm an explicit continuation instead
        // of depending on an unrelated later message; failure is typed
        if !self.deferred_native.is_empty() {
            self.wake_pump()?;
        }
        let slots: Vec<u32> = self
            .peer_ctx
            .registry
            .lock()
            .unwrap()
            .keys()
            .copied()
            .collect();
        for slot in slots {
            let Some(peer) = self
                .peer_ctx
                .registry
                .lock()
                .unwrap()
                .get(&slot)
                .and_then(|w| w.upgrade())
            else {
                continue;
            };
            let events = peer.borrow().drain()?;
            for ev in events {
                match ev {
                    HostEvent::Invalidate => {
                        self.counters
                            .requested_redraws
                            .fetch_add(1, Ordering::Relaxed);
                        self.invalidate_slot(slot);
                    }
                    HostEvent::Change => {
                        // committed native edit — read-back text, mint a
                        // revision, route the semantic event
                        self.native_commit(slot, &peer)?;
                    }
                    HostEvent::SelChange => {
                        self.native_selection(slot, &peer)?;
                    }
                    HostEvent::Capture(on) => {
                        if on {
                            unsafe {
                                windows::Win32::UI::Input::KeyboardAndMouse::SetCapture(self.hwnd);
                            }
                            self.native_capture = peer.borrow().node_opt();
                        } else {
                            unsafe {
                                let _ =
                                    windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture();
                            }
                            self.native_capture = None;
                        }
                    }
                    HostEvent::Caret => {
                        self.counters
                            .native_caret_redraws
                            .fetch_add(1, Ordering::Relaxed);
                        self.apply_caret(&peer);
                    }
                    HostEvent::GrabFocus => {}
                }
            }
        }
        // armed-timer cleanup is the peer Drop's job — the pool is the
        // authority, there is no second store to prune
        Ok(())
    }

    fn invalidate_slot(&self, slot: u32) {
        if let Some(r) = self
            .rects
            .keys()
            .find(|id| id.slot == slot)
            .and_then(|id| self.rects.get(id))
        {
            let s = self.peer_ctx.scale.get();
            let mut p = r.physical(s);
            // invalidation must never under-cover — +1 keeps the bottom/
            // right edge inside the damage rect (conservative clip, not a
            // unit conversion)
            p.right += 1;
            p.bottom += 1;
            let rc: RECT = p.into();
            unsafe {
                let _ = windows::Win32::Graphics::Gdi::InvalidateRect(
                    Some(self.hwnd),
                    Some(&rc),
                    false,
                );
            }
        }
    }

    /// A committed native edit: read the peer's committed text, mint a
    /// checked revision, emit the semantic `Edit` event. An IME composition
    /// in progress forwards the FINAL commit before CompositionEnd — the
    /// transient preedit never reaches `on_edit`. The peer borrow NEVER
    /// spans an `rt.*` call — a consumer handler may touch its own
    /// `TextValue`/peer handle during the event.
    fn native_commit(&mut self, slot: u32, peer: &Rc<RefCell<WindowlessPeer>>) -> UiResult {
        let (text, anchor, focus, node, base, binding) = {
            let p = peer.borrow();
            (
                p.text()?,
                p.selection_utf16()?.0,
                p.selection_utf16()?.1,
                p.node(),
                p.peer_rev(),
                p.binding(),
            )
        };
        // EN_CHANGE during an active composition means preedit leaked into
        // the committed channel — the runtime contract is committed-only,
        // so a preedit notification is suppressed, never routed to
        // TextValue or Submit.
        if self.rt.composition_active(node) {
            self.counters
                .preedit_commits_suppressed
                .fetch_add(1, Ordering::Relaxed);
            return Ok(());
        }
        let result = crate::text::TextRevision::mint();
        if let Some(binding) = binding {
            // UTF-8 positions of the selection at THIS commit — a failed
            // index conversion is a typed failure, never (0,0)/end-of-text
            let to_utf8 = |i: usize| {
                crate::text::utf16_index_to_utf8(&text, i).ok_or_else(|| {
                    UiError::Platform(format!("selection offset {i} not representable"))
                })
            };
            let sel = TextSelection {
                revision: result,
                anchor: to_utf8(anchor)?,
                focus: to_utf8(focus)?,
            };
            self.rt.edit_event(
                node,
                TextEdit {
                    text,
                    base,
                    result,
                    origin: EditOrigin::NativePeer,
                    binding,
                },
            )?;
            self.rt.selection_event(node, sel)?;
            peer.borrow().record_commit(result);
        }
        let _ = slot;
        Ok(())
    }

    fn native_selection(&mut self, _slot: u32, peer: &Rc<RefCell<WindowlessPeer>>) -> UiResult {
        let (text, anchor, focus, rev, node) = {
            let p = peer.borrow();
            let (a, f) = p.selection_utf16()?;
            (p.text()?, a, f, p.peer_rev(), p.node())
        };
        // a selection notification during composition reflects PREEDIT
        // coordinates — suppressed like commits; the committed snapshot
        // is the only state consumers may observe
        if self.rt.composition_active(node) {
            return Ok(());
        }
        // checked conversions — a failed UTF-16->UTF-8 index is a typed
        // failure, never a fallback to (0,0) or end-of-text
        let to_utf8 = |i: usize| {
            crate::text::utf16_index_to_utf8(&text, i)
                .ok_or_else(|| UiError::Platform(format!("selection offset {i} not representable")))
        };
        let sel = TextSelection {
            revision: rev,
            anchor: to_utf8(anchor)?,
            focus: to_utf8(focus)?,
        };
        self.rt.selection_event(node, sel)?;
        Ok(())
    }

    /// Peer-targeted coordinate authority — ONE effective geometry.
    /// While a composition-pending relatch is parked, `effective_geometry`
    /// still returns the old bounds/scale — pointer/caret math agrees
    /// with the surface msftedit actually renders, not the pending layout.
    fn peer_origin(&self, id: NodeId) -> Option<space::PeerOrigin> {
        let peer = self.peer_for(id)?;
        let p = peer.borrow();
        let (r, sc) = p.effective_geometry();
        space::PeerOrigin::from_logical(r, sc)
    }

    /// Keyboard/pointer focus moved — caret drawn only for the focus owner.
    fn apply_caret(&self, peer: &Rc<RefCell<WindowlessPeer>>) {
        let p = peer.borrow();
        let Some(node) = p.node_opt() else { return };
        if self.focus != Some(node) {
            return;
        }
        let (created, shown, pos, size) = p.caret();
        if !created {
            return;
        }
        // the caret reports peer-local px — land it through THE effective
        // peer geometry (the snapped origin both pointer and host callbacks
        // share); during a pending relatch this is still the OLD space —
        // matching where msftedit actually draws the caret
        let Some(org) = self.peer_origin(node) else {
            return; // unrepresentable peer origin — drop the placement
        };
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::*;
            let Some(at) = org.to_client(space::PeerLocalPoint(pos.into())) else {
                return; // unrepresentable — drop the placement
            };
            let _ = CreateCaret(self.hwnd, None, size.cx, size.cy);
            let _ = SetCaretPos(at.0.x, at.0.y);
            if shown {
                let _ = ShowCaret(Some(self.hwnd));
            }
        }
    }

    // ----- native timers ----------------------------------------------------
    //
    // One ID namespace: framework timers are 1 (deadline); peer timers live
    // at NATIVE_TIMER_BASE + n. The map keys on the WIN32 id and carries the
    // full NodeId (slot+generation), so a stale fire or a reused slot can
    // never deliver into a replacement peer. A repeated (node, richedit-id)
    // request reschedules the SAME win32 id rather than allocating a new
    // one; disarm kills every win32 id for that (node, nid).

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
    pub(crate) fn relayout(&mut self) -> UiResult {
        if self.peer_ctx.hwnd.get().is_invalid() {
            return Ok(());
        }
        // state branches can consume content insets — expose the live
        // interaction state so layout resolves the same style the paint does
        self.peer_ctx.hot.set(self.hot);
        self.peer_ctx.pressed.set(self.pressed);
        self.peer_ctx.focus.set(self.focus);
        let mut layout = LayoutCache::new(self.peer_ctx.scale.get());
        let (rects, order) = layout.run(&mut self.rt, &self.peer_ctx)?;
        // prune stale ids — backend cache holds live NodeIds only
        self.rects = rects;
        self.order = order;
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
    fn accumulate_ink_damage(&self, new_ink: &HashMap<NodeId, LogicalRect>) {
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
    fn ink_map(&self) -> HashMap<NodeId, LogicalRect> {
        let mut m: HashMap<NodeId, LogicalRect> = HashMap::new();
        for (&id, &r) in self.rects.iter() {
            let mut i = r;
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
                        _ => None,
                    }
                });
            if let Some(sh) = shadow {
                i = i.union(render::shadow_ink_rect(&r, &sh));
            }
            m.insert(id, i);
        }
        m
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

    fn paint_inner(&mut self, draw: impl FnOnce(&Self) -> UiResult) -> UiResult {
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

    #[cfg(test)]
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
    fn show_tooltip(&mut self, node: NodeId) {
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

    /// A node nested inside a semantic `Action` is decorative content —
    /// never the hit target; the enclosing Action owns press/focus.
    /// Parents are slot-indexed — walk slots, generation is irrelevant to
    /// ancestry (a dead parent can't have live children).
    fn inside_action(&self, id: NodeId) -> bool {
        let mut cur = self.rt.arena.get(id).and_then(|n| n.parent);
        while let Some(slot) = cur {
            match self.rt.arena.slot(slot) {
                Some(pn) => {
                    if matches!(pn.data, NodeData::Action { .. }) {
                        return true;
                    }
                    cur = pn.parent;
                }
                None => return false,
            }
        }
        false
    }

    /// Hit-test a DIP point: topmost visible interactive node wins; nodes
    /// inside an Action defer to it (Action owns the semantic target).
    pub(crate) fn hit_test(&self, p: Point) -> Option<NodeId> {
        for &id in self.order.iter().rev() {
            let Some(n) = self.rt.arena.get(id) else {
                continue;
            };
            if n.visibility != Visibility::Visible || self.inside_action(id) {
                continue;
            }
            let Some(r) = self.rects.get(&id) else {
                continue;
            };
            if r.contains(p) {
                match &n.data {
                    // disabled/non-interactive nodes are inert chrome — no
                    // hover, pressed, focus, or activation state attaches
                    NodeData::Button { .. } | NodeData::Editor { .. } | NodeData::Action { .. }
                        if n.interactive() =>
                    {
                        return Some(id);
                    }
                    // CustomRender owns its own hit geometry — the rect is
                    // only the bounding box; an actionable custom shape can
                    // refuse a hit inside its bounds
                    NodeData::Custom { render, .. } if n.interactive() => {
                        let local = Point {
                            x: p.x - r.x,
                            y: p.y - r.y,
                        };
                        if render.hit_test(local, *r) {
                            return Some(id);
                        }
                    }
                    _ => {}
                }
            }
        }
        None
    }

    /// Route a pointer event — editors get the raw Win32 message (the
    /// caller's wparam carries MK_* modifier/button flags, preserved
    /// end-to-end); semantic nodes get `NodeEvent` phases.
    pub(crate) fn pointer(
        &mut self,
        phase: crate::node::PointerPhase,
        pos_px: space::ClientPhysicalPoint,
        button: Option<PointerButton>,
        wparam: usize,
    ) -> UiResult {
        let pos: Point = {
            let s = self.peer_ctx.scale.get();
            pos_px.0.logical(s)
        };
        let hit = if self.native_capture.is_some() {
            self.native_capture
        } else {
            self.hit_test(pos)
        };
        self.mouse = pos;
        match phase {
            crate::node::PointerPhase::Enter => {}
            crate::node::PointerPhase::Leave => {}
            _ => {}
        }
        // hot tracking for semantic nodes
        if hit != self.hot {
            if let Some(old) = self.hot {
                self.push_input(
                    old,
                    NodeEvent::Pointer(
                        PointerEvent {
                            position: pos,
                            button,
                            modifiers: modifiers_now(),
                        },
                        crate::node::PointerPhase::Leave,
                    ),
                )?;
            }
            if let Some(id) = hit
                && !self.peer_for(id).is_some()
            {
                self.push_input(
                    id,
                    NodeEvent::Pointer(
                        PointerEvent {
                            position: pos,
                            button,
                            modifiers: modifiers_now(),
                        },
                        crate::node::PointerPhase::Enter,
                    ),
                )?;
            }
            self.hot = hit;
        }
        if let Some(id) = hit {
            // native peer? forward the raw message + focus on down
            if self.peer_for(id).is_some() {
                // peer-local = content-local — THE shared transform:
                // snapped physical origin, integer subtraction only
                // (fractional-origin re-rounding disagreement resolved by
                // geom::PeerOrigin — Gate 6/20)
                let Some(origin) = self.peer_origin(id) else {
                    return Ok(()); // unrepresentable peer origin — drop
                };
                let Some(lp_px) = origin.to_local(pos_px).map(|l| l.0) else {
                    return Ok(()); // unrepresentable coordinate — drop
                };
                let msg = match (phase, button) {
                    (crate::node::PointerPhase::Down, Some(PointerButton::Primary)) => {
                        self.set_focus(Some(id))?;
                        WM_LBUTTONDOWN
                    }
                    (crate::node::PointerPhase::Up, Some(PointerButton::Primary)) => WM_LBUTTONUP,
                    _ => WM_MOUSEMOVE,
                };
                // out-of-MAKELPARAM-range px coordinates cannot reach the
                // peer — a contract failure, surfaced not truncated
                let Some(lp) = space::try_lparam_px(lp_px) else {
                    return Err(crate::UiError::Platform(
                        "peer-local coordinate outside LPARAM range".into(),
                    ));
                };
                // wparam is the caller's MK_* flags — preserved metadata
                self.deliver_native(id, msg, wparam, lp)?;
                self.service_peer_events()?;
                return self.turn();
            }
            if phase == crate::node::PointerPhase::Down && button == Some(PointerButton::Primary) {
                self.set_focus(Some(id))?;
            }
            // semantic node events
            let ev = match phase {
                crate::node::PointerPhase::Down => {
                    let old = self.interact(id);
                    self.pressed = Some(id);
                    self.mark_state_dirty(id, old);
                    NodeEvent::Pointer(
                        PointerEvent {
                            position: pos,
                            button,
                            modifiers: modifiers_now(),
                        },
                        crate::node::PointerPhase::Down,
                    )
                }
                crate::node::PointerPhase::Up => {
                    let was_pressed = self.pressed == Some(id);
                    let old = self.interact(id);
                    self.pressed = None;
                    if was_pressed {
                        self.mark_state_dirty(id, old);
                    }
                    // press = down+up on the same node
                    if was_pressed {
                        self.push_input(id, NodeEvent::Press)?;
                        return self.turn();
                    }
                    NodeEvent::Pointer(
                        PointerEvent {
                            position: pos,
                            button,
                            modifiers: modifiers_now(),
                        },
                        crate::node::PointerPhase::Up,
                    )
                }
                _ => NodeEvent::Pointer(
                    PointerEvent {
                        position: pos,
                        button,
                        modifiers: Modifiers::default(),
                    },
                    phase,
                ),
            };
            self.push_input(id, ev)?;
        }
        self.turn()
    }

    fn push_input(&mut self, node: NodeId, ev: NodeEvent) -> UiResult {
        self.rt
            .events
            .push(QueuedEvent { node, payload: ev })
            .map_err(|_| UiError::QueueOverflow)
    }

    /// Registry lookup that also proves generation identity — a slot reuse
    /// never routes input to a stale peer's replacement.
    /// A node's resolved editor chrome (recipe + authored patch) — the
    /// transform contract's style input.
    pub(crate) fn editor_chrome_of(&self, node: NodeId) -> crate::style::BoxStyle {
        match self.rt.arena.get(node).map(|n| &n.data) {
            Some(crate::node::NodeData::Editor { patch, .. }) => layout::editor_chrome(patch),
            _ => crate::style::text_input_chrome_recipe(),
        }
    }

    pub(crate) fn peer_for(&self, node: NodeId) -> Option<Rc<RefCell<WindowlessPeer>>> {
        self.peer_ctx
            .registry
            .lock()
            .unwrap()
            .get(&node.slot)
            .and_then(|w| w.upgrade())
            .filter(|p| p.borrow().node() == node)
    }

    /// Keyboard: focused editor gets the raw message FIRST — native
    /// handling decides whether the key remains available for framework
    /// interpretation (a key consumed by msftedit — e.g. Enter confirming
    /// an IME composition — never produces Submit). Everything else is a
    /// semantic `NodeEvent::Key`/`Press`.
    pub(crate) fn key_msg(&mut self, msg: u32, vk: usize, lparam: isize) -> UiResult {
        let key = vk as u32;
        let ev = match key {
            k if k == VK_TAB.0 as u32 => Key::Tab,
            k if k == VK_RETURN.0 as u32 => Key::Enter,
            k if k == VK_SPACE.0 as u32 => Key::Space,
            k if k == VK_ESCAPE.0 as u32 => Key::Escape,
            k if k == VK_BACK.0 as u32 => Key::Backspace,
            k if k == VK_DELETE.0 as u32 => Key::Delete,
            k if k == VK_LEFT.0 as u32 => Key::ArrowLeft,
            k if k == VK_RIGHT.0 as u32 => Key::ArrowRight,
            k if k == VK_UP.0 as u32 => Key::ArrowUp,
            k if k == VK_DOWN.0 as u32 => Key::ArrowDown,
            other => Key::Other(other),
        };
        // Tab traversal — focus moves through eligible nodes
        if msg == WM_KEYDOWN && key == VK_TAB.0 as u32 {
            self.focus_step(unsafe { GetKeyState(VK_SHIFT.0 as i32) } < 0)?;
            return self.turn();
        }
        // focused editor gets the raw key (native editing, IME) —
        // a deferred replay delivers to the arrival-time owner
        if let Some(focus) = self.focused_target()
            && self.peer_for(focus).is_some()
        {
            // SUBMIT CONTRACT — the canonical boundary is: plain Enter
            // submits, Shift+Enter edits (newline). The decision is made
            // BEFORE delivery because a submitting Enter must NOT reach
            // msftedit (it would insert a newline AND submit — two effects
            // for one keypress, which the contract forbids).
            if msg == WM_KEYDOWN && ev == Key::Enter && !self.rt.composition_active(focus) {
                match self.submit_decision(focus) {
                    SubmitDecision::Submit => {
                        self.push_input(focus, NodeEvent::Submit)?;
                        return self.turn();
                    }
                    SubmitDecision::Edit => {
                        // editing Enter (newline / modifier-not-met) — falls
                        // through to normal delivery, never also submits
                    }
                }
            }
            // send first — the consumed signal decides availability; a
            // deferred delivery reports no result, and an active
            // composition owns the keyboard either way
            let delivered = self.deliver_native(focus, msg, vk, lparam)?;
            self.service_peer_events()?;
            let consumed = match delivered {
                Some(s) => s.consumed(),
                // no synchronous result — treat as consumed (never submit
                // on an unknown; the queued key still reaches the peer)
                None => true,
            };
            if msg == WM_KEYDOWN && !self.rt.composition_active(focus) {
                // unconsumed → policy decides; a consumed key is editing
                // alone (the Enter/submit fork happened BEFORE delivery —
                // a consumed Enter was never a submit candidate)
                if !consumed && let Some(sub) = self.check_submit(focus, ev) {
                    self.push_input(focus, sub)?;
                }
            }
            return self.turn();
        }
        if msg != WM_KEYDOWN {
            return Ok(());
        }
        // semantic focus: Space/Enter on a button is Press — a deferred
        // replay delivers to the ARRIVAL-captured owner: an explicitly
        // absent owner must not activate a newly focused button, and a
        // captured A must not be redirected to B
        if let Some(focus) = self.focused_target() {
            match ev {
                Key::Space | Key::Enter => {
                    self.push_input(focus, NodeEvent::Press)?;
                    return self.turn();
                }
                _ => {
                    self.push_input(
                        focus,
                        NodeEvent::Key(KeyEvent {
                            key: ev,
                            modifiers: modifiers_now(),
                        }),
                    )?;
                    return self.turn();
                }
            }
        }
        Ok(())
    }

    /// The Enter decision — one authority for the submit/edit fork.
    /// IME-committed Enter never reaches here (composition owns it).
    fn submit_decision(&self, node: NodeId) -> SubmitDecision {
        let Some(n) = self.rt.arena.get(node) else {
            return SubmitDecision::Edit;
        };
        let NodeData::Editor {
            submit, multiline, ..
        } = &n.data
        else {
            return SubmitDecision::Edit;
        };
        let shift =
            unsafe { windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(VK_SHIFT.0 as i32) }
                < 0;
        let ctrl = unsafe {
            windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(VK_CONTROL.0 as i32)
        } < 0;
        match (submit, multiline, shift, ctrl) {
            // single-line has no newline affordance — Enter submits under
            // ANY modifier state (Shift is meaningless without a newline)
            (SubmitPolicy::Enter, false, _, _) => SubmitDecision::Submit,
            // multiline: plain Enter submits; Shift+Enter is the newline
            (SubmitPolicy::Enter, true, true, _) => SubmitDecision::Edit,
            (SubmitPolicy::Enter, true, false, _) => SubmitDecision::Submit,
            (SubmitPolicy::ModifierEnter, _, _, true) => SubmitDecision::Submit,
            _ => SubmitDecision::Edit,
        }
    }

    /// Non-Enter keys that arrived unconsumed — reserved for future
    /// semantic keys; today only Enter has policy.
    fn check_submit(&self, node: NodeId, key: Key) -> Option<NodeEvent> {
        if key != Key::Enter {
            return None;
        }
        match self.submit_decision(node) {
            SubmitDecision::Submit => Some(NodeEvent::Submit),
            SubmitDecision::Edit => None,
        }
    }
    /// Focus next/previous eligible node (Tab/Shift-Tab traversal).
    pub(crate) fn focus_step(&mut self, back: bool) -> UiResult {
        let eligible: Vec<NodeId> = self
            .order
            .iter()
            .copied()
            .filter(|&id| {
                let Some(n) = self.rt.arena.get(id) else {
                    return false;
                };
                n.visibility == Visibility::Visible
                    && n.interactive()
                    && matches!(
                        n.data,
                        NodeData::Button { .. } | NodeData::Editor { .. } | NodeData::Action { .. }
                    )
            })
            .collect();
        if eligible.is_empty() {
            return Ok(());
        }
        let idx = self
            .focus
            .and_then(|f| eligible.iter().position(|&id| id == f))
            .unwrap_or(usize::MAX);
        let next = if back {
            match idx {
                usize::MAX => eligible.len() - 1,
                0 => eligible.len() - 1,
                i => i - 1,
            }
        } else {
            match idx {
                usize::MAX => 0,
                i => (i + 1) % eligible.len(),
            }
        };
        self.set_focus(Some(eligible[next]))
    }

    /// Focus a node — blur the old (still routed even when it just hid),
    /// focus the new. Peers get the raw focus messages so caret/edit state
    /// tracks the framework focus owner.
    pub(crate) fn set_focus(&mut self, node: Option<NodeId>) -> UiResult {
        if self.focus == node {
            return Ok(());
        }
        let olds: Vec<(NodeId, InteractState)> = [self.focus, node]
            .into_iter()
            .flatten()
            .map(|id| (id, self.interact(id)))
            .collect();
        for (id, old) in olds {
            self.mark_state_dirty(id, old);
        }
        if let Some(old) = self.focus {
            if self.rt.arena.is_live(old) {
                self.push_input(old, NodeEvent::Focus(false))?;
            }
            self.deliver_native(old, WM_KILLFOCUS, 0, 0)?;
        }
        self.focus = node;
        if let Some(u) = &self.uia {
            u.set_focus(node);
        }
        if let Some(id) = node {
            self.push_input(id, NodeEvent::Focus(true))?;
            self.deliver_native(id, WM_SETFOCUS, 0, 0)?;
        }
        Ok(())
    }

    /// The resolved interaction-state inputs — what the visual would be
    /// IF (pressed,hot,focus) held these values. Callers snapshot it
    /// BEFORE and AFTER flipping the state fields.
    fn interact(&self, id: NodeId) -> InteractState {
        InteractState {
            pressed: self.pressed == Some(id),
            hot: self.hot == Some(id),
            focus: self.focus == Some(id),
        }
    }

    /// Resolve the node's effective visual under a hypothetical
    /// interaction state — the compare target for transition
    /// classification. Same recipe+patch+state chain the renderer runs.
    fn resolved_at(&self, id: NodeId, st: InteractState) -> Option<crate::style::VisualStyle> {
        let n = self.rt.arena.get(id)?;
        let dark = self.rt.theme.dark;
        let forced = self.rt.forced_resolver();
        match &n.data {
            NodeData::Button {
                variant,
                size,
                style,
                disabled,
                ..
            } => {
                let state = crate::style::StyleState::classify(*disabled, st.pressed, st.hot);
                let mut v =
                    crate::style::resolve_button(*variant, *size, style, state, st.focus, dark);
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
                })
            }
            NodeData::Editor { patch, .. } => {
                let mut b = crate::style::resolve_text_input_chrome(patch);
                crate::style::os_enforce_box(&mut b, forced);
                Some(crate::style::VisualStyle {
                    box_style: b,
                    text_style: crate::style::TextStyle::default(),
                })
            }
            _ => None,
        }
    }

    /// An interaction-state transition on `id` — classify by the ACTUAL
    /// old->new resolved visual difference, never by "some branch
    /// somewhere has metrics". Identical resolved output = no work;
    /// metric-bearing fields moving = LAYOUT|PAINT; paint-only = PAINT.
    fn mark_state_dirty(&self, id: NodeId, old: InteractState) {
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

    /// `WM_GETOBJECT` — build/refresh the UIA root + live children once per
    /// request. Rebuild remints live generations, fencing stale providers.
    pub(crate) fn uia_provider(&mut self) -> UiResult<Option<uia::IRawRoot>> {
        use windows::Win32::UI::Accessibility::*;
        if self.uia.is_none() {
            let root = uia::UiaRoot::new(self.hwnd, "rust-ui")
                .map_err(|e| UiError::Platform(format!("UiaRoot: {e}")))?;
            // a late-created root starts with the backend's CURRENT focus —
            // a provider built after focus was taken must not lie
            root.set_focus(self.focus);
            self.uia = Some(root);
        }
        let Some(u) = self.uia.as_ref() else {
            return Ok(None);
        };
        let mut kids = Vec::new();
        for &id in &self.order {
            let Some(n) = self.rt.arena.get(id) else {
                continue;
            };
            let Some(r) = self.rects.get(&id).copied() else {
                continue;
            };
            let scale = self.peer_ctx.scale.get();
            let pr = r.physical(scale); // DIP -> client px (UIA wants px)
            // checked conversion — a failed ClientToScreen skips the node
            // rather than publishing unsound coordinates
            let Some(sp) = space::client_to_screen(
                self.hwnd,
                space::ClientPhysicalPoint(space::PhysicalPoint {
                    x: pr.left,
                    y: pr.top,
                }),
            ) else {
                continue;
            };
            let pt = sp.0;
            let rect = UiaRect {
                left: pt.x as f64,
                top: pt.y as f64,
                width: (pr.right - pr.left) as f64,
                height: (pr.bottom - pr.top) as f64,
            };
            let (name, ct, loc) = match &n.data {
                NodeData::Button { text, .. } => {
                    (text.to_string(), UIA_ButtonControlTypeId, "Button")
                }
                NodeData::Label { text, .. } => (text.to_string(), UIA_TextControlTypeId, "Text"),
                NodeData::Editor {
                    accessible_label, ..
                } => (
                    accessible_label
                        .clone()
                        .unwrap_or_else(|| "editor".into())
                        .to_string(),
                    UIA_EditControlTypeId,
                    "Edit",
                ),
                NodeData::Custom { render, .. } => {
                    let s = render.semantics();
                    let (ct, loc) = match s.role {
                        crate::ui::Role::Button => (UIA_ButtonControlTypeId, "Button"),
                        crate::ui::Role::Image => (UIA_ImageControlTypeId, "Image"),
                        crate::ui::Role::Text => (UIA_TextControlTypeId, "Text"),
                        _ => (UIA_CustomControlTypeId, "Custom"),
                    };
                    (s.label, ct, loc)
                }
                // semantic action — Button when actionable (Invoke), else a
                // named group; children are decorative
                NodeData::Action { label, .. } => {
                    if n.factories.on_press.is_some() {
                        (label.to_string(), UIA_ButtonControlTypeId, "Button")
                    } else {
                        (label.to_string(), UIA_GroupControlTypeId, "Group")
                    }
                }
                _ => continue,
            };
            kids.push(uia::ChildBuild {
                automation_id: n.automation_id.as_deref().unwrap_or("").to_string(),
                id,
                name,
                ct,
                localized: loc,
                rect,
                actionable: n.factories.on_press.is_some(),
                enabled: n.interactive(),
                peer_node: matches!(n.data, NodeData::Editor { .. }),
                native: None,
            });
        }
        // editors get the REAL windowless provider when available — the
        // registry is generation-keyed so a same-slot replacement can
        // never inherit the retired provider
        for k in &mut kids {
            if !k.peer_node {
                continue;
            }
            let registered = u
                .native
                .lock()
                .unwrap()
                .get(&k.id.slot)
                .is_some_and(|(g, _)| *g == k.id.generation);
            if registered {
                continue; // rebuild() picks it up from the registry
            }
            let Some(peer) = self.peer_for(k.id) else {
                continue;
            };
            let Some(iid) = peer.borrow().windowless_acc_iid() else {
                continue;
            };
            if let Ok(raw) = peer.borrow().query_iid(&iid) {
                let acc: IRicheditWindowlessAccessibility =
                    unsafe { windows::core::Interface::from_raw(raw) };
                if let Ok(site) = u.site(k.id)
                    && let Ok(prov) = unsafe { acc.CreateProvider(&site) }
                    && let Ok(frag) = prov.cast::<IRawElementProviderFragment>()
                {
                    u.register_native(k.id, frag.clone()).ok();
                    k.native = Some(frag);
                }
            }
        }
        u.rebuild(kids)
            .map_err(|e| UiError::Platform(format!("uia rebuild: {e}")))?;
        Ok(Some(u.provider()))
    }

    /// Push the current tree into the UIA tables when a client has ever
    /// asked for them — called at each commit boundary so external clients
    /// see live name/enabled/bounds/order WITHOUT needing a fresh
    /// WM_GETOBJECT. No-op until the root exists.
    pub(crate) fn uia_refresh(&mut self) -> UiResult {
        if self.uia.is_none() {
            return Ok(());
        }
        self.uia_provider()?;
        Ok(())
    }

    /// UIA press/focus post landed — the generation check happens HERE so a
    /// stale external press can't hit a newer peer, and eligibility is the
    /// SAME contract as pointer input: a disabled node cannot be focused
    /// or invoked through UIA either.
    pub(crate) fn uia_action(&mut self, slot: u32, generation: u64, focus: bool) -> UiResult {
        let id = NodeId { slot, generation };
        let eligible = self
            .rt
            .arena
            .get(id)
            .is_some_and(|n| n.interactive() && n.visibility == Visibility::Visible);
        if !eligible {
            return Ok(());
        }
        if focus {
            self.set_focus(Some(id))?;
        } else {
            self.push_input(id, NodeEvent::Press)?;
        }
        self.turn()
    }

    /// IDC_IBEAM inside an eligible editor, IDC_ARROW elsewhere (the class
    /// cursor stays arrow so nothing inherits a busy cursor).
    pub(crate) fn update_cursor(&self) {
        unsafe {
            let cur = if self.peer_for_opt(self.hot).is_some() {
                IDC_IBEAM
            } else {
                IDC_ARROW
            };
            let _ = SetCursor(LoadCursorW(None, cur).ok());
        }
    }
    /// peer at hit location — editor = Some; painted node = None
    pub(crate) fn peer_for_opt(&self, hit: Option<NodeId>) -> Option<Rc<RefCell<WindowlessPeer>>> {
        hit.and_then(|id| self.peer_for(id))
    }

    /// physical-window-px -> window-client DIP
    /// client px point from an LPARAM — the ONLY packed-coords decoder;
    /// logical conversion happens inside pointer/hover through ScaleFactor
    pub(crate) fn pt_px(&self, lp: LPARAM) -> space::ClientPhysicalPoint {
        space::ClientPhysicalPoint(space::PhysicalPoint {
            x: lp.0 as i16 as i32,
            y: ((lp.0 >> 16) as i16) as i32,
        })
    }
    /// client px -> window DIP via the shared ScaleFactor
    pub(crate) fn pt(&self, lp: LPARAM) -> Point {
        let s = self.peer_ctx.scale.get();
        let d = self.pt_px(lp).0.logical(s);
        Point { x: d.x, y: d.y }
    }
    /// peer holding focus (if any) — deferred replays resolve the
    /// arrival-time owner so a focus move mid-queue can't redirect input
    pub(crate) fn focus_peer(&self) -> Option<Rc<RefCell<WindowlessPeer>>> {
        self.focused_target().and_then(|id| self.peer_for(id))
    }
    /// WM_CHAR/WM_SYSCHAR — the focused peer sees the raw message with its
    /// real lparam (repeat count, scan code, alt flag are part of the
    /// native text-input contract)
    /// WM_CHAR/WM_SYSCHAR — a focused NATIVE peer sees the raw message;
    /// a focused painted/custom node sees the public semantic route
    /// (`Key::Char`). Surrogate pairs are joined here so consumers get a
    /// complete `char` — never a lone lead unit.
    pub(crate) fn char_msg(&mut self, msg: u32, wparam: usize, lparam: isize) -> UiResult {
        let Some(id) = self.focused_target() else {
            return self.turn();
        };
        if self.peer_for(id).is_some() {
            self.deliver_native(id, msg, wparam, lparam)?;
            self.service_peer_events()?;
            return self.turn();
        }
        if msg != WM_CHAR {
            return Ok(()); // WM_SYSCHAR carries no portable semantic
        }
        let unit = wparam as u32;
        let ch = if (0xD800..0xDC00).contains(&unit) {
            self.pending_lead_surrogate.set(Some(unit as u16));
            return self.turn(); // pair not yet complete
        } else if (0xDC00..0xE000).contains(&unit) {
            let Some(lead) = self.pending_lead_surrogate.take() else {
                return self.turn(); // stray trail unit — drop
            };
            char::from_u32(0x10000 + (((lead as u32) - 0xD800) << 10) + (unit - 0xDC00))
        } else {
            char::from_u32(unit)
        };
        if let Some(c) = ch {
            self.push_input(
                id,
                NodeEvent::Key(KeyEvent {
                    key: Key::Char(c),
                    modifiers: modifiers_now(),
                }),
            )?;
        }
        self.turn()
    }
    /// DPI changed — scale update + full damage, no recreate
    pub(crate) fn dpi_changed(&mut self, dpi: u32) -> UiResult {
        let scale = space::scale_from_dpi(dpi);
        self.peer_ctx.scale.set(scale);
        self.renderer.borrow_mut().set_dpi(scale);
        self.relayout()
    }
    /// Drain the reentrant queue — one owned message at a time through
    /// the SAME dispatch path, FIFO. Runs while the caller legitimately
    /// holds `&mut Backend` (after a dispatch unwound or in run()).
    /// Bounded: a pathological native callback storm cannot spin forever;
    /// leftover work surfaces as QueueOverflow instead of starvation.
    /// Drain the owned reentrant queue — bounded sub-passes whose policy
    /// decisions run against the COMPLETE queue, including items produced
    /// reentrantly by the dispatches themselves. Contract:
    ///
    /// - a delivery error is recorded AND draining continues — mandatory
    ///   teardown (a queued WM_NCDESTROY cleanup leg) can never be skipped
    ///   by an earlier failed input message;
    /// - teardown queued before OR DURING the drain is delivered by this
    ///   drain — after each pass, live-cell arrivals are merged and
    ///   re-evaluated before any shutdown/continuation decision;
    /// - after teardown, every remaining or late-arriving ordinary item
    ///   is disposed without dispatch — a dead window takes no work;
    /// - when the bounded ordinary budget runs out AND work remains, an
    ///   explicit WM_PUMP continuation is posted — `pump_queued` is set
    ///   only after the post SUCCEEDS (a failed post leaves it false so
    ///   the state is honest and a later caller can retry);
    /// - exactly-128 processed items is not itself an error — overflow is
    ///   reported only when the queue still holds work after the cap;
    /// - a push-side overflow flag is surfaced here as QueueOverflow.
    pub(crate) fn drain_reentrant(&mut self) -> UiResult {
        let hwnd = self.hwnd;
        let mut consumed = 0usize;
        let mut out = crate::platform::win32::window::DrainOutcome {
            tore: false,
            disposed: 0,
            first_err: None,
            remainder: false,
        };
        loop {
            // pull the whole backlog into a local pass; dispatching may
            // push MORE arrivals into the live cell — they merge back in
            // before the next policy decision below
            let mut q = std::mem::take(&mut *self.reentrant_queue.borrow_mut());
            let pass =
                crate::platform::win32::window::drain_queue(&mut q, &mut consumed, out.tore, |m| {
                    self.in_dispatch.set(true);
                    self.arrival_focus.set(m.arrival_focus);
                    let r = crate::platform::win32::window::wndproc::dispatch_owned(hwnd, m, self)
                        .map(|_| ());
                    self.arrival_focus.set(None);
                    self.in_dispatch.set(false);
                    r
                });
            out.tore |= pass.tore;
            out.disposed += pass.disposed;
            out.first_err = out.first_err.or(pass.first_err);
            {
                let mut cell = self.reentrant_queue.borrow_mut();
                q.extend(cell.drain(..));
                *cell = q;
            }
            if self.reentrant_queue.borrow().is_empty() {
                break;
            }
            if out.tore {
                // dead window: arrivals produced during/after teardown are
                // disposed, never dispatched — keep passing until empty
                continue;
            }
            let teardown_queued = self
                .reentrant_queue
                .borrow()
                .iter()
                .any(|m| crate::platform::win32::window::is_teardown(m.msg));
            if consumed >= crate::platform::win32::window::REENTRANT_DRAIN_MAX && !teardown_queued {
                // ordinary remainder past the budget — continuation owed
                out.remainder = true;
                break;
            }
            // otherwise: budget remains, or a teardown arrival is pending —
            // either way the next pass keeps working THIS drain
        }
        // remaining ordinary work -> checked continuation (shared wake
        // authority). Post-teardown there IS no remainder — the closed
        // window's queue was disposed, and a post on a dead hwnd is
        // declined by the closed check inside wake_pump anyway.
        if out.remainder {
            self.wake_pump()?;
        }
        if self.queue_overflowed.replace(false) {
            return Err(UiError::QueueOverflow);
        }
        if let Some(e) = out.first_err {
            return Err(e);
        }
        Ok(())
    }

    /// THE cross-thread mailbox wake authority — `SetEvent` on the owned
    /// `MailboxEvent` is the entire wake: it is independent of the
    /// receiver's message-queue capacity, SendMessage cooperation and
    /// timer synthesis, and the run loop's `MsgWaitForMultipleObjectsEx`
    /// is ALWAYS waiting on it — so an accepted envelope implies an
    /// already-owned durable signal (no accepted-but-unwakeable state).
    ///
    /// `SetEvent` failure means the event handle is dead — `Latched`
    /// reports the terminal outcome to the mailbox signal path, which
    /// owns the rollback/close contract for its context.
    fn mailbox_wake(ev: &MailboxEvent, closed: &AtomicBool) -> WakeOutcome {
        if closed.load(Ordering::SeqCst) {
            return WakeOutcome::Skipped;
        }
        if ev.signal() {
            WakeOutcome::Signaled
        } else {
            WakeOutcome::Latched
        }
    }

    /// test seam: exercise the wake authority against a caller-provided
    /// event (live event -> Signaled; dead handle -> Latched; closed
    /// window -> Skipped)
    #[cfg(test)]
    pub(crate) fn mailbox_wake_for_test(ev: &MailboxEvent, closed: &AtomicBool) -> WakeOutcome {
        Self::mailbox_wake(ev, closed)
    }

    /// One Backend entry under the native-entry ownership guard — the
    /// same authority the initial turn and every event-driven turn use:
    /// synchronous native callbacks that re-enter during the turn are
    /// QUEUED (in_dispatch), never form a second mutable Backend route,
    /// and the queue is drained before the entry completes.
    pub(crate) fn guarded_turn(&mut self) -> UiResult {
        self.in_dispatch.set(true);
        let r = (|| {
            self.turn()?;
            #[cfg(feature = "devtools")]
            self.service_devtools()?;
            Ok(())
        })();
        self.in_dispatch.set(false);
        r.and_then(|_| self.drain_reentrant())?;
        #[cfg(feature = "devtools")]
        self.complete_devtools_idle();
        Ok(())
    }

    /// fatal (typed) error from inside a WndProc — surface on next turn
    pub(crate) fn mark_fatal(&mut self, e: crate::UiError) {
        self.fatal = Some(e);
        // wake the pump so the error surfaces on this thread — no post on a
        // create-time null HWND
        if !self.hwnd.0.is_null() {
            unsafe {
                let _ = PostMessageW(Some(self.hwnd), WM_PUMP, WPARAM(0), LPARAM(0));
            }
        }
    }

    /// IME composition boundaries — composition gates conflict handling and
    /// pending-proposal resolution in the runtime.
    pub(crate) fn ime_start(&mut self) {
        // the REPLAY/captured owner starts composition — not whatever is
        // focused now; ownership is pinned until END arrives for it. An
        // explicitly absent captured owner stays ownerless FOREVER (Some
        // (None)) — delivery/end never fall back to a later-focused peer.
        let owner = self.focused_target();
        self.ime_owner.set(Some(owner));
        if let Some(id) = owner {
            self.rt.composition_start(id);
            if let Some(peer) = self.peer_for(id) {
                peer.borrow().set_composing(true);
            }
        }
    }
    /// The composition-boundary flag falls BEFORE the final drain — a
    /// commit emitted by the end must route as committed text, not be
    /// discarded as residual preedit. Proposals resolve last, against the
    /// TRUE committed revision.
    pub(crate) fn ime_end(&mut self) -> UiResult {
        // the composition belongs to the peer STARTCOMPOSITION reached —
        // never to a node that gained focus mid-composition; an ownerless
        // start ends ownerless (no peer acquires the bookkeeping)
        let owner = match self.ime_owner.take() {
            Some(captured) => captured,
            None => self.focused_target(),
        };
        if let Some(id) = owner {
            if let Some(peer) = self.peer_for(id) {
                peer.borrow().set_composing(false);
            }
            self.rt.composition_clear(id);
        }
        self.service_peer_events()?;
        if let Some(id) = owner {
            // ACKNOWLEDGE FIRST: a final committed EN_CHANGE drained while
            // composing was down may already be QUEUED — pump it into the
            // shared mirror BEFORE comparing, or reconcile would emit a
            // duplicate commit that splits native/shared revisions
            self.rt.pump()?;
            // RECONCILE only the remaining divergence — the ending commit
            // that arrived while `composing` was still up (suppressed as
            // preedit) never queued, so peer-vs-mirror divergence is the
            // ground truth; agreement = nothing to rebuild
            if let Some(peer) = self.peer_for(id) {
                self.reconcile_commit(&peer)?;
            }
            // acknowledge the reconciliation edit, THEN resolve proposals
            // against the true committed revision, then relatch
            self.rt.pump()?;
            self.rt.composition_end(id)?;
            if let Some(peer) = self.peer_for(id) {
                peer.borrow().finish_pending_relatch()?;
            }
        }
        self.turn()
    }

    /// Post-composition truth reconcile: if the peer's text differs from
    /// the runtime's committed mirror, the final commit was suppressed as
    /// preedit during composition — emit it now, before proposal verdicts.
    /// No-op when peer and mirror already agree (IME cancelled, or the
    /// commit already routed normally).
    fn reconcile_commit(&mut self, peer: &Rc<RefCell<WindowlessPeer>>) -> UiResult {
        let (text, node, base, binding) = {
            let p = peer.borrow();
            (p.text()?, p.node(), p.peer_rev(), p.binding())
        };
        let Some(binding) = binding else {
            return Ok(());
        };
        let Some((committed, mirror_base)) = self.rt.committed_editor(node) else {
            return Ok(());
        };
        if text == committed {
            return Ok(()); // already in agreement — nothing to commit
        }
        // the reconcile base must be the mirror's peer_revision — the
        // sequence of real commits, not the peer's possibly-advanced counter
        let result = crate::text::TextRevision::mint();
        let _ = base;
        self.rt.edit_event(
            node,
            TextEdit {
                text,
                base: mirror_base,
                result,
                origin: EditOrigin::NativePeer,
                binding,
            },
        )?;
        peer.borrow().record_commit(result);
        self.counters
            .ime_reconciled_commits
            .fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Backend teardown — UIA disconnects BEFORE peers/surfaces/model die.
    /// Deterministic: pending deferred deliveries drop, every native timer
    /// dies with the window, capture/caret release.
    pub(crate) fn shutdown(&mut self) {
        #[cfg(feature = "devtools")]
        self.devtools.take();
        if let Some(u) = self.uia.take() {
            u.close();
        }
        self.deferred_native.clear();
        {
            let mut pool = self.peer_ctx.timer_pool.lock().unwrap();
            for tid in pool.live_ids() {
                unsafe {
                    let _ = KillTimer(Some(self.hwnd), tid);
                }
            }
            pool.clear();
        }
        unsafe {
            let _ = KillTimer(Some(self.hwnd), window::DEADLINE_TIMER);
        }
        if self.native_capture.is_some() {
            unsafe {
                let _ = windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture();
            }
            self.native_capture = None;
        }
        self.renderer.borrow_mut().hide_tooltip();
        unsafe {
            let _ = DestroyCaret();
        }
        self.rt.shutdown();
    }

    /// Take the recorded fatal error — the message loop consumes it.
    pub(crate) fn take_fatal(&mut self) -> Option<crate::UiError> {
        self.fatal.take()
    }

    /// Commit/update boundary: a removed/disabled/hidden focus or capture
    /// owner is resolved BEFORE layout/paint see it. The native side
    /// follows — blur message, capture release, caret destroyed.
    pub(crate) fn sanitize_focus(&mut self) -> UiResult {
        let ok = |f: NodeId| {
            self.rt
                .arena
                .get(f)
                .is_some_and(|n| n.interactive() && n.visibility == Visibility::Visible)
        };
        // capture release: a captured node that died or went inert lets go
        if let Some(cap) = self.native_capture
            && !ok(cap)
        {
            unsafe {
                let _ = windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture();
            }
            self.native_capture = None;
        }
        if let Some(pr) = self.pressed
            && !ok(pr)
        {
            let st = InteractState {
                pressed: true,
                hot: self.hot == Some(pr),
                focus: self.focus == Some(pr),
            };
            self.pressed = None;
            self.mark_state_dirty(pr, st);
        }
        if let Some(h) = self.hot
            && !ok(h)
        {
            let st = InteractState {
                pressed: self.pressed == Some(h),
                hot: true,
                focus: self.focus == Some(h),
            };
            self.hot = None;
            self.mark_state_dirty(h, st);
        }
        let valid = self.focus.is_some_and(|f| ok(f));
        if !valid {
            if let Some(id) = self.focus.take() {
                // blur the native peer even when the node is dead — its
                // peer may still be mid-teardown; a queued blur is enough
                self.deliver_native(id, WM_KILLFOCUS, 0, 0)?;
                if self.rt.arena.is_live(id) {
                    self.push_input(id, NodeEvent::Focus(false))?;
                }
            }
            if let Some(u) = &self.uia {
                u.set_focus(None);
            }
            unsafe {
                let _ = DestroyCaret();
            }
            self.focus_step(false)?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// App::run — the real entry point
// ---------------------------------------------------------------------------

/// COM lifetime for the owning UI thread — initialized on entry, dropped on
/// exit even when `run` errors early.
struct OleGuard;
impl OleGuard {
    fn init() -> UiResult<OleGuard> {
        unsafe {
            windows::Win32::System::Ole::OleInitialize(None)
                .map_err(|e| UiError::Platform(format!("OleInitialize: {e}")))?;
        }
        Ok(OleGuard)
    }
}
impl Drop for OleGuard {
    fn drop(&mut self) {
        unsafe {
            windows::Win32::System::Ole::OleUninitialize();
        }
    }
}

/// The blocking Win32 run loop: window + Runtime + mailbox wake seam.
pub(crate) fn run<S, M, U, V>(app: App<S, M, U, V>) -> UiResult
where
    S: 'static,
    M: 'static,
    U: Fn(&mut S, M, &mut UpdateCtx<'_, M>) + 'static,
    V: Fn(&S, &mut crate::Ui<'_, '_, M>) + 'static,
{
    let _ole = OleGuard::init()?;
    // PerMonitorV2 before any window exists — GetDpiForWindow reports the
    // real monitor DPI so DIP layout/render scale at 100/125/150/200
    unsafe {
        let _ = windows::Win32::UI::HiDpi::SetProcessDpiAwarenessContext(
            windows::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
        );
    }
    let msft = Msftedit::load()?;
    let title = app.title.clone();

    // actual OS appearance + effective reduced motion — BEFORE first mount
    let appearance = os_appearance();
    let reduced = effective_reduced(&Theme::light(), &appearance);

    let sink = Arc::new(Mutex::new(Vec::new()));
    let peer_ctx = Rc::new(PeerCtx {
        lib: msft.clone(),
        hwnd: std::cell::Cell::new(HWND::default()),
        scale: std::cell::Cell::new(space::ScaleFactor::ONE),
        hot: std::cell::Cell::new(None),
        pressed: std::cell::Cell::new(None),
        focus: std::cell::Cell::new(None),
        sink: sink.clone(),
        registry: Arc::new(Mutex::new(HashMap::new())),
        timer_pool: Arc::new(Mutex::new(crate::platform::win32::text::TimerPool::new())),
        next_id: AtomicU64::new(1),
        sys_resolver: std::cell::Cell::new(sys_color),
        colors: std::cell::RefCell::new((appearance, Theme::dark())),
    });
    let factory_ctx = peer_ctx.clone();
    let factory = factory_ctx.make_factory();
    // the theme keeps the AUTHORED reduced-motion policy — System stays
    // System so an OS change can re-resolve; the resolver probes SPI_*
    let theme = Theme::light();
    let _ = reduced; // initial probe consumed via motion_resolver below
    let mut rt = crate::runtime::Runtime::new(
        app.state,
        app.update,
        app.view,
        app.executor,
        Box::new(factory),
        theme.clone(),
        appearance,
        app.mailbox,
    );
    rt.set_forced_resolver(Box::new(sys_color));
    rt.set_motion_resolver(Box::new(|| unsafe {
        let mut v = windows_core::BOOL::default();
        let _ = windows::Win32::UI::WindowsAndMessaging::SystemParametersInfoW(
            windows::Win32::UI::WindowsAndMessaging::SPI_GETCLIENTAREAANIMATION,
            0,
            Some(&mut v as *mut windows_core::BOOL as *mut std::ffi::c_void),
            windows::Win32::UI::WindowsAndMessaging::SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
        !v.as_bool()
    }));
    rt.refresh_reduced();

    let mut backend = Box::new(Backend {
        #[cfg(feature = "devtools")]
        devtools: None,
        #[cfg(feature = "devtools")]
        paint_epoch: std::cell::Cell::new(0),
        rt,
        hwnd: HWND::default(),
        peer_ctx,
        renderer: RefCell::new(Renderer::new()?),
        rects: HashMap::new(),
        order: Vec::new(),
        focus: None,
        hot: None,
        pressed: None,
        native_capture: None,
        mouse: Point { x: 0.0, y: 0.0 },
        deadline_timer: None,
        tooltip_for: None,
        uia: None,
        counters: Arc::new(PerfCounters::new()),
        closed: Arc::new(AtomicBool::new(false)),
        fatal: None,
        in_turn: std::cell::Cell::new(false),
        reentrant_queue: std::cell::RefCell::new(std::collections::VecDeque::new()),
        arrival_focus: std::cell::Cell::new(None),
        ime_owner: std::cell::Cell::new(None),
        queue_overflowed: std::cell::Cell::new(false),
        pending_lead_surrogate: std::cell::Cell::new(None),
        in_dispatch: std::cell::Cell::new(false),
        pump_queued: std::cell::Cell::new(false),
        deferred_native: std::collections::VecDeque::new(),
        state_dirty: std::cell::Cell::new(0),
        damage: std::cell::Cell::new(None),
        committed_ink: std::cell::RefCell::new(HashMap::new()),
        applied_palette: std::cell::RefCell::new(None),
    });
    backend.rt.theme = theme;

    // THE mailbox wake authority — ONE owned auto-reset event, created
    // BEFORE the HWND takes the live Backend pointer: a create failure
    // returns before any native window exists, so no HWND can ever
    // dangle a pointer to a Backend being unwound
    let wake_event = init_wake_event(MailboxEvent::create())?;

    // the window carries a stable pointer to the route object — its lifetime
    // is the loop's lifetime; WM_NCDESTROY clears it before `backend` drops
    let route = backend.as_mut() as *mut Backend<S, M, U, V>;
    let hwnd = match window::create(&title, route) {
        Ok(h) => h,
        Err(e) => {
            // window died before it could hold the route — nothing to detach
            return Err(e);
        }
    };
    backend.hwnd = hwnd;
    backend.peer_ctx.hwnd.set(hwnd);
    // peers that mounted inside CreateWindowExW latched an invalid hwnd —
    // repair them once the window exists (IME/caret/coord conversion and
    // msftedit's UIA provider both need the host window handle)
    for p in backend
        .peer_ctx
        .registry
        .lock()
        .unwrap()
        .values()
        .filter_map(|w| w.upgrade())
    {
        p.borrow().set_hwnd(hwnd);
    }
    let scale = unsafe { space::scale_from_dpi(windows::Win32::UI::HiDpi::GetDpiForWindow(hwnd)) };
    backend
        .peer_ctx
        .scale
        .set(space::ScaleFactor(scale.0.max(0.5)));

    // producers signal the owned event via the ONE wake authority. The
    // callback Arc holds the event alive for every admitted signaler —
    // mailbox.close() detaches the stored callback first, and any
    // in-flight clone still signals a VALID handle; the HANDLE closes
    // only when the last Arc (loop + callback + in-flight) drops.
    let closed = backend.closed.clone();
    let ev = wake_event.clone();
    if backend
        .rt
        .mailbox
        .install_wake(Arc::new(move || {
            matches!(
                Backend::<S, M, U, V>::mailbox_wake(&ev, &closed),
                WakeOutcome::Signaled | WakeOutcome::Skipped
            )
        }))
        .is_err()
    {
        return bail(
            &mut backend,
            UiError::Platform("mailbox wake install failed".into()),
        );
    }

    /// Every failure path after window creation destroys the HWND first —
    /// WM_NCDESTROY detaches the route pointer so the live window can never
    /// call into a dropped Backend.
    fn bail<S, M, U, V>(backend: &mut Backend<S, M, U, V>, e: crate::UiError) -> UiResult
    where
        M: 'static,
        U: Fn(&mut S, M, &mut UpdateCtx<'_, M>),
        V: Fn(&S, &mut crate::Ui<'_, '_, M>),
    {
        if !backend.hwnd.0.is_null() {
            // DestroyWindow synchronously dispatches WM_NCDESTROY — the
            // SAME native-entry ownership guard wraps the call, so a
            // reentrant trampoline sees in_dispatch and takes the deferred
            // path; the queued teardown is then drained explicitly.
            backend.in_dispatch.set(true);
            unsafe {
                let _ = DestroyWindow(backend.hwnd);
            }
            backend.in_dispatch.set(false);
            let _ = backend.drain_reentrant();
            // belt: teardown must complete even if the drain errored
            backend.shutdown();
            backend.hwnd = HWND::default();
        }
        Err(e)
    }

    // the initial turn is a native-entry backend operation — peers mount
    // inside it and their host callbacks can re-enter the WndProc; run it
    // under the same ownership guard as real dispatch
    // initial view/layout/paint — same native-entry ownership authority
    // as every later event-driven turn
    if let Err(e) = backend.guarded_turn() {
        return bail(&mut backend, e);
    }

    #[cfg(feature = "devtools")]
    {
        backend.devtools = match crate::devtools::client::Server::start(
            backend.rt.mailbox.clone(),
            backend.hwnd.0 as usize,
        ) {
            Ok(s) => s,
            Err(e) => return bail(&mut backend, UiError::Platform(e.to_string())),
        };
    }

    // blocking wait on BOTH authorities — the mailbox wake event AND the
    // message queue. Idle sleeps with zero polling: the wait returns on
    // the event (mailbox work accepted), on queued input, or on WM_QUIT.
    let handles = [wake_event.0];
    unsafe {
        let mut msg = MSG::default();
        'run: loop {
            // block until: event signaled (WAIT_OBJECT_0) or unseen input
            // arrives (WAIT_OBJECT_0 + handle_count). MWMO_INPUTAVAILABLE
            // already-seen input is fine because the PeekMessage sweep
            // drains ALL of it. Every result class is explicit —
            // WAIT_FAILED/undocumented -> typed terminal teardown.
            let wait = MsgWaitForMultipleObjectsEx(
                Some(&handles),
                INFINITE,
                QS_ALLINPUT,
                MWMO_INPUTAVAILABLE,
            );
            match classify_wait(wait, handles.len()) {
                Ok(WaitClass::Event) => {
                    // mailbox signaled — one guarded bounded turn drains
                    // a sweep; reentrant native work queues, never forms
                    // a second entry; backlog continuation re-signals below
                    if let Err(e) = backend.guarded_turn() {
                        return bail(&mut backend, e);
                    }
                }
                Ok(WaitClass::Input) => {}
                Err(e) => return bail(&mut backend, e),
            }
            // drain every queued message (also covers WaitClass::Input)
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                if msg.message == WM_QUIT {
                    break 'run;
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
                // a WndProc-level failure is recorded, not dropped — the
                // next safe point surfaces it and tears the window down
                if let Some(e) = backend.take_fatal() {
                    return bail(&mut backend, e);
                }
            }
            // bounded-drain continuation — a turn can leave mailbox work
            // when the sweep bound hit mid-drain or dispatch enqueued
            // more; re-signal through the SAME checked authority — a
            // failed signal applies the terminal contract inside poke_ui
            if backend.rt.mailbox.queue_len() > 0 && !backend.rt.mailbox.poke_ui() {
                return bail(
                    &mut backend,
                    UiError::Platform("mailbox wake signal failed".into()),
                );
            }
        }
    }
    // a failed cross-thread wake that never reached a turn still
    // surfaces — the loop is ending, so report the broken progress
    // guarantee rather than silently exiting over stranded work
    if backend.rt.mailbox.take_wake_failure() {
        return Err(UiError::Platform("mailbox wake signal failed".into()));
    }
    // evidence hook (RUI_PERF=<path>): dump the frame counters on clean
    // shutdown — proves event-driven idle + native-drive accounting
    if let Ok(path) = std::env::var("RUI_PERF") {
        let c = &backend.counters;
        let line = format!(
            "uptime_ms={} requested_redraws={} animation_callbacks={} animation_timer_fires={} caret_redraws={} native_timer_fires={}\n",
            c.start.elapsed().as_millis(),
            c.requested_redraws.load(Ordering::Relaxed),
            c.animation_callbacks.load(Ordering::Relaxed),
            c.animation_timer_fires.load(Ordering::Relaxed),
            c.native_caret_redraws.load(Ordering::Relaxed),
            c.native_timer_fires.load(Ordering::Relaxed),
        );
        let _ = std::fs::write(path, line);
    }
    Ok(())
}

/// OS appearance: HKCU Themes\Personalize\AppsUseLightTheme (DWORD).
fn os_appearance() -> Appearance {
    use windows::Win32::System::Registry::*;
    use windows::Win32::UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW};
    use windows::Win32::UI::WindowsAndMessaging::{SPI_GETHIGHCONTRAST, SystemParametersInfoW};
    let mut dark = false;
    let mut forced = false;
    unsafe {
        let mut key = HKEY::default();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            None,
            KEY_READ,
            &mut key,
        )
        .is_ok()
        {
            let mut val = 0u32;
            let mut len = 4u32;
            if RegGetValueW(
                key,
                PCWSTR::null(),
                w!("AppsUseLightTheme"),
                RRF_RT_REG_DWORD,
                None,
                Some(&mut val as *mut u32 as *mut c_void),
                Some(&mut len),
            )
            .is_ok()
            {
                dark = val == 0;
            }
            let _ = RegCloseKey(key);
        }
        let mut hc = HIGHCONTRASTW::default();
        hc.cbSize = std::mem::size_of::<HIGHCONTRASTW>() as u32;
        if SystemParametersInfoW(
            SPI_GETHIGHCONTRAST,
            0,
            Some(&mut hc as *mut _ as *mut c_void),
            Default::default(),
        )
        .is_ok()
        {
            forced = hc.dwFlags.contains(HCF_HIGHCONTRASTON);
        }
    }
    Appearance {
        dark,
        forced_colors: forced,
    }
}

/// GetSysColor-backed forced-colors resolver — the OS palette for the
/// shared SystemColor slots. Installed on the runtime so the classifier
/// and renderer resolve the same concrete colors.
pub(crate) fn sys_color(sc: crate::style::SystemColor) -> [f32; 4] {
    use windows::Win32::Graphics::Gdi::{GetSysColor, SYS_COLOR_INDEX};
    let idx = match sc {
        crate::style::SystemColor::Window => SYS_COLOR_INDEX(5), // COLOR_WINDOW
        crate::style::SystemColor::WindowText => SYS_COLOR_INDEX(8), // COLOR_WINDOWTEXT
        crate::style::SystemColor::Highlight => SYS_COLOR_INDEX(13), // COLOR_HIGHLIGHT
        crate::style::SystemColor::HighlightText => SYS_COLOR_INDEX(14), // COLOR_HIGHLIGHTTEXT
        crate::style::SystemColor::GrayText => SYS_COLOR_INDEX(17), // COLOR_GRAYTEXT
        crate::style::SystemColor::InactiveBorder => SYS_COLOR_INDEX(11), // COLOR_INACTIVEBORDER
        crate::style::SystemColor::HotTrack => SYS_COLOR_INDEX(26), // COLOR_HOTLIGHT
    };
    let rgb = unsafe { GetSysColor(idx) };
    [
        (rgb & 0xff) as f32 / 255.0,
        ((rgb >> 8) & 0xff) as f32 / 255.0,
        ((rgb >> 16) & 0xff) as f32 / 255.0,
        1.0,
    ]
}

/// Effective reduced motion: Reduce=true, NoPreference=false, System reads
/// SPI_GETCLIENTAREAANIMATION.
fn effective_reduced(theme: &Theme, _appearance: &Appearance) -> bool {
    match theme.reduced_motion {
        ReducedMotion::Reduce => true,
        ReducedMotion::NoPreference => false,
        ReducedMotion::System => unsafe {
            let mut v = windows_core::BOOL::default();
            let _ = windows::Win32::UI::WindowsAndMessaging::SystemParametersInfoW(
                SPI_GETCLIENTAREAANIMATION,
                0,
                Some(&mut v as *mut BOOL as *mut c_void),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            );
            !v.as_bool()
        },
    }
}
