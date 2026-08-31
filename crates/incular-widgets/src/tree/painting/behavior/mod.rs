//! Family-local display-list generation.
//!
//! Painting orchestration and painter-order traversal stay in `painting.rs`;
//! this module owns only render-kind-specific local drawing behavior.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PaintFamily {
    Visual,
    Content,
    Scrolling,
    ShaderMask,
    Noop,
}

impl PaintFamily {
    fn for_kind(kind: &RenderKind) -> Self {
        match kind {
            RenderKind::Box { .. }
            | RenderKind::Shape { .. }
            | RenderKind::CustomPaint { .. }
            | RenderKind::Decorated { .. }
            | RenderKind::Banner { .. }
            | RenderKind::Button { .. } => Self::Visual,

            RenderKind::Text { .. }
            | RenderKind::SelectableText { .. }
            | RenderKind::SelectionArea
            | RenderKind::SelectionContainer
            | RenderKind::SelectionListener
            | RenderKind::IndexedSemantics
            | RenderKind::SemanticsDebugger { .. }
            | RenderKind::Image { .. }
            | RenderKind::TextField { .. } => Self::Content,

            RenderKind::Scroll { .. } | RenderKind::SliverViewport { .. } => Self::Scrolling,
            RenderKind::ShaderMask { .. } => Self::ShaderMask,

            RenderKind::Padding { .. }
            | RenderKind::Constrained { .. }
            | RenderKind::Limited { .. }
            | RenderKind::Overflow { .. }
            | RenderKind::Unconstrained { .. }
            | RenderKind::Fractional { .. }
            | RenderKind::Baseline { .. }
            | RenderKind::RepaintBoundary
            | RenderKind::Gesture
            | RenderKind::Align { .. }
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
            | RenderKind::AspectRatio { .. }
            | RenderKind::RawScrollbar { .. }
            | RenderKind::ListWheelScrollView { .. }
            | RenderKind::ListWheelViewport { .. }
            | RenderKind::DraggableScrollableSheet { .. }
            | RenderKind::DraggableScrollableActuator { .. }
            | RenderKind::TwoDimensionalScrollView { .. }
            | RenderKind::TwoDimensionalViewport { .. }
            | RenderKind::PersistentHeader { .. }
            | RenderKind::Translate { .. }
            | RenderKind::Transform { .. }
            | RenderKind::Scale { .. }
            | RenderKind::Rotation { .. }
            | RenderKind::FittedBox { .. }
            | RenderKind::Opacity { .. }
            | RenderKind::Blur { .. }
            | RenderKind::DropShadow { .. }
            | RenderKind::ColorFiltered { .. }
            | RenderKind::Blend { .. }
            | RenderKind::BackdropFilter { .. }
            | RenderKind::AnnotatedRegion { .. }
            | RenderKind::Leader { .. }
            | RenderKind::Follower { .. } => Self::Noop,
        }
    }
}

mod content;
mod scrolling;
mod shader_mask;
mod visual;

impl WidgetTree {
    pub(super) fn paint_kind(
        &mut self,
        id: RenderObjectId,
        kind: RenderKind,
        size: Size,
        cache: &mut DisplayList,
    ) -> Option<Color> {
        match PaintFamily::for_kind(&kind) {
            PaintFamily::Visual => self.paint_visual_kind(id, kind, size, cache),
            PaintFamily::Content => {
                self.paint_content_kind(id, kind, size, cache);
                None
            }
            PaintFamily::Scrolling => {
                self.paint_scrolling_kind(id, kind, size, cache);
                None
            }
            PaintFamily::ShaderMask => {
                self.paint_shader_mask_kind(id, kind, size);
                None
            }
            PaintFamily::Noop => None,
        }
    }
}
