use super::*;

/// Owns one window's surface, transient buffers, and retained compositor
/// caches. Device-level resources are borrowed from [`SharedGpuContext`].
/// Colors and coverage are straight alpha and use ordinary source-alpha
/// blending.
pub struct WgpuRenderer {
    pub(super) shared: SharedGpuContext,
    pub(super) window_gpu: WindowGpuState,
    pub(super) device: wgpu::Device,
    pub(super) queue: wgpu::Queue,
    pub(super) gpu_profiler: GpuProfiler,
    /// Armed by `render_composited`; consumed by the next top-level pass.
    pub(super) profiler_next_pass: bool,
    /// Renderer frame ids waiting for `wgpu-profiler`'s asynchronous mapping.
    pub(super) profiler_frames: VecDeque<u64>,
    pub(super) latest_gpu_timing: Option<GpuFrameTimings>,
    pub(super) capture_requested: bool,
    pub(super) last_capture: Option<Result<CapturedFrame, String>>,
    pub(super) capture_supported: bool,
    pub(super) rectangle_pipeline: wgpu::RenderPipeline,
    pub(super) text_pipeline: wgpu::RenderPipeline,
    pub(super) image_pipeline: wgpu::RenderPipeline,
    pub(super) rounded_rect_pipeline: wgpu::RenderPipeline,
    pub(super) path_pipeline: wgpu::RenderPipeline,
    pub(super) composite_pipeline: wgpu::RenderPipeline,
    pub(super) straight_alpha_present_pipeline: wgpu::RenderPipeline,
    pub(super) fixed_blend_pipelines: Vec<wgpu::RenderPipeline>,
    pub(super) blur_pipeline: wgpu::RenderPipeline,
    pub(super) resample_pipeline: wgpu::RenderPipeline,
    pub(super) color_matrix_pipeline: wgpu::RenderPipeline,
    pub(super) blend_pipeline: wgpu::RenderPipeline,
    pub(super) stencil_rrect_increment_pipeline: wgpu::RenderPipeline,
    pub(super) stencil_rrect_decrement_pipeline: wgpu::RenderPipeline,
    pub(super) stencil_path_increment_pipeline: wgpu::RenderPipeline,
    pub(super) stencil_path_decrement_pipeline: wgpu::RenderPipeline,
    pub(super) mesh: wgpu::Buffer,
    pub(super) instances: wgpu::Buffer,
    pub(super) instance_capacity: usize,
    pub(super) glyph_instances: wgpu::Buffer,
    pub(super) glyph_instance_capacity: usize,
    pub(super) image_instances: wgpu::Buffer,
    pub(super) image_instance_capacity: usize,
    pub(super) rounded_rect_instances: wgpu::Buffer,
    pub(super) rounded_rect_instance_capacity: usize,
    pub(super) path_instances: wgpu::Buffer,
    pub(super) path_instance_capacity: usize,
    pub(super) composite_instances: wgpu::Buffer,
    pub(super) composite_instance_capacity: usize,
    pub(super) cpu_path_cache: HashMap<PathMeshKey, PathMesh>,
    pub(super) gpu_path_cache: HashMap<PathMeshKey, GpuPathMesh>,
    pub(super) gradient_bind_group_layout: wgpu::BindGroupLayout,
    pub(super) gradient_sampler: wgpu::Sampler,
    pub(super) gradient_cache: RendererImageCache<GradientResourceKey, GpuGradient>,
    pub(super) solid_gradient: GpuGradient,
    pub(super) atlas_bind_group_layout: wgpu::BindGroupLayout,
    pub(super) atlas_sampler: wgpu::Sampler,
    pub(super) image_bind_group_layout: wgpu::BindGroupLayout,
    pub(super) image_samplers: HashMap<ImageSampling, wgpu::Sampler>,
    /// This window's image retention, coordinated with the shared owner
    /// through [`RendererImageCache`]: frame use is recorded locally
    /// without shared locks and flushed in one batch per frame.
    pub(super) image_cache: RendererImageCache<ImageId, GpuImage>,
    pub(super) composite_bind_group_layout: wgpu::BindGroupLayout,
    pub(super) composite_sampler: wgpu::Sampler,
    pub(super) blur_bind_group_layout: wgpu::BindGroupLayout,
    pub(super) blur_sampler: wgpu::Sampler,
    pub(super) blur_params: wgpu::Buffer,
    pub(super) blur_kernel_cache: HashMap<BlurKernelKey, BlurKernel>,
    pub(super) color_matrix_bind_group_layout: wgpu::BindGroupLayout,
    pub(super) color_matrix_sampler: wgpu::Sampler,
    pub(super) color_matrix_params: wgpu::Buffer,
    pub(super) blend_bind_group_layout: wgpu::BindGroupLayout,
    pub(super) blend_sampler: wgpu::Sampler,
    pub(super) destination_targets: Option<DestinationTargets>,
    /// Full-surface retained scene target used only when the native compositor
    /// requires straight/postmultiplied RGB.
    pub(super) presentation_target: Option<OffscreenTarget>,
    pub(super) offscreen_cache: HashMap<incular_painting::LayerId, OffscreenCacheEntry>,
    pub(super) effect_cache: HashMap<incular_painting::LayerId, EffectCacheEntry>,
    pub(super) offscreen_target_pool: OffscreenTargetPool,
    pub(super) offscreen_cache_budget: usize,
    pub(super) device_generation: u64,
    pub(super) target_width: u32,
    pub(super) target_height: u32,
    pub(super) target_origin: Offset,
    pub(super) offscreen_nesting_depth: u64,
    pub(super) atlas_pages: Vec<GpuAtlasPage>,
    pub(super) counters: GpuCounters,
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
    pub(super) fn scene_background_clear(&self) -> wgpu::Color {
        let [red, green, blue, alpha] = self.background_color.to_linear_rgba();
        wgpu::Color {
            r: f64::from(red * alpha),
            g: f64::from(green * alpha),
            b: f64::from(blue * alpha),
            a: f64::from(alpha),
        }
    }

    /// Creates a renderer retaining its native window through an owned target.
    pub async fn new(
        target: WindowSurfaceTarget,
        size: PhysicalSize,
        transparency_mode: TransparencyMode,
        background_color: Color,
    ) -> Result<Self, RendererError> {
        let shared = SharedGpuContext::new(target.clone(), transparency_mode).await?;
        Self::new_with_shared(shared, target, size, transparency_mode, background_color).await
    }
    /// Creates a renderer for one native window using an existing shared GPU
    /// device context. No `wgpu::Instance`, adapter, device, or queue is
    /// recreated by this method.
    pub async fn new_with_shared(
        shared: SharedGpuContext,
        target: WindowSurfaceTarget,
        size: PhysicalSize,
        transparency_mode: TransparencyMode,
        background_color: Color,
    ) -> Result<Self, RendererError> {
        let surface = shared.create_surface(target.clone())?;
        let device = shared.inner.device.clone();
        let queue = shared.inner.queue.clone();
        let texture_limit = device.limits().max_texture_dimension_2d;
        if texture_limit < u32::from(ATLAS_PAGE_SIZE) {
            return Err(RendererError::GlyphAtlasPageTooLarge {
                page: ATLAS_PAGE_SIZE,
                limit: texture_limit,
            });
        }
        let capabilities = surface.get_capabilities(&shared.inner.adapter);
        let mut config = surface
            .get_default_config(&shared.inner.adapter, size.width.max(1), size.height.max(1))
            .ok_or(RendererError::SurfaceConfigurationUnsupported)?;
        let alpha_plan = SurfaceAlphaPlan::select(transparency_mode, &capabilities.alpha_modes)
            .map_err(RendererError::SurfaceAlpha)?;
        config.alpha_mode = alpha_plan.composite_mode();
        // Surface readback is optional in wgpu. Request it only when the
        // adapter advertises COPY_SRC; the renderer reports capture as
        // unavailable on surfaces that cannot be copied safely.
        let capture_supported = surface_capture_supported(&capabilities, config.format);
        if capture_supported {
            config.usage |= wgpu::TextureUsages::COPY_SRC;
        }
        if !size.is_zero() {
            surface.configure(&device, &config);
        }
        if let Some(pipelines) = shared.pipeline_resources(config.format) {
            return Self::from_shared_pipeline_resources(
                shared,
                target,
                surface,
                config,
                alpha_plan,
                transparency_mode,
                background_color,
                size,
                device,
                queue,
                pipelines,
            );
        }
        // Device-level resources (layouts, samplers, unit quad, gradient LUT,
        // and every pipeline) are created once per target format and shared by
        // all windows using that format. A contract regression fails
        // initialization with a labeled error instead of panicking on draw.
        let shared_pipelines =
            create_shared_pipeline_resources(&device, &queue, config.format).await?;
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
        let SharedPipelineResources {
            mesh,
            rectangle_pipeline,
            text_pipeline,
            image_pipeline,
            rounded_rect_pipeline,
            path_pipeline,
            composite_pipeline,
            straight_alpha_present_pipeline,
            fixed_blend_pipelines,
            blur_pipeline,
            resample_pipeline,
            color_matrix_pipeline,
            blend_pipeline,
            stencil_rrect_increment_pipeline,
            stencil_rrect_decrement_pipeline,
            stencil_path_increment_pipeline,
            stencil_path_decrement_pipeline,
            gradient_bind_group_layout,
            gradient_sampler,
            solid_gradient,
            atlas_bind_group_layout,
            atlas_sampler,
            image_bind_group_layout,
            image_samplers,
            composite_bind_group_layout,
            composite_sampler,
            blur_bind_group_layout,
            blur_sampler,
            color_matrix_bind_group_layout,
            color_matrix_sampler,
            blend_bind_group_layout,
            blend_sampler,
        } = shared_pipelines.clone();
        shared.register_pipeline_resources(format, shared_pipelines);
        let gpu_profiler = create_gpu_profiler(&device)?;
        Ok(Self {
            shared,
            window_gpu: WindowGpuState {
                target,
                surface,
                config,
                transparency_mode,
                background_color,
                alpha_plan,
                stencil_texture,
                stencil_view,
                presentation: WindowGpuPresentation::new(size),
            },
            device,
            queue,
            gpu_profiler,
            profiler_next_pass: false,
            profiler_frames: VecDeque::new(),
            latest_gpu_timing: None,
            capture_requested: false,
            last_capture: None,
            capture_supported,
            rectangle_pipeline,
            text_pipeline,
            image_pipeline,
            rounded_rect_pipeline,
            path_pipeline,
            composite_pipeline,
            straight_alpha_present_pipeline,
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
            gradient_cache: RendererImageCache::new(),
            solid_gradient,
            atlas_bind_group_layout,
            atlas_sampler,
            image_bind_group_layout,
            image_samplers,
            image_cache: RendererImageCache::new(),
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
            presentation_target: None,
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
                surface_present_pipeline_creations: 1,
                // Blur/resample share one stable pipeline family; these are
                // retained as explicit diagnostics for effect setup.
                stencil_texture_creations: 1,
                stencil_pipeline_creations: 4,
                ..GpuCounters::default()
            },
        })
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn from_shared_pipeline_resources(
        shared: SharedGpuContext,
        target: WindowSurfaceTarget,
        surface: wgpu::Surface<'static>,
        config: wgpu::SurfaceConfiguration,
        alpha_plan: SurfaceAlphaPlan,
        transparency_mode: TransparencyMode,
        background_color: Color,
        size: PhysicalSize,
        device: wgpu::Device,
        queue: wgpu::Queue,
        pipelines: Arc<SharedPipelineResources>,
    ) -> Result<Self, RendererError> {
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
        let capture_supported = config.usage.contains(wgpu::TextureUsages::COPY_SRC)
            && matches!(
                config.format,
                wgpu::TextureFormat::Rgba8Unorm
                    | wgpu::TextureFormat::Rgba8UnormSrgb
                    | wgpu::TextureFormat::Bgra8Unorm
                    | wgpu::TextureFormat::Bgra8UnormSrgb
            );
        let gpu_profiler = create_gpu_profiler(&device)?;
        Ok(Self {
            shared,
            window_gpu: WindowGpuState {
                target,
                surface,
                config,
                transparency_mode,
                background_color,
                alpha_plan,
                stencil_texture,
                stencil_view,
                presentation: WindowGpuPresentation::new(size),
            },
            device: device.clone(),
            queue,
            gpu_profiler,
            profiler_next_pass: false,
            profiler_frames: VecDeque::new(),
            latest_gpu_timing: None,
            capture_requested: false,
            last_capture: None,
            capture_supported,
            rectangle_pipeline: pipelines.rectangle_pipeline.clone(),
            text_pipeline: pipelines.text_pipeline.clone(),
            image_pipeline: pipelines.image_pipeline.clone(),
            rounded_rect_pipeline: pipelines.rounded_rect_pipeline.clone(),
            path_pipeline: pipelines.path_pipeline.clone(),
            composite_pipeline: pipelines.composite_pipeline.clone(),
            straight_alpha_present_pipeline: pipelines.straight_alpha_present_pipeline.clone(),
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
            gradient_cache: RendererImageCache::new(),
            solid_gradient: pipelines.solid_gradient.clone(),
            atlas_bind_group_layout: pipelines.atlas_bind_group_layout.clone(),
            atlas_sampler: pipelines.atlas_sampler.clone(),
            image_bind_group_layout: pipelines.image_bind_group_layout.clone(),
            image_samplers: pipelines.image_samplers.clone(),
            image_cache: RendererImageCache::new(),
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
            presentation_target: None,
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
        })
    }
    #[must_use]
    pub fn counters(&self) -> GpuCounters {
        let mut counters = self.counters_snapshot();
        counters.texture_upload_bytes += self
            .shared
            .inner
            .texture_upload_bytes
            .load(std::sync::atomic::Ordering::Relaxed);
        counters
    }
    pub(super) fn counters_snapshot(&self) -> GpuCounters {
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
        self.presentation_target = None;
        self.counters.surface_present_cached_bytes = 0;
        self.counters.blend_intermediate_cached_bytes = 0;
    }

    pub(super) fn refresh_surface_alpha_plan(&mut self) -> Result<(), RendererError> {
        let capabilities = self.surface.get_capabilities(&self.shared.inner.adapter);
        let alpha_plan =
            SurfaceAlphaPlan::select(self.transparency_mode, &capabilities.alpha_modes)
                .map_err(RendererError::SurfaceAlpha)?;
        self.config.alpha_mode = alpha_plan.composite_mode();
        self.capture_supported = surface_capture_supported(&capabilities, self.config.format);
        self.config.usage.remove(wgpu::TextureUsages::COPY_SRC);
        if self.capture_supported {
            self.config.usage.insert(wgpu::TextureUsages::COPY_SRC);
        }
        if self.alpha_plan != alpha_plan {
            self.presentation_target = None;
            self.counters.surface_present_cached_bytes = 0;
        }
        self.alpha_plan = alpha_plan;
        Ok(())
    }
}

fn surface_capture_supported(
    capabilities: &wgpu::SurfaceCapabilities,
    format: wgpu::TextureFormat,
) -> bool {
    capabilities.usages.contains(wgpu::TextureUsages::COPY_SRC)
        && matches!(
            format,
            wgpu::TextureFormat::Rgba8Unorm
                | wgpu::TextureFormat::Rgba8UnormSrgb
                | wgpu::TextureFormat::Bgra8Unorm
                | wgpu::TextureFormat::Bgra8UnormSrgb
        )
}
