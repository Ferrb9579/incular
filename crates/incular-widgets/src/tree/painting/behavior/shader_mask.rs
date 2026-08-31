use super::super::*;

impl WidgetTree {
    pub(super) fn paint_shader_mask_kind(
        &mut self,
        id: RenderObjectId,
        kind: RenderKind,
        size: Size,
    ) {
        match kind {
            RenderKind::ShaderMask { shader, blend_mode } => {
                let mask = shader.call(Rect::from_origin_size(Offset::ZERO, size));
                if let Some(layer) = self
                    .renders
                    .get(id.0)
                    .and_then(|node| node.object.layers.shader_mask)
                {
                    self.compositor.update_shader_mask(
                        layer,
                        mask,
                        blend_mode,
                        size,
                        self.render_world_transform(id),
                    );
                }
            }
            _ => unreachable!("shader-mask paint received a different render kind"),
        }
    }
}
