impl<'ui, 'tx, M: 'static> Ui<'ui, 'tx, M> {
    /// Stage a Toggle leaf. Automation ids come from the enclosing
    /// `ui.named`/`ui.part` wrapper — the leaf stages plain.
    fn stage_toggle(
        &mut self,
        kind: crate::theme::ToggleKind,
        size: crate::theme::ToggleSize,
        checked: bool,
        disabled: bool,
        invalid: bool,
        label: Option<&'static str>,
        accessible_name: Option<&'static str>,
        presented: Presented,
        layout: LayoutSpec,
        factories: EventFactorySet,
    ) {
        self.tx.stage_leaf(
            crate::node::KIND_TOGGLE,
            None,
            Visibility::Visible,
            layout,
            factories,
            move |_| NodeData::Toggle {
                kind,
                checked,
                disabled,
                invalid,
                size,
                label: label.map(Rc::from),
                accessible_name: accessible_name.map(Rc::from),
            },
        );
        if presented.any()
            && let Some(index) = self.tx.frames.last().unwrap().children.last().copied()
        {
            self.tx.nodes[index as usize].presented = presented;
        }
    }

    /// `ui.checkbox` — 16×16 selection box; app model owns `checked`,
    /// `on_change` fires on the shared Press path. Stage inside
    /// `ui.named`/`ui.part` for automation ids (like `ui.icon`).
    pub fn checkbox(&mut self, props: Checkbox, on_change: impl Fn() -> M + 'static) {
        let mut fac = EventFactorySet::default();
        fac.on_press = Some(Box::new(move || Box::new(on_change()) as Box<dyn Any>));
        self.stage_toggle(
            crate::theme::ToggleKind::Checkbox,
            crate::theme::ToggleSize::Default,
            props.checked,
            props.disabled,
            props.invalid,
            props.label,
            props.accessible_name,
            props.presented,
            LayoutSpec::default(),
            fac,
        );
    }

    /// `ui.switch` — pill track + thumb; `props.size` picks the frozen
    /// default or `size-sm` track.
    pub fn switch(&mut self, props: Switch, on_change: impl Fn() -> M + 'static) {
        let mut fac = EventFactorySet::default();
        fac.on_press = Some(Box::new(move || Box::new(on_change()) as Box<dyn Any>));
        self.stage_toggle(
            crate::theme::ToggleKind::Switch,
            props.size,
            props.checked,
            props.disabled,
            props.invalid,
            props.label,
            props.accessible_name,
            props.presented,
            LayoutSpec::default(),
            fac,
        );
    }

    /// `ui.radio_group` — the group's `selected` string owns the checked
    /// item; each `item` is a `part("item[key]")` 16×16 RadioItem toggle
    /// whose Press fires `on_select(key)`. Stage as a column (frozen 8dp
    /// item gap) inside the caller's `ui.named`/`part` scope.
    pub fn radio_group<F>(
        &mut self,
        selected: &str,
        on_select: F,
        draw: impl FnOnce(&mut RadioGroup<'_, 'ui, 'tx, M>),
    ) where
        F: Fn(&'static str) -> M + 'static,
    {
        let mut rg = RadioGroup {
            ui: self,
            selected: selected.to_string(),
            on_select: Rc::new(on_select),
        };
        draw(&mut rg);
    }

    /// Post-stage role tag on the last child pushed in the current
    /// frame — used by composite builders (`ui.select`, `ui.tabs`) whose
    /// children are ordinary kinds carrying richer semantics.
    fn set_role_last(&mut self, role: SemanticRole) {
        if let Some(i) = self.tx.frames.last().unwrap().children.last().copied() {
            self.tx.nodes[i as usize].role = Some(role);
        }
    }

    /// `ui.select` — the frozen combobox: a `Select`-variant trigger
    /// (value text or `data-placeholder`, chevron end icon, combobox
    /// semantics) plus an anchored `PopupRole::Listbox` popup staged only
    /// while `open`. Focus model: `focus_on_open` moves focus INTO the
    /// listbox (the `selected` item when set, else the first enabled);
    /// arrows/Home/End walk enabled options (`nav_target`), Enter/Space
    /// activates the live option, Escape/outside-press dismiss via the
    /// popup foundation; closing restores focus to the trigger.
    pub fn select<F>(
        &mut self,
        props: Select<'_>,
        on_change: F,
        draw: impl FnOnce(&mut SelectItems<'_, '_, '_, M>),
    ) where
        F: Fn(SelectChange) -> M + 'static,
    {
        let on_change = Rc::new(on_change);
        // trigger — the combobox value button (w-fit natural)
        let text = props.value.unwrap_or(props.placeholder);
        let mut b = self
            .button(text)
            .variant(ButtonVariant::Select)
            .size(props.size)
            .icon_end(crate::icons::Icon::ChevronDown)
            .icon_color(Some(crate::style::Color::Role(ColorRole::MutedForeground)))
            .expanded(Some(props.open))
            .disabled(props.disabled)
            .invalid(props.invalid)
            .presented(props.presented)
            .automation_id(props.automation_id)
            .on_press({
                let cb = on_change.clone();
                let open = props.open;
                move || cb(SelectChange::Open(!open))
            });
        if props.value.is_none() {
            b = b.style(crate::style::select_placeholder_patch());
        }
        drop(b);
        self.set_role_last(SemanticRole::ComboBox);
        // listbox popup — chrome + content column
        let selected = props.selected;
        let pw = props.width.unwrap_or(crate::geom::dp(208.0));
        let mut p = self
            .popover(props.open, |ui| {
                ui.column(
                    Column::new()
                        .align(Align::Stretch)
                        .padding(Space::None)
                        .gap(Space::None),
                    |ui| {
                        let mut st = SelectItems {
                            ui,
                            selected,
                            popup_width: pw.0,
                            on_change: on_change.clone(),
                        };
                        draw(&mut st);
                    },
                );
            })
            .anchor(props.automation_id)
            .side(crate::geom::Side::Bottom)
            .align(Align::Center)
            .side_offset(crate::geom::dp(4.0))
            .role(crate::node::PopupRole::Listbox)
            .width(Length::Fixed(pw))
            .style(crate::style::select_content_patch());
        if let Some(id) = props.content_id {
            p = p.automation_id(id);
        }
        p.on_open_change(move |o| on_change(SelectChange::Open(o)));
    }

    /// `ui.tabs` — the frozen tab list: `list` part chrome (muted pill /
    /// line lane) wrapping a row of keyed `tab[key]` triggers. Press
    /// delivers `on_select(key)`; directional nav is the shared policy —
    /// ArrowLeft/Right move focus AND activate the target (Radix
    /// automatic activation), Home/End jump first/last, disabled tabs
    /// are skipped.
    pub fn tabs<F>(
        &mut self,
        props: Tabs<'_>,
        on_select: F,
        draw: impl FnOnce(&mut TabList<'_, '_, '_, M>),
    ) where
        F: Fn(&'static str) -> M + 'static,
    {
        let variant = props.variant;
        let selected = props.selected;
        let on_select = Rc::new(on_select);
        self.part("list", |ui| {
            ui.box_(
                BoxProps::new()
                    .style(crate::style::tabs_list_recipe(variant))
                    .height(Length::Fixed(crate::geom::dp(32.0))),
                |ui| {
                    ui.row(
                        Row::new()
                            .gap(match variant {
                                TabsVariant::Line => Space::Xs,
                                TabsVariant::Default => Space::None,
                            })
                            // exact chrome — the list recipe owns insets;
                            // opt out of the generic Space::Sm row padding
                            .padding(Space::None)
                            .align(Align::Center),
                        |ui| {
                            let mut tl = TabList {
                                ui,
                                variant,
                                selected,
                                on_select: on_select.clone(),
                            };
                            draw(&mut tl);
                        },
                    );
                },
            );
            ui.set_role_last(SemanticRole::TabList);
        });
    }

    /// Reusable desktop dropdown menu. The application owns `open` and
    /// every checked/radio value; menu callbacks are typed requests.
    pub fn dropdown_menu(
        &mut self,
        props: DropdownMenu<'_>,
        on_change: impl Fn(DropdownMenuChange) -> M + 'static,
        draw: impl FnOnce(&mut DropdownMenuItems<'_, '_, '_, M>),
    ) {
        let on_change = Rc::new(on_change);
        let trigger = self
            .button(props.label)
            .automation_id(props.automation_id)
            .variant(ButtonVariant::Outline)
            .expanded(Some(props.open))
            .on_press({
                let cb = on_change.clone();
                let open = props.open;
                move || cb(DropdownMenuChange::Open(!open))
            });
        drop(trigger);
        self.set_role_last(SemanticRole::MenuButton);
        let mut popup = self
            .popover(props.open, |ui| {
                ui.column(
                    Column::new()
                        .align(Align::Stretch)
                        .padding(Space::None)
                        .gap(Space::None),
                    |ui| {
                        let mut items = DropdownMenuItems {
                            ui,
                            on_change: on_change.clone(),
                            width: props.width.0,
                        };
                        draw(&mut items);
                    },
                );
            })
            .anchor(props.automation_id)
            .side(crate::geom::Side::Bottom)
            .align(Align::Start)
            .side_offset(crate::geom::dp(4.0))
            .role(crate::node::PopupRole::Menu)
            .width(Length::Fixed(props.width))
            .style(crate::style::select_content_patch());
        if let Some(id) = props.content_id {
            popup = popup.automation_id(id);
        }
        popup.on_open_change(move |open| on_change(DropdownMenuChange::Open(open)));
    }

    /// Sidebar-13 desktop SettingsNav. This component owns only the
    /// navigation rail; compose its content pane from ordinary components.
    pub fn settings_nav(
        &mut self,
        props: SettingsNav<'_>,
        on_select: impl Fn(&'static str) -> M + 'static,
        draw: impl FnOnce(&mut SettingsNavGroups<'_, '_, '_, M>),
    ) {
        let selected = props.selected;
        let on_select = Rc::new(on_select);
        let width = crate::geom::dp(if props.collapsed { 48.0 } else { 256.0 });
        self.named(props.automation_id, |ui| {
            ui.box_(
                crate::style::BoxProps::new()
                    .style(crate::style::settings_nav_surface())
                    .width(Length::Fixed(width)),
                |ui| {
                    ui.column(
                        Column::new()
                            .align(Align::Stretch)
                            .padding(Space::None)
                            .gap(Space::None),
                        |ui| {
                            if !props.collapsed {
                                ui.part("header", |ui| {
                                    ui.text("Settings")
                                        .style(crate::style::settings_nav_header_style())
                                        .chrome(crate::style::settings_nav_header_box())
                                        .height(Length::Fixed(crate::geom::dp(44.0)));
                                });
                            }
                            let mut groups = SettingsNavGroups {
                                ui,
                                collapsed: props.collapsed,
                                selected,
                                on_select: on_select.clone(),
                            };
                            draw(&mut groups);
                        },
                    );
                },
            );
        });
    }

    /// Standalone SettingsNavItem state specimen. It uses the same
    /// recipe and runtime semantics as an item inside `settings_nav`.
    pub fn settings_nav_item(
        &mut self,
        id: &str,
        label: &'static str,
        icon: crate::icons::Icon,
        active: bool,
        opts: SettingsNavItemOpts,
        on_press: impl Fn() -> M + 'static,
    ) {
        stage_settings_nav_item(
            self,
            id,
            id,
            label,
            icon,
            active,
            false,
            opts,
            Rc::new(on_press),
        );
    }

    /// `ui.field` — the frozen 288dp field container: `Vertical` stacks
    /// label/control/description at 8dp gaps (Stretch); `Horizontal` is a
    /// `Center`-aligned row at 8dp gap. Compose `ui.part` + the
    /// `field_*_style` recipes inside.
    pub fn field(&mut self, props: Field, draw: impl FnOnce(&mut Ui<'_, '_, M>)) {
        let layout = LayoutSpec {
            width: props.width.map(Length::Fixed).unwrap_or(Length::Fill(1)),
            ..Default::default()
        };
        match props.orientation {
            crate::theme::FieldOrientation::Vertical => self.tx.stage_container(
                crate::node::KIND_COLUMN,
                None,
                Column::new()
                    .gap(Space::Sm)
                    .align(Align::Stretch)
                    .padding(Space::None)
                    .props(),
                layout,
                EventFactorySet::default(),
                Visibility::Visible,
                draw,
                false,
                |props| NodeData::Container {
                    kind: crate::node::KIND_COLUMN,
                    props,
                },
            ),
            crate::theme::FieldOrientation::Horizontal => self.tx.stage_container(
                crate::node::KIND_ROW,
                None,
                Row::new()
                    .gap(Space::Sm)
                    .align(Align::Center)
                    .padding(Space::None)
                    .props(),
                layout,
                EventFactorySet::default(),
                Visibility::Visible,
                draw,
                false,
                |props| NodeData::Container {
                    kind: crate::node::KIND_ROW,
                    props,
                },
            ),
        }
    }

    /// `ui.input` — the frozen `input` element: a KIND_STACK pairing the
    /// editor leaf (automation id, opaque borderless content chrome) with
    /// a transparent overlay box carrying the 10dp radius + 1dp border +
    /// disabled `bg-input/50` fill. The overlay owns the frame so the
    /// radius never clips the windowless peer.
    pub fn input<'a, 'b>(&'a mut self, value: &'b TextValue) -> InputBuilder<'a, 'ui, 'b, M> {
        InputBuilder::new(self.tx, value, false)
    }

    /// `ui.textarea` — same stack composition at the multiline chrome
    /// (`py-2` = 9dp top/bottom incl. the 1dp border).
    pub fn textarea<'a, 'b>(&'a mut self, value: &'b TextValue) -> InputBuilder<'a, 'ui, 'b, M> {
        InputBuilder::new(self.tx, value, true)
    }
}
