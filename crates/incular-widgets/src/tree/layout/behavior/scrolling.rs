use super::super::*;

impl WidgetTree {
    pub(super) fn layout_scrolling_kind(
        &mut self,
        id: RenderObjectId,
        kind: RenderKind,
        children: &[RenderObjectId],
        constraints: Constraints,
    ) -> (Size, Vec<Offset>) {
        match kind {
            RenderKind::Scroll {
                controller,
                axis,
                reverse: _,
                physics,
            } => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, scroll_constraints(axis, constraints));
                    let content = self
                        .render_live(child, "retained render must remain live")
                        .size;
                    let size = scroll_size(axis, constraints, content);
                    controller.update_extents_with_physics(
                        axis.main_extent(content),
                        scroll_viewport_extent(axis, size),
                        physics,
                    );
                    (size, vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::RawScrollbar { controller, style } => {
                let (size, offsets) = if let Some(&child) = children.first() {
                    self.layout_render(child, constraints.loosen());
                    let size = constraints.constrain(
                        self.render_live(child, "retained render must remain live")
                            .size,
                    );
                    (size, vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                };
                let state = self.raw_scrollbar_state_live_mut(id);
                let replace = match state.scrollbar.as_ref() {
                    Some(scrollbar) => scrollbar.controller() != controller,
                    None => true,
                };
                if replace {
                    let mut scrollbar = RawScrollbar::new(controller.clone());
                    scrollbar.set_style(style);
                    state.scrollbar = Some(scrollbar);
                } else if let Some(scrollbar) = state.scrollbar.as_mut()
                    && scrollbar.style() != style
                {
                    scrollbar.set_style(style);
                }
                let _ = state
                    .scrollbar
                    .as_ref()
                    .expect("raw scrollbar state")
                    .geometry(size);
                (size, offsets)
            }
            RenderKind::ListWheelScrollView { .. } | RenderKind::ListWheelViewport { .. } => {
                let (layout_size, placements) = self
                    .wheel_state_live(id)
                    .layout
                    .as_ref()
                    .map(|layout| {
                        (
                            layout.size,
                            layout
                                .children
                                .iter()
                                .map(|child| {
                                    (
                                        Constraints::new(
                                            0.0,
                                            layout.size.width,
                                            child.untransformed_rect.size.height,
                                            child.untransformed_rect.size.height,
                                        ),
                                        child.untransformed_rect.origin,
                                    )
                                })
                                .collect::<Vec<_>>(),
                        )
                    })
                    .unwrap_or_else(|| (advanced_viewport_size(constraints), Vec::new()));
                let mut offsets = Vec::with_capacity(children.len());
                for (child, (child_constraints, offset)) in children.iter().zip(placements) {
                    self.layout_render(*child, child_constraints);
                    offsets.push(offset);
                }
                (constraints.constrain(layout_size), offsets)
            }
            RenderKind::DraggableScrollableSheet { config } => {
                if let Some(state) = self
                    .render_live(id, "draggable-sheet render must remain live")
                    .draggable_sheet_state()
                    .and_then(|state| state.state.clone())
                {
                    let viewport_size = advanced_viewport_size(constraints);
                    state.set_parent_height(viewport_size.height);
                    let extent = state.extent();
                    let child_height = extent.current_pixels().clamp(0.0, viewport_size.height);
                    let sheet_height = if config.expand {
                        viewport_size.height
                    } else {
                        child_height
                    };
                    let size = constraints.constrain(Size::new(viewport_size.width, sheet_height));
                    let child_constraints =
                        Constraints::new(0.0, size.width, child_height, child_height);
                    let offsets = if let Some(&child) = children.first() {
                        self.layout_render(child, child_constraints);
                        vec![Offset::new(0.0, (size.height - child_height).max(0.0))]
                    } else {
                        Vec::new()
                    };
                    (size, offsets)
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::DraggableScrollableActuator { .. } => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, constraints.loosen());
                    let size = constraints.constrain(
                        self.render_live(child, "retained render must remain live")
                            .size,
                    );
                    (size, vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::TwoDimensionalScrollView { .. }
            | RenderKind::TwoDimensionalViewport { .. } => {
                let (layout_size, placements) = self
                    .two_dimensional_state_live(id)
                    .layout
                    .as_ref()
                    .map(|layout| {
                        (
                            layout.size,
                            layout
                                .children
                                .iter()
                                .map(|child| {
                                    (child.constraints.as_box_constraints(), child.paint_offset)
                                })
                                .collect::<Vec<_>>(),
                        )
                    })
                    .unwrap_or_else(|| (advanced_viewport_size(constraints), Vec::new()));
                let mut offsets = Vec::with_capacity(children.len());
                for (child, (child_constraints, offset)) in children.iter().zip(placements) {
                    self.layout_render(*child, child_constraints);
                    offsets.push(offset);
                }
                (constraints.constrain(layout_size), offsets)
            }
            _ => unreachable!("scrolling layout received a non-scrolling render kind"),
        }
    }
}
