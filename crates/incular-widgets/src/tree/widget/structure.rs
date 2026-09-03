//! Structural metadata for the sealed built-in widget taxonomy.
//!
//! Child topology, stable type identity, and lowering families live here so descriptor construction and retained execution do not duplicate taxonomy knowledge.

use super::super::*;

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
        && same_optional_callback(&left.on_trackpad_gesture, &right.on_trackpad_gesture)
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
    WindowDragRegion,
    WindowResizeRegion,
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
            Self::WindowDragRegion => "WindowDragRegion",
            Self::WindowResizeRegion => "WindowResizeRegion",
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

    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub(crate) fn len(&self) -> usize {
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
