/// `ui.box_` props — opaque: authored `BoxStyle` + retained layout props,
/// so visual style never overwrites size inputs.
#[derive(Copy, Clone, Default, PartialEq)]
pub struct BoxProps {
    pub style: BoxStyle,
    pub(crate) layout: crate::geom::LayoutSpec,
    /// show/hide — hidden boxes stage but suspend children + paint
    pub(crate) visibility: crate::Visibility,
}

impl BoxProps {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn style(mut self, style: BoxStyle) -> Self {
        self.style = style;
        self
    }
    pub fn width(mut self, width: crate::geom::Length) -> Self {
        self.layout.width = width;
        self
    }
    pub fn height(mut self, height: crate::geom::Length) -> Self {
        self.layout.height = height;
        self
    }
    pub fn visibility(mut self, v: crate::Visibility) -> Self {
        self.visibility = v;
        self
    }
    pub fn min_width(mut self, v: Dp) -> Self {
        self.layout.min_width = Some(v);
        self
    }
    pub fn max_width(mut self, v: Dp) -> Self {
        self.layout.max_width = Some(v);
        self
    }
    /// Signed both-sides margin — negative values grow the border box
    /// beyond the allocated slot (frozen `-mx-1` breakout).
    pub fn margin_x(mut self, v: Dp) -> Self {
        self.layout.margin_x = v;
        self
    }
    pub fn margin_y(mut self, v: Dp) -> Self {
        self.layout.margin_y = v;
        self
    }
    pub fn min_height(mut self, v: Dp) -> Self {
        self.layout.min_height = Some(v);
        self
    }
    pub fn max_height(mut self, v: Dp) -> Self {
        self.layout.max_height = Some(v);
        self
    }
}

// ---------------------------------------------------------------------------
// resolution — deterministic ordered composition; OS enforcement is a
// separate final step the backend applies to resolved colors
// ---------------------------------------------------------------------------

/// Interaction branch selection — exclusive priority, never specificity.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum StyleState {
    Disabled,
    Pressed,
    Hover,
    Normal,
}

impl StyleState {
    pub fn classify(disabled: bool, pressed: bool, hover: bool) -> Self {
        if disabled {
            StyleState::Disabled
        } else if pressed {
            StyleState::Pressed
        } else if hover {
            StyleState::Hover
        } else {
            StyleState::Normal
        }
    }
}

// ---------------------------------------------------------------------------
// color resolution — the shared palette (renderers convert to their format)
// ---------------------------------------------------------------------------

/// shadcn neutral palette per `ColorRole` — the frozen
/// `reference/shadcn-gallery-v0.1/tokens.json` sRGB values, ONE authority.
/// `rgba()` decodes `#rrggbb` + token alpha; translucent roles bake the
/// frozen alpha in.
pub fn role_color(role: ColorRole, dark: bool) -> [f32; 4] {
    use ColorRole::*;
    const fn rgba(hex: u32, a: f32) -> [f32; 4] {
        [
            ((hex >> 16) & 0xff) as f32 / 255.0,
            ((hex >> 8) & 0xff) as f32 / 255.0,
            (hex & 0xff) as f32 / 255.0,
            a,
        ]
    }
    match (role, dark) {
        (Background, false) => rgba(0xffffff, 1.0),
        (Background, true) => rgba(0x0a0a0a, 1.0),
        (Foreground, false) => rgba(0x0a0a0a, 1.0),
        (Foreground, true) => rgba(0xfafafa, 1.0),
        (Card, false) => rgba(0xffffff, 1.0),
        (Card, true) => rgba(0x171717, 1.0),
        (CardForeground, false) => rgba(0x0a0a0a, 1.0),
        (CardForeground, true) => rgba(0xfafafa, 1.0),
        (Primary, false) => rgba(0x171717, 1.0),
        (Primary, true) => rgba(0xe5e5e5, 1.0),
        (PrimaryForeground, false) => rgba(0xfafafa, 1.0),
        (PrimaryForeground, true) => rgba(0x171717, 1.0),
        (Secondary, false) => rgba(0xf5f5f5, 1.0),
        (Secondary, true) => rgba(0x262626, 1.0),
        (SecondaryForeground, false) => rgba(0x171717, 1.0),
        (SecondaryForeground, true) => rgba(0xfafafa, 1.0),
        (Muted, false) => rgba(0xf5f5f5, 1.0),
        (Muted, true) => rgba(0x262626, 1.0),
        (MutedForeground, false) => rgba(0x737373, 1.0),
        (MutedForeground, true) => rgba(0xa1a1a1, 1.0),
        (Accent, false) => rgba(0xf5f5f5, 1.0),
        (Accent, true) => rgba(0x262626, 1.0),
        (AccentForeground, false) => rgba(0x171717, 1.0),
        (AccentForeground, true) => rgba(0xfafafa, 1.0),
        (Border, false) => rgba(0xe5e5e5, 1.0),
        (Border, true) => rgba(0xffffff, 0.10),
        (Input, false) => rgba(0xe5e5e5, 1.0),
        (Input, true) => rgba(0xffffff, 0.15),
        (Ring, false) => rgba(0xa1a1a1, 1.0),
        (Ring, true) => rgba(0x737373, 1.0),
        (Surface, false) => rgba(0xf8f8f8, 1.0),
        (Surface, true) => rgba(0x161616, 1.0),
        (SurfaceForeground, false) => rgba(0x0a0a0a, 1.0),
        (SurfaceForeground, true) => rgba(0xa1a1a1, 1.0),
        (Code, false) => rgba(0xf8f8f8, 1.0),
        (Code, true) => rgba(0x161616, 1.0),
        (CodeForeground, false) => rgba(0x0a0a0a, 1.0),
        (CodeForeground, true) => rgba(0xa1a1a1, 1.0),
        (CodeHighlight, false) => rgba(0xf2f2f2, 1.0),
        (CodeHighlight, true) => rgba(0x262626, 1.0),
        (CodeNumber, false) => rgba(0x747474, 1.0),
        (CodeNumber, true) => rgba(0xa4a4a4, 1.0),
        (Selection, false) => rgba(0x000000, 1.0),
        (Selection, true) => rgba(0xe5e5e5, 1.0),
        (SelectionForeground, false) => rgba(0xffffff, 1.0),
        (SelectionForeground, true) => rgba(0x171717, 1.0),
        (Destructive, false) => rgba(0xe7000b, 1.0),
        (Destructive, true) => rgba(0xff6467, 1.0),
        (DestructiveForeground, false) => rgba(0xfcf3f3, 1.0),
        (DestructiveForeground, true) => rgba(0xdf2225, 1.0),
        // enforcement focus indicator — frozen `ring-50`
        (Focus, false) => rgba(0xa1a1a1, 0.5),
        (Focus, true) => rgba(0x737373, 0.5),
        (Shadow, false) => rgba(0x000000, 0.10),
        (Shadow, true) => rgba(0x000000, 0.35),
        // ---- translucent tokens --------------------------------------
        (Foreground10, false) => rgba(0x0a0a0a, 0.10),
        (Foreground10, true) => rgba(0xfafafa, 0.10),
        (Foreground60, false) => rgba(0x0a0a0a, 0.60),
        (Foreground60, true) => rgba(0xfafafa, 0.60),
        (Muted40, false) => rgba(0xf5f5f5, 0.4),
        (Muted40, true) => rgba(0x262626, 0.4),
        (Muted50, false) => rgba(0xf5f5f5, 0.5),
        (Muted50, true) => rgba(0x262626, 0.5),
        (Input30, _) => rgba(0xffffff, 0.045),
        (Input50, _) => rgba(0xffffff, 0.075),
        (Primary80, false) => rgba(0x171717, 0.8),
        (Primary80, true) => rgba(0xe5e5e5, 0.8),
        (SecondaryHover, false) => rgba(0xe7e7e7, 1.0),
        (SecondaryHover, true) => rgba(0x2f2f2f, 1.0),
        (DestructiveTint, false) => rgba(0xe7000b, 0.10),
        (DestructiveTint, true) => rgba(0xff6467, 0.20),
        (DestructiveTintHover, false) => rgba(0xe7000b, 0.20),
        (DestructiveTintHover, true) => rgba(0xff6467, 0.30),
        (DestructiveRing, false) => rgba(0xe7000b, 0.20),
        (DestructiveRing, true) => rgba(0xff6467, 0.40),
        (Destructive40, false) => rgba(0xe7000b, 0.40),
        (Destructive40, true) => rgba(0xff6467, 0.40),
        (Destructive50, _) => rgba(0xff6467, 0.50),
        // `disabled:opacity-50` on the transparent/15% input interior:
        // light -> transparent over the page, dark -> the 4.5% wash halved
        (InputDisabled, false) => rgba(0xffffff, 0.0),
        (InputDisabled, true) => rgba(0xffffff, 0.0225),
        // `disabled:opacity-50` on `border-input` (light 0.922 solid,
        // dark white/15): the border fades with the whole control
        (InputDisabledBorder, false) => rgba(0xe5e5e5, 0.5),
        (InputDisabledBorder, true) => rgba(0xffffff, 0.075),
        // frozen `aria-invalid:border-destructive`: light full, dark /50
        (InputDestructive, false) => rgba(0xe7000b, 1.0),
        (InputDestructive, true) => rgba(0xff6467, 0.5),
        (Destructive90, false) => rgba(0xe7000b, 0.90),
        (Destructive90, true) => rgba(0xff6467, 0.90),
        (Sidebar, false) => rgba(0xfafafa, 1.0),
        (Sidebar, true) => rgba(0x171717, 1.0),
        (SidebarForeground, false) => rgba(0x0a0a0a, 1.0),
        (SidebarForeground, true) => rgba(0xfafafa, 1.0),
        (SidebarAccent, false) => rgba(0xf5f5f5, 1.0),
        (SidebarAccent, true) => rgba(0x262626, 1.0),
        (SidebarAccentForeground, false) => rgba(0x171717, 1.0),
        (SidebarAccentForeground, true) => rgba(0xfafafa, 1.0),
        (SidebarRing, false) => rgba(0xa1a1a1, 1.0),
        (SidebarRing, true) => rgba(0x737373, 1.0),
        (SidebarForeground70, false) => rgba(0x0a0a0a, 0.70),
        (SidebarForeground70, true) => rgba(0xfafafa, 0.70),
    }
}

/// Authored `Color` -> straight sRGB floats (backends premultiply as needed).
pub fn resolve_color(c: Color, dark: bool) -> [f32; 4] {
    match c {
        Color::Role(r) => role_color(r, dark),
        Color::Rgba { r, g, b, a } => [
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
            a as f32 / 255.0,
        ],
    }
}

// ---------------------------------------------------------------------------
// component recipes — ordinary inspectable functions/tables, theme -> values
// ---------------------------------------------------------------------------

use crate::theme::{ButtonVariant, ControlSize};

/// Semantic OS-control color slots. Under forced colors a role resolves to
/// a SYSTEM color slot; the platform adapter turns the slot into the real
/// OS RGB (GetSysColor on Windows). The mapping itself is shared policy —
/// a backend never invents its own role->system routing.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum SystemColor {
    /// COLOR_WINDOW — page/canvas background
    Window,
    /// COLOR_WINDOWTEXT — primary text on Window
    WindowText,
    /// COLOR_HIGHLIGHT — selection/accent fill
    Highlight,
    /// COLOR_HIGHLIGHTTEXT — text on Highlight
    HighlightText,
    /// COLOR_GRAYTEXT — disabled/muted text
    GrayText,
    /// COLOR_INACTIVEBORDER — secondary strokes/dividers
    InactiveBorder,
    /// COLOR_HOTLIGHT — interactive affordance
    HotTrack,
}

/// The shared role -> system-slot mapping used under forced colors.
/// Covers the whole `ColorRole` vocabulary — a role can never escape
/// enforcement.
pub fn system_slot(role: crate::theme::ColorRole) -> SystemColor {
    use crate::theme::ColorRole as R;
    match role {
        R::Background
        | R::Muted
        | R::Card
        | R::Surface
        | R::Code
        | R::Secondary
        | R::Muted50
        | R::Muted40
        | R::DestructiveTint
        | R::DestructiveTintHover
        | R::Input30
        | R::Input50
        | R::InputDisabled
        | R::SecondaryHover
        | R::Sidebar
        | R::SidebarAccent => SystemColor::Window,
        R::InputDestructive => SystemColor::Highlight,
        R::Foreground
        | R::Destructive
        | R::Destructive90
        | R::CardForeground
        | R::SecondaryForeground
        | R::SurfaceForeground
        | R::CodeForeground
        | R::CodeHighlight
        | R::Primary
        | R::DestructiveForeground => SystemColor::WindowText,
        R::SidebarForeground | R::SidebarAccentForeground => SystemColor::WindowText,
        R::MutedForeground
        | R::CodeNumber
        | R::Shadow
        | R::Foreground60
        | R::SidebarForeground70 => SystemColor::GrayText,
        R::Accent
        | R::Focus
        | R::Ring
        | R::Primary80
        | R::Selection
        | R::DestructiveRing
        | R::Destructive40
        | R::Destructive50 => SystemColor::Highlight,
        R::SidebarRing => SystemColor::Highlight,
        R::AccentForeground | R::PrimaryForeground | R::SelectionForeground => {
            SystemColor::HighlightText
        }
        R::Border | R::Input | R::InputDisabledBorder | R::Foreground10 => {
            SystemColor::InactiveBorder
        }
    }
}

/// OS/accessibility enforcement — applied AFTER all recipe+consumer
/// layers. Forced-colors/high-contrast: decorative shadow resolves to
/// `None` (the approved contract); required focus visibility lives in the
/// focus enforcement layer and survives. Colors keep their role
/// resolution — under forced colors the OS-level palette follows.
fn enforce_color(c: &mut Color, sys: &dyn Fn(SystemColor) -> [f32; 4]) {
    if let Color::Role(r) = c {
        let [r8, g8, b8, a8] = sys(system_slot(*r));
        *c = Color::rgba(
            (r8 * 255.0).round() as u8,
            (g8 * 255.0).round() as u8,
            (b8 * 255.0).round() as u8,
            (a8 * 255.0).round() as u8,
        );
    }
}

fn enforce_box_colors(b: &mut BoxStyle, sys: &dyn Fn(SystemColor) -> [f32; 4]) {
    enforce_color(&mut b.background, sys);
    for side in [
        &mut b.border.top,
        &mut b.border.right,
        &mut b.border.bottom,
        &mut b.border.left,
    ] {
        enforce_color(&mut side.color, sys);
    }
    if let Some(s) = &mut b.shadow {
        enforce_color(&mut s.color, sys);
    }
}

/// `forced_resolver` = the platform's system-color lookup — `None` keeps
/// the authored palette (forced-colors off). `Some` rewrites every role to
/// its system slot's real RGB and removes decorative shadow.
pub(crate) fn os_enforce_box(
    b: &mut BoxStyle,
    forced_resolver: Option<&dyn Fn(SystemColor) -> [f32; 4]>,
) {
    if let Some(sys) = forced_resolver {
        b.shadow = None;
        enforce_box_colors(b, sys);
    }
}
pub(crate) fn os_enforce_visual(
    v: &mut VisualStyle,
    forced_resolver: Option<&dyn Fn(SystemColor) -> [f32; 4]>,
) {
    os_enforce_box(&mut v.box_style, forced_resolver);
    if let Some(sys) = forced_resolver {
        enforce_color(&mut v.text_style.foreground, sys);
        v.text_style.size = TextSize::Body; // OS text size wins
    }
}

