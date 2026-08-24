//! Canonical Flutter-style basic layout widget descriptors.
//!
//! Provides [`Padding`], [`Align`], [`Center`], [`SizedBox`], [`ColoredBox`],
//! [`ConstrainedBox`], [`LimitedBox`], [`OverflowBox`], [`UnconstrainedBox`],
//! [`FractionallySizedBox`], [`Baseline`], [`AspectRatio`], [`FittedBox`],
//! [`Visibility`], [`Offstage`], [`LayoutBuilder`], [`RepaintBoundary`], [`CustomPaint`],
//! and clipping widgets ([`ClipRect`], [`ClipRRect`], [`ClipOval`], [`ClipPath`]).

use std::rc::Rc;
use std::sync::Arc;

use incular_config::{Alignment, Axis, Clip, Constraints, EdgeInsets};
use incular_core::{Color, Size};
use incular_rendering::{CornerRadii, DisplayList, Path};

use crate::{DecoratedBox, ImageFit, Widget, WidgetKind};

/// Insets its child by given [`EdgeInsets`].
#[derive(Clone, Debug, PartialEq)]
pub struct Padding {
    padding: EdgeInsets,
    child: Widget,
}

impl Padding {
    /// Creates a padding widget wrapping a child with the given insets.
    #[must_use]
    pub fn new(padding: impl Into<EdgeInsets>, child: impl Into<Widget>) -> Self {
        Self {
            padding: padding.into(),
            child: child.into(),
        }
    }

    /// Convenience constructor with uniform padding on all edges.
    #[must_use]
    pub fn all(value: f32, child: impl Into<Widget>) -> Self {
        Self::new(EdgeInsets::all(value), child)
    }

    /// Convenience constructor with symmetric padding.
    #[must_use]
    pub fn symmetric(horizontal: f32, vertical: f32, child: impl Into<Widget>) -> Self {
        Self::new(EdgeInsets::symmetric(horizontal, vertical), child)
    }

    /// Convenience constructor with zero padding.
    #[must_use]
    pub fn zero(child: impl Into<Widget>) -> Self {
        Self::new(EdgeInsets::ZERO, child)
    }
}

impl From<Padding> for Widget {
    fn from(value: Padding) -> Self {
        Widget::from_kind(WidgetKind::Padding {
            padding: value.padding,
            child: Box::new(value.child),
        })
    }
}

/// Positions a child inside itself with fractional alignment and optional size factors.
#[derive(Clone, Debug, PartialEq)]
pub struct Align {
    alignment: Alignment,
    width_factor: Option<f32>,
    height_factor: Option<f32>,
    child: Widget,
}

impl Align {
    /// Creates an align widget with explicit alignment and a child.
    #[must_use]
    pub fn new(alignment: Alignment, child: impl Into<Widget>) -> Self {
        Self {
            alignment,
            width_factor: None,
            height_factor: None,
            child: child.into(),
        }
    }

    /// Sets the width scaling factor relative to child width.
    #[must_use]
    pub fn width_factor(mut self, factor: f32) -> Self {
        self.width_factor = Some(factor.max(0.0));
        self
    }

    /// Sets the height scaling factor relative to child height.
    #[must_use]
    pub fn height_factor(mut self, factor: f32) -> Self {
        self.height_factor = Some(factor.max(0.0));
        self
    }

    #[must_use]
    pub fn get_alignment(&self) -> Alignment {
        self.alignment
    }
}

impl From<Align> for Widget {
    fn from(value: Align) -> Self {
        Widget::from_kind(WidgetKind::Align {
            alignment: value.alignment,
            width_factor: value.width_factor,
            height_factor: value.height_factor,
            child: Box::new(value.child),
        })
    }
}

/// Centers its child within itself.
#[derive(Clone, Debug, PartialEq)]
pub struct Center {
    width_factor: Option<f32>,
    height_factor: Option<f32>,
    child: Widget,
}

impl Center {
    /// Creates a Center widget with a child.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            width_factor: None,
            height_factor: None,
            child: child.into(),
        }
    }

    /// Sets the width scaling factor relative to child width.
    #[must_use]
    pub fn width_factor(mut self, factor: f32) -> Self {
        self.width_factor = Some(factor.max(0.0));
        self
    }

    /// Sets the height scaling factor relative to child height.
    #[must_use]
    pub fn height_factor(mut self, factor: f32) -> Self {
        self.height_factor = Some(factor.max(0.0));
        self
    }
}

impl From<Center> for Widget {
    fn from(value: Center) -> Self {
        let mut align = Align::new(Alignment::CENTER, value.child);
        if let Some(wf) = value.width_factor {
            align = align.width_factor(wf);
        }
        if let Some(hf) = value.height_factor {
            align = align.height_factor(hf);
        }
        align.into()
    }
}

/// A box with a specified size.
#[derive(Clone, Debug, PartialEq)]
pub struct SizedBox {
    width: Option<f32>,
    height: Option<f32>,
    child: Option<Widget>,
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
        if let Some(child) = value.child {
            let mut constraints = Constraints::unbounded();
            if let Some(w_val) = w {
                if w_val.is_finite() {
                    constraints = Constraints::new(
                        w_val,
                        w_val,
                        constraints.min_height,
                        constraints.max_height,
                    );
                } else {
                    constraints = Constraints::new(
                        f32::INFINITY,
                        f32::INFINITY,
                        constraints.min_height,
                        constraints.max_height,
                    );
                }
            }
            if let Some(h_val) = h {
                if h_val.is_finite() {
                    constraints = Constraints::new(
                        constraints.min_width,
                        constraints.max_width,
                        h_val,
                        h_val,
                    );
                } else {
                    constraints = Constraints::new(
                        constraints.min_width,
                        constraints.max_width,
                        f32::INFINITY,
                        f32::INFINITY,
                    );
                }
            }
            ConstrainedBox::new(constraints, child).into()
        } else {
            let w_val = w.unwrap_or(0.0);
            let h_val = h.unwrap_or(0.0);
            Widget::from_kind(WidgetKind::Box {
                size: Size::new(
                    if w_val.is_finite() {
                        w_val.max(0.0)
                    } else {
                        0.0
                    },
                    if h_val.is_finite() {
                        h_val.max(0.0)
                    } else {
                        0.0
                    },
                ),
                color: Color::TRANSPARENT,
            })
        }
    }
}

/// Paints a solid background color behind a child widget.
#[derive(Clone, Debug, PartialEq)]
pub struct ColoredBox {
    color: Color,
    child: Option<Widget>,
}

impl ColoredBox {
    /// Creates a ColoredBox wrapping a child with a background color.
    #[must_use]
    pub fn new(color: Color, child: impl Into<Widget>) -> Self {
        Self {
            color,
            child: Some(child.into()),
        }
    }

    /// Sets the background color.
    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// Sets the child widget.
    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self
    }
}

impl From<ColoredBox> for Widget {
    fn from(value: ColoredBox) -> Self {
        let child = value.child.unwrap_or_else(|| SizedBox::shrink().into());
        DecoratedBox::new(child).background(value.color).into()
    }
}

/// Imposes additional layout constraints on its child.
#[derive(Clone, Debug, PartialEq)]
pub struct ConstrainedBox {
    constraints: Constraints,
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
            child: Box::new(value.child),
        })
    }
}

/// Applies maximum dimensions only when incoming constraints are unbounded.
#[derive(Clone, Debug, PartialEq)]
pub struct LimitedBox {
    max_width: f32,
    max_height: f32,
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
            child: Box::new(value.child),
        })
    }
}

/// Allows its child to overflow parent bounds while parent reports constrained size.
#[derive(Clone, Debug, PartialEq)]
pub struct OverflowBox {
    min_width: Option<f32>,
    max_width: Option<f32>,
    min_height: Option<f32>,
    max_height: Option<f32>,
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
            child: Box::new(value.child),
        })
    }
}

/// Allows its child to size naturally without parent constraints.
#[derive(Clone, Debug, PartialEq)]
pub struct UnconstrainedBox {
    constrained_axis: Option<Axis>,
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
            child: Box::new(value.child),
        })
    }
}

/// Sizes child as a fraction of incoming parent constraints.
#[derive(Clone, Debug, PartialEq)]
pub struct FractionallySizedBox {
    width_factor: Option<f32>,
    height_factor: Option<f32>,
    child: Widget,
}

impl FractionallySizedBox {
    /// Creates a FractionallySizedBox.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            width_factor: None,
            height_factor: None,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn width_factor(mut self, factor: f32) -> Self {
        self.width_factor = Some(factor.max(0.0));
        self
    }

    #[must_use]
    pub fn height_factor(mut self, factor: f32) -> Self {
        self.height_factor = Some(factor.max(0.0));
        self
    }
}

impl From<FractionallySizedBox> for Widget {
    fn from(value: FractionallySizedBox) -> Self {
        Widget::from_kind(WidgetKind::Fractional {
            width_factor: value.width_factor,
            height_factor: value.height_factor,
            child: Box::new(value.child),
        })
    }
}

/// Positions child baseline at an explicit vertical offset.
#[derive(Clone, Debug, PartialEq)]
pub struct Baseline {
    baseline: f32,
    child: Widget,
}

impl Baseline {
    /// Creates a Baseline widget.
    #[must_use]
    pub fn new(baseline: f32, child: impl Into<Widget>) -> Self {
        Self {
            baseline: baseline.max(0.0),
            child: child.into(),
        }
    }
}

impl From<Baseline> for Widget {
    fn from(value: Baseline) -> Self {
        Widget::from_kind(WidgetKind::Baseline {
            baseline: value.baseline,
            child: Box::new(value.child),
        })
    }
}

/// Sizes its child to match a specified aspect ratio.
#[derive(Clone, Debug, PartialEq)]
pub struct AspectRatio {
    aspect_ratio: f32,
    child: Widget,
}

impl AspectRatio {
    /// Creates an AspectRatio widget.
    #[must_use]
    pub fn new(aspect_ratio: f32, child: impl Into<Widget>) -> Self {
        Self {
            aspect_ratio: aspect_ratio.max(f32::EPSILON),
            child: child.into(),
        }
    }
}

impl From<AspectRatio> for Widget {
    fn from(value: AspectRatio) -> Self {
        Widget::from_kind(WidgetKind::AspectRatio {
            ratio: value.aspect_ratio,
            child: Box::new(value.child),
        })
    }
}

/// Scales and positions its child according to [`ImageFit`] and [`Alignment`].
#[derive(Clone, Debug, PartialEq)]
pub struct FittedBox {
    fit: ImageFit,
    alignment: Alignment,
    child: Widget,
}

impl FittedBox {
    /// Creates a FittedBox.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            fit: ImageFit::Contain,
            alignment: Alignment::CENTER,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn fit(mut self, fit: ImageFit) -> Self {
        self.fit = fit;
        self
    }

    #[must_use]
    pub fn alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }
}

impl From<FittedBox> for Widget {
    fn from(value: FittedBox) -> Self {
        Widget::fitted_box(value.fit, value.alignment, value.child)
    }
}

/// Conditionally displays a child or hides it from layout, paint, hit-testing, and semantics.
#[derive(Clone, Debug, PartialEq)]
pub struct Visibility {
    visible: bool,
    maintain_state: bool,
    maintain_size: bool,
    maintain_animation: bool,
    maintain_semantics: bool,
    replacement: Option<Widget>,
    child: Widget,
}

impl Visibility {
    /// Creates a Visibility wrapper.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            visible: true,
            maintain_state: false,
            maintain_size: false,
            maintain_animation: false,
            maintain_semantics: false,
            replacement: None,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    #[must_use]
    pub fn maintain_state(mut self, maintain: bool) -> Self {
        self.maintain_state = maintain;
        self
    }

    #[must_use]
    pub fn maintain_size(mut self, maintain: bool) -> Self {
        self.maintain_size = maintain;
        self
    }

    #[must_use]
    pub fn replacement(mut self, replacement: impl Into<Widget>) -> Self {
        self.replacement = Some(replacement.into());
        self
    }
}

impl From<Visibility> for Widget {
    fn from(value: Visibility) -> Self {
        if !value.visible && !value.maintain_state && !value.maintain_size {
            value
                .replacement
                .unwrap_or_else(|| SizedBox::shrink().into())
        } else {
            Widget::from_kind(WidgetKind::Visibility {
                visible: value.visible,
                child: Box::new(value.child),
            })
        }
    }
}

/// Hides its child offstage without unmounting the retained element.
#[derive(Clone, Debug, PartialEq)]
pub struct Offstage {
    offstage: bool,
    child: Widget,
}

impl Offstage {
    /// Creates an Offstage widget.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            offstage: true,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn offstage(mut self, offstage: bool) -> Self {
        self.offstage = offstage;
        self
    }
}

impl From<Offstage> for Widget {
    fn from(value: Offstage) -> Self {
        Visibility::new(value.child)
            .visible(!value.offstage)
            .maintain_state(true)
            .into()
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
        })
    }
}

/// Isolates display list painting and caching into a dedicated layer.
#[derive(Clone, Debug, PartialEq)]
pub struct RepaintBoundary {
    child: Widget,
}

impl RepaintBoundary {
    /// Creates a RepaintBoundary.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl From<RepaintBoundary> for Widget {
    fn from(value: RepaintBoundary) -> Self {
        Widget::from_kind(WidgetKind::RepaintBoundary {
            child: Box::new(value.child),
        })
    }
}

/// Paints an explicit [`DisplayList`] with a fixed size.
#[derive(Clone, Debug, PartialEq)]
pub struct CustomPaint {
    size: Size,
    display_list: DisplayList,
}

impl CustomPaint {
    /// Creates a CustomPaint widget.
    #[must_use]
    pub fn new(size: Size, display_list: DisplayList) -> Self {
        Self { size, display_list }
    }
}

impl From<CustomPaint> for Widget {
    fn from(value: CustomPaint) -> Self {
        Widget::from_kind(WidgetKind::CustomPaint {
            size: value.size,
            display_list: value.display_list,
        })
    }
}

/// Clips its child using a rectangular boundary.
#[derive(Clone, Debug, PartialEq)]
pub struct ClipRect {
    clip_behavior: Clip,
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
            child: Box::new(value.child),
        })
    }
}

/// Clips its child using a rounded-rectangular boundary.
#[derive(Clone, Debug, PartialEq)]
pub struct ClipRRect {
    radius: CornerRadii,
    clip_behavior: Clip,
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
            child: Box::new(value.child),
        })
    }
}

/// Clips its child using an elliptical / oval boundary.
#[derive(Clone, Debug, PartialEq)]
pub struct ClipOval {
    clip_behavior: Clip,
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
            child: Box::new(value.child),
        })
    }
}

/// Clips its child using an arbitrary [`Path`] boundary.
#[derive(Clone, Debug, PartialEq)]
pub struct ClipPath {
    path: Arc<Path>,
    clip_behavior: Clip,
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
            child: Box::new(value.child),
        })
    }
}
