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

use crate::tree::BoxFit;
use crate::{DecoratedBox, Widget, WidgetKind};

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
        if let Some(w_val) = w {
            if w_val.is_finite() {
                has_constraints = true;
                constraints = Constraints::new(
                    w_val.max(0.0),
                    w_val.max(0.0),
                    constraints.min_height,
                    constraints.max_height,
                );
            }
        }
        if let Some(h_val) = h {
            if h_val.is_finite() {
                has_constraints = true;
                constraints = Constraints::new(
                    constraints.min_width,
                    constraints.max_width,
                    h_val.max(0.0),
                    h_val.max(0.0),
                );
            }
        }
        if has_constraints {
            ConstrainedBox::new(constraints, child).into()
        } else {
            child
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

/// Scales and positions its child according to [`BoxFit`] and [`Alignment`].
#[derive(Clone, Debug, PartialEq)]
pub struct FittedBox {
    fit: BoxFit,
    alignment: Alignment,
    child: Widget,
}

impl FittedBox {
    /// Creates a FittedBox.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            fit: BoxFit::Contain,
            alignment: Alignment::CENTER,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn fit(mut self, fit: BoxFit) -> Self {
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
            environment: None,
            revision: None,
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

    /// Creates a CustomPaint widget from a custom painter.
    #[must_use]
    pub fn from_painter(size: Size, painter: impl crate::CustomPainter) -> Self {
        let display_list = painter.paint(size);
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

/// Sizes its child to the child's intrinsic width.
#[derive(Clone, Debug, PartialEq)]
pub struct IntrinsicWidth {
    step_width: Option<f32>,
    step_height: Option<f32>,
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
#[derive(Clone, Debug, PartialEq)]
pub struct IntrinsicHeight {
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
#[derive(Clone, Debug, PartialEq)]
pub struct SizedOverflowBox {
    size: Size,
    alignment: Alignment,
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

/// Rotates its child by an integral number of quarter turns (90 degrees each).
#[derive(Clone, Debug, PartialEq)]
pub struct RotatedBox {
    quarter_turns: i32,
    child: Widget,
}

impl RotatedBox {
    #[must_use]
    pub fn new(quarter_turns: i32, child: impl Into<Widget>) -> Self {
        Self {
            quarter_turns,
            child: child.into(),
        }
    }
}

impl From<RotatedBox> for Widget {
    fn from(value: RotatedBox) -> Self {
        let radians = (value.quarter_turns as f32) * std::f32::consts::FRAC_PI_2;
        crate::Transform::rotation(radians, value.child).into()
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

/// Wraps a child subtree with an explicit retained key.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyedSubtree {
    key: crate::tree::Key,
    child: Widget,
}

impl KeyedSubtree {
    #[must_use]
    pub fn new(key: impl Into<crate::tree::Key>, child: impl Into<Widget>) -> Self {
        Self {
            key: key.into(),
            child: child.into(),
        }
    }
}

impl From<KeyedSubtree> for Widget {
    fn from(value: KeyedSubtree) -> Self {
        let mut child = value.child;
        child.key = Some(value.key);
        child
    }
}

/// A placeholder box with cross lines and fallback sizing.
#[derive(Clone, Debug, PartialEq)]
pub struct Placeholder {
    color: Color,
    stroke_width: f32,
    fallback_width: f32,
    fallback_height: f32,
}

impl Default for Placeholder {
    fn default() -> Self {
        Self::new()
    }
}

impl Placeholder {
    #[must_use]
    pub fn new() -> Self {
        Self {
            color: Color::rgba(117, 117, 117, 255),
            stroke_width: 2.0,
            fallback_width: 400.0,
            fallback_height: 400.0,
        }
    }

    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    #[must_use]
    pub fn stroke_width(mut self, width: f32) -> Self {
        self.stroke_width = width.max(0.1);
        self
    }

    #[must_use]
    pub fn fallback_width(mut self, width: f32) -> Self {
        self.fallback_width = width.max(0.0);
        self
    }

    #[must_use]
    pub fn fallback_height(mut self, height: f32) -> Self {
        self.fallback_height = height.max(0.0);
        self
    }
}

impl From<Placeholder> for Widget {
    fn from(value: Placeholder) -> Self {
        SizedBox::new()
            .width(value.fallback_width)
            .height(value.fallback_height)
            .child(ColoredBox::new(
                Color::rgba(value.color.red, value.color.green, value.color.blue, 32),
                SizedBox::shrink(),
            ))
            .into()
    }
}

/// Informs parent of preferred size.
#[derive(Clone, Debug, PartialEq)]
pub struct PreferredSize {
    preferred_size: Size,
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

/// Translates its child by a fraction of the child's size.
#[derive(Clone, Debug, PartialEq)]
pub struct FractionalTranslation {
    translation: incular_core::Offset,
    transform_hit_tests: bool,
    child: Widget,
}

impl FractionalTranslation {
    #[must_use]
    pub fn new(translation: incular_core::Offset, child: impl Into<Widget>) -> Self {
        Self {
            translation,
            transform_hit_tests: true,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn transform_hit_tests(mut self, transform: bool) -> Self {
        self.transform_hit_tests = transform;
        self
    }
}

impl From<FractionalTranslation> for Widget {
    fn from(value: FractionalTranslation) -> Self {
        crate::Transform::translation(value.translation, value.child).into()
    }
}

/// A cell in a Table widget.
#[derive(Clone, Debug, PartialEq)]
pub struct TableCell {
    vertical_alignment: Option<incular_config::CrossAxisAlignment>,
    child: Widget,
}

impl TableCell {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            vertical_alignment: None,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn vertical_alignment(mut self, alignment: incular_config::CrossAxisAlignment) -> Self {
        self.vertical_alignment = Some(alignment);
        self
    }
}

impl From<TableCell> for Widget {
    fn from(value: TableCell) -> Self {
        value.child
    }
}

/// Lays out children in a row with spacing; wraps or flows when width is constrained.
#[derive(Clone, Debug, PartialEq)]
pub struct OverflowBar {
    spacing: f32,
    overflow_spacing: f32,
    overflow_alignment: incular_config::WrapCrossAlignment,
    children: Vec<Widget>,
}

impl OverflowBar {
    #[must_use]
    pub fn new(children: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        Self {
            spacing: 0.0,
            overflow_spacing: 0.0,
            overflow_alignment: incular_config::WrapCrossAlignment::Start,
            children: children.into_iter().map(Into::into).collect(),
        }
    }

    #[must_use]
    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing;
        self
    }

    #[must_use]
    pub fn overflow_spacing(mut self, spacing: f32) -> Self {
        self.overflow_spacing = spacing;
        self
    }

    #[must_use]
    pub fn overflow_alignment(mut self, alignment: incular_config::WrapCrossAlignment) -> Self {
        self.overflow_alignment = alignment;
        self
    }
}

impl From<OverflowBar> for Widget {
    fn from(value: OverflowBar) -> Self {
        crate::layout::Wrap::new(value.children)
            .spacing(value.spacing)
            .run_spacing(value.overflow_spacing)
            .cross_axis_alignment(value.overflow_alignment)
            .into()
    }
}

/// App bar navigation toolbar with leading, middle, and trailing slots.
#[derive(Clone, Debug, PartialEq)]
pub struct NavigationToolbar {
    leading: Option<Widget>,
    middle: Option<Widget>,
    trailing: Option<Widget>,
    center_middle: bool,
    middle_spacing: f32,
}

impl Default for NavigationToolbar {
    fn default() -> Self {
        Self::new()
    }
}

impl NavigationToolbar {
    #[must_use]
    pub fn new() -> Self {
        Self {
            leading: None,
            middle: None,
            trailing: None,
            center_middle: true,
            middle_spacing: 16.0,
        }
    }

    #[must_use]
    pub fn leading(mut self, widget: impl Into<Widget>) -> Self {
        self.leading = Some(widget.into());
        self
    }

    #[must_use]
    pub fn middle(mut self, widget: impl Into<Widget>) -> Self {
        self.middle = Some(widget.into());
        self
    }

    #[must_use]
    pub fn trailing(mut self, widget: impl Into<Widget>) -> Self {
        self.trailing = Some(widget.into());
        self
    }

    #[must_use]
    pub fn center_middle(mut self, center: bool) -> Self {
        self.center_middle = center;
        self
    }
}

impl From<NavigationToolbar> for Widget {
    fn from(value: NavigationToolbar) -> Self {
        let mut row_children = Vec::new();
        if let Some(leading) = value.leading {
            row_children.push(leading);
        }
        if let Some(middle) = value.middle {
            if value.center_middle {
                row_children.push(crate::layout::Expanded::new(Center::new(middle)).into());
            } else {
                row_children.push(middle);
            }
        } else if value.center_middle {
            row_children.push(crate::layout::Spacer::new().into());
        }
        if let Some(trailing) = value.trailing {
            row_children.push(trailing);
        }
        crate::layout::Row::new(row_children)
            .main_axis_alignment(incular_config::MainAxisAlignment::SpaceBetween)
            .cross_axis_alignment(incular_config::CrossAxisAlignment::Center)
            .into()
    }
}

/// An interactive viewer enabling panning and zooming on a child widget.
#[derive(Clone, Debug, PartialEq)]
pub struct InteractiveViewer {
    min_scale: f32,
    max_scale: f32,
    pan_enabled: bool,
    scale_enabled: bool,
    child: Widget,
}

impl InteractiveViewer {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            min_scale: 0.8,
            max_scale: 2.5,
            pan_enabled: true,
            scale_enabled: true,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn min_scale(mut self, scale: f32) -> Self {
        self.min_scale = scale.max(0.1);
        self
    }

    #[must_use]
    pub fn max_scale(mut self, scale: f32) -> Self {
        self.max_scale = scale.max(self.min_scale);
        self
    }

    #[must_use]
    pub fn pan_enabled(mut self, enabled: bool) -> Self {
        self.pan_enabled = enabled;
        self
    }

    #[must_use]
    pub fn scale_enabled(mut self, enabled: bool) -> Self {
        self.scale_enabled = enabled;
        self
    }
}

impl From<InteractiveViewer> for Widget {
    fn from(value: InteractiveViewer) -> Self {
        value.child
    }
}

/// Explicit positioning strategy for dual-pane splits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SplitPosition {
    /// Fixed extent for the first (left/top) pane from start.
    FromStart(f32),
    /// Fixed extent for the second (right/bottom) pane from end.
    FromEnd(f32),
    /// Proportional fraction (0.0 .. 1.0) between panes.
    Fraction(f32),
}

/// A resizable dual-pane container separated by an interactive divider.
#[derive(Clone)]
pub struct SplitView {
    axis: Axis,
    first: Widget,
    second: Widget,
    position: SplitPosition,
    min_first: f32,
    max_first: f32,
    min_second: f32,
    max_second: f32,
    divider_hit_extent: f32,
    divider_visual_extent: f32,
    divider_color: Option<Color>,
    on_split_changed: Option<std::rc::Rc<dyn Fn(f32)>>,
}

impl SplitView {
    #[must_use]
    pub fn horizontal(first: impl Into<Widget>, second: impl Into<Widget>) -> Self {
        Self {
            axis: Axis::Horizontal,
            first: first.into(),
            second: second.into(),
            position: SplitPosition::Fraction(0.5),
            min_first: 0.0,
            max_first: f32::INFINITY,
            min_second: 0.0,
            max_second: f32::INFINITY,
            divider_hit_extent: 8.0,
            divider_visual_extent: 1.0,
            divider_color: None,
            on_split_changed: None,
        }
    }

    #[must_use]
    pub fn vertical(first: impl Into<Widget>, second: impl Into<Widget>) -> Self {
        Self {
            axis: Axis::Vertical,
            first: first.into(),
            second: second.into(),
            position: SplitPosition::Fraction(0.5),
            min_first: 0.0,
            max_first: f32::INFINITY,
            min_second: 0.0,
            max_second: f32::INFINITY,
            divider_hit_extent: 8.0,
            divider_visual_extent: 1.0,
            divider_color: None,
            on_split_changed: None,
        }
    }

    /// Sets explicit fixed extent for the first pane.
    #[must_use]
    pub fn first_extent(mut self, extent: f32) -> Self {
        self.position = SplitPosition::FromStart(extent.max(0.0));
        self
    }

    /// Sets explicit fixed extent for the second pane.
    #[must_use]
    pub fn second_extent(mut self, extent: f32) -> Self {
        self.position = SplitPosition::FromEnd(extent.max(0.0));
        self
    }

    /// Sets explicit split positioning strategy.
    #[must_use]
    pub fn split_position(mut self, position: SplitPosition) -> Self {
        self.position = position;
        self
    }

    /// Convenience alias for `first_extent`.
    #[must_use]
    pub fn split_offset(self, offset: f32) -> Self {
        self.first_extent(offset)
    }

    /// Sets proportional split fraction between 0.0 and 1.0.
    #[must_use]
    pub fn split_fraction(self, fraction: f32) -> Self {
        self.split_position(SplitPosition::Fraction(fraction.clamp(0.0, 1.0)))
    }

    #[must_use]
    pub fn min_first(mut self, min: f32) -> Self {
        self.min_first = min.max(0.0);
        self
    }

    #[must_use]
    pub fn max_first(mut self, max: f32) -> Self {
        self.max_first = max.max(0.0);
        self
    }

    #[must_use]
    pub fn min_second(mut self, min: f32) -> Self {
        self.min_second = min.max(0.0);
        self
    }

    #[must_use]
    pub fn max_second(mut self, max: f32) -> Self {
        self.max_second = max.max(0.0);
        self
    }

    #[must_use]
    pub fn divider_thickness(mut self, thickness: f32) -> Self {
        self.divider_hit_extent = thickness.max(1.0);
        self.divider_visual_extent = thickness.max(0.0);
        self
    }

    #[must_use]
    pub fn divider_hit_extent(mut self, extent: f32) -> Self {
        self.divider_hit_extent = extent.max(1.0);
        self
    }

    #[must_use]
    pub fn divider_visual_extent(mut self, extent: f32) -> Self {
        self.divider_visual_extent = extent.max(0.0);
        self
    }

    #[must_use]
    pub fn divider_color(mut self, color: Color) -> Self {
        self.divider_color = Some(color);
        self
    }

    #[must_use]
    pub fn on_split_changed(mut self, callback: impl Fn(f32) + 'static) -> Self {
        self.on_split_changed = Some(std::rc::Rc::new(callback));
        self
    }
}

impl From<SplitView> for Widget {
    fn from(view: SplitView) -> Self {
        let callback = view.on_split_changed.clone();
        let axis = view.axis;
        let hit_thickness = view.divider_hit_extent.max(1.0);
        let visual_thickness = view.divider_visual_extent.min(hit_thickness).max(0.0);

        let divider_visual: Widget =
            if let Some(color) = view.divider_color.filter(|_| visual_thickness > 0.0) {
                crate::Container::new()
                    .width(if axis == Axis::Horizontal {
                        visual_thickness
                    } else {
                        0.0
                    })
                    .height(if axis == Axis::Vertical {
                        visual_thickness
                    } else {
                        0.0
                    })
                    .color(color)
                    .into()
            } else {
                SizedBox::new().into()
            };

        let divider = crate::GestureDetector::new(
            crate::Container::new()
                .width(if axis == Axis::Horizontal {
                    hit_thickness
                } else {
                    0.0
                })
                .height(if axis == Axis::Vertical {
                    hit_thickness
                } else {
                    0.0
                })
                .alignment(Alignment::CENTER)
                .child(divider_visual),
        )
        .on_pan_update(move |delta: incular_core::Offset| {
            if let Some(ref cb) = callback {
                cb(if axis == Axis::Horizontal {
                    delta.x
                } else {
                    delta.y
                });
            }
        });

        match view.position {
            SplitPosition::FromStart(extent) => {
                let clamped = extent.clamp(view.min_first, view.max_first);
                match axis {
                    Axis::Horizontal => {
                        let first = SizedBox::new().width(clamped).child(view.first);
                        let items: Vec<Widget> = vec![
                            first.into(),
                            divider.into(),
                            crate::Expanded::new(view.second).into(),
                        ];
                        crate::Row::new(items).into()
                    }
                    Axis::Vertical => {
                        let first = SizedBox::new().height(clamped).child(view.first);
                        let items: Vec<Widget> = vec![
                            first.into(),
                            divider.into(),
                            crate::Expanded::new(view.second).into(),
                        ];
                        crate::Column::new(items).into()
                    }
                }
            }
            SplitPosition::FromEnd(extent) => {
                let clamped = extent.clamp(view.min_second, view.max_second);
                match axis {
                    Axis::Horizontal => {
                        let second = SizedBox::new().width(clamped).child(view.second);
                        let items: Vec<Widget> = vec![
                            crate::Expanded::new(view.first).into(),
                            divider.into(),
                            second.into(),
                        ];
                        crate::Row::new(items).into()
                    }
                    Axis::Vertical => {
                        let second = SizedBox::new().height(clamped).child(view.second);
                        let items: Vec<Widget> = vec![
                            crate::Expanded::new(view.first).into(),
                            divider.into(),
                            second.into(),
                        ];
                        crate::Column::new(items).into()
                    }
                }
            }
            SplitPosition::Fraction(frac) => {
                let f1 = ((frac * 1000.0) as u32).max(1);
                let f2 = (((1.0 - frac) * 1000.0) as u32).max(1);
                match axis {
                    Axis::Horizontal => {
                        let items: Vec<Widget> = vec![
                            crate::Flexible::new(view.first).flex(f1).into(),
                            divider.into(),
                            crate::Flexible::new(view.second).flex(f2).into(),
                        ];
                        crate::Row::new(items).into()
                    }
                    Axis::Vertical => {
                        let items: Vec<Widget> = vec![
                            crate::Flexible::new(view.first).flex(f1).into(),
                            divider.into(),
                            crate::Flexible::new(view.second).flex(f2).into(),
                        ];
                        crate::Column::new(items).into()
                    }
                }
            }
        }
    }
}
