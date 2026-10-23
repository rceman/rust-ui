impl HostBox {
    fn s(&self) -> std::cell::Ref<'_, PeerShared> {
        self.state.borrow()
    }
    fn m(&self) -> std::cell::RefMut<'_, PeerShared> {
        self.state.borrow_mut()
    }
}

impl ITextHost_Impl for HostBox {
    fn TxGetDC(&self) -> HDC {
        let hwnd = self.s().host.hwnd;
        if hwnd.is_invalid() {
            // GetDC(NULL) would return the whole-screen DC — never hand
            // that out; a null HDC is the documented "unavailable" answer
            return HDC::default();
        }
        unsafe { GetDC(Some(hwnd)) }
    }
    fn TxReleaseDC(&self, hdc: HDC) -> i32 {
        let hwnd = self.s().host.hwnd;
        unsafe { ReleaseDC(Some(hwnd), hdc) }
    }
    fn TxShowScrollBar(&self, _fnbar: i32, _fshow: BOOL) -> BOOL {
        BOOL(0)
    }
    fn TxEnableScrollBar(&self, _fusbflags: SCROLLBAR_CONSTANTS, _fuarrowflags: i32) -> BOOL {
        BOOL(0)
    }
    fn TxSetScrollRange(&self, _fnbar: i32, _nminpos: i32, _nmaxpos: i32, _fredraw: BOOL) -> BOOL {
        BOOL(0)
    }
    fn TxSetScrollPos(&self, _fnbar: i32, _npos: i32, _fredraw: BOOL) -> BOOL {
        BOOL(0)
    }
    fn TxInvalidateRect(&self, _prc: *mut RECT, _fmode: BOOL) {
        self.m().host.ev(HostEvent::Invalidate);
    }
    fn TxViewChange(&self, _fupdate: BOOL) {
        self.m().host.ev(HostEvent::Invalidate);
    }
    fn TxCreateCaret(&self, _hbmp: HBITMAP, xwidth: i32, yheight: i32) -> BOOL {
        let mut s = self.m();
        s.host.caret_size = SIZE {
            cx: xwidth,
            cy: yheight,
        };
        s.host.caret_created = true;
        s.host.ev(HostEvent::Caret);
        BOOL(1)
    }
    fn TxShowCaret(&self, fshow: BOOL) -> BOOL {
        let mut s = self.m();
        s.host.caret_shown = fshow.as_bool();
        s.host.ev(HostEvent::Caret);
        s.host.ev(HostEvent::Invalidate);
        BOOL(1)
    }
    fn TxSetCaretPos(&self, x: i32, y: i32) -> BOOL {
        let mut s = self.m();
        s.host.caret_pos = POINT { x, y };
        s.host.ev(HostEvent::Caret);
        s.host.ev(HostEvent::Invalidate);
        BOOL(1)
    }
    /// richedit arms a native timer — synchronously, because the BOOL
    /// answer is the contract: FALSE means "no timer exists" and msftedit
    /// must not wait on a tick that will never come. The win32 id is
    /// allocated from the shared pool (collision-free while live); both
    /// capacity exhaustion and a SetTimer failure return FALSE.
    fn TxSetTimer(&self, idtimer: u32, utimeout: u32) -> BOOL {
        // re-arming the same logical (node,idtimer) RESETS the existing
        // win32 id — SetTimer on a live id replaces it; allocating a
        // second id would orphan a periodic timer that keeps firing
        let (hwnd, node, pool, existing) = {
            let s = self.s();
            (
                s.host.hwnd,
                s.host.node,
                s.host.timer_pool.clone(),
                s.host.armed_timers.get(&idtimer).copied(),
            )
        };
        let Some(node) = node else {
            return BOOL(0); // unattached host cannot route a timer
        };
        if hwnd.is_invalid() {
            return BOOL(0);
        }
        let tid = match existing {
            Some(tid) => tid,
            None => match pool.lock().unwrap().alloc() {
                Some(tid) => tid,
                None => return BOOL(0), // capacity exhausted — honest failure
            },
        };
        let armed = unsafe {
            windows::Win32::UI::WindowsAndMessaging::SetTimer(Some(hwnd), tid, utimeout, None)
        };
        if armed == 0 {
            if existing.is_none() {
                // never-armed id: tombstone it so a stray tick can't bind
                pool.lock().unwrap().kill(tid);
            }
            return BOOL(0);
        }
        pool.lock().unwrap().arm(tid, node, idtimer);
        self.m().host.armed_timers.insert(idtimer, tid);
        BOOL(1)
    }
    fn TxKillTimer(&self, idtimer: u32) {
        let (hwnd, tid, pool) = {
            let mut s = self.m();
            let Some(tid) = s.host.armed_timers.remove(&idtimer) else {
                return;
            };
            (s.host.hwnd, tid, s.host.timer_pool.clone())
        };
        // tombstone, not erase — a WM_TIMER queued before KillTimer must
        // still resolve to a dead owner
        pool.lock().unwrap().kill(tid);
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::KillTimer(Some(hwnd), tid);
        }
    }
    fn TxScrollWindowEx(
        &self,
        _dx: i32,
        _dy: i32,
        _lprcscroll: *mut RECT,
        _lprcclip: *mut RECT,
        _hrgnupdate: HRGN,
        _lprcupdate: *mut RECT,
        _fuscroll: SCROLL_WINDOW_FLAGS,
    ) {
        self.m().host.ev(HostEvent::Invalidate);
    }
    fn TxSetCapture(&self, fcapture: BOOL) {
        self.m().host.ev(HostEvent::Capture(fcapture.as_bool()));
    }
    fn TxSetFocus(&self) {
        let hwnd = self.s().host.hwnd;
        if !hwnd.is_invalid() {
            unsafe {
                let _ = SetFocus(Some(hwnd));
            }
        }
    }
    fn TxSetCursor(&self, hcur: HCURSOR, _ftext: BOOL) {
        unsafe {
            let _ = SetCursor(Some(hcur));
        }
    }
    fn TxScreenToClient(&self, lppt: *mut POINT) -> BOOL {
        let (scale, bounds, hwnd) = {
            let s = self.s();
            (s.host.scale, s.host.bounds, s.host.hwnd)
        };
        unsafe {
            // screen px -> window-client px -> peer-local px — the SHARED
            // snapped-origin transform, not a re-rounded logical subtract
            let Some(c) = super::space::screen_to_client(
                hwnd,
                super::space::ScreenPhysicalPoint((*lppt).into()),
            ) else {
                return BOOL(0); // failed conversion is failure, not identity
            };
            let Some(origin) = super::space::PeerOrigin::from_logical(bounds, scale) else {
                return BOOL(0); // unrepresentable origin is failure
            };
            let Some(local) = origin.to_local(c) else {
                return BOOL(0); // unrepresentable coordinate is failure
            };
            (*lppt).x = local.0.x;
            (*lppt).y = local.0.y;
            BOOL(1)
        }
    }
    fn TxClientToScreen(&self, lppt: *mut POINT) -> BOOL {
        let (scale, bounds, hwnd) = {
            let s = self.s();
            (s.host.scale, s.host.bounds, s.host.hwnd)
        };
        unsafe {
            let Some(origin) = super::space::PeerOrigin::from_logical(bounds, scale) else {
                return BOOL(0); // unrepresentable origin is failure
            };
            let Some(client) = origin.to_client(super::space::PeerLocalPoint((*lppt).into()))
            else {
                return BOOL(0); // unrepresentable coordinate is failure
            };
            let Some(s) = super::space::client_to_screen(hwnd, client) else {
                return BOOL(0);
            };
            *lppt = s.0.into();
            BOOL(1)
        }
    }
    fn TxActivate(&self, _ploldstate: *mut i32) -> Result<()> {
        Ok(())
    }
    fn TxDeactivate(&self, _lnewstate: i32) -> Result<()> {
        Ok(())
    }
    fn TxGetClientRect(&self, prc: *mut RECT) -> Result<()> {
        unsafe {
            // local PX rect — host units are physical px under the host
            // DC's dpi (DIP-only when the process is DPI-unaware at 96)
            let (sc, b) = {
                let s = self.s();
                (s.host.scale, s.host.bounds)
            };
            *prc = LogicalRect::local(b.width, b.height).physical(sc).into();
        }
        Ok(())
    }
    fn TxGetViewInset(&self, prc: *mut RECT) -> Result<()> {
        unsafe {
            // content padding lives in the bitmap's destination rect —
            // the format space itself gets no inset
            *prc = RECT::default();
        }
        Ok(())
    }
    fn TxGetCharFormat(&self, ppcf: *const *const CHARFORMATW) -> Result<()> {
        unsafe {
            *(ppcf as *mut *const CHARFORMATW) = &*self.s().host.cf;
        }
        Ok(())
    }
    fn TxGetParaFormat(&self, pppf: *const *const PARAFORMAT) -> Result<()> {
        unsafe {
            // the Base member sits at offset 0 — msftedit reads cbSize and
            // continues into the v2 tail inside our box
            *(pppf as *mut *const PARAFORMAT) = &self.s().host.pf.Base;
        }
        Ok(())
    }
    fn TxGetSysColor(&self, nindex: SYS_COLOR_INDEX) -> COLORREF {
        let s = self.s();
        match nindex {
            COLOR_WINDOWTEXT => s.host.fg,
            COLOR_HIGHLIGHT => s.host.sel_bg,
            COLOR_HIGHLIGHTTEXT => s.host.sel_fg,
            COLOR_GRAYTEXT => s.host.fg,
            // real field color, not 0 — a #000 selection band equals
            // COLOR_WINDOW=0 and msftedit then skips drawing it
            COLOR_WINDOW => s.host.win_bg,
            _ => unsafe { COLORREF(GetSysColor(nindex)) },
        }
    }
    fn TxGetBackStyle(&self, pstyle: *mut TXTBACKSTYLE) -> Result<()> {
        unsafe {
            *pstyle = TXTBACK_TRANSPARENT;
        }
        Ok(())
    }
    fn TxGetMaxLength(&self, plength: *mut u32) -> Result<()> {
        unsafe {
            *plength = 4096;
        }
        Ok(())
    }
    fn TxGetScrollBars(&self, pdwscrollbar: *mut u32) -> Result<()> {
        unsafe {
            *pdwscrollbar = 0;
        }
        Ok(())
    }
    fn TxGetPasswordChar(&self) -> Result<i8> {
        Ok(0)
    }
    fn TxGetAcceleratorPos(&self, pcp: *mut i32) -> Result<()> {
        unsafe {
            *pcp = -1;
        }
        Ok(())
    }
    fn TxGetExtent(&self, lpextent: *mut SIZE) -> Result<()> {
        let b = self.s().host.bounds;
        // unit exception, local and documented: the extent is HIMETRIC
        // (1/100 mm — neither DIP nor px). dip*2540/96 is exact because
        // host-px * 2540/dcDpi cancels the scale factor.
        let hm = 2540.0 / 96.0;
        unsafe {
            *lpextent = SIZE {
                cx: (b.width * hm).round() as i32,
                cy: (b.height * hm).round() as i32,
            };
        }
        Ok(())
    }
    fn OnTxCharFormatChange(&self, pcf: *const CHARFORMATW) -> Result<()> {
        unsafe {
            *self.m().host.cf = *pcf;
        }
        Ok(())
    }
    fn OnTxParaFormatChange(&self, pppf: *const PARAFORMAT) -> Result<()> {
        unsafe {
            // source may be v1 or v2 — copy only what it actually carries
            // (a blind v2 read could walk past a v1 payload)
            let n = ((*pppf).cbSize as usize).min(std::mem::size_of::<PARAFORMAT2>());
            std::ptr::copy_nonoverlapping(
                pppf as *const u8,
                &mut *self.m().host.pf as *mut PARAFORMAT2 as *mut u8,
                n,
            );
        }
        Ok(())
    }
    fn TxGetPropertyBits(&self, dwmask: u32, pdwbits: *mut u32) -> Result<()> {
        unsafe {
            *pdwbits = self.s().host.bits & dwmask;
        }
        Ok(())
    }
    fn TxNotify(&self, inotify: u32, pv: *mut c_void) -> Result<()> {
        if inotify == EN_CHANGE_CODE {
            self.m().host.ev(HostEvent::Change);
        } else if inotify == EN_SELCHANGE_CODE {
            self.m().host.ev(HostEvent::SelChange);
        } else if inotify == EN_REQUESTRESIZE_CODE && !pv.is_null() {
            #[repr(C)]
            struct ReqResize {
                _nm: [usize; 3],
                rc: RECT,
            }
            let rr = unsafe { std::ptr::read_unaligned(pv as *const ReqResize) };
            self.m().host.natural = SIZE {
                cx: rr.rc.right - rr.rc.left,
                cy: rr.rc.bottom - rr.rc.top,
            };
        }
        Ok(())
    }
    fn TxImmGetContext(&self) -> HIMC {
        let hwnd = self.s().host.hwnd;
        unsafe { ImmGetContext(hwnd) }
    }
    fn TxImmReleaseContext(&self, himc: HIMC) {
        let hwnd = self.s().host.hwnd;
        unsafe {
            let _ = ImmReleaseContext(hwnd, himc);
        }
    }
    fn TxGetSelectionBarWidth(&self, lselbarwidth: *mut i32) -> Result<()> {
        unsafe {
            *lselbarwidth = 0;
        }
        Ok(())
    }
}

impl ITextHost2_Impl for HostBox {
    fn TxIsDoubleClickPending(&self) -> BOOL {
        BOOL(0)
    }
    fn TxGetWindow(&self, phwnd: *mut HWND) -> Result<()> {
        unsafe {
            *phwnd = self.s().host.hwnd;
        }
        Ok(())
    }
    fn TxSetForegroundWindow(&self) -> Result<()> {
        Ok(())
    }
    fn TxGetPalette(&self) -> HPALETTE {
        HPALETTE::default()
    }
    fn TxGetEastAsianFlags(&self, pflags: *mut i32) -> Result<()> {
        unsafe {
            *pflags = 0;
        }
        Ok(())
    }
    fn TxSetCursor2(&self, hcur: HCURSOR, _btext: BOOL) -> HCURSOR {
        unsafe {
            let _ = SetCursor(Some(hcur));
            hcur
        }
    }
    fn TxFreeTextServicesNotification(&self) {}
    fn TxGetEditStyle(&self, _dwitem: u32, pdwdata: *mut u32) -> Result<()> {
        unsafe {
            *pdwdata = 0;
        }
        Ok(())
    }
    fn TxGetWindowStyles(&self, pdwstyle: *mut u32, pdwexstyle: *mut u32) -> Result<()> {
        unsafe {
            *pdwstyle = WS_CHILD.0 | WS_VISIBLE.0 | ES_MULTILINE as u32;
            *pdwexstyle = 0;
        }
        Ok(())
    }
    fn TxShowDropCaret(&self, _fshow: BOOL, _hdc: HDC, _prc: *mut RECT) -> Result<()> {
        Err(E_NOTIMPL.into())
    }
    fn TxDestroyCaret(&self) -> Result<()> {
        Ok(())
    }
    fn TxGetHorzExtent(&self, plhorzextent: *mut i32) -> Result<()> {
        unsafe {
            *plhorzextent = 0;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// the peer itself
// ---------------------------------------------------------------------------

