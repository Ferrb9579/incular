//! Retained layer, clipping, physical-model, and grid descriptors.

use std::sync::Arc;

use incular_config::Clip;
use incular_core::{Color, Offset};
use incular_rendering::{CornerRadii, Path};
use typed_builder::TypedBuilder;

use crate::{ClipRRect, DecoratedBox, DropShadow, Widget};

/// Renders its child into a retained raster snapshot for performance optimization.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct SnapshotWidget {
    #[builder(setter(into))]
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
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct ClipRSuperellipse {
    #[builder(setter(transform = |radius: f32| radius.max(0.0)))]
    pub(super) radius: f32,
    #[builder(default = Clip::AntiAlias)]
    pub(super) clip_behavior: Clip,
    #[builder(setter(into))]
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
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct PhysicalModel {
    color: Color,
    #[builder(default = Color::rgba(0, 0, 0, 100))]
    pub(super) shadow_color: Color,
    #[builder(
        default = 0.0,
        setter(transform = |elevation: f32| elevation.max(0.0))
    )]
    pub(super) elevation: f32,
    #[builder(default = CornerRadii::default())]
    border_radius: CornerRadii,
    #[builder(default = Clip::None)]
    clip_behavior: Clip,
    #[builder(setter(into))]
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
        let surface = DecoratedBox::new(value.child)
            .background(value.color)
            .radius(value.border_radius.top_left);
        if value.elevation > 0.0 {
            DropShadow::new(
                Offset::new(0.0, value.elevation * 0.18),
                (value.elevation * 0.55).max(1.0),
                value.shadow_color,
                surface,
            )
            .into()
        } else {
            surface.into()
        }
    }
}

/// A physical layer widget with custom path shape and elevation.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct PhysicalShape {
    #[builder(setter(into))]
    clipper: Arc<Path>,
    color: Color,
    #[builder(default = Color::rgba(0, 0, 0, 100))]
    shadow_color: Color,
    #[builder(
        default = 0.0,
        setter(transform = |elevation: f32| elevation.max(0.0))
    )]
    pub(super) elevation: f32,
    #[builder(setter(into))]
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
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct GridPaper {
    #[builder(default = Color::rgba(120, 160, 240, 60))]
    color: Color,
    #[builder(
        default = 100.0,
        setter(transform = |interval: f32| interval.max(1.0))
    )]
    interval: f32,
    #[builder(default = 2)]
    divisions: usize,
    #[builder(default = 5)]
    subdivisions: usize,
    #[builder(default, setter(strip_option, into))]
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
