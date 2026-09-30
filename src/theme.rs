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
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum ReducedMotion {
    #[default]
    System,
    Reduce,
    NoPreference,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ColorRole {
    Background,
    Foreground,
    Muted,
    MutedForeground,
    Accent,
    AccentForeground,
    Border,
    Destructive,
    DestructiveForeground,
    Focus,
    /// shadow tint — used by `Shadow` when authored via a role
    Shadow,
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum Space {
    Xs,
    #[default]
    Sm,
    Md,
    Lg,
}

impl Space {
    pub fn dp(self) -> Dp {
        Dp(match self {
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

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum ButtonVariant {
    #[default]
    Primary,
    Secondary,
    Ghost,
    Destructive,
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum ControlSize {
    Sm,
    #[default]
    Md,
    Lg,
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

    pub fn resolve(mode: ThemeMode, appearance: Appearance) -> Self {
        let dark = match mode {
            ThemeMode::Light => false,
            ThemeMode::Dark => true,
            ThemeMode::System => appearance.dark,
        };
        if dark { Theme::dark() } else { Theme::light() }
    }

    pub fn appearance(&self) -> Appearance {
        Appearance { dark: self.dark }
    }
}
