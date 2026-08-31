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
    /// Compatibility layout entry point. Recoverable application-authored
    /// errors are recorded in [`Self::last_tree_error`] instead of panicking.
    /// Runtime/frame code should prefer [`Self::try_layout`].
    pub fn layout(&mut self, constraints: Constraints) {
        let _ = self.try_layout(constraints);
    }

    pub fn try_layout(&mut self, constraints: Constraints) -> Result<(), TreeError> {
        self.pending_tree_error = None;
        self.last_tree_error = None;
        let _phase_guard = self.guard_phase_root(FramePhase::Layout);
        self.refresh_text_fields();
        self.refresh_sliver_ranges();
        self.refresh_stateful_layout_builders();
        if let Some(root) = self.root.and_then(|id| self.render_id(id)) {
            self.layout_render(root, constraints);
        }
        self.refresh_selection_states();
        self.refresh_notification_listeners();
        if let Some(error) = self.pending_tree_error.take() {
            self.last_tree_error = Some(error.clone());
            return Err(error);
        }
        Ok(())
    }

    pub(super) fn record_tree_error(&mut self, error: TreeError) {
        if self.pending_tree_error.is_none() {
            self.pending_tree_error = Some(error);
        }
    }

    pub(super) fn record_tree_result(&mut self, result: Result<(), TreeError>) -> bool {
        match result {
            Ok(()) => true,
            Err(error) => {
                self.record_tree_error(error);
                false
            }
        }
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
                let RenderKind::TextField { controller, .. } = &render.object.kind else {
                    return None;
                };
                let (content, visual) = controller.revisions();
                let state = render
                    .text_field_state()
                    .expect("text-field render must own text-field state");
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
    fn prepare_advanced_children(&mut self, id: RenderObjectId, constraints: Constraints) -> bool {
        let Some(element_id) = self.element_for_render(id) else {
            return true;
        };
        let kind = self
            .renders
            .get(id.0)
            .expect("advanced scrolling render")
            .object
            .kind
            .clone();
        let viewport_size = advanced_viewport_size(constraints);
        match kind {
            RenderKind::RawScrollbar { .. } => {
                let child = self.elements.get(element_id.0).and_then(|element| {
                    let WidgetKind::RawScrollbar { child, .. } = &element.widget.kind else {
                        return None;
                    };
                    Some(child.as_ref().clone())
                });
                if let Some(child) = child {
                    let result = self.materialize_advanced_children(
                        element_id,
                        vec![(AdvancedChildKey::RawScrollbar, child)],
                    );
                    if !self.record_tree_result(result) {
                        return false;
                    }
                }
            }
            RenderKind::ListWheelScrollView { view } => {
                let layout = view
                    .0
                    .borrow_mut()
                    .viewport_mut()
                    .layout_with_measure(viewport_size, |_, child_constraints| {
                        child_constraints.biggest()
                    });
                let desired = layout
                    .children
                    .iter()
                    .map(|child| (AdvancedChildKey::Wheel(child.index), child.child.clone()))
                    .collect();
                let result = self.materialize_advanced_children(element_id, desired);
                if !self.record_tree_result(result) {
                    return false;
                }
                self.renders
                    .get_mut(id.0)
                    .expect("wheel render")
                    .wheel_state_mut()
                    .expect("wheel render must own wheel state")
                    .layout = Some(layout);
            }
            RenderKind::ListWheelViewport { viewport } => {
                let layout = viewport
                    .0
                    .borrow_mut()
                    .layout_with_measure(viewport_size, |_, child_constraints| {
                        child_constraints.biggest()
                    });
                let desired = layout
                    .children
                    .iter()
                    .map(|child| (AdvancedChildKey::Wheel(child.index), child.child.clone()))
                    .collect();
                let result = self.materialize_advanced_children(element_id, desired);
                if !self.record_tree_result(result) {
                    return false;
                }
                self.renders
                    .get_mut(id.0)
                    .expect("wheel render")
                    .wheel_state_mut()
                    .expect("wheel render must own wheel state")
                    .layout = Some(layout);
            }
            RenderKind::DraggableScrollableSheet { sheet } => {
                let state = if let Some(state) = self
                    .renders
                    .get(id.0)
                    .and_then(|render| render.draggable_sheet_state())
                    .and_then(|state| state.state.clone())
                {
                    state
                } else {
                    let (state, child) = sheet.0.borrow().mount();
                    self.renders
                        .get_mut(id.0)
                        .expect("draggable sheet render")
                        .draggable_sheet_state_mut()
                        .expect("draggable-sheet render must own draggable state")
                        .state = Some(state.clone());
                    let result = self.materialize_advanced_children(
                        element_id,
                        vec![(AdvancedChildKey::Sheet, child)],
                    );
                    if !self.record_tree_result(result) {
                        return false;
                    }
                    state
                };
                state.set_parent_height(viewport_size.height);
                state.set_parent_controllers(self.ancestor_scroll_controllers(element_id));
                if let Some(actuator) = self.nearest_draggable_actuator(element_id) {
                    state.attach_actuator(&actuator);
                }
            }
            RenderKind::DraggableScrollableActuator { .. } => {
                let child = self.elements.get(element_id.0).and_then(|element| {
                    let WidgetKind::DraggableScrollableActuator { child, .. } =
                        &element.widget.kind
                    else {
                        return None;
                    };
                    Some(child.as_ref().clone())
                });
                if let Some(child) = child {
                    let result = self.materialize_advanced_children(
                        element_id,
                        vec![(AdvancedChildKey::Actuator, child)],
                    );
                    if !self.record_tree_result(result) {
                        return false;
                    }
                }
            }
            RenderKind::TwoDimensionalScrollView { view } => {
                let layout = view
                    .0
                    .borrow_mut()
                    .viewport_mut()
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
                let result = self.materialize_advanced_children(element_id, desired);
                if !self.record_tree_result(result) {
                    return false;
                }
                self.renders
                    .get_mut(id.0)
                    .expect("two-dimensional render")
                    .two_dimensional_state_mut()
                    .expect("two-dimensional render must own viewport state")
                    .layout = Some(layout);
            }
            RenderKind::TwoDimensionalViewport { viewport } => {
                let layout = viewport
                    .0
                    .borrow_mut()
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
                let result = self.materialize_advanced_children(element_id, desired);
                if !self.record_tree_result(result) {
                    return false;
                }
                self.renders
                    .get_mut(id.0)
                    .expect("two-dimensional render")
                    .two_dimensional_state_mut()
                    .expect("two-dimensional render must own viewport state")
                    .layout = Some(layout);
            }
            _ => {}
        }
        true
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
            if let WidgetKind::DraggableScrollableActuator { actuator, .. } = &element.widget.kind {
                return Some(actuator.0.as_ref().clone());
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
        let node = self.renders.get(id.0).expect("live render");
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
        let node = self.renders.get(id.0).expect("live follower");
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
            let node = self.renders.get(id.0).expect("live render");
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
            let node = self.renders.get(id.0).expect("live render");
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
        with_recursive_tree_stack(|| {
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
            self.renders.get(id.0).expect("live").object.kind,
            RenderKind::LayoutBuilder
        ) {
            let result = self.materialize_layout_builder(id, constraints);
            if !self.record_tree_result(result) {
                return;
            }
        }
        if self.renders.get(id.0).is_some_and(|render| {
            matches!(
                render.object.kind,
                RenderKind::RawScrollbar { .. }
                    | RenderKind::ListWheelScrollView { .. }
                    | RenderKind::ListWheelViewport { .. }
                    | RenderKind::DraggableScrollableSheet { .. }
                    | RenderKind::DraggableScrollableActuator { .. }
                    | RenderKind::TwoDimensionalScrollView { .. }
                    | RenderKind::TwoDimensionalViewport { .. }
            )
        }) && !self.prepare_advanced_children(id, constraints)
        {
            return;
        }
        let (kind, children) = {
            let n = self.renders.get(id.0).expect("live");
            (n.object.kind.clone(), n.children.clone())
        };
        let (size, offsets) = self.layout_kind(id, kind, &children, constraints);
        for (child, offset) in children.into_iter().zip(offsets) {
            // The parent computes this placement after the child has completed
            // its own layout. Update the retained placement layer here rather
            // than waiting for a later child layout pass (which may not occur).
            let child = self.renders.get_mut(child.0).expect("live");
            child.offset = offset;
            self.compositor
                .update_transform(child.object.layers.root, CoreTransform::translation(offset));
        }
        let node = self.renders.get_mut(id.0).expect("live");
        node.size = size;
        node.constraints = Some(constraints);
        node.dirty.remove(DirtyFlags::LAYOUT);
        node.dirty.insert(DirtyFlags::PAINT);
        self.compositor.update_transform(
            node.object.layers.root,
            CoreTransform::translation(node.offset),
        );
        // Static affine wrappers, clips, and geometry-sensitive effect layers
        // receive their initial retained geometry during layout. Later
        // controller changes are handled by `update_compositor`.
        let (layers, kind, transform) = {
            let node = self.renders.get(id.0).expect("live");
            (
                node.object.layers.clone(),
                node.object.kind.clone(),
                self.content_transform(id),
            )
        };
        layers.update_layout_geometry(&mut self.compositor, &kind, size, transform);
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
