

/// `text_input`/`text_area` share one builder — `multiline` picks the peer.
pub struct TextInputBuilder<'a, 'ui, 'b, M: 'static> {
    automation_id: Option<Rc<str>>,
    tx: &'a mut Tx<'ui>,
    value: &'b TextValue,
    multiline: bool,
    placeholder: Option<Rc<str>>,
    read_only: bool,
    disabled: bool,
    submit: SubmitPolicy,
    max_lines: Option<u32>,
    accessible_label: Option<Rc<str>>,
    /// semantic `aria-invalid` — unconditional destructive ring
    invalid: bool,
    /// staged selection — UTF-16 `[start, end)`
    selection: Option<(usize, usize)>,
    /// authored exact line box — `None` keeps the font's natural height
    line_height: Option<crate::geom::Dp>,
    presented: Presented,
    /// capability-limited patch — chrome paints the frame, `foreground`
    /// is the only peer-routed field (approved boundary: no size/weight)
    patch: crate::style::TextInputStylePatch,
    visibility: Visibility,
    layout: LayoutSpec,
    factories: EventFactorySet,
    _m: std::marker::PhantomData<fn(&'ui (), &'b ()) -> M>,
}

impl<'a, 'ui, 'b, M: 'static> TextInputBuilder<'a, 'ui, 'b, M> {
    pub fn automation_id(mut self, id: &str) -> Self {
        self.automation_id = Some(Rc::from(id));
        self
    }

    /// capability-limited patch — chrome (`background`/`border`/`radii`/
    /// `padding`/`shadow`) paints the frame; `foreground` is routed to the
    /// peer through the typed adapter. Repeated `.style(..)` merges
    /// fieldwise — `None` preserves, `Some` wins.
    pub fn style(mut self, p: crate::style::TextInputStylePatch) -> Self {
        self.patch.merge(&p);
        self
    }
    /// semantic `aria-invalid` — the frozen invalid ring is unconditional
    /// (`ring-destructive` always-on); orthogonal to disabled/focus
    pub fn invalid(mut self, on: bool) -> Self {
        self.invalid = on;
        self
    }
    /// staged selection — UTF-16 `[start, end)` applied to the peer while
    /// staged; omit to leave selection to native/user control
    pub fn selection(mut self, start: usize, end: usize) -> Self {
        self.selection = Some((start, end));
        self
    }
    /// exact line box — the authored Dp count is calibrated into the
    /// peer font (D2DDWRITE ignores paragraph line-spacing; pitch comes
    /// from `CHARFORMAT.yHeight`). Omit for natural height. Mount-time
    /// only: not diffed/applied on update — a changed value needs a
    /// remount (same as `multiline`)
    pub fn line_height(mut self, h: crate::geom::Dp) -> Self {
        self.line_height = Some(h);
        self
    }
    /// presentation-only staged visual state (`Presented` docs)
    pub fn presented(mut self, p: Presented) -> Self {
        self.presented = p;
        self
    }
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
        let before = self.tx.frames.last().unwrap().children.len();
        let kind = if self.multiline {
            crate::node::KIND_TEXT_AREA
        } else {
            crate::node::KIND_TEXT_INPUT
        };
        // staged snapshot: Rc clones of the value's committed/pending — the
        // peer sync itself is carried by the retained node at commit
        let snapshot = self.value.snapshot();
        let (
            placeholder,
            accessible_label,
            read_only,
            submit,
            max_lines,
            disabled,
            invalid,
            multiline,
            patch,
            selection,
            line_height,
        ) = (
            self.placeholder.clone(),
            self.accessible_label.clone(),
            self.read_only,
            self.submit,
            self.max_lines,
            self.disabled,
            self.invalid,
            self.multiline,
            self.patch,
            self.selection,
            self.line_height,
        );
        let presented = self.presented;
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
                patch,
                invalid,
                selection,
                line_height,
                sync: Default::default(),
            },
        );
        if let Some(index) = self
            .tx
            .frames
            .last()
            .unwrap()
            .children
            .last()
            .copied()
            .filter(|_| self.tx.frames.last().unwrap().children.len() > before)
        {
            if let Some(id) = self.automation_id.take() {
                self.tx.nodes[index as usize].automation_id = Some(id);
            }
            self.tx.nodes[index as usize].presented = presented;
        }
    }
}

/// Custom leaf — immutable `Rc<dyn CustomRender>` snapshot.
pub struct CustomBuilder<'a, 'ui, M: 'static> {
    automation_id: Option<Rc<str>>,
    tx: &'a mut Tx<'ui>,
    render: Rc<dyn CustomRender>,
    frame_events: bool,
    visibility: Visibility,
    layout: LayoutSpec,
    factories: EventFactorySet,
    _m: std::marker::PhantomData<fn() -> M>,
}

impl<'a, 'ui, M: 'static> CustomBuilder<'a, 'ui, M> {
    pub fn automation_id(mut self, id: &str) -> Self {
        self.automation_id = Some(Rc::from(id));
        self
    }

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
        let before = self.tx.frames.last().unwrap().children.len();
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
        if let Some(id) = self.automation_id.take()
            && self.tx.frames.last().unwrap().children.len() > before
            && let Some(index) = self.tx.frames.last().unwrap().children.last().copied()
        {
            self.tx.nodes[index as usize].automation_id = Some(id);
        }
    }
}



