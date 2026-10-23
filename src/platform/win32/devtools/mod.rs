use super::*;
use crate::devtools::{self as d, Error, Node, Rect, Result, Snapshot};
use crate::runtime::UpdateCtx;
use crate::{NodeId, UiResult};
use serde_json::{Value, json};

mod parts;
mod service;

pub(super) fn platform(e: impl ToString) -> Error {
    Error::new("UI_ERROR", e)
}

impl<S, M, U, V> Backend<S, M, U, V>
where
    M: 'static,
    U: Fn(&mut S, M, &mut UpdateCtx<'_, M>),
    V: Fn(&S, &mut crate::Ui<'_, '_, M>),
{
    /// Does `id`'s scroll-viewport clip fully exclude `r`? The one
    /// clip-visibility truth nodes AND parts share (hit-test/paint/UIA
    /// read the same map) — a part's rect is checked against the clip of
    /// the node that owns it.
    pub(super) fn clip_fully_excludes(&self, id: NodeId, r: LogicalRect) -> bool {
        self.clips.get(&id).is_some_and(|c| !r.intersects(*c))
    }
    pub(super) fn dev_snapshot(&self) -> Result<Snapshot> {
        let scale = self.peer_ctx.scale.get();
        let mut client = RECT::default();
        unsafe {
            GetClientRect(self.hwnd, &mut client).map_err(platform)?;
        }
        let physical = Rect {
            x: 0.,
            y: 0.,
            width: f64::from(client.right - client.left),
            height: f64::from(client.bottom - client.top),
        };
        let mut count = 0;
        let root = self.dev_node(self.rt.root, 0, &mut count)?;
        let snapshot = Snapshot {
            schema: d::SNAPSHOT_SCHEMA.into(),
            scale_factor: scale.0 as f64,
            client_dp: Rect {
                x: 0.,
                y: 0.,
                width: f64::from(scale.to_logical(client.right - client.left)),
                height: f64::from(scale.to_logical(client.bottom - client.top)),
            },
            client_px: physical,
            root,
        };
        snapshot.index()?;
        Ok(snapshot)
    }
    pub(crate) fn dev_node(&self, id: NodeId, depth: usize, count: &mut usize) -> Result<Node> {
        *count += 1;
        if depth > 128 || *count > 8192 {
            return Err(Error::new("MESSAGE_TOO_LARGE", "semantic tree limit"));
        }
        let n = self
            .rt
            .arena
            .get(id)
            .ok_or_else(|| Error::new("TARGET_NOT_FOUND", "stale semantic node"))?;
        let r = if id == self.rt.root {
            let mut c = RECT::default();
            unsafe {
                GetClientRect(self.hwnd, &mut c).map_err(platform)?;
            }
            LogicalRect {
                x: 0.,
                y: 0.,
                width: self.peer_ctx.scale.get().to_logical(c.right - c.left),
                height: self.peer_ctx.scale.get().to_logical(c.bottom - c.top),
            }
        } else {
            self.rects.get(&id).copied().unwrap_or_default()
        };
        let p = r.physical(self.peer_ctx.scale.get());
        let (kind, label, enabled) = match &n.data {
            NodeData::Empty => ("root", String::new(), true),
            NodeData::Container { .. } => ("container", String::new(), true),
            NodeData::Label { text, .. } => ("label", text.chars().take(1024).collect(), true),
            NodeData::Button { text, disabled, .. } => {
                ("button", text.chars().take(1024).collect(), !*disabled)
            }
            NodeData::Action {
                label, disabled, ..
            } => ("action", label.chars().take(1024).collect(), !*disabled),
            NodeData::Editor {
                snapshot, disabled, ..
            } => (
                "text-input",
                snapshot.committed.chars().take(1024).collect(),
                !*disabled,
            ),
            NodeData::Toggle {
                kind,
                disabled,
                label,
                ..
            } => (
                match kind {
                    crate::theme::ToggleKind::Checkbox => "checkbox",
                    crate::theme::ToggleKind::RadioItem => "radio",
                    crate::theme::ToggleKind::Switch => "switch",
                },
                label.as_deref().unwrap_or("").chars().take(1024).collect(),
                !*disabled,
            ),
            NodeData::Popup { .. } => ("popup", String::new(), true),
            NodeData::Custom { .. } => ("custom", String::new(), true),
            NodeData::Icon { .. } => ("icon", String::new(), true),
        };
        // a semantic role refines the kind label — the snapshot vocabulary
        // mirrors UIA control types (combobox/option/tablist/tab)
        let kind = match n.role {
            Some(crate::node::SemanticRole::ComboBox) => "combobox",
            Some(crate::node::SemanticRole::Option) => "option",
            Some(crate::node::SemanticRole::TabList) => "tablist",
            Some(crate::node::SemanticRole::Tab) => "tab",
            Some(crate::node::SemanticRole::MenuItem) => "menuitem",
            Some(crate::node::SemanticRole::MenuItemCheckbox) => "menuitemcheckbox",
            Some(crate::node::SemanticRole::MenuItemRadio) => "menuitemradio",
            Some(crate::node::SemanticRole::MenuItemDestructive) => "menuitem",
            Some(crate::node::SemanticRole::MenuButton) => "menu-button",
            Some(crate::node::SemanticRole::SettingsNavItem) => "navigation-item",
            Some(crate::node::SemanticRole::Group) => "group",
            None => kind,
        };
        let children = n
            .children
            .iter()
            .map(|slot| {
                self.dev_node(
                    NodeId {
                        slot: *slot,
                        generation: self.rt.arena.generation_of(*slot),
                    },
                    depth + 1,
                    count,
                )
            })
            .collect::<Result<Vec<_>>>()?;
        // semantic parts: `ui.part` chains inside the automation owner
        // (nested owners bound a deeper scope), plus the button's
        // synthetic `icon` part through the shared icon geometry. Only
        // nodes that own an automation_id may emit parts — a bare
        // part-marked child is collected by its owner, not self-emitted.
        let mut parts = Vec::new();
        if n.automation_id.is_some() {
            // the owner's own part key seeds the descendant prefix so
            // nested `ui.part` chains compose `pre.line[1]`, not `line[1]`
            let mut prefix = Vec::new();
            if let Some(pk) = &n.part_key {
                parts.push(d::PartNode {
                    key: pk.to_string(),
                    rect_dp: Rect {
                        x: r.x as f64,
                        y: r.y as f64,
                        width: r.width as f64,
                        height: r.height as f64,
                    },
                    rect_px: Rect {
                        x: p.left as f64,
                        y: p.top as f64,
                        width: (p.right - p.left) as f64,
                        height: (p.bottom - p.top) as f64,
                    },
                    visible: n.visibility == crate::Visibility::Visible
                        && !self.clip_fully_excludes(id, r),
                    reason: None,
                });
                prefix.push(pk.to_string());
            }
            self.collect_parts(id, &mut prefix, &mut parts);
            if let NodeData::Button {
                icon_start,
                icon_end,
                icon_only,
                size,
                variant,
                style,
                invalid,
                selected,
                ..
            } = &n.data
            {
                let has_icon = *icon_only || icon_start.is_some() || icon_end.is_some();
                if has_icon {
                    let vs = crate::style::resolve_button(
                        *variant,
                        *size,
                        style,
                        crate::style::StyleState::Normal,
                        false,
                        false,
                        *selected,
                        *invalid,
                    );
                    // one part per populated icon edge, mirroring the paint
                    // path: a single edge keeps the canonical `icon` key;
                    // a both-edge button gets `icon.start` / `icon.end` so
                    // no populated edge silently disappears.
                    let both = icon_start.is_some() && icon_end.is_some();
                    let mut emit_icon = |end: bool, key: &str| {
                        if let Some(ir) = crate::layout::button_icon_rect(&n.data, &vs, &r, end) {
                            let ip = ir.physical(self.peer_ctx.scale.get());
                            parts.push(d::PartNode {
                                key: key.into(),
                                rect_dp: Rect {
                                    x: ir.x as f64,
                                    y: ir.y as f64,
                                    width: ir.width as f64,
                                    height: ir.height as f64,
                                },
                                rect_px: Rect {
                                    x: ip.left as f64,
                                    y: ip.top as f64,
                                    width: (ip.right - ip.left) as f64,
                                    height: (ip.bottom - ip.top) as f64,
                                },
                                visible: n.visibility == crate::Visibility::Visible
                                    && !self.clip_fully_excludes(id, ir),
                                reason: None,
                            });
                        }
                    };
                    if *icon_only {
                        emit_icon(false, "icon");
                    } else if both {
                        emit_icon(false, "icon.start");
                        emit_icon(true, "icon.end");
                    } else {
                        emit_icon(icon_start.is_none(), "icon");
                    }
                }
            }
            // named toggle — synthetic geometry parts (same helpers as paint)
            if let NodeData::Toggle {
                kind,
                size,
                checked,
                label,
                ..
            } = &n.data
            {
                for (key, pr) in
                    toggle_part_rects(self, kind, *size, *checked, label.as_deref(), &r)
                {
                    let pp = pr.physical(self.peer_ctx.scale.get());
                    parts.push(d::PartNode {
                        key: key.into(),
                        rect_dp: Rect {
                            x: pr.x as f64,
                            y: pr.y as f64,
                            width: pr.width as f64,
                            height: pr.height as f64,
                        },
                        rect_px: Rect {
                            x: pp.left as f64,
                            y: pp.top as f64,
                            width: (pp.right - pp.left) as f64,
                            height: (pp.bottom - pp.top) as f64,
                        },
                        visible: n.visibility == crate::Visibility::Visible
                            && !self.clip_fully_excludes(id, pr),
                        reason: None,
                    });
                }
            }
        }
        // a scroll clip that fully excludes the node's rect reports the
        // truth — the node is laid out (rect is real) but painted out of
        // the viewport and unhittable
        let clipped_out = self.clip_fully_excludes(id, r);
        Ok(Node {
            automation_id: n.automation_id.as_deref().map(str::to_owned),
            kind: kind.into(),
            label,
            rect_dp: Rect {
                x: r.x as f64,
                y: r.y as f64,
                width: r.width as f64,
                height: r.height as f64,
            },
            rect_px: Rect {
                x: p.left as f64,
                y: p.top as f64,
                width: (p.right - p.left) as f64,
                height: (p.bottom - p.top) as f64,
            },
            visible: !clipped_out
                && n.visibility == crate::Visibility::Visible
                && r.width > 0.
                && r.height > 0.,
            enabled,
            // runtime truth — presented flags never move these fields
            focused: self.focus == Some(id),
            focus_visible: self.focus == Some(id),
            hovered: self.hot == Some(id),
            pressed: self.pressed == Some(id),
            presented: n.presented.any().then(|| d::Presented {
                hover: n.presented.hover,
                pressed: n.presented.pressed,
                focus_visible: n.presented.focus_visible,
            }),
            selection: match &n.data {
                crate::node::NodeData::Editor { selection, .. } => *selection,
                _ => None,
            },
            line_height: match &n.data {
                crate::node::NodeData::Editor { line_height, .. } => line_height.map(|d| d.0),
                _ => None,
            },
            line_twips: self.peer_for(id).map(|p| p.borrow().line_twips_latched()),
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
                (NodeData::Toggle { checked, .. }, _) => Some(*checked),
                _ => None,
            },
            expanded: match (&n.data, n.role) {
                (
                    NodeData::Button { expanded, .. },
                    Some(
                        crate::node::SemanticRole::ComboBox | crate::node::SemanticRole::MenuButton,
                    ),
                ) => *expanded,
                _ => None,
            },
            parts,
            children,
        })
    }
}

/// Intrinsic synthetic parts of a Toggle leaf — shared key+geometry list
/// used by `dev_node` (named toggles) and `collect_parts` (unnamed
/// toggles inside an automation owner's subtree).
pub(super) fn toggle_part_rects<S, M, U, V>(
    be: &Backend<S, M, U, V>,
    kind: &crate::theme::ToggleKind,
    size: crate::theme::ToggleSize,
    checked: bool,
    label: Option<&str>,
    r: &LogicalRect,
) -> Vec<(&'static str, LogicalRect)>
where
    M: 'static,
    U: Fn(&mut S, M, &mut UpdateCtx<'_, M>),
    V: Fn(&S, &mut crate::Ui<'_, '_, M>),
{
    let _ = be;
    let mut out: Vec<(&'static str, LogicalRect)> = Vec::new();
    match kind {
        crate::theme::ToggleKind::Checkbox => {
            if checked {
                let ir = crate::layout::toggle_checkbox_indicator_rect(r);
                out.push(("indicator", ir));
                out.push(("indicator.icon", ir));
            }
            if let Some(l) = label {
                let ts = crate::style::resolve_toggle_visual(
                    *kind,
                    size,
                    checked,
                    false,
                    crate::style::StyleState::Normal,
                    false,
                    false,
                )
                .text_style;
                let (tw, _) = super::render::measure_text(l, f32::MAX, &ts).unwrap_or((0.0, 14.0));
                out.push(("label", crate::layout::toggle_label_rect(r, tw)));
            }
        }
        crate::theme::ToggleKind::RadioItem => {
            let ir = crate::layout::toggle_radio_indicator_rect(r);
            out.push(("indicator", ir));
            if checked {
                out.push(("indicator.icon", crate::layout::toggle_radio_dot_rect(r)));
            }
        }
        crate::theme::ToggleKind::Switch => {
            out.push(("thumb", crate::layout::toggle_thumb_rect(size, checked, r)));
        }
    }
    out
}
