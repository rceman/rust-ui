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

// ---------------------------------------------------------------------------
// style preflight — finite numbers, nonnegative geometry, positive text size,
/// checked before ANY native mutation at commit time
// ---------------------------------------------------------------------------

fn dp_ok(d: &Dp) -> bool {
    d.0.is_finite() && d.0 >= 0.0
}
fn insets_ok(i: &Insets) -> bool {
    dp_ok(&i.top) && dp_ok(&i.right) && dp_ok(&i.bottom) && dp_ok(&i.left)
}
fn radii_ok(r: &CornerRadii) -> bool {
    dp_ok(&r.top_left) && dp_ok(&r.top_right) && dp_ok(&r.bottom_right) && dp_ok(&r.bottom_left)
}
fn border_ok(b: &Border) -> bool {
    [&b.top, &b.right, &b.bottom, &b.left]
        .iter()
        .all(|s| dp_ok(&s.width))
}
fn shadow_ok(s: &Shadow) -> bool {
    s.offset_x.0.is_finite()
        && s.offset_y.0.is_finite()
        && s.blur_sigma.0.is_finite()
        && s.blur_sigma.0 >= 0.0
}
fn text_ok(t: &TextStyle) -> bool {
    let size_ok = match t.size {
        TextSize::Body => true,
        TextSize::Exact(d) => d.0.is_finite() && d.0 > 0.0,
    };
    let lh_ok = t.line_height.is_none_or(|d| d.0.is_finite() && d.0 > 0.0);
    size_ok && lh_ok && t.letter_spacing.is_finite()
}

