use super::super::super::*;

pub(super) fn lower_scrolling(widget: &Widget) -> RenderKind {
    match widget.kind() {
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
        WidgetKind::ListWheelScrollView { config } => RenderKind::ListWheelScrollView {
            config: config.clone(),
        },
        WidgetKind::ListWheelViewport { config } => RenderKind::ListWheelViewport {
            config: config.clone(),
        },
        WidgetKind::DraggableScrollableSheet { config } => RenderKind::DraggableScrollableSheet {
            config: config.clone(),
        },
        WidgetKind::DraggableScrollableActuator { actuator, .. } => {
            RenderKind::DraggableScrollableActuator {
                actuator: actuator.clone(),
            }
        }
        WidgetKind::TwoDimensionalScrollView { config } => RenderKind::TwoDimensionalScrollView {
            config: config.clone(),
        },
        WidgetKind::TwoDimensionalViewport { config } => RenderKind::TwoDimensionalViewport {
            config: config.clone(),
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
