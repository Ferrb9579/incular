//! Retained widgets whose behavior is defined by compositor layers rather
//! than by a standalone render object.

use std::{any::Any, rc::Rc};

use incular_config::Alignment;
use incular_core::{Offset, Rect};
use incular_rendering::{Annotation, BlendMode, GaussianBlur, LayerAnchor, LayerLink, Shader};

use crate::tree::{Widget, WidgetKind};

/// The renderer-neutral form of Flutter's `ShaderCallback`.
///
/// A callback is evaluated once for a retained paint pass with the widget's
/// local bounds. Keeping the callback behind an identity-bearing value lets
/// reconciliation distinguish a changed shader source without putting a
/// backend shader object into the widget tree.
#[derive(Clone)]
pub struct ShaderCallback(Rc<dyn Fn(Rect) -> Shader>);

impl ShaderCallback {
    #[must_use]
    pub fn new(callback: impl Fn(Rect) -> Shader + 'static) -> Self {
        Self(Rc::new(callback))
    }

    #[must_use]
    pub fn call(&self, bounds: Rect) -> Shader {
        (self.0)(bounds)
    }
}

impl<F> From<F> for ShaderCallback
where
    F: Fn(Rect) -> Shader + 'static,
{
    fn from(callback: F) -> Self {
        Self::new(callback)
    }
}

impl std::fmt::Debug for ShaderCallback {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ShaderCallback(..)")
    }
}

impl PartialEq for ShaderCallback {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// Applies a callback-generated shader to a child using Flutter's default
/// `BlendMode::modulate` behavior.
#[derive(Clone, Debug, PartialEq)]
pub struct ShaderMask {
    shader: ShaderCallback,
    blend_mode: BlendMode,
    child: Widget,
}

impl ShaderMask {
    #[must_use]
    pub fn new(shader: impl Into<ShaderCallback>, child: impl Into<Widget>) -> Self {
        Self {
            shader: shader.into(),
            blend_mode: BlendMode::Modulate,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn shader_callback(mut self, shader: impl Into<ShaderCallback>) -> Self {
        self.shader = shader.into();
        self
    }

    #[must_use]
    pub fn blend_mode(mut self, blend_mode: BlendMode) -> Self {
        self.blend_mode = blend_mode;
        self
    }
}

impl From<ShaderMask> for Widget {
    fn from(value: ShaderMask) -> Self {
        Widget::from_kind(WidgetKind::ShaderMask {
            shader: value.shader,
            blend_mode: value.blend_mode,
            child: std::rc::Rc::new(value.child),
        })
    }
}

/// Filters the already-painted backdrop behind a child. The filter is
/// disabled without removing the child from layout or paint order.
#[derive(Clone, Debug, PartialEq)]
pub struct BackdropFilter {
    blur: GaussianBlur,
    blend_mode: BlendMode,
    enabled: bool,
    child: Widget,
}

impl BackdropFilter {
    #[must_use]
    pub fn new(blur: GaussianBlur, child: impl Into<Widget>) -> Self {
        Self {
            blur: GaussianBlur::new(blur.sigma_x, blur.sigma_y),
            blend_mode: BlendMode::SrcOver,
            enabled: true,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn blur(sigma: f32, child: impl Into<Widget>) -> Self {
        Self::new(GaussianBlur::uniform(sigma), child)
    }

    #[must_use]
    pub fn asymmetric(sigma_x: f32, sigma_y: f32, child: impl Into<Widget>) -> Self {
        Self::new(GaussianBlur::new(sigma_x, sigma_y), child)
    }

    #[must_use]
    pub fn filter(mut self, blur: GaussianBlur) -> Self {
        self.blur = GaussianBlur::new(blur.sigma_x, blur.sigma_y);
        self
    }

    #[must_use]
    pub fn blend_mode(mut self, blend_mode: BlendMode) -> Self {
        self.blend_mode = blend_mode;
        self
    }

    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

impl From<BackdropFilter> for Widget {
    fn from(value: BackdropFilter) -> Self {
        Widget::from_kind(WidgetKind::BackdropFilter {
            blur: value.blur,
            blend_mode: value.blend_mode,
            enabled: value.enabled,
            child: std::rc::Rc::new(value.child),
        })
    }
}

/// Associates a strongly typed platform value with a painted region. The
/// value is erased only inside the renderer-neutral layer tree and can be
/// recovered with `LayerTree::find_annotation`.
#[derive(Clone)]
pub struct AnnotatedRegion<T: Any + Clone + 'static> {
    value: T,
    sized: bool,
    child: Widget,
}

impl<T: Any + Clone + 'static> AnnotatedRegion<T> {
    #[must_use]
    pub fn new(value: T, child: impl Into<Widget>) -> Self {
        Self {
            value,
            sized: true,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn sized(mut self, sized: bool) -> Self {
        self.sized = sized;
        self
    }
}

impl<T: Any + Clone + 'static> std::fmt::Debug for AnnotatedRegion<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AnnotatedRegion")
            .field("value_type", &std::any::TypeId::of::<T>())
            .field("sized", &self.sized)
            .field("child", &self.child)
            .finish()
    }
}

impl<T: Any + Clone + 'static> From<AnnotatedRegion<T>> for Widget {
    fn from(value: AnnotatedRegion<T>) -> Self {
        Widget::from_kind(WidgetKind::AnnotatedRegion {
            annotation: Annotation::new(value.value),
            sized: value.sized,
            child: std::rc::Rc::new(value.child),
        })
    }
}

/// Identifies a target that followers can resolve across unrelated retained
/// subtrees. The link is intentionally shared and cheap to clone.
#[derive(Clone, Debug, PartialEq)]
pub struct CompositedTransformTarget {
    link: LayerLink,
    child: Widget,
}

impl CompositedTransformTarget {
    #[must_use]
    pub fn new(link: LayerLink, child: impl Into<Widget>) -> Self {
        Self {
            link,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn link(&self) -> LayerLink {
        self.link.clone()
    }
}

impl From<CompositedTransformTarget> for Widget {
    fn from(value: CompositedTransformTarget) -> Self {
        Widget::from_kind(WidgetKind::CompositedTransformTarget {
            link: value.link,
            child: std::rc::Rc::new(value.child),
        })
    }
}

/// Positions a child relative to a [`CompositedTransformTarget`]. Anchors
/// use Flutter's normalized `Alignment` convention; an unlinked follower
/// keeps its normal layout position when `show_when_unlinked` is true.
#[derive(Clone, Debug, PartialEq)]
pub struct CompositedTransformFollower {
    link: LayerLink,
    show_when_unlinked: bool,
    offset: Offset,
    target_anchor: LayerAnchor,
    follower_anchor: LayerAnchor,
    child: Widget,
}

impl CompositedTransformFollower {
    #[must_use]
    pub fn new(link: LayerLink, child: impl Into<Widget>) -> Self {
        Self {
            link,
            show_when_unlinked: true,
            offset: Offset::ZERO,
            target_anchor: LayerAnchor::TOP_LEFT,
            follower_anchor: LayerAnchor::TOP_LEFT,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn show_when_unlinked(mut self, show: bool) -> Self {
        self.show_when_unlinked = show;
        self
    }

    #[must_use]
    pub fn offset(mut self, offset: Offset) -> Self {
        self.offset = finite_offset(offset);
        self
    }

    #[must_use]
    pub fn target_anchor(mut self, anchor: Alignment) -> Self {
        self.target_anchor = LayerAnchor::new(anchor.x, anchor.y);
        self
    }

    #[must_use]
    pub fn follower_anchor(mut self, anchor: Alignment) -> Self {
        self.follower_anchor = LayerAnchor::new(anchor.x, anchor.y);
        self
    }

    #[must_use]
    pub fn target_layer_anchor(mut self, anchor: LayerAnchor) -> Self {
        self.target_anchor = anchor;
        self
    }

    #[must_use]
    pub fn follower_layer_anchor(mut self, anchor: LayerAnchor) -> Self {
        self.follower_anchor = anchor;
        self
    }
}

impl From<CompositedTransformFollower> for Widget {
    fn from(value: CompositedTransformFollower) -> Self {
        Widget::from_kind(WidgetKind::CompositedTransformFollower {
            link: value.link,
            show_when_unlinked: value.show_when_unlinked,
            offset: value.offset,
            target_anchor: value.target_anchor,
            follower_anchor: value.follower_anchor,
            child: std::rc::Rc::new(value.child),
        })
    }
}

fn finite_offset(offset: Offset) -> Offset {
    Offset::new(
        if offset.x.is_finite() { offset.x } else { 0. },
        if offset.y.is_finite() { offset.y } else { 0. },
    )
}
