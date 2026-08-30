use super::prelude::*;

/// CPU tessellation output. It deliberately contains no GPU objects so the
/// renderer may cache/upload it independently of Incular's Path type.
#[derive(Clone, Debug, Default)]
pub struct PathMesh {
    pub vertices: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}
fn lyon_path(path: &incular_painting::Path) -> LyonPath {
    let mut b = LyonPath::builder();
    let mut open = false;
    for v in path.bez_path().elements() {
        match *v {
            PathEl::MoveTo(p) => {
                if open {
                    b.end(false);
                }
                b.begin(point(p.x as f32, p.y as f32));
                open = true;
            }
            PathEl::LineTo(p) => {
                b.line_to(point(p.x as f32, p.y as f32));
            }
            PathEl::QuadTo(c, p) => {
                b.quadratic_bezier_to(point(c.x as f32, c.y as f32), point(p.x as f32, p.y as f32));
            }
            PathEl::CurveTo(a, c, p) => {
                b.cubic_bezier_to(
                    point(a.x as f32, a.y as f32),
                    point(c.x as f32, c.y as f32),
                    point(p.x as f32, p.y as f32),
                );
            }
            PathEl::ClosePath => {
                b.close();
                open = false;
            }
        }
    }
    if open {
        b.end(false);
    }
    b.build()
}
/// Translates Kurbo Bézier elements into Lyon events without flattening.
pub fn tessellate_path(
    path: &incular_painting::Path,
    fill_rule: incular_painting::FillRule,
    stroke: Option<incular_painting::Stroke>,
) -> Option<PathMesh> {
    let p = lyon_path(path);
    // Lyon's compact simple builder emits u16 indices. Convert at the renderer
    // boundary so GPU draws use u32 and never wrap if a mesh approaches Lyon's
    // builder limit (Lyon returns an error instead of overflowing).
    let mut out: VertexBuffers<lyon_tessellation::math::Point, u16> = VertexBuffers::new();
    if let Some(s) = stroke {
        let mut tess = StrokeTessellator::new();
        let o = StrokeOptions::default()
            .with_line_width(s.width.max(0.))
            .with_miter_limit(s.miter_limit.max(1.01))
            .with_line_cap(match s.cap {
                LineCap::Butt => lyon_tessellation::LineCap::Butt,
                LineCap::Round => lyon_tessellation::LineCap::Round,
                LineCap::Square => lyon_tessellation::LineCap::Square,
            })
            .with_line_join(match s.join {
                LineJoin::Miter => lyon_tessellation::LineJoin::Miter,
                LineJoin::Round => lyon_tessellation::LineJoin::Round,
                LineJoin::Bevel => lyon_tessellation::LineJoin::Bevel,
            });
        tess.tessellate_path(&p, &o, &mut simple_builder(&mut out))
            .ok()?;
    } else {
        let mut tess = FillTessellator::new();
        let o = FillOptions::default().with_fill_rule(match fill_rule {
            incular_painting::FillRule::NonZero => LyonFillRule::NonZero,
            incular_painting::FillRule::EvenOdd => LyonFillRule::EvenOdd,
        });
        tess.tessellate_path(&p, &o, &mut simple_builder(&mut out))
            .ok()?;
    }
    Some(PathMesh {
        vertices: out.vertices.into_iter().map(|p| [p.x, p.y]).collect(),
        indices: out.indices.into_iter().map(u32::from).collect(),
    })
}
