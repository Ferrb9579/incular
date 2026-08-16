//! Deterministic layout algorithms over measured child sizes.
//!
//! The functions in this module do not know what a child widget is. A caller
//! supplies a `Size` (and, where relevant, flex/position metadata), receives a
//! [`LayoutResult`], and applies the returned offsets to its own tree.

use crate::{
    Align, Alignment, AspectRatio, Axis, Constraints, CrossAxisAlignment, EdgeInsets, Flex,
    FlexFit, MainAxisAlignment, MainAxisSize, Positioned, Stack, StackFit, Table, TextDirection,
    VerticalDirection, Wrap, WrapAlignment, WrapCrossAlignment,
};
use crate::{ChildLayout, LayoutResult, Offset, Size};
use std::borrow::Borrow;

/// Measured child input for [`layout_flex`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlexChild {
    pub size: Size,
    pub flex: u32,
    pub fit: FlexFit,
    pub baseline: Option<f32>,
}

impl FlexChild {
    #[must_use]
    pub const fn new(size: Size) -> Self {
        Self {
            size,
            flex: 0,
            fit: FlexFit::Loose,
            baseline: None,
        }
    }

    #[must_use]
    pub const fn flexible(size: Size, flex: u32, fit: FlexFit) -> Self {
        Self {
            size,
            flex,
            fit,
            baseline: None,
        }
    }

    #[must_use]
    pub const fn expanded(size: Size, flex: u32) -> Self {
        Self::flexible(size, flex, FlexFit::Tight)
    }

    #[must_use]
    pub const fn with_baseline(mut self, baseline: f32) -> Self {
        self.baseline = Some(baseline);
        self
    }
}

impl From<Size> for FlexChild {
    fn from(size: Size) -> Self {
        Self::new(size)
    }
}

/// Optional baseline metadata for a child accepted by alignment algorithms.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AlignmentChild {
    pub size: Size,
    pub baseline: Option<f32>,
}

impl AlignmentChild {
    #[must_use]
    pub const fn new(size: Size) -> Self {
        Self {
            size,
            baseline: None,
        }
    }

    #[must_use]
    pub const fn with_baseline(mut self, baseline: f32) -> Self {
        self.baseline = Some(baseline);
        self
    }
}

impl From<Size> for AlignmentChild {
    fn from(size: Size) -> Self {
        Self::new(size)
    }
}

/// Child metadata used by [`layout_baseline`].
pub type BaselineChild = AlignmentChild;

/// A positioned or non-positioned stack child.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StackChild {
    pub size: Size,
    pub position: StackPosition,
    pub baseline: Option<f32>,
}

impl StackChild {
    #[must_use]
    pub const fn new(size: Size) -> Self {
        Self {
            size,
            position: StackPosition::NonPositioned,
            baseline: None,
        }
    }

    #[must_use]
    pub const fn positioned(size: Size, position: Positioned) -> Self {
        Self {
            size,
            position: StackPosition::Positioned(position),
            baseline: None,
        }
    }

    #[must_use]
    pub const fn with_baseline(mut self, baseline: f32) -> Self {
        self.baseline = Some(baseline);
        self
    }
}

impl From<Size> for StackChild {
    fn from(size: Size) -> Self {
        Self::new(size)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StackPosition {
    NonPositioned,
    Positioned(Positioned),
}

/// Measured child input for [`layout_wrap`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WrapChild {
    pub size: Size,
    pub baseline: Option<f32>,
}

impl WrapChild {
    #[must_use]
    pub const fn new(size: Size) -> Self {
        Self {
            size,
            baseline: None,
        }
    }
}

impl From<Size> for WrapChild {
    fn from(size: Size) -> Self {
        Self::new(size)
    }
}

/// Measured child input for [`layout_table`]. Children are consumed in
/// row-major order; a table descriptor determines the number of columns.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TableChild {
    pub size: Size,
    pub baseline: Option<f32>,
}

impl TableChild {
    #[must_use]
    pub const fn new(size: Size) -> Self {
        Self {
            size,
            baseline: None,
        }
    }
}

impl From<Size> for TableChild {
    fn from(size: Size) -> Self {
        Self::new(size)
    }
}

/// Layouts a flex line using Flutter-style flex, alignment, and spacing
/// semantics. The child order is preserved in the result, including when the
/// main axis is reversed by text/vertical direction.
#[must_use]
pub fn layout_flex<C>(constraints: Constraints, children: &[FlexChild], config: C) -> LayoutResult
where
    C: Borrow<Flex>,
{
    let config = config.borrow();
    let direction = config.direction;
    let spacing = clean_nonnegative(config.spacing);
    let child_count = children.len();
    let fixed_main: f32 = children
        .iter()
        .filter(|child| child.flex == 0)
        .map(|child| direction.main_extent(child.size))
        .sum();
    let fixed_cross: f32 = children
        .iter()
        .map(|child| direction.cross_extent(child.size))
        .fold(0.0, f32::max);
    let fixed_count = children.iter().filter(|child| child.flex == 0).count();
    let fixed_with_spacing = fixed_main + spacing * child_count.saturating_sub(1) as f32;
    let max_main = main_max(constraints, direction);
    let min_main = main_min(constraints, direction);
    let target_main = if matches!(config.main_axis_size, MainAxisSize::Max) && max_main.is_finite()
    {
        max_main
    } else {
        fixed_with_spacing.max(min_main)
    };
    let flex_units: u32 = children
        .iter()
        .filter(|child| child.flex > 0)
        .map(|child| child.flex)
        .sum();
    let free_for_flex = (target_main - fixed_with_spacing).max(0.0);

    let mut sizes = Vec::with_capacity(child_count);
    for child in children {
        let main = if child.flex == 0 || flex_units == 0 {
            direction.main_extent(child.size)
        } else {
            let share = free_for_flex * child.flex as f32 / flex_units as f32;
            match child.fit {
                FlexFit::Tight => share,
                FlexFit::Loose => direction.main_extent(child.size).min(share),
            }
        };
        let cross = direction.cross_extent(child.size);
        sizes.push((main.max(0.0), cross.max(0.0)));
    }

    // A loose flex line is content-sized; a max-sized line is constrained by
    // the incoming maximum. In both cases the final value remains valid.
    let used_main: f32 = sizes.iter().map(|(main, _)| *main).sum::<f32>()
        + spacing * child_count.saturating_sub(1) as f32;
    let line_main = if matches!(config.main_axis_size, MainAxisSize::Max) && max_main.is_finite() {
        max_main
    } else {
        used_main.max(min_main)
    };
    let natural_cross = sizes.iter().map(|(_, cross)| *cross).fold(0.0, f32::max);
    let line_cross = cross_constrain(
        constraints,
        direction,
        if matches!(config.cross_axis_alignment, CrossAxisAlignment::Stretch)
            && cross_max(constraints, direction).is_finite()
        {
            cross_max(constraints, direction)
        } else {
            natural_cross
        },
    );

    let remaining = (line_main - used_main).max(0.0);
    let (leading, extra_gap) =
        main_distribution(config.main_axis_alignment, remaining, child_count);
    let reverse = match direction {
        Axis::Horizontal => matches!(config.text_direction, TextDirection::Rtl),
        Axis::Vertical => matches!(config.vertical_direction, VerticalDirection::Up),
    };
    let mut offsets = vec![ChildLayout::default(); child_count];
    let mut cursor = leading;
    let baseline = if matches!(config.cross_axis_alignment, CrossAxisAlignment::Baseline) {
        children
            .iter()
            .zip(sizes.iter())
            .filter_map(|(child, (_, cross))| child.baseline.map(|value| value.min(*cross)))
            .fold(0.0, f32::max)
    } else {
        0.0
    };

    for visual_index in 0..child_count {
        let index = if reverse {
            child_count - 1 - visual_index
        } else {
            visual_index
        };
        let (main, natural_cross) = sizes[index];
        let mut child_cross = natural_cross;
        let cross_offset = match config.cross_axis_alignment {
            CrossAxisAlignment::Start => 0.0,
            CrossAxisAlignment::End => (line_cross - child_cross).max(0.0),
            CrossAxisAlignment::Center => (line_cross - child_cross) * 0.5,
            CrossAxisAlignment::Stretch => {
                child_cross = line_cross;
                0.0
            }
            CrossAxisAlignment::Baseline => {
                let child_baseline = children[index].baseline.unwrap_or(child_cross);
                (baseline - child_baseline).max(0.0)
            }
        };
        let child_size = direction.size(main, child_cross);
        offsets[index] = ChildLayout::new(direction.offset(cursor, cross_offset), child_size)
            .with_optional_baseline(children[index].baseline);
        cursor += main + spacing + extra_gap;
    }

    let _ = fixed_count; // Kept named for diagnostics/readability above.
    let _ = fixed_cross;
    LayoutResult::new(direction.size(line_main, line_cross), offsets)
}

/// Positions a measured child according to an [`Align`] descriptor.
#[must_use]
pub fn layout_align<C>(constraints: Constraints, child: AlignmentChild, config: C) -> LayoutResult
where
    C: Borrow<Align>,
{
    let config = config.borrow();
    let width = config
        .width_factor
        .and_then(|factor| {
            finite_factor(factor).and_then(|factor| safe_mul(child.size.width, factor))
        })
        .unwrap_or_else(|| {
            if constraints.is_width_bounded() {
                constraints.max_width
            } else {
                child.size.width
            }
        });
    let height = config
        .height_factor
        .and_then(|factor| {
            finite_factor(factor).and_then(|factor| safe_mul(child.size.height, factor))
        })
        .unwrap_or_else(|| {
            if constraints.is_height_bounded() {
                constraints.max_height
            } else {
                child.size.height
            }
        });
    let size = constraints.constrain(Size::new(width.max(0.0), height.max(0.0)));
    let offset = config.alignment.within(size, child.size);
    LayoutResult::new(
        size,
        vec![ChildLayout::new(offset, child.size).with_optional_baseline(child.baseline)],
    )
}

/// Centers a measured child in the incoming constraints.
#[must_use]
pub fn layout_center(constraints: Constraints, child: AlignmentChild) -> LayoutResult {
    layout_align(constraints, child, Align::new(Alignment::CENTER))
}

/// Adds physical padding around a measured child.
#[must_use]
pub fn layout_padding<I>(constraints: Constraints, child: Size, insets: I) -> LayoutResult
where
    I: Borrow<EdgeInsets>,
{
    let insets = insets.borrow().normalized();
    let desired = Size::new(
        safe_add(child.width, insets.horizontal()),
        safe_add(child.height, insets.vertical()),
    );
    let size = constraints.constrain(desired);
    let offset = Offset::new(
        insets.left.min((size.width - child.width).max(0.0)),
        insets.top.min((size.height - child.height).max(0.0)),
    );
    LayoutResult::new(size, vec![ChildLayout::new(offset, child)])
}

/// Applies optional width/height sizing while preserving the measured child.
#[must_use]
pub fn layout_sized_box<S>(constraints: Constraints, child: Size, config: S) -> LayoutResult
where
    S: Borrow<crate::SizedBox>,
{
    let config = config.borrow();
    let width = config
        .width
        .filter(|value| value.is_finite())
        .unwrap_or(child.width)
        .max(0.0);
    let height = config
        .height
        .filter(|value| value.is_finite())
        .unwrap_or(child.height)
        .max(0.0);
    let size = constraints.constrain(Size::new(width, height));
    LayoutResult::new(size, vec![ChildLayout::new(Offset::ZERO, child)])
}

/// Intersects a parent constraint with an additional `ConstrainedBox` range.
#[must_use]
pub fn layout_constrained_box<C>(constraints: Constraints, child: Size, config: C) -> LayoutResult
where
    C: Borrow<crate::ConstrainedBox>,
{
    let effective = enforce_constraints(constraints, config.borrow().constraints);
    let size = effective.constrain(child);
    LayoutResult::new(size, vec![ChildLayout::new(Offset::ZERO, child)])
}

/// Lays out a child with unconstrained axes, then places it within the parent.
#[must_use]
pub fn layout_unconstrained_box<C>(constraints: Constraints, child: Size, config: C) -> LayoutResult
where
    C: Borrow<crate::UnconstrainedBox>,
{
    let config = config.borrow();
    let loosened = match config.constrained_axis {
        None => Constraints::unbounded(),
        Some(Axis::Horizontal) => Constraints::new(
            constraints.min_width,
            constraints.max_width,
            0.0,
            constraints.max_height,
        ),
        Some(Axis::Vertical) => Constraints::new(
            0.0,
            constraints.max_width,
            constraints.min_height,
            constraints.max_height,
        ),
    };
    let child = loosened.constrain(child);
    let size = constraints.constrain(child);
    LayoutResult::new(
        size,
        vec![ChildLayout::new(
            config.alignment.within(size, child),
            child,
        )],
    )
}

/// Fits a child to an aspect ratio while respecting both incoming bounds.
#[must_use]
pub fn layout_aspect_ratio<A>(constraints: Constraints, child: Size, config: A) -> LayoutResult
where
    A: Borrow<AspectRatio>,
{
    let ratio = config.borrow().aspect_ratio;
    let ratio = if ratio.is_finite() && ratio > 0.0 {
        ratio
    } else {
        1.0
    };
    let width = constraints.max_width;
    let height = constraints.max_height;
    let (mut target_width, mut target_height) = if width.is_finite() && height.is_finite() {
        let by_width_height = width / ratio;
        if by_width_height.is_finite() && by_width_height <= height {
            (width, by_width_height)
        } else {
            (safe_mul(height, ratio).unwrap_or(child.width), height)
        }
    } else if width.is_finite() {
        (width, width / ratio)
    } else if height.is_finite() {
        (safe_mul(height, ratio).unwrap_or(child.width), height)
    } else if child.height > 0.0 {
        (
            safe_mul(child.height, ratio).unwrap_or(child.width),
            child.height,
        )
    } else {
        (child.width.max(0.0), child.width.max(0.0) / ratio)
    };
    if !target_width.is_finite() || !target_height.is_finite() {
        target_width = child.width;
        target_height = child.height;
    }
    target_width = target_width.max(constraints.min_width);
    target_height = target_height.max(constraints.min_height);
    let size = constraints.constrain(Size::new(target_width, target_height));
    LayoutResult::new(size, vec![ChildLayout::new(Offset::ZERO, child)])
}

/// Sizes a box as a fraction of its parent's available bounded dimensions.
#[must_use]
pub fn layout_fractionally_sized<F>(
    constraints: Constraints,
    child: Size,
    config: F,
) -> LayoutResult
where
    F: Borrow<crate::FractionallySizedBox>,
{
    let config = config.borrow();
    let width = config
        .width_factor
        .and_then(finite_factor)
        .map(|factor| constraints.max_width * factor)
        .filter(|value| value.is_finite())
        .unwrap_or(child.width);
    let height = config
        .height_factor
        .and_then(finite_factor)
        .map(|factor| constraints.max_height * factor)
        .filter(|value| value.is_finite())
        .unwrap_or(child.height);
    let size = constraints.constrain(Size::new(width.max(0.0), height.max(0.0)));
    LayoutResult::new(
        size,
        vec![ChildLayout::new(
            config.alignment.within(size, child),
            child,
        )],
    )
}

/// Places a measured child at a requested baseline.
#[must_use]
pub fn layout_baseline(
    constraints: Constraints,
    child: BaselineChild,
    baseline: f32,
) -> LayoutResult {
    let target = if baseline.is_finite() {
        baseline.max(0.0)
    } else {
        0.0
    };
    let child_baseline = child.baseline.unwrap_or(child.size.height).max(0.0);
    let offset_y = (target - child_baseline).max(0.0);
    let desired = Size::new(child.size.width, safe_add(child.size.height, offset_y));
    let size = constraints.constrain(desired);
    LayoutResult::new(
        size,
        vec![
            ChildLayout::new(Offset::new(0.0, offset_y), child.size)
                .with_optional_baseline(child.baseline),
        ],
    )
}

/// Lays out a stack with non-positioned children defining its intrinsic size
/// and positioned children resolved against the resulting box.
#[must_use]
pub fn layout_stack<S>(constraints: Constraints, children: &[StackChild], config: S) -> LayoutResult
where
    S: Borrow<Stack>,
{
    let config = config.borrow();
    let mut intrinsic_width: f32 = 0.0;
    let mut intrinsic_height: f32 = 0.0;
    for child in children {
        if matches!(child.position, StackPosition::NonPositioned) {
            intrinsic_width = intrinsic_width.max(child.size.width);
            intrinsic_height = intrinsic_height.max(child.size.height);
        }
    }
    let desired = if matches!(config.fit, StackFit::Expand) {
        Size::new(
            if constraints.max_width.is_finite() {
                constraints.max_width
            } else {
                intrinsic_width
            },
            if constraints.max_height.is_finite() {
                constraints.max_height
            } else {
                intrinsic_height
            },
        )
    } else {
        Size::new(intrinsic_width, intrinsic_height)
    };
    let stack_size = constraints.constrain(desired);
    let mut result = Vec::with_capacity(children.len());
    for child in children {
        let (size, offset) = match child.position {
            StackPosition::NonPositioned => {
                (child.size, config.alignment.within(stack_size, child.size))
            }
            StackPosition::Positioned(position) => {
                positioned_geometry(stack_size, child.size, position)
            }
        };
        result.push(ChildLayout::new(offset, size).with_optional_baseline(child.baseline));
    }
    LayoutResult::new(stack_size, result)
}

/// A line-breaking wrap layout. Children remain in input order; line and item
/// alignment are applied inside the constrained main/cross extents.
#[must_use]
pub fn layout_wrap<W>(constraints: Constraints, children: &[WrapChild], config: W) -> LayoutResult
where
    W: Borrow<Wrap>,
{
    let config = config.borrow();
    let direction = config.direction;
    let spacing = clean_nonnegative(config.spacing);
    let run_spacing = clean_nonnegative(config.run_spacing);
    let available_main = main_max(constraints, direction);
    let mut runs: Vec<WrapRun> = Vec::new();
    let mut current = WrapRun::default();
    for (index, child) in children.iter().enumerate() {
        let extent = direction.main_extent(child.size);
        let next = if current.indices.is_empty() {
            extent
        } else {
            current.main + spacing + extent
        };
        if !current.indices.is_empty() && available_main.is_finite() && next > available_main {
            runs.push(current);
            current = WrapRun::default();
        }
        current.main = if current.indices.is_empty() {
            extent
        } else {
            current.main + spacing + extent
        };
        current.cross = current.cross.max(direction.cross_extent(child.size));
        current.indices.push(index);
    }
    if !current.indices.is_empty() {
        runs.push(current);
    }

    let natural_main = runs.iter().map(|run| run.main).fold(0.0, f32::max);
    let natural_cross = runs.iter().map(|run| run.cross).sum::<f32>()
        + run_spacing * runs.len().saturating_sub(1) as f32;
    let line_main = if available_main.is_finite() {
        available_main
    } else {
        main_min(constraints, direction).max(natural_main)
    };
    let cross_maximum = cross_max(constraints, direction);
    let line_cross = if cross_maximum.is_finite() {
        cross_maximum
    } else {
        cross_min(constraints, direction).max(natural_cross)
    };
    let mut output = vec![ChildLayout::default(); children.len()];
    let cross_remaining = (line_cross - natural_cross).max(0.0);
    let (run_leading, run_extra) =
        wrap_distribution(config.run_alignment, cross_remaining, runs.len());
    let reverse_main = match direction {
        Axis::Horizontal => matches!(config.text_direction, TextDirection::Rtl),
        Axis::Vertical => matches!(config.vertical_direction, VerticalDirection::Up),
    };
    let reverse_cross = matches!(config.vertical_direction, VerticalDirection::Up)
        && matches!(direction, Axis::Horizontal);
    let mut cross_cursor = run_leading;
    for run_index in 0..runs.len() {
        let visual_run = if reverse_cross {
            runs.len() - 1 - run_index
        } else {
            run_index
        };
        let run = &runs[visual_run];
        let remaining = (line_main - run.main).max(0.0);
        let (leading, extra) = wrap_distribution(config.alignment, remaining, run.indices.len());
        let mut main_cursor = leading;
        for visual_item in 0..run.indices.len() {
            let item_position = if reverse_main {
                run.indices.len() - 1 - visual_item
            } else {
                visual_item
            };
            let index = run.indices[item_position];
            let child = children[index];
            let child_cross = direction.cross_extent(child.size);
            let cross_offset = match config.cross_axis_alignment {
                WrapCrossAlignment::Start => 0.0,
                WrapCrossAlignment::End => (run.cross - child_cross).max(0.0),
                WrapCrossAlignment::Center => (run.cross - child_cross) * 0.5,
            };
            output[index] = ChildLayout::new(
                direction.offset(main_cursor, cross_cursor + cross_offset),
                child.size,
            )
            .with_optional_baseline(child.baseline);
            main_cursor += direction.main_extent(child.size) + spacing + extra;
        }
        cross_cursor += run.cross + run_spacing + run_extra;
    }
    LayoutResult::new(direction.size(line_main, line_cross), output)
}

/// Lays out row-major table cells using max-content column widths and row
/// heights. If `columns` is zero, it is treated as one to keep the result
/// deterministic and avoid an invalid division.
#[must_use]
pub fn layout_table<T>(constraints: Constraints, children: &[TableChild], config: T) -> LayoutResult
where
    T: Borrow<Table>,
{
    let config = config.borrow();
    let columns = config.columns.max(1);
    let row_count = children.len().div_ceil(columns);
    if children.is_empty() {
        return LayoutResult::empty(constraints.constrain(Size::ZERO));
    }
    let mut widths: Vec<f32> = vec![0.0; columns];
    let mut heights: Vec<f32> = vec![0.0; row_count];
    for (index, child) in children.iter().enumerate() {
        widths[index % columns] = widths[index % columns].max(child.size.width);
        heights[index / columns] = heights[index / columns].max(child.size.height);
    }
    let column_spacing = clean_nonnegative(config.column_spacing);
    let row_spacing = clean_nonnegative(config.row_spacing);
    let natural_width =
        widths.iter().sum::<f32>() + column_spacing * columns.saturating_sub(1) as f32;
    let natural_height =
        heights.iter().sum::<f32>() + row_spacing * row_count.saturating_sub(1) as f32;
    let size = constraints.constrain(Size::new(natural_width, natural_height));
    let mut output = Vec::with_capacity(children.len());
    let mut y = 0.0;
    for (row, row_height) in heights.iter().enumerate() {
        let mut x = 0.0;
        for (column, column_width) in widths.iter().enumerate() {
            let index = row * columns + column;
            let Some(child) = children.get(index).copied() else {
                break;
            };
            output.push(
                ChildLayout::new(Offset::new(x, y), child.size)
                    .with_optional_baseline(child.baseline),
            );
            x += *column_width + column_spacing;
        }
        y += *row_height + row_spacing;
    }
    LayoutResult::new(size, output)
}

#[derive(Clone, Debug, Default)]
struct WrapRun {
    indices: Vec<usize>,
    main: f32,
    cross: f32,
}

fn positioned_geometry(parent: Size, child: Size, position: Positioned) -> (Size, Offset) {
    let left = finite_nonnegative(position.left);
    let right = finite_nonnegative(position.right);
    let top = finite_nonnegative(position.top);
    let bottom = finite_nonnegative(position.bottom);
    let width = if let (Some(left), Some(right)) = (left, right) {
        (parent.width - left - right).max(0.0)
    } else {
        finite_nonnegative(position.width).unwrap_or(child.width)
    };
    let height = if let (Some(top), Some(bottom)) = (top, bottom) {
        (parent.height - top - bottom).max(0.0)
    } else {
        finite_nonnegative(position.height).unwrap_or(child.height)
    };
    let x = left.unwrap_or_else(|| {
        right
            .map(|right| (parent.width - right - width).max(0.0))
            .unwrap_or(0.0)
    });
    let y = top.unwrap_or_else(|| {
        bottom
            .map(|bottom| (parent.height - bottom - height).max(0.0))
            .unwrap_or(0.0)
    });
    (Size::new(width, height), Offset::new(x, y))
}

fn main_distribution(alignment: MainAxisAlignment, remaining: f32, count: usize) -> (f32, f32) {
    if count == 0 {
        return (0.0, 0.0);
    }
    match alignment {
        MainAxisAlignment::Start => (0.0, 0.0),
        MainAxisAlignment::End => (remaining, 0.0),
        MainAxisAlignment::Center => (remaining * 0.5, 0.0),
        MainAxisAlignment::SpaceBetween if count > 1 => (0.0, remaining / (count - 1) as f32),
        MainAxisAlignment::SpaceAround => {
            let gap = remaining / count as f32;
            (gap * 0.5, gap)
        }
        MainAxisAlignment::SpaceEvenly => {
            let gap = remaining / (count + 1) as f32;
            (gap, gap)
        }
        MainAxisAlignment::SpaceBetween => (0.0, 0.0),
    }
}

fn wrap_distribution(alignment: WrapAlignment, remaining: f32, count: usize) -> (f32, f32) {
    if count == 0 {
        return (0.0, 0.0);
    }
    match alignment {
        WrapAlignment::Start => (0.0, 0.0),
        WrapAlignment::End => (remaining, 0.0),
        WrapAlignment::Center => (remaining * 0.5, 0.0),
        WrapAlignment::SpaceBetween if count > 1 => (0.0, remaining / (count - 1) as f32),
        WrapAlignment::SpaceAround => {
            let gap = remaining / count as f32;
            (gap * 0.5, gap)
        }
        WrapAlignment::SpaceEvenly => {
            let gap = remaining / (count + 1) as f32;
            (gap, gap)
        }
        WrapAlignment::SpaceBetween => (0.0, 0.0),
    }
}

fn finite_factor(value: f32) -> Option<f32> {
    (value.is_finite() && value >= 0.0).then_some(value)
}

fn finite_nonnegative(value: Option<f32>) -> Option<f32> {
    value.and_then(finite_factor)
}

fn clean_nonnegative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

fn safe_add(left: f32, right: f32) -> f32 {
    let value = left + right;
    if value.is_finite() { value } else { f32::MAX }
}

fn safe_mul(left: f32, right: f32) -> Option<f32> {
    let value = left * right;
    value.is_finite().then_some(value.max(0.0))
}

fn main_min(constraints: Constraints, axis: Axis) -> f32 {
    match axis {
        Axis::Horizontal => constraints.min_width,
        Axis::Vertical => constraints.min_height,
    }
}

fn main_max(constraints: Constraints, axis: Axis) -> f32 {
    match axis {
        Axis::Horizontal => constraints.max_width,
        Axis::Vertical => constraints.max_height,
    }
}

fn cross_min(constraints: Constraints, axis: Axis) -> f32 {
    match axis {
        Axis::Horizontal => constraints.min_height,
        Axis::Vertical => constraints.min_width,
    }
}

fn cross_max(constraints: Constraints, axis: Axis) -> f32 {
    match axis {
        Axis::Horizontal => constraints.max_height,
        Axis::Vertical => constraints.max_width,
    }
}

fn cross_constrain(constraints: Constraints, axis: Axis, value: f32) -> f32 {
    match axis {
        Axis::Horizontal => value.clamp(constraints.min_height, constraints.max_height),
        Axis::Vertical => value.clamp(constraints.min_width, constraints.max_width),
    }
}

fn enforce_constraints(a: Constraints, b: Constraints) -> Constraints {
    let min_width = a.min_width.max(b.min_width);
    let max_width = a.max_width.min(b.max_width);
    let min_height = a.min_height.max(b.min_height);
    let max_height = a.max_height.min(b.max_height);
    Constraints::new(
        min_width,
        max_width.max(min_width),
        min_height,
        max_height.max(min_height),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MainAxisAlignment, MainAxisSize};

    #[test]
    fn flex_distributes_tight_children_and_preserves_order() {
        let config = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_main_axis_alignment(MainAxisAlignment::Start)
            .with_cross_axis_alignment(CrossAxisAlignment::Start);
        let result = layout_flex(
            Constraints::tight(Size::new(100.0, 20.0)),
            &[
                FlexChild::new(Size::new(10.0, 5.0)),
                FlexChild::expanded(Size::new(1.0, 5.0), 1),
            ],
            config,
        );
        assert_eq!(result.size, Size::new(100.0, 20.0));
        assert_eq!(result.children[0].offset, Offset::ZERO);
        assert_eq!(result.children[1].offset.x, 10.0);
        assert_eq!(result.children[1].size.width, 90.0);
    }

    #[test]
    fn padding_and_alignment_have_expected_geometry() {
        let padded = layout_padding(
            Constraints::unbounded(),
            Size::new(10.0, 8.0),
            EdgeInsets::all(2.0),
        );
        assert_eq!(padded.size, Size::new(14.0, 12.0));
        let aligned = layout_align(
            Constraints::tight(Size::new(100.0, 50.0)),
            AlignmentChild::new(Size::new(20.0, 10.0)),
            Align::new(Alignment::CENTER),
        );
        assert_eq!(aligned.children[0].offset, Offset::new(40.0, 20.0));
    }

    #[test]
    fn wrap_breaks_lines_at_the_main_axis_bound() {
        let result = layout_wrap(
            Constraints::new(25.0, 25.0, 0.0, f32::INFINITY),
            &[
                WrapChild::new(Size::new(10.0, 4.0)),
                WrapChild::new(Size::new(10.0, 5.0)),
                WrapChild::new(Size::new(10.0, 6.0)),
            ],
            Wrap::default(),
        );
        assert_eq!(result.children[0].offset, Offset::ZERO);
        assert_eq!(result.children[2].offset.y, 5.0);
    }

    #[test]
    fn table_uses_max_content_columns() {
        let result = layout_table(
            Constraints::unbounded(),
            &[
                TableChild::new(Size::new(20.0, 4.0)),
                TableChild::new(Size::new(10.0, 5.0)),
                TableChild::new(Size::new(5.0, 3.0)),
            ],
            Table::new(2),
        );
        assert_eq!(result.size, Size::new(30.0, 8.0));
        assert_eq!(result.children[2].offset, Offset::new(0.0, 5.0));
    }
}
