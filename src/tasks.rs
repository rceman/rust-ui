use std::any::Any;
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use crate::key::{ErasedKey, KeyId};

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

/// The app-supplied execution engine. No Tokio dependency: spawn hands the
/// boxed future to whatever runtime the product runs.
pub trait Executor: Send + Sync {
    fn spawn(&self, task: BoxFuture<()>) -> Result<(), TaskStartError>;
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum TaskStartError {
    /// spawn was called before `App::executor` configured an executor
    NoExecutor,
    /// more than `MAX_ACTIVE_TASKS` live registrations
    RegistryFull,
    /// the executor refused the future
    Rejected,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SendError {
    Closed,
    Cancelled,
    /// producer-waiter budget exhausted (bounded backpressure surface)
    TooManyWaiters,
    /// wake establishment failed AT acceptance — the triggering send's
    /// envelope was rolled back and the mailbox closed terminally; the
    /// sender gets a typed failure, never Ok over discarded work
    Wake,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ProxySendError {
    Full,
    Closed,
    /// wake establishment failed AT acceptance — the triggering send's
    /// envelope was rolled back and the mailbox closed terminally
    Wake,
}

/// One window's message/task mailbox: bounded, never lossy.
pub(crate) const MAILBOX_CAP: usize = 64;
/// Live task registrations (spawns) per window.
pub(crate) const MAX_ACTIVE_TASKS: usize = 8;
/// Blocked producer waiters on a full mailbox.
pub(crate) const MAX_SEND_WAITERS: usize = 64;

static REG_SERIAL: AtomicU64 = AtomicU64::new(1);
static WAITER_SERIAL: AtomicU64 = AtomicU64::new(1);

/// Checked monotonic serial — fails rather than wraps into a reused id.
pub(crate) fn checked_serial(s: &AtomicU64) -> u64 {
    s.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |x| x.checked_add(1))
        .expect("serial space exhausted")
}

/// Bounded waiter set on one cancellation flag: multiple independent
/// `cancelled()` futures + the registration's `CancelWatch` can all wait —
/// each registers under its own id and unregisters on drop. Bounded by the
/// consumer's outstanding cancelled-futures, not by retained registrations.
#[derive(Default)]
pub(crate) struct WakerSet {
    map: Mutex<HashMap<u64, Waker>>,
}

impl WakerSet {
    fn register(&self, id: u64, waker: Waker) {
        self.map.lock().unwrap().insert(id, waker);
    }
    fn remove(&self, id: u64) {
        self.map.lock().unwrap().remove(&id);
    }
    /// Take all wakers then wake — callers must NOT hold this set's lock.
    fn wake_all(&self) {
        let wakers: Vec<Waker> = {
            let mut m = self.map.lock().unwrap();
            m.drain().map(|(_, w)| w).collect()
        };
        for w in wakers {
            w.wake();
        }
    }
}

/// Cooperative cancellation handle handed to the worker.
#[derive(Clone)]
pub struct CancelToken {
    flag: Arc<AtomicBool>,
    waiters: Arc<WakerSet>,
}

impl CancelToken {
    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }

    /// Resolves when cancellation is fenced.
    pub async fn cancelled(&self) {
        CancelFuture {
            flag: self.flag.clone(),
            waiters: self.waiters.clone(),
            id: checked_serial(&WAITER_SERIAL) + 1,
            registered: false,
        }
        .await
    }
}

struct CancelFuture {
    flag: Arc<AtomicBool>,
    waiters: Arc<WakerSet>,
    id: u64,
    registered: bool,
}

impl Future for CancelFuture {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let this = &mut *self;
        // register FIRST, then re-check the flag — closes the check/register
        // race where a cancel between them is lost
        this.waiters.register(this.id, cx.waker().clone());
        this.registered = true;
        if this.flag.load(Ordering::SeqCst) {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

impl Drop for CancelFuture {
    fn drop(&mut self) {
        if self.registered {
            self.waiters.remove(self.id);
        }
    }
}

/// registration id used by external proxy envelopes (never a task)
pub(crate) const PROXY_REG: u64 = 0;

/// Registration in-flight charge: counts an outstanding envelope until it is
/// dequeued/dropped/purged. The LAST outstanding charge of a completed
/// registration lifecycle-wakes the UI so `reap_completed` runs even when no
/// new envelope arrives.
pub(crate) struct Charge {
    count: Arc<AtomicU64>,
    /// Weak — the queue owns envelopes which own charges; a strong back-edge
    /// would leak the whole mailbox through the cycle
    mailbox: std::sync::Weak<Mailbox>,
}

impl Charge {
    fn new(count: Arc<AtomicU64>, mailbox: std::sync::Weak<Mailbox>) -> Self {
        count.fetch_add(1, Ordering::SeqCst);
        Charge { count, mailbox }
    }
}

impl Drop for Charge {
    fn drop(&mut self) {
        if self.count.fetch_sub(1, Ordering::SeqCst) == 1 {
            // last outstanding work item released — let the UI reap. If the
            // mailbox is already gone (queue dropped), no wake is needed.
            // A false return means the wake authority died — the mailbox
            // applied its terminal contract inside poke_ui itself.
            if let Some(mb) = self.mailbox.upgrade() {
                let _ = mb.poke_ui();
            }
        }
    }
}

/// One queued envelope: erased `Any + Send` payload plus the opaque
/// registration id it was accepted under (never the app's task key). The
/// charge decrements exactly once on dequeue/drop/purge.
pub(crate) struct Envelope {
    pub reg: u64,
    pub payload: Box<dyn Any + Send>,
    charge: Option<Charge>,
}

/// Producer waiter, deduped per pending send future.
struct Waiter {
    id: u64,
    waker: Waker,
}

/// Wake-authority state — lives INSIDE the one state mutex, so a signal
/// attempt serializes against every acceptance/lifecycle path: while a
/// callback is in flight the lock is held and no producer can commit an
/// acceptance past a signal that is about to fail.
enum WakeState {
    /// No seam installed yet. `owed` = a startup-wake obligation accrued:
    /// a pre-install acceptance or lifecycle poke means `install_wake`
    /// MUST synchronously signal (and surfaces the typed failure to
    /// `App::run` if that install-time signal fails). It is NOT a durable
    /// signal — nothing was ever delivered anywhere. Never cleared by
    /// `wake_seen` — only the install consumes it.
    Uninstalled { owed: bool },
    /// Seam installed, no undelivered signal outstanding.
    Idle,
    /// A signal was delivered since the last `wake_seen` — reuse it.
    Confirmed,
    /// The wake authority reported failure, or the mailbox closed —
    /// no signal, no callback, no revival.
    Terminal,
}

struct MailboxState {
    queue: VecDeque<Envelope>,
    closed: bool,
    waiters: Vec<Waiter>,
    /// the installed wake seam — invoked ONLY under this lock. Callback
    /// contract: strictly NO mailbox reentry (the real seam is
    /// `SetEvent` on an owned event — a pure signal, no state access).
    wake_cb: Option<Arc<dyn Fn() -> bool + Send + Sync>>,
    wake_state: WakeState,
    /// latched: a wake callback reported failure — the run loop reads it
    /// once at its safe point; a later `close()` must not clear it.
    wake_failed: bool,
}

impl MailboxState {
    /// Invoke/observe the wake authority UNDER the state lock.
    /// `true` = the caller's commit is safe: the signal was delivered, a
    /// Confirmed edge was reused, OR — pre-install — a startup-wake
    /// obligation was recorded that `install_wake` discharges
    /// synchronously (a failed install-time pulse terminal-closes and
    /// reports the typed failure to `App::run`). `false` = Terminal, or
    /// the callback reported terminal failure — the CALLER applies
    /// `terminal_locked` (still under the lock).
    fn signal(&mut self) -> bool {
        match self.wake_state {
            WakeState::Terminal => false,
            WakeState::Confirmed => true,
            WakeState::Uninstalled { .. } => {
                // no durable signal exists yet — the acceptance is
                // recorded as a startup obligation install_wake pulses
                self.wake_state = WakeState::Uninstalled { owed: true };
                true
            }
            WakeState::Idle => {
                // the seam is always Some in an installed state
                let cb = self.wake_cb.clone().expect("installed wake seam");
                if cb() {
                    self.wake_state = WakeState::Confirmed;
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Terminal close under the already-held lock: detach the callback,
    /// mark closed + Terminal, drain queue + waiters. The failure latch
    /// is untouched — `take_wake_failure` still surfaces a real failure
    /// after a routine close. Drops/wakes happen AFTER the lock releases.
    fn terminal_locked(
        &mut self,
    ) -> (
        Vec<Envelope>,
        Vec<Waiter>,
        Option<Arc<dyn Fn() -> bool + Send + Sync>>,
    ) {
        self.closed = true;
        self.wake_state = WakeState::Terminal;
        let cb = self.wake_cb.take();
        (
            self.queue.drain(..).collect(),
            self.waiters.drain(..).collect(),
            cb,
        )
    }
}

/// Outcome of an atomic push-or-register-wait attempt — the envelope is
/// handed back on Wait/Closed so no payload is ever consumed by a failed push.
enum PushOutcome {
    Pushed,
    Waiting(Envelope),
    /// waiter budget exhausted
    WaitersFull(Envelope),
    Closed(Envelope),
    /// the triggering send's own wake establishment failed — the
    /// envelope was rolled back before acceptance committed and the
    /// mailbox closed terminally
    WakeFailed(Envelope),
}

/// Shared bounded mailbox. The UI thread drains it; workers push into it.
/// Waking is coalesced through one seam callback — no periodic polling.
/// Lock discipline: the ONE `state` mutex serializes lifecycle signals,
/// acceptance, install, wake_seen, and close. The installed wake callback
/// is invoked only under it (callback contract: no mailbox reentry —
/// the real seam is `SetEvent`); every drop (envelopes, charges, the
/// callback Arc) and every waker wake happens AFTER the lock releases.
pub(crate) struct Mailbox {
    state: Mutex<MailboxState>,
}

type WakeCb = Arc<dyn Fn() -> bool + Send + Sync>;

impl Mailbox {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Mailbox {
            state: Mutex::new(MailboxState {
                queue: VecDeque::with_capacity(MAILBOX_CAP),
                closed: false,
                waiters: Vec::new(),
                wake_cb: None,
                wake_state: WakeState::Uninstalled { owed: false },
                wake_failed: false,
            }),
        })
    }

    /// Run-loop wake callback. Installs under the state lock: a rejected
    /// mailbox reports `Closed` without installing; otherwise the seam
    /// takes over in `Idle` and — if a pre-install send accrued `owed`,
    /// a delivered Confirmed edge outlived the previous seam, or the
    /// queue is nonempty — pulses synchronously. A failed pulse applies
    /// the terminal contract under the same lock and reports `Wake`.
    pub(crate) fn install_wake(&self, wake: WakeCb) -> Result<(), ProxySendError> {
        let (dropped, waiters, old_cb, failed_cb, err) = {
            let mut st = self.state.lock().unwrap();
            if st.closed {
                return Err(ProxySendError::Closed);
            }
            // an earlier Confirmed edge is owed to the NEW seam — the
            // installed-callback contract re-establishes the pending wake
            let owed = match st.wake_state {
                WakeState::Uninstalled { owed } => owed,
                WakeState::Confirmed => true,
                _ => false,
            };
            let old_cb = st.wake_cb.replace(wake);
            st.wake_state = WakeState::Idle;
            if owed || !st.queue.is_empty() {
                if st.signal() {
                    (Vec::new(), Vec::new(), old_cb, None, None)
                } else {
                    st.wake_failed = true;
                    let (d, w, failed_cb) = st.terminal_locked();
                    (d, w, old_cb, failed_cb, Some(ProxySendError::Wake))
                }
            } else {
                (Vec::new(), Vec::new(), old_cb, None, None)
            }
        };
        // callback Arc destruction happens AFTER the state lock releases —
        // a captured destructor may safely reenter mailbox state
        drop(old_cb);
        drop(failed_cb);
        drop(dropped);
        for w in waiters {
            w.waker.wake();
        }
        match err {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    /// Run-loop safe point: did a cross-thread wake signal fail since the
    /// last check? Latch cleared on read — each failure surfaces once.
    pub(crate) fn take_wake_failure(&self) -> bool {
        let mut st = self.state.lock().unwrap();
        let f = st.wake_failed;
        st.wake_failed = false;
        f
    }

    /// Edge-triggered wake for lifecycle/continuation signals — invoked
    /// under the state lock, serialized against acceptance. `false` =
    /// the wake authority reported terminal failure (terminal contract
    /// applied under the same lock before any producer could commit) OR
    /// the mailbox is already Terminal — no signal, no revival.
    pub(crate) fn poke_ui(&self) -> bool {
        let (dropped, waiters, cb, ok) = {
            let mut st = self.state.lock().unwrap();
            if matches!(st.wake_state, WakeState::Terminal) {
                (Vec::new(), Vec::new(), None, false)
            } else if st.signal() {
                (Vec::new(), Vec::new(), None, true)
            } else {
                st.wake_failed = true;
                let (d, w, c) = st.terminal_locked();
                (d, w, c, false)
            }
        };
        drop(cb);
        drop(dropped);
        for w in waiters {
            w.waker.wake();
        }
        ok
    }

    /// The run loop observed the wake — consumes ONLY a delivered
    /// (Confirmed) edge; an Uninstalled `owed` obligation survives until
    /// `install_wake` discharges it.
    pub(crate) fn wake_seen(&self) {
        let mut st = self.state.lock().unwrap();
        if matches!(st.wake_state, WakeState::Confirmed) {
            st.wake_state = WakeState::Idle;
        }
    }

    /// Atomic push-or-register-wait under ONE lock — no lost wake between a
    /// failed push and waiter registration. `waiter_id` dedups repolls of
    /// the same pending future.
    /// Atomic push-or-register-wait under ONE state lock. `check_fenced`
    /// re-verifies cancellation/completion under that same lock.
    /// ACCEPTANCE LINEARIZES AT push+signal under this one lock: the
    /// envelope commits only once a wake is accounted for: INSTALLED → a
    /// just-delivered or Confirmed-reused signal; NOT-YET-INSTALLED → a
    /// recorded startup obligation `install_wake` synchronously
    /// discharges (and whose failure terminal-closes + reports typed
    /// Wake to `App::run`). Coalescing only ever reuses a Confirmed edge —
    /// `owed` is a pre-install accounting, not an owed coalesced signal.
    /// If the wake authority reports failure, THIS envelope is rolled
    /// back (it is provably still the tail under the lock), the mailbox
    /// closes terminally, producers wake — the triggering send gets a
    /// typed Wake failure, never Ok over discarded work.
    fn push_or_wait(
        &self,
        env: Envelope,
        check_fenced: &dyn Fn() -> bool,
        waiter: (u64, Waker),
    ) -> PushOutcome {
        let mut st = self.state.lock().unwrap();
        if st.closed || check_fenced() {
            return PushOutcome::Closed(env);
        }
        if st.queue.len() >= MAILBOX_CAP {
            let (id, waker) = waiter;
            if let Some(w) = st.waiters.iter_mut().find(|w| w.id == id) {
                w.waker = waker; // same future re-polled: update, don't accumulate
            } else {
                if st.waiters.len() >= MAX_SEND_WAITERS {
                    return PushOutcome::WaitersFull(env);
                }
                st.waiters.push(Waiter { id, waker });
            }
            return PushOutcome::Waiting(env);
        }
        st.queue.push_back(env);
        if st.signal() {
            return PushOutcome::Pushed;
        }
        // wake establishment failed AT acceptance — roll THIS envelope
        // back, close terminally UNDER this lock (before any other
        // producer can commit), and hand the payload back to the sender
        let env = st.queue.pop_back().unwrap();
        st.wake_failed = true;
        let (dropped, waiters, cb) = st.terminal_locked();
        drop(st);
        drop(cb);
        drop(dropped);
        for w in waiters {
            w.waker.wake();
        }
        PushOutcome::WakeFailed(env)
    }

    /// Bounded push for non-async callers (UiProxy). Returns the envelope
    /// back on Full/Closed/Wake so nothing is consumed by a failed push.
    /// ACCEPTANCE LINEARIZES AT push+signal under one lock — Ok(()) means
    /// the envelope committed AND a wake is accounted for — an installed
    /// Confirmed/delivered signal, or a recorded startup obligation
    /// `install_wake` discharges synchronously (reporting failure typed).
    pub(crate) fn try_push(
        self: &Arc<Self>,
        env: Envelope,
    ) -> Result<(), (ProxySendError, Envelope)> {
        let mut st = self.state.lock().unwrap();
        if st.closed {
            return Err((ProxySendError::Closed, env));
        }
        if st.queue.len() >= MAILBOX_CAP {
            return Err((ProxySendError::Full, env));
        }
        st.queue.push_back(env);
        if st.signal() {
            return Ok(());
        }
        // wake establishment failed AT acceptance — roll THIS envelope
        // back (provably the tail), close terminally under this lock,
        // typed Wake error
        let env = st.queue.pop_back().unwrap();
        st.wake_failed = true;
        let (dropped, waiters, cb) = st.terminal_locked();
        drop(st);
        drop(cb);
        drop(dropped);
        for w in waiters {
            w.waker.wake();
        }
        Err((ProxySendError::Wake, env))
    }

    /// Remove a pending-future waiter (called from `SendFuture::drop`).
    fn unregister_waiter(&self, id: u64) {
        self.state.lock().unwrap().waiters.retain(|w| w.id != id);
    }

    /// Pop one envelope; freed capacity wakes producers OUTSIDE the lock.
    pub(crate) fn pop(&self) -> Option<Envelope> {
        let (env, waiters) = {
            let mut st = self.state.lock().unwrap();
            let env = st.queue.pop_front();
            let wake = if env.is_some() && st.queue.len() < MAILBOX_CAP {
                st.waiters.drain(..).collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            (env, wake)
        };
        for w in waiters {
            w.waker.wake();
        }
        env
    }

    /// Mark a registration cancelled AND detach its queued envelopes under
    /// the same state lock — a producer's push rechecks the flag under this
    /// lock, so cancellation and stale-envelope removal are one atomic step.
    /// Envelopes drop (releasing charges) and waiters wake only after the
    /// lock is released.
    pub(crate) fn fence_reg(&self, reg: u64, cancelled: &std::sync::atomic::AtomicBool) {
        let (dropped, waiters) = {
            let mut st = self.state.lock().unwrap();
            cancelled.store(true, Ordering::SeqCst);
            let mut rest = VecDeque::with_capacity(st.queue.len());
            let mut dropped = Vec::new();
            for e in st.queue.drain(..) {
                if e.reg == reg {
                    dropped.push(e);
                } else {
                    rest.push_back(e);
                }
            }
            st.queue = rest;
            (dropped, st.waiters.drain(..).collect::<Vec<_>>())
        };
        // drops happen here — Charge release may poke the UI, which is safe
        // now that the state lock is free
        drop(dropped);
        for w in waiters {
            w.waker.wake();
        }
    }

    /// Back-compat alias for registrations with no cancel flag at this site.
    pub(crate) fn purge_reg(&self, reg: u64) {
        self.fence_reg(reg, &std::sync::atomic::AtomicBool::new(false));
    }

    /// Window close: closed + Terminal + callback detached under the one
    /// lock — a retained `UiProxy` can never resurrect a stale HWND seam.
    /// Every pending producer/cancel waiter wakes, and every envelope
    /// drops, only after the lock releases. A real wake failure stays
    /// latched — a routine close must not erase it.
    pub(crate) fn close(&self) {
        let (dropped, waiters, cb) = {
            let mut st = self.state.lock().unwrap();
            st.terminal_locked()
        };
        drop(cb);
        drop(dropped); // release charges outside the lock
        for w in waiters {
            w.waker.wake();
        }
    }

    pub(crate) fn is_closed(&self) -> bool {
        self.state.lock().unwrap().closed
    }

    /// Visible backlog depth — used by the run loop's continuation check.
    pub(crate) fn queue_len(&self) -> usize {
        self.state.lock().unwrap().queue.len()
    }

    /// test seam: clone the installed wake callback as an in-flight
    /// signaler would (proves the owned event outlives detach)
    #[cfg(test)]
    pub(crate) fn clone_wake_cb(&self) -> Option<Arc<dyn Fn() -> bool + Send + Sync>> {
        self.state.lock().unwrap().wake_cb.clone()
    }

    /// test seam: is the ONE mailbox state lock currently held? The
    /// overlap tests use this as structural proof that a producer
    /// serialized behind an in-flight lifecycle signal.
    #[cfg(test)]
    pub(crate) fn state_try_locked(&self) -> bool {
        self.state.try_lock().is_err()
    }
}

/// Shared channel state carried by `TaskSender`/`Job`/`CancelWatch` —
/// the worker side knows only the opaque registration id and these flags.
struct ChanCore {
    mailbox: Arc<Mailbox>,
    reg: u64,
    cancelled: Arc<AtomicBool>,
    /// the registration is finished — reclaim when pending drains
    completed: Arc<AtomicBool>,
    /// queued envelopes still in flight for this registration — UI drain
    /// decrements; completion only reaps at 0
    pending_envs: Arc<AtomicU64>,
    cancel_waiters: Arc<WakerSet>,
}

impl ChanCore {
    fn fenced(&self) -> bool {
        self.mailbox.is_closed() || self.cancelled.load(Ordering::SeqCst)
    }
}

/// Typed sender handed to a `Job`. `map` stores a lifting factory so each
/// chunk becomes the parent's message type at send time.
pub struct TaskSender<M> {
    core: Arc<ChanCore>,
    lift: Arc<dyn Fn(M) -> Box<dyn Any + Send> + Send + Sync>,
}

impl<M> Clone for TaskSender<M> {
    fn clone(&self) -> Self {
        TaskSender {
            core: self.core.clone(),
            lift: self.lift.clone(),
        }
    }
}

impl<M: Send + 'static> TaskSender<M> {
    /// Is the channel closed/cancelled? Cheap public probe for workers.
    pub fn is_closed(&self) -> bool {
        self.core.fenced()
    }

    /// Bounded send: pending while the mailbox is full (bounded deduped
    /// waiter set, woken by the UI drain/purge/close), `Err` on
    /// close/cancel/completion. Chunks keep stream order, never coalesced.
    /// UI-side sends never block synchronously — the future suspends.
    pub async fn send(&self, msg: M) -> Result<(), SendError> {
        self.send_future(msg).await
    }

    /// Private concrete future for the async `send`.
    fn send_future(&self, msg: M) -> SendFuture {
        SendFuture {
            core: self.core.clone(),
            payload: Some((self.lift)(msg)),
            waiter_id: checked_serial(&WAITER_SERIAL),
            registered: false,
            charge: Some(Charge::new(
                self.core.pending_envs.clone(),
                Arc::downgrade(&self.core.mailbox),
            )),
        }
    }

    /// Lift chunks of another type into `M`.
    pub fn map<N: Send + 'static>(
        &self,
        f: impl Fn(N) -> M + Send + Sync + 'static,
    ) -> TaskSender<N> {
        let lift = self.lift.clone();
        TaskSender {
            core: self.core.clone(),
            lift: Arc::new(move |n| lift(f(n))),
        }
    }
}

/// The job context handed to a spawned task factory.
pub struct Job<M> {
    sender: TaskSender<M>,
    cancel: CancelToken,
}

impl<M: Send + 'static> Job<M> {
    pub fn cancellation(&self) -> CancelToken {
        self.cancel.clone()
    }
    pub fn sender(&self) -> &TaskSender<M> {
        &self.sender
    }
    pub async fn send(&self, msg: M) -> Result<(), SendError> {
        self.sender.send(msg).await
    }
}

/// Async bounded send — pending on full with a deduped waiter slot that is
/// removed on drop.
///
/// `charged` tracks the `pending_envs` reservation made at `send()` time:
/// the charge is held until the envelope lands in the queue (then the queue
/// owns it until drain) OR released on error/drop — a completed task can
/// only be reclaimed once all accepted chunks have been dequeued.
pub struct SendFuture {
    core: Arc<ChanCore>,
    payload: Option<Box<dyn Any + Send>>,
    waiter_id: u64,
    registered: bool,
    /// the in-flight charge — handed INTO the envelope on a successful push
    /// (the queue then owns it until dequeue/purge drops it)
    charge: Option<Charge>,
}

impl SendFuture {
    /// Reclaim the charge from a handed-back envelope.
    fn reclaim(&mut self, env: Envelope) {
        self.payload = Some(env.payload);
        self.charge = env.charge;
    }
}

impl Future for SendFuture {
    type Output = Result<(), SendError>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = &mut *self;
        if this.core.fenced() {
            this.charge = None; // drop releases the charge
            return Poll::Ready(Err(if this.core.mailbox.is_closed() {
                SendError::Closed
            } else {
                SendError::Cancelled
            }));
        }
        if this.core.completed.load(Ordering::SeqCst) {
            this.charge = None;
            return Poll::Ready(Err(SendError::Closed));
        }
        let Some(payload) = this.payload.take() else {
            this.charge = None;
            return Poll::Ready(Err(SendError::Closed));
        };
        let core = this.core.clone();
        match this.core.mailbox.push_or_wait(
            Envelope {
                reg: this.core.reg,
                payload,
                charge: this.charge.take(),
            },
            &move || core.cancelled.load(Ordering::SeqCst) || core.completed.load(Ordering::SeqCst),
            (this.waiter_id, cx.waker().clone()),
        ) {
            PushOutcome::Pushed => Poll::Ready(Ok(())),
            PushOutcome::Waiting(env) => {
                this.reclaim(env);
                this.registered = true;
                Poll::Pending
            }
            PushOutcome::WaitersFull(env) => {
                this.reclaim(env);
                this.charge = None;
                Poll::Ready(Err(SendError::TooManyWaiters))
            }
            PushOutcome::Closed(env) => {
                this.reclaim(env);
                this.charge = None;
                Poll::Ready(Err(SendError::Closed))
            }
            PushOutcome::WakeFailed(env) => {
                this.reclaim(env);
                this.charge = None;
                Poll::Ready(Err(SendError::Wake))
            }
        }
    }
}

impl Drop for SendFuture {
    fn drop(&mut self) {
        if self.registered {
            self.core.mailbox.unregister_waiter(self.waiter_id);
        }
        // the Charge guard releases on drop — the last outstanding charge
        // lifecycle-wakes the UI so the registration can be reaped
    }
}

/// External worker bridge (`App::proxy`) — same bounded mailbox, no task gen.
pub struct UiProxy<M> {
    mailbox: Arc<Mailbox>,
    _marker: std::marker::PhantomData<fn(M)>,
}

impl<M: Send + 'static> UiProxy<M> {
    pub(crate) fn new(mailbox: Arc<Mailbox>) -> Self {
        UiProxy {
            mailbox,
            _marker: std::marker::PhantomData,
        }
    }

    /// Bounded non-blocking send — typed `Full`/`Closed`, never a silent drop.
    pub fn try_send(&self, msg: M) -> Result<(), ProxySendError> {
        self.mailbox
            .try_push(Envelope {
                reg: PROXY_REG,
                payload: Box::new(msg),
                charge: None,
            })
            .map_err(|(e, _env)| e)
    }
}

/// Message adapter chain inside a `ui.scope` — scope keys compose into an
/// ordered path; identity is the FULL key sequence (equality-checked with
/// TypeId, never a hash fingerprint).
#[derive(Clone)]
pub(crate) struct ScopePath {
    /// ordered scope keys, outermost first
    pub(crate) keys: Rc<Vec<ErasedKey>>,
    /// per-scope message lifts, innermost last — applied leaf->root
    pub(crate) lifts: Rc<Vec<Rc<dyn Fn(Box<dyn Any>) -> Box<dyn Any>>>>,
}

impl ScopePath {
    pub(crate) fn root() -> Self {
        ScopePath {
            keys: Rc::new(Vec::new()),
            lifts: Rc::new(Vec::new()),
        }
    }

    pub(crate) fn push(
        &self,
        key: ErasedKey,
        lift: Rc<dyn Fn(Box<dyn Any>) -> Box<dyn Any>>,
    ) -> ScopePath {
        let mut keys = (*self.keys).clone();
        keys.push(key);
        let mut lifts = (*self.lifts).clone();
        lifts.push(lift);
        ScopePath {
            keys: Rc::new(keys),
            lifts: Rc::new(lifts),
        }
    }
}

/// Registry key: full ordered scope path + local task key. Equality walks
/// every element with TypeId-aware `ErasedKey` equality — hash is only a
/// bucket hint, collisions can never alias two namespaces.
type TaskSlotKey = (Rc<Vec<ErasedKey>>, ErasedKey);

struct Registration {
    reg_id: u64,
    cancelled: Arc<AtomicBool>,
    completed: Arc<AtomicBool>,
    pending_envs: Arc<AtomicU64>,
    cancel_waiters: Arc<WakerSet>,
    lift: Box<dyn Fn(Box<dyn Any>) -> Option<Box<dyn Any>>>,
}

pub(crate) struct Dequeued {
    pub(crate) msg: Box<dyn Any>,
    pub(crate) reg: u64,
}

#[derive(Default)]
pub(crate) struct TaskRegistry {
    /// (scope path, task key) -> live registration
    regs: HashMap<TaskSlotKey, Registration>,
    /// opaque reg_id -> slot key (dequeue-time resolution only)
    by_reg: HashMap<u64, TaskSlotKey>,
}

impl TaskRegistry {
    pub(crate) fn live_count(&self) -> usize {
        self.regs.len()
    }

    /// Spawn under (scope, key): any previous registration for the slot is
    /// fenced and BOTH map entries removed immediately — a stale worker-side
    /// token can never resolve again. The fresh generation's Job carries
    /// only the opaque `reg_id`.
    pub(crate) fn spawn<M: Send + 'static>(
        &mut self,
        scope: &ScopePath,
        key: impl KeyId,
        mailbox: &Arc<Mailbox>,
    ) -> Result<(Job<M>, u64), TaskStartError> {
        let slot_key: TaskSlotKey = (scope.keys.clone(), ErasedKey::new(key));
        if !self.regs.contains_key(&slot_key) && self.regs.len() >= MAX_ACTIVE_TASKS {
            return Err(TaskStartError::RegistryFull);
        }
        self.fence(&slot_key, mailbox);

        let reg_id = checked_serial(&REG_SERIAL);
        let cancelled = Arc::new(AtomicBool::new(false));
        let completed = Arc::new(AtomicBool::new(false));
        let pending_envs = Arc::new(AtomicU64::new(0));
        let cancel_waiters = Arc::new(WakerSet::default());
        let lifts = scope.lifts.clone();
        let core = Arc::new(ChanCore {
            mailbox: mailbox.clone(),
            reg: reg_id,
            cancelled: cancelled.clone(),
            completed: completed.clone(),
            pending_envs: pending_envs.clone(),
            cancel_waiters: cancel_waiters.clone(),
        });
        let job = Job {
            sender: TaskSender {
                core,
                lift: Arc::new(|m: M| Box::new(m) as Box<dyn Any + Send>),
            },
            cancel: CancelToken {
                flag: cancelled.clone(),
                waiters: cancel_waiters.clone(),
            },
        };
        self.regs.insert(
            slot_key.clone(),
            Registration {
                reg_id,
                cancelled,
                completed,
                pending_envs,
                cancel_waiters,
                lift: Box::new(move |payload| {
                    // apply scope adapters leaf->root: last pushed is innermost
                    let mut m: Box<dyn Any> = payload;
                    for f in lifts.iter().rev() {
                        m = f(m);
                    }
                    Some(m)
                }),
            },
        );
        self.by_reg.insert(reg_id, slot_key);
        Ok((job, reg_id))
    }

    /// Fence one registration: flags set, both map entries dropped (opaque
    /// token dead), queued envelopes purged, cancel waker woken — the wake
    /// itself happens outside every lock via `CancelWatch`'s own poll.
    fn fence(&mut self, slot_key: &TaskSlotKey, mailbox: &Arc<Mailbox>) {
        let Some(reg) = self.regs.remove(slot_key) else {
            return;
        };
        self.by_reg.remove(&reg.reg_id);
        // cancel flag + stale-envelope detach are ONE atomic step under the
        // mailbox state lock (through fence_reg) — a producer whose push
        // rechecks under that lock can never land after the fence
        mailbox.fence_reg(reg.reg_id, &reg.cancelled);
        reg.completed.store(true, Ordering::SeqCst);
        // every registered cancellation waiter (watch + independent
        // `cancelled()` futures) is taken out and woken OUTSIDE the mailbox
        // lock — a waker that reenters cancel/queue state can't deadlock
        reg.cancel_waiters.wake_all();
    }

    /// Fence by opaque registration id (executor-refused spawn cleanup).
    pub(crate) fn cancel_reg(&mut self, reg_id: u64, mailbox: &Arc<Mailbox>) {
        if let Some(slot_key) = self.by_reg.get(&reg_id).cloned() {
            self.fence(&slot_key, mailbox);
        }
    }

    /// `cx.cancel(key)`: synchronous fence of send + dequeue + maps.
    pub(crate) fn cancel(&mut self, scope: &ScopePath, key: impl KeyId, mailbox: &Arc<Mailbox>) {
        let slot_key: TaskSlotKey = (scope.keys.clone(), ErasedKey::new(key));
        self.fence(&slot_key, mailbox);
    }

    /// Resolve an envelope at dequeue: unknown/fenced registrations drop the
    /// envelope; live ones lift to the root message. Decrements the
    /// registration's in-flight count so completion reaps after drain.
    pub(crate) fn resolve(&mut self, env: Envelope) -> Option<Dequeued> {
        if env.reg == PROXY_REG {
            return Some(Dequeued {
                msg: env.payload as Box<dyn Any>,
                reg: PROXY_REG,
            });
        }
        let slot = self.by_reg.get(&env.reg)?.clone();
        let reg = self.regs.get(&slot)?;
        if reg.cancelled.load(Ordering::SeqCst) {
            return None;
        }
        (reg.lift)(env.payload).map(|m| Dequeued {
            msg: m,
            reg: env.reg,
        })
    }

    /// Reap completed registrations whose queued envelopes have drained.
    /// Only `completed && pending_envs == 0` slots are removed — accepted
    /// chunks always deliver first, and a stale reg_id can't evict a
    /// replacement (the slot-key check guards it).
    pub(crate) fn reap_completed(&mut self) {
        let dead: Vec<TaskSlotKey> = self
            .regs
            .iter()
            .filter(|(_, r)| {
                r.completed.load(Ordering::SeqCst) && r.pending_envs.load(Ordering::SeqCst) == 0
            })
            .map(|(k, _)| k.clone())
            .collect();
        for k in dead {
            if let Some(r) = self.regs.get(&k) {
                let reg_id = r.reg_id;
                if self.by_reg.get(&reg_id) == Some(&k) {
                    self.by_reg.remove(&reg_id);
                }
                self.regs.remove(&k);
            }
        }
    }

    /// All bits a `CancelWatch` needs to wrap the spawned future.
    pub(crate) fn job_bits(
        &self,
        reg_id: u64,
    ) -> (Arc<AtomicBool>, Arc<AtomicBool>, Arc<WakerSet>) {
        let slot = self.by_reg.get(&reg_id).unwrap();
        let reg = self.regs.get(slot).unwrap();
        (
            reg.cancelled.clone(),
            reg.completed.clone(),
            reg.cancel_waiters.clone(),
        )
    }

    /// Window close: fence everything, purge every queued envelope, drop the
    /// maps — no tombstones survive teardown.
    pub(crate) fn fence_all(&mut self, mailbox: &Arc<Mailbox>) {
        let keys: Vec<TaskSlotKey> = self.regs.keys().cloned().collect();
        for k in keys {
            self.fence(&k, mailbox);
        }
        debug_assert!(self.regs.is_empty() && self.by_reg.is_empty());
    }
}

/// Cancellation/completion wrapper around a spawned task future: registers
/// the cancel waker BEFORE re-checking the flag (no lost cancel), drops the
/// inner future as soon as it is fenced, and on normal completion marks the
/// registration done — the UI reaps it after accepted chunks drain.
pub(crate) struct CancelWatch {
    pub(crate) inner: BoxFuture<()>,
    pub(crate) cancelled: Arc<AtomicBool>,
    pub(crate) completed: Arc<AtomicBool>,
    /// the watch registers itself under the reserved slot 0 — consumer
    /// `cancelled()` futures use the serial range
    pub(crate) cancel_waiters: Arc<WakerSet>,
    pub(crate) mailbox: Arc<Mailbox>,
    pub(crate) watch_registered: bool,
}

impl Future for CancelWatch {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let this = &mut *self;
        // register first, then re-check — a cancel that lands between
        // check and register can never be lost
        this.cancel_waiters.register(0, cx.waker().clone());
        this.watch_registered = true;
        if this.cancelled.load(Ordering::SeqCst) || this.mailbox.is_closed() {
            return Poll::Ready(()); // fenced/closed: stop polling, drop the future
        }
        match this.inner.as_mut().poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(()) => {
                // normal completion: mark done — the registration reaps once
                // its accepted envelopes have drained (no queue slot needed)
                this.completed.store(true, Ordering::SeqCst);
                let _ = this.mailbox.poke_ui(); // lifecycle signal
                Poll::Ready(())
            }
        }
    }
}
