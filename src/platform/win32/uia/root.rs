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
    pub automation_id: String,
    pub id: NodeId,
    pub name: String,
    pub ct: UIA_CONTROLTYPE_ID,
    pub localized: &'static str,
    pub rect: UiaRect,
    /// actionable — exposes IInvokeProvider (press factories)
    pub actionable: bool,
    /// !disabled && visible
    pub enabled: bool,
    /// rect ∩ viewport clip is empty — nothing of the node is on screen
    pub offscreen: bool,
    pub selected: Option<bool>,
    pub checked: Option<bool>,
    pub expanded: Option<bool>,
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

