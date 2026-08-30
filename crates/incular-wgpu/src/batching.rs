use super::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RectangleInstance {
    pub rect: Rect,
    pub color: Color,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RectangleBatch {
    instances: Vec<RectangleInstance>,
}
impl RectangleBatch {
    #[must_use]
    pub fn instances(&self) -> &[RectangleInstance] {
        &self.instances
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.instances.is_empty()
    }
}
/// Rectangle-only compatibility inspection. Actual rendering uses an ordered
/// mixed rectangle/glyph stream and never globally sorts by pipeline.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BatchPlan {
    batches: Vec<RectangleBatch>,
}
impl BatchPlan {
    #[must_use]
    pub fn lower(list: &DisplayList) -> Self {
        let mut batches = Vec::new();
        let mut current = RectangleBatch::default();
        let mut transforms = vec![Transform::IDENTITY];
        for command in list.commands() {
            match command {
                PaintCommand::Rect { rect, color } => current.instances.push(RectangleInstance {
                    rect: transforms
                        .last()
                        .expect("transform stack")
                        .transform_rect_bbox(*rect),
                    color: *color,
                }),
                PaintCommand::PushTransform {
                    transform: local_transform,
                } => transforms.push(
                    transforms
                        .last()
                        .expect("transform stack")
                        .then(*local_transform),
                ),
                PaintCommand::PopTransform => {
                    if transforms.len() > 1 {
                        transforms.pop();
                    }
                }
                PaintCommand::Image { .. }
                | PaintCommand::RRect { .. }
                | PaintCommand::Border { .. }
                | PaintCommand::FillPath { .. }
                | PaintCommand::StrokePath { .. }
                | PaintCommand::PushClip { .. }
                | PaintCommand::PushClipRRect { .. }
                | PaintCommand::PushClipOval { .. }
                | PaintCommand::PushClipPath { .. }
                | PaintCommand::PopClip
                | PaintCommand::GlyphRun { .. }
                | PaintCommand::PushOpacity { .. }
                | PaintCommand::PopOpacity
                | PaintCommand::PushBlur { .. }
                | PaintCommand::PushDropShadow { .. }
                | PaintCommand::PushColorFilter { .. }
                | PaintCommand::PushBlend { .. }
                | PaintCommand::PushShaderMask { .. }
                | PaintCommand::PushBackdropFilter { .. }
                | PaintCommand::PopEffect => {
                    if !current.is_empty() {
                        batches.push(std::mem::take(&mut current));
                    }
                }
            }
        }
        if !current.is_empty() {
            batches.push(current);
        }
        Self { batches }
    }
    #[must_use]
    pub fn batches(&self) -> &[RectangleBatch] {
        &self.batches
    }
    #[must_use]
    pub fn rectangle_count(&self) -> usize {
        self.batches.iter().map(|batch| batch.instances.len()).sum()
    }
}
