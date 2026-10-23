{
                    let mut canvas = D2dCanvas {
                        target: &target,
                        dark,
                        forced,
                        geo_stack: Vec::new(),
                        path_geos: Vec::new(),
                    };
                    render.paint(
                        &mut canvas,
                        crate::geom::Rect {
                            x: r.x,
                            y: r.y,
                            width: r.width,
                            height: r.height,
                        },
                    );
}
