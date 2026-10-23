use super::*;

impl<S, M, U, V> Backend<S, M, U, V>
where
    M: 'static,
    U: Fn(&mut S, M, &mut UpdateCtx<'_, M>),
    V: Fn(&S, &mut crate::Ui<'_, '_, M>),
{
    /// `WM_GETOBJECT` — build/refresh the UIA root + live children once per
    /// request. Rebuild remints live generations, fencing stale providers.
    pub(crate) fn uia_provider(&mut self) -> UiResult<Option<uia::IRawRoot>> {
        use windows::Win32::UI::Accessibility::*;
        if self.uia.is_none() {
            let root = uia::UiaRoot::new(self.hwnd, "rust-ui")
                .map_err(|e| UiError::Platform(format!("UiaRoot: {e}")))?;
            // a late-created root starts with the backend's CURRENT focus —
            // a provider built after focus was taken must not lie
            root.set_focus(self.focus);
            self.uia = Some(root);
        }
        let Some(u) = self.uia.as_ref() else {
            return Ok(None);
        };
        let mut kids = Vec::new();
        for &id in &self.order {
            let Some(n) = self.rt.arena.get(id) else {
                continue;
            };
            let Some(r) = self.rects.get(&id).copied() else {
                continue;
            };
            // the published bounds are the VISIBLE band — the translated
            // node rect intersected with its scroll-viewport clip, never
            // a phantom off-window extent. An empty intersection means the
            // node is fully clipped → IsOffscreen (dev_tools' `visible`
            // reports the same truth — clip-out OR zero-size).
            let vr = self.clips.get(&id).map_or(r, |c| r.intersection(*c));
            let offscreen = vr.width <= 0.0 || vr.height <= 0.0;
            let scale = self.peer_ctx.scale.get();
            let pr = vr.physical(scale); // DIP -> client px (UIA wants px)
            // checked conversion — a failed ClientToScreen skips the node
            // rather than publishing unsound coordinates
            let Some(sp) = space::client_to_screen(
                self.hwnd,
                space::ClientPhysicalPoint(space::PhysicalPoint {
                    x: pr.left,
                    y: pr.top,
                }),
            ) else {
                continue;
            };
            let pt = sp.0;
            let rect = UiaRect {
                left: pt.x as f64,
                top: pt.y as f64,
                width: (pr.right - pr.left) as f64,
                height: (pr.bottom - pr.top) as f64,
            };
            let (name, ct, loc) = match &n.data {
                NodeData::Button {
                    text,
                    accessible_name,
                    ..
                } => (
                    // icon-only buttons carry no visible text — the
                    // semantic name comes from `.accessible_name`
                    accessible_name
                        .as_deref()
                        .unwrap_or(text.as_ref())
                        .to_string(),
                    UIA_ButtonControlTypeId,
                    "Button",
                ),
                NodeData::Label { text, .. } => (text.to_string(), UIA_TextControlTypeId, "Text"),
                NodeData::Editor {
                    accessible_label, ..
                } => (
                    accessible_label
                        .clone()
                        .unwrap_or_else(|| "editor".into())
                        .to_string(),
                    UIA_EditControlTypeId,
                    "Edit",
                ),
                NodeData::Toggle {
                    kind,
                    label,
                    accessible_name,
                    ..
                } => {
                    (
                        accessible_name
                            .as_deref()
                            .or(label.as_deref())
                            .unwrap_or("toggle")
                            .to_string(),
                        match kind {
                            crate::theme::ToggleKind::Checkbox
                            | crate::theme::ToggleKind::Switch => UIA_CheckBoxControlTypeId,
                            crate::theme::ToggleKind::RadioItem => UIA_RadioButtonControlTypeId,
                        },
                        match kind {
                            crate::theme::ToggleKind::Checkbox
                            | crate::theme::ToggleKind::Switch => "CheckBox",
                            crate::theme::ToggleKind::RadioItem => "RadioButton",
                        },
                    )
                }
                NodeData::Custom { render, .. } => {
                    let s = render.semantics();
                    let (ct, loc) = match s.role {
                        crate::ui::Role::Button => (UIA_ButtonControlTypeId, "Button"),
                        crate::ui::Role::Image => (UIA_ImageControlTypeId, "Image"),
                        crate::ui::Role::Text => (UIA_TextControlTypeId, "Text"),
                        _ => (UIA_CustomControlTypeId, "Custom"),
                    };
                    (s.label, ct, loc)
                }
                NodeData::Popup { role, .. } => (
                    n.automation_id.as_deref().unwrap_or("popup").to_string(),
                    match role {
                        crate::node::PopupRole::Popup => UIA_PaneControlTypeId,
                        crate::node::PopupRole::Menu => UIA_MenuControlTypeId,
                        crate::node::PopupRole::Listbox => UIA_ListControlTypeId,
                    },
                    match role {
                        crate::node::PopupRole::Popup => "Pane",
                        crate::node::PopupRole::Menu => "Menu",
                        crate::node::PopupRole::Listbox => "List",
                    },
                ),
                // semantic action — Button when actionable (Invoke), else a
                // named group; children are decorative
                NodeData::Action { label, .. } => {
                    if n.factories.on_press.is_some() {
                        (label.to_string(), UIA_ButtonControlTypeId, "Button")
                    } else {
                        (label.to_string(), UIA_GroupControlTypeId, "Group")
                    }
                }
                // role-only semantics on a generic container (the tablist
                // chrome box) still emit a control node
                _ => match n.role {
                    Some(role) => (
                        n.automation_id.as_deref().unwrap_or("group").to_string(),
                        semantic_uia_ct(role).0,
                        semantic_uia_ct(role).1,
                    ),
                    None => continue,
                },
            };
            // a semantic role refines the kind's control type — combobox
            // trigger / option / tab / tablist
            let (name, ct, loc) = match n.role {
                Some(role) => (name, semantic_uia_ct(role).0, semantic_uia_ct(role).1),
                None => (name, ct, loc),
            };
            kids.push(uia::ChildBuild {
                automation_id: n.automation_id.as_deref().unwrap_or("").to_string(),
                id,
                name,
                ct,
                localized: loc,
                rect,
                actionable: n.factories.on_press.is_some(),
                enabled: n.interactive(),
                offscreen,
                selected: match (&n.data, n.role) {
                    (
                        NodeData::Button { selected, .. },
                        Some(
                            crate::node::SemanticRole::Option
                            | crate::node::SemanticRole::Tab
                            | crate::node::SemanticRole::SettingsNavItem,
                        ),
                    ) => Some(*selected),
                    (
                        NodeData::Action { checked, .. },
                        Some(crate::node::SemanticRole::SettingsNavItem),
                    ) => *checked,
                    _ => None,
                },
                checked: match (&n.data, n.role) {
                    (
                        NodeData::Action { checked, .. },
                        Some(
                            crate::node::SemanticRole::MenuItemCheckbox
                            | crate::node::SemanticRole::MenuItemRadio,
                        ),
                    ) => *checked,
                    _ => None,
                },
                expanded: match (&n.data, n.role) {
                    (
                        NodeData::Button { expanded, .. },
                        Some(
                            crate::node::SemanticRole::ComboBox
                            | crate::node::SemanticRole::MenuButton,
                        ),
                    ) => *expanded,
                    _ => None,
                },
                peer_node: matches!(n.data, NodeData::Editor { .. }),
                native: None,
            });
        }
        // editors get the REAL windowless provider when available — the
        // registry is generation-keyed so a same-slot replacement can
        // never inherit the retired provider
        for k in &mut kids {
            if !k.peer_node {
                continue;
            }
            let registered = u
                .native
                .lock()
                .unwrap()
                .get(&k.id.slot)
                .is_some_and(|(g, _)| *g == k.id.generation);
            if registered {
                continue; // rebuild() picks it up from the registry
            }
            let Some(peer) = self.peer_for(k.id) else {
                continue;
            };
            let Some(iid) = peer.borrow().windowless_acc_iid() else {
                continue;
            };
            if let Ok(raw) = peer.borrow().query_iid(&iid) {
                let acc: IRicheditWindowlessAccessibility =
                    unsafe { windows::core::Interface::from_raw(raw) };
                if let Ok(site) = u.site(k.id)
                    && let Ok(prov) = unsafe { acc.CreateProvider(&site) }
                    && let Ok(frag) = prov.cast::<IRawElementProviderFragment>()
                {
                    u.register_native(k.id, frag.clone()).ok();
                    k.native = Some(frag);
                }
            }
        }
        u.rebuild(kids)
            .map_err(|e| UiError::Platform(format!("uia rebuild: {e}")))?;
        Ok(Some(u.provider()))
    }

    /// Push the current tree into the UIA tables when a client has ever
    /// asked for them — called at each commit boundary so external clients
    /// see live name/enabled/bounds/order WITHOUT needing a fresh
    /// WM_GETOBJECT. No-op until the root exists.
    pub(crate) fn uia_refresh(&mut self) -> UiResult {
        let t = std::time::Instant::now();
        let r = self.uia_refresh_impl();
        ResizeCounters::bump(
            &RESIZE.uia_runs,
            &RESIZE.uia_us,
            &RESIZE.uia_max_us,
            t.elapsed().as_micros() as u64,
        );
        r
    }

    pub(super) fn uia_refresh_impl(&mut self) -> UiResult {
        if self.uia.is_none() {
            return Ok(());
        }
        self.uia_provider()?;
        Ok(())
    }

    /// UIA press/focus post landed — the generation check happens HERE so a
    /// stale external press can't hit a newer peer, and eligibility is the
    /// SAME contract as pointer input: a disabled node cannot be focused
    /// or invoked through UIA either.
    pub(crate) fn uia_action(&mut self, slot: u32, generation: u64, focus: bool) -> UiResult {
        let id = NodeId { slot, generation };
        let eligible = self
            .rt
            .arena
            .get(id)
            .is_some_and(|n| n.interactive() && n.visibility == Visibility::Visible);
        if !eligible {
            return Ok(());
        }
        if focus {
            self.set_focus(Some(id))?;
            // focus host events (and any pending Change/SelChange) are
            // routed before the turn's pump — same boundary as the
            // pointer-down and devctl focus paths
            self.service_peer_events()?;
        } else {
            self.push_input(id, NodeEvent::Press)?;
        }
        self.turn()
    }
}

pub(super) fn semantic_uia_ct(
    role: crate::node::SemanticRole,
) -> (
    windows::Win32::UI::Accessibility::UIA_CONTROLTYPE_ID,
    &'static str,
) {
    use crate::node::SemanticRole;
    use windows::Win32::UI::Accessibility::*;
    match role {
        SemanticRole::ComboBox => (UIA_ComboBoxControlTypeId, "ComboBox"),
        SemanticRole::Option => (UIA_ListItemControlTypeId, "ListItem"),
        SemanticRole::TabList => (UIA_TabControlTypeId, "Tab"),
        SemanticRole::Tab => (UIA_TabItemControlTypeId, "TabItem"),
        SemanticRole::MenuItem
        | SemanticRole::MenuItemCheckbox
        | SemanticRole::MenuItemRadio
        | SemanticRole::MenuItemDestructive => (UIA_MenuItemControlTypeId, "MenuItem"),
        SemanticRole::MenuButton => (UIA_ButtonControlTypeId, "Button"),
        SemanticRole::SettingsNavItem => (UIA_ButtonControlTypeId, "Button"),
        SemanticRole::Group => (UIA_GroupControlTypeId, "Group"),
    }
}
