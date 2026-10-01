//! Windowless RichEdit peer: `ITextHost2` host + `ITextServices2` peer drawn
//! into the window's Direct2D frame (`TxDrawD2D`).
//!
//! IID notes: windows-rs declares `ITextHost`, `ITextHost2`, `ITextServices`
//! and `ITextServices2` with a zero IID (implement-only). The real interface
//! IDs only exist as data exports inside msftedit.dll. [`Msftedit`] loads
//! them; the host's `QueryInterface` answers them explicitly, otherwise
//! msftedit never sees `ITextHost2` and the D2D path stays off.
//!
//! Units: the entire host coordinate space is **DIP**. `TxGetClientRect`/
//! Host units are PHYSICAL PIXELS at the host DC's DPI — verified
//! empirically: under a DPI-aware process msftedit divides `lprcBounds`
//! by `dcDpi/96` to get target-logical units (at 125% DIP bounds rendered
//! at arg/1.25); under an unaware process dcDpi=96 makes px==DIP. The
//! peer's local space (`TxGetClientRect`, `lprcBounds`, caret, lparams)
//! is therefore px; `host.bounds`/`draw`/`natural_size` keep DIP at the
//! Rust boundary and convert at the seam. `TxGetExtent` is HIMETRIC
//! (`himetric = px * 2540/dcDpi = dip * 2540/96`).
//!
//! Object layout: msftedit assumes a C++ single-inheritance host — it calls
//! `ITextHost2` methods through the `ITextHost` pointer. The host carries ONE
//! vtbl (the full `ITextHost2` vtbl, OFFSET 0); `QueryInterface` answers
//! `IUnknown`/`ITextHost`/`ITextHost2` all with `base+0`.

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::sync::Arc;

use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct2D::ID2D1RenderTarget;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::UI::Controls::EM_LIMITTEXT;
use windows::Win32::UI::Controls::RichEdit::*;
use windows::Win32::UI::Input::Ime::*;
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::*;

use super::space::{LogicalPoint, LogicalRect, PhysicalPoint, ScaleFactor};
use crate::text::{BindingToken, TextRevision};
use crate::{NodeId, UiError, UiResult};

/// missing from the windows bindings

/// missing from the windows bindings
const EM_GETLINECOUNT: u32 = 0x00BA; // standard edit message
/// RichEdit notifications we route (missing from bindings).
const EN_CHANGE_CODE: u32 = 0x0300;
const EN_REQUESTRESIZE_CODE: u32 = 0x0701;
const EN_SELCHANGE_CODE: u32 = 0x0702;
/// native timer ids are namespaced per peer slot: slot<<16 | richedit-id
pub(crate) const TIMER_KIND_NATIVE: u64 = 0x4e54494d_0000_0000;

const fn colorref(c: [f32; 4]) -> COLORREF {
    COLORREF(
        ((c[0] * 255.0 + 0.5) as u32)
            | (((c[1] * 255.0 + 0.5) as u32) << 8)
            | (((c[2] * 255.0 + 0.5) as u32) << 16),
    )
}

// ---------------------------------------------------------------------------
// msftedit.dll loader — module handle, entry point, exported IIDs
// ---------------------------------------------------------------------------

pub(crate) type CreateTextServicesFn =
    unsafe extern "system" fn(*mut c_void, *mut c_void, *mut *mut c_void) -> HRESULT;

pub(crate) struct Msftedit {
    pub module: HMODULE,
    create_text_services: CreateTextServicesFn,
    pub iid_text_services: GUID,
    pub iid_text_services2: GUID,
    pub iid_text_host: GUID,
    pub iid_text_host2: GUID,
    /// `IID_IRicheditWindowlessAccessibility` — exported like the rest
    pub iid_windowless_acc: Option<GUID>,
}

// A loaded module handle + entry points are immutable and safe to share.
unsafe impl Send for Msftedit {}
unsafe impl Sync for Msftedit {}

impl Msftedit {
    /// Load msftedit + resolve entry points. `OleInitialize` is the caller's
    /// responsibility (per UI thread via the backend's RAII guard).
    pub(crate) fn load() -> UiResult<Arc<Msftedit>> {
        unsafe {
            let module = LoadLibraryExW(w!("msftedit.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32)
                .map_err(|e| UiError::Platform(format!("LoadLibraryExW(msftedit.dll): {e}")))?;
            let data_iid = |name: &std::ffi::CStr| -> UiResult<GUID> {
                let p = GetProcAddress(module, PCSTR(name.as_ptr() as *const u8));
                if p.is_none() {
                    return Err(UiError::Platform(format!("missing export {name:?}")));
                }
                Ok(*(p.unwrap() as *const GUID))
            };
            let create = GetProcAddress(module, s!("CreateTextServices"));
            let create =
                create.ok_or_else(|| UiError::Platform("no CreateTextServices export".into()))?;
            Ok(Arc::new(Msftedit {
                module,
                create_text_services: std::mem::transmute::<*const u8, CreateTextServicesFn>(
                    create as *const u8,
                ),
                iid_text_services: data_iid(c"IID_ITextServices")?,
                iid_text_services2: data_iid(c"IID_ITextServices2")?,
                iid_text_host: data_iid(c"IID_ITextHost")?,
                iid_text_host2: data_iid(c"IID_ITextHost2")?,
                iid_windowless_acc: data_iid(c"IID_IRicheditWindowlessAccessibility").ok(),
            }))
        }
    }
}

// ---------------------------------------------------------------------------
// host events the peer drains after every text-services call
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HostEvent {
    /// text services invalidated its view
    Invalidate,
    /// EN_CHANGE: committed content changed
    Change,
    /// EN_SELCHANGE: selection changed
    SelChange,
    /// richedit wants mouse capture set/released
    Capture(bool),
    /// caret geometry or visibility changed (system caret)
    Caret,
    /// richedit wants focus moved to the owner window
    GrabFocus,
}

/// Everything `TxGet*` reads back from the host.
pub(crate) struct HostShared {
    /// owning window (IME context, timers, capture)
    pub hwnd: HWND,
    /// global bounds in window DIP (for ScreenToClient conversions)
    pub bounds: LogicalRect,
    /// px-per-DIP — the ONLY scale authority (see space.rs)
    pub scale: ScaleFactor,
    /// property bits advertised via TxGetPropertyBits
    pub bits: u32,
    /// latest EN_REQUESTRESIZE size — client units are PHYSICAL px
    /// (the host space; see space.rs); divide by scale for DIP
    pub natural: SIZE,
    pub cf: Box<CHARFORMATW>,
    pub pf: Box<PARAFORMAT>,
    pub fg: COLORREF,
    /// the AUTHORED foreground (Color, not a resolved COLORREF) — theme
    /// flips re-resolve roles through it; a literal `Color::Rgba` is
    /// invariant under resolve and so correctly survives flips
    pub fg_authored: crate::style::Color,
    pub sel_bg: COLORREF,
    pub sel_fg: COLORREF,
    /// system-caret state recorded from Tx*Caret calls
    pub caret_pos: POINT,
    pub caret_size: SIZE,
    pub caret_shown: bool,
    pub caret_created: bool,
    /// richedit-requested timers armed by THIS host (richedit id ->
    /// win32 id) — KillTimer/Drop resolve through it; the armed win32
    /// ids live in the shared `timer_pool` (existence + routing owner)
    pub armed_timers: std::collections::BTreeMap<u32, usize>,
    /// this host's node — assigned at attach; timer routing needs it
    pub node: Option<crate::NodeId>,
    /// THE armed-timer authority — win32 id -> (owner, richedit id);
    /// shared by every peer on the window
    pub timer_pool:
        std::sync::Arc<std::sync::Mutex<std::collections::HashMap<usize, (crate::NodeId, u32)>>>,
    /// monotonically increasing allocator sequence (skip-live policy)
    pub timer_seq: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    /// bounded host-event log — same 128 bound as the runtime event queue;
    /// overflow is a QueueOverflow failure, not a silent drop
    pub events: Vec<HostEvent>,
    /// event buffer overflowed — next drain returns QueueOverflow
    pub overflow: bool,
    /// the host's read-only state gate
    pub read_only: bool,
}

impl HostShared {
    /// bounded push — an overflow is flagged, not silently dropped; the next
    /// `drain` surfaces it as `QueueOverflow`
    fn ev(&mut self, e: HostEvent) {
        if self.events.len() >= crate::event::EVENT_QUEUE_CAP {
            self.overflow = true;
            return;
        }
        self.events.push(e);
    }
}

fn bools(v: bool) -> BOOL {
    BOOL::from(v)
}

/// Allocate a collision-free win32 timer id — scans forward from the
/// sequence and skips still-live ids; `None` when the namespace is full
/// (capacity is honestly reported to msftedit, never silently dropped).
pub(crate) fn alloc_native_timer(
    pool: &std::sync::Arc<std::sync::Mutex<std::collections::HashMap<usize, (crate::NodeId, u32)>>>,
    seq: &std::sync::Arc<std::sync::atomic::AtomicUsize>,
) -> Option<usize> {
    use super::window::{NATIVE_TIMER_BASE, NATIVE_TIMER_CAP};
    let p = pool.lock().unwrap();
    for _ in 0..NATIVE_TIMER_CAP {
        let n = seq.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tid = NATIVE_TIMER_BASE + (n % NATIVE_TIMER_CAP);
        if !p.contains_key(&tid) {
            return Some(tid);
        }
    }
    None
}

/// Explicit TextServices activation state — in-place activation (draw/
/// measure) and UI activation (focus-owned UI: caret, selection visibility,
/// IME target) are SEPARATE contracts; a measured peer is never implicitly
/// UI-active.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum Activation {
    Inactive,
    InPlace,
    Ui,
}

/// Peer-visible state the host callbacks need — the msftedit reference is
/// kept per peer so the DLL ref guard outlives every COM object the peer
/// built.
pub(crate) struct PeerShared {
    pub lib: Arc<Msftedit>,
    pub host: HostShared,
}

/// COM object handed to `CreateTextServices`.
#[repr(C)]
pub(crate) struct HostBox {
    host: &'static ITextHost_Vtbl,
    host2: &'static ITextHost2_Vtbl,
    refs: Cell<u32>,
    state: RefCell<PeerShared>,
}

// Single-inheritance layout: slot 0 carries the full ITextHost2 vtbl with
// OFFSET 0 so both ITextHost and ITextHost2 pointers are base+0.
static VT_HOST2: ITextHost2_Vtbl = ITextHost2_Vtbl::new::<HostBox, 0>();

impl IUnknownImpl for HostBox {
    type Impl = HostBox;
    fn get_impl(&self) -> &Self::Impl {
        self
    }
    fn get_impl_mut(&mut self) -> &mut Self::Impl {
        self
    }
    fn into_inner(self) -> Self::Impl {
        self
    }
    unsafe fn QueryInterface(&self, iid: *const GUID, out: *mut *mut c_void) -> HRESULT {
        unsafe {
            *out = std::ptr::null_mut();
            let iids = {
                let s = self.s();
                (s.lib.iid_text_host, s.lib.iid_text_host2)
            };
            let base = self as *const HostBox as *mut c_void;
            if *iid == IUnknown::IID || *iid == iids.0 || *iid == iids.1 {
                // single-inheritance: one vtbl serves all three IIDs
                *out = base;
            } else {
                return E_NOINTERFACE;
            }
            self.AddRef();
            S_OK
        }
    }
    fn AddRef(&self) -> u32 {
        self.refs.set(self.refs.get() + 1);
        self.refs.get()
    }
    unsafe fn Release(this: *mut Self) -> u32 {
        unsafe {
            let b = &*this;
            let n = b.refs.get() - 1;
            b.refs.set(n);
            if n == 0 {
                drop(Box::from_raw(this));
            }
            n
        }
    }
    fn is_reference_count_one(&self) -> bool {
        self.refs.get() == 1
    }
    fn to_object(&self) -> ComObject<HostBox>
    where
        HostBox: ComObjectInner<Outer = HostBox>,
    {
        unimplemented!("HostBox is not a ComObject")
    }
    unsafe fn GetTrustLevel(&self, value: *mut i32) -> HRESULT {
        unsafe {
            if !value.is_null() {
                *value = 0;
            }
        }
        S_OK
    }
}

impl ComObjectInner for HostBox {
    type Outer = HostBox;
    fn into_object(self) -> ComObject<Self> {
        unimplemented!("HostBox is constructed manually")
    }
}

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
        let (hwnd, node, pool, seq) = {
            let s = self.s();
            (
                s.host.hwnd,
                s.host.node,
                s.host.timer_pool.clone(),
                s.host.timer_seq.clone(),
            )
        };
        let Some(node) = node else {
            return BOOL(0); // unattached host cannot route a timer
        };
        if hwnd.is_invalid() {
            return BOOL(0);
        }
        let Some(tid) = alloc_native_timer(&pool, &seq) else {
            return BOOL(0); // capacity exhausted — honest failure
        };
        let armed = unsafe {
            windows::Win32::UI::WindowsAndMessaging::SetTimer(Some(hwnd), tid, utimeout, None)
        };
        if armed == 0 {
            pool.lock().unwrap().remove(&tid);
            return BOOL(0);
        }
        pool.lock().unwrap().insert(tid, (node, idtimer));
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
        pool.lock().unwrap().remove(&tid);
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
            let origin = super::space::PeerOrigin::from_logical(bounds, scale);
            let local = origin.to_local(c);
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
            let origin = super::space::PeerOrigin::from_logical(bounds, scale);
            let client = origin.to_client(super::space::PeerLocalPoint((*lppt).into()));
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
            *(pppf as *mut *const PARAFORMAT) = &*self.s().host.pf;
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
            COLOR_WINDOW => COLORREF(0), // TXTBACK_TRANSPARENT — we paint
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
            *self.m().host.pf = *pppf;
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

/// One windowless RichEdit instance — the `TextPeer` implementation the
/// backend's `peer_factory` produces. UI-thread only.
pub(crate) struct WindowlessPeer {
    tx: Option<ITextServices2>,
    host: *mut HostBox,
    /// opaque peer id for the runtime's logging/diagnostics
    id: u64,
    /// owning node (generation-checked when routing events back)
    node: NodeId,
    /// the peer mirror revision: last applied/committed revision the CORE
    /// agreed on — native edits tag their base with it
    peer_rev: Cell<TextRevision>,
    /// mount binding — tags every emitted edit/conflict
    binding: Option<BindingToken>,
    /// ui-side sink the backend drains each turn (node->event)
    sink: std::sync::Arc<std::sync::Mutex<Vec<super::NativeSinkItem>>>,
    /// multiline vs single-line init
    multiline: bool,
    /// THE TextServices activation state — NOT one boolean (F04):
    /// InPlace = formatted against the px client space (measure/draw);
    /// Ui = additionally UI-active — focus owner only (msftedit allows a
    /// single UI-active control per container).
    activation: Cell<Activation>,
    /// msftedit-derived single-line height in DIP — `natural_size` reports
    /// cy = line_count × line; dividing by EM_GETLINECOUNT gives the real
    /// metric regardless of content. 0 = unmeasured.
    line_h: Cell<f32>,
}

impl WindowlessPeer {
    fn shared(&self) -> std::cell::Ref<'_, PeerShared> {
        unsafe { (*self.host).state.borrow() }
    }
    fn shared_mut(&self) -> std::cell::RefMut<'_, PeerShared> {
        unsafe { (*self.host).state.borrow_mut() }
    }
    fn tx(&self) -> &ITextServices2 {
        self.tx.as_ref().unwrap()
    }

    /// owning node id (set by `attach` after mount)
    pub(crate) fn node(&self) -> NodeId {
        self.node
    }
    pub(crate) fn node_opt(&self) -> Option<NodeId> {
        (self.node.slot != u32::MAX).then_some(self.node)
    }
    /// the peer mirror — last core-agreed revision for native base tags
    pub(crate) fn peer_rev(&self) -> TextRevision {
        self.peer_rev.get()
    }
    pub(crate) fn binding(&self) -> Option<BindingToken> {
        self.binding
    }
    /// the core accepted a native commit — advance the mirror
    pub(crate) fn record_commit(&self, rev: TextRevision) {
        self.peer_rev.set(rev);
    }
    /// exported windowless-accessibility IID (None when msftedit lacks it)
    pub(crate) fn windowless_acc_iid(&self) -> Option<windows::core::GUID> {
        self.shared().lib.iid_windowless_acc
    }

    /// Re-style live content: patch the host default char format and push
    /// it over ALL existing text. The stack copy is what EM_SETCHARFORMAT
    /// consumes — the host borrow never crosses a `send` (msftedit calls
    /// back into the host synchronously).
    /// Push the resolved editable foreground over ALL content. The only
    /// style a native peer accepts — size/weight/face stay OS-owned.
    /// `authored` is the consumer's `Color` — roles re-resolve on theme
    /// flips; concrete literals are invariant by construction.
    pub(crate) fn apply_format(&mut self, fg: COLORREF, authored: crate::style::Color) {
        let cf = {
            let mut sh = self.shared_mut();
            sh.host.fg_authored = authored;
            sh.host.cf.crTextColor = fg;
            sh.host.fg = fg;
            *sh.host.cf
        };
        let _ = self.send(EM_SETCHARFORMAT, SCF_ALL as usize, &cf as *const _ as isize);
    }

    /// Build the host + text services for one peer.
    pub(crate) fn create(
        id: u64,
        lib: &Arc<Msftedit>,
        hwnd: HWND,
        scale: ScaleFactor,
        cfg: &PeerConfig,
        sink: std::sync::Arc<std::sync::Mutex<Vec<super::NativeSinkItem>>>,
        timer_pool: std::sync::Arc<
            std::sync::Mutex<std::collections::HashMap<usize, (crate::NodeId, u32)>>,
        >,
        timer_seq: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    ) -> UiResult<WindowlessPeer> {
        let mut face = [0u16; 32];
        for (i, c) in cfg.face.encode_utf16().take(31).enumerate() {
            face[i] = c;
        }
        let shared = PeerShared {
            lib: lib.clone(),
            host: HostShared {
                hwnd,
                bounds: LogicalRect::default(),
                scale,
                bits: TXTBIT_WORDWRAP
                    | TXTBIT_AUTOWORDSEL
                    | TXTBIT_DISABLEDRAG
                    | TXTBIT_D2DDWRITE
                    | TXTBIT_D2DPIXELSNAPPED
                    | if cfg.multiline { TXTBIT_MULTILINE } else { 0 }
                    | if cfg.read_only { TXTBIT_READONLY } else { 0 },
                natural: SIZE::default(),
                cf: Box::new(CHARFORMATW {
                    cbSize: std::mem::size_of::<CHARFORMATW>() as u32,
                    dwMask: CFM_ALL,
                    // CFE_AUTOCOLOR: draw-time theme recoloring without
                    // rewriting character formats on every view
                    dwEffects: CFE_EFFECTS(CFE_AUTOCOLOR.0 | if cfg.bold { CFE_BOLD.0 } else { 0 }),
                    yHeight: cfg.size_twips,
                    yOffset: 0,
                    crTextColor: colorref(cfg.fg),
                    bCharSet: DEFAULT_CHARSET,
                    bPitchAndFamily: 0,
                    szFaceName: face,
                }),
                pf: Box::new(PARAFORMAT {
                    cbSize: std::mem::size_of::<PARAFORMAT>() as u32,
                    ..Default::default()
                }),
                fg: colorref(cfg.fg),
                fg_authored: cfg.fg_authored,
                sel_bg: colorref(cfg.sel_bg),
                sel_fg: colorref(cfg.sel_fg),
                caret_pos: POINT::default(),
                caret_size: SIZE::default(),
                caret_shown: false,
                caret_created: false,
                armed_timers: std::collections::BTreeMap::new(),
                node: None,
                timer_pool: timer_pool.clone(),
                timer_seq: timer_seq.clone(),
                events: Vec::new(),
                read_only: cfg.read_only,
                overflow: false,
            },
        };
        let host = Box::into_raw(Box::new(HostBox {
            host: unsafe { &*(&VT_HOST2 as *const ITextHost2_Vtbl as *const ITextHost_Vtbl) },
            host2: &VT_HOST2,
            refs: Cell::new(1),
            state: RefCell::new(shared),
        }));
        // RAII: if CreateTextServices or the QI fails, the host box (and
        // its owned `unk`) must not leak — the guard owns it until the
        // peer is fully constructed
        struct HostGuard(*mut HostBox);
        impl Drop for HostGuard {
            fn drop(&mut self) {
                unsafe {
                    drop(Box::from_raw(self.0));
                }
            }
        }
        let guard = HostGuard(host);
        unsafe {
            let mut punk: *mut c_void = std::ptr::null_mut();
            (lib.create_text_services)(std::ptr::null_mut(), host as *mut c_void, &mut punk)
                .ok()
                .map_err(|e| UiError::Platform(format!("CreateTextServices: {e}")))?;
            let unk = IUnknown::from_raw(punk);
            let mut tx2_raw: *mut c_void = std::ptr::null_mut();
            (Interface::vtable(&unk).QueryInterface)(
                unk.as_raw(),
                &lib.iid_text_services2 as *const GUID,
                &mut tx2_raw,
            )
            .ok()
            .map_err(|e| UiError::Platform(format!("QI(ITextServices2): {e}")))?;
            let tx2 = ITextServices2::from_raw(tx2_raw);
            let mut res = LRESULT(0);
            // plain-text mode, event mask, no URL detect, bounded length
            let _ = tx2.TxSendMessage(
                EM_SETTEXTMODE,
                WPARAM(TM_PLAINTEXT.0 as usize),
                LPARAM(0),
                &mut res,
            );
            let _ = tx2.TxSendMessage(
                EM_SETEVENTMASK,
                WPARAM(0),
                LPARAM(
                    (ENM_CHANGE
                        | ENM_REQUESTRESIZE
                        | ENM_SELCHANGE
                        | ENM_KEYEVENTS
                        | ENM_MOUSEEVENTS) as isize,
                ),
                &mut res,
            );

            let _ = tx2.TxSendMessage(EM_AUTOURLDETECT, WPARAM(0), LPARAM(0), &mut res);
            let _ = tx2.TxSendMessage(EM_LIMITTEXT, WPARAM(4096), LPARAM(0), &mut res);
            // NOTE: no OnTxInPlaceActivate yet — activation latches the
            // format space; peers mount before layout, so activation waits
            // for the first non-empty bounds (see ensure_activated)
            let peer = WindowlessPeer {
                tx: Some(tx2),
                host,
                id,
                node: NodeId {
                    slot: u32::MAX,
                    generation: u64::MAX,
                },
                peer_rev: Cell::new(TextRevision::default()),
                binding: None,
                sink,
                multiline: cfg.multiline,
                activation: Cell::new(Activation::Inactive),
                line_h: Cell::new(0.0),
            };
            std::mem::forget(guard);
            Ok(peer)
        }
    }

    /// Forward a window message (already in the peer's local DIP space for
    /// pointer coords) into the text service. Host callbacks run INSIDE
    /// this call — no `RefCell` borrow may be held across it. The HRESULT
    /// is captured raw (the safe binding drops it) — it's the documented
    /// processed/fallback signal.
    pub(crate) fn send(&self, msg: u32, wparam: usize, lparam: isize) -> UiResult<NativeSend> {
        // focus arrival is the ONLY driver of UI activation — the backend
        // owns focus; a measured/drawn peer is never implicitly UI-active
        if msg == WM_SETFOCUS {
            self.set_ui_active(true)?;
        }
        let out = self.send_raw(msg, wparam, lparam);
        if msg == WM_KILLFOCUS {
            self.set_ui_active(false)?;
        }
        out
    }

    fn send_raw(&self, msg: u32, wparam: usize, lparam: isize) -> UiResult<NativeSend> {
        unsafe {
            let mut res = LRESULT(0);
            let vtbl = Interface::vtable(self.tx());
            let hr = (vtbl.base__.TxSendMessage)(
                self.tx().as_raw(),
                msg,
                WPARAM(wparam),
                LPARAM(lparam),
                &mut res,
            );
            if !hr.is_ok() && hr != S_FALSE {
                return Err(UiError::Platform(format!("TxSendMessage {msg:#x}: {hr:?}")));
            }
            Ok(NativeSend { hr, lr: res.0 })
        }
    }

    /// Read committed text verbatim (UTF-16 -> UTF-8 checked; no lossy
    /// fallback — a transient failure is a typed error, never "empty").
    pub(crate) fn text(&self) -> UiResult<String> {
        unsafe {
            let mut b = BSTR::default();
            self.tx()
                .TxGetText(&mut b)
                .map_err(|e| UiError::Platform(format!("TxGetText: {e}")))?;
            let s = b.to_string();
            Ok(s)
        }
    }

    /// Selection as native UTF-16 (anchor, focus) char offsets — the
    /// direction is preserved via ITextDocument's selection flags, not
    /// EM_EXGETSEL's sorted cpMin/cpMax.
    pub(crate) fn selection_utf16(&self) -> UiResult<(usize, usize)> {
        unsafe {
            let mut doc_raw: *mut c_void = std::ptr::null_mut();
            let vt = Interface::vtable(self.tx());
            (vt.base__.base__.QueryInterface)(
                self.tx().as_raw(),
                &ITextDocument::IID as *const GUID,
                &mut doc_raw,
            )
            .ok()
            .map_err(|e| UiError::Platform(format!("QI(ITextDocument): {e}")))?;
            let doc = ITextDocument::from_raw(doc_raw);
            let sel: ITextSelection = doc
                .GetSelection()
                .map_err(|e| UiError::Platform(format!("GetSelection: {e}")))?;
            let flags = sel
                .GetFlags()
                .map_err(|e| UiError::Platform(format!("GetFlags: {e}")))?;
            let (cp_start, cp_end) = (
                sel.GetStart()
                    .map_err(|e| UiError::Platform(format!("GetStart: {e}")))?,
                sel.GetEnd()
                    .map_err(|e| UiError::Platform(format!("GetEnd: {e}")))?,
            );
            // tomSelStartActive: START is the focus (direction preserved —
            // EM_EXGETSEL's sorted cpMin/cpMax would lose it)
            let (anchor, focus) = if (flags & tomSelStartActive.0) != 0 {
                (cp_end, cp_start)
            } else {
                (cp_start, cp_end)
            };
            Ok((anchor as usize, focus as usize))
        }
    }

    /// Draw the peer's content directly into the window's D2D target.
    /// `bounds` is the peer's CONTENT rect in window DIP
    /// (`editor_content_rect` is the single shared transform). `lprcBounds`
    /// for `TxDrawD2D` is in the host's PHYSICAL-PIXEL space — msftedit
    /// divides by `dcDpi/96` internally, so `LogicalRect::rectl` performs THE
    /// conversion at this seam. The format space stays the peer's LOCAL
    /// client rect (px); one transform at the seam, nothing to cache.
    pub(crate) fn draw(&self, rt: &ID2D1RenderTarget, bounds: LogicalRect) -> UiResult<()> {
        self.ensure_in_place()?;
        if !(bounds.width > 0.0
            && bounds.height > 0.0
            && bounds.x.is_finite()
            && bounds.y.is_finite())
        {
            return Ok(());
        }
        let sc = self.shared().host.scale;
        let mut rc = bounds.rectl(sc);
        unsafe {
            self.tx()
                .TxDrawD2D(rt, &mut rc, std::ptr::null_mut(), 0)
                .map_err(|e| UiError::Platform(format!("TxDrawD2D(peer): {e}")))
        }
    }

    /// Natural content size in DIP at the given layout width (REQRESIZE).
    /// msftedit formats in the client width, so the width must be current
    /// before measuring; a tall scratch bottom gives it room to report.
    pub(crate) fn natural_size(&self, width_dip: f32) -> UiResult<(f32, f32)> {
        {
            let mut s = self.shared_mut();
            s.host.bounds.width = width_dip;
            if s.host.bounds.height <= 0.0 {
                // scratch height — the service reports natural extent via
                // REQRESIZE regardless of clip height
                s.host.bounds.height = 4000.0;
            }
            // freshness: a stale extent must never masquerade as a fresh
            // measurement — reset before requesting so a dropped
            // notification is an error, not last pass's numbers
            s.host.natural = SIZE::default();
        }
        self.ensure_in_place()?;
        self.send(EM_REQUESTRESIZE, 0, 0)?;
        let sc = self.shared().host.scale;
        let px = self.shared().host.natural;
        if px.cy <= 0 {
            return Err(UiError::Platform(
                "EM_REQUESTRESIZE produced no fresh extent".into(),
            ));
        }
        // per-line metric from msftedit itself — cy / EM_GETLINECOUNT is
        // content-independent; layout's cap uses this, not a magic DIP.
        // Client units are px — `px_to_dip` is THE seam.
        if px.cy > 0
            && let Ok(ns) = self.send(EM_GETLINECOUNT, 0, 0)
        {
            self.line_h
                .set(sc.to_logical(px.cy) / (ns.lr.max(1) as f32));
        }
        Ok((sc.to_logical(px.cx), sc.to_logical(px.cy)))
    }

    /// measured single-line height (DIP); 0.0 until a `natural_size` pass
    /// latches it — layout falls back to its constant until then
    pub(crate) fn line_height(&self) -> f32 {
        self.line_h.get()
    }

    /// Global bounds (window DIP) + DPI scale — updates without recreate.
    /// The first non-empty bounds activate the text service (the format
    /// space latches at activation, so activating at mount — before layout —
    /// would latch a 0×0 space and draw nothing). The client rect is a live
    /// property (`TxGetClientRect` reads `host.bounds`) — never relatched,
    /// matching the proven mascot contract.
    pub(crate) fn apply_bounds(&self, bounds: LogicalRect, scale: ScaleFactor) -> UiResult {
        let prev = self.shared().host.scale;
        self.shared_mut().host.bounds = bounds;
        self.shared_mut().host.scale = scale;
        if self.activation.get() != Activation::Inactive && prev != scale {
            // the service latches its view in *physical px* at activation —
            // a scale change re-interprets the same DIP bounds as different
            // px, so the peer must relatch or every draw lands at the stale
            // scale's coordinates (the row-1 defect). Relatch preserves the
            // NATIVE editing state: the COM object stays alive, so text,
            // undo history and composition survive by contract; the
            // selection/active-end direction is snapshotted and restored
            // because deactivate is free to collapse it.
            let was_ui = self.activation.get() == Activation::Ui;
            let sel = self.selection_utf16().ok();
            unsafe {
                if was_ui {
                    self.tx()
                        .OnTxUIDeactivate()
                        .map_err(|e| UiError::Platform(format!("OnTxUIDeactivate: {e}")))?;
                }
                self.tx()
                    .OnTxInPlaceDeactivate()
                    .map_err(|e| UiError::Platform(format!("OnTxInPlaceDeactivate: {e}")))?;
            }
            self.activation.set(Activation::Inactive);
            self.ensure_in_place()?;
            if let Some((anchor, focus)) = sel {
                self.restore_selection_utf16(anchor, focus)?;
            }
            if was_ui {
                self.set_ui_active(true)?;
            }
        } else {
            self.ensure_in_place()?;
        }
        Ok(())
    }

    /// Re-apply a directional (anchor, focus) selection after a relatch —
    /// SetStart/SetEnd + the tomSelStartActive flag restore direction.
    fn restore_selection_utf16(&self, anchor: usize, focus: usize) -> UiResult {
        unsafe {
            let mut doc_raw: *mut c_void = std::ptr::null_mut();
            let vt = Interface::vtable(self.tx());
            (vt.base__.base__.QueryInterface)(
                self.tx().as_raw(),
                &ITextDocument::IID as *const GUID,
                &mut doc_raw,
            )
            .ok()
            .map_err(|e| UiError::Platform(format!("QI(ITextDocument): {e}")))?;
            let doc = ITextDocument::from_raw(doc_raw);
            let sel: ITextSelection = doc
                .GetSelection()
                .map_err(|e| UiError::Platform(format!("GetSelection: {e}")))?;
            sel.SetStart(anchor as i32)
                .and_then(|_| sel.SetEnd(focus as i32))
                .map_err(|e| UiError::Platform(format!("restore selection: {e}")))?;
            let mut flags = sel
                .GetFlags()
                .map_err(|e| UiError::Platform(format!("GetFlags: {e}")))?;
            if anchor != focus {
                if focus < anchor {
                    flags |= tomSelStartActive.0;
                } else {
                    flags &= !tomSelStartActive.0;
                }
                sel.SetFlags(flags)
                    .map_err(|e| UiError::Platform(format!("SetFlags: {e}")))?;
            }
            Ok(())
        }
    }

    /// Activate IN PLACE once bounds are non-empty. Checked — the state
    /// only advances when msftedit accepts the activation.
    /// Idempotent. UI activation is a separate transition (focus-driven).
    fn ensure_in_place(&self) -> UiResult {
        if self.activation.get() != Activation::Inactive || self.tx.is_none() {
            return Ok(());
        }
        let b = self.shared().host.bounds;
        if b.width <= 0.0 || b.height <= 0.0 {
            return Ok(());
        }
        let sc = self.shared().host.scale;
        // activation rect in the host's px units (see the unit contract at
        // the top of the file / space.rs)
        let mut local: RECT = LogicalRect::local(b.width, b.height).physical(sc).into();
        unsafe {
            self.tx()
                .OnTxInPlaceActivate(&mut local)
                .map_err(|e| UiError::Platform(format!("OnTxInPlaceActivate: {e}")))?;
        }
        self.activation.set(Activation::InPlace);
        Ok(())
    }

    /// UI activation — the focus-owner transition. Checked: the state
    /// advances only when the service accepts. Called by `send` when the
    /// backend routes WM_SETFOCUS/WM_KILLFOCUS to this peer.
    fn set_ui_active(&self, on: bool) -> UiResult {
        match (self.activation.get(), on) {
            (Activation::InPlace, true) => {
                unsafe {
                    self.tx()
                        .OnTxUIActivate()
                        .map_err(|e| UiError::Platform(format!("OnTxUIActivate: {e}")))?;
                }
                self.activation.set(Activation::Ui);
            }
            (Activation::Ui, false) => {
                unsafe {
                    self.tx()
                        .OnTxUIDeactivate()
                        .map_err(|e| UiError::Platform(format!("OnTxUIDeactivate: {e}")))?;
                }
                self.activation.set(Activation::InPlace);
            }
            _ => {}
        }
        Ok(())
    }

    /// Theme colors — CFE_AUTOCOLOR resolves `COLOR_WINDOWTEXT` through
    /// `TxGetSysColor` at draw time; no run-format rewrite needed.
    /// Theme palette flip — selection colors follow the palette and the
    /// AUTHORED foreground re-resolves: a role tracks the new theme; an
    /// authored literal is already concrete and stands.
    pub(crate) fn set_colors(&self, fg: [f32; 4], sel_bg: [f32; 4], sel_fg: [f32; 4], dark: bool) {
        let mut s = self.shared_mut();
        let resolved = crate::style::resolve_color(s.host.fg_authored, dark);
        s.host.fg = colorref(resolved);
        s.host.cf.crTextColor = colorref(resolved);
        let _ = fg; // authored path is authoritative; palette fg unused here
        s.host.sel_bg = colorref(sel_bg);
        s.host.sel_fg = colorref(sel_fg);
        s.host.ev(HostEvent::Invalidate);
    }

    /// The host's read-only gate (separate from disabled input routing) —
    /// pushes ES_READONLY behavior into the service so native editing,
    /// selection changes through keys, and paste all stop.
    /// Live read-only toggle — TXTBIT_READONLY is a host PROPERTY BIT in
    /// the windowless contract (EM_SETREADONLY/EM_SETOPTIONS don't apply):
    /// update the bits the host reports, then OnTxPropertyBitsChange makes
    /// the service re-query. Editing, paste and IME are all dead inside
    /// msftedit — not just gated in the input router.
    pub(crate) fn set_read_only(&self, ro: bool) {
        {
            let mut s = self.shared_mut();
            s.host.read_only = ro;
            if ro {
                s.host.bits |= TXTBIT_READONLY;
            } else {
                s.host.bits &= !TXTBIT_READONLY;
            }
        }
        if let Some(tx) = &self.tx {
            let bits = self.shared().host.bits;
            let _ = unsafe { tx.OnTxPropertyBitsChange(TXTBIT_READONLY, bits) };
        }
    }

    /// The peer's host hwnd (assigned when the window exists).
    pub(crate) fn set_hwnd(&self, hwnd: HWND) {
        self.shared_mut().host.hwnd = hwnd;
    }

    /// Richedit-requested native timer ids currently armed.
    pub(crate) fn native_timers(&self) -> Vec<u32> {
        self.shared().host.armed_timers.keys().copied().collect()
    }

    /// Caret state as `(created, shown, pos, size)` — the backend draws the
    /// real Win32 system caret when this peer owns focus.
    pub(crate) fn caret(&self) -> (bool, bool, POINT, SIZE) {
        let s = self.shared();
        (
            s.host.caret_created,
            s.host.caret_shown,
            s.host.caret_pos,
            s.host.caret_size,
        )
    }

    /// Drain host events queued while a text-services call ran.
    pub(crate) fn drain(&self) -> UiResult<Vec<HostEvent>> {
        let (overflow, events) = {
            let mut s = self.shared_mut();
            let ov = std::mem::replace(&mut s.host.overflow, false);
            (ov, std::mem::take(&mut s.host.events))
        };
        if overflow {
            return Err(UiError::QueueOverflow);
        }
        Ok(events)
    }

    /// Queue a semantic event for the backend to route into the runtime.
    fn emit(&self, item: super::NativeSinkItem) {
        let mut sink = self.sink.lock().unwrap();
        if sink.len() < crate::event::EVENT_QUEUE_CAP {
            sink.push(item);
        } else {
            // bounded — the backend's drain surfaces overflow as typed
            self.shared_mut().host.overflow = true;
        }
    }

    /// QI the text services for an arbitrary IID (windowless accessibility).
    pub(crate) fn query_iid(&self, iid: &GUID) -> UiResult<*mut c_void> {
        unsafe {
            let mut p: *mut c_void = std::ptr::null_mut();
            let vt = Interface::vtable(self.tx());
            (vt.base__.base__.QueryInterface)(self.tx().as_raw(), iid as *const GUID, &mut p)
                .ok()
                .map_err(|e| UiError::Platform(format!("query_iid: {e}")))?;
            Ok(p)
        }
    }
}

/// One synchronous `TxSendMessage` outcome: the HRESULT carries
/// processing/fallback information (`S_OK` = the service handled it,
/// `S_FALSE` = the host should process it), the LRESULT is the message's
/// own result.
pub(crate) struct NativeSend {
    pub hr: HRESULT,
    pub lr: isize,
}

impl NativeSend {
    /// `S_FALSE` → the service did not consume the key; the framework may
    /// interpret it (submit/shortcut). During IME composition a confirm
    /// key is consumed (`S_OK`), so it never submits.
    pub(crate) fn consumed(&self) -> bool {
        self.hr != S_FALSE
    }
}

/// `CreateTextServices` config — theme/format at peer creation.
pub(crate) struct PeerConfig {
    pub multiline: bool,
    pub read_only: bool,
    pub face: String,
    pub size_twips: i32,
    pub fg: [f32; 4],
    /// the consumer's authored foreground `Color` (roles re-resolve)
    pub fg_authored: crate::style::Color,
    pub sel_bg: [f32; 4],
    pub sel_fg: [f32; 4],
    /// bold flag folded into CHARFORMAT dwEffects (CFM_BOLD)
    pub bold: bool,
}

impl crate::node::TextPeer for WindowlessPeer {
    fn peer_id(&self) -> u64 {
        self.id
    }

    fn initialize(
        &mut self,
        text: &str,
        revision: TextRevision,
        binding: BindingToken,
    ) -> UiResult {
        unsafe {
            let h = HSTRING::from(text);
            self.tx()
                .TxSetText(PCWSTR(h.as_ptr()))
                .map_err(|e| UiError::Platform(format!("TxSetText(init): {e}")))?;
        }
        self.binding = Some(binding);
        self.peer_rev.set(revision);
        // mount-time init must NOT fabricate an acknowledgement — the sync
        // only accepts a real Programmatic ack for a queued proposal
        self.drain().map(|_| ())
    }

    fn set_text(&mut self, text: &str, base: TextRevision, requested: TextRevision) -> UiResult {
        unsafe {
            let h = HSTRING::from(text);
            self.tx()
                .TxSetText(PCWSTR(h.as_ptr()))
                .map_err(|e| UiError::Platform(format!("TxSetText: {e}")))?;
        }
        // actual read-back — the ack must describe what the peer committed
        let committed = self.text()?;
        self.peer_rev.set(requested);
        if let Some(b) = self.binding {
            self.emit(super::NativeSinkItem::Edit {
                node: self.node,
                edit: crate::text::TextEdit {
                    text: committed,
                    base,
                    result: requested,
                    origin: crate::text::EditOrigin::Programmatic,
                    binding: b,
                },
            });
        }
        Ok(())
    }

    fn release(&mut self) {
        // text services release first (may call back into the host)
        // return the surface's bytes to the aggregate budget before the
        // native objects go — accounting mirrors allocation
        drop(self.tx.take());
        unsafe {
            HostBox::Release(self.host);
        }
        self.host = std::ptr::null_mut();
    }

    fn attach(&mut self, node: NodeId) {
        self.node = node;
        self.shared_mut().host.node = Some(node);
    }
}

impl Drop for WindowlessPeer {
    fn drop(&mut self) {
        if !self.host.is_null() {
            drop(self.tx.take());
            // kill every native timer this host armed — a dead peer can
            // never receive WM_TIMER, so its ids must not stay live
            let (hwnd, tids, pool) = {
                let mut s = self.shared_mut();
                let t: Vec<usize> = s.host.armed_timers.values().copied().collect();
                s.host.armed_timers.clear();
                (s.host.hwnd, t, s.host.timer_pool.clone())
            };
            for tid in tids {
                pool.lock().unwrap().remove(&tid);
                if !hwnd.is_invalid() {
                    unsafe {
                        let _ = windows::Win32::UI::WindowsAndMessaging::KillTimer(Some(hwnd), tid);
                    }
                }
            }
            unsafe {
                HostBox::Release(self.host);
            }
        }
    }
}
