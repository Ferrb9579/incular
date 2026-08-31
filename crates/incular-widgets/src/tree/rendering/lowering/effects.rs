use super::super::super::*;

pub(super) fn lower_effects(widget: &Widget) -> RenderKind {
    match &widget.kind {
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
        WidgetKind::ShaderMask {
            shader, blend_mode, ..
        } => RenderKind::ShaderMask {
            shader: shader.clone(),
            blend_mode: *blend_mode,
        },
        WidgetKind::BackdropFilter {
            blur,
            blend_mode,
            enabled,
            ..
        } => RenderKind::BackdropFilter {
            blur: *blur,
            blend_mode: *blend_mode,
            enabled: *enabled,
        },
        WidgetKind::AnnotatedRegion {
            annotation, sized, ..
        } => RenderKind::AnnotatedRegion {
            annotation: annotation.clone(),
            sized: *sized,
        },
        WidgetKind::CompositedTransformTarget { link, .. } => {
            RenderKind::Leader { link: link.clone() }
        }
        WidgetKind::CompositedTransformFollower {
            link,
            show_when_unlinked,
            offset,
            target_anchor,
            follower_anchor,
            ..
        } => RenderKind::Follower {
            link: link.clone(),
            show_when_unlinked: *show_when_unlinked,
            offset: *offset,
            target_anchor: *target_anchor,
            follower_anchor: *follower_anchor,
        },
        _ => unreachable!("effect lowering received a non-effect widget"),
    }
}
