//! Widget-to-render lowering and renderer-independent visual helpers.

use super::*;

#[cfg(feature = "devtools")]
impl WidgetKind {
    pub fn dev_type_name_widget(&self) -> String {
        crate::devtools_props::kind_display_name(self)
    }
}

#[cfg(feature = "devtools")]
impl RenderKind {
    pub fn dev_type_name_render(&self) -> String {
        crate::devtools_props::kind_display_name_render(self)
    }

    pub fn dev_type_name(&self) -> String {
        let name = match self {
            RenderKind::Box { .. } => "Box",
            RenderKind::Shape { .. } => "Shape",
            RenderKind::CustomPaint { .. } => "CustomPaint",
            RenderKind::Decorated { .. } => "DecoratedBox",
            RenderKind::Button { .. } => "Button",
            RenderKind::Text { .. } => "Text",
            RenderKind::SelectableText { .. } => "SelectableText",
            RenderKind::SelectionArea => "SelectionArea",
            RenderKind::TextField { .. } => "TextField",
            RenderKind::Image { .. } => "Image",
            RenderKind::Padding { .. } => "Padding",
            RenderKind::Constrained { .. } => "ConstrainedBox",
            RenderKind::Limited { .. } => "LimitedBox",
            RenderKind::Overflow { .. } => "OverflowBox",
            RenderKind::Unconstrained { .. } => "UnconstrainedBox",
            RenderKind::Fractional { .. } => "FractionallySizedBox",
            RenderKind::Baseline { .. } => "Baseline",
            RenderKind::RepaintBoundary => "RepaintBoundary",
            RenderKind::Gesture => "GestureDetector",
            RenderKind::Align { .. } => "Align",
            RenderKind::Flex { flex, .. } => {
                return match flex.direction {
                    incular_config::Axis::Vertical => "Column".into(),
                    incular_config::Axis::Horizontal => "Row".into(),
                };
            }
            RenderKind::Wrap { .. } => "Wrap",
            RenderKind::Table { .. } => "Table",
            RenderKind::Stack { .. } => "Stack",
            RenderKind::IndexedStack { .. } => "IndexedStack",
            RenderKind::Positioned { .. } => "Positioned",
            RenderKind::SafeArea { .. } => "SafeArea",
            RenderKind::ClipRect { .. } => "ClipRect",
            RenderKind::ClipRRect { .. } => "ClipRRect",
            RenderKind::ClipOval { .. } => "ClipOval",
            RenderKind::ClipPath { .. } => "ClipPath",
            RenderKind::Visibility { .. } => "Visibility",
            RenderKind::AspectRatio { .. } => "AspectRatio",
            RenderKind::Scroll { .. } => "ScrollView",
            RenderKind::PersistentHeader { .. } => "PersistentHeader",
            RenderKind::SliverViewport { .. } => "SliverViewport",
            RenderKind::LayoutBuilder => "LayoutBuilder",
            RenderKind::Translate { .. } => "Translate",
            RenderKind::Transform { .. } => "Transform",
            RenderKind::Scale { .. } => "Scale",
            RenderKind::Rotation { .. } => "Rotation",
            RenderKind::FittedBox { .. } => "FittedBox",
            RenderKind::Opacity { .. } => "Opacity",
            RenderKind::Blur { .. } => "Blur",
            RenderKind::DropShadow { .. } => "DropShadow",
            RenderKind::ColorFiltered { .. } => "ColorFiltered",
            RenderKind::Blend { .. } => "Blend",
            RenderKind::Flexible { .. } => "Flexible",
        };
        name.to_owned()
    }
}

pub(super) fn resolve_text_style(
    style: &TextStyle,
    environment: Option<&Rc<dyn Any>>,
) -> TextStyle {
    environment
        .and_then(environment_value::<TextStyle>)
        .map_or_else(|| style.clone(), |default| default.merge(style))
}

pub(super) fn render_kind(widget: &Widget, environment: Option<&Rc<dyn Any>>) -> RenderKind {
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
        WidgetKind::Scroll {
            controller,
            axis,
            reverse,
            physics,
            ..
        } => RenderKind::Scroll {
            controller: controller.clone(),
            axis: *axis,
            reverse: *reverse,
            physics: *physics,
        },
        WidgetKind::PersistentHeader {
            controller,
            axis,
            reverse,
            pinned,
            ..
        } => RenderKind::PersistentHeader {
            controller: controller.clone(),
            axis: *axis,
            reverse: *reverse,
            pinned: *pinned,
        },
        WidgetKind::NotificationListener { .. } => RenderKind::Gesture,
        WidgetKind::SliverViewport { config } => RenderKind::SliverViewport {
            config: config.clone(),
        },
        WidgetKind::Translate { controller, .. } => RenderKind::Translate {
            controller: controller.clone(),
        },
        WidgetKind::Transform {
            transform, origin, ..
        } => RenderKind::Transform {
            transform: *transform,
            origin: *origin,
        },
        WidgetKind::Scale {
            controller, origin, ..
        } => RenderKind::Scale {
            controller: controller.clone(),
            origin: *origin,
        },
        WidgetKind::Rotation {
            controller,
            origin,
            alignment,
            ..
        } => RenderKind::Rotation {
            controller: controller.clone(),
            origin: *origin,
            alignment: *alignment,
        },
        WidgetKind::FittedBox { fit, alignment, .. } => RenderKind::FittedBox {
            fit: *fit,
            alignment: *alignment,
        },
        WidgetKind::Opacity {
            alpha, controller, ..
        } => RenderKind::Opacity {
            alpha: *alpha,
            controller: controller.clone(),
        },
        WidgetKind::Blur {
            sigma_x,
            sigma_y,
            controller,
            ..
        } => RenderKind::Blur {
            sigma_x: *sigma_x,
            sigma_y: *sigma_y,
            controller: controller.clone(),
        },
        WidgetKind::DropShadow {
            offset,
            sigma_x,
            sigma_y,
            color,
            controller,
            ..
        } => RenderKind::DropShadow {
            offset: *offset,
            sigma_x: *sigma_x,
            sigma_y: *sigma_y,
            color: *color,
            controller: controller.clone(),
        },
        WidgetKind::ColorFiltered {
            filter, controller, ..
        } => RenderKind::ColorFiltered {
            filter: *filter,
            controller: controller.clone(),
        },
        WidgetKind::Blend { mode, .. } => RenderKind::Blend { mode: *mode },
    }
}

/// Carries a compositor transition across a declarative rebuild when a
/// controlled application creates a fresh controller value.  The widget
/// descriptor is replaced, but the retained render object is still the same
/// semantic/control node; jumping directly to the new controller value would
/// make externally owned toggles and switches visibly snap.
pub(super) fn carry_replaced_transition(old: &RenderKind, new: &RenderKind) {
    const REPLACED_TRANSITION: Duration = Duration::from_millis(140);

    match (old, new) {
        (
            RenderKind::Opacity {
                alpha: old_alpha,
                controller: old_controller,
            },
            RenderKind::Opacity {
                alpha: new_alpha,
                controller: Some(new_controller),
            },
        ) => {
            let replaced = match old_controller {
                Some(old_controller) => old_controller != new_controller,
                None => true,
            };
            let current_alpha = old_controller
                .as_ref()
                .map_or(*old_alpha, OpacityController::opacity);
            if replaced && (current_alpha - new_alpha).abs() > f32::EPSILON {
                new_controller.set_opacity(current_alpha);
                new_controller.animate_to(*new_alpha, REPLACED_TRANSITION, Instant::now());
            }
        }
        (
            RenderKind::Translate {
                controller: old_controller,
            },
            RenderKind::Translate {
                controller: new_controller,
            },
        ) if old_controller != new_controller => {
            let old_offset = old_controller.offset();
            let new_offset = new_controller.offset();
            if old_offset != new_offset {
                new_controller.set_offset(old_offset);
                new_controller.animate_to(new_offset, REPLACED_TRANSITION, Instant::now());
            }
        }
        (
            RenderKind::Scale {
                controller: old_controller,
                ..
            },
            RenderKind::Scale {
                controller: new_controller,
                ..
            },
        ) if old_controller != new_controller => {
            let old_scale = old_controller.scale();
            let new_scale = new_controller.scale();
            if (old_scale - new_scale).abs() > f32::EPSILON {
                new_controller.set_scale(old_scale);
                new_controller.animate_to(new_scale, REPLACED_TRANSITION, Instant::now());
            }
        }
        (
            RenderKind::Rotation {
                controller: old_controller,
                ..
            },
            RenderKind::Rotation {
                controller: new_controller,
                ..
            },
        ) if old_controller != new_controller => {
            let old_radians = old_controller.radians();
            let new_radians = new_controller.radians();
            if (old_radians - new_radians).abs() > f32::EPSILON {
                new_controller.set_radians(old_radians);
                new_controller.animate_to(new_radians, REPLACED_TRANSITION, Instant::now());
            }
        }
        _ => {}
    }
}

pub(super) fn transform_around(
    transform: CoreTransform,
    origin: Option<Offset>,
    size: Size,
) -> CoreTransform {
    let origin = origin.unwrap_or(Offset::new(size.width * 0.5, size.height * 0.5));
    CoreTransform::translation(origin)
        .then(transform)
        .then(CoreTransform::translation(Offset::new(
            -origin.x, -origin.y,
        )))
}

pub(super) fn transform_around_alignment(
    transform: CoreTransform,
    origin: Option<Offset>,
    alignment: Option<Alignment>,
    size: Size,
) -> CoreTransform {
    let origin = origin.or_else(|| alignment.map(|alignment| alignment.within(size, Size::ZERO)));
    transform_around(transform, origin, size)
}

pub(super) fn fitted_transform(
    source: Size,
    bounds: Size,
    fit: ImageFit,
    alignment: Alignment,
) -> CoreTransform {
    if source.width <= 0. || source.height <= 0. || bounds.width <= 0. || bounds.height <= 0. {
        return CoreTransform::IDENTITY;
    }
    let sx = bounds.width / source.width;
    let sy = bounds.height / source.height;
    let (scale_x, scale_y) = match fit {
        ImageFit::Fill => (sx, sy),
        ImageFit::Cover => {
            let scale = sx.max(sy);
            (scale, scale)
        }
        ImageFit::FitWidth => (sx, sx),
        ImageFit::FitHeight => (sy, sy),
        ImageFit::None => (1., 1.),
        ImageFit::ScaleDown => {
            let scale = sx.min(sy).min(1.);
            (scale, scale)
        }
        ImageFit::Contain => {
            let scale = sx.min(sy);
            (scale, scale)
        }
    };
    let fitted = Size::new(source.width * scale_x, source.height * scale_y);
    let offset = alignment.within(bounds, fitted);
    CoreTransform::translation(offset).then(CoreTransform::scale_non_uniform(scale_x, scale_y))
}

/// Returns the pixel source crop and logical destination for a fit operation.
#[must_use]
pub fn image_fit_rects(
    source: Rect,
    bounds: Rect,
    fit: ImageFit,
    alignment: Alignment,
) -> (Rect, Rect) {
    if source.size.width == 0.
        || source.size.height == 0.
        || bounds.size.width == 0.
        || bounds.size.height == 0.
    {
        return (source, Rect::from_origin_size(bounds.origin, Size::ZERO));
    }
    if fit == ImageFit::Fill {
        return (source, bounds);
    }
    let sx = bounds.size.width / source.size.width;
    let sy = bounds.size.height / source.size.height;
    let (scale_x, scale_y) = match fit {
        ImageFit::Cover => {
            let s = sx.max(sy);
            (s, s)
        }
        ImageFit::FitWidth => (sx, sx),
        ImageFit::FitHeight => (sy, sy),
        ImageFit::None => (1., 1.),
        ImageFit::ScaleDown => {
            let s = sx.min(sy).min(1.);
            (s, s)
        }
        _ => {
            let s = sx.min(sy);
            (s, s)
        }
    };
    let scale = scale_x.min(scale_y);
    let rendered = Size::new(source.size.width * scale, source.size.height * scale);
    if fit == ImageFit::Cover {
        let crop = Size::new(
            (bounds.size.width / scale).min(source.size.width),
            (bounds.size.height / scale).min(source.size.height),
        );
        let x = source.origin.x + (source.size.width - crop.width) * (alignment.x + 1.) / 2.;
        let y = source.origin.y + (source.size.height - crop.height) * (alignment.y + 1.) / 2.;
        return (Rect::from_origin_size(Offset::new(x, y), crop), bounds);
    }
    let origin = Offset::new(
        bounds.origin.x + (bounds.size.width - rendered.width) * (alignment.x + 1.) / 2.,
        bounds.origin.y + (bounds.size.height - rendered.height) * (alignment.y + 1.) / 2.,
    );
    (source, Rect::from_origin_size(origin, rendered))
}

/// Returns all local destinations required to repeat one image tile within a
/// bounded paint rectangle. Non-repeated axes retain the fitted destination.
#[must_use]
pub fn image_repeat_destinations(
    destination: Rect,
    bounds: Rect,
    repeat: ImageRepeat,
) -> Vec<Rect> {
    if matches!(repeat, ImageRepeat::NoRepeat)
        || destination.size.width <= 0.
        || destination.size.height <= 0.
    {
        return vec![destination];
    }
    let repeat_x = matches!(repeat, ImageRepeat::RepeatX | ImageRepeat::Repeat);
    let repeat_y = matches!(repeat, ImageRepeat::RepeatY | ImageRepeat::Repeat);
    let start_x = if repeat_x {
        destination.origin.x
            - ((destination.origin.x - bounds.origin.x) / destination.size.width).ceil()
                * destination.size.width
    } else {
        destination.origin.x
    };
    let start_y = if repeat_y {
        destination.origin.y
            - ((destination.origin.y - bounds.origin.y) / destination.size.height).ceil()
                * destination.size.height
    } else {
        destination.origin.y
    };
    let x_limit = if repeat_x {
        bounds.origin.x + bounds.size.width
    } else {
        start_x + destination.size.width
    };
    let y_limit = if repeat_y {
        bounds.origin.y + bounds.size.height
    } else {
        start_y + destination.size.height
    };
    let mut tiles = Vec::new();
    let mut y = start_y;
    while y < y_limit && tiles.len() < 16_384 {
        let mut x = start_x;
        while x < x_limit && tiles.len() < 16_384 {
            tiles.push(Rect::from_origin_size(Offset::new(x, y), destination.size));
            x += destination.size.width;
        }
        y += destination.size.height;
    }
    tiles
}

pub(super) fn text_paint_only_change(old: &RenderKind, new: &RenderKind) -> bool {
    let (
        RenderKind::Text {
            text: old_text,
            style: old_style,
            align: old_align,
            soft_wrap: old_soft_wrap,
            max_lines: old_max_lines,
            overflow: old_overflow,
        },
        RenderKind::Text {
            text: new_text,
            style: new_style,
            align: new_align,
            soft_wrap: new_soft_wrap,
            max_lines: new_max_lines,
            overflow: new_overflow,
        },
    ) = (old, new)
    else {
        return false;
    };
    old_text == new_text
        && old_align == new_align
        && old_soft_wrap == new_soft_wrap
        && old_max_lines == new_max_lines
        && old_overflow == new_overflow
        && old_style.family == new_style.family
        && old_style.size == new_style.size
        && old_style.weight == new_style.weight
        && old_style.style == new_style.style
        && old_style.line_height == new_style.line_height
        && old_style.letter_spacing == new_style.letter_spacing
}

pub(super) fn custom_paint_only_change(old: &RenderKind, new: &RenderKind) -> bool {
    matches!(
        (old, new),
        (
            RenderKind::CustomPaint {
                desired: old_size,
                ..
            },
            RenderKind::CustomPaint {
                desired: new_size,
                ..
            }
        ) if old_size == new_size
    )
}

pub(super) fn opacity_composite_only_change(old: &RenderKind, new: &RenderKind) -> bool {
    matches!(
        (old, new),
        (RenderKind::Opacity { .. }, RenderKind::Opacity { .. })
    )
}

pub(super) fn effect_composite_only_change(old: &RenderKind, new: &RenderKind) -> bool {
    matches!(
        (old, new),
        (RenderKind::Blur { .. }, RenderKind::Blur { .. })
            | (RenderKind::DropShadow { .. }, RenderKind::DropShadow { .. })
            | (
                RenderKind::ColorFiltered { .. },
                RenderKind::ColorFiltered { .. }
            )
            | (RenderKind::Blend { .. }, RenderKind::Blend { .. })
    )
}

pub(super) fn affine_composite_only_change(old: &RenderKind, new: &RenderKind) -> bool {
    matches!(
        (old, new),
        (RenderKind::Transform { .. }, RenderKind::Transform { .. })
            | (RenderKind::Scale { .. }, RenderKind::Scale { .. })
            | (RenderKind::Rotation { .. }, RenderKind::Rotation { .. })
    )
}

#[cfg(test)]
mod image_fit_tests {
    use super::*;

    #[test]
    fn contain_cover_fill_and_scale_down_are_deterministic() {
        let source = Rect::from_origin_size(Offset::ZERO, Size::new(400., 200.));
        let bounds = Rect::from_origin_size(Offset::ZERO, Size::new(200., 200.));
        let (_, contain) = image_fit_rects(source, bounds, ImageFit::Contain, Alignment::CENTER);
        assert_eq!(contain.size, Size::new(200., 100.));
        let (cover_source, cover_destination) =
            image_fit_rects(source, bounds, ImageFit::Cover, Alignment::CENTER);
        assert_eq!(cover_destination, bounds);
        assert_eq!(cover_source.size, Size::new(200., 200.));
        assert_eq!(
            image_fit_rects(source, bounds, ImageFit::Fill, Alignment::CENTER).1,
            bounds
        );
        assert_eq!(
            image_fit_rects(
                Rect::from_origin_size(Offset::ZERO, Size::new(40., 20.)),
                bounds,
                ImageFit::ScaleDown,
                Alignment::CENTER
            )
            .1
            .size,
            Size::new(40., 20.)
        );
    }

    #[test]
    fn repeat_destinations_cover_requested_axes_from_the_fitted_origin() {
        let destination = Rect::from_origin_size(Offset::new(5., 2.), Size::new(10., 4.));
        let bounds = Rect::from_origin_size(Offset::ZERO, Size::new(30., 10.));
        let tiles = image_repeat_destinations(destination, bounds, ImageRepeat::RepeatX);
        assert_eq!(tiles.len(), 4);
        assert_eq!(tiles[0].origin, Offset::new(-5., 2.));
        assert_eq!(tiles[3].origin, Offset::new(25., 2.));
        assert_eq!(
            image_repeat_destinations(destination, bounds, ImageRepeat::NoRepeat),
            vec![destination]
        );
    }
}
