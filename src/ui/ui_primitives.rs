impl<'ui, 'tx, M: 'static> Ui<'ui, 'tx, M> {
    pub fn button<'a>(&'a mut self, text: &'a str) -> ButtonBuilder<'a, 'ui, M> {
        ButtonBuilder {
            automation_id: None,
            tx: self.tx,
            text,
            variant: ButtonVariant::default(),
            style: ButtonStylePatch::default(),
            disabled: false,
            size: ControlSize::default(),
            motion: Some(MotionToken::Hover),
            tooltip: None,
            icon_start: None,
            icon_end: None,
            icon_only: false,
            accessible_name: None,
            invalid: false,
            selected: false,
            expanded: None,
            icon_color: None,
            presented: Presented::default(),
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
            automation_id: None,
            tx: self.tx,
            value,
            multiline: false,
            placeholder: None,
            read_only: false,
            disabled: false,
            submit: SubmitPolicy::default(),
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

    pub fn text_area<'a, 'b>(
        &'a mut self,
        value: &'b TextValue,
    ) -> TextInputBuilder<'a, 'ui, 'b, M> {
        TextInputBuilder {
            automation_id: None,
            tx: self.tx,
            value,
            multiline: true,
            placeholder: None,
            read_only: false,
            disabled: false,
            submit: SubmitPolicy::None,
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

    pub fn custom(&mut self, render: Rc<dyn CustomRender>) -> CustomBuilder<'_, 'ui, M> {
        CustomBuilder {
            automation_id: None,
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
            automation_id: None,
            part_key: None,
            role: None,
            key,
            data: NodeData::Container {
                kind: crate::node::KIND_SCOPE,
                props: ContainerProps::default(),
            },
            presented: Presented::default(),
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
            Visibility::Visible,
            |ui: &mut Ui<'_, '_, M>| {
                for item in items {
                    let k = ErasedKey::new(key(item));
                    ui.tx.stage_container(
                        crate::node::KIND_GROUP,
                        Some(k),
                        ContainerProps::default(),
                        LayoutSpec::default(),
                        EventFactorySet::default(),
                        Visibility::Visible,
                        |u| draw(u, item),
                        false,
                        |props| NodeData::Container {
                            kind: crate::node::KIND_GROUP,
                            props,
                        },
                    );
                }
            },
            false,
            |props| NodeData::Container {
                kind: crate::node::KIND_GROUP,
                props,
            },
        );
    }

    /// `ui.box_` — noninteractive painted box container; `BoxProps` carries
    /// the authored full `BoxStyle` plus layout inputs (size stays layout).
    pub fn box_(&mut self, props: BoxProps, draw: impl FnOnce(&mut Ui<'_, '_, M>)) {
        let cp = ContainerProps {
            full: Some(props.style),
            ..Default::default()
        };
        self.tx.stage_container(
            crate::node::KIND_BOX,
            None,
            cp,
            props.layout,
            EventFactorySet::default(),
            props.visibility,
            draw,
            false,
            |props| NodeData::Container {
                kind: crate::node::KIND_BOX,
                props,
            },
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
            Visibility::Visible,
            draw,
            true,
            move |_| NodeData::Action {
                label: Rc::from(props.label.as_str()),
                style: props.style,
                disabled: props.disabled,
                checked: None,
            },
        );
        // the staged node is the last pushed (containers append last)
        let staged = (self.tx.nodes.len() > before).then_some(self.tx.nodes.len() as u32 - 1);
        ActionBuilder {
            tx: self.tx,
            staged,
            _m: std::marker::PhantomData,
        }
    }

    /// `ui.popover` — the generic anchored overlay layer. Stages a
    /// KIND_POPUP node inline ONLY when `open`; closed stages nothing.
    /// Placement is backend-resolved against the `anchor` node's laid-out
    /// rect each pass (flip-on-overflow + viewport clamp, shared policy in
    /// `crate::layout::resolve_popup_placement`). `dismiss_light`
    /// (default on) emits `on_open_change(false)` on an outside press;
    /// Escape dismisses the topmost open popup; `focus_on_open` (default
    /// on) moves focus to the first eligible descendant and restores the
    /// pre-open focus on close. `modal` paints a dim scrim and gates
    /// hit/focus/scroll to the popup subtree.
    ///
    /// Constraint: do NOT stage a `popover` inside an `ui.action`'s draw
    /// closure — `inside_action` covers the whole subtree and would make
    /// the popup inert (arena-live but unhittable). Popups anchor against
    /// sibling nodes by automation id instead.
    pub fn popover<'a>(
        &'a mut self,
        open: bool,
        draw: impl FnOnce(&mut Ui<'_, '_, M>),
    ) -> PopupBuilder<'a, 'ui, M> {
        let before = self.tx.nodes.len();
        if open {
            self.tx.stage_container(
                crate::node::KIND_POPUP,
                None,
                ContainerProps::default(),
                LayoutSpec::default(),
                EventFactorySet::default(),
                Visibility::Visible,
                draw,
                false,
                |props| NodeData::Popup {
                    anchor: None,
                    side: crate::geom::Side::default(),
                    align: Align::Center,
                    offset: Point::default(),
                    role: crate::node::PopupRole::default(),
                    modal: false,
                    dismiss_light: true,
                    focus_on_open: true,
                    props,
                },
            );
        }
        let staged = (self.tx.nodes.len() > before).then_some(self.tx.nodes.len() as u32 - 1);
        PopupBuilder {
            tx: self.tx,
            staged,
            _m: std::marker::PhantomData,
        }
    }

    /// CORE typography specimen — the frozen `typography.<kind>` recipe
    /// (style + chrome + wrap) applied to a display text leaf.
    /// CodeBlock is composite — use `ui.code_block`.
    pub fn typography<'a>(
        &'a mut self,
        kind: crate::typography::Typography,
        text: &'a str,
    ) -> TextBuilder<'a, 'ui, &'a str, M> {
        let mut b = self.text(text).style(kind.style()).wrap(kind.wraps());
        if let Some(c) = kind.chrome() {
            b = b.chrome(c);
        }
        // inline-code is an inline content-hugging box — it must not
        // stretch to its parent's width in a Stretch-aligned column
        if kind == crate::typography::Typography::InlineCode {
            b = b.width(crate::geom::Length::Content);
        }
        b
    }

    /// `typography.code-block` — mono code lines inside a `Surface`-role
    /// rounded container (`radius-2xl` 18dp). Semantic parts: `pre`
    /// marks the container itself (the frozen `pre` part rect is the
    /// element rect) and `pre.line[N]` the 1-based code lines. Attach
    /// the specimen's `automation_id` through `ui.named` — it lands on
    /// the outer container.
    pub fn code_block<'a>(&'a mut self, lines: &[&'a str]) {
        let chrome = crate::style::BoxStyle {
            background: crate::style::Color::Role(ColorRole::Code),
            border: crate::style::Border::default(),
            radii: crate::style::CornerRadii::all(crate::geom::Dp(18.0)),
            padding: crate::style::Insets {
                top: crate::geom::Dp(14.0),
                right: crate::geom::Dp(16.0),
                bottom: crate::geom::Dp(14.0),
                left: crate::geom::Dp(16.0),
            },
            shadow: None,
        };
        let mono = crate::style::TextStyle {
            font: crate::theme::FontFamily::Mono,
            size: crate::style::TextSize::Exact(crate::geom::Dp(14.0)),
            weight: crate::style::TextWeight::Normal,
            foreground: crate::style::Color::Role(ColorRole::CodeForeground),
            line_height: Some(crate::geom::Dp(20.0)),
            ..crate::style::TextStyle::default()
        };
        self.part("pre", |ui| {
            ui.box_(BoxProps::new().style(chrome), |ui| {
                // zero-pad/zero-gap — the chrome owns the frozen 14/16
                // insets; lines stack at their 20dp line-height
                ui.column(Column::new().padding(Space::None).gap(Space::None), |ui| {
                    for (i, line) in lines.iter().enumerate() {
                        let key = format!("line[{}]", i + 1);
                        ui.part(&key, |ui| {
                            ui.text(*line).style(mono);
                        });
                    }
                });
            });
        });
    }

    /// `ui.text` — styled text leaf; `label` is a recipe convenience over
    /// the same renderer.
    pub fn text<'a, T: AsRef<str>>(&'a mut self, text: T) -> TextBuilder<'a, 'ui, T, M> {
        TextBuilder {
            automation_id: None,
            tx: self.tx,
            text,
            wrap: false,
            style: crate::style::TextStyle::default(),
            chrome: None,
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
