/// A payload that counts its own drops — proves accepted work delivers and
/// failed work drops exactly once (never double, never silently kept).
struct DropPayload {
    id: u64,
    drops: Arc<std::sync::atomic::AtomicUsize>,
}
impl Drop for DropPayload {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

/// F01 — a lifecycle signal IN FLIGHT owns the state lock: an overlapping
/// synchronous send must serialize behind it. When the signal fails the
/// producer resolves Closed (terminal) — never Ok over discarded work.
#[test]
fn lifecycle_inflight_failure_serializes_overlap_send_closed() {
    use std::sync::mpsc;
    let mb = crate::tasks::Mailbox::new();
    let (entered_tx, entered_rx) = mpsc::channel::<()>();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let release_rx = Mutex::new(release_rx);
    // deterministic in-flight window: report entered, pause, fail
    mb.install_wake(Arc::new(move || {
        let _ = entered_tx.send(());
        // bounded pause — a hung test fails on timeout, never deadlocks
        let _ = release_rx
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(5));
        false
    }))
    .unwrap();
    let mb2 = mb.clone();
    let poke = std::thread::spawn(move || mb2.poke_ui());
    entered_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("the lifecycle callback entered");

    let drops = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let (started_tx, started_rx) = mpsc::channel::<()>();
    let (done_tx, done_rx) = mpsc::channel();
    let mb3 = mb.clone();
    let d2 = drops.clone();
    let send = std::thread::spawn(move || {
        let p = crate::tasks::UiProxy::<DropPayload>::new(mb3);
        // producer-started marker IMMEDIATELY before the send attempt —
        // the main thread knows the worker is inside the lock wait
        let _ = started_tx.send(());
        let r = p.try_send(DropPayload { id: 7, drops: d2 });
        let _ = done_tx.send(r);
    });
    started_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("the producer started its attempt");

    // while the lifecycle call owns the lock the producer must be parked —
    // no result, and the state lock provably reads held (not a scheduling
    // guess: try_lock is the structural evidence)
    let early = done_rx.recv_timeout(Duration::from_millis(200));
    let locked = mb.state_try_locked();
    // release + join FIRST — a later assertion failure can never leave a
    // blocked background thread behind
    let _ = release_tx.send(());
    let poke_ok = poke.join().expect("poke thread panicked");
    let r = done_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("the serialized send resolves");
    send.join().unwrap();
    assert!(
        early.is_err(),
        "overlap send resolved before the in-flight lifecycle finished"
    );
    assert!(locked, "the state lock is held for the whole signal");
    assert!(!poke_ok, "the failing signal reports terminal");
    assert_eq!(r, Err(crate::tasks::ProxySendError::Closed));
    assert_eq!(
        drops.load(Ordering::SeqCst),
        1,
        "payload dropped exactly once"
    );
    assert_eq!(mb.queue_len(), 0);
    assert!(mb.pop().is_none(), "terminal queue is empty");
    assert!(mb.is_closed());
    assert!(mb.take_wake_failure(), "the failure is latched");
    assert_eq!(
        crate::tasks::UiProxy::<u32>::new(mb.clone()).try_send(1),
        Err(crate::tasks::ProxySendError::Closed)
    );
}

/// F01 — the same in-flight window through a REAL `TaskSender::send`
/// (the actual producer future, not a wrapper): the poll serializes
/// behind the failing lifecycle signal and resolves `Err(Closed)`.
#[test]
fn lifecycle_inflight_failure_serializes_overlap_async_send() {
    use std::sync::mpsc;
    let mb = crate::tasks::Mailbox::new();
    let (entered_tx, entered_rx) = mpsc::channel::<()>();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let release_rx = Mutex::new(release_rx);
    mb.install_wake(Arc::new(move || {
        let _ = entered_tx.send(());
        let _ = release_rx
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(5));
        false
    }))
    .unwrap();
    let mb2 = mb.clone();
    let poke = std::thread::spawn(move || mb2.poke_ui());
    entered_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("the lifecycle callback entered");

    // a real registration + Job — the registry stays alive UI-side
    let mut reg = crate::tasks::TaskRegistry::default();
    let (job, _rid) = reg
        .spawn::<DropPayload>(&crate::tasks::ScopePath::root(), "overlap", &mb)
        .unwrap();
    let sender = job.sender().clone();
    let probe = sender.clone(); // retained for the post-terminal assertion
    let drops = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let (started_tx, started_rx) = mpsc::channel::<()>();
    let (done_tx, done_rx) = mpsc::channel();
    let d2 = drops.clone();
    let send = std::thread::spawn(move || {
        let fut = sender.send(DropPayload { id: 9, drops: d2 });
        let mut fut = std::pin::pin!(fut);
        let mut cx = Context::from_waker(std::task::Waker::noop());
        let _ = started_tx.send(());
        let out = fut.as_mut().poll(&mut cx);
        let _ = done_tx.send(out);
        // `fut` drops here — its charge releases after the result
    });
    started_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("the producer started its poll");

    let early = done_rx.recv_timeout(Duration::from_millis(200));
    let locked = mb.state_try_locked();
    let _ = release_tx.send(());
    let poke_ok = poke.join().unwrap();
    let r = done_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("the serialized send resolves");
    send.join().unwrap();
    assert!(
        early.is_err(),
        "overlap async send resolved before the in-flight lifecycle finished"
    );
    assert!(locked);
    assert!(!poke_ok);
    match r {
        Poll::Ready(Err(crate::tasks::SendError::Closed)) => {}
        other => panic!("overlap async send must resolve Closed, got {other:?}"),
    }
    assert_eq!(
        drops.load(Ordering::SeqCst),
        1,
        "payload dropped exactly once"
    );
    assert_eq!(mb.queue_len(), 0);
    assert!(mb.pop().is_none());
    assert!(mb.is_closed());
    assert!(mb.take_wake_failure(), "the failure is latched");
    // the retained sender sees the same terminal state — Ready Err Closed
    {
        let fut = probe.send(DropPayload {
            id: 10,
            drops: drops.clone(),
        });
        let mut fut = std::pin::pin!(fut);
        let mut cx = Context::from_waker(std::task::Waker::noop());
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(Err(crate::tasks::SendError::Closed)) => {}
            other => panic!("post-terminal send must resolve Closed, got {other:?}"),
        }
    }
    assert_eq!(
        drops.load(Ordering::SeqCst),
        2,
        "the second payload also dropped exactly once"
    );
}

/// F01 — the same overlap on a SUCCEEDING lifecycle signal: the producer
/// serializes behind the in-flight call, accepts on the Confirmed edge
/// (no second invoke), and the payload delivers FIFO + drops once.
#[test]
fn lifecycle_inflight_success_serializes_then_overlap_send_ok() {
    use std::sync::mpsc;
    let mb = crate::tasks::Mailbox::new();
    let (entered_tx, entered_rx) = mpsc::channel::<()>();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let release_rx = Mutex::new(release_rx);
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let c2 = calls.clone();
    mb.install_wake(Arc::new(move || {
        c2.fetch_add(1, Ordering::SeqCst);
        let _ = entered_tx.send(());
        let _ = release_rx
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(5));
        true
    }))
    .unwrap();
    let mb2 = mb.clone();
    let poke = std::thread::spawn(move || mb2.poke_ui());
    entered_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("the lifecycle callback entered");

    let drops = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let (started_tx, started_rx) = mpsc::channel::<()>();
    let (done_tx, done_rx) = mpsc::channel();
    let mb3 = mb.clone();
    let d2 = drops.clone();
    let send = std::thread::spawn(move || {
        let p = crate::tasks::UiProxy::<DropPayload>::new(mb3);
        let _ = started_tx.send(());
        let r = p.try_send(DropPayload { id: 7, drops: d2 });
        let _ = done_tx.send(r);
    });
    started_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("the producer started its attempt");

    let early = done_rx.recv_timeout(Duration::from_millis(200));
    let locked = mb.state_try_locked();
    let _ = release_tx.send(());
    let poke_ok = poke.join().unwrap();
    let r = done_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    send.join().unwrap();
    assert!(
        early.is_err(),
        "overlap send resolved while the signal was still in flight"
    );
    assert!(locked);
    assert!(poke_ok, "the succeeding signal reports ok");
    assert_eq!(r, Ok(()));
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "the overlapped acceptance reuses the Confirmed edge"
    );
    // FIFO delivery of the exact payload — alive until actual dequeue
    assert_eq!(drops.load(Ordering::SeqCst), 0, "queued payload still live");
    let env = mb.pop().expect("the accepted envelope exists");
    let p = env.payload.downcast::<DropPayload>().unwrap();
    assert_eq!(p.id, 7);
    drop(p);
    assert!(mb.pop().is_none(), "exactly one envelope — no duplicates");
    assert_eq!(
        drops.load(Ordering::SeqCst),
        1,
        "payload dropped exactly once"
    );
}

/// F01 — the async success overlap: a real `TaskSender::send` polled on
/// the producer thread serializes behind the in-flight signal, resolves
/// `Ready(Ok)` on the Confirmed edge (callback count stays 1), and the
/// exact payload delivers with no duplicates and one drop.
#[test]
fn lifecycle_inflight_success_serializes_then_overlap_async_send_ok() {
    use std::sync::mpsc;
    let mb = crate::tasks::Mailbox::new();
    let (entered_tx, entered_rx) = mpsc::channel::<()>();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let release_rx = Mutex::new(release_rx);
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let c2 = calls.clone();
    mb.install_wake(Arc::new(move || {
        c2.fetch_add(1, Ordering::SeqCst);
        let _ = entered_tx.send(());
        let _ = release_rx
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(5));
        true
    }))
    .unwrap();
    let mb2 = mb.clone();
    let poke = std::thread::spawn(move || mb2.poke_ui());
    entered_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("the lifecycle callback entered");

    let mut reg = crate::tasks::TaskRegistry::default();
    let (job, _rid) = reg
        .spawn::<DropPayload>(&crate::tasks::ScopePath::root(), "overlap-ok", &mb)
        .unwrap();
    let sender = job.sender().clone();
    let drops = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let (started_tx, started_rx) = mpsc::channel::<()>();
    let (done_tx, done_rx) = mpsc::channel();
    let d2 = drops.clone();
    let send = std::thread::spawn(move || {
        let fut = sender.send(DropPayload { id: 9, drops: d2 });
        let mut fut = std::pin::pin!(fut);
        let mut cx = Context::from_waker(std::task::Waker::noop());
        let _ = started_tx.send(());
        let out = fut.as_mut().poll(&mut cx);
        let _ = done_tx.send(out);
    });
    started_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("the producer started its poll");

    let early = done_rx.recv_timeout(Duration::from_millis(200));
    let locked = mb.state_try_locked();
    let _ = release_tx.send(());
    let poke_ok = poke.join().unwrap();
    let r = done_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    send.join().unwrap();
    assert!(
        early.is_err(),
        "overlap async send resolved while the signal was in flight"
    );
    assert!(locked);
    assert!(poke_ok);
    match r {
        Poll::Ready(Ok(())) => {}
        other => panic!("overlap async send must resolve Ok, got {other:?}"),
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1, "Confirmed edge reused");
    // exact payload identity — one envelope, no duplicates, one drop
    let env = mb.pop().expect("the accepted envelope exists");
    let p = env.payload.downcast::<DropPayload>().unwrap();
    assert_eq!(p.id, 9);
    drop(p);
    assert!(mb.pop().is_none(), "no duplicate envelope");
    assert_eq!(
        drops.load(Ordering::SeqCst),
        1,
        "payload dropped exactly once"
    );
}

/// F01 — callback Arc destruction happens ONLY after the state lock
/// releases: a destructor captured in the wake closure may reenter
/// mailbox state. Proven on BOTH paths — the install-time failing pulse
/// (the just-installed callback detaches on terminal close) and seam
/// replacement (the previous callback detaches). The guard records
/// `state_try_locked` at destruction so a wrong drop site surfaces as an
/// assertion failure, not a deadlock.
#[test]
fn callback_destructor_runs_outside_the_state_lock() {
    struct ReentryGuard {
        mb: std::sync::Weak<crate::tasks::Mailbox>,
        hits: Arc<Mutex<Vec<bool>>>,
    }
    impl Drop for ReentryGuard {
        fn drop(&mut self) {
            if let Some(mb) = self.mb.upgrade() {
                let held = mb.state_try_locked();
                // assert BEFORE reentry — a regression must fail here,
                // not hang on a lock it would itself hold
                assert!(!held, "callback destructor must run after mailbox unlock");
                let _len = mb.queue_len();
                let _closed = mb.is_closed();
                self.hits.lock().unwrap().push(held);
            }
        }
    }
    // path A — failing install: the installed callback is detached by the
    // terminal close inside install_wake; its destructor must reenter a
    // FREE lock
    let mb = crate::tasks::Mailbox::new();
    let hits = Arc::new(Mutex::new(Vec::<bool>::new()));
    crate::tasks::UiProxy::<u32>::new(mb.clone())
        .try_send(1)
        .unwrap();
    let g = std::sync::Mutex::new(ReentryGuard {
        mb: Arc::downgrade(&mb),
        hits: hits.clone(),
    });
    assert_eq!(
        mb.install_wake(Arc::new(move || {
            let _g = g.lock().unwrap(); // keeps the guard's ownership in the cb
            false
        })),
        Err(crate::tasks::ProxySendError::Wake)
    );
    // force destruction of the captured guard now that the cb is detached
    assert_eq!(
        hits.lock().unwrap().as_slice(),
        &[false],
        "destructor reentered a free lock (Wake-failure detach)"
    );

    // path B — replacement: install seam A (holding the guard), then
    // install B — A's callback detaches and its destructor drops post-lock
    let mb2 = crate::tasks::Mailbox::new();
    let hits2 = Arc::new(Mutex::new(Vec::<bool>::new()));
    let g2 = std::sync::Mutex::new(ReentryGuard {
        mb: Arc::downgrade(&mb2),
        hits: hits2.clone(),
    });
    mb2.install_wake(Arc::new(move || {
        let _g = g2.lock().unwrap();
        true
    }))
    .unwrap();
    crate::tasks::UiProxy::<u32>::new(mb2.clone())
        .try_send(5)
        .unwrap();
    mb2.install_wake(Arc::new(|| true)).unwrap();
    assert_eq!(
        hits2.lock().unwrap().as_slice(),
        &[false],
        "replaced callback destroyed outside the lock"
    );
}
