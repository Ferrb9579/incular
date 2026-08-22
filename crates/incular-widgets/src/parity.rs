//! Flutter-style widget descriptors built on Incular's retained [`Widget`]
//! tree.  These types deliberately compose the small renderer-native set
//! instead of mirroring Flutter's inheritance hierarchy.

use incular_config::{Alignment, Axis, Constraints, EdgeInsets, FlexFit};
use incular_core::{Color, Size};

use crate::{DecoratedBox, Widget};

/// A horizontal flex container.
#[derive(Clone, Default)]
pub struct Row {
    children: Vec<Widget>,
}
impl Row {
    #[must_use]
    pub fn new(children: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        Self {
            children: children.into_iter().map(Into::into).collect(),
        }
    }
    #[must_use]
    pub fn children(mut self, children: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        self.children = children.into_iter().map(Into::into).collect();
        self
    }
}
impl From<Row> for Widget {
    fn from(value: Row) -> Self {
        Widget::row(value.children)
    }
}

/// A vertical flex container.
#[derive(Clone, Default)]
pub struct Column {
    children: Vec<Widget>,
}
impl Column {
    #[must_use]
    pub fn new(children: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        Self {
            children: children.into_iter().map(Into::into).collect(),
        }
    }
    #[must_use]
    pub fn children(mut self, children: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        self.children = children.into_iter().map(Into::into).collect();
        self
    }
}
impl From<Column> for Widget {
    fn from(value: Column) -> Self {
        Widget::column(value.children)
    }
}

/// A direction-configurable flex container.
#[derive(Clone)]
pub struct Flex {
    direction: Axis,
    children: Vec<Widget>,
}
impl Flex {
    #[must_use]
    pub fn new(direction: Axis, children: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        Self {
            direction,
            children: children.into_iter().map(Into::into).collect(),
        }
    }
    #[must_use]
    pub fn horizontal(children: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        Self::new(Axis::Horizontal, children)
    }
    #[must_use]
    pub fn vertical(children: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        Self::new(Axis::Vertical, children)
    }
}
impl From<Flex> for Widget {
    fn from(value: Flex) -> Self {
        match value.direction {
            Axis::Horizontal => Widget::row(value.children),
            Axis::Vertical => Widget::column(value.children),
        }
    }
}

/// Allocates a proportional share of a bounded `Row` or `Column` main axis.
#[derive(Clone)]
pub struct Flexible {
    flex: u32,
    fit: FlexFit,
    child: Widget,
}
impl Flexible {
    #[must_use]
    pub fn new(flex: u32, child: impl Into<Widget>) -> Self {
        Self {
            flex: flex.max(1),
            fit: FlexFit::Loose,
            child: child.into(),
        }
    }
    #[must_use]
    pub fn tight(mut self) -> Self {
        self.fit = FlexFit::Tight;
        self
    }
    #[must_use]
    pub fn fit(mut self, fit: FlexFit) -> Self {
        self.fit = fit;
        self
    }
}
impl From<Flexible> for Widget {
    fn from(value: Flexible) -> Self {
        Widget::flexible(value.flex, value.fit, value.child)
    }
}

/// A tight flexible child. `Spacer` is `Expanded` around a zero-size box.
#[derive(Clone)]
pub struct Expanded {
    flex: u32,
    child: Widget,
}
impl Expanded {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            flex: 1,
            child: child.into(),
        }
    }
    #[must_use]
    pub fn flex(mut self, flex: u32) -> Self {
        self.flex = flex.max(1);
        self
    }
}
impl From<Expanded> for Widget {
    fn from(value: Expanded) -> Self {
        Widget::flexible(value.flex, FlexFit::Tight, value.child)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spacer {
    flex: u32,
}
impl Spacer {
    #[must_use]
    pub fn new() -> Self {
        Self { flex: 1 }
    }
    #[must_use]
    pub fn flex(mut self, flex: u32) -> Self {
        self.flex = flex.max(1);
        self
    }
}
impl Default for Spacer {
    fn default() -> Self {
        Self::new()
    }
}
impl From<Spacer> for Widget {
    fn from(value: Spacer) -> Self {
        Widget::flexible(
            value.flex,
            FlexFit::Tight,
            Widget::fixed_box(Size::ZERO, Color::TRANSPARENT),
        )
    }
}

/// Packs children into multiple runs whenever the main-axis bound is reached.
#[derive(Clone)]
pub struct Wrap {
    direction: Axis,
    spacing: f32,
    run_spacing: f32,
    children: Vec<Widget>,
}
impl Wrap {
    #[must_use]
    pub fn new(children: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        Self {
            direction: Axis::Horizontal,
            spacing: 0.,
            run_spacing: 0.,
            children: children.into_iter().map(Into::into).collect(),
        }
    }
    #[must_use]
    pub fn direction(mut self, direction: Axis) -> Self {
        self.direction = direction;
        self
    }
    #[must_use]
    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing;
        self
    }
    #[must_use]
    pub fn run_spacing(mut self, run_spacing: f32) -> Self {
        self.run_spacing = run_spacing;
        self
    }
}
impl From<Wrap> for Widget {
    fn from(value: Wrap) -> Self {
        Widget::wrap(
            value.direction,
            value.spacing,
            value.run_spacing,
            value.children,
        )
    }
}

/// A row-major max-content table.
#[derive(Clone)]
pub struct Table {
    columns: usize,
    column_spacing: f32,
    row_spacing: f32,
    children: Vec<Widget>,
}
impl Table {
    #[must_use]
    pub fn new(columns: usize, children: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        Self {
            columns: columns.max(1),
            column_spacing: 0.,
            row_spacing: 0.,
            children: children.into_iter().map(Into::into).collect(),
        }
    }
    #[must_use]
    pub fn column_spacing(mut self, spacing: f32) -> Self {
        self.column_spacing = spacing;
        self
    }
    #[must_use]
    pub fn row_spacing(mut self, spacing: f32) -> Self {
        self.row_spacing = spacing;
        self
    }
}
impl From<Table> for Widget {
    fn from(value: Table) -> Self {
        Widget::table(
            value.columns,
            value.column_spacing,
            value.row_spacing,
            value.children,
        )
    }
}

/// Overlays children in paint order. The last child is front-most for hit
/// testing; non-positioned children use the supplied alignment.
#[derive(Clone)]
pub struct Stack {
    alignment: Alignment,
    children: Vec<Widget>,
}
impl Stack {
    #[must_use]
    pub fn new(children: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        Self::aligned(Alignment::TOP_LEFT, children)
    }
    #[must_use]
    pub fn aligned(
        alignment: Alignment,
        children: impl IntoIterator<Item = impl Into<Widget>>,
    ) -> Self {
        Self {
            alignment,
            children: children.into_iter().map(Into::into).collect(),
        }
    }
    #[must_use]
    pub fn alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }
}
impl From<Stack> for Widget {
    fn from(value: Stack) -> Self {
        Widget::stack(value.alignment, value.children)
    }
}

/// Pins one child to supplied stack edges or explicit dimensions.
#[derive(Clone)]
pub struct Positioned {
    left: Option<f32>,
    top: Option<f32>,
    right: Option<f32>,
    bottom: Option<f32>,
    width: Option<f32>,
    height: Option<f32>,
    child: Widget,
}
impl Positioned {
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
    pub fn left(mut self, value: f32) -> Self {
        self.left = Some(value);
        self
    }
    #[must_use]
    pub fn top(mut self, value: f32) -> Self {
        self.top = Some(value);
        self
    }
    #[must_use]
    pub fn right(mut self, value: f32) -> Self {
        self.right = Some(value);
        self
    }
    #[must_use]
    pub fn bottom(mut self, value: f32) -> Self {
        self.bottom = Some(value);
        self
    }
    #[must_use]
    pub fn width(mut self, value: f32) -> Self {
        self.width = Some(value);
        self
    }
    #[must_use]
    pub fn height(mut self, value: f32) -> Self {
        self.height = Some(value);
        self
    }
}
impl From<Positioned> for Widget {
    fn from(value: Positioned) -> Self {
        Widget::positioned(
            value.left,
            value.top,
            value.right,
            value.bottom,
            value.width,
            value.height,
            value.child,
        )
    }
}

#[derive(Clone)]
pub struct IndexedStack {
    alignment: Alignment,
    index: usize,
    children: Vec<Widget>,
}
impl IndexedStack {
    #[must_use]
    pub fn new(children: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        Self {
            alignment: Alignment::TOP_LEFT,
            index: 0,
            children: children.into_iter().map(Into::into).collect(),
        }
    }
    #[must_use]
    pub fn alignment(mut self, value: Alignment) -> Self {
        self.alignment = value;
        self
    }
    #[must_use]
    pub fn index(mut self, value: usize) -> Self {
        self.index = value;
        self
    }
}
impl From<IndexedStack> for Widget {
    fn from(value: IndexedStack) -> Self {
        Widget::indexed_stack(value.alignment, value.index, value.children)
    }
}

pub struct LayoutBuilder {
    builder: Box<dyn Fn(Constraints) -> Widget>,
}
impl LayoutBuilder {
    #[must_use]
    pub fn new(builder: impl Fn(Constraints) -> Widget + 'static) -> Self {
        Self {
            builder: Box::new(builder),
        }
    }
}
impl From<LayoutBuilder> for Widget {
    fn from(value: LayoutBuilder) -> Self {
        Widget::layout_builder(value.builder)
    }
}

/// Insets around one child.
#[derive(Clone)]
pub struct Padding {
    padding: EdgeInsets,
    child: Widget,
}
impl Padding {
    #[must_use]
    pub fn new(padding: EdgeInsets, child: impl Into<Widget>) -> Self {
        Self {
            padding,
            child: child.into(),
        }
    }
    #[must_use]
    pub fn all(value: f32, child: impl Into<Widget>) -> Self {
        Self::new(EdgeInsets::all(value), child)
    }
}
impl From<Padding> for Widget {
    fn from(value: Padding) -> Self {
        Widget::padding(value.padding, value.child)
    }
}

/// Positions one child inside its available box.
#[derive(Clone)]
pub struct Align {
    alignment: Alignment,
    child: Widget,
}
impl Align {
    #[must_use]
    pub fn new(alignment: Alignment, child: impl Into<Widget>) -> Self {
        Self {
            alignment,
            child: child.into(),
        }
    }
}
impl From<Align> for Widget {
    fn from(value: Align) -> Self {
        Widget::align(value.alignment, value.child)
    }
}

/// Centers one child inside its available box.
#[derive(Clone)]
pub struct Center {
    child: Widget,
}
impl Center {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}
impl From<Center> for Widget {
    fn from(value: Center) -> Self {
        Widget::align(Alignment::CENTER, value.child)
    }
}

/// Gives a child an explicit layout size.
#[derive(Clone)]
pub struct SizedBox {
    size: Size,
    child: Widget,
}
impl SizedBox {
    #[must_use]
    pub fn new(size: Size, child: impl Into<Widget>) -> Self {
        Self {
            size,
            child: child.into(),
        }
    }
    #[must_use]
    pub fn square(side: f32, child: impl Into<Widget>) -> Self {
        Self::new(Size::new(side, side), child)
    }
}
impl From<SizedBox> for Widget {
    fn from(value: SizedBox) -> Self {
        DecoratedBox::new(value.child).size(value.size).into()
    }
}

/// A fixed-size colored box, useful as a leaf in examples and tests.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColoredBox {
    size: Size,
    color: Color,
}
impl ColoredBox {
    #[must_use]
    pub fn new(size: Size, color: Color) -> Self {
        Self { size, color }
    }
}
impl From<ColoredBox> for Widget {
    fn from(value: ColoredBox) -> Self {
        Widget::fixed_box(value.size, value.color)
    }
}

/// Layout constraints supplied to a child.  The retained renderer's
/// constraint propagation is exposed through this descriptor for APIs that
/// need to store a constraints value before building their child.
#[derive(Clone)]
pub struct ConstrainedBox {
    constraints: Constraints,
    child: Widget,
}
impl ConstrainedBox {
    #[must_use]
    pub fn new(constraints: Constraints, child: impl Into<Widget>) -> Self {
        Self {
            constraints,
            child: child.into(),
        }
    }
    #[must_use]
    pub fn constraints(&self) -> Constraints {
        self.constraints
    }
}
impl From<ConstrainedBox> for Widget {
    fn from(value: ConstrainedBox) -> Self {
        Widget::constrained(value.constraints, value.child)
    }
}

/// Applies maximum dimensions only where the incoming axis is unbounded.
#[derive(Clone)]
pub struct LimitedBox {
    max_width: f32,
    max_height: f32,
    child: Widget,
}
impl LimitedBox {
    #[must_use]
    pub fn new(max_width: f32, max_height: f32, child: impl Into<Widget>) -> Self {
        Self {
            max_width,
            max_height,
            child: child.into(),
        }
    }
}
impl From<LimitedBox> for Widget {
    fn from(value: LimitedBox) -> Self {
        Widget::limited_box(value.max_width, value.max_height, value.child)
    }
}

/// Lets a child receive independently configured constraints while this
/// wrapper itself continues reporting a size constrained by its parent.
#[derive(Clone)]
pub struct OverflowBox {
    min_width: Option<f32>,
    max_width: Option<f32>,
    min_height: Option<f32>,
    max_height: Option<f32>,
    child: Widget,
}
impl OverflowBox {
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
        Widget::overflow_box(
            value.min_width,
            value.max_width,
            value.min_height,
            value.max_height,
            value.child,
        )
    }
}

/// Lets a child use its natural size while the wrapper itself remains bounded
/// by its parent. An optional axis retains parent constraints on that axis.
#[derive(Clone)]
pub struct UnconstrainedBox {
    constrained_axis: Option<Axis>,
    child: Widget,
}
impl UnconstrainedBox {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            constrained_axis: None,
            child: child.into(),
        }
    }
    #[must_use]
    pub fn constrained_axis(mut self, axis: Axis) -> Self {
        self.constrained_axis = Some(axis);
        self
    }
}
impl From<UnconstrainedBox> for Widget {
    fn from(value: UnconstrainedBox) -> Self {
        Widget::unconstrained(value.constrained_axis, value.child)
    }
}

/// Sizes a child as a fraction of its finite parent bounds.
#[derive(Clone)]
pub struct FractionallySizedBox {
    width_factor: Option<f32>,
    height_factor: Option<f32>,
    child: Widget,
}
impl FractionallySizedBox {
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
        self.width_factor = Some(factor);
        self
    }
    #[must_use]
    pub fn height_factor(mut self, factor: f32) -> Self {
        self.height_factor = Some(factor);
        self
    }
}
impl From<FractionallySizedBox> for Widget {
    fn from(value: FractionallySizedBox) -> Self {
        Widget::fractionally_sized(value.width_factor, value.height_factor, value.child)
    }
}

/// Positions a child's reported baseline at a fixed offset from the top.
#[derive(Clone)]
pub struct Baseline {
    baseline: f32,
    child: Widget,
}
impl Baseline {
    #[must_use]
    pub fn new(baseline: f32, child: impl Into<Widget>) -> Self {
        Self {
            baseline,
            child: child.into(),
        }
    }
}
impl From<Baseline> for Widget {
    fn from(value: Baseline) -> Self {
        Widget::baseline(value.baseline, value.child)
    }
}

/// Conditionally includes a child in layout, paint, hit testing, and
/// semantics. A hidden child remains a lightweight declarative value and can
/// be shown again by rebuilding with `visible(true)`.
#[derive(Clone)]
pub struct Visibility {
    visible: bool,
    child: Widget,
}
impl Visibility {
    #[must_use]
    pub fn new(visible: bool, child: impl Into<Widget>) -> Self {
        Self {
            visible,
            child: child.into(),
        }
    }
    #[must_use]
    pub fn visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }
}
impl From<Visibility> for Widget {
    fn from(value: Visibility) -> Self {
        Widget::visibility(value.visible, value.child)
    }
}

/// An offstage child is the `Visibility(false, child)` convenience spelling.
#[derive(Clone)]
pub struct Offstage {
    child: Widget,
}
impl Offstage {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}
impl From<Offstage> for Widget {
    fn from(value: Offstage) -> Self {
        Widget::visibility(false, value.child)
    }
}

/// Sizes a child to the largest box with the requested width/height ratio
/// that fits the incoming retained-tree constraints.
#[derive(Clone)]
pub struct AspectRatio {
    ratio: f32,
    child: Widget,
}
impl AspectRatio {
    #[must_use]
    pub fn new(ratio: f32, child: impl Into<Widget>) -> Self {
        assert!(
            ratio.is_finite() && ratio > 0.,
            "aspect ratio must be finite and positive"
        );
        Self {
            ratio,
            child: child.into(),
        }
    }
}
impl From<AspectRatio> for Widget {
    fn from(value: AspectRatio) -> Self {
        Widget::aspect_ratio(value.ratio, value.child)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_and_column_build_retained_widgets() {
        let child = ColoredBox::new(Size::new(10., 10.), Color::WHITE);
        let _row: Widget = Row::new([child]).into();
        let _column: Widget = Column::new([child]).into();
    }

    #[test]
    fn alignment_wrappers_convert_to_widgets() {
        let child = Widget::fixed_box(Size::new(1., 1.), Color::WHITE);
        let _: Widget = Center::new(child.clone()).into();
        let _: Widget = Padding::all(4., child).into();
    }

    #[test]
    fn stack_builds_a_retained_container() {
        let child = ColoredBox::new(Size::new(1., 1.), Color::WHITE);
        let _: Widget = Stack::new([child]).into();
    }

    #[test]
    fn wrap_builds_a_retained_container() {
        let child = ColoredBox::new(Size::new(1., 1.), Color::WHITE);
        let _: Widget = Wrap::new([child]).spacing(2.).run_spacing(3.).into();
    }

    #[test]
    fn table_builds_a_retained_container() {
        let child = ColoredBox::new(Size::new(1., 1.), Color::WHITE);
        let _: Widget = Table::new(2, [child, child]).into();
    }

    #[test]
    fn baseline_builds_a_retained_container() {
        let _: Widget = Baseline::new(4., ColoredBox::new(Size::new(1., 1.), Color::WHITE)).into();
    }
}
