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
    pub fn automation_id(self, id: &str) -> Self {
        if let Some(i) = self.staged {
            self.tx.nodes[i as usize].automation_id = Some(Rc::from(id));
        }
        self
    }
    /// presentation-only staged visual state (`Presented` docs)
    pub fn presented(self, p: Presented) -> Self {
        if let Some(i) = self.staged {
            self.tx.nodes[i as usize].presented = p;
        }
        self
    }
    pub fn semantic_role(self, role: SemanticRole) -> Self {
        if let Some(i) = self.staged {
            self.tx.nodes[i as usize].role = Some(role);
        }
        self
    }
    pub fn semantic_checked(self, checked: Option<bool>) -> Self {
        if let Some(i) = self.staged
            && let NodeData::Action {
                checked: staged, ..
            } = &mut self.tx.nodes[i as usize].data
        {
            *staged = checked;
        }
        self
    }
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

/// `ui.popover` builder — the popup node stages at `popover()` (children
/// draw inside it); setters retrofit onto the staged node. When `open` is
/// false nothing staged and every setter is a no-op.
pub struct PopupBuilder<'a, 'ui, M: 'static> {
    tx: &'a mut Tx<'ui>,
    /// staged node index in `tx.nodes` — None when closed or rejected
    staged: Option<u32>,
    _m: std::marker::PhantomData<fn() -> M>,
}

impl<'a, 'ui, M: 'static> PopupBuilder<'a, 'ui, M> {
    /// Mutate the staged Popup payload — no-op when nothing staged.
    fn edit(&mut self, f: impl FnOnce(&mut PopupFields<'_>)) {
        if let Some(i) = self.staged
            && let StagedNode {
                data:
                    NodeData::Popup {
                        anchor,
                        side,
                        align,
                        offset,
                        role,
                        modal,
                        dismiss_light,
                        focus_on_open,
                        props,
                    },
                ..
            } = &mut self.tx.nodes[i as usize]
        {
            f(&mut PopupFields {
                anchor,
                side,
                align,
                offset,
                role,
                modal,
                dismiss_light,
                focus_on_open,
                props,
            });
        }
    }

    /// automation id of the node the popup anchors to (e.g. the trigger)
    pub fn anchor(mut self, automation_id: &str) -> Self {
        self.edit(|p| *p.anchor = Some(Rc::from(automation_id)));
        self
    }
    /// anchor edge the popup opens from (default Bottom)
    pub fn side(mut self, s: crate::geom::Side) -> Self {
        self.edit(|p| *p.side = s);
        self
    }
    /// cross-axis alignment against the anchor (default Center)
    pub fn align(mut self, a: Align) -> Self {
        self.edit(|p| *p.align = a);
        self
    }
    /// gap along the side axis in Dp (Radix `sideOffset`)
    pub fn side_offset(mut self, d: crate::geom::Dp) -> Self {
        self.edit(|p| match *p.side {
            crate::geom::Side::Top | crate::geom::Side::Bottom => p.offset.y = d.0,
            crate::geom::Side::Left | crate::geom::Side::Right => p.offset.x = d.0,
        });
        self
    }
    /// shift along the align axis in Dp (Radix `alignOffset`)
    pub fn align_offset(mut self, d: crate::geom::Dp) -> Self {
        self.edit(|p| match *p.side {
            crate::geom::Side::Top | crate::geom::Side::Bottom => p.offset.x = d.0,
            crate::geom::Side::Left | crate::geom::Side::Right => p.offset.y = d.0,
        });
        self
    }
    /// semantic overlay role — drives the UIA control type
    pub fn role(mut self, r: crate::node::PopupRole) -> Self {
        self.edit(|p| *p.role = r);
        self
    }
    /// modal overlay — scrim + hit/focus/scroll gate to the subtree.
    /// `modal(true)` also clears `dismiss_light` (a modal surface never
    /// light-dismisses); re-enable it afterwards only for deliberate
    /// hybrid behavior.
    pub fn modal(mut self, on: bool) -> Self {
        self.edit(|p| {
            *p.modal = on;
            if on {
                *p.dismiss_light = false;
            }
        });
        self
    }
    /// outside-press dismisses via `on_open_change(false)` (default on)
    pub fn dismiss_light(mut self, on: bool) -> Self {
        self.edit(|p| *p.dismiss_light = on);
        self
    }
    /// move focus to the first eligible descendant on open, restore on
    /// close (default on)
    pub fn focus_on_open(mut self, on: bool) -> Self {
        self.edit(|p| *p.focus_on_open = on);
        self
    }
    pub fn automation_id(self, id: &str) -> Self {
        if let Some(i) = self.staged {
            self.tx.nodes[i as usize].automation_id = Some(Rc::from(id));
        }
        self
    }
    /// surgical chrome patch over the popup's surface recipe — later
    /// `.style` calls deep-merge, `None` never erases
    pub fn style(mut self, p: crate::style::BoxStylePatch) -> Self {
        let mut patch = self
            .staged
            .and_then(|i| match &self.tx.nodes[i as usize].data {
                NodeData::Popup { props, .. } => Some(props.patch),
                _ => None,
            })
            .unwrap_or_default();
        patch.merge(&p);
        self.edit(|f| f.props.patch = patch);
        self
    }
    /// open-state events — `false` on outside-press/Escape dismissals
    pub fn on_open_change(self, f: impl Fn(bool) -> M + 'static) -> Self {
        if let Some(i) = self.staged {
            self.tx.nodes[i as usize].factories.on_open_change =
                Some(Box::new(move |o| Box::new(f(o))));
        }
        self
    }
    /// popup width — the natural measure is children-driven; a select
    /// content pins the frozen 208dp lane
    pub fn width(self, w: crate::geom::Length) -> Self {
        if let Some(i) = self.staged {
            self.tx.nodes[i as usize].layout.width = w;
        }
        self
    }
    /// popup height override — natural (content-measured) by default
    pub fn height(self, h: crate::geom::Length) -> Self {
        if let Some(i) = self.staged {
            self.tx.nodes[i as usize].layout.height = h;
        }
        self
    }
}

/// Borrowed field views for `PopupBuilder::edit` — keeps the destructure
/// out of every setter.
struct PopupFields<'a> {
    anchor: &'a mut Option<Rc<str>>,
    side: &'a mut crate::geom::Side,
    align: &'a mut Align,
    offset: &'a mut Point,
    role: &'a mut crate::node::PopupRole,
    modal: &'a mut bool,
    dismiss_light: &'a mut bool,
    focus_on_open: &'a mut bool,
    props: &'a mut ContainerProps,
}

/// `ui.text` — styled text leaf with a full authored `TextStyle`.
pub struct TextBuilder<'a, 'ui, T: AsRef<str>, M: 'static> {
    automation_id: Option<Rc<str>>,
    tx: &'a mut Tx<'ui>,
    text: T,
    wrap: bool,
    style: crate::style::TextStyle,
    /// painted chrome under the text (rule lines, code surfaces)
    chrome: Option<crate::style::BoxStyle>,
    visibility: Visibility,
    layout: LayoutSpec,
    _m: std::marker::PhantomData<fn() -> M>,
}

impl<'a, 'ui, T: AsRef<str>, M: 'static> TextBuilder<'a, 'ui, T, M> {
    pub fn automation_id(mut self, id: &str) -> Self {
        self.automation_id = Some(Rc::from(id));
        self
    }

    /// full authored style — primitives want complete styles, not patches
    pub fn style(mut self, s: crate::style::TextStyle) -> Self {
        self.style = s;
        self
    }
    /// painted chrome under the text — rule lines / code surfaces
    pub fn chrome(mut self, c: crate::style::BoxStyle) -> Self {
        self.chrome = Some(c);
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
        let before = self.tx.frames.last().unwrap().children.len();
        // authored full style lands as a patch over the label recipe — the
        // staged label shape is shared with `ui.label`
        let mut patch = crate::style::TextStylePatch::default();
        patch.foreground = Some(self.style.foreground);
        if let crate::style::TextSize::Exact(d) = self.style.size {
            patch.size = Some(d);
        }
        patch.weight = Some(self.style.weight);
        patch.font = Some(self.style.font);
        patch.line_height = self.style.line_height;
        patch.underline = Some(self.style.underline);
        patch.letter_spacing = Some(self.style.letter_spacing);
        let text = self.text.as_ref();
        let wrap = self.wrap;
        let chrome = self.chrome;
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

/// `ui.icon` — decorative vector icon leaf (Lucide stroke geometry).
/// Carries no interaction; `size`/`color` are authored directly.
pub struct IconBuilder<'a, 'ui, M: 'static> {
    automation_id: Option<Rc<str>>,
    tx: &'a mut Tx<'ui>,
    icon: crate::icons::Icon,
    size: crate::geom::Dp,
    color: crate::style::Color,
    visibility: Visibility,
    layout: LayoutSpec,
    _m: std::marker::PhantomData<fn() -> M>,
}

impl<'a, 'ui, M: 'static> IconBuilder<'a, 'ui, M> {
    pub fn automation_id(mut self, id: &str) -> Self {
        self.automation_id = Some(Rc::from(id));
        self
    }
    /// icon edge in dp — the viewBox scales `size / 24`; the frozen
    /// specimen size is 16 (`size-4`)
    pub fn size(mut self, s: crate::geom::Dp) -> Self {
        self.size = s;
        self
    }
    /// stroke color — the `currentColor` analog; recipes/authors set it
    pub fn color(mut self, c: crate::style::Color) -> Self {
        self.color = c;
        self
    }
    /// role shorthand — `Icon` strokes resolve through the theme palette
    pub fn color_role(mut self, r: ColorRole) -> Self {
        self.color = crate::style::Color::Role(r);
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

impl<'a, 'ui, M: 'static> Drop for IconBuilder<'a, 'ui, M> {
    fn drop(&mut self) {
        let before = self.tx.frames.last().unwrap().children.len();
        let (icon, size, color) = (self.icon, self.size, self.color);
        self.tx.stage_leaf(
            crate::node::KIND_ICON,
            None,
            self.visibility,
            self.layout,
            EventFactorySet::default(),
            move |_| NodeData::Icon { icon, size, color },
        );
        if let Some(id) = self.automation_id.take()
            && self.tx.frames.last().unwrap().children.len() > before
            && let Some(index) = self.tx.frames.last().unwrap().children.last().copied()
        {
            self.tx.nodes[index as usize].automation_id = Some(id);
        }
    }
}

// ------------------------- forms APIs ---------------------------------------

