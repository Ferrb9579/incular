//! Constraint propagation, retained layout, transforms, and viewport geometry.

use super::*;

mod behavior;

use super::rendering::{fitted_transform, transform_around, transform_around_alignment};
use super::text::text_field_display;

impl WidgetTree {
    pub fn mark_paint(&mut self, id: ElementId) -> Result<(), TreeError> {
        let render = self.render_id(id).ok_or(TreeError::MissingElement(id))?;
        self.mark_render_dirty(render, DirtyFlags::PAINT, false);
        Ok(())
    }
    pub fn layout(&mut self, constraints: Constraints) -> Result<(), TreeError> {
        let _phase_guard = self.guard_phase_root(FramePhase::Layout);
        // Runtime-owned environment fields and inherited scopes share the same
        // exact dependency tracker. Drain those invalidations before consulting
        // retained layout caches so an environment-dependent LayoutBuilder can
        // rematerialize without forcing unrelated subtree rebuilds.
        self.apply_inherited_invalidations();
        self.refresh_text_fields();
        self.refresh_sliver_ranges();
        self.refresh_stateful_layout_builders();
        if let Some(root) = self.root.and_then(|id| self.render_id(id)) {
            self.layout_render(root, constraints)?;
        }
        self.resolve_transient_placements(constraints)?;
        self.refresh_selection_states();
        self.refresh_notification_listeners();
        Ok(())
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
                } = &element.widget.kind()
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
                let RenderKind::TextField { controller, .. } = &render.object.kind else {
                    return None;
                };
                let (content, visual) = controller.revisions();
                let state = render.text_field_state()?;
                ((content != state.content_revision) || (visual != state.visual_revision))
                    .then_some(RenderObjectId(raw))
            })
            .collect::<Vec<_>>();
        for render in pending {
            // Text width can change the size seen by an unconstrained parent.
            self.mark_render_dirty(render, DirtyFlags::LAYOUT | DirtyFlags::PAINT, true);
        }
    }

    /// Materializes the child window owned by an advanced scrolling model
    /// before the normal retained layout pass snapshots its children.
    fn prepare_advanced_children(
        &mut self,
        id: RenderObjectId,
        constraints: Constraints,
    ) -> Result<(), TreeError> {
        let Some(element_id) = self.element_for_render(id) else {
            return Ok(());
        };
        let kind = self
            .render_live(id, "advanced scrolling render must remain live")
            .object
            .kind
            .clone();
        let viewport_size = advanced_viewport_size(constraints);
        match kind {
            RenderKind::ListWheelScrollView { .. } | RenderKind::ListWheelViewport { .. } => {
                self.prepare_wheel_children(id, element_id, viewport_size)?;
            }
            RenderKind::DraggableScrollableSheet { config } => {
                let retained = self
                    .renders
                    .get(id.0)
                    .and_then(|render| render.draggable_sheet_state())
                    .map(|state| (state.state.clone(), state.mounted_config.clone()));
                let state = if let Some((Some(state), mounted_config)) = retained {
                    let builder_changed = mounted_config
                        .as_ref()
                        .is_none_or(|mounted| !mounted.builder_ptr_eq(&config));
                    if builder_changed {
                        let child = config.build_child(&state);
                        self.reconcile_advanced_children(
                            element_id,
                            vec![(AdvancedChildKey::Sheet, child)],
                        )?;
                    }
                    config.update_mounted_state(&state, mounted_config.as_deref());
                    self.draggable_sheet_state_live_mut(id).mounted_config = Some(config.clone());
                    state
                } else {
                    let (state, child) = config.mount();
                    self.reconcile_advanced_children(
                        element_id,
                        vec![(AdvancedChildKey::Sheet, child)],
                    )?;
                    let retained = self.draggable_sheet_state_live_mut(id);
                    retained.state = Some(state.clone());
                    retained.mounted_config = Some(config.clone());
                    state
                };
                state.set_parent_height(viewport_size.height);
                state.set_parent_controllers(self.ancestor_scroll_controllers(element_id));
                if let Some(actuator) = self.nearest_draggable_actuator(element_id) {
                    state.attach_actuator(&actuator);
                }
            }
            RenderKind::TwoDimensionalScrollView { .. }
            | RenderKind::TwoDimensionalViewport { .. } => {
                self.prepare_two_dimensional_children(id, element_id, viewport_size)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn prepare_wheel_children(
        &mut self,
        id: RenderObjectId,
        element_id: ElementId,
        viewport_size: Size,
    ) -> Result<(), TreeError> {
        let layout = self
            .wheel_state_live_mut(id)
            .viewport
            .as_mut()
            .expect("wheel render must own retained viewport")
            .layout_with_measure(viewport_size, |_, child_constraints| {
                child_constraints.biggest()
            });
        let desired = layout
            .children
            .iter()
            .map(|child| (AdvancedChildKey::Wheel(child.index), child.child.clone()))
            .collect();
        self.reconcile_advanced_children(element_id, desired)?;
        self.wheel_state_live_mut(id).layout = Some(layout);
        Ok(())
    }

    fn prepare_two_dimensional_children(
        &mut self,
        id: RenderObjectId,
        element_id: ElementId,
        viewport_size: Size,
    ) -> Result<(), TreeError> {
        let layout = self
            .two_dimensional_state_live_mut(id)
            .viewport
            .as_mut()
            .expect("two-dimensional render must own retained viewport")
            .layout_with_measure(viewport_size, |_, child_constraints| {
                child_constraints.biggest()
            });
        let desired = layout
            .children
            .iter()
            .map(|child| {
                (
                    AdvancedChildKey::TwoDimensional(child.vicinity),
                    child.child.clone(),
                )
            })
            .collect();
        self.reconcile_advanced_children(element_id, desired)?;
        self.two_dimensional_state_live_mut(id).layout = Some(layout);
        Ok(())
    }

    fn ancestor_scroll_controllers(&self, element_id: ElementId) -> Vec<ScrollController> {
        let mut controllers = Vec::new();
        let mut parent = self
            .elements
            .get(element_id.0)
            .and_then(|element| element.parent);
        while let Some(candidate) = parent {
            if let Some(controller) = self.scroll_controller_for_element(candidate)
                && !controllers.contains(&controller)
            {
                controllers.push(controller);
            }
            parent = self
                .elements
                .get(candidate.0)
                .and_then(|element| element.parent);
        }
        controllers
    }

    fn nearest_draggable_actuator(
        &self,
        element_id: ElementId,
    ) -> Option<DraggableScrollableActuator> {
        let mut parent = self
            .elements
            .get(element_id.0)
            .and_then(|element| element.parent);
        while let Some(candidate) = parent {
            let element = self.elements.get(candidate.0)?;
            if let WidgetKind::DraggableScrollableActuator { actuator, .. } = &element.widget.kind()
            {
                return Some(actuator.clone());
            }
            parent = element.parent;
        }
        None
    }

    #[doc(hidden)]
    pub fn content_transform(&self, id: RenderObjectId) -> Option<CoreTransform> {
        let node = self.renders.get(id.0)?;
        match &node.object.kind {
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
        let node = self.render_live(id, "child transform render must remain live");
        match &node.object.kind {
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
            RenderKind::Follower { .. } => self.follower_content_transform(id),
            _ => self
                .content_transform(id)
                .unwrap_or(CoreTransform::IDENTITY),
        }
    }
    pub(super) fn follower_content_transform(&self, id: RenderObjectId) -> CoreTransform {
        let node = self.render_live(id, "follower render must remain live");
        let RenderKind::Follower {
            link,
            show_when_unlinked,
            offset,
            target_anchor,
            follower_anchor,
        } = &node.object.kind
        else {
            return CoreTransform::IDENTITY;
        };
        let base = self.render_world_transform(id);
        let Some(leader_transform) = link.leader_transform() else {
            // An unlinked follower remains at its normal layout placement when
            // requested. A hidden follower is culled by the compositor; the
            // identity here keeps hit testing and diagnostics deterministic.
            let _ = show_when_unlinked;
            return CoreTransform::IDENTITY;
        };
        let leader_size = link.leader_size().unwrap_or(Size::ZERO);
        let target = leader_transform.transform_point(target_anchor.along_size(leader_size));
        let leader_origin = leader_transform.transform_point(Offset::ZERO);
        let offset_world = leader_transform.transform_point(*offset) - leader_origin;
        let Some(target_in_parent) = base.inverse_transform_point(target + offset_world) else {
            return CoreTransform::IDENTITY;
        };
        CoreTransform::translation(target_in_parent - follower_anchor.along_size(node.size))
    }
    pub(super) fn render_world_transform(&self, id: RenderObjectId) -> CoreTransform {
        let mut path = Vec::new();
        let mut cursor = Some(id);
        while let Some(current) = cursor {
            path.push(current);
            cursor = self
                .render_live(current, "world-transform ancestor must remain live")
                .parent;
        }
        path.reverse();
        let mut world = CoreTransform::IDENTITY;
        for (index, current) in path.iter().enumerate() {
            let node = self.render_live(*current, "world-transform path render must remain live");
            world = world.then(CoreTransform::translation(node.offset));
            if index + 1 != path.len() {
                world = world.then(self.child_content_transform(*current));
            }
        }
        world
    }
    pub(super) fn semantic_bounds(&self, id: RenderObjectId, size: Size) -> Rect {
        let mut world = self.render_world_transform(id);
        if self.content_transform(id).is_some()
            || matches!(
                self.renders.get(id.0).map(|node| &node.object.kind),
                Some(RenderKind::Follower { .. })
            )
        {
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
            let node = self.render_live(id, "render-origin path must remain live");
            origin = origin + node.offset;
            match &node.object.kind {
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
            let node = self.render_live(id, "viewport-origin path must remain live");
            origin = origin + node.offset;
            if !is_self {
                match &node.object.kind {
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
            .filter_map(|(raw, node)| match &node.object.kind {
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
            } = &node.object.kind
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
            let node = self.render_live_mut(render, "dirty propagation render must remain live");
            node.dirty.insert(flags);
            current = if propagate_layout && flags.contains(DirtyFlags::LAYOUT) {
                node.parent
            } else {
                None
            };
        }
    }
    pub(super) fn layout_render(
        &mut self,
        id: RenderObjectId,
        constraints: Constraints,
    ) -> Result<(), TreeError> {
        with_recursive_tree_stack(|| self.layout_render_inner(id, constraints))
    }

    pub(super) fn layout_render_inner(
        &mut self,
        id: RenderObjectId,
        constraints: Constraints,
    ) -> Result<(), TreeError> {
        let _node_guard = self.guard_render(FramePhase::Layout, id, Some(constraints));
        let needs = self
            .render_live(id, "layout render must remain live")
            .dirty
            .contains(DirtyFlags::LAYOUT)
            || self
                .render_live(id, "retained render must remain live")
                .constraints
                != Some(constraints);
        if !needs {
            self.diagnostics.layout_cache_hits += 1;
            return Ok(());
        }
        #[cfg(feature = "devtools")]
        let trace = self
            .element_for_render(id)
            .and_then(|element| self.devtools_trace_begin_element(element, TracePhase::Layout));
        #[cfg(feature = "devtools")]
        let old_layout = self.deep_trace.as_ref().map(|_| {
            let render = self.render_live(id, "retained render must remain live");
            (render.constraints, render.size)
        });
        if matches!(
            self.render_live(id, "retained render must remain live")
                .object
                .kind,
            RenderKind::LayoutBuilder
        ) {
            self.materialize_layout_builder(id, constraints)?;
        }
        if self.renders.get(id.0).is_some_and(|render| {
            matches!(
                render.object.kind,
                RenderKind::ListWheelScrollView { .. }
                    | RenderKind::ListWheelViewport { .. }
                    | RenderKind::DraggableScrollableSheet { .. }
                    | RenderKind::TwoDimensionalScrollView { .. }
                    | RenderKind::TwoDimensionalViewport { .. }
            )
        }) {
            self.prepare_advanced_children(id, constraints)?;
        }
        let (kind, children) = {
            let n = self.render_live(id, "retained render must remain live");
            (n.object.kind.clone(), n.children.clone())
        };
        let (size, offsets) = self.layout_kind(id, kind, &children, constraints)?;
        for (child, offset) in children.into_iter().zip(offsets) {
            // The parent computes this placement after the child has completed
            // its own layout. Update the retained placement layer here rather
            // than waiting for a later child layout pass (which may not occur).
            let layer = {
                let child = self.render_live_mut(child, "retained render must remain live");
                child.offset = offset;
                child.object.layers.root
            };
            self.compositor
                .update_transform(layer, CoreTransform::translation(offset));
        }
        let (layer, node_offset) = {
            let node = self.render_live_mut(id, "retained render must remain live");
            node.size = size;
            node.constraints = Some(constraints);
            node.dirty.remove(DirtyFlags::LAYOUT);
            node.dirty.insert(DirtyFlags::PAINT);
            (node.object.layers.root, node.offset)
        };
        self.compositor
            .update_transform(layer, CoreTransform::translation(node_offset));
        // Static affine wrappers, clips, and geometry-sensitive effect layers
        // receive their initial retained geometry during layout. Later
        // controller changes are handled by `update_compositor`.
        let (layers, kind, transform) = {
            let node = self.render_live(id, "retained render must remain live");
            (
                node.object.layers.clone(),
                node.object.kind.clone(),
                self.content_transform(id),
            )
        };
        layers.update_layout_geometry(&mut self.compositor, &kind, size, transform);
        self.diagnostics.layouts += 1;
        #[cfg(feature = "devtools")]
        if let Some(element) = self.element_for_render(id)
            && let Some(element) = self.elements.get_mut(element.0)
        {
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
        #[cfg(feature = "devtools")]
        self.devtools_trace_end(trace);
        Ok(())
    }
}

fn advanced_viewport_size(constraints: Constraints) -> Size {
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
    constraints.constrain(Size::new(width, height))
}
