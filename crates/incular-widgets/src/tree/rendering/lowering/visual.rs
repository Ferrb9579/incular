use super::super::super::*;
use super::super::resolve_text_style;
pub(super) fn lower_visual(widget: &Widget, environment: Option<&Rc<dyn Any>>) -> RenderKind {
    match &widget.kind {
        WidgetKind::Box { size, color } => RenderKind::Box {
            desired: *size,
            color: *color,
        },
        WidgetKind::Shape {
            path,
            fill,
            stroke,
            size,
        } => RenderKind::Shape {
            path: path.clone(),
            fill: fill.clone(),
            stroke: stroke.clone(),
            desired: size.unwrap_or_else(|| path.bounds().map_or(Size::ZERO, |bounds| bounds.size)),
        },
        WidgetKind::CustomPaint { size, display_list } => RenderKind::CustomPaint {
            desired: *size,
            display_list: display_list.clone(),
        },
        WidgetKind::Decorated {
            size,
            background,
            border,
            radius,
            ..
        } => RenderKind::Decorated {
            desired: *size,
            background: background.clone(),
            border: *border,
            radius: *radius,
        },
        WidgetKind::Banner {
            message,
            text_direction,
            location,
            layout_direction,
            color,
            text_style,
            shadow,
            ..
        } => {
            let ambient_direction = environment
                .and_then(environment_value::<TextDirection>)
                .unwrap_or(TextDirection::Ltr);
            RenderKind::Banner {
                message: message.clone(),
                text_direction: text_direction.unwrap_or(ambient_direction),
                location: *location,
                layout_direction: layout_direction.unwrap_or(ambient_direction),
                color: *color,
                text_style: text_style.clone(),
                shadow: *shadow,
            }
        }
        WidgetKind::Button(spec) => RenderKind::Button {
            desired: spec.size,
            color: spec.color,
            hover_color: spec.hover_color,
            pressed_color: spec.pressed_color,
            focused_color: spec.focused_color,
            disabled_color: spec.disabled_color,
            enabled: spec.enabled,
            focusable_when_disabled: spec.focusable_when_disabled,
        },
        WidgetKind::Text {
            text,
            style,
            align,
            soft_wrap,
            max_lines,
            overflow,
        } => RenderKind::Text {
            text: text.clone(),
            style: resolve_text_style(style, environment),
            align: *align,
            soft_wrap: *soft_wrap,
            max_lines: *max_lines,
            overflow: *overflow,
        },
        WidgetKind::SelectableText { text, style, align } => RenderKind::SelectableText {
            text: text.clone(),
            style: resolve_text_style(style, environment),
            align: *align,
        },
        WidgetKind::SelectionArea { .. } => RenderKind::SelectionArea,
        WidgetKind::SelectionContainer { .. } => RenderKind::SelectionContainer,
        WidgetKind::SelectionListener { .. } => RenderKind::SelectionListener,
        WidgetKind::IndexedSemantics { .. } => RenderKind::IndexedSemantics,
        WidgetKind::SemanticsDebugger {
            label_style,
            max_nodes,
            ..
        } => RenderKind::SemanticsDebugger {
            label_style: label_style.clone(),
            max_nodes: *max_nodes,
        },
        WidgetKind::Image {
            image,
            width,
            height,
            fit,
            repeat,
            alignment,
            sampling,
        } => RenderKind::Image {
            image: image.clone(),
            width: *width,
            height: *height,
            fit: *fit,
            repeat: *repeat,
            alignment: *alignment,
            sampling: *sampling,
        },
        WidgetKind::TextField(spec) => RenderKind::TextField {
            controller: spec.controller.clone(),
            desired: spec.size,
            style: resolve_text_style(&spec.style, environment),
            placeholder: spec.placeholder.clone(),
            multiline: spec.multiline,
            min_lines: spec.min_lines,
            max_lines: spec.max_lines,
            expands: spec.expands,
            text_align: spec.text_align,
            enabled: spec.enabled,
            read_only: spec.read_only,
            obscure_text: spec.obscure_text,
            cursor_width: spec.cursor_width,
            cursor_height: spec.cursor_height,
            cursor_radius: spec.cursor_radius,
            show_cursor: spec.show_cursor,
            cursor_color: spec.cursor_color,
            selection_color: spec.selection_color,
        },
        _ => unreachable!("visual lowering received a non-visual widget"),
    }
}
