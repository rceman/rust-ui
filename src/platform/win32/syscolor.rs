use super::*;

pub(crate) fn os_appearance() -> Appearance {
    use windows::Win32::System::Registry::*;
    use windows::Win32::UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW};
    use windows::Win32::UI::WindowsAndMessaging::{SPI_GETHIGHCONTRAST, SystemParametersInfoW};
    let mut dark = false;
    let mut forced = false;
    unsafe {
        let mut key = HKEY::default();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            None,
            KEY_READ,
            &mut key,
        )
        .is_ok()
        {
            let mut val = 0u32;
            let mut len = 4u32;
            if RegGetValueW(
                key,
                PCWSTR::null(),
                w!("AppsUseLightTheme"),
                RRF_RT_REG_DWORD,
                None,
                Some(&mut val as *mut u32 as *mut c_void),
                Some(&mut len),
            )
            .is_ok()
            {
                dark = val == 0;
            }
            let _ = RegCloseKey(key);
        }
        let mut hc = HIGHCONTRASTW::default();
        hc.cbSize = std::mem::size_of::<HIGHCONTRASTW>() as u32;
        if SystemParametersInfoW(
            SPI_GETHIGHCONTRAST,
            0,
            Some(&mut hc as *mut _ as *mut c_void),
            Default::default(),
        )
        .is_ok()
        {
            forced = hc.dwFlags.contains(HCF_HIGHCONTRASTON);
        }
    }
    Appearance {
        dark,
        forced_colors: forced,
    }
}

/// GetSysColor-backed forced-colors resolver — the OS palette for the
/// shared SystemColor slots. Installed on the runtime so the classifier
/// and renderer resolve the same concrete colors.
pub(crate) fn sys_color(sc: crate::style::SystemColor) -> [f32; 4] {
    use windows::Win32::Graphics::Gdi::{GetSysColor, SYS_COLOR_INDEX};
    let idx = match sc {
        crate::style::SystemColor::Window => SYS_COLOR_INDEX(5), // COLOR_WINDOW
        crate::style::SystemColor::WindowText => SYS_COLOR_INDEX(8), // COLOR_WINDOWTEXT
        crate::style::SystemColor::Highlight => SYS_COLOR_INDEX(13), // COLOR_HIGHLIGHT
        crate::style::SystemColor::HighlightText => SYS_COLOR_INDEX(14), // COLOR_HIGHLIGHTTEXT
        crate::style::SystemColor::GrayText => SYS_COLOR_INDEX(17), // COLOR_GRAYTEXT
        crate::style::SystemColor::InactiveBorder => SYS_COLOR_INDEX(11), // COLOR_INACTIVEBORDER
        crate::style::SystemColor::HotTrack => SYS_COLOR_INDEX(26), // COLOR_HOTLIGHT
    };
    let rgb = unsafe { GetSysColor(idx) };
    [
        (rgb & 0xff) as f32 / 255.0,
        ((rgb >> 8) & 0xff) as f32 / 255.0,
        ((rgb >> 16) & 0xff) as f32 / 255.0,
        1.0,
    ]
}

/// Effective reduced motion: Reduce=true, NoPreference=false, System reads
/// SPI_GETCLIENTAREAANIMATION.
pub(super) fn effective_reduced(theme: &Theme, _appearance: &Appearance) -> bool {
    match theme.reduced_motion {
        ReducedMotion::Reduce => true,
        ReducedMotion::NoPreference => false,
        ReducedMotion::System => unsafe {
            let mut v = windows_core::BOOL::default();
            let _ = windows::Win32::UI::WindowsAndMessaging::SystemParametersInfoW(
                SPI_GETCLIENTAREAANIMATION,
                0,
                Some(&mut v as *mut BOOL as *mut c_void),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            );
            !v.as_bool()
        },
    }
}
