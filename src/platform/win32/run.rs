use super::syscolor::effective_reduced;
use super::*;

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
        trans: std::cell::RefCell::new(HashMap::new()),
        shown: std::cell::RefCell::new(HashMap::new()),
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
    // Atomic DWM reveal:
    // 1. Presenting to ID2D1HwndRenderTarget (software/GDI target) while
    //    !WS_VISIBLE is clipped to an empty visrgn, and ShowWindow(SW_SHOW)
    //    does not synchronously dispatch WM_PAINT — so calling DwmFlush()
    //    immediately after ShowWindow without a post-show paint exposed an
    //    unpainted redirection surface for ~1 VBlank (~1/5 launches).
    // 2. Cloaking (DWMWA_CLOAK = 13) before ShowWindow makes WS_VISIBLE
    //    true (valid visrgn + live DWM surface) while keeping the window
    //    invisible to the compositor until the post-show D2D paint lands.
    // The hidden paint inside guarded_turn() was clipped to an empty
    // visrgn; clear committed_ink so ShowWindow's synchronous WM_ERASEBKGND
    // fills the surface-matched class brush (0x18181B dark / 0xFAFAFA light)
    // before the post-show D2D paint commits.
    backend.committed_ink.borrow_mut().clear();
    unsafe {
        const DWMWA_TRANSITIONS_FORCEDISABLED: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(3);
        const DWMWA_CLOAK: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(13);
        let on: windows_core::BOOL = windows_core::BOOL(1);
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_TRANSITIONS_FORCEDISABLED,
            &on as *const _ as *const std::ffi::c_void,
            std::mem::size_of_val(&on) as u32,
        );
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_CLOAK,
            &on as *const _ as *const std::ffi::c_void,
            std::mem::size_of_val(&on) as u32,
        );
        let _ = windows::Win32::UI::WindowsAndMessaging::ShowWindow(
            hwnd,
            windows::Win32::UI::WindowsAndMessaging::SW_SHOW,
        );
    }
    // Re-assert non-client dark caption/border and present the first D2D
    // frame into the now-visible (WS_VISIBLE) HWND both before and
    // immediately after uncloaking (text layouts and shadow rasters are
    // already cached, so the post-uncloak present is a ~1ms blit).
    backend.dark_caption.set(None);
    backend.sync_dark_caption(backend.rt.theme.dark);
    backend.in_dispatch.set(true);
    backend.renderer.borrow_mut().resize();
    let reveal_paint = backend.paint();
    unsafe {
        const DWMWA_CLOAK: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(13);
        let off: windows_core::BOOL = windows_core::BOOL(0);
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_CLOAK,
            &off as *const _ as *const std::ffi::c_void,
            std::mem::size_of_val(&off) as u32,
        );
    }
    let reveal_paint = reveal_paint.and_then(|()| backend.paint());
    backend.in_dispatch.set(false);
    if let Err(e) = reveal_paint.and_then(|()| backend.drain_reentrant()) {
        return bail(&mut backend, e);
    }
    unsafe {
        let _ = windows::Win32::Graphics::Gdi::ValidateRect(Some(hwnd), None);
        let _ = windows::Win32::Graphics::Dwm::DwmFlush();
    }

    #[cfg(feature = "devtools")]
    {
        backend.devtools = match crate::devtools::server::Server::start(
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
