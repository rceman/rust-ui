//! UI Automation: a real windowless RichEdit provider per mounted editor
//! (msftedit's exported `IID_IRicheditWindowlessAccessibility` +
//! `CreateProvider(IRawElementProviderWindowlessSite)`), plus painted
//! fragments for semantic controls (Button/Custom/Label), under ONE
//! HWND-backed fragment root.
//!
//! Cycle discipline: the root, site and fragments share ONE
//! `Arc<Snapshot>`; back-references (site→root, fragment→root, native
//! provider→site) are `windows::core::Weak`, never strong — root→child→
//! site→root can't form. A rebuilt snapshot bumps the live-generation set so
//! a retired provider returns `UIA_E_ELEMENTNOTAVAILABLE` on next call.

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::{Arc, Mutex};

use windows::Win32::Foundation::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::Ole::*;
use windows::Win32::System::Variant::*;
use windows::Win32::UI::Accessibility::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::*;

use crate::{NodeId, UiResult};

// ---------------------------------------------------------------------------
// shared snapshot
// ---------------------------------------------------------------------------

/// Live-generation map: slot -> current accepted generation. A remount or
/// snapshot rebuild re-mints it; a stale external provider's generation
/// stops matching and every accessor fails `UIA_E_ELEMENTNOTAVAILABLE`.
/// Every order entry is one of OUR COM objects (PaintFragment or the
/// EditorFragment wrapper) — all Weak-able, no classic-COM strong refs
/// held in the order (the msftedit provider lives inside the wrapper).
/// One live child association — the provider object plus its mutable
/// live state; `rebuild` retains it per surviving NodeId and updates the
/// state in place.
struct ChildShared {
    frag: IRawElementProviderFragment,
    state: Arc<ChildState>,
}

pub(crate) struct Snapshot {
    closed: std::sync::atomic::AtomicBool,
    /// slot -> live generation (retained active order rebuilt per tree)
    live: Mutex<HashMap<u32, u64>>,
    /// active-order children — our provider objects, weak (cycle break)
    order: Mutex<Vec<(NodeId, Weak<IRawElementProviderFragment>)>>,
    /// NodeId -> live child association — generation-keyed: a same-slot
    /// new-generation node gets a NEW entry (old provider's weak dies
    /// with the entry, external refs fail live_ok)
    children: Mutex<HashMap<NodeId, ChildShared>>,
    hwnd: HWND,
    /// posted-action sink — press/focus ride PostMessage, never direct calls
    post_hwnd: HWND,
    /// actual focus owner (set on runtime focus events) — resolved to a
    /// live fragment through `order` on demand
    focus: Mutex<Option<NodeId>>,
}

/// Our per-fragment runtime id as a VARIANT safearray — the UIA client
/// reads `UIA_RuntimeIdPropertyId` through GetPropertyValue (the fragment
/// GetRuntimeId entry point is never called for providers reached this
/// way), so unique ids MUST come from the property path. Without them
/// every child resolves to the host's id and FindAll dedupes to one.
unsafe fn runtime_id_variant(id: NodeId) -> VARIANT {
    let sa = SafeArrayCreateVector(VT_I4, 0, 3);
    if sa.is_null() {
        return VARIANT::default();
    }
    let vals = [
        UiaAppendRuntimeId as i32,
        id.slot as i32,
        (id.generation & 0x7fff_ffff) as i32,
    ];
    for (i, v) in vals.iter().enumerate() {
        let _ = SafeArrayPutElement(sa, &(i as i32), v as *const i32 as *const c_void);
    }
    // VT_I4|VT_ARRAY variant holding the safearray
    let mut var = VARIANT::default();
    (*(&mut var as *mut VARIANT as *mut VARIANT_MANUAL)).vt = (VT_I4 | VT_ARRAY).0;
    (*(&mut var as *mut VARIANT as *mut VARIANT_MANUAL)).parray = sa;
    var
}

#[repr(C)]
struct VARIANT_MANUAL {
    vt: u16,
    _r1: u16,
    _r2: u16,
    _r3: u16,
    parray: *mut SAFEARRAY,
    _pad: usize,
}

impl Snapshot {
    fn live_ok(&self, slot: u32, generation: u64) -> bool {
        if self.closed.load(std::sync::atomic::Ordering::SeqCst) {
            return false;
        }
        self.live
            .lock()
            .unwrap()
            .get(&slot)
            .is_some_and(|g| *g == generation)
    }
}

// ---------------------------------------------------------------------------
// windowless site — parent navigation resolves through Weak
// ---------------------------------------------------------------------------

#[implement(IRawElementProviderWindowlessSite)]
struct WindowlessSite {
    snap: Arc<Snapshot>,
    root: Mutex<Weak<IRawElementProviderFragment>>,
    /// the editor node this site serves — sibling navigation resolves
    /// through the active order relative to it
    node: NodeId,
}

impl IRawElementProviderWindowlessSite_Impl for WindowlessSite_Impl {
    fn GetAdjacentFragment(
        &self,
        direction: NavigateDirection,
    ) -> Result<IRawElementProviderFragment> {
        match direction {
            NavigateDirection_Parent => {
                if let Some(r) = self.root.lock().unwrap().upgrade() {
                    return Ok(r);
                }
            }
            NavigateDirection_NextSibling | NavigateDirection_PreviousSibling => {
                let order = self.snap.order.lock().unwrap();
                if let Some(i) = order.iter().position(|(id, _)| *id == self.node) {
                    let j = if direction == NavigateDirection_NextSibling {
                        i + 1
                    } else {
                        i.checked_sub(1).unwrap_or(usize::MAX)
                    };
                    if let Some((_, e)) = order.get(j)
                        && let Some(f) = e.upgrade()
                    {
                        return Ok(f);
                    }
                }
            }
            _ => {}
        }
        Err(E_NOTIMPL.into())
    }
    fn GetRuntimeIdPrefix(&self) -> Result<*mut SAFEARRAY> {
        Ok(std::ptr::null_mut())
    }
}

// ---------------------------------------------------------------------------
// painted fragment — Button/Custom/Label children
// ---------------------------------------------------------------------------

/// The live state every provider reads through — `rebuild` updates it IN
/// PLACE for a surviving NodeId so providers persist across tree changes
/// and property values never go stale between WM_GETOBJECT calls.
struct ChildState {
    name: Mutex<String>,
    rect: Mutex<UiaRect>,
    enabled: std::sync::atomic::AtomicBool,
    actionable: std::sync::atomic::AtomicBool,
}

#[implement(
    IRawElementProviderSimple,
    IRawElementProviderFragment,
    IInvokeProvider
)]
struct PaintFragment {
    snap: Arc<Snapshot>,
    node: NodeId,
    state: Arc<ChildState>,
    control_type: UIA_CONTROLTYPE_ID,
    localized: &'static str,
    invoke: Mutex<Weak<IInvokeProvider>>,
    root: Mutex<Weak<IRawElementProviderFragment>>,
}

impl PaintFragment {
    fn live(&self) -> bool {
        self.snap.live_ok(self.node.slot, self.node.generation)
    }
    fn enabled(&self) -> bool {
        self.state.enabled.load(std::sync::atomic::Ordering::SeqCst)
    }
    fn actionable(&self) -> bool {
        self.state
            .actionable
            .load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl IRawElementProviderSimple_Impl for PaintFragment_Impl {
    fn ProviderOptions(&self) -> Result<ProviderOptions> {
        Ok(ProviderOptions_ServerSideProvider | ProviderOptions_UseComThreading)
    }
    fn GetPatternProvider(&self, patternid: UIA_PATTERN_ID) -> Result<IUnknown> {
        if patternid == UIA_InvokePatternId && self.actionable() && self.enabled() && self.live() {
            if let Some(p) = self.invoke.lock().unwrap().upgrade() {
                return Ok(p.cast::<IUnknown>().unwrap());
            }
        }
        Err(E_NOTIMPL.into())
    }
    fn GetPropertyValue(&self, propertyid: UIA_PROPERTY_ID) -> Result<VARIANT> {
        if !self.live() {
            return Err(windows::core::Error::from_hresult(HRESULT(
                UIA_E_ELEMENTNOTAVAILABLE as i32,
            )));
        }
        match propertyid {
            x if x == UIA_NamePropertyId => {
                Ok(VARIANT::from(BSTR::from(&*self.state.name.lock().unwrap())))
            }
            x if x == UIA_RuntimeIdPropertyId => Ok(unsafe { runtime_id_variant(self.node) }),
            x if x == UIA_ControlTypePropertyId => Ok(VARIANT::from(self.control_type.0 as i32)),
            x if x == UIA_LocalizedControlTypePropertyId => {
                Ok(VARIANT::from(BSTR::from(self.localized)))
            }
            x if x == UIA_IsEnabledPropertyId => Ok(VARIANT::from(self.enabled())),
            x if x == UIA_IsKeyboardFocusablePropertyId => {
                Ok(VARIANT::from(self.actionable() && self.enabled()))
            }
            // every child is a real control/content element — VT_EMPTY
            // reads as FALSE and drops the element from control/content
            // views and conditional FindAll evaluation
            x if x == UIA_IsControlElementPropertyId => Ok(VARIANT::from(true)),
            x if x == UIA_IsContentElementPropertyId => Ok(VARIANT::from(true)),
            x if x == UIA_HasKeyboardFocusPropertyId => {
                let focused = *self.snap.focus.lock().unwrap() == Some(self.node);
                Ok(VARIANT::from(focused))
            }
            _ => Ok(VARIANT::default()),
        }
    }
    fn HostRawElementProvider(&self) -> Result<IRawElementProviderSimple> {
        // fragments are NOT window-hosted — returning the hwnd provider
        // makes UIA identify every child AS the window (identical runtime
        // ids → FindAll dedupes the whole collection to one)
        Err(E_NOTIMPL.into())
    }
}

impl IRawElementProviderFragment_Impl for PaintFragment_Impl {
    fn Navigate(&self, direction: NavigateDirection) -> Result<IRawElementProviderFragment> {
        if !self.live() {
            return Err(windows::core::Error::from_hresult(HRESULT(
                UIA_E_ELEMENTNOTAVAILABLE as i32,
            )));
        }
        let order = self.snap.order.lock().unwrap();
        let idx = order.iter().position(|(id, _)| *id == self.node);
        match direction {
            NavigateDirection_Parent => self
                .root
                .lock()
                .unwrap()
                .upgrade()
                .ok_or_else(|| E_NOTIMPL.into()),
            NavigateDirection_NextSibling => idx
                .and_then(|i| order.get(i + 1))
                .and_then(|(_, e)| e.upgrade())
                .ok_or_else(|| E_NOTIMPL.into()),
            NavigateDirection_PreviousSibling => idx
                .and_then(|i| i.checked_sub(1))
                .and_then(|i| order.get(i))
                .and_then(|(_, e)| e.upgrade())
                .ok_or_else(|| E_NOTIMPL.into()),
            _ => Err(E_NOTIMPL.into()),
        }
    }
    fn GetRuntimeId(&self) -> Result<*mut SAFEARRAY> {
        if !self.live() {
            return Err(windows::core::Error::from_hresult(HRESULT(
                UIA_E_ELEMENTNOTAVAILABLE as i32,
            )));
        }
        // NULL → UIA assigns a unique id per fragment (documented). Our
        // own SAFEARRAY ids were getting normalized identically and UIA
        // dedupes same-id elements — auto ids remove that failure mode;
        // staleness is fenced by live(), not by the rid.
        Ok(std::ptr::null_mut())
    }
    fn BoundingRectangle(&self) -> Result<UiaRect> {
        if !self.live() {
            return Err(windows::core::Error::from_hresult(HRESULT(
                UIA_E_ELEMENTNOTAVAILABLE as i32,
            )));
        }
        Ok(*self.state.rect.lock().unwrap())
    }
    fn GetEmbeddedFragmentRoots(&self) -> Result<*mut SAFEARRAY> {
        // contract wants a SAFEARRAY (possibly empty), never null
        unsafe { Ok(SafeArrayCreateVector(VT_UNKNOWN, 0, 0)) }
    }
    fn SetFocus(&self) -> Result<()> {
        if !self.live() || !self.enabled() {
            return Err(windows::core::Error::from_hresult(HRESULT(
                UIA_E_ELEMENTNOTAVAILABLE as i32,
            )));
        }
        // generation-fenced focus — posted, never a direct runtime call
        unsafe {
            let _ = PostMessageW(
                Some(self.snap.post_hwnd),
                super::WM_UIA_FOCUS,
                WPARAM(self.node.slot as usize),
                LPARAM(self.node.generation as isize),
            );
        }
        Ok(())
    }
    fn FragmentRoot(&self) -> Result<IRawElementProviderFragmentRoot> {
        self.root
            .lock()
            .unwrap()
            .upgrade()
            .and_then(|f| f.cast::<IRawElementProviderFragmentRoot>().ok())
            .ok_or_else(|| E_NOTIMPL.into())
    }
}

impl IInvokeProvider_Impl for PaintFragment_Impl {
    fn Invoke(&self) -> Result<()> {
        if !self.live() || !self.enabled() {
            return Err(windows::core::Error::from_hresult(HRESULT(
                UIA_E_ELEMENTNOTAVAILABLE as i32,
            )));
        }
        unsafe {
            // generation-fenced press — posted onto the UI thread
            let _ = PostMessageW(
                Some(self.snap.post_hwnd),
                super::WM_UIA_PRESS,
                WPARAM(self.node.slot as usize),
                LPARAM(self.node.generation as isize),
            );
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// editor fragment — delegates to msftedit's native windowless provider,
// but owns Name/Bounds/IsEnabled/HasKeyboardFocus/SetFocus so the editor
// carries a REAL label and live runtime state (msftedit reports no useful
// Name on a windowless site and its BoundingRectangle needs our rect).
// Navigation/patterns/RuntimeId forward to the inner provider — Text
// pattern, native editing semantics, sibling order all stay native.
// ---------------------------------------------------------------------------

#[implement(IRawElementProviderSimple, IRawElementProviderFragment)]
struct EditorFragment {
    inner: IRawElementProviderFragment,
    inner_simple: IRawElementProviderSimple,
    snap: Arc<Snapshot>,
    node: NodeId,
    state: Arc<ChildState>,
    root: Mutex<Weak<IRawElementProviderFragment>>,
}

impl EditorFragment {
    fn live(&self) -> bool {
        self.snap.live_ok(self.node.slot, self.node.generation)
    }
    fn unavailable() -> Error {
        Error::from_hresult(HRESULT(UIA_E_ELEMENTNOTAVAILABLE as i32))
    }
}

impl IRawElementProviderSimple_Impl for EditorFragment_Impl {
    fn ProviderOptions(&self) -> Result<ProviderOptions> {
        // OUR options, not the delegate's — msftedit may advertise
        // client-side/override flags that make UIA swap in a different
        // proxy mid-enumeration
        Ok(ProviderOptions_ServerSideProvider | ProviderOptions_UseComThreading)
    }
    fn GetPatternProvider(&self, patternid: UIA_PATTERN_ID) -> Result<IUnknown> {
        if !self.this.live() {
            return Err(EditorFragment::unavailable());
        }
        unsafe { self.this.inner_simple.GetPatternProvider(patternid) }
    }
    fn GetPropertyValue(&self, propertyid: UIA_PROPERTY_ID) -> Result<VARIANT> {
        if !self.this.live() {
            return Err(EditorFragment::unavailable());
        }
        match propertyid {
            x if x == UIA_NamePropertyId => Ok(VARIANT::from(BSTR::from(
                &*self.this.state.name.lock().unwrap(),
            ))),
            x if x == UIA_IsEnabledPropertyId => Ok(VARIANT::from(
                self.this
                    .state
                    .enabled
                    .load(std::sync::atomic::Ordering::SeqCst),
            )),
            x if x == UIA_IsKeyboardFocusablePropertyId => Ok(VARIANT::from(
                self.this
                    .state
                    .enabled
                    .load(std::sync::atomic::Ordering::SeqCst),
            )),
            x if x == UIA_HasKeyboardFocusPropertyId => {
                let focused = *self.this.snap.focus.lock().unwrap() == Some(self.this.node);
                Ok(VARIANT::from(focused))
            }
            // a failed inner property degrades to VT_EMPTY (the documented
            // "not supported" answer) — msftedit's windowless provider can
            // error on properties it can't serve, and an error here breaks
            // external enumeration (FindAll aborts the whole collection)
            _ => unsafe {
                Ok(self
                    .this
                    .inner_simple
                    .GetPropertyValue(propertyid)
                    .unwrap_or_default())
            },
        }
    }
    fn HostRawElementProvider(&self) -> Result<IRawElementProviderSimple> {
        Err(E_NOTIMPL.into())
    }
}

impl IRawElementProviderFragment_Impl for EditorFragment_Impl {
    fn Navigate(&self, direction: NavigateDirection) -> Result<IRawElementProviderFragment> {
        if !self.this.live() {
            return Err(EditorFragment::unavailable());
        }
        match direction {
            NavigateDirection_Parent => self
                .this
                .root
                .lock()
                .unwrap()
                .upgrade()
                .ok_or_else(|| E_NOTIMPL.into()),
            // siblings walk OUR order — delegating to msftedit's provider
            // breaks external enumeration when it can't answer
            NavigateDirection_NextSibling | NavigateDirection_PreviousSibling => {
                let order = self.this.snap.order.lock().unwrap();
                let idx = order.iter().position(|(id, _)| *id == self.this.node);
                let j = idx.and_then(|i| {
                    if direction == NavigateDirection_NextSibling {
                        i.checked_add(1)
                    } else {
                        i.checked_sub(1)
                    }
                });
                j.and_then(|j| order.get(j))
                    .and_then(|(_, e)| e.upgrade())
                    .ok_or_else(|| E_NOTIMPL.into())
            }
            // the editor is a LEAF — forwarding FirstChild/LastChild to
            // msftedit's provider leaks the host window's caption-button
            // fragments whose sibling chain recurses back through this
            // root (observed: a 1749-element infinite Descendants walk)
            _ => Err(E_NOTIMPL.into()),
        }
    }
    fn GetRuntimeId(&self) -> Result<*mut SAFEARRAY> {
        if !self.this.live() {
            return Err(EditorFragment::unavailable());
        }
        // UIA-assigned unique id — same rationale as PaintFragment
        Ok(std::ptr::null_mut())
    }
    fn BoundingRectangle(&self) -> Result<UiaRect> {
        if !self.this.live() {
            return Err(EditorFragment::unavailable());
        }
        // our authoritative screen rect — msftedit's is unreliable on a
        // windowless site
        Ok(*self.this.state.rect.lock().unwrap())
    }
    fn GetEmbeddedFragmentRoots(&self) -> Result<*mut SAFEARRAY> {
        // leaf editor — no embedded roots; empty array, never null
        unsafe { Ok(SafeArrayCreateVector(VT_UNKNOWN, 0, 0)) }
    }
    fn SetFocus(&self) -> Result<()> {
        if !self.this.live() || !self.state.enabled.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(EditorFragment::unavailable());
        }
        unsafe {
            let _ = PostMessageW(
                Some(self.this.snap.post_hwnd),
                super::WM_UIA_FOCUS,
                WPARAM(self.this.node.slot as usize),
                LPARAM(self.this.node.generation as isize),
            );
        }
        Ok(())
    }
    fn FragmentRoot(&self) -> Result<IRawElementProviderFragmentRoot> {
        unsafe { self.this.inner.FragmentRoot() }
    }
}

// ---------------------------------------------------------------------------
// fragment root — HWND-backed, active-order children
// ---------------------------------------------------------------------------

#[implement(
    IRawElementProviderSimple,
    IRawElementProviderFragment,
    IRawElementProviderFragmentRoot
)]
struct RootFragment {
    snap: Arc<Snapshot>,
    hwnd: HWND,
    name: String,
    me_root: Mutex<Weak<IRawElementProviderFragmentRoot>>,
    me_frag: Mutex<Weak<IRawElementProviderFragment>>,
}

impl IRawElementProviderSimple_Impl for RootFragment_Impl {
    fn ProviderOptions(&self) -> Result<ProviderOptions> {
        Ok(ProviderOptions_ServerSideProvider | ProviderOptions_UseComThreading)
    }
    fn GetPatternProvider(&self, _patternid: UIA_PATTERN_ID) -> Result<IUnknown> {
        Err(E_NOTIMPL.into())
    }
    fn GetPropertyValue(&self, propertyid: UIA_PROPERTY_ID) -> Result<VARIANT> {
        if propertyid == UIA_NamePropertyId {
            return Ok(VARIANT::from(BSTR::from(&*self.name)));
        }
        Ok(VARIANT::default())
    }
    fn HostRawElementProvider(&self) -> Result<IRawElementProviderSimple> {
        unsafe { UiaHostProviderFromHwnd(self.hwnd) }
    }
}

impl IRawElementProviderFragment_Impl for RootFragment_Impl {
    fn Navigate(&self, direction: NavigateDirection) -> Result<IRawElementProviderFragment> {
        let order = self.snap.order.lock().unwrap();
        match direction {
            NavigateDirection_FirstChild => order
                .first()
                .and_then(|(_, e)| e.upgrade())
                .ok_or_else(|| E_NOTIMPL.into()),
            NavigateDirection_LastChild => order
                .last()
                .and_then(|(_, e)| e.upgrade())
                .ok_or_else(|| E_NOTIMPL.into()),
            _ => Err(E_NOTIMPL.into()),
        }
    }
    fn GetRuntimeId(&self) -> Result<*mut SAFEARRAY> {
        Ok(std::ptr::null_mut())
    }
    fn BoundingRectangle(&self) -> Result<UiaRect> {
        unsafe {
            let mut rc = RECT::default();
            let _ = GetWindowRect(self.hwnd, &mut rc);
            Ok(UiaRect {
                left: rc.left as f64,
                top: rc.top as f64,
                width: (rc.right - rc.left) as f64,
                height: (rc.bottom - rc.top) as f64,
            })
        }
    }
    fn GetEmbeddedFragmentRoots(&self) -> Result<*mut SAFEARRAY> {
        unsafe { Ok(SafeArrayCreateVector(VT_UNKNOWN, 0, 0)) }
    }
    fn SetFocus(&self) -> Result<()> {
        Ok(())
    }
    fn FragmentRoot(&self) -> Result<IRawElementProviderFragmentRoot> {
        self.me_root
            .lock()
            .unwrap()
            .upgrade()
            .ok_or_else(|| E_NOTIMPL.into())
    }
}

impl IRawElementProviderFragmentRoot_Impl for RootFragment_Impl {
    fn ElementProviderFromPoint(&self, x: f64, y: f64) -> Result<IRawElementProviderFragment> {
        // topmost (last-in-active-order) fragment whose LIVE bounds contain
        // the screen point — rects are read from the shared child state so
        // hit-testing tracks layout/scroll without rebuilding providers
        let order = self.snap.order.lock().unwrap();
        for (id, e) in order.iter().rev() {
            let r = match self.snap.children.lock().unwrap().get(id) {
                Some(ch) => *ch.state.rect.lock().unwrap(),
                None => continue,
            };
            if x >= r.left
                && x < r.left + r.width
                && y >= r.top
                && y < r.top + r.height
                && let Some(f) = e.upgrade()
            {
                return Ok(f);
            }
        }
        Err(E_NOTIMPL.into())
    }
    fn GetFocus(&self) -> Result<IRawElementProviderFragment> {
        let focus = *self.snap.focus.lock().unwrap();
        self.snap
            .order
            .lock()
            .unwrap()
            .iter()
            .find(|(id, _)| Some(*id) == focus)
            .and_then(|(_, e)| e.upgrade())
            .ok_or_else(|| E_NOTIMPL.into())
    }
}

// ---------------------------------------------------------------------------
// root construction + child rebuild
// ---------------------------------------------------------------------------

/// Built once per backend — the root provider `WM_GETOBJECT` returns, the
/// shared snapshot all weak routing consults, and the site every native
/// editor provider parents through.
/// One active-order UIA child a rebuild installs.
pub(crate) struct ChildBuild {
    pub id: NodeId,
    pub name: String,
    pub ct: UIA_CONTROLTYPE_ID,
    pub localized: &'static str,
    pub rect: UiaRect,
    /// actionable — exposes IInvokeProvider (press factories)
    pub actionable: bool,
    /// !disabled && visible
    pub enabled: bool,
    /// the node's real windowless provider when this is an editor
    pub native: Option<IRawElementProviderFragment>,
    /// the node is an editor — needs the native provider if available
    pub peer_node: bool,
}

/// Convenience alias for the root provider interface.
pub(crate) type IRawRoot = IRawElementProviderFragmentRoot;

pub(crate) struct UiaRoot {
    pub snap: Arc<Snapshot>,
    root: IRawElementProviderFragmentRoot,
    root_frag: IRawElementProviderFragment,
    /// editor slot -> (generation, msftedit's provider fragment). The
    /// generation is part of the association — a same-slot replacement
    /// peer can NEVER inherit the old provider.
    pub(crate) native: Mutex<HashMap<u32, (u64, IRawElementProviderFragment)>>,
}

impl UiaRoot {
    pub(crate) fn new(hwnd: HWND, title: &str) -> windows::core::Result<UiaRoot> {
        let snap = Arc::new(Snapshot {
            closed: std::sync::atomic::AtomicBool::new(false),
            live: Mutex::new(HashMap::new()),
            order: Mutex::new(Vec::new()),
            children: Mutex::new(HashMap::new()),
            hwnd,
            post_hwnd: hwnd,
            focus: Mutex::new(None),
        });
        let site_co = windows_core::ComObject::new(WindowlessSite {
            snap: snap.clone(),
            root: Mutex::new(Weak::new()),
            node: NodeId {
                slot: u32::MAX,
                generation: 0,
            },
        });
        let root_co = windows_core::ComObject::new(RootFragment {
            snap: snap.clone(),
            hwnd,
            name: title.to_string(),
            me_root: Mutex::new(Weak::new()),
            me_frag: Mutex::new(Weak::new()),
        });
        let root: IRawElementProviderFragmentRoot = root_co.to_interface();
        let root_frag: IRawElementProviderFragment = root_co.to_interface();
        // self-references via Weak — no strong root->self cycle
        *root_co.get().me_root.lock().unwrap() = root.downgrade()?;
        *root_co.get().me_frag.lock().unwrap() = root_frag.downgrade()?;
        // site answers child->Parent with the root — Weak, not owned
        *site_co.get().root.lock().unwrap() = root_frag.downgrade()?;
        Ok(UiaRoot {
            snap,
            root,
            root_frag,
            native: Mutex::new(HashMap::new()),
        })
    }

    /// The site a native editor provider calls back through — built lazily
    /// (kept inside UiaRoot to share one site across editors).
    pub(crate) fn site(
        &self,
        node: NodeId,
    ) -> windows::core::Result<IRawElementProviderWindowlessSite> {
        let site_co = windows_core::ComObject::new(WindowlessSite {
            snap: self.snap.clone(),
            root: Mutex::new(Weak::new()),
            node,
        });
        let site: IRawElementProviderWindowlessSite = site_co.to_interface();
        *site_co.get().root.lock().unwrap() = self.root_frag.downgrade()?;
        Ok(site)
    }

    /// Rebuild the live set + active-order children from the retained tree.
    /// Surviving NodeIds keep their provider object — its live state is
    /// updated in place (name/enabled/bounds/actionable follow the real
    /// tree between WM_GETOBJECT calls). Dead ids drop their handle, the
    /// order weak dies, and external references fail `live_ok`. A same-
    /// slot different-generation id is a NEW association — the retired
    /// fragment can never act for its replacement.
    ///
    /// Notifications: structure changes raise `ChildrenInvalidated` on the
    /// root; enabled flips raise `IsEnabled` property-change on the live
    /// fragment. Raised AFTER the new state is installed.
    pub(crate) fn rebuild(&self, children: Vec<ChildBuild>) -> windows::core::Result<()> {
        let mut live = HashMap::new();
        let mut order = Vec::new();
        let mut structure_changed = false;
        let mut enabled_flips: Vec<(IRawElementProviderFragment, bool)> = Vec::new();
        {
            let mut table = self.snap.children.lock().unwrap();
            for k in children {
                live.insert(k.id.slot, k.id.generation);
                // reuse the surviving association when the NodeId matches —
                // state updates in place (no provider churn per rebuild)
                if let Some(h) = table.get(&k.id) {
                    *h.state.name.lock().unwrap() = k.name;
                    *h.state.rect.lock().unwrap() = k.rect;
                    let prev = h
                        .state
                        .enabled
                        .swap(k.enabled, std::sync::atomic::Ordering::SeqCst);
                    if prev != k.enabled {
                        enabled_flips.push((h.frag.clone(), k.enabled));
                    }
                    h.state
                        .actionable
                        .store(k.actionable, std::sync::atomic::Ordering::SeqCst);
                    order.push((k.id, h.frag.downgrade()?));
                    continue;
                }
                structure_changed = true;
                let state = Arc::new(ChildState {
                    name: Mutex::new(k.name),
                    rect: Mutex::new(k.rect),
                    enabled: std::sync::atomic::AtomicBool::new(k.enabled),
                    actionable: std::sync::atomic::AtomicBool::new(k.actionable),
                });
                // editor → wrap the peer's native provider (delegating
                // fragment owns Name/Bounds/Enabled/Focus); painted → our
                // own fragment
                let native = if k.peer_node {
                    match k.native {
                        Some(f) => Some(f),
                        None => self
                            .native
                            .lock()
                            .unwrap()
                            .get(&k.id.slot)
                            .filter(|(g, _)| *g == k.id.generation)
                            .map(|(_, f)| f.clone()),
                    }
                } else {
                    None
                };
                let frag: IRawElementProviderFragment = if let Some(inner) = native {
                    let inner_simple: IRawElementProviderSimple = inner.cast()?;
                    let co = windows_core::ComObject::new(EditorFragment {
                        inner,
                        inner_simple,
                        snap: self.snap.clone(),
                        node: k.id,
                        state: state.clone(),
                        root: Mutex::new(Weak::new()),
                    });
                    let f: IRawElementProviderFragment = co.to_interface();
                    *co.get().root.lock().unwrap() = self.root_frag.downgrade()?;
                    f
                } else {
                    let co = windows_core::ComObject::new(PaintFragment {
                        snap: self.snap.clone(),
                        node: k.id,
                        state: state.clone(),
                        control_type: k.ct,
                        localized: k.localized,
                        invoke: Mutex::new(Weak::new()),
                        root: Mutex::new(Weak::new()),
                    });
                    let f: IRawElementProviderFragment = co.to_interface();
                    let invoke: IInvokeProvider = co.to_interface();
                    *co.get().invoke.lock().unwrap() = invoke.downgrade()?;
                    *co.get().root.lock().unwrap() = self.root_frag.downgrade()?;
                    f
                };
                table.insert(
                    k.id,
                    ChildShared {
                        frag: frag.clone(),
                        state,
                    },
                );
                order.push((k.id, frag.downgrade()?));
            }
            // prune dead + stale-generation associations
            if table
                .keys()
                .any(|id| live.get(&id.slot) != Some(&id.generation))
            {
                structure_changed = true;
            }
            table.retain(|id, _| live.get(&id.slot) == Some(&id.generation));
            self.native
                .lock()
                .unwrap()
                .retain(|slot, (g, _)| live.get(slot) == Some(g));
            *self.snap.live.lock().unwrap() = live;
            *self.snap.order.lock().unwrap() = order;
        }
        // notifications ride the public API — safe from any thread
        unsafe {
            for (frag, enabled) in enabled_flips {
                if let Ok(simple) = frag.cast::<IRawElementProviderSimple>() {
                    let _ = UiaRaiseAutomationPropertyChangedEvent(
                        &simple,
                        UIA_IsEnabledPropertyId,
                        &VARIANT::from(!enabled),
                        &VARIANT::from(enabled),
                    );
                }
            }
            if structure_changed && let Ok(simple) = self.root.cast::<IRawElementProviderSimple>() {
                let _ = UiaRaiseStructureChangedEvent(
                    &simple,
                    StructureChangeType_ChildrenInvalidated,
                    std::ptr::null_mut(),
                    0,
                );
            }
        }
        Ok(())
    }

    /// Register a native editor provider (built by the peer + site) —
    /// keyed on the full NodeId's generation so a replacement peer can
    /// never inherit the retired provider.
    pub(crate) fn register_native(
        &self,
        id: NodeId,
        frag: IRawElementProviderFragment,
    ) -> UiResult {
        self.native
            .lock()
            .unwrap()
            .insert(id.slot, (id.generation, frag));
        Ok(())
    }

    /// Runtime focus moved — record the owner so GetFocus/HasKeyboardFocus
    /// resolve through the live order, and raise the focus event off the
    /// focused fragment (external clients depend on the notification).
    pub(crate) fn set_focus(&self, id: Option<NodeId>) {
        let changed = *self.snap.focus.lock().unwrap() != id;
        *self.snap.focus.lock().unwrap() = id;
        if !changed {
            return;
        }
        let frag = id.and_then(|id| {
            self.snap
                .children
                .lock()
                .unwrap()
                .get(&id)
                .map(|c| c.frag.clone())
        });
        if let Some(frag) = frag
            && let Ok(simple) = frag.cast::<IRawElementProviderSimple>()
        {
            unsafe {
                let _ = UiaRaiseAutomationEvent(&simple, UIA_AutomationFocusChangedEventId);
            }
        }
    }

    /// Disconnect all UIA — the root unregisters from UIA's tables (a stale
    /// external reference to the ROOT dies too), then the closed flag +
    /// tables clear BEFORE the backend tears down peers and surfaces.
    pub(crate) fn close(&self) {
        self.snap
            .closed
            .store(true, std::sync::atomic::Ordering::SeqCst);
        unsafe {
            let _ = self
                .root
                .cast::<IRawElementProviderSimple>()
                .ok()
                .map(|r| UiaDisconnectProvider(&r));
        }
        self.snap.live.lock().unwrap().clear();
        self.snap.order.lock().unwrap().clear();
        self.snap.children.lock().unwrap().clear();
        self.native.lock().unwrap().clear();
    }

    pub(crate) fn provider(&self) -> IRawElementProviderFragmentRoot {
        self.root.clone()
    }
}
