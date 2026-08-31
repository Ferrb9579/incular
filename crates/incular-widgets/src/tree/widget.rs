//! Declarative widget values, kinds, and constructors.
//!
//! This module owns the immutable widget description consumed by the retained tree.

use super::*;

/// The first built-in widgets. Their values contain no mutable runtime state.
#[derive(Clone)]
pub struct Widget {
    pub key: Option<Key>,
    pub kind: WidgetKind,
    pub(super) semantics: SemanticProperties,
}

impl Drop for Widget {
    fn drop(&mut self) {
        // WidgetKind holds declarative child edges in Rc. The final strong
        // reference can still recursively destroy a deep, perfectly valid
        // descriptor chain through Rc's drop glue. Detach uniquely owned edges
        // level by level so descriptor destruction is memory-bounded rather
        // than native-stack-bounded.
        let mut pending = std::mem::replace(
            &mut self.kind,
            WidgetKind::Box {
                size: Size::ZERO,
                color: Color::TRANSPARENT,
            },
        )
        .into_owned_children();
        while let Some(child) = pending.pop() {
            let Ok(mut child) = Rc::try_unwrap(child) else {
                // Another declarative or retained owner still holds this
                // descriptor. Dropping this edge only decrements the count and
                // cannot recursively destroy the descendant chain here.
                continue;
            };
            let grandchildren = std::mem::replace(
                &mut child.kind,
                WidgetKind::Box {
                    size: Size::ZERO,
                    color: Color::TRANSPARENT,
                },
            )
            .into_owned_children();
            pending.extend(grandchildren);
            // `child` now owns no Widget descendants, so its normal Drop is
            // constant-stack even when it came from a pathological chain.
        }
    }
}

/// Converts the controller's logical offset into the physical content offset
/// used by a viewport transform.  A reversed viewport keeps the controller's
/// public range in the same `0..max` coordinate system, while its leading edge
/// is the content's physical trailing edge.
#[inline]
pub(super) fn physical_scroll_offset(controller: &ScrollController, reverse: bool) -> f32 {
    if reverse {
        (controller.max_offset() - controller.offset()).max(0.)
    } else {
        controller.offset()
    }
}

#[inline]
pub(super) fn scroll_translation(
    controller: &ScrollController,
    axis: Axis,
    reverse: bool,
) -> Offset {
    let offset = physical_scroll_offset(controller, reverse);
    axis.offset(-offset, 0.)
}

#[inline]
pub(super) fn scroll_viewport_extent(axis: Axis, size: Size) -> f32 {
    axis.main_extent(size)
}

#[inline]
pub(super) fn scroll_delta_for_axis(axis: Axis, delta: Offset) -> f32 {
    match axis {
        Axis::Horizontal => delta.x,
        Axis::Vertical => delta.y,
    }
}

#[inline]
pub(super) fn scroll_constraints(axis: Axis, constraints: Constraints) -> Constraints {
    match axis {
        Axis::Vertical => Constraints::new(0., constraints.max_width, 0., f32::INFINITY),
        Axis::Horizontal => Constraints::new(0., f32::INFINITY, 0., constraints.max_height),
    }
}

#[inline]
pub(super) fn scroll_size(axis: Axis, constraints: Constraints, content: Size) -> Size {
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
pub(super) fn sliver_viewport_size(
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
pub(super) fn sliver_anchor(
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
pub(super) fn enforced_constraints(parent: Constraints, additional: Constraints) -> Constraints {
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

pub(super) fn unconstrained_constraints(
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

pub(super) fn fractional_constraints(
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
impl std::fmt::Debug for Widget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Widget")
            .field("key", &self.key)
            .field("kind", &self.kind)
            .finish()
    }
}
impl PartialEq for Widget {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key && self.kind == other.kind && self.semantics == other.semantics
    }
}
impl std::fmt::Debug for WidgetKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Box { size, color } => f
                .debug_struct("Box")
                .field("size", size)
                .field("color", color)
                .finish(),
            Self::Shape {
                path,
                fill,
                stroke,
                size,
            } => f
                .debug_struct("PathView")
                .field("path", &path.id())
                .field("fill", fill)
                .field("stroke", stroke)
                .field("size", size)
                .finish(),
            Self::CustomPaint { size, display_list } => f
                .debug_struct("CustomPaint")
                .field("size", size)
                .field("command_count", &display_list.len())
                .finish(),
            Self::Decorated {
                size,
                background,
                border,
                radius,
                ..
            } => f
                .debug_struct("DecoratedBox")
                .field("size", size)
                .field("background", background)
                .field("border", border)
                .field("radius", radius)
                .finish(),
            Self::Banner {
                message,
                text_direction,
                location,
                layout_direction,
                color,
                text_style,
                shadow,
                child,
            } => f
                .debug_struct("Banner")
                .field("message", message)
                .field("text_direction", text_direction)
                .field("location", location)
                .field("layout_direction", layout_direction)
                .field("color", color)
                .field("text_style", text_style)
                .field("shadow", shadow)
                .field("child", child)
                .finish(),
            Self::Button {
                size,
                color,
                action,
                ..
            } => f
                .debug_struct("Button")
                .field("size", size)
                .field("color", color)
                .field("action", action)
                .finish(),
            Self::Text {
                text,
                style,
                align,
                soft_wrap,
                max_lines,
                overflow,
            } => f
                .debug_struct("Text")
                .field("text", text)
                .field("style", style)
                .field("align", align)
                .field("soft_wrap", soft_wrap)
                .field("max_lines", max_lines)
                .field("overflow", overflow)
                .finish(),
            Self::Image {
                image,
                width,
                height,
                fit,
                repeat,
                alignment,
                sampling: _,
            } => f
                .debug_struct("Image")
                .field("id", &image.id())
                .field("width", width)
                .field("height", height)
                .field("fit", fit)
                .field("repeat", repeat)
                .field("alignment", alignment)
                .finish(),
            Self::TextField { placeholder, .. } => f
                .debug_struct("TextField")
                .field("placeholder", placeholder)
                .finish(),
            Self::Padding { padding, child } => f
                .debug_struct("Padding")
                .field("padding", padding)
                .field("child", child)
                .finish(),
            Self::Constrained { constraints, child } => f
                .debug_struct("ConstrainedBox")
                .field("constraints", constraints)
                .field("child", child)
                .finish(),
            Self::Limited {
                max_width,
                max_height,
                child,
            } => f
                .debug_struct("LimitedBox")
                .field("max_width", max_width)
                .field("max_height", max_height)
                .field("child", child)
                .finish(),
            Self::Overflow {
                min_width,
                max_width,
                min_height,
                max_height,
                child,
            } => f
                .debug_struct("OverflowBox")
                .field("min_width", min_width)
                .field("max_width", max_width)
                .field("min_height", min_height)
                .field("max_height", max_height)
                .field("child", child)
                .finish(),
            Self::Unconstrained {
                constrained_axis,
                child,
            } => f
                .debug_struct("UnconstrainedBox")
                .field("constrained_axis", constrained_axis)
                .field("child", child)
                .finish(),
            Self::Fractional {
                width_factor,
                height_factor,
                child,
            } => f
                .debug_struct("FractionallySizedBox")
                .field("width_factor", width_factor)
                .field("height_factor", height_factor)
                .field("child", child)
                .finish(),
            Self::Baseline { baseline, child } => f
                .debug_struct("Baseline")
                .field("baseline", baseline)
                .field("child", child)
                .finish(),
            Self::RepaintBoundary { child } => f
                .debug_struct("RepaintBoundary")
                .field("child", child)
                .finish(),
            Self::Gesture { child, .. } => f
                .debug_struct("GestureDetector")
                .field("child", child)
                .finish(),
            Self::RawInput { kind, child } => f
                .debug_struct("RawInput")
                .field("type", &kind.type_())
                .field("child", child)
                .finish(),
            Self::Draggable { child, .. } => {
                f.debug_struct("Draggable").field("child", child).finish()
            }
            Self::DragTarget { child, .. } => {
                f.debug_struct("DragTarget").field("child", child).finish()
            }
            Self::IgnorePointer { ignoring, child } => f
                .debug_struct("IgnorePointer")
                .field("ignoring", ignoring)
                .field("child", child)
                .finish(),
            Self::AbsorbPointer { absorbing, child } => f
                .debug_struct("AbsorbPointer")
                .field("absorbing", absorbing)
                .field("child", child)
                .finish(),
            Self::Align {
                alignment,
                width_factor,
                height_factor,
                child,
            } => f
                .debug_struct("Align")
                .field("alignment", alignment)
                .field("width_factor", width_factor)
                .field("height_factor", height_factor)
                .field("child", child)
                .finish(),
            Self::Flex {
                axis,
                main_axis_alignment,
                main_axis_size,
                cross_axis_alignment,
                text_direction,
                vertical_direction,
                spacing,
                children,
            } => f
                .debug_struct("Flex")
                .field("axis", axis)
                .field("main_axis_alignment", main_axis_alignment)
                .field("main_axis_size", main_axis_size)
                .field("cross_axis_alignment", cross_axis_alignment)
                .field("text_direction", text_direction)
                .field("vertical_direction", vertical_direction)
                .field("spacing", spacing)
                .field("children", children)
                .finish(),
            Self::Flexible { flex, fit, child } => f
                .debug_struct("Flexible")
                .field("flex", flex)
                .field("fit", fit)
                .field("child", child)
                .finish(),
            Self::Wrap {
                axis,
                alignment,
                spacing,
                run_alignment,
                run_spacing,
                cross_axis_alignment,
                text_direction,
                vertical_direction,
                children,
            } => f
                .debug_struct("Wrap")
                .field("axis", axis)
                .field("alignment", alignment)
                .field("spacing", spacing)
                .field("run_alignment", run_alignment)
                .field("run_spacing", run_spacing)
                .field("cross_axis_alignment", cross_axis_alignment)
                .field("text_direction", text_direction)
                .field("vertical_direction", vertical_direction)
                .field("children", children)
                .finish(),
            Self::Table {
                columns,
                column_spacing,
                row_spacing,
                children,
            } => f
                .debug_struct("Table")
                .field("columns", columns)
                .field("column_spacing", column_spacing)
                .field("row_spacing", row_spacing)
                .field("children", children)
                .finish(),
            Self::Stack {
                alignment,
                text_direction,
                fit,
                clip_behavior,
                children,
            } => f
                .debug_struct("Stack")
                .field("alignment", alignment)
                .field("text_direction", text_direction)
                .field("fit", fit)
                .field("clip_behavior", clip_behavior)
                .field("children", children)
                .finish(),
            Self::SafeArea {
                minimum,
                left,
                top,
                right,
                bottom,
                maintain_bottom_view_padding,
                child,
            } => f
                .debug_struct("SafeArea")
                .field("minimum", minimum)
                .field("left", left)
                .field("top", top)
                .field("right", right)
                .field("bottom", bottom)
                .field("maintain_bottom_view_padding", maintain_bottom_view_padding)
                .field("child", child)
                .finish(),
            Self::ClipRect {
                clip_behavior,
                child,
            } => f
                .debug_struct("ClipRect")
                .field("clip_behavior", clip_behavior)
                .field("child", child)
                .finish(),
            Self::ClipRRect {
                radius,
                clip_behavior,
                child,
            } => f
                .debug_struct("ClipRRect")
                .field("radius", radius)
                .field("clip_behavior", clip_behavior)
                .field("child", child)
                .finish(),
            Self::ClipOval {
                clip_behavior,
                child,
            } => f
                .debug_struct("ClipOval")
                .field("clip_behavior", clip_behavior)
                .field("child", child)
                .finish(),
            Self::ClipPath {
                path,
                clip_behavior,
                child,
            } => f
                .debug_struct("ClipPath")
                .field("path", path)
                .field("clip_behavior", clip_behavior)
                .field("child", child)
                .finish(),
            Self::Positioned {
                left,
                top,
                right,
                bottom,
                width,
                height,
                child,
            } => f
                .debug_struct("Positioned")
                .field("left", left)
                .field("top", top)
                .field("right", right)
                .field("bottom", bottom)
                .field("width", width)
                .field("height", height)
                .field("child", child)
                .finish(),
            Self::IndexedStack {
                alignment,
                index,
                children,
            } => f
                .debug_struct("IndexedStack")
                .field("alignment", alignment)
                .field("index", index)
                .field("children", children)
                .finish(),
            Self::LayoutBuilder { .. } => f.debug_struct("LayoutBuilder").finish(),
            Self::Visibility { visible, child } => f
                .debug_struct("Visibility")
                .field("visible", visible)
                .field("child", child)
                .finish(),
            Self::AspectRatio { ratio, child } => f
                .debug_struct("AspectRatio")
                .field("ratio", ratio)
                .field("child", child)
                .finish(),
            Self::Scroll { .. } => f.debug_struct("ScrollView").finish(),
            Self::RawScrollbar {
                controller,
                style,
                child,
            } => f
                .debug_struct("RawScrollbar")
                .field("controller", controller)
                .field("style", style)
                .field("child", child)
                .finish(),
            Self::ListWheelScrollView { view } => f
                .debug_struct("ListWheelScrollView")
                .field("view", view)
                .finish(),
            Self::ListWheelViewport { viewport } => f
                .debug_struct("ListWheelViewport")
                .field("viewport", viewport)
                .finish(),
            Self::DraggableScrollableSheet { sheet } => f
                .debug_struct("DraggableScrollableSheet")
                .field("sheet", sheet)
                .finish(),
            Self::DraggableScrollableActuator { actuator, child } => f
                .debug_struct("DraggableScrollableActuator")
                .field("actuator", actuator)
                .field("child", child)
                .finish(),
            Self::TwoDimensionalScrollView { view } => f
                .debug_struct("TwoDimensionalScrollView")
                .field("view", view)
                .finish(),
            Self::TwoDimensionalViewport { viewport } => f
                .debug_struct("TwoDimensionalViewport")
                .field("viewport", viewport)
                .finish(),
            Self::PersistentHeader { .. } => f.debug_struct("PersistentHeader").finish(),
            Self::NotificationListener { child, .. } => f
                .debug_struct("NotificationListener")
                .field("child", child)
                .finish(),
            Self::SliverViewport { config } => f
                .debug_struct("SliverViewport")
                .field("config", config)
                .finish(),
            Self::Translate { .. } => f.debug_struct("Translate").finish(),
            Self::Transform {
                transform, origin, ..
            } => f
                .debug_struct("Transform")
                .field("transform", transform)
                .field("origin", origin)
                .finish(),
            Self::Scale {
                controller, origin, ..
            } => f
                .debug_struct("ScaleTransition")
                .field("controller", controller)
                .field("origin", origin)
                .finish(),
            Self::Rotation {
                controller, origin, ..
            } => f
                .debug_struct("RotationTransition")
                .field("controller", controller)
                .field("origin", origin)
                .finish(),
            Self::FittedBox { fit, alignment, .. } => f
                .debug_struct("FittedBox")
                .field("fit", fit)
                .field("alignment", alignment)
                .finish(),
            Self::Opacity {
                alpha, controller, ..
            } => f
                .debug_struct("Opacity")
                .field("alpha", alpha)
                .field("controller", controller)
                .finish(),
            Self::Blur {
                sigma_x,
                sigma_y,
                controller,
                ..
            } => f
                .debug_struct("Blur")
                .field("sigma_x", sigma_x)
                .field("sigma_y", sigma_y)
                .field("controller", controller)
                .finish(),
            Self::DropShadow {
                offset,
                sigma_x,
                sigma_y,
                color,
                controller,
                ..
            } => f
                .debug_struct("DropShadow")
                .field("offset", offset)
                .field("sigma_x", sigma_x)
                .field("sigma_y", sigma_y)
                .field("color", color)
                .field("controller", controller)
                .finish(),
            Self::ColorFiltered {
                filter, controller, ..
            } => f
                .debug_struct("ColorFiltered")
                .field("filter", filter)
                .field("controller", controller)
                .finish(),
            Self::Blend { mode, .. } => f.debug_struct("Blend").field("mode", mode).finish(),
            Self::ShaderMask { blend_mode, .. } => f
                .debug_struct("ShaderMask")
                .field("blend_mode", blend_mode)
                .finish(),
            Self::BackdropFilter {
                blur,
                blend_mode,
                enabled,
                ..
            } => f
                .debug_struct("BackdropFilter")
                .field("blur", blur)
                .field("blend_mode", blend_mode)
                .field("enabled", enabled)
                .finish(),
            Self::AnnotatedRegion { sized, .. } => f
                .debug_struct("AnnotatedRegion")
                .field("sized", sized)
                .finish(),
            Self::CompositedTransformTarget { link, .. } => f
                .debug_struct("CompositedTransformTarget")
                .field("link", link)
                .finish(),
            Self::CompositedTransformFollower {
                link,
                show_when_unlinked,
                offset,
                target_anchor,
                follower_anchor,
                ..
            } => f
                .debug_struct("CompositedTransformFollower")
                .field("link", link)
                .field("show_when_unlinked", show_when_unlinked)
                .field("offset", offset)
                .field("target_anchor", target_anchor)
                .field("follower_anchor", follower_anchor)
                .finish(),
            Self::SelectableText { text, style, align } => f
                .debug_struct("SelectableText")
                .field("text", text)
                .field("style", style)
                .field("align", align)
                .finish(),
            Self::SelectionArea { controller, .. } => f
                .debug_struct("SelectionArea")
                .field("controller", controller)
                .finish(),
            Self::SelectionContainer { delegate, .. } => f
                .debug_struct("SelectionContainer")
                .field("delegate", delegate)
                .finish(),
            Self::SelectionListener {
                notifier, delegate, ..
            } => f
                .debug_struct("SelectionListener")
                .field("notifier", notifier)
                .field("delegate", delegate)
                .finish(),
            Self::IndexedSemantics { index, .. } => f
                .debug_struct("IndexedSemantics")
                .field("index", index)
                .finish(),
            Self::SemanticsDebugger {
                label_style,
                max_nodes,
                ..
            } => f
                .debug_struct("SemanticsDebugger")
                .field("label_style", label_style)
                .field("max_nodes", max_nodes)
                .finish(),
        }
    }
}
impl PartialEq for WidgetKind {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Box { size: a, color: b }, Self::Box { size: c, color: d }) => a == c && b == d,
            (
                Self::Shape {
                    path: a,
                    fill: b,
                    stroke: c,
                    size: d,
                },
                Self::Shape {
                    path: e,
                    fill: f,
                    stroke: g,
                    size: h,
                },
            ) => a == e && b == f && c == g && d == h,
            (
                Self::CustomPaint {
                    size: a,
                    display_list: b,
                },
                Self::CustomPaint {
                    size: c,
                    display_list: d,
                },
            ) => a == c && b == d,
            (
                Self::Limited {
                    max_width: a,
                    max_height: b,
                    child: c,
                },
                Self::Limited {
                    max_width: d,
                    max_height: e,
                    child: f,
                },
            ) => a == d && b == e && c == f,
            (
                Self::SelectableText {
                    text: a,
                    style: b,
                    align: c,
                },
                Self::SelectableText {
                    text: d,
                    style: e,
                    align: f,
                },
            ) => a == d && b == e && c == f,
            (
                Self::SelectionArea {
                    controller: a,
                    child: b,
                },
                Self::SelectionArea {
                    controller: c,
                    child: d,
                },
            ) => a == c && b == d,
            (
                Self::SelectionContainer {
                    delegate: a,
                    child: b,
                },
                Self::SelectionContainer {
                    delegate: c,
                    child: d,
                },
            ) => a == c && b == d,
            (
                Self::SelectionListener {
                    notifier: a,
                    delegate: b,
                    child: c,
                },
                Self::SelectionListener {
                    notifier: d,
                    delegate: e,
                    child: f,
                },
            ) => a == d && b == e && c == f,
            (
                Self::IndexedSemantics { index: a, child: b },
                Self::IndexedSemantics { index: c, child: d },
            ) => a == c && b == d,
            (
                Self::SemanticsDebugger {
                    label_style: a,
                    max_nodes: b,
                    child: c,
                },
                Self::SemanticsDebugger {
                    label_style: d,
                    max_nodes: e,
                    child: f,
                },
            ) => a == d && b == e && c == f,
            (
                Self::Overflow {
                    min_width: a,
                    max_width: b,
                    min_height: c,
                    max_height: d,
                    child: e,
                },
                Self::Overflow {
                    min_width: f,
                    max_width: g,
                    min_height: h,
                    max_height: i,
                    child: j,
                },
            ) => a == f && b == g && c == h && d == i && e == j,
            (Self::RepaintBoundary { child: a }, Self::RepaintBoundary { child: b }) => a == b,
            (
                Self::Gesture {
                    behavior: a_behavior,
                    callbacks: a,
                    child: b,
                },
                Self::Gesture {
                    behavior: b_behavior,
                    callbacks: c,
                    child: d,
                },
            ) => a_behavior == b_behavior && gesture_callbacks_eq(a, c) && b == d,
            (
                Self::RawInput {
                    kind: left_kind,
                    child: left_child,
                },
                Self::RawInput {
                    kind: right_kind,
                    child: right_child,
                },
            ) => left_kind == right_kind && left_child == right_child,
            (
                Self::Draggable {
                    source: a,
                    child: b,
                },
                Self::Draggable {
                    source: c,
                    child: d,
                },
            ) => Rc::ptr_eq(a, c) && b == d,
            (
                Self::DragTarget {
                    target: a,
                    child: b,
                },
                Self::DragTarget {
                    target: c,
                    child: d,
                },
            ) => Rc::ptr_eq(a, c) && b == d,
            (
                Self::IgnorePointer {
                    ignoring: a,
                    child: b,
                },
                Self::IgnorePointer {
                    ignoring: c,
                    child: d,
                },
            ) => a == c && b == d,
            (
                Self::AbsorbPointer {
                    absorbing: a,
                    child: b,
                },
                Self::AbsorbPointer {
                    absorbing: c,
                    child: d,
                },
            ) => a == c && b == d,
            (
                Self::Table {
                    columns: a,
                    column_spacing: b,
                    row_spacing: c,
                    children: d,
                },
                Self::Table {
                    columns: e,
                    column_spacing: f,
                    row_spacing: g,
                    children: h,
                },
            ) => a == e && b == f && c == g && d == h,
            (
                Self::Decorated {
                    size: a,
                    background: b,
                    border: c,
                    radius: d,
                    child: e,
                },
                Self::Decorated {
                    size: f,
                    background: g,
                    border: h,
                    radius: i,
                    child: j,
                },
            ) => a == f && b == g && c == h && d == i && e == j,
            (
                Self::Banner {
                    message: a,
                    text_direction: b,
                    location: c,
                    layout_direction: d,
                    color: e,
                    text_style: f,
                    shadow: g,
                    child: h,
                },
                Self::Banner {
                    message: i,
                    text_direction: j,
                    location: k,
                    layout_direction: l,
                    color: m,
                    text_style: n,
                    shadow: o,
                    child: p,
                },
            ) => a == i && b == j && c == k && d == l && e == m && f == n && g == o && h == p,
            (
                Self::Button {
                    size: a,
                    color: b,
                    hover_color: c0,
                    pressed_color: d0,
                    focused_color: e0,
                    disabled_color: f0,
                    enabled: g0,
                    focusable_when_disabled: h0,
                    action: c,
                    callback: d,
                    hover_action: i,
                    hover_callback: j,
                    exit_action: k,
                    exit_callback: l,
                    has_callback: m,
                    child: n,
                },
                Self::Button {
                    size: e,
                    color: f,
                    hover_color: c1,
                    pressed_color: d1,
                    focused_color: e1,
                    disabled_color: f1,
                    enabled: g1,
                    focusable_when_disabled: h1,
                    action: g,
                    callback: h,
                    hover_action: o,
                    hover_callback: p,
                    exit_action: q,
                    exit_callback: r,
                    has_callback: s,
                    child: t,
                },
            ) => {
                a == e
                    && b == f
                    && c0 == c1
                    && d0 == d1
                    && e0 == e1
                    && f0 == f1
                    && g0 == g1
                    && h0 == h1
                    && c == g
                    && i == o
                    && k == q
                    && m == s
                    && n == t
                    && match (d, h) {
                        (Some(x), Some(y)) => Rc::ptr_eq(x, y),
                        (None, None) => true,
                        _ => false,
                    }
                    && match (j, p) {
                        (Some(x), Some(y)) => Rc::ptr_eq(x, y),
                        (None, None) => true,
                        _ => false,
                    }
                    && match (l, r) {
                        (Some(x), Some(y)) => Rc::ptr_eq(x, y),
                        (None, None) => true,
                        _ => false,
                    }
            }
            (
                Self::Text {
                    text: a,
                    style: b,
                    align: c,
                    soft_wrap: d,
                    max_lines: e,
                    overflow: f,
                },
                Self::Text {
                    text: g,
                    style: h,
                    align: i,
                    soft_wrap: j,
                    max_lines: k,
                    overflow: l,
                },
            ) => a == g && b == h && c == i && d == j && e == k && f == l,
            (
                Self::Baseline {
                    baseline: a,
                    child: b,
                },
                Self::Baseline {
                    baseline: c,
                    child: d,
                },
            ) => a == c && b == d,
            (
                Self::Image {
                    image: a,
                    width: b,
                    height: c,
                    fit: d,
                    repeat: e,
                    alignment: k,
                    sampling: m,
                },
                Self::Image {
                    image: f,
                    width: g,
                    height: h,
                    fit: i,
                    repeat: j,
                    alignment: l,
                    sampling: n,
                },
            ) => a == f && b == g && c == h && d == i && e == j && k == l && m == n,
            (
                Self::TextField {
                    controller: a,
                    size: b,
                    style: c,
                    placeholder: d,
                    on_submit: e,
                    multiline: k,
                    min_lines: l,
                    max_lines: m,
                    expands: n,
                    text_align: o,
                    enabled: p,
                    read_only: q,
                    obscure_text: r,
                    cursor_width: s,
                    cursor_height: t,
                    cursor_radius: u,
                    show_cursor: v,
                    cursor_color: w,
                    selection_color: x,
                },
                Self::TextField {
                    controller: f,
                    size: g,
                    style: h,
                    placeholder: i,
                    on_submit: j,
                    multiline: l2,
                    min_lines: l3,
                    max_lines: m2,
                    expands: n2,
                    text_align: o2,
                    enabled: y,
                    read_only: z,
                    obscure_text: aa,
                    cursor_width: ab,
                    cursor_height: ac,
                    cursor_radius: ad,
                    show_cursor: ae,
                    cursor_color: af,
                    selection_color: ag,
                },
            ) => {
                a == f
                    && b == g
                    && c == h
                    && d == i
                    && k == l2
                    && l == l3
                    && m == m2
                    && n == n2
                    && o == o2
                    && p == y
                    && q == z
                    && r == aa
                    && s == ab
                    && t == ac
                    && u == ad
                    && v == ae
                    && w == af
                    && x == ag
                    && match (e, j) {
                        (Some(left), Some(right)) => Rc::ptr_eq(left, right),
                        (None, None) => true,
                        _ => false,
                    }
            }
            (
                Self::Padding {
                    padding: a,
                    child: b,
                },
                Self::Padding {
                    padding: c,
                    child: d,
                },
            ) => a == c && b == d,
            (
                Self::Fractional {
                    width_factor: a,
                    height_factor: b,
                    child: c,
                },
                Self::Fractional {
                    width_factor: d,
                    height_factor: e,
                    child: f,
                },
            ) => a == d && b == e && c == f,
            (
                Self::Wrap {
                    axis: a,
                    alignment: b,
                    spacing: c,
                    run_alignment: d,
                    run_spacing: e,
                    cross_axis_alignment: f,
                    text_direction: g,
                    vertical_direction: h,
                    children: i,
                },
                Self::Wrap {
                    axis: j,
                    alignment: k,
                    spacing: l,
                    run_alignment: m,
                    run_spacing: n,
                    cross_axis_alignment: o,
                    text_direction: p,
                    vertical_direction: q,
                    children: r,
                },
            ) => {
                a == j
                    && b == k
                    && c == l
                    && d == m
                    && e == n
                    && f == o
                    && g == p
                    && h == q
                    && i == r
            }
            (
                Self::Constrained {
                    constraints: a,
                    child: b,
                },
                Self::Constrained {
                    constraints: c,
                    child: d,
                },
            ) => a == c && b == d,
            (
                Self::Unconstrained {
                    constrained_axis: a,
                    child: b,
                },
                Self::Unconstrained {
                    constrained_axis: c,
                    child: d,
                },
            ) => a == c && b == d,
            (
                Self::Scroll {
                    controller: a,
                    axis: e,
                    reverse: f,
                    physics: g,
                    child: b,
                },
                Self::Scroll {
                    controller: c,
                    axis: h,
                    reverse: i,
                    physics: j,
                    child: d,
                },
            ) => a == c && e == h && f == i && g == j && b == d,
            (
                Self::RawScrollbar {
                    controller: a,
                    style: b,
                    child: c,
                },
                Self::RawScrollbar {
                    controller: d,
                    style: e,
                    child: f,
                },
            ) => a == d && b == e && c == f,
            (Self::ListWheelScrollView { view: a }, Self::ListWheelScrollView { view: b }) => {
                a == b
            }
            (Self::ListWheelViewport { viewport: a }, Self::ListWheelViewport { viewport: b }) => {
                a == b
            }
            (
                Self::DraggableScrollableSheet { sheet: a },
                Self::DraggableScrollableSheet { sheet: b },
            ) => a == b,
            (
                Self::DraggableScrollableActuator {
                    actuator: a,
                    child: b,
                },
                Self::DraggableScrollableActuator {
                    actuator: c,
                    child: d,
                },
            ) => a == c && b == d,
            (
                Self::TwoDimensionalScrollView { view: a },
                Self::TwoDimensionalScrollView { view: b },
            ) => a == b,
            (
                Self::TwoDimensionalViewport { viewport: a },
                Self::TwoDimensionalViewport { viewport: b },
            ) => a == b,
            (
                Self::PersistentHeader {
                    controller: a,
                    axis: e,
                    reverse: f,
                    pinned: g,
                    child: b,
                },
                Self::PersistentHeader {
                    controller: c,
                    axis: h,
                    reverse: i,
                    pinned: j,
                    child: d,
                },
            ) => a == c && e == h && f == i && g == j && b == d,
            (Self::SliverViewport { config: a }, Self::SliverViewport { config: b }) => a == b,
            (
                Self::Translate {
                    controller: a,
                    child: b,
                },
                Self::Translate {
                    controller: c,
                    child: d,
                },
            ) => Rc::ptr_eq(&a.offset, &c.offset) && b == d,
            (
                Self::Transform {
                    transform: a,
                    origin: b,
                    child: c,
                },
                Self::Transform {
                    transform: d,
                    origin: e,
                    child: f,
                },
            ) => a == d && b == e && c == f,
            (
                Self::Scale {
                    controller: a,
                    origin: b,
                    child: c,
                },
                Self::Scale {
                    controller: d,
                    origin: e,
                    child: f,
                },
            ) => a == d && b == e && c == f,
            (
                Self::Rotation {
                    controller: a,
                    origin: b,
                    alignment: c,
                    child: d,
                    ..
                },
                Self::Rotation {
                    controller: e,
                    origin: f,
                    alignment: g,
                    child: h,
                    ..
                },
            ) => a == e && b == f && c == g && d == h,
            (
                Self::FittedBox {
                    fit: a,
                    alignment: b,
                    child: c,
                },
                Self::FittedBox {
                    fit: d,
                    alignment: e,
                    child: f,
                },
            ) => a == d && b == e && c == f,
            (
                Self::Opacity {
                    alpha: a,
                    controller: b,
                    child: c,
                },
                Self::Opacity {
                    alpha: d,
                    controller: e,
                    child: f,
                },
            ) => a == d && b == e && c == f,
            (
                Self::Blur {
                    sigma_x: a,
                    sigma_y: b,
                    controller: c,
                    child: d,
                },
                Self::Blur {
                    sigma_x: e,
                    sigma_y: f,
                    controller: g,
                    child: h,
                },
            ) => a == e && b == f && c == g && d == h,
            (
                Self::DropShadow {
                    offset: a,
                    sigma_x: b,
                    sigma_y: c,
                    color: d,
                    controller: e,
                    child: f,
                },
                Self::DropShadow {
                    offset: g,
                    sigma_x: h,
                    sigma_y: i,
                    color: j,
                    controller: k,
                    child: l,
                },
            ) => a == g && b == h && c == i && d == j && e == k && f == l,
            (
                Self::ColorFiltered {
                    filter: a,
                    controller: b,
                    child: c,
                },
                Self::ColorFiltered {
                    filter: d,
                    controller: e,
                    child: f,
                },
            ) => a == d && b == e && c == f,
            (Self::Blend { mode: a, child: b }, Self::Blend { mode: c, child: d }) => {
                a == c && b == d
            }
            (
                Self::ShaderMask {
                    shader: a,
                    blend_mode: b,
                    child: c,
                },
                Self::ShaderMask {
                    shader: d,
                    blend_mode: e,
                    child: f,
                },
            ) => a == d && b == e && c == f,
            (
                Self::BackdropFilter {
                    blur: a,
                    blend_mode: b,
                    enabled: c,
                    child: d,
                },
                Self::BackdropFilter {
                    blur: e,
                    blend_mode: f,
                    enabled: g,
                    child: h,
                },
            ) => a == e && b == f && c == g && d == h,
            (
                Self::AnnotatedRegion {
                    annotation: a,
                    sized: b,
                    child: c,
                },
                Self::AnnotatedRegion {
                    annotation: d,
                    sized: e,
                    child: f,
                },
            ) => a == d && b == e && c == f,
            (
                Self::CompositedTransformTarget { link: a, child: b },
                Self::CompositedTransformTarget { link: c, child: d },
            ) => a == c && b == d,
            (
                Self::CompositedTransformFollower {
                    link: a,
                    show_when_unlinked: b,
                    offset: c,
                    target_anchor: d,
                    follower_anchor: e,
                    child: f,
                },
                Self::CompositedTransformFollower {
                    link: g,
                    show_when_unlinked: h,
                    offset: i,
                    target_anchor: j,
                    follower_anchor: k,
                    child: l,
                },
            ) => a == g && b == h && c == i && d == j && e == k && f == l,
            (
                Self::Align {
                    alignment: a,
                    width_factor: b,
                    height_factor: c,
                    child: d,
                },
                Self::Align {
                    alignment: e,
                    width_factor: f,
                    height_factor: g,
                    child: h,
                },
            ) => a == e && b == f && c == g && d == h,
            (
                Self::Flex {
                    axis: a,
                    main_axis_alignment: b,
                    main_axis_size: c,
                    cross_axis_alignment: d,
                    text_direction: e,
                    vertical_direction: f,
                    spacing: g,
                    children: h,
                },
                Self::Flex {
                    axis: i,
                    main_axis_alignment: j,
                    main_axis_size: k,
                    cross_axis_alignment: l,
                    text_direction: m,
                    vertical_direction: n,
                    spacing: o,
                    children: p,
                },
            ) => a == i && b == j && c == k && d == l && e == m && f == n && g == o && h == p,
            (
                Self::Flexible {
                    flex: a,
                    fit: b,
                    child: c,
                },
                Self::Flexible {
                    flex: d,
                    fit: e,
                    child: f,
                },
            ) => a == d && b == e && c == f,

            (
                Self::Stack {
                    alignment: a,
                    text_direction: b,
                    fit: c,
                    clip_behavior: d,
                    children: e,
                },
                Self::Stack {
                    alignment: f,
                    text_direction: g,
                    fit: h,
                    clip_behavior: i,
                    children: j,
                },
            ) => a == f && b == g && c == h && d == i && e == j,
            (
                Self::Positioned {
                    left: a,
                    top: b,
                    right: c,
                    bottom: d,
                    width: e,
                    height: f,
                    child: g,
                },
                Self::Positioned {
                    left: h,
                    top: i,
                    right: j,
                    bottom: k,
                    width: l,
                    height: m,
                    child: n,
                },
            ) => a == h && b == i && c == j && d == k && e == l && f == m && g == n,
            (
                Self::IndexedStack {
                    alignment: a,
                    index: b,
                    children: c,
                },
                Self::IndexedStack {
                    alignment: d,
                    index: e,
                    children: f,
                },
            ) => a == d && b == e && c == f,
            (
                Self::SafeArea {
                    minimum: a,
                    left: b,
                    top: c,
                    right: d,
                    bottom: e,
                    maintain_bottom_view_padding: f,
                    child: g,
                },
                Self::SafeArea {
                    minimum: h,
                    left: i,
                    top: j,
                    right: k,
                    bottom: l,
                    maintain_bottom_view_padding: m,
                    child: n,
                },
            ) => a == h && b == i && c == j && d == k && e == l && f == m && g == n,
            (
                Self::ClipRect {
                    clip_behavior: a,
                    child: b,
                },
                Self::ClipRect {
                    clip_behavior: c,
                    child: d,
                },
            ) => a == c && b == d,
            (
                Self::ClipRRect {
                    radius: a,
                    clip_behavior: b,
                    child: c,
                },
                Self::ClipRRect {
                    radius: d,
                    clip_behavior: e,
                    child: f,
                },
            ) => a == d && b == e && c == f,
            (
                Self::ClipOval {
                    clip_behavior: a,
                    child: b,
                },
                Self::ClipOval {
                    clip_behavior: c,
                    child: d,
                },
            ) => a == c && b == d,
            (
                Self::ClipPath {
                    path: a,
                    clip_behavior: b,
                    child: c,
                },
                Self::ClipPath {
                    path: d,
                    clip_behavior: e,
                    child: f,
                },
            ) => a == d && b == e && c == f,
            (
                Self::LayoutBuilder {
                    builder: a,
                    environment: c,
                    environment_boundary: g,
                    revision: e,
                },
                Self::LayoutBuilder {
                    builder: b,
                    environment: d,
                    environment_boundary: h,
                    revision: f,
                },
            ) => {
                Rc::ptr_eq(a, b)
                    && match (c, d) {
                        (Some(x), Some(y)) => Rc::ptr_eq(x, y),
                        (None, None) => true,
                        _ => false,
                    }
                    && match (e, f) {
                        (Some(x), Some(y)) => Rc::ptr_eq(x, y),
                        (None, None) => true,
                        _ => false,
                    }
                    && g == h
            }
            (
                Self::Visibility {
                    visible: a,
                    child: b,
                },
                Self::Visibility {
                    visible: c,
                    child: d,
                },
            ) => a == c && b == d,
            (
                Self::AspectRatio { ratio: a, child: b },
                Self::AspectRatio { ratio: c, child: d },
            ) => a == c && b == d,
            (
                Self::NotificationListener {
                    callback: a,
                    child: b,
                },
                Self::NotificationListener {
                    callback: c,
                    child: d,
                },
            ) => {
                let callback_equal = match (a, c) {
                    (Some(left), Some(right)) => Rc::ptr_eq(left, right),
                    (None, None) => true,
                    _ => false,
                };
                callback_equal && b.as_ref() == d.as_ref()
            }
            _ => false,
        }
    }
}

fn gesture_callbacks_eq(left: &GestureCallbacks, right: &GestureCallbacks) -> bool {
    fn same_callback<T: ?Sized>(left: &Option<Rc<T>>, right: &Option<Rc<T>>) -> bool {
        match (left, right) {
            (Some(left), Some(right)) => Rc::ptr_eq(left, right),
            (None, None) => true,
            _ => false,
        }
    }

    same_callback(&left.on_tap, &right.on_tap)
        && same_callback(&left.on_tap_down, &right.on_tap_down)
        && same_callback(&left.on_tap_up, &right.on_tap_up)
        && same_callback(&left.on_tap_cancel, &right.on_tap_cancel)
        && same_callback(&left.on_double_tap, &right.on_double_tap)
        && same_callback(&left.on_double_tap_down, &right.on_double_tap_down)
        && same_callback(&left.on_double_tap_cancel, &right.on_double_tap_cancel)
        && same_callback(&left.on_long_press, &right.on_long_press)
        && same_callback(&left.on_long_press_start, &right.on_long_press_start)
        && same_callback(
            &left.on_long_press_move_update,
            &right.on_long_press_move_update,
        )
        && same_callback(&left.on_long_press_up, &right.on_long_press_up)
        && same_callback(&left.on_long_press_end, &right.on_long_press_end)
        && same_callback(&left.on_pan_down, &right.on_pan_down)
        && same_callback(&left.on_pan_start, &right.on_pan_start)
        && same_callback(&left.on_pan_update, &right.on_pan_update)
        && same_callback(&left.on_pan_end, &right.on_pan_end)
        && same_callback(&left.on_pan_cancel, &right.on_pan_cancel)
        && same_callback(
            &left.on_horizontal_drag_down,
            &right.on_horizontal_drag_down,
        )
        && same_callback(
            &left.on_horizontal_drag_start,
            &right.on_horizontal_drag_start,
        )
        && same_callback(
            &left.on_horizontal_drag_update,
            &right.on_horizontal_drag_update,
        )
        && same_callback(&left.on_horizontal_drag_end, &right.on_horizontal_drag_end)
        && same_callback(
            &left.on_horizontal_drag_cancel,
            &right.on_horizontal_drag_cancel,
        )
        && same_callback(&left.on_vertical_drag_down, &right.on_vertical_drag_down)
        && same_callback(&left.on_vertical_drag_start, &right.on_vertical_drag_start)
        && same_callback(
            &left.on_vertical_drag_update,
            &right.on_vertical_drag_update,
        )
        && same_callback(&left.on_vertical_drag_end, &right.on_vertical_drag_end)
        && same_callback(
            &left.on_vertical_drag_cancel,
            &right.on_vertical_drag_cancel,
        )
        && same_callback(&left.on_scale_start, &right.on_scale_start)
        && same_callback(&left.on_scale_update, &right.on_scale_update)
        && same_callback(&left.on_scale_end, &right.on_scale_end)
        && same_callback(&left.on_cancel, &right.on_cancel)
        && same_callback(&left.on_key, &right.on_key)
        && same_callback(&left.on_key_down, &right.on_key_down)
        && same_callback(&left.on_key_repeat, &right.on_key_repeat)
        && same_callback(&left.on_key_up, &right.on_key_up)
        && same_callback(&left.on_shortcut, &right.on_shortcut)
        && same_callback(&left.shortcut_scope, &right.shortcut_scope)
        && same_callback(&left.action_scope, &right.action_scope)
        && same_callback(&left.action_listener, &right.action_listener)
        && same_callback(
            &left.action_invocation_listener,
            &right.action_invocation_listener,
        )
        && left.focus_node == right.focus_node
        && same_callback(&left.focus_behavior, &right.focus_behavior)
        && left.autofocus == right.autofocus
        && left.include_semantics == right.include_semantics
        && left.mouse_cursor == right.mouse_cursor
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WidgetType {
    Box,
    Shape,
    CustomPaint,
    Decorated,
    Banner,
    Image,
    Button,
    Text,
    SelectableText,
    SelectionArea,
    SelectionContainer,
    SelectionListener,
    IndexedSemantics,
    SemanticsDebugger,
    TextField,
    Padding,
    Constrained,
    Limited,
    Overflow,
    Unconstrained,
    Fractional,
    Baseline,
    RepaintBoundary,
    Gesture,
    Listener,
    RawGestureDetector,
    MouseRegion,
    TapRegion,
    TapRegionSurface,
    TextFieldTapRegion,
    Draggable,
    DragTarget,
    IgnorePointer,
    AbsorbPointer,
    Align,
    Flex,
    Flexible,
    Wrap,
    Table,
    Stack,
    Positioned,
    IndexedStack,
    SafeArea,
    ClipRect,
    ClipRRect,
    ClipOval,
    ClipPath,
    LayoutBuilder,
    Visibility,
    AspectRatio,
    Scroll,
    RawScrollbar,
    ListWheelScrollView,
    ListWheelViewport,
    DraggableScrollableSheet,
    DraggableScrollableActuator,
    TwoDimensionalScrollView,
    TwoDimensionalViewport,
    PersistentHeader,
    NotificationListener,
    SliverViewport,
    Translate,
    Transform,
    Scale,
    Rotation,
    FittedBox,
    Opacity,
    Blur,
    DropShadow,
    ColorFiltered,
    Blend,
    ShaderMask,
    BackdropFilter,
    AnnotatedRegion,
    CompositedTransformTarget,
    CompositedTransformFollower,
}
impl WidgetType {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Box => "Box",
            Self::Shape => "Shape",
            Self::CustomPaint => "CustomPaint",
            Self::Decorated => "DecoratedBox",
            Self::Banner => "Banner",
            Self::Image => "Image",
            Self::Button => "Button",
            Self::Text => "Text",
            Self::SelectableText => "SelectableText",
            Self::SelectionArea => "SelectionArea",
            Self::SelectionContainer => "SelectionContainer",
            Self::SelectionListener => "SelectionListener",
            Self::IndexedSemantics => "IndexedSemantics",
            Self::SemanticsDebugger => "SemanticsDebugger",
            Self::TextField => "TextField",
            Self::Padding => "Padding",
            Self::Constrained => "ConstrainedBox",
            Self::Limited => "LimitedBox",
            Self::Overflow => "OverflowBox",
            Self::Unconstrained => "UnconstrainedBox",
            Self::Fractional => "FractionallySizedBox",
            Self::Baseline => "Baseline",
            Self::RepaintBoundary => "RepaintBoundary",
            Self::Gesture => "GestureDetector",
            Self::Listener => "Listener",
            Self::RawGestureDetector => "RawGestureDetector",
            Self::MouseRegion => "MouseRegion",
            Self::TapRegion => "TapRegion",
            Self::TapRegionSurface => "TapRegionSurface",
            Self::TextFieldTapRegion => "TextFieldTapRegion",
            Self::Draggable => "Draggable",
            Self::DragTarget => "DragTarget",
            Self::IgnorePointer => "IgnorePointer",
            Self::AbsorbPointer => "AbsorbPointer",
            Self::Align => "Align",
            Self::Flex => "Flex",
            Self::Flexible => "Flexible",
            Self::Wrap => "Wrap",
            Self::Table => "Table",
            Self::Stack => "Stack",
            Self::Positioned => "Positioned",
            Self::IndexedStack => "IndexedStack",
            Self::SafeArea => "SafeArea",
            Self::ClipRect => "ClipRect",
            Self::ClipRRect => "ClipRRect",
            Self::ClipOval => "ClipOval",
            Self::ClipPath => "ClipPath",
            Self::LayoutBuilder => "LayoutBuilder",
            Self::Visibility => "Visibility",
            Self::AspectRatio => "AspectRatio",
            Self::Scroll => "ScrollView",
            Self::RawScrollbar => "RawScrollbar",
            Self::ListWheelScrollView => "ListWheelScrollView",
            Self::ListWheelViewport => "ListWheelViewport",
            Self::DraggableScrollableSheet => "DraggableScrollableSheet",
            Self::DraggableScrollableActuator => "DraggableScrollableActuator",
            Self::TwoDimensionalScrollView => "TwoDimensionalScrollView",
            Self::TwoDimensionalViewport => "TwoDimensionalViewport",
            Self::PersistentHeader => "PersistentHeader",
            Self::NotificationListener => "NotificationListener",
            Self::SliverViewport => "SliverViewport",
            Self::Translate => "Translate",
            Self::Transform => "Transform",
            Self::Scale => "ScaleTransition",
            Self::Rotation => "RotationTransition",
            Self::FittedBox => "FittedBox",
            Self::Opacity => "Opacity",
            Self::Blur => "ImageFiltered",
            Self::DropShadow => "DropShadow",
            Self::ColorFiltered => "ColorFiltered",
            Self::Blend => "Blend",
            Self::ShaderMask => "ShaderMask",
            Self::BackdropFilter => "BackdropFilter",
            Self::AnnotatedRegion => "AnnotatedRegion",
            Self::CompositedTransformTarget => "CompositedTransformTarget",
            Self::CompositedTransformFollower => "CompositedTransformFollower",
        }
    }
}
impl WidgetKind {
    fn into_owned_children(self) -> Vec<Rc<Widget>> {
        match self {
            WidgetKind::Box { .. }
            | WidgetKind::Shape { .. }
            | WidgetKind::CustomPaint { .. }
            | WidgetKind::Text { .. }
            | WidgetKind::SelectableText { .. }
            | WidgetKind::TextField { .. }
            | WidgetKind::Image { .. }
            | WidgetKind::ListWheelScrollView { .. }
            | WidgetKind::ListWheelViewport { .. }
            | WidgetKind::DraggableScrollableSheet { .. }
            | WidgetKind::TwoDimensionalScrollView { .. }
            | WidgetKind::TwoDimensionalViewport { .. }
            | WidgetKind::SliverViewport { .. }
            | WidgetKind::LayoutBuilder { .. } => Vec::new(),
            WidgetKind::Button { child, .. } | WidgetKind::Banner { child, .. } => {
                child.into_iter().collect()
            }
            WidgetKind::RawInput { child, .. } => child.into_iter().collect(),
            WidgetKind::Decorated { child, .. }
            | WidgetKind::Padding { child, .. }
            | WidgetKind::Constrained { child, .. }
            | WidgetKind::Limited { child, .. }
            | WidgetKind::Overflow { child, .. }
            | WidgetKind::Unconstrained { child, .. }
            | WidgetKind::Fractional { child, .. }
            | WidgetKind::Baseline { child, .. }
            | WidgetKind::RepaintBoundary { child }
            | WidgetKind::Gesture { child, .. }
            | WidgetKind::Draggable { child, .. }
            | WidgetKind::DragTarget { child, .. }
            | WidgetKind::IgnorePointer { child, .. }
            | WidgetKind::AbsorbPointer { child, .. }
            | WidgetKind::Align { child, .. }
            | WidgetKind::Flexible { child, .. }
            | WidgetKind::Positioned { child, .. }
            | WidgetKind::SafeArea { child, .. }
            | WidgetKind::ClipRect { child, .. }
            | WidgetKind::ClipRRect { child, .. }
            | WidgetKind::ClipOval { child, .. }
            | WidgetKind::ClipPath { child, .. }
            | WidgetKind::Visibility { child, .. }
            | WidgetKind::AspectRatio { child, .. }
            | WidgetKind::Scroll { child, .. }
            | WidgetKind::RawScrollbar { child, .. }
            | WidgetKind::DraggableScrollableActuator { child, .. }
            | WidgetKind::PersistentHeader { child, .. }
            | WidgetKind::NotificationListener { child, .. }
            | WidgetKind::Translate { child, .. }
            | WidgetKind::Transform { child, .. }
            | WidgetKind::Scale { child, .. }
            | WidgetKind::Rotation { child, .. }
            | WidgetKind::FittedBox { child, .. }
            | WidgetKind::Opacity { child, .. }
            | WidgetKind::Blur { child, .. }
            | WidgetKind::DropShadow { child, .. }
            | WidgetKind::ColorFiltered { child, .. }
            | WidgetKind::Blend { child, .. }
            | WidgetKind::ShaderMask { child, .. }
            | WidgetKind::BackdropFilter { child, .. }
            | WidgetKind::AnnotatedRegion { child, .. }
            | WidgetKind::CompositedTransformTarget { child, .. }
            | WidgetKind::CompositedTransformFollower { child, .. }
            | WidgetKind::SelectionArea { child, .. }
            | WidgetKind::SelectionContainer { child, .. }
            | WidgetKind::SelectionListener { child, .. }
            | WidgetKind::IndexedSemantics { child, .. }
            | WidgetKind::SemanticsDebugger { child, .. } => vec![child],
            WidgetKind::Flex { children, .. }
            | WidgetKind::Wrap { children, .. }
            | WidgetKind::Table { children, .. }
            | WidgetKind::Stack { children, .. }
            | WidgetKind::IndexedStack { children, .. } => children,
        }
    }
}

impl Widget {
    #[must_use]
    pub fn into_kind(mut self) -> WidgetKind {
        std::mem::replace(
            &mut self.kind,
            WidgetKind::Box {
                size: Size::ZERO,
                color: Color::TRANSPARENT,
            },
        )
    }

    /// Creates a widget from a internal kind descriptor.
    #[must_use]
    pub fn from_kind(kind: WidgetKind) -> Self {
        Self {
            key: None,
            kind,
            semantics: SemanticProperties::default(),
        }
    }

    /// Attaches retained focus traversal metadata to a transparent wrapper.
    /// This is crate-internal because public callers use the Flutter-shaped
    /// `FocusTraversal*` widgets.
    #[doc(hidden)]
    pub(crate) fn with_focus_traversal_policy(mut self, policy: FocusTraversalPolicyKind) -> Self {
        self.semantics.focus_traversal_policy = Some(policy);
        self
    }

    #[doc(hidden)]
    pub(crate) fn with_focus_traversal_order(mut self, order: Option<f64>) -> Self {
        self.semantics.focus_traversal_order = order.filter(|value| value.is_finite());
        self
    }

    #[doc(hidden)]
    pub(crate) fn with_excluded_focus(mut self, excluding: bool) -> Self {
        self.semantics.exclude_focus = excluding;
        self
    }

    #[doc(hidden)]
    pub(crate) fn with_excluded_focus_traversal(mut self, excluding: bool) -> Self {
        self.semantics.exclude_focus_traversal = excluding;
        self
    }

    /// Attaches the retained undo-history capacity to a text-field subtree.
    /// The runtime consumes this metadata without making Widgets depend on
    /// the runtime crate.
    #[doc(hidden)]
    pub(crate) fn with_undo_history_max_entries(mut self, max_entries: usize) -> Self {
        self.semantics.undo_history_max_entries = Some(max_entries);
        self
    }

    /// Marks this retained subtree as the initial focus scope.
    #[doc(hidden)]
    pub(crate) fn with_focus_scope_autofocus(mut self, autofocus: bool) -> Self {
        self.semantics.focus_scope_autofocus = autofocus;
        self
    }

    /// Attaches native text-input hints to the retained editor without making
    /// the renderer-neutral widget depend on a platform window handle.
    #[doc(hidden)]
    pub(crate) fn with_text_input_hints(
        mut self,
        input_type: TextInputTypeHint,
        input_action: TextInputActionHint,
    ) -> Self {
        self.semantics.text_input_type = Some(input_type);
        self.semantics.text_input_action = Some(input_action);
        self
    }

    #[doc(hidden)]
    pub(crate) fn with_semantic_callback(
        mut self,
        action: SemanticActionKind,
        callback: Rc<dyn Fn() + 'static>,
    ) -> Self {
        match action {
            SemanticActionKind::Activate => self.semantics.callbacks.activate = Some(callback),
            SemanticActionKind::Increment => self.semantics.callbacks.increment = Some(callback),
            SemanticActionKind::Decrement => self.semantics.callbacks.decrement = Some(callback),
            SemanticActionKind::ScrollForward => {
                self.semantics.callbacks.scroll_forward = Some(callback)
            }
            SemanticActionKind::ScrollBackward => {
                self.semantics.callbacks.scroll_backward = Some(callback)
            }
            SemanticActionKind::Focus
            | SemanticActionKind::SetText
            | SemanticActionKind::SetSelection => {}
        }
        self
    }

    /// Text content when this widget is text-like (DevTools labels only).
    pub fn text_if_any(&self) -> Option<String> {
        match &self.kind {
            WidgetKind::Text { text, .. } | WidgetKind::SelectableText { text, .. } => {
                Some(text.clone())
            }
            _ => None,
        }
    }

    /// Returns the best text label exposed by this widget or a transparent
    /// semantic/layout wrapper around it.
    ///
    /// Control crates use this when a Flutter-shaped control accepts a custom
    /// child instead of a dedicated `label` argument. The visual child and
    /// the accessible name are separate concepts, but text children provide a
    /// safe default for ordinary custom-content controls.
    #[doc(hidden)]
    pub fn semantic_text(&self) -> Option<String> {
        self.semantics
            .label
            .clone()
            .filter(|label| !label.trim().is_empty())
            .or_else(|| {
                self.semantics
                    .explicit
                    .as_ref()
                    .and_then(|semantics| semantics.label.clone())
                    .filter(|label| !label.trim().is_empty())
            })
            .or_else(|| widget_text(self))
    }

    #[must_use]
    pub fn box_(size: Size, color: Color) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Box { size, color },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn fixed_box(size: Size, color: Color) -> Self {
        Self::box_(size, color)
    }
    #[must_use]
    pub(super) fn shape(
        path: Arc<Path>,
        fill: Option<Brush>,
        stroke: Option<(Brush, Stroke)>,
        size: Option<Size>,
    ) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Shape {
                path,
                fill,
                stroke,
                size,
            },
            semantics: SemanticProperties::default(),
        }
    }
    /// Paints a caller-provided renderer-neutral display list at a fixed
    /// logical size.
    #[must_use]
    pub fn custom_paint(size: Size, display_list: DisplayList) -> Self {
        Self {
            key: None,
            kind: WidgetKind::CustomPaint { size, display_list },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub(super) fn decorated(
        size: Option<Size>,
        background: Option<Brush>,
        border: Option<Border>,
        radius: CornerRadii,
        child: Widget,
    ) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Decorated {
                size,
                background,
                border,
                radius,
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub(crate) fn button(size: Size, color: Color, action: ActionId) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Button {
                size,
                color,
                hover_color: None,
                pressed_color: None,
                focused_color: None,
                disabled_color: None,
                enabled: true,
                focusable_when_disabled: false,
                action,
                callback: None,
                hover_action: ActionId(0),
                hover_callback: None,
                exit_action: ActionId(0),
                exit_callback: None,
                has_callback: false,
                child: None,
            },
            semantics: SemanticProperties::default(),
        }
    }

    /// Builds the retained action surface shared by sibling control crates.
    ///
    /// This is crate-internal plumbing rather than a Flutter-facing widget;
    /// applications should use `GestureDetector` or a Material button.
    #[doc(hidden)]
    pub(crate) fn action_surface(surface: crate::internal::ActionSurface) -> Self {
        let semantic_label = if surface.label.is_empty() {
            surface.content.as_ref().and_then(Widget::semantic_text)
        } else {
            Some(surface.label.clone())
        };
        let label = surface.content.unwrap_or_else(|| {
            Widget::padding(
                surface.padding,
                Widget::text_styled(surface.label, surface.label_style, TextAlign::Start),
            )
        });
        Self {
            key: None,
            kind: WidgetKind::Button {
                size: surface.size,
                color: surface.color,
                hover_color: surface.hover_color,
                pressed_color: surface.pressed_color,
                focused_color: surface.focused_color,
                disabled_color: surface.disabled_color,
                enabled: surface.enabled,
                focusable_when_disabled: surface.focusable_when_disabled,
                action: ActionId(0),
                callback: surface.callback,
                hover_action: ActionId(0),
                hover_callback: surface.hover_callback,
                exit_action: ActionId(0),
                exit_callback: surface.exit_callback,
                has_callback: false,
                child: Some(std::rc::Rc::new(label)),
            },
            semantics: SemanticProperties {
                // Custom content is inspected for a text or explicit semantic
                // label so low-level controls retain a useful accessible name.
                label: semantic_label,
                ..SemanticProperties::default()
            },
        }
    }
    pub fn bind_callbacks(&mut self, allocate: &mut impl FnMut(Rc<dyn Fn()>) -> ActionId) {
        if let WidgetKind::Button {
            action,
            callback,
            hover_action,
            hover_callback,
            exit_action,
            exit_callback,
            has_callback,
            ..
        } = &mut self.kind
        {
            if let Some(callback) = callback.take() {
                *action = allocate(callback);
                *has_callback = true;
            }
            if let Some(callback) = hover_callback.take() {
                *hover_action = allocate(callback);
            }
            if let Some(callback) = exit_callback.take() {
                *exit_action = allocate(callback);
            }
        }
    }

    #[must_use]
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Text {
                text: text.into(),
                style: TextStyle::default(),
                align: TextAlign::Start,
                soft_wrap: true,
                max_lines: None,
                overflow: TextOverflow::Clip,
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn text_styled(text: impl Into<String>, style: TextStyle, align: TextAlign) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Text {
                text: text.into(),
                style,
                align,
                soft_wrap: true,
                max_lines: None,
                overflow: TextOverflow::Clip,
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn text_configured(
        text: impl Into<String>,
        style: TextStyle,
        align: TextAlign,
        soft_wrap: bool,
        max_lines: Option<usize>,
        overflow: TextOverflow,
    ) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Text {
                text: text.into(),
                style,
                align,
                soft_wrap,
                max_lines,
                overflow,
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn selectable_text_styled(
        text: impl Into<String>,
        style: TextStyle,
        align: TextAlign,
    ) -> Self {
        Self {
            key: None,
            kind: WidgetKind::SelectableText {
                text: text.into(),
                style,
                align,
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn selection_area(controller: SelectionAreaController, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::SelectionArea {
                controller,
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn selection_container(delegate: SelectionContainerDelegate, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::SelectionContainer {
                delegate,
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub(crate) fn selection_listener(notifier: SelectionListenerNotifier, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::SelectionListener {
                delegate: SelectionContainerDelegate::with_controller(notifier.controller()),
                notifier,
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub(crate) fn indexed_semantics(index: usize, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::IndexedSemantics {
                index,
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub(crate) fn semantics_debugger(
        label_style: TextStyle,
        max_nodes: usize,
        child: Self,
    ) -> Self {
        Self {
            key: None,
            kind: WidgetKind::SemanticsDebugger {
                label_style,
                max_nodes,
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    pub(crate) fn semantic_index(&self) -> Option<usize> {
        match &self.kind {
            WidgetKind::IndexedSemantics { index, .. } => Some(*index),
            _ => None,
        }
    }
    #[must_use]
    pub(super) fn image(
        image: ImageHandle,
        width: Option<f32>,
        height: Option<f32>,
        fit: ImageFit,
        repeat: ImageRepeat,
        alignment: Alignment,
        sampling: ImageSampling,
    ) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Image {
                image,
                width,
                height,
                fit,
                repeat,
                alignment,
                sampling,
            },
            semantics: SemanticProperties::default(),
        }
    }
    /// Creates the retained core editing primitive used by
    /// [`EditableText`]. Higher-level Material controls may add chrome and
    /// validation around this descriptor.
    #[must_use]
    pub fn editable_text(
        controller: TextEditingController,
        size: Size,
        style: TextStyle,
        placeholder: String,
        on_submit: Option<Rc<dyn Fn(String)>>,
        multiline: bool,
    ) -> Self {
        Self::editable_text_configured(
            controller,
            size,
            style,
            placeholder,
            on_submit,
            multiline,
            None,
            None,
            false,
            TextAlign::Start,
            true,
            false,
            false,
        )
    }

    /// Creates an editable text primitive with the common editing policies
    /// used by Material text fields.  The original [`Widget::editable_text`]
    /// constructor remains as the renderer-neutral, enabled single-line
    /// default; Material uses this richer boundary so read-only, obscured,
    /// alignment, and multiline sizing are real retained properties rather
    /// than decoration-only metadata.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn editable_text_configured(
        controller: TextEditingController,
        size: Size,
        style: TextStyle,
        placeholder: String,
        on_submit: Option<Rc<dyn Fn(String)>>,
        multiline: bool,
        min_lines: Option<usize>,
        max_lines: Option<usize>,
        expands: bool,
        text_align: TextAlign,
        enabled: bool,
        read_only: bool,
        obscure_text: bool,
    ) -> Self {
        Self::editable_text_configured_with_cursor(
            controller,
            size,
            style,
            placeholder,
            on_submit,
            multiline,
            min_lines,
            max_lines,
            expands,
            text_align,
            enabled,
            read_only,
            obscure_text,
            1.0,
            None,
            0.0,
            true,
            Color::WHITE,
            Color::rgba(72, 120, 220, 150),
        )
    }

    /// Creates the editable primitive with renderer-neutral caret and
    /// selection paint controls. Material uses this richer boundary for
    /// `TextField` cursor/selection configuration while the legacy constructor
    /// above keeps its source-compatible defaults.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn editable_text_configured_with_cursor(
        controller: TextEditingController,
        size: Size,
        style: TextStyle,
        placeholder: String,
        on_submit: Option<Rc<dyn Fn(String)>>,
        multiline: bool,
        min_lines: Option<usize>,
        max_lines: Option<usize>,
        expands: bool,
        text_align: TextAlign,
        enabled: bool,
        read_only: bool,
        obscure_text: bool,
        cursor_width: f32,
        cursor_height: Option<f32>,
        cursor_radius: f32,
        show_cursor: bool,
        cursor_color: Color,
        selection_color: Color,
    ) -> Self {
        Self {
            key: None,
            kind: WidgetKind::TextField {
                controller,
                size,
                style,
                placeholder,
                on_submit,
                multiline,
                min_lines: min_lines.map(|value| value.max(1)),
                max_lines: max_lines.map(|value| value.max(1)),
                expands,
                text_align,
                enabled,
                read_only,
                obscure_text,
                cursor_width: cursor_width.max(0.0),
                cursor_height: cursor_height.map(|value| value.max(0.0)),
                cursor_radius: cursor_radius.max(0.0),
                show_cursor,
                cursor_color,
                selection_color,
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn padding(padding: EdgeInsets, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Padding {
                padding,
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    /// Tightens the incoming layout bounds before passing them to `child`.
    #[must_use]
    pub fn constrained(constraints: Constraints, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Constrained {
                constraints,
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn limited_box(max_width: f32, max_height: f32, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Limited {
                max_width: finite_non_negative(max_width),
                max_height: finite_non_negative(max_height),
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn overflow_box(
        min_width: Option<f32>,
        max_width: Option<f32>,
        min_height: Option<f32>,
        max_height: Option<f32>,
        child: Self,
    ) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Overflow {
                min_width: min_width.map(finite_non_negative),
                max_width: max_width.map(finite_non_negative),
                min_height: min_height.map(finite_non_negative),
                max_height: max_height.map(finite_non_negative),
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    /// Lets a child take its natural size, optionally retaining the parent's
    /// limits on one axis while this wrapper itself still fits its parent.
    #[must_use]
    pub fn unconstrained(constrained_axis: Option<Axis>, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Unconstrained {
                constrained_axis,
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    /// Sizes a child to factors of the finite parent bounds on the specified
    /// axes. Missing factors preserve the child's natural size on that axis.
    #[must_use]
    pub fn fractionally_sized(
        width_factor: Option<f32>,
        height_factor: Option<f32>,
        child: Self,
    ) -> Self {
        for factor in [width_factor, height_factor].into_iter().flatten() {
            assert!(
                factor.is_finite() && factor >= 0.,
                "fractional factors must be finite and non-negative"
            );
        }
        Self {
            key: None,
            kind: WidgetKind::Fractional {
                width_factor,
                height_factor,
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    /// Positions a child so that its reported baseline is at `baseline`.
    #[must_use]
    pub fn baseline(baseline: f32, child: Self) -> Self {
        assert!(
            baseline.is_finite() && baseline >= 0.,
            "baseline must be finite and non-negative"
        );
        Self {
            key: None,
            kind: WidgetKind::Baseline {
                baseline,
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    /// Creates an explicit retained picture boundary around `child`.
    ///
    /// Descendant paint changes update their own cached picture without
    /// repainting this boundary's otherwise empty retained picture.
    #[must_use]
    pub fn repaint_boundary(child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::RepaintBoundary {
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    /// Attaches tap, double-tap, long-press, and pan recognition to a retained
    /// subtree. A hit-tested down event captures the sequence for this region.
    #[must_use]
    pub fn gesture(callbacks: GestureCallbacks, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Gesture {
                behavior: crate::gestures::HitTestBehavior::DeferToChild,
                callbacks: Box::new(callbacks),
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    pub(crate) fn draggable(source: Rc<dyn RetainedDragSource>, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Draggable {
                source,
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    pub(crate) fn drag_target(target: Rc<dyn RetainedDragTarget>, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::DragTarget {
                target,
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    /// Removes this subtree from pointer hit testing while leaving painting and
    /// semantics intact. Siblings behind it remain eligible for the event.
    #[must_use]
    pub fn ignore_pointer(ignoring: bool, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::IgnorePointer {
                ignoring,
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    /// Intercepts pointer hit testing at this boundary. Descendants do not
    /// receive ordinary retained interaction while painting and semantics are
    /// preserved.
    #[must_use]
    pub fn absorb_pointer(absorbing: bool, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::AbsorbPointer {
                absorbing,
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn align(alignment: Alignment, child: Self) -> Self {
        crate::layout::Align::new(alignment, child).into()
    }
    #[must_use]
    pub fn row(children: impl Into<Vec<Self>>) -> Self {
        crate::layout::Row::new(children.into())
            .main_axis_size(incular_config::MainAxisSize::Min)
            .cross_axis_alignment(incular_config::CrossAxisAlignment::Start)
            .into()
    }
    #[must_use]
    pub fn column(children: impl Into<Vec<Self>>) -> Self {
        crate::layout::Column::new(children.into())
            .main_axis_size(incular_config::MainAxisSize::Min)
            .cross_axis_alignment(incular_config::CrossAxisAlignment::Start)
            .into()
    }
    #[must_use]
    pub fn flex(direction: Axis, children: impl Into<Vec<Self>>) -> Self {
        crate::layout::Flex::new(direction, children.into()).into()
    }
    #[must_use]
    pub fn flexible(flex: u32, fit: incular_config::FlexFit, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Flexible {
                flex: flex.max(1),
                fit,
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    /// Packs children into successive runs when the main-axis bound is
    /// exhausted. The axis chooses whether runs flow horizontally or vertically.
    #[must_use]
    pub fn wrap(
        axis: Axis,
        spacing: f32,
        run_spacing: f32,
        children: impl Into<Vec<Self>>,
    ) -> Self {
        crate::layout::Wrap::new(children.into())
            .direction(axis)
            .spacing(spacing)
            .run_spacing(run_spacing)
            .into()
    }
    /// Places row-major children in max-content columns.
    #[must_use]
    pub fn table(
        columns: usize,
        column_spacing: f32,
        row_spacing: f32,
        children: impl Into<Vec<Self>>,
    ) -> Self {
        crate::layout::Table::new(columns, children.into())
            .column_spacing(column_spacing)
            .row_spacing(row_spacing)
            .into()
    }
    /// Paints children in order at the same origin. The last child is the
    /// front-most hit-test target, matching Flutter's stack semantics.
    #[must_use]
    pub fn stack(alignment: Alignment, children: impl Into<Vec<Self>>) -> Self {
        crate::layout::Stack::new(children.into())
            .alignment(alignment)
            .into()
    }
    #[must_use]
    pub fn positioned(
        left: Option<f32>,
        top: Option<f32>,
        right: Option<f32>,
        bottom: Option<f32>,
        width: Option<f32>,
        height: Option<f32>,
        child: Self,
    ) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Positioned {
                left: left.map(finite_non_negative),
                top: top.map(finite_non_negative),
                right: right.map(finite_non_negative),
                bottom: bottom.map(finite_non_negative),
                width: width.map(finite_non_negative),
                height: height.map(finite_non_negative),
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn indexed_stack(
        alignment: Alignment,
        index: usize,
        children: impl Into<Vec<Self>>,
    ) -> Self {
        Self {
            key: None,
            kind: WidgetKind::IndexedStack {
                alignment,
                index,
                children: children.into().into_iter().map(std::rc::Rc::new).collect(),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn layout_builder(builder: impl Fn(Constraints) -> Self + 'static) -> Self {
        Self {
            key: None,
            kind: WidgetKind::LayoutBuilder {
                builder: Rc::new(builder),
                environment: None,
                environment_boundary: false,
                revision: None,
            },
            semantics: SemanticProperties::default(),
        }
    }

    /// Wraps a child in an ambient retained builder environment. The wrapper
    /// is transparent to layout and paint; descendants read the value when
    /// their own deferred builders are materialized.
    #[must_use]
    pub fn environment_scope<T: Any>(value: T, child: Self) -> Self {
        let value: Rc<dyn Any> = Rc::new(value);
        Self {
            key: None,
            kind: WidgetKind::LayoutBuilder {
                builder: Rc::new(move |_| child.clone()),
                environment: Some(value),
                environment_boundary: false,
                revision: None,
            },
            semantics: SemanticProperties::default(),
        }
    }

    /// Creates a transparent retained node that prevents descendants from
    /// reading typed environments installed above it.
    #[must_use]
    pub fn environment_boundary(child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::LayoutBuilder {
                builder: Rc::new(move |_| child.clone()),
                environment: None,
                environment_boundary: true,
                revision: None,
            },
            semantics: SemanticProperties::default(),
        }
    }

    /// Creates a retained layout builder backed by an explicit local state
    /// revision.  A callback can increment `revision` and the next frame will
    /// rebuild only this builder's child, preserving the rest of the tree.
    /// This is the primitive used by uncontrolled controls such as checkbox,
    /// switch, toggle, and slider.
    #[must_use]
    pub fn stateful_layout_builder(
        revision: Rc<Cell<u64>>,
        builder: impl Fn(Constraints) -> Self + 'static,
    ) -> Self {
        Self {
            key: None,
            kind: WidgetKind::LayoutBuilder {
                builder: Rc::new(builder),
                environment: None,
                environment_boundary: false,
                revision: Some(revision),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn visibility(visible: bool, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Visibility {
                visible,
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn aspect_ratio(ratio: f32, child: Self) -> Self {
        assert!(
            ratio.is_finite() && ratio > 0.,
            "aspect ratio must be finite and positive"
        );
        Self {
            key: None,
            kind: WidgetKind::AspectRatio {
                ratio,
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn scroll_view(controller: ScrollController, child: Self) -> Self {
        Self::scroll_view_with_config(
            controller,
            child,
            Axis::Vertical,
            false,
            ScrollPhysics::default(),
        )
    }

    /// Creates a retained viewport with its complete scroll policy.  The
    /// public `scroll_view` helper intentionally keeps its historical
    /// vertical/clamping defaults; scroll descriptors use this crate-local
    /// constructor so axis, reverse, and physics survive lowering.
    #[must_use]
    pub(crate) fn scroll_view_with_config(
        controller: ScrollController,
        child: Self,
        axis: Axis,
        reverse: bool,
        physics: ScrollPhysics,
    ) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Scroll {
                controller,
                axis,
                reverse,
                physics,
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }

    /// Creates an interactive raw scrollbar overlay around an existing
    /// scrollable child. The controller remains the single source of truth
    /// for metrics and offset.
    #[must_use]
    pub fn raw_scrollbar(controller: ScrollController, child: impl Into<Self>) -> Self {
        Self::raw_scrollbar_with_style(controller, RawScrollbarStyle::default(), child)
    }

    /// Creates a raw scrollbar with explicit renderer-independent styling.
    #[must_use]
    pub fn raw_scrollbar_with_style(
        controller: ScrollController,
        style: RawScrollbarStyle,
        child: impl Into<Self>,
    ) -> Self {
        let child = child.into();
        Self {
            key: None,
            kind: WidgetKind::RawScrollbar {
                controller,
                style,
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }

    /// Creates a retained list-wheel viewport from its focused model.
    #[must_use]
    pub fn list_wheel_viewport(viewport: ListWheelViewport<Self>) -> Self {
        Self {
            key: None,
            kind: WidgetKind::ListWheelViewport {
                viewport: RetainedWheelViewport(Rc::new(RefCell::new(viewport))),
            },
            semantics: SemanticProperties::default(),
        }
    }

    /// Creates a retained list-wheel scroll view from its focused model.
    #[must_use]
    pub fn list_wheel_scroll_view(view: ListWheelScrollView<Self>) -> Self {
        Self {
            key: None,
            kind: WidgetKind::ListWheelScrollView {
                view: RetainedWheelScrollView(Rc::new(RefCell::new(view))),
            },
            semantics: SemanticProperties::default(),
        }
    }

    /// Creates a retained draggable sheet from its focused model.
    #[must_use]
    pub fn draggable_scrollable_sheet(sheet: DraggableScrollableSheet<Self>) -> Self {
        Self {
            key: None,
            kind: WidgetKind::DraggableScrollableSheet {
                sheet: RetainedDraggableSheet(Rc::new(RefCell::new(sheet))),
            },
            semantics: SemanticProperties::default(),
        }
    }

    /// Creates an actuator wrapper which resets the nearest descendant sheets.
    #[must_use]
    pub fn draggable_scrollable_actuator(
        actuator: DraggableScrollableActuator,
        child: impl Into<Self>,
    ) -> Self {
        Self {
            key: None,
            kind: WidgetKind::DraggableScrollableActuator {
                actuator: RetainedActuator(Rc::new(actuator)),
                child: std::rc::Rc::new(child.into()),
            },
            semantics: SemanticProperties::default(),
        }
    }

    /// Creates a retained two-dimensional viewport from its focused model.
    #[must_use]
    pub fn two_dimensional_viewport(viewport: TwoDimensionalViewport<Self>) -> Self {
        Self {
            key: None,
            kind: WidgetKind::TwoDimensionalViewport {
                viewport: RetainedTwoDimensionalViewport(Rc::new(RefCell::new(viewport))),
            },
            semantics: SemanticProperties::default(),
        }
    }

    /// Creates a retained two-dimensional scroll view from its focused model.
    #[must_use]
    pub fn two_dimensional_scroll_view(view: TwoDimensionalScrollView<Self>) -> Self {
        Self {
            key: None,
            kind: WidgetKind::TwoDimensionalScrollView {
                view: RetainedTwoDimensionalScrollView(Rc::new(RefCell::new(view))),
            },
            semantics: SemanticProperties::default(),
        }
    }
    /// Keeps this flow child at the leading edge of `controller`'s viewport
    /// once it reaches that edge. Consecutive persistent headers push their
    /// predecessors away instead of visually overlapping them.
    #[must_use]
    pub fn persistent_header(controller: ScrollController, child: Self) -> Self {
        Self::persistent_header_with_config(controller, child, Axis::Vertical, false, true)
    }
    /// Creates a persistent header with the axis and direction of its sliver
    /// viewport.  The plain helper retains the historical vertical defaults;
    /// sliver descriptors use this configured form during lowering.
    #[must_use]
    pub(crate) fn persistent_header_with_config(
        controller: ScrollController,
        child: Self,
        axis: Axis,
        reverse: bool,
        pinned: bool,
    ) -> Self {
        Self {
            key: None,
            kind: WidgetKind::PersistentHeader {
                controller,
                axis,
                reverse,
                pinned,
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }

    /// Creates a retained notification-listener wrapper. The wrapper has no
    /// visual effect; the tree installs the callback on descendant scroll
    /// positions after those positions are mounted.
    pub(crate) fn notification_listener(
        callback: Option<Rc<dyn Fn(ScrollNotification) -> bool>>,
        child: Self,
    ) -> Self {
        Self {
            key: None,
            kind: WidgetKind::NotificationListener {
                callback,
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }

    /// Creates a retained sliver viewport. Sliver children are materialized by
    /// the viewport delegate during layout, so they are not represented as a
    /// declarative `Column` child list.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn sliver_viewport_with_delegate_options(
        controller: ScrollController,
        axis: Axis,
        reverse: bool,
        physics: ScrollPhysics,
        cache_extent: f32,
        shrink_wrap: bool,
        clip_behavior: Clip,
        delegate: Rc<dyn SliverViewportDelegate>,
    ) -> Self {
        Self {
            key: None,
            kind: WidgetKind::SliverViewport {
                config: Rc::new(SliverViewportConfig {
                    controller,
                    axis,
                    reverse,
                    physics,
                    cache_extent: cache_extent.max(0.),
                    shrink_wrap,
                    clip_behavior,
                    delegate,
                }),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn translate(controller: TranslationController, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Translate {
                controller,
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    /// Applies an arbitrary Kurbo-backed affine transform after layout.
    /// The transform is compositor-only and defaults to the child's center.
    #[must_use]
    pub fn transform(transform: CoreTransform, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Transform {
                transform,
                origin: None,
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn transform_around(transform: CoreTransform, origin: Offset, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Transform {
                transform,
                origin: Some(finite_offset(origin)),
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn scale(scale: f32, child: Self) -> Self {
        Self::transform(CoreTransform::scale(scale), child)
    }
    #[must_use]
    pub fn rotate(radians: f32, child: Self) -> Self {
        Self::transform(CoreTransform::rotation(radians), child)
    }
    #[must_use]
    pub fn fitted_box(fit: ImageFit, alignment: Alignment, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::FittedBox {
                fit,
                alignment,
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn controlled_scale(controller: ScaleController, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Scale {
                controller,
                origin: None,
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn controlled_rotation(controller: RotationController, child: Self) -> Self {
        Self::controlled_rotation_with_alignment(controller, None, child)
    }
    #[must_use]
    pub(crate) fn controlled_rotation_with_alignment(
        controller: RotationController,
        alignment: Option<Alignment>,
        child: Self,
    ) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Rotation {
                controller,
                origin: None,
                alignment,
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn opacity(alpha: f32, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Opacity {
                alpha: normalize_opacity(alpha),
                controller: None,
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn controlled_opacity(controller: OpacityController, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Opacity {
                alpha: controller.opacity(),
                controller: Some(controller),
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn blur(sigma: f32, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Blur {
                sigma_x: normalize_sigma(sigma),
                sigma_y: normalize_sigma(sigma),
                controller: None,
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn asymmetric_blur(sigma_x: f32, sigma_y: f32, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Blur {
                sigma_x: normalize_sigma(sigma_x),
                sigma_y: normalize_sigma(sigma_y),
                controller: None,
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn controlled_blur(controller: BlurController, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Blur {
                sigma_x: controller.sigma(),
                sigma_y: controller.sigma(),
                controller: Some(controller),
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn drop_shadow(offset: Offset, sigma: f32, color: Color, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::DropShadow {
                offset: finite_offset(offset),
                sigma_x: normalize_sigma(sigma),
                sigma_y: normalize_sigma(sigma),
                color,
                controller: None,
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn controlled_drop_shadow(controller: DropShadowController, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::DropShadow {
                offset: controller.offset(),
                sigma_x: controller.sigma(),
                sigma_y: controller.sigma(),
                color: controller.color(),
                controller: Some(controller),
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn color_filtered(filter: ColorFilter, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::ColorFiltered {
                filter,
                controller: None,
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn color_matrix(filter: ColorFilter, child: Self) -> Self {
        Self::color_filtered(filter, child)
    }
    #[must_use]
    pub fn controlled_color_filtered(controller: ColorFilterController, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::ColorFiltered {
                filter: controller.filter(),
                controller: Some(controller),
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn blend(mode: BlendMode, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Blend {
                mode,
                child: std::rc::Rc::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn with_key(mut self, key: impl Into<Key>) -> Self {
        self.key = Some(key.into());
        self
    }
    /// Overrides the accessible label contributed by this meaningful widget.
    #[must_use]
    pub fn accessibility_label(mut self, label: impl Into<String>) -> Self {
        self.semantics.label = Some(label.into());
        self
    }
    /// Adds a screen-reader description without changing visible text.
    #[must_use]
    pub fn accessibility_description(mut self, description: impl Into<String>) -> Self {
        self.semantics.description = Some(description.into());
        self
    }
    /// Supplies explicit Incular semantic metadata for this visual widget.
    /// This is useful for icon-only controls, meaningful images, headings,
    /// dialogs, and custom-painted controls; it never exposes native adapter
    /// types to application code.
    #[must_use]
    pub fn semantics(mut self, semantics: ExplicitSemantics) -> Self {
        self.semantics.explicit = Some(semantics);
        self
    }
    /// Excludes this widget and its implementation-detail subtree from semantics.
    #[must_use]
    pub fn exclude_semantics(mut self) -> Self {
        self.semantics.hidden = true;
        self
    }
    /// Merges meaningful descendants into one logical accessible node. The
    /// widget itself must have explicit semantics or a meaningful built-in
    /// role; descendants are not exposed separately.
    #[must_use]
    pub fn merge_semantics(mut self) -> Self {
        self.semantics.merge_descendants = true;
        self
    }
    /// Suppresses preceding semantic siblings at this stacking level. Use it
    /// for a modal/dialog region so screen-reader traversal cannot fall through
    /// to visual content behind the active modal.
    #[must_use]
    pub fn block_semantics(mut self) -> Self {
        self.semantics.block_previous_siblings = true;
        self
    }
    #[must_use]
    pub fn key(&self) -> Option<&Key> {
        self.key.as_ref()
    }
    pub(super) fn type_(&self) -> WidgetType {
        match &self.kind {
            WidgetKind::Box { .. } => WidgetType::Box,
            WidgetKind::Shape { .. } => WidgetType::Shape,
            WidgetKind::CustomPaint { .. } => WidgetType::CustomPaint,
            WidgetKind::Decorated { .. } => WidgetType::Decorated,
            WidgetKind::Banner { .. } => WidgetType::Banner,
            WidgetKind::Image { .. } => WidgetType::Image,
            WidgetKind::Button { .. } => WidgetType::Button,
            WidgetKind::Text { .. } => WidgetType::Text,
            WidgetKind::SelectableText { .. } => WidgetType::SelectableText,
            WidgetKind::SelectionArea { .. } => WidgetType::SelectionArea,
            WidgetKind::SelectionContainer { .. } => WidgetType::SelectionContainer,
            WidgetKind::SelectionListener { .. } => WidgetType::SelectionListener,
            WidgetKind::IndexedSemantics { .. } => WidgetType::IndexedSemantics,
            WidgetKind::SemanticsDebugger { .. } => WidgetType::SemanticsDebugger,
            WidgetKind::TextField { .. } => WidgetType::TextField,
            WidgetKind::Padding { .. } => WidgetType::Padding,
            WidgetKind::Constrained { .. } => WidgetType::Constrained,
            WidgetKind::Limited { .. } => WidgetType::Limited,
            WidgetKind::Overflow { .. } => WidgetType::Overflow,
            WidgetKind::Unconstrained { .. } => WidgetType::Unconstrained,
            WidgetKind::Fractional { .. } => WidgetType::Fractional,
            WidgetKind::Baseline { .. } => WidgetType::Baseline,
            WidgetKind::RepaintBoundary { .. } => WidgetType::RepaintBoundary,
            WidgetKind::Gesture { .. } => WidgetType::Gesture,
            WidgetKind::RawInput { kind, .. } => kind.type_(),
            WidgetKind::Draggable { .. } => WidgetType::Draggable,
            WidgetKind::DragTarget { .. } => WidgetType::DragTarget,
            WidgetKind::IgnorePointer { .. } => WidgetType::IgnorePointer,
            WidgetKind::AbsorbPointer { .. } => WidgetType::AbsorbPointer,
            WidgetKind::Align { .. } => WidgetType::Align,
            WidgetKind::Flex { .. } => WidgetType::Flex,
            WidgetKind::Flexible { .. } => WidgetType::Flexible,
            WidgetKind::Wrap { .. } => WidgetType::Wrap,
            WidgetKind::Table { .. } => WidgetType::Table,
            WidgetKind::Stack { .. } => WidgetType::Stack,
            WidgetKind::Positioned { .. } => WidgetType::Positioned,
            WidgetKind::IndexedStack { .. } => WidgetType::IndexedStack,
            WidgetKind::SafeArea { .. } => WidgetType::SafeArea,
            WidgetKind::ClipRect { .. } => WidgetType::ClipRect,
            WidgetKind::ClipRRect { .. } => WidgetType::ClipRRect,
            WidgetKind::ClipOval { .. } => WidgetType::ClipOval,
            WidgetKind::ClipPath { .. } => WidgetType::ClipPath,
            WidgetKind::LayoutBuilder { .. } => WidgetType::LayoutBuilder,
            WidgetKind::Visibility { .. } => WidgetType::Visibility,
            WidgetKind::AspectRatio { .. } => WidgetType::AspectRatio,
            WidgetKind::Scroll { .. } => WidgetType::Scroll,
            WidgetKind::RawScrollbar { .. } => WidgetType::RawScrollbar,
            WidgetKind::ListWheelScrollView { .. } => WidgetType::ListWheelScrollView,
            WidgetKind::ListWheelViewport { .. } => WidgetType::ListWheelViewport,
            WidgetKind::DraggableScrollableSheet { .. } => WidgetType::DraggableScrollableSheet,
            WidgetKind::DraggableScrollableActuator { .. } => {
                WidgetType::DraggableScrollableActuator
            }
            WidgetKind::TwoDimensionalScrollView { .. } => WidgetType::TwoDimensionalScrollView,
            WidgetKind::TwoDimensionalViewport { .. } => WidgetType::TwoDimensionalViewport,
            WidgetKind::PersistentHeader { .. } => WidgetType::PersistentHeader,
            WidgetKind::NotificationListener { .. } => WidgetType::NotificationListener,
            WidgetKind::SliverViewport { .. } => WidgetType::SliverViewport,
            WidgetKind::Translate { .. } => WidgetType::Translate,
            WidgetKind::Transform { .. } => WidgetType::Transform,
            WidgetKind::Scale { .. } => WidgetType::Scale,
            WidgetKind::Rotation { .. } => WidgetType::Rotation,
            WidgetKind::FittedBox { .. } => WidgetType::FittedBox,
            WidgetKind::Opacity { .. } => WidgetType::Opacity,
            WidgetKind::Blur { .. } => WidgetType::Blur,
            WidgetKind::DropShadow { .. } => WidgetType::DropShadow,
            WidgetKind::ColorFiltered { .. } => WidgetType::ColorFiltered,
            WidgetKind::Blend { .. } => WidgetType::Blend,
            WidgetKind::ShaderMask { .. } => WidgetType::ShaderMask,
            WidgetKind::BackdropFilter { .. } => WidgetType::BackdropFilter,
            WidgetKind::AnnotatedRegion { .. } => WidgetType::AnnotatedRegion,
            WidgetKind::CompositedTransformTarget { .. } => WidgetType::CompositedTransformTarget,
            WidgetKind::CompositedTransformFollower { .. } => {
                WidgetType::CompositedTransformFollower
            }
        }
    }
    /// Shallow child view for reconciliation. Deep per-child clones were the
    /// measured allocation fire on wide trees (Task 15); reconciliation only
    /// needs references because cloning happens once per *created* element.
    pub(super) fn children_refs(&self) -> Vec<&Widget> {
        match &self.kind {
            WidgetKind::Box { .. }
            | WidgetKind::Shape { .. }
            | WidgetKind::CustomPaint { .. }
            | WidgetKind::Text { .. }
            | WidgetKind::SelectableText { .. }
            | WidgetKind::TextField { .. }
            | WidgetKind::Image { .. } => Vec::new(),
            WidgetKind::Button { child, .. } => child.iter().map(|c| c.as_ref()).collect(),
            WidgetKind::Banner { child, .. } => child.iter().map(|c| c.as_ref()).collect(),
            WidgetKind::Padding { child, .. }
            | WidgetKind::Constrained { child, .. }
            | WidgetKind::Limited { child, .. }
            | WidgetKind::Overflow { child, .. }
            | WidgetKind::Unconstrained { child, .. }
            | WidgetKind::Fractional { child, .. }
            | WidgetKind::Baseline { child, .. }
            | WidgetKind::RepaintBoundary { child, .. }
            | WidgetKind::Gesture { child, .. }
            | WidgetKind::Draggable { child, .. }
            | WidgetKind::DragTarget { child, .. }
            | WidgetKind::IgnorePointer { child, .. }
            | WidgetKind::AbsorbPointer { child, .. }
            | WidgetKind::Align { child, .. }
            | WidgetKind::Flexible { child, .. }
            | WidgetKind::Positioned { child, .. }
            | WidgetKind::SafeArea { child, .. }
            | WidgetKind::ClipRect { child, .. }
            | WidgetKind::ClipRRect { child, .. }
            | WidgetKind::ClipOval { child, .. }
            | WidgetKind::ClipPath { child, .. }
            | WidgetKind::Visibility { child, .. }
            | WidgetKind::AspectRatio { child, .. }
            | WidgetKind::Scroll { child, .. }
            | WidgetKind::RawScrollbar { child, .. }
            | WidgetKind::DraggableScrollableActuator { child, .. }
            | WidgetKind::PersistentHeader { child, .. }
            | WidgetKind::NotificationListener { child, .. }
            | WidgetKind::Translate { child, .. }
            | WidgetKind::Transform { child, .. }
            | WidgetKind::Scale { child, .. }
            | WidgetKind::Rotation { child, .. }
            | WidgetKind::FittedBox { child, .. }
            | WidgetKind::Decorated { child, .. }
            | WidgetKind::Opacity { child, .. }
            | WidgetKind::Blur { child, .. }
            | WidgetKind::DropShadow { child, .. }
            | WidgetKind::ColorFiltered { child, .. }
            | WidgetKind::Blend { child, .. }
            | WidgetKind::ShaderMask { child, .. }
            | WidgetKind::BackdropFilter { child, .. }
            | WidgetKind::AnnotatedRegion { child, .. }
            | WidgetKind::CompositedTransformTarget { child, .. }
            | WidgetKind::CompositedTransformFollower { child, .. } => vec![child.as_ref()],
            WidgetKind::RawInput { child, .. } => {
                child.iter().map(|child| child.as_ref()).collect()
            }
            WidgetKind::SelectionArea { child, .. }
            | WidgetKind::SelectionContainer { child, .. }
            | WidgetKind::SelectionListener { child, .. }
            | WidgetKind::IndexedSemantics { child, .. }
            | WidgetKind::SemanticsDebugger { child, .. } => vec![child.as_ref()],
            WidgetKind::Flex { children, .. }
            | WidgetKind::Wrap { children, .. }
            | WidgetKind::Table { children, .. }
            | WidgetKind::Stack { children, .. }
            | WidgetKind::IndexedStack { children, .. } => {
                children.iter().map(Rc::as_ref).collect()
            }
            WidgetKind::ListWheelScrollView { .. }
            | WidgetKind::ListWheelViewport { .. }
            | WidgetKind::DraggableScrollableSheet { .. }
            | WidgetKind::TwoDimensionalScrollView { .. }
            | WidgetKind::TwoDimensionalViewport { .. }
            | WidgetKind::SliverViewport { .. }
            | WidgetKind::LayoutBuilder { .. } => Vec::new(),
        }
    }
}

impl RawScrollbar {
    /// Lowers this stateful scrollbar model into a retained widget overlay.
    #[must_use]
    pub fn into_widget(self, child: impl Into<Widget>) -> Widget {
        Widget::raw_scrollbar_with_style(self.controller(), self.style(), child)
    }

    /// Alias for [`Self::into_widget`] with Flutter-style wrapper wording.
    #[must_use]
    pub fn with_child(self, child: impl Into<Widget>) -> Widget {
        self.into_widget(child)
    }
}

impl From<RawScrollbar> for Widget {
    fn from(value: RawScrollbar) -> Self {
        value.into_widget(Widget::box_(Size::ZERO, Color::TRANSPARENT))
    }
}

impl From<ListWheelViewport<Widget>> for Widget {
    fn from(value: ListWheelViewport<Widget>) -> Self {
        Widget::list_wheel_viewport(value)
    }
}

impl From<ListWheelScrollView<Widget>> for Widget {
    fn from(value: ListWheelScrollView<Widget>) -> Self {
        Widget::list_wheel_scroll_view(value)
    }
}

impl From<DraggableScrollableSheet<Widget>> for Widget {
    fn from(value: DraggableScrollableSheet<Widget>) -> Self {
        Widget::draggable_scrollable_sheet(value)
    }
}

impl DraggableScrollableActuator {
    /// Lowers this reset channel into a transparent retained wrapper.
    #[must_use]
    pub fn with_child(self, child: impl Into<Widget>) -> Widget {
        Widget::draggable_scrollable_actuator(self, child)
    }
}

impl From<DraggableScrollableActuator> for Widget {
    fn from(value: DraggableScrollableActuator) -> Self {
        value.with_child(Widget::box_(Size::ZERO, Color::TRANSPARENT))
    }
}

impl From<TwoDimensionalViewport<Widget>> for Widget {
    fn from(value: TwoDimensionalViewport<Widget>) -> Self {
        Widget::two_dimensional_viewport(value)
    }
}

impl From<TwoDimensionalScrollView<Widget>> for Widget {
    fn from(value: TwoDimensionalScrollView<Widget>) -> Self {
        Widget::two_dimensional_scroll_view(value)
    }
}
