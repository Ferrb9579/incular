//! Retained `wgpu` renderer for renderer-neutral Incular display lists.
//!
//! Shaping remains in `incular-text`. This crate rasterizes its glyph IDs at
//! physical DPI, retains their coverage masks in `R8Unorm` atlas textures, and
//! draws atlas-backed instanced quads in display-list order.
use bytemuck::{Pod, Zeroable};
use fontdue::{Font, FontSettings};
use incular_assets::FontId;
use incular_core::{Color, Offset, Rect, Size, Transform};
use incular_image::{ImageHandle, ImageId};
use incular_platform::{PhysicalSize, RawWindowHandles};
use incular_rendering as incular_painting;
use incular_rendering::{
    BlendMode, Brush, ColorFilter, DisplayList, DropShadowEffect, FillRule, GaussianBlur, GlyphRun,
    GradientId, ImageSampling, LineCap, LineJoin, PaintCommand, Path, PathId, RRect, Stroke,
    blur_bounds, drop_shadow_bounds, gaussian_kernel_weights, normalize_opacity, normalize_sigma,
    sample_gradient_stops,
};
use kurbo::PathEl;
use lyon_tessellation::{
    FillOptions, FillRule as LyonFillRule, FillTessellator, StrokeOptions, StrokeTessellator,
    VertexBuffers, geometry_builder::simple_builder, math::point, path::Path as LyonPath,
};
use std::collections::{HashMap, hash_map::Entry};
use std::fmt::Write as _;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use wgpu::util::DeviceExt;

const ATLAS_PAGE_SIZE: u16 = 1024;
const ATLAS_PADDING: u16 = 1;
const OVERSIZE_PAGE_AREA_DIVISOR: u32 = 4;
/// Prevent an untrusted font size from causing a multi-gigabyte CPU bitmap
/// allocation before the renderer can reject it.
const MAX_GLYPH_BITMAP_BYTES: usize = 8 * 1024 * 1024;
const MAX_GLYPH_RASTER_PPEM: f32 = 1024.;
const GLYPH_ATLAS_FILTER: wgpu::FilterMode = wgpu::FilterMode::Linear;
const IMAGE_CACHE_MAX_UNUSED_FRAMES: u64 = 600;
const PATH_CACHE_MAX_UNUSED_FRAMES: u64 = 600;
const GRADIENT_CACHE_MAX_UNUSED_FRAMES: u64 = 600;
const OFFSCREEN_CACHE_MAX_UNUSED_FRAMES: u64 = 600;
/// Each normalized gradient is resampled into this compact one-dimensional
/// lookup texture. This deterministic representation supports any stop count:
/// every stop participates in the premultiplied-linear samples.
const GRADIENT_LUT_SAMPLES: u32 = 256;
const MAX_STENCIL_CLIP_DEPTH: u8 = u8::MAX;
/// Direct Gaussian kernels are capped at this radius. Larger physical sigma
/// values select the explicit multi-scale path before reaching the shader.
const MAX_BLUR_RADIUS: usize = 48;
const BLUR_WEIGHT_SLOTS: usize = MAX_BLUR_RADIUS + 16;
const LARGE_BLUR_SIGMA_THRESHOLD: f32 = 16.;
/// Kernel coefficients are stable across tiny animation/DPI floating-point
/// differences. Quantizing only the CPU resource key does not change the
/// effect's bounds or filtered-result key.
const BLUR_KERNEL_QUANTUM: f32 = 1. / 64.;

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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RectangleInstance {
    pub rect: Rect,
    pub color: Color,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RectangleBatch {
    instances: Vec<RectangleInstance>,
}
impl RectangleBatch {
    #[must_use]
    pub fn instances(&self) -> &[RectangleInstance] {
        &self.instances
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.instances.is_empty()
    }
}
/// Rectangle-only compatibility inspection. Actual rendering uses an ordered
/// mixed rectangle/glyph stream and never globally sorts by pipeline.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BatchPlan {
    batches: Vec<RectangleBatch>,
}
impl BatchPlan {
    #[must_use]
    pub fn lower(list: &DisplayList) -> Self {
        let mut batches = Vec::new();
        let mut current = RectangleBatch::default();
        let mut transforms = vec![Transform::IDENTITY];
        for command in list.commands() {
            match command {
                PaintCommand::Rect { rect, color } => current.instances.push(RectangleInstance {
                    rect: transforms
                        .last()
                        .expect("transform stack")
                        .transform_rect_bbox(*rect),
                    color: *color,
                }),
                PaintCommand::PushTransform {
                    transform: local_transform,
                } => transforms.push(
                    transforms
                        .last()
                        .expect("transform stack")
                        .then(*local_transform),
                ),
                PaintCommand::PopTransform => {
                    if transforms.len() > 1 {
                        transforms.pop();
                    }
                }
                PaintCommand::Image { .. }
                | PaintCommand::RRect { .. }
                | PaintCommand::Border { .. }
                | PaintCommand::FillPath { .. }
                | PaintCommand::StrokePath { .. }
                | PaintCommand::PushClip { .. }
                | PaintCommand::PushClipRRect { .. }
                | PaintCommand::PushClipOval { .. }
                | PaintCommand::PushClipPath { .. }
                | PaintCommand::PopClip
                | PaintCommand::GlyphRun { .. }
                | PaintCommand::PushOpacity { .. }
                | PaintCommand::PopOpacity
                | PaintCommand::PushBlur { .. }
                | PaintCommand::PushDropShadow { .. }
                | PaintCommand::PushColorFilter { .. }
                | PaintCommand::PushBlend { .. }
                | PaintCommand::PopEffect => {
                    if !current.is_empty() {
                        batches.push(std::mem::take(&mut current));
                    }
                }
            }
        }
        if !current.is_empty() {
            batches.push(current);
        }
        Self { batches }
    }
    #[must_use]
    pub fn batches(&self) -> &[RectangleBatch] {
        &self.batches
    }
    #[must_use]
    pub fn rectangle_count(&self) -> usize {
        self.batches.iter().map(|batch| batch.instances.len()).sum()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GpuCounters {
    pub frames: u64,
    pub draw_calls: u64,
    pub rectangle_instances: u64,
    pub glyph_instances: u64,
    pub image_instances: u64,
    /// Rectangle instance-buffer reallocations (legacy name retained).
    pub buffer_reallocations: u64,
    pub glyph_instance_buffer_reallocations: u64,
    pub glyph_cache_hits: u64,
    pub glyph_cache_misses: u64,
    pub glyphs_rasterized: u64,
    pub glyphs_skipped: u64,
    pub glyph_atlas_uploads: u64,
    pub glyph_atlas_pages: u64,
    pub micro_glyph_rasters: u64,
    pub small_glyph_rasters: u64,
    pub normal_glyph_rasters: u64,
    pub large_glyph_rasters: u64,
    pub huge_glyph_rasters: u64,
    pub oversize_glyph_rasters: u64,
    /// Parsed Fontdue objects created on a `FontId` cache miss.
    pub font_parser_cache_misses: u64,
    pub font_parser_cache_hits: u64,
    pub rasterizer_errors: u64,
    /// Aggregate cold raster time; cache hits do not contribute.
    pub raster_time_total: Duration,
    pub oversize_cache_hits: u64,
    pub oversize_cache_misses: u64,
    pub atlas_texture_recreations: u64,
    pub text_draw_calls: u64,
    pub text_pipeline_creations: u64,
    pub rectangle_pipeline_creations: u64,
    pub image_pipeline_creations: u64,
    pub image_cache_hits: u64,
    pub image_cache_misses: u64,
    pub image_texture_uploads: u64,
    pub image_texture_creations: u64,
    pub image_texture_evictions: u64,
    pub image_draw_calls: u64,
    pub rounded_rect_instances: u64,
    pub gradient_instances: u64,
    pub gradient_draw_calls: u64,
    pub gradient_cache_hits: u64,
    pub gradient_cache_misses: u64,
    pub gradient_resource_creations: u64,
    pub gradient_resource_uploads: u64,
    pub gradient_resource_evictions: u64,
    pub linear_gradient_instances: u64,
    pub radial_gradient_instances: u64,
    pub path_tessellation_requests: u64,
    pub path_tessellation_cache_hits: u64,
    pub path_tessellation_cache_misses: u64,
    pub path_fill_tessellations: u64,
    pub path_stroke_tessellations: u64,
    pub path_cpu_vertices: u64,
    pub path_cpu_indices: u64,
    pub path_gpu_cache_hits: u64,
    pub path_gpu_cache_misses: u64,
    pub path_gpu_uploads: u64,
    pub path_gpu_evictions: u64,
    pub path_draw_calls: u64,
    pub path_triangles: u64,
    pub path_pipeline_creations: u64,
    pub composite_pipeline_creations: u64,
    /// Retained `Depth24PlusStencil8` attachments. Recreation only happens
    /// when the physical surface target changes.
    pub stencil_texture_creations: u64,
    pub stencil_texture_recreations: u64,
    pub stencil_pipeline_creations: u64,
    pub clip_rect_pushes: u64,
    pub clip_rrect_pushes: u64,
    pub clip_path_pushes: u64,
    pub clip_pops: u64,
    pub stencil_mask_draws: u64,
    pub stencil_depth_max: u64,
    pub clip_culled_draws: u64,
    pub offscreen_group_cache_hits: u64,
    pub offscreen_group_cache_misses: u64,
    pub offscreen_group_rerenders: u64,
    pub offscreen_color_texture_creations: u64,
    pub offscreen_stencil_texture_creations: u64,
    pub offscreen_texture_reuses: u64,
    pub offscreen_texture_evictions: u64,
    pub offscreen_cached_bytes: usize,
    pub offscreen_peak_cached_bytes: usize,
    pub offscreen_render_passes: u64,
    pub offscreen_composite_draws: u64,
    pub opacity_zero_fast_paths: u64,
    pub opacity_one_fast_paths: u64,
    pub max_offscreen_nesting_depth: u64,
    /// Retained effect/source transitions. Source counters are deliberately
    /// separate from filtered-result counters so parameter animation can be
    /// diagnosed without guessing which stage reran.
    pub effect_source_cache_hits: u64,
    pub effect_source_cache_misses: u64,
    pub effect_source_rerenders: u64,
    pub blur_cache_hits: u64,
    pub blur_cache_misses: u64,
    pub blur_horizontal_passes: u64,
    pub blur_vertical_passes: u64,
    pub blur_kernel_cache_hits: u64,
    pub blur_kernel_cache_misses: u64,
    pub blur_kernel_uploads: u64,
    pub blur_downsample_passes: u64,
    pub blur_upsample_passes: u64,
    pub drop_shadow_composites: u64,
    pub drop_shadow_blur_reuses: u64,
    pub effect_texture_creations: u64,
    pub effect_texture_pool_hits: u64,
    pub effect_texture_pool_misses: u64,
    pub effect_texture_evictions: u64,
    pub effect_cached_bytes: usize,
    pub effect_peak_bytes: usize,
    pub effect_chain_compilations: u64,
    pub effect_stage_cache_hits: u64,
    pub effect_stage_cache_misses: u64,
    pub effect_stage_rerenders: u64,
    pub effect_stage_fusions: u64,
    pub color_matrix_passes: u64,
    pub color_matrix_fused_composites: u64,
    pub blend_fixed_function_draws: u64,
    pub blend_destination_read_draws: u64,
    pub blend_intermediate_target_creations: u64,
    pub blend_target_reuses: u64,
    pub blend_intermediate_cached_bytes: usize,
    pub blend_intermediate_peak_bytes: usize,
    pub effect_chain_cached_bytes: usize,
    /// Destination-dependent promotion scopes. SrcOver-only frames keep this
    /// at zero; sparse scenes use a tight retained scene scope instead of the
    /// complete presentation surface.
    pub full_frame_intermediate_passes: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GlyphCacheKey {
    pub font: FontId,
    pub glyph: u16,
    /// Rounded physical ppem. Fontdue accepts a scalar size, while the cache
    /// remains deterministic across equal logical size/DPI requests.
    pub physical_size: u16,
}
/// Physical-pixel class used to select and explain raster behavior.  These
/// bounds are intentionally expressed in ppem, never logical widget pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GlyphSizeClass {
    Micro,
    Small,
    Normal,
    Large,
    Huge,
}

const fn size_class(ppem: u16) -> GlyphSizeClass {
    match ppem {
        0..=7 => GlyphSizeClass::Micro,
        8..=15 => GlyphSizeClass::Small,
        16..=95 => GlyphSizeClass::Normal,
        96..=255 => GlyphSizeClass::Large,
        _ => GlyphSizeClass::Huge,
    }
}

/// Which retained atlas pool owns a mask.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum GlyphAtlasClass {
    #[default]
    Normal,
    Oversize,
}

/// The renderer's DPI-specific request for a grayscale glyph mask.
///
/// Layout and shaping stay in logical pixels; only this request enters the
/// physical-pixel raster cache.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlyphRasterRequest {
    pub logical_font_size: f32,
    pub scale_factor: f32,
    pub physical_size: u16,
    pub supported: bool,
}
impl GlyphRasterRequest {
    #[must_use]
    pub fn new(logical_font_size: f32, scale_factor: f64) -> Self {
        let scale_factor = normalized_scale(scale_factor);
        let physical_size = logical_font_size * scale_factor;
        Self {
            logical_font_size,
            scale_factor,
            physical_size: physical_size.round().clamp(1., f32::from(u16::MAX)) as u16,
            supported: physical_size.is_finite()
                && (1. ..=MAX_GLYPH_RASTER_PPEM).contains(&physical_size),
        }
    }
}
/// Deterministic information for diagnosing a cached glyph without logging on
/// the hot path. Atlas rectangles are physical texels; font size is logical.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlyphRasterDebug {
    pub font: FontId,
    pub glyph: u16,
    pub logical_font_size: f32,
    pub scale_factor: f32,
    pub requested_physical_size: u16,
    pub atlas_class: GlyphAtlasClass,
    pub bitmap_size: [u16; 2],
    pub bitmap_bytes: usize,
    pub bearing: [i16; 2],
    pub atlas_page: u16,
    pub allocation_rect: [u16; 4],
    pub content_rect: [u16; 4],
    pub uv_rect: [f32; 4],
}
/// Glyph coverage is sampled with bilinear filtering. Padding is intentionally
/// exposed for diagnostics and atlas invariant tests.
pub const GLYPH_ATLAS_PADDING: u16 = ATLAS_PADDING;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AtlasEntry {
    pub page: u16,
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub atlas_class: GlyphAtlasClass,
    /// Physical-pixel raster bounds relative to the shaped baseline origin.
    pub bearing_x: i16,
    pub bearing_y: i16,
}
impl AtlasEntry {
    #[must_use]
    pub fn uv_rect(self) -> [f32; 4] {
        let page = f32::from(ATLAS_PAGE_SIZE);
        [
            f32::from(self.x) / page,
            f32::from(self.y) / page,
            f32::from(self.x + self.width) / page,
            f32::from(self.y + self.height) / page,
        ]
    }
    #[must_use]
    pub fn allocation_rect(self) -> [u16; 4] {
        [
            self.x - ATLAS_PADDING,
            self.y - ATLAS_PADDING,
            self.width + ATLAS_PADDING * 2,
            self.height + ATLAS_PADDING * 2,
        ]
    }
}
#[derive(Clone, Debug)]
pub struct RasterizedGlyph {
    pub entry: AtlasEntry,
    pub bitmap: Option<Vec<u8>>,
}
/// Compact diagnostic for checking that a coverage mask has not accidentally
/// become binary during rasterization.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CoverageHistogram {
    pub transparent: usize,
    pub opaque: usize,
    pub intermediate: usize,
}

#[must_use]
pub fn coverage_histogram(bytes: &[u8]) -> CoverageHistogram {
    let mut histogram = CoverageHistogram::default();
    for &value in bytes {
        match value {
            0 => histogram.transparent += 1,
            u8::MAX => histogram.opaque += 1,
            _ => histogram.intermediate += 1,
        }
    }
    histogram
}

#[derive(Default)]
struct AtlasPage {
    next_x: u16,
    next_y: u16,
    row_height: u16,
    class: GlyphAtlasClass,
    content_area: u32,
    allocated_area: u32,
}
impl AtlasPage {
    fn normal() -> Self {
        Self {
            class: GlyphAtlasClass::Normal,
            ..Self::default()
        }
    }
    fn oversize() -> Self {
        Self {
            class: GlyphAtlasClass::Oversize,
            ..Self::default()
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GlyphAtlasMemory {
    pub normal_pages: u16,
    pub normal_bytes: usize,
    pub normal_content_area: u64,
    pub normal_allocated_area: u64,
    pub oversize_pages: u16,
    pub oversize_bytes: usize,
    pub oversize_content_area: u64,
    pub oversize_allocated_area: u64,
}
/// CPU metadata for retained atlas pages. `WgpuRenderer` maps each page index
/// lazily to one persistent `R8Unorm` texture; entries never move or compact.
pub struct GlyphAtlas {
    pages: Vec<AtlasPage>,
    entries: HashMap<GlyphCacheKey, AtlasEntry>,
    /// Parsed once per stable font identity, then reused for every uncached
    /// glyph bitmap. `FontSettings::collection_index` preserves TTC/OTC faces.
    fonts: HashMap<FontId, Font>,
    counters: GpuCounters,
}
impl Default for GlyphAtlas {
    fn default() -> Self {
        Self::new()
    }
}
impl GlyphAtlas {
    #[must_use]
    pub fn new() -> Self {
        Self {
            pages: vec![AtlasPage::normal()],
            entries: HashMap::new(),
            fonts: HashMap::new(),
            counters: GpuCounters {
                glyph_atlas_pages: 1,
                ..GpuCounters::default()
            },
        }
    }
    #[must_use]
    pub const fn counters(&self) -> GpuCounters {
        self.counters
    }
    #[must_use]
    pub fn entry(&self, key: GlyphCacheKey) -> Option<AtlasEntry> {
        self.entries.get(&key).copied()
    }
    #[must_use]
    pub fn memory(&self) -> GlyphAtlasMemory {
        let mut memory = GlyphAtlasMemory::default();
        let page_bytes = usize::from(ATLAS_PAGE_SIZE).pow(2);
        for page in &self.pages {
            match page.class {
                GlyphAtlasClass::Normal => {
                    memory.normal_pages += 1;
                    memory.normal_bytes += page_bytes;
                    memory.normal_content_area += u64::from(page.content_area);
                    memory.normal_allocated_area += u64::from(page.allocated_area);
                }
                GlyphAtlasClass::Oversize => {
                    memory.oversize_pages += 1;
                    memory.oversize_bytes += page_bytes;
                    memory.oversize_content_area += u64::from(page.content_area);
                    memory.oversize_allocated_area += u64::from(page.allocated_area);
                }
            }
        }
        memory
    }
    pub fn lookup_or_rasterize(
        &mut self,
        run: &GlyphRun,
        glyph: u16,
        scale: f64,
    ) -> Option<RasterizedGlyph> {
        let request = GlyphRasterRequest::new(run.font_size, scale);
        if !request.supported {
            self.counters.glyphs_skipped += 1;
            return None;
        }
        let key = GlyphCacheKey {
            font: run.font.id(),
            glyph,
            physical_size: request.physical_size,
        };
        if let Some(entry) = self.entries.get(&key).copied() {
            self.counters.glyph_cache_hits += 1;
            if entry.atlas_class == GlyphAtlasClass::Oversize {
                self.counters.oversize_cache_hits += 1;
            }
            return Some(RasterizedGlyph {
                entry,
                bitmap: None,
            });
        }
        self.counters.glyph_cache_misses += 1;
        let font = match self.fonts.entry(key.font) {
            Entry::Occupied(entry) => {
                self.counters.font_parser_cache_hits += 1;
                entry.into_mut()
            }
            Entry::Vacant(entry) => {
                let settings = FontSettings {
                    collection_index: run.font.face_index(),
                    ..FontSettings::default()
                };
                let parsed = Font::from_bytes(run.font.bytes().as_ref(), settings).ok()?;
                self.counters.font_parser_cache_misses += 1;
                entry.insert(parsed)
            }
        };
        let started = Instant::now();
        let (metrics, bitmap) = font.rasterize_indexed(key.glyph, f32::from(key.physical_size));
        self.counters.raster_time_total = self
            .counters
            .raster_time_total
            .saturating_add(started.elapsed());
        let bitmap_bytes = metrics.width.checked_mul(metrics.height)?;
        if bitmap_bytes > MAX_GLYPH_BITMAP_BYTES || bitmap.len() != bitmap_bytes {
            self.counters.glyphs_skipped += 1;
            return None;
        }
        let width = u16::try_from(metrics.width).ok()?;
        let height = u16::try_from(metrics.height).ok()?;
        self.counters.glyphs_rasterized += 1;
        match size_class(request.physical_size) {
            GlyphSizeClass::Micro => self.counters.micro_glyph_rasters += 1,
            GlyphSizeClass::Small => self.counters.small_glyph_rasters += 1,
            GlyphSizeClass::Normal => self.counters.normal_glyph_rasters += 1,
            GlyphSizeClass::Large => self.counters.large_glyph_rasters += 1,
            GlyphSizeClass::Huge => self.counters.huge_glyph_rasters += 1,
        }
        let bearing_x = clamp_i16(metrics.xmin);
        let bearing_y = clamp_i16(metrics.ymin);
        // Spaces deliberately have a cache entry but no texture write or quad.
        if width == 0 || height == 0 {
            let entry = AtlasEntry {
                page: 0,
                x: 0,
                y: 0,
                width: 0,
                height: 0,
                atlas_class: GlyphAtlasClass::Normal,
                bearing_x,
                bearing_y,
            };
            self.entries.insert(key, entry);
            return Some(RasterizedGlyph {
                entry,
                bitmap: None,
            });
        }
        let entry = self.allocate(width, height, bearing_x, bearing_y)?;
        if entry.atlas_class == GlyphAtlasClass::Oversize {
            self.counters.oversize_glyph_rasters += 1;
            self.counters.oversize_cache_misses += 1;
        }
        self.entries.insert(key, entry);
        self.counters.glyph_atlas_uploads += 1;
        Some(RasterizedGlyph {
            entry,
            bitmap: Some(bitmap),
        })
    }
    /// Returns cache metadata for a glyph already requested at `scale`.
    #[must_use]
    pub fn debug_glyph(&self, run: &GlyphRun, glyph: u16, scale: f64) -> Option<GlyphRasterDebug> {
        let request = GlyphRasterRequest::new(run.font_size, scale);
        let entry = self.entry(GlyphCacheKey {
            font: run.font.id(),
            glyph,
            physical_size: request.physical_size,
        })?;
        Some(GlyphRasterDebug {
            font: run.font.id(),
            glyph,
            logical_font_size: request.logical_font_size,
            scale_factor: request.scale_factor,
            requested_physical_size: request.physical_size,
            atlas_class: entry.atlas_class,
            bitmap_size: [entry.width, entry.height],
            bitmap_bytes: usize::from(entry.width) * usize::from(entry.height),
            bearing: [entry.bearing_x, entry.bearing_y],
            atlas_page: entry.page,
            allocation_rect: entry.allocation_rect(),
            content_rect: [entry.x, entry.y, entry.width, entry.height],
            uv_rect: entry.uv_rect(),
        })
    }
    fn allocate(
        &mut self,
        width: u16,
        height: u16,
        bearing_x: i16,
        bearing_y: i16,
    ) -> Option<AtlasEntry> {
        let stored_width = width.checked_add(ATLAS_PADDING * 2)?;
        let stored_height = height.checked_add(ATLAS_PADDING * 2)?;
        if stored_width > ATLAS_PAGE_SIZE || stored_height > ATLAS_PAGE_SIZE {
            return None;
        }
        let stored_area = u32::from(stored_width) * u32::from(stored_height);
        let page_area = u32::from(ATLAS_PAGE_SIZE) * u32::from(ATLAS_PAGE_SIZE);
        if stored_area >= page_area / OVERSIZE_PAGE_AREA_DIVISOR {
            let page_index = self.pages.len();
            let mut page = AtlasPage::oversize();
            let entry = AtlasEntry {
                page: page_index.try_into().ok()?,
                x: ATLAS_PADDING,
                y: ATLAS_PADDING,
                width,
                height,
                atlas_class: GlyphAtlasClass::Oversize,
                bearing_x,
                bearing_y,
            };
            page.next_x = stored_width;
            page.row_height = stored_height;
            page.content_area = u32::from(width) * u32::from(height);
            page.allocated_area = stored_area;
            self.pages.push(page);
            self.counters.glyph_atlas_pages += 1;
            return Some(entry);
        }
        let page_index = self
            .pages
            .iter()
            .rposition(|page| page.class == GlyphAtlasClass::Normal)?;
        let page = &mut self.pages[page_index];
        if page.next_x + stored_width > ATLAS_PAGE_SIZE {
            page.next_x = 0;
            page.next_y = page.next_y.saturating_add(page.row_height);
            page.row_height = 0;
        }
        if page.next_y + stored_height > ATLAS_PAGE_SIZE {
            self.pages.push(AtlasPage::normal());
            self.counters.glyph_atlas_pages += 1;
            return self.allocate(width, height, bearing_x, bearing_y);
        }
        let entry = AtlasEntry {
            page: page_index.try_into().ok()?,
            x: page.next_x + ATLAS_PADDING,
            y: page.next_y + ATLAS_PADDING,
            width,
            height,
            atlas_class: GlyphAtlasClass::Normal,
            bearing_x,
            bearing_y,
        };
        page.next_x += stored_width;
        page.row_height = page.row_height.max(stored_height);
        page.content_area += u32::from(width) * u32::from(height);
        page.allocated_area += stored_area;
        Some(entry)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderStats {
    pub draw_calls: u32,
    pub rectangle_instances: u32,
    pub glyph_instances: u32,
    pub image_instances: u32,
    pub buffer_reallocated: bool,
    pub glyph_buffer_reallocated: bool,
    pub image_buffer_reallocated: bool,
    pub presented: bool,
}
#[derive(Debug)]
pub enum RendererError {
    Adapter(wgpu::RequestAdapterError),
    Device(wgpu::RequestDeviceError),
    Surface(wgpu::CreateSurfaceError),
    ImageTooLarge { width: u32, height: u32, limit: u32 },
    GlyphAtlasPageTooLarge { page: u16, limit: u32 },
    StencilDepthOverflow,
    UnbalancedClipStack,
    OffscreenTargetTooLarge { width: u32, height: u32, limit: u32 },
    OutOfMemory,
}
impl std::fmt::Display for RendererError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Adapter(e) => write!(f, "unable to acquire GPU adapter: {e}"),
            Self::Device(e) => write!(f, "unable to acquire GPU device: {e}"),
            Self::Surface(e) => write!(f, "unable to create GPU surface: {e}"),
            Self::ImageTooLarge {
                width,
                height,
                limit,
            } => write!(
                f,
                "image {width}x{height} exceeds GPU texture limit {limit}"
            ),
            Self::GlyphAtlasPageTooLarge { page, limit } => write!(
                f,
                "glyph atlas page {page}x{page} exceeds GPU texture limit {limit}"
            ),
            Self::StencilDepthOverflow => {
                write!(f, "nested non-rectangular clip depth exceeds 255")
            }
            Self::UnbalancedClipStack => {
                write!(f, "display list contains an unbalanced clip stack")
            }
            Self::OffscreenTargetTooLarge {
                width,
                height,
                limit,
            } => write!(
                f,
                "opacity group target {width}x{height} exceeds GPU texture limit {limit}"
            ),
            Self::OutOfMemory => write!(f, "GPU surface ran out of memory"),
        }
    }
}
impl std::error::Error for RendererError {}

/// Stable, process-local identity for an immutable GPU resource shared by all
/// windows attached to one [`SharedGpuContext`]. It intentionally exposes no
/// native or `wgpu` handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SharedGpuResourceId(u64);
impl SharedGpuResourceId {
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// A headless record of shared-resource identity. The live GPU context uses
/// this same registry when it creates image, glyph, and pipeline resources;
/// keeping it independent of an adapter makes ownership tests display-server
/// free.
#[derive(Debug, Default)]
pub struct SharedGpuResourceRegistry {
    next_identity: u64,
    images: HashMap<ImageId, SharedGpuResourceId>,
    glyphs: HashMap<GlyphCacheKey, SharedGpuResourceId>,
}
impl SharedGpuResourceRegistry {
    fn allocate(&mut self) -> SharedGpuResourceId {
        self.next_identity = self.next_identity.saturating_add(1).max(1);
        SharedGpuResourceId(self.next_identity)
    }
    /// Returns the one context-local identity allocated for `image`.
    pub fn image_identity(&mut self, image: ImageId) -> SharedGpuResourceId {
        if let Some(identity) = self.images.get(&image) {
            return *identity;
        }
        let identity = self.allocate();
        self.images.insert(image, identity);
        identity
    }
    /// Returns the one context-local identity allocated for this DPI-specific
    /// glyph raster key.
    pub fn glyph_identity(&mut self, glyph: GlyphCacheKey) -> SharedGpuResourceId {
        if let Some(identity) = self.glyphs.get(&glyph) {
            return *identity;
        }
        let identity = self.allocate();
        self.glyphs.insert(glyph, identity);
        identity
    }
    #[must_use]
    pub fn image_count(&self) -> usize {
        self.images.len()
    }
    #[must_use]
    pub fn glyph_count(&self) -> usize {
        self.glyphs.len()
    }
}

/// Read-only shared GPU ownership diagnostics. Counts describe one
/// application/device, rather than any individual presentation surface.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SharedGpuDiagnostics {
    pub device_generation: u64,
    pub pipeline_variants: usize,
    pub pipeline_count: usize,
    pub shared_image_resources: usize,
    pub shared_glyph_resources: usize,
    pub shared_gradient_resources: usize,
    pub glyph_atlas_pages: usize,
}

/// Per-window presentation state that is independent of the shared GPU
/// device. It deliberately remains useful in headless tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowGpuPresentation {
    pub physical_size: PhysicalSize,
    pub surface_generation: u64,
    pub configured: bool,
    pub presented_frames: u64,
    pub skipped_frames: u64,
}
impl WindowGpuPresentation {
    #[must_use]
    pub const fn new(physical_size: PhysicalSize) -> Self {
        let configured = !physical_size.is_zero();
        Self {
            physical_size,
            surface_generation: if configured { 1 } else { 0 },
            configured,
            presented_frames: 0,
            skipped_frames: 0,
        }
    }
    /// Updates only this window's physical presentation state. A zero-sized
    /// surface is intentionally left unconfigured until it becomes valid.
    pub fn resize(&mut self, physical_size: PhysicalSize) -> bool {
        self.physical_size = physical_size;
        self.configured = !physical_size.is_zero();
        if self.configured {
            self.surface_generation = self.surface_generation.saturating_add(1);
        }
        self.configured
    }
    /// Records a recoverable surface loss. Device loss belongs to the shared
    /// context; this method cannot affect another window's surface state.
    pub fn surface_lost(&mut self) {
        if self.configured {
            self.surface_generation = self.surface_generation.saturating_add(1);
        }
    }
    pub fn record_present(&mut self) {
        self.presented_frames = self.presented_frames.saturating_add(1);
    }
    pub fn record_skipped(&mut self) {
        self.skipped_frames = self.skipped_frames.saturating_add(1);
    }
}

/// Surface and compositor ownership for exactly one native window. The raw
/// handles and `wgpu` surface remain private: native Winit objects never
/// escape through this API.
pub struct WindowGpuState {
    handles: RawWindowHandles,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    stencil_texture: wgpu::Texture,
    stencil_view: wgpu::TextureView,
    presentation: WindowGpuPresentation,
}
impl WindowGpuState {
    #[must_use]
    pub const fn presentation(&self) -> WindowGpuPresentation {
        self.presentation
    }
}

/// Device-wide GPU ownership. Cloning this value never creates another
/// `Instance`, `Adapter`, `Device`, or `Queue`; it merely gives another window
/// access to the same application-owned GPU context.
#[derive(Clone)]
pub struct SharedGpuContext {
    inner: Arc<SharedGpuContextInner>,
}
struct SharedGpuContextInner {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    device_generation: u64,
    pipelines: Mutex<HashMap<wgpu::TextureFormat, Arc<SharedPipelineResources>>>,
    resources: Mutex<SharedGpuResources>,
}
struct SharedGpuResources {
    registry: SharedGpuResourceRegistry,
    images: HashMap<ImageId, Arc<SharedGpuImage>>,
    gradients: HashMap<GradientResourceKey, Arc<SharedGpuGradient>>,
    glyph_atlas: GlyphAtlas,
    glyph_pages: Vec<SharedGpuAtlasPage>,
}
struct SharedGpuImage {
    identity: SharedGpuResourceId,
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
}
struct SharedGpuGradient {
    resource: GpuGradient,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct GradientResourceKey {
    gradient: GradientId,
    /// A bind group is pipeline-layout compatible only within this explicit
    /// target-format pipeline variant.
    format: wgpu::TextureFormat,
}
struct SharedGpuAtlasPage {
    texture: wgpu::Texture,
}
impl SharedGpuContext {
    /// Creates the one device context to be shared by every desktop window in
    /// an application. `handles` is used only to choose a compatible adapter;
    /// the temporary surface is dropped before this method returns.
    pub async fn new(handles: RawWindowHandles) -> Result<Self, RendererError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: handles.display,
                raw_window_handle: handles.window,
            })
        }
        .map_err(RendererError::Surface)?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .map_err(RendererError::Adapter)?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .map_err(RendererError::Device)?;
        drop(surface);
        Ok(Self {
            inner: Arc::new(SharedGpuContextInner {
                instance,
                adapter,
                device,
                queue,
                device_generation: 1,
                pipelines: Mutex::new(HashMap::new()),
                resources: Mutex::new(SharedGpuResources {
                    registry: SharedGpuResourceRegistry::default(),
                    images: HashMap::new(),
                    gradients: HashMap::new(),
                    glyph_atlas: GlyphAtlas::new(),
                    glyph_pages: Vec::new(),
                }),
            }),
        })
    }
    /// Creates a renderer for one additional native surface without creating a
    /// second device context.
    pub async fn create_renderer(
        &self,
        handles: RawWindowHandles,
        size: PhysicalSize,
    ) -> Result<WgpuRenderer, RendererError> {
        WgpuRenderer::new_with_shared(self.clone(), handles, size).await
    }
    #[must_use]
    pub fn diagnostics(&self) -> SharedGpuDiagnostics {
        let pipelines = self.inner.pipelines.lock().expect("shared pipeline lock");
        let resources = self.inner.resources.lock().expect("shared resource lock");
        SharedGpuDiagnostics {
            device_generation: self.inner.device_generation,
            pipeline_variants: pipelines.len(),
            pipeline_count: pipelines.len().saturating_mul(25),
            shared_image_resources: resources.registry.image_count(),
            shared_glyph_resources: resources.registry.glyph_count(),
            shared_gradient_resources: resources.gradients.len(),
            glyph_atlas_pages: resources.glyph_atlas.pages.len(),
        }
    }
    #[must_use]
    pub fn image_resource_identity(&self, image: ImageId) -> Option<SharedGpuResourceId> {
        self.inner
            .resources
            .lock()
            .expect("shared resource lock")
            .registry
            .images
            .get(&image)
            .copied()
    }
    #[must_use]
    pub fn glyph_resource_identity(&self, glyph: GlyphCacheKey) -> Option<SharedGpuResourceId> {
        self.inner
            .resources
            .lock()
            .expect("shared resource lock")
            .registry
            .glyphs
            .get(&glyph)
            .copied()
    }
    fn create_surface(
        &self,
        handles: RawWindowHandles,
    ) -> Result<wgpu::Surface<'static>, RendererError> {
        unsafe {
            self.inner
                .instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: handles.display,
                    raw_window_handle: handles.window,
                })
        }
        .map_err(RendererError::Surface)
    }
    fn pipeline_resources(
        &self,
        format: wgpu::TextureFormat,
    ) -> Option<Arc<SharedPipelineResources>> {
        self.inner
            .pipelines
            .lock()
            .expect("shared pipeline lock")
            .get(&format)
            .cloned()
    }
    fn register_pipeline_resources(
        &self,
        format: wgpu::TextureFormat,
        resources: SharedPipelineResources,
    ) {
        self.inner
            .pipelines
            .lock()
            .expect("shared pipeline lock")
            .entry(format)
            .or_insert_with(|| Arc::new(resources));
    }
    fn image_resource(
        &self,
        image: &ImageHandle,
    ) -> Result<(Arc<SharedGpuImage>, bool), RendererError> {
        let id = image.id();
        let mut resources = self.inner.resources.lock().expect("shared resource lock");
        if let Some(resource) = resources.images.get(&id) {
            return Ok((Arc::clone(resource), false));
        }
        let decoded = image.decoded();
        let limit = self.inner.device.limits().max_texture_dimension_2d;
        if decoded.width() > limit || decoded.height() > limit {
            return Err(RendererError::ImageTooLarge {
                width: decoded.width(),
                height: decoded.height(),
                limit,
            });
        }
        let texture = self.inner.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("incular shared image texture"),
            size: wgpu::Extent3d {
                width: decoded.width(),
                height: decoded.height(),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.inner.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            decoded.pixels().as_ref(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(decoded.width() * 4),
                rows_per_image: Some(decoded.height()),
            },
            wgpu::Extent3d {
                width: decoded.width(),
                height: decoded.height(),
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let resource = Arc::new(SharedGpuImage {
            identity: resources.registry.image_identity(id),
            _texture: texture,
            view,
            width: decoded.width(),
            height: decoded.height(),
        });
        resources.images.insert(id, Arc::clone(&resource));
        Ok((resource, true))
    }
    fn rasterize_glyph(&self, run: &GlyphRun, glyph: u16, scale: f64) -> Option<RasterizedGlyph> {
        let mut resources = self.inner.resources.lock().expect("shared resource lock");
        let request = GlyphRasterRequest::new(run.font_size, scale);
        let key = GlyphCacheKey {
            font: run.font.id(),
            glyph,
            physical_size: request.physical_size,
        };
        let raster = resources
            .glyph_atlas
            .lookup_or_rasterize(run, glyph, scale)?;
        resources.registry.glyph_identity(key);
        Some(raster)
    }
    fn glyph_counters(&self) -> GpuCounters {
        self.inner
            .resources
            .lock()
            .expect("shared resource lock")
            .glyph_atlas
            .counters()
    }
    fn gradient_resource(
        &self,
        id: GradientId,
        format: wgpu::TextureFormat,
        pixels: &[[u8; 4]],
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
    ) -> (Arc<SharedGpuGradient>, bool) {
        let mut resources = self.inner.resources.lock().expect("shared resource lock");
        let key = GradientResourceKey {
            gradient: id,
            format,
        };
        if let Some(resource) = resources.gradients.get(&key) {
            return (Arc::clone(resource), false);
        }
        let resource = Arc::new(SharedGpuGradient {
            resource: create_gradient_resource(
                &self.inner.device,
                &self.inner.queue,
                layout,
                sampler,
                pixels,
            ),
        });
        resources.gradients.insert(key, Arc::clone(&resource));
        (resource, true)
    }
    fn shared_glyph_texture(&self, page: u16) -> wgpu::Texture {
        let mut resources = self.inner.resources.lock().expect("shared resource lock");
        while resources.glyph_pages.len() <= usize::from(page) {
            resources.glyph_pages.push(SharedGpuAtlasPage {
                texture: self.inner.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("incular shared glyph atlas page"),
                    size: wgpu::Extent3d {
                        width: u32::from(ATLAS_PAGE_SIZE),
                        height: u32::from(ATLAS_PAGE_SIZE),
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::R8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                }),
            });
        }
        resources.glyph_pages[usize::from(page)].texture.clone()
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct GpuInstance {
    rect: [f32; 4],
    color: [f32; 4],
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct GpuGlyphInstance {
    rect: [f32; 4],
    affine: [f32; 4],
    translation: [f32; 4],
    surface: [f32; 4],
    uv: [f32; 4],
    color: [f32; 4],
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct GpuImageInstance {
    rect: [f32; 4],
    affine: [f32; 4],
    translation: [f32; 4],
    surface: [f32; 4],
    uv: [f32; 4],
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct GpuRRectInstance {
    rect: [f32; 4],
    affine: [f32; 4],
    translation: [f32; 4],
    surface: [f32; 4],
    /// Radii in physical pixels, TL/TR/BR/BL.
    radii: [f32; 4],
    color_a: [f32; 4],
    color_b: [f32; 4],
    /// Gradient start/end in physical pixels, or radial center/radius.
    gradient: [f32; 4],
    /// x=kind (0 solid, 1 linear, 2 radial), y=inside border width.
    options: [f32; 4],
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct GpuPathInstance {
    /// Physical-space affine linear matrix `[a, b, c, d]`.
    affine: [f32; 4],
    /// Physical-space affine translation `[e, f]`.
    translation: [f32; 4],
    surface: [f32; 4],
    color: [f32; 4],
    gradient: [f32; 4],
    options: [f32; 4],
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct GpuCompositeInstance {
    rect: [f32; 4],
    uv: [f32; 4],
    alpha: [f32; 4],
    /// Straight shadow color; the shader converts it to premultiplied output
    /// using the sampled blurred alpha. Zero means ordinary offscreen draw.
    color: [f32; 4],
    options: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct GpuBlurParams {
    /// Source coordinate of the first output pixel, in input physical pixels.
    source_origin: [f32; 2],
    /// Input source dimensions in physical pixels.
    source_size: [f32; 2],
    /// Output dimensions in physical pixels.
    output_size: [f32; 2],
    /// For direct blur this is a unit axis; for resampling it is the input
    /// pixels traversed by one output pixel.
    direction: [f32; 2],
    radius: u32,
    mode: u32,
    _padding: [u32; 2],
    weights: [f32; BLUR_WEIGHT_SLOTS],
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct GpuColorMatrixParams {
    matrix: [f32; 20],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct BlurKernelKey {
    sigma_bits: u32,
    downsample_factor: u32,
}
#[derive(Clone, Debug)]
struct BlurKernel {
    radius: u32,
    weights: [f32; BLUR_WEIGHT_SLOTS],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum PathMeshKind {
    Fill(FillRule),
    Stroke {
        width: u32,
        cap: LineCap,
        join: LineJoin,
        miter_limit: u32,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct PathMeshKey {
    path: PathId,
    kind: PathMeshKind,
}
struct GpuPathMesh {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
    last_used_frame: u64,
}
#[derive(Clone)]
struct GpuGradient {
    _texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    last_used_frame: u64,
}

const RECT_SHADER: &str = r#"
struct Out { @builtin(position) position: vec4<f32>, @location(0) color: vec4<f32> };
@vertex fn vs_main(@location(0) quad: vec2<f32>, @location(1) rect: vec4<f32>, @location(2) color: vec4<f32>) -> Out { var out: Out; out.position = vec4<f32>(rect.xy + quad * rect.zw, 0., 1.); out.color = color; return out; }
@fragment fn fs_main(input: Out) -> @location(0) vec4<f32> { return input.color; }
"#;
const TEXT_SHADER: &str = r#"
struct Out { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) color: vec4<f32> };
@group(0) @binding(0) var atlas: texture_2d<f32>;
@group(0) @binding(1) var atlas_sampler: sampler;
@vertex fn vs_main(@location(0) quad: vec2<f32>, @location(1) rect: vec4<f32>, @location(2) affine: vec4<f32>, @location(3) translation: vec4<f32>, @location(4) surface: vec4<f32>, @location(5) uv: vec4<f32>, @location(6) color: vec4<f32>) -> Out { var out: Out; let local=rect.xy+quad*rect.zw; let p=vec2<f32>(affine.x*local.x+affine.z*local.y+translation.x,affine.y*local.x+affine.w*local.y+translation.y); out.position=vec4<f32>(p.x/surface.x*2.-1.,1.-p.y/surface.y*2.,0.,1.); out.uv=uv.xy+quad*(uv.zw-uv.xy); out.color=color; return out; }
@fragment fn fs_main(input: Out) -> @location(0) vec4<f32> { let coverage = textureSample(atlas, atlas_sampler, input.uv).r; return vec4<f32>(input.color.rgb, input.color.a * coverage); }
"#;
const IMAGE_SHADER: &str = r#"
struct Out { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> };
@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var image_sampler: sampler;
@vertex fn vs_main(@location(0) quad: vec2<f32>, @location(1) rect: vec4<f32>, @location(2) affine: vec4<f32>, @location(3) translation: vec4<f32>, @location(4) surface: vec4<f32>, @location(5) uv: vec4<f32>) -> Out { var out: Out; let local=rect.xy+quad*rect.zw; let p=vec2<f32>(affine.x*local.x+affine.z*local.y+translation.x,affine.y*local.x+affine.w*local.y+translation.y); out.position=vec4<f32>(p.x/surface.x*2.-1.,1.-p.y/surface.y*2.,0.,1.); out.uv=uv.xy+quad*(uv.zw-uv.xy); return out; }
// The image texture decodes sRGB into linear sample values. Source pixels are
// straight alpha, and ALPHA_BLENDING is straight source-over blending.
@fragment fn fs_main(input: Out) -> @location(0) vec4<f32> { return textureSample(image, image_sampler, input.uv); }
"#;
const RRECT_SHADER: &str = r#"
struct Out { @builtin(position) position: vec4<f32>, @location(0) local: vec2<f32>, @location(1) radii: vec4<f32>, @location(2) a: vec4<f32>, @location(3) b: vec4<f32>, @location(4) gradient: vec4<f32>, @location(5) options: vec4<f32> };
@group(0) @binding(0) var gradient_lut: texture_2d<f32>;
@group(0) @binding(1) var gradient_sampler: sampler;
@vertex fn vs_main(@location(0) quad: vec2<f32>, @location(1) rect: vec4<f32>, @location(2) affine: vec4<f32>, @location(3) translation: vec4<f32>, @location(4) surface: vec4<f32>, @location(5) radii: vec4<f32>, @location(6) a: vec4<f32>, @location(7) b: vec4<f32>, @location(8) gradient: vec4<f32>, @location(9) options: vec4<f32>) -> Out { var o: Out; let local=rect.xy+quad*rect.zw; let p=vec2<f32>(affine.x*local.x+affine.z*local.y+translation.x,affine.y*local.x+affine.w*local.y+translation.y); o.position=vec4(p.x/surface.x*2.-1.,1.-p.y/surface.y*2.,0.,1.); o.local=quad*options.zw; o.radii=radii; o.a=a; o.b=b; o.gradient=gradient; o.options=options; return o; }
fn radius_at(p: vec2<f32>, size: vec2<f32>, r: vec4<f32>) -> f32 { if (p.y < size.y*.5) { if (p.x < size.x*.5) { return r.x; } return r.y; } if (p.x >= size.x*.5) { return r.z; } return r.w; }
fn rounded_distance(p: vec2<f32>, size: vec2<f32>, r: vec4<f32>) -> f32 { let q=p-size*.5; let radius=radius_at(p,size,r); let d=abs(q)-(size*.5-vec2(radius)); return length(max(d,vec2(0.)))+min(max(d.x,d.y),0.)-radius; }
fn lookup(t: f32) -> vec4<f32> { let p=textureSampleLevel(gradient_lut,gradient_sampler,vec2(clamp(t,0.,1.),.5),0.); return select(vec4(0.),vec4(p.rgb/max(p.a,.00001),p.a),p.a>0.); }
@fragment fn fs_main(i: Out) -> @location(0) vec4<f32> { let size=i.options.zw; let outer=rounded_distance(i.local,size,i.radii); var edge=1.-smoothstep(-1.,1.,outer); if(i.options.y>0.) { let width=i.options.y; let inner=rounded_distance(i.local-vec2(width), max(size-vec2(2.*width),vec2(0.)), max(i.radii-vec4(width),vec4(0.))); edge*=smoothstep(-1.,1.,inner); } var t=0.; if(i.options.x==1.) { let v=i.gradient.zw-i.gradient.xy; t=clamp(dot(i.local-i.gradient.xy,v)/max(dot(v,v),.0001),0.,1.); } else if(i.options.x==2.) { t=clamp(length(i.local-i.gradient.xy)/max(i.gradient.z,.0001),0.,1.); } else if(i.options.x==3.) { t=fract((atan2(i.local.y-i.gradient.y,i.local.x-i.gradient.x)-i.gradient.z)/6.2831853); } let color=select(i.a,lookup(t),i.options.x>0.); return vec4(color.rgb,color.a*edge); }
"#;
const PATH_SHADER: &str = r#"
struct Out { @builtin(position) position: vec4<f32>, @location(0) local: vec2<f32>, @location(1) color: vec4<f32>, @location(2) gradient: vec4<f32>, @location(3) options: vec4<f32> };
@group(0) @binding(0) var gradient_lut: texture_2d<f32>;
@group(0) @binding(1) var gradient_sampler: sampler;
@vertex fn vs_main(@location(0) local: vec2<f32>, @location(1) affine: vec4<f32>, @location(2) translation: vec4<f32>, @location(3) surface: vec4<f32>, @location(4) color: vec4<f32>, @location(5) gradient: vec4<f32>, @location(6) options: vec4<f32>) -> Out {
  var out: Out;
  let physical = vec2<f32>(
    affine.x * local.x + affine.z * local.y + translation.x,
    affine.y * local.x + affine.w * local.y + translation.y,
  );
  out.position = vec4<f32>(physical.x / surface.x * 2. - 1., 1. - physical.y / surface.y * 2., 0., 1.);
  out.color = color;
  out.local = local;
  out.gradient = gradient;
  out.options = options;
  return out;
}
fn lookup(t: f32) -> vec4<f32> { let p=textureSampleLevel(gradient_lut,gradient_sampler,vec2(clamp(t,0.,1.),.5),0.); return select(vec4(0.),vec4(p.rgb/max(p.a,.00001),p.a),p.a>0.); }
@fragment fn fs_main(i: Out) -> @location(0) vec4<f32> { var t=0.; if(i.options.x==1.) { let d=i.gradient.zw-i.gradient.xy; t=clamp(dot(i.local-i.gradient.xy,d)/max(dot(d,d),.0001),0.,1.); } else if(i.options.x==2.) { t=clamp(length(i.local-i.gradient.xy)/max(i.gradient.z,.0001),0.,1.); } else if(i.options.x==3.) { t=fract((atan2(i.local.y-i.gradient.y,i.local.x-i.gradient.x)-i.gradient.z)/6.2831853); } return select(i.color,lookup(t),i.options.x>0.); }
"#;
const COMPOSITE_SHADER: &str = r#"
struct Out { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) alpha: f32, @location(2) color: vec4<f32>, @location(3) mode: f32 };
@group(0) @binding(0) var group_texture: texture_2d<f32>;
@group(0) @binding(1) var target_sampler: sampler;
@vertex fn vs_main(@location(0) quad: vec2<f32>, @location(1) rect: vec4<f32>, @location(2) uv: vec4<f32>, @location(3) alpha: vec4<f32>, @location(4) color: vec4<f32>, @location(5) options: vec4<f32>) -> Out {
  var out: Out;
  out.position = vec4<f32>(rect.xy + quad * rect.zw, 0., 1.);
  out.uv = uv.xy + quad * (uv.zw - uv.xy);
  out.alpha = alpha.x;
  out.color = color;
  out.mode = options.x;
  return out;
}
// Offscreen color is premultiplied. Multiplying both stored RGB and alpha by
// the group alpha exactly once preserves overlap semantics at the parent.
@fragment fn fs_main(input: Out) -> @location(0) vec4<f32> {
  let sample = textureSample(group_texture, target_sampler, input.uv);
  if (input.mode > 0.5) {
    let a = sample.a * input.color.a * input.alpha;
    return vec4<f32>(input.color.rgb * a, a);
  }
  return vec4<f32>(sample.rgb * input.alpha, sample.a * input.alpha);
}
"#;

const FIXED_BLEND_SHADER: &str = r#"
struct Out { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) alpha: f32 };
@group(0) @binding(0) var group_texture: texture_2d<f32>;
@group(0) @binding(1) var target_sampler: sampler;
@vertex fn vs_main(@location(0) quad: vec2<f32>, @location(1) rect: vec4<f32>, @location(2) uv: vec4<f32>, @location(3) alpha: vec4<f32>, @location(4) color: vec4<f32>, @location(5) options: vec4<f32>) -> Out {
  var out: Out;
  out.position = vec4<f32>(rect.xy + quad * rect.zw, 0., 1.);
  out.uv = uv.xy + quad * (uv.zw - uv.xy);
  out.alpha = alpha.x;
  return out;
}
@fragment fn fs_main(input: Out) -> @location(0) vec4<f32> {
  let sample = textureSample(group_texture, target_sampler, input.uv);
  return sample * input.alpha;
}
"#;

const BLUR_SHADER: &str = r#"
struct Params {
  source_origin: vec2<f32>,
  source_size: vec2<f32>,
  output_size: vec2<f32>,
  direction: vec2<f32>,
  radius: u32,
  mode: u32,
  _padding: vec2<u32>,
  weights: array<vec4<f32>, 16>,
};
struct Out { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> };
@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;
@group(0) @binding(2) var<uniform> params: Params;
@vertex fn vs_main(@location(0) quad: vec2<f32>) -> Out {
  var out: Out;
  out.position = vec4<f32>(quad * 2. - 1., 0., 1.);
  out.uv = quad;
  return out;
}
fn sample_transparent(px: vec2<f32>) -> vec4<f32> {
  if (px.x < 0. || px.y < 0. || px.x >= params.source_size.x || px.y >= params.source_size.y) {
    return vec4<f32>(0.);
  }
  return textureSampleLevel(source, source_sampler, (px + vec2<f32>(0.5)) / params.source_size, 0.);
}
@fragment fn fs_main(input: Out) -> @location(0) vec4<f32> {
  let out_px = input.uv * params.output_size;
  var result = vec4<f32>(0.);
  for (var i: i32 = -48; i <= 48; i = i + 1) {
    if (abs(i) <= i32(params.radius)) {
      let px = out_px + params.direction * f32(i) - params.source_origin;
      let weight_index = abs(i);
      result += sample_transparent(px) * params.weights[weight_index / 4][weight_index % 4];
    }
  }
  return result;
}
"#;

const RESAMPLE_SHADER: &str = r#"
struct Params {
  source_origin: vec2<f32>,
  source_size: vec2<f32>,
  output_size: vec2<f32>,
  direction: vec2<f32>,
  radius: u32,
  mode: u32,
  _padding: vec2<u32>,
  weights: array<vec4<f32>, 16>,
};
struct Out { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> };
@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;
@group(0) @binding(2) var<uniform> params: Params;
@vertex fn vs_main(@location(0) quad: vec2<f32>) -> Out {
  var out: Out;
  out.position = vec4<f32>(quad * 2. - 1., 0., 1.);
  out.uv = quad;
  return out;
}
@fragment fn fs_main(input: Out) -> @location(0) vec4<f32> {
  let out_px = input.uv * params.output_size;
  let px = out_px * params.direction - params.source_origin;
  if (px.x < 0. || px.y < 0. || px.x >= params.source_size.x || px.y >= params.source_size.y) {
    return vec4<f32>(0.);
  }
  return textureSampleLevel(source, source_sampler, (px + vec2<f32>(0.5)) / params.source_size, 0.);
}
"#;

const COLOR_MATRIX_SHADER: &str = r#"
struct Params { matrix: array<vec4<f32>, 5> };
struct Out { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> };
@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;
@group(0) @binding(2) var<uniform> params: Params;
@vertex fn vs_main(@location(0) quad: vec2<f32>) -> Out {
  var out: Out;
  out.position = vec4<f32>(quad * 2. - 1., 0., 1.);
  out.uv = quad;
  return out;
}
@fragment fn fs_main(input: Out) -> @location(0) vec4<f32> {
  let sample = textureSampleLevel(source, source_sampler, input.uv, 0.);
  let a = clamp(sample.a, 0., 1.);
  let straight_rgb = clamp(sample.rgb / max(a, .000001), vec3<f32>(0.), vec3<f32>(1.));
  let straight = select(vec4<f32>(0., 0., 0., a), vec4<f32>(straight_rgb, a), a > .000001);
  let c0 = params.matrix[0];
  let c1 = params.matrix[1];
  let c2 = params.matrix[2];
  let c3 = params.matrix[3];
  let c4 = params.matrix[4];
  let filtered = vec4<f32>(
    dot(c0, straight) + c1.x,
    c1.y * straight.x + c1.z * straight.y + c1.w * straight.z + c2.x * straight.w + c2.y,
    c2.z * straight.x + c2.w * straight.y + c3.x * straight.z + c3.y * straight.w + c3.z,
    c3.w * straight.x + c4.x * straight.y + c4.y * straight.z + c4.z * straight.w + c4.w
  );
  let out_a = clamp(filtered.a, 0., 1.);
  let out_rgb = clamp(filtered.rgb, vec3<f32>(0.), vec3<f32>(1.)) * out_a;
  return vec4<f32>(out_rgb, out_a);
}
"#;

const BLEND_SHADER: &str = r#"
struct Out { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) alpha: f32, @location(2) color: vec4<f32>, @location(3) options: vec4<f32> };
@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var destination: texture_2d<f32>;
@group(0) @binding(2) var blend_sampler: sampler;
@vertex fn vs_main(@location(0) quad: vec2<f32>, @location(1) rect: vec4<f32>, @location(2) uv: vec4<f32>, @location(3) alpha: vec4<f32>, @location(4) color: vec4<f32>, @location(5) options: vec4<f32>) -> Out {
  var out: Out;
  out.position = vec4<f32>(rect.xy + quad * rect.zw, 0., 1.);
  out.uv = uv.xy + quad * (uv.zw - uv.xy);
  out.alpha = alpha.x;
  out.color = color;
  out.options = options;
  return out;
}
fn straight_rgb(value: vec4<f32>) -> vec3<f32> {
  let alpha = clamp(value.a, 0., 1.);
  let rgb = clamp(value.rgb / max(alpha, .000001), vec3<f32>(0.), vec3<f32>(1.));
  return select(vec3<f32>(0.), rgb, alpha > .000001);
}
fn artistic(mode: u32, s: vec3<f32>, d: vec3<f32>) -> vec3<f32> {
  var out = s;
  if (mode == 11u) { out = s * d; }
  else if (mode == 12u) { out = s + d - s * d; }
  else if (mode == 13u) { out = select(2. * s * d, 1. - 2. * (1. - s) * (1. - d), d > vec3<f32>(.5)); }
  else if (mode == 14u) { out = min(s, d); }
  else if (mode == 15u) { out = max(s, d); }
  else if (mode == 16u) { out = select(min(d / max(1. - s, vec3<f32>(.000001)), vec3<f32>(1.)), vec3<f32>(1.), s >= vec3<f32>(1.)); }
  else if (mode == 17u) { out = select(max(1. - (1. - d) / max(s, vec3<f32>(.000001)), vec3<f32>(0.)), vec3<f32>(0.), s <= vec3<f32>(0.)); }
  else if (mode == 18u) { out = select(2. * s * d, 1. - 2. * (1. - s) * (1. - d), s > vec3<f32>(.5)); }
  else if (mode == 19u) {
    let low = d - (1. - 2. * s) * d * (1. - d);
    let g = select(sqrt(d), ((16. * d - 12.) * d + 4.) * d, d <= vec3<f32>(.25));
    out = select(low, d + (2. * s - 1.) * (g - d), s > vec3<f32>(.5));
  }
  else if (mode == 20u) { out = abs(d - s); }
  else if (mode == 21u) { out = s + d - 2. * s * d; }
  return clamp(out, vec3<f32>(0.), vec3<f32>(1.));
}
@fragment fn fs_main(input: Out) -> @location(0) vec4<f32> {
  let src_sample = textureSampleLevel(source, blend_sampler, input.uv, 0.) * input.alpha;
  let dst_uv = input.position.xy / max(input.options.yz, vec2<f32>(1.));
  let dst_sample = textureSampleLevel(destination, blend_sampler, dst_uv, 0.);
  let mode = u32(input.options.x + .5);
  let as_ = clamp(src_sample.a, 0., 1.);
  let ad = clamp(dst_sample.a, 0., 1.);
  let ao = clamp(as_ + ad - as_ * ad, 0., 1.);
  var out = vec4<f32>(0.);
  if (mode == 0u) { out = vec4<f32>(src_sample.rgb + dst_sample.rgb * (1. - as_), ao); }
  else if (mode == 1u) { out = src_sample; }
  else if (mode == 2u) { out = vec4<f32>(dst_sample.rgb + src_sample.rgb * (1. - ad), ao); }
  else if (mode == 3u) { out = vec4<f32>(src_sample.rgb * ad, as_ * ad); }
  else if (mode == 4u) { out = vec4<f32>(dst_sample.rgb * as_, ad * as_); }
  else if (mode == 5u) { out = vec4<f32>(src_sample.rgb * (1. - ad), as_ * (1. - ad)); }
  else if (mode == 6u) { out = vec4<f32>(dst_sample.rgb * (1. - as_), ad * (1. - as_)); }
  else if (mode == 7u) { out = vec4<f32>(src_sample.rgb * ad + dst_sample.rgb * (1. - as_), ad); }
  else if (mode == 8u) { out = vec4<f32>(dst_sample.rgb * as_ + src_sample.rgb * (1. - ad), as_); }
  else if (mode == 9u) { out = vec4<f32>(src_sample.rgb * (1. - ad) + dst_sample.rgb * (1. - as_), clamp(as_ + ad - 2. * as_ * ad, 0., 1.)); }
  else if (mode == 10u) { out = vec4<f32>(min(src_sample.rgb + dst_sample.rgb, vec3<f32>(1.)), min(as_ + ad, 1.)); }
  else {
    let blended = artistic(mode, straight_rgb(src_sample), straight_rgb(dst_sample));
    out = vec4<f32>(clamp(src_sample.rgb * (1. - ad) + dst_sample.rgb * (1. - as_) + as_ * ad * blended, vec3<f32>(0.), vec3<f32>(1.)), ao);
  }
  return clamp(out, vec4<f32>(0.), vec4<f32>(1.));
}
"#;

struct GpuAtlasPage {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
}
struct GpuImage {
    resource: Arc<SharedGpuImage>,
    bind_groups: HashMap<ImageSampling, wgpu::BindGroup>,
    last_used_frame: u64,
}
/// Resources that are immutable for a target format and can therefore be
/// cloned by every surface using that format. A clone is another handle to the
/// same `wgpu` object, not a duplicate GPU allocation.
#[derive(Clone)]
struct SharedPipelineResources {
    rectangle_pipeline: wgpu::RenderPipeline,
    text_pipeline: wgpu::RenderPipeline,
    image_pipeline: wgpu::RenderPipeline,
    rounded_rect_pipeline: wgpu::RenderPipeline,
    path_pipeline: wgpu::RenderPipeline,
    composite_pipeline: wgpu::RenderPipeline,
    fixed_blend_pipelines: Vec<wgpu::RenderPipeline>,
    blur_pipeline: wgpu::RenderPipeline,
    resample_pipeline: wgpu::RenderPipeline,
    color_matrix_pipeline: wgpu::RenderPipeline,
    blend_pipeline: wgpu::RenderPipeline,
    stencil_rrect_increment_pipeline: wgpu::RenderPipeline,
    stencil_rrect_decrement_pipeline: wgpu::RenderPipeline,
    stencil_path_increment_pipeline: wgpu::RenderPipeline,
    stencil_path_decrement_pipeline: wgpu::RenderPipeline,
    mesh: wgpu::Buffer,
    gradient_bind_group_layout: wgpu::BindGroupLayout,
    gradient_sampler: wgpu::Sampler,
    solid_gradient: GpuGradient,
    atlas_bind_group_layout: wgpu::BindGroupLayout,
    atlas_sampler: wgpu::Sampler,
    image_bind_group_layout: wgpu::BindGroupLayout,
    image_samplers: HashMap<ImageSampling, wgpu::Sampler>,
    composite_bind_group_layout: wgpu::BindGroupLayout,
    composite_sampler: wgpu::Sampler,
    blur_bind_group_layout: wgpu::BindGroupLayout,
    blur_sampler: wgpu::Sampler,
    color_matrix_bind_group_layout: wgpu::BindGroupLayout,
    color_matrix_sampler: wgpu::Sampler,
    blend_bind_group_layout: wgpu::BindGroupLayout,
    blend_sampler: wgpu::Sampler,
}
#[derive(Clone)]
struct OffscreenTarget {
    _color: wgpu::Texture,
    color_view: wgpu::TextureView,
    _stencil: wgpu::Texture,
    stencil_view: wgpu::TextureView,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    has_stencil: bool,
}
struct DestinationTargets {
    first: OffscreenTarget,
    second: OffscreenTarget,
    width: u32,
    height: u32,
}
impl OffscreenTarget {
    fn bytes(&self) -> usize {
        let color_bytes = self.width as usize * self.height as usize * 4;
        color_bytes + if self.has_stencil { color_bytes } else { 0 }
    }
}
#[derive(Default)]
struct OffscreenTargetPool {
    free: Vec<OffscreenTarget>,
    bytes: usize,
}
impl OffscreenTargetPool {
    const MAX_BYTES: usize = 16 * 1024 * 1024;

    fn take(
        &mut self,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        has_stencil: bool,
    ) -> Option<OffscreenTarget> {
        let index = self.free.iter().position(|target| {
            target.width == width
                && target.height == height
                && target.format == format
                && target.has_stencil == has_stencil
        })?;
        let target = self.free.swap_remove(index);
        self.bytes = self.bytes.saturating_sub(target.bytes());
        Some(target)
    }

    fn recycle(&mut self, target: OffscreenTarget) {
        let bytes = target.bytes();
        if self.bytes.saturating_add(bytes) > Self::MAX_BYTES {
            return;
        }
        self.bytes = self.bytes.saturating_add(bytes);
        self.free.push(target);
    }
}
struct OffscreenCacheEntry {
    target: OffscreenTarget,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
    scale_factor_bits: u32,
    generation: u64,
    device_generation: u64,
    last_used_frame: u64,
    bytes: usize,
}
struct EffectCacheEntry {
    target: OffscreenTarget,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
    scale_factor_bits: u32,
    source_generation: u64,
    sigma_x_bits: u32,
    sigma_y_bits: u32,
    matrix_bits: [u32; 20],
    device_generation: u64,
    last_used_frame: u64,
    bytes: usize,
    downsample_factor: u32,
}
#[derive(Clone, Copy, Debug)]
struct CachedSource {
    origin: Offset,
    width: u32,
    height: u32,
}
#[derive(Clone, Copy, Debug)]
struct CachedEffect {
    origin: Offset,
    width: u32,
    height: u32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
struct ClipRect {
    rect: Rect,
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum ClipState {
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
enum ClipMask {
    RRect(GpuRRectInstance),
    Path {
        key: PathMeshKey,
        instance: GpuPathInstance,
    },
}
#[derive(Clone, Debug)]
enum DrawBatch {
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
struct GlyphSurface {
    transform: Transform,
    width: f32,
    height: f32,
    scale: f32,
}
#[derive(Clone, Copy)]
struct PathPlacement {
    transform: Transform,
    scale: f32,
}

/// Owns one window's surface, transient buffers, and retained compositor
/// caches. Device-level resources are borrowed from [`SharedGpuContext`].
/// Colors and coverage are straight alpha and use ordinary source-alpha
/// blending.
pub struct WgpuRenderer {
    shared: SharedGpuContext,
    window_gpu: WindowGpuState,
    device: wgpu::Device,
    queue: wgpu::Queue,
    rectangle_pipeline: wgpu::RenderPipeline,
    text_pipeline: wgpu::RenderPipeline,
    image_pipeline: wgpu::RenderPipeline,
    rounded_rect_pipeline: wgpu::RenderPipeline,
    path_pipeline: wgpu::RenderPipeline,
    composite_pipeline: wgpu::RenderPipeline,
    fixed_blend_pipelines: Vec<wgpu::RenderPipeline>,
    blur_pipeline: wgpu::RenderPipeline,
    resample_pipeline: wgpu::RenderPipeline,
    color_matrix_pipeline: wgpu::RenderPipeline,
    blend_pipeline: wgpu::RenderPipeline,
    stencil_rrect_increment_pipeline: wgpu::RenderPipeline,
    stencil_rrect_decrement_pipeline: wgpu::RenderPipeline,
    stencil_path_increment_pipeline: wgpu::RenderPipeline,
    stencil_path_decrement_pipeline: wgpu::RenderPipeline,
    mesh: wgpu::Buffer,
    instances: wgpu::Buffer,
    instance_capacity: usize,
    glyph_instances: wgpu::Buffer,
    glyph_instance_capacity: usize,
    image_instances: wgpu::Buffer,
    image_instance_capacity: usize,
    rounded_rect_instances: wgpu::Buffer,
    rounded_rect_instance_capacity: usize,
    path_instances: wgpu::Buffer,
    path_instance_capacity: usize,
    composite_instances: wgpu::Buffer,
    composite_instance_capacity: usize,
    cpu_path_cache: HashMap<PathMeshKey, PathMesh>,
    gpu_path_cache: HashMap<PathMeshKey, GpuPathMesh>,
    gradient_bind_group_layout: wgpu::BindGroupLayout,
    gradient_sampler: wgpu::Sampler,
    gradient_cache: HashMap<GradientId, GpuGradient>,
    solid_gradient: GpuGradient,
    atlas_bind_group_layout: wgpu::BindGroupLayout,
    atlas_sampler: wgpu::Sampler,
    image_bind_group_layout: wgpu::BindGroupLayout,
    image_samplers: HashMap<ImageSampling, wgpu::Sampler>,
    image_cache: HashMap<ImageId, GpuImage>,
    composite_bind_group_layout: wgpu::BindGroupLayout,
    composite_sampler: wgpu::Sampler,
    blur_bind_group_layout: wgpu::BindGroupLayout,
    blur_sampler: wgpu::Sampler,
    blur_params: wgpu::Buffer,
    blur_kernel_cache: HashMap<BlurKernelKey, BlurKernel>,
    color_matrix_bind_group_layout: wgpu::BindGroupLayout,
    color_matrix_sampler: wgpu::Sampler,
    color_matrix_params: wgpu::Buffer,
    blend_bind_group_layout: wgpu::BindGroupLayout,
    blend_sampler: wgpu::Sampler,
    destination_targets: Option<DestinationTargets>,
    offscreen_cache: HashMap<incular_painting::LayerId, OffscreenCacheEntry>,
    effect_cache: HashMap<incular_painting::LayerId, EffectCacheEntry>,
    offscreen_target_pool: OffscreenTargetPool,
    offscreen_cache_budget: usize,
    device_generation: u64,
    target_width: u32,
    target_height: u32,
    target_origin: Offset,
    offscreen_nesting_depth: u64,
    atlas_pages: Vec<GpuAtlasPage>,
    counters: GpuCounters,
}
impl Deref for WgpuRenderer {
    type Target = WindowGpuState;

    fn deref(&self) -> &Self::Target {
        &self.window_gpu
    }
}
impl DerefMut for WgpuRenderer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.window_gpu
    }
}
impl WgpuRenderer {
    /// # Safety boundary
    /// `handles` must describe a window that outlives this renderer.
    pub async fn new(handles: RawWindowHandles, size: PhysicalSize) -> Result<Self, RendererError> {
        let shared = SharedGpuContext::new(handles).await?;
        Self::new_with_shared(shared, handles, size).await
    }
    /// Creates a renderer for one native window using an existing shared GPU
    /// device context. No `wgpu::Instance`, adapter, device, or queue is
    /// recreated by this method.
    pub async fn new_with_shared(
        shared: SharedGpuContext,
        handles: RawWindowHandles,
        size: PhysicalSize,
    ) -> Result<Self, RendererError> {
        let surface = shared.create_surface(handles)?;
        let device = shared.inner.device.clone();
        let queue = shared.inner.queue.clone();
        let texture_limit = device.limits().max_texture_dimension_2d;
        if texture_limit < u32::from(ATLAS_PAGE_SIZE) {
            return Err(RendererError::GlyphAtlasPageTooLarge {
                page: ATLAS_PAGE_SIZE,
                limit: texture_limit,
            });
        }
        let config = surface
            .get_default_config(&shared.inner.adapter, size.width.max(1), size.height.max(1))
            .expect("surface config");
        if !size.is_zero() {
            surface.configure(&device, &config);
        }
        if let Some(pipelines) = shared.pipeline_resources(config.format) {
            return Ok(Self::from_shared_pipeline_resources(
                shared, handles, surface, config, size, device, queue, pipelines,
            ));
        }
        let mesh = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("incular unit quad mesh"),
            contents: bytemuck::cast_slice(&[
                [0_f32, 0_f32],
                [1., 0.],
                [0., 1.],
                [0., 1.],
                [1., 0.],
                [1., 1.],
            ]),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let atlas_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("incular glyph atlas layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("incular glyph atlas sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            // Coverage masks can be positioned at fractional physical pixels.
            // Bilinear filtering preserves grayscale antialiasing; each atlas
            // allocation has a zero-coverage border to prevent glyph bleed.
            mag_filter: GLYPH_ATLAS_FILTER,
            min_filter: GLYPH_ATLAS_FILTER,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let image_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("incular image layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let composite_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("incular opacity composite layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let composite_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("incular opacity composite sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let blur_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("incular gaussian effect layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });
        let blur_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("incular gaussian linear sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let color_matrix_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("incular color matrix layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });
        let color_matrix_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("incular color matrix sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let blend_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("incular destination blend layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let blend_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("incular destination blend sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let gradient_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("incular gradient lookup layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let gradient_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("incular gradient lookup sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let solid_gradient = create_gradient_resource(
            &device,
            &queue,
            &gradient_bind_group_layout,
            &gradient_sampler,
            &[[255, 255, 255, 255]; 1],
        );
        let mut image_samplers = HashMap::new();
        for (sampling, filter) in [
            (ImageSampling::Linear, wgpu::FilterMode::Linear),
            (ImageSampling::Nearest, wgpu::FilterMode::Nearest),
        ] {
            image_samplers.insert(
                sampling,
                device.create_sampler(&wgpu::SamplerDescriptor {
                    label: Some("incular retained image sampler"),
                    address_mode_u: wgpu::AddressMode::ClampToEdge,
                    address_mode_v: wgpu::AddressMode::ClampToEdge,
                    address_mode_w: wgpu::AddressMode::ClampToEdge,
                    mag_filter: filter,
                    min_filter: filter,
                    mipmap_filter: wgpu::MipmapFilterMode::Nearest,
                    ..Default::default()
                }),
            );
        }
        let rectangle_pipeline = create_rectangle_pipeline(&device, config.format);
        let text_pipeline = create_text_pipeline(&device, config.format, &atlas_bind_group_layout);
        let image_pipeline =
            create_image_pipeline(&device, config.format, &image_bind_group_layout);
        let rounded_rect_pipeline =
            create_rounded_rect_pipeline(&device, config.format, &gradient_bind_group_layout);
        let path_pipeline =
            create_path_pipeline(&device, config.format, &gradient_bind_group_layout);
        let composite_pipeline =
            create_composite_pipeline(&device, config.format, &composite_bind_group_layout);
        let fixed_blend_pipelines: Vec<_> = [
            BlendMode::SrcOver,
            BlendMode::Src,
            BlendMode::DstOver,
            BlendMode::SrcIn,
            BlendMode::DstIn,
            BlendMode::SrcOut,
            BlendMode::DstOut,
            BlendMode::SrcAtop,
            BlendMode::DstAtop,
            BlendMode::Xor,
            BlendMode::Plus,
        ]
        .into_iter()
        .map(|mode| {
            create_fixed_blend_pipeline(&device, config.format, &composite_bind_group_layout, mode)
        })
        .collect();
        let blur_pipeline = create_effect_pipeline(
            &device,
            config.format,
            BLUR_SHADER,
            "incular separable gaussian blur pipeline",
            &blur_bind_group_layout,
        );
        let resample_pipeline = create_effect_pipeline(
            &device,
            config.format,
            RESAMPLE_SHADER,
            "incular effect resample pipeline",
            &blur_bind_group_layout,
        );
        let color_matrix_pipeline = create_effect_pipeline(
            &device,
            config.format,
            COLOR_MATRIX_SHADER,
            "incular color matrix pipeline",
            &color_matrix_bind_group_layout,
        );
        let blend_pipeline =
            create_blend_pipeline(&device, config.format, &blend_bind_group_layout);
        let stencil_rrect_increment_pipeline = create_rounded_rect_mask_pipeline(
            &device,
            config.format,
            &gradient_bind_group_layout,
            wgpu::StencilOperation::IncrementClamp,
        );
        let stencil_rrect_decrement_pipeline = create_rounded_rect_mask_pipeline(
            &device,
            config.format,
            &gradient_bind_group_layout,
            wgpu::StencilOperation::DecrementClamp,
        );
        let stencil_path_increment_pipeline = create_path_mask_pipeline(
            &device,
            config.format,
            &gradient_bind_group_layout,
            wgpu::StencilOperation::IncrementClamp,
        );
        let stencil_path_decrement_pipeline = create_path_mask_pipeline(
            &device,
            config.format,
            &gradient_bind_group_layout,
            wgpu::StencilOperation::DecrementClamp,
        );
        let (stencil_texture, stencil_view) =
            create_stencil_attachment(&device, config.width, config.height);
        let instances = create_instance_buffer(&device, 1);
        let glyph_instances = create_glyph_buffer(&device, 1);
        let image_instances = create_image_buffer(&device, 1);
        let rounded_rect_instances = create_rrect_buffer(&device, 1);
        let path_instances = create_path_instance_buffer(&device, 1);
        let composite_instances = create_composite_buffer(&device, 1);
        let blur_params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("incular gaussian parameters"),
            size: std::mem::size_of::<GpuBlurParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let color_matrix_params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("incular color matrix parameters"),
            size: std::mem::size_of::<GpuColorMatrixParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let target_width = config.width;
        let target_height = config.height;
        let device_generation = shared.inner.device_generation;
        let format = config.format;
        let shared_pipelines = SharedPipelineResources {
            rectangle_pipeline: rectangle_pipeline.clone(),
            text_pipeline: text_pipeline.clone(),
            image_pipeline: image_pipeline.clone(),
            rounded_rect_pipeline: rounded_rect_pipeline.clone(),
            path_pipeline: path_pipeline.clone(),
            composite_pipeline: composite_pipeline.clone(),
            fixed_blend_pipelines: fixed_blend_pipelines.clone(),
            blur_pipeline: blur_pipeline.clone(),
            resample_pipeline: resample_pipeline.clone(),
            color_matrix_pipeline: color_matrix_pipeline.clone(),
            blend_pipeline: blend_pipeline.clone(),
            stencil_rrect_increment_pipeline: stencil_rrect_increment_pipeline.clone(),
            stencil_rrect_decrement_pipeline: stencil_rrect_decrement_pipeline.clone(),
            stencil_path_increment_pipeline: stencil_path_increment_pipeline.clone(),
            stencil_path_decrement_pipeline: stencil_path_decrement_pipeline.clone(),
            mesh: mesh.clone(),
            gradient_bind_group_layout: gradient_bind_group_layout.clone(),
            gradient_sampler: gradient_sampler.clone(),
            solid_gradient: solid_gradient.clone(),
            atlas_bind_group_layout: atlas_bind_group_layout.clone(),
            atlas_sampler: atlas_sampler.clone(),
            image_bind_group_layout: image_bind_group_layout.clone(),
            image_samplers: image_samplers.clone(),
            composite_bind_group_layout: composite_bind_group_layout.clone(),
            composite_sampler: composite_sampler.clone(),
            blur_bind_group_layout: blur_bind_group_layout.clone(),
            blur_sampler: blur_sampler.clone(),
            color_matrix_bind_group_layout: color_matrix_bind_group_layout.clone(),
            color_matrix_sampler: color_matrix_sampler.clone(),
            blend_bind_group_layout: blend_bind_group_layout.clone(),
            blend_sampler: blend_sampler.clone(),
        };
        shared.register_pipeline_resources(format, shared_pipelines);
        Ok(Self {
            shared,
            window_gpu: WindowGpuState {
                handles,
                surface,
                config,
                stencil_texture,
                stencil_view,
                presentation: WindowGpuPresentation::new(size),
            },
            device,
            queue,
            rectangle_pipeline,
            text_pipeline,
            image_pipeline,
            rounded_rect_pipeline,
            path_pipeline,
            composite_pipeline,
            fixed_blend_pipelines,
            blur_pipeline,
            resample_pipeline,
            color_matrix_pipeline,
            blend_pipeline,
            stencil_rrect_increment_pipeline,
            stencil_rrect_decrement_pipeline,
            stencil_path_increment_pipeline,
            stencil_path_decrement_pipeline,
            mesh,
            instances,
            instance_capacity: 1,
            glyph_instances,
            glyph_instance_capacity: 1,
            image_instances,
            image_instance_capacity: 1,
            rounded_rect_instances,
            rounded_rect_instance_capacity: 1,
            path_instances,
            path_instance_capacity: 1,
            composite_instances,
            composite_instance_capacity: 1,
            cpu_path_cache: HashMap::new(),
            gpu_path_cache: HashMap::new(),
            gradient_bind_group_layout,
            gradient_sampler,
            gradient_cache: HashMap::new(),
            solid_gradient,
            atlas_bind_group_layout,
            atlas_sampler,
            image_bind_group_layout,
            image_samplers,
            image_cache: HashMap::new(),
            composite_bind_group_layout,
            composite_sampler,
            blur_bind_group_layout,
            blur_sampler,
            blur_params,
            blur_kernel_cache: HashMap::new(),
            color_matrix_bind_group_layout,
            color_matrix_sampler,
            color_matrix_params,
            blend_bind_group_layout,
            blend_sampler,
            destination_targets: None,
            offscreen_cache: HashMap::new(),
            effect_cache: HashMap::new(),
            offscreen_target_pool: OffscreenTargetPool::default(),
            offscreen_cache_budget: 64 * 1024 * 1024,
            device_generation,
            target_width,
            target_height,
            target_origin: Offset::ZERO,
            offscreen_nesting_depth: 0,
            atlas_pages: Vec::new(),
            counters: GpuCounters {
                rectangle_pipeline_creations: 1,
                text_pipeline_creations: 1,
                image_pipeline_creations: 1,
                path_pipeline_creations: 1,
                composite_pipeline_creations: 1,
                // Blur/resample share one stable pipeline family; these are
                // retained as explicit diagnostics for effect setup.
                stencil_texture_creations: 1,
                stencil_pipeline_creations: 4,
                ..GpuCounters::default()
            },
        })
    }
    #[allow(clippy::too_many_arguments)]
    fn from_shared_pipeline_resources(
        shared: SharedGpuContext,
        handles: RawWindowHandles,
        surface: wgpu::Surface<'static>,
        config: wgpu::SurfaceConfiguration,
        size: PhysicalSize,
        device: wgpu::Device,
        queue: wgpu::Queue,
        pipelines: Arc<SharedPipelineResources>,
    ) -> Self {
        let (stencil_texture, stencil_view) =
            create_stencil_attachment(&device, config.width, config.height);
        let instances = create_instance_buffer(&device, 1);
        let glyph_instances = create_glyph_buffer(&device, 1);
        let image_instances = create_image_buffer(&device, 1);
        let rounded_rect_instances = create_rrect_buffer(&device, 1);
        let path_instances = create_path_instance_buffer(&device, 1);
        let composite_instances = create_composite_buffer(&device, 1);
        let blur_params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("incular gaussian parameters"),
            size: std::mem::size_of::<GpuBlurParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let color_matrix_params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("incular color matrix parameters"),
            size: std::mem::size_of::<GpuColorMatrixParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let target_width = config.width;
        let target_height = config.height;
        let device_generation = shared.inner.device_generation;
        Self {
            shared,
            window_gpu: WindowGpuState {
                handles,
                surface,
                config,
                stencil_texture,
                stencil_view,
                presentation: WindowGpuPresentation::new(size),
            },
            device,
            queue,
            rectangle_pipeline: pipelines.rectangle_pipeline.clone(),
            text_pipeline: pipelines.text_pipeline.clone(),
            image_pipeline: pipelines.image_pipeline.clone(),
            rounded_rect_pipeline: pipelines.rounded_rect_pipeline.clone(),
            path_pipeline: pipelines.path_pipeline.clone(),
            composite_pipeline: pipelines.composite_pipeline.clone(),
            fixed_blend_pipelines: pipelines.fixed_blend_pipelines.clone(),
            blur_pipeline: pipelines.blur_pipeline.clone(),
            resample_pipeline: pipelines.resample_pipeline.clone(),
            color_matrix_pipeline: pipelines.color_matrix_pipeline.clone(),
            blend_pipeline: pipelines.blend_pipeline.clone(),
            stencil_rrect_increment_pipeline: pipelines.stencil_rrect_increment_pipeline.clone(),
            stencil_rrect_decrement_pipeline: pipelines.stencil_rrect_decrement_pipeline.clone(),
            stencil_path_increment_pipeline: pipelines.stencil_path_increment_pipeline.clone(),
            stencil_path_decrement_pipeline: pipelines.stencil_path_decrement_pipeline.clone(),
            mesh: pipelines.mesh.clone(),
            instances,
            instance_capacity: 1,
            glyph_instances,
            glyph_instance_capacity: 1,
            image_instances,
            image_instance_capacity: 1,
            rounded_rect_instances,
            rounded_rect_instance_capacity: 1,
            path_instances,
            path_instance_capacity: 1,
            composite_instances,
            composite_instance_capacity: 1,
            cpu_path_cache: HashMap::new(),
            gpu_path_cache: HashMap::new(),
            gradient_bind_group_layout: pipelines.gradient_bind_group_layout.clone(),
            gradient_sampler: pipelines.gradient_sampler.clone(),
            gradient_cache: HashMap::new(),
            solid_gradient: pipelines.solid_gradient.clone(),
            atlas_bind_group_layout: pipelines.atlas_bind_group_layout.clone(),
            atlas_sampler: pipelines.atlas_sampler.clone(),
            image_bind_group_layout: pipelines.image_bind_group_layout.clone(),
            image_samplers: pipelines.image_samplers.clone(),
            image_cache: HashMap::new(),
            composite_bind_group_layout: pipelines.composite_bind_group_layout.clone(),
            composite_sampler: pipelines.composite_sampler.clone(),
            blur_bind_group_layout: pipelines.blur_bind_group_layout.clone(),
            blur_sampler: pipelines.blur_sampler.clone(),
            blur_params,
            blur_kernel_cache: HashMap::new(),
            color_matrix_bind_group_layout: pipelines.color_matrix_bind_group_layout.clone(),
            color_matrix_sampler: pipelines.color_matrix_sampler.clone(),
            color_matrix_params,
            blend_bind_group_layout: pipelines.blend_bind_group_layout.clone(),
            blend_sampler: pipelines.blend_sampler.clone(),
            destination_targets: None,
            offscreen_cache: HashMap::new(),
            effect_cache: HashMap::new(),
            offscreen_target_pool: OffscreenTargetPool::default(),
            offscreen_cache_budget: 64 * 1024 * 1024,
            device_generation,
            target_width,
            target_height,
            target_origin: Offset::ZERO,
            offscreen_nesting_depth: 0,
            atlas_pages: Vec::new(),
            counters: GpuCounters {
                stencil_texture_creations: 1,
                stencil_pipeline_creations: 0,
                ..GpuCounters::default()
            },
        }
    }
    #[must_use]
    pub fn counters(&self) -> GpuCounters {
        let mut counters = self.counters;
        let atlas = self.shared.glyph_counters();
        counters.glyph_cache_hits = atlas.glyph_cache_hits;
        counters.glyph_cache_misses = atlas.glyph_cache_misses;
        counters.glyphs_rasterized = atlas.glyphs_rasterized;
        counters.glyphs_skipped = atlas.glyphs_skipped;
        counters.glyph_atlas_uploads = atlas.glyph_atlas_uploads;
        counters.glyph_atlas_pages = atlas.glyph_atlas_pages;
        counters.micro_glyph_rasters = atlas.micro_glyph_rasters;
        counters.small_glyph_rasters = atlas.small_glyph_rasters;
        counters.normal_glyph_rasters = atlas.normal_glyph_rasters;
        counters.large_glyph_rasters = atlas.large_glyph_rasters;
        counters.huge_glyph_rasters = atlas.huge_glyph_rasters;
        counters.oversize_glyph_rasters = atlas.oversize_glyph_rasters;
        counters.font_parser_cache_hits = atlas.font_parser_cache_hits;
        counters.font_parser_cache_misses = atlas.font_parser_cache_misses;
        counters.rasterizer_errors = atlas.rasterizer_errors;
        counters.raster_time_total = atlas.raster_time_total;
        counters.oversize_cache_hits = atlas.oversize_cache_hits;
        counters.oversize_cache_misses = atlas.oversize_cache_misses;
        counters
    }
    /// The shared application-owned GPU context used by this window.
    #[must_use]
    pub fn shared_context(&self) -> SharedGpuContext {
        self.shared.clone()
    }
    /// Presentation state owned by this renderer's window only.
    #[must_use]
    pub const fn window_gpu_state(&self) -> &WindowGpuState {
        &self.window_gpu
    }
    /// Returns effect cache state for the latest lowered command stream.
    /// This is intentionally textual and exposes no GPU handles; it is useful
    /// alongside `LayerTree::debug_tree()` when diagnosing retained effects.
    #[must_use]
    pub fn effect_debug_tree(&self, list: &DisplayList, scale_factor: f64) -> String {
        let scale = normalized_scale(scale_factor);
        let mut out = String::new();
        for command in list.commands() {
            match command {
                PaintCommand::PushBlur {
                    layer,
                    blur,
                    bounds,
                    ..
                } => {
                    let source_warm = self.offscreen_cache.contains_key(layer);
                    let result = self.effect_cache.get(layer);
                    let physical = result
                        .map(|entry| (entry.width, entry.height))
                        .unwrap_or_else(|| {
                            physical_debug_size(
                                blur_bounds(*bounds, blur.sigma_x, blur.sigma_y),
                                scale,
                            )
                        });
                    let _ = writeln!(
                        out,
                        "Blur#{layer:?} sigma=({:.3},{:.3}) source_cache={} blur_cache={} physical_bounds={}x{}",
                        blur.sigma_x,
                        blur.sigma_y,
                        if source_warm { "warm" } else { "cold" },
                        if blur.sigma_x <= f32::EPSILON && blur.sigma_y <= f32::EPSILON {
                            "identity"
                        } else if result.is_some() {
                            "warm"
                        } else {
                            "cold"
                        },
                        physical.0,
                        physical.1,
                    );
                }
                PaintCommand::PushDropShadow {
                    layer,
                    shadow,
                    bounds,
                    ..
                } => {
                    let source_warm = self.offscreen_cache.contains_key(layer);
                    let result = self.effect_cache.get(layer);
                    let physical = result
                        .map(|entry| (entry.width, entry.height))
                        .unwrap_or_else(|| {
                            physical_debug_size(
                                blur_bounds(*bounds, shadow.sigma_x, shadow.sigma_y),
                                scale,
                            )
                        });
                    let _ = writeln!(
                        out,
                        "DropShadow#{layer:?} sigma=({:.3},{:.3}) offset=({:.3},{:.3}) source_cache={} mask_cache={} physical_bounds={}x{}",
                        shadow.sigma_x,
                        shadow.sigma_y,
                        shadow.offset.x,
                        shadow.offset.y,
                        if source_warm { "warm" } else { "cold" },
                        if shadow.sigma_x <= f32::EPSILON && shadow.sigma_y <= f32::EPSILON {
                            "identity"
                        } else if result.is_some() {
                            "warm"
                        } else {
                            "cold"
                        },
                        physical.0,
                        physical.1,
                    );
                }
                PaintCommand::PushColorFilter { layer, filter, .. } => {
                    let source_warm = self.offscreen_cache.contains_key(layer);
                    let result = self.effect_cache.get(layer);
                    let matrix = filter.to_matrix();
                    let cache = if filter.is_identity() {
                        "identity"
                    } else if result.is_some() {
                        "warm"
                    } else {
                        "cold"
                    };
                    let _ = writeln!(
                        out,
                        "ColorMatrix#{layer:?} source_cache={} stage_cache={} m00={:.3} m11={:.3} m22={:.3} m33={:.3}",
                        if source_warm { "warm" } else { "cold" },
                        cache,
                        matrix[0],
                        matrix[6],
                        matrix[12],
                        matrix[18],
                    );
                }
                PaintCommand::PushBlend { layer, mode, .. } => {
                    let source_warm = self.offscreen_cache.contains_key(layer);
                    let _ = writeln!(
                        out,
                        "Blend#{layer:?} mode={mode:?} source_cache={} path={}",
                        if source_warm { "warm" } else { "cold" },
                        if mode.requires_destination_read() {
                            "destination-read"
                        } else {
                            "fixed-function-compatible"
                        },
                    );
                }
                _ => {}
            }
        }
        out
    }
    #[must_use]
    pub fn physical_size(&self) -> PhysicalSize {
        self.window_gpu.presentation.physical_size
    }
    pub fn resize(&mut self, size: PhysicalSize) {
        if !self.window_gpu.presentation.resize(size) {
            return;
        }
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);
        (self.stencil_texture, self.stencil_view) =
            create_stencil_attachment(&self.device, size.width, size.height);
        self.counters.stencil_texture_recreations += 1;
        self.destination_targets = None;
        self.counters.blend_intermediate_cached_bytes = 0;
    }

    fn ensure_destination_targets(&mut self, width: u32, height: u32) {
        if self
            .destination_targets
            .as_ref()
            .is_some_and(|targets| targets.width == width && targets.height == height)
        {
            self.counters.blend_target_reuses += 1;
            return;
        }
        self.destination_targets = Some(DestinationTargets {
            first: self.create_offscreen_target(
                width,
                height,
                "incular destination composition target A",
            ),
            second: self.create_offscreen_target(
                width,
                height,
                "incular destination composition target B",
            ),
            width,
            height,
        });
        let bytes = self
            .destination_targets
            .as_ref()
            .map_or(0, |targets| targets.first.bytes() + targets.second.bytes());
        self.counters.blend_intermediate_cached_bytes = bytes;
        self.counters.blend_intermediate_peak_bytes =
            self.counters.blend_intermediate_peak_bytes.max(bytes);
        self.counters.blend_intermediate_target_creations += 2;
    }

    fn copy_color_texture(
        &self,
        source: &OffscreenTarget,
        destination: &OffscreenTarget,
        width: u32,
        height: u32,
        label: &'static str,
    ) {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) });
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &source._color,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &destination._color,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(Some(encoder.finish()));
    }

    fn prepare_batch_capacity(&mut self, batches: &[DrawBatch]) {
        self.ensure_rectangle_capacity(count_rectangles(batches));
        self.ensure_glyph_capacity(count_glyphs(batches));
        self.ensure_image_capacity(count_images(batches));
        self.ensure_composite_capacity(count_composites(batches));
        let rounded = count_rounded(batches);
        if rounded > self.rounded_rect_instance_capacity {
            self.rounded_rect_instance_capacity = rounded.next_power_of_two();
            self.rounded_rect_instances =
                create_rrect_buffer(&self.device, self.rounded_rect_instance_capacity);
        }
        let paths = count_paths(batches);
        if paths > self.path_instance_capacity {
            self.path_instance_capacity = paths.next_power_of_two();
            self.path_instances =
                create_path_instance_buffer(&self.device, self.path_instance_capacity);
        }
    }

    /// Rebuilds the stencil state that is active at a destination-promotion
    /// boundary.  Each ping-pong pass starts with a cleared stencil attachment,
    /// so masks pushed before the boundary have to be replayed before the next
    /// painter segment.  The replay list contains only increment operations;
    /// balanced decrements in the segment itself still update the new pass's
    /// state normally.
    fn destination_segment(
        batches: &[DrawBatch],
        start: usize,
        end: usize,
        active_start: &[DrawBatch],
    ) -> (Vec<DrawBatch>, Vec<DrawBatch>) {
        let mut active = active_start.to_vec();
        let mut segment = active_start.to_vec();
        for batch in &batches[start..end] {
            segment.push(batch.clone());
            match batch {
                DrawBatch::StencilRRect {
                    increment: true, ..
                }
                | DrawBatch::StencilPath {
                    increment: true, ..
                } => active.push(batch.clone()),
                DrawBatch::StencilRRect {
                    increment: false, ..
                }
                | DrawBatch::StencilPath {
                    increment: false, ..
                } => {
                    let _ = active.pop();
                }
                _ => {}
            }
        }
        (segment, active)
    }

    /// Renders an ordered stream that contains destination-reading blend
    /// batches using two sampleable composition targets.  Each segment is
    /// painted into the current target, copied to the alternate target before
    /// a blend draw, and the blend shader samples the untouched current target.
    /// This keeps the read and write textures distinct while preserving
    /// painter order and clip/stencil state.
    #[allow(clippy::too_many_arguments)]
    fn render_destination_batches(
        &mut self,
        batches: &[DrawBatch],
        scale: f32,
        width: u32,
        height: u32,
        targets: &mut DestinationTargets,
        clear: wgpu::Color,
        label: &'static str,
    ) -> (bool, u32, u32, u32) {
        let mut current_first = true;
        let mut first_pass = true;
        let mut segment_start = 0_usize;
        let mut draw_calls = 0_u32;
        let mut text_draw_calls = 0_u32;
        let mut passes = 0_u32;
        let mut active_stencils = Vec::new();

        for (index, batch) in batches.iter().enumerate() {
            if !matches!(
                batch,
                DrawBatch::Blend {
                    mode,
                    ..
                } if mode.requires_destination_read()
            ) {
                continue;
            }
            let (segment, next_active_stencils) =
                Self::destination_segment(batches, segment_start, index, &active_stencils);
            let (current, _) = if current_first {
                (&targets.first, &targets.second)
            } else {
                (&targets.second, &targets.first)
            };
            self.prepare_batch_capacity(&segment);
            self.upload_instance_data(&segment, scale, width, height);
            self.prepare_image_bind_groups(&segment);
            let current_view = current.color_view.clone();
            let current_stencil = current.stencil_view.clone();
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) });
            let (draws, text) = self.encode_batches(
                &mut encoder,
                &current_view,
                &current_stencil,
                &segment,
                width,
                height,
                scale,
                clear,
                None,
                !first_pass,
            );
            self.queue.submit(Some(encoder.finish()));
            draw_calls += draws;
            text_draw_calls += text;
            passes += 1;
            first_pass = false;
            active_stencils = next_active_stencils;

            let (current, alternate) = if current_first {
                (&targets.first, &targets.second)
            } else {
                (&targets.second, &targets.first)
            };
            self.copy_color_texture(
                current,
                alternate,
                width,
                height,
                "incular destination composition copy",
            );
            let mut blend_segment = active_stencils.clone();
            blend_segment.push(batch.clone());
            self.prepare_batch_capacity(&blend_segment);
            self.upload_instance_data(&blend_segment, scale, width, height);
            self.prepare_image_bind_groups(&blend_segment);
            let current_view = current.color_view.clone();
            let alternate_view = alternate.color_view.clone();
            let alternate_stencil = alternate.stencil_view.clone();
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("incular destination blend pass"),
                });
            let (draws, text) = self.encode_batches(
                &mut encoder,
                &alternate_view,
                &alternate_stencil,
                &blend_segment,
                width,
                height,
                scale,
                clear,
                Some(&current_view),
                true,
            );
            self.queue.submit(Some(encoder.finish()));
            draw_calls += draws;
            text_draw_calls += text;
            passes += 1;
            current_first = !current_first;
            segment_start = index + 1;
        }

        let (trailing, _) =
            Self::destination_segment(batches, segment_start, batches.len(), &active_stencils);
        let (current, _) = if current_first {
            (&targets.first, &targets.second)
        } else {
            (&targets.second, &targets.first)
        };
        self.prepare_batch_capacity(&trailing);
        self.upload_instance_data(&trailing, scale, width, height);
        self.prepare_image_bind_groups(&trailing);
        let current_view = current.color_view.clone();
        let current_stencil = current.stencil_view.clone();
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) });
        let (draws, text) = self.encode_batches(
            &mut encoder,
            &current_view,
            &current_stencil,
            &trailing,
            width,
            height,
            scale,
            clear,
            None,
            !first_pass,
        );
        self.queue.submit(Some(encoder.finish()));
        draw_calls += draws;
        text_draw_calls += text;
        passes += 1;
        (current_first, draw_calls, text_draw_calls, passes)
    }

    #[allow(clippy::too_many_arguments)]
    fn present_composition_target(
        &mut self,
        target: &OffscreenTarget,
        frame_view: &wgpu::TextureView,
        target_origin: Offset,
        target_width: u32,
        target_height: u32,
        parent_width: u32,
        parent_height: u32,
        scale: f32,
    ) -> u32 {
        self.ensure_composite_capacity(1);
        let instance = composite_instance(
            target_origin,
            Offset::ZERO,
            parent_width,
            parent_height,
            target_width,
            target_height,
            scale,
            1.,
        );
        self.queue
            .write_buffer(&self.composite_instances, 0, bytemuck::bytes_of(&instance));
        let bind_group = self.create_composite_bind_group(
            target,
            "incular destination composition presentation bind group",
        );
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("incular destination composition presentation"),
            });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("incular destination composition presentation pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: frame_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.04,
                        g: 0.04,
                        b: 0.06,
                        a: 1.,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.stencil_view,
                depth_ops: None,
                stencil_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0),
                    store: wgpu::StoreOp::Store,
                }),
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_stencil_reference(0);
        pass.set_pipeline(&self.composite_pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.set_vertex_buffer(0, self.mesh.slice(..));
        pass.set_vertex_buffer(
            1,
            self.composite_instances
                .slice(..std::mem::size_of::<GpuCompositeInstance>() as u64),
        );
        pass.draw(0..6, 0..1);
        drop(pass);
        self.queue.submit(Some(encoder.finish()));
        1
    }
    pub fn render(
        &mut self,
        list: &DisplayList,
        scale_factor: f64,
    ) -> Result<RenderStats, RendererError> {
        let stats = self.render_composited(list, scale_factor)?;
        if stats.presented {
            self.window_gpu.presentation.record_present();
        } else {
            self.window_gpu.presentation.record_skipped();
        }
        Ok(stats)
    }
    #[allow(dead_code)]
    fn render_legacy(
        &mut self,
        list: &DisplayList,
        scale_factor: f64,
    ) -> Result<RenderStats, RendererError> {
        if !self.window_gpu.presentation.configured {
            return Ok(RenderStats::default());
        }
        let scale = normalized_scale(scale_factor);
        let batches = self.lower_draw_batches(list, scale)?;
        let rectangles = batches
            .iter()
            .map(|b| {
                if let DrawBatch::Rectangles { clip, instances } = b
                    && *clip != ClipState::Empty
                {
                    instances.len()
                } else {
                    0
                }
            })
            .sum();
        let glyphs = batches
            .iter()
            .map(|b| {
                if let DrawBatch::Glyphs {
                    clip, instances, ..
                } = b
                    && *clip != ClipState::Empty
                {
                    instances.len()
                } else {
                    0
                }
            })
            .sum();
        let images = batches
            .iter()
            .map(|b| {
                if let DrawBatch::Images {
                    clip, instances, ..
                } = b
                    && *clip != ClipState::Empty
                {
                    instances.len()
                } else {
                    0
                }
            })
            .sum();
        let rounded: usize = batches
            .iter()
            .map(|b| {
                if let DrawBatch::RoundedRects {
                    clip, instances, ..
                } = b
                    && *clip != ClipState::Empty
                {
                    instances.len()
                } else {
                    0
                }
            })
            .sum::<usize>() + batches.iter().filter(|b| matches!(b, DrawBatch::StencilRRect { clip, .. } if *clip != ClipState::Empty)).count();
        let paths: usize = batches
            .iter()
            .filter(
                |batch| matches!(batch, DrawBatch::Path { clip, .. } if *clip != ClipState::Empty),
            )
            .count() + batches.iter().filter(|b| matches!(b, DrawBatch::StencilPath { clip, .. } if *clip != ClipState::Empty)).count();
        let rectangle_reallocated = self.ensure_rectangle_capacity(rectangles);
        let glyph_reallocated = self.ensure_glyph_capacity(glyphs);
        let image_reallocated = self.ensure_image_capacity(images);
        if rounded > self.rounded_rect_instance_capacity {
            self.rounded_rect_instance_capacity = rounded.next_power_of_two();
            self.rounded_rect_instances =
                create_rrect_buffer(&self.device, self.rounded_rect_instance_capacity);
        }
        if paths > self.path_instance_capacity {
            self.path_instance_capacity = paths.next_power_of_two();
            self.path_instances =
                create_path_instance_buffer(&self.device, self.path_instance_capacity);
        }
        self.upload_instance_data(&batches, scale, self.config.width, self.config.height);
        self.prepare_image_bind_groups(&batches);
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => frame,
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                self.surface.configure(&self.device, &self.config);
                frame
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return Ok(RenderStats::default());
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                self.surface = self.shared.create_surface(self.handles)?;
                self.surface.configure(&self.device, &self.config);
                self.window_gpu.presentation.surface_lost();
                return Ok(RenderStats::default());
            }
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => return Ok(RenderStats::default()),
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("incular frame encoder"),
            });
        let mut draw_calls = 0_u32;
        let mut text_draw_calls = 0_u32;
        let mut rectangle_offset = 0_u64;
        let mut glyph_offset = 0_u64;
        let mut image_offset = 0_u64;
        let mut rounded_offset = 0_u64;
        let mut path_offset = 0_u64;
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("incular ordered UI pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.04,
                            g: 0.04,
                            b: 0.06,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.stencil_view,
                    depth_ops: None,
                    stencil_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0),
                        store: wgpu::StoreOp::Store,
                    }),
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            for batch in &batches {
                let clip = match batch {
                    DrawBatch::Rectangles { clip, .. }
                    | DrawBatch::Glyphs { clip, .. }
                    | DrawBatch::Images { clip, .. }
                    | DrawBatch::RoundedRects { clip, .. }
                    | DrawBatch::Path { clip, .. }
                    | DrawBatch::StencilRRect { clip, .. }
                    | DrawBatch::StencilPath { clip, .. }
                    | DrawBatch::Offscreen { clip, .. }
                    | DrawBatch::Filtered { clip, .. }
                    | DrawBatch::Shadow { clip, .. }
                    | DrawBatch::Blend { clip, .. } => *clip,
                };
                if !set_scissor(
                    &mut pass,
                    clip,
                    self.config.width,
                    self.config.height,
                    scale,
                ) {
                    // Instance data was uploaded in display-list order even
                    // when a scissor is wholly outside the surface. Consume
                    // its slot so a later visible path keeps its paint data.
                    if matches!(
                        batch,
                        DrawBatch::Path { .. } | DrawBatch::StencilPath { .. }
                    ) {
                        path_offset += std::mem::size_of::<GpuPathInstance>() as u64;
                    }
                    if matches!(batch, DrawBatch::StencilRRect { .. }) {
                        rounded_offset += std::mem::size_of::<GpuRRectInstance>() as u64;
                    }
                    self.counters.clip_culled_draws += 1;
                    continue;
                }
                pass.set_stencil_reference(u32::from(stencil_depth(clip)));
                match batch {
                    DrawBatch::Rectangles { instances, .. } if !instances.is_empty() => {
                        let start = rectangle_offset;
                        rectangle_offset +=
                            (instances.len() * std::mem::size_of::<GpuInstance>()) as u64;
                        pass.set_pipeline(&self.rectangle_pipeline);
                        pass.set_vertex_buffer(0, self.mesh.slice(..));
                        pass.set_vertex_buffer(1, self.instances.slice(start..rectangle_offset));
                        pass.draw(0..6, 0..instances.len() as u32);
                        draw_calls += 1;
                    }
                    DrawBatch::Glyphs {
                        page, instances, ..
                    } if !instances.is_empty() => {
                        let Some(atlas_page) = self.atlas_pages.get(usize::from(*page)) else {
                            continue;
                        };
                        let start = glyph_offset;
                        glyph_offset +=
                            (instances.len() * std::mem::size_of::<GpuGlyphInstance>()) as u64;
                        pass.set_pipeline(&self.text_pipeline);
                        pass.set_bind_group(0, &atlas_page.bind_group, &[]);
                        pass.set_vertex_buffer(0, self.mesh.slice(..));
                        pass.set_vertex_buffer(1, self.glyph_instances.slice(start..glyph_offset));
                        pass.draw(0..6, 0..instances.len() as u32);
                        draw_calls += 1;
                        text_draw_calls += 1;
                    }
                    DrawBatch::Images {
                        image,
                        sampling,
                        instances,
                        ..
                    } if !instances.is_empty() => {
                        let Some(bind_group) = self.image_bind_group(*image, *sampling) else {
                            continue;
                        };
                        let start = image_offset;
                        image_offset +=
                            (instances.len() * std::mem::size_of::<GpuImageInstance>()) as u64;
                        pass.set_pipeline(&self.image_pipeline);
                        pass.set_bind_group(0, bind_group, &[]);
                        pass.set_vertex_buffer(0, self.mesh.slice(..));
                        pass.set_vertex_buffer(1, self.image_instances.slice(start..image_offset));
                        pass.draw(0..6, 0..instances.len() as u32);
                        draw_calls += 1;
                        self.counters.image_draw_calls += 1;
                    }
                    DrawBatch::RoundedRects {
                        gradient,
                        instances,
                        ..
                    } if !instances.is_empty() => {
                        let start = rounded_offset;
                        rounded_offset +=
                            (instances.len() * std::mem::size_of::<GpuRRectInstance>()) as u64;
                        pass.set_pipeline(&self.rounded_rect_pipeline);
                        pass.set_bind_group(0, self.gradient_bind_group(*gradient), &[]);
                        pass.set_vertex_buffer(0, self.mesh.slice(..));
                        pass.set_vertex_buffer(
                            1,
                            self.rounded_rect_instances.slice(start..rounded_offset),
                        );
                        pass.draw(0..6, 0..instances.len() as u32);
                        draw_calls += 1;
                    }
                    DrawBatch::Path { key, gradient, .. } => {
                        if let Some(mesh) = self.gpu_path_cache.get_mut(key) {
                            mesh.last_used_frame = self.counters.frames;
                        } else {
                            continue;
                        }
                        let gradient_bind_group = self.gradient_bind_group(*gradient);
                        let Some(mesh) = self.gpu_path_cache.get(key) else {
                            continue;
                        };
                        pass.set_pipeline(&self.path_pipeline);
                        pass.set_bind_group(0, gradient_bind_group, &[]);
                        pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                        let instance_start = path_offset;
                        path_offset += std::mem::size_of::<GpuPathInstance>() as u64;
                        pass.set_vertex_buffer(
                            1,
                            self.path_instances.slice(instance_start..path_offset),
                        );
                        pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                        pass.draw_indexed(0..mesh.index_count, 0, 0..1);
                        draw_calls += 1;
                        self.counters.path_draw_calls += 1;
                        self.counters.path_triangles += u64::from(mesh.index_count / 3);
                    }
                    DrawBatch::StencilRRect { increment, .. } => {
                        let start = rounded_offset;
                        rounded_offset += std::mem::size_of::<GpuRRectInstance>() as u64;
                        pass.set_pipeline(if *increment {
                            &self.stencil_rrect_increment_pipeline
                        } else {
                            &self.stencil_rrect_decrement_pipeline
                        });
                        pass.set_bind_group(0, self.gradient_bind_group(None), &[]);
                        pass.set_vertex_buffer(0, self.mesh.slice(..));
                        pass.set_vertex_buffer(
                            1,
                            self.rounded_rect_instances.slice(start..rounded_offset),
                        );
                        pass.draw(0..6, 0..1);
                        draw_calls += 1;
                        self.counters.stencil_mask_draws += 1;
                    }
                    DrawBatch::StencilPath { key, increment, .. } => {
                        let Some(mesh) = self.gpu_path_cache.get(key) else {
                            continue;
                        };
                        let start = path_offset;
                        path_offset += std::mem::size_of::<GpuPathInstance>() as u64;
                        pass.set_pipeline(if *increment {
                            &self.stencil_path_increment_pipeline
                        } else {
                            &self.stencil_path_decrement_pipeline
                        });
                        pass.set_bind_group(0, self.gradient_bind_group(None), &[]);
                        pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                        pass.set_vertex_buffer(1, self.path_instances.slice(start..path_offset));
                        pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                        pass.draw_indexed(0..mesh.index_count, 0, 0..1);
                        draw_calls += 1;
                        self.counters.stencil_mask_draws += 1;
                    }
                    _ => {}
                }
            }
        }
        self.queue.submit(Some(encoder.finish()));
        self.queue.present(frame);
        self.counters.frames += 1;
        self.evict_unused_images();
        self.evict_unused_path_meshes();
        self.evict_unused_gradients();
        self.counters.draw_calls += u64::from(draw_calls);
        self.counters.text_draw_calls += u64::from(text_draw_calls);
        self.counters.rectangle_instances += rectangles as u64;
        self.counters.glyph_instances += glyphs as u64;
        self.counters.image_instances += images as u64;
        self.counters.rounded_rect_instances += rounded as u64;
        Ok(RenderStats {
            draw_calls,
            rectangle_instances: rectangles as u32,
            glyph_instances: glyphs as u32,
            image_instances: images as u32,
            buffer_reallocated: rectangle_reallocated,
            glyph_buffer_reallocated: glyph_reallocated,
            image_buffer_reallocated: image_reallocated,
            presented: true,
        })
    }
    fn render_composited(
        &mut self,
        list: &DisplayList,
        scale_factor: f64,
    ) -> Result<RenderStats, RendererError> {
        if !self.window_gpu.presentation.configured {
            return Ok(RenderStats::default());
        }
        let scale = normalized_scale(scale_factor);
        let promote_destination = commands_have_destination_blend(list.commands());
        let (target_origin, target_width, target_height) = if promote_destination {
            destination_composition_scope(list, scale, self.config.width, self.config.height)
        } else {
            (Offset::ZERO, self.config.width, self.config.height)
        };
        self.target_width = target_width;
        self.target_height = target_height;
        self.target_origin = target_origin;
        let batches = self.lower_commands(
            list.commands(),
            scale,
            Transform::translation(Offset::new(-target_origin.x, -target_origin.y)),
            ClipState::Unbounded,
        )?;
        let rectangles = batches
            .iter()
            .filter_map(|batch| match batch {
                DrawBatch::Rectangles { clip, instances } if *clip != ClipState::Empty => {
                    Some(instances.len())
                }
                _ => None,
            })
            .sum::<usize>();
        let glyphs = batches
            .iter()
            .filter_map(|batch| match batch {
                DrawBatch::Glyphs {
                    clip, instances, ..
                } if *clip != ClipState::Empty => Some(instances.len()),
                _ => None,
            })
            .sum::<usize>();
        let images = batches
            .iter()
            .filter_map(|batch| match batch {
                DrawBatch::Images {
                    clip, instances, ..
                } if *clip != ClipState::Empty => Some(instances.len()),
                _ => None,
            })
            .sum::<usize>();
        let rounded = batches
            .iter()
            .map(|batch| match batch {
                DrawBatch::RoundedRects {
                    clip, instances, ..
                } if *clip != ClipState::Empty => instances.len(),
                DrawBatch::StencilRRect { clip, .. } if *clip != ClipState::Empty => 1,
                _ => 0,
            })
            .sum::<usize>();
        let paths = batches
            .iter()
            .map(|batch| match batch {
                DrawBatch::Path { clip, .. } | DrawBatch::StencilPath { clip, .. }
                    if *clip != ClipState::Empty =>
                {
                    1
                }
                _ => 0,
            })
            .sum::<usize>();
        let composites = batches
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
            .count();
        let rectangle_reallocated = self.ensure_rectangle_capacity(rectangles);
        let glyph_reallocated = self.ensure_glyph_capacity(glyphs);
        let image_reallocated = self.ensure_image_capacity(images);
        let _composite_reallocated = self.ensure_composite_capacity(composites);
        if rounded > self.rounded_rect_instance_capacity {
            self.rounded_rect_instance_capacity = rounded.next_power_of_two();
            self.rounded_rect_instances =
                create_rrect_buffer(&self.device, self.rounded_rect_instance_capacity);
        }
        if paths > self.path_instance_capacity {
            self.path_instance_capacity = paths.next_power_of_two();
            self.path_instances =
                create_path_instance_buffer(&self.device, self.path_instance_capacity);
        }
        self.upload_instance_data(&batches, scale, self.target_width, self.target_height);
        self.prepare_image_bind_groups(&batches);
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => frame,
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                self.surface.configure(&self.device, &self.config);
                frame
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return Ok(RenderStats::default());
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                self.surface = self.shared.create_surface(self.handles)?;
                self.surface.configure(&self.device, &self.config);
                self.window_gpu.presentation.surface_lost();
                return Ok(RenderStats::default());
            }
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => return Ok(RenderStats::default()),
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let clear = wgpu::Color {
            r: 0.04,
            g: 0.04,
            b: 0.06,
            a: 1.0,
        };
        let has_destination_blend = batches.iter().any(|batch| {
            matches!(
                batch,
                DrawBatch::Blend {
                    mode,
                    ..
                } if mode.requires_destination_read()
            )
        });
        let (draw_calls, text_draw_calls) = if has_destination_blend {
            // A destination-read blend promotes only this composition scope;
            // ordinary SrcOver frames continue through the direct surface
            // path above.
            self.ensure_destination_targets(target_width, target_height);
            let mut targets = self
                .destination_targets
                .take()
                .expect("destination targets after ensure");
            let (current_first, draws, text, passes) = self.render_destination_batches(
                &batches,
                scale,
                target_width,
                target_height,
                &mut targets,
                clear,
                "incular destination composition segment",
            );
            self.counters.full_frame_intermediate_passes += 1;
            self.counters.offscreen_render_passes += u64::from(passes);
            let final_target = if current_first {
                targets.first.clone()
            } else {
                targets.second.clone()
            };
            let present_draw = self.present_composition_target(
                &final_target,
                &view,
                target_origin,
                target_width,
                target_height,
                self.config.width,
                self.config.height,
                scale,
            );
            self.destination_targets = Some(targets);
            (draws + present_draw, text)
        } else {
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("incular frame encoder"),
                });
            let root_stencil = self.stencil_view.clone();
            let result = self.encode_batches(
                &mut encoder,
                &view,
                &root_stencil,
                &batches,
                target_width,
                target_height,
                scale,
                clear,
                None,
                false,
            );
            self.queue.submit(Some(encoder.finish()));
            result
        };
        self.queue.present(frame);
        self.counters.frames += 1;
        self.evict_unused_images();
        self.evict_unused_path_meshes();
        self.evict_unused_gradients();
        self.evict_offscreen_cache();
        self.counters.draw_calls += u64::from(draw_calls);
        self.counters.text_draw_calls += u64::from(text_draw_calls);
        self.counters.rectangle_instances += rectangles as u64;
        self.counters.glyph_instances += glyphs as u64;
        self.counters.image_instances += images as u64;
        self.counters.rounded_rect_instances += rounded as u64;
        Ok(RenderStats {
            draw_calls,
            rectangle_instances: rectangles as u32,
            glyph_instances: glyphs as u32,
            image_instances: images as u32,
            buffer_reallocated: rectangle_reallocated,
            glyph_buffer_reallocated: glyph_reallocated,
            image_buffer_reallocated: image_reallocated,
            presented: true,
        })
    }
    fn ensure_rectangle_capacity(&mut self, required: usize) -> bool {
        if required <= self.instance_capacity {
            return false;
        }
        self.instance_capacity = required.next_power_of_two();
        self.instances = create_instance_buffer(&self.device, self.instance_capacity);
        self.counters.buffer_reallocations += 1;
        true
    }
    fn ensure_glyph_capacity(&mut self, required: usize) -> bool {
        if required <= self.glyph_instance_capacity {
            return false;
        }
        self.glyph_instance_capacity = required.next_power_of_two();
        self.glyph_instances = create_glyph_buffer(&self.device, self.glyph_instance_capacity);
        self.counters.glyph_instance_buffer_reallocations += 1;
        true
    }
    fn ensure_image_capacity(&mut self, required: usize) -> bool {
        if required <= self.image_instance_capacity {
            return false;
        }
        self.image_instance_capacity = required.next_power_of_two();
        self.image_instances = create_image_buffer(&self.device, self.image_instance_capacity);
        true
    }
    fn ensure_composite_capacity(&mut self, required: usize) -> bool {
        if required <= self.composite_instance_capacity {
            return false;
        }
        self.composite_instance_capacity = required.next_power_of_two();
        self.composite_instances =
            create_composite_buffer(&self.device, self.composite_instance_capacity);
        true
    }
    fn upload_instance_data(&self, batches: &[DrawBatch], scale: f32, width: u32, height: u32) {
        let mut rectangle_offset = 0_u64;
        let mut glyph_offset = 0_u64;
        let mut image_offset = 0_u64;
        let mut rounded_offset = 0_u64;
        let mut path_offset = 0_u64;
        let mut composite_offset = 0_u64;
        for batch in batches {
            match batch {
                DrawBatch::Rectangles { clip, instances }
                    if *clip != ClipState::Empty && !instances.is_empty() =>
                {
                    let gpu: Vec<_> = instances
                        .iter()
                        .map(|instance| {
                            logical_instance(*instance, width as f32, height as f32, scale)
                        })
                        .collect();
                    self.queue.write_buffer(
                        &self.instances,
                        rectangle_offset,
                        bytemuck::cast_slice(&gpu),
                    );
                    rectangle_offset += (gpu.len() * std::mem::size_of::<GpuInstance>()) as u64;
                }
                DrawBatch::Glyphs {
                    clip, instances, ..
                } if *clip != ClipState::Empty && !instances.is_empty() => {
                    self.queue.write_buffer(
                        &self.glyph_instances,
                        glyph_offset,
                        bytemuck::cast_slice(instances),
                    );
                    glyph_offset +=
                        (instances.len() * std::mem::size_of::<GpuGlyphInstance>()) as u64;
                }
                DrawBatch::Images {
                    clip, instances, ..
                } if *clip != ClipState::Empty && !instances.is_empty() => {
                    self.queue.write_buffer(
                        &self.image_instances,
                        image_offset,
                        bytemuck::cast_slice(instances),
                    );
                    image_offset +=
                        (instances.len() * std::mem::size_of::<GpuImageInstance>()) as u64;
                }
                DrawBatch::RoundedRects {
                    clip, instances, ..
                } if *clip != ClipState::Empty && !instances.is_empty() => {
                    self.queue.write_buffer(
                        &self.rounded_rect_instances,
                        rounded_offset,
                        bytemuck::cast_slice(instances),
                    );
                    rounded_offset +=
                        (instances.len() * std::mem::size_of::<GpuRRectInstance>()) as u64;
                }
                DrawBatch::Path { clip, instance, .. } if *clip != ClipState::Empty => {
                    self.queue.write_buffer(
                        &self.path_instances,
                        path_offset,
                        bytemuck::bytes_of(instance),
                    );
                    path_offset += std::mem::size_of::<GpuPathInstance>() as u64;
                }
                DrawBatch::StencilRRect { clip, instance, .. } if *clip != ClipState::Empty => {
                    self.queue.write_buffer(
                        &self.rounded_rect_instances,
                        rounded_offset,
                        bytemuck::bytes_of(instance),
                    );
                    rounded_offset += std::mem::size_of::<GpuRRectInstance>() as u64;
                }
                DrawBatch::StencilPath { clip, instance, .. } if *clip != ClipState::Empty => {
                    self.queue.write_buffer(
                        &self.path_instances,
                        path_offset,
                        bytemuck::bytes_of(instance),
                    );
                    path_offset += std::mem::size_of::<GpuPathInstance>() as u64;
                }
                DrawBatch::Offscreen { clip, instance, .. } if *clip != ClipState::Empty => {
                    self.queue.write_buffer(
                        &self.composite_instances,
                        composite_offset,
                        bytemuck::bytes_of(instance),
                    );
                    composite_offset += std::mem::size_of::<GpuCompositeInstance>() as u64;
                }
                DrawBatch::Filtered { clip, instance, .. }
                | DrawBatch::Shadow { clip, instance, .. }
                | DrawBatch::Blend { clip, instance, .. }
                    if *clip != ClipState::Empty =>
                {
                    self.queue.write_buffer(
                        &self.composite_instances,
                        composite_offset,
                        bytemuck::bytes_of(instance),
                    );
                    composite_offset += std::mem::size_of::<GpuCompositeInstance>() as u64;
                }
                _ => {}
            }
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn encode_batches(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        stencil_view: &wgpu::TextureView,
        batches: &[DrawBatch],
        width: u32,
        height: u32,
        scale: f32,
        clear: wgpu::Color,
        destination_view: Option<&wgpu::TextureView>,
        load_existing: bool,
    ) -> (u32, u32) {
        let mut draw_calls = 0_u32;
        let mut text_draw_calls = 0_u32;
        let mut rectangle_offset = 0_u64;
        let mut glyph_offset = 0_u64;
        let mut image_offset = 0_u64;
        let mut rounded_offset = 0_u64;
        let mut path_offset = 0_u64;
        let mut composite_offset = 0_u64;
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("incular retained compositor pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: if load_existing {
                        wgpu::LoadOp::Load
                    } else {
                        wgpu::LoadOp::Clear(clear)
                    },
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: stencil_view,
                depth_ops: None,
                stencil_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0),
                    store: wgpu::StoreOp::Store,
                }),
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        for batch in batches {
            let clip = match batch {
                DrawBatch::Rectangles { clip, .. }
                | DrawBatch::Glyphs { clip, .. }
                | DrawBatch::Images { clip, .. }
                | DrawBatch::RoundedRects { clip, .. }
                | DrawBatch::Path { clip, .. }
                | DrawBatch::StencilRRect { clip, .. }
                | DrawBatch::StencilPath { clip, .. }
                | DrawBatch::Offscreen { clip, .. }
                | DrawBatch::Filtered { clip, .. }
                | DrawBatch::Shadow { clip, .. }
                | DrawBatch::Blend { clip, .. } => *clip,
            };
            if clip == ClipState::Empty {
                continue;
            }
            if !set_scissor(&mut pass, clip, width, height, scale) {
                match batch {
                    DrawBatch::Rectangles { instances, .. } => {
                        rectangle_offset +=
                            (instances.len() * std::mem::size_of::<GpuInstance>()) as u64;
                    }
                    DrawBatch::Glyphs { instances, .. } => {
                        glyph_offset +=
                            (instances.len() * std::mem::size_of::<GpuGlyphInstance>()) as u64;
                    }
                    DrawBatch::Images { instances, .. } => {
                        image_offset +=
                            (instances.len() * std::mem::size_of::<GpuImageInstance>()) as u64;
                    }
                    DrawBatch::RoundedRects { instances, .. } => {
                        rounded_offset +=
                            (instances.len() * std::mem::size_of::<GpuRRectInstance>()) as u64;
                    }
                    DrawBatch::Path { .. } | DrawBatch::StencilPath { .. } => {
                        path_offset += std::mem::size_of::<GpuPathInstance>() as u64;
                    }
                    DrawBatch::StencilRRect { .. } => {
                        rounded_offset += std::mem::size_of::<GpuRRectInstance>() as u64;
                    }
                    DrawBatch::Offscreen { .. } => {
                        composite_offset += std::mem::size_of::<GpuCompositeInstance>() as u64;
                    }
                    DrawBatch::Filtered { .. } | DrawBatch::Shadow { .. } => {
                        composite_offset += std::mem::size_of::<GpuCompositeInstance>() as u64;
                    }
                    DrawBatch::Blend { .. } => {
                        composite_offset += std::mem::size_of::<GpuCompositeInstance>() as u64;
                    }
                }
                self.counters.clip_culled_draws += 1;
                continue;
            }
            pass.set_stencil_reference(u32::from(stencil_depth(clip)));
            match batch {
                DrawBatch::Rectangles { instances, .. } if !instances.is_empty() => {
                    let start = rectangle_offset;
                    rectangle_offset +=
                        (instances.len() * std::mem::size_of::<GpuInstance>()) as u64;
                    pass.set_pipeline(&self.rectangle_pipeline);
                    pass.set_vertex_buffer(0, self.mesh.slice(..));
                    pass.set_vertex_buffer(1, self.instances.slice(start..rectangle_offset));
                    pass.draw(0..6, 0..instances.len() as u32);
                    draw_calls += 1;
                }
                DrawBatch::Glyphs {
                    page, instances, ..
                } if !instances.is_empty() => {
                    let start = glyph_offset;
                    glyph_offset +=
                        (instances.len() * std::mem::size_of::<GpuGlyphInstance>()) as u64;
                    let Some(atlas_page) = self.atlas_pages.get(usize::from(*page)) else {
                        continue;
                    };
                    pass.set_pipeline(&self.text_pipeline);
                    pass.set_bind_group(0, &atlas_page.bind_group, &[]);
                    pass.set_vertex_buffer(0, self.mesh.slice(..));
                    pass.set_vertex_buffer(1, self.glyph_instances.slice(start..glyph_offset));
                    pass.draw(0..6, 0..instances.len() as u32);
                    draw_calls += 1;
                    text_draw_calls += 1;
                }
                DrawBatch::Images {
                    image,
                    sampling,
                    instances,
                    ..
                } if !instances.is_empty() => {
                    let start = image_offset;
                    image_offset +=
                        (instances.len() * std::mem::size_of::<GpuImageInstance>()) as u64;
                    let Some(bind_group) = self.image_bind_group(*image, *sampling) else {
                        continue;
                    };
                    pass.set_pipeline(&self.image_pipeline);
                    pass.set_bind_group(0, bind_group, &[]);
                    pass.set_vertex_buffer(0, self.mesh.slice(..));
                    pass.set_vertex_buffer(1, self.image_instances.slice(start..image_offset));
                    pass.draw(0..6, 0..instances.len() as u32);
                    draw_calls += 1;
                    self.counters.image_draw_calls += 1;
                }
                DrawBatch::RoundedRects {
                    gradient,
                    instances,
                    ..
                } if !instances.is_empty() => {
                    let start = rounded_offset;
                    rounded_offset +=
                        (instances.len() * std::mem::size_of::<GpuRRectInstance>()) as u64;
                    pass.set_pipeline(&self.rounded_rect_pipeline);
                    pass.set_bind_group(0, self.gradient_bind_group(*gradient), &[]);
                    pass.set_vertex_buffer(0, self.mesh.slice(..));
                    pass.set_vertex_buffer(
                        1,
                        self.rounded_rect_instances.slice(start..rounded_offset),
                    );
                    pass.draw(0..6, 0..instances.len() as u32);
                    draw_calls += 1;
                }
                DrawBatch::Path { key, gradient, .. } => {
                    let start = path_offset;
                    path_offset += std::mem::size_of::<GpuPathInstance>() as u64;
                    if let Some(mesh) = self.gpu_path_cache.get_mut(key) {
                        mesh.last_used_frame = self.counters.frames;
                    } else {
                        continue;
                    }
                    let gradient_bind_group = self.gradient_bind_group(*gradient);
                    let Some(mesh) = self.gpu_path_cache.get(key) else {
                        continue;
                    };
                    pass.set_pipeline(&self.path_pipeline);
                    pass.set_bind_group(0, gradient_bind_group, &[]);
                    pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                    pass.set_vertex_buffer(1, self.path_instances.slice(start..path_offset));
                    pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..mesh.index_count, 0, 0..1);
                    draw_calls += 1;
                    self.counters.path_draw_calls += 1;
                    self.counters.path_triangles += u64::from(mesh.index_count / 3);
                }
                DrawBatch::StencilRRect { increment, .. } => {
                    let start = rounded_offset;
                    rounded_offset += std::mem::size_of::<GpuRRectInstance>() as u64;
                    pass.set_pipeline(if *increment {
                        &self.stencil_rrect_increment_pipeline
                    } else {
                        &self.stencil_rrect_decrement_pipeline
                    });
                    pass.set_bind_group(0, self.gradient_bind_group(None), &[]);
                    pass.set_vertex_buffer(0, self.mesh.slice(..));
                    pass.set_vertex_buffer(
                        1,
                        self.rounded_rect_instances.slice(start..rounded_offset),
                    );
                    pass.draw(0..6, 0..1);
                    draw_calls += 1;
                    self.counters.stencil_mask_draws += 1;
                }
                DrawBatch::StencilPath { key, increment, .. } => {
                    let start = path_offset;
                    path_offset += std::mem::size_of::<GpuPathInstance>() as u64;
                    let Some(mesh) = self.gpu_path_cache.get(key) else {
                        continue;
                    };
                    pass.set_pipeline(if *increment {
                        &self.stencil_path_increment_pipeline
                    } else {
                        &self.stencil_path_decrement_pipeline
                    });
                    pass.set_bind_group(0, self.gradient_bind_group(None), &[]);
                    pass.set_vertex_buffer(0, self.mesh.slice(..));
                    pass.set_vertex_buffer(1, self.path_instances.slice(start..path_offset));
                    pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..mesh.index_count, 0, 0..1);
                    draw_calls += 1;
                    self.counters.stencil_mask_draws += 1;
                }
                DrawBatch::Offscreen {
                    layer, instance, ..
                } => {
                    let start = composite_offset;
                    composite_offset += std::mem::size_of::<GpuCompositeInstance>() as u64;
                    if let Some(entry) = self.offscreen_cache.get_mut(layer) {
                        entry.last_used_frame = self.counters.frames;
                    } else {
                        continue;
                    }
                    let Some(entry) = self.offscreen_cache.get(layer) else {
                        continue;
                    };
                    pass.set_pipeline(&self.composite_pipeline);
                    pass.set_bind_group(0, &entry.bind_group, &[]);
                    pass.set_vertex_buffer(0, self.mesh.slice(..));
                    pass.set_vertex_buffer(
                        1,
                        self.composite_instances.slice(
                            start..start + std::mem::size_of::<GpuCompositeInstance>() as u64,
                        ),
                    );
                    pass.draw(0..6, 0..1);
                    let _ = instance;
                    draw_calls += 1;
                    self.counters.offscreen_composite_draws += 1;
                }
                DrawBatch::Filtered {
                    layer, instance, ..
                } => {
                    let start = composite_offset;
                    composite_offset += std::mem::size_of::<GpuCompositeInstance>() as u64;
                    if let Some(entry) = self.effect_cache.get_mut(layer) {
                        entry.last_used_frame = self.counters.frames;
                    } else {
                        continue;
                    }
                    let Some(entry) = self.effect_cache.get(layer) else {
                        continue;
                    };
                    pass.set_pipeline(&self.composite_pipeline);
                    pass.set_bind_group(0, &entry.bind_group, &[]);
                    pass.set_vertex_buffer(0, self.mesh.slice(..));
                    pass.set_vertex_buffer(
                        1,
                        self.composite_instances.slice(
                            start..start + std::mem::size_of::<GpuCompositeInstance>() as u64,
                        ),
                    );
                    pass.draw(0..6, 0..1);
                    let _ = instance;
                    draw_calls += 1;
                    self.counters.offscreen_composite_draws += 1;
                }
                DrawBatch::Shadow {
                    layer, instance, ..
                } => {
                    let start = composite_offset;
                    composite_offset += std::mem::size_of::<GpuCompositeInstance>() as u64;
                    if let Some(entry) = self.effect_cache.get_mut(layer) {
                        entry.last_used_frame = self.counters.frames;
                    } else if let Some(entry) = self.offscreen_cache.get_mut(layer) {
                        entry.last_used_frame = self.counters.frames;
                    } else {
                        continue;
                    }
                    pass.set_pipeline(&self.composite_pipeline);
                    if let Some(entry) = self.effect_cache.get(layer) {
                        pass.set_bind_group(0, &entry.bind_group, &[]);
                    } else if let Some(entry) = self.offscreen_cache.get(layer) {
                        pass.set_bind_group(0, &entry.bind_group, &[]);
                    } else {
                        continue;
                    }
                    pass.set_vertex_buffer(0, self.mesh.slice(..));
                    pass.set_vertex_buffer(
                        1,
                        self.composite_instances.slice(
                            start..start + std::mem::size_of::<GpuCompositeInstance>() as u64,
                        ),
                    );
                    pass.draw(0..6, 0..1);
                    let _ = instance;
                    draw_calls += 1;
                    self.counters.drop_shadow_composites += 1;
                }
                DrawBatch::Blend {
                    layer,
                    instance,
                    mode,
                    ..
                } => {
                    let start = composite_offset;
                    composite_offset += std::mem::size_of::<GpuCompositeInstance>() as u64;
                    let Some(source) = self.offscreen_cache.get_mut(layer) else {
                        continue;
                    };
                    source.last_used_frame = self.counters.frames;
                    if !mode.requires_destination_read() {
                        let Some(pipeline) = self.fixed_blend_pipelines.get(mode.code() as usize)
                        else {
                            continue;
                        };
                        pass.set_pipeline(pipeline);
                        pass.set_bind_group(0, &source.bind_group, &[]);
                        pass.set_vertex_buffer(0, self.mesh.slice(..));
                        pass.set_vertex_buffer(
                            1,
                            self.composite_instances.slice(
                                start..start + std::mem::size_of::<GpuCompositeInstance>() as u64,
                            ),
                        );
                        pass.draw(0..6, 0..1);
                        self.counters.blend_fixed_function_draws += 1;
                        draw_calls += 1;
                        let _ = instance;
                        continue;
                    }
                    let Some(destination_view) = destination_view else {
                        continue;
                    };
                    let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("incular destination blend bind group"),
                        layout: &self.blend_bind_group_layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::TextureView(
                                    &source.target.color_view,
                                ),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::TextureView(destination_view),
                            },
                            wgpu::BindGroupEntry {
                                binding: 2,
                                resource: wgpu::BindingResource::Sampler(&self.blend_sampler),
                            },
                        ],
                    });
                    pass.set_pipeline(&self.blend_pipeline);
                    pass.set_bind_group(0, &bind_group, &[]);
                    pass.set_vertex_buffer(0, self.mesh.slice(..));
                    pass.set_vertex_buffer(
                        1,
                        self.composite_instances.slice(
                            start..start + std::mem::size_of::<GpuCompositeInstance>() as u64,
                        ),
                    );
                    pass.draw(0..6, 0..1);
                    self.counters.blend_destination_read_draws += 1;
                    if !mode.requires_destination_read() {
                        self.counters.blend_fixed_function_draws += 1;
                    }
                    draw_calls += 1;
                    let _ = instance;
                }
                _ => {}
            }
        }
        drop(pass);
        (draw_calls, text_draw_calls)
    }
    fn lower_draw_batches(
        &mut self,
        list: &DisplayList,
        scale: f32,
    ) -> Result<Vec<DrawBatch>, RendererError> {
        self.target_width = self.config.width;
        self.target_height = self.config.height;
        self.target_origin = Offset::ZERO;
        self.lower_commands(
            list.commands(),
            scale,
            Transform::IDENTITY,
            ClipState::Unbounded,
        )
    }
    fn lower_commands(
        &mut self,
        commands: &[PaintCommand],
        scale: f32,
        initial_transform: Transform,
        initial_clip: ClipState,
    ) -> Result<Vec<DrawBatch>, RendererError> {
        let mut batches = Vec::new();
        let mut transforms = vec![initial_transform];
        let mut clips = vec![initial_clip];
        let mut clip_masks: Vec<Option<ClipMask>> = vec![None];
        let mut command_index = 0_usize;
        while command_index < commands.len() {
            let command = &commands[command_index];
            let transform = *transforms.last().expect("transform stack");
            let translation = transform.translation_offset();
            match command {
                PaintCommand::Rect { rect, color } => {
                    let clip = *clips.last().expect("clip stack");
                    if transform.is_translation() {
                        append_rectangle(
                            &mut batches,
                            clip,
                            RectangleInstance {
                                rect: translated_rect(*rect, translation),
                                color: *color,
                            },
                        );
                    } else {
                        // The fast rectangle instance shader is axis-aligned.
                        // Preserve arbitrary retained affine geometry by
                        // routing it through the already-cached Lyon path
                        // pipeline instead of expanding to a bounding box.
                        self.append_path(
                            &mut batches,
                            clip,
                            &Arc::new(rect_path(*rect)),
                            PathMeshKind::Fill(FillRule::NonZero),
                            &Brush::Solid(*color),
                            PathPlacement { transform, scale },
                        );
                    }
                }
                // Common solid rounded primitives retain painter order even on
                // renderers that do not yet select the analytic pipeline.
                PaintCommand::RRect { rrect, brush } => append_rrect(
                    &mut batches,
                    *clips.last().expect("clip stack"),
                    self.ensure_gradient(brush),
                    rrect_instance(
                        *rrect,
                        brush,
                        transform,
                        scale,
                        self.target_width as f32,
                        self.target_height as f32,
                    ),
                ),
                PaintCommand::Border { rrect, border } => {
                    // Inside-aligned border: analytical shader discards the inset.
                    append_rrect(
                        &mut batches,
                        *clips.last().expect("clip stack"),
                        None,
                        border_instance(
                            *rrect,
                            *border,
                            transform,
                            scale,
                            self.target_width as f32,
                            self.target_height as f32,
                        ),
                    );
                }
                PaintCommand::FillPath {
                    path,
                    brush,
                    fill_rule,
                } => self.append_path(
                    &mut batches,
                    *clips.last().expect("clip stack"),
                    path,
                    PathMeshKind::Fill(*fill_rule),
                    brush,
                    PathPlacement { transform, scale },
                ),
                PaintCommand::StrokePath {
                    path,
                    brush,
                    stroke,
                } => self.append_path(
                    &mut batches,
                    *clips.last().expect("clip stack"),
                    path,
                    stroke_mesh_kind(*stroke),
                    brush,
                    PathPlacement { transform, scale },
                ),
                PaintCommand::GlyphRun { run, color } => {
                    if *clips.last().expect("clip stack") == ClipState::Empty {
                        command_index += 1;
                        continue;
                    }
                    for glyph in run.glyphs.iter() {
                        let Some(raster) =
                            self.shared.rasterize_glyph(run, glyph.id, f64::from(scale))
                        else {
                            self.counters.glyphs_skipped += 1;
                            continue;
                        };
                        if let Some(bitmap) = raster.bitmap.as_deref() {
                            self.upload_glyph(raster.entry, bitmap);
                        } else if raster.entry.width > 0 && raster.entry.height > 0 {
                            // The texture is device-shared, while this window
                            // still needs its own pipeline-compatible bind
                            // group for that atlas page.
                            self.ensure_atlas_page(raster.entry.page);
                        }
                        if raster.entry.width > 0 && raster.entry.height > 0 {
                            append_glyph(
                                &mut batches,
                                *clips.last().expect("clip stack"),
                                raster.entry.page,
                                glyph_instance(
                                    run,
                                    glyph.offset,
                                    raster.entry,
                                    *color,
                                    GlyphSurface {
                                        transform,
                                        width: self.target_width as f32,
                                        height: self.target_height as f32,
                                        scale,
                                    },
                                ),
                            );
                        }
                    }
                }
                PaintCommand::Image {
                    image,
                    source,
                    destination,
                    sampling,
                } => {
                    if *clips.last().expect("clip stack") == ClipState::Empty {
                        command_index += 1;
                        continue;
                    }
                    self.ensure_gpu_image(image)?;
                    append_image(
                        &mut batches,
                        *clips.last().expect("clip stack"),
                        image.id(),
                        *sampling,
                        image_instance(
                            *destination,
                            *source,
                            image,
                            self.target_width as f32,
                            self.target_height as f32,
                            scale,
                            transform,
                        ),
                    );
                }
                PaintCommand::PushOpacity {
                    layer,
                    alpha,
                    generation,
                    bounds,
                } => {
                    let end = find_opacity_end(commands, command_index)?;
                    let start = command_index + 1;
                    let parent_clip = *clips.last().expect("clip stack");
                    let normalized = normalize_opacity(*alpha);
                    command_index = end.saturating_add(1);
                    if normalized <= 0. {
                        self.counters.opacity_zero_fast_paths += 1;
                        continue;
                    }
                    if normalized >= 1. {
                        self.counters.opacity_one_fast_paths += 1;
                        let child = self.lower_commands(
                            &commands[start..end],
                            scale,
                            transform,
                            parent_clip,
                        )?;
                        batches.extend(child);
                        continue;
                    }
                    if let Some(batch) = self.lower_opacity_group(
                        &commands[start..end],
                        scale,
                        *layer,
                        normalized,
                        *generation,
                        *bounds,
                        parent_clip,
                        translation,
                    )? {
                        batches.push(batch);
                    }
                    continue;
                }
                PaintCommand::PopOpacity => {
                    return Err(RendererError::UnbalancedClipStack);
                }
                PaintCommand::PushBlur {
                    layer,
                    blur,
                    generation,
                    bounds,
                } => {
                    let end = find_effect_end(commands, command_index)?;
                    let start = command_index + 1;
                    let parent_clip = *clips.last().expect("clip stack");
                    command_index = end.saturating_add(1);
                    let blur = GaussianBlur::new(blur.sigma_x, blur.sigma_y);
                    if blur.sigma_x <= f32::EPSILON && blur.sigma_y <= f32::EPSILON {
                        let child = self.lower_commands(
                            &commands[start..end],
                            scale,
                            transform,
                            parent_clip,
                        )?;
                        batches.extend(child);
                    } else if let Some(batch) = self.lower_blur_group(
                        &commands[start..end],
                        scale,
                        *layer,
                        blur,
                        *generation,
                        *bounds,
                        parent_clip,
                        translation,
                    )? {
                        batches.push(batch);
                    }
                    continue;
                }
                PaintCommand::PushDropShadow {
                    layer,
                    shadow,
                    generation,
                    bounds,
                } => {
                    let end = find_effect_end(commands, command_index)?;
                    let start = command_index + 1;
                    let parent_clip = *clips.last().expect("clip stack");
                    command_index = end.saturating_add(1);
                    let shadow = DropShadowEffect::asymmetric(
                        shadow.offset,
                        shadow.sigma_x,
                        shadow.sigma_y,
                        shadow.color,
                    );
                    let child = self.lower_drop_shadow_group(
                        &commands[start..end],
                        scale,
                        *layer,
                        shadow,
                        *generation,
                        *bounds,
                        parent_clip,
                        translation,
                    )?;
                    batches.extend(child);
                    continue;
                }
                PaintCommand::PushColorFilter {
                    layer,
                    filter,
                    generation,
                    bounds,
                } => {
                    let end = find_effect_end(commands, command_index)?;
                    let start = command_index + 1;
                    let parent_clip = *clips.last().expect("clip stack");
                    command_index = end.saturating_add(1);
                    if filter.is_identity() {
                        let child = self.lower_commands(
                            &commands[start..end],
                            scale,
                            transform,
                            parent_clip,
                        )?;
                        batches.extend(child);
                    } else if let Some(inner_commands) =
                        commands.get(start..end).filter(|child| !child.is_empty())
                        && let Some(PaintCommand::PushColorFilter {
                            filter: inner_filter,
                            generation: inner_generation,
                            ..
                        }) = inner_commands.first()
                        && let Ok(inner_end) = find_effect_end(inner_commands, 0)
                        && inner_end + 1 == inner_commands.len()
                    {
                        // Nested color matrices are adjacent single-input
                        // stages.  Remove the inner boundary and compose
                        // `inner` followed by `outer`; no blur or shadow is
                        // crossed, so painter order remains exact.
                        let combined = inner_filter.then(*filter);
                        if let Some(batch) = self.lower_color_filter_group(
                            &inner_commands[1..inner_end],
                            scale,
                            *layer,
                            combined,
                            // The fused pass's source is the inner
                            // filter's input. Its generation deliberately
                            // excludes the inner matrix itself, so changing
                            // either adjacent matrix rerenders only this
                            // fused stage rather than its source texture.
                            *inner_generation,
                            *bounds,
                            parent_clip,
                            translation,
                        )? {
                            batches.push(batch);
                        }
                        self.counters.effect_stage_fusions += 1;
                    } else if let Some(batch) = self.lower_color_filter_group(
                        &commands[start..end],
                        scale,
                        *layer,
                        *filter,
                        *generation,
                        *bounds,
                        parent_clip,
                        translation,
                    )? {
                        batches.push(batch);
                    }
                    continue;
                }
                PaintCommand::PushBlend {
                    layer,
                    mode,
                    generation,
                    bounds,
                } => {
                    let end = find_effect_end(commands, command_index)?;
                    let start = command_index + 1;
                    let parent_clip = *clips.last().expect("clip stack");
                    command_index = end.saturating_add(1);
                    if *mode == BlendMode::SrcOver {
                        // SrcOver is the ordinary painter operation. Keep it
                        // on the direct path instead of isolating a source
                        // texture solely to apply the default blend mode.
                        let child = self.lower_commands(
                            &commands[start..end],
                            scale,
                            transform,
                            parent_clip,
                        )?;
                        batches.extend(child);
                    } else if let Some(batch) = self.lower_blend_group(
                        &commands[start..end],
                        scale,
                        *layer,
                        *mode,
                        *generation,
                        *bounds,
                        parent_clip,
                        translation,
                    )? {
                        batches.push(batch);
                    }
                    continue;
                }
                PaintCommand::PopEffect => {
                    return Err(RendererError::UnbalancedClipStack);
                }
                PaintCommand::PushTransform {
                    transform: local_transform,
                } => transforms.push(transform.then(*local_transform)),
                PaintCommand::PopTransform => {
                    if transforms.len() > 1 {
                        transforms.pop();
                    }
                }
                PaintCommand::PushClip { rect } => {
                    let next = ClipRect {
                        rect: transform.transform_rect_bbox(*rect),
                    };
                    let combined = match *clips.last().expect("clip stack") {
                        ClipState::Unbounded => ClipState::Rect(next),
                        ClipState::Rect(current) => intersect_rect(current.rect, next.rect)
                            .map(|rect| ClipState::Rect(ClipRect { rect }))
                            .unwrap_or(ClipState::Empty),
                        ClipState::Stencil { rect, depth } => match rect {
                            Some(current) => intersect_rect(current.rect, next.rect)
                                .map(|rect| ClipState::Stencil {
                                    rect: Some(ClipRect { rect }),
                                    depth,
                                })
                                .unwrap_or(ClipState::Empty),
                            None => ClipState::Stencil {
                                rect: Some(next),
                                depth,
                            },
                        },
                        ClipState::Empty => ClipState::Empty,
                    };
                    clips.push(combined);
                    clip_masks.push(None);
                    self.counters.clip_rect_pushes += 1;
                }
                PaintCommand::PushClipRRect { rrect } => {
                    let next = ClipRect {
                        rect: translated_rect(rrect.rect, translation),
                    };
                    let parent = *clips.last().expect("clip stack");
                    let scissor = intersect_clip_with_rect(parent, next);
                    let depth = stencil_depth(parent);
                    if depth == MAX_STENCIL_CLIP_DEPTH {
                        return Err(RendererError::StencilDepthOverflow);
                    }
                    let instance = rrect_instance(
                        *rrect,
                        &Brush::Solid(Color::TRANSPARENT),
                        transform,
                        scale,
                        self.target_width as f32,
                        self.target_height as f32,
                    );
                    if scissor == ClipState::Empty {
                        clips.push(ClipState::Empty);
                        clip_masks.push(None);
                    } else {
                        batches.push(DrawBatch::StencilRRect {
                            clip: parent,
                            instance,
                            increment: true,
                        });
                        clips.push(with_stencil_depth(scissor, depth + 1));
                        clip_masks.push(Some(ClipMask::RRect(instance)));
                        self.counters.stencil_depth_max =
                            self.counters.stencil_depth_max.max(u64::from(depth + 1));
                    }
                    self.counters.clip_rrect_pushes += 1;
                }
                PaintCommand::PushClipOval { rect } => {
                    let parent = *clips.last().expect("clip stack");
                    let depth = stencil_depth(parent);
                    let path = Arc::new(oval_path(*rect));
                    let key = PathMeshKey {
                        path: path.id(),
                        kind: PathMeshKind::Fill(FillRule::NonZero),
                    };
                    if self.ensure_path_mesh(key, &path) {
                        let instance = clip_path_instance(
                            transform,
                            scale,
                            self.target_width as f32,
                            self.target_height as f32,
                        );
                        batches.push(DrawBatch::StencilPath {
                            clip: parent,
                            key,
                            instance,
                            increment: true,
                        });
                        clips.push(with_stencil_depth(parent, depth + 1));
                        clip_masks.push(Some(ClipMask::Path { key, instance }));
                    } else {
                        clips.push(ClipState::Empty);
                        clip_masks.push(None);
                    }
                }
                PaintCommand::PushClipPath { path, fill_rule } => {
                    let parent = *clips.last().expect("clip stack");
                    let depth = stencil_depth(parent);
                    if depth == MAX_STENCIL_CLIP_DEPTH {
                        return Err(RendererError::StencilDepthOverflow);
                    }
                    let Some(bounds) = path.bounds() else {
                        clips.push(ClipState::Empty);
                        clip_masks.push(None);
                        self.counters.clip_path_pushes += 1;
                        command_index += 1;
                        continue;
                    };
                    let scissor = intersect_clip_with_rect(
                        parent,
                        ClipRect {
                            rect: translated_rect(bounds, translation),
                        },
                    );
                    let key = PathMeshKey {
                        path: path.id(),
                        kind: PathMeshKind::Fill(*fill_rule),
                    };
                    if scissor == ClipState::Empty || !self.ensure_path_mesh(key, path) {
                        clips.push(ClipState::Empty);
                        clip_masks.push(None);
                    } else {
                        let instance = clip_path_instance(
                            transform,
                            scale,
                            self.target_width as f32,
                            self.target_height as f32,
                        );
                        batches.push(DrawBatch::StencilPath {
                            clip: parent,
                            key,
                            instance,
                            increment: true,
                        });
                        clips.push(with_stencil_depth(scissor, depth + 1));
                        clip_masks.push(Some(ClipMask::Path { key, instance }));
                        self.counters.stencil_depth_max =
                            self.counters.stencil_depth_max.max(u64::from(depth + 1));
                    }
                    self.counters.clip_path_pushes += 1;
                }
                PaintCommand::PopClip => {
                    if clips.len() > 1 {
                        let child = clips.pop().expect("checked clip stack");
                        let parent = *clips.last().expect("parent clip stack");
                        if let Some(mask) = clip_masks.pop().expect("matching mask stack") {
                            debug_assert_eq!(stencil_depth(child), stencil_depth(parent) + 1);
                            match mask {
                                ClipMask::RRect(instance) => {
                                    batches.push(DrawBatch::StencilRRect {
                                        clip: child,
                                        instance,
                                        increment: false,
                                    })
                                }
                                ClipMask::Path { key, instance } => {
                                    batches.push(DrawBatch::StencilPath {
                                        clip: child,
                                        key,
                                        instance,
                                        increment: false,
                                    })
                                }
                            }
                        }
                    }
                    self.counters.clip_pops += 1;
                }
            }
            command_index += 1;
        }
        if clips.len() != 1 || clip_masks.len() != 1 {
            return Err(RendererError::UnbalancedClipStack);
        }
        Ok(batches)
    }
    #[allow(clippy::too_many_arguments)]
    fn lower_opacity_group(
        &mut self,
        commands: &[PaintCommand],
        scale: f32,
        layer: incular_painting::LayerId,
        alpha: f32,
        generation: u64,
        bounds: Rect,
        parent_clip: ClipState,
        translation: Offset,
    ) -> Result<Option<DrawBatch>, RendererError> {
        let active_width = self.target_width;
        let active_height = self.target_height;
        let active_origin = self.target_origin;
        if parent_clip == ClipState::Empty {
            return Ok(None);
        }
        if !bounds.origin.x.is_finite()
            || !bounds.origin.y.is_finite()
            || !bounds.size.width.is_finite()
            || !bounds.size.height.is_finite()
            || bounds.size.width <= 0.
            || bounds.size.height <= 0.
        {
            return Ok(None);
        }
        let left = ((bounds.origin.x - active_origin.x) * scale).floor();
        let top = ((bounds.origin.y - active_origin.y) * scale).floor();
        let right = ((bounds.origin.x + bounds.size.width - active_origin.x) * scale).ceil();
        let bottom = ((bounds.origin.y + bounds.size.height - active_origin.y) * scale).ceil();
        let width_f = right - left;
        let height_f = bottom - top;
        let limit = self.device.limits().max_texture_dimension_2d;
        if width_f <= 0. || height_f <= 0. {
            return Ok(None);
        }
        if width_f > limit as f32 || height_f > limit as f32 {
            return Err(RendererError::OffscreenTargetTooLarge {
                width: width_f.min(u32::MAX as f32) as u32,
                height: height_f.min(u32::MAX as f32) as u32,
                limit,
            });
        }
        let width = width_f as u32;
        let height = height_f as u32;
        if width == 0 || height == 0 {
            return Ok(None);
        }
        let target_origin = Offset::new(
            active_origin.x + left / scale,
            active_origin.y + top / scale,
        );
        let frame = self.counters.frames;
        if let Some(entry) = self.offscreen_cache.get_mut(&layer)
            && entry.generation == generation
            && entry.width == width
            && entry.height == height
            && entry.scale_factor_bits == scale.to_bits()
            && entry.target.format == self.window_gpu.config.format
            && entry.device_generation == self.device_generation
        {
            entry.last_used_frame = frame;
            self.counters.offscreen_group_cache_hits += 1;
            self.counters.offscreen_texture_reuses += 1;
            return Ok(Some(DrawBatch::Offscreen {
                clip: parent_clip,
                layer,
                instance: composite_instance(
                    target_origin,
                    active_origin,
                    active_width,
                    active_height,
                    width,
                    height,
                    scale,
                    alpha,
                ),
            }));
        }
        if let Some(previous) = self.offscreen_cache.remove(&layer) {
            self.counters.offscreen_cached_bytes = self
                .counters
                .offscreen_cached_bytes
                .saturating_sub(previous.bytes);
            self.offscreen_target_pool.recycle(previous.target);
        }
        let saved_target = (self.target_width, self.target_height, self.target_origin);
        self.target_width = width;
        self.target_height = height;
        self.target_origin = target_origin;
        self.offscreen_nesting_depth += 1;
        self.counters.max_offscreen_nesting_depth = self
            .counters
            .max_offscreen_nesting_depth
            .max(self.offscreen_nesting_depth);
        let child_translation = translation - (target_origin - active_origin);
        let child_result = self.lower_commands(
            commands,
            scale,
            Transform::translation(child_translation),
            ClipState::Unbounded,
        );
        let child_batches = match child_result {
            Ok(batches) => batches,
            Err(error) => {
                self.target_width = saved_target.0;
                self.target_height = saved_target.1;
                self.target_origin = saved_target.2;
                self.offscreen_nesting_depth = self.offscreen_nesting_depth.saturating_sub(1);
                if let Some(previous) = self.offscreen_cache.remove(&layer) {
                    self.counters.offscreen_cached_bytes = self
                        .counters
                        .offscreen_cached_bytes
                        .saturating_sub(previous.bytes);
                    self.offscreen_target_pool.recycle(previous.target);
                }
                return Err(error);
            }
        };
        // Clip-only command streams (or an opacity wrapper around no child)
        // have no pixels to isolate.  Avoid allocating a color/stencil target,
        // submitting an empty pass, and emitting a composite draw for them.
        let child_has_content = child_batches.iter().any(|batch| match batch {
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
            // Stencil increment/decrement batches are only clip setup; they
            // become useful when paired with actual content above.
            DrawBatch::StencilRRect { .. } | DrawBatch::StencilPath { .. } => false,
            _ => false,
        });
        if !child_has_content {
            self.target_width = saved_target.0;
            self.target_height = saved_target.1;
            self.target_origin = saved_target.2;
            self.offscreen_nesting_depth = self.offscreen_nesting_depth.saturating_sub(1);
            return Ok(None);
        }
        let target = if let Some(target) =
            self.offscreen_target_pool
                .take(width, height, self.config.format, true)
        {
            self.counters.offscreen_texture_reuses += 1;
            target
        } else {
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("incular retained opacity color target"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.config.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let color_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let (stencil, stencil_view) = create_stencil_attachment(&self.device, width, height);
            self.counters.offscreen_color_texture_creations += 1;
            self.counters.offscreen_stencil_texture_creations += 1;
            OffscreenTarget {
                _color: texture,
                color_view,
                _stencil: stencil,
                stencil_view,
                width,
                height,
                format: self.config.format,
                has_stencil: true,
            }
        };
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("incular retained opacity target bind group"),
            layout: &self.composite_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&target.color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.composite_sampler),
                },
            ],
        });
        let bytes = target.bytes();
        self.offscreen_cache.insert(
            layer,
            OffscreenCacheEntry {
                target,
                bind_group,
                width,
                height,
                scale_factor_bits: scale.to_bits(),
                generation,
                device_generation: self.device_generation,
                last_used_frame: frame,
                bytes,
            },
        );
        self.counters.offscreen_cached_bytes =
            self.counters.offscreen_cached_bytes.saturating_add(bytes);
        self.counters.offscreen_peak_cached_bytes = self
            .counters
            .offscreen_peak_cached_bytes
            .max(self.counters.offscreen_cached_bytes);
        self.counters.offscreen_group_cache_misses += 1;
        self.counters.offscreen_group_rerenders += 1;
        let child_rectangles = child_batches
            .iter()
            .map(|batch| match batch {
                DrawBatch::Rectangles { clip, instances } if *clip != ClipState::Empty => {
                    instances.len()
                }
                _ => 0,
            })
            .sum::<usize>();
        let child_glyphs = child_batches
            .iter()
            .map(|batch| match batch {
                DrawBatch::Glyphs {
                    clip, instances, ..
                } if *clip != ClipState::Empty => instances.len(),
                _ => 0,
            })
            .sum::<usize>();
        let child_images = child_batches
            .iter()
            .map(|batch| match batch {
                DrawBatch::Images {
                    clip, instances, ..
                } if *clip != ClipState::Empty => instances.len(),
                _ => 0,
            })
            .sum::<usize>();
        let child_rounded = child_batches
            .iter()
            .map(|batch| match batch {
                DrawBatch::RoundedRects {
                    clip, instances, ..
                } if *clip != ClipState::Empty => instances.len(),
                DrawBatch::StencilRRect { clip, .. } if *clip != ClipState::Empty => 1,
                _ => 0,
            })
            .sum::<usize>();
        let child_paths = child_batches
            .iter()
            .map(|batch| match batch {
                DrawBatch::Path { clip, .. } | DrawBatch::StencilPath { clip, .. }
                    if *clip != ClipState::Empty =>
                {
                    1
                }
                _ => 0,
            })
            .sum::<usize>();
        let child_composites = child_batches
            .iter()
            .filter(|batch| {
                matches!(
                    batch,
                    DrawBatch::Offscreen { clip, .. }
                        | DrawBatch::Filtered { clip, .. }
                        | DrawBatch::Shadow { clip, .. }
                        if *clip != ClipState::Empty
                )
            })
            .count();
        self.ensure_rectangle_capacity(child_rectangles);
        self.ensure_glyph_capacity(child_glyphs);
        self.ensure_image_capacity(child_images);
        self.ensure_composite_capacity(child_composites);
        if child_rounded > self.rounded_rect_instance_capacity {
            self.rounded_rect_instance_capacity = child_rounded.next_power_of_two();
            self.rounded_rect_instances =
                create_rrect_buffer(&self.device, self.rounded_rect_instance_capacity);
        }
        if child_paths > self.path_instance_capacity {
            self.path_instance_capacity = child_paths.next_power_of_two();
            self.path_instances =
                create_path_instance_buffer(&self.device, self.path_instance_capacity);
        }
        self.upload_instance_data(&child_batches, scale, width, height);
        self.prepare_image_bind_groups(&child_batches);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("incular opacity group encoder"),
            });
        let (target_view, target_stencil) = {
            let entry = self
                .offscreen_cache
                .get(&layer)
                .expect("new opacity target in cache");
            (
                entry.target.color_view.clone(),
                entry.target.stencil_view.clone(),
            )
        };
        self.encode_batches(
            &mut encoder,
            &target_view,
            &target_stencil,
            &child_batches,
            width,
            height,
            scale,
            wgpu::Color::TRANSPARENT,
            None,
            false,
        );
        self.queue.submit(Some(encoder.finish()));
        self.counters.offscreen_render_passes += 1;
        self.target_width = saved_target.0;
        self.target_height = saved_target.1;
        self.target_origin = saved_target.2;
        self.offscreen_nesting_depth = self.offscreen_nesting_depth.saturating_sub(1);
        Ok(Some(DrawBatch::Offscreen {
            clip: parent_clip,
            layer,
            instance: composite_instance(
                target_origin,
                active_origin,
                active_width,
                active_height,
                width,
                height,
                scale,
                alpha,
            ),
        }))
    }
    #[allow(clippy::too_many_arguments)]
    fn lower_source_group(
        &mut self,
        commands: &[PaintCommand],
        scale: f32,
        layer: incular_painting::LayerId,
        generation: u64,
        bounds: Rect,
        parent_clip: ClipState,
        translation: Offset,
    ) -> Result<Option<CachedSource>, RendererError> {
        let active_origin = self.target_origin;
        let base_source = !commands_have_effects(commands);
        if parent_clip == ClipState::Empty
            || !bounds.origin.x.is_finite()
            || !bounds.origin.y.is_finite()
            || !bounds.size.width.is_finite()
            || !bounds.size.height.is_finite()
            || bounds.size.width <= 0.
            || bounds.size.height <= 0.
        {
            return Ok(None);
        }
        let left = ((bounds.origin.x - active_origin.x) * scale).floor();
        let top = ((bounds.origin.y - active_origin.y) * scale).floor();
        let right = ((bounds.origin.x + bounds.size.width - active_origin.x) * scale).ceil();
        let bottom = ((bounds.origin.y + bounds.size.height - active_origin.y) * scale).ceil();
        let width_f = right - left;
        let height_f = bottom - top;
        let limit = self.device.limits().max_texture_dimension_2d;
        if width_f <= 0. || height_f <= 0. {
            return Ok(None);
        }
        if width_f > limit as f32 || height_f > limit as f32 {
            return Err(RendererError::OffscreenTargetTooLarge {
                width: width_f.min(u32::MAX as f32) as u32,
                height: height_f.min(u32::MAX as f32) as u32,
                limit,
            });
        }
        let width = width_f as u32;
        let height = height_f as u32;
        if width == 0 || height == 0 {
            return Ok(None);
        }
        let target_origin = Offset::new(
            active_origin.x + left / scale,
            active_origin.y + top / scale,
        );
        let frame = self.counters.frames;
        if let Some(entry) = self.offscreen_cache.get_mut(&layer)
            && entry.generation == generation
            && entry.width == width
            && entry.height == height
            && entry.scale_factor_bits == scale.to_bits()
            && entry.target.format == self.window_gpu.config.format
            && entry.device_generation == self.device_generation
        {
            entry.last_used_frame = frame;
            if base_source {
                self.counters.effect_source_cache_hits += 1;
            } else {
                self.counters.effect_stage_cache_hits += 1;
            }
            self.counters.offscreen_texture_reuses += 1;
            return Ok(Some(CachedSource {
                origin: target_origin,
                width,
                height,
            }));
        }
        if let Some(previous) = self.offscreen_cache.remove(&layer) {
            self.counters.offscreen_cached_bytes = self
                .counters
                .offscreen_cached_bytes
                .saturating_sub(previous.bytes);
            self.offscreen_target_pool.recycle(previous.target);
        }
        let saved_target = (self.target_width, self.target_height, self.target_origin);
        self.target_width = width;
        self.target_height = height;
        self.target_origin = target_origin;
        self.offscreen_nesting_depth += 1;
        self.counters.max_offscreen_nesting_depth = self
            .counters
            .max_offscreen_nesting_depth
            .max(self.offscreen_nesting_depth);
        let child_translation = translation - (target_origin - active_origin);
        let child_batches = match self.lower_commands(
            commands,
            scale,
            Transform::translation(child_translation),
            ClipState::Unbounded,
        ) {
            Ok(batches) => batches,
            Err(error) => {
                self.target_width = saved_target.0;
                self.target_height = saved_target.1;
                self.target_origin = saved_target.2;
                self.offscreen_nesting_depth = self.offscreen_nesting_depth.saturating_sub(1);
                return Err(error);
            }
        };
        if !batches_have_content(&child_batches) {
            self.target_width = saved_target.0;
            self.target_height = saved_target.1;
            self.target_origin = saved_target.2;
            self.offscreen_nesting_depth = self.offscreen_nesting_depth.saturating_sub(1);
            return Ok(None);
        }
        let target = if let Some(target) =
            self.offscreen_target_pool
                .take(width, height, self.config.format, true)
        {
            self.counters.offscreen_texture_reuses += 1;
            target
        } else {
            self.create_offscreen_target(width, height, "incular retained effect source")
        };
        let bind_group =
            self.create_composite_bind_group(&target, "incular retained effect source bind group");
        let bytes = target.bytes();
        self.offscreen_cache.insert(
            layer,
            OffscreenCacheEntry {
                target,
                bind_group,
                width,
                height,
                scale_factor_bits: scale.to_bits(),
                generation,
                device_generation: self.device_generation,
                last_used_frame: frame,
                bytes,
            },
        );
        self.counters.offscreen_cached_bytes =
            self.counters.offscreen_cached_bytes.saturating_add(bytes);
        self.counters.offscreen_peak_cached_bytes = self
            .counters
            .offscreen_peak_cached_bytes
            .max(self.counters.offscreen_cached_bytes);
        if base_source {
            self.counters.effect_source_cache_misses += 1;
            self.counters.effect_source_rerenders += 1;
        } else {
            self.counters.effect_stage_cache_misses += 1;
            self.counters.effect_stage_rerenders += 1;
        }
        self.render_cached_batches(
            &child_batches,
            scale,
            width,
            height,
            layer,
            "incular effect source encoder",
        );
        self.target_width = saved_target.0;
        self.target_height = saved_target.1;
        self.target_origin = saved_target.2;
        self.offscreen_nesting_depth = self.offscreen_nesting_depth.saturating_sub(1);
        Ok(Some(CachedSource {
            origin: target_origin,
            width,
            height,
        }))
    }

    fn create_offscreen_target(
        &mut self,
        width: u32,
        height: u32,
        label: &'static str,
    ) -> OffscreenTarget {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let color_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let (stencil, stencil_view) = create_stencil_attachment(&self.device, width, height);
        self.counters.offscreen_color_texture_creations += 1;
        self.counters.offscreen_stencil_texture_creations += 1;
        OffscreenTarget {
            _color: texture,
            color_view,
            _stencil: stencil,
            stencil_view,
            width,
            height,
            format: self.config.format,
            has_stencil: true,
        }
    }

    fn create_composite_bind_group(
        &self,
        target: &OffscreenTarget,
        label: &'static str,
    ) -> wgpu::BindGroup {
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout: &self.composite_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&target.color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.composite_sampler),
                },
            ],
        })
    }

    fn render_cached_batches(
        &mut self,
        batches: &[DrawBatch],
        scale: f32,
        width: u32,
        height: u32,
        layer: incular_painting::LayerId,
        label: &'static str,
    ) {
        if batches.iter().any(|batch| {
            matches!(
                batch,
                DrawBatch::Blend {
                    mode,
                    ..
                } if mode.requires_destination_read()
            )
        }) {
            // Nested destination reads use the same ping-pong graph as the
            // presentation path, then copy the completed result into the
            // retained source target.  The copy is outside the blend pass,
            // so no texture is sampled while it is being rendered.
            let Some(output) = self
                .offscreen_cache
                .get(&layer)
                .map(|entry| entry.target.clone())
            else {
                return;
            };
            self.ensure_destination_targets(width, height);
            let mut targets = self
                .destination_targets
                .take()
                .expect("destination targets after ensure");
            let (current_first, _, _, passes) = self.render_destination_batches(
                batches,
                scale,
                width,
                height,
                &mut targets,
                wgpu::Color::TRANSPARENT,
                label,
            );
            let final_target = if current_first {
                &targets.first
            } else {
                &targets.second
            };
            self.copy_color_texture(
                final_target,
                &output,
                width,
                height,
                "incular nested destination composition copy",
            );
            self.destination_targets = Some(targets);
            self.counters.offscreen_render_passes += u64::from(passes);
            return;
        }
        let rectangles = count_rectangles(batches);
        let glyphs = count_glyphs(batches);
        let images = count_images(batches);
        let rounded = count_rounded(batches);
        let paths = count_paths(batches);
        let composites = count_composites(batches);
        self.ensure_rectangle_capacity(rectangles);
        self.ensure_glyph_capacity(glyphs);
        self.ensure_image_capacity(images);
        self.ensure_composite_capacity(composites);
        if rounded > self.rounded_rect_instance_capacity {
            self.rounded_rect_instance_capacity = rounded.next_power_of_two();
            self.rounded_rect_instances =
                create_rrect_buffer(&self.device, self.rounded_rect_instance_capacity);
        }
        if paths > self.path_instance_capacity {
            self.path_instance_capacity = paths.next_power_of_two();
            self.path_instances =
                create_path_instance_buffer(&self.device, self.path_instance_capacity);
        }
        self.upload_instance_data(batches, scale, width, height);
        self.prepare_image_bind_groups(batches);
        let Some(entry) = self.offscreen_cache.get(&layer) else {
            return;
        };
        let color_view = entry.target.color_view.clone();
        let stencil_view = entry.target.stencil_view.clone();
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) });
        self.encode_batches(
            &mut encoder,
            &color_view,
            &stencil_view,
            batches,
            width,
            height,
            scale,
            wgpu::Color::TRANSPARENT,
            None,
            false,
        );
        self.queue.submit(Some(encoder.finish()));
        self.counters.offscreen_render_passes += 1;
    }

    #[allow(clippy::too_many_arguments)]
    fn lower_blur_group(
        &mut self,
        commands: &[PaintCommand],
        scale: f32,
        layer: incular_painting::LayerId,
        blur: GaussianBlur,
        generation: u64,
        bounds: Rect,
        parent_clip: ClipState,
        translation: Offset,
    ) -> Result<Option<DrawBatch>, RendererError> {
        let Some(source) = self.lower_source_group(
            commands,
            scale,
            layer,
            generation,
            bounds,
            parent_clip,
            translation,
        )?
        else {
            return Ok(None);
        };
        let Some(effect) = self.render_blur_from_source(
            scale,
            layer,
            generation,
            bounds,
            GaussianBlur::new(blur.sigma_x, blur.sigma_y),
            source,
            false,
        )?
        else {
            return Ok(None);
        };
        self.counters.effect_chain_compilations += 1;
        let active_origin = self.target_origin;
        Ok(Some(DrawBatch::Filtered {
            clip: parent_clip,
            layer,
            instance: composite_instance(
                effect.origin,
                active_origin,
                self.target_width,
                self.target_height,
                effect.width,
                effect.height,
                scale,
                1.,
            ),
        }))
    }

    #[allow(clippy::too_many_arguments)]
    fn lower_color_filter_group(
        &mut self,
        commands: &[PaintCommand],
        scale: f32,
        layer: incular_painting::LayerId,
        filter: ColorFilter,
        generation: u64,
        bounds: Rect,
        parent_clip: ClipState,
        translation: Offset,
    ) -> Result<Option<DrawBatch>, RendererError> {
        let Some(source) = self.lower_source_group(
            commands,
            scale,
            layer,
            generation,
            bounds,
            parent_clip,
            translation,
        )?
        else {
            return Ok(None);
        };
        let Some(effect) =
            self.render_color_filter_from_source(scale, layer, generation, filter, source)?
        else {
            return Ok(None);
        };
        self.counters.effect_chain_compilations += 1;
        Ok(Some(DrawBatch::Filtered {
            clip: parent_clip,
            layer,
            instance: composite_instance(
                effect.origin,
                self.target_origin,
                self.target_width,
                self.target_height,
                effect.width,
                effect.height,
                scale,
                1.,
            ),
        }))
    }

    #[allow(clippy::too_many_arguments)]
    fn lower_blend_group(
        &mut self,
        commands: &[PaintCommand],
        scale: f32,
        layer: incular_painting::LayerId,
        mode: BlendMode,
        generation: u64,
        bounds: Rect,
        parent_clip: ClipState,
        translation: Offset,
    ) -> Result<Option<DrawBatch>, RendererError> {
        if parent_clip == ClipState::Empty {
            return Ok(None);
        }
        let Some(source) = self.lower_source_group(
            commands,
            scale,
            layer,
            generation,
            bounds,
            parent_clip,
            translation,
        )?
        else {
            return Ok(None);
        };
        self.counters.effect_chain_compilations += 1;
        Ok(Some(DrawBatch::Blend {
            clip: parent_clip,
            layer,
            mode,
            instance: blend_instance(
                source.origin,
                self.target_origin,
                self.target_width,
                self.target_height,
                source.width,
                source.height,
                scale,
                mode,
            ),
        }))
    }

    fn render_color_filter_from_source(
        &mut self,
        scale: f32,
        layer: incular_painting::LayerId,
        generation: u64,
        filter: ColorFilter,
        source: CachedSource,
    ) -> Result<Option<CachedEffect>, RendererError> {
        let frame = self.counters.frames;
        let matrix_bits = filter.to_matrix().map(f32::to_bits);
        if let Some(entry) = self.effect_cache.get_mut(&layer)
            && entry.source_generation == generation
            && entry.width == source.width
            && entry.height == source.height
            && entry.scale_factor_bits == scale.to_bits()
            && entry.matrix_bits == matrix_bits
            && entry.sigma_x_bits == 0
            && entry.sigma_y_bits == 0
            && entry.target.format == self.window_gpu.config.format
            && entry.device_generation == self.device_generation
        {
            entry.last_used_frame = frame;
            self.counters.effect_stage_cache_hits += 1;
            return Ok(Some(CachedEffect {
                origin: source.origin,
                width: source.width,
                height: source.height,
            }));
        }
        if let Some(previous) = self.effect_cache.remove(&layer) {
            self.remove_effect_cache_bytes(previous.bytes);
            self.offscreen_target_pool.recycle(previous.target);
        }
        let target = if let Some(target) =
            self.offscreen_target_pool
                .take(source.width, source.height, self.config.format, true)
        {
            self.counters.effect_texture_pool_hits += 1;
            target
        } else {
            self.counters.effect_texture_pool_misses += 1;
            self.counters.effect_texture_creations += 1;
            self.create_offscreen_target(source.width, source.height, "incular color matrix result")
        };
        let source_view = self
            .offscreen_cache
            .get(&layer)
            .expect("source cache for color filter")
            .target
            .color_view
            .clone();
        self.run_color_matrix_pass(&source_view, &target.color_view, filter);
        let bind_group =
            self.create_composite_bind_group(&target, "incular color matrix bind group");
        let bytes = target.bytes();
        self.effect_cache.insert(
            layer,
            EffectCacheEntry {
                target,
                bind_group,
                width: source.width,
                height: source.height,
                scale_factor_bits: scale.to_bits(),
                source_generation: generation,
                sigma_x_bits: 0,
                sigma_y_bits: 0,
                matrix_bits,
                device_generation: self.device_generation,
                last_used_frame: frame,
                bytes,
                downsample_factor: 1,
            },
        );
        self.add_effect_cache_bytes(bytes);
        self.counters.effect_stage_cache_misses += 1;
        self.counters.effect_stage_rerenders += 1;
        self.counters.color_matrix_passes += 1;
        Ok(Some(CachedEffect {
            origin: source.origin,
            width: source.width,
            height: source.height,
        }))
    }

    #[allow(clippy::too_many_arguments)]
    fn lower_drop_shadow_group(
        &mut self,
        commands: &[PaintCommand],
        scale: f32,
        layer: incular_painting::LayerId,
        shadow: DropShadowEffect,
        generation: u64,
        bounds: Rect,
        parent_clip: ClipState,
        translation: Offset,
    ) -> Result<Vec<DrawBatch>, RendererError> {
        let Some(source) = self.lower_source_group(
            commands,
            scale,
            layer,
            generation,
            bounds,
            parent_clip,
            translation,
        )?
        else {
            return Ok(Vec::new());
        };
        self.counters.effect_chain_compilations += 1;
        let active_origin = self.target_origin;
        let blur = GaussianBlur::new(shadow.sigma_x, shadow.sigma_y);
        let shadow_batch = if blur.sigma_x <= f32::EPSILON && blur.sigma_y <= f32::EPSILON {
            DrawBatch::Shadow {
                clip: parent_clip,
                layer,
                instance: composite_instance_with_color(
                    source.origin + shadow.offset,
                    active_origin,
                    self.target_width,
                    self.target_height,
                    source.width,
                    source.height,
                    scale,
                    1.,
                    shadow.color,
                    true,
                ),
            }
        } else {
            let Some(effect) =
                self.render_blur_from_source(scale, layer, generation, bounds, blur, source, true)?
            else {
                return Ok(Vec::new());
            };
            DrawBatch::Shadow {
                clip: parent_clip,
                layer,
                instance: composite_instance_with_color(
                    effect.origin + shadow.offset,
                    active_origin,
                    self.target_width,
                    self.target_height,
                    effect.width,
                    effect.height,
                    scale,
                    1.,
                    shadow.color,
                    true,
                ),
            }
        };
        Ok(vec![
            shadow_batch,
            DrawBatch::Offscreen {
                clip: parent_clip,
                layer,
                instance: composite_instance(
                    source.origin,
                    active_origin,
                    self.target_width,
                    self.target_height,
                    source.width,
                    source.height,
                    scale,
                    1.,
                ),
            },
        ])
    }

    #[allow(clippy::too_many_arguments)]
    fn render_blur_from_source(
        &mut self,
        scale: f32,
        layer: incular_painting::LayerId,
        generation: u64,
        source_bounds: Rect,
        blur: GaussianBlur,
        source: CachedSource,
        for_shadow: bool,
    ) -> Result<Option<CachedEffect>, RendererError> {
        let active_origin = self.target_origin;
        let effect_bounds = blur_bounds(source_bounds, blur.sigma_x, blur.sigma_y);
        let left = ((effect_bounds.origin.x - active_origin.x) * scale).floor();
        let top = ((effect_bounds.origin.y - active_origin.y) * scale).floor();
        let right =
            ((effect_bounds.origin.x + effect_bounds.size.width - active_origin.x) * scale).ceil();
        let bottom =
            ((effect_bounds.origin.y + effect_bounds.size.height - active_origin.y) * scale).ceil();
        let width_f = right - left;
        let height_f = bottom - top;
        let limit = self.device.limits().max_texture_dimension_2d;
        if width_f <= 0. || height_f <= 0. {
            return Ok(None);
        }
        if width_f > limit as f32 || height_f > limit as f32 {
            return Err(RendererError::OffscreenTargetTooLarge {
                width: width_f.min(u32::MAX as f32) as u32,
                height: height_f.min(u32::MAX as f32) as u32,
                limit,
            });
        }
        let width = width_f as u32;
        let height = height_f as u32;
        let effect_origin = Offset::new(
            active_origin.x + left / scale,
            active_origin.y + top / scale,
        );
        let sigma_x_physical = normalize_sigma(blur.sigma_x * scale);
        let sigma_y_physical = normalize_sigma(blur.sigma_y * scale);
        let downsample_factor =
            choose_blur_downsample_factor(sigma_x_physical.max(sigma_y_physical));
        let frame = self.counters.frames;
        if let Some(entry) = self.effect_cache.get_mut(&layer)
            && entry.source_generation == generation
            && entry.width == width
            && entry.height == height
            && entry.scale_factor_bits == scale.to_bits()
            && entry.sigma_x_bits == sigma_x_physical.to_bits()
            && entry.sigma_y_bits == sigma_y_physical.to_bits()
            && entry.downsample_factor == downsample_factor
            && entry.target.format == self.window_gpu.config.format
            && entry.device_generation == self.device_generation
        {
            entry.last_used_frame = frame;
            self.counters.blur_cache_hits += 1;
            self.counters.effect_stage_cache_hits += 1;
            if for_shadow {
                self.counters.drop_shadow_blur_reuses += 1;
            }
            return Ok(Some(CachedEffect {
                origin: effect_origin,
                width,
                height,
            }));
        }
        if let Some(previous) = self.effect_cache.remove(&layer) {
            self.remove_effect_cache_bytes(previous.bytes);
            self.offscreen_target_pool.recycle(previous.target);
        }
        let final_target = if let Some(target) =
            self.offscreen_target_pool
                .take(width, height, self.config.format, true)
        {
            self.counters.effect_texture_pool_hits += 1;
            target
        } else {
            self.counters.effect_texture_pool_misses += 1;
            self.counters.effect_texture_creations += 1;
            self.create_offscreen_target(width, height, "incular retained blur result")
        };
        let padding = [
            ((source.origin.x - effect_origin.x) * scale).max(0.),
            ((source.origin.y - effect_origin.y) * scale).max(0.),
        ];
        let source_size = [source.width as f32, source.height as f32];
        let output_size = [width as f32, height as f32];
        let mut temps = Vec::new();
        let factor = downsample_factor as f32;
        let low_width = ((width as f32) / factor).ceil().max(1.) as u32;
        let low_height = ((height as f32) / factor).ceil().max(1.) as u32;
        let (horizontal, vertical) = if downsample_factor == 1 {
            let horizontal = self.take_effect_temp(width, height);
            (horizontal, None)
        } else {
            let downsample = self.take_effect_temp(low_width, low_height);
            let horizontal = self.take_effect_temp(low_width, low_height);
            let vertical = self.take_effect_temp(low_width, low_height);
            temps.push(downsample);
            (horizontal, Some(vertical))
        };
        let (horizontal, vertical_for_large) = (horizontal, vertical);
        let source_view = self
            .offscreen_cache
            .get(&layer)
            .expect("source cache for effect")
            .target
            .color_view
            .clone();
        if downsample_factor > 1 {
            let downsample = &temps[0];
            self.run_effect_pass(
                &source_view,
                &downsample.color_view,
                [padding[0], padding[1]],
                source_size,
                [low_width as f32, low_height as f32],
                [factor, factor],
                None,
            );
            self.counters.blur_downsample_passes += 1;
        }
        let blur_source_view = if downsample_factor > 1 {
            temps[0].color_view.clone()
        } else {
            source_view.clone()
        };
        let blur_source_size = if downsample_factor > 1 {
            [low_width as f32, low_height as f32]
        } else {
            source_size
        };
        let blur_output_size = if downsample_factor > 1 {
            [low_width as f32, low_height as f32]
        } else {
            output_size
        };
        let kernel_x = self.blur_kernel(sigma_x_physical / factor, downsample_factor);
        let kernel_y = self.blur_kernel(sigma_y_physical / factor, downsample_factor);
        self.run_effect_pass(
            &blur_source_view,
            &horizontal.color_view,
            if downsample_factor > 1 {
                [0., 0.]
            } else {
                padding
            },
            blur_source_size,
            blur_output_size,
            [1., 0.],
            Some(&kernel_x),
        );
        self.counters.blur_horizontal_passes += 1;
        let vertical_target = vertical_for_large.as_ref().unwrap_or(&final_target);
        self.run_effect_pass(
            &horizontal.color_view,
            &vertical_target.color_view,
            [0., 0.],
            blur_output_size,
            blur_output_size,
            [0., 1.],
            Some(&kernel_y),
        );
        self.counters.blur_vertical_passes += 1;
        if downsample_factor > 1 {
            self.run_effect_pass(
                &vertical_target.color_view,
                &final_target.color_view,
                [0., 0.],
                [low_width as f32, low_height as f32],
                output_size,
                [1. / factor, 1. / factor],
                None,
            );
            self.counters.blur_upsample_passes += 1;
        }
        self.recycle_effect_temp(horizontal);
        if let Some(vertical) = vertical_for_large {
            self.recycle_effect_temp(vertical);
        }
        for temp in temps {
            self.recycle_effect_temp(temp);
        }
        self.counters.blur_cache_misses += 1;
        self.counters.effect_stage_cache_misses += 1;
        self.counters.effect_stage_rerenders += 1;
        let bind_group =
            self.create_composite_bind_group(&final_target, "incular retained blur bind group");
        let bytes = final_target.bytes();
        self.effect_cache.insert(
            layer,
            EffectCacheEntry {
                target: final_target,
                bind_group,
                width,
                height,
                scale_factor_bits: scale.to_bits(),
                source_generation: generation,
                sigma_x_bits: sigma_x_physical.to_bits(),
                sigma_y_bits: sigma_y_physical.to_bits(),
                matrix_bits: [0; 20],
                device_generation: self.device_generation,
                last_used_frame: frame,
                bytes,
                downsample_factor,
            },
        );
        self.add_effect_cache_bytes(bytes);
        Ok(Some(CachedEffect {
            origin: effect_origin,
            width,
            height,
        }))
    }

    fn take_effect_temp(&mut self, width: u32, height: u32) -> OffscreenTarget {
        if let Some(target) =
            self.offscreen_target_pool
                .take(width, height, self.config.format, true)
        {
            self.counters.effect_texture_pool_hits += 1;
            target
        } else {
            self.counters.effect_texture_pool_misses += 1;
            self.counters.effect_texture_creations += 1;
            self.create_offscreen_target(width, height, "incular transient blur target")
        }
    }
    fn recycle_effect_temp(&mut self, target: OffscreenTarget) {
        self.offscreen_target_pool.recycle(target);
    }
    fn run_color_matrix_pass(
        &mut self,
        source: &wgpu::TextureView,
        destination: &wgpu::TextureView,
        filter: ColorFilter,
    ) {
        let params = GpuColorMatrixParams {
            matrix: filter.to_matrix(),
        };
        self.queue
            .write_buffer(&self.color_matrix_params, 0, bytemuck::bytes_of(&params));
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("incular color matrix pass bind group"),
            layout: &self.color_matrix_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.color_matrix_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.color_matrix_params.as_entire_binding(),
                },
            ],
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("incular color matrix pass encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("incular color matrix pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: destination,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.color_matrix_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.set_vertex_buffer(0, self.mesh.slice(..));
            pass.draw(0..6, 0..1);
        }
        self.queue.submit(Some(encoder.finish()));
    }
    fn add_effect_cache_bytes(&mut self, bytes: usize) {
        self.counters.offscreen_cached_bytes =
            self.counters.offscreen_cached_bytes.saturating_add(bytes);
        self.counters.effect_cached_bytes = self.counters.effect_cached_bytes.saturating_add(bytes);
        self.counters.effect_chain_cached_bytes = self.counters.effect_cached_bytes;
        self.counters.effect_peak_bytes = self
            .counters
            .effect_peak_bytes
            .max(self.counters.effect_cached_bytes);
        self.counters.offscreen_peak_cached_bytes = self
            .counters
            .offscreen_peak_cached_bytes
            .max(self.counters.offscreen_cached_bytes);
    }
    fn remove_effect_cache_bytes(&mut self, bytes: usize) {
        self.counters.offscreen_cached_bytes =
            self.counters.offscreen_cached_bytes.saturating_sub(bytes);
        self.counters.effect_cached_bytes = self.counters.effect_cached_bytes.saturating_sub(bytes);
        self.counters.effect_chain_cached_bytes = self.counters.effect_cached_bytes;
    }
    fn blur_kernel(&mut self, sigma: f32, downsample_factor: u32) -> BlurKernel {
        let sigma = quantize_blur_sigma(sigma);
        let key = BlurKernelKey {
            sigma_bits: sigma.to_bits(),
            downsample_factor,
        };
        if let Some(kernel) = self.blur_kernel_cache.get(&key) {
            self.counters.blur_kernel_cache_hits += 1;
            return kernel.clone();
        }
        let full = gaussian_kernel_weights(sigma);
        let radius = ((full.len().saturating_sub(1)) / 2).min(MAX_BLUR_RADIUS) as u32;
        let center = full.len() / 2;
        let mut weights = [0.; BLUR_WEIGHT_SLOTS];
        let radius = radius as usize;
        weights[..=radius].copy_from_slice(&full[center..=center + radius]);
        let kernel = BlurKernel {
            radius: radius as u32,
            weights,
        };
        self.blur_kernel_cache.insert(key, kernel.clone());
        self.counters.blur_kernel_cache_misses += 1;
        self.counters.blur_kernel_uploads += 1;
        kernel
    }
    #[allow(clippy::too_many_arguments)]
    fn run_effect_pass(
        &mut self,
        source: &wgpu::TextureView,
        destination: &wgpu::TextureView,
        source_origin: [f32; 2],
        source_size: [f32; 2],
        output_size: [f32; 2],
        direction: [f32; 2],
        kernel: Option<&BlurKernel>,
    ) {
        let mut weights = [0.; BLUR_WEIGHT_SLOTS];
        let radius = kernel.map_or(0, |kernel| {
            weights = kernel.weights;
            kernel.radius
        });
        let params = GpuBlurParams {
            source_origin,
            source_size,
            output_size,
            direction,
            radius,
            mode: u32::from(kernel.is_none()),
            _padding: [0; 2],
            weights,
        };
        self.queue
            .write_buffer(&self.blur_params, 0, bytemuck::bytes_of(&params));
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("incular transient effect bind group"),
            layout: &self.blur_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.blur_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.blur_params.as_entire_binding(),
                },
            ],
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("incular transient effect pass"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("incular gaussian pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: destination,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(if kernel.is_some() {
                &self.blur_pipeline
            } else {
                &self.resample_pipeline
            });
            pass.set_bind_group(0, &bind_group, &[]);
            pass.set_vertex_buffer(0, self.mesh.slice(..));
            pass.draw(0..6, 0..1);
        }
        self.queue.submit(Some(encoder.finish()));
        self.counters.offscreen_render_passes += 1;
    }

    fn append_path(
        &mut self,
        batches: &mut Vec<DrawBatch>,
        clip: ClipState,
        path: &std::sync::Arc<Path>,
        kind: PathMeshKind,
        brush: &Brush,
        placement: PathPlacement,
    ) {
        if clip == ClipState::Empty || !path_visible(path, placement.transform, clip) {
            return;
        }
        let key = PathMeshKey {
            path: path.id(),
            kind,
        };
        if !self.ensure_path_mesh(key, path) {
            return;
        }
        let gradient_id = self.ensure_gradient(brush);
        let (color, gradient, options) = path_paint(brush);
        batches.push(DrawBatch::Path {
            clip,
            key,
            gradient: gradient_id,
            instance: GpuPathInstance {
                affine: physical_affine(placement.transform, placement.scale).0,
                translation: physical_affine(placement.transform, placement.scale).1,
                surface: [self.target_width as f32, self.target_height as f32, 0., 0.],
                color: color.to_linear_rgba(),
                gradient,
                options,
            },
        });
    }
    fn ensure_path_mesh(&mut self, key: PathMeshKey, path: &Path) -> bool {
        self.counters.path_tessellation_requests += 1;
        let mesh = match self.cpu_path_cache.entry(key) {
            Entry::Occupied(entry) => {
                self.counters.path_tessellation_cache_hits += 1;
                entry.into_mut()
            }
            Entry::Vacant(entry) => {
                self.counters.path_tessellation_cache_misses += 1;
                let (rule, stroke) = match key.kind {
                    PathMeshKind::Fill(rule) => {
                        self.counters.path_fill_tessellations += 1;
                        (rule, None)
                    }
                    PathMeshKind::Stroke {
                        width,
                        cap,
                        join,
                        miter_limit,
                    } => {
                        self.counters.path_stroke_tessellations += 1;
                        (
                            FillRule::NonZero,
                            Some(Stroke {
                                width: f32::from_bits(width),
                                cap,
                                join,
                                miter_limit: f32::from_bits(miter_limit),
                            }),
                        )
                    }
                };
                let Some(mesh) = tessellate_path(path, rule, stroke) else {
                    return false;
                };
                self.counters.path_cpu_vertices += mesh.vertices.len() as u64;
                self.counters.path_cpu_indices += mesh.indices.len() as u64;
                if mesh.vertices.is_empty() || mesh.indices.is_empty() {
                    return false;
                }
                entry.insert(mesh)
            }
        };
        if let Some(gpu) = self.gpu_path_cache.get_mut(&key) {
            gpu.last_used_frame = self.counters.frames;
            self.counters.path_gpu_cache_hits += 1;
            return true;
        }
        let vertices = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("incular retained path vertices"),
                contents: bytemuck::cast_slice(&mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let indices = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("incular retained path indices"),
                contents: bytemuck::cast_slice(&mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });
        self.gpu_path_cache.insert(
            key,
            GpuPathMesh {
                vertices,
                indices,
                index_count: mesh.indices.len() as u32,
                last_used_frame: self.counters.frames,
            },
        );
        self.counters.path_gpu_cache_misses += 1;
        self.counters.path_gpu_uploads += 1;
        true
    }
    /// Makes the paint lookup retained independently from all geometry.
    fn ensure_gradient(&mut self, brush: &Brush) -> Option<GradientId> {
        let id = gradient_id(brush)?;
        self.counters.gradient_instances += 1;
        match brush {
            Brush::LinearGradient(_) => self.counters.linear_gradient_instances += 1,
            Brush::RadialGradient(_) => self.counters.radial_gradient_instances += 1,
            Brush::SweepGradient(_) => self.counters.radial_gradient_instances += 1,
            Brush::Solid(_) => {}
        }
        if let Some(resource) = self.gradient_cache.get_mut(&id) {
            resource.last_used_frame = self.counters.frames;
            self.counters.gradient_cache_hits += 1;
            return Some(id);
        }
        let stops = match brush {
            Brush::LinearGradient(g) => &g.stops,
            Brush::RadialGradient(g) => &g.stops,
            Brush::SweepGradient(g) => &g.stops,
            Brush::Solid(_) => unreachable!("solid has no gradient id"),
        };
        // LUT texels are premultiplied linear RGB. The shader unpremultiplies
        // before the renderer's straight-alpha source-over blend, avoiding
        // dark fringes across transparent stops.
        let pixels = gradient_lut_pixels(stops);
        let (shared_resource, uploaded) = self.shared.gradient_resource(
            id,
            self.window_gpu.config.format,
            &pixels,
            &self.gradient_bind_group_layout,
            &self.gradient_sampler,
        );
        self.gradient_cache
            .insert(id, shared_resource.resource.clone());
        self.counters.gradient_cache_misses += 1;
        if uploaded {
            self.counters.gradient_resource_creations += 1;
            self.counters.gradient_resource_uploads += 1;
        }
        Some(id)
    }
    fn gradient_bind_group(&self, id: Option<GradientId>) -> &wgpu::BindGroup {
        id.and_then(|id| {
            self.gradient_cache
                .get(&id)
                .map(|resource| &resource.bind_group)
        })
        .unwrap_or(&self.solid_gradient.bind_group)
    }
    fn ensure_gpu_image(&mut self, image: &ImageHandle) -> Result<(), RendererError> {
        let id = image.id();
        if let Some(resource) = self.image_cache.get_mut(&id) {
            debug_assert_eq!(resource.resource.width, image.decoded().width());
            debug_assert_eq!(resource.resource.height, image.decoded().height());
            resource.last_used_frame = self.counters.frames;
            self.counters.image_cache_hits += 1;
            return Ok(());
        }
        let (resource, uploaded) = self.shared.image_resource(image)?;
        debug_assert_eq!(
            self.shared.image_resource_identity(id),
            Some(resource.identity),
            "window image cache must reference the context-wide texture identity"
        );
        self.image_cache.insert(
            id,
            GpuImage {
                resource,
                bind_groups: HashMap::new(),
                last_used_frame: self.counters.frames,
            },
        );
        self.counters.image_cache_misses += 1;
        if uploaded {
            self.counters.image_texture_creations += 1;
            self.counters.image_texture_uploads += 1;
        }
        Ok(())
    }
    fn evict_unused_images(&mut self) {
        let frame = self.counters.frames;
        let before = self.image_cache.len();
        self.image_cache.retain(|_, image| {
            frame.saturating_sub(image.last_used_frame) <= IMAGE_CACHE_MAX_UNUSED_FRAMES
        });
        self.counters.image_texture_evictions += (before - self.image_cache.len()) as u64;
    }
    fn evict_unused_path_meshes(&mut self) {
        let frame = self.counters.frames;
        let before = self.gpu_path_cache.len();
        self.gpu_path_cache.retain(|_, mesh| {
            frame.saturating_sub(mesh.last_used_frame) <= PATH_CACHE_MAX_UNUSED_FRAMES
        });
        self.counters.path_gpu_evictions += (before - self.gpu_path_cache.len()) as u64;
    }
    fn evict_unused_gradients(&mut self) {
        let frame = self.counters.frames;
        let before = self.gradient_cache.len();
        self.gradient_cache.retain(|_, gradient| {
            frame.saturating_sub(gradient.last_used_frame) <= GRADIENT_CACHE_MAX_UNUSED_FRAMES
        });
        self.counters.gradient_resource_evictions += (before - self.gradient_cache.len()) as u64;
    }
    fn evict_offscreen_cache(&mut self) {
        let frame = self.counters.frames;
        loop {
            let source = self
                .offscreen_cache
                .iter()
                .min_by_key(|(_, entry)| entry.last_used_frame)
                .map(|(layer, _)| (*layer, true));
            let effect = self
                .effect_cache
                .iter()
                .min_by_key(|(_, entry)| entry.last_used_frame)
                .map(|(layer, _)| (*layer, false));
            let Some((layer, is_source)) = (match (source, effect) {
                (Some(left), Some(right)) => Some(if left.1 && !right.1 {
                    // Pick the oldest timestamp rather than privileging one
                    // cache. The bool only identifies ownership.
                    let left_frame = self
                        .offscreen_cache
                        .get(&left.0)
                        .map_or(u64::MAX, |entry| entry.last_used_frame);
                    let right_frame = self
                        .effect_cache
                        .get(&right.0)
                        .map_or(u64::MAX, |entry| entry.last_used_frame);
                    if left_frame <= right_frame {
                        left
                    } else {
                        right
                    }
                } else {
                    left
                }),
                (Some(value), None) | (None, Some(value)) => Some(value),
                (None, None) => None,
            }) else {
                break;
            };
            let last_used = if is_source {
                self.offscreen_cache
                    .get(&layer)
                    .map_or(u64::MAX, |entry| entry.last_used_frame)
            } else {
                self.effect_cache
                    .get(&layer)
                    .map_or(u64::MAX, |entry| entry.last_used_frame)
            };
            let stale = frame.saturating_sub(last_used) > OFFSCREEN_CACHE_MAX_UNUSED_FRAMES;
            if self.counters.offscreen_cached_bytes <= self.offscreen_cache_budget && !stale {
                break;
            }
            if is_source {
                let Some(entry) = self.offscreen_cache.remove(&layer) else {
                    continue;
                };
                self.counters.offscreen_cached_bytes = self
                    .counters
                    .offscreen_cached_bytes
                    .saturating_sub(entry.bytes);
                self.offscreen_target_pool.recycle(entry.target);
            } else {
                let Some(entry) = self.effect_cache.remove(&layer) else {
                    continue;
                };
                self.remove_effect_cache_bytes(entry.bytes);
                self.offscreen_target_pool.recycle(entry.target);
            }
            self.counters.offscreen_texture_evictions += 1;
            self.counters.effect_texture_evictions += u64::from(!is_source);
        }
    }
    fn prepare_image_bind_groups(&mut self, batches: &[DrawBatch]) {
        for batch in batches {
            if let DrawBatch::Images {
                image, sampling, ..
            } = batch
            {
                let already_bound = self
                    .image_cache
                    .get(image)
                    .is_some_and(|resource| resource.bind_groups.contains_key(sampling));
                if already_bound {
                    continue;
                }
                let Some(view) = self
                    .image_cache
                    .get(image)
                    .map(|resource| &resource.resource.view)
                else {
                    continue;
                };
                let Some(sampler) = self.image_samplers.get(sampling) else {
                    continue;
                };
                let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("incular retained image bind group"),
                    layout: &self.image_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(sampler),
                        },
                    ],
                });
                if let Some(resource) = self.image_cache.get_mut(image) {
                    resource.bind_groups.insert(*sampling, bind_group);
                }
            }
        }
    }
    fn image_bind_group(
        &self,
        image: ImageId,
        sampling: ImageSampling,
    ) -> Option<&wgpu::BindGroup> {
        self.image_cache.get(&image)?.bind_groups.get(&sampling)
    }
    fn upload_glyph(&mut self, entry: AtlasEntry, bitmap: &[u8]) {
        self.ensure_atlas_page(entry.page);
        let page = &self.atlas_pages[usize::from(entry.page)];
        let padded_width = usize::from(entry.width + ATLAS_PADDING * 2);
        let padded_height = usize::from(entry.height + ATLAS_PADDING * 2);
        // A freshly allocated texture has undefined contents. Explicitly write
        // the allocation, including its transparent border, before enabling
        // linear filtering so adjacent glyphs can never bleed into this mask.
        let mut padded = vec![0_u8; padded_width * padded_height];
        for row in 0..usize::from(entry.height) {
            let source_start = row * usize::from(entry.width);
            let destination_start =
                (row + usize::from(ATLAS_PADDING)) * padded_width + usize::from(ATLAS_PADDING);
            padded[destination_start..destination_start + usize::from(entry.width)]
                .copy_from_slice(&bitmap[source_start..source_start + usize::from(entry.width)]);
        }
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &page.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: u32::from(entry.x - ATLAS_PADDING),
                    y: u32::from(entry.y - ATLAS_PADDING),
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &padded,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_width as u32),
                rows_per_image: Some(padded_height as u32),
            },
            wgpu::Extent3d {
                width: padded_width as u32,
                height: padded_height as u32,
                depth_or_array_layers: 1,
            },
        );
    }
    fn ensure_atlas_page(&mut self, page: u16) {
        while self.atlas_pages.len() <= usize::from(page) {
            let texture = self
                .shared
                .shared_glyph_texture(self.atlas_pages.len() as u16);
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("incular glyph atlas bind group"),
                layout: &self.atlas_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.atlas_sampler),
                    },
                ],
            });
            self.atlas_pages.push(GpuAtlasPage {
                texture,
                bind_group,
            });
            self.counters.atlas_texture_recreations += 1;
        }
    }
}

fn create_rectangle_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    create_pipeline(
        device,
        format,
        RECT_SHADER,
        "incular rectangle pipeline",
        None,
        rectangle_layout(),
        content_stencil(),
        wgpu::ColorWrites::ALL,
    )
}
fn create_text_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    atlas: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    create_pipeline(
        device,
        format,
        TEXT_SHADER,
        "incular text pipeline",
        Some(atlas),
        glyph_layout(),
        content_stencil(),
        wgpu::ColorWrites::ALL,
    )
}
fn create_image_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    images: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    create_pipeline(
        device,
        format,
        IMAGE_SHADER,
        "incular image pipeline",
        Some(images),
        image_layout(),
        content_stencil(),
        wgpu::ColorWrites::ALL,
    )
}
fn create_rounded_rect_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    gradients: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    create_pipeline(
        device,
        format,
        RRECT_SHADER,
        "incular analytic rounded rectangle pipeline",
        Some(gradients),
        rrect_layout(),
        content_stencil(),
        wgpu::ColorWrites::ALL,
    )
}
fn create_path_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    gradients: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("incular retained path shader"),
        source: wgpu::ShaderSource::Wgsl(PATH_SHADER.into()),
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("incular retained path pipeline"),
        layout: Some(
            &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("incular path gradient layout"),
                bind_group_layouts: &[Some(gradients)],
                immediate_size: 0,
            }),
        ),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[Some(quad_layout()), Some(path_instance_layout())],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: content_stencil(),
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}
fn create_composite_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("incular opacity composite shader"),
        source: wgpu::ShaderSource::Wgsl(COMPOSITE_SHADER.into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("incular opacity composite layout"),
        bind_group_layouts: &[Some(layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("incular opacity composite pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[Some(quad_layout()), Some(composite_layout())],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: content_stencil(),
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn create_fixed_blend_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    layout: &wgpu::BindGroupLayout,
    mode: BlendMode,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("incular fixed-function blend shader"),
        source: wgpu::ShaderSource::Wgsl(FIXED_BLEND_SHADER.into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("incular fixed-function blend layout"),
        bind_group_layouts: &[Some(layout)],
        immediate_size: 0,
    });
    let factor = |source: wgpu::BlendFactor, destination: wgpu::BlendFactor| wgpu::BlendComponent {
        src_factor: source,
        dst_factor: destination,
        operation: wgpu::BlendOperation::Add,
    };
    let (source, destination) = match mode {
        BlendMode::SrcOver => (wgpu::BlendFactor::One, wgpu::BlendFactor::OneMinusSrcAlpha),
        BlendMode::Src => (wgpu::BlendFactor::One, wgpu::BlendFactor::Zero),
        BlendMode::DstOver => (wgpu::BlendFactor::OneMinusDstAlpha, wgpu::BlendFactor::One),
        BlendMode::SrcIn => (wgpu::BlendFactor::DstAlpha, wgpu::BlendFactor::Zero),
        BlendMode::DstIn => (wgpu::BlendFactor::Zero, wgpu::BlendFactor::SrcAlpha),
        BlendMode::SrcOut => (wgpu::BlendFactor::OneMinusDstAlpha, wgpu::BlendFactor::Zero),
        BlendMode::DstOut => (wgpu::BlendFactor::Zero, wgpu::BlendFactor::OneMinusSrcAlpha),
        BlendMode::SrcAtop => (
            wgpu::BlendFactor::DstAlpha,
            wgpu::BlendFactor::OneMinusSrcAlpha,
        ),
        BlendMode::DstAtop => (
            wgpu::BlendFactor::OneMinusDstAlpha,
            wgpu::BlendFactor::SrcAlpha,
        ),
        BlendMode::Xor => (
            wgpu::BlendFactor::OneMinusDstAlpha,
            wgpu::BlendFactor::OneMinusSrcAlpha,
        ),
        BlendMode::Plus => (wgpu::BlendFactor::One, wgpu::BlendFactor::One),
        _ => unreachable!("fixed blend pipeline only supports Porter-Duff modes"),
    };
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("incular fixed-function blend pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[Some(quad_layout()), Some(composite_layout())],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: content_stencil(),
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState {
                    color: factor(source, destination),
                    alpha: factor(source, destination),
                }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn create_effect_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    source: &str,
    label: &'static str,
    layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("incular effect pipeline layout"),
        bind_group_layouts: &[Some(layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[Some(quad_layout())],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}
fn create_blend_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("incular destination blend shader"),
        source: wgpu::ShaderSource::Wgsl(BLEND_SHADER.into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("incular destination blend pipeline layout"),
        bind_group_layouts: &[Some(layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("incular destination blend pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[Some(quad_layout()), Some(composite_layout())],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: content_stencil(),
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}
fn stencil_state(operation: wgpu::StencilOperation) -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: wgpu::TextureFormat::Depth24PlusStencil8,
        depth_write_enabled: Some(false),
        depth_compare: Some(wgpu::CompareFunction::Always),
        stencil: wgpu::StencilState {
            front: wgpu::StencilFaceState {
                compare: wgpu::CompareFunction::Equal,
                fail_op: wgpu::StencilOperation::Keep,
                depth_fail_op: wgpu::StencilOperation::Keep,
                pass_op: operation,
            },
            back: wgpu::StencilFaceState {
                compare: wgpu::CompareFunction::Equal,
                fail_op: wgpu::StencilOperation::Keep,
                depth_fail_op: wgpu::StencilOperation::Keep,
                pass_op: operation,
            },
            read_mask: u32::from(u8::MAX),
            write_mask: u32::from(u8::MAX),
        },
        bias: wgpu::DepthBiasState::default(),
    }
}
fn content_stencil() -> Option<wgpu::DepthStencilState> {
    Some(stencil_state(wgpu::StencilOperation::Keep))
}
fn create_stencil_attachment(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("incular retained stencil attachment"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth24PlusStencil8,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}
fn create_rounded_rect_mask_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    gradients: &wgpu::BindGroupLayout,
    operation: wgpu::StencilOperation,
) -> wgpu::RenderPipeline {
    create_pipeline(
        device,
        format,
        RRECT_SHADER,
        "incular rounded clip mask",
        Some(gradients),
        rrect_layout(),
        Some(stencil_state(operation)),
        wgpu::ColorWrites::empty(),
    )
}
fn create_path_mask_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    gradients: &wgpu::BindGroupLayout,
    operation: wgpu::StencilOperation,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("incular path clip mask shader"),
        source: wgpu::ShaderSource::Wgsl(PATH_SHADER.into()),
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("incular path clip mask pipeline"),
        layout: Some(
            &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("incular path clip layout"),
                bind_group_layouts: &[Some(gradients)],
                immediate_size: 0,
            }),
        ),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[Some(quad_layout()), Some(path_instance_layout())],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: Some(stencil_state(operation)),
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: None,
                write_mask: wgpu::ColorWrites::empty(),
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}
#[allow(clippy::too_many_arguments)] // Pipeline descriptor fields stay explicit at call sites.
fn create_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    source: &str,
    label: &'static str,
    atlas: Option<&wgpu::BindGroupLayout>,
    second: wgpu::VertexBufferLayout<'static>,
    depth_stencil: Option<wgpu::DepthStencilState>,
    write_mask: wgpu::ColorWrites,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let layout = atlas.map(|atlas| {
        device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("incular text layout"),
            bind_group_layouts: &[Some(atlas)],
            immediate_size: 0,
        })
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: layout.as_ref(),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[Some(quad_layout()), Some(second)],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}
fn quad_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: 8,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x2,
            offset: 0,
            shader_location: 0,
        }],
    }
}
fn rectangle_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<GpuInstance>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 0,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 16,
                shader_location: 2,
            },
        ],
    }
}
fn glyph_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<GpuGlyphInstance>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 0,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 16,
                shader_location: 2,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 32,
                shader_location: 3,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 48,
                shader_location: 4,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 64,
                shader_location: 5,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 80,
                shader_location: 6,
            },
        ],
    }
}
fn image_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<GpuImageInstance>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 0,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 16,
                shader_location: 2,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 32,
                shader_location: 3,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 48,
                shader_location: 4,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 64,
                shader_location: 5,
            },
        ],
    }
}
fn rrect_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<GpuRRectInstance>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 0,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 16,
                shader_location: 2,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 32,
                shader_location: 3,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 48,
                shader_location: 4,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 64,
                shader_location: 5,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 80,
                shader_location: 6,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 96,
                shader_location: 7,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 112,
                shader_location: 8,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 128,
                shader_location: 9,
            },
        ],
    }
}
fn path_instance_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<GpuPathInstance>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 0,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 16,
                shader_location: 2,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 32,
                shader_location: 3,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 48,
                shader_location: 4,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 64,
                shader_location: 5,
            },
        ],
    }
}
fn composite_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<GpuCompositeInstance>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 0,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 16,
                shader_location: 2,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 32,
                shader_location: 3,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 48,
                shader_location: 4,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 64,
                shader_location: 5,
            },
        ],
    }
}
fn create_instance_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("incular rectangle instances"),
        size: (capacity * std::mem::size_of::<GpuInstance>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}
fn create_glyph_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("incular glyph instances"),
        size: (capacity * std::mem::size_of::<GpuGlyphInstance>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}
fn create_image_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("incular image instances"),
        size: (capacity * std::mem::size_of::<GpuImageInstance>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}
fn create_rrect_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("incular rounded rectangle instances"),
        size: (capacity * std::mem::size_of::<GpuRRectInstance>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}
fn create_path_instance_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("incular path paint instances"),
        size: (capacity * std::mem::size_of::<GpuPathInstance>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}
fn create_composite_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("incular opacity composite instances"),
        size: (capacity * std::mem::size_of::<GpuCompositeInstance>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}
fn create_gradient_resource(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    pixels: &[[u8; 4]],
) -> GpuGradient {
    let width = pixels.len().max(1) as u32;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("incular retained gradient lookup"),
        size: wgpu::Extent3d {
            width,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        bytemuck::cast_slice(pixels),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(1),
        },
        wgpu::Extent3d {
            width,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("incular retained gradient bind group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    });
    GpuGradient {
        _texture: texture,
        bind_group,
        last_used_frame: 0,
    }
}
fn gradient_lut_pixels(stops: &incular_painting::GradientStops) -> Vec<[u8; 4]> {
    (0..GRADIENT_LUT_SAMPLES)
        .map(|index| {
            let t = index as f32 / (GRADIENT_LUT_SAMPLES - 1) as f32;
            sample_gradient_stops(stops, t)
                .map(|channel| (channel.clamp(0., 1.) * 255.).round() as u8)
        })
        .collect()
}
fn stroke_mesh_kind(stroke: Stroke) -> PathMeshKind {
    PathMeshKind::Stroke {
        width: stroke.width.max(0.).to_bits(),
        cap: stroke.cap,
        join: stroke.join,
        miter_limit: stroke.miter_limit.max(1.01).to_bits(),
    }
}
fn rect_path(rect: Rect) -> Path {
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
fn oval_path(rect: Rect) -> Path {
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
fn path_visible(path: &Path, transform: Transform, clip: ClipState) -> bool {
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
fn stencil_depth(clip: ClipState) -> u8 {
    match clip {
        ClipState::Stencil { depth, .. } => depth,
        _ => 0,
    }
}
fn with_stencil_depth(clip: ClipState, depth: u8) -> ClipState {
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
fn intersect_clip_with_rect(clip: ClipState, next: ClipRect) -> ClipState {
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

fn batches_have_content(batches: &[DrawBatch]) -> bool {
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
fn count_rectangles(batches: &[DrawBatch]) -> usize {
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
fn count_glyphs(batches: &[DrawBatch]) -> usize {
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
fn count_images(batches: &[DrawBatch]) -> usize {
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
fn count_rounded(batches: &[DrawBatch]) -> usize {
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
fn count_paths(batches: &[DrawBatch]) -> usize {
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
fn count_composites(batches: &[DrawBatch]) -> usize {
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
fn choose_blur_downsample_factor(sigma_physical: f32) -> u32 {
    let mut factor = 1_u32;
    let mut effective = normalize_sigma(sigma_physical);
    while effective > LARGE_BLUR_SIGMA_THRESHOLD && factor < 64 {
        factor *= 2;
        effective = sigma_physical / factor as f32;
    }
    factor
}
#[must_use]
fn quantize_blur_sigma(sigma: f32) -> f32 {
    let sigma = normalize_sigma(sigma);
    if sigma <= f32::EPSILON {
        return 0.;
    }
    (sigma / BLUR_KERNEL_QUANTUM).round() * BLUR_KERNEL_QUANTUM
}
fn append_rectangle(batches: &mut Vec<DrawBatch>, clip: ClipState, instance: RectangleInstance) {
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
fn append_glyph(
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
fn append_image(
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
fn append_rrect(
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
fn gradient_id(brush: &Brush) -> Option<GradientId> {
    match brush {
        Brush::Solid(_) => None,
        Brush::LinearGradient(g) => Some(g.stops.id()),
        Brush::RadialGradient(g) => Some(g.stops.id()),
        Brush::SweepGradient(g) => Some(g.stops.id()),
    }
}
fn path_paint(brush: &Brush) -> (Color, [f32; 4], [f32; 4]) {
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
fn clip_path_instance(
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

fn physical_affine(transform: Transform, scale: f32) -> ([f32; 4], [f32; 4]) {
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
fn rrect_instance(
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
fn border_instance(
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
fn glyph_instance(
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
fn image_instance(
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
fn logical_instance(
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
fn ndc_rect(x: f32, y: f32, w: f32, h: f32, width: f32, height: f32) -> [f32; 4] {
    [
        2. * x / width - 1.,
        1. - 2. * y / height,
        2. * w / width,
        -2. * h / height,
    ]
}
#[allow(clippy::too_many_arguments)]
fn composite_instance(
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
fn composite_instance_with_color(
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
fn blend_instance(
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
/// Applies an isolated group alpha to a premultiplied offscreen sample. The
/// compositor shader mirrors this operation before using premultiplied
/// source-over blending.
#[cfg(test)]
fn apply_group_alpha(sample: [f32; 4], alpha: f32) -> [f32; 4] {
    let alpha = normalize_opacity(alpha);
    [
        sample[0] * alpha,
        sample[1] * alpha,
        sample[2] * alpha,
        sample[3] * alpha,
    ]
}
fn find_opacity_end(commands: &[PaintCommand], start: usize) -> Result<usize, RendererError> {
    let mut depth = 0_usize;
    for (index, command) in commands.iter().enumerate().skip(start) {
        match command {
            PaintCommand::PushOpacity { .. } => depth += 1,
            PaintCommand::PopOpacity => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Ok(index);
                }
            }
            _ => {}
        }
    }
    Err(RendererError::UnbalancedClipStack)
}

fn find_effect_end(commands: &[PaintCommand], start: usize) -> Result<usize, RendererError> {
    let mut depth = 0_usize;
    for (index, command) in commands.iter().enumerate().skip(start) {
        match command {
            PaintCommand::PushBlur { .. }
            | PaintCommand::PushDropShadow { .. }
            | PaintCommand::PushColorFilter { .. }
            | PaintCommand::PushBlend { .. } => depth += 1,
            PaintCommand::PopEffect => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Ok(index);
                }
            }
            _ => {}
        }
    }
    Err(RendererError::UnbalancedClipStack)
}
fn commands_have_effects(commands: &[PaintCommand]) -> bool {
    commands.iter().any(|command| {
        matches!(
            command,
            PaintCommand::PushOpacity { .. }
                | PaintCommand::PushBlur { .. }
                | PaintCommand::PushDropShadow { .. }
                | PaintCommand::PushColorFilter { .. }
                | PaintCommand::PushBlend { .. }
        )
    })
}

fn commands_have_destination_blend(commands: &[PaintCommand]) -> bool {
    commands.iter().any(|command| {
        matches!(
            command,
            PaintCommand::PushBlend { mode, .. } if mode.requires_destination_read()
        )
    })
}

fn add_composition_bounds(bounds: &mut Option<Rect>, candidate: Rect) {
    if !candidate.origin.x.is_finite()
        || !candidate.origin.y.is_finite()
        || !candidate.size.width.is_finite()
        || !candidate.size.height.is_finite()
        || candidate.size.width <= 0.
        || candidate.size.height <= 0.
    {
        return;
    }
    *bounds = Some(match bounds.take() {
        Some(current) => {
            let left = current.origin.x.min(candidate.origin.x);
            let top = current.origin.y.min(candidate.origin.y);
            let right = (current.origin.x + current.size.width)
                .max(candidate.origin.x + candidate.size.width);
            let bottom = (current.origin.y + current.size.height)
                .max(candidate.origin.y + candidate.size.height);
            Rect::from_origin_size(
                Offset::new(left, top),
                Size::new(right - left, bottom - top),
            )
        }
        None => candidate,
    });
}

/// Computes a conservative logical scene scope for destination promotion. It
/// deliberately includes every visible command (and expanded blur/shadow
/// bounds), so presenting a tight target cannot omit content outside a small
/// blend group. The ordinary direct path still uses the complete surface.
fn display_list_composition_bounds(commands: &[PaintCommand]) -> Option<Rect> {
    let mut transforms = vec![Offset::ZERO];
    let mut bounds = None;
    for command in commands {
        let translation = *transforms.last().expect("root transform");
        match command {
            PaintCommand::Rect { rect, .. }
            | PaintCommand::Image {
                destination: rect, ..
            } => add_composition_bounds(&mut bounds, translated_rect(*rect, translation)),
            PaintCommand::RRect { rrect, .. } | PaintCommand::Border { rrect, .. } => {
                add_composition_bounds(&mut bounds, translated_rect(rrect.rect, translation));
            }
            PaintCommand::FillPath { path, .. } | PaintCommand::StrokePath { path, .. } => {
                if let Some(path_bounds) = path.bounds() {
                    let margin = match command {
                        PaintCommand::StrokePath { stroke, .. } => {
                            (stroke.width.max(0.) * 0.5 * stroke.miter_limit.max(1.)).max(0.)
                        }
                        _ => 0.,
                    };
                    add_composition_bounds(
                        &mut bounds,
                        Rect::from_origin_size(
                            Offset::new(
                                path_bounds.origin.x - margin + translation.x,
                                path_bounds.origin.y - margin + translation.y,
                            ),
                            Size::new(
                                path_bounds.size.width + margin * 2.,
                                path_bounds.size.height + margin * 2.,
                            ),
                        ),
                    );
                }
            }
            PaintCommand::GlyphRun { run, .. } => {
                if let Some((left, right)) = run
                    .glyphs
                    .iter()
                    .map(|glyph| (glyph.offset.x, glyph.offset.x + glyph.advance.max(0.)))
                    .reduce(|(left, right), (next_left, next_right)| {
                        (left.min(next_left), right.max(next_right))
                    })
                {
                    let font_size = run.font_size.abs().max(1.);
                    let x_margin = font_size * 0.25;
                    add_composition_bounds(
                        &mut bounds,
                        Rect::from_origin_size(
                            Offset::new(
                                run.origin.x + left - x_margin + translation.x,
                                run.origin.y - font_size * 1.35 + translation.y,
                            ),
                            Size::new((right - left + x_margin * 2.).max(1.), font_size * 1.7),
                        ),
                    );
                }
            }
            // Effect bounds emitted by LayerTree are already in the current
            // world coordinate space. Expand only the effects that can paint
            // outside their source rectangle.
            PaintCommand::PushOpacity { bounds: rect, .. }
            | PaintCommand::PushColorFilter { bounds: rect, .. }
            | PaintCommand::PushBlend { bounds: rect, .. } => {
                add_composition_bounds(&mut bounds, *rect);
            }
            PaintCommand::PushBlur {
                bounds: rect, blur, ..
            } => {
                add_composition_bounds(&mut bounds, blur_bounds(*rect, blur.sigma_x, blur.sigma_y))
            }
            PaintCommand::PushDropShadow {
                bounds: rect,
                shadow,
                ..
            } => add_composition_bounds(
                &mut bounds,
                drop_shadow_bounds(*rect, shadow.offset, shadow.sigma_x, shadow.sigma_y),
            ),
            PaintCommand::PushTransform { transform } => {
                transforms.push(translation + transform.translation_offset());
            }
            PaintCommand::PopTransform => {
                if transforms.len() > 1 {
                    transforms.pop();
                }
            }
            PaintCommand::PushClip { .. }
            | PaintCommand::PushClipRRect { .. }
            | PaintCommand::PushClipOval { .. }
            | PaintCommand::PushClipPath { .. }
            | PaintCommand::PopClip
            | PaintCommand::PopOpacity
            | PaintCommand::PopEffect => {}
        }
    }
    bounds
}

fn destination_composition_scope(
    list: &DisplayList,
    scale: f32,
    surface_width: u32,
    surface_height: u32,
) -> (Offset, u32, u32) {
    let full = Rect::from_origin_size(
        Offset::ZERO,
        Size::new(surface_width as f32 / scale, surface_height as f32 / scale),
    );
    let scope = display_list_composition_bounds(list.commands())
        .and_then(|bounds| intersect_rect(bounds, full))
        .unwrap_or(full);
    let left = (scope.origin.x * scale).floor().max(0.) as u32;
    let top = (scope.origin.y * scale).floor().max(0.) as u32;
    let right = ((scope.origin.x + scope.size.width) * scale)
        .ceil()
        .min(surface_width as f32) as u32;
    let bottom = ((scope.origin.y + scope.size.height) * scale)
        .ceil()
        .min(surface_height as f32) as u32;
    if right <= left || bottom <= top {
        return (Offset::ZERO, surface_width.max(1), surface_height.max(1));
    }
    (
        Offset::new(left as f32 / scale, top as f32 / scale),
        right - left,
        bottom - top,
    )
}

fn translated_rect(rect: Rect, offset: Offset) -> Rect {
    Rect::from_origin_size(rect.origin + offset, rect.size)
}
fn intersect_rect(a: Rect, b: Rect) -> Option<Rect> {
    let left = a.origin.x.max(b.origin.x);
    let top = a.origin.y.max(b.origin.y);
    let right = (a.origin.x + a.size.width).min(b.origin.x + b.size.width);
    let bottom = (a.origin.y + a.size.height).min(b.origin.y + b.size.height);
    (right > left && bottom > top).then(|| {
        Rect::from_origin_size(
            Offset::new(left, top),
            Size::new(right - left, bottom - top),
        )
    })
}
fn set_scissor(
    pass: &mut wgpu::RenderPass<'_>,
    clip: ClipState,
    width: u32,
    height: u32,
    scale: f32,
) -> bool {
    let clip = match clip {
        ClipState::Rect(clip) => Some(clip),
        ClipState::Stencil { rect, .. } => rect,
        ClipState::Empty => return false,
        ClipState::Unbounded => {
            pass.set_scissor_rect(0, 0, width, height);
            return true;
        }
    };
    let Some(clip) = clip else {
        pass.set_scissor_rect(0, 0, width, height);
        return true;
    };
    let x0 = (clip.rect.origin.x * scale).floor().max(0.) as u32;
    let y0 = (clip.rect.origin.y * scale).floor().max(0.) as u32;
    let x1 = ((clip.rect.origin.x + clip.rect.size.width) * scale)
        .ceil()
        .max(0.) as u32;
    let y1 = ((clip.rect.origin.y + clip.rect.size.height) * scale)
        .ceil()
        .max(0.) as u32;
    let x0 = x0.min(width);
    let y0 = y0.min(height);
    let scissor_width = x1.min(width).saturating_sub(x0);
    let scissor_height = y1.min(height).saturating_sub(y0);
    if scissor_width == 0 || scissor_height == 0 {
        return false;
    }
    pass.set_scissor_rect(x0, y0, scissor_width, scissor_height);
    true
}
fn normalized_scale(scale: f64) -> f32 {
    if scale.is_finite() && scale > 0. {
        scale as f32
    } else {
        1.
    }
}
fn physical_debug_size(bounds: Rect, scale: f32) -> (u32, u32) {
    (
        (bounds.size.width.max(0.) * scale).ceil() as u32,
        (bounds.size.height.max(0.) * scale).ceil() as u32,
    )
}
fn clamp_i16(value: i32) -> i16 {
    value.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
}

#[cfg(test)]
mod tests {
    use super::*;
    use incular_core::Transform;
    use incular_text::{TextAlign, TextEngine, TextStyle};
    #[test]
    fn gpu_gradient_lut_contains_five_stop_middle_colors() {
        let stops = incular_painting::GradientStops::new(vec![
            incular_painting::GradientStop {
                offset: 0.,
                color: Color::rgba(255, 0, 0, 255),
            },
            incular_painting::GradientStop {
                offset: 0.25,
                color: Color::rgba(255, 255, 0, 255),
            },
            incular_painting::GradientStop {
                offset: 0.5,
                color: Color::rgba(0, 255, 0, 255),
            },
            incular_painting::GradientStop {
                offset: 0.75,
                color: Color::rgba(0, 255, 255, 255),
            },
            incular_painting::GradientStop {
                offset: 1.,
                color: Color::rgba(0, 0, 255, 255),
            },
        ]);
        let lut = gradient_lut_pixels(&stops);
        assert_eq!(lut.len(), GRADIENT_LUT_SAMPLES as usize);
        assert_eq!(lut[0], [255, 0, 0, 255]);
        assert!(lut[64][0] > 240 && lut[64][1] > 240, "yellow reaches LUT");
        assert!(lut[128][1] > 240 && lut[128][0] < 16, "green reaches LUT");
        assert!(
            lut[191][1] > 240 && lut[191][2] > 240,
            "cyan stop reaches LUT"
        );
        assert_eq!(lut[255], [0, 0, 255, 255]);
    }
    #[test]
    fn lyon_tessellates_curves_and_fill_rules() {
        let mut b = incular_painting::Path::builder();
        b.move_to(Offset::new(0., 0.))
            .quadratic_to(Offset::new(20., 30.), Offset::new(40., 0.))
            .cubic_to(
                Offset::new(35., 20.),
                Offset::new(5., 20.),
                Offset::new(0., 0.),
            )
            .close();
        let path = b.build();
        for rule in [
            incular_painting::FillRule::NonZero,
            incular_painting::FillRule::EvenOdd,
        ] {
            let mesh = tessellate_path(&path, rule, None).expect("finite curve mesh");
            assert!(!mesh.indices.is_empty());
            assert!(
                mesh.vertices
                    .iter()
                    .all(|p| p[0].is_finite() && p[1].is_finite())
            );
        }
    }
    #[test]
    fn stroke_tessellation_maps_caps_joins_and_widths_to_distinct_mesh_keys() {
        let mut b = Path::builder();
        b.move_to(Offset::new(1., 1.))
            .line_to(Offset::new(20., 1.))
            .line_to(Offset::new(20., 20.));
        let path = b.build();
        let butt = Stroke {
            width: 2.,
            cap: LineCap::Butt,
            join: LineJoin::Miter,
            ..Stroke::default()
        };
        let round = Stroke {
            cap: LineCap::Round,
            join: LineJoin::Round,
            ..butt
        };
        assert_ne!(stroke_mesh_kind(butt), stroke_mesh_kind(round));
        assert_ne!(
            tessellate_path(&path, FillRule::NonZero, Some(butt))
                .expect("butt stroke")
                .indices,
            tessellate_path(&path, FillRule::NonZero, Some(round))
                .expect("round stroke")
                .indices,
        );
        assert_ne!(
            stroke_mesh_kind(butt),
            stroke_mesh_kind(Stroke { width: 6., ..butt }),
        );
    }
    #[test]
    fn empty_and_degenerate_paths_are_safe_noop_meshes() {
        let empty = Path::default();
        assert!(
            tessellate_path(&empty, FillRule::NonZero, None).is_none_or(|m| m.indices.is_empty())
        );
        let mut b = Path::builder();
        b.move_to(Offset::new(3., 3.)).line_to(Offset::new(3., 3.));
        let degenerate = b.build();
        assert!(
            tessellate_path(&degenerate, FillRule::NonZero, None)
                .is_none_or(|m| m.indices.is_empty())
        );
    }
    #[test]
    fn lowering_preserves_order_and_transforms() {
        let mut list = DisplayList::new();
        list.push(PaintCommand::Rect {
            rect: Rect::from_origin_size(Offset::ZERO, Size::new(1., 1.)),
            color: Color::WHITE,
        });
        list.push(PaintCommand::PushTransform {
            transform: Transform::translation(Offset::new(2., 3.)),
        });
        list.push(PaintCommand::Rect {
            rect: Rect::from_origin_size(Offset::ZERO, Size::new(1., 1.)),
            color: Color::BLACK,
        });
        let plan = BatchPlan::lower(&list);
        assert_eq!(plan.rectangle_count(), 2);
        assert_eq!(
            plan.batches()[0].instances()[1].rect.origin,
            Offset::new(2., 3.)
        );
    }

    #[test]
    fn affine_lowering_preserves_rotated_bounds_and_path_matrix() {
        let mut list = DisplayList::new();
        list.push(PaintCommand::PushTransform {
            transform: Transform::rotation(std::f32::consts::FRAC_PI_2),
        });
        list.push(PaintCommand::Rect {
            rect: Rect::from_origin_size(Offset::ZERO, Size::new(10., 20.)),
            color: Color::WHITE,
        });
        let plan = BatchPlan::lower(&list);
        let bounds = plan.batches()[0].instances()[0].rect;
        assert!((bounds.size.width - 20.).abs() < 0.0001);
        assert!((bounds.size.height - 10.).abs() < 0.0001);

        let (linear, translation) = physical_affine(
            Transform::translation(Offset::new(4., 5.)).then(Transform::scale_non_uniform(2., 3.)),
            2.,
        );
        assert_eq!(linear, [4., 0., 0., 6.]);
        assert_eq!(translation[..2], [8., 10.]);
    }
    #[test]
    fn ndc_conversion_uses_scale_factor() {
        let i = logical_instance(
            RectangleInstance {
                rect: Rect::from_origin_size(Offset::new(10., 10.), Size::new(20., 10.)),
                color: Color::WHITE,
            },
            200.,
            100.,
            2.,
        );
        assert_eq!(i.rect, [-0.8, 0.6, 0.4, -0.4]);
    }
    #[test]
    fn image_uvs_preserve_top_to_bottom_orientation_and_source_regions() {
        // Unique RGBA corners model red/green/blue/yellow decoded rows; the
        // UV mapping must leave top-left at (0, 0), not flip it vertically.
        let image = ImageHandle::from_rgba8(
            2,
            2,
            [
                255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 128,
            ],
        )
        .unwrap();
        let destination = Rect::from_origin_size(Offset::ZERO, Size::new(20., 20.));
        let whole = image_instance(
            destination,
            Rect::from_origin_size(Offset::ZERO, Size::new(2., 2.)),
            &image,
            100.,
            100.,
            1.,
            Transform::IDENTITY,
        );
        assert_eq!(whole.uv, [0., 0., 1., 1.]);
        let left = image_instance(
            destination,
            Rect::from_origin_size(Offset::ZERO, Size::new(1., 2.)),
            &image,
            100.,
            100.,
            1.,
            Transform::IDENTITY,
        );
        let right = image_instance(
            destination,
            Rect::from_origin_size(Offset::new(1., 0.), Size::new(1., 2.)),
            &image,
            100.,
            100.,
            1.,
            Transform::IDENTITY,
        );
        let single = image_instance(
            destination,
            Rect::from_origin_size(Offset::new(1., 1.), Size::new(1., 1.)),
            &image,
            100.,
            100.,
            1.,
            Transform::IDENTITY,
        );
        assert_eq!(left.uv, [0., 0., 0.5, 1.]);
        assert_eq!(right.uv, [0.5, 0., 1., 1.]);
        assert_eq!(single.uv, [0.5, 0.5, 1., 1.]);
    }
    #[test]
    fn image_sampling_has_only_retained_renderer_neutral_modes() {
        assert_ne!(ImageSampling::Linear, ImageSampling::Nearest);
        assert_eq!(ImageSampling::default(), ImageSampling::Linear);
    }
    #[test]
    fn atlas_allocates_new_pages_without_reusing_entries() {
        let mut atlas = GlyphAtlas::new();
        let first = atlas.allocate(1000, 700, 0, 0).unwrap();
        let second = atlas.allocate(1000, 700, 0, 0).unwrap();
        assert_ne!(first.page, second.page);
        assert_eq!(first.atlas_class, GlyphAtlasClass::Oversize);
        assert_eq!(second.atlas_class, GlyphAtlasClass::Oversize);
        // One retained normal UI page plus one dedicated page per giant glyph.
        assert_eq!(atlas.counters().glyph_atlas_pages, 3);
    }
    #[test]
    fn atlas_uvs_use_the_allocated_region() {
        let entry = AtlasEntry {
            page: 3,
            x: 11,
            y: 20,
            width: 10,
            height: 4,
            atlas_class: GlyphAtlasClass::Normal,
            bearing_x: -2,
            bearing_y: 3,
        };
        assert_eq!(
            entry.uv_rect(),
            [11. / 1024., 20. / 1024., 21. / 1024., 24. / 1024.]
        );
        assert_eq!(entry.allocation_rect(), [10, 19, 12, 6]);
    }
    #[test]
    fn physical_raster_requests_cover_supported_dpi_scales_once() {
        for (scale, physical) in [
            (1.0, 16),
            (1.25, 20),
            (1.5, 24),
            (1.75, 28),
            (2.0, 32),
            (2.5, 40),
            (3.0, 48),
        ] {
            let request = GlyphRasterRequest::new(16., scale);
            assert_eq!(request.physical_size, physical);
            assert_eq!(request.logical_font_size, 16.);
            assert_eq!(request.scale_factor, scale as f32);
        }
    }
    #[test]
    fn glyph_atlas_uses_linear_coverage_filtering() {
        assert_eq!(GLYPH_ATLAS_FILTER, wgpu::FilterMode::Linear);
        assert_eq!(GLYPH_ATLAS_PADDING, 1);
    }
    #[test]
    fn atlas_padding_keeps_odd_sized_content_inside_allocation() {
        let mut atlas = GlyphAtlas::new();
        let entry = atlas.allocate(7, 9, -1, 2).expect("odd glyph fits");
        assert_eq!(entry.allocation_rect(), [0, 0, 9, 11]);
        assert_eq!(entry.x, GLYPH_ATLAS_PADDING);
        assert_eq!(entry.y, GLYPH_ATLAS_PADDING);
        assert!(entry.x + entry.width + GLYPH_ATLAS_PADDING <= ATLAS_PAGE_SIZE);
        assert!(entry.y + entry.height + GLYPH_ATLAS_PADDING <= ATLAS_PAGE_SIZE);
    }
    #[test]
    fn glyph_quad_uses_baseline_and_bearings() {
        let entry = AtlasEntry {
            page: 0,
            x: 1,
            y: 1,
            width: 8,
            height: 10,
            atlas_class: GlyphAtlasClass::Normal,
            bearing_x: -2,
            bearing_y: -3,
        };
        let run = GlyphRun {
            font: incular_assets::FontHandle::new(FontId(1), Vec::new()),
            font_size: 12.,
            origin: Offset::new(20., 30.),
            glyphs: Vec::new().into(),
        };
        let instance = glyph_instance(
            &run,
            Offset::new(1., 2.),
            entry,
            Color::WHITE,
            GlyphSurface {
                transform: Transform::IDENTITY,
                width: 100.,
                height: 100.,
                scale: 1.,
            },
        );
        assert_eq!(instance.rect, [19., 21., 8., 10.]);
        assert_eq!(instance.affine, [1., 0., 0., 1.]);
    }
    #[test]
    fn clips_intersect_in_logical_coordinates() {
        let a = Rect::from_origin_size(Offset::new(0., 0.), Size::new(10., 10.));
        let b = Rect::from_origin_size(Offset::new(5., 4.), Size::new(10., 10.));
        assert_eq!(
            intersect_rect(a, b),
            Some(Rect::from_origin_size(
                Offset::new(5., 4.),
                Size::new(5., 6.)
            ))
        );
    }
    #[test]
    fn destination_scope_uses_tight_scene_bounds_and_surface_clipping() {
        let mut list = DisplayList::new();
        list.push(PaintCommand::Rect {
            rect: Rect::from_origin_size(Offset::new(20., 30.), Size::new(40., 50.)),
            color: Color::WHITE,
        });
        let (origin, width, height) = destination_composition_scope(&list, 1., 800, 600);
        assert_eq!(origin, Offset::new(20., 30.));
        assert_eq!((width, height), (40, 50));

        let mut clipped = DisplayList::new();
        clipped.push(PaintCommand::Rect {
            rect: Rect::from_origin_size(Offset::new(-20., -10.), Size::new(40., 40.)),
            color: Color::WHITE,
        });
        let (origin, width, height) = destination_composition_scope(&clipped, 1., 800, 600);
        assert_eq!(origin, Offset::ZERO);
        assert_eq!((width, height), (20, 30));
    }
    #[test]
    fn premultiplied_shadow_colorization_matches_source_over_inputs() {
        let color = Color::rgba(220, 40, 80, 128);
        let sample = incular_painting::premultiplied_shadow_sample(color, 0.25);
        let [red, green, blue, alpha] = color.to_linear_rgba();
        let expected_alpha = alpha * 0.25;
        assert!((sample[3] - expected_alpha).abs() < 1e-6);
        assert!((sample[0] - red * expected_alpha).abs() < 1e-6);
        assert!((sample[1] - green * expected_alpha).abs() < 1e-6);
        assert!((sample[2] - blue * expected_alpha).abs() < 1e-6);
    }
    #[test]
    fn premultiplied_blur_edge_preserves_color_alpha_ratio() {
        let weights = gaussian_kernel_weights(1.5);
        let center = weights.len() / 2;
        let mut output = [0.; 4];
        // A transparent black texel next to an opaque red texel. Convolving
        // premultiplied values must keep the red channel equal to alpha; an
        // unpremultiply/re-premultiply mistake would introduce a dark fringe.
        for (index, weight) in weights.iter().enumerate() {
            let sample = if index >= center {
                [1., 0., 0., 1.]
            } else {
                [0.; 4]
            };
            for channel in 0..4 {
                output[channel] += sample[channel] * weight;
            }
        }
        assert!(output[3] > 0. && output[3] < 1.);
        assert!((output[0] - output[3]).abs() < 1e-6);
        assert_eq!(output[1], 0.);
        assert_eq!(output[2], 0.);
    }
    #[test]
    fn large_sigma_selects_bounded_multiscale_kernel_cost() {
        assert_eq!(choose_blur_downsample_factor(0.), 1);
        assert_eq!(choose_blur_downsample_factor(16.), 1);
        assert_eq!(choose_blur_downsample_factor(17.), 2);
        assert_eq!(choose_blur_downsample_factor(100.), 8);
        for sigma in [1., 16., 100., 1000.] {
            let factor = choose_blur_downsample_factor(sigma);
            let kernel = gaussian_kernel_weights(sigma / factor as f32);
            assert!(kernel.len() <= MAX_BLUR_RADIUS * 2 + 1);
        }
    }
    #[test]
    fn blur_kernel_key_is_quantized_without_exceeding_cutoff() {
        let first = quantize_blur_sigma(4.001);
        let second = quantize_blur_sigma(4.006);
        assert_eq!(first, second);
        assert!(gaussian_kernel_weights(first).len() <= MAX_BLUR_RADIUS * 2 + 1);
    }
    #[test]
    fn effect_end_finds_nested_blur_and_shadow_boundaries() {
        let mut tree = incular_painting::LayerTree::new();
        let layer = tree.create_blur(GaussianBlur::uniform(1.));
        let mut list = DisplayList::new();
        list.push(PaintCommand::PushBlur {
            layer,
            blur: GaussianBlur::uniform(4.),
            generation: 1,
            bounds: Rect::from_origin_size(Offset::ZERO, Size::new(4., 4.)),
        });
        list.push(PaintCommand::PushDropShadow {
            layer,
            shadow: DropShadowEffect::new(Offset::ZERO, 2., Color::BLACK),
            generation: 1,
            bounds: Rect::from_origin_size(Offset::ZERO, Size::new(4., 4.)),
        });
        list.push(PaintCommand::PopEffect);
        list.push(PaintCommand::PopEffect);
        assert_eq!(find_effect_end(list.commands(), 0).unwrap(), 3);
    }
    #[test]
    fn raster_cache_reuses_color_independent_glyphs_but_not_dpi_size() {
        let mut text = TextEngine::new();
        let layout = text.layout("Hello", &TextStyle::default(), None, TextAlign::Start);
        let run = &layout.lines[0].runs[0];
        let glyph = run.glyphs[0].id;
        let mut atlas = GlyphAtlas::new();
        let first = atlas.lookup_or_rasterize(run, glyph, 1.0).unwrap().entry;
        // Glyph color is deliberately not an atlas key: the text pipeline puts
        // it in each instance, so a color-only repaint is a cache hit.
        let same_color_changed = atlas.lookup_or_rasterize(run, glyph, 1.0).unwrap().entry;
        let higher_dpi = atlas.lookup_or_rasterize(run, glyph, 2.0).unwrap().entry;
        let one_x_request = GlyphRasterRequest::new(run.font_size, 1.0);
        let two_x_request = GlyphRasterRequest::new(run.font_size, 2.0);
        assert_eq!(first, same_color_changed);
        assert_ne!(
            GlyphCacheKey {
                font: run.font.id(),
                glyph,
                physical_size: one_x_request.physical_size,
            },
            GlyphCacheKey {
                font: run.font.id(),
                glyph,
                physical_size: two_x_request.physical_size,
            }
        );
        assert!(
            atlas
                .entry(GlyphCacheKey {
                    font: run.font.id(),
                    glyph,
                    physical_size: two_x_request.physical_size,
                })
                .is_some()
        );
        assert!(higher_dpi.width > 0);
        assert_eq!(atlas.counters().glyph_cache_hits, 1);
    }
    #[test]
    fn dpi_change_creates_one_new_variant_then_warms() {
        let mut text = TextEngine::new();
        let layout = text.layout("H", &TextStyle::default(), None, TextAlign::Start);
        let run = &layout.lines[0].runs[0];
        let glyph = run.glyphs[0].id;
        let mut atlas = GlyphAtlas::new();
        let _ = atlas.lookup_or_rasterize(run, glyph, 1.0).unwrap();
        let one_x = atlas.counters();
        let _ = atlas.lookup_or_rasterize(run, glyph, 2.0).unwrap();
        let two_x = atlas.counters();
        let _ = atlas.lookup_or_rasterize(run, glyph, 2.0).unwrap();
        let warm_two_x = atlas.counters();
        assert_eq!(two_x.glyphs_rasterized - one_x.glyphs_rasterized, 1);
        assert_eq!(two_x.glyph_atlas_uploads - one_x.glyph_atlas_uploads, 1);
        assert_eq!(warm_two_x.glyphs_rasterized, two_x.glyphs_rasterized);
        assert_eq!(warm_two_x.glyph_atlas_uploads, two_x.glyph_atlas_uploads);
    }
    #[test]
    fn glyph_debug_reports_logical_and_physical_units() {
        let mut text = TextEngine::new();
        let layout = text.layout("H", &TextStyle::default(), None, TextAlign::Start);
        let run = &layout.lines[0].runs[0];
        let glyph = run.glyphs[0].id;
        let mut atlas = GlyphAtlas::new();
        let _ = atlas.lookup_or_rasterize(run, glyph, 1.5).unwrap();
        let info = atlas
            .debug_glyph(run, glyph, 1.5)
            .expect("cached diagnostic");
        assert_eq!(info.logical_font_size, run.font_size);
        assert_eq!(
            info.requested_physical_size,
            (run.font_size * 1.5).round() as u16
        );
        assert_eq!(
            info.allocation_rect[2],
            info.bitmap_size[0] + 2 * GLYPH_ATLAS_PADDING
        );
        assert_eq!(
            info.allocation_rect[3],
            info.bitmap_size[1] + 2 * GLYPH_ATLAS_PADDING
        );
        assert_eq!(
            info.bitmap_bytes,
            usize::from(info.bitmap_size[0]) * usize::from(info.bitmap_size[1])
        );
    }
    #[test]
    fn raster_size_class_uses_physical_ppem() {
        assert_eq!(
            size_class(GlyphRasterRequest::new(6., 1.).physical_size),
            GlyphSizeClass::Micro
        );
        assert_eq!(
            size_class(GlyphRasterRequest::new(6., 2.).physical_size),
            GlyphSizeClass::Small
        );
        assert_eq!(
            size_class(GlyphRasterRequest::new(6., 3.).physical_size),
            GlyphSizeClass::Normal
        );
        assert_eq!(
            size_class(GlyphRasterRequest::new(48., 2.).physical_size),
            GlyphSizeClass::Large
        );
        assert_eq!(
            size_class(GlyphRasterRequest::new(256., 2.).physical_size),
            GlyphSizeClass::Huge
        );
    }
    #[test]
    fn micro_normal_and_huge_masks_are_safe_and_warm() {
        let mut text = TextEngine::new();
        let mut atlas = GlyphAtlas::new();
        for size in [
            4., 5., 6., 7., 8., 9., 10., 11., 12., 14., 16., 18., 20., 24., 32., 48., 64., 96.,
            128., 192., 256., 384., 512., 768., 1024.,
        ] {
            let layout = text.layout(
                "H",
                &TextStyle {
                    size,
                    ..TextStyle::default()
                },
                None,
                TextAlign::Start,
            );
            let run = &layout.lines[0].runs[0];
            let glyph = run.glyphs[0].id;
            let first = atlas
                .lookup_or_rasterize(run, glyph, 1.)
                .expect("supported size");
            assert!(first.entry.width > 0 && first.entry.height > 0);
            assert!(first.entry.x + first.entry.width + GLYPH_ATLAS_PADDING <= ATLAS_PAGE_SIZE);
            assert!(first.entry.y + first.entry.height + GLYPH_ATLAS_PADDING <= ATLAS_PAGE_SIZE);
            assert!(atlas.lookup_or_rasterize(run, glyph, 1.).is_some());
        }
        assert!(atlas.counters().micro_glyph_rasters >= 4);
        assert!(atlas.counters().normal_glyph_rasters > 0);
        assert!(atlas.counters().huge_glyph_rasters > 0);
    }
    #[test]
    fn oversize_pages_are_separate_from_normal_ui_atlas() {
        let mut atlas = GlyphAtlas::new();
        let ui = atlas.allocate(20, 20, 0, 0).expect("ui glyph");
        let before = atlas.memory();
        let huge = atlas.allocate(700, 700, 0, 0).expect("oversize glyph");
        let memory = atlas.memory();
        assert_eq!(ui.atlas_class, GlyphAtlasClass::Normal);
        assert_eq!(huge.atlas_class, GlyphAtlasClass::Oversize);
        assert_ne!(ui.page, huge.page);
        assert_eq!(memory.normal_pages, before.normal_pages);
        assert_eq!(memory.normal_allocated_area, before.normal_allocated_area);
        assert_eq!(memory.oversize_pages, 1);
        assert_eq!(memory.oversize_bytes, usize::from(ATLAS_PAGE_SIZE).pow(2));
    }
    #[test]
    fn unsupported_gigantic_requests_are_rejected_without_rasterizing() {
        let request = GlyphRasterRequest::new(1_000_000_000., 1.);
        assert!(!request.supported);
        let mut text = TextEngine::new();
        let layout = text.layout(
            "H",
            &TextStyle {
                size: 1_000_000_000.,
                ..TextStyle::default()
            },
            None,
            TextAlign::Start,
        );
        let run = &layout.lines[0].runs[0];
        let mut atlas = GlyphAtlas::new();
        assert!(
            atlas
                .lookup_or_rasterize(run, run.glyphs[0].id, 1.)
                .is_none()
        );
        assert_eq!(atlas.counters().glyphs_rasterized, 0);
    }
    #[test]
    fn fractional_gpu_placement_reuses_one_fontdue_mask() {
        let mut text = TextEngine::new();
        let layout = text.layout("H", &TextStyle::default(), None, TextAlign::Start);
        let run = &layout.lines[0].runs[0];
        let glyph = run.glyphs[0].id;
        let mut atlas = GlyphAtlas::new();
        let first = atlas.lookup_or_rasterize(run, glyph, 1.).unwrap().entry;
        let cold = atlas.counters();
        for _x in [0., 0.25, 0.5, 0.75] {
            assert_eq!(
                atlas.lookup_or_rasterize(run, glyph, 1.).unwrap().entry,
                first
            );
        }
        let warm = atlas.counters();
        assert_eq!(warm.glyphs_rasterized, cold.glyphs_rasterized);
        assert_eq!(warm.glyph_atlas_uploads, cold.glyph_atlas_uploads);
    }
    #[test]
    fn glyph_cache_keeps_font_ids_separate() {
        let mut text = TextEngine::new();
        let layout = text.layout("H", &TextStyle::default(), None, TextAlign::Start);
        let run = &layout.lines[0].runs[0];
        let glyph = run.glyphs[0].id;
        let mut alternate = (**run).clone();
        alternate.font = incular_assets::FontHandle::with_face_index(
            FontId(run.font.id().0.wrapping_add(1)),
            run.font.bytes().clone(),
            run.font.face_index(),
        );

        let mut atlas = GlyphAtlas::new();
        let first = atlas.lookup_or_rasterize(run, glyph, 1.).unwrap().entry;
        let before_alternate = atlas.counters();
        let second = atlas
            .lookup_or_rasterize(&alternate, glyph, 1.)
            .unwrap()
            .entry;
        let after_alternate = atlas.counters();

        let physical_size = GlyphRasterRequest::new(run.font_size, 1.).physical_size;
        assert!(
            atlas
                .entry(GlyphCacheKey {
                    font: run.font.id(),
                    glyph,
                    physical_size,
                })
                .is_some()
        );
        assert!(
            atlas
                .entry(GlyphCacheKey {
                    font: alternate.font.id(),
                    glyph,
                    physical_size,
                })
                .is_some()
        );
        assert_ne!(first, second);
        assert_eq!(
            after_alternate.glyphs_rasterized - before_alternate.glyphs_rasterized,
            1
        );
        assert_eq!(
            after_alternate.font_parser_cache_misses - before_alternate.font_parser_cache_misses,
            1
        );
    }
    #[test]
    fn counter_text_warms_the_atlas_incrementally() {
        fn rasterize(atlas: &mut GlyphAtlas, text: &mut TextEngine, value: &str) {
            for label in ["Incular Counter", value, "Increment"] {
                let layout = text.layout(label, &TextStyle::default(), None, TextAlign::Start);
                for line in layout.lines.iter() {
                    for run in line.runs.iter() {
                        for glyph in run.glyphs.iter() {
                            let _ = atlas.lookup_or_rasterize(run, glyph.id, 1.0);
                        }
                    }
                }
            }
        }
        let mut atlas = GlyphAtlas::new();
        let mut text = TextEngine::new();
        rasterize(&mut atlas, &mut text, "Count: 0");
        let first = atlas.counters();
        rasterize(&mut atlas, &mut text, "Count: 1");
        let first_click = atlas.counters();
        rasterize(&mut atlas, &mut text, "Count: 1");
        let warm = atlas.counters();
        assert!(first.glyphs_rasterized > 0 && first.glyph_atlas_uploads > 0);
        assert_eq!(first_click.glyph_cache_misses - first.glyph_cache_misses, 1);
        assert_eq!(
            first_click.glyph_atlas_uploads - first.glyph_atlas_uploads,
            1
        );
        assert_eq!(warm.glyph_cache_misses, first_click.glyph_cache_misses);
        assert_eq!(warm.glyphs_rasterized, first_click.glyphs_rasterized);
        assert_eq!(warm.glyph_atlas_uploads, first_click.glyph_atlas_uploads);
        eprintln!(
            "counter atlas diagnostics: first={first:?}, click={first_click:?}, warm={warm:?}"
        );
    }
    fn premultiplied_over(dst: [f32; 4], src: [f32; 4]) -> [f32; 4] {
        let keep = 1. - src[3];
        [
            src[0] + dst[0] * keep,
            src[1] + dst[1] * keep,
            src[2] + dst[2] * keep,
            src[3] + dst[3] * keep,
        ]
    }
    #[test]
    fn isolated_group_alpha_is_applied_once_to_overlap() {
        let red = [1., 0., 0., 1.];
        let blue = [0., 0., 1., 1.];
        let group_overlap = premultiplied_over(red, blue);
        let isolated = apply_group_alpha(group_overlap, 0.5);
        assert_eq!(isolated, [0., 0., 0.5, 0.5]);

        // Applying 0.5 independently to each opaque child produces a
        // different overlap alpha (0.75), which is the bug this compositor
        // boundary prevents.
        let descendant_alpha =
            premultiplied_over(apply_group_alpha(red, 0.5), apply_group_alpha(blue, 0.5));
        assert!((descendant_alpha[3] - 0.75).abs() < f32::EPSILON);
        assert_ne!(isolated, descendant_alpha);
    }
    #[test]
    fn premultiplied_partial_child_and_nested_opacity_are_stable() {
        let child = [0.5, 0., 0., 0.5];
        assert_eq!(apply_group_alpha(child, 0.5), [0.25, 0., 0., 0.25]);
        let nested = apply_group_alpha(apply_group_alpha([0.2, 0.4, 0.6, 1.], 0.5), 0.5);
        assert_eq!(nested, [0.05, 0.1, 0.15, 0.25]);
    }

    #[test]
    fn blend_codes_are_unique_and_reference_math_is_finite() {
        let modes = [
            BlendMode::SrcOver,
            BlendMode::Src,
            BlendMode::DstOver,
            BlendMode::SrcIn,
            BlendMode::DstIn,
            BlendMode::SrcOut,
            BlendMode::DstOut,
            BlendMode::SrcAtop,
            BlendMode::DstAtop,
            BlendMode::Xor,
            BlendMode::Plus,
            BlendMode::Multiply,
            BlendMode::Screen,
            BlendMode::Overlay,
            BlendMode::Darken,
            BlendMode::Lighten,
            BlendMode::ColorDodge,
            BlendMode::ColorBurn,
            BlendMode::HardLight,
            BlendMode::SoftLight,
            BlendMode::Difference,
            BlendMode::Exclusion,
        ];
        let mut codes = modes.iter().map(|mode| mode.code()).collect::<Vec<_>>();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), modes.len());
        for mode in modes {
            for (source, destination) in [
                ([0., 0., 0., 0.], [0., 0., 0., 0.]),
                ([0.2, 0.1, 0.05, 0.5], [0.3, 0.2, 0.1, 0.5]),
                ([0.1, 0.3, 0.2, 1.], [0.4, 0.1, 0.7, 1.]),
            ] {
                let output = incular_painting::blend_premultiplied(mode, source, destination);
                assert!(output.iter().all(|value| value.is_finite()));
                assert!(output[..3].iter().all(|value| *value <= output[3] + 1e-6));
            }
        }
    }

    #[test]
    fn destination_blend_classification_keeps_porter_duff_on_fixed_path() {
        assert!(!BlendMode::SrcOver.requires_destination_read());
        assert!(!BlendMode::Src.requires_destination_read());
        assert!(!BlendMode::Plus.requires_destination_read());
        assert!(BlendMode::Multiply.requires_destination_read());
        assert!(BlendMode::Overlay.requires_destination_read());
        assert!(BlendMode::Exclusion.requires_destination_read());
    }

    #[test]
    fn shared_resource_registry_reuses_image_and_matching_glyph_identity() {
        let mut resources = SharedGpuResourceRegistry::default();
        let image = ImageId(17);
        let image_a = resources.image_identity(image);
        let image_b = resources.image_identity(image);
        assert_eq!(image_a, image_b);

        let one_x = GlyphCacheKey {
            font: FontId(4),
            glyph: 73,
            physical_size: 16,
        };
        let same_one_x = resources.glyph_identity(one_x);
        assert_eq!(resources.glyph_identity(one_x), same_one_x);
        // DPI belongs to glyph identity, so 2x legitimately creates a second
        // mask while remaining in the same context-wide atlas resource set.
        assert_ne!(
            resources.glyph_identity(GlyphCacheKey {
                physical_size: 32,
                ..one_x
            }),
            same_one_x
        );
        assert_eq!(resources.image_count(), 1);
        assert_eq!(resources.glyph_count(), 2);
    }

    #[test]
    fn per_window_presentation_resize_loss_and_zero_size_are_isolated() {
        let mut window_a = WindowGpuPresentation::new(PhysicalSize::new(640, 480));
        let mut window_b = WindowGpuPresentation::new(PhysicalSize::new(1920, 1080));
        let b_before = window_b;

        assert!(window_a.resize(PhysicalSize::new(800, 600)));
        let after_resize = window_a.surface_generation;
        window_a.surface_lost();
        assert!(window_a.surface_generation > after_resize);
        assert_eq!(window_b, b_before);

        assert!(!window_a.resize(PhysicalSize::new(0, 600)));
        assert!(!window_a.configured);
        assert_eq!(window_a.physical_size, PhysicalSize::new(0, 600));
        // A zero-sized/minimized A must not configure, resize, or otherwise
        // invalidate B's independent presentation state.
        assert_eq!(window_b, b_before);
        window_b.record_present();
        assert_eq!(window_b.presented_frames, 1);
        assert_eq!(window_a.presented_frames, 0);
    }
}
