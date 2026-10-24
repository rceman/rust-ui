/// Frozen dropdown-menu item chrome. Child content owns its ink so a
/// destructive item can retain the destructive role while highlighted.

pub fn dropdown_item_style(destructive: bool) -> ActionStyle {
    let mut base = BoxStyle::new();
    base.background = Color::rgba(0, 0, 0, 0);
    base.radii = CornerRadii::all(Dp(8.0));
    base.padding = Insets {
        top: Dp(4.0),
        right: Dp(6.0),
        bottom: Dp(4.0),
        left: Dp(6.0),
    };
    let mut hover = BoxStylePatch::new();
    hover.background = Some(Color::Role(if destructive {
        ColorRole::DestructiveTint
    } else {
        ColorRole::Accent
    }));
    ActionStyle::new(base).hover(hover).focus_visible({
        let mut patch = BoxStylePatch::new();
        patch.background = Some(Color::Role(if destructive {
            ColorRole::DestructiveTint
        } else {
            ColorRole::Accent
        }));
        patch
    })
}

pub fn dropdown_label_style() -> TextStyle {
    TextStyle {
        foreground: Color::Role(ColorRole::MutedForeground),
        size: TextSize::Exact(Dp(12.0)),
        weight: TextWeight::Medium,
        line_height: Some(Dp(16.0)),
        ..TextStyle::default()
    }
}

pub fn dropdown_label_box() -> BoxStyle {
    BoxStyle {
        padding: Insets {
            top: Dp(4.0),
            right: Dp(6.0),
            bottom: Dp(4.0),
            left: Dp(6.0),
        },
        ..BoxStyle::default()
    }
}

pub fn dropdown_item_text_style(foreground: ColorRole) -> TextStyle {
    TextStyle {
        foreground: Color::Role(foreground),
        size: TextSize::Exact(Dp(14.0)),
        weight: TextWeight::Normal,
        line_height: Some(Dp(20.0)),
        ..TextStyle::default()
    }
}

pub fn dropdown_shortcut_style() -> TextStyle {
    TextStyle {
        foreground: Color::Role(ColorRole::MutedForeground),
        size: TextSize::Exact(Dp(12.0)),
        weight: TextWeight::Normal,
        line_height: Some(Dp(16.0)),
        letter_spacing: 0.6,
        ..TextStyle::default()
    }
}

pub fn settings_nav_surface() -> BoxStyle {
    BoxStyle {
        background: Color::Role(ColorRole::Sidebar),
        ..BoxStyle::default()
    }
}

pub fn settings_nav_header_style() -> TextStyle {
    TextStyle {
        foreground: Color::Role(ColorRole::SidebarForeground),
        size: TextSize::Exact(Dp(14.0)),
        weight: TextWeight::Semibold,
        line_height: Some(Dp(20.0)),
        ..TextStyle::default()
    }
}

pub fn settings_nav_header_box() -> BoxStyle {
    BoxStyle {
        padding: Insets {
            top: Dp(12.0),
            right: Dp(16.0),
            bottom: Dp(12.0),
            left: Dp(16.0),
        },
        ..BoxStyle::default()
    }
}

pub fn settings_nav_group_style() -> TextStyle {
    TextStyle {
        foreground: Color::Role(ColorRole::SidebarForeground70),
        size: TextSize::Exact(Dp(12.0)),
        weight: TextWeight::Medium,
        line_height: Some(Dp(16.0)),
        ..TextStyle::default()
    }
}

pub fn settings_nav_group_box() -> BoxStyle {
    BoxStyle {
        padding: Insets::all(Dp(8.0)),
        ..BoxStyle::default()
    }
}

pub fn settings_nav_item_text_style(active: bool) -> TextStyle {
    TextStyle {
        foreground: Color::Role(if active {
            ColorRole::SidebarAccentForeground
        } else {
            ColorRole::SidebarForeground
        }),
        size: TextSize::Exact(Dp(14.0)),
        weight: if active {
            TextWeight::Medium
        } else {
            TextWeight::Normal
        },
        line_height: Some(Dp(20.0)),
        ..TextStyle::default()
    }
}

/// Sidebar navigation row. Active state is semantic input; hover and
/// focus-visible remain runtime/presentation interaction states.
pub fn settings_nav_item_style(active: bool) -> ActionStyle {
    let mut base = BoxStyle::new();
    base.background = Color::Role(if active {
        ColorRole::SidebarAccent
    } else {
        ColorRole::Sidebar
    });
    base.radii = CornerRadii::all(Dp(8.0));
    base.padding = Insets::default();
    let mut hover = BoxStylePatch::new();
    hover.background = Some(Color::Role(ColorRole::SidebarAccent));
    ActionStyle::new(base).hover(hover)
}

/// Native editor chrome recipe — the pill frame around the peer:
/// Background-role surface, Border-role 1dp outline, 6dp radii, and the
/// safety padding the peer surface insets by. Authored chrome patches
/// overlay this; the peer island remains opaque and unclipped.
pub fn text_input_chrome_recipe() -> BoxStyle {
    BoxStyle {
        background: Color::Role(ColorRole::Background),
        border: Border::all(BorderSide::new(Dp(1.0), Color::Role(ColorRole::Border))),
        radii: CornerRadii::all(Dp(6.0)),
        padding: Insets {
            top: Dp(5.0),
            right: Dp(6.0),
            bottom: Dp(5.0),
            left: Dp(6.0),
        },
        shadow: None,
    }
}

/// Resolve native editor chrome — recipe + authored patch. The paired
/// safety inset the peer sits inside derives from the RESOLVED authored
/// values (pre-normalization radii, per the approved rule).
pub fn resolve_text_input_chrome(patch: &TextInputStylePatch) -> BoxStyle {
    let mut b = text_input_chrome_recipe();
    b.patch(&patch.chrome);
    b
}

/// Resolved painted style for one `NodeData::Toggle` under an interaction
/// state — the box_style describes the indicator/track chrome (label text
/// lives in `text_style`). The frozen palettes: checkbox/radio checked =
/// `bg-primary` + `border-primary` + `PrimaryForeground` mark; unchecked =
/// transparent + `Border`; switch checked track = `bg-primary`, unchecked
/// = `bg-input`; invalid = `border-destructive` (light) /
/// `border-destructive/50` (dark) + `ring-destructive` enforcement.
pub fn resolve_toggle_visual(
    kind: crate::theme::ToggleKind,
    size: crate::theme::ToggleSize,
    checked: bool,
    invalid: bool,
    state: StyleState,
    focus_visible: bool,
    dark: bool,
) -> VisualStyle {
    let _ = (size, state, focus_visible); // state branches reserved — normal only today
    let border_color = if invalid {
        if dark {
            Color::Role(ColorRole::Destructive50)
        } else {
            Color::Role(ColorRole::Destructive)
        }
    } else if checked {
        Color::Role(ColorRole::Primary)
    } else {
        Color::Role(ColorRole::Border)
    };
    let (background, radii) = match kind {
        crate::theme::ToggleKind::Checkbox => (
            if checked {
                Color::Role(ColorRole::Primary)
            } else {
                Color::rgba(0, 0, 0, 0)
            },
            4.0,
        ),
        crate::theme::ToggleKind::RadioItem => (
            if checked {
                Color::Role(ColorRole::Primary)
            } else {
                Color::rgba(0, 0, 0, 0)
            },
            9999.0,
        ),
        crate::theme::ToggleKind::Switch => (
            if checked {
                Color::Role(ColorRole::Primary)
            } else {
                Color::Role(ColorRole::Input)
            },
            9999.0,
        ),
    };
    VisualStyle {
        box_style: BoxStyle {
            background,
            border: Border::all(BorderSide::new(Dp(1.0), border_color)),
            radii: CornerRadii::all(Dp(radii)),
            padding: Insets::default(),
            shadow: None,
        },
        text_style: TextStyle {
            foreground: Color::Role(ColorRole::Foreground),
            size: TextSize::Exact(Dp(14.0)),
            weight: TextWeight::Normal,
            font: crate::theme::FontFamily::Sans,
            line_height: Some(Dp(14.0)),
            underline: false,
            letter_spacing: 0.0,
        },
        focus_ring: Color::Role(crate::layout::ring_color_role(invalid)),
    }
}

/// `input`/`textarea` overlay chrome — the transparent top layer of the
/// two-node stack: it carries ONLY the visible frame (border + radii 10 +
/// optional `bg-input/50` disabled fill); the editor leaf under it owns
/// the opaque content rect. `invalid` picks the theme-conditional
/// `InputDestructive` border role; text metrics never touch this layer.
pub fn input_overlay_style(disabled: bool, invalid: bool) -> BoxStyle {
    BoxStyle {
        background: if disabled {
            Color::Role(ColorRole::InputDisabled)
        } else {
            Color::rgba(0, 0, 0, 0)
        },
        border: Border::all(BorderSide::new(
            Dp(1.0),
            if invalid {
                Color::Role(ColorRole::InputDestructive)
            } else {
                Color::Role(ColorRole::Border)
            },
        )),
        radii: CornerRadii::all(Dp(10.0)),
        padding: Insets::default(),
        shadow: None,
    }
}

/// Editor-leaf chrome inside the input/textarea stack — border/radii
/// stripped (the overlay owns the frame); padding carries the frozen
/// content offset (input `py-1 px-2.5` = 4v/10h; textarea `py-2` = 8v/
/// 10h). Peer surface = element rect minus this inset.
pub fn input_editor_chrome(multiline: bool) -> BoxStyle {
    let pad_v = if multiline { 8.0 } else { 4.0 };
    BoxStyle {
        background: Color::Role(ColorRole::Background),
        border: Border::default(),
        radii: CornerRadii::default(),
        padding: Insets {
            top: Dp(pad_v),
            right: Dp(10.0),
            bottom: Dp(pad_v),
            left: Dp(10.0),
        },
        shadow: None,
    }
}

/// `field` label part — `text-sm` medium; the invalid field flips it to
/// the destructive role (frozen `text-destructive` on `.invalid`).
pub fn field_label_style(invalid: bool) -> TextStyle {
    TextStyle {
        foreground: Color::Role(if invalid {
            ColorRole::Destructive
        } else {
            ColorRole::Foreground
        }),
        size: TextSize::Exact(Dp(14.0)),
        weight: TextWeight::Medium,
        line_height: Some(Dp(14.0)),
        ..TextStyle::default()
    }
}

/// `field` description part — `text-sm` muted-foreground at the frozen
/// 21dp line box.
pub fn field_description_style() -> TextStyle {
    TextStyle {
        foreground: Color::Role(ColorRole::MutedForeground),
        size: TextSize::Exact(Dp(14.0)),
        weight: TextWeight::Normal,
        line_height: Some(Dp(21.0)),
        ..TextStyle::default()
    }
}

/// `field` error part — `text-sm` destructive at the frozen 20dp line box.
pub fn field_error_style() -> TextStyle {
    TextStyle {
        foreground: Color::Role(ColorRole::Destructive),
        size: TextSize::Exact(Dp(14.0)),
        weight: TextWeight::Normal,
        line_height: Some(Dp(20.0)),
        ..TextStyle::default()
    }
}
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
