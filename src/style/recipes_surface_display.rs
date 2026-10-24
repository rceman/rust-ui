/// Surface recipe — muted panel chrome with border + radii + padding.

pub fn surface_recipe() -> BoxStyle {
    BoxStyle {
        background: Color::Role(ColorRole::Muted),
        border: Border::all(BorderSide::new(Dp(1.0), Color::Role(ColorRole::Border))),
        radii: CornerRadii::all(Dp(8.0)),
        padding: Insets::all(Dp(12.0)),
        shadow: None,
    }
}

/// Card container recipe — the frozen `cn-card` chrome: `bg-card`,
/// `rounded-xl` (14dp), `ring-1 ring-foreground/10` expressed as a 1dp
/// spread shadow (CSS `0 0 0 1px` — the edge ring, not a border), and
/// top padding = `--card-spacing` (16 default / 12 sm). Side/bottom
/// padding is 0 — the card's parts own their insets.
pub fn card_recipe(size: crate::theme::CardSize) -> BoxStyle {
    let spacing = card_spacing(size);
    BoxStyle {
        background: Color::Role(ColorRole::Card),
        border: Border::default(),
        radii: CornerRadii::all(Dp(14.0)),
        padding: Insets {
            top: spacing,
            right: Dp(0.0),
            bottom: Dp(0.0),
            left: Dp(0.0),
        },
        shadow: Some(Shadow {
            color: Color::Role(ColorRole::Foreground10),
            offset_x: Dp(0.0),
            offset_y: Dp(0.0),
            blur_sigma: Dp(0.0),
            spread: Dp(1.0),
        }),
    }
}

/// Frozen `--card-spacing` token for one card size.
pub fn card_spacing(size: crate::theme::CardSize) -> Dp {
    match size {
        crate::theme::CardSize::Default => Dp(16.0),
        crate::theme::CardSize::Sm => Dp(12.0),
    }
}

// ------------------------- card part recipes ------------------------------
// The parts composed inside a `ui.card` body (`#header`, `#title`,
// `#description`, `#content`, `#footer`) carry the frozen contract chrome
// and typography — these recipes centralize it so part geometry never
// inherits generic container defaults. `size` selects Default / Sm.

/// `#header` / `#content` horizontal inset — the `--card-spacing` token
/// (16 Default / 12 Sm) applied to the left/right edges only.
pub fn card_part_insets(size: crate::theme::CardSize) -> Insets {
    let p = card_spacing(size);
    Insets {
        right: p,
        left: p,
        ..Insets::default()
    }
}

/// `#header` / `#content` box chrome — carries only the horizontal inset
/// so the part's `padding` doesn't inherit a container default.
pub fn card_part_box(size: crate::theme::CardSize) -> BoxStyle {
    BoxStyle {
        padding: card_part_insets(size),
        ..BoxStyle::default()
    }
}

/// `#header.title` — Default `16px / lh22`, Sm `14px / lh19.25`, medium.
pub fn card_title_style(size: crate::theme::CardSize) -> TextStyle {
    let (fs, lh) = match size {
        crate::theme::CardSize::Default => (16.0, 22.0),
        crate::theme::CardSize::Sm => (14.0, 19.25),
    };
    TextStyle {
        foreground: Color::Role(ColorRole::CardForeground),
        size: TextSize::Exact(Dp(fs)),
        weight: TextWeight::Medium,
        line_height: Some(Dp(lh)),
        ..TextStyle::default()
    }
}

/// `#content` body text — `14px / lh20` in `CardForeground`.
pub fn card_content_style() -> TextStyle {
    TextStyle {
        foreground: Color::Role(ColorRole::CardForeground),
        size: TextSize::Exact(Dp(14.0)),
        line_height: Some(Dp(20.0)),
        ..TextStyle::default()
    }
}

/// `#footer` chrome — `bg-muted/50`, `1dp` `border-t`, `--card-spacing`
/// padding on all sides. Buttons inside are `Sm`, right-aligned.
pub fn card_footer_style(size: crate::theme::CardSize) -> BoxStyle {
    let p = card_spacing(size);
    BoxStyle {
        background: Color::Role(ColorRole::Muted50),
        border: Border {
            top: BorderSide::new(Dp(1.0), Color::Role(ColorRole::Border)),
            ..Border::default()
        },
        radii: CornerRadii::default(),
        padding: Insets {
            top: p,
            right: p,
            bottom: p,
            left: p,
        },
        shadow: None,
    }
}

/// Label recipe — foreground role + body text.
pub fn label_recipe() -> TextStyle {
    TextStyle::default()
}

// ---------------------------------------------------------------------------
// static Slice-2 component recipes — separator / badge / alert / skeleton /
// kbd. Geometry and roles per the frozen contract
// (`reference/shadcn-gallery-v0.1/contract.json` + `tokens.json` + the
// `.style-nova` css); all colors resolve through `role_color`.
// ---------------------------------------------------------------------------

/// `ui.separator` chrome — the frozen `bg-border` rule. The 1dp logical
/// thickness and the fill/stretch major axis are LAYOUT inputs (the
/// `Separator` props produce the `LayoutSpec`), not paint.
pub fn separator_recipe() -> BoxStyle {
    BoxStyle {
        background: Color::Role(ColorRole::Border),
        border: Border::default(),
        radii: CornerRadii::default(),
        padding: Insets::default(),
        shadow: None,
    }
}

/// Badge recipe — the frozen `cn-badge` contract: 20dp pill (radius-4xl
/// 26dp), 1dp border, 8dp inline padding, `text-xs` 12dp medium at 16dp
/// line pitch. The frozen element is a FIXED 20dp box whose 16dp line
/// overflows the CSS content box symmetrically; a content-measured box
/// reproduces the identical border-box and text inset with a 1dp block
/// padding (border 1 + pad 1 = the frozen 2dp text edge).
pub fn badge_recipe(variant: crate::theme::BadgeVariant) -> VisualStyle {
    use crate::theme::BadgeVariant as V;
    let (bg, fg, border_color) = match variant {
        V::Default => (
            Color::Role(ColorRole::Primary),
            Color::Role(ColorRole::PrimaryForeground),
            Color::rgba(0, 0, 0, 0),
        ),
        V::Secondary => (
            Color::Role(ColorRole::Secondary),
            Color::Role(ColorRole::SecondaryForeground),
            Color::rgba(0, 0, 0, 0),
        ),
        V::Outline => (
            Color::rgba(0, 0, 0, 0),
            Color::Role(ColorRole::Foreground),
            Color::Role(ColorRole::Border),
        ),
        V::Destructive => (
            // `bg-destructive/10` light / `/20` dark — the frozen
            // `destructive-10-20` translucent token
            Color::Role(ColorRole::DestructiveTint),
            Color::Role(ColorRole::Destructive),
            Color::rgba(0, 0, 0, 0),
        ),
        V::Ghost => (
            Color::rgba(0, 0, 0, 0),
            Color::Role(ColorRole::Foreground),
            Color::rgba(0, 0, 0, 0),
        ),
        V::Link => (
            Color::rgba(0, 0, 0, 0),
            Color::Role(ColorRole::Primary),
            Color::rgba(0, 0, 0, 0),
        ),
    };
    VisualStyle {
        box_style: BoxStyle {
            background: bg,
            border: Border::all(BorderSide::new(Dp(1.0), border_color)),
            radii: CornerRadii::all(Dp(26.0)),
            padding: Insets {
                top: Dp(1.0),
                right: Dp(8.0),
                bottom: Dp(1.0),
                left: Dp(8.0),
            },
            shadow: None,
        },
        text_style: TextStyle {
            foreground: fg,
            size: TextSize::Exact(Dp(12.0)),
            weight: TextWeight::Medium,
            line_height: Some(Dp(16.0)),
            ..TextStyle::default()
        },
        focus_ring: Color::Role(ColorRole::Focus),
    }
}

/// `ui.alert` container chrome — the frozen `cn-alert` box: `bg-card`,
/// 1dp `border`, `rounded-lg` (radius 10dp), padding-block 8 / inline 10.
/// Both variants share this chrome (the destructive variant carries its
/// semantics in the PART colors, not the box).
pub fn alert_recipe(_variant: crate::theme::AlertVariant) -> BoxStyle {
    BoxStyle {
        background: Color::Role(ColorRole::Card),
        border: Border::all(BorderSide::new(Dp(1.0), Color::Role(ColorRole::Border))),
        radii: CornerRadii::all(Dp(10.0)),
        padding: Insets {
            top: Dp(8.0),
            right: Dp(10.0),
            bottom: Dp(8.0),
            left: Dp(10.0),
        },
        shadow: None,
    }
}

/// `title` part text — 14dp medium at the frozen 20dp line pitch.
/// Default reads `card-foreground`, destructive `destructive`.
pub fn alert_title_style(variant: crate::theme::AlertVariant) -> TextStyle {
    TextStyle {
        foreground: Color::Role(match variant {
            crate::theme::AlertVariant::Default => ColorRole::CardForeground,
            crate::theme::AlertVariant::Destructive => ColorRole::Destructive,
        }),
        size: TextSize::Exact(Dp(14.0)),
        weight: TextWeight::Medium,
        line_height: Some(Dp(20.0)),
        ..TextStyle::default()
    }
}

/// `description` part text — 14dp normal at the frozen 20dp line pitch.
/// Default reads `muted-foreground`; destructive is the frozen
/// `text-destructive/90` token.
pub fn alert_description_style(variant: crate::theme::AlertVariant) -> TextStyle {
    TextStyle {
        foreground: Color::Role(match variant {
            crate::theme::AlertVariant::Default => ColorRole::MutedForeground,
            crate::theme::AlertVariant::Destructive => ColorRole::Destructive90,
        }),
        size: TextSize::Exact(Dp(14.0)),
        weight: TextWeight::Normal,
        line_height: Some(Dp(20.0)),
        ..TextStyle::default()
    }
}

/// `icon` part stroke role — `card-foreground` / `destructive` (the
/// destructive variant's svg resolves `currentColor`).
pub fn alert_icon_role(variant: crate::theme::AlertVariant) -> ColorRole {
    match variant {
        crate::theme::AlertVariant::Default => ColorRole::CardForeground,
        crate::theme::AlertVariant::Destructive => ColorRole::Destructive,
    }
}

/// `ui.skeleton` chrome — the frozen `cn-skeleton` `bg-muted` fill:
/// radius-md (8dp) for lines, `rounded-full` (9999, normalized to the
/// half-extent at paint) for the avatar circle.
pub fn skeleton_recipe(circle: bool) -> BoxStyle {
    BoxStyle {
        background: Color::Role(ColorRole::Muted),
        border: Border::default(),
        radii: CornerRadii::all(Dp(if circle { 9999.0 } else { 8.0 })),
        padding: Insets::default(),
        shadow: None,
    }
}

/// `ui.kbd` chrome+text — the frozen `cn-kbd` key cap: `bg-muted`
/// radius-sm (6dp) box, 20dp tall at a 20dp minimum width, `text-xs`
/// 12dp medium in `muted-foreground` at the 16dp line pitch. The frozen
/// box is fixed-height like the badge's; the 4dp effective vertical
/// inset (2+2 around a 16dp line) matches the frozen text placement.
pub fn kbd_recipe() -> VisualStyle {
    VisualStyle {
        box_style: BoxStyle {
            background: Color::Role(ColorRole::Muted),
            border: Border::default(),
            radii: CornerRadii::all(Dp(6.0)),
            padding: Insets {
                top: Dp(2.0),
                right: Dp(4.0),
                bottom: Dp(2.0),
                left: Dp(4.0),
            },
            shadow: None,
        },
        text_style: TextStyle {
            foreground: Color::Role(ColorRole::MutedForeground),
            size: TextSize::Exact(Dp(12.0)),
            weight: TextWeight::Medium,
            line_height: Some(Dp(16.0)),
            ..TextStyle::default()
        },
        focus_ring: Color::Role(ColorRole::Focus),
    }
}

// ---------------------------------------------------------------------------
// forms recipes — Slice-2 forms families (toggle controls, field text,
// input/textarea overlay). Frozen geometry per the contract: checkbox/radio
// 16×16 with a 14dp indicator part, switch tracks 32×18.4 / 24×14, label
// painted at +24dp. `disabled` paints a whole-control 0.5 layer at render —
// like the button recipe it is not a style value.
// ---------------------------------------------------------------------------

