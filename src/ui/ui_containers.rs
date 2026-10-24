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
            Visibility::Visible,
            draw,
            false,
            |props| NodeData::Container {
                kind: crate::node::KIND_GROUP,
                props,
            },
        );
    }

    /// Assign a stable semantic ID to exactly one child produced by `draw`.
    /// This metadata is independent of retained/keyed identity.
    pub fn named(&mut self, id: &str, draw: impl FnOnce(&mut Ui<'_, '_, M>)) {
        let before = self.tx.frames.last().unwrap().children.len();
        draw(self);
        let children = &self.tx.frames.last().unwrap().children;
        if children.len() != before + 1 {
            self.tx.diagnostics.push(UiDiagnostic::InvalidComposition);
            return;
        }
        let index = children[before];
        let existing = self.tx.nodes[index as usize].automation_id.clone();
        match existing.as_deref() {
            // An id produced inside `draw` (e.g. a builder's `.automation_id`)
            // stays authoritative: identical is idempotent, a different id is
            // a staged conflict — diagnostic, never an overwrite.
            Some(existing) if existing == id => {}
            Some(_) => self
                .tx
                .diagnostics
                .push(UiDiagnostic::DuplicateAutomationId),
            None => self.tx.nodes[index as usize].automation_id = Some(Rc::from(id)),
        }
    }

    /// Mark exactly one child produced by `draw` as a semantic PART of the
    /// enclosing automation-id owner (`owner#part`). Part keys are scoped
    /// to the owner and chain with enclosing parts — `part("header")` +
    /// `part("title")` inside emits `header.title`. Generic container
    /// chrome only: the part marker never changes layout, painting, or
    /// semantics beyond snapshot identity.
    pub fn part(&mut self, key: &str, draw: impl FnOnce(&mut Ui<'_, '_, M>)) {
        let before = self.tx.frames.last().unwrap().children.len();
        draw(self);
        let children = &self.tx.frames.last().unwrap().children;
        if children.len() != before + 1 {
            self.tx.diagnostics.push(UiDiagnostic::InvalidComposition);
            return;
        }
        let index = children[before];
        let node = &mut self.tx.nodes[index as usize];
        if node.part_key.is_some() {
            self.tx.diagnostics.push(UiDiagnostic::InvalidComposition);
            return;
        }
        node.part_key = Some(Rc::from(key));
    }

    pub fn column(&mut self, props: Column, draw: impl FnOnce(&mut Ui<'_, '_, M>)) {
        self.tx.stage_container(
            crate::node::KIND_COLUMN,
            None,
            props.props(),
            LayoutSpec {
                width: props.width,
                height: props.height,
                ..Default::default()
            },
            EventFactorySet::default(),
            Visibility::Visible,
            draw,
            false,
            |props| NodeData::Container {
                kind: crate::node::KIND_COLUMN,
                props,
            },
        );
    }
    pub fn row(&mut self, props: Row, draw: impl FnOnce(&mut Ui<'_, '_, M>)) {
        self.tx.stage_container(
            crate::node::KIND_ROW,
            None,
            props.props(),
            LayoutSpec {
                width: props.width,
                height: props.height,
                ..Default::default()
            },
            EventFactorySet::default(),
            Visibility::Visible,
            draw,
            false,
            |props| NodeData::Container {
                kind: crate::node::KIND_ROW,
                props,
            },
        );
    }
    pub fn stack(&mut self, props: Stack, draw: impl FnOnce(&mut Ui<'_, '_, M>)) {
        self.tx.stage_container(
            crate::node::KIND_STACK,
            None,
            props.props(),
            LayoutSpec::default(),
            EventFactorySet::default(),
            Visibility::Visible,
            draw,
            false,
            |props| NodeData::Container {
                kind: crate::node::KIND_STACK,
                props,
            },
        );
    }
    pub fn surface(&mut self, props: Surface, draw: impl FnOnce(&mut Ui<'_, '_, M>)) {
        self.tx.stage_container(
            crate::node::KIND_SURFACE,
            None,
            props.props(),
            LayoutSpec::default(),
            EventFactorySet::default(),
            Visibility::Visible,
            draw,
            false,
            |props| NodeData::Container {
                kind: crate::node::KIND_SURFACE,
                props,
            },
        );
    }

    /// `ui.scroll` — vertical scroll viewport. `offset` is the app's
    /// committed scroll position (Dp); wheel and devctl scroll intents
    /// arrive as `ScrollOffset` deltas through `on_scroll`, producing `M`
    /// so `update` can own the state. The viewport fills the remaining
    /// main-axis space it is offered (`height: Fill(1)`); children stack
    /// as a column at unbounded height and clip to the viewport inner.
    pub fn scroll(
        &mut self,
        props: Scroll,
        offset: crate::geom::Dp,
        on_scroll: impl Fn(crate::event::ScrollOffset) -> M + 'static,
        draw: impl FnOnce(&mut Ui<'_, '_, M>),
    ) {
        let mut factories = EventFactorySet::default();
        factories.on_scroll = Some(Box::new(move |o| Box::new(on_scroll(o))));
        // the explicit argument is authoritative over any staged field
        let props = Scroll { offset, ..props };
        self.tx.stage_container(
            crate::node::KIND_SCROLL,
            None,
            props.props(),
            LayoutSpec {
                height: crate::geom::Length::Fill(1),
                ..Default::default()
            },
            factories,
            Visibility::Visible,
            draw,
            false,
            |props| NodeData::Container {
                kind: crate::node::KIND_SCROLL,
                props,
            },
        );
    }

    /// Card container — `bg-card`, `rounded-xl`, `ring-1
    /// ring-foreground/10`, top padding = `--card-spacing`. Column
    /// semantics: children are the card's parts (header/title/
    /// description/content/footer), spaced by the card-spacing token.
    /// Compose parts with generic `column`/`row`/`box_`/`part` — the
    /// card itself stays a plain styled container.
    pub fn card(&mut self, props: Card, draw: impl FnOnce(&mut Ui<'_, '_, M>)) {
        let layout = props
            .width
            .map_or_else(LayoutSpec::default, |w| LayoutSpec {
                width: crate::geom::Length::Fixed(w),
                ..Default::default()
            });
        self.tx.stage_container(
            crate::node::KIND_CARD,
            None,
            props.props(),
            layout,
            EventFactorySet::default(),
            Visibility::Visible,
            draw,
            false,
            |props| NodeData::Container {
                kind: crate::node::KIND_CARD,
                props,
            },
        );
    }

    /// `ui.separator` — the frozen `bg-border` rule: a decorative 1dp
    /// line in the `Border` role. `Horizontal` fills the offered width
    /// (stretch/fill), `Vertical` the offered height — use
    /// `Separator::length` or a stretch-aligned parent where no extent
    /// is offered.
    pub fn separator(&mut self, props: Separator) {
        self.tx.stage_container(
            crate::node::KIND_BOX,
            None,
            ContainerProps {
                full: Some(crate::style::separator_recipe()),
                ..Default::default()
            },
            props.layout(),
            EventFactorySet::default(),
            Visibility::Visible,
            |_: &mut Ui<'_, '_, M>| {},
            false,
            |props| NodeData::Container {
                kind: crate::node::KIND_BOX,
                props,
            },
        );
    }

    /// `ui.alert` — the frozen `cn-alert` container: card chrome,
    /// `rounded-lg` (10dp), 1dp border, padding-block 8 / inline 10.
    /// Compose the `icon`/`title`/`description` parts inside with
    /// `ui.part` + the `alert_*` style recipes.
    pub fn alert(&mut self, props: Alert, draw: impl FnOnce(&mut Ui<'_, '_, M>)) {
        let mut full = crate::style::alert_recipe(props.variant);
        full.patch(&props.patch);
        let layout = props
            .width
            .map_or_else(LayoutSpec::default, |w| LayoutSpec {
                width: Length::Fixed(w),
                ..Default::default()
            });
        self.tx.stage_container(
            crate::node::KIND_BOX,
            None,
            ContainerProps {
                full: Some(full),
                ..Default::default()
            },
            layout,
            EventFactorySet::default(),
            Visibility::Visible,
            draw,
            false,
            |props| NodeData::Container {
                kind: crate::node::KIND_BOX,
                props,
            },
        );
    }

    /// `ui.skeleton` — a static `bg-muted` placeholder block at an
    /// authored size (radius-md; `circle()` selects `rounded-full`).
    /// Deliberately no animation or timers — the frozen contract is
    /// static (reduced-motion product scope).
    pub fn skeleton(&mut self, props: Skeleton) {
        self.tx.stage_container(
            crate::node::KIND_BOX,
            None,
            ContainerProps {
                full: Some(crate::style::skeleton_recipe(props.circle)),
                ..Default::default()
            },
            LayoutSpec {
                width: props.width,
                height: props.height,
                ..Default::default()
            },
            EventFactorySet::default(),
            Visibility::Visible,
            |_: &mut Ui<'_, '_, M>| {},
            false,
            |props| NodeData::Container {
                kind: crate::node::KIND_BOX,
                props,
            },
        );
    }

    /// `ui.kbd_group` — the frozen `cn-kbd-group` inline row (gap-1,
    /// items-center). Children are `ui.kbd` caps marked as
    /// `ui.part("key[..]", ..)` parts of the group's automation owner.
    pub fn kbd_group(&mut self, draw: impl FnOnce(&mut Ui<'_, '_, M>)) {
        self.row(
            Row::new()
                .gap(Space::Xs)
                .align(Align::Center)
                .padding(Space::None),
            draw,
        );
    }

    // ----- leaves -----------------------------------------------------------

    /// Decorative vector icon leaf — Lucide stroke geometry at `size`.
    /// Inert (no interaction); an icon inside a button contributes the
    /// `icon` part, not a separate semantic node.
    pub fn icon(&mut self, icon: crate::icons::Icon) -> IconBuilder<'_, 'ui, M> {
        IconBuilder {
            automation_id: None,
            tx: self.tx,
            icon,
            size: crate::icons::Icon::DEFAULT_SIZE,
            color: crate::style::Color::Role(ColorRole::Foreground),
            visibility: Visibility::Visible,
            layout: LayoutSpec::default(),
            _m: std::marker::PhantomData,
        }
    }

    /// Icon-only button — one `Button` node with a decorative central
    /// icon, square size-class measure (`icon-xs/sm/icon/icon-lg`),
    /// and a REQUIRED accessible name (no visible text). `text` is the
    /// button's name; it is never painted.
    pub fn icon_button<'a>(
        &'a mut self,
        icon: crate::icons::Icon,
        text: &'a str,
    ) -> ButtonBuilder<'a, 'ui, M> {
        self.button(text)
            .icon_start(icon)
            .icon_only(true)
            .accessible_name(text)
    }

    /// `impl AsRef<str>` — an owned `String`/`format!` value or a `&str`
    /// both stage directly; the retained `Rc<str>` is reused when the text
    /// is unchanged, so borrowed strings cost no copy.
    pub fn label<'a, T: AsRef<str>>(&'a mut self, text: T) -> LabelBuilder<'a, 'ui, T, M> {
        LabelBuilder {
            automation_id: None,
            tx: self.tx,
            text,
            wrap: false,
            color_role: ColorRole::Foreground,
            patch: crate::style::TextStylePatch::default(),
            chrome: None,
            visibility: Visibility::Visible,
            layout: LayoutSpec::default(),
            factories: EventFactorySet::default(),
            _m: std::marker::PhantomData,
        }
    }

    /// `ui.badge` — a content-hugging `cn-badge` pill: the variant
    /// recipe's chrome + `text-xs` medium label. Returns the underlying
    /// `LabelBuilder` so automation ids and surgical text patches
    /// compose exactly as on `ui.label`. Not interactive — no press or
    /// hover states beyond the frozen `normal` capture.
    pub fn badge<'a, T: AsRef<str>>(
        &'a mut self,
        variant: BadgeVariant,
        text: T,
    ) -> LabelBuilder<'a, 'ui, T, M> {
        let vs = crate::style::badge_recipe(variant);
        self.label(text)
            .chrome(vs.box_style)
            .style(text_to_patch(&vs.text_style))
    }

    /// `ui.kbd` — a single `cn-kbd` key cap: `bg-muted` radius-sm box,
    /// `text-xs` medium `muted-foreground`, 20dp minimum width. Content-
    /// hugging — group caps with `ui.kbd_group` + `ui.part("key[..]",..)`.
    pub fn kbd<'a, T: AsRef<str>>(&'a mut self, text: T) -> LabelBuilder<'a, 'ui, T, M> {
        let vs = crate::style::kbd_recipe();
        self.label(text)
            .chrome(vs.box_style)
            .style(text_to_patch(&vs.text_style))
            .min_width(crate::geom::Dp(20.0))
    }

}
