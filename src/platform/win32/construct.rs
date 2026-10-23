use super::*;

impl<S, M, U, V> Backend<S, M, U, V>
where
    M: 'static,
    U: Fn(&mut S, M, &mut UpdateCtx<'_, M>),
    V: Fn(&S, &mut crate::Ui<'_, '_, M>),
{
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
    pub(super) fn mailbox_wake(ev: &MailboxEvent, closed: &AtomicBool) -> WakeOutcome {
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
            paint_epoch: std::cell::Cell::new(0),
            rt,
            hwnd: HWND::default(),
            dark_caption: std::cell::Cell::new(None),
            peer_ctx,
            renderer: RefCell::new(Renderer::new()?),
            rects: HashMap::new(),
            order: Vec::new(),
            clips: HashMap::new(),
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
            size_pending: std::cell::Cell::new(false),
            laid_client: std::cell::Cell::new((0.0, 0.0)),
            resize_trace_path: std::env::var_os("RUI_RESIZE_TRACE").map(std::path::PathBuf::from),
            size_seq: std::cell::Cell::new(0),
            sizing_session: std::cell::Cell::new(false),
            move_pending: std::cell::Cell::new(false),
            damage: std::cell::Cell::new(None),
            committed_ink: std::cell::RefCell::new(HashMap::new()),
            applied_palette: std::cell::RefCell::new(None),
            open_popups: HashMap::new(),
            popup_seq: 0,
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

/// The set of messages whose wparam/lparam are plain scalars — the only
/// ones permitted in the deferred queue.

/// Owned kernel wake event — the HANDLE lives exactly as long as the
/// last `Arc<MailboxEvent>`: the run loop holds one, the installed wake
/// callback holds one, and any in-flight signaler cloned from the
/// callback holds one until its call returns. `CloseHandle` can never
/// precede the last legitimate signaler — a detached callback that was
/// already cloned still signals a VALID handle.
pub(crate) struct MailboxEvent(pub(super) HANDLE);
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
