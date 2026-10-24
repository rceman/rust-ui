
/// `ui.radio_group` item stager — passed to the group draw closure.
pub struct RadioGroup<'a, 'ui, 'tx, M: 'static> {
    ui: &'a mut Ui<'ui, 'tx, M>,
    selected: String,
    on_select: Rc<dyn Fn(&'static str) -> M>,
}
impl<'a, 'ui, 'tx, M: 'static> RadioGroup<'a, 'ui, 'tx, M> {
    /// One `item[key]` part: 16×16 radio toggle, checked when the group's
    /// `selected == key`; Press delivers `on_select(key)`.
    pub fn item(&mut self, key: &'static str, label: &'static str, opts: RadioItemOpts) {
        let cb = self.on_select.clone();
        let checked = self.selected == key;
        self.ui.part(&format!("item[{key}]"), |ui| {
            let mut fac = EventFactorySet::default();
            fac.on_press = Some(Box::new(move || Box::new(cb(key)) as Box<dyn Any>));
            ui.stage_toggle(
                crate::theme::ToggleKind::RadioItem,
                crate::theme::ToggleSize::Default,
                checked,
                opts.disabled,
                opts.invalid,
                Some(label),
                None,
                opts.presented,
                LayoutSpec::default(),
                fac,
            );
        });
    }
}

/// `ui.select` content stager — passed to the select draw closure. Items
/// stage inside the listbox column; each `item[key]` carries option
/// semantics, `label[key]` is a group heading, `separator()` is the
/// bleeding hairline between groups.
pub struct SelectItems<'a, 'ui, 'tx, M: 'static> {
    ui: &'a mut Ui<'ui, 'tx, M>,
    selected: Option<&'static str>,
    popup_width: f32,
    on_change: Rc<dyn Fn(SelectChange) -> M>,
}
impl<'a, 'ui, 'tx, M: 'static> SelectItems<'a, 'ui, 'tx, M> {
    pub fn group(
        &mut self,
        key: &'static str,
        text: &'static str,
        draw: impl FnOnce(&mut SelectItems<'_, '_, '_, M>),
    ) {
        let selected = self.selected;
        let popup_width = self.popup_width;
        let on_change = self.on_change.clone();
        self.ui.part(&format!("group[{key}]"), |ui| {
            ui.column(
                Column::new()
                    .align(Align::Stretch)
                    .padding(Space::None)
                    .gap(Space::None),
                |ui| {
                    let mut items = SelectItems {
                        ui,
                        selected,
                        popup_width,
                        on_change,
                    };
                    items.label(key, text);
                    draw(&mut items);
                },
            );
        });
        self.ui.set_role_last(SemanticRole::Group);
    }

    /// `label[key]` part — group heading (`h-6 px-1.5 py-1 text-xs
    /// muted-foreground`).
    pub fn label(&mut self, key: &str, text: &str) {
        self.ui.part(&format!("label[{key}]"), |ui| {
            ui.text(text)
                .style(crate::style::select_label_style())
                .chrome(crate::style::BoxStyle {
                    padding: crate::style::Insets {
                        top: crate::geom::dp(4.0),
                        right: crate::geom::dp(6.0),
                        bottom: crate::geom::dp(4.0),
                        left: crate::geom::dp(6.0),
                    },
                    ..crate::style::BoxStyle::default()
                })
                .height(Length::Fixed(crate::geom::dp(24.0)));
        });
    }

    /// `item[key]` part — the option row (h-7, `SelectItem` recipe,
    /// check indicator when `selected`); Press delivers
    /// `SelectChange::Pick(key)`.
    pub fn item(&mut self, key: &'static str, text: &'static str, opts: SelectItemOpts) {
        let cb = self.on_change.clone();
        let selected = self.selected == Some(key);
        self.ui.part(&format!("item[{key}]"), |ui| {
            let mut b = ui
                .button(text)
                .variant(ButtonVariant::SelectItem)
                .selected(selected)
                .disabled(opts.disabled)
                .presented(opts.presented)
                .width(Length::Fill(1))
                .height(Length::Fixed(crate::geom::dp(28.0)))
                .on_press(move || cb(SelectChange::Pick(key)));
            if selected {
                b = b.icon_end(crate::icons::Icon::Check);
            }
            if let Some(id) = opts.automation_id {
                b = b.automation_id(id);
            }
            drop(b);
            ui.set_role_last(SemanticRole::Option);
        });
    }

    /// `separator` part — the `-mx-1 my-1 h-px` bleeding hairline: a
    /// popup-width line centered so it bleeds 4dp past the content lane
    /// to the popup border while staying inside the popup rect.
    /// Frozen select separators sit BETWEEN `p-1` groups, so the contract
    /// zone is group-pad(4) + my-1(4) + line(1) + my-1(4) + group-pad(4)
    /// = 17dp; the `Space::Sm` wrapper models the whole zone (the group
    /// columns stay unpadded so items aren't double-inset). Adjacent
    /// groups without a separator lose the inter-group 8dp — unexercised
    /// by the frozen specimen; true group `p-1` needs per-side insets.
    pub fn separator(&mut self) {
        let w = self.popup_width;
        self.ui.column(
            Column::new().padding(Space::Sm).align(Align::Center),
            |ui| {
                ui.part("separator", |ui| {
                    ui.box_(
                        BoxProps::new()
                            .width(Length::Fixed(crate::geom::dp(w)))
                            .height(Length::Fixed(crate::geom::dp(1.0)))
                            .style(crate::style::BoxStyle {
                                background: crate::style::Color::Role(ColorRole::Border),
                                ..crate::style::BoxStyle::default()
                            }),
                        |_| {},
                    );
                });
            },
        );
    }
}

/// `ui.tabs` trigger stager — passed to the tabs draw closure inside the
/// list's row.
pub struct TabList<'a, 'ui, 'tx, M: 'static> {
    ui: &'a mut Ui<'ui, 'tx, M>,
    variant: TabsVariant,
    selected: &'a str,
    on_select: Rc<dyn Fn(&'static str) -> M>,
}
impl<'a, 'ui, 'tx, M: 'static> TabList<'a, 'ui, 'tx, M> {
    /// One `tab[key]` part — the keyed trigger (h-25px tab semantics);
    /// `selected` paints the recipe's active chrome (chip + shadow, or
    /// the line underline marker). Press delivers `on_select(key)`.
    pub fn tab(&mut self, key: &'static str, label: &'static str, opts: TabOpts) {
        let cb = self.on_select.clone();
        let active = self.selected == key;
        let variant = match self.variant {
            TabsVariant::Line => ButtonVariant::TabLine,
            TabsVariant::Default => ButtonVariant::Tab,
        };
        self.ui.part(&format!("tab[{key}]"), |ui| {
            let mut b = ui
                .button(label)
                .variant(variant)
                .selected(active)
                .disabled(opts.disabled)
                .presented(opts.presented)
                .height(Length::Fixed(crate::geom::dp(25.0)))
                .on_press(move || cb(key));
            if let Some(id) = opts.automation_id {
                b = b.automation_id(id);
            }
            drop(b);
            ui.set_role_last(SemanticRole::Tab);
        });
    }
}

pub struct DropdownMenuItems<'a, 'ui, 'tx, M: 'static> {
    ui: &'a mut Ui<'ui, 'tx, M>,
    on_change: Rc<dyn Fn(DropdownMenuChange) -> M>,
    width: f32,
}

impl<'a, 'ui, 'tx, M: 'static> DropdownMenuItems<'a, 'ui, 'tx, M> {
    pub fn group(
        &mut self,
        key: &'static str,
        label: &'static str,
        draw: impl FnOnce(&mut DropdownMenuItems<'_, '_, '_, M>),
    ) {
        let cb = self.on_change.clone();
        let width = self.width;
        self.ui.part(&format!("group[{key}]"), |ui| {
            ui.column(
                Column::new()
                    .align(Align::Stretch)
                    .padding(Space::None)
                    .gap(Space::None),
                |ui| {
                    ui.part(&format!("label[{key}]"), |ui| {
                        ui.text(label)
                            .style(crate::style::dropdown_label_style())
                            .chrome(crate::style::dropdown_label_box())
                            .height(Length::Fixed(crate::geom::dp(24.0)));
                    });
                    draw(&mut DropdownMenuItems {
                        ui,
                        on_change: cb,
                        width,
                    });
                },
            );
        });
        self.ui.set_role_last(SemanticRole::Group);
    }

    pub fn item(&mut self, key: &'static str, label: &'static str, opts: DropdownItemOpts) {
        let cb = self.on_change.clone();
        let role = match opts.kind {
            DropdownItemKind::Normal => SemanticRole::MenuItem,
            DropdownItemKind::Destructive => SemanticRole::MenuItemDestructive,
            DropdownItemKind::Checkbox { .. } => SemanticRole::MenuItemCheckbox,
            DropdownItemKind::Radio { .. } => SemanticRole::MenuItemRadio,
        };
        let checked = match opts.kind {
            DropdownItemKind::Checkbox { checked } => Some(checked),
            DropdownItemKind::Radio { selected } => Some(selected),
            _ => None,
        };
        let destructive = opts.kind == DropdownItemKind::Destructive;
        let activation = match opts.kind {
            DropdownItemKind::Normal | DropdownItemKind::Destructive => {
                DropdownMenuChange::Activate(key)
            }
            DropdownItemKind::Checkbox { checked } => DropdownMenuChange::Checkbox(key, !checked),
            DropdownItemKind::Radio { .. } => DropdownMenuChange::Radio(key),
        };
        self.ui.part(&format!("item[{key}]"), |ui| {
            let mut action = ui
                .action(
                    Action::new()
                        .label(label)
                        .style(crate::style::dropdown_item_style(destructive))
                        .disabled(opts.disabled),
                    |ui| {
                        ui.row(
                            Row::new()
                                .width(Length::Fill(1))
                                .height(Length::Fixed(crate::geom::dp(20.0)))
                                .align(Align::Center)
                                .padding(Space::None),
                            |ui| {
                                ui.row(
                                    Row::new()
                                        .align(Align::Center)
                                        .gap(Space::Xs)
                                        .padding(Space::None),
                                    |ui| {
                                        if let Some(icon) = opts.icon {
                                            ui.part("icon", |ui| {
                                                ui.icon(icon)
                                                    .size(crate::geom::dp(16.0))
                                                    .color_role(if destructive {
                                                        ColorRole::Destructive
                                                    } else {
                                                        ColorRole::Foreground
                                                    });
                                            });
                                        }
                                        ui.text(label).style(
                                            crate::style::dropdown_item_text_style(
                                                if destructive {
                                                    ColorRole::Destructive
                                                } else {
                                                    ColorRole::Foreground
                                                },
                                            ),
                                        );
                                    },
                                );
                                ui.box_(BoxProps::new().width(Length::Fill(1)), |_| {});
                                match opts.kind {
                                    DropdownItemKind::Checkbox { checked }
                                    | DropdownItemKind::Radio { selected: checked } => {
                                        ui.part("indicator", |ui| {
                                            ui.box_(
                                                BoxProps::new()
                                                    .width(Length::Fixed(crate::geom::dp(18.0))),
                                                |ui| {
                                                    if checked {
                                                        ui.icon(crate::icons::Icon::Check)
                                                            .size(crate::geom::dp(16.0));
                                                    }
                                                },
                                            );
                                        });
                                    }
                                    _ => {
                                        if let Some(shortcut) = opts.shortcut {
                                            ui.part("shortcut", |ui| {
                                                ui.text(shortcut)
                                                    .style(crate::style::dropdown_shortcut_style());
                                            });
                                        }
                                    }
                                }
                            },
                        );
                    },
                )
                .semantic_role(role)
                .semantic_checked(checked)
                .presented(opts.presented)
                .on_press(move || cb(activation));
            if let Some(id) = opts.automation_id {
                action = action.automation_id(id);
            }
            drop(action);
        });
    }

    pub fn separator(&mut self) {
        let width = self.width;
        self.ui.part("separator", |ui| {
            ui.column(
                Column::new().padding(Space::Xs).align(Align::Center),
                |ui| {
                    ui.box_(
                        BoxProps::new()
                            .width(Length::Fixed(crate::geom::dp(width)))
                            .height(Length::Fixed(crate::geom::dp(1.0)))
                            .style(crate::style::BoxStyle {
                                background: crate::style::Color::Role(ColorRole::Border),
                                ..Default::default()
                            }),
                        |_| {},
                    );
                },
            );
        });
    }
}
