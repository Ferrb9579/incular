use incular_config::{Alignment, Axis};
use incular_core::Transform as CoreTransform;
use typed_builder::TypedBuilder;

use crate::{Align, DecoratedBox, Positioned, Transform, Widget};

/// An explicit animation widget that transitions child alignment.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct AlignTransition {
    alignment: Alignment,
    #[builder(default, setter(strip_option))]
    width_factor: Option<f32>,
    #[builder(default, setter(strip_option))]
    height_factor: Option<f32>,
    #[builder(setter(into))]
    child: Widget,
}

impl AlignTransition {
    #[must_use]
    pub fn new(alignment: Alignment, child: impl Into<Widget>) -> Self {
        Self {
            alignment,
            width_factor: None,
            height_factor: None,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn width_factor(mut self, factor: f32) -> Self {
        self.width_factor = Some(factor);
        self
    }

    #[must_use]
    pub fn height_factor(mut self, factor: f32) -> Self {
        self.height_factor = Some(factor);
        self
    }
}

impl From<AlignTransition> for Widget {
    fn from(value: AlignTransition) -> Self {
        let mut a = Align::new(value.alignment, value.child);
        if let Some(wf) = value.width_factor {
            a = a.width_factor(wf);
        }
        if let Some(hf) = value.height_factor {
            a = a.height_factor(hf);
        }
        a.into()
    }
}

/// An explicit animation widget that animates its own size and clips its child.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct SizeTransition {
    #[builder(default = Axis::Vertical)]
    axis: Axis,
    #[builder(setter(transform = |size_factor: f32| size_factor.clamp(0.0, 1.0)))]
    size_factor: f32,
    #[builder(default)]
    axis_alignment: f32,
    #[builder(setter(into))]
    child: Widget,
}

impl SizeTransition {
    #[must_use]
    pub fn new(size_factor: f32, child: impl Into<Widget>) -> Self {
        Self {
            axis: Axis::Vertical,
            size_factor: size_factor.clamp(0.0, 1.0),
            axis_alignment: 0.0,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn axis(mut self, axis: Axis) -> Self {
        self.axis = axis;
        self
    }

    #[must_use]
    pub fn axis_alignment(mut self, alignment: f32) -> Self {
        self.axis_alignment = alignment;
        self
    }
}

impl From<SizeTransition> for Widget {
    fn from(value: SizeTransition) -> Self {
        match value.axis {
            Axis::Horizontal => crate::layout::FractionallySizedBox::new(value.child)
                .width_factor(value.size_factor)
                .into(),
            Axis::Vertical => crate::layout::FractionallySizedBox::new(value.child)
                .height_factor(value.size_factor)
                .into(),
        }
    }
}

/// An explicit animation widget that animates a [`Positioned`] child in a [`Stack`](crate::Stack).
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct PositionedTransition {
    #[builder(default, setter(strip_option))]
    left: Option<f32>,
    #[builder(default, setter(strip_option))]
    top: Option<f32>,
    #[builder(default, setter(strip_option))]
    right: Option<f32>,
    #[builder(default, setter(strip_option))]
    bottom: Option<f32>,
    #[builder(default, setter(strip_option))]
    width: Option<f32>,
    #[builder(default, setter(strip_option))]
    height: Option<f32>,
    #[builder(setter(into))]
    child: Widget,
}

impl PositionedTransition {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            left: None,
            top: None,
            right: None,
            bottom: None,
            width: None,
            height: None,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn left(mut self, left: f32) -> Self {
        self.left = Some(left);
        self
    }

    #[must_use]
    pub fn top(mut self, top: f32) -> Self {
        self.top = Some(top);
        self
    }

    #[must_use]
    pub fn right(mut self, right: f32) -> Self {
        self.right = Some(right);
        self
    }

    #[must_use]
    pub fn bottom(mut self, bottom: f32) -> Self {
        self.bottom = Some(bottom);
        self
    }
}

impl From<PositionedTransition> for Widget {
    fn from(value: PositionedTransition) -> Self {
        let mut p = Positioned::new(value.child);
        if let Some(v) = value.left {
            p = p.left(v);
        }
        if let Some(v) = value.top {
            p = p.top(v);
        }
        if let Some(v) = value.right {
            p = p.right(v);
        }
        if let Some(v) = value.bottom {
            p = p.bottom(v);
        }
        p.into()
    }
}

/// An explicit animation widget that animates a relative positioned rect.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct RelativePositionedTransition {
    #[builder(setter(into))]
    child: Widget,
}

impl RelativePositionedTransition {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl From<RelativePositionedTransition> for Widget {
    fn from(value: RelativePositionedTransition) -> Self {
        Positioned::new(value.child).into()
    }
}

/// An explicit animation widget that transitions the decoration of a [`DecoratedBox`].
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct DecoratedBoxTransition {
    #[builder(setter(into))]
    child: Widget,
}

impl DecoratedBoxTransition {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl From<DecoratedBoxTransition> for Widget {
    fn from(value: DecoratedBoxTransition) -> Self {
        DecoratedBox::new(value.child).into()
    }
}

/// An explicit animation widget that transitions default text styles.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct DefaultTextStyleTransition {
    #[builder(setter(into))]
    child: Widget,
}

impl DefaultTextStyleTransition {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl From<DefaultTextStyleTransition> for Widget {
    fn from(value: DefaultTextStyleTransition) -> Self {
        value.child
    }
}

/// An explicit animation widget that transitions an arbitrary affine transform matrix.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct MatrixTransition {
    #[builder(setter(into))]
    transform: CoreTransform,
    #[builder(setter(into))]
    child: Widget,
}

impl MatrixTransition {
    #[must_use]
    pub fn new(transform: CoreTransform, child: impl Into<Widget>) -> Self {
        Self {
            transform,
            child: child.into(),
        }
    }
}

impl From<MatrixTransition> for Widget {
    fn from(value: MatrixTransition) -> Self {
        Transform::new(value.transform, value.child).into()
    }
}

/// Builds a dual forward/reverse transition composition.
#[derive(Clone, TypedBuilder)]
pub struct DualTransitionBuilder {
    #[builder(setter(into))]
    child: Widget,
}

impl DualTransitionBuilder {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl From<DualTransitionBuilder> for Widget {
    fn from(value: DualTransitionBuilder) -> Self {
        value.child
    }
}
