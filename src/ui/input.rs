pub struct InputBuilder<'a, 'ui, 'b, M: 'static> {
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
    invalid: bool,
    selection: Option<(usize, usize)>,
    /// authored exact line box — `None` keeps the font's natural height
    line_height: Option<crate::geom::Dp>,
    presented: Presented,
    patch: crate::style::TextInputStylePatch,
    visibility: Visibility,
    layout: LayoutSpec,
    factories: EventFactorySet,
    _m: std::marker::PhantomData<fn(&'ui (), &'b ()) -> M>,
}

impl<'a, 'ui, 'b, M: 'static> InputBuilder<'a, 'ui, 'b, M> {
    fn new(tx: &'a mut Tx<'ui>, value: &'b TextValue, multiline: bool) -> Self {
        InputBuilder {
            automation_id: None,
            tx,
            value,
            multiline,
            placeholder: None,
            read_only: false,
            disabled: false,
            submit: if multiline {
                SubmitPolicy::None
            } else {
                SubmitPolicy::default()
            },
            max_lines: None,
            accessible_label: None,
            invalid: false,
            selection: None,
            line_height: None,
            presented: Presented::default(),
            patch: crate::style::TextInputStylePatch::default(),
            visibility: Visibility::Visible,
            layout: LayoutSpec::default(),
            factories: EventFactorySet::default(),
            _m: std::marker::PhantomData,
        }
    }
    pub fn automation_id(mut self, id: &str) -> Self {
        self.automation_id = Some(Rc::from(id));
        self
    }
    pub fn style(mut self, p: crate::style::TextInputStylePatch) -> Self {
        self.patch.merge(&p);
        self
    }
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

impl<'a, 'ui, 'b, M: 'static> Drop for InputBuilder<'a, 'ui, 'b, M> {
    /// Stage the two-node stack: (1) editor leaf carrying the automation
    /// id + factories at the full element rect with the borderless
    /// `input_editor_chrome`; (2) overlay `box_` painted after, carrying
    /// border/radii-10/invalid/disabled-translucent. The stack container
    /// takes this builder's layout so element geometry is unchanged.
    fn drop(&mut self) {
        let kind = if self.multiline {
            crate::node::KIND_TEXT_AREA
        } else {
            crate::node::KIND_TEXT_INPUT
        };
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
            layout,
            visibility,
            selection,
            line_height,
            presented,
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
            self.layout,
            self.visibility,
            self.selection,
            self.line_height,
            self.presented,
        );
        // authored foreground still reaches the peer; chrome is the stack
        // chrome (author's `chrome` fields merge over the frozen inset),
        // flattened to a full patch so the recipe's border/radii are gone
        let mut chrome = crate::style::input_editor_chrome(multiline);
        chrome.patch(&patch.chrome);
        let mut epatch = patch;
        epatch.chrome = full_box_patch(&chrome);
        let auto = self.automation_id.clone();
        let mut factories = std::mem::take(&mut self.factories);
        self.tx.stage_container(
            crate::node::KIND_STACK,
            None,
            ContainerProps::default(),
            layout,
            EventFactorySet::default(),
            visibility,
            move |ui: &mut Ui<'_, '_, M>| {
                let before = ui.tx.frames.last().unwrap().children.len();
                ui.tx.stage_leaf(
                    kind,
                    None,
                    Visibility::Visible,
                    LayoutSpec::default(),
                    std::mem::take(&mut factories),
                    move |_| NodeData::Editor {
                        snapshot,
                        multiline,
                        read_only,
                        disabled,
                        submit,
                        max_lines,
                        placeholder,
                        accessible_label,
                        patch: epatch,
                        invalid,
                        selection,
                        line_height,
                        sync: Default::default(),
                    },
                );
                if ui.tx.frames.last().unwrap().children.len() > before
                    && let Some(i) = ui.tx.frames.last().unwrap().children.last().copied()
                {
                    if let Some(id) = auto {
                        ui.tx.nodes[i as usize].automation_id = Some(id);
                    }
                    ui.tx.nodes[i as usize].presented = presented;
                }
                ui.box_(
                    crate::style::BoxProps::new()
                        .style(crate::style::input_overlay_style(disabled, invalid)),
                    |_| {},
                );
            },
            false,
            |props| NodeData::Container {
                kind: crate::node::KIND_STACK,
                props,
            },
        );
    }
}

/// Flatten a resolved `BoxStyle` into a full `BoxStylePatch` — every
/// field Some/Set so a base recipe's values can never leak through the
/// merge (used by the input/textarea stack chrome).
fn full_box_patch(b: &crate::style::BoxStyle) -> crate::style::BoxStylePatch {
    let mut p = crate::style::BoxStylePatch::default();
    p.background = Some(b.background);
    for (dst, src) in [
        (&mut p.border.top, &b.border.top),
        (&mut p.border.right, &b.border.right),
        (&mut p.border.bottom, &b.border.bottom),
        (&mut p.border.left, &b.border.left),
    ] {
        dst.width = Some(src.width);
        dst.color = Some(src.color);
    }
    for (dst, src) in [
        (&mut p.radii.top_left, &b.radii.top_left),
        (&mut p.radii.top_right, &b.radii.top_right),
        (&mut p.radii.bottom_right, &b.radii.bottom_right),
        (&mut p.radii.bottom_left, &b.radii.bottom_left),
    ] {
        *dst = Some(*src);
    }
    for (dst, src) in [
        (&mut p.padding.top, &b.padding.top),
        (&mut p.padding.right, &b.padding.right),
        (&mut p.padding.bottom, &b.padding.bottom),
        (&mut p.padding.left, &b.padding.left),
    ] {
        *dst = Some(*src);
    }
    p.shadow = match &b.shadow {
        Some(s) => crate::style::ShadowPatch::Set(*s),
        None => crate::style::ShadowPatch::Remove,
    };
    p
}
