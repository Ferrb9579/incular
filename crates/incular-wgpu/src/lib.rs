//! Retained `wgpu` renderer for renderer-neutral Incular display lists.
//!
//! Shaping remains in `incular-text`. This crate rasterizes its glyph IDs at
//! physical DPI, retains their coverage masks in `R8Unorm` atlas textures, and
//! draws atlas-backed instanced quads in display-list order.
use bytemuck::{Pod, Zeroable};
use fontdue::{Font, FontSettings};
use incular_assets::FontId;
use incular_core::{Color, Offset, Rect, Size};
use incular_painting::{DisplayList, GlyphRun, PaintCommand};
use incular_platform::{PhysicalSize, RawWindowHandles};
use std::collections::{HashMap, hash_map::Entry};
use wgpu::util::DeviceExt;

const ATLAS_PAGE_SIZE: u16 = 1024;
const ATLAS_PADDING: u16 = 1;

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
                PaintCommand::PushClip { .. }
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
    pub buffer_reallocated: bool,
    pub glyph_buffer_reallocated: bool,
    pub presented: bool,
}
#[derive(Debug)]
pub enum RendererError {
    Adapter(wgpu::RequestAdapterError),
    Device(wgpu::RequestDeviceError),
    Surface(wgpu::CreateSurfaceError),
    OutOfMemory,
}
impl std::fmt::Display for RendererError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Adapter(e) => write!(f, "unable to acquire GPU adapter: {e}"),
            Self::Device(e) => write!(f, "unable to acquire GPU device: {e}"),
            Self::Surface(e) => write!(f, "unable to create GPU surface: {e}"),
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

struct GpuAtlasPage {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
}
#[derive(Clone, Copy, Debug, PartialEq)]
struct ClipRect {
    rect: Rect,
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum ClipState {
    Unbounded,
    Rect(ClipRect),
    Empty,
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
}
#[derive(Clone, Copy)]
struct GlyphSurface {
    translation: Offset,
    width: f32,
    height: f32,
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
    mesh: wgpu::Buffer,
    instances: wgpu::Buffer,
    instance_capacity: usize,
    glyph_instances: wgpu::Buffer,
    glyph_instance_capacity: usize,
    atlas_bind_group_layout: wgpu::BindGroupLayout,
    atlas_sampler: wgpu::Sampler,
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
        let rectangle_pipeline = create_rectangle_pipeline(&device, config.format);
        let text_pipeline = create_text_pipeline(&device, config.format, &atlas_bind_group_layout);
        let instances = create_instance_buffer(&device, 1);
        let glyph_instances = create_glyph_buffer(&device, 1);
        Ok(Self {
            _instance: instance,
            handles,
            surface,
            device,
            queue,
            config,
            rectangle_pipeline,
            text_pipeline,
            mesh,
            instances,
            instance_capacity: 1,
            glyph_instances,
            glyph_instance_capacity: 1,
            atlas_bind_group_layout,
            atlas_sampler,
            atlas_pages: Vec::new(),
            counters: GpuCounters {
                rectangle_pipeline_creations: 1,
                text_pipeline_creations: 1,
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
        let batches = self.lower_draw_batches(list, scale);
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
        let rectangle_reallocated = self.ensure_rectangle_capacity(rectangles);
        let glyph_reallocated = self.ensure_glyph_capacity(glyphs);
        self.upload_instance_data(&batches, scale);
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
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            for batch in &batches {
                let clip = match batch {
                    DrawBatch::Rectangles { clip, .. } | DrawBatch::Glyphs { clip, .. } => *clip,
                };
                if !set_scissor(
                    &mut pass,
                    clip,
                    self.config.width,
                    self.config.height,
                    scale,
                ) {
                    continue;
                }
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
                    _ => {}
                }
            }
        }
        self.queue.submit(Some(encoder.finish()));
        self.queue.present(frame);
        self.counters.frames += 1;
        self.counters.draw_calls += u64::from(draw_calls);
        self.counters.text_draw_calls += u64::from(text_draw_calls);
        self.counters.rectangle_instances += rectangles as u64;
        self.counters.glyph_instances += glyphs as u64;
        Ok(RenderStats {
            draw_calls,
            rectangle_instances: rectangles as u32,
            glyph_instances: glyphs as u32,
            buffer_reallocated: rectangle_reallocated,
            glyph_buffer_reallocated: glyph_reallocated,
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
    fn upload_instance_data(&self, batches: &[DrawBatch], scale: f32) {
        let mut rectangle_offset = 0_u64;
        let mut glyph_offset = 0_u64;
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
                _ => {}
            }
        }
    }
    fn lower_draw_batches(&mut self, list: &DisplayList, scale: f32) -> Vec<DrawBatch> {
        let mut batches = Vec::new();
        let mut transforms = vec![Offset::ZERO];
        let mut clips = vec![ClipState::Unbounded];
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
                        ClipState::Empty => ClipState::Empty,
                    };
                    clips.push(combined);
                }
                PaintCommand::PopClip => {
                    if clips.len() > 1 {
                        clips.pop();
                    }
                }
            }
        }
        batches
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
    )
}
fn create_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    source: &str,
    label: &'static str,
    atlas: Option<&wgpu::BindGroupLayout>,
    second: wgpu::VertexBufferLayout<'static>,
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
        depth_stencil: None,
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
    let ClipState::Rect(clip) = clip else {
        if clip == ClipState::Empty {
            return false;
        }
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
