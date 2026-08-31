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
        WidgetKind::Button {
            size,
            color,
            hover_color,
            pressed_color,
            focused_color,
            disabled_color,
            enabled,
            focusable_when_disabled,
            ..
        } => RenderKind::Button {
            desired: *size,
            color: *color,
            hover_color: *hover_color,
            pressed_color: *pressed_color,
            focused_color: *focused_color,
            disabled_color: *disabled_color,
            enabled: *enabled,
            focusable_when_disabled: *focusable_when_disabled,
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
        WidgetKind::TextField {
            controller,
            size,
            style,
            placeholder,
            on_submit: _,
            multiline,
            min_lines,
            max_lines,
            expands,
            text_align,
            enabled,
            read_only,
            obscure_text,
            cursor_width,
            cursor_height,
            cursor_radius,
            show_cursor,
            cursor_color,
            selection_color,
        } => RenderKind::TextField {
            controller: controller.clone(),
            desired: *size,
            style: resolve_text_style(style, environment),
            placeholder: placeholder.clone(),
            multiline: *multiline,
            min_lines: *min_lines,
            max_lines: *max_lines,
            expands: *expands,
            text_align: *text_align,
            enabled: *enabled,
            read_only: *read_only,
            obscure_text: *obscure_text,
            cursor_width: *cursor_width,
            cursor_height: *cursor_height,
            cursor_radius: *cursor_radius,
            show_cursor: *show_cursor,
            cursor_color: *cursor_color,
            selection_color: *selection_color,
        },
        _ => unreachable!("visual lowering received a non-visual widget"),
    }
}
