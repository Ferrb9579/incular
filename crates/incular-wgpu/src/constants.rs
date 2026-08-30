pub(crate) const ATLAS_PAGE_SIZE: u16 = 1024;
pub(crate) const ATLAS_PADDING: u16 = 1;
pub(crate) const OVERSIZE_PAGE_AREA_DIVISOR: u32 = 4;
/// Prevent an untrusted font size from causing a multi-gigabyte CPU bitmap
/// allocation before the renderer can reject it.
pub(crate) const MAX_GLYPH_BITMAP_BYTES: usize = 8 * 1024 * 1024;
pub(crate) const MAX_GLYPH_RASTER_PPEM: f32 = 1024.;
pub(crate) const GLYPH_ATLAS_FILTER: wgpu::FilterMode = wgpu::FilterMode::Linear;
pub(crate) const IMAGE_CACHE_MAX_UNUSED_FRAMES: u64 = 600;
pub(crate) const PATH_CACHE_MAX_UNUSED_FRAMES: u64 = 600;
pub(crate) const GRADIENT_CACHE_MAX_UNUSED_FRAMES: u64 = 600;
pub(crate) const OFFSCREEN_CACHE_MAX_UNUSED_FRAMES: u64 = 600;
/// Each normalized gradient is resampled into this compact one-dimensional
/// lookup texture. This deterministic representation supports any stop count:
/// every stop participates in the premultiplied-linear samples.
pub(crate) const GRADIENT_LUT_SAMPLES: u32 = 256;
pub(crate) const MAX_STENCIL_CLIP_DEPTH: u8 = u8::MAX;
/// Direct Gaussian kernels are capped at this radius. Larger physical sigma
/// values select the explicit multi-scale path before reaching the shader.
pub(crate) const MAX_BLUR_RADIUS: usize = 48;
pub(crate) const BLUR_WEIGHT_SLOTS: usize = MAX_BLUR_RADIUS + 16;
pub(crate) const LARGE_BLUR_SIGMA_THRESHOLD: f32 = 16.;
/// Kernel coefficients are stable across tiny animation/DPI floating-point
/// differences. Quantizing only the CPU resource key does not change the
/// effect's bounds or filtered-result key.
pub(crate) const BLUR_KERNEL_QUANTUM: f32 = 1. / 64.;
