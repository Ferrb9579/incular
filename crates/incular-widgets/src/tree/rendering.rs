//! Widget-to-render lowering and renderer-independent visual helpers.

use super::*;

mod lowering;
pub use lowering::render_kind;

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
            RenderKind::RawScrollbar { .. } => "RawScrollbar",
            RenderKind::ListWheelScrollView { .. } => "ListWheelScrollView",
            RenderKind::ListWheelViewport { .. } => "ListWheelViewport",
            RenderKind::DraggableScrollableSheet { .. } => "DraggableScrollableSheet",
            RenderKind::DraggableScrollableActuator { .. } => "DraggableScrollableActuator",
            RenderKind::TwoDimensionalScrollView { .. } => "TwoDimensionalScrollView",
            RenderKind::TwoDimensionalViewport { .. } => "TwoDimensionalViewport",
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
            RenderKind::ShaderMask { .. } => "ShaderMask",
            RenderKind::BackdropFilter { .. } => "BackdropFilter",
            RenderKind::AnnotatedRegion { .. } => "AnnotatedRegion",
            RenderKind::Leader { .. } => "CompositedTransformTarget",
            RenderKind::Follower { .. } => "CompositedTransformFollower",
            RenderKind::Flexible { .. } => "Flexible",
        };
        name.to_owned()
    }
}

pub(super) fn resolve_text_style(style: &TextStyle, context: &DependencyContext) -> TextStyle {
    context
        .depend::<TextStyle>()
        .map_or_else(|| style.clone(), |default| default.merge(style))
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

/// Resolves a retained transform against the measured size: an optional
/// child-size fraction becomes an absolute translation applied after the
/// absolute transform, then the whole thing pivots around the origin.
/// Every projection (compositor, hit testing, semantics) resolves through
/// this one function so the three can never disagree.
pub(super) fn resolve_transform(
    transform: CoreTransform,
    origin: Option<Offset>,
    fraction: Option<Offset>,
    size: Size,
) -> CoreTransform {
    let absolute = match fraction {
        Some(fraction) => transform.then(CoreTransform::translation(Offset::new(
            fraction.x * size.width,
            fraction.y * size.height,
        ))),
        None => transform,
    };
    transform_around(absolute, origin, size)
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
