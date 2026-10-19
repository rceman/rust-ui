//! Product font registration — the canonical Geist build ships as TTF
//! under `assets/fonts/geist/` (see its `UPSTREAM.md` for the frozen
//! geist@1.7.2 provenance). Registration is PRIVATE to the process
//! (`FR_PRIVATE | FR_NOT_ENUM`): nothing is installed globally, nothing
//! is fetched at runtime, and the font set is identical on every host.
//!
//! The DirectWrite factory is asked for an updated system collection
//! AFTER registration so `CreateTextFormat("Geist"/"Geist Mono")`
//! resolves the bundled faces. A failed/missing face is reported through
//! `geist()` — callers that need parity (the Gallery) fail closed rather
//! than silently substituting Segoe.

use std::path::PathBuf;
use std::sync::OnceLock;

use windows::Win32::Graphics::Gdi::{
    AddFontResourceExW, FONT_RESOURCE_CHARACTERISTICS, FR_NOT_ENUM, FR_PRIVATE,
};
use windows::core::PCWSTR;

/// Canonical family names — `FontFamily::Sans` / `FontFamily::Mono`.
pub(crate) const SANS_FAMILY: &[u16] = &[
    'G' as u16, 'e' as u16, 'i' as u16, 's' as u16, 't' as u16, 0,
];
pub(crate) const MONO_FAMILY: &[u16] = &[
    'G' as u16, 'e' as u16, 'i' as u16, 's' as u16, 't' as u16, ' ' as u16, 'M' as u16, 'o' as u16,
    'n' as u16, 'o' as u16, 0,
];

/// Which bundled faces the runtime found after private registration.
#[derive(Copy, Clone, Debug, Default)]
pub(crate) struct GeistStatus {
    pub sans: bool,
    pub mono: bool,
}

static STATUS: OnceLock<GeistStatus> = OnceLock::new();
static COLLECTION: OnceLock<Option<windows::Win32::Graphics::DirectWrite::IDWriteFontCollection>> =
    OnceLock::new();

/// Locate `assets/fonts/geist` — next to the exe, the crate manifest
/// dir (dev runs), or the current working directory. First hit wins.
fn font_dir() -> Option<PathBuf> {
    let mut cands = Vec::new();
    if let Ok(exe) = std::env::current_exe()
        && let Some(d) = exe.parent()
    {
        cands.push(d.join("assets/fonts/geist"));
        // target/{debug,release}/example → crate root is 3 levels up
        cands.push(d.join("../../assets/fonts/geist"));
        cands.push(d.join("../../../assets/fonts/geist"));
    }
    cands.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/fonts/geist"));
    cands.push(PathBuf::from("assets/fonts/geist"));
    cands.into_iter().find(|p| p.is_dir())
}

/// Register the bundled TTFs privately for this process and report which
/// canonical families DirectWrite can see afterwards. Idempotent.
pub(crate) fn register() -> GeistStatus {
    *STATUS.get_or_init(|| {
        let mut st = GeistStatus::default();
        let Some(dir) = font_dir() else {
            return st;
        };
        let register_one = |name: &str| -> bool {
            let path = dir.join(name);
            if !path.is_file() {
                return false;
            }
            let wide: Vec<u16> = path.to_string_lossy().encode_utf16().chain([0]).collect();
            unsafe {
                AddFontResourceExW(
                    PCWSTR(wide.as_ptr()),
                    FONT_RESOURCE_CHARACTERISTICS(FR_PRIVATE.0 | FR_NOT_ENUM.0),
                    None,
                ) > 0
            }
        };
        st.sans = register_one("Geist-Variable.ttf");
        st.mono = register_one("GeistMono-Variable.ttf");
        st
    })
}

/// Did the bundled Geist faces register? Parity-critical callers check
/// this rather than assuming the platform resolved a family name.
pub(crate) fn geist() -> GeistStatus {
    register()
}

/// A DirectWrite font collection that includes the privately registered
/// faces — refreshed after registration, cached for the process. `None`
/// when DirectWrite itself is unavailable (the renderer already fails
/// closed then).
pub(crate) fn collection(
    factory: &windows::Win32::Graphics::DirectWrite::IDWriteFactory,
) -> Option<windows::Win32::Graphics::DirectWrite::IDWriteFontCollection> {
    register();
    COLLECTION
        .get_or_init(|| unsafe {
            let mut coll = None;
            factory
                .GetSystemFontCollection(&mut coll as *mut _, true)
                .ok()?;
            coll
        })
        .clone()
}
