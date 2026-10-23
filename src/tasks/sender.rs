use super::mailbox::{ChanCore, PushOutcome};
use super::*;

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
