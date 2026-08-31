use super::prelude::*;
use super::*;

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
    pub(crate) handles: RawWindowHandles,
    pub(crate) surface: wgpu::Surface<'static>,
    pub(crate) config: wgpu::SurfaceConfiguration,
    pub(crate) stencil_texture: wgpu::Texture,
    pub(crate) stencil_view: wgpu::TextureView,
    pub(crate) presentation: WindowGpuPresentation,
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
    pub(crate) inner: Arc<SharedGpuContextInner>,
}
pub(crate) struct SharedGpuContextInner {
    pub(crate) instance: wgpu::Instance,
    pub(crate) adapter: wgpu::Adapter,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) device_generation: u64,
    pub(crate) pipelines: Mutex<HashMap<wgpu::TextureFormat, Arc<SharedPipelineResources>>>,
    pub(crate) resources: Mutex<SharedGpuResources>,
    /// Bytes written for retained device-level textures (images, gradient
    /// LUTs). These uploads happen once per resource rather than per frame.
    pub(crate) texture_upload_bytes: std::sync::atomic::AtomicU64,
}
pub(crate) struct SharedGpuResources {
    pub(crate) registry: SharedGpuResourceRegistry,
    pub(crate) images: HashMap<ImageId, Arc<SharedGpuImage>>,
    pub(crate) gradients: HashMap<GradientResourceKey, Arc<SharedGpuGradient>>,
    pub(crate) glyph_atlas: GlyphAtlas,
    pub(crate) glyph_pages: Vec<SharedGpuAtlasPage>,
}
pub(crate) struct SharedGpuImage {
    pub(crate) identity: SharedGpuResourceId,
    pub(crate) _texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    pub(crate) width: u32,
    pub(crate) height: u32,
}
pub(crate) struct SharedGpuGradient {
    pub(crate) resource: GpuGradient,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct GradientResourceKey {
    pub(crate) gradient: GradientId,
    /// A bind group is pipeline-layout compatible only within this explicit
    /// target-format pipeline variant.
    pub(crate) format: wgpu::TextureFormat,
}
pub(crate) struct SharedGpuAtlasPage {
    pub(crate) texture: wgpu::Texture,
}
impl SharedGpuContext {
    /// Creates the one device context to be shared by every desktop window in
    /// an application. `handles` is used only to choose a compatible adapter;
    /// the temporary surface is dropped before this method returns.
    pub async fn new(handles: RawWindowHandles) -> Result<Self, RendererError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        // SAFETY: `RawWindowHandles` is captured from a live native window by
        // the platform runner. The temporary surface is used only while that
        // window remains alive to select a compatible adapter, then dropped
        // before this function returns.
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
        // Timestamp support is additive and optional: adapters that expose it
        // get non-blocking GPU frame timing through wgpu-profiler, everyone
        // else reports `GPU timing unavailable` instead of failing
        // initialization.
        let adapter_features = adapter.features();
        let mut required_features = wgpu::Features::default();
        if adapter_features.contains(wgpu::Features::TIMESTAMP_QUERY) {
            required_features |= wgpu::Features::TIMESTAMP_QUERY;
        }
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("incular shared device"),
                required_features,
                ..Default::default()
            })
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
                texture_upload_bytes: std::sync::atomic::AtomicU64::new(0),
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
            pipeline_count: pipelines.len().saturating_mul(pipeline_contracts().len()),
            shared_image_resources: resources.registry.image_count(),
            shared_glyph_resources: resources.registry.glyph_count(),
            shared_gradient_resources: resources.gradients.len(),
            glyph_atlas_pages: resources.glyph_atlas.page_count(),
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
    pub(crate) fn create_surface(
        &self,
        handles: RawWindowHandles,
    ) -> Result<wgpu::Surface<'static>, RendererError> {
        // SAFETY: the platform runner guarantees that both raw handles remain
        // valid for the lifetime of the renderer/surface. `WgpuRenderer` owns
        // no native window handle and is torn down before the platform window.
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
    pub(crate) fn pipeline_resources(
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
    pub(crate) fn register_pipeline_resources(
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
    pub(crate) fn image_resource(
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
        self.inner.texture_upload_bytes.fetch_add(
            decoded.pixels().len() as u64,
            std::sync::atomic::Ordering::Relaxed,
        );
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
    pub(crate) fn rasterize_glyph(
        &self,
        run: &GlyphRun,
        glyph: u16,
        scale: f64,
    ) -> Option<RasterizedGlyph> {
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
    pub(crate) fn glyph_counters(&self) -> GpuCounters {
        self.inner
            .resources
            .lock()
            .expect("shared resource lock")
            .glyph_atlas
            .counters()
    }
    pub(crate) fn gradient_resource(
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
        self.inner.texture_upload_bytes.fetch_add(
            (pixels.len().max(1) * 4) as u64,
            std::sync::atomic::Ordering::Relaxed,
        );
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
    pub(crate) fn shared_glyph_texture(&self, page: u16) -> wgpu::Texture {
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
