//! CORE typography recipes — the frozen `typography.*` specimen styles as
//! typed `TextStyle`/`BoxStyle` values. One recipe per frozen kind;
//! `Typography` is the component surface (`ui.typography`, `ui.code_block`
//! consume it). Sizes/weights/line-pitches are the frozen contract values
//! (`reference/shadcn-gallery-v0.1/contract.json` + `tokens.json`); colors
//! are theme roles, never literals.

use crate::geom::Dp;
use crate::style::{
    Border, BorderSide, BoxStyle, Color, CornerRadii, Insets, TextStyle, TextWeight,
};
use crate::theme::{ColorRole, FontFamily};

/// The frozen CORE typography vocabulary (`typography.<key>` specimens).
/// `CodeBlock` is composite — `ui.code_block(&lines)` stages it.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Typography {
    /// `typography.h1` — 48/800 lh48 tracking-tight, balanced wrap
    H1,
    /// `typography.h2` — 30/600 lh36 tracking-tight + bottom rule
    H2,
    /// `typography.h3` — 24/600 lh32 tracking-tight
    H3,
    /// `typography.h4` — 20/600 lh28 tracking-tight
    H4,
    /// `typography.p` — 16/400 lh28
    Paragraph,
    /// `typography.lead` — 20/400 lh28 muted
    Lead,
    /// `typography.small` — 14/500 lh14
    Small,
    /// `typography.muted` — 14/400 lh20 muted
    Muted,
    /// `typography.inline-code` — mono 14/600 lh20 on muted chrome
    InlineCode,
}

/// CSS `tracking-tight` — −0.025em of the font size (≥ text-2xl headings).
fn tight(size: f32) -> f32 {
    -0.025 * size
}

impl Typography {
    /// The frozen text recipe for this kind.
    pub fn style(self) -> TextStyle {
        let mut s = TextStyle::default();
        match self {
            Typography::H1 => {
                s.size = crate::style::TextSize::Exact(Dp(48.0));
                s.weight = TextWeight::Extrabold;
                s.line_height = Some(Dp(48.0));
                s.letter_spacing = tight(48.0);
            }
            Typography::H2 => {
                s.size = crate::style::TextSize::Exact(Dp(30.0));
                s.weight = TextWeight::Semibold;
                s.line_height = Some(Dp(36.0));
                s.letter_spacing = tight(30.0);
            }
            Typography::H3 => {
                s.size = crate::style::TextSize::Exact(Dp(24.0));
                s.weight = TextWeight::Semibold;
                s.line_height = Some(Dp(32.0));
                s.letter_spacing = tight(24.0);
            }
            Typography::H4 => {
                s.size = crate::style::TextSize::Exact(Dp(20.0));
                s.weight = TextWeight::Semibold;
                s.line_height = Some(Dp(28.0));
                s.letter_spacing = tight(20.0);
            }
            Typography::Paragraph => {
                s.size = crate::style::TextSize::Exact(Dp(16.0));
                s.line_height = Some(Dp(28.0));
            }
            Typography::Lead => {
                s.foreground = Color::Role(ColorRole::MutedForeground);
                s.size = crate::style::TextSize::Exact(Dp(20.0));
                s.line_height = Some(Dp(28.0));
            }
            Typography::Small => {
                s.size = crate::style::TextSize::Exact(Dp(14.0));
                s.weight = TextWeight::Medium;
                s.line_height = Some(Dp(14.0));
            }
            Typography::Muted => {
                s.foreground = Color::Role(ColorRole::MutedForeground);
                s.size = crate::style::TextSize::Exact(Dp(14.0));
                s.line_height = Some(Dp(20.0));
            }
            Typography::InlineCode => {
                s.font = FontFamily::Mono;
                s.size = crate::style::TextSize::Exact(Dp(14.0));
                s.weight = TextWeight::Semibold;
                s.line_height = Some(Dp(20.0));
            }
        }
        s
    }

    /// Painted chrome under the text when the recipe carries it:
    /// h2's `border-b pb-2` rule, inline-code's `bg-muted rounded-sm
    /// px-[0.3rem] py-[0.2rem]` surface.
    pub fn chrome(self) -> Option<BoxStyle> {
        match self {
            Typography::H2 => Some(BoxStyle {
                background: Color::rgba(0, 0, 0, 0),
                border: Border {
                    bottom: BorderSide::new(Dp(1.0), Color::Role(ColorRole::Border)),
                    ..Border::default()
                },
                radii: CornerRadii::default(),
                padding: Insets {
                    bottom: Dp(8.0),
                    ..Insets::default()
                },
                shadow: None,
            }),
            Typography::InlineCode => Some(BoxStyle {
                background: Color::Role(ColorRole::Muted),
                border: Border::default(),
                radii: CornerRadii::all(Dp(6.0)),
                padding: Insets {
                    top: Dp(3.2),
                    right: Dp(4.8),
                    bottom: Dp(3.2),
                    left: Dp(4.8),
                },
                shadow: None,
            }),
            _ => None,
        }
    }

    /// Whether the frozen recipe wraps (h1 `text-balance`, p, lead).
    pub fn wraps(self) -> bool {
        matches!(
            self,
            Typography::H1 | Typography::H2 | Typography::Paragraph | Typography::Lead
        )
    }
}
