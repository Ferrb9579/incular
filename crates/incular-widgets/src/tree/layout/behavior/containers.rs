use super::super::*;

impl WidgetTree {
    pub(super) fn layout_container_kind(
        &mut self,
        _id: RenderObjectId,
        kind: RenderKind,
        children: &[RenderObjectId],
        constraints: Constraints,
    ) -> Result<(Size, Vec<Offset>), TreeError> {
        Ok(match kind {
            RenderKind::Align {
                alignment,
                width_factor,
                height_factor,
            } => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, constraints.loosen())?;
                    let child_size = self
                        .render_live(child, "retained render must remain live")
                        .size;
                    let result = incular_layout::layout_align(
                        constraints,
                        child_size.into(),
                        incular_layout::Align {
                            alignment,
                            width_factor,
                            height_factor,
                        },
                    );
                    (
                        result.size,
                        result.children.into_iter().map(|c| c.offset).collect(),
                    )
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::Flex { flex } => {
                let cross_max = match flex.direction {
                    Axis::Horizontal => constraints.max_height(),
                    Axis::Vertical => constraints.max_width(),
                };
                let main_max = match flex.direction {
                    Axis::Horizontal => constraints.max_width(),
                    Axis::Vertical => constraints.max_height(),
                };
                let loose = match flex.direction {
                    Axis::Horizontal => Constraints::new(0., f32::INFINITY, 0., cross_max),
                    Axis::Vertical => Constraints::new(0., cross_max, 0., f32::INFINITY),
                };
                let flex_meta = children
                    .iter()
                    .map(|child| {
                        let render_node =
                            self.render_live(child, "retained render must remain live");
                        match &render_node.object.kind {
                            RenderKind::Flexible { flex: f, fit } => (*f, *fit),
                            _ => (0, FlexFit::Loose),
                        }
                    })
                    .collect::<Vec<_>>();

                let mut flex_children = Vec::with_capacity(children.len());
                let mut occupied_non_flex = 0.0;
                for (child, (f, fit)) in children.iter().zip(&flex_meta) {
                    if *f == 0 || !main_max.is_finite() {
                        self.layout_render(*child, loose)?;
                        let render = self.render_live(child, "retained render must remain live");
                        let size = render.size;
                        // Baselines ride into the algorithm so
                        // cross-axis baseline alignment resolves instead
                        // of silently behaving as start.
                        let mut flex_child = incular_layout::FlexChild::new(size);
                        if let Some(baseline) = render.baseline {
                            flex_child = flex_child.with_baseline(baseline);
                        }
                        occupied_non_flex += flex.direction.main_extent(size);
                        flex_children.push(flex_child);
                    } else {
                        flex_children.push(incular_layout::FlexChild::flexible(
                            Size::ZERO,
                            *f,
                            *fit,
                        ));
                    }
                }

                let total_flex: u32 = flex_meta.iter().map(|(f, _)| *f).sum();
                let spacing = flex.spacing.max(0.0) * children.len().saturating_sub(1) as f32;
                if main_max.is_finite() && total_flex > 0 {
                    let available_for_flex = (main_max - occupied_non_flex - spacing).max(0.0);
                    for (i, (child, (f, fit))) in children.iter().zip(&flex_meta).enumerate() {
                        if *f > 0 {
                            let allocation = available_for_flex * *f as f32 / total_flex as f32;
                            let child_constraints = match flex.direction {
                                Axis::Horizontal => Constraints::new(
                                    if *fit == FlexFit::Tight {
                                        allocation
                                    } else {
                                        0.0
                                    },
                                    allocation,
                                    0.0,
                                    cross_max,
                                ),
                                Axis::Vertical => Constraints::new(
                                    0.0,
                                    cross_max,
                                    if *fit == FlexFit::Tight {
                                        allocation
                                    } else {
                                        0.0
                                    },
                                    allocation,
                                ),
                            };
                            self.layout_render(*child, child_constraints)?;
                            let render =
                                self.render_live(child, "retained render must remain live");
                            let size = render.size;
                            let mut flex_child =
                                incular_layout::FlexChild::flexible(size, *f, *fit);
                            if let Some(baseline) = render.baseline {
                                flex_child = flex_child.with_baseline(baseline);
                            }
                            flex_children[i] = flex_child;
                        }
                    }
                }

                let result = incular_layout::layout_flex(constraints, &flex_children, flex);
                (
                    result.size,
                    result.children.into_iter().map(|c| c.offset).collect(),
                )
            }
            RenderKind::Flexible { .. } | RenderKind::Positioned { .. } => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, constraints)?;
                    let size = self
                        .render_live(child, "retained render must remain live")
                        .size;
                    (constraints.constrain(size), vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::Wrap { wrap } => {
                let child_constraints = constraints.loosen();
                for child in children {
                    self.layout_render(*child, child_constraints)?;
                }
                let wrap_children = children
                    .iter()
                    .map(|child| {
                        incular_layout::WrapChild::new(
                            self.render_live(child, "retained render must remain live")
                                .size,
                        )
                    })
                    .collect::<Vec<_>>();

                let result = incular_layout::layout_wrap(constraints, &wrap_children, wrap);
                (
                    result.size,
                    result.children.into_iter().map(|c| c.offset).collect(),
                )
            }
            RenderKind::Table {
                columns,
                column_spacing,
                row_spacing,
            } => {
                for child in children {
                    self.layout_render(*child, constraints.loosen())?;
                }
                let cells = children
                    .iter()
                    .map(|child| {
                        incular_layout::TableChild::new(
                            self.render_live(child, "retained render must remain live")
                                .size,
                        )
                    })
                    .collect::<Vec<_>>();
                let config = incular_layout::Table {
                    columns,
                    column_spacing,
                    row_spacing,
                };
                let result = incular_layout::layout_table(constraints, &cells, config);
                (
                    result.size,
                    result
                        .children
                        .into_iter()
                        .map(|cell| cell.offset)
                        .collect(),
                )
            }
            RenderKind::Stack { stack } => {
                let non_positioned_constraints = match stack.fit {
                    StackFit::Loose => constraints.loosen(),
                    StackFit::Expand => Constraints::tight(constraints.biggest()),
                    StackFit::Passthrough => constraints,
                };
                let mut max_non_pos_w: f32 = 0.0;
                let mut max_non_pos_h: f32 = 0.0;
                for child in children {
                    let render_node = self.render_live(child, "retained render must remain live");
                    if !matches!(render_node.object.kind, RenderKind::Positioned { .. }) {
                        self.layout_render(*child, non_positioned_constraints)?;
                        let size = self
                            .render_live(child, "retained render must remain live")
                            .size;
                        max_non_pos_w = max_non_pos_w.max(size.width);
                        max_non_pos_h = max_non_pos_h.max(size.height);
                    }
                }
                let stack_size = constraints.constrain(if matches!(stack.fit, StackFit::Expand) {
                    constraints.biggest()
                } else {
                    Size::new(max_non_pos_w, max_non_pos_h)
                });

                let mut stack_children = Vec::with_capacity(children.len());
                for child in children {
                    let render_node = self.render_live(child, "retained render must remain live");
                    if let RenderKind::Positioned {
                        left,
                        top,
                        right,
                        bottom,
                        width,
                        height,
                    } = render_node.object.kind
                    {
                        let pos = incular_layout::Positioned {
                            left,
                            top,
                            right,
                            bottom,
                            width,
                            height,
                        };
                        // Share the positioning policy: a configured axis
                        // (explicit size or opposing edges) tightens the
                        // child, so the size it adopts equals the size the
                        // algorithm anchors with and right/bottom insets are
                        // exact. Unconfigured axes stay loosely bounded by
                        // the stack.
                        let child_w = incular_layout::positioned_axis_size(
                            stack_size.width,
                            left,
                            right,
                            width,
                        );
                        let child_h = incular_layout::positioned_axis_size(
                            stack_size.height,
                            top,
                            bottom,
                            height,
                        );
                        let (min_w, max_w) = child_w.map_or((0.0, stack_size.width), |w| (w, w));
                        let (min_h, max_h) = child_h.map_or((0.0, stack_size.height), |h| (h, h));
                        let child_constraints = Constraints::new(min_w, max_w, min_h, max_h);
                        self.layout_render(*child, child_constraints)?;
                        let size = self
                            .render_live(child, "retained render must remain live")
                            .size;
                        stack_children.push(incular_layout::StackChild::positioned(size, pos));
                    } else {
                        let size = self
                            .render_live(child, "retained render must remain live")
                            .size;
                        stack_children.push(incular_layout::StackChild::new(size));
                    }
                }

                let result = incular_layout::layout_stack(constraints, &stack_children, stack);
                (
                    result.size,
                    result.children.into_iter().map(|c| c.offset).collect(),
                )
            }
            RenderKind::IndexedStack { alignment, fit, .. } => {
                // Fit applies the established Stack policy to every child
                // (all children are measured so inactive ones keep state
                // and identity), then the widest/tallest drives the box.
                let child_constraints = match fit {
                    StackFit::Loose => constraints.loosen(),
                    StackFit::Expand => Constraints::tight(constraints.biggest()),
                    StackFit::Passthrough => constraints,
                };
                let mut natural = Size::ZERO;
                for child in children {
                    self.layout_render(*child, child_constraints)?;
                    let child_size = self
                        .render_live(child, "retained render must remain live")
                        .size;
                    natural = Size::new(
                        natural.width.max(child_size.width),
                        natural.height.max(child_size.height),
                    );
                }
                let size = constraints.constrain(if matches!(fit, StackFit::Expand) {
                    constraints.biggest()
                } else {
                    natural
                });
                let offsets = children
                    .iter()
                    .map(|child| {
                        alignment.within(
                            size,
                            self.render_live(child, "retained render must remain live")
                                .size,
                        )
                    })
                    .collect();
                (size, offsets)
            }
            RenderKind::SafeArea {
                minimum,
                left,
                top,
                right,
                bottom,
                maintain_bottom_view_padding,
            } => {
                let ambient = self.environment.safe_insets.normalized();
                // A maintained bottom edge also honors the keyboard/IME
                // view inset, so showing the keyboard never collapses the
                // bottom padding below the obscured height. Disabled edges
                // stay at their minimum regardless.
                let view_bottom = if maintain_bottom_view_padding {
                    self.environment.view_insets.normalized().bottom.max(0.0)
                } else {
                    0.0
                };
                let insets = EdgeInsets::only(
                    if left {
                        ambient.left.max(minimum.left)
                    } else {
                        minimum.left
                    },
                    if top {
                        ambient.top.max(minimum.top)
                    } else {
                        minimum.top
                    },
                    if right {
                        ambient.right.max(minimum.right)
                    } else {
                        minimum.right
                    },
                    if bottom {
                        ambient.bottom.max(minimum.bottom).max(view_bottom)
                    } else {
                        minimum.bottom
                    },
                );
                let child_constraints = constraints.deflate(insets.horizontal(), insets.vertical());
                if let Some(&child) = children.first() {
                    self.layout_render(child, child_constraints)?;
                    let child_size = self
                        .render_live(child, "retained render must remain live")
                        .size;
                    let size = constraints.constrain(Size::new(
                        child_size.width + insets.horizontal(),
                        child_size.height + insets.vertical(),
                    ));
                    (size, vec![Offset::new(insets.left, insets.top)])
                } else {
                    let size =
                        constraints.constrain(Size::new(insets.horizontal(), insets.vertical()));
                    (size, Vec::new())
                }
            }
            RenderKind::ClipRect { .. }
            | RenderKind::ClipRRect { .. }
            | RenderKind::ClipOval { .. }
            | RenderKind::ClipPath { .. } => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, constraints)?;
                    let size = self
                        .render_live(child, "retained render must remain live")
                        .size;
                    (constraints.constrain(size), vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::LayoutBuilder => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, constraints)?;
                    let size = self
                        .render_live(child, "retained render must remain live")
                        .size;
                    (constraints.constrain(size), vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::Visibility {
                visible,
                maintain_size,
            } => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, constraints.loosen())?;
                    let measured = self
                        .render_live(child, "retained render must remain live")
                        .size;
                    let size = constraints.constrain(if visible || maintain_size {
                        measured
                    } else {
                        Size::ZERO
                    });
                    (size, vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::AspectRatio { ratio } => {
                let mut size = if constraints.is_width_bounded() {
                    Size::new(constraints.max_width(), constraints.max_width() / ratio)
                } else if constraints.is_height_bounded() {
                    Size::new(constraints.max_height() * ratio, constraints.max_height())
                } else {
                    Size::ZERO
                };
                if size.height > constraints.max_height() {
                    size = Size::new(constraints.max_height() * ratio, constraints.max_height());
                }
                size = constraints.constrain(size);
                if let Some(&child) = children.first() {
                    self.layout_render(child, Constraints::tight(size))?;
                    (size, vec![Offset::ZERO])
                } else {
                    (size, Vec::new())
                }
            }
            _ => unreachable!("container layout received a non-container render kind"),
        })
    }
}
