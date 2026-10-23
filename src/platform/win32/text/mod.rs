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
//! Host units are PHYSICAL PIXELS at the host DC's DPI — verified
//! empirically: under a DPI-aware process msftedit divides `lprcBounds`
//! by `dcDpi/96` to get target-logical units (at 125% DIP bounds rendered
//! at arg/1.25); under an unaware process dcDpi=96 makes px==DIP. The
//! peer's local space (`TxGetClientRect`, `lprcBounds`, caret, lparams)
//! is therefore px; `host.bounds`/`draw`/`natural_size` keep DIP at the
//! Rust boundary and convert at the seam. `TxGetExtent` is HIMETRIC
//! (`himetric = px * 2540/dcDpi = dip * 2540/96`).
//!
//! Object layout: msftedit assumes a C++ single-inheritance host — it calls
//! `ITextHost2` methods through the `ITextHost` pointer. The host carries ONE
//! vtbl (the full `ITextHost2` vtbl, OFFSET 0); `QueryInterface` answers
//! `IUnknown`/`ITextHost`/`ITextHost2` all with `base+0`.

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::sync::Arc;

use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D_SIZE_U, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR, D2D1_COMPATIBLE_RENDER_TARGET_OPTIONS_NONE,
    ID2D1BitmapRenderTarget, ID2D1RenderTarget,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::UI::Controls::EM_LIMITTEXT;
use windows::Win32::UI::Controls::RichEdit::*;
use windows::Win32::UI::Input::Ime::*;
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::*;

use super::space::{LogicalRect, ScaleFactor};
use crate::text::{BindingToken, TextRevision};
use crate::{NodeId, UiError, UiResult};

/// missing from the windows bindings

/// missing from the windows bindings
const EM_GETLINECOUNT: u32 = 0x00BA; // standard edit message
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

include!("host.rs");
include!("com.rs");
include!("hostbox.rs");
include!("peer_a.rs");
include!("peer_b.rs");
include!("peer_c.rs");
