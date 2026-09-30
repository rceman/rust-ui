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

use std::collections::{HashMap, HashSet};
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
pub(crate) struct Snapshot {
    closed: std::sync::atomic::AtomicBool,
    /// slot -> live generation (retained active order rebuilt per tree)
    live: Mutex<HashMap<u32, u64>>,
    /// active-order children — providers only (weak, to break cycles)
    order: Mutex<Vec<(NodeId, Weak<IRawElementProviderFragment>)>>,
    hwnd: HWND,
    /// posted-action sink — press/focus ride PostMessage, never direct calls
    post_hwnd: HWND,
    /// actual focus owner as a weak fragment (set on runtime focus events)
    focus: Mutex<Weak<IRawElementProviderFragment>>,
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
}

impl IRawElementProviderWindowlessSite_Impl for WindowlessSite_Impl {
    fn GetAdjacentFragment(
        &self,
        direction: NavigateDirection,
    ) -> Result<IRawElementProviderFragment> {
        if direction == NavigateDirection_Parent
            && let Some(r) = self.root.lock().unwrap().upgrade()
        {
            return Ok(r);
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

#[implement(
    IRawElementProviderSimple,
    IRawElementProviderFragment,
    IInvokeProvider
)]
struct PaintFragment {
    snap: Arc<Snapshot>,
    node: NodeId,
    name: String,
    control_type: UIA_CONTROLTYPE_ID,
    localized: &'static str,
    rect: Mutex<UiaRect>,
    invoke: Mutex<Weak<IInvokeProvider>>,
    root: Mutex<Weak<IRawElementProviderFragment>>,
}

impl PaintFragment {
    fn live(&self) -> bool {
        self.snap.live_ok(self.node.slot, self.node.generation)
    }
}

impl IRawElementProviderSimple_Impl for PaintFragment_Impl {
    fn ProviderOptions(&self) -> Result<ProviderOptions> {
        Ok(ProviderOptions_ServerSideProvider | ProviderOptions_UseComThreading)
    }
    fn GetPatternProvider(&self, patternid: UIA_PATTERN_ID) -> Result<IUnknown> {
        if patternid == UIA_InvokePatternId {
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
            x if x == UIA_NamePropertyId => Ok(VARIANT::from(BSTR::from(&*self.name))),
            x if x == UIA_ControlTypePropertyId => Ok(VARIANT::from(self.control_type.0 as i32)),
            x if x == UIA_LocalizedControlTypePropertyId => {
                Ok(VARIANT::from(BSTR::from(self.localized)))
            }
            _ => Ok(VARIANT::default()),
        }
    }
    fn HostRawElementProvider(&self) -> Result<IRawElementProviderSimple> {
        unsafe { UiaHostProviderFromHwnd(self.snap.hwnd) }
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
                .and_then(|(_, w)| w.upgrade())
                .ok_or_else(|| E_NOTIMPL.into()),
            NavigateDirection_PreviousSibling => idx
                .and_then(|i| i.checked_sub(1))
                .and_then(|i| order.get(i))
                .and_then(|(_, w)| w.upgrade())
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
        unsafe {
            let sa = SafeArrayCreateVector(VT_I4, 0, 3);
            if sa.is_null() {
                return Err(E_OUTOFMEMORY.into());
            }
            let vals = [
                UiaAppendRuntimeId as i32,
                self.node.slot as i32,
                self.node.generation as i32,
            ];
            for (i, v) in vals.iter().enumerate() {
                let _ = SafeArrayPutElement(sa, &(i as i32), v as *const i32 as *const c_void);
            }
            Ok(sa)
        }
    }
    fn BoundingRectangle(&self) -> Result<UiaRect> {
        if !self.live() {
            return Err(windows::core::Error::from_hresult(HRESULT(
                UIA_E_ELEMENTNOTAVAILABLE as i32,
            )));
        }
        Ok(*self.rect.lock().unwrap())
    }
    fn GetEmbeddedFragmentRoots(&self) -> Result<*mut SAFEARRAY> {
        Ok(std::ptr::null_mut())
    }
    fn SetFocus(&self) -> Result<()> {
        if !self.live() {
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
        if !self.live() {
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
                .and_then(|(_, w)| w.upgrade())
                .ok_or_else(|| E_NOTIMPL.into()),
            NavigateDirection_LastChild => order
                .last()
                .and_then(|(_, w)| w.upgrade())
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
        Ok(std::ptr::null_mut())
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
    fn ElementProviderFromPoint(&self, _x: f64, _y: f64) -> Result<IRawElementProviderFragment> {
        Err(E_NOTIMPL.into())
    }
    fn GetFocus(&self) -> Result<IRawElementProviderFragment> {
        self.snap
            .focus
            .lock()
            .unwrap()
            .upgrade()
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
    /// editor slot -> its native provider's fragment (kept so next/prev
    /// navigation can answer while the peer lives)
    native: Mutex<HashMap<u32, IRawElementProviderFragment>>,
}

impl UiaRoot {
    pub(crate) fn new(hwnd: HWND, title: &str) -> windows::core::Result<UiaRoot> {
        let snap = Arc::new(Snapshot {
            closed: std::sync::atomic::AtomicBool::new(false),
            live: Mutex::new(HashMap::new()),
            order: Mutex::new(Vec::new()),
            hwnd,
            post_hwnd: hwnd,
            focus: Mutex::new(Weak::new()),
        });
        let site_co = windows_core::ComObject::new(WindowlessSite {
            snap: snap.clone(),
            root: Mutex::new(Weak::new()),
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
    pub(crate) fn site(&self) -> windows::core::Result<IRawElementProviderWindowlessSite> {
        let site_co = windows_core::ComObject::new(WindowlessSite {
            snap: self.snap.clone(),
            root: Mutex::new(Weak::new()),
        });
        let site: IRawElementProviderWindowlessSite = site_co.to_interface();
        *site_co.get().root.lock().unwrap() = self.root_frag.downgrade()?;
        Ok(site)
    }

    /// Rebuild the live set + active-order children from the retained tree.
    /// Each call re-mints the live map, invalidating any retired provider's
    /// generation before its next accessor runs.
    pub(crate) fn rebuild(&self, children: Vec<ChildBuild>) -> windows::core::Result<()> {
        // native providers stay registered (editors persist across rebuilds)
        let mut live = HashMap::new();
        let mut order = Vec::new();
        for k in children {
            live.insert(k.id.slot, k.id.generation);
            if let Some(native) = k.native {
                order.push((k.id, native.downgrade()?));
                continue;
            }
            let co = windows_core::ComObject::new(PaintFragment {
                snap: self.snap.clone(),
                node: k.id,
                name: k.name,
                control_type: k.ct,
                localized: k.localized,
                rect: Mutex::new(k.rect),
                invoke: Mutex::new(Weak::new()),
                root: Mutex::new(Weak::new()),
            });
            let frag: IRawElementProviderFragment = co.to_interface();
            let invoke: IInvokeProvider = co.to_interface();
            *co.get().invoke.lock().unwrap() = invoke.downgrade()?;
            *co.get().root.lock().unwrap() = self.root_frag.downgrade()?;
            order.push((k.id, frag.downgrade()?));
        }
        *self.snap.live.lock().unwrap() = live;
        *self.snap.order.lock().unwrap() = order;
        Ok(())
    }

    /// Register a native editor provider (built by the peer + site) into the
    /// child order at the right active position.
    pub(crate) fn register_native(&self, slot: u32, frag: IRawElementProviderFragment) -> UiResult {
        self.native.lock().unwrap().insert(slot, frag);
        Ok(())
    }

    /// Disconnect all UIA — closed flag + provider tables drop BEFORE the
    /// backend tears down text peers and surfaces.
    pub(crate) fn close(&self) {
        self.snap
            .closed
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.snap.live.lock().unwrap().clear();
        self.snap.order.lock().unwrap().clear();
        self.native.lock().unwrap().clear();
    }

    pub(crate) fn provider(&self) -> IRawElementProviderFragmentRoot {
        self.root.clone()
    }
}
