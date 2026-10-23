use std::any::Any;
use std::collections::HashMap;
use std::rc::Rc;

use crate::UiDiagnostic;
use crate::arena::Arena;
use crate::event::{EventFactorySet, FrameTime, KeyEvent, PointerEvent, ScrollOffset};
use crate::geom::{Align, Justify, LayoutSpec, Length, Point, Visibility};
use crate::key::{ChildKey, ErasedKey, KeyId};
use crate::node::{ContainerProps, MsgAdapters, NodeData, NodeId, Presented, SemanticRole};
use crate::style::{Action, BoxProps, ButtonStylePatch};
use crate::text::{TextConflict, TextEdit, TextSelection, TextValue};
use crate::theme::{
    AlertVariant, Appearance, BadgeVariant, ButtonVariant, ColorRole, ControlSize, MotionToken,
    SeparatorOrientation, Space, SubmitPolicy, TabsVariant, Theme,
};

/// What a custom view contributes to the tree — an immutable `Rc` snapshot;
/// no borrowed view data is retained.
pub trait CustomRender {
    fn measure(&self, constraints: crate::geom::Constraints) -> crate::geom::Size;
    fn paint(&self, canvas: &mut dyn Canvas, bounds: crate::geom::Rect);
    fn hit_test(&self, local: Point, bounds: crate::geom::Rect) -> bool {
        let _ = local;
        let _ = bounds;
        true
    }
    fn semantics(&self) -> Semantics;
}

#[derive(Clone, Debug)]
pub struct Semantics {
    pub role: Role,
    pub label: String,
    pub actions: Vec<SemanticsAction>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Role {
    Button,
    Image,
    Text,
    Custom,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SemanticsAction {
    Press,
    Focus,
}

/// Small platform-neutral canvas op set — path and text only in v0.1; no
/// raw OS or GPU handles.
pub trait Canvas {
    fn path(&mut self, path: &Path2d, paint: Paint);
    fn text(&mut self, run: &TextRun, origin: Point);
}

#[derive(Clone, Debug, Default)]
pub struct Path2d {
    pub ops: Vec<PathOp>,
}

#[derive(Clone, Debug)]
pub enum PathOp {
    MoveTo(Point),
    LineTo(Point),
    QuadTo(Point, Point),
    Close,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Paint {
    FillRole(ColorRole),
    Rgba(u8, u8, u8, u8),
}

impl Paint {
    /// Theme-role fill — resolved against the active theme at draw time.
    pub fn fill_role(role: ColorRole) -> Paint {
        Paint::FillRole(role)
    }
}

#[derive(Clone, Debug)]
pub struct TextRun {
    pub text: Rc<str>,
    pub size: f32,
    pub color_role: ColorRole,
}

/// The traversal visitor passed to `view(&S, &mut Ui<M>)`. Nothing it stages
/// touches the retained arena until the transaction commits.
pub struct Ui<'ui, 'tx, M> {
    pub(crate) tx: &'tx mut Tx<'ui>,
    pub(crate) _m: std::marker::PhantomData<&'ui M>,
}

include!("types_a.rs");
include!("types_b.rs");
include!("types_c.rs");
include!("impl_a.rs");
include!("impl_b.rs");
include!("impl_c.rs");
include!("builders.rs");
include!("builders2.rs");
include!("builders3.rs");
include!("groups.rs");
include!("groups2.rs");
include!("input.rs");
