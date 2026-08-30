//! Retained `wgpu` renderer for renderer-neutral Incular display lists.
//!
//! Shaping remains in `incular-text`. This crate rasterizes its glyph IDs at
//! physical DPI, retains their coverage masks in `R8Unorm` atlas textures, and
//! draws atlas-backed instanced quads in display-list order.
//!
//! The implementation is organized by ownership and rendering responsibility;
//! this module intentionally remains the stable public facade.

#[allow(unused_imports)]
mod prelude {
    pub(crate) use bytemuck::{Pod, Zeroable};
    pub(crate) use fontdue::{Font, FontSettings};
    pub(crate) use incular_assets::FontId;
    pub(crate) use incular_core::{Color, Offset, Rect, Size, Transform};
    pub(crate) use incular_image::{ImageHandle, ImageId};
    pub(crate) use incular_platform::{PhysicalSize, RawWindowHandles};
    pub(crate) use incular_rendering as incular_painting;
    pub(crate) use incular_rendering::{
        BlendMode, Brush, ColorFilter, DisplayList, DropShadowEffect, FillRule, GaussianBlur,
        GlyphRun, GradientId, ImageSampling, LineCap, LineJoin, PaintCommand, Path, PathId, RRect,
        Stroke, blur_bounds, drop_shadow_bounds, gaussian_kernel_weights, normalize_opacity,
        normalize_sigma, sample_gradient_stops,
    };
    pub(crate) use kurbo::PathEl;
    pub(crate) use lyon_tessellation::{
        FillOptions, FillRule as LyonFillRule, FillTessellator, StrokeOptions, StrokeTessellator,
        VertexBuffers, geometry_builder::simple_builder, math::point, path::Path as LyonPath,
    };
    pub(crate) use std::collections::{HashMap, VecDeque, hash_map::Entry};
    pub(crate) use std::fmt::Write;
    pub(crate) use std::ops::{Deref, DerefMut};
    pub(crate) use std::sync::{Arc, Mutex};
    pub(crate) use std::time::{Duration, Instant};
    pub(crate) use wgpu::util::DeviceExt;
    pub(crate) use wgpu_profiler::{GpuProfiler, GpuProfilerSettings, GpuTimerQueryResult};
}

mod batching;
mod compositor;
mod constants;
mod diagnostics;
mod geometry;
mod glyphs;
mod pipelines;
mod render_passes;
mod render_plan;
mod renderer;
mod resources;

pub use batching::{BatchPlan, RectangleBatch, RectangleInstance};
pub use diagnostics::{CapturedFrame, GpuCounters, GpuFrameTimings, RenderStats, RendererError};
pub use geometry::{PathMesh, tessellate_path};
pub use glyphs::{
    AtlasEntry, CoverageHistogram, GLYPH_ATLAS_PADDING, GlyphAtlas, GlyphAtlasClass,
    GlyphAtlasMemory, GlyphCacheKey, GlyphRasterDebug, GlyphRasterRequest, GlyphSizeClass,
    RasterizedGlyph, coverage_histogram,
};
pub use renderer::WgpuRenderer;
pub use resources::{
    SharedGpuContext, SharedGpuDiagnostics, SharedGpuResourceId, SharedGpuResourceRegistry,
    WindowGpuPresentation, WindowGpuState,
};

pub(crate) use compositor::*;
pub(crate) use constants::*;
pub(crate) use diagnostics::{create_gpu_profiler, query_duration_us};
pub(crate) use pipelines::*;
pub(crate) use render_passes::*;
pub(crate) use render_plan::*;
