use super::*;

pub(crate) fn dirty_diff(
    old: &NodeData,
    new: &NodeData,
    dark: bool,
    forced: Option<&dyn Fn(crate::style::SystemColor) -> [f32; 4]>,
) -> u8 {
    const LAYOUT: u8 = 0b0000_0001;
    const PAINT: u8 = 0b0000_0010;
    const SEMANTICS: u8 = 0b0000_1000;
    match (old, new) {
        (
            NodeData::Container { kind: ak, props: a },
            NodeData::Container { kind: bk, props: b },
        ) => {
            if ak != bk {
                return LAYOUT | PAINT | SEMANTICS;
            }
            let (al, bl) = (a.layout_clone(), b.layout_clone());
            let mut d = 0;
            if al != bl {
                d |= LAYOUT;
            }
            // resolved boxes — recipe+patch+full, OS-enforced
            let mut ra = a.resolved_box(*ak);
            let mut rb = b.resolved_box(*bk);
            if let Some(v) = &mut ra {
                crate::style::os_enforce_box(v, forced);
            }
            if let Some(v) = &mut rb {
                crate::style::os_enforce_box(v, forced);
            }
            match (ra, rb) {
                (Some(ra), Some(rb)) => box_dirty(&ra, &rb, dark, &mut d),
                (ra, rb) => {
                    if ra.is_some() != rb.is_some() {
                        d |= PAINT | LAYOUT;
                    }
                }
            }
            d
        }
        (
            NodeData::Popup {
                anchor: aa,
                side: asi,
                align: aal,
                offset: ao,
                role: ar,
                modal: am,
                dismiss_light: ad,
                focus_on_open: af,
                props: ap,
            },
            NodeData::Popup {
                anchor: ba,
                side: bsi,
                align: bal,
                offset: bo,
                role: br,
                modal: bm,
                dismiss_light: bd,
                focus_on_open: bf,
                props: bp,
            },
        ) => {
            let mut d = 0;
            // placement inputs — the anchored rect is computed at layout
            if aa != ba || asi != bsi || aal != bal || ao != bo {
                d |= LAYOUT | PAINT;
            }
            // `modal` feeds layout (viewport scrim rect vs content rect)
            // AND paint (scrim) — a kept popup flipping it must relayout
            if am != bm {
                d |= LAYOUT | PAINT | SEMANTICS;
            }
            // dismiss/focus/role semantics — UIA + input routing truth
            if ar != br || ad != bd || af != bf {
                d |= SEMANTICS;
            }
            // popup chrome resolves like a surface container
            let mut ra = ap.resolved_box(KIND_POPUP);
            let mut rb = bp.resolved_box(KIND_POPUP);
            if let Some(v) = &mut ra {
                crate::style::os_enforce_box(v, forced);
            }
            if let Some(v) = &mut rb {
                crate::style::os_enforce_box(v, forced);
            }
            if let (Some(ra), Some(rb)) = (ra, rb) {
                box_dirty(&ra, &rb, dark, &mut d);
            }
            d
        }
        (
            NodeData::Label {
                text: at,
                wrap: aw,
                color_role: ac,
                patch: ap,
                chrome: ach,
            },
            NodeData::Label {
                text: bt,
                wrap: bw,
                color_role: bc,
                patch: bp,
                chrome: bch,
            },
        ) => {
            if at != bt || aw != bw {
                LAYOUT | PAINT
            } else {
                // resolved label text = recipe + patch
                let mut ra = crate::style::label_recipe();
                ra.patch(ap);
                let mut rb = crate::style::label_recipe();
                rb.patch(bp);
                let mut d = 0;
                if ac != bc {
                    d |= PAINT;
                }
                if ach != bch {
                    d |= PAINT;
                    let ci = |c: &Option<crate::style::BoxStyle>| {
                        c.map_or(0.0, |b| {
                            b.border.top.width.0
                                + b.border.bottom.width.0
                                + b.border.left.width.0
                                + b.border.right.width.0
                                + b.padding.top.0
                                + b.padding.bottom.0
                                + b.padding.left.0
                                + b.padding.right.0
                        })
                    };
                    if ci(ach) != ci(bch) {
                        d |= LAYOUT;
                    }
                }
                if ra != rb {
                    d |= PAINT;
                    if ra.size != rb.size
                        || ra.weight != rb.weight
                        || ra.font != rb.font
                        || ra.line_height != rb.line_height
                        || ra.letter_spacing != rb.letter_spacing
                    {
                        d |= LAYOUT;
                    }
                }
                d
            }
        }
        (
            NodeData::Button {
                text: at,
                variant: av,
                style: ast,
                disabled: ad,
                size: asz,
                motion: am,
                tooltip: atp,
                icon_start: ais,
                icon_end: aie,
                icon_only: aio,
                accessible_name: aan,
                invalid: ai,
                selected: ase,
                expanded: aex,
                icon_color: aic,
            },
            NodeData::Button {
                text: bt,
                variant: bv,
                style: bst,
                disabled: bd,
                size: bsz,
                motion: bm,
                tooltip: btp,
                icon_start: bis,
                icon_end: bie,
                icon_only: bio,
                accessible_name: ban,
                invalid: bi,
                selected: bse,
                expanded: bex,
                icon_color: bic,
            },
        ) => {
            let mut d = 0;
            if at != bt || asz != bsz || ais != bis || aie != bie || aio != bio {
                d |= LAYOUT;
            }
            if at != bt || av != bv || am != bm || ais != bis || aie != bie || aic != bic {
                d |= PAINT;
            }
            if ad != bd || atp != btp || aan != ban || ase != bse || aex != bex {
                d |= SEMANTICS;
            }
            // RESOLVED normal-state styles — recipe+patch applied, then
            // OS enforcement: two authored forms producing the same
            // concrete output are a strict no-op
            let mut ra = crate::style::resolve_button(
                *av,
                *asz,
                ast,
                crate::style::StyleState::Normal,
                false,
                dark,
                *ase,
                *ai,
            );
            crate::style::os_enforce_visual(&mut ra, forced);
            let mut rb = crate::style::resolve_button(
                *bv,
                *bsz,
                bst,
                crate::style::StyleState::Normal,
                false,
                dark,
                *bse,
                *bi,
            );
            crate::style::os_enforce_visual(&mut rb, forced);
            visual_dirty(&ra, &rb, dark, &mut d);
            // state branches' metrics feed layout too — padding in hover/
            // pressed/disabled/focus branches
            for st in [
                crate::style::StyleState::Hover,
                crate::style::StyleState::Pressed,
                crate::style::StyleState::Disabled,
            ] {
                let ra = crate::style::resolve_button(*av, *asz, ast, st, false, dark, *ase, *ai);
                let rb = crate::style::resolve_button(*bv, *bsz, bst, st, false, dark, *bse, *bi);
                if ra.box_style.padding != rb.box_style.padding
                    || ra.box_style.border.top.width != rb.box_style.border.top.width
                    || ra.box_style.border.right.width != rb.box_style.border.right.width
                    || ra.box_style.border.bottom.width != rb.box_style.border.bottom.width
                    || ra.box_style.border.left.width != rb.box_style.border.left.width
                {
                    d |= PAINT | LAYOUT;
                } else if ra != rb {
                    d |= PAINT;
                }
            }
            d
        }
        (
            NodeData::Icon {
                icon: ai,
                size: asz,
                color: ac,
            },
            NodeData::Icon {
                icon: bi,
                size: bsz,
                color: bc,
            },
        ) => {
            let mut d = 0;
            if ai != bi || asz != bsz {
                d |= LAYOUT;
            }
            if ai != bi
                || crate::style::resolve_color(*ac, dark) != crate::style::resolve_color(*bc, dark)
            {
                d |= PAINT;
            }
            d
        }
        (
            NodeData::Editor {
                snapshot: asnap,
                read_only: aro,
                disabled: ad,
                max_lines: aml,
                placeholder: aph,
                accessible_label: aal,
                submit: asu,
                patch: ap,
                invalid: ainv,
                selection: asl,
                ..
            },
            NodeData::Editor {
                snapshot: bsnap,
                read_only: bro,
                disabled: bd,
                max_lines: bml,
                placeholder: bph,
                accessible_label: bal,
                submit: bsu,
                patch: bp,
                invalid: binv,
                selection: bsl,
                ..
            },
        ) => {
            let mut d = 0;
            if aml != bml {
                d |= LAYOUT;
            }
            if ainv != binv || asl != bsl {
                d |= PAINT;
            }
            // committed text feeds the peer's natural measurement — a
            // content change can change the natural extent, so it is a
            // layout-classified dirty, not paint-only
            if asnap.committed != bsnap.committed {
                d |= PAINT | LAYOUT;
            }
            if aph != bph {
                d |= PAINT;
            }
            if ap != bp {
                // foreground -> peer repaint; chrome -> resolved box diff
                if ap.foreground != bp.foreground {
                    d |= PAINT;
                }
                let mut ca = crate::style::resolve_text_input_chrome(ap);
                let mut cb = crate::style::resolve_text_input_chrome(bp);
                crate::style::os_enforce_box(&mut ca, forced);
                crate::style::os_enforce_box(&mut cb, forced);
                box_dirty(&ca, &cb, dark, &mut d);
            }
            if aro != bro || ad != bd || aal != bal || asu != bsu {
                d |= SEMANTICS;
            }
            d
        }
        (
            NodeData::Toggle {
                kind: ak,
                checked: ac,
                disabled: ad,
                invalid: ai,
                size: asz,
                label: al,
                accessible_name: aan,
            },
            NodeData::Toggle {
                kind: bk,
                checked: bc,
                disabled: bd,
                invalid: bi,
                size: bsz,
                label: bl,
                accessible_name: ban,
            },
        ) => {
            let mut d = 0;
            if ak != bk || asz != bsz {
                d |= LAYOUT | PAINT;
            }
            if ac != bc || ai != bi || al != bl {
                d |= PAINT;
            }
            if ad != bd || aan != ban {
                d |= SEMANTICS;
            }
            d
        }
        (
            NodeData::Custom {
                render: ar,
                frame_events: af,
            },
            NodeData::Custom {
                render: br,
                frame_events: bf,
            },
        ) => {
            let mut d = 0;
            if !std::rc::Rc::ptr_eq(ar, br) {
                d |= PAINT;
            }
            if af != bf {
                d |= SEMANTICS;
            }
            d
        }
        (
            NodeData::Action {
                label: al,
                style: ast,
                disabled: ad,
                checked: ac,
            },
            NodeData::Action {
                label: bl,
                style: bst,
                disabled: bd,
                checked: bc,
            },
        ) => {
            let mut d = 0;
            // resolved normal-state box — padding/border widths feed the
            // action's content insets; colors/shadow are paint-only
            let ra = ast.resolve(*ad, false, false, false);
            let rb = bst.resolve(*bd, false, false, false);
            box_dirty(&ra, &rb, dark, &mut d);
            // state branches can also carry metrics-bearing fields — only
            // when a branch's patch actually changed
            for (ap, bp) in [ast.hover, ast.pressed, ast.disabled, ast.focus_visible]
                .iter()
                .zip([bst.hover, bst.pressed, bst.disabled, bst.focus_visible].iter())
            {
                if ap == bp {
                    continue;
                }
                d |= PAINT;
                if ap.is_some_and(|p| crate::style::patch_touches_layout(&p))
                    || bp.is_some_and(|p| crate::style::patch_touches_layout(&p))
                {
                    d |= LAYOUT;
                }
            }
            if ad != bd || al != bl || ac != bc {
                d |= SEMANTICS;
            }
            d
        }
        _ => 0b0000_0111, // kind swap — everything
    }
}

impl ContainerProps {
    fn visual_clone(&self) -> (crate::style::BoxStylePatch, Option<crate::style::BoxStyle>) {
        (self.patch, self.full)
    }
    fn layout_clone(
        &self,
    ) -> (
        Option<Space>,
        Option<Align>,
        Option<Justify>,
        Option<Space>,
        crate::geom::Dp,
    ) {
        (
            self.gap,
            self.align,
            self.justify,
            self.padding,
            self.scroll,
        )
    }
}

impl Node {
    pub(crate) fn kind_tag(&self) -> u8 {
        match &self.data {
            NodeData::Empty => 0,
            NodeData::Container { kind, .. } => *kind,
            NodeData::Label { .. } => KIND_LABEL,
            NodeData::Button { .. } => KIND_BUTTON,
            NodeData::Editor { multiline, .. } => {
                if *multiline {
                    KIND_TEXT_AREA
                } else {
                    KIND_TEXT_INPUT
                }
            }
            NodeData::Custom { .. } => KIND_CUSTOM,
            NodeData::Action { .. } => KIND_ACTION,
            NodeData::Icon { .. } => KIND_ICON,
            NodeData::Toggle { .. } => KIND_TOGGLE,
            NodeData::Popup { .. } => KIND_POPUP,
        }
    }

    /// Interactive gate: factory events (press/pointer/key/focus/frame) are
    /// withheld while hidden or disabled.
    pub(crate) fn interactive(&self) -> bool {
        if self.visibility != Visibility::Visible {
            return false;
        }
        match &self.data {
            NodeData::Button { disabled, .. } => !disabled,
            NodeData::Editor { disabled, .. } => !disabled,
            NodeData::Action { disabled, .. } => !disabled,
            NodeData::Toggle { disabled, .. } => !disabled,
            _ => true,
        }
    }

    /// Which lease this node's editor owns (for the mounted-editor index).
    pub(crate) fn lease(&self) -> Option<LeaseCell> {
        match &self.data {
            NodeData::Editor { snapshot, .. } => Some(snapshot.lease.clone()),
            _ => None,
        }
    }
}
