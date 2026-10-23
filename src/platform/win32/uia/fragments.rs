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
            x if x == UIA_AutomationIdPropertyId => Ok(VARIANT::from(BSTR::from(
                &*self.state.automation_id.lock().unwrap(),
            ))),
            x if x == UIA_NamePropertyId => {
                Ok(VARIANT::from(BSTR::from(&*self.state.name.lock().unwrap())))
            }
            x if x == UIA_RuntimeIdPropertyId => Ok(unsafe { runtime_id_variant(self.node) }),
            x if x == UIA_ControlTypePropertyId => Ok(VARIANT::from(self.control_type.0 as i32)),
            x if x == UIA_LocalizedControlTypePropertyId => {
                Ok(VARIANT::from(BSTR::from(self.localized)))
            }
            x if x == UIA_IsEnabledPropertyId => Ok(VARIANT::from(self.enabled())),
            x if x == UIA_SelectionItemIsSelectedPropertyId => Ok(self
                .state
                .selected
                .lock()
                .unwrap()
                .map(VARIANT::from)
                .unwrap_or_default()),
            x if x == UIA_ToggleToggleStatePropertyId => Ok(self
                .state
                .checked
                .lock()
                .unwrap()
                .map(|checked| VARIANT::from(if checked { 1i32 } else { 0i32 }))
                .unwrap_or_default()),
            x if x == UIA_ExpandCollapseExpandCollapseStatePropertyId => Ok(self
                .state
                .expanded
                .lock()
                .unwrap()
                .map(|expanded| VARIANT::from(if expanded { 1i32 } else { 0i32 }))
                .unwrap_or_default()),
            x if x == UIA_IsKeyboardFocusablePropertyId => {
                Ok(VARIANT::from(self.actionable() && self.enabled()))
            }
            // scroll-clip truth — a fully-clipped element must not
            // masquerade as on-screen content (VT_EMPTY reads as
            // "not offscreen")
            x if x == UIA_IsOffscreenPropertyId => Ok(VARIANT::from(
                self.state
                    .offscreen
                    .load(std::sync::atomic::Ordering::SeqCst),
            )),
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

