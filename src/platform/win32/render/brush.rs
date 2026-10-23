use super::super::*;
use super::guards::{box_geometry, brush_color, resolve_render};
use super::*;
use crate::style::{BoxStyle, CornerRadii, Shadow};

/// Paint one resolved `BoxStyle` at `r` — shadow under, fill, then per-side
/// borders (later sides own shared corners, T<R<B<L).
pub(crate) fn paint_box(
    target: &ID2D1RenderTarget,
    r: &D2D_RECT_F,
    style: &BoxStyle,
    dark: bool,
    forced: Option<&dyn Fn(crate::style::SystemColor) -> [f32; 4]>,
    dpi: super::super::space::ScaleFactor,
    shadow_cache: &RefCell<ShadowCache>,
    target_gen: u64,
) -> Result<()> {
    // OS enforcement — forced colors map roles to the real system
    // palette and suppress decorative shadow (shared authority)
    let mut enforced;
    let style = if forced.is_some() {
        enforced = *style;
        crate::style::os_enforce_box(&mut enforced, forced);
        &enforced
    } else {
        style
    };
    unsafe {
        if let Some(sh) = &style.shadow {
            draw_shadow(
                target,
                r,
                &style.radii,
                sh,
                dark,
                dpi,
                shadow_cache,
                target_gen,
            )?;
        }
        let bg = resolve_color_f(style.background, dark);
        let geo = box_geometry(target, r, &style.radii)?;
        if bg.a > 0.0 {
            let b = target.CreateSolidColorBrush(&bg, None)?;
            target.FillGeometry(&geo, &b, None);
        }
        if style.border.any() {
            if let Some(side) = style.border.uniform() {
                if side.width.0 > 0.0 {
                    let c = resolve_render(side.color, dark, forced);
                    if c.a > 0.0 {
                        let b = target.CreateSolidColorBrush(&c, None)?;
                        // midline stroke: inset by half the width so the
                        // stroke lands exactly inside the outer edge
                        let mid = box_geometry(
                            target,
                            &D2D_RECT_F {
                                left: r.left + side.width.0 / 2.0,
                                top: r.top + side.width.0 / 2.0,
                                right: r.right - side.width.0 / 2.0,
                                bottom: r.bottom - side.width.0 / 2.0,
                            },
                            &shrink_radii(&style.radii, side.width.0 / 2.0),
                        )?;
                        target.DrawGeometry(&mid, &b, side.width.0, None);
                    }
                }
            } else {
                // non-uniform: each side painted as a strip clipped to the
                // rounded silhouette (PushLayer geometric mask). The
                // `mask` geometry stays owned — the params borrow a clone
                // that is released right after PopLayer (ManuallyDrop in
                // the field type is an ABI detail, not a leak license).
                let mask = box_geometry(target, r, &style.radii)?;
                // RAII-paired layer: EVERY fallible allocation happens
                // BEFORE PushLayer — a `?` between push/pop would strand
                // the layer and leak the params' borrowed mask clone.
                // The corner partition is the DIAGONAL bisector: each side
                // is a trapezoid whose inner edge runs between the
                // adjacent inner corners — corner cells split on the
                // diagonal, so arbitrarily thick opposing sides can never
                // overlap.
                let (mut lw, mut tw, mut rw, mut bw) = (
                    style.border.left.width.0.max(0.0),
                    style.border.top.width.0.max(0.0),
                    style.border.right.width.0.max(0.0),
                    style.border.bottom.width.0.max(0.0),
                );
                // BOUNDED — opposing insets whose sum exceeds the box
                // extent would cross; scale both so they exactly fill
                // (degenerate inner corner at the midline, never overlap)
                let (w, h) = (r.right - r.left, r.bottom - r.top);
                if lw + rw > w && w > 0.0 {
                    let k = w / (lw + rw);
                    lw *= k;
                    rw *= k;
                }
                if tw + bw > h && h > 0.0 {
                    let k = h / (tw + bw);
                    tw *= k;
                    bw *= k;
                }
                let trapezoids: [[Vector2; 4]; 4] = [
                    // top side: outer top edge, inner edge inset by lw/rw
                    [
                        Vector2 {
                            X: r.left,
                            Y: r.top,
                        },
                        Vector2 {
                            X: r.right,
                            Y: r.top,
                        },
                        Vector2 {
                            X: r.right - rw,
                            Y: r.top + tw,
                        },
                        Vector2 {
                            X: r.left + lw,
                            Y: r.top + tw,
                        },
                    ],
                    // right side: outer right edge, inner inset by tw/bw
                    [
                        Vector2 {
                            X: r.right,
                            Y: r.top,
                        },
                        Vector2 {
                            X: r.right,
                            Y: r.bottom,
                        },
                        Vector2 {
                            X: r.right - rw,
                            Y: r.bottom - bw,
                        },
                        Vector2 {
                            X: r.right - rw,
                            Y: r.top + tw,
                        },
                    ],
                    // bottom side
                    [
                        Vector2 {
                            X: r.right,
                            Y: r.bottom,
                        },
                        Vector2 {
                            X: r.left,
                            Y: r.bottom,
                        },
                        Vector2 {
                            X: r.left + lw,
                            Y: r.bottom - bw,
                        },
                        Vector2 {
                            X: r.right - rw,
                            Y: r.bottom - bw,
                        },
                    ],
                    // left side
                    [
                        Vector2 {
                            X: r.left,
                            Y: r.bottom,
                        },
                        Vector2 {
                            X: r.left,
                            Y: r.top,
                        },
                        Vector2 {
                            X: r.left + lw,
                            Y: r.top + tw,
                        },
                        Vector2 {
                            X: r.left + lw,
                            Y: r.bottom - bw,
                        },
                    ],
                ];
                let sides = [
                    style.border.top,
                    style.border.right,
                    style.border.bottom,
                    style.border.left,
                ];
                let mut parts_v: Vec<(ID2D1SolidColorBrush, ID2D1PathGeometry)> = Vec::new();
                for (i, side) in sides.iter().enumerate() {
                    if side.width.0 <= 0.0 {
                        continue;
                    }
                    let c = resolve_render(side.color, dark, forced);
                    if c.a <= 0.0 {
                        continue;
                    }
                    let b = target.CreateSolidColorBrush(&c, None)?;
                    let geo = target.GetFactory()?.CreatePathGeometry()?;
                    {
                        let sink = geo.Open()?;
                        sink.SetFillMode(D2D1_FILL_MODE_WINDING);
                        let p = trapezoids[i];
                        sink.BeginFigure(p[0], D2D1_FIGURE_BEGIN_FILLED);
                        sink.AddLines(&[p[1], p[2], p[3]]);
                        sink.EndFigure(D2D1_FIGURE_END_CLOSED);
                        sink.Close()?;
                    }
                    parts_v.push((b, geo));
                }
                // the mask clone is acquired ONLY after every fallible
                // allocation — a `?` before this point can no longer
                // strand a ManuallyDrop addref
                let mut params = D2D1_LAYER_PARAMETERS {
                    contentBounds: D2D_RECT_F {
                        left: f32::MIN,
                        top: f32::MIN,
                        right: f32::MAX,
                        bottom: f32::MAX,
                    },
                    geometricMask: std::mem::ManuallyDrop::new(Some(mask.clone())),
                    maskAntialiasMode: D2D1_ANTIALIAS_MODE_ALIASED,
                    maskTransform: Matrix3x2::identity(),
                    opacity: 1.0,
                    opacityBrush: std::mem::ManuallyDrop::new(None),
                    layerOptions: D2D1_LAYER_OPTIONS_NONE,
                };
                target.PushLayer(&params, None);
                for (b, geo) in &parts_v {
                    target.FillGeometry(geo, b, None);
                }
                target.PopLayer();
                // release the params' clone now that the layer is popped
                std::mem::ManuallyDrop::drop(&mut params.geometricMask);
            }
        }
    }
    Ok(())
}

pub(crate) fn shrink_radii(r: &CornerRadii, d: f32) -> CornerRadii {
    CornerRadii {
        top_left: crate::geom::Dp((r.top_left.0 - d).max(0.0)),
        top_right: crate::geom::Dp((r.top_right.0 - d).max(0.0)),
        bottom_right: crate::geom::Dp((r.bottom_right.0 - d).max(0.0)),
        bottom_left: crate::geom::Dp((r.bottom_left.0 - d).max(0.0)),
    }
}

/// Required focus indicator — painted OUTSIDE the box in the Focus role.
/// This is the enforcement layer: it runs at render after every style
/// layer resolved, so no consumer patch can erase focus visibility.
pub(crate) unsafe fn paint_focus_ring(
    target: &ID2D1RenderTarget,
    r: &D2D_RECT_F,
    radii: &crate::style::CornerRadii,
    dark: bool,
    forced: Option<&dyn Fn(crate::style::SystemColor) -> [f32; 4]>,
) -> Result<()> {
    paint_ring_color(
        target,
        r,
        radii,
        crate::style::Color::Role(crate::theme::ColorRole::Focus),
        dark,
        forced,
    )
}

/// Ring painter with an explicit authored color — the destructive variant
/// used by invalid controls. The corner radius follows the element's own
/// normalized radii at +1.5dp stroke centerline (`layout::ring_radius`)
/// — checkbox rings 5.5, inputs 11.5, pills stay pill.
pub(crate) unsafe fn paint_ring_color(
    target: &ID2D1RenderTarget,
    r: &D2D_RECT_F,
    radii: &crate::style::CornerRadii,
    color: crate::style::Color,
    dark: bool,
    forced: Option<&dyn Fn(crate::style::SystemColor) -> [f32; 4]>,
) -> Result<()> {
    paint_ring_color_width(target, r, radii, color, 3.0, dark, forced)
}

pub(crate) unsafe fn paint_ring_color_width(
    target: &ID2D1RenderTarget,
    r: &D2D_RECT_F,
    radii: &crate::style::CornerRadii,
    color: crate::style::Color,
    width: f32,
    dark: bool,
    forced: Option<&dyn Fn(crate::style::SystemColor) -> [f32; 4]>,
) -> Result<()> {
    unsafe {
        let ring = target.CreateSolidColorBrush(&resolve_render(color, dark, forced), None)?;
        let spread = width.max(0.0) / 2.0;
        let norm = radii.normalized(r.right - r.left, r.bottom - r.top);
        let radius = norm.top_left.0 + spread;
        let outer = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: r.left - spread,
                top: r.top - spread,
                right: r.right + spread,
                bottom: r.bottom + spread,
            },
            radiusX: radius,
            radiusY: radius,
        };
        let og: ID2D1RoundedRectangleGeometry = target
            .GetFactory()?
            .CreateRoundedRectangleGeometry(&outer)?;
        target.DrawGeometry(&og, &ring, width.max(0.0), None);
    }
    Ok(())
}
