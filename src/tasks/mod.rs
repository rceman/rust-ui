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
                let mb: std::sync::Arc<Mailbox> = mb;
                let _ = mb.poke_ui();
            }
        }
    }
}

/// One queued envelope: erased `Any + Send` payload plus the opaque
/// registration id it was accepted under (never the app's task key). The
/// charge decrements exactly once on dequeue/drop/purge.
pub(crate) mod mailbox;
pub(crate) mod sender;
pub(crate) use mailbox::{ChanCore, Envelope, Mailbox, MailboxState, PushOutcome, Waiter};
pub use sender::*;
