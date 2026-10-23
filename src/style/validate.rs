/// resolved `BoxStyle` — checked before paint/layout use

pub(crate) fn box_style_ok(s: &BoxStyle) -> bool {
    insets_ok(&s.padding)
        && radii_ok(&s.radii)
        && border_ok(&s.border)
        && s.shadow.as_ref().is_none_or(|sh| shadow_ok(sh))
}
/// resolved `VisualStyle`
pub(crate) fn visual_style_ok(s: &VisualStyle) -> bool {
    box_style_ok(&s.box_style) && text_ok(&s.text_style)
}
/// sparse box patch — every authored field checked
pub(crate) fn box_patch_ok(p: &BoxStylePatch) -> bool {
    let dp_opt = |d: &Option<Dp>| d.is_none_or(|d| dp_ok(&d));
    let sides_ok = [
        &p.border.top,
        &p.border.right,
        &p.border.bottom,
        &p.border.left,
    ]
    .iter()
    .all(|s| dp_opt(&s.width));
    let radii_ok = [
        &p.radii.top_left,
        &p.radii.top_right,
        &p.radii.bottom_right,
        &p.radii.bottom_left,
    ]
    .iter()
    .all(|r| dp_opt(r));
    let pad_ok = [
        &p.padding.top,
        &p.padding.right,
        &p.padding.bottom,
        &p.padding.left,
    ]
    .iter()
    .all(|i| dp_opt(i));
    let sh_ok = match &p.shadow {
        ShadowPatch::Set(s) => shadow_ok(s),
        _ => true,
    };
    sides_ok && radii_ok && pad_ok && sh_ok
}
/// sparse text patch — Exact sizes must be finite+positive
pub(crate) fn text_patch_ok(p: &TextStylePatch) -> bool {
    p.size.is_none_or(|d| d.0.is_finite() && d.0 > 0.0)
}
/// sparse visual patch
pub(crate) fn visual_patch_ok(p: &VisualStylePatch) -> bool {
    box_patch_ok(&p.box_style) && text_patch_ok(&p.text_style)
}
/// full `ButtonStylePatch` — base plus every state branch
pub(crate) fn button_patch_ok(p: &ButtonStylePatch) -> bool {
    visual_patch_ok(&p.styles.base)
        && [
            &p.styles.hover,
            &p.styles.pressed,
            &p.styles.disabled,
            &p.styles.focus_visible,
        ]
        .iter()
        .all(|s| s.is_none_or(|s| visual_patch_ok(&s)))
}
/// `TextInputStylePatch` — chrome fields validated; foreground opaque
/// checks happen against the resolved color (role → palette) at commit.
pub(crate) fn text_input_patch_ok(p: &TextInputStylePatch) -> bool {
    box_patch_ok(&p.chrome)
}

/// v0.1 opacity rule: peer backing (chrome background) and the editable
/// foreground must resolve fully opaque — translucent values would let
/// live editing pixels blend through and are `UiError::Unsupported`.
pub(crate) fn text_input_opaque(b: &BoxStyle, fg: Color, dark: bool) -> bool {
    resolve_color(b.background, dark)[3] >= 1.0 && resolve_color(fg, dark)[3] >= 1.0
}

/// `ActionStyle` — base + optional state patches
pub(crate) fn action_style_ok(s: &ActionStyle) -> bool {
    box_style_ok(&s.base)
        && [&s.hover, &s.pressed, &s.disabled, &s.focus_visible]
            .iter()
            .all(|p| p.is_none_or(|p| box_patch_ok(&p)))
}

// ---------------------------------------------------------------------------
// forced-colors — the semantic slot mapping is shared; the platform owns RGB
// ---------------------------------------------------------------------------

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
        R::Border | R::Input | R::Foreground10 => SystemColor::InactiveBorder,
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

/// Does a box patch touch any field that feeds layout insets (padding) or
/// content-consuming border widths? Shadow/background/color never do.
pub(crate) fn patch_touches_layout(p: &BoxStylePatch) -> bool {
    p.padding.top.is_some()
        || p.padding.right.is_some()
        || p.padding.bottom.is_some()
        || p.padding.left.is_some()
        || p.border.top.width.is_some()
        || p.border.right.width.is_some()
        || p.border.bottom.width.is_some()
        || p.border.left.width.is_some()
}

// ---------------------------------------------------------------------------
// Slice-3 component recipes — select popup chrome, tabs list, select parts.
// ---------------------------------------------------------------------------

/// `cn-select-content` chrome patch — `bg-popover ring-1
/// ring-foreground/10 shadow-md rounded-lg` at the frozen p-1 content
/// lane (3dp patch padding + the 1dp ring border = 4dp effective inset).
/// The single-shadow primitive carries the dominant `shadow-md` layer
/// (`0 4px 6px -1px`); the `ring-1` edge is the 1dp `foreground/10`
/// border — same 10%-alpha 1px ink the CSS ring paints.
pub fn select_content_patch() -> BoxStylePatch {
    let mut p = BoxStylePatch::new();
    p.background = Some(Color::Role(ColorRole::Card));
    p.border = p.border.all(BorderSidePatch {
        width: Some(Dp(1.0)),
        color: Some(Color::Role(ColorRole::Foreground10)),
    });
    p.radii = CornerRadiiPatch {
        top_left: Some(Dp(10.0)),
        top_right: Some(Dp(10.0)),
        bottom_right: Some(Dp(10.0)),
        bottom_left: Some(Dp(10.0)),
    };
    p.padding = InsetsPatch::all(Dp(3.0));
    p.shadow = ShadowPatch::Set(Shadow {
        color: Color::Role(ColorRole::Shadow),
        offset_x: Dp(0.0),
        offset_y: Dp(4.0),
        blur_sigma: Dp(3.0),
        spread: Dp(-1.0),
    });
    p
}

/// `cn-tabs-list` chrome — `rounded-lg p-[3px]` muted pill (default) or
/// transparent `rounded-none` lane (line). Height/gap are staged by the
/// list's layout spec, not the chrome.
pub fn tabs_list_recipe(variant: crate::theme::TabsVariant) -> BoxStyle {
    BoxStyle {
        background: match variant {
            crate::theme::TabsVariant::Default => Color::Role(ColorRole::Muted),
            crate::theme::TabsVariant::Line => Color::rgba(0, 0, 0, 0),
        },
        border: Border::default(),
        radii: CornerRadii::all(Dp(match variant {
            crate::theme::TabsVariant::Default => 10.0,
            crate::theme::TabsVariant::Line => 0.0,
        })),
        padding: Insets::all(Dp(3.0)),
        shadow: None,
    }
}

/// `cn-select-label` — group label text (12px muted-foreground).
pub fn select_label_style() -> TextStyle {
    TextStyle {
        foreground: Color::Role(ColorRole::MutedForeground),
        size: TextSize::Exact(Dp(12.0)),
        weight: TextWeight::Normal,
        line_height: Some(Dp(16.0)),
        ..TextStyle::default()
    }
}

/// `cn-select-value` placeholder tint — the `data-placeholder` state is
/// staged: `muted-foreground` until the control holds a value.
pub fn select_placeholder_patch() -> ButtonStylePatch {
    let mut p = ButtonStylePatch::new();
    p.styles.base.text_style.foreground = Some(Color::Role(ColorRole::MutedForeground));
    p
}
