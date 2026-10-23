#[test]
fn card_recipe_shadow_has_unit_spread_ring() {
    for size in [crate::theme::CardSize::Default, crate::theme::CardSize::Sm] {
        let bs = crate::style::card_recipe(size);
        let sh = bs.shadow.expect("card carries the ring-1 spread shadow");
        assert_eq!(sh.spread.0, 1.0);
        assert_eq!(sh.blur_sigma.0, 0.0);
        assert_eq!(sh.offset_x.0, 0.0);
        assert_eq!(sh.offset_y.0, 0.0);
    }
}

#[test]
fn card_part_recipes_match_frozen_contract() {
    use crate::theme::CardSize;
    // part horizontal inset = --card-spacing (16 / 12)
    assert_eq!(
        crate::style::card_part_insets(CardSize::Default).left.0,
        16.0
    );
    assert_eq!(crate::style::card_part_insets(CardSize::Sm).left.0, 12.0);
    // title: Default 16/lh22, Sm 14/lh19.25, medium weight
    let d = crate::style::card_title_style(CardSize::Default);
    assert_eq!(d.size, crate::style::TextSize::Exact(crate::geom::Dp(16.0)));
    assert_eq!(d.line_height, Some(crate::geom::Dp(22.0)));
    assert_eq!(d.weight, crate::style::TextWeight::Medium);
    let s = crate::style::card_title_style(CardSize::Sm);
    assert_eq!(s.size, crate::style::TextSize::Exact(crate::geom::Dp(14.0)));
    assert_eq!(s.line_height, Some(crate::geom::Dp(19.25)));
    // content: 14/lh20 CardForeground
    let c = crate::style::card_content_style();
    assert_eq!(c.size, crate::style::TextSize::Exact(crate::geom::Dp(14.0)));
    assert_eq!(c.line_height, Some(crate::geom::Dp(20.0)));
    // footer: bg-muted/50 + 1dp border-top + spacing padding
    let f = crate::style::card_footer_style(CardSize::Default);
    assert_eq!(f.border.top.width.0, 1.0);
    assert_eq!(f.padding.top.0, 16.0);
    let f2 = crate::style::card_footer_style(CardSize::Sm);
    assert_eq!(f2.padding.top.0, 12.0);
}

#[test]
fn inline_code_recipe_is_13px_and_wraps_content() {
    let s = crate::typography::Typography::InlineCode.style();
    assert_eq!(s.size, crate::style::TextSize::Exact(crate::geom::Dp(13.0)));
    assert_eq!(s.weight, crate::style::TextWeight::Semibold);
    assert_eq!(s.line_height, Some(crate::geom::Dp(20.0)));
}

// ---- S03: Column/Row main-axis justify ------------------------------------

/// `justify_offset` is the ONE shared policy behind Row (horizontal) and
/// Column/Card (vertical) main-axis placement.
#[test]
fn justify_offset_is_shared_row_column_policy() {
    use crate::geom::Justify;
    use crate::layout::justify_offset;
    // used < inner: Start packs at origin, Center halves slack, End takes all
    assert_eq!(justify_offset(Justify::Start, 300.0, 60.0), 0.0);
    assert_eq!(justify_offset(Justify::Center, 300.0, 60.0), 120.0);
    assert_eq!(justify_offset(Justify::End, 300.0, 60.0), 240.0);
    // overflow and exact-fit clamp slack to zero — never a negative origin
    for j in [Justify::Start, Justify::Center, Justify::End] {
        assert_eq!(justify_offset(j, 40.0, 60.0), 0.0, "overflow {j:?}");
        assert_eq!(justify_offset(j, 60.0, 60.0), 0.0, "exact fit {j:?}");
    }
    // a Fill child consumes the whole axis -> used == inner -> offset 0
    assert_eq!(justify_offset(Justify::End, 300.0, 300.0), 0.0);
}

/// S03 — `Column::justify` must move children on the vertical main axis
/// through the real `KIND_COLUMN` walk branch (it was silently ignored).
/// Row coverage rides the same seam for axis symmetry.
#[cfg(windows)]
#[test]
fn column_and_row_justify_position_children_on_main_axis() {
    use crate::geom::{Dp, Justify, Length};
    use crate::node::{KIND_COLUMN, KIND_ROW};
    use crate::platform::win32::PeerCtx;
    use crate::platform::win32::layout::LayoutCache;
    use crate::platform::win32::space::{LogicalRect, ScaleFactor};
    use crate::style::BoxProps;
    use crate::{Column, Row, Space};

    // three 20dp fixed children inside a container rect, walked for each
    // justify — returns child origins on the container's main axis
    let column_ys = |justify: Justify, inner_h: f32| -> Vec<f32> {
        let mut rig = Rig::new(
            (),
            |_: &mut (), _: Msg, _: &mut UpdateCtx<Msg>| {},
            move |_, ui| {
                ui.column(
                    Column::new()
                        .justify(justify)
                        .gap(Space::None)
                        .padding(Space::None),
                    |ui| {
                        for _ in 0..3 {
                            ui.box_(BoxProps::new().height(Length::Fixed(Dp(20.0))), |_| {});
                        }
                    },
                );
            },
        );
        rig.view().unwrap();
        let col = rig.root_children()[0];
        let Rig { mut rt, .. } = rig;
        let ctx = PeerCtx::for_test().unwrap();
        let mut rects = std::collections::HashMap::new();
        let mut order = Vec::new();
        let mut clips = std::collections::HashMap::new();
        LayoutCache::new(ScaleFactor::ONE)
            .walk(
                &mut rt,
                &ctx,
                col,
                LogicalRect {
                    x: 0.0,
                    y: 0.0,
                    width: 200.0,
                    height: inner_h,
                },
                &mut rects,
                &mut order,
                &mut clips,
                &mut Vec::new(),
                None,
                KIND_COLUMN,
            )
            .unwrap();
        rt.arena
            .get(col)
            .unwrap()
            .children
            .iter()
            .map(|slot| {
                let cid = crate::NodeId {
                    slot: *slot,
                    generation: rt.arena.generation_of(*slot),
                };
                rects.get(&cid).unwrap().y
            })
            .collect()
    };
    // used = 60 in a 300dp column
    assert_eq!(column_ys(Justify::Start, 300.0), vec![0.0, 20.0, 40.0]);
    assert_eq!(column_ys(Justify::Center, 300.0), vec![120.0, 140.0, 160.0]);
    assert_eq!(column_ys(Justify::End, 300.0), vec![240.0, 260.0, 280.0]);
    // overflow (60 used in 40) clamps — Start-like, never negative
    assert_eq!(column_ys(Justify::End, 40.0), vec![0.0, 20.0, 40.0]);

    // Row symmetry through the same shared policy
    let row_xs = |justify: Justify| -> Vec<f32> {
        let mut rig = Rig::new(
            (),
            |_: &mut (), _: Msg, _: &mut UpdateCtx<Msg>| {},
            move |_, ui| {
                ui.row(
                    Row::new()
                        .justify(justify)
                        .gap(Space::None)
                        .padding(Space::None),
                    |ui| {
                        for _ in 0..3 {
                            ui.box_(BoxProps::new().width(Length::Fixed(Dp(20.0))), |_| {});
                        }
                    },
                );
            },
        );
        rig.view().unwrap();
        let row = rig.root_children()[0];
        let Rig { mut rt, .. } = rig;
        let ctx = PeerCtx::for_test().unwrap();
        let mut rects = std::collections::HashMap::new();
        let mut order = Vec::new();
        let mut clips = std::collections::HashMap::new();
        LayoutCache::new(ScaleFactor::ONE)
            .walk(
                &mut rt,
                &ctx,
                row,
                LogicalRect {
                    x: 0.0,
                    y: 0.0,
                    width: 300.0,
                    height: 32.0,
                },
                &mut rects,
                &mut order,
                &mut clips,
                &mut Vec::new(),
                None,
                KIND_ROW,
            )
            .unwrap();
        rt.arena
            .get(row)
            .unwrap()
            .children
            .iter()
            .map(|slot| {
                let cid = crate::NodeId {
                    slot: *slot,
                    generation: rt.arena.generation_of(*slot),
                };
                rects.get(&cid).unwrap().x
            })
            .collect()
    };
    assert_eq!(row_xs(Justify::Start), vec![0.0, 20.0, 40.0]);
    assert_eq!(row_xs(Justify::Center), vec![120.0, 140.0, 160.0]);
    assert_eq!(row_xs(Justify::End), vec![240.0, 260.0, 280.0]);
}

/// S02 — devtools semantic parts must expose the actually-populated icon
/// edge and compose descendant part keys under the owner's part_key.
#[cfg(all(windows, feature = "devtools"))]
#[test]
fn devtools_parts_expose_populated_icon_edges_and_owner_prefix() {
    use crate::platform::win32::space::LogicalRect;
    use crate::platform::win32::{Backend, PeerCtx};
    use crate::{Column, Space};

    let mut rig = Rig::new(
        (),
        |_: &mut (), _: Msg, _: &mut UpdateCtx<Msg>| {},
        |_, ui| {
            // leading-only, trailing-only and both-edge icon buttons
            ui.named("button.start", |ui| {
                ui.button("L")
                    .icon_start(crate::icons::Icon::ArrowLeftCircle)
                    .on_press(|| Msg::Press);
            });
            ui.named("button.end", |ui| {
                ui.button("R")
                    .icon_end(crate::icons::Icon::ArrowRight)
                    .on_press(|| Msg::Press);
            });
            ui.named("button.both", |ui| {
                ui.button("B")
                    .icon_start(crate::icons::Icon::ArrowLeftCircle)
                    .icon_end(crate::icons::Icon::ArrowRight)
                    .on_press(|| Msg::Press);
            });
            // owner node carrying automation_id AND part_key ("pre"),
            // like the code_block recipe
            ui.named("code", |ui| {
                ui.part("pre", |ui| {
                    ui.column(Column::new().gap(Space::None).padding(Space::None), |ui| {
                        for i in 1..=3usize {
                            ui.part(&format!("line[{i}]"), |ui| {
                                ui.label(format!("l{i}"));
                            });
                        }
                    });
                });
            });
        },
    );
    rig.view().unwrap();
    let kids = rig.root_children();
    let Rig { rt, .. } = rig;
    let ctx = PeerCtx::for_test().unwrap();
    let mut be = Backend::for_test(rt, ctx).unwrap();
    // give the walked nodes real rects — icon-edge math reads the owner rect
    let r = LogicalRect {
        x: 0.0,
        y: 0.0,
        width: 120.0,
        height: 32.0,
    };
    for k in &kids {
        be.rects.insert(*k, r);
    }
    let mut count = 0;
    let start = be.dev_node(kids[0], 0, &mut count).unwrap();
    let icon = start
        .parts
        .iter()
        .find(|p| p.key == "icon")
        .expect("leading icon part");
    assert!(
        icon.rect_dp.x < 20.0,
        "leading icon must sit on the start edge, got {icon:?}"
    );
    let end = be.dev_node(kids[1], 0, &mut count).unwrap();
    let icon = end
        .parts
        .iter()
        .find(|p| p.key == "icon")
        .expect("trailing icon part must resolve");
    assert!(
        icon.rect_dp.x > 60.0,
        "trailing icon must sit on the end edge, got {icon:?}"
    );
    let both = be.dev_node(kids[2], 0, &mut count).unwrap();
    let keys: Vec<&str> = both.parts.iter().map(|p| p.key.as_str()).collect();
    assert!(
        keys.contains(&"icon.start") && keys.contains(&"icon.end"),
        "both-edge button must emit both icon parts deterministically, got {keys:?}"
    );
    let code = be.dev_node(kids[3], 0, &mut count).unwrap();
    let keys: Vec<&str> = code.parts.iter().map(|p| p.key.as_str()).collect();
    assert_eq!(
        keys,
        ["pre", "pre.line[1]", "pre.line[2]", "pre.line[3]"],
        "owner part_key must seed descendant part prefixes"
    );
    // cross-axis sanity: a non-icon owner emits no icon parts at all
    let line = &code.children[0];
    assert!(
        line.parts.is_empty(),
        "plain column emits no synthetic parts"
    );
}

// ============ pre-slice-3 presentation state + ring geometry ============

/// `Presented` is staged presentation-only visual state — `runtime ||
/// presented` is the one OR rule. A staged `.presented(PRESSED)` lands on
/// the retained node and feeds `node_insets` exactly like a runtime press.
#[test]
fn presented_pressed_feeds_action_insets() {
    use crate::geom::Dp;
    use crate::node::Presented;
    use crate::style::{ActionStyle, BoxStyle, BoxStylePatch, InsetsPatch};
    let style = ActionStyle::new(BoxStyle::new()).pressed(BoxStylePatch {
        padding: InsetsPatch::all(Dp(9.0)),
        ..Default::default()
    });
    let mut rig = Rig::new(
        (),
        |_, _, _| {},
        move |_, ui| {
            ui.action(
                crate::style::Action::new().label("a").style(style.clone()),
                |ui| {
                    ui.text("x");
                },
            )
            .presented(Presented::PRESSED);
        },
    );
    rig.view().unwrap();
    let id = rig.root_children()[0];
    let n = rig.rt.arena.get(id).unwrap();
    assert_eq!(n.presented, Presented::PRESSED);
    // runtime Interaction is empty — only the presented bit is set, and it
    // still resolves the pressed-branch insets
    let ins = crate::layout::node_insets(id, n, &crate::layout::Interaction::default());
    assert_eq!(
        ins,
        crate::style::Insets::all(Dp(9.0)),
        "presented pressed must resolve the pressed branch insets"
    );
}

/// The pure OR helper every resolved_* backend path shares — presented
/// adds, never substitutes, and a runtime owner on ANOTHER node does not
/// leak.
#[test]
fn presented_or_is_runtime_or_presented() {
    use crate::layout::presented_or;
    let a = crate::NodeId {
        slot: 3,
        generation: 1,
    };
    let b = crate::NodeId {
        slot: 7,
        generation: 1,
    };
    assert!(presented_or(Some(a), a, false));
    assert!(presented_or(None, a, true));
    assert!(presented_or(Some(b), a, true));
    assert!(!presented_or(None, a, false));
    assert!(!presented_or(Some(b), a, false));
}

/// The frozen ring centerline = component corner radius + 1.5dp (3dp band
/// outside the border): checkbox r4 → 5.5, input/button r10 → 11.5, a
/// pill stays pill, and clamped-negative inputs floor at 1.5.
#[test]
fn ring_radius_follows_component_corner() {
    use crate::layout::ring_radius;
    assert_eq!(ring_radius(4.0), 5.5);
    assert_eq!(ring_radius(10.0), 11.5);
    assert!(ring_radius(9999.0) > 9000.0, "pill radius stays large");
    assert_eq!(ring_radius(-2.0), 1.5);
}

/// Invalid always wins the ring — one ring only, destructive over focus.
#[test]
fn ring_color_role_invalid_wins_over_focus() {
    use crate::layout::ring_color_role;
    use crate::theme::ColorRole;
    assert_eq!(ring_color_role(true), ColorRole::DestructiveRing);
    assert_eq!(ring_color_role(false), ColorRole::Focus);
}
