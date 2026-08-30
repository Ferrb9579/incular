use super::prelude::*;
use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ClipRect {
    pub(crate) rect: Rect,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum ClipState {
    Unbounded,
    Rect(ClipRect),
    /// `rect` remains the conservative scissor while `depth` is the exact
    /// nested non-rectangular stencil membership required by content.
    Stencil {
        rect: Option<ClipRect>,
        depth: u8,
    },
    Empty,
}
#[derive(Clone, Copy, Debug)]
pub(crate) enum ClipMask {
    RRect(GpuRRectInstance),
    Path {
        key: PathMeshKey,
        instance: GpuPathInstance,
    },
}
#[derive(Clone, Debug)]
pub(crate) enum DrawBatch {
    Rectangles {
        clip: ClipState,
        instances: Vec<RectangleInstance>,
    },
    Glyphs {
        page: u16,
        clip: ClipState,
        instances: Vec<GpuGlyphInstance>,
    },
    Images {
        image: ImageId,
        sampling: ImageSampling,
        clip: ClipState,
        instances: Vec<GpuImageInstance>,
    },
    RoundedRects {
        clip: ClipState,
        gradient: Option<GradientId>,
        instances: Vec<GpuRRectInstance>,
    },
    /// Kept as individual commands so cached geometry can interleave with all
    /// other painter operations without global pipeline reordering.
    Path {
        clip: ClipState,
        key: PathMeshKey,
        gradient: Option<GradientId>,
        instance: GpuPathInstance,
    },
    StencilRRect {
        clip: ClipState,
        instance: GpuRRectInstance,
        increment: bool,
    },
    StencilPath {
        clip: ClipState,
        key: PathMeshKey,
        instance: GpuPathInstance,
        increment: bool,
    },
    Offscreen {
        clip: ClipState,
        layer: incular_painting::LayerId,
        instance: GpuCompositeInstance,
    },
    Filtered {
        clip: ClipState,
        layer: incular_painting::LayerId,
        instance: GpuCompositeInstance,
    },
    Shadow {
        clip: ClipState,
        layer: incular_painting::LayerId,
        instance: GpuCompositeInstance,
    },
    Blend {
        clip: ClipState,
        layer: incular_painting::LayerId,
        mode: BlendMode,
        instance: GpuCompositeInstance,
    },
}
#[derive(Clone, Copy)]
pub(crate) struct GlyphSurface {
    pub(crate) transform: Transform,
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) scale: f32,
}
#[derive(Clone, Copy)]
pub(crate) struct PathPlacement {
    pub(crate) transform: Transform,
    pub(crate) scale: f32,
}
pub(crate) fn stroke_mesh_kind(stroke: Stroke) -> PathMeshKind {
    PathMeshKind::Stroke {
        width: stroke.width.max(0.).to_bits(),
        cap: stroke.cap,
        join: stroke.join,
        miter_limit: stroke.miter_limit.max(1.01).to_bits(),
    }
}
pub(crate) fn rect_path(rect: Rect) -> Path {
    let mut builder = Path::builder();
    builder
        .move_to(rect.origin)
        .line_to(Offset::new(rect.origin.x + rect.size.width, rect.origin.y))
        .line_to(Offset::new(
            rect.origin.x + rect.size.width,
            rect.origin.y + rect.size.height,
        ))
        .line_to(Offset::new(rect.origin.x, rect.origin.y + rect.size.height))
        .close();
    builder.build()
}
pub(crate) fn oval_path(rect: Rect) -> Path {
    const KAPPA: f32 = 0.552_284_8;
    let rx = rect.size.width * 0.5;
    let ry = rect.size.height * 0.5;
    let cx = rect.origin.x + rx;
    let cy = rect.origin.y + ry;
    let mut builder = Path::builder();
    builder
        .move_to(Offset::new(cx + rx, cy))
        .cubic_to(
            Offset::new(cx + rx, cy + KAPPA * ry),
            Offset::new(cx + KAPPA * rx, cy + ry),
            Offset::new(cx, cy + ry),
        )
        .cubic_to(
            Offset::new(cx - KAPPA * rx, cy + ry),
            Offset::new(cx - rx, cy + KAPPA * ry),
            Offset::new(cx - rx, cy),
        )
        .cubic_to(
            Offset::new(cx - rx, cy - KAPPA * ry),
            Offset::new(cx - KAPPA * rx, cy - ry),
            Offset::new(cx, cy - ry),
        )
        .cubic_to(
            Offset::new(cx + KAPPA * rx, cy - ry),
            Offset::new(cx + rx, cy - KAPPA * ry),
            Offset::new(cx + rx, cy),
        )
        .close();
    builder.build()
}
pub(crate) fn path_visible(path: &Path, transform: Transform, clip: ClipState) -> bool {
    let Some(bounds) = path.bounds() else {
        return false;
    };
    let world = transform.transform_rect_bbox(bounds);
    match clip {
        ClipState::Unbounded => true,
        ClipState::Rect(clip) => intersect_rect(world, clip.rect).is_some(),
        ClipState::Stencil { rect, .. } => {
            rect.is_none_or(|clip| intersect_rect(world, clip.rect).is_some())
        }
        ClipState::Empty => false,
    }
}
pub(crate) fn stencil_depth(clip: ClipState) -> u8 {
    match clip {
        ClipState::Stencil { depth, .. } => depth,
        _ => 0,
    }
}
pub(crate) fn with_stencil_depth(clip: ClipState, depth: u8) -> ClipState {
    match clip {
        ClipState::Unbounded => ClipState::Stencil { rect: None, depth },
        ClipState::Rect(rect) => ClipState::Stencil {
            rect: Some(rect),
            depth,
        },
        ClipState::Stencil { rect, .. } => ClipState::Stencil { rect, depth },
        ClipState::Empty => ClipState::Empty,
    }
}
pub(crate) fn intersect_clip_with_rect(clip: ClipState, next: ClipRect) -> ClipState {
    match clip {
        ClipState::Unbounded => ClipState::Rect(next),
        ClipState::Rect(current) => intersect_rect(current.rect, next.rect)
            .map(|rect| ClipState::Rect(ClipRect { rect }))
            .unwrap_or(ClipState::Empty),
        ClipState::Stencil { rect: None, depth } => ClipState::Stencil {
            rect: Some(next),
            depth,
        },
        ClipState::Stencil {
            rect: Some(current),
            depth,
        } => intersect_rect(current.rect, next.rect)
            .map(|rect| ClipState::Stencil {
                rect: Some(ClipRect { rect }),
                depth,
            })
            .unwrap_or(ClipState::Empty),
        ClipState::Empty => ClipState::Empty,
    }
}

pub(crate) fn batches_have_content(batches: &[DrawBatch]) -> bool {
    batches.iter().any(|batch| match batch {
        DrawBatch::Rectangles { clip, instances } if *clip != ClipState::Empty => {
            !instances.is_empty()
        }
        DrawBatch::RoundedRects {
            clip, instances, ..
        } if *clip != ClipState::Empty => !instances.is_empty(),
        DrawBatch::Glyphs {
            clip, instances, ..
        } if *clip != ClipState::Empty => !instances.is_empty(),
        DrawBatch::Images {
            clip, instances, ..
        } if *clip != ClipState::Empty => !instances.is_empty(),
        DrawBatch::Path { clip, .. }
        | DrawBatch::Offscreen { clip, .. }
        | DrawBatch::Filtered { clip, .. }
        | DrawBatch::Shadow { clip, .. }
        | DrawBatch::Blend { clip, .. }
            if *clip != ClipState::Empty =>
        {
            true
        }
        DrawBatch::StencilRRect { .. } | DrawBatch::StencilPath { .. } => false,
        _ => false,
    })
}
pub(crate) fn count_rectangles(batches: &[DrawBatch]) -> usize {
    batches
        .iter()
        .map(|batch| match batch {
            DrawBatch::Rectangles { clip, instances } if *clip != ClipState::Empty => {
                instances.len()
            }
            _ => 0,
        })
        .sum()
}
pub(crate) fn count_glyphs(batches: &[DrawBatch]) -> usize {
    batches
        .iter()
        .map(|batch| match batch {
            DrawBatch::Glyphs {
                clip, instances, ..
            } if *clip != ClipState::Empty => instances.len(),
            _ => 0,
        })
        .sum()
}
pub(crate) fn count_images(batches: &[DrawBatch]) -> usize {
    batches
        .iter()
        .map(|batch| match batch {
            DrawBatch::Images {
                clip, instances, ..
            } if *clip != ClipState::Empty => instances.len(),
            _ => 0,
        })
        .sum()
}
pub(crate) fn count_rounded(batches: &[DrawBatch]) -> usize {
    batches
        .iter()
        .map(|batch| match batch {
            DrawBatch::RoundedRects {
                clip, instances, ..
            } if *clip != ClipState::Empty => instances.len(),
            DrawBatch::StencilRRect { clip, .. } if *clip != ClipState::Empty => 1,
            _ => 0,
        })
        .sum()
}
pub(crate) fn count_paths(batches: &[DrawBatch]) -> usize {
    batches
        .iter()
        .filter(|batch| {
            matches!(
                batch,
                DrawBatch::Path { clip, .. } | DrawBatch::StencilPath { clip, .. }
                    if *clip != ClipState::Empty
            )
        })
        .count()
}
pub(crate) fn count_composites(batches: &[DrawBatch]) -> usize {
    batches
        .iter()
        .filter(|batch| {
            matches!(
                batch,
                    DrawBatch::Offscreen { clip, .. }
                        | DrawBatch::Filtered { clip, .. }
                    | DrawBatch::Shadow { clip, .. }
                    | DrawBatch::Blend { clip, .. }
                    if *clip != ClipState::Empty
            )
        })
        .count()
}
#[must_use]
pub(crate) fn choose_blur_downsample_factor(sigma_physical: f32) -> u32 {
    let mut factor = 1_u32;
    let mut effective = normalize_sigma(sigma_physical);
    while effective > LARGE_BLUR_SIGMA_THRESHOLD && factor < 64 {
        factor *= 2;
        effective = sigma_physical / factor as f32;
    }
    factor
}
#[must_use]
pub(crate) fn quantize_blur_sigma(sigma: f32) -> f32 {
    let sigma = normalize_sigma(sigma);
    if sigma <= f32::EPSILON {
        return 0.;
    }
    (sigma / BLUR_KERNEL_QUANTUM).round() * BLUR_KERNEL_QUANTUM
}
pub(crate) fn append_rectangle(
    batches: &mut Vec<DrawBatch>,
    clip: ClipState,
    instance: RectangleInstance,
) {
    if let Some(DrawBatch::Rectangles {
        clip: old,
        instances,
    }) = batches.last_mut()
        && *old == clip
    {
        instances.push(instance);
    } else {
        batches.push(DrawBatch::Rectangles {
            clip,
            instances: vec![instance],
        });
    }
}
pub(crate) fn append_glyph(
    batches: &mut Vec<DrawBatch>,
    clip: ClipState,
    page: u16,
    instance: GpuGlyphInstance,
) {
    if let Some(DrawBatch::Glyphs {
        page: old_page,
        clip: old_clip,
        instances,
    }) = batches.last_mut()
        && *old_page == page
        && *old_clip == clip
    {
        instances.push(instance);
    } else {
        batches.push(DrawBatch::Glyphs {
            page,
            clip,
            instances: vec![instance],
        });
    }
}
pub(crate) fn append_image(
    batches: &mut Vec<DrawBatch>,
    clip: ClipState,
    image: ImageId,
    sampling: ImageSampling,
    instance: GpuImageInstance,
) {
    if let Some(DrawBatch::Images {
        image: old_image,
        sampling: old_sampling,
        clip: old_clip,
        instances,
    }) = batches.last_mut()
        && *old_image == image
        && *old_sampling == sampling
        && *old_clip == clip
    {
        instances.push(instance);
    } else {
        batches.push(DrawBatch::Images {
            image,
            sampling,
            clip,
            instances: vec![instance],
        });
    }
}
pub(crate) fn append_rrect(
    batches: &mut Vec<DrawBatch>,
    clip: ClipState,
    gradient: Option<GradientId>,
    instance: GpuRRectInstance,
) {
    if let Some(DrawBatch::RoundedRects {
        clip: old,
        gradient: old_gradient,
        instances,
    }) = batches.last_mut()
        && *old == clip
        && *old_gradient == gradient
    {
        instances.push(instance);
    } else {
        batches.push(DrawBatch::RoundedRects {
            clip,
            gradient,
            instances: vec![instance],
        });
    }
}
pub(crate) fn gradient_id(brush: &Brush) -> Option<GradientId> {
    match brush {
        Brush::Solid(_) => None,
        Brush::LinearGradient(g) => Some(g.stops.id()),
        Brush::RadialGradient(g) => Some(g.stops.id()),
        Brush::SweepGradient(g) => Some(g.stops.id()),
    }
}
pub(crate) fn path_paint(brush: &Brush) -> (Color, [f32; 4], [f32; 4]) {
    match brush {
        Brush::Solid(color) => (*color, [0.; 4], [0.; 4]),
        Brush::LinearGradient(g) => (
            Color::TRANSPARENT,
            [g.start.x, g.start.y, g.end.x, g.end.y],
            [1., 0., 0., 0.],
        ),
        Brush::RadialGradient(g) => (
            Color::TRANSPARENT,
            [g.center.x, g.center.y, g.radius.max(0.), 0.],
            [2., 0., 0., 0.],
        ),
        Brush::SweepGradient(g) => (
            Color::TRANSPARENT,
            [g.center.x, g.center.y, g.start_angle, 0.],
            [3., 0., 0., 0.],
        ),
    }
}
pub(crate) fn clip_path_instance(
    transform: Transform,
    scale: f32,
    width: f32,
    height: f32,
) -> GpuPathInstance {
    GpuPathInstance {
        affine: physical_affine(transform, scale).0,
        translation: physical_affine(transform, scale).1,
        surface: [width, height, 0., 0.],
        color: Color::TRANSPARENT.to_linear_rgba(),
        gradient: [0.; 4],
        options: [0.; 4],
    }
}

pub(crate) fn physical_affine(transform: Transform, scale: f32) -> ([f32; 4], [f32; 4]) {
    let [a, b, c, d, e, f] = transform.to_kurbo().as_coeffs();
    (
        [
            a as f32 * scale,
            b as f32 * scale,
            c as f32 * scale,
            d as f32 * scale,
        ],
        [e as f32 * scale, f as f32 * scale, 0., 0.],
    )
}
pub(crate) fn rrect_instance(
    rrect: RRect,
    brush: &Brush,
    transform: Transform,
    scale: f32,
    width: f32,
    height: f32,
) -> GpuRRectInstance {
    let rect = rrect.rect;
    let (kind, color_a, color_b, gradient) = match brush {
        Brush::Solid(color) => (0., *color, *color, [0.; 4]),
        Brush::LinearGradient(g) => {
            let stops = g.stops.as_slice();
            let end = stops.last().expect("normalized stops").color;
            (
                1.,
                stops[0].color,
                end,
                [
                    g.start.x * scale,
                    g.start.y * scale,
                    g.end.x * scale,
                    g.end.y * scale,
                ],
            )
        }
        Brush::RadialGradient(g) => {
            let stops = g.stops.as_slice();
            (
                2.,
                stops[0].color,
                stops.last().expect("normalized stops").color,
                [
                    g.center.x * scale,
                    g.center.y * scale,
                    g.radius.max(0.) * scale,
                    0.,
                ],
            )
        }
        Brush::SweepGradient(g) => {
            let stops = g.stops.as_slice();
            (
                3.,
                stops[0].color,
                stops.last().expect("normalized stops").color,
                [g.center.x * scale, g.center.y * scale, g.start_angle, 0.],
            )
        }
    };
    GpuRRectInstance {
        rect: [
            rect.origin.x,
            rect.origin.y,
            rect.size.width,
            rect.size.height,
        ],
        affine: physical_affine(transform, scale).0,
        translation: physical_affine(transform, scale).1,
        surface: [width, height, 0., 0.],
        radii: [
            rrect.radii.top_left * scale,
            rrect.radii.top_right * scale,
            rrect.radii.bottom_right * scale,
            rrect.radii.bottom_left * scale,
        ],
        color_a: color_a.to_linear_rgba(),
        color_b: color_b.to_linear_rgba(),
        gradient,
        options: [kind, 0., rect.size.width * scale, rect.size.height * scale],
    }
}
pub(crate) fn border_instance(
    rrect: RRect,
    border: incular_painting::Border,
    transform: Transform,
    scale: f32,
    width: f32,
    height: f32,
) -> GpuRRectInstance {
    let mut result = rrect_instance(
        rrect,
        &Brush::Solid(border.color),
        transform,
        scale,
        width,
        height,
    );
    result.options[1] = border
        .width
        .min(rrect.rect.size.width * 0.5)
        .min(rrect.rect.size.height * 0.5)
        * scale;
    result
}
pub(crate) fn glyph_instance(
    run: &GlyphRun,
    offset: Offset,
    entry: AtlasEntry,
    color: Color,
    surface: GlyphSurface,
) -> GpuGlyphInstance {
    // Fontdue's ymin is the bitmap's bottom relative to the baseline; the
    // Parley-to-Incular bridge preserves the renderer's Y-down glyph-offset
    // convention.
    let x = run.origin.x + offset.x + f32::from(entry.bearing_x) / surface.scale;
    let y = run.origin.y
        - offset.y
        - (f32::from(entry.bearing_y) + f32::from(entry.height)) / surface.scale;
    GpuGlyphInstance {
        rect: [
            x,
            y,
            f32::from(entry.width) / surface.scale,
            f32::from(entry.height) / surface.scale,
        ],
        affine: physical_affine(surface.transform, surface.scale).0,
        translation: physical_affine(surface.transform, surface.scale).1,
        surface: [surface.width, surface.height, 0., 0.],
        uv: entry.uv_rect(),
        color: color.to_linear_rgba(),
    }
}
pub(crate) fn image_instance(
    destination: Rect,
    source: Rect,
    image: &ImageHandle,
    width: f32,
    height: f32,
    scale: f32,
    transform: Transform,
) -> GpuImageInstance {
    let image_width = image.decoded().width() as f32;
    let image_height = image.decoded().height() as f32;
    GpuImageInstance {
        rect: [
            destination.origin.x,
            destination.origin.y,
            destination.size.width,
            destination.size.height,
        ],
        affine: physical_affine(transform, scale).0,
        translation: physical_affine(transform, scale).1,
        surface: [width, height, 0., 0.],
        // Decoders and wgpu texture uploads both use top-to-bottom rows, while
        // the quad is Y-down in NDC, so UV Y is intentionally not flipped.
        uv: [
            source.origin.x / image_width,
            source.origin.y / image_height,
            (source.origin.x + source.size.width) / image_width,
            (source.origin.y + source.size.height) / image_height,
        ],
    }
}
pub(crate) fn logical_instance(
    instance: RectangleInstance,
    width: f32,
    height: f32,
    scale: f32,
) -> GpuInstance {
    GpuInstance {
        rect: ndc_rect(
            instance.rect.origin.x * scale,
            instance.rect.origin.y * scale,
            instance.rect.size.width * scale,
            instance.rect.size.height * scale,
            width,
            height,
        ),
        color: instance.color.to_linear_rgba(),
    }
}
pub(crate) fn ndc_rect(x: f32, y: f32, w: f32, h: f32, width: f32, height: f32) -> [f32; 4] {
    [
        2. * x / width - 1.,
        1. - 2. * y / height,
        2. * w / width,
        -2. * h / height,
    ]
}
#[allow(clippy::too_many_arguments)]
pub(crate) fn composite_instance(
    target_origin: Offset,
    parent_origin: Offset,
    parent_width: u32,
    parent_height: u32,
    target_width: u32,
    target_height: u32,
    scale: f32,
    alpha: f32,
) -> GpuCompositeInstance {
    composite_instance_with_color(
        target_origin,
        parent_origin,
        parent_width,
        parent_height,
        target_width,
        target_height,
        scale,
        alpha,
        Color::WHITE,
        false,
    )
}
#[allow(clippy::too_many_arguments)]
pub(crate) fn composite_instance_with_color(
    target_origin: Offset,
    parent_origin: Offset,
    parent_width: u32,
    parent_height: u32,
    target_width: u32,
    target_height: u32,
    scale: f32,
    alpha: f32,
    color: Color,
    shadow: bool,
) -> GpuCompositeInstance {
    GpuCompositeInstance {
        rect: ndc_rect(
            (target_origin.x - parent_origin.x) * scale,
            (target_origin.y - parent_origin.y) * scale,
            target_width as f32,
            target_height as f32,
            parent_width as f32,
            parent_height as f32,
        ),
        uv: [0., 0., 1., 1.],
        alpha: [normalize_opacity(alpha), 0., 0., 0.],
        color: color.to_linear_rgba(),
        options: [f32::from(shadow as u8), 0., 0., 0.],
    }
}
#[allow(clippy::too_many_arguments)]
pub(crate) fn blend_instance(
    target_origin: Offset,
    parent_origin: Offset,
    parent_width: u32,
    parent_height: u32,
    target_width: u32,
    target_height: u32,
    scale: f32,
    mode: BlendMode,
) -> GpuCompositeInstance {
    GpuCompositeInstance {
        rect: ndc_rect(
            (target_origin.x - parent_origin.x) * scale,
            (target_origin.y - parent_origin.y) * scale,
            target_width as f32,
            target_height as f32,
            parent_width as f32,
            parent_height as f32,
        ),
        uv: [0., 0., 1., 1.],
        alpha: [1., 0., 0., 0.],
        color: Color::WHITE.to_linear_rgba(),
        options: [
            mode.code() as f32,
            parent_width as f32,
            parent_height as f32,
            0.,
        ],
    }
}
