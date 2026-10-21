//! rust-ui — retained native UI core (Windows-only spike, checkpoint 1).
//! One retained arena + staged transactions + bounded task/mailbox delivery
//! + a narrow interaction scheduler. The Windows backend lands separately;
//! `App::run` is a typed `Unsupported` until then.

mod app;
mod arena;
#[cfg(feature = "devtools")]
pub mod devtools;
mod event;
mod geom;
mod icons;
mod key;
mod layout;
mod node;
mod runtime;
mod sched;
mod style;
mod tasks;
mod text;
mod theme;
mod typography;
mod ui;

pub use app::App;
pub use event::{FrameTime, Key, KeyEvent, Modifiers, PointerButton, PointerEvent, ScrollOffset};
pub use geom::{
    Align, Constraints, Dp, Justify, LayoutSpec, Length, PhysicalPoint, PhysicalPx, PhysicalRect,
    PhysicalSize, Point, Rect, ScaleFactor, Size, Visibility, dp,
};
pub use icons::{Icon, IconOp};
pub use key::KeyId;
pub use node::{NodeId, Presented};
pub use runtime::UpdateCtx;
pub use style::{
    Action, ActionStyle, Border, BorderPatch, BorderSide, BorderSidePatch, BoxProps, BoxStyle,
    BoxStylePatch, ButtonStylePatch, Color, CornerRadii, CornerRadiiPatch, Insets, InsetsPatch,
    Shadow, ShadowPatch, StateStyles, StyleState, TextInputStylePatch, TextSize, TextStyle,
    TextStylePatch, TextWeight, VisualStyle, VisualStylePatch,
};
pub use tasks::{
    BoxFuture, CancelToken, Executor, Job, ProxySendError, SendError, TaskSender, TaskStartError,
    UiProxy,
};
pub use text::{
    AcceptOutcome, BindingToken, EditOrigin, TextConflict, TextEdit, TextRevision, TextSelection,
    TextValue,
};
pub use theme::{
    AlertVariant, Appearance, BadgeVariant, ButtonVariant, CardSize, ColorRole, ControlSize,
    FieldOrientation, FontFamily, MotionToken, Radius, ReducedMotion, SeparatorOrientation, Space,
    SubmitPolicy, Theme, ThemeMode, ToggleKind, ToggleSize,
};
pub use typography::Typography;
pub use ui::{
    ActionBuilder, Alert, ButtonBuilder, Canvas, Card, Checkbox, Column, CustomBuilder,
    CustomRender, Field, IconBuilder, InputBuilder, LabelBuilder, Paint, Path2d, PathOp,
    RadioGroup, RadioItemOpts, Role, Row, Scroll, Semantics, SemanticsAction, Separator, Skeleton,
    Stack, Surface, Switch, TextBuilder, TextInputBuilder, TextRun, Ui,
};

/// `use rust_ui::prelude::*` — the documented consumer surface in one place.
pub mod prelude {
    pub use crate::app::App;
    pub use crate::event::{FrameTime, KeyEvent, PointerEvent, ScrollOffset};
    pub use crate::geom::{
        Align, Constraints, Dp, Justify, LayoutSpec, Length, Point, Rect, Size, Visibility, dp,
    };
    pub use crate::node::Presented;
    pub use crate::runtime::UpdateCtx;
    pub use crate::style::{
        Action, ActionStyle, Border, BorderPatch, BorderSide, BorderSidePatch, BoxProps, BoxStyle,
        BoxStylePatch, ButtonStylePatch, Color, CornerRadii, CornerRadiiPatch, Insets, InsetsPatch,
        Shadow, ShadowPatch, StateStyles, TextSize, TextStyle, TextStylePatch, TextWeight,
        VisualStyle, VisualStylePatch, alert_description_style, alert_icon_role, alert_recipe,
        alert_title_style, badge_recipe, card_content_style, card_footer_style, card_part_box,
        card_part_insets, card_spacing, card_title_style, field_description_style,
        field_error_style, field_label_style, kbd_recipe, separator_recipe, skeleton_recipe,
    };
    pub use crate::text::{
        AcceptOutcome, BindingToken, EditOrigin, TextConflict, TextEdit, TextRevision,
        TextSelection, TextValue,
    };
    pub use crate::theme::{
        AlertVariant, Appearance, BadgeVariant, ButtonVariant, CardSize, ColorRole, ControlSize,
        FieldOrientation, FontFamily, MotionToken, Radius, ReducedMotion, SeparatorOrientation,
        Space, SubmitPolicy, Theme, ThemeMode, ToggleKind, ToggleSize,
    };
    pub use crate::typography::Typography;
    pub use crate::ui::{
        Alert, Canvas, Card, Checkbox, Column, CustomRender, Field, Paint, Path2d, PathOp,
        RadioItemOpts, Role, Row, Scroll, Semantics, SemanticsAction, Separator, Skeleton, Stack,
        Surface, Switch, TextRun, Ui,
    };
    pub use crate::{AssetError, SendError, TaskSender, UiDiagnostic, UiError, UiProxy, UiResult};
}

/// The one app-level result type (`UiResult` = `UiResult<()>`).
pub type UiResult<T = ()> = Result<T, UiError>;

#[derive(Debug)]
pub enum UiError {
    /// bounded native-event queue (128) overflowed — fatal, never silent
    QueueOverflow,
    Platform(String),
    Asset(AssetError),
    Unsupported(&'static str),
    InvalidUi(UiDiagnostic),
}

/// Typed structural diagnostics — fatal transaction aborts, never silent
/// coercion. Key fingerprints only, never user text.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum UiDiagnostic {
    DuplicateKey,
    DuplicateAutomationId,
    DuplicateTextBinding,
    InvalidLayout,
    /// a rejected programmatic proposal with no `on_conflict` handler —
    /// surfaced as a structured diagnostic and suppressed (no retry loop)
    UnhandledTextConflict,
    /// actionable/focusable/native-peer content inside `ui.action`
    InvalidComposition,
    /// a staged style carried a non-finite/negative/invalid value —
    /// rejected at commit preflight before any native mutation
    InvalidStyle,
}

impl std::fmt::Display for UiDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            UiDiagnostic::DuplicateAutomationId => "duplicate or invalid semantic automation ID",
            UiDiagnostic::DuplicateKey => "duplicate key in one parent scope",
            UiDiagnostic::DuplicateTextBinding => "TextValue bound to two mounted peers",
            UiDiagnostic::InvalidLayout => "invalid layout",
            UiDiagnostic::UnhandledTextConflict => "text conflict without on_conflict handler",
            UiDiagnostic::InvalidComposition => {
                "invalid composition (actionable/peer inside action)"
            }
            UiDiagnostic::InvalidStyle => "invalid style (non-finite/negative/out-of-range value)",
        };
        f.write_str(s)
    }
}

#[derive(Debug)]
pub struct AssetError {
    pub message: String,
}

impl std::fmt::Display for UiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UiError::QueueOverflow => write!(f, "native event queue overflow"),
            UiError::Platform(m) => write!(f, "platform error: {m}"),
            UiError::Asset(a) => write!(f, "asset error: {}", a.message),
            UiError::Unsupported(s) => write!(f, "unsupported: {s}"),
            UiError::InvalidUi(d) => write!(f, "invalid ui: {d}"),
        }
    }
}

impl std::error::Error for UiError {}

#[cfg(windows)]
impl From<windows::core::Error> for UiError {
    fn from(e: windows::core::Error) -> Self {
        UiError::Platform(e.to_string())
    }
}

mod platform;

/// Dev-harness seam — re-exports of the Win32 geometry adapter so the
/// repository's native probe/tests consume the SAME authority instead of
/// re-implementing Win32 semantics. Not application API.
#[cfg(windows)]
#[doc(hidden)]
pub mod dev {
    pub use crate::geom::{
        ClientPhysicalPoint, PeerLocalPoint, PhysicalPoint, ScreenPhysicalPoint,
    };
    pub use crate::platform::win32::space::{
        client_to_screen, dpi_of, scale_from_dpi, screen_to_client, try_lparam_px,
    };
}

#[cfg(test)]
mod tests;
