use super::super::super::*;

pub(super) fn lower_scrolling(widget: &Widget) -> RenderKind {
    match &widget.kind {
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
        WidgetKind::RawScrollbar {
            controller, style, ..
        } => RenderKind::RawScrollbar {
            controller: controller.clone(),
            style: *style,
        },
        WidgetKind::ListWheelScrollView { view } => {
            RenderKind::ListWheelScrollView { view: view.clone() }
        }
        WidgetKind::ListWheelViewport { viewport } => RenderKind::ListWheelViewport {
            viewport: viewport.clone(),
        },
        WidgetKind::DraggableScrollableSheet { sheet } => RenderKind::DraggableScrollableSheet {
            sheet: sheet.clone(),
        },
        WidgetKind::DraggableScrollableActuator { actuator, .. } => {
            RenderKind::DraggableScrollableActuator {
                actuator: actuator.clone(),
            }
        }
        WidgetKind::TwoDimensionalScrollView { view } => {
            RenderKind::TwoDimensionalScrollView { view: view.clone() }
        }
        WidgetKind::TwoDimensionalViewport { viewport } => RenderKind::TwoDimensionalViewport {
            viewport: viewport.clone(),
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
        _ => unreachable!("scroll lowering received a non-scrolling widget"),
    }
}
