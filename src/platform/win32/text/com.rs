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

