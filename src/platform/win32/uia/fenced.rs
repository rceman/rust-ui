fn fence_live(snap: &Arc<Snapshot>, node: NodeId) -> bool {
    snap.live_ok(node.slot, node.generation)
}

fn unavailable() -> Error {
    Error::from_hresult(HRESULT(UIA_E_ELEMENTNOTAVAILABLE as i32))
}

/// Wrap a range in the generation fence; a cached range on a dead node
/// returns UIA_E_ELEMENTNOTAVAILABLE instead of reaching stale msftedit
/// state.
fn fence_range(
    inner: ITextRangeProvider,
    snap: &Arc<Snapshot>,
    node: NodeId,
) -> Result<ITextRangeProvider> {
    let co = windows_core::ComObject::new(FencedRange {
        inner,
        snap: snap.clone(),
        node,
    });
    Ok(co.to_interface())
}

/// SAFEARRAY of ITextRangeProvider -> same shape, each element fenced.
/// OWNERSHIP: `sa` is ours on entry — the native provider transferred it;
/// we take the elements' AddRef'd values, then destroy the array
/// (both on success and on every failure path via `guard`). The
/// replacement array is likewise destroyed if the build aborts.
unsafe fn fence_range_array(
    sa: *mut SAFEARRAY,
    snap: &Arc<Snapshot>,
    node: NodeId,
) -> Result<*mut SAFEARRAY> {
    if sa.is_null() {
        return Ok(std::ptr::null_mut());
    }
    // `sa` is destroyed exactly once — guard covers every `?` below
    struct SaGuard(*mut SAFEARRAY);
    impl SaGuard {
        /// transfer ownership to the caller — forget the guard, keep array
        fn disarm(self) -> *mut SAFEARRAY {
            let p = self.0;
            std::mem::forget(self);
            p
        }
    }
    impl Drop for SaGuard {
        fn drop(&mut self) {
            unsafe {
                let _ = SafeArrayDestroy(self.0);
            }
        }
    }
    let _owned_in = SaGuard(sa);
    let lo = SafeArrayGetLBound(sa, 1)?;
    let hi = SafeArrayGetUBound(sa, 1)?;
    let n = (hi - lo + 1).max(0) as u32;
    let out = SafeArrayCreateVector(VT_UNKNOWN, 0, n);
    if out.is_null() {
        return Err(E_OUTOFMEMORY.into());
    }
    // `out` is destroyed on ANY error until explicitly disarmed
    let owned_out = SaGuard(out);
    for i in 0..n {
        let idx = lo + i as i32;
        let mut raw: *mut c_void = std::ptr::null_mut();
        // VT_UNKNOWN elements: GetElement hands an AddRef'd IUnknown
        SafeArrayGetElement(sa, &idx, &mut raw as *mut _ as *mut c_void)?;
        if raw.is_null() {
            continue;
        }
        let inner = ITextRangeProvider::from_raw(raw);
        if let Ok(fenced) = fence_range(inner, snap, node) {
            // VT_UNKNOWN insertion takes the INTERFACE POINTER VALUE —
            // PutElement AddRefs it itself. `&ptr` would be an extra
            // indirection (a stack address read as the COM pointer —
            // access violation). `fenced`'s owned ref drops normally;
            // the array holds its own reference.
            let ptr = windows_core::Interface::as_raw(&fenced);
            SafeArrayPutElement(out, &idx, ptr as *const c_void)?;
        }
    }
    Ok(owned_out.disarm())
}

/// SAFEARRAY of IRawElementProviderSimple -> same shape, each element
/// fenced through `fence_provider` — same ownership contract as
/// fence_range_array. `pub(crate)` so the F07 fixture can drive the real
/// path against a nonempty array of genuine native provider references.
pub(crate) unsafe fn fence_provider_array(
    sa: *mut SAFEARRAY,
    snap: &Arc<Snapshot>,
    node: NodeId,
) -> Result<*mut SAFEARRAY> {
    if sa.is_null() {
        return Ok(std::ptr::null_mut());
    }
    struct SaGuard(*mut SAFEARRAY);
    impl SaGuard {
        fn disarm(self) -> *mut SAFEARRAY {
            let p = self.0;
            std::mem::forget(self);
            p
        }
    }
    impl Drop for SaGuard {
        fn drop(&mut self) {
            unsafe {
                let _ = SafeArrayDestroy(self.0);
            }
        }
    }
    let _owned_in = SaGuard(sa);
    let lo = SafeArrayGetLBound(sa, 1)?;
    let hi = SafeArrayGetUBound(sa, 1)?;
    let n = (hi - lo + 1).max(0) as u32;
    let out = SafeArrayCreateVector(VT_UNKNOWN, 0, n);
    if out.is_null() {
        return Err(E_OUTOFMEMORY.into());
    }
    let owned_out = SaGuard(out);
    for i in 0..n {
        let idx = lo + i as i32;
        let mut raw: *mut c_void = std::ptr::null_mut();
        SafeArrayGetElement(sa, &idx, &mut raw as *mut _ as *mut c_void)?;
        if raw.is_null() {
            continue;
        }
        let inner = IRawElementProviderSimple::from_raw(raw);
        let fenced = fence_provider(inner, snap, node);
        // direct interface pointer — PutElement AddRefs; `fenced` drops
        // normally, releasing only the wrapper's own ref
        let ptr = windows_core::Interface::as_raw(&fenced);
        SafeArrayPutElement(out, &idx, ptr as *const c_void)?;
    }
    Ok(owned_out.disarm())
}

/// Text-pattern fence — all range-returning calls wrap their results;
/// GetEnclosingElement's provider is msftedit's OWN (escapes the fence
/// through the fragment's parenting anyway).
#[implement(ITextProvider)]
struct FencedText {
    inner: ITextProvider,
    snap: Arc<Snapshot>,
    node: NodeId,
}
impl ITextProvider_Impl for FencedText_Impl {
    fn GetSelection(&self) -> Result<*mut SAFEARRAY> {
        if !fence_live(&self.this.snap, self.this.node) {
            return Err(unavailable());
        }
        unsafe {
            let sa = self.this.inner.GetSelection()?;
            fence_range_array(sa, &self.this.snap, self.this.node)
        }
    }
    fn GetVisibleRanges(&self) -> Result<*mut SAFEARRAY> {
        if !fence_live(&self.this.snap, self.this.node) {
            return Err(unavailable());
        }
        unsafe {
            let sa = self.this.inner.GetVisibleRanges()?;
            fence_range_array(sa, &self.this.snap, self.this.node)
        }
    }
    fn RangeFromChild(
        &self,
        childelement: windows_core::Ref<IRawElementProviderSimple>,
    ) -> Result<ITextRangeProvider> {
        if !fence_live(&self.this.snap, self.this.node) {
            return Err(unavailable());
        }
        let r = unsafe { self.this.inner.RangeFromChild(childelement.ok()?) }?;
        fence_range(r, &self.this.snap, self.this.node)
    }
    fn RangeFromPoint(&self, point: &UiaPoint) -> Result<ITextRangeProvider> {
        if !fence_live(&self.this.snap, self.this.node) {
            return Err(unavailable());
        }
        let r = unsafe { self.this.inner.RangeFromPoint(*point) }?;
        fence_range(r, &self.this.snap, self.this.node)
    }
    fn DocumentRange(&self) -> Result<ITextRangeProvider> {
        if !fence_live(&self.this.snap, self.this.node) {
            return Err(unavailable());
        }
        let r = unsafe { self.this.inner.DocumentRange() }?;
        fence_range(r, &self.this.snap, self.this.node)
    }
    fn SupportedTextSelection(&self) -> Result<SupportedTextSelection> {
        if !fence_live(&self.this.snap, self.this.node) {
            return Err(unavailable());
        }
        unsafe { self.this.inner.SupportedTextSelection() }
    }
}

/// Value-pattern fence — IsReadOnly/Value mirror the live snapshot anyway.
#[implement(IValueProvider)]
struct FencedValue {
    inner: IValueProvider,
    snap: Arc<Snapshot>,
    node: NodeId,
}
impl IValueProvider_Impl for FencedValue_Impl {
    fn SetValue(&self, val: &PCWSTR) -> Result<()> {
        if !fence_live(&self.this.snap, self.this.node) {
            return Err(unavailable());
        }
        unsafe { self.this.inner.SetValue(*val) }
    }
    fn Value(&self) -> Result<BSTR> {
        if !fence_live(&self.this.snap, self.this.node) {
            return Err(unavailable());
        }
        unsafe { self.this.inner.Value() }
    }
    fn IsReadOnly(&self) -> Result<BOOL> {
        if !fence_live(&self.this.snap, self.this.node) {
            return Err(unavailable());
        }
        unsafe { self.this.inner.IsReadOnly() }
    }
}

/// Private marker — a fenced range can carry its inner out when another
/// rust-ui call needs the RAW msftedit range (Compare/MoveEndpointByRange
/// take a second range; msftedit must receive its own object).
#[windows::core::interface("6E4B2C91-4A3F-4E2B-9D5C-7F8A1B2C3D4E")]
unsafe trait IFencedRangeInner: windows::core::IUnknown {
    /// yields the raw msftedit range ONLY if this fenced range is still
    /// live — null means dead (a dead argument must never reach msftedit)
    unsafe fn inner_raw(&self) -> *mut core::ffi::c_void;
}

/// Unwrap a fenced range to its inner msftedit range — a foreign
/// (non-fenced) range passes through unchanged.
unsafe fn unwrap_range(r: &ITextRangeProvider) -> Result<ITextRangeProvider> {
    match r.cast::<IFencedRangeInner>() {
        Ok(m) => {
            // a STALE fenced argument is rejected at its own fence —
            // UIA_E_ELEMENTNOTAVAILABLE, not a generic pointer failure
            let raw = unsafe { m.inner_raw() };
            if raw.is_null() {
                return Err(unavailable());
            }
            Ok(ITextRangeProvider::from_raw(raw))
        }
        Err(_) => Ok(r.clone()),
    }
}

/// Range fence — every method checks liveness before touching msftedit;
/// range-returning calls re-wrap so cloned/sub-ranges stay fenced too.
#[implement(ITextRangeProvider, IFencedRangeInner)]
struct FencedRange {
    inner: ITextRangeProvider,
    snap: Arc<Snapshot>,
    node: NodeId,
}
impl IFencedRangeInner_Impl for FencedRange_Impl {
    unsafe fn inner_raw(&self) -> *mut core::ffi::c_void {
        if !fence_live(&self.this.snap, self.this.node) {
            return std::ptr::null_mut();
        }
        // ownership transfers — the caller wraps it with from_raw
        windows_core::Interface::into_raw(self.this.inner.clone())
    }
}

impl ITextRangeProvider_Impl for FencedRange_Impl {
    fn Clone(&self) -> Result<ITextRangeProvider> {
        if !fence_live(&self.this.snap, self.this.node) {
            return Err(unavailable());
        }
        fence_range(
            unsafe { self.this.inner.Clone() }?,
            &self.this.snap,
            self.this.node,
        )
    }
    fn Compare(&self, range: windows_core::Ref<ITextRangeProvider>) -> Result<BOOL> {
        if !fence_live(&self.this.snap, self.this.node) {
            return Err(unavailable());
        }
        let raw = unsafe { unwrap_range(range.ok()?) }?;
        unsafe { self.this.inner.Compare(&raw) }
    }
    fn CompareEndpoints(
        &self,
        endpoint: TextPatternRangeEndpoint,
        targetrange: windows_core::Ref<ITextRangeProvider>,
        targetendpoint: TextPatternRangeEndpoint,
    ) -> Result<i32> {
        if !fence_live(&self.this.snap, self.this.node) {
            return Err(unavailable());
        }
        let raw = unsafe { unwrap_range(targetrange.ok()?) }?;
        unsafe {
            self.this
                .inner
                .CompareEndpoints(endpoint, &raw, targetendpoint)
        }
    }
    fn ExpandToEnclosingUnit(&self, unit: TextUnit) -> Result<()> {
        if !fence_live(&self.this.snap, self.this.node) {
            return Err(unavailable());
        }
        unsafe { self.this.inner.ExpandToEnclosingUnit(unit) }
    }
    fn FindAttribute(
        &self,
        attributeid: UIA_TEXTATTRIBUTE_ID,
        val: &VARIANT,
        backward: BOOL,
    ) -> Result<ITextRangeProvider> {
        if !fence_live(&self.this.snap, self.this.node) {
            return Err(unavailable());
        }
        fence_range(
            unsafe {
                self.this
                    .inner
                    .FindAttribute(attributeid, val, backward.as_bool())
            }?,
            &self.this.snap,
            self.this.node,
        )
    }
    fn FindText(
        &self,
        text: &BSTR,
        backward: BOOL,
        ignorecase: BOOL,
    ) -> Result<ITextRangeProvider> {
        if !fence_live(&self.this.snap, self.this.node) {
            return Err(unavailable());
        }
        fence_range(
            unsafe {
                self.this
                    .inner
                    .FindText(text, backward.as_bool(), ignorecase.as_bool())
            }?,
            &self.this.snap,
            self.this.node,
        )
    }
    fn GetAttributeValue(&self, attributeid: UIA_TEXTATTRIBUTE_ID) -> Result<VARIANT> {
        if !fence_live(&self.this.snap, self.this.node) {
            return Err(unavailable());
        }
        unsafe { self.this.inner.GetAttributeValue(attributeid) }
    }
    fn GetBoundingRectangles(&self) -> Result<*mut SAFEARRAY> {
        if !fence_live(&self.this.snap, self.this.node) {
            return Err(unavailable());
        }
        unsafe { self.this.inner.GetBoundingRectangles() }
    }
    fn GetEnclosingElement(&self) -> Result<IRawElementProviderSimple> {
        if !fence_live(&self.this.snap, self.this.node) {
            return Err(unavailable());
        }
        // the enclosing provider is msftedit's OWN — re-fence it so a
        // retained client reference can't reach patterns past the gate
        let raw = unsafe { self.this.inner.GetEnclosingElement() }?;
        Ok(fence_provider(raw, &self.this.snap, self.this.node))
    }
    fn GetText(&self, maxlength: i32) -> Result<BSTR> {
        if !fence_live(&self.this.snap, self.this.node) {
            return Err(unavailable());
        }
        unsafe { self.this.inner.GetText(maxlength) }
    }
    fn Move(&self, unit: TextUnit, count: i32) -> Result<i32> {
        if !fence_live(&self.this.snap, self.this.node) {
            return Err(unavailable());
        }
        unsafe { self.this.inner.Move(unit, count) }
    }
    fn MoveEndpointByUnit(
        &self,
        endpoint: TextPatternRangeEndpoint,
        unit: TextUnit,
        count: i32,
    ) -> Result<i32> {
        if !fence_live(&self.this.snap, self.this.node) {
            return Err(unavailable());
        }
        unsafe { self.this.inner.MoveEndpointByUnit(endpoint, unit, count) }
    }
    fn MoveEndpointByRange(
        &self,
        endpoint: TextPatternRangeEndpoint,
        targetrange: windows_core::Ref<ITextRangeProvider>,
        targetendpoint: TextPatternRangeEndpoint,
    ) -> Result<()> {
        if !fence_live(&self.this.snap, self.this.node) {
            return Err(unavailable());
        }
        let raw = unsafe { unwrap_range(targetrange.ok()?) }?;
        unsafe {
            self.this
                .inner
                .MoveEndpointByRange(endpoint, &raw, targetendpoint)
        }
    }
    fn Select(&self) -> Result<()> {
        if !fence_live(&self.this.snap, self.this.node) {
            return Err(unavailable());
        }
        unsafe { self.this.inner.Select() }
    }
    fn AddToSelection(&self) -> Result<()> {
        if !fence_live(&self.this.snap, self.this.node) {
            return Err(unavailable());
        }
        unsafe { self.this.inner.AddToSelection() }
    }
    fn RemoveFromSelection(&self) -> Result<()> {
        if !fence_live(&self.this.snap, self.this.node) {
            return Err(unavailable());
        }
        unsafe { self.this.inner.RemoveFromSelection() }
    }
    fn ScrollIntoView(&self, aligntotop: BOOL) -> Result<()> {
        if !fence_live(&self.this.snap, self.this.node) {
            return Err(unavailable());
        }
        unsafe { self.this.inner.ScrollIntoView(aligntotop.as_bool()) }
    }
    fn GetChildren(&self) -> Result<*mut SAFEARRAY> {
        if !fence_live(&self.this.snap, self.this.node) {
            return Err(unavailable());
        }
        // each child provider is msftedit's — the returned array holds
        // fenced copies; the native array is destroyed, never leaked
        let sa = unsafe { self.this.inner.GetChildren() }?;
        unsafe { fence_provider_array(sa, &self.this.snap, self.this.node) }
    }
}

