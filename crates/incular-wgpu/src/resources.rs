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
    /// Drops the context-local image identity retained for an evicted
    /// texture. A later re-upload allocates a fresh identity; identity
    /// values are never reused, so a stale handle can never alias a new
    /// texture. Glyph identities are untouched by image eviction.
    pub fn remove_image(&mut self, image: ImageId) -> bool {
        self.images.remove(&image).is_some()
    }
}

/// Budget for device-owned shared image textures.
///
/// `max_bytes` counts nominal texel bytes per entry (`width × height × 4`
/// for the single-mip `Rgba8UnormSrgb` textures this cache admits),
/// computed with checked arithmetic. It is an accounting estimate, not
/// physical GPU memory: row-pitch padding, driver overhead, samplers, and
/// bind groups are not counted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SharedImageTextureBudget {
    pub max_entries: usize,
    pub max_bytes: u64,
}

impl Default for SharedImageTextureBudget {
    fn default() -> Self {
        Self {
            max_entries: 256,
            max_bytes: 256 * 1024 * 1024,
        }
    }
}

impl SharedImageTextureBudget {
    #[must_use]
    pub const fn new(max_entries: usize, max_bytes: u64) -> Self {
        Self {
            max_entries,
            max_bytes,
        }
    }
}

/// Nominal texel bytes for one `Rgba8UnormSrgb` texture extent. `None` on
/// arithmetic overflow — callers reject the upload instead of admitting an
/// unaccounted entry.
#[must_use]
pub fn shared_image_texture_bytes(width: u32, height: u32) -> Option<u64> {
    u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(4))
}

/// One retained entry: nominal bytes plus the last-use tick. Ticks come
/// from the cache's own monotonic counter, so interleaved renderers share
/// one recency order without any frame clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CachedSharedImageTexture {
    pub bytes: u64,
    pub last_use: u64,
}

/// One eviction: the dropped entry plus whether its texture stayed alive in
/// renderer bindings, active frames, or submitted work at drop time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SharedImageTextureEviction {
    pub id: ImageId,
    pub bytes: u64,
    pub live_elsewhere: bool,
}

/// Counters for the device-owned image-texture cache. Cumulative since
/// construction except where noted; gauges are current values.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SharedImageTextureCounters {
    /// Textures uploaded and admitted to the shared map.
    pub admissions: u64,
    /// Shared-map hits across all renderers (each reuses one upload).
    pub shared_hits: u64,
    /// Map entries dropped by budget eviction or limit tightening.
    pub evictions: u64,
    /// Nominal bytes of dropped entries (not freed GPU memory).
    pub evicted_bytes: u64,
    /// Dropped entries still referenced elsewhere at drop time.
    pub evicted_live: u64,
    /// Uploads served without admission (over the budget by size).
    pub unadmitted_uploads: u64,
}

/// Device-owned admission/eviction/accounting for shared image textures.
/// Holds metadata only — no wgpu types — so the policy is deterministic
/// without a display server. The single instance lives on
/// [`SharedGpuContext`]; every renderer admits and touches through it, so
/// one window's use keeps another window's textures alive and churn in one
/// window evicts oldest-first across both.
///
/// Ownership contract for one retained texture: exactly one cache entry
/// exists per retained texture, keyed by the content [`ImageId`]. The
/// texture stays alive while any of these hold it: the device-owned map
/// (one `Arc` each), a renderer's local map (bounded by that window's
/// unused-frame eviction and released on window disposal), or transient
/// frame locals and in-flight submissions (wgpu keeps submitted work valid
/// after the `Arc` drops, so eviction needs no frame delay). Bind groups
/// reference the texture view at the wgpu level without holding the `Arc`;
/// they keep GPU memory alive invisibly to these counters and die with
/// their owning renderer's local entry. Evicting a map entry therefore
/// drops one reference, never a texture: counters report dropped entries
/// and their nominal bytes, plus how many were still referenced elsewhere.
/// They never claim freed GPU memory.
#[derive(Debug, Default)]
pub struct SharedImageTextureCache {
    limits: SharedImageTextureBudget,
    entries: HashMap<ImageId, CachedSharedImageTexture>,
    /// Least-recently-used order; front is evicted first. Touch moves to
    /// the back. Length always equals entries length, so metadata scales
    /// with retained entries — themselves capped by the budget.
    lru: VecDeque<ImageId>,
    tick: u64,
    retained_bytes: u64,
    counters: SharedImageTextureCounters,
}

impl SharedImageTextureCache {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_limits(limits: SharedImageTextureBudget) -> Self {
        Self {
            limits,
            entries: HashMap::new(),
            lru: VecDeque::new(),
            tick: 0,
            retained_bytes: 0,
            counters: SharedImageTextureCounters::default(),
        }
    }

    #[must_use]
    pub const fn limits(&self) -> SharedImageTextureBudget {
        self.limits
    }

    #[must_use]
    pub const fn counters(&self) -> SharedImageTextureCounters {
        self.counters
    }

    #[must_use]
    pub fn retained_entries(&self) -> usize {
        self.entries.len()
    }

    /// Nominal texel bytes currently retained. An accounting estimate, not
    /// physical GPU memory (see [`SharedImageTextureBudget`]).
    #[must_use]
    pub const fn retained_bytes(&self) -> u64 {
        self.retained_bytes
    }

    /// Whether `bytes` may be admitted under the current limits. Zero
    /// limits, or an entry larger than the whole byte budget, never admit.
    #[must_use]
    pub fn fits(&self, bytes: u64) -> bool {
        self.limits.max_entries > 0 && bytes <= self.limits.max_bytes
    }

    /// Records a shared-map hit for `id`. Returns false for unknown ids.
    pub fn touch(&mut self, id: ImageId) -> bool {
        if !self.entries.contains_key(&id) {
            return false;
        }
        self.tick = self.tick.saturating_add(1);
        let tick = self.tick;
        if let Some(entry) = self.entries.get_mut(&id) {
            entry.last_use = tick;
        }
        if let Some(position) = self.lru.iter().position(|candidate| *candidate == id)
            && let Some(key) = self.lru.remove(position)
        {
            self.lru.push_back(key);
        }
        self.counters.shared_hits += 1;
        true
    }

    /// Admits `id` with nominal `bytes`, evicting least-recently-used
    /// entries first while over budget. `is_live` observes external
    /// references (renderer-held `Arc`s) so each eviction reports whether
    /// its texture survives the drop. An already-present id is treated as
    /// a touch; an over-budget entry is refused without effect, so the
    /// shared map (whose keys always equal these entries) never holds what
    /// the policy cannot account for.
    pub fn admit(
        &mut self,
        id: ImageId,
        bytes: u64,
        is_live: &dyn Fn(ImageId) -> bool,
    ) -> Vec<SharedImageTextureEviction> {
        if self.entries.contains_key(&id) {
            self.touch(id);
            return Vec::new();
        }
        if !self.fits(bytes) {
            return Vec::new();
        }
        self.tick = self.tick.saturating_add(1);
        let tick = self.tick;
        self.entries.insert(
            id,
            CachedSharedImageTexture {
                bytes,
                last_use: tick,
            },
        );
        self.lru.push_back(id);
        self.retained_bytes = self.retained_bytes.saturating_add(bytes);
        self.counters.admissions += 1;
        self.evict_excess(is_live)
    }

    /// Counts one upload served without admission (over budget by size).
    pub fn note_unadmitted_upload(&mut self) {
        self.counters.unadmitted_uploads += 1;
    }

    /// Replaces the budget and immediately trims oldest-first to it.
    pub fn set_limits(
        &mut self,
        limits: SharedImageTextureBudget,
        is_live: &dyn Fn(ImageId) -> bool,
    ) -> Vec<SharedImageTextureEviction> {
        self.limits = limits;
        self.evict_excess(is_live)
    }

    /// Evicts least-recently-used entries until both limits hold. The
    /// just-admitted entry sits at the back, so a fitting admission is
    /// never its own victim.
    fn evict_excess(
        &mut self,
        is_live: &dyn Fn(ImageId) -> bool,
    ) -> Vec<SharedImageTextureEviction> {
        let mut evicted = Vec::new();
        while self.entries.len() > self.limits.max_entries
            || self.retained_bytes > self.limits.max_bytes
        {
            let Some(oldest) = self.lru.pop_front() else {
                break;
            };
            if let Some(entry) = self.entries.remove(&oldest) {
                self.retained_bytes = self.retained_bytes.saturating_sub(entry.bytes);
                self.counters.evictions += 1;
                self.counters.evicted_bytes =
                    self.counters.evicted_bytes.saturating_add(entry.bytes);
                let live_elsewhere = is_live(oldest);
                if live_elsewhere {
                    self.counters.evicted_live += 1;
                }
                evicted.push(SharedImageTextureEviction {
                    id: oldest,
                    bytes: entry.bytes,
                    live_elsewhere,
                });
            }
        }
        debug_assert_eq!(self.lru.len(), self.entries.len());
        evicted
    }
}

/// Read-only shared GPU ownership diagnostics. Counts describe one
/// application/device, rather than any individual presentation surface.
/// Image-texture counters come from the device-owned admission policy:
/// `shared_image_*` describe retained map entries and dropped entries,
/// never freed GPU memory — an evicted entry still referenced elsewhere
/// (renderer bindings, active frames, submitted work) frees nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SharedGpuDiagnostics {
    pub device_generation: u64,
    pub pipeline_variants: usize,
    pub pipeline_count: usize,
    pub shared_image_resources: usize,
    pub shared_glyph_resources: usize,
    pub shared_gradient_resources: usize,
    pub glyph_atlas_pages: usize,
    /// Nominal texel bytes retained in the shared image map. An accounting
    /// estimate, not physical GPU memory (see [`SharedImageTextureBudget`]).
    pub shared_image_texel_bytes: u64,
    /// Map entries dropped by image-texture budget eviction.
    pub shared_image_evictions: u64,
    /// Dropped entries still referenced elsewhere at drop time.
    pub shared_image_evicted_live: u64,
    /// Uploads served without shared admission (over budget by size).
    pub shared_image_unadmitted: u64,
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

/// Surface and compositor ownership for exactly one native window. The owned
/// target keeps the native window alive through rendering and surface recovery.
pub struct WindowGpuState {
    pub(crate) target: WindowSurfaceTarget,
    pub(crate) surface: wgpu::Surface<'static>,
    pub(crate) config: wgpu::SurfaceConfiguration,
    pub(crate) transparency_mode: TransparencyMode,
    pub(crate) background_color: Color,
    pub(crate) alpha_plan: SurfaceAlphaPlan,
    pub(crate) stencil_texture: wgpu::Texture,
    pub(crate) stencil_view: wgpu::TextureView,
    pub(crate) presentation: WindowGpuPresentation,
}
impl WindowGpuState {
    #[must_use]
    pub const fn presentation(&self) -> WindowGpuPresentation {
        self.presentation
    }

    /// Concrete native-compositor alpha contract selected for this surface.
    #[must_use]
    pub const fn alpha_plan(&self) -> SurfaceAlphaPlan {
        self.alpha_plan
    }

    #[must_use]
    pub const fn transparency_mode(&self) -> TransparencyMode {
        self.transparency_mode
    }

    #[must_use]
    pub const fn background_color(&self) -> Color {
        self.background_color
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
    /// Device-owned admission/eviction/accounting for `images`. The map
    /// keys always equal the policy entries: every admission and eviction
    /// updates both together, and oversized uploads bypass both.
    pub(crate) image_textures: SharedImageTextureCache,
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
    /// an application. The owned target and transparency mode select an adapter
    /// that satisfies the first window's presentation contract. The temporary
    /// surface and target are released before this method returns; the shared
    /// context does not retain the first window.
    pub async fn new(
        target: WindowSurfaceTarget,
        transparency_mode: TransparencyMode,
    ) -> Result<Self, RendererError> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let surface = target
            .create_surface(&instance)
            .map_err(RendererError::Surface)?;
        let power_preference = wgpu::PowerPreference::from_env().unwrap_or_default();
        let preferred_adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                power_preference,
                ..Default::default()
            })
            .await
            .map_err(RendererError::Adapter)?;
        let adapter = if adapter_supports_window_transparency(
            &surface,
            &preferred_adapter,
            transparency_mode,
        ) {
            preferred_adapter
        } else {
            let candidates = instance
                .enumerate_adapters(wgpu::Backends::all())
                .await
                .into_iter()
                .filter(|adapter| adapter.is_surface_supported(&surface))
                .filter(|adapter| {
                    adapter_supports_window_transparency(&surface, adapter, transparency_mode)
                });
            select_fallback_adapter(candidates, power_preference).ok_or_else(|| {
                RendererError::SurfaceAlpha(match transparency_mode {
                    TransparencyMode::Transparent => {
                        crate::surface::SurfaceAlphaError::TransparentCompositingUnsupported
                    }
                    TransparencyMode::Opaque => {
                        crate::surface::SurfaceAlphaError::NoOpaqueCompositingMode
                    }
                })
            })?
        };
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
                    image_textures: SharedImageTextureCache::new(),
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
        target: WindowSurfaceTarget,
        size: PhysicalSize,
        transparency_mode: TransparencyMode,
        background_color: Color,
    ) -> Result<WgpuRenderer, RendererError> {
        WgpuRenderer::new_with_shared(
            self.clone(),
            target,
            size,
            transparency_mode,
            background_color,
        )
        .await
    }
    #[must_use]
    pub fn diagnostics(&self) -> SharedGpuDiagnostics {
        let pipelines = self.inner.pipelines.lock().expect("shared pipeline lock");
        let resources = self.inner.resources.lock().expect("shared resource lock");
        // The shared image map keys always equal the policy entries:
        // admission and eviction update both together, and oversized
        // uploads bypass both.
        debug_assert_eq!(
            resources.registry.image_count(),
            resources.image_textures.retained_entries()
        );
        let image_counters = resources.image_textures.counters();
        SharedGpuDiagnostics {
            device_generation: self.inner.device_generation,
            pipeline_variants: pipelines.len(),
            pipeline_count: pipelines.len().saturating_mul(pipeline_contracts().len()),
            shared_image_resources: resources.registry.image_count(),
            shared_glyph_resources: resources.registry.glyph_count(),
            shared_gradient_resources: resources.gradients.len(),
            glyph_atlas_pages: resources.glyph_atlas.page_count(),
            shared_image_texel_bytes: resources.image_textures.retained_bytes(),
            shared_image_evictions: image_counters.evictions,
            shared_image_evicted_live: image_counters.evicted_live,
            shared_image_unadmitted: image_counters.unadmitted_uploads,
        }
    }
    /// Context-local identity of the currently retained texture for
    /// `image`, if the budget admits one. Eviction drops the identity with
    /// the texture (a re-upload allocates a fresh one), and oversized
    /// uploads never register, so `None` means "not retained", never a
    /// missing allocation.
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
        target: WindowSurfaceTarget,
    ) -> Result<wgpu::Surface<'static>, RendererError> {
        target
            .create_surface(&self.inner.instance)
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
        // Split field borrows up front: the liveness closure below observes
        // the texture map while admission mutates the policy.
        let SharedGpuResources {
            images,
            image_textures,
            registry,
            ..
        } = &mut *resources;
        if let Some(resource) = images.get(&id) {
            let resource = Arc::clone(resource);
            // Cross-renderer reuse: one window's use refreshes the shared
            // recency order, keeping another window's textures alive.
            image_textures.touch(id);
            return Ok((resource, false));
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
        let bytes = shared_image_texture_bytes(decoded.width(), decoded.height()).ok_or(
            RendererError::ImageTooLarge {
                width: decoded.width(),
                height: decoded.height(),
                limit,
            },
        )?;
        // Oversized-for-budget textures still upload (the caller needs them
        // now and the renderer's local frame-evicted cache retains them),
        // but bypass shared admission: the shared map keys always equal the
        // policy entries, and one giant texture must not evict the working
        // set. Such uploads get a fresh identity without a registry entry.
        let admitted = image_textures.fits(bytes);
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
            identity: if admitted {
                registry.image_identity(id)
            } else {
                registry.allocate()
            },
            _texture: texture,
            view,
            width: decoded.width(),
            height: decoded.height(),
        });
        if admitted {
            // The map holds one reference; anything above one means a
            // renderer, an active frame, or submitted work still references
            // the texture. Evictions are reported as entry drops, never as
            // texture freeings.
            let evicted = image_textures.admit(id, bytes, &|candidate| {
                images
                    .get(&candidate)
                    .is_some_and(|held| Arc::strong_count(held) > 1)
            });
            for eviction in &evicted {
                images.remove(&eviction.id);
                registry.remove_image(eviction.id);
            }
            images.insert(id, Arc::clone(&resource));
        } else {
            image_textures.note_unadmitted_upload();
        }
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

fn adapter_supports_window_transparency(
    surface: &wgpu::Surface<'_>,
    adapter: &wgpu::Adapter,
    transparency_mode: TransparencyMode,
) -> bool {
    SurfaceAlphaPlan::select(
        transparency_mode,
        &surface.get_capabilities(adapter).alpha_modes,
    )
    .is_ok()
}

fn select_fallback_adapter(
    mut candidates: impl Iterator<Item = wgpu::Adapter>,
    preference: wgpu::PowerPreference,
) -> Option<wgpu::Adapter> {
    match preference {
        wgpu::PowerPreference::None => candidates.next(),
        wgpu::PowerPreference::LowPower => {
            candidates.max_by_key(|adapter| low_power_rank(adapter.get_info().device_type))
        }
        wgpu::PowerPreference::HighPerformance => {
            candidates.max_by_key(|adapter| high_performance_rank(adapter.get_info().device_type))
        }
    }
}

const fn low_power_rank(device_type: wgpu::DeviceType) -> u8 {
    match device_type {
        wgpu::DeviceType::IntegratedGpu => 5,
        wgpu::DeviceType::DiscreteGpu => 4,
        wgpu::DeviceType::VirtualGpu => 3,
        wgpu::DeviceType::Other => 2,
        wgpu::DeviceType::Cpu => 1,
    }
}

const fn high_performance_rank(device_type: wgpu::DeviceType) -> u8 {
    match device_type {
        wgpu::DeviceType::DiscreteGpu => 5,
        wgpu::DeviceType::IntegratedGpu => 4,
        wgpu::DeviceType::VirtualGpu => 3,
        wgpu::DeviceType::Other => 2,
        wgpu::DeviceType::Cpu => 1,
    }
}
