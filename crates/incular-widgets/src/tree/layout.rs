//! Constraint propagation, retained layout, transforms, and viewport geometry.

use super::*;

use super::rendering::{fitted_transform, transform_around, transform_around_alignment};
use super::text::text_field_display;

impl WidgetTree {
    pub fn mark_paint(&mut self, id: ElementId) -> Result<(), TreeError> {
        let render = self.render_id(id).ok_or(TreeError::MissingElement(id))?;
        self.mark_render_dirty(render, DirtyFlags::PAINT, false);
        Ok(())
    }
    pub fn layout(&mut self, constraints: Constraints) {
        let _phase_guard = self.guard_phase_root(FramePhase::Layout);
        self.refresh_text_fields();
        self.refresh_sliver_ranges();
        self.refresh_stateful_layout_builders();
        if let Some(root) = self.root.and_then(|id| self.render_id(id)) {
            self.layout_render(root, constraints);
        }
        self.refresh_notification_listeners();
    }

    /// Marks local-state layout builders dirty before the normal retained
    /// layout cache runs.  Input callbacks can mutate a control's revision
    /// without replacing its parent widget description; this keeps that
    /// update local while still allowing a changed child size to propagate.
    pub(super) fn refresh_stateful_layout_builders(&mut self) {
        let dirty = self
            .elements
            .iter()
            .filter_map(|(_, element)| {
                let WidgetKind::LayoutBuilder {
                    revision: Some(revision),
                    ..
                } = &element.widget.kind
                else {
                    return None;
                };
                (revision.get() != element.layout_builder_revision).then_some(element.render)
            })
            .collect::<Vec<_>>();
        for render in dirty {
            self.mark_render_dirty(render, DirtyFlags::LAYOUT | DirtyFlags::PAINT, true);
        }
    }
    /// Synchronizes the retained semantic arena after layout/compositor state
    /// is valid. Non-semantic layout widgets merge their descendants into the
    /// closest meaningful semantic ancestor.
    pub(super) fn refresh_text_fields(&mut self) {
        let pending = self
            .renders
            .iter()
            .filter_map(|(raw, render)| {
                let RenderKind::TextField { controller, .. } = &render.kind else {
                    return None;
                };
                let (content, visual) = controller.revisions();
                ((content != render.text_revision) || (visual != render.text_visual_revision))
                    .then_some(RenderObjectId(raw))
            })
            .collect::<Vec<_>>();
        for render in pending {
            // Text width can change the size seen by an unconstrained parent.
            self.mark_render_dirty(render, DirtyFlags::LAYOUT | DirtyFlags::PAINT, true);
        }
    }

    #[doc(hidden)]
    pub fn content_transform(&self, id: RenderObjectId) -> Option<CoreTransform> {
        let node = self.renders.get(id.0)?;
        match &node.kind {
            RenderKind::Transform { transform, origin } => {
                Some(transform_around(*transform, *origin, node.size))
            }
            RenderKind::Scale { controller, origin } => Some(transform_around(
                CoreTransform::scale(controller.scale()),
                *origin,
                node.size,
            )),
            RenderKind::Rotation {
                controller,
                origin,
                alignment,
            } => Some(transform_around_alignment(
                CoreTransform::rotation(controller.radians()),
                *origin,
                *alignment,
                node.size,
            )),
            RenderKind::FittedBox { fit, alignment } => {
                let child = *node.children.first()?;
                let child_size = self.renders.get(child.0)?.size;
                Some(fitted_transform(child_size, node.size, *fit, *alignment))
            }
            _ => None,
        }
    }
    pub(super) fn child_content_transform(&self, id: RenderObjectId) -> CoreTransform {
        let node = self.renders.get(id.0).expect("live render");
        match &node.kind {
            RenderKind::Scroll {
                controller,
                axis,
                reverse,
                ..
            } => CoreTransform::translation(scroll_translation(controller, *axis, *reverse)),
            RenderKind::SliverViewport { config } => CoreTransform::translation(
                scroll_translation(&config.controller, config.axis, config.reverse),
            ),
            RenderKind::PersistentHeader {
                controller,
                axis,
                reverse,
                pinned,
            } => CoreTransform::translation(
                self.persistent_header_translation(id, controller, *axis, *reverse, *pinned),
            ),
            RenderKind::Translate { controller } => CoreTransform::translation(controller.offset()),
            _ => self
                .content_transform(id)
                .unwrap_or(CoreTransform::IDENTITY),
        }
    }
    pub(super) fn render_world_transform(&self, id: RenderObjectId) -> CoreTransform {
        let mut path = Vec::new();
        let mut cursor = Some(id);
        while let Some(current) = cursor {
            path.push(current);
            cursor = self.renders.get(current.0).expect("live render").parent;
        }
        path.reverse();
        let mut world = CoreTransform::IDENTITY;
        for (index, current) in path.iter().enumerate() {
            let node = self.renders.get(current.0).expect("live render");
            world = world.then(CoreTransform::translation(node.offset));
            if index + 1 != path.len() {
                world = world.then(self.child_content_transform(*current));
            }
        }
        world
    }
    pub(super) fn semantic_bounds(&self, id: RenderObjectId, size: Size) -> Rect {
        let mut world = self.render_world_transform(id);
        if self.content_transform(id).is_some() {
            world = world.then(self.child_content_transform(id));
        }
        world.transform_rect_bbox(Rect::from_origin_size(Offset::ZERO, size))
    }
    /// Returns the retained world-space origin of a render object, including
    /// scrolling and compositor translations applied along its ancestor path.
    #[doc(hidden)]
    pub fn render_origin(&self, mut id: RenderObjectId) -> Offset {
        let mut origin = Offset::ZERO;
        loop {
            let node = self.renders.get(id.0).expect("live render");
            origin = origin + node.offset;
            match &node.kind {
                RenderKind::Scroll {
                    controller,
                    axis,
                    reverse,
                    ..
                } => origin = origin + scroll_translation(controller, *axis, *reverse),
                RenderKind::SliverViewport { config } => {
                    origin =
                        origin + scroll_translation(&config.controller, config.axis, config.reverse)
                }
                RenderKind::PersistentHeader {
                    controller,
                    axis,
                    reverse,
                    pinned,
                } => {
                    origin = origin
                        + self
                            .persistent_header_translation(id, controller, *axis, *reverse, *pinned)
                }
                RenderKind::Translate { controller } => origin = origin + controller.offset(),
                _ => {}
            }
            let Some(parent) = node.parent else {
                return origin;
            };
            id = parent;
        }
    }
    /// World origin of a render object's viewport/picture. Unlike
    /// `render_origin`, this deliberately does not apply the object's own
    /// scrolling transform: a scrollbar is attached to that viewport, not to
    /// its scrolling content. Ancestor transforms still apply.
    pub(super) fn render_viewport_origin(&self, mut id: RenderObjectId) -> Offset {
        let mut origin = Offset::ZERO;
        let mut is_self = true;
        loop {
            let node = self.renders.get(id.0).expect("live render");
            origin = origin + node.offset;
            if !is_self {
                match &node.kind {
                    RenderKind::Scroll {
                        controller,
                        axis,
                        reverse,
                        ..
                    } => origin = origin + scroll_translation(controller, *axis, *reverse),
                    RenderKind::SliverViewport { config } => {
                        origin = origin
                            + scroll_translation(&config.controller, config.axis, config.reverse)
                    }
                    RenderKind::PersistentHeader {
                        controller,
                        axis,
                        reverse,
                        pinned,
                    } => {
                        origin = origin
                            + self.persistent_header_translation(
                                id, controller, *axis, *reverse, *pinned,
                            )
                    }
                    RenderKind::Translate { controller } => origin = origin + controller.offset(),
                    _ => {}
                }
            }
            let Some(parent) = node.parent else {
                return origin;
            };
            id = parent;
            is_self = false;
        }
    }
    /// Returns the compositor-only compensation for a persistent header. A
    /// header remains a normal flow child; when it reaches the viewport edge,
    /// this transform cancels its parent's scroll transform. The following
    /// persistent sibling limits the compensation and pushes it away.
    pub(super) fn persistent_header_translation(
        &self,
        header: RenderObjectId,
        controller: &ScrollController,
        axis: Axis,
        reverse: bool,
        pinned: bool,
    ) -> Offset {
        if !pinned {
            return Offset::ZERO;
        }
        let Some((scroll, flow_position)) = self.scroll_flow_position(header, controller, axis)
        else {
            return Offset::ZERO;
        };
        let header_extent = self
            .renders
            .get(header.0)
            .map_or(0., |node| axis.main_extent(node.size));
        let viewport_extent = self
            .renders
            .get(scroll.0)
            .map_or(0., |node| axis.main_extent(node.size));
        let physical_scroll = physical_scroll_offset(controller, reverse);
        let current_position = flow_position - physical_scroll;
        let leading_position = if reverse {
            (viewport_extent - header_extent).max(0.)
        } else {
            0.
        };
        let next_y = self
            .renders
            .iter()
            .filter_map(|(raw, node)| match &node.kind {
                RenderKind::PersistentHeader {
                    controller: candidate,
                    axis: candidate_axis,
                    reverse: candidate_reverse,
                    pinned: candidate_pinned,
                } if candidate == controller
                    && *candidate_axis == axis
                    && *candidate_reverse == reverse
                    && *candidate_pinned =>
                {
                    self.scroll_flow_position(RenderObjectId(raw), candidate, axis)
                        .filter(|(candidate_scroll, position)| {
                            *candidate_scroll == scroll && *position > flow_position
                        })
                        .map(|(_, position)| position)
                }
                _ => None,
            })
            .min_by(|left, right| left.total_cmp(right));
        let translation = (leading_position - current_position).max(0.);
        let translation = next_y.map_or(translation, |next| {
            // The push distance is a flow-space relationship between
            // neighboring headers.  Applying the current scroll offset here
            // would double-count the content transform and make a preceding
            // header disappear too early.
            translation.min((next - flow_position - header_extent).max(0.))
        });
        axis.offset(translation, 0.)
    }
    /// Static y-position within the scroll content and the matching scroll
    /// viewport. Dynamic scroll/pinning transforms are deliberately excluded.
    pub(super) fn scroll_flow_position(
        &self,
        mut id: RenderObjectId,
        controller: &ScrollController,
        axis: Axis,
    ) -> Option<(RenderObjectId, f32)> {
        let mut position = 0.;
        loop {
            let node = self.renders.get(id.0)?;
            position += match axis {
                Axis::Horizontal => node.offset.x,
                Axis::Vertical => node.offset.y,
            };
            if let RenderKind::Scroll {
                controller: viewport,
                ..
            } = &node.kind
                && viewport == controller
            {
                return Some((id, position));
            }
            id = node.parent?;
        }
    }

    pub(super) fn mark_render_dirty(
        &mut self,
        id: RenderObjectId,
        flags: DirtyFlags,
        propagate_layout: bool,
    ) {
        let already_dirty = self
            .renders
            .get(id.0)
            .is_some_and(|node| node.dirty.contains(flags));
        self.diagnostics.dirty_requests += 1;
        if already_dirty {
            self.diagnostics.dirty_queue_deduplicated += 1;
        } else {
            self.diagnostics.dirty_queue_insertions += 1;
        }
        let mut current = Some(id);
        while let Some(render) = current {
            let node = self.renders.get_mut(render.0).expect("live render");
            node.dirty.insert(flags);
            current = if propagate_layout && flags.contains(DirtyFlags::LAYOUT) {
                node.parent
            } else {
                None
            };
        }
    }
    pub(super) fn layout_render(&mut self, id: RenderObjectId, constraints: Constraints) {
        stacker::maybe_grow(128 * 1024, 2 * 1024 * 1024, || {
            self.layout_render_inner(id, constraints);
        });
    }

    pub(super) fn layout_render_inner(&mut self, id: RenderObjectId, constraints: Constraints) {
        let _node_guard = self.guard_render(FramePhase::Layout, id, Some(constraints));
        let needs = self
            .renders
            .get(id.0)
            .expect("live")
            .dirty
            .contains(DirtyFlags::LAYOUT)
            || self.renders.get(id.0).expect("live").constraints != Some(constraints);
        if !needs {
            self.diagnostics.layout_cache_hits += 1;
            return;
        }
        #[cfg(feature = "devtools")]
        let trace = self
            .element_for_render(id)
            .and_then(|element| self.devtools_trace_begin_element(element, TracePhase::Layout));
        #[cfg(feature = "devtools")]
        let old_layout = self.deep_trace.as_ref().map(|_| {
            let render = self.renders.get(id.0).expect("live");
            (render.constraints, render.size)
        });
        if matches!(
            self.renders.get(id.0).expect("live").kind,
            RenderKind::LayoutBuilder
        ) {
            self.materialize_layout_builder(id, constraints);
        }
        let (kind, children) = {
            let n = self.renders.get(id.0).expect("live");
            (n.kind.clone(), n.children.clone())
        };
        let (size, offsets) = match kind {
            RenderKind::Box { desired, .. }
            | RenderKind::Shape { desired, .. }
            | RenderKind::CustomPaint { desired, .. } => {
                (constraints.constrain(desired), Vec::new())
            }
            RenderKind::Decorated { desired, .. } => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, constraints.loosen());
                    let child_size = self.renders.get(child.0).expect("live").size;
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
            RenderKind::Button { desired, .. } => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, constraints.loosen());
                    let child_size = self.renders.get(child.0).expect("live").size;
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
                    self.layout_render(child, child_constraints);
                    let s = self.renders.get(child.0).expect("live").size;
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
                    self.layout_render(child, child_constraints);
                    let size = self.renders.get(child.0).expect("live").size;
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
                    self.layout_render(child, child_constraints);
                    let size = self.renders.get(child.0).expect("live").size;
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
                    self.layout_render(child, child_constraints);
                    let size = self.renders.get(child.0).expect("live").size;
                    (constraints.constrain(size), vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::Unconstrained { constrained_axis } => {
                let child_constraints = unconstrained_constraints(constraints, constrained_axis);
                if let Some(&child) = children.first() {
                    self.layout_render(child, child_constraints);
                    let size = self.renders.get(child.0).expect("live").size;
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
                    self.layout_render(child, child_constraints);
                    let size = self.renders.get(child.0).expect("live").size;
                    (constraints.constrain(size), vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::Baseline { baseline } => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, constraints.loosen());
                    let child = self.renders.get(child.0).expect("live");
                    let result = incular_layout::layout_baseline(
                        constraints,
                        incular_layout::BaselineChild {
                            size: child.size,
                            baseline: child.baseline,
                        },
                        baseline,
                    );
                    self.renders.get_mut(id.0).expect("live").baseline = Some(baseline);
                    (result.size, vec![result.children[0].offset])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::RepaintBoundary | RenderKind::Gesture => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, constraints.loosen());
                    let size = constraints.constrain(self.renders.get(child.0).expect("live").size);
                    (size, vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::PersistentHeader { .. } => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, constraints.loosen());
                    let size = constraints.constrain(self.renders.get(child.0).expect("live").size);
                    (size, vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
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
                    self.materialize_sliver_children(id, &config, &sliver_layout);
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
                            child.layer,
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
            RenderKind::Align {
                alignment,
                width_factor,
                height_factor,
            } => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, constraints.loosen());
                    let child_size = self.renders.get(child.0).expect("live").size;
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
                    Axis::Horizontal => constraints.max_height,
                    Axis::Vertical => constraints.max_width,
                };
                let main_max = match flex.direction {
                    Axis::Horizontal => constraints.max_width,
                    Axis::Vertical => constraints.max_height,
                };
                let loose = match flex.direction {
                    Axis::Horizontal => Constraints::new(0., f32::INFINITY, 0., cross_max),
                    Axis::Vertical => Constraints::new(0., cross_max, 0., f32::INFINITY),
                };
                let flex_meta = children
                    .iter()
                    .map(|child| {
                        let render_node = self.renders.get(child.0).expect("live");
                        match &render_node.kind {
                            RenderKind::Flexible { flex: f, fit } => (*f, *fit),
                            _ => (0, FlexFit::Loose),
                        }
                    })
                    .collect::<Vec<_>>();

                let mut flex_children = Vec::with_capacity(children.len());
                let mut occupied_non_flex = 0.0;
                for (child, (f, fit)) in children.iter().zip(&flex_meta) {
                    if *f == 0 || !main_max.is_finite() {
                        self.layout_render(*child, loose);
                        let size = self.renders.get(child.0).expect("live").size;
                        occupied_non_flex += flex.direction.main_extent(size);
                        flex_children.push(incular_layout::FlexChild::new(size));
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
                            self.layout_render(*child, child_constraints);
                            let size = self.renders.get(child.0).expect("live").size;
                            flex_children[i] = incular_layout::FlexChild::flexible(size, *f, *fit);
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
                    self.layout_render(child, constraints);
                    let size = self.renders.get(child.0).expect("live").size;
                    (constraints.constrain(size), vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::Wrap { wrap } => {
                let child_constraints = constraints.loosen();
                for child in &children {
                    self.layout_render(*child, child_constraints);
                }
                let wrap_children = children
                    .iter()
                    .map(|child| {
                        incular_layout::WrapChild::new(
                            self.renders.get(child.0).expect("live").size,
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
                for child in &children {
                    self.layout_render(*child, constraints.loosen());
                }
                let cells = children
                    .iter()
                    .map(|child| {
                        incular_layout::TableChild::new(
                            self.renders.get(child.0).expect("live").size,
                        )
                    })
                    .collect::<Vec<_>>();
                let config = incular_layout::Table {
                    columns,
                    column_spacing,
                    row_spacing,
                    alignment: Alignment::TOP_LEFT,
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
                for child in &children {
                    let render_node = self.renders.get(child.0).expect("live");
                    if !matches!(render_node.kind, RenderKind::Positioned { .. }) {
                        self.layout_render(*child, non_positioned_constraints);
                        let size = self.renders.get(child.0).expect("live").size;
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
                for child in &children {
                    let render_node = self.renders.get(child.0).expect("live");
                    if let RenderKind::Positioned {
                        left,
                        top,
                        right,
                        bottom,
                        width,
                        height,
                    } = render_node.kind
                    {
                        let pos = incular_layout::Positioned {
                            left,
                            top,
                            right,
                            bottom,
                            width,
                            height,
                        };
                        let child_w = width.or_else(|| {
                            left.zip(right)
                                .map(|(l, r)| (stack_size.width - l - r).max(0.0))
                        });
                        let child_h = height.or_else(|| {
                            top.zip(bottom)
                                .map(|(t, b)| (stack_size.height - t - b).max(0.0))
                        });
                        let child_constraints = Constraints::new(
                            0.0,
                            child_w.unwrap_or(stack_size.width),
                            0.0,
                            child_h.unwrap_or(stack_size.height),
                        );
                        self.layout_render(*child, child_constraints);
                        let size = self.renders.get(child.0).expect("live").size;
                        stack_children.push(incular_layout::StackChild::positioned(size, pos));
                    } else {
                        let size = self.renders.get(child.0).expect("live").size;
                        stack_children.push(incular_layout::StackChild::new(size));
                    }
                }

                let result = incular_layout::layout_stack(constraints, &stack_children, stack);
                (
                    result.size,
                    result.children.into_iter().map(|c| c.offset).collect(),
                )
            }
            RenderKind::IndexedStack { alignment, .. } => {
                let mut natural = Size::ZERO;
                for child in &children {
                    self.layout_render(*child, constraints.loosen());
                    let child_size = self.renders.get(child.0).expect("live").size;
                    natural = Size::new(
                        natural.width.max(child_size.width),
                        natural.height.max(child_size.height),
                    );
                }
                let size = constraints.constrain(natural);
                let offsets = children
                    .iter()
                    .map(|child| {
                        alignment.within(size, self.renders.get(child.0).expect("live").size)
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
                maintain_bottom_view_padding: _,
            } => {
                let ambient = self.environment.safe_insets.normalized();
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
                        ambient.bottom.max(minimum.bottom)
                    } else {
                        minimum.bottom
                    },
                );
                let child_constraints = constraints.deflate(insets.horizontal(), insets.vertical());
                if let Some(&child) = children.first() {
                    self.layout_render(child, child_constraints);
                    let child_size = self.renders.get(child.0).expect("live").size;
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
                    self.layout_render(child, constraints);
                    let size = self.renders.get(child.0).expect("live").size;
                    (constraints.constrain(size), vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::LayoutBuilder => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, constraints);
                    let size = self.renders.get(child.0).expect("live").size;
                    (constraints.constrain(size), vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::Visibility { visible } => {
                if visible {
                    if let Some(&child) = children.first() {
                        self.layout_render(child, constraints.loosen());
                        let size =
                            constraints.constrain(self.renders.get(child.0).expect("live").size);
                        (size, vec![Offset::ZERO])
                    } else {
                        (constraints.constrain(Size::ZERO), Vec::new())
                    }
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::AspectRatio { ratio } => {
                let mut size = if constraints.is_width_bounded() {
                    Size::new(constraints.max_width, constraints.max_width / ratio)
                } else if constraints.is_height_bounded() {
                    Size::new(constraints.max_height * ratio, constraints.max_height)
                } else {
                    Size::ZERO
                };
                if size.height > constraints.max_height {
                    size = Size::new(constraints.max_height * ratio, constraints.max_height);
                }
                size = constraints.constrain(size);
                if let Some(&child) = children.first() {
                    self.layout_render(child, Constraints::tight(size));
                    (size, vec![Offset::ZERO])
                } else {
                    (size, Vec::new())
                }
            }
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
                let node = self.renders.get_mut(id.0).expect("live");
                node.text_layout = Some(layout.clone());
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
                let node = self.renders.get_mut(id.0).expect("live");
                node.text_layout = Some(layout.clone());
                node.baseline = Some(layout.metrics.baseline);
                (size, Vec::new())
            }
            RenderKind::SelectionArea => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, constraints);
                    let size = constraints.constrain(self.renders.get(child.0).expect("live").size);
                    (size, vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
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
                let node = self.renders.get_mut(id.0).expect("live");
                node.text_layout = Some(layout.clone());
                node.text_revision = revision;
                node.text_visual_revision = visual_revision;
                node.baseline = Some(layout.metrics.baseline);
                (size, Vec::new())
            }
            RenderKind::Scroll {
                controller,
                axis,
                reverse: _,
                physics,
            } => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, scroll_constraints(axis, constraints));
                    let content = self.renders.get(child.0).expect("live").size;
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
            RenderKind::Translate { .. } => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, constraints.loosen());
                    let size = constraints.constrain(self.renders.get(child.0).expect("live").size);
                    (size, vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::Transform { .. }
            | RenderKind::Scale { .. }
            | RenderKind::Rotation { .. } => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, constraints.loosen());
                    let size = constraints.constrain(self.renders.get(child.0).expect("live").size);
                    (size, vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::FittedBox { .. } => {
                if let Some(&child) = children.first() {
                    self.layout_render(
                        child,
                        Constraints::new(0., f32::INFINITY, 0., f32::INFINITY),
                    );
                    let child_size = self.renders.get(child.0).expect("live").size;
                    (constraints.constrain(child_size), vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::Opacity { .. } => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, constraints.loosen());
                    let size = constraints.constrain(self.renders.get(child.0).expect("live").size);
                    (size, vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::Blur { .. }
            | RenderKind::DropShadow { .. }
            | RenderKind::ColorFiltered { .. }
            | RenderKind::Blend { .. } => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, constraints.loosen());
                    let size = constraints.constrain(self.renders.get(child.0).expect("live").size);
                    (size, vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
        };
        for (child, offset) in children.into_iter().zip(offsets) {
            // The parent computes this placement after the child has completed
            // its own layout. Update the retained placement layer here rather
            // than waiting for a later child layout pass (which may not occur).
            let child = self.renders.get_mut(child.0).expect("live");
            child.offset = offset;
            self.compositor
                .update_transform(child.layer, CoreTransform::translation(offset));
        }
        let node = self.renders.get_mut(id.0).expect("live");
        node.size = size;
        node.constraints = Some(constraints);
        node.dirty.remove(DirtyFlags::LAYOUT);
        node.dirty.insert(DirtyFlags::PAINT);
        self.compositor
            .update_transform(node.layer, CoreTransform::translation(node.offset));
        if let Some(clip) = node.clip_layer {
            self.compositor
                .update_clip(clip, Rect::from_origin_size(Offset::ZERO, node.size));
        }
        // Static affine wrappers and FittedBox receive their initial retained
        // transform during layout. Later controller changes are handled by
        // `update_compositor` without revisiting this path.
        let (content_layer, transform) = {
            let node = self.renders.get(id.0).expect("live");
            (node.content_layer, self.content_transform(id))
        };
        if let (Some(content), Some(transform)) = (content_layer, transform) {
            self.compositor.update_transform(content, transform);
        }
        self.diagnostics.layouts += 1;
        #[cfg(feature = "devtools")]
        if let Some(element) = self.element_for_render(id) {
            if let Some(element) = self.elements.get_mut(element.0) {
                element.dev.layouts += 1;
                if let Some((old_constraints, old_size)) = old_layout
                    && (old_constraints != Some(constraints) || old_size != size)
                {
                    if old_constraints != Some(constraints) {
                        element
                            .dev
                            .layout_reason
                            .get_or_insert_with(|| "incoming constraints changed".into());
                    }
                    element
                        .dev
                        .paint_reason
                        .get_or_insert_with(|| "layout result invalidated paint".into());
                    const MAX_LAYOUT_HISTORY: usize = 64;
                    if element.dev.layout_history.len() == MAX_LAYOUT_HISTORY {
                        element.dev.layout_history.remove(0);
                    }
                    element.dev.layout_history.push(LayoutHistoryRecord {
                        sequence: element.dev.layouts,
                        old_constraints,
                        new_constraints: constraints,
                        old_size,
                        new_size: size,
                        cause: element
                            .dev
                            .last_cause
                            .as_ref()
                            .map(InvalidationCause::summary),
                    });
                }
            }
        }
        #[cfg(feature = "devtools")]
        self.devtools_trace_end(trace);
    }
}
