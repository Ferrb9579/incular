//! Retained `wgpu` renderer for renderer-neutral Incular display lists.
//!
//! Shaping remains in `incular-text`. This crate rasterizes its glyph IDs at
//! physical DPI, retains their coverage masks in `R8Unorm` atlas textures, and
//! draws atlas-backed instanced quads in display-list order.
use bytemuck::{Pod, Zeroable};
use fontdue::{Font, FontSettings};
use incular_assets::{FontId, ImageHandle, ImageId};
use incular_core::{Color, Offset, Rect, Size};
use incular_painting::{
    Brush, DisplayList, FillRule, GlyphRun, GradientId, ImageSampling, LineCap, LineJoin,
    PaintCommand, Path, PathId, RRect, Stroke, sample_gradient_stops,
};
use incular_platform::{PhysicalSize, RawWindowHandles};
use lyon_tessellation::{
    FillOptions, FillRule as LyonFillRule, FillTessellator, StrokeOptions, StrokeTessellator,
    VertexBuffers, geometry_builder::simple_builder, math::point, path::Path as LyonPath,
};
use std::collections::{HashMap, hash_map::Entry};
use wgpu::util::DeviceExt;

const ATLAS_PAGE_SIZE: u16 = 1024;
const ATLAS_PADDING: u16 = 1;
const IMAGE_CACHE_MAX_UNUSED_FRAMES: u64 = 600;
const PATH_CACHE_MAX_UNUSED_FRAMES: u64 = 600;
const GRADIENT_CACHE_MAX_UNUSED_FRAMES: u64 = 600;
/// Each normalized gradient is resampled into this compact one-dimensional
/// lookup texture. This deterministic representation supports any stop count:
/// every stop participates in the premultiplied-linear samples.
const GRADIENT_LUT_SAMPLES: u32 = 256;
const MAX_STENCIL_CLIP_DEPTH: u8 = u8::MAX;

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
    for v in path.verbs() {
        match *v {
            incular_painting::PathVerb::MoveTo(p) => {
                if open {
                    b.end(false);
                }
                b.begin(point(p.x, p.y));
                open = true;
            }
            incular_painting::PathVerb::LineTo(p) => {
                b.line_to(point(p.x, p.y));
            }
            incular_painting::PathVerb::QuadraticTo(c, p) => {
                b.quadratic_bezier_to(point(c.x, c.y), point(p.x, p.y));
            }
            incular_painting::PathVerb::CubicTo(a, c, p) => {
                b.cubic_bezier_to(point(a.x, a.y), point(c.x, c.y), point(p.x, p.y));
            }
            incular_painting::PathVerb::Close => {
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
/// Translates all public Incular verbs into Lyon events without flattening.
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
        let mut transforms = vec![Offset::ZERO];
        for command in list.commands() {
            match command {
                PaintCommand::Rect { rect, color } => current.instances.push(RectangleInstance {
                    rect: translated_rect(*rect, *transforms.last().expect("transform stack")),
                    color: *color,
                }),
                PaintCommand::PushTransform { transform } => transforms
                    .push(*transforms.last().expect("transform stack") + transform.translation),
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
                | PaintCommand::PushClipPath { .. }
                | PaintCommand::PopClip
                | PaintCommand::GlyphRun { .. } => {
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
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GlyphCacheKey {
    pub font: FontId,
    pub glyph: u16,
    pub physical_size: u16,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AtlasEntry {
    pub page: u16,
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    /// Physical-pixel raster bounds relative to the shaped baseline origin.
    pub bearing_x: i16,
    pub bearing_y: i16,
}
impl AtlasEntry {
    #[must_use]
    pub fn uv_rect(self) -> [f32; 4] {
        let page = f32::from(ATLAS_PAGE_SIZE);
        [
            (f32::from(self.x) + 0.5) / page,
            (f32::from(self.y) + 0.5) / page,
            (f32::from(self.x + self.width) - 0.5) / page,
            (f32::from(self.y + self.height) - 0.5) / page,
        ]
    }
}
#[derive(Clone, Debug)]
pub struct RasterizedGlyph {
    pub entry: AtlasEntry,
    pub bitmap: Option<Vec<u8>>,
}
#[derive(Default)]
struct AtlasPage {
    next_x: u16,
    next_y: u16,
    row_height: u16,
}
/// CPU metadata for retained atlas pages. `WgpuRenderer` maps each page index
/// lazily to one persistent `R8Unorm` texture; entries never move or compact.
pub struct GlyphAtlas {
    pages: Vec<AtlasPage>,
    entries: HashMap<GlyphCacheKey, AtlasEntry>,
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
            pages: vec![AtlasPage::default()],
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
    pub fn lookup_or_rasterize(
        &mut self,
        run: &GlyphRun,
        glyph: u16,
        scale: f64,
    ) -> Option<RasterizedGlyph> {
        let key = GlyphCacheKey {
            font: run.font.id(),
            glyph,
            physical_size: (run.font_size * normalized_scale(scale))
                .round()
                .clamp(1., f32::from(u16::MAX)) as u16,
        };
        if let Some(entry) = self.entries.get(&key).copied() {
            self.counters.glyph_cache_hits += 1;
            return Some(RasterizedGlyph {
                entry,
                bitmap: None,
            });
        }
        self.counters.glyph_cache_misses += 1;
        if let Entry::Vacant(entry) = self.fonts.entry(key.font) {
            entry
                .insert(Font::from_bytes(run.font.bytes().as_ref(), FontSettings::default()).ok()?);
        }
        let (metrics, bitmap) = self
            .fonts
            .get(&key.font)
            .expect("cached font")
            .rasterize_indexed(key.glyph, f32::from(key.physical_size));
        self.counters.glyphs_rasterized += 1;
        let bearing_x = clamp_i16(metrics.xmin);
        let bearing_y = clamp_i16(metrics.ymin);
        // Spaces deliberately have a cache entry but no texture write or quad.
        if metrics.width == 0 || metrics.height == 0 {
            let entry = AtlasEntry {
                page: 0,
                x: 0,
                y: 0,
                width: 0,
                height: 0,
                bearing_x,
                bearing_y,
            };
            self.entries.insert(key, entry);
            return Some(RasterizedGlyph {
                entry,
                bitmap: None,
            });
        }
        let entry = self.allocate(
            metrics.width.min(usize::from(u16::MAX)) as u16,
            metrics.height.min(usize::from(u16::MAX)) as u16,
            bearing_x,
            bearing_y,
        )?;
        self.entries.insert(key, entry);
        self.counters.glyph_atlas_uploads += 1;
        Some(RasterizedGlyph {
            entry,
            bitmap: Some(bitmap),
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
        let page_index = self.pages.len() - 1;
        let page = self.pages.last_mut().expect("atlas page");
        if page.next_x + stored_width > ATLAS_PAGE_SIZE {
            page.next_x = 0;
            page.next_y = page.next_y.saturating_add(page.row_height);
            page.row_height = 0;
        }
        if page.next_y + stored_height > ATLAS_PAGE_SIZE {
            self.pages.push(AtlasPage::default());
            self.counters.glyph_atlas_pages += 1;
            return self.allocate(width, height, bearing_x, bearing_y);
        }
        let entry = AtlasEntry {
            page: page_index as u16,
            x: page.next_x + ATLAS_PADDING,
            y: page.next_y + ATLAS_PADDING,
            width,
            height,
            bearing_x,
            bearing_y,
        };
        page.next_x += stored_width;
        page.row_height = page.row_height.max(stored_height);
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
    StencilDepthOverflow,
    UnbalancedClipStack,
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
            Self::StencilDepthOverflow => {
                write!(f, "nested non-rectangular clip depth exceeds 255")
            }
            Self::UnbalancedClipStack => {
                write!(f, "display list contains an unbalanced clip stack")
            }
            Self::OutOfMemory => write!(f, "GPU surface ran out of memory"),
        }
    }
}
impl std::error::Error for RendererError {}

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
    uv: [f32; 4],
    color: [f32; 4],
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct GpuImageInstance {
    rect: [f32; 4],
    uv: [f32; 4],
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct GpuRRectInstance {
    rect: [f32; 4],
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
    /// Translation and scale: local mesh vertices remain logical coordinates.
    placement: [f32; 4],
    surface: [f32; 4],
    color: [f32; 4],
    gradient: [f32; 4],
    options: [f32; 4],
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
@vertex fn vs_main(@location(0) quad: vec2<f32>, @location(1) rect: vec4<f32>, @location(2) uv: vec4<f32>, @location(3) color: vec4<f32>) -> Out { var out: Out; out.position = vec4<f32>(rect.xy + quad * rect.zw, 0., 1.); out.uv = uv.xy + quad * (uv.zw - uv.xy); out.color = color; return out; }
@fragment fn fs_main(input: Out) -> @location(0) vec4<f32> { let coverage = textureSample(atlas, atlas_sampler, input.uv).r; return vec4<f32>(input.color.rgb, input.color.a * coverage); }
"#;
const IMAGE_SHADER: &str = r#"
struct Out { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> };
@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var image_sampler: sampler;
@vertex fn vs_main(@location(0) quad: vec2<f32>, @location(1) rect: vec4<f32>, @location(2) uv: vec4<f32>) -> Out { var out: Out; out.position = vec4<f32>(rect.xy + quad * rect.zw, 0., 1.); out.uv = uv.xy + quad * (uv.zw - uv.xy); return out; }
// The image texture decodes sRGB into linear sample values. Source pixels are
// straight alpha, and ALPHA_BLENDING is straight source-over blending.
@fragment fn fs_main(input: Out) -> @location(0) vec4<f32> { return textureSample(image, image_sampler, input.uv); }
"#;
const RRECT_SHADER: &str = r#"
struct Out { @builtin(position) position: vec4<f32>, @location(0) local: vec2<f32>, @location(1) radii: vec4<f32>, @location(2) a: vec4<f32>, @location(3) b: vec4<f32>, @location(4) gradient: vec4<f32>, @location(5) options: vec4<f32> };
@group(0) @binding(0) var gradient_lut: texture_2d<f32>;
@group(0) @binding(1) var gradient_sampler: sampler;
@vertex fn vs_main(@location(0) quad: vec2<f32>, @location(1) rect: vec4<f32>, @location(2) radii: vec4<f32>, @location(3) a: vec4<f32>, @location(4) b: vec4<f32>, @location(5) gradient: vec4<f32>, @location(6) options: vec4<f32>) -> Out { var o: Out; o.position=vec4(rect.xy+quad*rect.zw,0.,1.); o.local=quad*options.zw; o.radii=radii; o.a=a; o.b=b; o.gradient=gradient; o.options=options; return o; }
fn radius_at(p: vec2<f32>, size: vec2<f32>, r: vec4<f32>) -> f32 { if (p.y < size.y*.5) { if (p.x < size.x*.5) { return r.x; } return r.y; } if (p.x >= size.x*.5) { return r.z; } return r.w; }
fn rounded_distance(p: vec2<f32>, size: vec2<f32>, r: vec4<f32>) -> f32 { let q=p-size*.5; let radius=radius_at(p,size,r); let d=abs(q)-(size*.5-vec2(radius)); return length(max(d,vec2(0.)))+min(max(d.x,d.y),0.)-radius; }
fn lookup(t: f32) -> vec4<f32> { let p=textureSampleLevel(gradient_lut,gradient_sampler,vec2(clamp(t,0.,1.),.5),0.); return select(vec4(0.),vec4(p.rgb/max(p.a,.00001),p.a),p.a>0.); }
@fragment fn fs_main(i: Out) -> @location(0) vec4<f32> { let size=i.options.zw; let outer=rounded_distance(i.local,size,i.radii); var edge=1.-smoothstep(-1.,1.,outer); if(i.options.y>0.) { let width=i.options.y; let inner=rounded_distance(i.local-vec2(width), max(size-vec2(2.*width),vec2(0.)), max(i.radii-vec4(width),vec4(0.))); edge*=smoothstep(-1.,1.,inner); } var t=0.; if(i.options.x==1.) { let v=i.gradient.zw-i.gradient.xy; t=clamp(dot(i.local-i.gradient.xy,v)/max(dot(v,v),.0001),0.,1.); } else if(i.options.x==2.) { t=clamp(length(i.local-i.gradient.xy)/max(i.gradient.z,.0001),0.,1.); } let color=select(i.a,lookup(t),i.options.x>0.); return vec4(color.rgb,color.a*edge); }
"#;
const PATH_SHADER: &str = r#"
struct Out { @builtin(position) position: vec4<f32>, @location(0) local: vec2<f32>, @location(1) color: vec4<f32>, @location(2) gradient: vec4<f32>, @location(3) options: vec4<f32> };
@group(0) @binding(0) var gradient_lut: texture_2d<f32>;
@group(0) @binding(1) var gradient_sampler: sampler;
@vertex fn vs_main(@location(0) local: vec2<f32>, @location(1) placement: vec4<f32>, @location(2) surface: vec4<f32>, @location(3) color: vec4<f32>, @location(4) gradient: vec4<f32>, @location(5) options: vec4<f32>) -> Out {
  var out: Out;
  let physical = local * placement.z + placement.xy;
  out.position = vec4<f32>(physical.x / surface.x * 2. - 1., 1. - physical.y / surface.y * 2., 0., 1.);
  out.color = color;
  out.local = local;
  out.gradient = gradient;
  out.options = options;
  return out;
}
fn lookup(t: f32) -> vec4<f32> { let p=textureSampleLevel(gradient_lut,gradient_sampler,vec2(clamp(t,0.,1.),.5),0.); return select(vec4(0.),vec4(p.rgb/max(p.a,.00001),p.a),p.a>0.); }
@fragment fn fs_main(i: Out) -> @location(0) vec4<f32> { var t=0.; if(i.options.x==1.) { let d=i.gradient.zw-i.gradient.xy; t=clamp(dot(i.local-i.gradient.xy,d)/max(dot(d,d),.0001),0.,1.); } else if(i.options.x==2.) { t=clamp(length(i.local-i.gradient.xy)/max(i.gradient.z,.0001),0.,1.); } return select(i.color,lookup(t),i.options.x>0.); }
"#;

struct GpuAtlasPage {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
}
struct GpuImage {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    bind_groups: HashMap<ImageSampling, wgpu::BindGroup>,
    width: u32,
    height: u32,
    last_used_frame: u64,
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
#[derive(Debug)]
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
}
#[derive(Clone, Copy)]
struct GlyphSurface {
    translation: Offset,
    width: f32,
    height: f32,
    scale: f32,
}
#[derive(Clone, Copy)]
struct PathPlacement {
    translation: Offset,
    scale: f32,
}

/// Owns surface, pipelines, buffers and atlas textures. Colors and coverage
/// are straight alpha and use ordinary source-alpha blending.
pub struct WgpuRenderer {
    _instance: wgpu::Instance,
    handles: RawWindowHandles,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    rectangle_pipeline: wgpu::RenderPipeline,
    text_pipeline: wgpu::RenderPipeline,
    image_pipeline: wgpu::RenderPipeline,
    rounded_rect_pipeline: wgpu::RenderPipeline,
    path_pipeline: wgpu::RenderPipeline,
    stencil_rrect_increment_pipeline: wgpu::RenderPipeline,
    stencil_rrect_decrement_pipeline: wgpu::RenderPipeline,
    stencil_path_increment_pipeline: wgpu::RenderPipeline,
    stencil_path_decrement_pipeline: wgpu::RenderPipeline,
    stencil_texture: wgpu::Texture,
    stencil_view: wgpu::TextureView,
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
    atlas_pages: Vec<GpuAtlasPage>,
    counters: GpuCounters,
    glyph_atlas: GlyphAtlas,
}
impl WgpuRenderer {
    /// # Safety boundary
    /// `handles` must describe a window that outlives this renderer.
    pub async fn new(handles: RawWindowHandles, size: PhysicalSize) -> Result<Self, RendererError> {
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
        let config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .expect("surface config");
        surface.configure(&device, &config);
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
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
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
        Ok(Self {
            _instance: instance,
            handles,
            surface,
            device,
            queue,
            config,
            rectangle_pipeline,
            text_pipeline,
            image_pipeline,
            rounded_rect_pipeline,
            path_pipeline,
            stencil_rrect_increment_pipeline,
            stencil_rrect_decrement_pipeline,
            stencil_path_increment_pipeline,
            stencil_path_decrement_pipeline,
            stencil_texture,
            stencil_view,
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
            atlas_pages: Vec::new(),
            counters: GpuCounters {
                rectangle_pipeline_creations: 1,
                text_pipeline_creations: 1,
                image_pipeline_creations: 1,
                path_pipeline_creations: 1,
                stencil_texture_creations: 1,
                stencil_pipeline_creations: 4,
                ..GpuCounters::default()
            },
            glyph_atlas: GlyphAtlas::new(),
        })
    }
    #[must_use]
    pub fn counters(&self) -> GpuCounters {
        let mut counters = self.counters;
        let atlas = self.glyph_atlas.counters();
        counters.glyph_cache_hits = atlas.glyph_cache_hits;
        counters.glyph_cache_misses = atlas.glyph_cache_misses;
        counters.glyphs_rasterized = atlas.glyphs_rasterized;
        counters.glyphs_skipped = atlas.glyphs_skipped;
        counters.glyph_atlas_uploads = atlas.glyph_atlas_uploads;
        counters.glyph_atlas_pages = atlas.glyph_atlas_pages;
        counters
    }
    #[must_use]
    pub fn physical_size(&self) -> PhysicalSize {
        PhysicalSize::new(self.config.width, self.config.height)
    }
    pub fn resize(&mut self, size: PhysicalSize) {
        if !size.is_zero() {
            self.config.width = size.width;
            self.config.height = size.height;
            self.surface.configure(&self.device, &self.config);
            (self.stencil_texture, self.stencil_view) =
                create_stencil_attachment(&self.device, size.width, size.height);
            self.counters.stencil_texture_recreations += 1;
        }
    }
    pub fn render(
        &mut self,
        list: &DisplayList,
        scale_factor: f64,
    ) -> Result<RenderStats, RendererError> {
        if self.config.width == 0 || self.config.height == 0 {
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
        self.upload_instance_data(&batches, scale);
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
                self.surface = unsafe {
                    self._instance
                        .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                            raw_display_handle: self.handles.display,
                            raw_window_handle: self.handles.window,
                        })
                }
                .map_err(RendererError::Surface)?;
                self.surface.configure(&self.device, &self.config);
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
                    | DrawBatch::StencilPath { clip, .. } => *clip,
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
    fn upload_instance_data(&self, batches: &[DrawBatch], scale: f32) {
        let mut rectangle_offset = 0_u64;
        let mut glyph_offset = 0_u64;
        let mut image_offset = 0_u64;
        let mut rounded_offset = 0_u64;
        let mut path_offset = 0_u64;
        for batch in batches {
            match batch {
                DrawBatch::Rectangles { clip, instances }
                    if *clip != ClipState::Empty && !instances.is_empty() =>
                {
                    let gpu: Vec<_> = instances
                        .iter()
                        .map(|instance| {
                            logical_instance(
                                *instance,
                                self.config.width as f32,
                                self.config.height as f32,
                                scale,
                            )
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
                _ => {}
            }
        }
    }
    fn lower_draw_batches(
        &mut self,
        list: &DisplayList,
        scale: f32,
    ) -> Result<Vec<DrawBatch>, RendererError> {
        let mut batches = Vec::new();
        let mut transforms = vec![Offset::ZERO];
        let mut clips = vec![ClipState::Unbounded];
        let mut clip_masks: Vec<Option<ClipMask>> = vec![None];
        for command in list.commands() {
            let translation = *transforms.last().expect("transform stack");
            match command {
                PaintCommand::Rect { rect, color } => append_rectangle(
                    &mut batches,
                    *clips.last().expect("clip stack"),
                    RectangleInstance {
                        rect: translated_rect(*rect, translation),
                        color: *color,
                    },
                ),
                // Common solid rounded primitives retain painter order even on
                // renderers that do not yet select the analytic pipeline.
                PaintCommand::RRect { rrect, brush } => append_rrect(
                    &mut batches,
                    *clips.last().expect("clip stack"),
                    self.ensure_gradient(brush),
                    rrect_instance(
                        *rrect,
                        brush,
                        translation,
                        scale,
                        self.config.width as f32,
                        self.config.height as f32,
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
                            translation,
                            scale,
                            self.config.width as f32,
                            self.config.height as f32,
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
                    PathPlacement { translation, scale },
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
                    PathPlacement { translation, scale },
                ),
                PaintCommand::GlyphRun { run, color } => {
                    if *clips.last().expect("clip stack") == ClipState::Empty {
                        continue;
                    }
                    for glyph in run.glyphs.iter() {
                        let Some(raster) =
                            self.glyph_atlas
                                .lookup_or_rasterize(run, glyph.id, f64::from(scale))
                        else {
                            self.glyph_atlas.counters.glyphs_skipped += 1;
                            continue;
                        };
                        if let Some(bitmap) = raster.bitmap.as_deref() {
                            self.upload_glyph(raster.entry, bitmap);
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
                                        translation,
                                        width: self.config.width as f32,
                                        height: self.config.height as f32,
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
                        continue;
                    }
                    self.ensure_gpu_image(image)?;
                    append_image(
                        &mut batches,
                        *clips.last().expect("clip stack"),
                        image.id(),
                        *sampling,
                        image_instance(
                            translated_rect(*destination, translation),
                            *source,
                            image,
                            self.config.width as f32,
                            self.config.height as f32,
                            scale,
                        ),
                    );
                }
                PaintCommand::PushTransform { transform } => {
                    transforms.push(translation + transform.translation)
                }
                PaintCommand::PopTransform => {
                    if transforms.len() > 1 {
                        transforms.pop();
                    }
                }
                PaintCommand::PushClip { rect } => {
                    let next = ClipRect {
                        rect: translated_rect(*rect, translation),
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
                        translation,
                        scale,
                        self.config.width as f32,
                        self.config.height as f32,
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
                            translation,
                            scale,
                            self.config.width as f32,
                            self.config.height as f32,
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
        }
        if clips.len() != 1 || clip_masks.len() != 1 {
            return Err(RendererError::UnbalancedClipStack);
        }
        Ok(batches)
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
        if clip == ClipState::Empty || !path_visible(path, placement.translation, clip) {
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
                placement: [
                    placement.translation.x * placement.scale,
                    placement.translation.y * placement.scale,
                    placement.scale,
                    0.,
                ],
                surface: [self.config.width as f32, self.config.height as f32, 0., 0.],
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
            Brush::Solid(_) => unreachable!("solid has no gradient id"),
        };
        // LUT texels are premultiplied linear RGB. The shader unpremultiplies
        // before the renderer's straight-alpha source-over blend, avoiding
        // dark fringes across transparent stops.
        let pixels = gradient_lut_pixels(stops);
        let resource = create_gradient_resource(
            &self.device,
            &self.queue,
            &self.gradient_bind_group_layout,
            &self.gradient_sampler,
            &pixels,
        );
        self.gradient_cache.insert(id, resource);
        self.counters.gradient_cache_misses += 1;
        self.counters.gradient_resource_creations += 1;
        self.counters.gradient_resource_uploads += 1;
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
            debug_assert_eq!(resource.width, image.decoded().width());
            debug_assert_eq!(resource.height, image.decoded().height());
            resource.last_used_frame = self.counters.frames;
            self.counters.image_cache_hits += 1;
            return Ok(());
        }
        let decoded = image.decoded();
        let limit = self.device.limits().max_texture_dimension_2d;
        if decoded.width() > limit || decoded.height() > limit {
            return Err(RendererError::ImageTooLarge {
                width: decoded.width(),
                height: decoded.height(),
                limit,
            });
        }
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("incular retained image texture"),
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
        self.queue.write_texture(
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
        self.image_cache.insert(
            id,
            GpuImage {
                _texture: texture,
                view,
                bind_groups: HashMap::new(),
                width: decoded.width(),
                height: decoded.height(),
                last_used_frame: self.counters.frames,
            },
        );
        self.counters.image_cache_misses += 1;
        self.counters.image_texture_creations += 1;
        self.counters.image_texture_uploads += 1;
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
                let Some(view) = self.image_cache.get(image).map(|resource| &resource.view) else {
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
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &page.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: u32::from(entry.x),
                    y: u32::from(entry.y),
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            bitmap,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(u32::from(entry.width)),
                rows_per_image: Some(u32::from(entry.height)),
            },
            wgpu::Extent3d {
                width: u32::from(entry.width),
                height: u32::from(entry.height),
                depth_or_array_layers: 1,
            },
        );
    }
    fn ensure_atlas_page(&mut self, page: u16) {
        while self.atlas_pages.len() <= usize::from(page) {
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("incular glyph atlas page"),
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
            });
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
fn path_visible(path: &Path, translation: Offset, clip: ClipState) -> bool {
    let Some(bounds) = path.bounds() else {
        return false;
    };
    let world = translated_rect(bounds, translation);
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
    }
}
fn clip_path_instance(translation: Offset, scale: f32, width: f32, height: f32) -> GpuPathInstance {
    GpuPathInstance {
        placement: [translation.x * scale, translation.y * scale, scale, 0.],
        surface: [width, height, 0., 0.],
        color: Color::TRANSPARENT.to_linear_rgba(),
        gradient: [0.; 4],
        options: [0.; 4],
    }
}
fn rrect_instance(
    rrect: RRect,
    brush: &Brush,
    translation: Offset,
    scale: f32,
    width: f32,
    height: f32,
) -> GpuRRectInstance {
    let rect = translated_rect(rrect.rect, translation);
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
    };
    GpuRRectInstance {
        rect: ndc_rect(
            rect.origin.x * scale,
            rect.origin.y * scale,
            rect.size.width * scale,
            rect.size.height * scale,
            width,
            height,
        ),
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
    translation: Offset,
    scale: f32,
    width: f32,
    height: f32,
) -> GpuRRectInstance {
    let mut result = rrect_instance(
        rrect,
        &Brush::Solid(border.color),
        translation,
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
    // fontdue ymin is the bitmap's bottom relative to baseline; rustybuzz's
    // positive Y offset is upwards while Incular's logical canvas is Y-down.
    let baseline_x = (surface.translation.x + run.origin.x + offset.x) * surface.scale;
    let baseline_y = (surface.translation.y + run.origin.y - offset.y) * surface.scale;
    let x = baseline_x + f32::from(entry.bearing_x);
    let y = baseline_y - f32::from(entry.bearing_y) - f32::from(entry.height);
    GpuGlyphInstance {
        rect: ndc_rect(
            x,
            y,
            f32::from(entry.width),
            f32::from(entry.height),
            surface.width,
            surface.height,
        ),
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
) -> GpuImageInstance {
    let image_width = image.decoded().width() as f32;
    let image_height = image.decoded().height() as f32;
    GpuImageInstance {
        rect: ndc_rect(
            destination.origin.x * scale,
            destination.origin.y * scale,
            destination.size.width * scale,
            destination.size.height * scale,
            width,
            height,
        ),
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
        );
        assert_eq!(whole.uv, [0., 0., 1., 1.]);
        let left = image_instance(
            destination,
            Rect::from_origin_size(Offset::ZERO, Size::new(1., 2.)),
            &image,
            100.,
            100.,
            1.,
        );
        let right = image_instance(
            destination,
            Rect::from_origin_size(Offset::new(1., 0.), Size::new(1., 2.)),
            &image,
            100.,
            100.,
            1.,
        );
        let single = image_instance(
            destination,
            Rect::from_origin_size(Offset::new(1., 1.), Size::new(1., 1.)),
            &image,
            100.,
            100.,
            1.,
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
        assert_eq!(atlas.counters().glyph_atlas_pages, 2);
    }
    #[test]
    fn atlas_uvs_use_the_allocated_region() {
        let entry = AtlasEntry {
            page: 3,
            x: 11,
            y: 20,
            width: 10,
            height: 4,
            bearing_x: -2,
            bearing_y: 3,
        };
        assert_eq!(
            entry.uv_rect(),
            [11.5 / 1024., 20.5 / 1024., 20.5 / 1024., 23.5 / 1024.]
        );
    }
    #[test]
    fn glyph_quad_uses_baseline_and_bearings() {
        let entry = AtlasEntry {
            page: 0,
            x: 1,
            y: 1,
            width: 8,
            height: 10,
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
                translation: Offset::ZERO,
                width: 100.,
                height: 100.,
                scale: 1.,
            },
        );
        assert_eq!(instance.rect, ndc_rect(19., 21., 8., 10., 100., 100.));
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
    fn raster_cache_reuses_color_independent_glyphs_but_not_dpi_size() {
        let mut text = TextEngine::new();
        let layout = text.layout("Hello", &TextStyle::default(), None, TextAlign::Start);
        let run = &layout.lines[0].run;
        let glyph = run.glyphs[0].id;
        let mut atlas = GlyphAtlas::new();
        let first = atlas.lookup_or_rasterize(run, glyph, 1.0).unwrap().entry;
        // Glyph color is deliberately not an atlas key: the text pipeline puts
        // it in each instance, so a color-only repaint is a cache hit.
        let same_color_changed = atlas.lookup_or_rasterize(run, glyph, 1.0).unwrap().entry;
        let higher_dpi = atlas.lookup_or_rasterize(run, glyph, 2.0).unwrap().entry;
        assert_eq!(first, same_color_changed);
        assert_ne!(
            GlyphCacheKey {
                font: run.font.id(),
                glyph,
                physical_size: run.font_size as u16
            },
            GlyphCacheKey {
                font: run.font.id(),
                glyph,
                physical_size: (run.font_size * 2.) as u16
            }
        );
        assert!(
            atlas
                .entry(GlyphCacheKey {
                    font: run.font.id(),
                    glyph,
                    physical_size: (run.font_size * 2.) as u16
                })
                .is_some()
        );
        assert!(higher_dpi.width > 0);
        assert_eq!(atlas.counters().glyph_cache_hits, 1);
    }
    #[test]
    fn counter_text_warms_the_atlas_incrementally() {
        fn rasterize(atlas: &mut GlyphAtlas, text: &mut TextEngine, value: &str) {
            for label in ["Incular Counter", value, "Increment"] {
                let layout = text.layout(label, &TextStyle::default(), None, TextAlign::Start);
                for line in layout.lines.iter() {
                    for glyph in line.run.glyphs.iter() {
                        let _ = atlas.lookup_or_rasterize(&line.run, glyph.id, 1.0);
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
}
