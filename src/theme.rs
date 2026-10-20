use crate::geom::Dp;

/// Resolved token set for one window; built from `Theme::light()` /
/// `Theme::dark()` or `Theme::resolve`.
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub dark: bool,
    pub reduced_motion: ReducedMotion,
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum ThemeMode {
    Light,
    Dark,
    #[default]
    System,
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct Appearance {
    pub dark: bool,
    /// OS forced-colors / high-contrast — drives the enforcement layer
    /// (decorative shadows resolve to None; focus visibility enforced)
    pub forced_colors: bool,
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum ReducedMotion {
    #[default]
    System,
    Reduce,
    NoPreference,
}

/// Semantic theme roles — ONE role per frozen Shadcn semantic token
/// (see `reference/shadcn-gallery-v0.1/tokens.json`; the concrete palette
/// lives in `style::role_color`, the only authority). Translucent roles
/// carry the frozen alpha baked in.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ColorRole {
    Background,
    Foreground,
    /// `card` — card/popover-class surfaces
    Card,
    CardForeground,
    Primary,
    PrimaryForeground,
    /// `secondary` fill — muted control surfaces
    Secondary,
    SecondaryForeground,
    Muted,
    MutedForeground,
    Accent,
    AccentForeground,
    Border,
    /// `input` — input-class border/fill (dark carries token alpha)
    Input,
    /// `ring` — focus-visible border color
    Ring,
    /// `surface` — code-block canvas
    Surface,
    SurfaceForeground,
    /// `code` — inline-code surface
    Code,
    CodeForeground,
    /// `code-highlight` — code-block emphasis line
    CodeHighlight,
    /// `code-number` — code line numbers / secondary code text
    CodeNumber,
    Selection,
    SelectionForeground,
    Destructive,
    DestructiveForeground,
    /// enforcement focus indicator — frozen `ring-50` (ring/50, 3px band)
    Focus,
    /// shadow tint — used by `Shadow` when authored via a role
    Shadow,
    // ---- translucent tokens (frozen `translucent` table) ---------------
    /// `foreground-10` — ring-foreground/10 card edge ring
    Foreground10,
    /// `foreground-60` — inactive nav/tabs text
    Foreground60,
    /// `muted-50` — ghost dark hover / card footer fill
    Muted50,
    /// `input-30-dark` — dark outline-control fill (#fff @ 4.5%)
    Input30,
    /// `input-50-dark` — dark outline-control hover fill (#fff @ 7.5%)
    Input50,
    /// `primary-80` — default-variant hover fill (primary/80)
    Primary80,
    /// `secondary-hover` — resolved color-mix(secondary, foreground 5%)
    SecondaryHover,
    /// `destructive-10-20` — destructive variant fill (light /10, dark /20)
    DestructiveTint,
    /// `destructive-20-30` — destructive hover fill (light /20, dark /30)
    DestructiveTintHover,
    /// `destructive-ring-20-40` — invalid + destructive-visible ring
    /// (light /20, dark /40)
    DestructiveRing,
    /// destructive focus-visible border (`border-destructive/40`)
    Destructive40,
    /// `destructive-50-dark` — dark invalid border (`border-destructive/50`)
    Destructive50,
    /// `input-50` — input/textarea disabled fill (`bg-input/50` light,
    /// `bg-input/80` dark — the frozen token carries white/12 in dark)
    InputDisabled,
    /// `input-destructive` — input/textarea invalid border
    /// (`aria-invalid:border-destructive` light = full destructive,
    /// `dark:aria-invalid:border-destructive/50` = /50 in dark)
    InputDestructive,
    /// `destructive-90` — destructive alert description
    /// (`text-destructive/90`, both themes 90% alpha)
    Destructive90,
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum Space {
    /// no spacing — for component chrome that carries its own exact
    /// geometry instead of inheriting generic container defaults
    None,
    Xs,
    #[default]
    Sm,
    Md,
    Lg,
}

impl Space {
    pub fn dp(self) -> Dp {
        Dp(match self {
            Space::None => 0.0,
            Space::Xs => 4.0,
            Space::Sm => 8.0,
            Space::Md => 12.0,
            Space::Lg => 16.0,
        })
    }
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum Radius {
    None,
    #[default]
    Sm,
    Md,
    Lg,
}

/// Narrow chrome animation classes — hover/focus colour transitions, tooltip
/// fade/scale, focus ring.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MotionToken {
    Hover,
    Tooltip,
    Focus,
}

/// Built-in Shadcn (base-nova/neutral) button variant vocabulary.
/// `Primary` is the frozen `default` variant.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum ButtonVariant {
    /// frozen `default` — bg-primary text-primary-foreground
    #[default]
    Primary,
    Secondary,
    Outline,
    Ghost,
    Destructive,
    Link,
}

/// Built-in Shadcn badge variant vocabulary — `Default` is the frozen
/// `badge.default` (bg-primary text-primary-foreground pill).
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum BadgeVariant {
    #[default]
    Default,
    Secondary,
    Outline,
    Destructive,
    Ghost,
    Link,
}

/// Built-in Shadcn alert variant vocabulary — `Default` is the frozen
/// `alert.default` (card chrome + foreground parts), `Destructive` keeps
/// the same card chrome but shifts icon/title/description to the
/// destructive roles.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum AlertVariant {
    #[default]
    Default,
    Destructive,
}

/// Separator orientation — `Horizontal` is a 1dp row-axis rule that fills
/// the offered cross-axis, `Vertical` the 1dp column-axis analog.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum SeparatorOrientation {
    #[default]
    Horizontal,
    Vertical,
}

/// Control size class — button and icon-button share the frozen control
/// heights: xs 24 / sm 28 / default 32 / lg 36 dp.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum ControlSize {
    /// 24dp (`size-xs` / `icon-xs`)
    Xs,
    /// 28dp (`size-sm` / `icon-sm`)
    Sm,
    /// 32dp (`default` / `icon`)
    #[default]
    Md,
    /// 36dp (`size-lg` / `icon-lg`)
    Lg,
}

impl ControlSize {
    /// Frozen `control_heights_px` entry for this size class.
    pub fn height(self) -> Dp {
        Dp(match self {
            ControlSize::Xs => 24.0,
            ControlSize::Sm => 28.0,
            ControlSize::Md => 32.0,
            ControlSize::Lg => 36.0,
        })
    }
}

/// Toggle-control kind — the three frozen selection controls share one
/// painted leaf (`NodeData::Toggle`): a 16dp `Checkbox`, a 16dp radio
/// `RadioItem` inside a `ui.radio_group`, and the pill `Switch`.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum ToggleKind {
    #[default]
    Checkbox,
    RadioItem,
    Switch,
}

/// Switch track size — frozen `switch.default` 32×18.4dp and
/// `switch.default.size-sm` 24×14dp.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum ToggleSize {
    /// 32×18.4 track, 16×16 thumb
    #[default]
    Default,
    /// 24×14 track, 12×12 thumb
    Sm,
}

/// Field layout orientation — `field.default` stacks label/control/
/// description vertically; `field.horizontal` puts a label+description
/// `content` part left and the control right.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum FieldOrientation {
    #[default]
    Vertical,
    Horizontal,
}

/// Card density — frozen `--card-spacing` token: Default = 16dp,
/// Sm = 12dp. Controls outer padding-top, part gap, and part side padding.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum CardSize {
    #[default]
    Default,
    Sm,
}

/// Text family token — the typed family slot on `TextStyle`. Maps to the
/// backend's registered family names (Windows: "Geist" / "Geist Mono").
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum FontFamily {
    /// product sans (canonical: Geist)
    #[default]
    Sans,
    /// product mono (canonical: Geist Mono)
    Mono,
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum SubmitPolicy {
    #[default]
    Enter,
    ModifierEnter,
    None,
}

impl Theme {
    pub fn light() -> Self {
        Theme {
            dark: false,
            reduced_motion: ReducedMotion::System,
        }
    }

    pub fn dark() -> Self {
        Theme {
            dark: true,
            reduced_motion: ReducedMotion::System,
        }
    }

    pub fn reduced_motion(mut self, policy: ReducedMotion) -> Self {
        self.reduced_motion = policy;
        self
    }

    /// Built-in Shadcn family + light/dark selection: `Light`/`Dark`
    /// force the token set; `System` follows the OS appearance.
    pub fn resolve(mode: ThemeMode, appearance: Appearance) -> Self {
        let dark = match mode {
            ThemeMode::Light => false,
            ThemeMode::Dark => true,
            ThemeMode::System => appearance.dark,
        };
        if dark { Theme::dark() } else { Theme::light() }
    }

    pub fn appearance(&self) -> Appearance {
        Appearance {
            dark: self.dark,
            forced_colors: false,
        }
    }
}
