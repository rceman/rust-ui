use super::super::*;
use super::*;

/// Pops one axis-aligned clip on drop — scroll viewports confine every
/// descendant's paint; unwinding (a `?` inside a draw arm) must never
/// leave a stale clip on the shared target.
pub(crate) struct ClipGuard<'a>(pub(crate) &'a ID2D1RenderTarget);

impl Drop for ClipGuard<'_> {
    fn drop(&mut self) {
        unsafe { self.0.PopAxisAlignedClip() };
    }
}

/// Pops one pushed layer on drop — same stranded-state contract as
/// ClipGuard: a `?` inside the arm must never leave a 0.5-opacity (or
/// masked) layer on the shared target for the rest of the frame.
pub(crate) struct LayerGuard<'a>(pub(crate) &'a ID2D1RenderTarget);

impl Drop for LayerGuard<'_> {
    fn drop(&mut self) {
        unsafe { self.0.PopLayer() };
    }
}

// ---------------------------------------------------------------------------
// Canvas — custom node paint over the D2D target
// ---------------------------------------------------------------------------

pub(crate) struct D2dCanvas<'a> {
    pub(crate) target: &'a ID2D1RenderTarget,
    pub(crate) dark: bool,
    /// forced-colors resolver — canvas fill/text roles are NOT exempt
    pub(crate) forced: Option<&'a dyn Fn(crate::style::SystemColor) -> [f32; 4]>,
    /// built geometries kept alive for the paint call's borrow scope
    pub(crate) geo_stack: Vec<ID2D1PathGeometry>,
    pub(crate) path_geos: Vec<ID2D1Geometry>,
}

impl Canvas for D2dCanvas<'_> {
    fn path(&mut self, path: &crate::ui::Path2d, paint: Paint) {
        unsafe {
            let Ok(factory) = self.target.GetFactory() else {
                return;
            };
            let Ok(geo) = factory.CreatePathGeometry() else {
                return;
            };
            let Ok(sink) = geo.Open() else {
                return;
            };
            sink.SetFillMode(D2D1_FILL_MODE_WINDING);
            let mut fig_open = false;
            for op in &path.ops {
                match op {
                    PathOp::MoveTo(p) => {
                        if fig_open {
                            sink.EndFigure(D2D1_FIGURE_END_OPEN);
                        }
                        sink.BeginFigure(Vector2 { X: p.x, Y: p.y }, D2D1_FIGURE_BEGIN_FILLED);
                        fig_open = true;
                    }
                    PathOp::LineTo(p) => {
                        if fig_open {
                            sink.AddLine(Vector2 { X: p.x, Y: p.y });
                        }
                    }
                    PathOp::QuadTo(c, p) => {
                        if fig_open {
                            sink.AddQuadraticBezier(&D2D1_QUADRATIC_BEZIER_SEGMENT {
                                point1: Vector2 { X: c.x, Y: c.y },
                                point2: Vector2 { X: p.x, Y: p.y },
                            });
                        }
                    }
                    PathOp::Close => {
                        if fig_open {
                            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
                            fig_open = false;
                        }
                    }
                }
            }
            if fig_open {
                sink.EndFigure(D2D1_FIGURE_END_OPEN);
            }
            if sink.Close().is_err() {
                return;
            }
            let color = match paint {
                Paint::FillRole(r) => {
                    resolve_render(crate::style::Color::Role(r), self.dark, self.forced)
                }
                Paint::Rgba(r, g, b, a) => D2D1_COLOR_F {
                    r: r as f32 / 255.0,
                    g: g as f32 / 255.0,
                    b: b as f32 / 255.0,
                    a: a as f32 / 255.0,
                },
            };
            if let Ok(brush) = self.target.CreateSolidColorBrush(&color, None) {
                self.target.FillGeometry(&geo, &brush, None);
            }
        }
    }
    fn text(&mut self, run: &TextRun, origin: Point) {
        let Ok(d) = dwrite() else { return };
        let Some(fmt) = d.formats[0].as_ref() else {
            return;
        };
        let wide: Vec<u16> = run.text.as_ref().encode_utf16().collect();
        unsafe {
            let Ok(lay) = d.factory.CreateTextLayout(&wide, fmt, f32::MAX, f32::MAX) else {
                return;
            };
            let c = resolve_render(
                crate::style::Color::Role(run.color_role),
                self.dark,
                self.forced,
            );
            if let Ok(brush) = self.target.CreateSolidColorBrush(&c, None) {
                self.target.DrawTextLayout(
                    Vector2 {
                        X: origin.x,
                        Y: origin.y,
                    },
                    &lay,
                    &brush,
                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                );
            }
        }
    }
}
// ---------------------------------------------------------------------------
// paint_box — the shared box painter (background + per-corner radii +
// per-side borders + one outer shadow). Every painted component resolves to
// a `BoxStyle` and lands here — no private parallel renderers.
// ---------------------------------------------------------------------------

use crate::style::{BoxStyle, CornerRadii, Shadow};
use windows_numerics::Matrix3x2;

/// Resolve an authored `Color` into the target's float4.
/// Color resolution under forced colors: roles go through the platform's
/// system-color lookup; authored literals stay authored (an app that names
/// an exact RGBA under HC means that RGBA).
pub(crate) fn resolve_render(
    c: crate::style::Color,
    dark: bool,
    sys: Option<&dyn Fn(crate::style::SystemColor) -> [f32; 4]>,
) -> D2D1_COLOR_F {
    if let (crate::style::Color::Role(r), Some(f)) = (c, sys) {
        let [rr, gg, bb, aa] = f(crate::style::system_slot(r));
        return D2D1_COLOR_F {
            r: rr,
            g: gg,
            b: bb,
            a: aa,
        };
    }
    brush_color(c, dark)
}

pub(crate) fn brush_color(c: crate::style::Color, dark: bool) -> D2D1_COLOR_F {
    let [r, g, b, a] = crate::style::resolve_color(c, dark);
    D2D1_COLOR_F { r, g, b, a }
}

/// Rounded-rect geometry with per-corner radii. Uniform radii use the D2D
/// rounded-rect geometry; mixed corners build an edge+arc path.
pub(crate) unsafe fn box_geometry(
    target: &ID2D1RenderTarget,
    r: &D2D_RECT_F,
    radii: &CornerRadii,
) -> Result<ID2D1Geometry> {
    unsafe {
        let f = target.GetFactory()?;
        // proportional radius normalization — adjacent pairs sharing an
        // edge scale down by ONE factor (CSS rule); independent per-corner
        // clamps distort authored proportions
        let norm = radii.normalized(r.right - r.left, r.bottom - r.top);
        if let Some(crate::geom::Dp(u)) = norm.uniform() {
            let g = f.CreateRoundedRectangleGeometry(&D2D1_ROUNDED_RECT {
                rect: *r,
                radiusX: u,
                radiusY: u,
            })?;
            return Ok(g.cast()?);
        }
        let g = f.CreatePathGeometry()?;
        let sink = g.Open()?;
        sink.SetFillMode(D2D1_FILL_MODE_WINDING);
        let (tl, tr, br, bl) = (
            norm.top_left.0,
            norm.top_right.0,
            norm.bottom_right.0,
            norm.bottom_left.0,
        );
        sink.BeginFigure(
            Vector2 {
                X: r.left + tl,
                Y: r.top,
            },
            D2D1_FIGURE_BEGIN_FILLED,
        );
        sink.AddLine(Vector2 {
            X: r.right - tr,
            Y: r.top,
        });
        if tr > 0.0 {
            sink.AddArc(&D2D1_ARC_SEGMENT {
                point: Vector2 {
                    X: r.right,
                    Y: r.top + tr,
                },
                size: D2D_SIZE_F {
                    width: tr,
                    height: tr,
                },
                rotationAngle: 0.0,
                sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                arcSize: D2D1_ARC_SIZE_SMALL,
            });
        }
        sink.AddLine(Vector2 {
            X: r.right,
            Y: r.bottom - br,
        });
        if br > 0.0 {
            sink.AddArc(&D2D1_ARC_SEGMENT {
                point: Vector2 {
                    X: r.right - br,
                    Y: r.bottom,
                },
                size: D2D_SIZE_F {
                    width: br,
                    height: br,
                },
                rotationAngle: 0.0,
                sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                arcSize: D2D1_ARC_SIZE_SMALL,
            });
        }
        sink.AddLine(Vector2 {
            X: r.left + bl,
            Y: r.bottom,
        });
        if bl > 0.0 {
            sink.AddArc(&D2D1_ARC_SEGMENT {
                point: Vector2 {
                    X: r.left,
                    Y: r.bottom - bl,
                },
                size: D2D_SIZE_F {
                    width: bl,
                    height: bl,
                },
                rotationAngle: 0.0,
                sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                arcSize: D2D1_ARC_SIZE_SMALL,
            });
        }
        sink.AddLine(Vector2 {
            X: r.left,
            Y: r.top + tl,
        });
        if tl > 0.0 {
            sink.AddArc(&D2D1_ARC_SEGMENT {
                point: Vector2 {
                    X: r.left + tl,
                    Y: r.top,
                },
                size: D2D_SIZE_F {
                    width: tl,
                    height: tl,
                },
                rotationAngle: 0.0,
                sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                arcSize: D2D1_ARC_SIZE_SMALL,
            });
        }
        sink.EndFigure(D2D1_FIGURE_END_CLOSED);
        sink.Close()?;
        Ok(g.cast()?)
    }
}

// ---------------------------------------------------------------------------
// bounded shadow cache — one shared raster budget across all shadows
// ---------------------------------------------------------------------------
