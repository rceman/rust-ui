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
use std::sync::{Arc, Mutex, OnceLock};

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

/// UiaRect -> VT_R8|VT_ARRAY VARIANT (the UIA BoundingRectangle shape).
fn bounding_variant(r: UiaRect) -> VARIANT {
    unsafe {
        let sa = SafeArrayCreateVector(VT_R8, 0, 4);
        for (i, v) in [r.left, r.top, r.width, r.height].iter().enumerate() {
            let _ = SafeArrayPutElement(sa, &(i as i32), v as *const f64 as *const c_void);
        }
        let mut var = VARIANT::default();
        let m = &mut *(&mut var as *mut VARIANT as *mut VARIANT_MANUAL);
        m.parray = sa;
        m.vt = (VT_ARRAY | VT_R8).0 as u16;
        var
    }
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

include!("site.rs");
include!("fragments.rs");
include!("editor.rs");
include!("fenced.rs");
include!("provider.rs");
include!("root.rs");
include!("uia_root.rs");
