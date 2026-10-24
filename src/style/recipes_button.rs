/// Button recipe — the frozen `cn-button` contract (base-nova/neutral):
/// base `VisualStyle` per variant/size + recipe state overlays as full
/// styles. Horizontal padding/radius/font per the frozen control tables;
/// `pressed` and `disabled` carry NO color change — pressed paints the
/// 1dp translate, disabled paints 0.5 opacity (both are whole-control
/// effects the style can't express). Focus-visible replaces the border
/// color (`border-ring` / `border-destructive/40`); the ring band itself
/// is the renderer's enforcement layer.

pub fn button_recipe(
    variant: ButtonVariant,
    size: ControlSize,
    dark: bool,
) -> StateStyles<VisualStyle> {
    let (pad_x, radius, font_px) = match size {
        ControlSize::Xs => (8.0, 8.0, 12.0),
        ControlSize::Sm => (10.0, 8.0, 12.8),
        ControlSize::Md => (10.0, 10.0, 14.0),
        ControlSize::Lg => (10.0, 10.0, 14.0),
    };
    let outline_border = if dark {
        Color::Role(ColorRole::Input)
    } else {
        Color::Role(ColorRole::Border)
    };
    let (bg, fg, border_color) = match variant {
        ButtonVariant::Primary => (
            Color::Role(ColorRole::Primary),
            Color::Role(ColorRole::PrimaryForeground),
            Color::rgba(0, 0, 0, 0),
        ),
        ButtonVariant::Secondary => (
            Color::Role(ColorRole::Secondary),
            Color::Role(ColorRole::SecondaryForeground),
            Color::rgba(0, 0, 0, 0),
        ),
        ButtonVariant::Outline => (
            if dark {
                Color::Role(ColorRole::Input30)
            } else {
                Color::Role(ColorRole::Background)
            },
            Color::Role(ColorRole::Foreground),
            outline_border,
        ),
        ButtonVariant::Ghost => (
            Color::rgba(0, 0, 0, 0),
            Color::Role(ColorRole::Foreground),
            Color::rgba(0, 0, 0, 0),
        ),
        ButtonVariant::Destructive => (
            Color::Role(ColorRole::DestructiveTint),
            Color::Role(ColorRole::Destructive),
            Color::rgba(0, 0, 0, 0),
        ),
        ButtonVariant::Link => (
            Color::rgba(0, 0, 0, 0),
            Color::Role(ColorRole::Primary),
            Color::rgba(0, 0, 0, 0),
        ),
        // `cn-select-trigger` — border-input both themes, transparent
        // light / input-30 dark surface, regular weight
        ButtonVariant::Select => (
            if dark {
                Color::Role(ColorRole::Input30)
            } else {
                Color::rgba(0, 0, 0, 0)
            },
            Color::Role(ColorRole::Foreground),
            Color::Role(ColorRole::Input),
        ),
        // `cn-select-item` — transparent option row; the hover/`focus`
        // accent chip paints in the state branch
        ButtonVariant::SelectItem => (
            Color::rgba(0, 0, 0, 0),
            Color::Role(ColorRole::Foreground),
            Color::rgba(0, 0, 0, 0),
        ),
        // `cn-tabs-trigger` — foreground/60 light, muted-foreground dark;
        // the active chrome is the recipe's `selected` overlay
        ButtonVariant::Tab | ButtonVariant::TabLine => (
            Color::rgba(0, 0, 0, 0),
            if dark {
                Color::Role(ColorRole::MutedForeground)
            } else {
                Color::Role(ColorRole::Foreground60)
            },
            Color::rgba(0, 0, 0, 0),
        ),
    };
    let base = VisualStyle {
        box_style: BoxStyle {
            background: bg,
            // `border border-transparent` — a real 1dp transparent border
            // everywhere; outline carries the visible one
            border: Border::all(BorderSide::new(Dp(1.0), border_color)),
            radii: CornerRadii::all(Dp(match variant {
                ButtonVariant::Tab | ButtonVariant::TabLine | ButtonVariant::SelectItem => 8.0,
                ButtonVariant::Select => {
                    if size == ControlSize::Sm {
                        8.0
                    } else {
                        10.0
                    }
                }
                _ => radius,
            })),
            padding: match variant {
                // select trigger: `py-2 pr-2 pl-2.5` + 1dp border
                ButtonVariant::Select => Insets {
                    top: Dp(8.0),
                    right: Dp(8.0),
                    bottom: Dp(8.0),
                    left: Dp(10.0),
                },
                // option row: `py-1 pr-8 pl-1.5` (pr-8 IS the indicator slot)
                ButtonVariant::SelectItem => Insets {
                    top: Dp(4.0),
                    right: Dp(32.0),
                    bottom: Dp(4.0),
                    left: Dp(6.0),
                },
                // tab: `px-1.5 py-0.5` inside a 1dp transparent border
                ButtonVariant::Tab | ButtonVariant::TabLine => Insets {
                    top: Dp(2.0),
                    right: Dp(6.0),
                    bottom: Dp(2.0),
                    left: Dp(6.0),
                },
                _ => Insets {
                    top: Dp(0.0),
                    right: Dp(pad_x),
                    bottom: Dp(0.0),
                    left: Dp(pad_x),
                },
            },
            shadow: None,
        },
        text_style: TextStyle {
            foreground: fg,
            size: TextSize::Exact(Dp(font_px)),
            weight: match variant {
                ButtonVariant::Tab | ButtonVariant::TabLine => TextWeight::Medium,
                ButtonVariant::Select | ButtonVariant::SelectItem => TextWeight::Normal,
                _ => TextWeight::Medium,
            },
            font: crate::theme::FontFamily::Sans,
            line_height: None,
            underline: false,
            letter_spacing: 0.0,
        },
        focus_ring: Color::Role(ColorRole::Focus),
    };
    // hover — full styles; the recipe state branch replaces the base
    let mut hover = base;
    match variant {
        ButtonVariant::Primary => hover.box_style.background = Color::Role(ColorRole::Primary80),
        ButtonVariant::Secondary => {
            hover.box_style.background = Color::Role(ColorRole::SecondaryHover)
        }
        ButtonVariant::Outline => {
            hover.box_style.background = if dark {
                Color::Role(ColorRole::Input50)
            } else {
                Color::Role(ColorRole::Muted)
            };
        }
        ButtonVariant::Ghost => {
            hover.box_style.background = if dark {
                Color::Role(ColorRole::Muted50)
            } else {
                Color::Role(ColorRole::Muted)
            };
        }
        ButtonVariant::Destructive => {
            hover.box_style.background = Color::Role(ColorRole::DestructiveTintHover)
        }
        ButtonVariant::Link => hover.text_style.underline = true,
        // `dark:hover:bg-input/50` — the trigger keeps its transparent
        // surface in light
        ButtonVariant::Select => {
            if dark {
                hover.box_style.background = Color::Role(ColorRole::Input50);
            }
        }
        // `focus:bg-accent focus:text-accent-foreground` — the option's
        // highlighted chip
        ButtonVariant::SelectItem => {
            hover.box_style.background = Color::Role(ColorRole::Accent);
            hover.text_style.foreground = Color::Role(ColorRole::AccentForeground);
        }
        // `hover:text-foreground` (both themes) — chrome stays
        ButtonVariant::Tab | ButtonVariant::TabLine => {
            hover.text_style.foreground = Color::Role(ColorRole::Foreground);
        }
    }
    // focus-visible border swap — `border-ring`, or
    // `border-destructive/40` on the destructive variant
    let mut focus_visible = base;
    let fv_border_color = match variant {
        ButtonVariant::Destructive => Color::Role(ColorRole::Destructive40),
        _ => Color::Role(ColorRole::Ring),
    };
    focus_visible.box_style.border = Border::all(BorderSide::new(Dp(1.0), fv_border_color));
    let _ = dark;
    StateStyles {
        base,
        hover: Some(hover),
        // pressed/disabled are whole-control effects (translate / opacity)
        // — no color branch; the painter owns the visual delta
        pressed: None,
        disabled: None,
        focus_visible: Some(focus_visible),
    }
}

/// Delta two full styles into a minimal patch.
fn state_delta(from: &VisualStyle, to: &VisualStyle) -> VisualStylePatch {
    let mut p = VisualStylePatch::new();
    if from.box_style.background != to.box_style.background {
        p.box_style.background = Some(to.box_style.background);
    }
    macro_rules! side {
        ($b:ident) => {{
            if from.box_style.border.$b != to.box_style.border.$b {
                p.box_style.border.$b.width = Some(to.box_style.border.$b.width);
                p.box_style.border.$b.color = Some(to.box_style.border.$b.color);
            }
        }};
    }
    side!(top);
    side!(right);
    side!(bottom);
    side!(left);
    macro_rules! corner {
        ($c:ident) => {{
            if from.box_style.radii.$c != to.box_style.radii.$c {
                p.box_style.radii.$c = Some(to.box_style.radii.$c);
            }
        }};
    }
    corner!(top_left);
    corner!(top_right);
    corner!(bottom_right);
    corner!(bottom_left);
    macro_rules! inset {
        ($i:ident) => {{
            if from.box_style.padding.$i != to.box_style.padding.$i {
                p.box_style.padding.$i = Some(to.box_style.padding.$i);
            }
        }};
    }
    inset!(top);
    inset!(right);
    inset!(bottom);
    inset!(left);
    if from.box_style.shadow != to.box_style.shadow {
        p.box_style.shadow = match &to.box_style.shadow {
            Some(s) => ShadowPatch::Set(*s),
            None => ShadowPatch::Remove,
        };
    }
    if from.text_style.foreground != to.text_style.foreground {
        p.text_style.foreground = Some(to.text_style.foreground);
    }
    if from.text_style.size != to.text_style.size
        && let TextSize::Exact(d) = to.text_style.size
    {
        p.text_style.size = Some(d);
    }
    if from.text_style.weight != to.text_style.weight {
        p.text_style.weight = Some(to.text_style.weight);
    }
    if from.text_style.font != to.text_style.font {
        p.text_style.font = Some(to.text_style.font);
    }
    if from.text_style.line_height != to.text_style.line_height
        && let Some(lh) = to.text_style.line_height
    {
        p.text_style.line_height = Some(lh);
    }
    if from.text_style.underline != to.text_style.underline {
        p.text_style.underline = Some(to.text_style.underline);
    }
    if from.text_style.letter_spacing != to.text_style.letter_spacing {
        p.text_style.letter_spacing = Some(to.text_style.letter_spacing);
    }
    p
}

/// Resolved button style for one interaction state — the deterministic
/// chain: recipe base -> recipe state overlay -> consumer base -> consumer
/// state overlay -> consumer focus overlay -> invalid semantic overlay.
/// OS enforcement comes after. `invalid` is the semantic `aria-invalid`
/// state (`border-destructive`, `ring-destructive/20` dark /40) — applied
/// AFTER all consumer layers so it can't be restyled away; it is
/// orthogonal to every interaction state.
pub fn resolve_button(
    variant: ButtonVariant,
    size: ControlSize,
    patch: &ButtonStylePatch,
    state: StyleState,
    focus_visible: bool,
    dark: bool,
    selected: bool,
    invalid: bool,
) -> VisualStyle {
    let recipe = button_recipe(variant, size, dark);
    let mut v = recipe.base;
    let recipe_branch = match state {
        StyleState::Disabled => recipe.disabled.as_ref(),
        StyleState::Pressed => recipe.pressed.as_ref(),
        StyleState::Hover => recipe.hover.as_ref(),
        StyleState::Normal => None,
    };
    if let Some(b) = recipe_branch {
        v = *b;
    }
    // recipe focus overlay — a recipe layer BEFORE consumer patches: a
    // consumer border patch may restyle it, but the painter draws the
    // required Focus ring independently (enforcement lives at render, so
    // no patch can erase focus visibility)
    if focus_visible
        && state != StyleState::Disabled
        && let Some(fv) = &recipe.focus_visible
    {
        v.box_style.border = fv.box_style.border;
    }
    v.patch(&patch.styles.base);
    let state_patch = match state {
        StyleState::Disabled => patch.styles.disabled.as_ref(),
        StyleState::Pressed => patch.styles.pressed.as_ref(),
        StyleState::Hover => patch.styles.hover.as_ref(),
        StyleState::Normal => None,
    };
    if let Some(p) = state_patch {
        v.patch(p);
    }
    if focus_visible
        && state != StyleState::Disabled
        && let Some(p) = &patch.styles.focus_visible
    {
        v.patch(p);
    }
    // selected semantic overlay — `data-active`/`aria-selected` chrome,
    // orthogonal to every interaction state like `invalid`
    if selected && let Some(p) = selected_overlay(variant, dark) {
        v.patch(&p);
    }
    // invalid semantic overlay — destructive border always (light full /
    // dark /50); the enforcement ring switches to the destructive tint so
    // an invalid+focused control keeps the `ring-destructive/*` band.
    if invalid {
        let bc = if dark {
            Color::Role(ColorRole::Destructive50)
        } else {
            Color::Role(ColorRole::Destructive)
        };
        for s in [
            &mut v.box_style.border.top,
            &mut v.box_style.border.right,
            &mut v.box_style.border.bottom,
            &mut v.box_style.border.left,
        ] {
            s.color = bc;
        }
    }
    // required focus indicator color: destructive ring for invalid or the
    // destructive variant (`ring-destructive/*`), ring/50 otherwise
    v.focus_ring = if invalid || variant == ButtonVariant::Destructive {
        Color::Role(ColorRole::DestructiveRing)
    } else {
        Color::Role(ColorRole::Focus)
    };
    v
}

/// Semantic `data-active`/`aria-selected` overlay per variant — applied
/// after every consumer layer (selected is semantic truth, not a state).
/// `None` for variants with no selected presentation.
fn selected_overlay(variant: ButtonVariant, dark: bool) -> Option<VisualStylePatch> {
    let mut p = VisualStylePatch::new();
    match variant {
        // `data-active:bg-background data-active:text-foreground
        // dark:data-active:border-input dark:data-active:bg-input/30
        // data-active:shadow-sm` — the chip over the muted list
        ButtonVariant::Tab => {
            p.box_style.background = Some(if dark {
                Color::Role(ColorRole::Input30)
            } else {
                Color::Role(ColorRole::Background)
            });
            p.text_style.foreground = Some(Color::Role(ColorRole::Foreground));
            let bc = if dark {
                Color::Role(ColorRole::Input)
            } else {
                Color::rgba(0, 0, 0, 0)
            };
            for s in [
                &mut p.box_style.border.top,
                &mut p.box_style.border.right,
                &mut p.box_style.border.bottom,
                &mut p.box_style.border.left,
            ] {
                s.color = Some(bc);
            }
            // frozen `shadow-sm` (Tailwind `0 1px 2px 0 rgb(0 0 0/.05)`,
            // approximated with one raster — the token table's second
            // -1 spread layer is inside the first's silhouette)
            p.box_style.shadow = ShadowPatch::Set(Shadow {
                color: Color::Role(ColorRole::Shadow),
                offset_x: Dp(0.0),
                offset_y: Dp(1.0),
                blur_sigma: Dp(1.5),
                spread: Dp(0.0),
            });
        }
        // line variant: `data-active:bg-transparent
        // dark:data-active:border-transparent` — full foreground, the
        // `::after` underline marker paints at render
        ButtonVariant::TabLine => {
            p.box_style.background = Some(Color::rgba(0, 0, 0, 0));
            p.text_style.foreground = Some(Color::Role(ColorRole::Foreground));
            let bc = Color::rgba(0, 0, 0, 0);
            for s in [
                &mut p.box_style.border.top,
                &mut p.box_style.border.right,
                &mut p.box_style.border.bottom,
                &mut p.box_style.border.left,
            ] {
                s.color = Some(bc);
            }
        }
        _ => return None,
    }
    Some(p)
}

