use super::*;

pub(crate) struct Envelope {
    pub reg: u64,
    pub payload: Box<dyn Any + Send>,
    pub(crate) charge: Option<Charge>,
}

/// Producer waiter, deduped per pending send future.
pub(crate) struct Waiter {
    pub(crate) id: u64,
    pub(crate) waker: Waker,
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

pub(crate) struct MailboxState {
    pub(crate) queue: VecDeque<Envelope>,
    pub(crate) closed: bool,
    pub(crate) waiters: Vec<Waiter>,
    /// the installed wake seam — invoked ONLY under this lock. Callback
    /// contract: strictly NO mailbox reentry (the real seam is
    /// `SetEvent` on an owned event — a pure signal, no state access).
    pub(crate) wake_cb: Option<Arc<dyn Fn() -> bool + Send + Sync>>,
    pub(crate) wake_state: WakeState,
    /// latched: a wake callback reported failure — the run loop reads it
    /// once at its safe point; a later `close()` must not clear it.
    pub(crate) wake_failed: bool,
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
    pub(crate) fn signal(&mut self) -> bool {
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
pub(crate) enum PushOutcome {
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
    pub(crate) state: Mutex<MailboxState>,
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
    pub(crate) fn push_or_wait(
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
    pub(crate) fn unregister_waiter(&self, id: u64) {
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
pub(crate) struct ChanCore {
    pub(crate) mailbox: Arc<Mailbox>,
    pub(crate) reg: u64,
    pub(crate) cancelled: Arc<AtomicBool>,
    /// the registration is finished — reclaim when pending drains
    pub(crate) completed: Arc<AtomicBool>,
    /// queued envelopes still in flight for this registration — UI drain
    /// decrements; completion only reaps at 0
    pub(crate) pending_envs: Arc<AtomicU64>,
    pub(crate) cancel_waiters: Arc<WakerSet>,
}

impl ChanCore {
    pub(crate) fn fenced(&self) -> bool {
        self.mailbox.is_closed() || self.cancelled.load(Ordering::SeqCst)
    }
}
