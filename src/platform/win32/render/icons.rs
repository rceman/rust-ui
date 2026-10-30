use super::super::*;
use super::guards::{box_geometry, brush_color, resolve_render};
use super::*;
use crate::style::{BoxStyle, CornerRadii, Shadow};

pub(crate) fn inherited_action_ink<S, M, U, V>(
    be: &Backend<S, M, U, V>,
    node: NodeId,
    n: &crate::node::Node,
) -> Option<crate::theme::ColorRole>
where
    M: 'static,
    U: Fn(&mut S, M, &mut UpdateCtx<'_, M>),
    V: Fn(&S, &mut crate::Ui<'_, '_, M>),
{
    let mut parent = n.parent;
    while let Some(slot) = parent {
        let action_id = NodeId {
            slot,
            generation: be.rt.arena.generation_of(slot),
        };
        let action = be.rt.arena.get(action_id)?;
        if let crate::node::NodeData::Action { disabled, .. } = &action.data
            && !disabled
            && (be.resolved_hot(action_id, action) || be.resolved_focus_visible(action_id, action))
        {
            return match action.role {
                Some(
                    crate::node::SemanticRole::MenuItem
                    | crate::node::SemanticRole::MenuItemCheckbox
                    | crate::node::SemanticRole::MenuItemRadio,
                ) => Some(crate::theme::ColorRole::AccentForeground),
                Some(crate::node::SemanticRole::SettingsNavItem) => {
                    Some(crate::theme::ColorRole::SidebarAccentForeground)
                }
                _ => None,
            };
        }
        parent = action.parent;
    }
    let _ = node;
    None
}

pub(crate) fn inherited_action_disabled<S, M, U, V>(
    be: &Backend<S, M, U, V>,
    n: &crate::node::Node,
) -> bool
where
    M: 'static,
    U: Fn(&mut S, M, &mut UpdateCtx<'_, M>),
    V: Fn(&S, &mut crate::Ui<'_, '_, M>),
{
    let mut parent = n.parent;
    while let Some(slot) = parent {
        let Some(action) = be.rt.arena.slot(slot) else {
            return false;
        };
        if matches!(
            &action.data,
            crate::node::NodeData::Action { disabled: true, .. }
        ) {
            return true;
        }
        parent = action.parent;
    }
    false
}

/// The DIP footprint a shadow's ink can reach — the box rect offset by
/// (dx,dy) and inflated by the blur pad (3σ covers the spread). This is
/// the per-node damage contribution for shadow changes: old and new rects
/// union through it.
pub(crate) fn shadow_ink_rect(
    r: &super::super::space::LogicalRect,
    s: &Shadow,
) -> super::super::space::LogicalRect {
    let pad = (s.blur_sigma.0.max(0.0) * 3.0 + s.spread.0.max(0.0)).max(1.0) + 1.0;
    super::super::space::LogicalRect {
        x: r.x + s.offset_x.0 - pad,
        y: r.y + s.offset_y.0 - pad,
        width: r.width + 2.0 * pad,
        height: r.height + 2.0 * pad,
    }
}

pub(crate) fn resolve_color_f(c: crate::style::Color, dark: bool) -> D2D1_COLOR_F {
    let [r, g, b, a] = crate::style::resolve_color(c, dark);
    D2D1_COLOR_F { r, g, b, a }
}

// ---------------------------------------------------------------------------
// Lucide icons — stroke vector geometry, scaled from the 24x24 viewBox
// ---------------------------------------------------------------------------

/// Stroke an icon's `IconOp` list at a target rect. Geometry scales
/// `size/24`; stroke width follows Lucide (2 user units at 24) so a 16dp
/// icon strokes 1.33dp — matching the frozen specimens.
pub(crate) unsafe fn draw_icon_at(
    target: &ID2D1RenderTarget,
    factory: &ID2D1Factory,
    icon: &crate::icons::Icon,
    r: &crate::geom::Rect,
    brush: &ID2D1SolidColorBrush,
) -> UiResult {
    let scale = r.width / 24.0;
    let geo = factory.CreatePathGeometry()?;
    {
        let sink = geo.Open()?;
        let mut open = false;
        for op in icon.path() {
            match *op {
                crate::icons::IconOp::MoveTo(x, y) => {
                    if open {
                        sink.EndFigure(D2D1_FIGURE_END_OPEN);
                    }
                    sink.BeginFigure(
                        Vector2 {
                            X: r.x + x * scale,
                            Y: r.y + y * scale,
                        },
                        D2D1_FIGURE_BEGIN_HOLLOW,
                    );
                    open = true;
                }
                crate::icons::IconOp::LineTo(x, y) => {
                    if open {
                        sink.AddLine(Vector2 {
                            X: r.x + x * scale,
                            Y: r.y + y * scale,
                        });
                    }
                }
                crate::icons::IconOp::QuadTo(cx, cy, x, y) => {
                    if open {
                        sink.AddQuadraticBezier(&D2D1_QUADRATIC_BEZIER_SEGMENT {
                            point1: Vector2 {
                                X: r.x + cx * scale,
                                Y: r.y + cy * scale,
                            },
                            point2: Vector2 {
                                X: r.x + x * scale,
                                Y: r.y + y * scale,
                            },
                        });
                    }
                }
                crate::icons::IconOp::CubicTo(cx1, cy1, cx2, cy2, x, y) => {
                    if open {
                        sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                            point1: Vector2 {
                                X: r.x + cx1 * scale,
                                Y: r.y + cy1 * scale,
                            },
                            point2: Vector2 {
                                X: r.x + cx2 * scale,
                                Y: r.y + cy2 * scale,
                            },
                            point3: Vector2 {
                                X: r.x + x * scale,
                                Y: r.y + y * scale,
                            },
                        });
                    }
                }
                crate::icons::IconOp::Close => {
                    if open {
                        sink.EndFigure(D2D1_FIGURE_END_CLOSED);
                        open = false;
                    }
                }
                crate::icons::IconOp::ArcTo {
                    rx,
                    ry,
                    x_axis_rotation,
                    large_arc,
                    sweep,
                    x,
                    y,
                } => {
                    // AddArc outside an open figure trips
                    // D2DERR_WRONG_STATE — one malformed icon must not
                    // fail the whole frame; skip continuation ops until a
                    // figure is open
                    if open {
                        sink.AddArc(&D2D1_ARC_SEGMENT {
                            point: Vector2 {
                                X: r.x + x * scale,
                                Y: r.y + y * scale,
                            },
                            size: D2D_SIZE_F {
                                width: rx * scale,
                                height: ry * scale,
                            },
                            rotationAngle: x_axis_rotation,
                            sweepDirection: if sweep {
                                D2D1_SWEEP_DIRECTION_CLOCKWISE
                            } else {
                                D2D1_SWEEP_DIRECTION_COUNTER_CLOCKWISE
                            },
                            arcSize: if large_arc {
                                D2D1_ARC_SIZE_LARGE
                            } else {
                                D2D1_ARC_SIZE_SMALL
                            },
                        });
                    }
                }
                crate::icons::IconOp::Circle { cx, cy, r: rad } => {
                    if open {
                        sink.EndFigure(D2D1_FIGURE_END_OPEN);
                        open = false;
                    }
                    sink.BeginFigure(
                        Vector2 {
                            X: r.x + (cx + rad) * scale,
                            Y: r.y + cy * scale,
                        },
                        D2D1_FIGURE_BEGIN_HOLLOW,
                    );
                    // two 180° arcs make the circle without ellipse ops
                    sink.AddArc(&D2D1_ARC_SEGMENT {
                        point: Vector2 {
                            X: r.x + (cx - rad) * scale,
                            Y: r.y + cy * scale,
                        },
                        size: D2D_SIZE_F {
                            width: rad * scale,
                            height: rad * scale,
                        },
                        rotationAngle: 0.0,
                        sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                        arcSize: D2D1_ARC_SIZE_LARGE,
                    });
                    sink.AddArc(&D2D1_ARC_SEGMENT {
                        point: Vector2 {
                            X: r.x + (cx + rad) * scale,
                            Y: r.y + cy * scale,
                        },
                        size: D2D_SIZE_F {
                            width: rad * scale,
                            height: rad * scale,
                        },
                        rotationAngle: 0.0,
                        sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                        arcSize: D2D1_ARC_SIZE_LARGE,
                    });
                    sink.EndFigure(D2D1_FIGURE_END_CLOSED);
                }
                crate::icons::IconOp::RoundRect { x, y, w, h, r: rad } => {
                    if open {
                        sink.EndFigure(D2D1_FIGURE_END_OPEN);
                        open = false;
                    }
                    // separate geometry — a path sink can't round corners
                    let g2 = factory.CreateRoundedRectangleGeometry(&D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: r.x + x * scale,
                            top: r.y + y * scale,
                            right: r.x + (x + w) * scale,
                            bottom: r.y + (y + h) * scale,
                        },
                        radiusX: rad * scale,
                        radiusY: rad * scale,
                    })?;
                    target.DrawGeometry(&g2, brush, 2.0 * scale, None);
                }
            }
        }
        if open {
            sink.EndFigure(D2D1_FIGURE_END_OPEN);
        }
        sink.Close()?;
    }
    let style = factory.CreateStrokeStyle(
        &D2D1_STROKE_STYLE_PROPERTIES {
            startCap: D2D1_CAP_STYLE_ROUND,
            endCap: D2D1_CAP_STYLE_ROUND,
            dashCap: D2D1_CAP_STYLE_ROUND,
            lineJoin: D2D1_LINE_JOIN_ROUND,
            ..Default::default()
        },
        None,
    )?;
    target.DrawGeometry(&geo, brush, 2.0 * scale, &style);
    Ok(())
}

/// Standalone icon leaf — the icon fills its layout rect.
pub(crate) unsafe fn draw_icon(
    target: &ID2D1RenderTarget,
    factory: &ID2D1Factory,
    icon: &crate::icons::Icon,
    _size: crate::geom::Dp,
    r: &super::super::space::LogicalRect,
    brush: &ID2D1SolidColorBrush,
) -> UiResult {
    draw_icon_at(
        target,
        factory,
        icon,
        &crate::geom::Rect {
            x: r.x,
            y: r.y,
            width: r.width,
            height: r.height,
        },
        brush,
    )
}
