// ---------------------------------------------------------------------------
// Shadcn CORE slice — theme tokens, component recipes, icon geometry
// ---------------------------------------------------------------------------

#[test]
fn frozen_light_palette_matches_tokens() {
    use crate::style::{Color, resolve_color};
    use crate::theme::ColorRole;
    // frozen tokens.json light family — the sRGB the contract binds
    let px = |r: u8, g: u8, b: u8, a: u8| {
        [
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
            a as f32 / 255.0,
        ]
    };
    assert_eq!(
        resolve_color(Color::Role(ColorRole::Background), false),
        px(255, 255, 255, 255)
    );
    assert_eq!(
        resolve_color(Color::Role(ColorRole::Foreground), false),
        px(10, 10, 10, 255)
    );
    assert_eq!(
        resolve_color(Color::Role(ColorRole::Primary), false),
        px(23, 23, 23, 255)
    );
    assert_eq!(
        resolve_color(Color::Role(ColorRole::PrimaryForeground), false),
        px(250, 250, 250, 255)
    );
    assert_eq!(
        resolve_color(Color::Role(ColorRole::Secondary), false),
        px(245, 245, 245, 255)
    );
    assert_eq!(
        resolve_color(Color::Role(ColorRole::MutedForeground), false),
        px(115, 115, 115, 255)
    );
    assert_eq!(
        resolve_color(Color::Role(ColorRole::Destructive), false),
        px(231, 0, 11, 255)
    );
    assert_eq!(
        resolve_color(Color::Role(ColorRole::Border), false),
        px(229, 229, 229, 255)
    );
    assert_eq!(
        resolve_color(Color::Role(ColorRole::Ring), false),
        px(161, 161, 161, 255)
    );
    assert_eq!(
        resolve_color(Color::Role(ColorRole::Surface), false),
        px(248, 248, 248, 255)
    );
}

#[test]
fn frozen_dark_palette_matches_tokens() {
    use crate::style::{Color, resolve_color};
    use crate::theme::ColorRole;
    let px = |r: u8, g: u8, b: u8, a: u8| {
        [
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
            a as f32 / 255.0,
        ]
    };
    assert_eq!(
        resolve_color(Color::Role(ColorRole::Background), true),
        px(10, 10, 10, 255)
    );
    assert_eq!(
        resolve_color(Color::Role(ColorRole::Foreground), true),
        px(250, 250, 250, 255)
    );
    assert_eq!(
        resolve_color(Color::Role(ColorRole::Card), true),
        px(23, 23, 23, 255)
    );
    assert_eq!(
        resolve_color(Color::Role(ColorRole::Primary), true),
        px(229, 229, 229, 255)
    );
    assert_eq!(
        resolve_color(Color::Role(ColorRole::Secondary), true),
        px(38, 38, 38, 255)
    );
    assert_eq!(
        resolve_color(Color::Role(ColorRole::Destructive), true),
        px(255, 100, 103, 255)
    );
    assert_eq!(
        resolve_color(Color::Role(ColorRole::MutedForeground), true),
        px(161, 161, 161, 255)
    );
    assert_eq!(
        resolve_color(Color::Role(ColorRole::Surface), true),
        px(22, 22, 22, 255)
    );
    assert_eq!(
        resolve_color(Color::Role(ColorRole::Ring), true),
        px(115, 115, 115, 255)
    );
}

#[test]
fn translucent_roles_match_tokens() {
    use crate::style::{Color, resolve_color};
    use crate::theme::ColorRole;
    // ring/50 = ring alpha 0.5 both modes; fg/10 the card ring
    let r = resolve_color(Color::Role(ColorRole::Focus), false);
    assert!((r[3] - 0.5).abs() < 0.01, "ring50 light alpha {r:?}");
    let f10 = resolve_color(Color::Role(ColorRole::Foreground10), false);
    assert!((f10[0] - 10.0 / 255.0).abs() < 0.01 && (f10[3] - 0.1).abs() < 0.01);
    let d = resolve_color(Color::Role(ColorRole::Foreground10), true);
    assert!((d[0] - 250.0 / 255.0).abs() < 0.01 && (d[3] - 0.1).abs() < 0.01);
}

#[test]
fn button_size_heights_match_contract() {
    use crate::theme::ControlSize;
    assert_eq!(ControlSize::Xs.height().0, 24.0);
    assert_eq!(ControlSize::Sm.height().0, 28.0);
    assert_eq!(ControlSize::Md.height().0, 32.0);
    assert_eq!(ControlSize::Lg.height().0, 36.0);
}

#[test]
fn button_primary_normal_is_frozen() {
    use crate::style::{ButtonStylePatch, Color, StyleState, resolve_button, resolve_color};
    use crate::theme::{ButtonVariant, ColorRole, ControlSize};
    let vs = resolve_button(
        ButtonVariant::Primary,
        ControlSize::Md,
        &ButtonStylePatch::default(),
        StyleState::Normal,
        false,
        false,
        false,
        false,
    );
    // bg-primary 171717, fg fafafa, pad 10 sides, r10, 1px transparent border
    assert_eq!(
        resolve_color(vs.box_style.background, false),
        resolve_color(Color::Role(ColorRole::Primary), false)
    );
    assert_eq!(
        resolve_color(vs.text_style.foreground, false),
        resolve_color(Color::Role(ColorRole::PrimaryForeground), false)
    );
    assert_eq!(vs.box_style.padding.left.0, 10.0);
    assert_eq!(vs.box_style.padding.right.0, 10.0);
    assert_eq!(vs.box_style.radii.top_left.0, 10.0);
    assert_eq!(vs.box_style.border.top.width.0, 1.0);
    assert_eq!(
        vs.text_style.size,
        crate::style::TextSize::Exact(crate::geom::Dp(14.0))
    );
    assert_eq!(vs.text_style.weight, crate::style::TextWeight::Medium);
}

#[test]
fn button_hover_pressed_states_differ() {
    use crate::style::{ButtonStylePatch, Color, StyleState, resolve_button, resolve_color};
    use crate::theme::{ButtonVariant, ControlSize};
    let p = ButtonStylePatch::default();
    let n = resolve_button(
        ButtonVariant::Primary,
        ControlSize::Md,
        &p,
        StyleState::Normal,
        false,
        false,
        false,
        false,
    );
    let h = resolve_button(
        ButtonVariant::Primary,
        ControlSize::Md,
        &p,
        StyleState::Hover,
        false,
        false,
        false,
        false,
    );
    // hover:bg-primary/80 — same hue at 80% alpha, distinct from normal
    let nb = resolve_color(n.box_style.background, false);
    let hb = resolve_color(h.box_style.background, false);
    assert_ne!(nb, hb);
    assert!(
        (hb[3] - nb[3] * 0.8).abs() < 0.02,
        "hover should be primary@80%"
    );
}

#[test]
fn invalid_is_not_destructive_variant() {
    use crate::style::{ButtonStylePatch, Color, StyleState, resolve_button, resolve_color};
    use crate::theme::{ButtonVariant, ControlSize};
    let p = ButtonStylePatch::default();
    let inv = resolve_button(
        ButtonVariant::Primary,
        ControlSize::Md,
        &p,
        StyleState::Normal,
        false,
        false,
        false,
        true,
    );
    let des = resolve_button(
        ButtonVariant::Destructive,
        ControlSize::Md,
        &p,
        StyleState::Normal,
        false,
        false,
        false,
        false,
    );
    let ib = resolve_color(inv.box_style.border.top.color, false);
    let db = resolve_color(des.box_style.border.top.color, false);
    // invalid = destructive BORDER + ring chrome on the primary surface;
    // destructive variant is a different background — not interchangeable
    assert_eq!(
        resolve_color(inv.box_style.background, false),
        resolve_color(Color::Role(ColorRole::Primary), false)
    );
    assert_ne!(
        resolve_color(des.box_style.background, false),
        resolve_color(Color::Role(ColorRole::Primary), false)
    );
    use crate::theme::ColorRole;
    let dcol = resolve_color(Color::Role(ColorRole::Destructive), false);
    assert!((ib[0] - dcol[0]).abs() < 0.01);
    let _ = db;
}

#[test]
fn icon_button_rect_centered() {
    use crate::layout::button_icon_rect;
    use crate::node::NodeData;
    use std::rc::Rc;
    // NodeData-only harness — button_icon_rect reads the data + resolved
    // style, so a full retained Node is unnecessary
    let n = NodeData::Button {
        text: Rc::from(""),
        variant: crate::theme::ButtonVariant::Primary,
        style: Default::default(),
        disabled: false,
        size: crate::theme::ControlSize::Md,
        motion: None,
        tooltip: None,
        icon_start: Some(crate::icons::Icon::ArrowRight),
        icon_end: None,
        icon_only: true,
        accessible_name: Some(Rc::from("next")),
        invalid: false,
        selected: false,
        expanded: None,
        icon_color: None,
    };
    let vs = crate::style::resolve_button(
        crate::theme::ButtonVariant::Primary,
        crate::theme::ControlSize::Md,
        &Default::default(),
        crate::style::StyleState::Normal,
        false,
        false,
        false,
        false,
    );
    let r = crate::geom::Rect {
        x: 0.0,
        y: 0.0,
        width: 32.0,
        height: 32.0,
    };
    // button_icon_rect takes a &Node — call through a helper instead
    let ir = crate::layout::button_icon_rect(&n, &vs, &r, false).unwrap();
    assert_eq!(ir.width, 16.0);
    assert_eq!(ir.x, 8.0);
    assert_eq!(ir.y, 8.0);
}

#[test]
fn icon_inline_start_geometry_matches_contract() {
    use crate::node::NodeData;
    use std::rc::Rc;
    let n = NodeData::Button {
        text: Rc::from("Add item"),
        variant: crate::theme::ButtonVariant::Primary,
        style: Default::default(),
        disabled: false,
        size: crate::theme::ControlSize::Md,
        motion: None,
        tooltip: None,
        icon_start: Some(crate::icons::Icon::ArrowLeftCircle),
        icon_end: None,
        icon_only: false,
        accessible_name: None,
        invalid: false,
        selected: false,
        expanded: None,
        icon_color: None,
    };
    let vs = crate::style::resolve_button(
        crate::theme::ButtonVariant::Primary,
        crate::theme::ControlSize::Md,
        &Default::default(),
        crate::style::StyleState::Normal,
        false,
        false,
        false,
        false,
    );
    let r = crate::geom::Rect {
        x: 0.0,
        y: 0.0,
        width: 100.5,
        height: 32.0,
    };
    let ir = crate::layout::button_icon_rect(&n, &vs, &r, false).unwrap();
    // frozen: icon @x9,y8 — border(1)+pad(10-2=8)=9, vcenter
    assert_eq!(ir.x, 9.0, "icon x {}", ir.x);
    assert_eq!(ir.y, 8.0, "icon y {}", ir.y);
    assert_eq!(ir.width, 16.0);
}

#[test]
fn card_recipe_is_frozen() {
    use crate::style::{card_recipe, card_spacing};
    use crate::theme::CardSize;
    let c = card_recipe(CardSize::Default);
    assert_eq!(c.radii.top_left.0, 14.0);
    let s = c.shadow.unwrap();
    assert_eq!(s.blur_sigma.0, 0.0);
    assert_eq!(s.spread.0, 1.0);
    assert_eq!(card_spacing(CardSize::Default).0, 16.0);
    assert_eq!(card_spacing(CardSize::Sm).0, 12.0);
}

#[test]
fn typography_recipes_match_contract() {
    use crate::style::TextSize;
    use crate::typography::Typography;
    let sz = |t: Typography| match t.style().size {
        TextSize::Exact(d) => d.0,
        _ => 0.0,
    };
    assert_eq!(sz(Typography::H1), 48.0);
    assert_eq!(sz(Typography::H2), 30.0);
    assert_eq!(sz(Typography::H3), 24.0);
    assert_eq!(sz(Typography::H4), 20.0);
    assert_eq!(sz(Typography::Paragraph), 16.0);
    assert_eq!(sz(Typography::Lead), 20.0);
    assert_eq!(sz(Typography::Small), 14.0);
    assert_eq!(sz(Typography::Muted), 14.0);
    assert_eq!(Typography::H1.style().line_height.unwrap().0, 48.0);
    assert_eq!(Typography::Paragraph.style().line_height.unwrap().0, 28.0);
    assert!((Typography::H1.style().letter_spacing - (-1.2)).abs() < 0.01);
    let ch = Typography::H2.chrome().unwrap();
    assert_eq!(ch.border.bottom.width.0, 1.0);
    assert_eq!(ch.padding.bottom.0, 8.0);
    assert_eq!(
        Typography::InlineCode.style().font,
        crate::theme::FontFamily::Mono
    );
    assert!(Typography::InlineCode.chrome().is_some());
    assert_eq!(
        Typography::InlineCode.style().weight,
        crate::style::TextWeight::Semibold
    );
}

#[test]
fn button_dirty_classification_catches_icon_fields() {
    use crate::node::NodeData;
    use std::rc::Rc;
    let mk = |icon_start, invalid| NodeData::Button {
        text: Rc::from("x"),
        variant: crate::theme::ButtonVariant::Primary,
        style: Default::default(),
        disabled: false,
        size: crate::theme::ControlSize::Md,
        motion: None,
        tooltip: None,
        icon_start,
        icon_end: None,
        icon_only: false,
        accessible_name: None,
        invalid,
        selected: false,
        expanded: None,
        icon_color: None,
    };
    let plain = mk(None, false);
    let icon = mk(Some(crate::icons::Icon::ArrowRight), false);
    let inv = mk(None, true);
    assert_ne!(crate::node::dirty_diff(&plain, &icon, false, None), 0);
    assert_ne!(crate::node::dirty_diff(&plain, &inv, false, None), 0);
}

// ==================== slice-1 visual rework — recipe tests ===================

#[test]
fn button_content_insets_reserves_icon_space() {
    use crate::theme::{ButtonVariant, ControlSize};
    let vs = crate::style::resolve_button(
        ButtonVariant::Primary,
        ControlSize::Md,
        &Default::default(),
        crate::style::StyleState::Normal,
        false,
        false,
        false,
        false,
    );
    let mk = |icon_start, icon_end| NodeData::Button {
        text: Rc::from("x"),
        variant: ButtonVariant::Primary,
        style: Default::default(),
        disabled: false,
        size: ControlSize::Md,
        motion: None,
        tooltip: None,
        icon_start,
        icon_end,
        icon_only: false,
        accessible_name: None,
        invalid: false,
        selected: false,
        expanded: None,
        icon_color: None,
    };
    // icon_start: left inset = border(1) + reduced-pad(8) + icon(16) + gap(6) = 31
    let s =
        crate::layout::button_content_insets(&mk(Some(crate::icons::Icon::ArrowRight), None), &vs);
    assert!(
        (s.left.0 - 31.0).abs() < 0.01,
        "icon_start left {}",
        s.left.0
    );
    // icon_end: right inset = 1 + 8 + 16 + 6 = 31
    let e =
        crate::layout::button_content_insets(&mk(None, Some(crate::icons::Icon::ArrowRight)), &vs);
    assert!(
        (e.right.0 - 31.0).abs() < 0.01,
        "icon_end right {}",
        e.right.0
    );
    // no icon: plain padding+border insets
    let n = crate::layout::button_content_insets(&mk(None, None), &vs);
    assert!(n.left.0 > 0.0 && n.left.0 < 31.0);
}

#[test]
fn invalid_button_gets_destructive_ring_recipe() {
    use crate::theme::{ButtonVariant, ColorRole, ControlSize};
    for v in [
        ButtonVariant::Primary,
        ButtonVariant::Secondary,
        ButtonVariant::Outline,
        ButtonVariant::Ghost,
        ButtonVariant::Destructive,
        ButtonVariant::Link,
    ] {
        let vs = crate::style::resolve_button(
            v,
            ControlSize::Md,
            &Default::default(),
            crate::style::StyleState::Normal,
            false,
            false,
            false,
            true, // invalid — ring must be destructive regardless of focus
        );
        assert_eq!(
            vs.focus_ring,
            crate::style::Color::Role(ColorRole::DestructiveRing),
            "{v:?}"
        );
    }
}
