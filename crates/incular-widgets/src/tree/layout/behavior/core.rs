use super::super::*;

impl WidgetTree {
    pub(super) fn layout_core_kind(
        &mut self,
        id: RenderObjectId,
        kind: RenderKind,
        children: &[RenderObjectId],
        constraints: Constraints,
    ) -> Result<(Size, Vec<Offset>), TreeError> {
        Ok(match kind {
            RenderKind::Box { desired, .. }
            | RenderKind::Shape { desired, .. }
            | RenderKind::CustomPaint { desired, .. } => {
                (constraints.constrain(desired), Vec::new())
            }
            RenderKind::Decorated { desired, .. } => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, constraints.loosen())?;
                    let child_size = self
                        .render_live(child, "retained render must remain live")
                        .size;
                    let wanted = desired.unwrap_or(child_size);
                    let size = constraints.constrain(Size::new(
                        wanted.width.max(child_size.width),
                        wanted.height.max(child_size.height),
                    ));
                    (size, vec![Offset::ZERO])
                } else {
                    (
                        constraints.constrain(desired.unwrap_or(Size::ZERO)),
                        Vec::new(),
                    )
                }
            }
            RenderKind::Banner {
                message,
                text_style,
                ..
            } => {
                let (size, offsets) = if let Some(&child) = children.first() {
                    self.layout_render(child, constraints.loosen())?;
                    let child_size = self
                        .render_live(child, "retained render must remain live")
                        .size;
                    (constraints.constrain(child_size), vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                };
                let _external_call = self
                    .recursion_diagnostics
                    .external_call(text_call_label("TextEngine::layout_with_options", &message));
                let text_layout = self.text_engine.layout_with_options(
                    &message,
                    &text_style,
                    TextLayoutOptions::new(Some(80.0), TextAlign::Center),
                );
                let node = self.render_live_mut(id, "retained render must remain live");
                node.set_text_layout(text_layout.clone());
                node.baseline = Some(text_layout.metrics.baseline);
                (size, offsets)
            }
            RenderKind::Button { desired, .. } => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, constraints.loosen())?;
                    let child_size = self
                        .render_live(child, "retained render must remain live")
                        .size;
                    let size = constraints.constrain(Size::new(
                        desired.width.max(child_size.width),
                        desired.height.max(child_size.height),
                    ));
                    (
                        size,
                        vec![Offset::new(
                            (size.width - child_size.width) / 2.0,
                            (size.height - child_size.height) / 2.0,
                        )],
                    )
                } else {
                    (constraints.constrain(desired), Vec::new())
                }
            }
            RenderKind::Padding { padding } => {
                let child_constraints =
                    constraints.deflate(padding.horizontal(), padding.vertical());
                if let Some(&child) = children.first() {
                    self.layout_render(child, child_constraints)?;
                    let s = self
                        .render_live(child, "retained render must remain live")
                        .size;
                    (
                        constraints.constrain(Size::new(
                            s.width + padding.horizontal(),
                            s.height + padding.vertical(),
                        )),
                        vec![Offset::new(padding.left, padding.top)],
                    )
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::Constrained {
                constraints: additional,
            } => {
                let child_constraints = enforced_constraints(constraints, additional);
                if let Some(&child) = children.first() {
                    self.layout_render(child, child_constraints)?;
                    let size = self
                        .render_live(child, "retained render must remain live")
                        .size;
                    (constraints.constrain(size), vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::Limited {
                max_width,
                max_height,
            } => {
                let child_constraints = Constraints::new(
                    constraints.min_width,
                    if constraints.max_width.is_infinite() {
                        max_width
                    } else {
                        constraints.max_width
                    },
                    constraints.min_height,
                    if constraints.max_height.is_infinite() {
                        max_height
                    } else {
                        constraints.max_height
                    },
                );
                if let Some(&child) = children.first() {
                    self.layout_render(child, child_constraints)?;
                    let size = self
                        .render_live(child, "retained render must remain live")
                        .size;
                    (constraints.constrain(size), vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::Overflow {
                min_width,
                max_width,
                min_height,
                max_height,
            } => {
                let child_constraints = Constraints::new(
                    min_width.unwrap_or(constraints.min_width),
                    max_width
                        .unwrap_or(constraints.max_width)
                        .max(min_width.unwrap_or(constraints.min_width)),
                    min_height.unwrap_or(constraints.min_height),
                    max_height
                        .unwrap_or(constraints.max_height)
                        .max(min_height.unwrap_or(constraints.min_height)),
                );
                if let Some(&child) = children.first() {
                    self.layout_render(child, child_constraints)?;
                    let size = self
                        .render_live(child, "retained render must remain live")
                        .size;
                    (constraints.constrain(size), vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::Unconstrained { constrained_axis } => {
                let child_constraints = unconstrained_constraints(constraints, constrained_axis);
                if let Some(&child) = children.first() {
                    self.layout_render(child, child_constraints)?;
                    let size = self
                        .render_live(child, "retained render must remain live")
                        .size;
                    (constraints.constrain(size), vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::Fractional {
                width_factor,
                height_factor,
            } => {
                let child_constraints =
                    fractional_constraints(constraints, width_factor, height_factor);
                if let Some(&child) = children.first() {
                    self.layout_render(child, child_constraints)?;
                    let size = self
                        .render_live(child, "retained render must remain live")
                        .size;
                    (constraints.constrain(size), vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::Baseline { baseline } => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, constraints.loosen())?;
                    let child = self.render_live(child, "retained render must remain live");
                    let result = incular_layout::layout_baseline(
                        constraints,
                        incular_layout::BaselineChild {
                            size: child.size,
                            baseline: child.baseline,
                        },
                        baseline,
                    );
                    self.render_live_mut(id, "retained render must remain live")
                        .baseline = Some(baseline);
                    (result.size, vec![result.children[0].offset])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::RepaintBoundary
            | RenderKind::Gesture
            | RenderKind::SelectionArea
            | RenderKind::SelectionContainer
            | RenderKind::SelectionListener
            | RenderKind::IndexedSemantics
            | RenderKind::SemanticsDebugger { .. } => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, constraints.loosen())?;
                    let size = constraints.constrain(
                        self.render_live(child, "retained render must remain live")
                            .size,
                    );
                    (size, vec![Offset::ZERO])
                } else {
                    let expands = self
                        .element_for_render(id)
                        .and_then(|element| self.elements.get(element.0))
                        .is_some_and(|element| {
                            matches!(
                                element.widget.kind(),
                                WidgetKind::RawInput { child: None, .. }
                            )
                        });
                    if expands {
                        let width = if constraints.max_width.is_finite() {
                            constraints.max_width
                        } else {
                            constraints.min_width
                        };
                        let height = if constraints.max_height.is_finite() {
                            constraints.max_height
                        } else {
                            constraints.min_height
                        };
                        (constraints.constrain(Size::new(width, height)), Vec::new())
                    } else {
                        (constraints.constrain(Size::ZERO), Vec::new())
                    }
                }
            }
            RenderKind::PersistentHeader { .. } => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, constraints.loosen())?;
                    let size = constraints.constrain(
                        self.render_live(child, "retained render must remain live")
                            .size,
                    );
                    (size, vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            _ => unreachable!("core layout received a non-core render kind"),
        })
    }
}
