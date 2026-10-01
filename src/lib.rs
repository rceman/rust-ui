//! rust-ui — retained native UI core (Windows-only spike, checkpoint 1).
//! One retained arena + staged transactions + bounded task/mailbox delivery
//! + a narrow interaction scheduler. The Windows backend lands separately;
//! `App::run` is a typed `Unsupported` until then.

mod app;
mod arena;
mod event;
mod geom;
mod key;
mod node;
mod runtime;
mod sched;
mod style;
mod tasks;
mod text;
mod theme;
mod ui;

pub use app::App;
pub use event::{FrameTime, Key, KeyEvent, Modifiers, PointerButton, PointerEvent, ScrollOffset};
pub use geom::{
    Align, Constraints, Dp, Justify, LayoutSpec, Length, Point, Rect, Size, Visibility, dp,
};
pub use key::KeyId;
pub use node::NodeId;
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
    Appearance, ButtonVariant, ColorRole, ControlSize, MotionToken, Radius, ReducedMotion, Space,
    SubmitPolicy, Theme, ThemeMode,
};
pub use ui::{
    ActionBuilder, ButtonBuilder, Canvas, Column, CustomBuilder, CustomRender, LabelBuilder, Paint,
    Path2d, PathOp, Role, Row, Semantics, SemanticsAction, Stack, Surface, TextBuilder,
    TextInputBuilder, TextRun, Ui,
};

/// `use rust_ui::prelude::*` — the documented consumer surface in one place.
pub mod prelude {
    pub use crate::app::App;
    pub use crate::event::{FrameTime, KeyEvent, PointerEvent, ScrollOffset};
    pub use crate::geom::{
        Align, Constraints, Dp, Justify, LayoutSpec, Length, Point, Rect, Size, Visibility, dp,
    };
    pub use crate::runtime::UpdateCtx;
    pub use crate::style::{
        Action, ActionStyle, Border, BorderPatch, BorderSide, BorderSidePatch, BoxProps, BoxStyle,
        BoxStylePatch, ButtonStylePatch, Color, CornerRadii, CornerRadiiPatch, Insets, InsetsPatch,
        Shadow, ShadowPatch, StateStyles, TextSize, TextStyle, TextStylePatch, TextWeight,
        VisualStyle, VisualStylePatch,
    };
    pub use crate::text::{
        AcceptOutcome, BindingToken, EditOrigin, TextConflict, TextEdit, TextRevision,
        TextSelection, TextValue,
    };
    pub use crate::theme::{
        Appearance, ButtonVariant, ColorRole, ControlSize, MotionToken, Radius, ReducedMotion,
        Space, SubmitPolicy, Theme, ThemeMode,
    };
    pub use crate::ui::{
        Canvas, Column, CustomRender, Paint, Path2d, PathOp, Role, Row, Semantics, SemanticsAction,
        Stack, Surface, TextRun, Ui,
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

#[cfg(test)]
mod tests;
