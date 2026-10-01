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
//! caret/invalidation coords are DIPs; `TxGetViewExtent` is fixed 96-DPI
//! HIMETRIC (`himetric = dip * 2540/96`), making the format space
//! scale-independent. `TxScreenToClient`/`TxClientToScreen` convert screen
//! px <-> DIP through the stored origin+scale.
//!
//! Object layout: msftedit assumes a C++ single-inheritance host — it calls
//! `ITextHost2` methods through the `ITextHost` pointer. The host carries ONE
//! vtbl (the full `ITextHost2` vtbl, OFFSET 0); `QueryInterface` answers
//! `IUnknown`/`ITextHost`/`ITextHost2` all with `base+0`.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::ffi::c_void;
use std::sync::Arc;
use std::sync::atomic::Ordering;

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

use crate::text::{BindingToken, TextRevision};
use crate::{NodeId, UiError, UiResult};

/// missing from the windows bindings
const EM_SETREADONLY: u32 = 0x40CF; // WM_USER + 31
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
    /// richedit asked the host for a timer (native id, ms)
    SetTimer(u32, u32),
    KillTimer(u32),
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
    pub bounds: RECT,
    /// DPI scale (px per DIP)
    pub scale: f32,
    /// property bits advertised via TxGetPropertyBits
    pub bits: u32,
    /// latest EN_REQUESTRESIZE size (client units = DIP)
    pub natural: SIZE,
    pub cf: Box<CHARFORMATW>,
    pub pf: Box<PARAFORMAT>,
    pub fg: COLORREF,
    pub sel_bg: COLORREF,
    pub sel_fg: COLORREF,
    /// system-caret state recorded from Tx*Caret calls
    pub caret_pos: POINT,
    pub caret_size: SIZE,
    pub caret_shown: bool,
    pub caret_created: bool,
    /// richedit-requested native timer ids currently installed
    pub timers: BTreeSet<u32>,
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
    fn TxSetTimer(&self, idtimer: u32, utimeout: u32) -> BOOL {
        let mut s = self.m();
        s.host.timers.insert(idtimer);
        s.host.ev(HostEvent::SetTimer(idtimer, utimeout));
        BOOL(1)
    }
    fn TxKillTimer(&self, idtimer: u32) {
        let mut s = self.m();
        s.host.timers.remove(&idtimer);
        s.host.ev(HostEvent::KillTimer(idtimer));
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
        bools(unsafe {
            let ok = ScreenToClient(hwnd, lppt).as_bool();
            if ok {
                // screen px -> window client DIP, then peer-local DIP
                (*lppt).x = ((*lppt).x as f32 / scale).round() as i32 - bounds.left;
                (*lppt).y = ((*lppt).y as f32 / scale).round() as i32 - bounds.top;
            }
            ok
        })
    }
    fn TxClientToScreen(&self, lppt: *mut POINT) -> BOOL {
        let (scale, bounds, hwnd) = {
            let s = self.s();
            (s.host.scale, s.host.bounds, s.host.hwnd)
        };
        bools(unsafe {
            // peer-local DIP -> screen px
            (*lppt).x = (((*lppt).x + bounds.left) as f32 * scale).round() as i32;
            (*lppt).y = (((*lppt).y + bounds.top) as f32 * scale).round() as i32;
            ClientToScreen(hwnd, lppt).as_bool()
        })
    }
    fn TxActivate(&self, _ploldstate: *mut i32) -> Result<()> {
        Ok(())
    }
    fn TxDeactivate(&self, _lnewstate: i32) -> Result<()> {
        Ok(())
    }
    fn TxGetClientRect(&self, prc: *mut RECT) -> Result<()> {
        unsafe {
            // local DIP rect — the peer's own coordinate space is (0,0,w,h)
            let b = self.s().host.bounds;
            *prc = RECT {
                left: 0,
                top: 0,
                right: b.right - b.left,
                bottom: b.bottom - b.top,
            };
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
        // view extent is HIMETRIC at a fixed 96 DPI — scale-independent
        let hm = 2540.0 / 96.0;
        unsafe {
            *lpextent = SIZE {
                cx: ((b.right - b.left) as f32 * hm).round() as i32,
                cy: ((b.bottom - b.top) as f32 * hm).round() as i32,
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
    /// OnTxInPlaceActivate once the peer has real (non-empty) bounds
    activated: Cell<bool>,
    /// peer-local compatible bitmap target — TxDrawD2D renders into this
    /// local space and we blit it into the window target at the global
    /// rect. Reuse key = (pixel w, pixel h, frame-target generation) —
    /// DPI moves and device recreation force re-rasterization.
    bmp_target: std::cell::RefCell<
        Option<(
            windows::Win32::Graphics::Direct2D::ID2D1BitmapRenderTarget,
            (u32, u32, u64),
        )>,
    >,
    /// this peer's current surface footprint in bytes (BGRA8)
    bmp_bytes: Cell<u64>,
    /// aggregate peer-surface byte budget shared via PeerCtx
    byte_budget: std::sync::Arc<std::sync::atomic::AtomicU64>,
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
    pub(crate) fn apply_format(&mut self, fg: COLORREF, size_twips: i32, bold: bool) {
        let cf = {
            let mut sh = self.shared_mut();
            sh.host.cf.crTextColor = fg;
            sh.host.cf.yHeight = size_twips;
            sh.host.fg = fg;
            let mut e = sh.host.cf.dwEffects.0;
            if bold {
                e |= CFE_BOLD.0;
            } else {
                e &= !CFE_BOLD.0;
            }
            sh.host.cf.dwEffects = CFE_EFFECTS(e);
            *sh.host.cf
        };
        let _ = self.send(EM_SETCHARFORMAT, SCF_ALL as usize, &cf as *const _ as isize);
    }

    /// Build the host + text services for one peer.
    pub(crate) fn create(
        id: u64,
        lib: &Arc<Msftedit>,
        hwnd: HWND,
        scale: f32,
        cfg: &PeerConfig,
        sink: std::sync::Arc<std::sync::Mutex<Vec<super::NativeSinkItem>>>,
        byte_budget: std::sync::Arc<std::sync::atomic::AtomicU64>,
    ) -> UiResult<WindowlessPeer> {
        let mut face = [0u16; 32];
        for (i, c) in cfg.face.encode_utf16().take(31).enumerate() {
            face[i] = c;
        }
        let shared = PeerShared {
            lib: lib.clone(),
            host: HostShared {
                hwnd,
                bounds: RECT::default(),
                scale,
                bits: TXTBIT_WORDWRAP
                    | TXTBIT_AUTOWORDSEL
                    | TXTBIT_DISABLEDRAG
                    | TXTBIT_D2DDWRITE
                    | TXTBIT_D2DPIXELSNAPPED
                    | if cfg.multiline { TXTBIT_MULTILINE } else { 0 },
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
                sel_bg: colorref(cfg.sel_bg),
                sel_fg: colorref(cfg.sel_fg),
                caret_pos: POINT::default(),
                caret_size: SIZE::default(),
                caret_shown: false,
                caret_created: false,
                timers: BTreeSet::new(),
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
            Ok(WindowlessPeer {
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
                activated: Cell::new(false),
                bmp_target: std::cell::RefCell::new(None),
                bmp_bytes: Cell::new(0),
                byte_budget,
            })
        }
    }

    /// Forward a window message (already in the peer's local DIP space for
    /// pointer coords) into the text service. Host callbacks run INSIDE
    /// this call — no `RefCell` borrow may be held across it. The HRESULT
    /// is captured raw (the safe binding drops it) — it's the documented
    /// processed/fallback signal.
    pub(crate) fn send(&self, msg: u32, wparam: usize, lparam: isize) -> UiResult<NativeSend> {
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

    /// Per-peer pixel ceiling: rejects absurd surfaces before arithmetic
    /// can overflow — a fullscreen 8K editor at 200% stays under this.
    const MAX_PEER_PX: u32 = 64 * 1024 * 1024;
    /// Aggregate ceiling across all peer surfaces (256 MB of BGRA).
    const MAX_PEER_BYTES_TOTAL: u64 = 256 * 1024 * 1024;

    /// Draw the peer's content into the window's D2D target. `bounds` is
    /// the peer's CONTENT rect in window DIP (`editor_content_rect` is the
    /// single shared transform). The peer paints into a per-peer
    /// compatible bitmap at LOCAL coords and we blit at the global rect —
    /// TxDrawD2D's anchoring into a shared hwnd target is unreliable.
    ///
    /// Surface identity = (pixel size, target generation). The generation
    /// bumps on DPI change and device/target recreation — both invalidate
    /// every cached bitmap deterministically. Pixel arithmetic is checked;
    /// per-peer and aggregate byte budgets bound retained surfaces.
    ///
    /// `clear` is the resolved opaque background — the island is OPAQUE,
    /// never dependent on transparent alpha compositing.
    pub(crate) fn draw(
        &self,
        rt: &ID2D1RenderTarget,
        bounds: (f32, f32, f32, f32),
        target_gen: u64,
        clear: [f32; 4],
    ) -> UiResult<()> {
        self.ensure_activated();
        let (l, t, r, b) = bounds;
        let (w, h) = (r - l, b - t);
        if !(w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0) {
            return Ok(());
        }
        let scale = self.shared().host.scale;
        // checked device-pixel conversion — no silent wrap
        let (w_px, h_px) = {
            let wp = (w as f64 * scale as f64).ceil();
            let hp = (h as f64 * scale as f64).ceil();
            if wp < 1.0 || hp < 1.0 || wp > u32::MAX as f64 || hp > u32::MAX as f64 {
                return Err(UiError::Platform("peer surface size out of range".into()));
            }
            (wp as u32, hp as u32)
        };
        let pixels = w_px
            .checked_mul(h_px)
            .filter(|p| *p <= Self::MAX_PEER_PX)
            .ok_or_else(|| UiError::Platform("peer surface pixel cap".into()))?;
        let bytes = (pixels as u64)
            .checked_mul(4)
            .ok_or_else(|| UiError::Platform("peer surface byte overflow".into()))?;

        let mut bmp_target = self.bmp_target.borrow_mut();
        let key = (w_px, h_px, target_gen);
        let bt = match &*bmp_target {
            Some((bt, k)) if *k == key => Some(bt.clone()),
            _ => None,
        };
        let bt = match bt {
            Some(bt) => bt,
            None => {
                // aggregate budget check BEFORE the allocation
                let old = self.bmp_bytes.get();
                let after = self
                    .byte_budget
                    .load(Ordering::Relaxed)
                    .checked_sub(old)
                    .and_then(|b| b.checked_add(bytes))
                    .filter(|b| *b <= Self::MAX_PEER_BYTES_TOTAL)
                    .ok_or_else(|| UiError::Platform("peer surface byte budget".into()))?;
                let nbt = unsafe {
                    let t = rt
                        .CreateCompatibleRenderTarget(
                            None,
                            Some(&windows::Win32::Graphics::Direct2D::Common::D2D_SIZE_U {
                                width: w_px,
                                height: h_px,
                            }),
                            None,
                            windows::Win32::Graphics::Direct2D::D2D1_COMPATIBLE_RENDER_TARGET_OPTIONS_NONE,
                        )
                        .map_err(|e| UiError::Platform(format!("compat target: {e}")))?;
                    let bt: windows::Win32::Graphics::Direct2D::ID2D1BitmapRenderTarget = t
                        .cast()
                        .map_err(|e| UiError::Platform(format!("compat target cast: {e}")))?;
                    bt.SetDpi(scale * 96.0, scale * 96.0);
                    bt
                };
                self.byte_budget.store(after, Ordering::Relaxed);
                self.bmp_bytes.set(bytes);
                *bmp_target = Some((nbt.clone(), key));
                nbt
            }
        };
        // render the peer into its own surface at LOCAL (0,0,w,h) —
        // OPAQUE clear: the island carries its own background
        unsafe {
            use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;
            bt.BeginDraw();
            bt.Clear(Some(&D2D1_COLOR_F {
                r: clear[0],
                g: clear[1],
                b: clear[2],
                a: 1.0,
            }));
            let rc = RECTL {
                left: 0,
                top: 0,
                right: w.round() as i32,
                bottom: h.round() as i32,
            };
            let rt2: ID2D1RenderTarget = bt
                .cast()
                .map_err(|e| UiError::Platform(format!("bt cast: {e}")))?;
            let draw_r = self.tx().TxDrawD2D(
                &rt2,
                &rc as *const RECTL as *mut RECTL,
                std::ptr::null_mut(),
                0,
            );
            let end_r = bt.EndDraw(None, None);
            if let Err(e) = draw_r.and(end_r) {
                return Err(UiError::Platform(format!("TxDrawD2D(peer): {e}")));
            }
            let bmp = bt.GetBitmap()?;
            let dest = windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F {
                left: l,
                top: t,
                right: r,
                bottom: b,
            };
            rt.DrawBitmap(
                &bmp,
                Some(&dest),
                1.0,
                windows::Win32::Graphics::Direct2D::D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
                None,
            );
        }
        Ok(())
    }

    /// Natural content size in DIP at the given layout width (REQRESIZE).
    /// msftedit formats in the client width, so the width must be current
    /// before measuring; a tall scratch bottom gives it room to report.
    pub(crate) fn natural_size(&self, width_dip: f32) -> UiResult<(f32, f32)> {
        let w = width_dip.round() as i32;
        {
            let mut s = self.shared_mut();
            let cur_w = s.host.bounds.right - s.host.bounds.left;
            if cur_w != w {
                s.host.bounds.right = s.host.bounds.left + w;
            }
            if s.host.bounds.bottom - s.host.bounds.top <= 0 {
                s.host.bounds.bottom = s.host.bounds.top + 4000;
            }
        }
        self.ensure_activated();
        let _ = self.send(EM_REQUESTRESIZE, 0, 0);
        let px = self.shared().host.natural;
        Ok((px.cx as f32, px.cy as f32))
    }

    /// Global bounds (window DIP) + DPI scale — updates without recreate.
    /// The first non-empty bounds activate the text service (the format
    /// space latches at activation, so activating at mount — before layout —
    /// would latch a 0×0 space and draw nothing). The client rect is a live
    /// property (`TxGetClientRect` reads `host.bounds`) — never relatched,
    /// matching the proven mascot contract.
    pub(crate) fn apply_bounds(&self, bounds: RECT, scale: f32) {
        self.shared_mut().host.bounds = bounds;
        self.shared_mut().host.scale = scale;
        self.ensure_activated();
    }

    /// Activate in-place + UI once bounds are non-empty. Idempotent.
    fn ensure_activated(&self) {
        if self.activated.get() || self.tx.is_none() {
            return;
        }
        let b = self.shared().host.bounds;
        let w = b.right - b.left;
        let h = b.bottom - b.top;
        if w <= 0 || h <= 0 {
            return;
        }
        let mut local = RECT {
            left: 0,
            top: 0,
            right: w,
            bottom: h,
        };
        unsafe {
            let _ = self.tx().OnTxInPlaceActivate(&mut local);
            let _ = self.tx().OnTxUIActivate();
        }
        self.activated.set(true);
    }

    /// Theme colors — CFE_AUTOCOLOR resolves `COLOR_WINDOWTEXT` through
    /// `TxGetSysColor` at draw time; no run-format rewrite needed.
    pub(crate) fn set_colors(&self, fg: [f32; 4], sel_bg: [f32; 4], sel_fg: [f32; 4]) {
        let mut s = self.shared_mut();
        s.host.fg = colorref(fg);
        s.host.sel_bg = colorref(sel_bg);
        s.host.sel_fg = colorref(sel_fg);
        s.host.cf.crTextColor = colorref(fg);
        s.host.ev(HostEvent::Invalidate);
    }

    /// The host's read-only gate (separate from disabled input routing) —
    /// pushes ES_READONLY behavior into the service so native editing,
    /// selection changes through keys, and paste all stop.
    pub(crate) fn set_read_only(&self, ro: bool) {
        self.shared_mut().host.read_only = ro;
        let _ = self.send(EM_SETREADONLY, ro as usize, 0);
    }

    /// The peer's host hwnd (assigned when the window exists).
    pub(crate) fn set_hwnd(&self, hwnd: HWND) {
        self.shared_mut().host.hwnd = hwnd;
    }

    /// Richedit-requested native timer ids currently armed.
    pub(crate) fn native_timers(&self) -> Vec<u32> {
        self.shared().host.timers.iter().copied().collect()
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
        let mut s = self.shared_mut();
        if s.host.overflow {
            s.host.overflow = false;
            return Err(UiError::QueueOverflow);
        }
        Ok(std::mem::take(&mut s.host.events))
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
        self.byte_budget
            .fetch_sub(self.bmp_bytes.get(), Ordering::Relaxed);
        self.bmp_bytes.set(0);
        self.bmp_target.borrow_mut().take();
        drop(self.tx.take());
        unsafe {
            HostBox::Release(self.host);
        }
        self.host = std::ptr::null_mut();
    }

    fn attach(&mut self, node: NodeId) {
        self.node = node;
    }
}

impl Drop for WindowlessPeer {
    fn drop(&mut self) {
        self.byte_budget
            .fetch_sub(self.bmp_bytes.get(), Ordering::Relaxed);
        self.bmp_bytes.set(0);
        if !self.host.is_null() {
            drop(self.tx.take());
            unsafe {
                HostBox::Release(self.host);
            }
        }
    }
}
