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
    /// Patterns a client can retain must stay behind the generation
    /// fence — a cached native pattern that outlives its node would act on
    /// dead state. The wrapper forwards only while `live_ok` passes and
    /// re-fences every range/pattern it hands out.
    fn GetPatternProvider(&self, patternid: UIA_PATTERN_ID) -> Result<IUnknown> {
        if !self.this.live() {
            return Err(EditorFragment::unavailable());
        }
        let raw = unsafe { self.this.inner_simple.GetPatternProvider(patternid) }?;
        if patternid == UIA_TextPatternId {
            let inner: ITextProvider = raw.cast()?;
            let co = windows_core::ComObject::new(FencedText {
                inner,
                snap: self.this.snap.clone(),
                node: self.this.node,
            });
            return Ok(co.to_interface::<IUnknown>());
        }
        if patternid == UIA_ValuePatternId {
            let inner: IValueProvider = raw.cast()?;
            let co = windows_core::ComObject::new(FencedValue {
                inner,
                snap: self.this.snap.clone(),
                node: self.this.node,
            });
            return Ok(co.to_interface::<IUnknown>());
        }
        // any other pattern msftedit serves would escape the fence — the
        // contract is fence-or-nothing, not "pass through and hope"
        Err(E_NOTIMPL.into())
    }
    fn GetPropertyValue(&self, propertyid: UIA_PROPERTY_ID) -> Result<VARIANT> {
        if !self.this.live() {
            return Err(EditorFragment::unavailable());
        }
        match propertyid {
            x if x == UIA_AutomationIdPropertyId => Ok(VARIANT::from(BSTR::from(
                &*self.this.state.automation_id.lock().unwrap(),
            ))),
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
            // OUR answer, not msftedit's — the delegate has no
            // viewport-clip knowledge on a windowless site
            x if x == UIA_IsOffscreenPropertyId => Ok(VARIANT::from(
                self.this
                    .state
                    .offscreen
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
        if !self.this.live() {
            return Err(EditorFragment::unavailable());
        }
        unsafe { self.this.inner.FragmentRoot() }
    }
}

// ---------------------------------------------------------------------------
// fenced native patterns — every COM reference that escapes to a client
// carries the SAME generation/liveness check as the fragment itself
// ---------------------------------------------------------------------------

