/// The capability-typed patch surface for `button`/`icon_button`.
#[derive(Copy, Clone, Default, PartialEq, Debug)]

pub struct ButtonStylePatch {
    pub styles: StateStyles<VisualStylePatch>,
}

impl ButtonStylePatch {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn background(mut self, color: Color) -> Self {
        self.styles.base.box_style.background = Some(color);
        self
    }
    pub fn foreground(mut self, color: Color) -> Self {
        self.styles.base.text_style.foreground = Some(color);
        self
    }
    pub fn border_bottom_width(mut self, width: Dp) -> Self {
        self.styles.base.box_style.border.bottom.width = Some(width);
        self
    }
    pub fn border_bottom_color(mut self, color: Color) -> Self {
        self.styles.base.box_style.border.bottom.color = Some(color);
        self
    }
    pub fn hover(mut self, patch: VisualStylePatch) -> Self {
        match &mut self.styles.hover {
            Some(h) => h.merge(&patch),
            None => self.styles.hover = Some(patch),
        }
        self
    }
    pub fn pressed(mut self, patch: VisualStylePatch) -> Self {
        match &mut self.styles.pressed {
            Some(h) => h.merge(&patch),
            None => self.styles.pressed = Some(patch),
        }
        self
    }
    pub fn disabled(mut self, patch: VisualStylePatch) -> Self {
        match &mut self.styles.disabled {
            Some(h) => h.merge(&patch),
            None => self.styles.disabled = Some(patch),
        }
        self
    }
    pub fn focus_visible(mut self, patch: VisualStylePatch) -> Self {
        match &mut self.styles.focus_visible {
            Some(h) => h.merge(&patch),
            None => self.styles.focus_visible = Some(patch),
        }
        self
    }
    /// merge — a later `.style(..)` on the same node build deep-merges
    /// fieldwise; `None` never erases an earlier Some in the same build
    pub fn merge(&mut self, o: &ButtonStylePatch) {
        self.styles.base.merge(&o.styles.base);
        macro_rules! slot {
            ($s:ident) => {{
                if let Some(o) = &o.styles.$s {
                    match &mut self.styles.$s {
                        Some(m) => m.merge(o),
                        None => self.styles.$s = Some(*o),
                    }
                }
            }};
        }
        slot!(hover);
        slot!(pressed);
        slot!(disabled);
        slot!(focus_visible);
    }
}

/// `ui.action` — semantic activation wrapper: a full authored `BoxStyle`
/// base plus optional state patches (children own their own styles).
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct ActionStyle {
    pub base: BoxStyle,
    pub hover: Option<BoxStylePatch>,
    pub pressed: Option<BoxStylePatch>,
    pub disabled: Option<BoxStylePatch>,
    pub focus_visible: Option<BoxStylePatch>,
}

impl ActionStyle {
    pub fn new(base: BoxStyle) -> Self {
        ActionStyle {
            base,
            hover: None,
            pressed: None,
            disabled: None,
            focus_visible: None,
        }
    }
    /// repeated state setters MERGE fieldwise — `Some` fields overwrite
    /// and `None` preserves an earlier call, matching the patch contract
    /// (`docs/STYLE_CUSTOMIZATION_MODEL.md` §6)
    pub fn hover(mut self, patch: BoxStylePatch) -> Self {
        match &mut self.hover {
            Some(h) => h.merge(&patch),
            None => self.hover = Some(patch),
        }
        self
    }
    pub fn pressed(mut self, patch: BoxStylePatch) -> Self {
        match &mut self.pressed {
            Some(h) => h.merge(&patch),
            None => self.pressed = Some(patch),
        }
        self
    }
    pub fn disabled(mut self, patch: BoxStylePatch) -> Self {
        match &mut self.disabled {
            Some(h) => h.merge(&patch),
            None => self.disabled = Some(patch),
        }
        self
    }
    pub fn focus_visible(mut self, patch: BoxStylePatch) -> Self {
        match &mut self.focus_visible {
            Some(h) => h.merge(&patch),
            None => self.focus_visible = Some(patch),
        }
        self
    }
    /// Any state branch carry metrics? Transition classification consults
    /// this — a metric-bearing branch needs layout on enter/leave.
    pub fn any_metric_branch(&self) -> bool {
        self.hover
            .as_ref()
            .is_some_and(BoxStylePatch::metric_affecting)
            || self
                .pressed
                .as_ref()
                .is_some_and(BoxStylePatch::metric_affecting)
            || self
                .disabled
                .as_ref()
                .is_some_and(BoxStylePatch::metric_affecting)
            || self
                .focus_visible
                .as_ref()
                .is_some_and(BoxStylePatch::metric_affecting)
    }

    /// Resolve the effective `BoxStyle` for the given interaction state:
    /// exclusive disabled > pressed > hover > normal, then focus_visible.
    pub fn resolve(
        &self,
        disabled: bool,
        pressed: bool,
        hover: bool,
        focus_visible: bool,
    ) -> BoxStyle {
        let mut b = self.base;
        if disabled {
            if let Some(p) = &self.disabled {
                b.patch(p);
            }
        } else if pressed {
            if let Some(p) = &self.pressed {
                b.patch(p);
            }
        } else if hover {
            if let Some(p) = &self.hover {
                b.patch(p);
            }
        }
        if focus_visible
            && !disabled
            && let Some(p) = &self.focus_visible
        {
            b.patch(p);
        }
        b
    }
}

/// `ui.action` props — accessible name mandatory, style + disabled flag.
#[derive(Clone, PartialEq)]
pub struct Action {
    pub label: String,
    pub style: ActionStyle,
    pub disabled: bool,
}

impl Action {
    pub fn new() -> Self {
        Action {
            label: String::new(),
            style: ActionStyle::new(BoxStyle::new()),
            disabled: false,
        }
    }
    pub fn label(mut self, name: &str) -> Self {
        self.label = name.to_string();
        self
    }
    pub fn style(mut self, style: ActionStyle) -> Self {
        self.style = style;
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

