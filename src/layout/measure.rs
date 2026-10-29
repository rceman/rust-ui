use super::*;

pub fn ring_radius(component_radius: f32) -> f32 {
    component_radius.max(0.0) + 1.5
}

/// The single ring-color decision — the invalid ring is unconditional and
/// never stacks: invalid wins over focus, one ring only.
pub fn ring_color_role(invalid: bool) -> crate::theme::ColorRole {
    if invalid {
        crate::theme::ColorRole::DestructiveRing
    } else {
        crate::theme::ColorRole::Focus
    }
}

// ---------------------------------------------------------------------------
// Button geometry — shared by measure/paint/devtools so the icon part,
// text content and natural size always agree
// ---------------------------------------------------------------------------

/// The frozen `has-data-[icon]` rule: an inline icon edge consumes 2dp
/// less side padding than a text edge (`px-2.5` 10 -> `pl/pr-2` 8;
/// `px-2` 8 -> `pl/pr-1.5` 6). Returns the icon's edge inset — the
/// CONTENT starts after border + reduced padding.
pub fn icon_side_inset(base_pad_x: f32) -> f32 {
    (base_pad_x - 2.0).max(0.0)
}

/// Icon edge size per size class — `size-3` xs, `size-3.5` sm,
/// `size-4` default/lg. Select-family controls always carry `size-4`
/// (`[&_svg:not([class*='size-'])]:size-4` — no size class scaling).
pub fn button_icon_size(size: crate::theme::ControlSize) -> Dp {
    use crate::theme::ControlSize::*;
    Dp(match size {
        Xs => 12.0,
        Sm => 14.0,
        Md | Lg => 16.0,
    })
}

/// Resolved icon edge size honoring the variant's frozen rule — the
/// select family keeps 16px at every size class.
pub fn button_variant_icon_size(
    variant: crate::theme::ButtonVariant,
    size: crate::theme::ControlSize,
) -> Dp {
    use crate::theme::ButtonVariant::*;
    match variant {
        Select | SelectItem => Dp(16.0),
        _ => button_icon_size(size),
    }
}

/// Distance from the element's border edge to the icon's own edge along
/// each icon side. Standard buttons apply the frozen `has-data-[icon]`
/// reduced padding (`icon_side_inset`); select-family controls park the
/// icon at the recipe's absolute slot — the chevron sits `pr-2` inside
/// the border and the option check mark parks at `right-2` inside the
/// 32dp reserved indicator lane.
pub fn button_variant_icon_edge(
    variant: crate::theme::ButtonVariant,
    border_w: f32,
    pad: f32,
) -> f32 {
    use crate::theme::ButtonVariant::*;
    match variant {
        Select | SelectItem => border_w + 8.0,
        _ => border_w + icon_side_inset(pad),
    }
}

/// Right-side content reservation when `icon_end` is populated. For
/// select-family controls the recipe padding IS the indicator slot —
/// the icon overlays inside it, so no extra reserve exists.
pub fn button_variant_end_reserve(
    variant: crate::theme::ButtonVariant,
    size: crate::theme::ControlSize,
    border_w: f32,
    pad: f32,
) -> f32 {
    use crate::theme::ButtonVariant::*;
    match variant {
        SelectItem => border_w + pad,
        Select => border_w + pad + button_icon_size(size).0 + button_icon_gap(size).0,
        _ => border_w + icon_side_inset(pad) + button_icon_size(size).0 + button_icon_gap(size).0,
    }
}

/// Line-variant tab `::after` marker — the frozen `bottom-[-5px]` +
/// `h-0.5` underline: a 2dp `bg-foreground` bar whose bottom edge hangs
/// 5dp below the tab's bottom edge, full tab width.
pub fn tab_underline_rect(r: &Rect) -> Rect {
    Rect {
        x: r.x,
        y: r.y + r.height + 3.0,
        width: r.width,
        height: 2.0,
    }
}

// ---------------------------------------------------------------------------
// Directional group navigation — roving focus inside a semantic group
// (tablist / listbox options). Shared policy: the backend supplies the
// paint order + focused node; the group boundary is the nearest ancestor
// carrying the group's semantic role (or the enclosing popup for options).
// ---------------------------------------------------------------------------

/// The next directional-nav target for `key` on `focused`, or `None`
/// when the key isn't a nav key for the focused node's role or no
/// eligible sibling exists. `Tab` cycles ArrowLeft/Right through
/// same-role siblings inside the nearest `TabList` ancestor (the
/// backend also activates the target — Radix automatic activation).
/// `Option` cycles ArrowUp/Down among same-role descendants of the
/// enclosing popup. Home/End jump first/last. Disabled/hidden siblings
/// are skipped (the `interactive` gate); arrows wrap.
pub fn nav_target(
    arena: &crate::arena::Arena,
    order: &[NodeId],
    focused: NodeId,
    key: crate::event::Key,
) -> Option<NodeId> {
    use crate::event::Key;
    use crate::node::SemanticRole;
    let n = arena.get(focused)?;
    let role = n.role?;
    // group boundary + accepted keys per role
    let (group_role, forward, back) = match role {
        SemanticRole::Tab => (
            SemanticRole::TabList,
            matches!(key, Key::ArrowRight | Key::End),
            matches!(key, Key::ArrowLeft | Key::Home),
        ),
        SemanticRole::Option => (
            SemanticRole::TabList, // sentinel — options bound by the popup
            matches!(key, Key::ArrowDown | Key::End),
            matches!(key, Key::ArrowUp | Key::Home),
        ),
        SemanticRole::MenuItem
        | SemanticRole::MenuItemCheckbox
        | SemanticRole::MenuItemRadio
        | SemanticRole::MenuItemDestructive => (
            SemanticRole::TabList, // menu entries are bounded by the popup
            matches!(key, Key::ArrowDown | Key::End),
            matches!(key, Key::ArrowUp | Key::Home),
        ),
        _ => return None,
    };
    if !forward && !back {
        return None;
    }
    // enclosing group: nearest ancestor with the group role; for options
    // the enclosing popup is the boundary
    let mut group: Option<u32> = None;
    let mut cur = n.parent;
    while let Some(slot) = cur {
        let pn = arena.slot(slot)?;
        match role {
            SemanticRole::Tab if pn.role == Some(group_role) => {
                group = Some(slot);
                break;
            }
            SemanticRole::Option
            | SemanticRole::MenuItem
            | SemanticRole::MenuItemCheckbox
            | SemanticRole::MenuItemRadio
            | SemanticRole::MenuItemDestructive
                if matches!(&pn.data, crate::node::NodeData::Popup { .. }) =>
            {
                group = Some(slot);
                break;
            }
            _ => {}
        }
        cur = pn.parent;
    }
    let group = group?;
    let inside = |id: NodeId| -> bool {
        let mut cur = arena.get(id).and_then(|n| n.parent);
        while let Some(slot) = cur {
            if slot == group {
                return true;
            }
            cur = arena.slot(slot).and_then(|n| n.parent);
        }
        false
    };
    // eligible siblings in paint order (the document order), including
    // the focused node itself — arrows walk this sequence, Home/End
    // jump to its ends
    let seq: Vec<NodeId> = order
        .iter()
        .copied()
        .filter(|&id| {
            inside(id)
                && arena.get(id).is_some_and(|n| {
                    (n.role == Some(role)
                        || matches!(
                            (role, n.role),
                            (
                                SemanticRole::MenuItem
                                    | SemanticRole::MenuItemCheckbox
                                    | SemanticRole::MenuItemRadio
                                    | SemanticRole::MenuItemDestructive,
                                Some(
                                    SemanticRole::MenuItem
                                        | SemanticRole::MenuItemCheckbox
                                        | SemanticRole::MenuItemRadio
                                        | SemanticRole::MenuItemDestructive
                                )
                            )
                        ))
                        && n.interactive()
                        && n.visibility == crate::geom::Visibility::Visible
                })
        })
        .collect();
    if seq.len() < 2 {
        return None;
    }
    let pos = seq.iter().position(|&id| id == focused)?;
    Some(match key {
        Key::Home => seq[0],
        Key::End => *seq.last().unwrap(),
        _ if forward => seq[(pos + 1) % seq.len()],
        _ => seq[(pos + seq.len() - 1) % seq.len()],
    })
}

/// Icon-to-text gap per size class — `gap-1` xs, `gap-1.5` otherwise.
pub fn button_icon_gap(size: crate::theme::ControlSize) -> Dp {
    Dp(match size {
        crate::theme::ControlSize::Xs => 4.0,
        _ => 6.0,
    })
}

/// The `icon` part rect inside a laid-out button — the shared authority
/// for the painter and the devtools part snapshot. `r` is the button's
/// layout rect (border-box); the icon sits centered vertically at the
/// content edge.
pub fn button_icon_rect(
    node: &crate::node::NodeData,
    vs: &crate::style::VisualStyle,
    r: &crate::geom::Rect,
    icon_end: bool,
) -> Option<crate::geom::Rect> {
    let crate::node::NodeData::Button {
        icon_start,
        icon_end: ie,
        icon_only,
        size,
        variant,
        ..
    } = node
    else {
        return None;
    };
    let is = *icon_start;
    let ie = *ie;
    let px = button_variant_icon_size(*variant, *size);
    let side = if icon_end { ie } else { is };
    if side.is_none() && !icon_only {
        return None;
    }
    let b = &vs.box_style;
    if *icon_only {
        let x = r.x + (r.width - px.0) / 2.0;
        let y = r.y + (r.height - px.0) / 2.0;
        return Some(crate::geom::Rect {
            x,
            y,
            width: px.0,
            height: px.0,
        });
    }
    let x = if icon_end {
        r.x + r.width
            - button_variant_icon_edge(*variant, b.border.right.width.0, b.padding.right.0)
            - px.0
    } else {
        r.x + button_variant_icon_edge(*variant, b.border.left.width.0, b.padding.left.0)
    };
    let y = r.y + (r.height - px.0) / 2.0;
    Some(crate::geom::Rect {
        x,
        y,
        width: px.0,
        height: px.0,
    })
}

// ---------------------------------------------------------------------------
// Toggle geometry — the shared authority for the toggle leaf's natural
// size, painted indicator/track/thumb/dot/label rects, and the devtools
// synthetic parts. Measure, paint and snapshot consume the same helpers so
// the three paths can never drift.
// ---------------------------------------------------------------------------

/// Natural (element) size of a `ToggleKind` — checkbox/radio 16×16;
/// switch tracks 32×18.4 default / 24×14 sm. The optional label paints
/// outside this rect at +24dp (frozen `owner_attributed` part); it is NOT
/// part of the element size.
pub fn toggle_natural_size(
    kind: crate::theme::ToggleKind,
    size: crate::theme::ToggleSize,
) -> (f32, f32) {
    use crate::theme::{ToggleKind as K, ToggleSize as S};
    match (kind, size) {
        (K::Checkbox, _) | (K::RadioItem, _) => (16.0, 16.0),
        (K::Switch, S::Default) => (32.0, 18.391),
        (K::Switch, S::Sm) => (24.0, 14.0),
    }
}

/// Checkbox indicator part — the frozen `(1,1,14,14)` inset box the check
/// icon fills.
pub fn toggle_checkbox_indicator_rect(r: &Rect) -> Rect {
    Rect {
        x: r.x + 1.0,
        y: r.y + 1.0,
        width: 14.0,
        height: 14.0,
    }
}

/// Radio indicator part — the frozen `(1,1,14,16)` ring inset.
pub fn toggle_radio_indicator_rect(r: &Rect) -> Rect {
    Rect {
        x: r.x + 1.0,
        y: r.y + 1.0,
        width: 14.0,
        height: 16.0,
    }
}

/// Radio dot — the frozen `(4,4,8,8)` filled circle inside a checked item.
pub fn toggle_radio_dot_rect(r: &Rect) -> Rect {
    Rect {
        x: r.x + 4.0,
        y: r.y + 4.0,
        width: 8.0,
        height: 8.0,
    }
}

/// Switch thumb rect — default `(1,1.19,16,16)` → checked `(15,…)`;
/// sm `(1,1,12,12)` → checked `(11,…)`.
pub fn toggle_thumb_rect(size: crate::theme::ToggleSize, checked: bool, r: &Rect) -> Rect {
    toggle_thumb_rect_t(size, checked as u8 as f32, r)
}

/// Fractional thumb — `t` is the eased 0..1 position between the
/// unchecked and checked anchors (`transition-transform` on the frozen
/// cn-switch thumb). `t = checked as f32` reproduces the static rects.
pub fn toggle_thumb_rect_t(size: crate::theme::ToggleSize, t: f32, r: &Rect) -> Rect {
    let (d, y, xu, xc) = match size {
        crate::theme::ToggleSize::Default => (16.0, 1.188, 1.0, 15.0),
        crate::theme::ToggleSize::Sm => (12.0, 1.0, 1.0, 11.0),
    };
    Rect {
        x: r.x + xu + (xc - xu) * t.clamp(0.0, 1.0),
        y: r.y + y,
        width: d,
        height: d,
    }
}

/// Toggle label part — painted at +24dp (16 box + 8 gap), 14dp high at
/// +1dp vertical offset (the frozen `label` part rect); `text_w` is the
/// measured ink advance.
pub fn toggle_label_rect(r: &Rect, text_w: f32) -> Rect {
    Rect {
        x: r.x + 24.0,
        y: r.y + 1.0,
        width: text_w,
        height: 14.0,
    }
}

/// Content insets for a button — resolved box padding/border plus the
/// reserved `icon_size + icon_gap` on each icon-bearing side, so the
/// text layout box never overlaps the icon. The icon edges themselves
/// keep the frozen `has-data-[icon]` reduced padding. Used by measure
/// (natural size) and paint (content/text placement), so the two can
/// never drift apart. `icon_only` drops both text edges.
pub fn button_content_insets(
    node: &crate::node::NodeData,
    vs: &crate::style::VisualStyle,
) -> Insets {
    let crate::node::NodeData::Button {
        icon_start,
        icon_end,
        icon_only,
        size,
        variant,
        ..
    } = node
    else {
        return Insets::default();
    };
    let b = &vs.box_style;
    let mut i = b.padding;
    i.top.0 += b.border.top.width.0;
    i.right.0 += b.border.right.width.0;
    i.bottom.0 += b.border.bottom.width.0;
    i.left.0 += b.border.left.width.0;
    if *icon_only {
        return i;
    }
    // icon edge = border + icon-side pad + icon + gap to the label;
    // select-family variants reserve the recipe's indicator slot instead
    let icon_reserve = |icon: &Option<crate::icons::Icon>, border: f32, pad: f32, end: bool| {
        icon.map(|_| {
            if end {
                button_variant_end_reserve(*variant, *size, border, pad)
            } else {
                border
                    + icon_side_inset(pad)
                    + button_variant_icon_size(*variant, *size).0
                    + button_icon_gap(*size).0
            }
        })
    };
    if let Some(l) = icon_reserve(icon_start, b.border.left.width.0, b.padding.left.0, false) {
        i.left.0 = l;
    }
    if let Some(r) = icon_reserve(icon_end, b.border.right.width.0, b.padding.right.0, true) {
        i.right.0 = r;
    }
    i
}
