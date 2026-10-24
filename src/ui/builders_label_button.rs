
/// Project a full recipe `TextStyle` into a `TextStylePatch` — the same
/// conversion `TextBuilder` applies when a primitive hands the label
/// pipeline a complete style (`TextSize::Body` stays unset so the label
/// recipe's base size survives).
fn text_to_patch(ts: &crate::style::TextStyle) -> crate::style::TextStylePatch {
    let mut p = crate::style::TextStylePatch::default();
    p.foreground = Some(ts.foreground);
    if let crate::style::TextSize::Exact(d) = ts.size {
        p.size = Some(d);
    }
    p.weight = Some(ts.weight);
    p.font = Some(ts.font);
    p.line_height = ts.line_height;
    p.underline = Some(ts.underline);
    p.letter_spacing = Some(ts.letter_spacing);
    p
}

// ============================ builders ======================================

/// Every builder stages on `Drop` — `.on_*(...)` setters register factories
/// retained at commit only.
pub struct LabelBuilder<'a, 'ui, T: AsRef<str>, M: 'static> {
    automation_id: Option<Rc<str>>,
    tx: &'a mut Tx<'ui>,
    text: T,
    wrap: bool,
    color_role: ColorRole,
    patch: crate::style::TextStylePatch,
    /// optional painted chrome under the text (rule lines, inline-code
    /// surface) — a full authored `BoxStyle`, no recipe merge
    chrome: Option<crate::style::BoxStyle>,
    visibility: Visibility,
    layout: LayoutSpec,
    factories: EventFactorySet,
    _m: std::marker::PhantomData<fn() -> M>,
}

impl<'a, 'ui, T: AsRef<str>, M: 'static> LabelBuilder<'a, 'ui, T, M> {
    pub fn automation_id(mut self, id: &str) -> Self {
        self.automation_id = Some(Rc::from(id));
        self
    }

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
    /// painted chrome under the text — the h2 rule line / inline-code
    /// surface. Authored full style; insets the text measure.
    pub fn chrome(mut self, c: crate::style::BoxStyle) -> Self {
        self.chrome = Some(c);
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
        let before = self.tx.frames.last().unwrap().children.len();
        let text = self.text.as_ref();
        let wrap = self.wrap;
        let role = self.color_role;
        let chrome = self.chrome;
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
                    chrome,
                }
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

pub struct ButtonBuilder<'a, 'ui, M: 'static> {
    automation_id: Option<Rc<str>>,
    tx: &'a mut Tx<'ui>,
    text: &'a str,
    variant: ButtonVariant,
    style: ButtonStylePatch,
    disabled: bool,
    size: ControlSize,
    motion: Option<MotionToken>,
    tooltip: Option<Rc<str>>,
    icon_start: Option<crate::icons::Icon>,
    icon_end: Option<crate::icons::Icon>,
    icon_only: bool,
    accessible_name: Option<Rc<str>>,
    invalid: bool,
    selected: bool,
    expanded: Option<bool>,
    icon_color: Option<crate::style::Color>,
    presented: Presented,
    visibility: Visibility,
    layout: LayoutSpec,
    factories: EventFactorySet,
    _m: std::marker::PhantomData<fn() -> M>,
}

impl<'a, 'ui, M: 'static> ButtonBuilder<'a, 'ui, M> {
    pub fn automation_id(mut self, id: &str) -> Self {
        self.automation_id = Some(Rc::from(id));
        self
    }

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
    /// decorative Lucide icon at the start edge (inline-start) — inert,
    /// styled by the button recipe's foreground
    pub fn icon_start(mut self, icon: crate::icons::Icon) -> Self {
        self.icon_start = Some(icon);
        self
    }
    /// decorative Lucide icon at the end edge (inline-end)
    pub fn icon_end(mut self, icon: crate::icons::Icon) -> Self {
        self.icon_end = Some(icon);
        self
    }
    /// icon-only control — `ui.icon_button` sets this; square
    /// size-class measure, no visible text
    pub(crate) fn icon_only(mut self, on: bool) -> Self {
        self.icon_only = on;
        self
    }
    /// accessible name used when the visible text isn't the control
    /// name — required on icon-only buttons (the button name, not the
    /// icon file)
    pub fn accessible_name(mut self, name: &str) -> Self {
        self.accessible_name = Some(Rc::from(name));
        self
    }
    /// semantic `aria-invalid` — destructive chrome; orthogonal to
    /// disabled/hover/pressed/focus
    pub fn invalid(mut self, on: bool) -> Self {
        self.invalid = on;
        self
    }
    /// semantic `data-active`/`aria-selected` — the recipe's selected
    /// overlay (variant-dependent); `focus_on_open` popups prefer a
    /// `selected` descendant for the open focus
    pub fn selected(mut self, on: bool) -> Self {
        self.selected = on;
        self
    }
    /// expanded/collapsed semantics for a popup-owning trigger
    pub fn expanded(mut self, expanded: Option<bool>) -> Self {
        self.expanded = expanded;
        self
    }
    /// icon tint override — `None` paints icons in the resolved
    /// foreground (`currentColor`); the select chevron keeps
    /// `muted-foreground` against a filled value
    pub fn icon_color(mut self, c: Option<crate::style::Color>) -> Self {
        self.icon_color = c;
        self
    }
    /// presentation-only staged visual state — paints/lays out exactly
    /// like the runtime state it mirrors; never grants real focus,
    /// hover, or interactivity
    pub fn presented(mut self, p: Presented) -> Self {
        self.presented = p;
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
        let before = self.tx.frames.last().unwrap().children.len();
        let text = self.text;
        let (
            variant,
            style,
            disabled,
            size,
            motion,
            tooltip,
            icon_start,
            icon_end,
            icon_only,
            accessible_name,
            invalid,
            selected,
            expanded,
            icon_color,
        ) = (
            self.variant,
            self.style,
            self.disabled,
            self.size,
            self.motion,
            self.tooltip.clone(),
            self.icon_start,
            self.icon_end,
            self.icon_only,
            self.accessible_name.clone(),
            self.invalid,
            self.selected,
            self.expanded,
            self.icon_color,
        );
        let presented = self.presented;
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
                    icon_start,
                    icon_end,
                    icon_only,
                    accessible_name,
                    invalid,
                    selected,
                    expanded,
                    icon_color,
                }
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