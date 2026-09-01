use super::prelude::*;
use crate::surface::SurfaceAlphaError;

impl GpuCounters {
    /// Total render pipelines created across every family.
    #[must_use]
    pub const fn total_pipeline_creations(&self) -> u64 {
        self.text_pipeline_creations
            + self.rectangle_pipeline_creations
            + self.image_pipeline_creations
            + self.path_pipeline_creations
            + self.composite_pipeline_creations
            + self.surface_present_pipeline_creations
            + self.stencil_pipeline_creations
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
    /// Final premultiplied->straight presentation pipeline creation. The
    /// pipeline is created once per target format even when a given surface can
    /// present the premultiplied scene directly.
    pub surface_present_pipeline_creations: u64,
    /// Full-surface premultiplied scene targets allocated for postmultiplied
    /// native presentation. Retained until resize.
    pub surface_present_target_creations: u64,
    /// Bytes retained by the current full-surface presentation target,
    /// including its stencil attachment. Zero for direct presentation.
    pub surface_present_cached_bytes: usize,
    /// Frames that required a final premultiplied->straight presentation pass.
    pub surface_present_conversion_passes: u64,
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
    // Task 14 profiler additions: steady-state invariants rely on these being
    // zero between initialization and genuine target-format changes.
    pub render_passes: u64,
    pub queue_submissions: u64,
    pub buffer_uploads: u64,
    pub buffer_upload_bytes: u64,
    pub texture_upload_bytes: u64,
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
    /// Rounded rects plus stencil-mask draws issued this frame.
    pub rounded_rect_instances: u32,
    /// Path draws (fills and strokes) issued this frame.
    pub path_draws: u32,
    /// Total render passes recorded, including offscreen effect passes.
    pub render_passes: u32,
    /// Path triangles submitted this frame (3 vertices each).
    pub path_triangles: u32,
    /// Instance-buffer bytes written this frame.
    pub upload_bytes: u64,
    /// Texture pixel bytes written this frame (glyph atlas, images, LUTs).
    pub texture_upload_bytes: u64,
    /// Queue submissions this frame (main pass plus any offscreen effects).
    pub queue_submissions: u32,
    /// Batching/lowering duration in microseconds.
    pub prepare_us: u32,
    /// Main-pass recording duration in microseconds.
    pub encode_us: u32,
    /// Final submit+present call duration in microseconds.
    pub submit_us: u32,
    /// Render pipelines created during this frame. Initialization and
    /// target-format changes aside, this must remain zero.
    pub pipelines_created: u32,
}

/// A GPU readback of one rendered Incular frame. Pixels are tightly packed,
/// top-to-bottom, straight-alpha RGBA8 regardless of the native surface's
/// channel order or compositor alpha representation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapturedFrame {
    pub width: u32,
    pub height: u32,
    pub rgba8: Vec<u8>,
}
impl RenderStats {
    /// Total instances across every instance-driven pipeline family.
    #[must_use]
    pub const fn total_instances(&self) -> u32 {
        self.rectangle_instances
            + self.glyph_instances
            + self.image_instances
            + self.rounded_rect_instances
    }
}
/// Latest resolved GPU timing sample for one window.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GpuFrameTimings {
    pub frame: u64,
    /// GPU duration of the main ordered UI render pass, in microseconds.
    pub main_pass_us: f64,
}

pub(crate) fn create_gpu_profiler(device: &wgpu::Device) -> Result<GpuProfiler, RendererError> {
    GpuProfiler::new(
        device,
        GpuProfilerSettings {
            enable_timer_queries: device.features().contains(wgpu::Features::TIMESTAMP_QUERY),
            // The renderer uses explicit pass queries below. Debug groups can
            // be enabled by a future RenderDoc/debug-marker setting without
            // changing the timing contract.
            enable_debug_groups: false,
            max_num_pending_frames: 3,
        },
    )
    .map_err(|error| RendererError::GpuProfiler(error.to_string()))
}

pub(crate) fn query_duration_us(results: &[GpuTimerQueryResult], label: &str) -> Option<f64> {
    results.iter().find_map(|result| {
        if result.label == label {
            result.time.as_ref().and_then(|range| {
                (range.end >= range.start).then_some((range.end - range.start) * 1_000_000.)
            })
        } else {
            query_duration_us(&result.nested_queries, label)
        }
    })
}

#[derive(Debug)]
pub enum RendererError {
    Adapter(wgpu::RequestAdapterError),
    Device(wgpu::RequestDeviceError),
    Surface(wgpu::CreateSurfaceError),
    SurfaceAlpha(SurfaceAlphaError),
    ImageTooLarge {
        width: u32,
        height: u32,
        limit: u32,
    },
    GlyphAtlasPageTooLarge {
        page: u16,
        limit: u32,
    },
    StencilDepthOverflow,
    UnbalancedClipStack,
    OffscreenTargetTooLarge {
        width: u32,
        height: u32,
        limit: u32,
    },
    OutOfMemory,
    GpuProfiler(String),
    /// A built-in render pipeline failed `wgpu` validation during renderer
    /// initialization. Initialization fails cleanly instead of continuing with
    /// a broken renderer; `label` names the pipeline and `reason` carries the
    /// `wgpu` validation message.
    PipelineCreation {
        label: String,
        reason: String,
    },
}
impl std::fmt::Display for RendererError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Adapter(e) => write!(f, "unable to acquire GPU adapter: {e}"),
            Self::Device(e) => write!(f, "unable to acquire GPU device: {e}"),
            Self::Surface(e) => write!(f, "unable to create GPU surface: {e}"),
            Self::SurfaceAlpha(error) => {
                write!(f, "GPU surface alpha configuration failed: {error}")
            }
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
            Self::GpuProfiler(error) => write!(f, "GPU profiler initialization failed: {error}"),
            Self::PipelineCreation { label, reason } => write!(
                f,
                "render pipeline '{label}' failed GPU validation: {reason}"
            ),
        }
    }
}
impl std::error::Error for RendererError {}
