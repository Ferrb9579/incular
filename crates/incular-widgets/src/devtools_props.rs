//! Structured inspection properties for built-in widget kinds.
//!
//! This is Incular's equivalent of Flutter's diagnostics properties: a closed,
//! typed contract rather than `Debug` dumps. Only these curated values cross
//! the DevTools wire; sensitive content is excluded at the source.

use crate::tree::WidgetKind;
use incular_devtools_protocol::{DebugProperty, DebugValue, PropertyChange};
fn prop(name: &str, value: DebugValue) -> DebugProperty {
    DebugProperty {
        name: name.to_owned(),
        value,
        overridden: false,
        editable: false,
    }
}

fn color_value(color: incular_core::Color) -> DebugValue {
    DebugValue::Color(color.red, color.green, color.blue, color.alpha)
}

fn size_value(size: incular_core::Size) -> DebugValue {
    DebugValue::Size([size.width, size.height])
}

fn offset_value(offset: incular_core::Offset) -> DebugValue {
    DebugValue::Offset([offset.x, offset.y])
}

/// Extracts the curated diagnostic properties for one widget kind. The
/// returned list is bounded; large collections are summarized.
pub fn inspect_properties(kind: &WidgetKind) -> Vec<DebugProperty> {
    let mut out = Vec::new();
    match kind {
        WidgetKind::Text {
            text,
            style,
            align,
            soft_wrap,
            max_lines,
            overflow,
        } => {
            out.push(prop("text", DebugValue::Str(truncate(text, 120))));
            out.push(prop("fontSize", DebugValue::Float(f64::from(style.size))));
            out.push(prop("color", color_value(style.color)));
            if let Some(height) = style.line_height {
                let val = match height {
                    incular_text::LineHeight::Normal => 0.0,
                    incular_text::LineHeight::Multiplier(m) => m as f64,
                    incular_text::LineHeight::Absolute(a) => a as f64,
                };
                out.push(prop("lineHeight", DebugValue::Float(val)));
            }
            out.push(prop(
                "fontWeight",
                DebugValue::Enum(format!("{:?}", style.weight)),
            ));
            out.push(prop("alignment", DebugValue::Enum(format!("{align:?}"))));
            out.push(prop("softWrap", DebugValue::Bool(*soft_wrap)));
            out.push(match max_lines {
                Some(lines) => prop("maxLines", DebugValue::Uint(*lines as u64)),
                None => prop("maxLines", DebugValue::Optional(None)),
            });
            out.push(prop("overflow", DebugValue::Enum(format!("{overflow:?}"))));
        }
        WidgetKind::Button(spec) => {
            out.push(prop("enabled", DebugValue::Bool(spec.callback.is_some())));
            if let Some(child) = spec.child.as_ref()
                && let Some(label) = child.text_if_any()
            {
                out.push(prop("label", DebugValue::Str(truncate(&label, 48))));
            }
        }
        WidgetKind::SelectionArea { controller, child } => {
            out.push(prop(
                "selectionRevision",
                DebugValue::Uint(controller.revision()),
            ));
            out.push(prop(
                "registeredChildCount",
                DebugValue::Uint(controller.registered_child_count() as u64),
            ));
            summarize_child(&mut out, child);
        }
        WidgetKind::SelectionContainer { delegate, child } => {
            out.push(prop("enabled", DebugValue::Bool(delegate.is_enabled())));
            out.push(prop(
                "selectionRevision",
                DebugValue::Uint(delegate.controller().revision()),
            ));
            summarize_child(&mut out, child);
        }
        WidgetKind::SelectionListener {
            notifier, child, ..
        } => {
            out.push(prop("registered", DebugValue::Bool(notifier.registered())));
            out.push(prop(
                "selectionRevision",
                DebugValue::Uint(notifier.revision()),
            ));
            summarize_child(&mut out, child);
        }
        WidgetKind::IndexedSemantics { index, child } => {
            out.push(prop("index", DebugValue::Uint(*index as u64)));
            summarize_child(&mut out, child);
        }
        WidgetKind::SemanticsDebugger {
            label_style,
            max_nodes,
            child,
        } => {
            out.push(prop("maxNodes", DebugValue::Uint(*max_nodes as u64)));
            out.push(prop(
                "fontSize",
                DebugValue::Float(f64::from(label_style.size)),
            ));
            summarize_child(&mut out, child);
        }
        WidgetKind::Flex { axis, children, .. } => {
            out.push(prop("direction", DebugValue::Enum(format!("{axis:?}"))));
            out.push(prop("childCount", DebugValue::Uint(children.len() as u64)));
        }
        WidgetKind::Wrap {
            axis,
            spacing,
            run_spacing,
            children,
            ..
        } => {
            out.push(prop("direction", DebugValue::Enum(format!("{axis:?}"))));
            out.push(prop("spacing", DebugValue::Float(f64::from(*spacing))));
            out.push(prop(
                "runSpacing",
                DebugValue::Float(f64::from(*run_spacing)),
            ));
            out.push(prop("childCount", DebugValue::Uint(children.len() as u64)));
        }
        WidgetKind::Stack {
            alignment,
            children,
            ..
        } => {
            out.push(prop(
                "alignment",
                DebugValue::Enum(format!("{alignment:?}")),
            ));
            out.push(prop("childCount", DebugValue::Uint(children.len() as u64)));
        }
        WidgetKind::Padding { padding, child } => {
            out.push(prop(
                "padding",
                DebugValue::Insets([padding.left, padding.top, padding.right, padding.bottom]),
            ));
            summarize_child(&mut out, child);
        }
        WidgetKind::Align {
            alignment, child, ..
        } => {
            out.push(prop(
                "alignment",
                DebugValue::Enum(format!("{alignment:?}")),
            ));
            summarize_child(&mut out, child);
        }
        WidgetKind::Decorated {
            background,
            border,
            radius,
            child,
            ..
        } => {
            if let Some(brush) = background {
                out.push(prop("background", brush_summary(brush)));
            }
            if let Some(border) = border {
                out.push(prop(
                    "borderWidth",
                    DebugValue::Float(f64::from(border.width)),
                ));
                out.push(prop("borderColor", color_value(border.color)));
            }
            out.push(prop(
                "radius",
                DebugValue::Float(f64::from(radius.top_left)),
            ));
            summarize_child(&mut out, child);
        }
        WidgetKind::Opacity {
            alpha,
            controller,
            child,
        } => {
            let mut opacity = prop("opacity", DebugValue::Float(f64::from(*alpha)));
            opacity.editable = controller.is_none();
            out.push(opacity);
            summarize_child(&mut out, child);
        }
        WidgetKind::Blur {
            sigma_x,
            sigma_y,
            child,
            ..
        } => {
            out.push(prop("sigmaX", DebugValue::Float(f64::from(*sigma_x))));
            out.push(prop("sigmaY", DebugValue::Float(f64::from(*sigma_y))));
            summarize_child(&mut out, child);
        }
        WidgetKind::Translate {
            controller, child, ..
        } => {
            let offset = controller.offset();
            out.push(prop("offset", offset_value(offset)));
            summarize_child(&mut out, child);
        }
        WidgetKind::Scale {
            controller, child, ..
        } => {
            out.push(prop(
                "scale",
                DebugValue::Float(f64::from(controller.scale())),
            ));
            summarize_child(&mut out, child);
        }
        WidgetKind::Rotation {
            controller, child, ..
        } => {
            out.push(prop(
                "angle",
                DebugValue::Float(f64::from(controller.angle_degrees())),
            ));
            summarize_child(&mut out, child);
        }
        WidgetKind::Image {
            image,
            width,
            height,
            sampling,
            ..
        } => {
            out.push(prop("imageId", DebugValue::Uint(image.id().0)));
            if let Some(width) = width {
                out.push(prop("width", DebugValue::Float(f64::from(*width))));
            }
            if let Some(height) = height {
                out.push(prop("height", DebugValue::Float(f64::from(*height))));
            }
            out.push(prop("sampling", DebugValue::Enum(format!("{sampling:?}"))));
        }
        WidgetKind::Scroll { controller, .. } => {
            out.push(prop(
                "scrollOffset",
                DebugValue::Float(f64::from(controller.offset())),
            ));
            out.push(prop(
                "maxOffset",
                DebugValue::Float(f64::from(controller.max_offset())),
            ));
            out.push(prop(
                "viewportExtent",
                DebugValue::Float(f64::from(controller.viewport_extent())),
            ));
            out.push(prop(
                "contentExtent",
                DebugValue::Float(f64::from(controller.content_extent())),
            ));
        }
        WidgetKind::SliverViewport { config } => {
            out.push(prop(
                "itemCount",
                DebugValue::Uint(config.delegate.child_count().unwrap_or(0) as u64),
            ));
            out.push(prop(
                "sliverCount",
                DebugValue::Uint(config.delegate.sliver_count() as u64),
            ));
            out.push(prop(
                "cacheExtent",
                DebugValue::Float(f64::from(config.cache_extent)),
            ));
        }
        WidgetKind::Visibility {
            visible,
            hidden,
            child,
        } => {
            out.push(prop("visible", DebugValue::Bool(*visible)));
            out.push(prop(
                "maintainSize",
                DebugValue::Bool(hidden.layout == crate::tree::HiddenLayout::PreserveSpace),
            ));
            out.push(prop(
                "maintainAnimation",
                DebugValue::Bool(hidden.animation),
            ));
            out.push(prop(
                "maintainSemantics",
                DebugValue::Bool(hidden.semantics),
            ));
            summarize_child(&mut out, child);
        }
        WidgetKind::Gesture {
            callbacks, child, ..
        } => {
            out.push(prop("hasTap", DebugValue::Bool(callbacks.on_tap.is_some())));
            out.push(prop(
                "hasDrag",
                DebugValue::Bool(
                    callbacks.on_horizontal_drag_update.is_some()
                        || callbacks.on_vertical_drag_update.is_some(),
                ),
            ));
            summarize_child(&mut out, child);
        }
        WidgetKind::RawInput { kind, child } => {
            out.push(prop(
                "inputType",
                DebugValue::Str(kind.type_().name().to_owned()),
            ));
            if let Some(child) = child {
                summarize_child(&mut out, child);
            }
        }
        WidgetKind::Box { size, .. } => {
            out.push(prop("size", size_value(*size)));
        }
        WidgetKind::Constrained { constraints, child } => {
            out.push(prop(
                "constraints",
                DebugValue::Constraints {
                    min_width: constraints.min_width,
                    max_width: constraints.max_width,
                    min_height: constraints.min_height,
                    max_height: constraints.max_height,
                },
            ));
            summarize_child(&mut out, child);
        }
        _ => {}
    }
    out
}

/// Produces a bounded, structured property diff for a compatible retained
/// widget update. Only curated inspectable properties participate: arbitrary
/// application objects are never recursively serialized for DevTools.
pub fn diff_properties(old: &WidgetKind, new: &WidgetKind) -> Vec<PropertyChange> {
    let old = inspect_properties(old)
        .into_iter()
        .map(|property| (property.name, property.value))
        .collect::<std::collections::BTreeMap<_, _>>();
    let new = inspect_properties(new)
        .into_iter()
        .map(|property| (property.name, property.value))
        .collect::<std::collections::BTreeMap<_, _>>();
    old.keys()
        .chain(new.keys())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .filter_map(|name| {
            let before = old.get(name).cloned();
            let after = new.get(name).cloned();
            (before != after).then(|| PropertyChange {
                name: name.clone(),
                old: before,
                new: after,
            })
        })
        .take(32)
        .collect()
}

fn summarize_child(out: &mut Vec<DebugProperty>, child: &crate::tree::Widget) {
    if let WidgetKind::Text { text, .. } = child.kind() {
        out.push(prop("child", DebugValue::Str(truncate(text, 24))));
    } else {
        out.push(prop(
            "child",
            DebugValue::Str(type_display_pub(child.kind())),
        ));
    }
}

fn brush_summary(brush: &incular_rendering::Brush) -> DebugValue {
    match brush {
        incular_rendering::Brush::Solid(color) => color_value(*color),
        other => DebugValue::Str(format!("{other:?}")),
    }
}

fn type_display_pub(kind: &WidgetKind) -> String {
    crate::devtools_props::kind_display_name(kind)
}

fn truncate(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        text.to_owned()
    } else {
        let mut out: String = text.chars().take(limit).collect();
        out.push('…');
        out
    }
}

/// Display name for any widget kind (single source used by tree + props).
pub fn kind_display_name(kind: &WidgetKind) -> String {
    match kind {
        WidgetKind::Flex { axis, .. } => match axis {
            incular_config::Axis::Vertical => "Column".to_owned(),
            incular_config::Axis::Horizontal => "Row".to_owned(),
        },
        _ => kind.structure().widget_type.name().to_owned(),
    }
}

/// Render-kind display names for the render-tree tab.
pub fn kind_display_name_render(kind: &crate::tree::RenderKind) -> String {
    use crate::tree::RenderKind;
    let name = match kind {
        RenderKind::Box { .. } => "Box",
        RenderKind::Shape { .. } => "Shape",
        RenderKind::CustomPaint { .. } => "CustomPaint",
        RenderKind::Decorated { .. } => "DecoratedBox",
        RenderKind::Banner { .. } => "Banner",
        RenderKind::Button { .. } => "Button",
        RenderKind::Text { .. } => "Text",
        RenderKind::SelectableText { .. } => "SelectableText",
        RenderKind::SelectionArea => "SelectionArea",
        RenderKind::SelectionContainer => "SelectionContainer",
        RenderKind::SelectionListener => "SelectionListener",
        RenderKind::IndexedSemantics => "IndexedSemantics",
        RenderKind::SemanticsDebugger { .. } => "SemanticsDebugger",
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
        RenderKind::Flexible { .. } => "Flexible",
        RenderKind::Positioned { .. } => "Positioned",
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
        RenderKind::Stack { .. } => "Stack",
        RenderKind::Wrap { .. } => "Wrap",
        RenderKind::Table { .. } => "Table",
        RenderKind::Flex { flex, .. } => {
            return match flex.direction {
                incular_config::Axis::Vertical => "Column".to_owned(),
                incular_config::Axis::Horizontal => "Row".to_owned(),
            };
        }
        RenderKind::IndexedStack { .. } => "IndexedStack",
        RenderKind::SafeArea { .. } => "SafeArea",
        RenderKind::ClipRect { .. } => "ClipRect",
        RenderKind::ClipRRect { .. } => "ClipRRect",
        RenderKind::ClipOval { .. } => "ClipOval",
        RenderKind::ClipPath { .. } => "ClipPath",
        RenderKind::ShaderMask { .. } => "ShaderMask",
        RenderKind::BackdropFilter { .. } => "BackdropFilter",
        RenderKind::AnnotatedRegion { .. } => "AnnotatedRegion",
        RenderKind::Leader { .. } => "CompositedTransformTarget",
        RenderKind::Follower { .. } => "CompositedTransformFollower",
        RenderKind::RawScrollbar { .. } => "RawScrollbar",
        RenderKind::ListWheelScrollView { .. } => "ListWheelScrollView",
        RenderKind::ListWheelViewport { .. } => "ListWheelViewport",
        RenderKind::DraggableScrollableSheet { .. } => "DraggableScrollableSheet",
        RenderKind::DraggableScrollableActuator { .. } => "DraggableScrollableActuator",
        RenderKind::TwoDimensionalScrollView { .. } => "TwoDimensionalScrollView",
        RenderKind::TwoDimensionalViewport { .. } => "TwoDimensionalViewport",
    };
    name.to_owned()
}
