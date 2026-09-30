//! Windows-only native backend: a real Win32 message loop + windowless
//! RichEdit text peers + a software-capable Direct2D/DirectWrite presenter.
//!
//! Structure: `window` owns HWND/WndProc plumbing, `text` owns the msftedit
//! host/peer, `layout` computes DIP rects, `render` paints the frame,
//! `uia` exposes the accessibility tree. The retained core stays generic —
//! this module only consumes its pub(crate) seams.

#![cfg(windows)]

mod layout;
mod render;
mod text;
mod uia;
mod window;

pub(crate) use layout::{DipRect, LayoutCache};
pub(crate) use render::Renderer;
pub(crate) use text::{HostEvent, Msftedit, PeerConfig, WindowlessPeer};

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::rc::Rc;
use std::rc::Weak as RcWeak;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
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
    pub scale: std::cell::Cell<f32>,
    pub sink: Arc<Mutex<Vec<NativeSinkItem>>>,
    pub registry: Arc<Mutex<HashMap<u32, RcWeak<RefCell<WindowlessPeer>>>>>,
    next_id: AtomicU64,
    /// live theme colors for peer creation (updated on theme switch)
    pub colors: std::cell::RefCell<(Appearance, Theme)>,
}

impl PeerCtx {
    /// The `peer_factory` closure passed into `Runtime::new`. The ctx is
    /// UI-thread-only (peers are windowless COM objects) — the registry is
    /// a routing index of Weak handles, not a second owner.
    fn make_factory(
        self: &Rc<PeerCtx>,
    ) -> impl Fn(bool) -> UiResult<Box<dyn crate::node::TextPeer>> + 'static {
        let ctx = self.clone();
        move |multiline| {
            let id = ctx.next_id.fetch_add(1, Ordering::SeqCst);
            let (appearance, theme) = ctx.colors.borrow().clone();
            let (fg, sel_bg, sel_fg) = palette(&theme, &appearance);
            let cfg = PeerConfig {
                multiline,
                read_only: false,
                face: "Segoe UI".into(),
                size_twips: 280, // 14pt = 280 twips
                fg,
                sel_bg,
                sel_fg,
            };
            let peer = WindowlessPeer::create(
                id,
                &ctx.lib,
                ctx.hwnd.get(),
                ctx.scale.get(),
                &cfg,
                ctx.sink.clone(),
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
pub(crate) fn palette(_theme: &Theme, appearance: &Appearance) -> ([f32; 4], [f32; 4], [f32; 4]) {
    if appearance.dark {
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

/// Waker seam: mailbox wake posts to the window — no polling.
pub(crate) const WM_PUMP: u32 = WM_APP + 7;
/// generation-fenced UIA actions routed through the UI thread
pub(crate) const WM_UIA_PRESS: u32 = WM_APP + 8;
pub(crate) const WM_UIA_FOCUS: u32 = WM_APP + 9;

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
    pub rects: HashMap<NodeId, DipRect>,
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
    /// peer-requested native timers: win32 id -> (slot, richedit id)
    pub native_timers: HashMap<usize, (u32, u32)>,
    next_timer_id: usize,
    /// currently-shown tooltip node (overlay window managed by renderer)
    pub tooltip_for: Option<NodeId>,
    /// UIA root — built on the first WM_GETOBJECT
    pub uia: Option<uia::UiaRoot>,
    pub counters: Arc<PerfCounters>,
    /// ended by WM_DESTROY — the pump exits
    pub closed: Arc<AtomicBool>,
    /// appearance snapshot the last review ran under
    last_appearance: Appearance,
    /// last fatal backend error surfaced from a WndProc
    fatal: Option<crate::UiError>,
    /// `turn()` is executing — native reentrancy (e.g. `SetFocus` inside
    /// `OnTxUIActivate`) must queue another pump instead of recursing
    in_turn: std::cell::Cell<bool>,
    /// one coalesced `WM_PUMP` outstanding for a deferred turn
    pump_queued: std::cell::Cell<bool>,
    /// native messages deferred while their target peer was borrowed —
    /// delivered at the top of the next `service_peer_events`
    deferred_native: Vec<(NodeId, u32, usize, isize)>,
}

impl<S, M, U, V> Backend<S, M, U, V>
where
    M: 'static,
    U: Fn(&mut S, M, &mut UpdateCtx<'_, M>),
    V: Fn(&S, &mut crate::Ui<'_, '_, M>),
{
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
            if !self.pump_queued.replace(true)
                && !self.hwnd.0.is_null()
                && !self.closed.load(Ordering::SeqCst)
            {
                unsafe {
                    let _ = PostMessageW(Some(self.hwnd), WM_PUMP, WPARAM(0), LPARAM(0));
                }
            }
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
        // scheduler due work: frames -> events, chrome -> render side
        let chrome = self.rt.pump_sched(std::time::Instant::now())?;
        for ev in &chrome {
            self.apply_chrome(ev);
        }
        // dirty classification -> layout / paint work
        let dirty = self.rt.drain_dirty();
        let mut needs_layout = !self.rects.is_empty() && self.order.is_empty();
        let mut needs_paint = !chrome.is_empty();
        for (id, d) in dirty {
            if d & crate::runtime::Runtime::<S, M, U, V>::DIRTY_LAYOUT != 0 {
                needs_layout = true;
            }
            if d & crate::runtime::Runtime::<S, M, U, V>::DIRTY_PAINT != 0 {
                needs_paint = true;
            }
            let _ = id;
        }
        if self.rects.is_empty() {
            needs_layout = true;
        }
        if needs_layout || updated {
            self.relayout()?;
            needs_paint = true;
        }
        if needs_paint {
            self.paint()?;
        }
        self.service_peer_events()?;
        self.rearm_deadline();
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

    /// Send a raw window message to a node's peer, deferring while the
    /// peer's `RefCell` is held (msftedit calls `TxSetFocus` mid-`send`,
    /// which re-enters `WM_SETFOCUS` synchronously — a second `borrow()`
    /// would panic). Deferred messages deliver at the top of the next
    /// `service_peer_events`, still generation-checked.
    pub(crate) fn send_native(
        &mut self,
        id: NodeId,
        msg: u32,
        wparam: usize,
        lparam: isize,
    ) {
        // NOTE: no peer_for() — its generation filter borrows the cell and
        // would panic during a live borrow; the deferred delivery runs the
        // slot lookup then instead
        let Some(peer) = self
            .peer_ctx
            .registry
            .lock()
            .unwrap()
            .get(&id.slot)
            .and_then(|w| w.upgrade())
        else {
            return;
        };
        match peer.try_borrow() {
            Ok(p) if p.node() == id => {
                // msftedit runs host callbacks inside the send — SetFocus,
                // timers, invalidation all re-enter the WndProc
                // synchronously; in_turn keeps a nested turn() deferred
                // until this send unwinds (restore, not clear — the send
                // may itself run inside an outer turn)
                let was = self.in_turn.replace(true);
                p.send(msg, wparam, lparam);
                self.in_turn.set(was);
            }
            Ok(_) => {}
            Err(_) => {
                if self.deferred_native.len() < crate::event::EVENT_QUEUE_CAP {
                    self.deferred_native.push((id, msg, wparam, lparam));
                }
            }
        }
    }

    /// Raw window message to the focused peer (IME, focus, wheel routing).
    pub(crate) fn send_focused(&mut self, msg: u32, wparam: usize, lparam: isize) {
        let Some(id) = self.focus else { return };
        self.send_native(id, msg, wparam, lparam);
    }

    /// Drain every live peer's host-event queue: caret/capture/timer/change
    /// notifications msftedit posted while a call ran.
    pub(crate) fn service_peer_events(&mut self) -> UiResult {
        // deferred focus/native sends first — the borrow that blocked them
        // is unwound by the time a turn calls this
        let pending: Vec<(NodeId, u32, usize, isize)> =
            std::mem::take(&mut self.deferred_native);
        for (id, msg, wp, lp) in pending {
            if let Some(peer) = self
                .peer_ctx
                .registry
                .lock()
                .unwrap()
                .get(&id.slot)
                .and_then(|w| w.upgrade())
                .filter(|p| p.borrow().node() == id)
            {
                peer.borrow().send(msg, wp, lp);
            }
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
                    HostEvent::SetTimer(id, ms) => {
                        self.arm_native_timer(slot, id, ms);
                    }
                    HostEvent::KillTimer(id) => {
                        self.disarm_native_timer(slot, id);
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
        Ok(())
    }

    fn invalidate_slot(&self, slot: u32) {
        if let Some(r) = self
            .rects
            .keys()
            .find(|id| id.slot == slot)
            .and_then(|id| self.rects.get(id))
        {
            let (x, y, w, h) = (r.x, r.y, r.w, r.h);
            let s = self.peer_ctx.scale.get();
            let rc = RECT {
                left: (x * s) as i32,
                top: (y * s) as i32,
                right: ((x + w) * s) as i32 + 1,
                bottom: ((y + h) * s) as i32 + 1,
            };
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
    /// transient preedit never reaches `on_edit`.
    fn native_commit(&mut self, slot: u32, peer: &Rc<RefCell<WindowlessPeer>>) -> UiResult {
        let p = peer.borrow();
        let text = p.text()?;
        let (anchor, focus) = p.selection_utf16().unwrap_or((0, 0));
        let _ = (anchor, focus);
        let node = p.node();
        let base = p.peer_rev();
        let result = crate::text::TextRevision::mint();
        if let Some(binding) = p.binding() {
            // UTF-8 positions of the selection at THIS commit — indices stay
            // consistent because we never normalize CR/LF
            let sel = TextSelection {
                revision: result,
                anchor: crate::text::utf16_index_to_utf8(&text, anchor).unwrap_or(text.len()),
                focus: crate::text::utf16_index_to_utf8(&text, focus).unwrap_or(text.len()),
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
            p.record_commit(result);
        }
        let _ = slot;
        Ok(())
    }

    fn native_selection(&mut self, _slot: u32, peer: &Rc<RefCell<WindowlessPeer>>) -> UiResult {
        let p = peer.borrow();
        let text = p.text()?;
        let (anchor, focus) = p.selection_utf16()?;
        let sel = TextSelection {
            revision: p.peer_rev(),
            anchor: crate::text::utf16_index_to_utf8(&text, anchor).unwrap_or(text.len()),
            focus: crate::text::utf16_index_to_utf8(&text, focus).unwrap_or(text.len()),
        };
        self.rt.selection_event(p.node(), sel)?;
        Ok(())
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
        let Some(r) = self.rects.get(&node).copied() else {
            return;
        };
        let s = self.peer_ctx.scale.get();
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::*;
            let _ = CreateCaret(
                self.hwnd,
                None,
                (size.cx as f32 * s) as i32,
                (size.cy as f32 * s) as i32,
            );
            let _ = SetCaretPos(
                ((r.x + pos.x as f32) * s) as i32,
                ((r.y + pos.y as f32) * s) as i32,
            );
            if shown {
                let _ = ShowCaret(Some(self.hwnd));
            }
        }
    }

    // ----- native timers ----------------------------------------------------

    fn arm_native_timer(&mut self, slot: u32, id: u32, ms: u32) {
        let tid = {
            self.next_timer_id += 1;
            self.next_timer_id
        };
        unsafe {
            SetTimer(Some(self.hwnd), tid, ms, None);
        }
        self.native_timers.insert(tid, (slot, id));
    }

    fn disarm_native_timer(&mut self, slot: u32, id: u32) {
        let dead: Vec<usize> = self
            .native_timers
            .iter()
            .filter(|(_, (s, i))| *s == slot && *i == id)
            .map(|(t, _)| *t)
            .collect::<Vec<usize>>();
        for t in dead {
            unsafe {
                let _ = KillTimer(Some(self.hwnd), t);
            }
            self.native_timers.remove(&t);
        }
    }

    /// `WM_TIMER` on a peer-owned id: forward into the peer, drain the host
    /// events the callback produced.
    pub(crate) fn native_timer_fire(&mut self, tid: usize) -> UiResult {
        let Some((slot, nid)) = self.native_timers.get(&tid).copied() else {
            return Ok(());
        };
        // one-shot per richedit request — kill before processing, rearm
        // inside if it asks again
        unsafe {
            let _ = KillTimer(Some(self.hwnd), tid);
        }
        self.native_timers.remove(&tid);
        self.counters
            .native_timer_fires
            .fetch_add(1, Ordering::Relaxed);
        if let Some(peer) = self
            .peer_ctx
            .registry
            .lock()
            .unwrap()
            .get(&slot)
            .and_then(|w| w.upgrade())
        {
            peer.borrow().send(WM_TIMER, nid as usize, 0);
        }
        self.service_peer_events()?;
        Ok(())
    }

    /// The single framework deadline timer — armed to the sched's earliest
    /// deadline; killed when there's no demand (no persistent polling).
    pub(crate) fn rearm_deadline(&mut self) {
        const DEADLINE_ID: usize = 1;
        let next = self.rt.next_deadline();
        match (self.deadline_timer, next) {
            (Some(_), Some(n)) => {
                let due = n
                    .saturating_duration_since(std::time::Instant::now())
                    .as_millis() as u32;
                unsafe {
                    let _ = KillTimer(Some(self.hwnd), DEADLINE_ID);
                    SetTimer(Some(self.hwnd), DEADLINE_ID, due.max(1), None);
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
                unsafe {
                    SetTimer(Some(self.hwnd), DEADLINE_ID, due.max(1), None);
                }
                self.deadline_timer = Some(n);
            }
            (None, None) => {}
        }
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
    pub(crate) fn relayout(&mut self) -> UiResult {
        let mut layout = LayoutCache::new(self.peer_ctx.scale.get());
        let (rects, order) = layout.run(&mut self.rt, &self.peer_ctx)?;
        // prune stale ids — backend cache holds live NodeIds only
        self.rects = rects;
        self.order = order;
        Ok(())
    }

    /// Present the frame — D2D target, axis-aligned clips, chrome paint.
    pub(crate) fn paint(&mut self) -> UiResult {
        if self.hwnd.0.is_null() {
            return Ok(());
        }
        self.renderer.borrow_mut().draw(&*self)?;
        Ok(())
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

    /// Hit-test a DIP point: topmost visible interactive node wins.
    pub(crate) fn hit_test(&self, p: Point) -> Option<NodeId> {
        for &id in self.order.iter().rev() {
            let Some(n) = self.rt.arena.get(id) else {
                continue;
            };
            if n.visibility != Visibility::Visible {
                continue;
            }
            let Some(r) = self.rects.get(&id) else {
                continue;
            };
            if r.contains(p) {
                match &n.data {
                    NodeData::Button { .. } | NodeData::Editor { .. } | NodeData::Custom { .. } => {
                        return Some(id);
                    }
                    _ => {}
                }
            }
        }
        None
    }

    /// Route a pointer event — editors get the raw Win32 message; semantic
    /// nodes get `NodeEvent` phases.
    pub(crate) fn pointer(
        &mut self,
        phase: crate::node::PointerPhase,
        pos: Point,
        button: Option<PointerButton>,
    ) -> UiResult {
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
                let _ = self.push_input(
                    old,
                    NodeEvent::Pointer(
                        PointerEvent {
                            position: pos,
                            button,
                            modifiers: Modifiers::default(),
                        },
                        crate::node::PointerPhase::Leave,
                    ),
                );
            }
            if let Some(id) = hit
                && !self.peer_for(id).is_some()
            {
                let _ = self.push_input(
                    id,
                    NodeEvent::Pointer(
                        PointerEvent {
                            position: pos,
                            button,
                            modifiers: Modifiers::default(),
                        },
                        crate::node::PointerPhase::Enter,
                    ),
                );
            }
            self.hot = hit;
        }
        if let Some(id) = hit {
            // native peer? forward the raw message + focus on down
            if self.peer_for(id).is_some() {
                let r = self.rects.get(&id).copied().unwrap_or_default();
                let local = Point {
                    x: pos.x - r.x,
                    y: pos.y - r.y,
                };
                let (msg, wp) = match (phase, button) {
                    (crate::node::PointerPhase::Down, Some(PointerButton::Primary)) => {
                        self.set_focus(Some(id));
                        (WM_LBUTTONDOWN, 0usize)
                    }
                    (crate::node::PointerPhase::Up, Some(PointerButton::Primary)) => {
                        (WM_LBUTTONUP, 0usize)
                    }
                    _ => (WM_MOUSEMOVE, 0usize),
                };
                let lp = ((local.y as isize) << 16) | (local.x as isize & 0xffff);
                self.send_native(id, msg, wp, lp);
                self.service_peer_events()?;
                return self.turn();
            }
            if phase == crate::node::PointerPhase::Down && button == Some(PointerButton::Primary) {
                self.set_focus(Some(id));
            }
            // semantic node events
            let ev = match phase {
                crate::node::PointerPhase::Down => {
                    self.pressed = Some(id);
                    NodeEvent::Pointer(
                        PointerEvent {
                            position: pos,
                            button,
                            modifiers: Modifiers::default(),
                        },
                        crate::node::PointerPhase::Down,
                    )
                }
                crate::node::PointerPhase::Up => {
                    let was_pressed = self.pressed == Some(id);
                    self.pressed = None;
                    // press = down+up on the same node
                    if was_pressed {
                        let _ = self.push_input(id, NodeEvent::Press);
                        return self.turn();
                    }
                    NodeEvent::Pointer(
                        PointerEvent {
                            position: pos,
                            button,
                            modifiers: Modifiers::default(),
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
            let _ = self.push_input(id, ev);
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
    pub(crate) fn peer_for(&self, node: NodeId) -> Option<Rc<RefCell<WindowlessPeer>>> {
        self.peer_ctx
            .registry
            .lock()
            .unwrap()
            .get(&node.slot)
            .and_then(|w| w.upgrade())
            .filter(|p| p.borrow().node() == node)
    }

    /// Keyboard: focused editor gets raw messages; everything else is a
    /// semantic `NodeEvent::Key` (buttons submit on Space/Enter).
    pub(crate) fn key(&mut self, key: u32) -> UiResult {
        let down = true;
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
        if key == VK_TAB.0 as u32 && down {
            self.focus_step(unsafe { GetKeyState(VK_SHIFT.0 as i32) } < 0);
            return self.turn();
        }
        // focused editor gets the raw key (native editing, IME)
        if let Some(focus) = self.focus
            && self.peer_for(focus).is_some()
            && down
        {
            let submit = self.check_submit(focus, ev);
            self.send_native(focus, WM_KEYDOWN, key as usize, 0);
            self.service_peer_events()?;
            if let Some(sub) = submit {
                self.push_input(focus, sub)?;
            }
            return self.turn();
        }
        if !down {
            return Ok(());
        }
        // semantic focus: Space/Enter on a button is Press
        if let Some(focus) = self.focus {
            match ev {
                Key::Space | Key::Enter => {
                    let _ = self.push_input(focus, NodeEvent::Press);
                    return self.turn();
                }
                _ => {
                    let _ = self.push_input(
                        focus,
                        NodeEvent::Key(KeyEvent {
                            key: ev,
                            modifiers: Modifiers::default(),
                        }),
                    );
                    return self.turn();
                }
            }
        }
        Ok(())
    }

    /// Editor Enter semantics per SubmitPolicy — IME-committed Enter never
    /// reaches here (it's consumed inside the composition).
    fn check_submit(&self, node: NodeId, key: Key) -> Option<NodeEvent> {
        if key != Key::Enter {
            return None;
        }
        let n = self.rt.arena.get(node)?;
        if let NodeData::Editor {
            submit, multiline, ..
        } = &n.data
        {
            match (submit, multiline) {
                (SubmitPolicy::Enter, false) => Some(NodeEvent::Submit),
                // multiline: Shift+Enter edits, plain Enter submits
                (SubmitPolicy::Enter, true) => {
                    let shift = unsafe {
                        windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(VK_SHIFT.0 as i32)
                    } < 0;
                    (!shift).then_some(NodeEvent::Submit)
                }
                (SubmitPolicy::ModifierEnter, _) => {
                    let ctrl = unsafe {
                        windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(
                            VK_CONTROL.0 as i32,
                        )
                    } < 0;
                    ctrl.then_some(NodeEvent::Submit)
                }
                _ => None,
            }
        } else {
            None
        }
    }

    /// Focus next/previous eligible node (Tab/Shift-Tab traversal).
    pub(crate) fn focus_step(&mut self, back: bool) {
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
                    && matches!(n.data, NodeData::Button { .. } | NodeData::Editor { .. })
            })
            .collect();
        if eligible.is_empty() {
            return;
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
        self.set_focus(Some(eligible[next]));
    }

    /// Focus a node — blur the old (still routed even when it just hid),
    /// focus the new. Peers get the raw focus messages so caret/edit state
    /// tracks the framework focus owner.
    pub(crate) fn set_focus(&mut self, node: Option<NodeId>) {
        if self.focus == node {
            return;
        }
        if let Some(old) = self.focus {
            if self.rt.arena.is_live(old) {
                let _ = self.push_input(old, NodeEvent::Focus(false));
            }
            self.send_native(old, WM_KILLFOCUS, 0, 0);
        }
        self.focus = node;
        if let Some(id) = node {
            let _ = self.push_input(id, NodeEvent::Focus(true));
            self.send_native(id, WM_SETFOCUS, 0, 0);
        }
    }

    /// WM_MOUSEMOVE: hot tracking (Enter/Leave) + raw move to hovered peer.
    pub(crate) fn hover(&mut self, pos: Point) -> UiResult {
        self.mouse = pos;
        let hit = if self.native_capture.is_some() {
            self.native_capture
        } else {
            self.hit_test(pos)
        };
        if hit != self.hot {
            if let Some(old) = self.hot
                && self.peer_for(old).is_none()
            {
                let _ = self.push_input(
                    old,
                    NodeEvent::Pointer(
                        PointerEvent {
                            position: pos,
                            button: None,
                            modifiers: Modifiers::default(),
                        },
                        crate::node::PointerPhase::Leave,
                    ),
                );
                self.rt.sched_unarm_tooltip(old);
            }
            self.hot = hit;
            if let Some(id) = hit {
                if self.peer_for(id).is_none() {
                    let _ = self.push_input(
                        id,
                        NodeEvent::Pointer(
                            PointerEvent {
                                position: pos,
                                button: None,
                                modifiers: Modifiers::default(),
                            },
                            crate::node::PointerPhase::Enter,
                        ),
                    );
                }
                if self.rt.node_has_tooltip(id) {
                    self.rt.sched_arm_tooltip(id);
                }
            }
        }
        // hovered editor receives the raw move (hover states, drag select)
        if let Some(id) = hit
            && let Some(peer) = self.peer_for(id)
        {
            let r = self.rects.get(&id).copied().unwrap_or_default();
            let lp = (((pos.y - r.y) as isize) << 16) | ((pos.x - r.x) as isize & 0xffff);
            peer.borrow().send(WM_MOUSEMOVE, 0, lp);
            self.service_peer_events()?;
        }
        self.turn()
    }

    /// Pointer left the window — hot leaves, tooltip disarms.
    pub(crate) fn leave(&mut self) -> UiResult {
        if let Some(old) = self.hot.take() {
            if self.peer_for(old).is_none() {
                let _ = self.push_input(
                    old,
                    NodeEvent::Pointer(
                        PointerEvent {
                            position: self.mouse,
                            button: None,
                            modifiers: Modifiers::default(),
                        },
                        crate::node::PointerPhase::Leave,
                    ),
                );
            }
            self.rt.sched_unarm_tooltip(old);
        }
        self.turn()
    }

    /// `WM_SETTINGCHANGE` — resnapshot OS appearance/reduced once.
    pub(crate) fn os_change(&mut self) -> UiResult {
        let appearance = os_appearance();
        self.rt.set_appearance(appearance);
        *self.peer_ctx.colors.borrow_mut() = (appearance, self.rt.theme.clone());
        self.last_appearance = appearance;
        self.turn()
    }

    /// `WM_GETOBJECT` — build/refresh the UIA root + live children once per
    /// request. Rebuild remints live generations, fencing stale providers.
    pub(crate) fn uia_provider(&mut self) -> UiResult<Option<uia::IRawRoot>> {
        use windows::Win32::UI::Accessibility::*;
        if self.uia.is_none() {
            let root = uia::UiaRoot::new(self.hwnd, "rust-ui")
                .map_err(|e| UiError::Platform(format!("UiaRoot: {e}")))?;
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
            let mut pt = POINT {
                x: (r.x * scale) as i32,
                y: (r.y * scale) as i32,
            };
            let _ = unsafe { ClientToScreen(self.hwnd, &mut pt) };
            let rect = UiaRect {
                left: pt.x as f64,
                top: pt.y as f64,
                width: (r.w * scale) as f64,
                height: (r.h * scale) as f64,
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
                _ => continue,
            };
            kids.push(uia::ChildBuild {
                id,
                name,
                ct,
                localized: loc,
                rect,
                peer_node: matches!(n.data, NodeData::Editor { .. }),
                native: None,
            });
        }
        // editors get the REAL windowless provider when available
        for k in &mut kids {
            if !k.peer_node {
                continue;
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
                if let Ok(site) = u.site()
                    && let Ok(prov) = unsafe { acc.CreateProvider(&site) }
                    && let Ok(frag) = prov.cast::<IRawElementProviderFragment>()
                {
                    u.register_native(k.id.slot, frag.clone()).ok();
                    k.native = Some(frag);
                }
            }
        }
        u.rebuild(kids)
            .map_err(|e| UiError::Platform(format!("uia rebuild: {e}")))?;
        Ok(Some(u.provider()))
    }

    /// UIA press/focus post landed — the generation check happens HERE so a
    /// stale external press can't hit a newer peer.
    pub(crate) fn uia_action(&mut self, slot: u32, generation: u64, focus: bool) -> UiResult {
        let id = NodeId { slot, generation };
        if !self.rt.arena.is_live(id) {
            return Ok(());
        }
        if focus {
            self.set_focus(Some(id));
        } else {
            let _ = self.push_input(id, NodeEvent::Press);
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
    pub(crate) fn pt(&self, lp: LPARAM) -> Point {
        let (x, y) = (lp.0 as i16 as i32, ((lp.0 >> 16) as i16) as i32);
        let s = self.peer_ctx.scale.get();
        Point {
            x: x as f32 / s,
            y: y as f32 / s,
        }
    }
    /// peer holding focus (if any)
    pub(crate) fn focus_peer(&self) -> Option<Rc<RefCell<WindowlessPeer>>> {
        self.focus.and_then(|id| self.peer_for(id))
    }
    /// WM_CHAR — focused peer sees the raw message (IME/text semantics)
    pub(crate) fn char(&mut self, _ch: u32) -> UiResult {
        if self.focus.is_some() {
            self.send_focused(WM_CHAR, _ch as usize, 0);
            self.service_peer_events()?;
        }
        self.turn()
    }
    /// DPI changed — scale update + full damage, no recreate
    pub(crate) fn dpi_changed(&mut self, dpi: u32) -> UiResult {
        let scale = dpi as f32 / 96.0;
        self.peer_ctx.scale.set(scale);
        self.renderer.borrow_mut().set_dpi(dpi as f32);
        self.relayout()
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
        if let Some(id) = self.focus {
            self.rt.composition_start(id);
        }
    }
    pub(crate) fn ime_end(&mut self) -> UiResult {
        if let Some(id) = self.focus {
            self.rt.composition_end(id)?;
        }
        self.turn()
    }

    /// Backend teardown — UIA disconnects BEFORE peers/surfaces/model die.
    pub(crate) fn shutdown(&mut self) {
        if let Some(u) = self.uia.take() {
            u.close();
        }
        self.renderer.borrow_mut().hide_tooltip();
        unsafe {
            let _ = DestroyCaret();
        }
        self.rt.shutdown();
    }

    /// After a commit/geometry change, validate the focused node — if it's
    /// gone/hidden/disabled, focus moves to the next eligible or the root.
    pub(crate) fn sanitize_focus(&mut self) {
        let valid = self.focus.is_some_and(|f| {
            self.rt
                .arena
                .get(f)
                .is_some_and(|n| n.interactive() && n.visibility == Visibility::Visible)
        });
        if !valid {
            let old = self.focus;
            self.focus = None;
            if let Some(id) = old
                && self.rt.arena.is_live(id)
            {
                let _ = self.push_input(id, NodeEvent::Focus(false));
            }
            self.focus_step(false);
        }
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
    let msft = Msftedit::load()?;
    let title = app.title.clone();

    // actual OS appearance + effective reduced motion — BEFORE first mount
    let appearance = os_appearance();
    let reduced = effective_reduced(&Theme::light(), &appearance);

    let sink = Arc::new(Mutex::new(Vec::new()));
    let peer_ctx = Rc::new(PeerCtx {
        lib: msft.clone(),
        hwnd: std::cell::Cell::new(HWND::default()),
        scale: std::cell::Cell::new(1.0),
        sink: sink.clone(),
        registry: Arc::new(Mutex::new(HashMap::new())),
        next_id: AtomicU64::new(1),
        colors: std::cell::RefCell::new((appearance, Theme::light())),
    });
    let factory_ctx = peer_ctx.clone();
    let factory = factory_ctx.make_factory();
    let mut theme = Theme::light();
    if theme.reduced_motion == ReducedMotion::System {
        theme.reduced_motion = if reduced {
            ReducedMotion::Reduce
        } else {
            ReducedMotion::NoPreference
        };
    }
    let rt = crate::runtime::Runtime::new(
        app.state,
        app.update,
        app.view,
        app.executor,
        Box::new(factory),
        theme.clone(),
        appearance,
        app.mailbox,
    );

    let mut backend = Box::new(Backend {
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
        native_timers: HashMap::new(),
        next_timer_id: 100,
        tooltip_for: None,
        uia: None,
        counters: Arc::new(PerfCounters::new()),
        closed: Arc::new(AtomicBool::new(false)),
        last_appearance: appearance,
        fatal: None,
        in_turn: std::cell::Cell::new(false),
        pump_queued: std::cell::Cell::new(false),
        deferred_native: Vec::new(),
    });
    backend.rt.theme = theme;

    // the window carries a stable pointer to the route object — its lifetime
    // is the loop's lifetime; WM_NCDESTROY clears it before `backend` drops
    let route = backend.as_mut() as *mut Backend<S, M, U, V>;
    let hwnd = window::create(&title, route)?;
    backend.hwnd = hwnd;
    backend.peer_ctx.hwnd.set(hwnd);
    let scale = unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(hwnd) as f32 / 96.0 };
    backend.peer_ctx.scale.set(scale.max(0.5));

    // mailbox -> posted pump (no polling)
    let closed = backend.closed.clone();
    let hwnd_raw = hwnd.0 as isize as usize;
    backend.rt.mailbox.install_wake(Arc::new(move || {
        if closed.load(Ordering::SeqCst) {
            return;
        }
        unsafe {
            let _ = PostMessageW(
                Some(HWND(hwnd_raw as *mut c_void)),
                WM_PUMP,
                WPARAM(0),
                LPARAM(0),
            );
        }
    }));

    if let Err(e) = backend.turn() {
        eprintln!("[turn-err] {e:?}");
        return Err(e);
    } // initial view/layout/paint

    // blocking GetMessage loop — exits when WM_QUIT lands
    unsafe {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    Ok(())
}

/// OS appearance: HKCU Themes\Personalize\AppsUseLightTheme (DWORD).
fn os_appearance() -> Appearance {
    use windows::Win32::System::Registry::*;
    let mut dark = false;
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
    }
    Appearance { dark }
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
