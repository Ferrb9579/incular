//! Family-local retained layout behavior.
//!
//! The dispatcher is intentionally exhaustive and algorithm-free. Individual
//! families own their layout algorithms while `WidgetTree` retains scheduling,
//! recursion protection, cache management, and topology ownership.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LayoutFamily {
    Core,
    Sliver,
    Containers,
    Text,
    Scrolling,
    Effects,
}

impl LayoutFamily {
    fn for_kind(kind: &RenderKind) -> Self {
        match kind {
            RenderKind::Box { .. }
            | RenderKind::Shape { .. }
            | RenderKind::CustomPaint { .. }
            | RenderKind::Decorated { .. }
            | RenderKind::Banner { .. }
            | RenderKind::Button { .. }
            | RenderKind::Padding { .. }
            | RenderKind::Constrained { .. }
            | RenderKind::Limited { .. }
            | RenderKind::Overflow { .. }
            | RenderKind::Unconstrained { .. }
            | RenderKind::Fractional { .. }
            | RenderKind::Baseline { .. }
            | RenderKind::RepaintBoundary
            | RenderKind::Gesture
            | RenderKind::SelectionArea
            | RenderKind::SelectionContainer
            | RenderKind::SelectionListener
            | RenderKind::IndexedSemantics
            | RenderKind::SemanticsDebugger { .. }
            | RenderKind::PersistentHeader { .. } => Self::Core,

            RenderKind::SliverViewport { .. } => Self::Sliver,

            RenderKind::Align { .. }
            | RenderKind::Flex { .. }
            | RenderKind::Flexible { .. }
            | RenderKind::Wrap { .. }
            | RenderKind::Table { .. }
            | RenderKind::Stack { .. }
            | RenderKind::Positioned { .. }
            | RenderKind::IndexedStack { .. }
            | RenderKind::SafeArea { .. }
            | RenderKind::ClipRect { .. }
            | RenderKind::ClipRRect { .. }
            | RenderKind::ClipOval { .. }
            | RenderKind::ClipPath { .. }
            | RenderKind::LayoutBuilder
            | RenderKind::Visibility { .. }
            | RenderKind::AspectRatio { .. } => Self::Containers,

            RenderKind::Text { .. }
            | RenderKind::SelectableText { .. }
            | RenderKind::Image { .. }
            | RenderKind::TextField { .. } => Self::Text,

            RenderKind::Scroll { .. }
            | RenderKind::RawScrollbar { .. }
            | RenderKind::ListWheelScrollView { .. }
            | RenderKind::ListWheelViewport { .. }
            | RenderKind::DraggableScrollableSheet { .. }
            | RenderKind::DraggableScrollableActuator { .. }
            | RenderKind::TwoDimensionalScrollView { .. }
            | RenderKind::TwoDimensionalViewport { .. } => Self::Scrolling,

            RenderKind::Translate { .. }
            | RenderKind::Transform { .. }
            | RenderKind::Scale { .. }
            | RenderKind::Rotation { .. }
            | RenderKind::FittedBox { .. }
            | RenderKind::Opacity { .. }
            | RenderKind::Blur { .. }
            | RenderKind::DropShadow { .. }
            | RenderKind::ColorFiltered { .. }
            | RenderKind::Blend { .. }
            | RenderKind::ShaderMask { .. }
            | RenderKind::BackdropFilter { .. }
            | RenderKind::AnnotatedRegion { .. }
            | RenderKind::Leader { .. }
            | RenderKind::Follower { .. } => Self::Effects,
        }
    }
}
mod containers;
mod core;
mod effects;
mod scrolling;
mod sliver;
mod text;

impl WidgetTree {
    pub(super) fn layout_kind(
        &mut self,
        id: RenderObjectId,
        kind: RenderKind,
        children: &[RenderObjectId],
        constraints: Constraints,
    ) -> (Size, Vec<Offset>) {
        match LayoutFamily::for_kind(&kind) {
            LayoutFamily::Core => self.layout_core_kind(id, kind, children, constraints),
            LayoutFamily::Sliver => self.layout_sliver_kind(id, kind, children, constraints),
            LayoutFamily::Containers => self.layout_container_kind(id, kind, children, constraints),
            LayoutFamily::Text => self.layout_text_kind(id, kind, children, constraints),
            LayoutFamily::Scrolling => self.layout_scrolling_kind(id, kind, children, constraints),
            LayoutFamily::Effects => self.layout_effect_kind(id, kind, children, constraints),
        }
    }
}
