use super::super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LoweringFamily {
    Visual,
    Layout,
    Scrolling,
    Effects,
}

impl LoweringFamily {
    fn for_widget(kind: &WidgetKind) -> Self {
        match kind {
            WidgetKind::Box { .. }
            | WidgetKind::Shape { .. }
            | WidgetKind::CustomPaint { .. }
            | WidgetKind::Decorated { .. }
            | WidgetKind::Banner { .. }
            | WidgetKind::Button { .. }
            | WidgetKind::Text { .. }
            | WidgetKind::SelectableText { .. }
            | WidgetKind::SelectionArea { .. }
            | WidgetKind::SelectionContainer { .. }
            | WidgetKind::SelectionListener { .. }
            | WidgetKind::IndexedSemantics { .. }
            | WidgetKind::SemanticsDebugger { .. }
            | WidgetKind::Image { .. }
            | WidgetKind::TextField { .. } => Self::Visual,

            WidgetKind::Padding { .. }
            | WidgetKind::Constrained { .. }
            | WidgetKind::Limited { .. }
            | WidgetKind::Overflow { .. }
            | WidgetKind::Unconstrained { .. }
            | WidgetKind::Fractional { .. }
            | WidgetKind::Baseline { .. }
            | WidgetKind::RepaintBoundary { .. }
            | WidgetKind::Gesture { .. }
            | WidgetKind::RawInput { .. }
            | WidgetKind::Draggable { .. }
            | WidgetKind::DragTarget { .. }
            | WidgetKind::IgnorePointer { .. }
            | WidgetKind::AbsorbPointer { .. }
            | WidgetKind::Align { .. }
            | WidgetKind::Flex { .. }
            | WidgetKind::Flexible { .. }
            | WidgetKind::Wrap { .. }
            | WidgetKind::Table { .. }
            | WidgetKind::Stack { .. }
            | WidgetKind::SafeArea { .. }
            | WidgetKind::ClipRect { .. }
            | WidgetKind::ClipRRect { .. }
            | WidgetKind::ClipOval { .. }
            | WidgetKind::ClipPath { .. }
            | WidgetKind::Positioned { .. }
            | WidgetKind::IndexedStack { .. }
            | WidgetKind::LayoutBuilder { .. }
            | WidgetKind::Visibility { .. }
            | WidgetKind::AspectRatio { .. } => Self::Layout,

            WidgetKind::Scroll { .. }
            | WidgetKind::RawScrollbar { .. }
            | WidgetKind::ListWheelScrollView { .. }
            | WidgetKind::ListWheelViewport { .. }
            | WidgetKind::DraggableScrollableSheet { .. }
            | WidgetKind::DraggableScrollableActuator { .. }
            | WidgetKind::TwoDimensionalScrollView { .. }
            | WidgetKind::TwoDimensionalViewport { .. }
            | WidgetKind::PersistentHeader { .. }
            | WidgetKind::NotificationListener { .. }
            | WidgetKind::SliverViewport { .. } => Self::Scrolling,

            WidgetKind::Translate { .. }
            | WidgetKind::Transform { .. }
            | WidgetKind::Scale { .. }
            | WidgetKind::Rotation { .. }
            | WidgetKind::FittedBox { .. }
            | WidgetKind::Opacity { .. }
            | WidgetKind::Blur { .. }
            | WidgetKind::DropShadow { .. }
            | WidgetKind::ColorFiltered { .. }
            | WidgetKind::Blend { .. }
            | WidgetKind::ShaderMask { .. }
            | WidgetKind::BackdropFilter { .. }
            | WidgetKind::AnnotatedRegion { .. }
            | WidgetKind::CompositedTransformTarget { .. }
            | WidgetKind::CompositedTransformFollower { .. } => Self::Effects,
        }
    }
}

#[doc(hidden)]
pub fn render_kind(widget: &Widget, environment: Option<&Rc<dyn Any>>) -> RenderKind {
    match LoweringFamily::for_widget(&widget.kind) {
        LoweringFamily::Visual => lower_visual(widget, environment),
        LoweringFamily::Layout => lower_layout(widget),
        LoweringFamily::Scrolling => lower_scrolling(widget),
        LoweringFamily::Effects => lower_effects(widget),
    }
}

mod effects;
mod layout;
mod scrolling;
mod visual;

use effects::lower_effects;
use layout::lower_layout;
use scrolling::lower_scrolling;
use visual::lower_visual;
