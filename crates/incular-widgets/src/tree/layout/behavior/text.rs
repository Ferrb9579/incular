use super::super::*;

impl WidgetTree {
    pub(super) fn layout_text_kind(
        &mut self,
        id: RenderObjectId,
        kind: RenderKind,
        _children: &[RenderObjectId],
        constraints: Constraints,
    ) -> Result<(Size, Vec<Offset>), TreeError> {
        Ok(match kind {
            RenderKind::Text {
                text,
                style,
                align,
                soft_wrap,
                max_lines,
                overflow,
            } => {
                let width = constraints
                    .is_width_bounded()
                    .then_some(constraints.max_width);
                let _external_call = self
                    .recursion_diagnostics
                    .external_call(text_call_label("TextEngine::layout_with_options", &text));
                let layout = self.text_engine.layout_with_options(
                    &text,
                    &style,
                    TextLayoutOptions::new(width, align)
                        .soft_wrap(soft_wrap)
                        .max_lines(max_lines)
                        .overflow(overflow),
                );
                let size = constraints.constrain(layout.metrics.size);
                let node = self.render_live_mut(id, "retained render must remain live");
                node.set_text_layout(layout.clone());
                node.baseline = Some(layout.metrics.baseline);
                (size, Vec::new())
            }
            RenderKind::SelectableText { text, style, align } => {
                let width = constraints
                    .is_width_bounded()
                    .then_some(constraints.max_width);
                let _external_call = self
                    .recursion_diagnostics
                    .external_call(text_call_label("TextEngine::layout", &text));
                let layout = self.text_engine.layout(&text, &style, width, align);
                let size = constraints.constrain(layout.metrics.size);
                let node = self.render_live_mut(id, "retained render must remain live");
                node.set_text_layout(layout.clone());
                node.baseline = Some(layout.metrics.baseline);
                (size, Vec::new())
            }
            RenderKind::Image {
                image,
                width,
                height,
                ..
            } => {
                let intrinsic = image.decoded();
                let ratio = intrinsic.width() as f32 / intrinsic.height() as f32;
                let natural = match (width, height) {
                    (Some(w), Some(h)) => Size::new(w, h),
                    (Some(w), None) => Size::new(w, w / ratio),
                    (None, Some(h)) => Size::new(h * ratio, h),
                    (None, None) => Size::new(intrinsic.width() as f32, intrinsic.height() as f32),
                };
                (constraints.constrain(natural), Vec::new())
            }
            RenderKind::TextField {
                controller,
                desired,
                style,
                placeholder,
                multiline,
                min_lines,
                max_lines,
                expands,
                text_align,
                obscure_text,
                ..
            } => {
                let display = text_field_display(&controller, &placeholder, obscure_text);
                let intrinsic_width = if desired.width > 0.0 {
                    desired.width
                } else if constraints.is_width_bounded() {
                    constraints.max_width
                } else {
                    260.0
                };
                let width_for_text = (intrinsic_width - 16.).max(0.);
                let _external_call = self
                    .recursion_diagnostics
                    .external_call(text_call_label("TextEngine::layout_with_options", &display));
                let layout = self.text_engine.layout_with_options(
                    &display,
                    &style,
                    TextLayoutOptions::new(Some(width_for_text), text_align)
                        .soft_wrap(multiline)
                        .max_lines(max_lines),
                );
                let line_height = layout.metrics.line_height.max(1.0);
                let minimum_lines = min_lines.unwrap_or(1).max(1) as f32;
                let minimum_height = (line_height * minimum_lines + 16.0).max(32.0);
                let intrinsic_height = if desired.height > 0.0 {
                    desired.height
                } else if expands && constraints.is_height_bounded() {
                    constraints.max_height
                } else {
                    layout.metrics.size.height.max(line_height) + 16.0
                };
                let size = constraints.constrain(Size::new(
                    intrinsic_width,
                    intrinsic_height.max(minimum_height),
                ));
                let (revision, visual_revision) = controller.revisions();
                self.render_live_mut(id, "retained render must remain live")
                    .set_text_layout(layout.clone());
                {
                    let state = self.text_field_state_live_mut(id);
                    state.content_revision = revision;
                    state.visual_revision = visual_revision;
                }
                self.render_live_mut(id, "retained render must remain live")
                    .baseline = Some(layout.metrics.baseline);
                (size, Vec::new())
            }
            _ => unreachable!("text layout received a non-text render kind"),
        })
    }
}
