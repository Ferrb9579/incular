use super::super::super::*;

pub(super) fn lower_layout(widget: &Widget) -> RenderKind {
    match widget.kind() {
        WidgetKind::Padding { padding, .. } => RenderKind::Padding { padding: *padding },
        WidgetKind::Constrained { constraints, .. } => RenderKind::Constrained {
            constraints: *constraints,
        },
        WidgetKind::Limited {
            max_width,
            max_height,
            ..
        } => RenderKind::Limited {
            max_width: *max_width,
            max_height: *max_height,
        },
        WidgetKind::Overflow {
            min_width,
            max_width,
            min_height,
            max_height,
            ..
        } => RenderKind::Overflow {
            min_width: *min_width,
            max_width: *max_width,
            min_height: *min_height,
            max_height: *max_height,
        },
        WidgetKind::Unconstrained {
            constrained_axis, ..
        } => RenderKind::Unconstrained {
            constrained_axis: *constrained_axis,
        },
        WidgetKind::Fractional {
            width_factor,
            height_factor,
            ..
        } => RenderKind::Fractional {
            width_factor: *width_factor,
            height_factor: *height_factor,
        },
        WidgetKind::Baseline { baseline, .. } => RenderKind::Baseline {
            baseline: *baseline,
        },
        WidgetKind::RepaintBoundary { .. } => RenderKind::RepaintBoundary,
        WidgetKind::Gesture { .. } => RenderKind::Gesture,
        WidgetKind::RawInput { .. } => RenderKind::Gesture,
        WidgetKind::Draggable { .. } | WidgetKind::DragTarget { .. } => RenderKind::Gesture,
        WidgetKind::IgnorePointer { .. } | WidgetKind::AbsorbPointer { .. } => RenderKind::Gesture,
        WidgetKind::Align {
            alignment,
            width_factor,
            height_factor,
            ..
        } => RenderKind::Align {
            alignment: *alignment,
            width_factor: *width_factor,
            height_factor: *height_factor,
        },
        WidgetKind::Flex {
            axis,
            main_axis_alignment,
            main_axis_size,
            cross_axis_alignment,
            text_direction,
            vertical_direction,
            spacing,
            ..
        } => RenderKind::Flex {
            flex: incular_layout::Flex {
                direction: *axis,
                main_axis_alignment: *main_axis_alignment,
                main_axis_size: *main_axis_size,
                cross_axis_alignment: *cross_axis_alignment,
                text_direction: *text_direction,
                vertical_direction: *vertical_direction,
                spacing: *spacing,
            },
        },
        WidgetKind::Flexible { flex, fit, .. } => RenderKind::Flexible {
            flex: *flex,
            fit: *fit,
        },
        WidgetKind::Wrap {
            axis,
            alignment,
            spacing,
            run_alignment,
            run_spacing,
            cross_axis_alignment,
            text_direction,
            vertical_direction,
            ..
        } => RenderKind::Wrap {
            wrap: incular_layout::Wrap {
                direction: *axis,
                alignment: *alignment,
                spacing: *spacing,
                run_alignment: *run_alignment,
                run_spacing: *run_spacing,
                cross_axis_alignment: *cross_axis_alignment,
                text_direction: *text_direction,
                vertical_direction: *vertical_direction,
            },
        },
        WidgetKind::Table {
            columns,
            column_spacing,
            row_spacing,
            ..
        } => RenderKind::Table {
            columns: *columns,
            column_spacing: *column_spacing,
            row_spacing: *row_spacing,
        },
        WidgetKind::Stack {
            alignment,
            text_direction,
            fit,
            clip_behavior,
            ..
        } => RenderKind::Stack {
            stack: incular_layout::Stack {
                alignment: *alignment,
                text_direction: *text_direction,
                fit: *fit,
                clip_behavior: *clip_behavior,
            },
        },
        WidgetKind::SafeArea {
            minimum,
            left,
            top,
            right,
            bottom,
            maintain_bottom_view_padding,
            ..
        } => RenderKind::SafeArea {
            minimum: *minimum,
            left: *left,
            top: *top,
            right: *right,
            bottom: *bottom,
            maintain_bottom_view_padding: *maintain_bottom_view_padding,
        },
        WidgetKind::ClipRect { clip_behavior, .. } => RenderKind::ClipRect {
            clip_behavior: *clip_behavior,
        },
        WidgetKind::ClipRRect {
            radius,
            clip_behavior,
            ..
        } => RenderKind::ClipRRect {
            radius: *radius,
            clip_behavior: *clip_behavior,
        },
        WidgetKind::ClipOval { clip_behavior, .. } => RenderKind::ClipOval {
            clip_behavior: *clip_behavior,
        },
        WidgetKind::ClipPath {
            path,
            clip_behavior,
            ..
        } => RenderKind::ClipPath {
            path: path.clone(),
            clip_behavior: *clip_behavior,
        },
        WidgetKind::Positioned {
            left,
            top,
            right,
            bottom,
            width,
            height,
            ..
        } => RenderKind::Positioned {
            left: *left,
            top: *top,
            right: *right,
            bottom: *bottom,
            width: *width,
            height: *height,
        },
        WidgetKind::IndexedStack {
            alignment, index, ..
        } => RenderKind::IndexedStack {
            alignment: *alignment,
            index: *index,
        },
        WidgetKind::LayoutBuilder { .. } => RenderKind::LayoutBuilder,
        WidgetKind::Visibility { visible, .. } => RenderKind::Visibility { visible: *visible },
        WidgetKind::AspectRatio { ratio, .. } => RenderKind::AspectRatio { ratio: *ratio },
        _ => unreachable!("layout lowering received a non-layout widget"),
    }
}
