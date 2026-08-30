//! Renderer-neutral scene, canvas, display-list, and compositor primitives.

mod compositor;
mod display_list;
mod effects;
mod geometry;
mod glyphs;
mod gradients;
mod paint;
mod paths;

pub use compositor::{
    Annotation, CompositorDiagnostics, FlattenedAnnotation, FlattenedPicture, LayerAnchor, LayerId,
    LayerKind, LayerLink, LayerTree, normalize_opacity,
};
pub use display_list::{Canvas, DisplayList, ImageSampling, PaintCommand};
pub use effects::{
    BlendMode, ColorFilter, ColorMatrix, DropShadowEffect, Effect, EffectChain, GaussianBlur,
    blend_premultiplied, blur_bounds, blur_margin, drop_shadow_bounds, gaussian_kernel_weights,
    normalize_sigma, premultiplied_shadow_sample,
};
pub use geometry::{CornerRadii, RRect};
pub use glyphs::{GlyphPosition, GlyphRun};
pub use gradients::{
    Brush, FilterQuality, GradientId, GradientStop, GradientStops, LinearGradient, RadialGradient,
    Shader, SweepGradient, linear_gradient_t, radial_gradient_t, sample_gradient_stops,
    sweep_gradient_t,
};
pub use paint::{
    Border, Decoration, FillRule, LineCap, LineJoin, Paint, PaintStyle, Shadow, Stroke,
};
pub use paths::{Path, PathBuilder, PathId};
