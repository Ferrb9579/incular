//! Size, intrinsic measurement, and constraint-manipulation primitives.

use std::rc::Rc;

use incular_config::{Alignment, Axis, Clip, Constraints};
use incular_core::{Color, Size};
use typed_builder::TypedBuilder;

use super::Align;
use crate::{Widget, WidgetKind};

/// A box with a specified size.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct SizedBox {
    #[builder(default, setter(strip_option))]
    width: Option<f32>,
    #[builder(default, setter(strip_option))]
    height: Option<f32>,
    #[builder(default, setter(strip_option, into))]
    pub(super) child: Option<Widget>,
}

impl Default for SizedBox {
    fn default() -> Self {
        Self::new()
    }
}

impl SizedBox {
    /// Creates an empty or unconstrained SizedBox.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            width: None,
            height: None,
            child: None,
        }
    }

    /// Creates a SizedBox with explicit dimensions and optional child.
    #[must_use]
    pub fn from_dimensions(
        width: Option<f32>,
        height: Option<f32>,
        child: Option<impl Into<Widget>>,
    ) -> Self {
        Self {
            width,
            height,
            child: child.map(Into::into),
        }
    }

    /// Sets the child widget.
    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self
    }

    /// Sets explicit width.
    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Sets explicit height.
    #[must_use]
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    /// Convenience constructor for square box.
    #[must_use]
    pub fn square(dimension: f32) -> Self {
        Self {
            width: Some(dimension),
            height: Some(dimension),
            child: None,
        }
    }

    /// Convenience constructor with `Size`.
    #[must_use]
    pub fn from_size(size: Size) -> Self {
        Self {
            width: Some(size.width),
            height: Some(size.height),
            child: None,
        }
    }

    /// Shrinks to zero size or tight minimum child constraints.
    #[must_use]
    pub const fn shrink() -> Self {
        Self {
            width: Some(0.0),
            height: Some(0.0),
            child: None,
        }
    }

    /// Expands to match parent constraints.
    #[must_use]
    pub const fn expand() -> Self {
        Self {
            width: Some(f32::INFINITY),
            height: Some(f32::INFINITY),
            child: None,
        }
    }

    /// Creates an empty SizedBox.
    #[must_use]
    pub fn empty() -> Self {
        Self::new()
    }
}

impl From<SizedBox> for Widget {
    fn from(value: SizedBox) -> Self {
        let w = value.width;
        let h = value.height;
        let is_expand = w == Some(f32::INFINITY) && h == Some(f32::INFINITY);
        if is_expand && value.child.is_none() {
            return Align::new(Alignment::CENTER, SizedBox::shrink()).into();
        }
        let child = value.child.unwrap_or_else(|| {
            Widget::from_kind(WidgetKind::Box {
                size: Size::ZERO,
                color: Color::TRANSPARENT,
            })
        });
        let mut constraints = Constraints::unbounded();
        let mut has_constraints = false;
        if let Some(w_val) = w
            && w_val.is_finite()
        {
            has_constraints = true;
            constraints = Constraints::new(
                w_val.max(0.0),
                w_val.max(0.0),
                constraints.min_height,
                constraints.max_height,
            );
        }
        if let Some(h_val) = h
            && h_val.is_finite()
        {
            has_constraints = true;
            constraints = Constraints::new(
                constraints.min_width,
                constraints.max_width,
                h_val.max(0.0),
                h_val.max(0.0),
            );
        }
        if has_constraints {
            ConstrainedBox::new(constraints, child).into()
        } else {
            child
        }
    }
}

/// Imposes additional layout constraints on its child.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct ConstrainedBox {
    constraints: Constraints,
    #[builder(setter(into))]
    child: Widget,
}

impl ConstrainedBox {
    /// Creates a ConstrainedBox.
    #[must_use]
    pub fn new(constraints: Constraints, child: impl Into<Widget>) -> Self {
        Self {
            constraints,
            child: child.into(),
        }
    }
}

impl From<ConstrainedBox> for Widget {
    fn from(value: ConstrainedBox) -> Self {
        Widget::from_kind(WidgetKind::Constrained {
            constraints: value.constraints,
            child: std::rc::Rc::new(value.child),
        })
    }
}

/// Applies maximum dimensions only when incoming constraints are unbounded.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct LimitedBox {
    #[builder(
        default = f32::INFINITY,
        setter(transform = |limit: f32| limit.max(0.0))
    )]
    max_width: f32,
    #[builder(
        default = f32::INFINITY,
        setter(transform = |limit: f32| limit.max(0.0))
    )]
    max_height: f32,
    #[builder(setter(into))]
    child: Widget,
}

impl LimitedBox {
    /// Creates a LimitedBox.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            max_width: f32::INFINITY,
            max_height: f32::INFINITY,
            child: child.into(),
        }
    }

    /// Sets the limit for unbounded width.
    #[must_use]
    pub fn max_width(mut self, max_width: f32) -> Self {
        self.max_width = max_width.max(0.0);
        self
    }

    /// Sets the limit for unbounded height.
    #[must_use]
    pub fn max_height(mut self, max_height: f32) -> Self {
        self.max_height = max_height.max(0.0);
        self
    }
}

impl From<LimitedBox> for Widget {
    fn from(value: LimitedBox) -> Self {
        Widget::from_kind(WidgetKind::Limited {
            max_width: value.max_width,
            max_height: value.max_height,
            child: std::rc::Rc::new(value.child),
        })
    }
}

/// Allows its child to overflow parent bounds while parent reports constrained size.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct OverflowBox {
    #[builder(default, setter(strip_option))]
    min_width: Option<f32>,
    #[builder(default, setter(strip_option))]
    max_width: Option<f32>,
    #[builder(default, setter(strip_option))]
    min_height: Option<f32>,
    #[builder(default, setter(strip_option))]
    max_height: Option<f32>,
    #[builder(setter(into))]
    child: Widget,
}

impl OverflowBox {
    /// Creates an OverflowBox.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn min_width(mut self, value: f32) -> Self {
        self.min_width = Some(value);
        self
    }

    #[must_use]
    pub fn max_width(mut self, value: f32) -> Self {
        self.max_width = Some(value);
        self
    }

    #[must_use]
    pub fn min_height(mut self, value: f32) -> Self {
        self.min_height = Some(value);
        self
    }

    #[must_use]
    pub fn max_height(mut self, value: f32) -> Self {
        self.max_height = Some(value);
        self
    }
}

impl From<OverflowBox> for Widget {
    fn from(value: OverflowBox) -> Self {
        Widget::from_kind(WidgetKind::Overflow {
            min_width: value.min_width,
            max_width: value.max_width,
            min_height: value.min_height,
            max_height: value.max_height,
            child: std::rc::Rc::new(value.child),
        })
    }
}

/// Allows its child to size naturally without parent constraints.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct UnconstrainedBox {
    #[builder(default, setter(strip_option))]
    constrained_axis: Option<Axis>,
    #[builder(setter(into))]
    child: Widget,
}

impl UnconstrainedBox {
    /// Creates an UnconstrainedBox.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            constrained_axis: None,
            child: child.into(),
        }
    }

    /// Retains constraints along a specific axis while loosening the orthogonal axis.
    #[must_use]
    pub fn constrained_axis(mut self, axis: Axis) -> Self {
        self.constrained_axis = Some(axis);
        self
    }
}

impl From<UnconstrainedBox> for Widget {
    fn from(value: UnconstrainedBox) -> Self {
        Widget::from_kind(WidgetKind::Unconstrained {
            constrained_axis: value.constrained_axis,
            child: std::rc::Rc::new(value.child),
        })
    }
}

/// Rebuilds a subtree when incoming layout constraints change.
#[derive(Clone)]
pub struct LayoutBuilder {
    builder: Rc<dyn Fn(Constraints) -> Widget>,
}

impl LayoutBuilder {
    /// Creates a LayoutBuilder with a constraint builder closure.
    #[must_use]
    pub fn new(builder: impl Fn(Constraints) -> Widget + 'static) -> Self {
        Self {
            builder: Rc::new(builder),
        }
    }
}

impl From<LayoutBuilder> for Widget {
    fn from(value: LayoutBuilder) -> Self {
        Widget::from_kind(WidgetKind::LayoutBuilder {
            builder: value.builder,
            environment: None,
            environment_boundary: false,
            revision: None,
        })
    }
}

/// Sizes its child to the child's intrinsic width.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct IntrinsicWidth {
    #[builder(default, setter(strip_option))]
    step_width: Option<f32>,
    #[builder(default, setter(strip_option))]
    step_height: Option<f32>,
    #[builder(setter(into))]
    child: Widget,
}

impl IntrinsicWidth {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            step_width: None,
            step_height: None,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn step_width(mut self, step: f32) -> Self {
        self.step_width = Some(step);
        self
    }

    #[must_use]
    pub fn step_height(mut self, step: f32) -> Self {
        self.step_height = Some(step);
        self
    }
}

impl From<IntrinsicWidth> for Widget {
    fn from(value: IntrinsicWidth) -> Self {
        UnconstrainedBox::new(value.child).into()
    }
}

/// Sizes its child to the child's intrinsic height.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct IntrinsicHeight {
    #[builder(setter(into))]
    child: Widget,
}

impl IntrinsicHeight {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl From<IntrinsicHeight> for Widget {
    fn from(value: IntrinsicHeight) -> Self {
        UnconstrainedBox::new(value.child).into()
    }
}

/// A box with specified size that allows its child to overflow with alignment.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct SizedOverflowBox {
    size: Size,
    #[builder(default = Alignment::CENTER)]
    alignment: Alignment,
    #[builder(setter(into))]
    child: Widget,
}

impl SizedOverflowBox {
    #[must_use]
    pub fn new(size: Size, child: impl Into<Widget>) -> Self {
        Self {
            size,
            alignment: Alignment::CENTER,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }
}

impl From<SizedOverflowBox> for Widget {
    fn from(value: SizedOverflowBox) -> Self {
        SizedBox::from_size(value.size)
            .child(Align::new(value.alignment, OverflowBox::new(value.child)))
            .into()
    }
}

/// Transforms incoming layout constraints before passing them to its child.
#[derive(Clone)]
pub struct ConstraintsTransformBox {
    transform: Rc<dyn Fn(Constraints) -> Constraints>,
    alignment: Alignment,
    clip_behavior: Clip,
    child: Widget,
}

impl ConstraintsTransformBox {
    #[must_use]
    pub fn new(
        transform: impl Fn(Constraints) -> Constraints + 'static,
        child: impl Into<Widget>,
    ) -> Self {
        Self {
            transform: Rc::new(transform),
            alignment: Alignment::CENTER,
            clip_behavior: Clip::None,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }

    #[must_use]
    pub fn clip_behavior(mut self, clip: Clip) -> Self {
        self.clip_behavior = clip;
        self
    }
}

impl From<ConstraintsTransformBox> for Widget {
    fn from(value: ConstraintsTransformBox) -> Self {
        let transform = value.transform;
        let child = value.child;
        let alignment = value.alignment;
        LayoutBuilder::new(move |incoming| {
            let child_constraints = transform(incoming);
            ConstrainedBox::new(child_constraints, Align::new(alignment, child.clone())).into()
        })
        .into()
    }
}

/// Informs parent of preferred size.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct PreferredSize {
    preferred_size: Size,
    #[builder(setter(into))]
    child: Widget,
}

impl PreferredSize {
    #[must_use]
    pub fn new(preferred_size: Size, child: impl Into<Widget>) -> Self {
        Self {
            preferred_size,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn preferred_size(&self) -> Size {
        self.preferred_size
    }
}

impl From<PreferredSize> for Widget {
    fn from(value: PreferredSize) -> Self {
        ConstrainedBox::new(
            Constraints::new(
                value.preferred_size.width,
                value.preferred_size.width,
                value.preferred_size.height,
                value.preferred_size.height,
            ),
            value.child,
        )
        .into()
    }
}
