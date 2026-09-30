use std::any::Any;
use std::collections::HashMap;
use std::rc::Rc;

use crate::UiDiagnostic;
use crate::arena::Arena;
use crate::event::{EventFactorySet, FrameTime, KeyEvent, PointerEvent, ScrollOffset};
use crate::geom::{Align, Justify, LayoutSpec, Point, Visibility};
use crate::key::{ChildKey, ErasedKey, KeyId};
use crate::node::{ContainerProps, MsgAdapters, NodeData, NodeId};
use crate::style::{
    Action, BoxProps, ButtonStylePatch,
};
use crate::text::{TextConflict, TextEdit, TextSelection, TextValue};
use crate::theme::{
    Appearance, ButtonVariant, ColorRole, ControlSize, MotionToken, Space,
    SubmitPolicy, Theme,
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

// ------------------------- container props ---------------------------------

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Row {
    pub gap: Space,
    pub align: Align,
    pub justify: Justify,
    pub padding: Space,
}

impl Row {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn gap(mut self, g: Space) -> Self {
        self.gap = g;
        self
    }
    pub fn align(mut self, a: Align) -> Self {
        self.align = a;
        self
    }
    pub fn justify(mut self, j: Justify) -> Self {
        self.justify = j;
        self
    }
    pub fn padding(mut self, p: Space) -> Self {
        self.padding = p;
        self
    }
    pub(crate) fn props(&self) -> ContainerProps {
        ContainerProps {
            gap: Some(self.gap),
            align: Some(self.align),
            justify: Some(self.justify),
            padding: Some(self.padding),
            ..Default::default()
        }
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Column {
    pub gap: Space,
    pub align: Align,
    pub justify: Justify,
    pub padding: Space,
}

impl Column {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn gap(mut self, g: Space) -> Self {
        self.gap = g;
        self
    }
    pub fn align(mut self, a: Align) -> Self {
        self.align = a;
        self
    }
    pub fn justify(mut self, j: Justify) -> Self {
        self.justify = j;
        self
    }
    pub fn padding(mut self, p: Space) -> Self {
        self.padding = p;
        self
    }
    pub(crate) fn props(&self) -> ContainerProps {
        ContainerProps {
            gap: Some(self.gap),
            align: Some(self.align),
            justify: Some(self.justify),
            padding: Some(self.padding),
            ..Default::default()
        }
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Stack {
    pub padding: Space,
}

impl Stack {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn padding(mut self, p: Space) -> Self {
        self.padding = p;
        self
    }
    pub(crate) fn props(&self) -> ContainerProps {
        ContainerProps {
            padding: Some(self.padding),
            ..Default::default()
        }
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Surface {
    pub padding: Space,
    /// surgical patch — merges fieldwise within this build
    pub patch: crate::style::BoxStylePatch,
}

impl Surface {
    pub fn new() -> Self {
        Self::default()
    }
    /// recipe convenience — populates `BoxStyle.padding` before the patch
    pub fn padding(mut self, p: Space) -> Self {
        self.padding = p;
        self
    }
    /// surgical patch — later `.style` calls deep-merge, `None` never erases
    pub fn style(mut self, p: crate::style::BoxStylePatch) -> Self {
        self.patch.merge(&p);
        self
    }
    pub(crate) fn props(&self) -> ContainerProps {
        let mut patch = self.patch;
        // Space padding is a recipe convenience feeding the same BoxStyle
        // padding — a patch field wins over the convenience value
        let d = self.padding.dp();
        for f in [
            &mut patch.padding.top,
            &mut patch.padding.right,
            &mut patch.padding.bottom,
            &mut patch.padding.left,
        ] {
            if f.is_none() {
                *f = Some(d);
            }
        }
        ContainerProps {
            padding: None, // resolved from the box style, not Space
            patch,
            ..Default::default()
        }
    }
}

// ------------------------- staging internals -------------------------------

/// Flat staged node — children are indices into `Tx.nodes`, so the whole
/// transaction is ONE reusable flat buffer, never a persistent second tree.
pub(crate) struct StagedNode {
    pub key: ChildKey,
    pub data: NodeData,
    pub visibility: Visibility,
    pub layout: LayoutSpec,
    pub factories: EventFactorySet,
    /// adapters from enclosing `ui.scope` calls (leaf -> root)
    pub adapters: MsgAdapters,
    /// indices into `Tx.nodes`
    pub children: Vec<u32>,
}

impl StagedNode {
    /// Slot placeholder while a node is moved out of the flat buffer.
    pub(crate) fn placeholder() -> Self {
        StagedNode {
            key: ChildKey::Static {
                kind: 0,
                ordinal: u32::MAX,
            },
            data: NodeData::Empty,
            visibility: Visibility::Hidden,
            layout: LayoutSpec::default(),
            factories: EventFactorySet::default(),
            adapters: Rc::new(Vec::new()),
            children: Vec::new(),
        }
    }

    pub(crate) fn kind_tag(&self) -> u8 {
        match &self.data {
            NodeData::Empty => 0,
            NodeData::Container { kind, .. } => *kind,
            NodeData::Label { .. } => crate::node::KIND_LABEL,
            NodeData::Button { .. } => crate::node::KIND_BUTTON,
            NodeData::Editor { multiline, .. } => {
                if *multiline {
                    crate::node::KIND_TEXT_AREA
                } else {
                    crate::node::KIND_TEXT_INPUT
                }
            }
            NodeData::Custom { .. } => crate::node::KIND_CUSTOM,
            NodeData::Action { .. } => crate::node::KIND_ACTION,
        }
    }

    /// lease identity for the mounted-editor index / duplicate checks
    pub(crate) fn lease_id(&self) -> Option<usize> {
        match &self.data {
            NodeData::Editor { snapshot, .. } => Some(Rc::as_ptr(&snapshot.lease) as usize),
            _ => None,
        }
    }
}

/// One frame = one parent's staging context. `retained` is the matched
/// retained node the frame corresponds to — its children drive O(1) matching
/// and Rc reuse.
pub(crate) struct UiFrame {
    /// child indices into `Tx.nodes` (emission order)
    pub children: Vec<u32>,
    /// single counter for static (no explicit key) siblings — identity is
    /// (kind, emission ordinal)
    pub static_ordinals: u32,
    /// duplicate detection bucketed by key hash — real equality inside the
    /// bucket, never hash-as-identity
    pub key_buckets: HashMap<u64, Vec<ErasedKey>>,
    /// the retained node this frame is staging against (None = no prior)
    pub retained: Option<NodeId>,
    /// retained children bucketed by key hash — built lazily on first match
    pub retained_buckets: Option<HashMap<u64, Vec<u32>>>,
}

impl UiFrame {
    /// Allocate the next static-sibling identity: single emission counter +
    /// kind tag (NOT a per-kind counter).
    fn child_key(
        &mut self,
        kind: u8,
        explicit: Option<ErasedKey>,
    ) -> Result<ChildKey, UiDiagnostic> {
        match explicit {
            Some(k) => {
                let h = k.hash();
                if let Some(bucket) = self.key_buckets.get(&h) {
                    // true equality — collisions can't alias
                    if bucket.iter().any(|e| e.key_eq(&k)) {
                        return Err(UiDiagnostic::DuplicateKey);
                    }
                }
                self.key_buckets.entry(h).or_default().push(k.clone());
                Ok(ChildKey::Explicit(k))
            }
            None => {
                let o = self.static_ordinals;
                self.static_ordinals += 1;
                Ok(ChildKey::Static { kind, ordinal: o })
            }
        }
    }
}

/// Non-generic transaction staging — flat node buffer + frame stack; the
/// retained arena is read-only through `view` and used for matching + Rc
/// reuse. Buffers are cleared (capacity retained) each view pass.
pub(crate) struct Tx<'r> {
    /// >0 while an `ui.action` draw closure runs — actionable/native-peer
    /// children staged inside are InvalidComposition diagnostics
    pub(crate) action_depth: u32,
    /// flat staged node arena — `StagedNode.children` index into this
    pub nodes: Vec<StagedNode>,
    pub frames: Vec<UiFrame>,
    /// active scope-adapter chain (leaf -> root)
    pub adapters: MsgAdapters,
    /// current window theme (staged changes propagate at commit)
    pub theme: Theme,
    /// OS appearance — supplied by the runtime, independent of `theme`
    pub appearance: Appearance,
    /// diagnostics staged by builders (fatal structural errors)
    pub diagnostics: Vec<UiDiagnostic>,
    /// read-only retained arena for matching + `Rc<str>` reuse
    pub retained: &'r Arena,
}

impl Tx<'_> {
    /// The retained node a frame is staging against, and its children.
    fn retained_children(&self, frame: &UiFrame) -> &[u32] {
        frame
            .retained
            .and_then(|id| self.retained.get(id))
            .map(|n| n.children.as_slice())
            .unwrap_or(&[])
    }

    /// Find the retained child matching `key`/`kind` in the current frame —
    /// hash-bucketed, equality-confirmed; never a quadratic sibling scan.
    fn match_retained(&mut self, frame_idx: usize, key: &ChildKey, kind: u8) -> Option<NodeId> {
        let h = key.hash();
        if self.frames[frame_idx].retained_buckets.is_none() {
            let frame = &self.frames[frame_idx];
            let mut buckets: HashMap<u64, Vec<u32>> = HashMap::new();
            for &slot in self.retained_children(frame) {
                let g = self.retained.generation_of(slot);
                if let Some(n) = self.retained.get(NodeId {
                    slot,
                    generation: g,
                }) {
                    buckets.entry(n.key.hash()).or_default().push(slot);
                }
            }
            self.frames[frame_idx].retained_buckets = Some(buckets);
        }
        let buckets = self.frames[frame_idx].retained_buckets.as_ref().unwrap();
        for &slot in buckets.get(&h).into_iter().flatten() {
            let g = self.retained.generation_of(slot);
            if let Some(n) = self.retained.get(NodeId {
                slot,
                generation: g,
            }) && n.key.key_eq(key)
                && n.kind_tag() == kind
            {
                return Some(NodeId {
                    slot,
                    generation: g,
                });
            }
        }
        None
    }

    /// Stage a leaf: key allocation + retained match + push into flat buffer.
    /// `build` gets the matched retained node (for Rc reuse).
    pub(crate) fn stage_leaf(
        &mut self,
        kind: u8,
        explicit: Option<ErasedKey>,
        visibility: Visibility,
        layout: LayoutSpec,
        factories: EventFactorySet,
        build: impl FnOnce(Option<&crate::node::Node>) -> NodeData,
    ) {
        if let Some(d) = layout.validate() {
            self.diagnostics.push(d);
            return;
        }
        // action content is decorative — no actionable/focusable/native
        // peer children inside a semantic action
        if self.action_depth > 0
            && matches!(kind, crate::node::KIND_BUTTON | crate::node::KIND_ACTION
                        | crate::node::KIND_TEXT_INPUT | crate::node::KIND_TEXT_AREA)
        {
            self.diagnostics.push(UiDiagnostic::InvalidComposition);
            return;
        }
        let frame_idx = self.frames.len() - 1;
        let key = match self.frames[frame_idx].child_key(kind, explicit) {
            Ok(k) => k,
            Err(d) => {
                self.diagnostics.push(d);
                return;
            }
        };
        let retained_node = self
            .match_retained(frame_idx, &key, kind)
            .and_then(|id| self.retained.get(id));
        let data = build(retained_node);
        let idx = self.nodes.len() as u32;
        self.nodes.push(StagedNode {
            key,
            data,
            visibility,
            layout,
            factories,
            adapters: self.adapters.clone(),
            children: Vec::new(),
        });
        self.frames[frame_idx].children.push(idx);
    }

    /// Stage a container: draw children into a fresh frame against the
    /// matched retained node, then append the container. `build` produces
    /// the node data (Container for layout kinds, Action for `ui.action`).
    fn stage_container<M: 'static>(
        &mut self,
        kind: u8,
        explicit: Option<ErasedKey>,
        props: ContainerProps,
        layout: LayoutSpec,
        factories: EventFactorySet,
        draw: impl FnOnce(&mut Ui<'_, '_, M>),
        action_depth: bool,
        build: impl FnOnce(ContainerProps) -> NodeData,
    ) {
        if let Some(d) = layout.validate() {
            self.diagnostics.push(d);
            return;
        }
        // nested actionable inside an action's decorative content is a
        // structural diagnostic, not event bubbling
        if self.action_depth > 0
            && matches!(kind, crate::node::KIND_BUTTON | crate::node::KIND_ACTION
                        | crate::node::KIND_TEXT_INPUT | crate::node::KIND_TEXT_AREA)
        {
            self.diagnostics.push(UiDiagnostic::InvalidComposition);
            return;
        }
        let frame_idx = self.frames.len() - 1;
        let key = match self.frames[frame_idx].child_key(kind, explicit) {
            Ok(k) => k,
            Err(d) => {
                self.diagnostics.push(d);
                return;
            }
        };
        let retained = self.match_retained(frame_idx, &key, kind);
        // push a child frame bound to the matched retained node
        self.frames.push(UiFrame {
            children: Vec::new(),
            static_ordinals: 0,
            retained_buckets: None,
            key_buckets: HashMap::new(),
            retained,
        });
        if action_depth {
            self.action_depth += 1;
        }
        // hand a Ui view of the same Tx to draw — self re-borrowed
        {
            let mut ui = Ui::<M> {
                tx: self,
                _m: std::marker::PhantomData,
            };
            draw(&mut ui);
        }
        if action_depth {
            self.action_depth -= 1;
        }
        let child_frame = self.frames.pop().unwrap();
        let data = build(props);
        // leaf kinds staged inside an action are checked at stage_leaf;
        // the container itself inside an action is still invalid
        let idx = self.nodes.len() as u32;
        self.nodes.push(StagedNode {
            key,
            data,
            visibility: Visibility::Visible,
            layout,
            factories,
            adapters: self.adapters.clone(),
            children: child_frame.children,
        });
        self.frames[frame_idx].children.push(idx);
    }
}

/// The traversal visitor passed to `view(&S, &mut Ui<M>)`. Nothing it stages
/// touches the retained arena until the transaction commits.
pub struct Ui<'ui, 'tx, M> {
    pub(crate) tx: &'tx mut Tx<'ui>,
    pub(crate) _m: std::marker::PhantomData<&'ui M>,
}

impl<'ui, 'tx, M: 'static> Ui<'ui, 'tx, M> {
    // ----- static-identity primitives --------------------------------------

    /// Explicit-keyed container — `group(key, |ui| ...)` stabilises
    /// conditional regions under the key's identity.
    pub fn group(&mut self, key: impl KeyId, draw: impl FnOnce(&mut Ui<'_, '_, M>)) {
        self.tx.stage_container(
            crate::node::KIND_GROUP,
            Some(ErasedKey::new(key)),
            ContainerProps::default(),
            LayoutSpec::default(),
            EventFactorySet::default(),
            draw,
            false,
            |props| NodeData::Container { kind: crate::node::KIND_GROUP, props },
        );
    }

    /// Unkeyed structural containers.
    pub fn column(&mut self, props: Column, draw: impl FnOnce(&mut Ui<'_, '_, M>)) {
        self.tx.stage_container(
            crate::node::KIND_COLUMN,
            None,
            props.props(),
            LayoutSpec::default(),
            EventFactorySet::default(),
            draw,
            false,
            |props| NodeData::Container { kind: crate::node::KIND_COLUMN, props },
        );
    }
    pub fn row(&mut self, props: Row, draw: impl FnOnce(&mut Ui<'_, '_, M>)) {
        self.tx.stage_container(
            crate::node::KIND_ROW,
            None,
            props.props(),
            LayoutSpec::default(),
            EventFactorySet::default(),
            draw,
            false,
            |props| NodeData::Container { kind: crate::node::KIND_ROW, props },
        );
    }
    pub fn stack(&mut self, props: Stack, draw: impl FnOnce(&mut Ui<'_, '_, M>)) {
        self.tx.stage_container(
            crate::node::KIND_STACK,
            None,
            props.props(),
            LayoutSpec::default(),
            EventFactorySet::default(),
            draw,
            false,
            |props| NodeData::Container { kind: crate::node::KIND_STACK, props },
        );
    }
        pub fn surface(&mut self, props: Surface, draw: impl FnOnce(&mut Ui<'_, '_, M>)) {
        self.tx.stage_container(
            crate::node::KIND_SURFACE,
            None,
            props.props(),
            LayoutSpec::default(),
            EventFactorySet::default(),
            draw,
            false,
            |props| NodeData::Container { kind: crate::node::KIND_SURFACE, props },
        );
    }

    // ----- leaves -----------------------------------------------------------

    /// `impl AsRef<str>` — an owned `String`/`format!` value or a `&str`
    /// both stage directly; the retained `Rc<str>` is reused when the text
    /// is unchanged, so borrowed strings cost no copy.
    pub fn label<'a, T: AsRef<str>>(&'a mut self, text: T) -> LabelBuilder<'a, 'ui, T, M> {
        LabelBuilder {
            tx: self.tx,
            text,
            wrap: false,
            color_role: ColorRole::Foreground,
            patch: crate::style::TextStylePatch::default(),
            visibility: Visibility::Visible,
            layout: LayoutSpec::default(),
            factories: EventFactorySet::default(),
            _m: std::marker::PhantomData,
        }
    }

    pub fn button<'a>(&'a mut self, text: &'a str) -> ButtonBuilder<'a, 'ui, M> {
        ButtonBuilder {
            tx: self.tx,
            text,
            variant: ButtonVariant::default(),
            style: ButtonStylePatch::default(),
            disabled: false,
            size: ControlSize::default(),
            motion: Some(MotionToken::Hover),
            tooltip: None,
            visibility: Visibility::Visible,
            layout: LayoutSpec::default(),
            factories: EventFactorySet::default(),
            _m: std::marker::PhantomData,
        }
    }

    pub fn text_input<'a, 'b>(
        &'a mut self,
        value: &'b TextValue,
    ) -> TextInputBuilder<'a, 'ui, 'b, M> {
        TextInputBuilder {
            tx: self.tx,
            value,
            multiline: false,
            placeholder: None,
            read_only: false,
            disabled: false,
            submit: SubmitPolicy::default(),
            max_lines: None,
            accessible_label: None,
            visibility: Visibility::Visible,
            layout: LayoutSpec::default(),
            factories: EventFactorySet::default(),
            _m: std::marker::PhantomData,
        }
    }

    pub fn text_area<'a, 'b>(
        &'a mut self,
        value: &'b TextValue,
    ) -> TextInputBuilder<'a, 'ui, 'b, M> {
        TextInputBuilder {
            tx: self.tx,
            value,
            multiline: true,
            placeholder: None,
            read_only: false,
            disabled: false,
            submit: SubmitPolicy::None,
            max_lines: None,
            accessible_label: None,
            visibility: Visibility::Visible,
            layout: LayoutSpec::default(),
            factories: EventFactorySet::default(),
            _m: std::marker::PhantomData,
        }
    }

    pub fn custom(&mut self, render: Rc<dyn CustomRender>) -> CustomBuilder<'_, 'ui, M> {
        CustomBuilder {
            tx: self.tx,
            render,
            frame_events: false,
            visibility: Visibility::Visible,
            layout: LayoutSpec::default(),
            factories: EventFactorySet::default(),
            _m: std::marker::PhantomData,
        }
    }

    // ----- scoped composition ------------------------------------------------

    /// `ui.scope(key, map, draw)` — a child message namespace: the child's
    /// `M2` is lifted to `M` through `map` at dispatch.
    pub fn scope<ChildMsg: 'static>(
        &mut self,
        key: impl KeyId,
        map: impl Fn(ChildMsg) -> M + 'static,
        draw: impl FnOnce(&mut Ui<'_, '_, ChildMsg>),
    ) {
        // adapter chain for children of this scope
        let parent_adapters = self.tx.adapters.clone();
        let mut lifted: Vec<Rc<dyn Fn(Box<dyn Any>) -> Box<dyn Any>>> =
            Vec::with_capacity(parent_adapters.len() + 1);
        lifted.push(Rc::new(move |child| {
            Box::new(map(*child.downcast::<ChildMsg>().unwrap())) as Box<dyn Any>
        }));
        for a in parent_adapters.iter() {
            lifted.push(a.clone());
        }
        let child_adapters: MsgAdapters = Rc::new(lifted);

        // stage the scope container in the parent frame
        let frame_idx = self.tx.frames.len() - 1;
        let key = match self.tx.frames[frame_idx]
            .child_key(crate::node::KIND_SCOPE, Some(ErasedKey::new(key)))
        {
            Ok(k) => k,
            Err(d) => {
                self.tx.diagnostics.push(d);
                return;
            }
        };
        let retained = self
            .tx
            .match_retained(frame_idx, &key, crate::node::KIND_SCOPE);
        self.tx.frames.push(UiFrame {
            children: Vec::new(),
            static_ordinals: 0,
            retained_buckets: None,
            key_buckets: HashMap::new(),
            retained,
        });
        // child's adapters swapped in for the draw body
        let saved = std::mem::replace(&mut self.tx.adapters, child_adapters);
        {
            let mut child_ui = Ui::<ChildMsg> {
                tx: self.tx,
                _m: std::marker::PhantomData,
            };
            draw(&mut child_ui);
        }
        self.tx.adapters = saved;
        let child_frame = self.tx.frames.pop().unwrap();
        let idx = self.tx.nodes.len() as u32;
        self.tx.nodes.push(StagedNode {
            key,
            data: NodeData::Container {
                kind: crate::node::KIND_SCOPE,
                props: ContainerProps::default(),
            },
            visibility: Visibility::Visible,
            layout: LayoutSpec::default(),
            factories: EventFactorySet::default(),
            adapters: self.tx.adapters.clone(),
            children: child_frame.children,
        });
        self.tx.frames[frame_idx].children.push(idx);
    }

    /// `keyed(items, key, draw)` — an implicit static-ordinal list scope (two
    /// adjacent `keyed` calls are distinct siblings), then a keyed scope per
    /// item inside it.
    pub fn keyed<T, K: Eq + std::hash::Hash + Clone + 'static>(
        &mut self,
        items: &[T],
        key: impl Fn(&T) -> K,
        draw: impl Fn(&mut Ui<'_, '_, M>, &T),
    ) {
        // implicit scope by static emission ordinal — no fixed constant key,
        // so adjacent `keyed` lists in one parent stay distinct
        self.tx.stage_container(
            crate::node::KIND_GROUP,
            None,
            ContainerProps::default(),
            LayoutSpec::default(),
            EventFactorySet::default(),
            |ui: &mut Ui<'_, '_, M>| {
                for item in items {
                    let k = ErasedKey::new(key(item));
                    ui.tx.stage_container(
                        crate::node::KIND_GROUP,
                        Some(k),
                        ContainerProps::default(),
                        LayoutSpec::default(),
                        EventFactorySet::default(),
                        |u| draw(u, item),
                        false,
                        |props| NodeData::Container { kind: crate::node::KIND_GROUP, props },
                    );
                }
            },
            false,
            |props| NodeData::Container { kind: crate::node::KIND_GROUP, props },
        );
    }

    /// `ui.box_` — noninteractive painted box container; `BoxProps` carries
    /// the authored full `BoxStyle` plus layout inputs (size stays layout).
    pub fn box_(&mut self, props: BoxProps, draw: impl FnOnce(&mut Ui<'_, '_, M>)) {
        let mut cp = ContainerProps {
            full: Some(props.style),
            ..Default::default()
        };
        // authored padding feeds the container inner insets
        let _ = &mut cp;
        self.tx.stage_container(
            crate::node::KIND_BOX,
            None,
            cp,
            props.layout,
            EventFactorySet::default(),
            draw,
            false,
            |props| NodeData::Container { kind: crate::node::KIND_BOX, props },
        );
    }

    /// `ui.action` — semantic activation wrapper over arbitrary decorative
    /// content. The action node is the single hit/focus/press owner; nested
    /// actionable or native-peer children are InvalidComposition.
    pub fn action<'a>(
        &'a mut self,
        props: Action,
        draw: impl FnOnce(&mut Ui<'_, '_, M>),
    ) -> ActionBuilder<'a, 'ui, M> {
        let before = self.tx.nodes.len();
        self.tx.stage_container(
            crate::node::KIND_ACTION,
            None,
            ContainerProps::default(),
            LayoutSpec::default(),
            EventFactorySet::default(),
            draw,
            true,
            move |_| NodeData::Action {
                label: Rc::from(props.label.as_str()),
                style: props.style,
                disabled: props.disabled,
            },
        );
        // the staged node is the last pushed (containers append last)
        let staged = (self.tx.nodes.len() > before)
            .then_some(self.tx.nodes.len() as u32 - 1);
        ActionBuilder {
            tx: self.tx,
            staged,
            _m: std::marker::PhantomData,
        }
    }

    /// `ui.text` — styled text leaf; `label` is a recipe convenience over
    /// the same renderer.
    pub fn text<'a, T: AsRef<str>>(&'a mut self, text: T) -> TextBuilder<'a, 'ui, T, M> {
        TextBuilder {
            tx: self.tx,
            text,
            wrap: false,
            style: crate::style::TextStyle::default(),
            visibility: Visibility::Visible,
            layout: LayoutSpec::default(),
            _m: std::marker::PhantomData,
        }
    }

    /// Stage a theme override — propagated to the runtime at commit.
    pub fn theme(&mut self, theme: Theme) {
        self.tx.theme = theme;
    }

    /// Actual OS appearance — supplied by the runtime, not derived from the
    /// selected theme.
    pub fn appearance(&self) -> Appearance {
        self.tx.appearance
    }

    /// The active window theme.
    pub fn theme_ref(&self) -> &Theme {
        &self.tx.theme
    }
}

// ============================ builders ======================================

/// Every builder stages on `Drop` — `.on_*(...)` setters register factories
/// retained at commit only.
pub struct LabelBuilder<'a, 'ui, T: AsRef<str>, M: 'static> {
    tx: &'a mut Tx<'ui>,
    text: T,
    wrap: bool,
    color_role: ColorRole,
    patch: crate::style::TextStylePatch,
    visibility: Visibility,
    layout: LayoutSpec,
    factories: EventFactorySet,
    _m: std::marker::PhantomData<fn() -> M>,
}

impl<'a, 'ui, T: AsRef<str>, M: 'static> LabelBuilder<'a, 'ui, T, M> {
    pub fn wrap(mut self, wrap: bool) -> Self {
        self.wrap = wrap;
        self
    }
    /// typed shorthand — sets only the recipe patch's foreground role
    pub fn color_role(mut self, r: ColorRole) -> Self {
        self.color_role = r;
        self
    }
    /// surgical text patch — merges fieldwise within this build
    pub fn style(mut self, p: crate::style::TextStylePatch) -> Self {
        self.patch.merge(&p);
        self
    }
    pub fn visibility(mut self, v: Visibility) -> Self {
        self.visibility = v;
        self
    }
    pub fn layout(mut self, l: LayoutSpec) -> Self {
        self.layout = l;
        self
    }
    pub fn width(mut self, w: crate::geom::Length) -> Self {
        self.layout.width = w;
        self
    }
    pub fn height(mut self, h: crate::geom::Length) -> Self {
        self.layout.height = h;
        self
    }
    pub fn min_width(mut self, d: crate::geom::Dp) -> Self {
        self.layout.min_width = Some(d);
        self
    }
    pub fn max_width(mut self, d: crate::geom::Dp) -> Self {
        self.layout.max_width = Some(d);
        self
    }
}

impl<'a, 'ui, T: AsRef<str>, M: 'static> Drop for LabelBuilder<'a, 'ui, T, M> {
    fn drop(&mut self) {
        let text = self.text.as_ref();
        let wrap = self.wrap;
        let role = self.color_role;
        let mut patch = self.patch;
        // color_role shorthand populates the patch's foreground only when
        // the consumer didn't set one explicitly
        if patch.foreground.is_none() {
            patch.foreground = Some(crate::style::Color::Role(role));
        }
        self.tx.stage_leaf(
            crate::node::KIND_LABEL,
            None,
            self.visibility,
            self.layout,
            std::mem::take(&mut self.factories),
            move |retained| {
                // reuse the retained Rc<str> when text is unchanged — no
                // per-view copy
                let text = match retained {
                    Some(crate::node::Node {
                        data: NodeData::Label { text: old, .. },
                        ..
                    }) if &**old == text => old.clone(),
                    _ => Rc::from(text),
                };
                NodeData::Label {
                    text,
                    wrap,
                    color_role: role,
                    patch,
                }
            },
        );
    }
}

pub struct ButtonBuilder<'a, 'ui, M: 'static> {
    tx: &'a mut Tx<'ui>,
    text: &'a str,
    variant: ButtonVariant,
    style: ButtonStylePatch,
    disabled: bool,
    size: ControlSize,
    motion: Option<MotionToken>,
    tooltip: Option<Rc<str>>,
    visibility: Visibility,
    layout: LayoutSpec,
    factories: EventFactorySet,
    _m: std::marker::PhantomData<fn() -> M>,
}

impl<'a, 'ui, M: 'static> ButtonBuilder<'a, 'ui, M> {
    pub fn variant(mut self, v: ButtonVariant) -> Self {
        self.variant = v;
        self
    }
    pub fn size(mut self, s: ControlSize) -> Self {
        self.size = s;
        self
    }
    /// surgical patch — merges fieldwise into the same build's patch;
    /// each view pass rebuilds from declaration (no history)
    pub fn style(mut self, s: ButtonStylePatch) -> Self {
        self.style.merge(&s);
        self
    }
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
    pub fn motion(mut self, t: Option<MotionToken>) -> Self {
        self.motion = t;
        self
    }
    /// neutral tooltip — semantic delayed show, reduced-motion-aware
    pub fn tooltip(mut self, text: &str) -> Self {
        self.tooltip = Some(Rc::from(text));
        self
    }
    pub fn visibility(mut self, v: Visibility) -> Self {
        self.visibility = v;
        self
    }
    pub fn layout(mut self, l: LayoutSpec) -> Self {
        self.layout = l;
        self
    }
    pub fn width(mut self, w: crate::geom::Length) -> Self {
        self.layout.width = w;
        self
    }
    pub fn height(mut self, h: crate::geom::Length) -> Self {
        self.layout.height = h;
        self
    }
    pub fn on_press(mut self, f: impl Fn() -> M + 'static) -> Self {
        self.factories.on_press = Some(Box::new(move || Box::new(f())));
        self
    }
    pub fn on_pointer_enter(mut self, f: impl Fn(PointerEvent) -> M + 'static) -> Self {
        self.factories.on_pointer_enter = Some(Box::new(move |e| Box::new(f(e))));
        self
    }
    pub fn on_pointer_leave(mut self, f: impl Fn(PointerEvent) -> M + 'static) -> Self {
        self.factories.on_pointer_leave = Some(Box::new(move |e| Box::new(f(e))));
        self
    }
    pub fn on_pointer_down(mut self, f: impl Fn(PointerEvent) -> M + 'static) -> Self {
        self.factories.on_pointer_down = Some(Box::new(move |e| Box::new(f(e))));
        self
    }
    pub fn on_pointer_up(mut self, f: impl Fn(PointerEvent) -> M + 'static) -> Self {
        self.factories.on_pointer_up = Some(Box::new(move |e| Box::new(f(e))));
        self
    }
    pub fn on_focus(mut self, f: impl Fn() -> M + 'static) -> Self {
        self.factories.on_focus = Some(Box::new(move || Box::new(f())));
        self
    }
    pub fn on_blur(mut self, f: impl Fn() -> M + 'static) -> Self {
        self.factories.on_blur = Some(Box::new(move || Box::new(f())));
        self
    }
    pub fn on_key(mut self, f: impl Fn(KeyEvent) -> Option<M> + 'static) -> Self {
        self.factories.on_key = Some(Box::new(move |e| f(e).map(|m| Box::new(m) as _)));
        self
    }
}

impl<'a, 'ui, M: 'static> Drop for ButtonBuilder<'a, 'ui, M> {
    fn drop(&mut self) {
        let text = self.text;
        let (variant, style, disabled, size, motion, tooltip) = (
            self.variant,
            self.style,
            self.disabled,
            self.size,
            self.motion,
            self.tooltip.clone(),
        );
        self.tx.stage_leaf(
            crate::node::KIND_BUTTON,
            None,
            self.visibility,
            self.layout,
            std::mem::take(&mut self.factories),
            move |retained| {
                let text = match retained {
                    Some(crate::node::Node {
                        data: NodeData::Button { text: old, .. },
                        ..
                    }) if &**old == text => old.clone(),
                    _ => Rc::from(text),
                };
                NodeData::Button {
                    text,
                    variant,
                    style,
                    disabled,
                    size,
                    motion,
                    tooltip,
                }
            },
        );
    }
}

/// `text_input`/`text_area` share one builder — `multiline` picks the peer.
pub struct TextInputBuilder<'a, 'ui, 'b, M: 'static> {
    tx: &'a mut Tx<'ui>,
    value: &'b TextValue,
    multiline: bool,
    placeholder: Option<Rc<str>>,
    read_only: bool,
    disabled: bool,
    submit: SubmitPolicy,
    max_lines: Option<u32>,
    accessible_label: Option<Rc<str>>,
    visibility: Visibility,
    layout: LayoutSpec,
    factories: EventFactorySet,
    _m: std::marker::PhantomData<fn(&'ui (), &'b ()) -> M>,
}

impl<'a, 'ui, 'b, M: 'static> TextInputBuilder<'a, 'ui, 'b, M> {
    pub fn placeholder(mut self, p: &str) -> Self {
        self.placeholder = Some(Rc::from(p));
        self
    }
    pub fn read_only(mut self, r: bool) -> Self {
        self.read_only = r;
        self
    }
    /// accessible name for the editable control (screen readers)
    pub fn label(mut self, l: &str) -> Self {
        self.accessible_label = Some(Rc::from(l));
        self
    }
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
    pub fn submit_policy(mut self, p: SubmitPolicy) -> Self {
        self.submit = p;
        self
    }
    pub fn max_lines(mut self, n: u32) -> Self {
        self.max_lines = Some(n);
        self
    }
    pub fn visibility(mut self, v: Visibility) -> Self {
        self.visibility = v;
        self
    }
    pub fn layout(mut self, l: LayoutSpec) -> Self {
        self.layout = l;
        self
    }
    pub fn width(mut self, w: crate::geom::Length) -> Self {
        self.layout.width = w;
        self
    }
    pub fn height(mut self, h: crate::geom::Length) -> Self {
        self.layout.height = h;
        self
    }
    /// committed edits — the payload is the actual `TextEdit`; the app
    /// accepts it via `value.accept(edit)` inside `update`.
    pub fn on_edit(mut self, f: impl Fn(TextEdit) -> M + 'static) -> Self {
        self.factories.on_edit = Some(Box::new(move |e| Box::new(f(e))));
        self
    }
    pub fn on_submit(mut self, f: impl Fn() -> M + 'static) -> Self {
        self.factories.on_submit = Some(Box::new(move || Box::new(f())));
        self
    }
    pub fn on_conflict(mut self, f: impl Fn(TextConflict) -> M + 'static) -> Self {
        self.factories.on_conflict = Some(Box::new(move |e| Box::new(f(e))));
        self
    }
    pub fn on_selection_changed(mut self, f: impl Fn(TextSelection) -> M + 'static) -> Self {
        self.factories.on_selection_changed = Some(Box::new(move |e| Box::new(f(e))));
        self
    }
}

impl<'a, 'ui, 'b, M: 'static> Drop for TextInputBuilder<'a, 'ui, 'b, M> {
    fn drop(&mut self) {
        let kind = if self.multiline {
            crate::node::KIND_TEXT_AREA
        } else {
            crate::node::KIND_TEXT_INPUT
        };
        // staged snapshot: Rc clones of the value's committed/pending — the
        // peer sync itself is carried by the retained node at commit
        let snapshot = self.value.snapshot();
        let (placeholder, accessible_label, read_only, submit, max_lines, disabled, multiline) = (
            self.placeholder.clone(),
            self.accessible_label.clone(),
            self.read_only,
            self.submit,
            self.max_lines,
            self.disabled,
            self.multiline,
        );
        self.tx.stage_leaf(
            kind,
            None,
            self.visibility,
            self.layout,
            std::mem::take(&mut self.factories),
            move |_| NodeData::Editor {
                snapshot,
                multiline,
                read_only,
                disabled,
                submit,
                max_lines,
                placeholder,
                accessible_label,
                sync: Default::default(),
            },
        );
    }
}

/// Custom leaf — immutable `Rc<dyn CustomRender>` snapshot.
pub struct CustomBuilder<'a, 'ui, M: 'static> {
    tx: &'a mut Tx<'ui>,
    render: Rc<dyn CustomRender>,
    frame_events: bool,
    visibility: Visibility,
    layout: LayoutSpec,
    factories: EventFactorySet,
    _m: std::marker::PhantomData<fn() -> M>,
}

impl<'a, 'ui, M: 'static> CustomBuilder<'a, 'ui, M> {
    /// decorative frame demand — gated by visibility + reduced motion
    pub fn frame_events(mut self, on: bool) -> Self {
        self.frame_events = on;
        self
    }
    pub fn visibility(mut self, v: Visibility) -> Self {
        self.visibility = v;
        self
    }
    pub fn layout(mut self, l: LayoutSpec) -> Self {
        self.layout = l;
        self
    }
    pub fn width(mut self, w: crate::geom::Length) -> Self {
        self.layout.width = w;
        self
    }
    pub fn height(mut self, h: crate::geom::Length) -> Self {
        self.layout.height = h;
        self
    }
    pub fn on_frame(mut self, f: impl Fn(FrameTime) -> M + 'static) -> Self {
        self.factories.on_frame = Some(Box::new(move |t| Box::new(f(t))));
        self
    }
    /// custom actionable content wires the same activation as a real button
    pub fn on_press(mut self, f: impl Fn() -> M + 'static) -> Self {
        self.factories.on_press = Some(Box::new(move || Box::new(f())));
        self
    }
    pub fn on_scroll(mut self, f: impl Fn(ScrollOffset) -> M + 'static) -> Self {
        self.factories.on_scroll = Some(Box::new(move |o| Box::new(f(o))));
        self
    }
}

impl<'a, 'ui, M: 'static> Drop for CustomBuilder<'a, 'ui, M> {
    fn drop(&mut self) {
        let render = self.render.clone();
        let frame_events = self.frame_events;
        self.tx.stage_leaf(
            crate::node::KIND_CUSTOM,
            None,
            self.visibility,
            self.layout,
            std::mem::take(&mut self.factories),
            move |_| NodeData::Custom {
                render,
                frame_events,
            },
        );
    }
}

/// `ui.action` builder — the node stages immediately (children must draw
/// inside it); factory methods retrofit onto the staged node before the
/// transaction commits.
pub struct ActionBuilder<'a, 'ui, M: 'static> {
    tx: &'a mut Tx<'ui>,
    /// staged node index in `tx.nodes` — None when the stage was rejected
    staged: Option<u32>,
    _m: std::marker::PhantomData<fn() -> M>,
}

impl<'a, 'ui, M: 'static> ActionBuilder<'a, 'ui, M> {
    fn set(&mut self, f: impl FnOnce(&mut EventFactorySet)) {
        if let Some(i) = self.staged {
            f(&mut self.tx.nodes[i as usize].factories);
        }
    }
    pub fn on_press(mut self, f: impl Fn() -> M + 'static) -> Self {
        self.set(|fac| fac.on_press = Some(Box::new(move || Box::new(f()))));
        self
    }
    pub fn on_focus(mut self, f: impl Fn() -> M + 'static) -> Self {
        self.set(|fac| fac.on_focus = Some(Box::new(move || Box::new(f()))));
        self
    }
    pub fn on_blur(mut self, f: impl Fn() -> M + 'static) -> Self {
        self.set(|fac| fac.on_blur = Some(Box::new(move || Box::new(f()))));
        self
    }
    pub fn on_key(mut self, f: impl Fn(KeyEvent) -> Option<M> + 'static) -> Self {
        self.set(|fac| fac.on_key = Some(Box::new(move |e| f(e).map(|m| Box::new(m) as _))));
        self
    }
    pub fn on_pointer_enter(mut self, f: impl Fn(PointerEvent) -> M + 'static) -> Self {
        self.set(|fac| fac.on_pointer_enter = Some(Box::new(move |e| Box::new(f(e)))));
        self
    }
    pub fn on_pointer_leave(mut self, f: impl Fn(PointerEvent) -> M + 'static) -> Self {
        self.set(|fac| fac.on_pointer_leave = Some(Box::new(move |e| Box::new(f(e)))));
        self
    }
}

/// `ui.text` — styled text leaf with a full authored `TextStyle`.
pub struct TextBuilder<'a, 'ui, T: AsRef<str>, M: 'static> {
    tx: &'a mut Tx<'ui>,
    text: T,
    wrap: bool,
    style: crate::style::TextStyle,
    visibility: Visibility,
    layout: LayoutSpec,
    _m: std::marker::PhantomData<fn() -> M>,
}

impl<'a, 'ui, T: AsRef<str>, M: 'static> TextBuilder<'a, 'ui, T, M> {
    /// full authored style — primitives want complete styles, not patches
    pub fn style(mut self, s: crate::style::TextStyle) -> Self {
        self.style = s;
        self
    }
    pub fn wrap(mut self, w: bool) -> Self {
        self.wrap = w;
        self
    }
    pub fn visibility(mut self, v: Visibility) -> Self {
        self.visibility = v;
        self
    }
    pub fn width(mut self, w: crate::geom::Length) -> Self {
        self.layout.width = w;
        self
    }
    pub fn height(mut self, h: crate::geom::Length) -> Self {
        self.layout.height = h;
        self
    }
}

impl<'a, 'ui, T: AsRef<str>, M: 'static> Drop for TextBuilder<'a, 'ui, T, M> {
    fn drop(&mut self) {
        // authored full style lands as a patch over the label recipe — the
        // staged label shape is shared with `ui.label`
        let mut patch = crate::style::TextStylePatch::default();
        patch.foreground = Some(self.style.foreground);
        if let crate::style::TextSize::Exact(d) = self.style.size {
            patch.size = Some(d);
        }
        patch.weight = Some(self.style.weight);
        let text = self.text.as_ref();
        let wrap = self.wrap;
        self.tx.stage_leaf(
            crate::node::KIND_LABEL,
            None,
            self.visibility,
            self.layout,
            EventFactorySet::default(),
            move |retained| {
                let text = match retained {
                    Some(crate::node::Node {
                        data: NodeData::Label { text: old, .. },
                        ..
                    }) if &**old == text => old.clone(),
                    _ => Rc::from(text),
                };
                NodeData::Label {
                    text,
                    wrap,
                    color_role: ColorRole::Foreground,
                    patch,
                }
            },
        );
    }
}
