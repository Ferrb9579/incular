use super::super::*;

impl WidgetTree {
    pub(super) fn layout_sliver_kind(
        &mut self,
        id: RenderObjectId,
        kind: RenderKind,
        _children: &[RenderObjectId],
        constraints: Constraints,
    ) -> (Size, Vec<Offset>) {
        match kind {
            RenderKind::SliverViewport { config } => {
                config
                    .controller
                    .set_metrics_context(config.axis, config.reverse);
                let mut size = sliver_viewport_size(config.axis, constraints, config.shrink_wrap);
                let mut viewport_extent = scroll_viewport_extent(config.axis, size);
                let cache_extent = config.cache_extent.max(0.);
                let cross_extent = config.axis.cross_extent(size);
                let make_constraints = |physical_offset: f32, viewport: f32| {
                    // Match RenderViewport's forward layout contract for
                    // overscroll: a negative physical position is represented
                    // as leading overlap and a reduced paint extent, while
                    // the sliver scroll offset itself stays non-negative.
                    let overlap = physical_offset.min(0.);
                    SliverConstraints::new(
                        config.axis,
                        config.reverse,
                        physical_offset.max(0.),
                        0.,
                        overlap,
                        (viewport + overlap).max(0.),
                        cross_extent,
                        viewport,
                        viewport + cache_extent * 2.,
                        -cache_extent,
                    )
                };
                let mut sliver_layout = config.delegate.perform_layout(make_constraints(
                    physical_scroll_offset(&config.controller, config.reverse),
                    viewport_extent,
                ));
                config.controller.update_extents_with_physics(
                    sliver_layout.geometry.scroll_extent,
                    viewport_extent,
                    config.physics,
                );

                // A shrink-wrapping viewport derives its own main-axis size
                // from the sliver geometry. The first pass supplies a
                // provisional zero/unbounded extent, then the real viewport
                // extent is laid out again before children are materialized.
                if config.shrink_wrap {
                    let content_extent = sliver_layout.geometry.scroll_extent.max(0.);
                    let main = if config.axis.is_vertical() {
                        content_extent.clamp(constraints.min_height, constraints.max_height)
                    } else {
                        content_extent.clamp(constraints.min_width, constraints.max_width)
                    };
                    size = constraints.constrain(config.axis.size(main, cross_extent));
                    viewport_extent = scroll_viewport_extent(config.axis, size);
                    sliver_layout = config.delegate.perform_layout(make_constraints(
                        physical_scroll_offset(&config.controller, config.reverse),
                        viewport_extent,
                    ));
                    config.controller.update_extents_with_physics(
                        sliver_layout.geometry.scroll_extent,
                        viewport_extent,
                        config.physics,
                    );
                }

                // Extent updates can clamp a restored/programmatic offset or
                // establish the physical origin of a reversed viewport. One
                // corrective layout keeps geometry and the retained range in
                // the same coordinate space from the first frame.
                sliver_layout = config.delegate.perform_layout(make_constraints(
                    physical_scroll_offset(&config.controller, config.reverse),
                    viewport_extent,
                ));
                config.controller.update_extents_with_physics(
                    sliver_layout.geometry.scroll_extent,
                    viewport_extent,
                    config.physics,
                );

                for _ in 0..3 {
                    let physical_before =
                        physical_scroll_offset(&config.controller, config.reverse);
                    let anchor_before = sliver_anchor(&sliver_layout, physical_before);
                    let result = self.reconcile_sliver_children(id, &config, &sliver_layout);
                    if !self.record_tree_result(result) {
                        return (size, Vec::new());
                    }
                    let materialized = self
                        .renders
                        .get(id.0)
                        .expect("sliver viewport render")
                        .children
                        .clone();
                    let mut pass_changed = false;
                    for (child, layout) in materialized.into_iter().zip(&sliver_layout.children) {
                        self.layout_render(child, layout.constraints);
                        let measured = config
                            .axis
                            .main_extent(self.renders.get(child.0).expect("sliver child").size);
                        pass_changed |= config.delegate.set_child_extent(layout.id, measured);
                        let child = self.renders.get_mut(child.0).expect("sliver child");
                        child.offset = config.axis.offset(layout.offset, layout.cross_offset);
                        self.compositor.update_transform(
                            child.object.layers.root,
                            CoreTransform::translation(child.offset),
                        );
                    }
                    if !pass_changed {
                        break;
                    }
                    let mut next_layout = config
                        .delegate
                        .perform_layout(make_constraints(physical_before, viewport_extent));
                    config.controller.update_extents_with_physics(
                        next_layout.geometry.scroll_extent,
                        viewport_extent,
                        config.physics,
                    );
                    // A reversed viewport's physical origin depends on the
                    // newly measured max offset. Recompute the layout after
                    // updating extents before comparing anchors; otherwise a
                    // first-frame estimate change can turn logical offset 0
                    // into an artificial scroll to the middle of the list.
                    let physical_after_extent =
                        physical_scroll_offset(&config.controller, config.reverse);
                    if physical_after_extent != physical_before {
                        next_layout = config.delegate.perform_layout(make_constraints(
                            physical_after_extent,
                            viewport_extent,
                        ));
                        config.controller.update_extents_with_physics(
                            next_layout.geometry.scroll_extent,
                            viewport_extent,
                            config.physics,
                        );
                    }
                    if let Some((anchor_id, anchor_offset)) = anchor_before
                        && let Some(updated) = next_layout
                            .children
                            .iter()
                            .find(|child| child.id == anchor_id)
                    {
                        // Compare visual positions, not raw content offsets.
                        // The physical origin may have changed when a reverse
                        // viewport learned its real content extent.
                        let delta = (updated.offset - physical_after_extent)
                            - (anchor_offset - physical_before);
                        if delta.is_finite() && delta.abs() > f32::EPSILON {
                            let corrected_physical = (physical_after_extent + delta).max(0.);
                            let corrected_logical = if config.reverse {
                                (config.controller.max_offset() - corrected_physical).max(0.)
                            } else {
                                corrected_physical
                            };
                            if config.controller.jump_to(corrected_logical) {
                                next_layout = config.delegate.perform_layout(make_constraints(
                                    physical_scroll_offset(&config.controller, config.reverse),
                                    viewport_extent,
                                ));
                                config.controller.update_extents_with_physics(
                                    next_layout.geometry.scroll_extent,
                                    viewport_extent,
                                    config.physics,
                                );
                            }
                        }
                    }
                    sliver_layout = next_layout;
                }
                if let Some(correction) = sliver_layout.geometry.scroll_offset_correction {
                    let physical = (physical_scroll_offset(&config.controller, config.reverse)
                        + correction)
                        .max(0.);
                    let logical = if config.reverse {
                        (config.controller.max_offset() - physical).max(0.)
                    } else {
                        physical
                    };
                    config.controller.jump_to(logical);
                }
                self.diagnostics.lazy_layouts += 1;
                (size, Vec::new())
            }
            _ => unreachable!("sliver layout received a non-sliver render kind"),
        }
    }
}
