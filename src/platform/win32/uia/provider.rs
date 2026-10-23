fn fence_provider(
    inner: IRawElementProviderSimple,
    snap: &Arc<Snapshot>,
    node: NodeId,
) -> IRawElementProviderSimple {
    let co = windows_core::ComObject::new(FencedProvider {
        inner,
        frag: OnceLock::new(),
        snap: snap.clone(),
        node,
    });
    co.to_interface()
}

/// Provider fence — wraps a native `IRawElementProviderSimple` that
/// escaped our fragment (enclosing element, children arrays). Its own
/// fragment surface is lazily QI'd and equally fenced.
#[implement(IRawElementProviderSimple, IRawElementProviderFragment)]
struct FencedProvider {
    inner: IRawElementProviderSimple,
    /// inner QI'd for IRawElementProviderFragment — cached once
    frag: OnceLock<Option<IRawElementProviderFragment>>,
    snap: Arc<Snapshot>,
    node: NodeId,
}

impl FencedProvider {
    fn frag(&self) -> Option<IRawElementProviderFragment> {
        self.frag
            .get_or_init(|| self.inner.cast::<IRawElementProviderFragment>().ok())
            .clone()
    }
}

impl IRawElementProviderSimple_Impl for FencedProvider_Impl {
    fn ProviderOptions(&self) -> Result<ProviderOptions> {
        if !fence_live(&self.this.snap, self.this.node) {
            return Err(unavailable());
        }
        Ok(ProviderOptions_ServerSideProvider | ProviderOptions_UseComThreading)
    }
    fn GetPatternProvider(&self, patternid: UIA_PATTERN_ID) -> Result<IUnknown> {
        if !fence_live(&self.this.snap, self.this.node) {
            return Err(unavailable());
        }
        let raw = unsafe { self.this.inner.GetPatternProvider(patternid) }?;
        // re-fence known patterns; anything else is fence-or-nothing
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
        Err(E_NOTIMPL.into())
    }
    fn GetPropertyValue(&self, propertyid: UIA_PROPERTY_ID) -> Result<VARIANT> {
        if !fence_live(&self.this.snap, self.this.node) {
            return Err(unavailable());
        }
        unsafe { self.this.inner.GetPropertyValue(propertyid) }
    }
    fn HostRawElementProvider(&self) -> Result<IRawElementProviderSimple> {
        if !fence_live(&self.this.snap, self.this.node) {
            return Err(unavailable());
        }
        let raw = unsafe { self.this.inner.HostRawElementProvider() }?;
        Ok(fence_provider(raw, &self.this.snap, self.this.node))
    }
}

impl IRawElementProviderFragment_Impl for FencedProvider_Impl {
    fn Navigate(&self, direction: NavigateDirection) -> Result<IRawElementProviderFragment> {
        if !fence_live(&self.this.snap, self.this.node) {
            return Err(unavailable());
        }
        let Some(frag) = self.this.frag() else {
            return Err(E_NOTIMPL.into());
        };
        let raw = unsafe { frag.Navigate(direction) }?;
        Ok(fence_fragment(raw, &self.this.snap, self.this.node))
    }
    fn GetRuntimeId(&self) -> Result<*mut SAFEARRAY> {
        if !fence_live(&self.this.snap, self.this.node) {
            return Err(unavailable());
        }
        match self.this.frag() {
            Some(f) => unsafe { f.GetRuntimeId() },
            None => Err(E_NOTIMPL.into()),
        }
    }
    fn BoundingRectangle(&self) -> Result<UiaRect> {
        if !fence_live(&self.this.snap, self.this.node) {
            return Err(unavailable());
        }
        match self.this.frag() {
            Some(f) => unsafe { f.BoundingRectangle() },
            None => Err(E_NOTIMPL.into()),
        }
    }
    fn GetEmbeddedFragmentRoots(&self) -> Result<*mut SAFEARRAY> {
        if !fence_live(&self.this.snap, self.this.node) {
            return Err(unavailable());
        }
        match self.this.frag() {
            Some(f) => {
                let sa = unsafe { f.GetEmbeddedFragmentRoots() }?;
                unsafe { fence_provider_array(sa, &self.this.snap, self.this.node) }
            }
            None => Err(E_NOTIMPL.into()),
        }
    }
    fn SetFocus(&self) -> Result<()> {
        if !fence_live(&self.this.snap, self.this.node) {
            return Err(unavailable());
        }
        match self.this.frag() {
            Some(f) => unsafe { f.SetFocus() },
            None => Err(E_NOTIMPL.into()),
        }
    }
    fn FragmentRoot(&self) -> Result<IRawElementProviderFragmentRoot> {
        if !fence_live(&self.this.snap, self.this.node) {
            return Err(unavailable());
        }
        match self.this.frag() {
            Some(f) => unsafe { f.FragmentRoot() },
            None => Err(E_NOTIMPL.into()),
        }
    }
}

/// Wrap a native fragment — returned to clients as the provider surface.
fn fence_fragment(
    frag: IRawElementProviderFragment,
    snap: &Arc<Snapshot>,
    node: NodeId,
) -> IRawElementProviderFragment {
    let simple: IRawElementProviderSimple = frag.cast().unwrap();
    let co = windows_core::ComObject::new(FencedProvider {
        inner: simple,
        frag: {
            let l = OnceLock::new();
            let _ = l.set(Some(frag));
            l
        },
        snap: snap.clone(),
        node,
    });
    co.to_interface()
}

// ---------------------------------------------------------------------------
// fragment root — HWND-backed, active-order children
// ---------------------------------------------------------------------------

