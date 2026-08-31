//! Declarative widget values, kinds, and constructors.
//!
//! This module owns the immutable widget description consumed by the retained tree.

use super::*;

/// Immutable declarative descriptor node shared by cheap [`Widget`] handles.
///
/// The type is doc-hidden because `Widget` is the public transport value; the
/// node exists only as the reference-counted ownership boundary. Its fields are
/// crate-private so framework implementation code can keep exhaustive matching
/// local without exposing representation details to applications.
#[doc(hidden)]
#[derive(Clone)]
pub struct WidgetNode {
    pub(crate) key: Option<Key>,
    pub(crate) kind: WidgetKind,
    pub(crate) semantics: SemanticProperties,
}

/// A cheap immutable declarative widget handle.
///
/// Cloning a `Widget` clones one reference-counted descriptor pointer; retained
/// identity continues to belong exclusively to [`Element`].
pub struct Widget {
    node: Option<Rc<WidgetNode>>,
}

impl Clone for Widget {
    fn clone(&self) -> Self {
        Self {
            node: self.node.clone(),
        }
    }
}

impl std::ops::Deref for Widget {
    type Target = WidgetNode;

    fn deref(&self) -> &Self::Target {
        self.node.as_deref().expect("live widget descriptor handle")
    }
}

impl std::ops::DerefMut for Widget {
    fn deref_mut(&mut self) -> &mut Self::Target {
        Rc::make_mut(self.node.as_mut().expect("live widget descriptor handle"))
    }
}

struct WidgetDropQueue {
    active: bool,
    pending: Vec<Rc<WidgetNode>>,
}

thread_local! {
    static WIDGET_DROP_QUEUE: RefCell<WidgetDropQueue> = const {
        RefCell::new(WidgetDropQueue {
            active: false,
            pending: Vec::new(),
        })
    };
}

impl Drop for Widget {
    fn drop(&mut self) {
        let Some(node) = self.node.take() else {
            return;
        };
        WIDGET_DROP_QUEUE.with(|queue| {
            {
                let mut queue = queue.borrow_mut();
                queue.pending.push(node);
                if queue.active {
                    return;
                }
                queue.active = true;
            }

            struct ResetDropQueue<'a>(&'a RefCell<WidgetDropQueue>);
            impl Drop for ResetDropQueue<'_> {
                fn drop(&mut self) {
                    self.0.borrow_mut().active = false;
                }
            }
            let _reset = ResetDropQueue(queue);

            loop {
                let next = queue.borrow_mut().pending.pop();
                let Some(node) = next else {
                    break;
                };
                match Rc::try_unwrap(node) {
                    // Dropping the unique node invokes Drop for its child Widget
                    // handles. Because this queue is active, those drops enqueue
                    // their nodes instead of recursively destroying them.
                    Ok(node) => drop(node),
                    // Another descriptor/clone still owns this node. Releasing
                    // this edge cannot destroy its descendants yet.
                    Err(node) => drop(node),
                }
            }
        });
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
            Self::Button(spec) => f
                .debug_struct("Button")
                .field("size", &spec.size)
                .field("color", &spec.color)
                .field("action", &spec.action)
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
            Self::TextField(spec) => f
                .debug_struct("TextField")
                .field("placeholder", &spec.placeholder)
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
            Self::ListWheelScrollView { config } => f
                .debug_struct("ListWheelScrollView")
                .field("config", config)
                .finish(),
            Self::ListWheelViewport { config } => f
                .debug_struct("ListWheelViewport")
                .field("config", config)
                .finish(),
            Self::DraggableScrollableSheet { config } => f
                .debug_struct("DraggableScrollableSheet")
                .field("config", config)
                .finish(),
            Self::DraggableScrollableActuator { actuator, child } => f
                .debug_struct("DraggableScrollableActuator")
                .field("actuator", actuator)
                .field("child", child)
                .finish(),
            Self::TwoDimensionalScrollView { config } => f
                .debug_struct("TwoDimensionalScrollView")
                .field("config", config)
                .finish(),
            Self::TwoDimensionalViewport { config } => f
                .debug_struct("TwoDimensionalViewport")
                .field("config", config)
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
fn same_optional_callback<T: ?Sized>(left: &Option<Rc<T>>, right: &Option<Rc<T>>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => Rc::ptr_eq(left, right),
        (None, None) => true,
        _ => false,
    }
}

impl PartialEq for ButtonSpec {
    fn eq(&self, other: &Self) -> bool {
        self.size == other.size
            && self.color == other.color
            && self.hover_color == other.hover_color
            && self.pressed_color == other.pressed_color
            && self.focused_color == other.focused_color
            && self.disabled_color == other.disabled_color
            && self.enabled == other.enabled
            && self.focusable_when_disabled == other.focusable_when_disabled
            && self.action == other.action
            && self.hover_action == other.hover_action
            && self.exit_action == other.exit_action
            && self.has_callback == other.has_callback
            && self.child == other.child
            && same_optional_callback(&self.callback, &other.callback)
            && same_optional_callback(&self.hover_callback, &other.hover_callback)
            && same_optional_callback(&self.exit_callback, &other.exit_callback)
    }
}

impl PartialEq for TextFieldSpec {
    fn eq(&self, other: &Self) -> bool {
        self.controller == other.controller
            && self.size == other.size
            && self.style == other.style
            && self.placeholder == other.placeholder
            && self.multiline == other.multiline
            && self.min_lines == other.min_lines
            && self.max_lines == other.max_lines
            && self.expands == other.expands
            && self.text_align == other.text_align
            && self.enabled == other.enabled
            && self.read_only == other.read_only
            && self.obscure_text == other.obscure_text
            && self.cursor_width == other.cursor_width
            && self.cursor_height == other.cursor_height
            && self.cursor_radius == other.cursor_radius
            && self.show_cursor == other.show_cursor
            && self.cursor_color == other.cursor_color
            && self.selection_color == other.selection_color
            && same_optional_callback(&self.on_submit, &other.on_submit)
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
            (Self::Button(left), Self::Button(right)) => left == right,
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
            (Self::TextField(left), Self::TextField(right)) => left == right,
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
            (Self::ListWheelScrollView { config: a }, Self::ListWheelScrollView { config: b }) => {
                a == b
            }
            (Self::ListWheelViewport { config: a }, Self::ListWheelViewport { config: b }) => {
                a == b
            }
            (
                Self::DraggableScrollableSheet { config: a },
                Self::DraggableScrollableSheet { config: b },
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
                Self::TwoDimensionalScrollView { config: a },
                Self::TwoDimensionalScrollView { config: b },
            ) => a == b,
            (
                Self::TwoDimensionalViewport { config: a },
                Self::TwoDimensionalViewport { config: b },
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
                        (Some(x), Some(y)) => {
                            x.type_id == y.type_id && Rc::ptr_eq(&x.value, &y.value)
                        }
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
                callback_equal && b == d
            }
            _ => false,
        }
    }
}

fn gesture_callbacks_eq(left: &GestureCallbacks, right: &GestureCallbacks) -> bool {
    same_optional_callback(&left.on_tap, &right.on_tap)
        && same_optional_callback(&left.on_tap_down, &right.on_tap_down)
        && same_optional_callback(&left.on_tap_up, &right.on_tap_up)
        && same_optional_callback(&left.on_tap_cancel, &right.on_tap_cancel)
        && same_optional_callback(&left.on_double_tap, &right.on_double_tap)
        && same_optional_callback(&left.on_double_tap_down, &right.on_double_tap_down)
        && same_optional_callback(&left.on_double_tap_cancel, &right.on_double_tap_cancel)
        && same_optional_callback(&left.on_long_press, &right.on_long_press)
        && same_optional_callback(&left.on_long_press_start, &right.on_long_press_start)
        && same_optional_callback(
            &left.on_long_press_move_update,
            &right.on_long_press_move_update,
        )
        && same_optional_callback(&left.on_long_press_up, &right.on_long_press_up)
        && same_optional_callback(&left.on_long_press_end, &right.on_long_press_end)
        && same_optional_callback(&left.on_pan_down, &right.on_pan_down)
        && same_optional_callback(&left.on_pan_start, &right.on_pan_start)
        && same_optional_callback(&left.on_pan_update, &right.on_pan_update)
        && same_optional_callback(&left.on_pan_end, &right.on_pan_end)
        && same_optional_callback(&left.on_pan_cancel, &right.on_pan_cancel)
        && same_optional_callback(
            &left.on_horizontal_drag_down,
            &right.on_horizontal_drag_down,
        )
        && same_optional_callback(
            &left.on_horizontal_drag_start,
            &right.on_horizontal_drag_start,
        )
        && same_optional_callback(
            &left.on_horizontal_drag_update,
            &right.on_horizontal_drag_update,
        )
        && same_optional_callback(&left.on_horizontal_drag_end, &right.on_horizontal_drag_end)
        && same_optional_callback(
            &left.on_horizontal_drag_cancel,
            &right.on_horizontal_drag_cancel,
        )
        && same_optional_callback(&left.on_vertical_drag_down, &right.on_vertical_drag_down)
        && same_optional_callback(&left.on_vertical_drag_start, &right.on_vertical_drag_start)
        && same_optional_callback(
            &left.on_vertical_drag_update,
            &right.on_vertical_drag_update,
        )
        && same_optional_callback(&left.on_vertical_drag_end, &right.on_vertical_drag_end)
        && same_optional_callback(
            &left.on_vertical_drag_cancel,
            &right.on_vertical_drag_cancel,
        )
        && same_optional_callback(&left.on_scale_start, &right.on_scale_start)
        && same_optional_callback(&left.on_scale_update, &right.on_scale_update)
        && same_optional_callback(&left.on_scale_end, &right.on_scale_end)
        && same_optional_callback(&left.on_cancel, &right.on_cancel)
        && same_optional_callback(&left.on_key, &right.on_key)
        && same_optional_callback(&left.on_key_down, &right.on_key_down)
        && same_optional_callback(&left.on_key_repeat, &right.on_key_repeat)
        && same_optional_callback(&left.on_key_up, &right.on_key_up)
        && same_optional_callback(&left.on_shortcut, &right.on_shortcut)
        && same_optional_callback(&left.shortcut_scope, &right.shortcut_scope)
        && same_optional_callback(&left.action_scope, &right.action_scope)
        && same_optional_callback(&left.action_listener, &right.action_listener)
        && same_optional_callback(
            &left.action_invocation_listener,
            &right.action_invocation_listener,
        )
        && left.focus_node == right.focus_node
        && same_optional_callback(&left.focus_behavior, &right.focus_behavior)
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

/// Coarse lowering family used to route a declarative descriptor to one
/// focused lowering module. This classification is structural metadata: it is
/// defined alongside child topology and widget type so adding a built-in
/// widget has one authoritative classification point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LoweringFamily {
    Visual,
    Layout,
    Scrolling,
    Effects,
}

/// Declarative child topology. Dynamic children are materialized by retained
/// protocols (layout builders, slivers and advanced viewports) rather than
/// appearing as ordinary descriptor children.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChildShape {
    None,
    Optional,
    Single,
    Many,
    Dynamic,
}

/// Allocation-free borrowed view of the ordinary declarative child edges of a
/// widget descriptor.
pub(crate) enum WidgetChildren<'a> {
    None,
    Optional(Option<&'a Widget>),
    Single(&'a Widget),
    Many(&'a [Widget]),
    Dynamic,
}

impl WidgetChildren<'_> {
    pub(crate) fn shape(&self) -> ChildShape {
        match self {
            Self::None => ChildShape::None,
            Self::Optional(_) => ChildShape::Optional,
            Self::Single(_) => ChildShape::Single,
            Self::Many(_) => ChildShape::Many,
            Self::Dynamic => ChildShape::Dynamic,
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub(super) fn len(&self) -> usize {
        match self {
            Self::None | Self::Dynamic => 0,
            Self::Optional(child) => usize::from(child.is_some()),
            Self::Single(_) => 1,
            Self::Many(children) => children.len(),
        }
    }
}

pub(crate) enum WidgetChildrenIter<'a> {
    None,
    One(Option<&'a Widget>),
    Many(std::slice::Iter<'a, Widget>),
}

impl<'a> Iterator for WidgetChildrenIter<'a> {
    type Item = &'a Widget;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::None => None,
            Self::One(child) => child.take(),
            Self::Many(children) => children.next(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = match self {
            Self::None => 0,
            Self::One(child) => usize::from(child.is_some()),
            Self::Many(children) => children.len(),
        };
        (len, Some(len))
    }
}

impl ExactSizeIterator for WidgetChildrenIter<'_> {}

impl<'a> IntoIterator for WidgetChildren<'a> {
    type Item = &'a Widget;
    type IntoIter = WidgetChildrenIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            Self::None | Self::Dynamic => WidgetChildrenIter::None,
            Self::Optional(child) => WidgetChildrenIter::One(child),
            Self::Single(child) => WidgetChildrenIter::One(Some(child)),
            Self::Many(children) => WidgetChildrenIter::Many(children.iter()),
        }
    }
}

pub(crate) struct WidgetStructure<'a> {
    pub(crate) widget_type: WidgetType,
    pub(crate) lowering_family: LoweringFamily,
    pub(crate) children: WidgetChildren<'a>,
}

impl WidgetKind {
    pub(crate) fn structure(&self) -> WidgetStructure<'_> {
        use ChildShape::{Dynamic, Many, None, Optional, Single};
        use LoweringFamily::{Effects, Layout, Scrolling, Visual};
        use WidgetChildren::{
            Dynamic as DynamicChildren, Many as ManyChildren, None as NoChildren,
        };

        let (widget_type, lowering_family, child_shape, children) = match self {
            WidgetKind::Box { .. } => (WidgetType::Box, Visual, None, NoChildren),
            WidgetKind::Shape { .. } => (WidgetType::Shape, Visual, None, NoChildren),
            WidgetKind::CustomPaint { .. } => (WidgetType::CustomPaint, Visual, None, NoChildren),
            WidgetKind::Decorated { child, .. } => (
                WidgetType::Decorated,
                Visual,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::Banner { child, .. } => (
                WidgetType::Banner,
                Visual,
                Optional,
                WidgetChildren::Optional(child.as_ref()),
            ),
            WidgetKind::Button(spec) => (
                WidgetType::Button,
                Visual,
                Optional,
                WidgetChildren::Optional(spec.child.as_ref()),
            ),
            WidgetKind::Text { .. } => (WidgetType::Text, Visual, None, NoChildren),
            WidgetKind::SelectableText { .. } => {
                (WidgetType::SelectableText, Visual, None, NoChildren)
            }
            WidgetKind::SelectionArea { child, .. } => (
                WidgetType::SelectionArea,
                Visual,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::SelectionContainer { child, .. } => (
                WidgetType::SelectionContainer,
                Visual,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::SelectionListener { child, .. } => (
                WidgetType::SelectionListener,
                Visual,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::IndexedSemantics { child, .. } => (
                WidgetType::IndexedSemantics,
                Visual,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::SemanticsDebugger { child, .. } => (
                WidgetType::SemanticsDebugger,
                Visual,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::Image { .. } => (WidgetType::Image, Visual, None, NoChildren),
            WidgetKind::TextField(_) => (WidgetType::TextField, Visual, None, NoChildren),

            WidgetKind::Padding { child, .. } => (
                WidgetType::Padding,
                Layout,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::Constrained { child, .. } => (
                WidgetType::Constrained,
                Layout,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::Limited { child, .. } => (
                WidgetType::Limited,
                Layout,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::Overflow { child, .. } => (
                WidgetType::Overflow,
                Layout,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::Unconstrained { child, .. } => (
                WidgetType::Unconstrained,
                Layout,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::Fractional { child, .. } => (
                WidgetType::Fractional,
                Layout,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::Baseline { child, .. } => (
                WidgetType::Baseline,
                Layout,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::RepaintBoundary { child } => (
                WidgetType::RepaintBoundary,
                Layout,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::Gesture { child, .. } => (
                WidgetType::Gesture,
                Layout,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::RawInput { kind, child } => (
                kind.type_(),
                Layout,
                Optional,
                WidgetChildren::Optional(child.as_ref()),
            ),
            WidgetKind::Draggable { child, .. } => (
                WidgetType::Draggable,
                Layout,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::DragTarget { child, .. } => (
                WidgetType::DragTarget,
                Layout,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::IgnorePointer { child, .. } => (
                WidgetType::IgnorePointer,
                Layout,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::AbsorbPointer { child, .. } => (
                WidgetType::AbsorbPointer,
                Layout,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::Align { child, .. } => (
                WidgetType::Align,
                Layout,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::Flex { children, .. } => {
                (WidgetType::Flex, Layout, Many, ManyChildren(children))
            }
            WidgetKind::Flexible { child, .. } => (
                WidgetType::Flexible,
                Layout,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::Wrap { children, .. } => {
                (WidgetType::Wrap, Layout, Many, ManyChildren(children))
            }
            WidgetKind::Table { children, .. } => {
                (WidgetType::Table, Layout, Many, ManyChildren(children))
            }
            WidgetKind::Stack { children, .. } => {
                (WidgetType::Stack, Layout, Many, ManyChildren(children))
            }
            WidgetKind::Positioned { child, .. } => (
                WidgetType::Positioned,
                Layout,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::IndexedStack { children, .. } => (
                WidgetType::IndexedStack,
                Layout,
                Many,
                ManyChildren(children),
            ),
            WidgetKind::SafeArea { child, .. } => (
                WidgetType::SafeArea,
                Layout,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::ClipRect { child, .. } => (
                WidgetType::ClipRect,
                Layout,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::ClipRRect { child, .. } => (
                WidgetType::ClipRRect,
                Layout,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::ClipOval { child, .. } => (
                WidgetType::ClipOval,
                Layout,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::ClipPath { child, .. } => (
                WidgetType::ClipPath,
                Layout,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::LayoutBuilder { .. } => {
                (WidgetType::LayoutBuilder, Layout, Dynamic, DynamicChildren)
            }
            WidgetKind::Visibility { child, .. } => (
                WidgetType::Visibility,
                Layout,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::AspectRatio { child, .. } => (
                WidgetType::AspectRatio,
                Layout,
                Single,
                WidgetChildren::Single(child),
            ),

            WidgetKind::Scroll { child, .. } => (
                WidgetType::Scroll,
                Scrolling,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::RawScrollbar { child, .. } => (
                WidgetType::RawScrollbar,
                Scrolling,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::ListWheelScrollView { .. } => (
                WidgetType::ListWheelScrollView,
                Scrolling,
                Dynamic,
                DynamicChildren,
            ),
            WidgetKind::ListWheelViewport { .. } => (
                WidgetType::ListWheelViewport,
                Scrolling,
                Dynamic,
                DynamicChildren,
            ),
            WidgetKind::DraggableScrollableSheet { .. } => (
                WidgetType::DraggableScrollableSheet,
                Scrolling,
                Dynamic,
                DynamicChildren,
            ),
            WidgetKind::DraggableScrollableActuator { child, .. } => (
                WidgetType::DraggableScrollableActuator,
                Scrolling,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::TwoDimensionalScrollView { .. } => (
                WidgetType::TwoDimensionalScrollView,
                Scrolling,
                Dynamic,
                DynamicChildren,
            ),
            WidgetKind::TwoDimensionalViewport { .. } => (
                WidgetType::TwoDimensionalViewport,
                Scrolling,
                Dynamic,
                DynamicChildren,
            ),
            WidgetKind::PersistentHeader { child, .. } => (
                WidgetType::PersistentHeader,
                Scrolling,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::NotificationListener { child, .. } => (
                WidgetType::NotificationListener,
                Scrolling,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::SliverViewport { .. } => (
                WidgetType::SliverViewport,
                Scrolling,
                Dynamic,
                DynamicChildren,
            ),

            WidgetKind::Translate { child, .. } => (
                WidgetType::Translate,
                Effects,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::Transform { child, .. } => (
                WidgetType::Transform,
                Effects,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::Scale { child, .. } => (
                WidgetType::Scale,
                Effects,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::Rotation { child, .. } => (
                WidgetType::Rotation,
                Effects,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::FittedBox { child, .. } => (
                WidgetType::FittedBox,
                Effects,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::Opacity { child, .. } => (
                WidgetType::Opacity,
                Effects,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::Blur { child, .. } => (
                WidgetType::Blur,
                Effects,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::DropShadow { child, .. } => (
                WidgetType::DropShadow,
                Effects,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::ColorFiltered { child, .. } => (
                WidgetType::ColorFiltered,
                Effects,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::Blend { child, .. } => (
                WidgetType::Blend,
                Effects,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::ShaderMask { child, .. } => (
                WidgetType::ShaderMask,
                Effects,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::BackdropFilter { child, .. } => (
                WidgetType::BackdropFilter,
                Effects,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::AnnotatedRegion { child, .. } => (
                WidgetType::AnnotatedRegion,
                Effects,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::CompositedTransformTarget { child, .. } => (
                WidgetType::CompositedTransformTarget,
                Effects,
                Single,
                WidgetChildren::Single(child),
            ),
            WidgetKind::CompositedTransformFollower { child, .. } => (
                WidgetType::CompositedTransformFollower,
                Effects,
                Single,
                WidgetChildren::Single(child),
            ),
        };

        debug_assert_eq!(children.shape(), child_shape);
        WidgetStructure {
            widget_type,
            lowering_family,
            children,
        }
    }
}

impl Widget {
    fn from_node(node: WidgetNode) -> Self {
        Self {
            node: Some(Rc::new(node)),
        }
    }

    /// Returns the immutable descriptor node backing this handle.
    #[doc(hidden)]
    #[must_use]
    pub fn node(&self) -> &WidgetNode {
        self.node.as_deref().expect("live widget descriptor handle")
    }

    /// Returns whether two handles refer to the exact same declarative
    /// descriptor allocation.
    ///
    /// Descriptor identity is only an optimization hint; retained identity
    /// still belongs to `Element` and reconciliation continues to honor keys
    /// and widget type compatibility.
    #[doc(hidden)]
    #[must_use]
    pub fn ptr_eq(&self, other: &Self) -> bool {
        match (&self.node, &other.node) {
            (Some(left), Some(right)) => Rc::ptr_eq(left, right),
            (None, None) => true,
            _ => false,
        }
    }

    /// Returns the framework-internal descriptor kind by borrow.
    ///
    /// This is intentionally hidden from the Flutter-facing prelude. Plan 16
    /// removes the remaining workspace consumers that inspect built-in kinds.
    #[doc(hidden)]
    #[must_use]
    pub fn kind(&self) -> &WidgetKind {
        &self.node().kind
    }

    /// Creates a widget from a internal kind descriptor.
    #[must_use]
    pub fn from_kind(kind: WidgetKind) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind,
            semantics: SemanticProperties::default(),
        })
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
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Box { size, color },
            semantics: SemanticProperties::default(),
        })
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
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Shape {
                path,
                fill,
                stroke,
                size,
            },
            semantics: SemanticProperties::default(),
        })
    }
    /// Paints a caller-provided renderer-neutral display list at a fixed
    /// logical size.
    #[must_use]
    pub fn custom_paint(size: Size, display_list: DisplayList) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::CustomPaint { size, display_list },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub(super) fn decorated(
        size: Option<Size>,
        background: Option<Brush>,
        border: Option<Border>,
        radius: CornerRadii,
        child: Widget,
    ) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Decorated {
                size,
                background,
                border,
                radius,
                child,
            },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub(crate) fn button(size: Size, color: Color, action: ActionId) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Button(ButtonSpec {
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
            }),
            semantics: SemanticProperties::default(),
        })
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
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Button(ButtonSpec {
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
                child: Some(label),
            }),
            semantics: SemanticProperties {
                // Custom content is inspected for a text or explicit semantic
                // label so low-level controls retain a useful accessible name.
                label: semantic_label,
                ..SemanticProperties::default()
            },
        })
    }
    pub fn bind_callbacks(&mut self, allocate: &mut impl FnMut(Rc<dyn Fn()>) -> ActionId) {
        if let WidgetKind::Button(spec) = &mut self.kind {
            if let Some(callback) = spec.callback.take() {
                spec.action = allocate(callback);
                spec.has_callback = true;
            }
            if let Some(callback) = spec.hover_callback.take() {
                spec.hover_action = allocate(callback);
            }
            if let Some(callback) = spec.exit_callback.take() {
                spec.exit_action = allocate(callback);
            }
        }
    }

    #[must_use]
    pub fn text(text: impl Into<String>) -> Self {
        Self::from_node(WidgetNode {
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
        })
    }
    #[must_use]
    pub fn text_styled(text: impl Into<String>, style: TextStyle, align: TextAlign) -> Self {
        Self::from_node(WidgetNode {
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
        })
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
        Self::from_node(WidgetNode {
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
        })
    }
    #[must_use]
    pub fn selectable_text_styled(
        text: impl Into<String>,
        style: TextStyle,
        align: TextAlign,
    ) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::SelectableText {
                text: text.into(),
                style,
                align,
            },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub fn selection_area(controller: SelectionAreaController, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::SelectionArea { controller, child },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub fn selection_container(delegate: SelectionContainerDelegate, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::SelectionContainer { delegate, child },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub(crate) fn selection_listener(notifier: SelectionListenerNotifier, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::SelectionListener {
                delegate: SelectionContainerDelegate::with_controller(notifier.controller()),
                notifier,
                child,
            },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub(crate) fn indexed_semantics(index: usize, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::IndexedSemantics { index, child },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub(crate) fn semantics_debugger(
        label_style: TextStyle,
        max_nodes: usize,
        child: Self,
    ) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::SemanticsDebugger {
                label_style,
                max_nodes,
                child,
            },
            semantics: SemanticProperties::default(),
        })
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
        Self::from_node(WidgetNode {
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
        })
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
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::TextField(TextFieldSpec {
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
            }),
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub fn padding(padding: EdgeInsets, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Padding { padding, child },
            semantics: SemanticProperties::default(),
        })
    }
    /// Tightens the incoming layout bounds before passing them to `child`.
    #[must_use]
    pub fn constrained(constraints: Constraints, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Constrained { constraints, child },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub fn limited_box(max_width: f32, max_height: f32, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Limited {
                max_width: finite_non_negative(max_width),
                max_height: finite_non_negative(max_height),
                child,
            },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub fn overflow_box(
        min_width: Option<f32>,
        max_width: Option<f32>,
        min_height: Option<f32>,
        max_height: Option<f32>,
        child: Self,
    ) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Overflow {
                min_width: min_width.map(finite_non_negative),
                max_width: max_width.map(finite_non_negative),
                min_height: min_height.map(finite_non_negative),
                max_height: max_height.map(finite_non_negative),
                child,
            },
            semantics: SemanticProperties::default(),
        })
    }
    /// Lets a child take its natural size, optionally retaining the parent's
    /// limits on one axis while this wrapper itself still fits its parent.
    #[must_use]
    pub fn unconstrained(constrained_axis: Option<Axis>, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Unconstrained {
                constrained_axis,
                child,
            },
            semantics: SemanticProperties::default(),
        })
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
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Fractional {
                width_factor,
                height_factor,
                child,
            },
            semantics: SemanticProperties::default(),
        })
    }
    /// Positions a child so that its reported baseline is at `baseline`.
    #[must_use]
    pub fn baseline(baseline: f32, child: Self) -> Self {
        assert!(
            baseline.is_finite() && baseline >= 0.,
            "baseline must be finite and non-negative"
        );
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Baseline { baseline, child },
            semantics: SemanticProperties::default(),
        })
    }
    /// Creates an explicit retained picture boundary around `child`.
    ///
    /// Descendant paint changes update their own cached picture without
    /// repainting this boundary's otherwise empty retained picture.
    #[must_use]
    pub fn repaint_boundary(child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::RepaintBoundary { child },
            semantics: SemanticProperties::default(),
        })
    }
    /// Attaches tap, double-tap, long-press, and pan recognition to a retained
    /// subtree. A hit-tested down event captures the sequence for this region.
    #[must_use]
    pub fn gesture(callbacks: GestureCallbacks, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Gesture {
                behavior: crate::gestures::HitTestBehavior::DeferToChild,
                callbacks: Box::new(callbacks),
                child,
            },
            semantics: SemanticProperties::default(),
        })
    }
    pub(crate) fn draggable(source: Rc<dyn RetainedDragSource>, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Draggable { source, child },
            semantics: SemanticProperties::default(),
        })
    }
    pub(crate) fn drag_target(target: Rc<dyn RetainedDragTarget>, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::DragTarget { target, child },
            semantics: SemanticProperties::default(),
        })
    }
    /// Removes this subtree from pointer hit testing while leaving painting and
    /// semantics intact. Siblings behind it remain eligible for the event.
    #[must_use]
    pub fn ignore_pointer(ignoring: bool, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::IgnorePointer { ignoring, child },
            semantics: SemanticProperties::default(),
        })
    }
    /// Intercepts pointer hit testing at this boundary. Descendants do not
    /// receive ordinary retained interaction while painting and semantics are
    /// preserved.
    #[must_use]
    pub fn absorb_pointer(absorbing: bool, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::AbsorbPointer { absorbing, child },
            semantics: SemanticProperties::default(),
        })
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
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Flexible {
                flex: flex.max(1),
                fit,
                child,
            },
            semantics: SemanticProperties::default(),
        })
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
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Positioned {
                left: left.map(finite_non_negative),
                top: top.map(finite_non_negative),
                right: right.map(finite_non_negative),
                bottom: bottom.map(finite_non_negative),
                width: width.map(finite_non_negative),
                height: height.map(finite_non_negative),
                child,
            },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub fn indexed_stack(
        alignment: Alignment,
        index: usize,
        children: impl Into<Vec<Self>>,
    ) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::IndexedStack {
                alignment,
                index,
                children: children.into(),
            },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub fn layout_builder(
        builder: impl for<'a> Fn(&BuildContext<'a>, Constraints) -> Self + 'static,
    ) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::LayoutBuilder {
                builder: Rc::new(builder),
                environment: None,
                environment_boundary: false,
                revision: None,
            },
            semantics: SemanticProperties::default(),
        })
    }

    /// Wraps a child in a retained typed inherited scope. The wrapper is
    /// transparent to layout and paint; descendant builders read the value
    /// through their explicit [`BuildContext`].
    #[must_use]
    pub fn environment_scope<T: Any>(value: T, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::LayoutBuilder {
                builder: Rc::new(move |_, _| child.clone()),
                environment: Some(InheritedScopeValue::new(value)),
                environment_boundary: false,
                revision: None,
            },
            semantics: SemanticProperties::default(),
        })
    }

    /// Creates a transparent retained node that prevents descendants from
    /// reading typed environments installed above it.
    #[must_use]
    pub fn environment_boundary(child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::LayoutBuilder {
                builder: Rc::new(move |_, _| child.clone()),
                environment: None,
                environment_boundary: true,
                revision: None,
            },
            semantics: SemanticProperties::default(),
        })
    }

    /// Creates a retained layout builder backed by an explicit local state
    /// revision.  A callback can increment `revision` and the next frame will
    /// rebuild only this builder's child, preserving the rest of the tree.
    /// This is the primitive used by uncontrolled controls such as checkbox,
    /// switch, toggle, and slider.
    #[must_use]
    pub fn stateful_layout_builder(
        revision: Rc<Cell<u64>>,
        builder: impl for<'a> Fn(&BuildContext<'a>, Constraints) -> Self + 'static,
    ) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::LayoutBuilder {
                builder: Rc::new(builder),
                environment: None,
                environment_boundary: false,
                revision: Some(revision),
            },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub fn visibility(visible: bool, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Visibility { visible, child },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub fn aspect_ratio(ratio: f32, child: Self) -> Self {
        assert!(
            ratio.is_finite() && ratio > 0.,
            "aspect ratio must be finite and positive"
        );
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::AspectRatio { ratio, child },
            semantics: SemanticProperties::default(),
        })
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
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Scroll {
                controller,
                axis,
                reverse,
                physics,
                child,
            },
            semantics: SemanticProperties::default(),
        })
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
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::RawScrollbar {
                controller,
                style,
                child,
            },
            semantics: SemanticProperties::default(),
        })
    }

    /// Creates a retained list-wheel viewport from its focused model.
    #[must_use]
    pub fn list_wheel_viewport(viewport: ListWheelViewport<Self>) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::ListWheelViewport {
                config: Rc::new(viewport.into_retained_config()),
            },
            semantics: SemanticProperties::default(),
        })
    }

    /// Creates a retained list-wheel scroll view from its focused model.
    #[must_use]
    pub fn list_wheel_scroll_view(view: ListWheelScrollView<Self>) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::ListWheelScrollView {
                config: Rc::new(view.into_retained_config()),
            },
            semantics: SemanticProperties::default(),
        })
    }

    /// Creates a retained draggable sheet from its focused model.
    #[must_use]
    pub fn draggable_scrollable_sheet(sheet: DraggableScrollableSheet<Self>) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::DraggableScrollableSheet {
                config: Rc::new(sheet.into_retained_config()),
            },
            semantics: SemanticProperties::default(),
        })
    }

    /// Creates an actuator wrapper which resets the nearest descendant sheets.
    #[must_use]
    pub fn draggable_scrollable_actuator(
        actuator: DraggableScrollableActuator,
        child: impl Into<Self>,
    ) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::DraggableScrollableActuator {
                actuator,
                child: child.into(),
            },
            semantics: SemanticProperties::default(),
        })
    }

    /// Creates a retained two-dimensional viewport from its focused model.
    #[must_use]
    pub fn two_dimensional_viewport(viewport: TwoDimensionalViewport<Self>) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::TwoDimensionalViewport {
                config: Rc::new(viewport.into_retained_config()),
            },
            semantics: SemanticProperties::default(),
        })
    }

    /// Creates a retained two-dimensional scroll view from its focused model.
    #[must_use]
    pub fn two_dimensional_scroll_view(view: TwoDimensionalScrollView<Self>) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::TwoDimensionalScrollView {
                config: Rc::new(view.into_retained_config()),
            },
            semantics: SemanticProperties::default(),
        })
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
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::PersistentHeader {
                controller,
                axis,
                reverse,
                pinned,
                child,
            },
            semantics: SemanticProperties::default(),
        })
    }

    /// Creates a retained notification-listener wrapper. The wrapper has no
    /// visual effect; the tree installs the callback on descendant scroll
    /// positions after those positions are mounted.
    pub(crate) fn notification_listener(
        callback: Option<Rc<dyn Fn(ScrollNotification) -> bool>>,
        child: Self,
    ) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::NotificationListener { callback, child },
            semantics: SemanticProperties::default(),
        })
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
        Self::from_node(WidgetNode {
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
        })
    }
    #[must_use]
    pub fn translate(controller: TranslationController, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Translate { controller, child },
            semantics: SemanticProperties::default(),
        })
    }
    /// Applies an arbitrary Kurbo-backed affine transform after layout.
    /// The transform is compositor-only and defaults to the child's center.
    #[must_use]
    pub fn transform(transform: CoreTransform, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Transform {
                transform,
                origin: None,
                child,
            },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub fn transform_around(transform: CoreTransform, origin: Offset, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Transform {
                transform,
                origin: Some(finite_offset(origin)),
                child,
            },
            semantics: SemanticProperties::default(),
        })
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
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::FittedBox {
                fit,
                alignment,
                child,
            },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub fn controlled_scale(controller: ScaleController, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Scale {
                controller,
                origin: None,
                child,
            },
            semantics: SemanticProperties::default(),
        })
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
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Rotation {
                controller,
                origin: None,
                alignment,
                child,
            },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub fn opacity(alpha: f32, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Opacity {
                alpha: normalize_opacity(alpha),
                controller: None,
                child,
            },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub fn controlled_opacity(controller: OpacityController, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Opacity {
                alpha: controller.opacity(),
                controller: Some(controller),
                child,
            },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub fn blur(sigma: f32, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Blur {
                sigma_x: normalize_sigma(sigma),
                sigma_y: normalize_sigma(sigma),
                controller: None,
                child,
            },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub fn asymmetric_blur(sigma_x: f32, sigma_y: f32, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Blur {
                sigma_x: normalize_sigma(sigma_x),
                sigma_y: normalize_sigma(sigma_y),
                controller: None,
                child,
            },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub fn controlled_blur(controller: BlurController, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Blur {
                sigma_x: controller.sigma(),
                sigma_y: controller.sigma(),
                controller: Some(controller),
                child,
            },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub fn drop_shadow(offset: Offset, sigma: f32, color: Color, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::DropShadow {
                offset: finite_offset(offset),
                sigma_x: normalize_sigma(sigma),
                sigma_y: normalize_sigma(sigma),
                color,
                controller: None,
                child,
            },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub fn controlled_drop_shadow(controller: DropShadowController, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::DropShadow {
                offset: controller.offset(),
                sigma_x: controller.sigma(),
                sigma_y: controller.sigma(),
                color: controller.color(),
                controller: Some(controller),
                child,
            },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub fn color_filtered(filter: ColorFilter, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::ColorFiltered {
                filter,
                controller: None,
                child,
            },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub fn color_matrix(filter: ColorFilter, child: Self) -> Self {
        Self::color_filtered(filter, child)
    }
    #[must_use]
    pub fn controlled_color_filtered(controller: ColorFilterController, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::ColorFiltered {
                filter: controller.filter(),
                controller: Some(controller),
                child,
            },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub fn blend(mode: BlendMode, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Blend { mode, child },
            semantics: SemanticProperties::default(),
        })
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
        self.kind.structure().widget_type
    }
    /// Shallow child view for reconciliation. Deep per-child clones were the
    /// measured allocation fire on wide trees (Task 15); reconciliation only
    /// needs references because cloning happens once per *created* element.
    pub(super) fn children_refs(&self) -> WidgetChildren<'_> {
        self.kind.structure().children
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
