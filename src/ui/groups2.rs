
pub struct SettingsNavGroups<'a, 'ui, 'tx, M: 'static> {
    ui: &'a mut Ui<'ui, 'tx, M>,
    collapsed: bool,
    selected: &'static str,
    on_select: Rc<dyn Fn(&'static str) -> M>,
}

impl<'a, 'ui, 'tx, M: 'static> SettingsNavGroups<'a, 'ui, 'tx, M> {
    pub fn group(
        &mut self,
        key: &'static str,
        label: &'static str,
        draw: impl FnOnce(&mut SettingsNavItems<'_, '_, '_, M>),
    ) {
        let collapsed = self.collapsed;
        let selected = self.selected;
        let on_select = self.on_select.clone();
        self.ui.part(&format!("group[{key}]"), |ui| {
            ui.column(
                Column::new()
                    .align(Align::Stretch)
                    .padding(Space::Sm)
                    .gap(Space::None),
                |ui| {
                    if !collapsed {
                        ui.part("group-label", |ui| {
                            ui.text(label)
                                .style(crate::style::settings_nav_group_style())
                                .chrome(crate::style::settings_nav_group_box())
                                .height(Length::Fixed(crate::geom::dp(32.0)));
                        });
                    }
                    draw(&mut SettingsNavItems {
                        ui,
                        collapsed,
                        selected,
                        on_select,
                    });
                },
            );
        });
        self.ui.set_role_last(SemanticRole::Group);
    }
}

pub struct SettingsNavItems<'a, 'ui, 'tx, M: 'static> {
    ui: &'a mut Ui<'ui, 'tx, M>,
    collapsed: bool,
    selected: &'static str,
    on_select: Rc<dyn Fn(&'static str) -> M>,
}

impl<'a, 'ui, 'tx, M: 'static> SettingsNavItems<'a, 'ui, 'tx, M> {
    pub fn item(
        &mut self,
        key: &'static str,
        label: &'static str,
        icon: crate::icons::Icon,
        opts: SettingsNavItemOpts,
    ) {
        let id = opts.automation_id.unwrap_or_else(|| settings_nav_id(key));
        let cb = self.on_select.clone();
        stage_settings_nav_item(
            self.ui,
            id,
            key,
            label,
            icon,
            self.selected == key,
            self.collapsed,
            opts,
            Rc::new(move || cb(key)),
        );
    }
}

fn settings_nav_id(key: &'static str) -> &'static str {
    match key {
        "general" => "settings-nav.general",
        "appearance" => "settings-nav.appearance",
        "agents" => "settings-nav.agents",
        "voice" => "settings-nav.voice",
        "integrations" => "settings-nav.integrations",
        "advanced" => "settings-nav.advanced",
        _ => key,
    }
}

fn stage_settings_nav_item<'ui, 'tx, M: 'static>(
    ui: &mut Ui<'ui, 'tx, M>,
    id: &str,
    part_key: &str,
    label: &'static str,
    icon: crate::icons::Icon,
    active: bool,
    collapsed: bool,
    opts: SettingsNavItemOpts,
    on_press: Rc<dyn Fn() -> M>,
) {
    let item_id = id.to_string();
    ui.part(&format!("item[{part_key}]"), |ui| {
        let mut action = ui
            .action(
                Action::new()
                    .label(label)
                    .style(crate::style::settings_nav_item_style(active))
                    .disabled(opts.disabled),
                |ui| {
                    ui.row(
                        Row::new()
                            .width(Length::Fill(1))
                            .height(Length::Fixed(crate::geom::dp(32.0)))
                            .align(Align::Center)
                            .justify(if collapsed {
                                Justify::Center
                            } else {
                                Justify::Start
                            })
                            .gap(if collapsed { Space::None } else { Space::Sm })
                            .padding(if collapsed { Space::None } else { Space::Sm }),
                        |ui| {
                            ui.icon(icon)
                                .size(crate::geom::dp(16.0))
                                .color_role(if active {
                                    ColorRole::SidebarAccentForeground
                                } else {
                                    ColorRole::SidebarForeground
                                });
                            if !collapsed {
                                let style = crate::style::settings_nav_item_text_style(active);
                                ui.text(label).style(style);
                            }
                        },
                    );
                },
            )
            .semantic_role(SemanticRole::SettingsNavItem)
            .semantic_checked(Some(active))
            .presented(opts.presented)
            .on_press(move || on_press());
        if let Some(id) = opts.automation_id {
            action = action.automation_id(id);
        } else {
            action = action.automation_id(&item_id);
        }
        drop(action);
    });
}

