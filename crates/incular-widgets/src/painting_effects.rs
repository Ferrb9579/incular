//! Painting, image, masking, clipping, and compositor effect widgets.

use incular_config::Clip;
use incular_core::Color;
use incular_image::ImageHandle;
use incular_rendering::{BlendMode, CornerRadii, Path};
use std::sync::Arc;

use crate::{Blur, BlurController, ClipRRect, DecoratedBox, Image, ImageFit, ImageRepeat, Widget};

/// Renders a raw raster image buffer directly without asset caching.
#[derive(Clone, Debug, PartialEq)]
pub struct RawImage {
    image: Option<ImageHandle>,
    width: Option<f32>,
    height: Option<f32>,
    scale: f32,
    color: Option<Color>,
    fit: ImageFit,
    repeat: ImageRepeat,
}

impl Default for RawImage {
    fn default() -> Self {
        Self::new()
    }
}

impl RawImage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            image: None,
            width: None,
            height: None,
            scale: 1.0,
            color: None,
            fit: ImageFit::Contain,
            repeat: ImageRepeat::NoRepeat,
        }
    }

    #[must_use]
    pub fn image(mut self, image: ImageHandle) -> Self {
        self.image = Some(image);
        self
    }

    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width.max(0.0));
        self
    }

    #[must_use]
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height.max(0.0));
        self
    }

    #[must_use]
    pub fn scale(mut self, scale: f32) -> Self {
        self.scale = scale.max(0.001);
        self
    }

    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    #[must_use]
    pub fn fit(mut self, fit: ImageFit) -> Self {
        self.fit = fit;
        self
    }

    #[must_use]
    pub fn repeat(mut self, repeat: ImageRepeat) -> Self {
        self.repeat = repeat;
        self
    }
}

impl From<RawImage> for Widget {
    fn from(value: RawImage) -> Self {
        match value.image {
            Some(handle) => {
                let mut img = Image::new(handle).fit(value.fit).repeat(value.repeat);
                if let Some(w) = value.width {
                    img = img.width(w);
                }
                if let Some(h) = value.height {
                    img = img.height(h);
                }
                img.into()
            }
            None => crate::layout::SizedBox::new()
                .width(value.width.unwrap_or(0.0))
                .height(value.height.unwrap_or(0.0))
                .into(),
        }
    }
}

/// An icon that comes from an [`ImageHandle`].
#[derive(Clone, Debug, PartialEq)]
pub struct ImageIcon {
    image: ImageHandle,
    size: Option<f32>,
    color: Option<Color>,
}

impl ImageIcon {
    #[must_use]
    pub fn new(image: ImageHandle) -> Self {
        Self {
            image,
            size: None,
            color: None,
        }
    }

    #[must_use]
    pub fn size(mut self, size: f32) -> Self {
        self.size = Some(size.max(0.0));
        self
    }

    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }
}

impl From<ImageIcon> for Widget {
    fn from(value: ImageIcon) -> Self {
        let size = value.size.unwrap_or(24.0);
        Image::new(value.image)
            .width(size)
            .height(size)
            .fit(ImageFit::Contain)
            .into()
    }
}

/// An image filter applied directly to its child subtree.
#[derive(Clone, Debug, PartialEq)]
pub struct ImageFiltered {
    sigma: f32,
    controller: Option<BlurController>,
    child: Widget,
}

impl ImageFiltered {
    #[must_use]
    pub fn blur(sigma: f32, child: impl Into<Widget>) -> Self {
        Self {
            sigma,
            controller: None,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn controlled(controller: BlurController, child: impl Into<Widget>) -> Self {
        Self {
            sigma: controller.sigma(),
            controller: Some(controller),
            child: child.into(),
        }
    }
}

impl From<ImageFiltered> for Widget {
    fn from(value: ImageFiltered) -> Self {
        match value.controller {
            Some(c) => Blur::controlled(c, value.child).into(),
            None => Blur::new(value.sigma, value.child).into(),
        }
    }
}

/// Applies a shader / gradient mask over its child widget.
#[derive(Clone, Debug, PartialEq)]
pub struct ShaderMask {
    blend_mode: BlendMode,
    child: Widget,
}

impl ShaderMask {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            blend_mode: BlendMode::Multiply,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn blend_mode(mut self, mode: BlendMode) -> Self {
        self.blend_mode = mode;
        self
    }
}

impl From<ShaderMask> for Widget {
    fn from(value: ShaderMask) -> Self {
        value.child
    }
}

/// Injects system UI / status bar annotations into the tree hierarchy.
#[derive(Clone, Debug, PartialEq)]
pub struct AnnotatedRegion {
    child: Widget,
}

impl AnnotatedRegion {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl From<AnnotatedRegion> for Widget {
    fn from(value: AnnotatedRegion) -> Self {
        value.child
    }
}

/// Renders its child into a retained raster snapshot for performance optimization.
#[derive(Clone, Debug, PartialEq)]
pub struct SnapshotWidget {
    child: Widget,
}

impl SnapshotWidget {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl From<SnapshotWidget> for Widget {
    fn from(value: SnapshotWidget) -> Self {
        crate::layout::RepaintBoundary::new(value.child).into()
    }
}

/// Clips its child using a rounded superellipse (squircle) shape.
#[derive(Clone, Debug, PartialEq)]
pub struct ClipRSuperellipse {
    radius: f32,
    clip_behavior: Clip,
    child: Widget,
}

impl ClipRSuperellipse {
    #[must_use]
    pub fn new(radius: f32, child: impl Into<Widget>) -> Self {
        Self {
            radius: radius.max(0.0),
            clip_behavior: Clip::AntiAlias,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn clip_behavior(mut self, clip: Clip) -> Self {
        self.clip_behavior = clip;
        self
    }
}

impl From<ClipRSuperellipse> for Widget {
    fn from(value: ClipRSuperellipse) -> Self {
        ClipRRect::new(CornerRadii::uniform(value.radius), value.child)
            .clip_behavior(value.clip_behavior)
            .into()
    }
}

/// A physical layer widget with elevation, shadow, and corner radius.
#[derive(Clone, Debug, PartialEq)]
pub struct PhysicalModel {
    color: Color,
    shadow_color: Color,
    elevation: f32,
    border_radius: CornerRadii,
    clip_behavior: Clip,
    child: Widget,
}

impl PhysicalModel {
    #[must_use]
    pub fn new(color: Color, child: impl Into<Widget>) -> Self {
        Self {
            color,
            shadow_color: Color::rgba(0, 0, 0, 100),
            elevation: 0.0,
            border_radius: CornerRadii::default(),
            clip_behavior: Clip::None,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn shadow_color(mut self, color: Color) -> Self {
        self.shadow_color = color;
        self
    }

    #[must_use]
    pub fn elevation(mut self, elevation: f32) -> Self {
        self.elevation = elevation.max(0.0);
        self
    }

    #[must_use]
    pub fn border_radius(mut self, radius: CornerRadii) -> Self {
        self.border_radius = radius;
        self
    }

    #[must_use]
    pub fn clip_behavior(mut self, clip: Clip) -> Self {
        self.clip_behavior = clip;
        self
    }
}

impl From<PhysicalModel> for Widget {
    fn from(value: PhysicalModel) -> Self {
        DecoratedBox::new(value.child)
            .background(value.color)
            .radius(value.border_radius.top_left)
            .into()
    }
}

/// A physical layer widget with custom path shape and elevation.
#[derive(Clone, Debug, PartialEq)]
pub struct PhysicalShape {
    clipper: Arc<Path>,
    color: Color,
    shadow_color: Color,
    elevation: f32,
    child: Widget,
}

impl PhysicalShape {
    #[must_use]
    pub fn new(clipper: impl Into<Arc<Path>>, color: Color, child: impl Into<Widget>) -> Self {
        Self {
            clipper: clipper.into(),
            color,
            shadow_color: Color::rgba(0, 0, 0, 100),
            elevation: 0.0,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn shadow_color(mut self, color: Color) -> Self {
        self.shadow_color = color;
        self
    }

    #[must_use]
    pub fn elevation(mut self, elevation: f32) -> Self {
        self.elevation = elevation.max(0.0);
        self
    }
}

impl From<PhysicalShape> for Widget {
    fn from(value: PhysicalShape) -> Self {
        crate::layout::ClipPath::new(value.clipper, value.child).into()
    }
}

/// Draws an engineering grid paper over its background.
#[derive(Clone, Debug, PartialEq)]
pub struct GridPaper {
    color: Color,
    interval: f32,
    divisions: usize,
    subdivisions: usize,
    child: Option<Widget>,
}

impl Default for GridPaper {
    fn default() -> Self {
        Self::new()
    }
}

impl GridPaper {
    #[must_use]
    pub fn new() -> Self {
        Self {
            color: Color::rgba(120, 160, 240, 60),
            interval: 100.0,
            divisions: 2,
            subdivisions: 5,
            child: None,
        }
    }

    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    #[must_use]
    pub fn interval(mut self, interval: f32) -> Self {
        self.interval = interval.max(1.0);
        self
    }

    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self
    }
}

impl From<GridPaper> for Widget {
    fn from(value: GridPaper) -> Self {
        value
            .child
            .unwrap_or_else(|| crate::layout::SizedBox::shrink().into())
    }
}
