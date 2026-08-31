//! Constraint and viewport geometry shared by widget lowering and retained layout.

use super::super::*;

/// Converts the controller's logical offset into the physical content offset
/// used by a viewport transform.  A reversed viewport keeps the controller's
/// public range in the same `0..max` coordinate system, while its leading edge
/// is the content's physical trailing edge.
#[inline]
pub(in crate::tree) fn physical_scroll_offset(controller: &ScrollController, reverse: bool) -> f32 {
    if reverse {
        (controller.max_offset() - controller.offset()).max(0.)
    } else {
        controller.offset()
    }
}

#[inline]
pub(in crate::tree) fn scroll_translation(
    controller: &ScrollController,
    axis: Axis,
    reverse: bool,
) -> Offset {
    let offset = physical_scroll_offset(controller, reverse);
    axis.offset(-offset, 0.)
}

#[inline]
pub(in crate::tree) fn scroll_viewport_extent(axis: Axis, size: Size) -> f32 {
    axis.main_extent(size)
}

#[inline]
pub(in crate::tree) fn scroll_delta_for_axis(axis: Axis, delta: Offset) -> f32 {
    match axis {
        Axis::Horizontal => delta.x,
        Axis::Vertical => delta.y,
    }
}

#[inline]
pub(in crate::tree) fn scroll_constraints(axis: Axis, constraints: Constraints) -> Constraints {
    match axis {
        Axis::Vertical => Constraints::new(0., constraints.max_width, 0., f32::INFINITY),
        Axis::Horizontal => Constraints::new(0., f32::INFINITY, 0., constraints.max_height),
    }
}

#[inline]
pub(in crate::tree) fn scroll_size(axis: Axis, constraints: Constraints, content: Size) -> Size {
    let main = if axis.main_extent(content).is_finite()
        && ((axis.is_horizontal() && constraints.is_width_bounded())
            || (axis.is_vertical() && constraints.is_height_bounded()))
    {
        axis.main_extent(constraints.biggest())
    } else {
        axis.main_extent(content)
    };
    let cross = if (axis.is_horizontal() && constraints.is_height_bounded())
        || (axis.is_vertical() && constraints.is_width_bounded())
    {
        axis.cross_extent(constraints.biggest())
    } else {
        axis.cross_extent(content)
    };
    constraints.constrain(axis.size(main, cross))
}

#[inline]
pub(in crate::tree) fn sliver_viewport_size(
    axis: Axis,
    constraints: Constraints,
    shrink_wrap: bool,
) -> Size {
    let biggest = constraints.biggest();
    let main = if shrink_wrap
        && ((axis.is_vertical() && !constraints.is_height_bounded())
            || (axis.is_horizontal() && !constraints.is_width_bounded()))
    {
        0.
    } else {
        axis.main_extent(biggest)
    };
    let cross = axis.cross_extent(biggest);
    constraints.constrain(axis.size(main, cross))
}

/// Selects the first retained sliver child at or after the physical viewport
/// origin. Keeping its logical identity and offset lets a variable-extent
/// sliver compensate the scroll position when rows above that child are
/// measured more accurately.
pub(in crate::tree) fn sliver_anchor(
    layout: &SliverViewportLayout,
    physical_offset: f32,
) -> Option<(SliverChildId, f32)> {
    layout
        .children
        .iter()
        .find(|child| {
            child.offset + child.extent > physical_offset
                || (child.extent <= 0. && child.offset >= physical_offset)
        })
        .map(|child| (child.id, child.offset))
}

/// Applies a child's additional constraints without ever allowing it to
/// escape the bounds imposed by its parent.
pub(in crate::tree) fn enforced_constraints(
    parent: Constraints,
    additional: Constraints,
) -> Constraints {
    Constraints::new(
        additional
            .min_width
            .clamp(parent.min_width, parent.max_width),
        additional
            .max_width
            .clamp(parent.min_width, parent.max_width),
        additional
            .min_height
            .clamp(parent.min_height, parent.max_height),
        additional
            .max_height
            .clamp(parent.min_height, parent.max_height),
    )
}

pub(in crate::tree) fn unconstrained_constraints(
    parent: Constraints,
    constrained_axis: Option<Axis>,
) -> Constraints {
    match constrained_axis {
        Some(Axis::Horizontal) => {
            Constraints::new(parent.min_width, parent.max_width, 0., f32::INFINITY)
        }
        Some(Axis::Vertical) => {
            Constraints::new(0., f32::INFINITY, parent.min_height, parent.max_height)
        }
        None => Constraints::unbounded(),
    }
}

pub(in crate::tree) fn fractional_constraints(
    parent: Constraints,
    width_factor: Option<f32>,
    height_factor: Option<f32>,
) -> Constraints {
    let width = width_factor
        .filter(|_| parent.max_width.is_finite())
        .map(|factor| parent.max_width * factor);
    let height = height_factor
        .filter(|_| parent.max_height.is_finite())
        .map(|factor| parent.max_height * factor);
    Constraints::new(
        width.unwrap_or(0.),
        width.unwrap_or(parent.max_width),
        height.unwrap_or(0.),
        height.unwrap_or(parent.max_height),
    )
}
