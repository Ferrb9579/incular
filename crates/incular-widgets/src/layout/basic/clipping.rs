//! Clipping layout primitives.

use std::sync::Arc;

use incular_config::Clip;
use incular_rendering::{CornerRadii, Path};
use typed_builder::TypedBuilder;

use crate::{Widget, WidgetKind};

/// Clips its child using a rectangular boundary.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct ClipRect {
    #[builder(default = Clip::HardEdge)]
    clip_behavior: Clip,
    #[builder(setter(into))]
    child: Widget,
}

impl ClipRect {
    /// Creates a ClipRect widget.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            clip_behavior: Clip::HardEdge,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn clip_behavior(mut self, clip: Clip) -> Self {
        self.clip_behavior = clip;
        self
    }
}

impl From<ClipRect> for Widget {
    fn from(value: ClipRect) -> Self {
        Widget::from_kind(WidgetKind::ClipRect {
            clip_behavior: value.clip_behavior,
            child: std::rc::Rc::new(value.child),
        })
    }
}

/// Clips its child using a rounded-rectangular boundary.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct ClipRRect {
    #[builder(setter(into))]
    radius: CornerRadii,
    #[builder(default = Clip::AntiAlias)]
    clip_behavior: Clip,
    #[builder(setter(into))]
    child: Widget,
}

impl ClipRRect {
    /// Creates a ClipRRect with uniform or corner radii.
    #[must_use]
    pub fn new(radius: impl Into<CornerRadii>, child: impl Into<Widget>) -> Self {
        Self {
            radius: radius.into(),
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

impl From<ClipRRect> for Widget {
    fn from(value: ClipRRect) -> Self {
        Widget::from_kind(WidgetKind::ClipRRect {
            radius: value.radius,
            clip_behavior: value.clip_behavior,
            child: std::rc::Rc::new(value.child),
        })
    }
}

/// Clips its child using an elliptical / oval boundary.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct ClipOval {
    #[builder(default = Clip::AntiAlias)]
    clip_behavior: Clip,
    #[builder(setter(into))]
    child: Widget,
}

impl ClipOval {
    /// Creates a ClipOval widget.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
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

impl From<ClipOval> for Widget {
    fn from(value: ClipOval) -> Self {
        Widget::from_kind(WidgetKind::ClipOval {
            clip_behavior: value.clip_behavior,
            child: std::rc::Rc::new(value.child),
        })
    }
}

/// Clips its child using an arbitrary [`Path`] boundary.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct ClipPath {
    #[builder(setter(into))]
    path: Arc<Path>,
    #[builder(default = Clip::AntiAlias)]
    clip_behavior: Clip,
    #[builder(setter(into))]
    child: Widget,
}

impl ClipPath {
    /// Creates a ClipPath widget.
    #[must_use]
    pub fn new(path: impl Into<Arc<Path>>, child: impl Into<Widget>) -> Self {
        Self {
            path: path.into(),
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

impl From<ClipPath> for Widget {
    fn from(value: ClipPath) -> Self {
        Widget::from_kind(WidgetKind::ClipPath {
            path: value.path,
            clip_behavior: value.clip_behavior,
            child: std::rc::Rc::new(value.child),
        })
    }
}
