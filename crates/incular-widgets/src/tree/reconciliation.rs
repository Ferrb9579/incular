//! Element mounting, reconciliation, lazy materialization, and retained lifecycle.

use super::*;

use super::rendering::{carry_replaced_transition, render_kind};
use super::widget::WidgetChildren;

struct DesiredDynamicChild<K> {
    key: K,
    widget: Widget,
}

struct DynamicChildResult<K> {
    keys: Vec<K>,
    children: Vec<ElementId>,
}

impl WidgetTree {
    fn inherited_contexts_for(
        &self,
        parent: Option<ElementId>,
        boundary: bool,
        scope: Option<&InheritedScopeValue>,
    ) -> (DependencyContext, DependencyContext) {
        self.inherited_contexts_for_consumers(
            parent,
            boundary,
            scope,
            ConsumerId::new(),
            ConsumerId::new(),
        )
    }

    fn inherited_contexts_for_consumers(
        &self,
        parent: Option<ElementId>,
        boundary: bool,
        scope: Option<&InheritedScopeValue>,
        build_consumer: ConsumerId,
        render_consumer: ConsumerId,
    ) -> (DependencyContext, DependencyContext) {
        let (parent_build, parent_render) =
            parent.and_then(|id| self.elements.get(id.0)).map_or_else(
                || (self.dependency_root.clone(), self.dependency_root.clone()),
                |element| {
                    (
                        element.build_context.clone(),
                        element.render_context.clone(),
                    )
                },
            );
        let mut build = if boundary {
            parent_build.boundary_for_consumer(build_consumer)
        } else {
            parent_build.for_related_consumer(build_consumer)
        };
        let mut render = if boundary {
            parent_render.boundary_for_consumer(render_consumer)
        } else {
            parent_render.for_related_consumer(render_consumer)
        };
        if let Some(scope) = scope {
            build = build.provide_erased(scope.type_id, scope.value.clone());
            render = render.provide_erased(scope.type_id, scope.value.clone());
        }
        (build, render)
    }

    /// Rebinds contexts only when inherited topology itself changes (a lookup
    /// boundary is added/removed or a scope changes the type it provides).
    /// Ordinary value updates never walk the subtree; they invalidate exact
    /// subscribers through the retained dependency tracker.
    fn rebind_inherited_subtree(&mut self, root: ElementId) {
        let mut work = vec![root];
        while let Some(id) = work.pop() {
            let Some(element) = self.elements.get(id.0) else {
                continue;
            };
            let parent = element.parent;
            let boundary = element.environment_boundary;
            let scope = element.environment_override.clone();
            let build_consumer = element.build_context.consumer_id();
            let render_consumer = element.render_context.consumer_id();
            let old_build = element.build_context.clone();
            let old_render = element.render_context.clone();
            let widget = element.widget.clone();
            let render = element.render;
            let children = element.children.clone();
            old_build.clear_dependencies();
            old_render.clear_dependencies();
            let (build_context, render_context) = self.inherited_contexts_for_consumers(
                parent,
                boundary,
                scope.as_ref(),
                build_consumer,
                render_consumer,
            );
            let new_kind = render_context.build(|context| render_kind(&widget, context));
            let invalidation = self
                .render_live_mut(render, "rebound render must remain live")
                .object
                .update_kind(new_kind);
            if let Some(element) = self.elements.get_mut(id.0) {
                element.build_context = build_context;
                element.render_context = render_context;
                element.layout_builder_constraints = None;
                element.layout_builder_revision = 0;
            }
            if matches!(widget.kind(), WidgetKind::LayoutBuilder { .. })
                || invalidation.contains(Invalidation::LAYOUT)
            {
                self.mark_render_dirty(render, DirtyFlags::LAYOUT | DirtyFlags::PAINT, true);
            } else if invalidation.contains(Invalidation::PAINT) {
                self.mark_render_dirty(render, DirtyFlags::PAINT, false);
            }
            work.extend(children.into_iter().rev());
        }
    }

    pub fn update(&mut self, id: ElementId, widget: Widget) -> Result<(), TreeError> {
        if !self.elements.contains(id.0) {
            return Err(TreeError::MissingElement(id));
        }
        let result = self.update_existing(id, &widget);
        if result.is_ok() {
            self.refresh_notification_listeners();
        }
        result
    }

    pub(super) fn mount_element(
        &mut self,
        parent: Option<ElementId>,
        widget: Widget,
    ) -> Result<ElementId, TreeError> {
        // Mount is intentionally transactional for application-authored
        // configuration errors. Validate the complete immutable description
        // before allocating any retained elements, render nodes, layers, or
        // subscriptions.
        self.validate_widget_subtree(&widget, parent)?;
        enum MountWork {
            Create {
                parent: Option<ElementId>,
                widget: Box<Widget>,
            },
            Finish(ElementId),
        }

        let mut work = vec![MountWork::Create {
            parent,
            widget: Box::new(widget),
        }];
        let mut root = None;
        while let Some(next) = work.pop() {
            match next {
                MountWork::Create { parent, widget } => {
                    let children = widget
                        .children_refs()
                        .into_iter()
                        .cloned()
                        .collect::<Vec<_>>();
                    let id = self.mount_element_node(parent, *widget)?;
                    if root.is_none() {
                        root = Some(id);
                    }
                    if let Some(parent) = parent {
                        self.element_live_mut(parent, "mount parent must remain live")
                            .children
                            .push(id);
                    }
                    work.push(MountWork::Finish(id));
                    for child in children.into_iter().rev() {
                        work.push(MountWork::Create {
                            parent: Some(id),
                            widget: Box::new(child),
                        });
                    }
                }
                MountWork::Finish(id) => {
                    self.sync_render_children(id);
                    self.sync_raw_input_state(id);
                    if self
                        .elements
                        .get(id.0)
                        .is_some_and(|element| element.parent.is_none())
                    {
                        let layer = self
                            .render_for_live_element(id, "mounted root must own a live render")
                            .object
                            .layers
                            .root;
                        self.compositor.set_root(layer);
                    }
                }
            }
        }
        Ok(root.expect("mount work always contains the root widget"))
    }

    pub(super) fn mount_element_node(
        &mut self,
        parent: Option<ElementId>,
        mut widget: Widget,
    ) -> Result<ElementId, TreeError> {
        let handlers = &mut self.pending_handlers;
        let next = &mut self.next_action;
        widget.bind_callbacks(&mut |callback| {
            let action = ActionId(*next);
            *next += 1;
            handlers.push((action, callback));
            action
        });
        let (environment_override, environment_boundary) = match widget.kind() {
            WidgetKind::LayoutBuilder {
                environment,
                environment_boundary,
                ..
            } => (environment.clone(), *environment_boundary),
            _ => (None, false),
        };
        let platform_menu_binding = environment_override
            .as_ref()
            .and_then(|scope| {
                scope
                    .value
                    .downcast_ref::<crate::platform_widgets::PlatformMenuRetainedMarker>()
            })
            .map(|marker| marker.binding.clone());
        let (build_context, render_context) = self.inherited_contexts_for(
            parent,
            environment_boundary,
            environment_override.as_ref(),
        );
        self.check_keys_borrowed(None, widget.children_refs())?;
        let layers = RenderLayers::create(&mut self.compositor, widget.kind());
        let kind = render_context.build(|context| render_kind(&widget, context));
        let render = self.renders.insert(RenderNode {
            parent: None,
            children: Vec::new(),
            geometry: RenderGeometry {
                size: Size::ZERO,
                offset: Offset::ZERO,
                constraints: None,
                baseline: None,
            },
            dirty: DirtyFlags::LAYOUT | DirtyFlags::PAINT,
            object: RenderObjectPayload::new(kind, layers),
        });
        let id = ElementId(self.elements.insert(Element {
            parent,
            children: Vec::new(),
            widget: widget.clone(),
            render: RenderObjectId(render),
            dirty: DirtyFlags::NONE,
            sliver_child_ids: Vec::new(),
            sliver_child_semantic_indices: Vec::new(),
            sliver_overlay_ids: HashSet::new(),
            advanced_child_keys: Vec::new(),
            notification_subscriptions: Vec::new(),
            sliver_delegate_revision: 0,
            sliver_scroll_revision: 0,
            layout_builder_constraints: None,
            layout_builder_revision: 0,
            build_context: build_context.clone(),
            render_context: render_context.clone(),
            environment_override,
            environment_boundary,
            #[cfg(feature = "devtools")]
            dev: ElementDevData::default(),
        }));
        self.inherited_consumers.insert(
            build_context.consumer_id(),
            (id, InheritedDependencyKind::Build),
        );
        self.inherited_consumers.insert(
            render_context.consumer_id(),
            (id, InheritedDependencyKind::Render),
        );
        if let WidgetKind::SelectionListener { notifier, .. } = widget.kind() {
            notifier.register();
        }
        if let Some(binding) = platform_menu_binding {
            binding.install_if_bound();
        }
        self.diagnostics.mounts += 1;
        Ok(id)
    }

    pub(super) fn apply_inherited_invalidations(&mut self) {
        let dirty = self.dependency_root.take_dirty_consumers();
        for consumer in dirty {
            let Some(&(id, kind)) = self.inherited_consumers.get(&consumer) else {
                continue;
            };
            let Some(element) = self.elements.get(id.0) else {
                continue;
            };
            let render = element.render;
            match kind {
                InheritedDependencyKind::Build => {
                    if let Some(element) = self.elements.get_mut(id.0) {
                        element.layout_builder_constraints = None;
                        element.layout_builder_revision = 0;
                    }
                    self.mark_render_dirty(render, DirtyFlags::LAYOUT | DirtyFlags::PAINT, true);
                    // The drain is the content-change source: a build
                    // dependency changed value, so a rebuilt descendant may
                    // measure differently. Invalidate the enclosing sliver
                    // now; scrolling and measurement never enter this drain.
                    self.invalidate_enclosing_sliver_measurement(id);
                }
                InheritedDependencyKind::Render => {
                    let (widget, context) = {
                        let element = self.element_live(id, "inherited consumer must remain live");
                        (element.widget.clone(), element.render_context.clone())
                    };
                    let new_kind = context.build(|context| render_kind(&widget, context));
                    let invalidation = self
                        .render_live_mut(render, "inherited render must remain live")
                        .object
                        .update_kind(new_kind);
                    if invalidation.contains(Invalidation::LAYOUT) {
                        self.mark_render_dirty(
                            render,
                            DirtyFlags::LAYOUT | DirtyFlags::PAINT,
                            true,
                        );
                        // Layout-affecting render updates (a re-wrapped text
                        // run, for example) change intrinsic size without
                        // rebuilding any widget. Paint-only updates take no
                        // invalidation branch here by construction.
                        self.invalidate_enclosing_sliver_measurement(id);
                    } else if invalidation.contains(Invalidation::PAINT) {
                        self.mark_render_dirty(render, DirtyFlags::PAINT, false);
                    }
                }
            }
        }
    }

    pub(super) fn update_existing(
        &mut self,
        id: ElementId,
        widget: &Widget,
    ) -> Result<(), TreeError> {
        let mut widget = widget.clone();
        let platform_menu_reconciliation =
            self.prepare_platform_menu_reconciliation(id, &mut widget);
        let handlers = &mut self.pending_handlers;
        let next = &mut self.next_action;
        widget.bind_callbacks(&mut |callback| {
            let action = ActionId(*next);
            *next += 1;
            handlers.push((action, callback));
            action
        });
        let result = with_recursive_tree_stack(|| self.update_existing_inner(id, &widget));
        if result.is_ok()
            && let Some((retained, incoming)) = platform_menu_reconciliation
        {
            retained.reconcile_from(&incoming);
        }
        result
    }

    fn prepare_platform_menu_reconciliation(
        &self,
        id: ElementId,
        widget: &mut Widget,
    ) -> Option<(
        crate::platform_widgets::PlatformMenuBinding,
        crate::platform_widgets::PlatformMenuBinding,
    )> {
        let old_scope = self
            .elements
            .get(id.0)
            .and_then(|element| element.environment_override.clone())?;
        let old_marker = old_scope
            .value
            .downcast_ref::<crate::platform_widgets::PlatformMenuRetainedMarker>()?;
        let new_marker =
            widget.environment_value::<crate::platform_widgets::PlatformMenuRetainedMarker>()?;
        let retained = old_marker.binding.clone();
        let incoming = new_marker.binding.clone();
        if let WidgetKind::LayoutBuilder { environment, .. } = widget.kind_mut() {
            *environment = Some(old_scope);
        }
        Some((retained, incoming))
    }

    pub(super) fn update_existing_inner(
        &mut self,
        id: ElementId,
        widget: &Widget,
    ) -> Result<(), TreeError> {
        let _node_guard = self.guard_element(FramePhase::Build, id);
        // Keep the unchanged-descriptor bailout only where equality cannot
        // recurse through declarative child edges. Full Widget equality is a
        // structural operation and a pathological but valid unary tree can be
        // thousands of nodes deep; using it here would turn reconciliation
        // into native-stack recursion before the retained traversal policy can
        // take effect. Wide/static trees still retain the important leaf fast
        // path, while non-leaf widgets reconcile their direct children.
        let current = self
            .elements
            .get(id.0)
            .ok_or(TreeError::MissingElement(id))?;
        if current.widget.ptr_eq(widget) {
            self.diagnostics.identical_child_bailouts += 1;
            return Ok(());
        }
        if current.widget.children_refs().is_empty()
            && widget.children_refs().is_empty()
            && current.widget == *widget
        {
            self.diagnostics.identical_child_bailouts += 1;
            return Ok(());
        }
        #[cfg(feature = "devtools")]
        let trace = self.devtools_trace_begin_element(id, TracePhase::Build);
        let old = self
            .elements
            .get(id.0)
            .ok_or(TreeError::MissingElement(id))?
            .widget
            .clone();
        if let WidgetKind::SelectionListener {
            notifier: old_notifier,
            ..
        } = old.kind()
            && let WidgetKind::SelectionListener {
                notifier: new_notifier,
                ..
            } = widget.kind()
            && old_notifier != new_notifier
        {
            old_notifier.unregister();
            new_notifier.register();
        }
        let (old_override, old_boundary, render_context) = {
            let element = self.element_live(id, "retained element must remain live");
            (
                element.environment_override.clone(),
                element.environment_boundary,
                element.render_context.clone(),
            )
        };
        let (new_override, new_boundary) = match widget.kind() {
            WidgetKind::LayoutBuilder {
                environment,
                environment_boundary,
                ..
            } => (environment.clone(), *environment_boundary),
            _ => (None, false),
        };
        let old_type = old_override.as_ref().map(|scope| scope.type_id);
        let new_type = new_override.as_ref().map(|scope| scope.type_id);
        let environment_topology_changed = old_boundary != new_boundary || old_type != new_type;
        let environment_value_changed = !environment_topology_changed
            && match (&old_override, &new_override) {
                (Some(old), Some(new)) => !Rc::ptr_eq(&old.value, &new.value),
                (None, None) => false,
                _ => false,
            };
        #[cfg(feature = "devtools")]
        let property_changes = crate::devtools_props::diff_properties(old.kind(), widget.kind());
        debug_assert_eq!(
            old.type_(),
            widget.type_(),
            "only compatible elements may update"
        );
        self.check_keys_borrowed(Some(id), widget.children_refs())?;
        let render = self
            .element_live(id, "retained element must remain live")
            .render;
        let old_kind = self
            .render_live(render, "updated render must remain live")
            .object
            .kind
            .clone();
        let new_kind = render_context.build(|context| render_kind(widget, context));
        carry_replaced_transition(&old_kind, &new_kind);
        // A rebuilt viewport descriptor starts with fresh sliver estimates.
        // Hand compatible retained measurement to the replacement before it
        // lays out, so replacing a header during overscroll keeps its true
        // size instead of flashing an estimate and spuriously moving the
        // scroll range. Stretched presentation is never inherited.
        if let (
            RenderKind::SliverViewport { config: previous },
            RenderKind::SliverViewport { config: next },
        ) = (&old_kind, &new_kind)
        {
            next.delegate.adopt_compatible_state(&*previous.delegate);
        }
        #[cfg(feature = "devtools")]
        let mut work_reasons: (Option<String>, Option<String>, Option<String>) = (None, None, None);
        let mut layer_structure_changed = false;
        if old_kind != new_kind {
            let mut layers = self
                .render_live(render, "updated render must remain live")
                .object
                .layers
                .clone();
            layer_structure_changed =
                layers.reconcile_structure(&mut self.compositor, widget.kind());
            self.render_live_mut(render, "updated render must remain live")
                .object
                .layers = layers;
            let invalidation = self
                .render_live_mut(render, "updated render must remain live")
                .object
                .update_kind(new_kind.clone());
            let (layers, size) = {
                let node = self.render_live(render, "updated render must remain live");
                (node.object.layers.clone(), node.size)
            };
            let world_transform = self.render_world_transform(render);
            let content_transform = self.content_transform(render);
            layers.update_from_kind(
                &mut self.compositor,
                &new_kind,
                size,
                world_transform,
                content_transform,
            );
            if invalidation.contains(Invalidation::COMPOSITE) {
                self.diagnostics.compositor_only_updates += 1;
                #[cfg(feature = "devtools")]
                {
                    work_reasons.2 = Some("retained compositor property changed".into());
                }
            } else if invalidation.contains(Invalidation::PAINT)
                && !invalidation.contains(Invalidation::LAYOUT)
            {
                #[cfg(feature = "devtools")]
                {
                    work_reasons.1 = Some("paint-only configuration changed".into());
                }
                self.mark_render_dirty(render, DirtyFlags::PAINT, false);
            } else if invalidation.contains(Invalidation::LAYOUT) {
                #[cfg(feature = "devtools")]
                {
                    work_reasons.0 = Some("layout-affecting configuration changed".into());
                    work_reasons.1 = Some("layout result invalidated paint".into());
                }
                self.mark_render_dirty(render, DirtyFlags::LAYOUT | DirtyFlags::PAINT, true);
            }
        }
        {
            let element = self.element_live_mut(id, "retained element must remain live");
            element.widget = widget.clone();
            element.environment_override = new_override;
            element.environment_boundary = new_boundary;
            if environment_topology_changed || environment_value_changed {
                element.layout_builder_constraints = None;
            }
            element.dirty.remove(DirtyFlags::BUILD);
        }
        let visibility_changed = matches!(
            (&old_kind, &new_kind),
            (RenderKind::Visibility { visible: before, .. },
             RenderKind::Visibility { visible: after, .. }) if before != after
        );
        if layer_structure_changed || visibility_changed {
            self.sync_render_children(id);
        }
        if environment_topology_changed {
            self.rebind_inherited_subtree(id);
        } else if environment_value_changed {
            let (build_context, render_context, scope) = {
                let element = self.element_live(id, "retained element must remain live");
                (
                    element.build_context.clone(),
                    element.render_context.clone(),
                    element
                        .environment_override
                        .clone()
                        .expect("value change retains a scope"),
                )
            };
            build_context.set_erased(scope.type_id, scope.value.clone());
            render_context.set_erased(scope.type_id, scope.value);
            self.apply_inherited_invalidations();
        }
        if matches!(widget.kind(), WidgetKind::LayoutBuilder { .. }) {
            // A new descriptor may carry a different builder closure while
            // retaining the same constraints/revision value. Force one
            // materialization so updates cannot leave the old child mounted.
            if let Some(element) = self.elements.get_mut(id.0) {
                element.layout_builder_constraints = None;
                element.layout_builder_revision = 0;
            }
            // LayoutBuilder has a stable render kind, so the normal
            // old-kind/new-kind invalidation above does not run. Propagate the
            // dirty bit explicitly; otherwise an unchanged parent can return
            // from the layout cache before this builder gets a chance to
            // materialize the new closure output.
            self.mark_render_dirty(render, DirtyFlags::LAYOUT | DirtyFlags::PAINT, true);
        }
        self.diagnostics.rebuilds += 1;
        #[cfg(feature = "devtools")]
        {
            let element = self.element_live_mut(id, "retained element must remain live");
            element.dev.builds += 1;
            element.dev.revision += 1;
            element.dev.property_changes = property_changes;
            element.dev.layout_reason = work_reasons.0;
            element.dev.paint_reason = work_reasons.1;
            element.dev.composite_reason = work_reasons.2;
        }
        // Generated children are owned by the dynamic-child protocol, not by
        // ordinary declarative child topology. Preserve the currently mounted
        // set across compatible descriptor updates; the next dynamic
        // materialization validates and commits the replacement set
        // transactionally. This applies uniformly to slivers, advanced
        // scrolling families and LayoutBuilder.
        if matches!(widget.kind().structure().children, WidgetChildren::Dynamic) {
            #[cfg(feature = "devtools")]
            self.devtools_trace_end(trace);
            return Ok(());
        }
        let previous = self
            .element_live(id, "retained element must remain live")
            .children
            .clone();
        let desired = widget.children_refs().into_iter().collect::<Vec<_>>();
        let reconciled = self.reconcile_children(id, previous.clone(), &desired)?;
        if reconciled != previous {
            self.element_live_mut(id, "retained element must remain live")
                .children = reconciled;
            self.sync_render_children(id);
        }
        self.sync_raw_input_state(id);
        #[cfg(feature = "devtools")]
        self.devtools_trace_end(trace);
        Ok(())
    }
    pub(super) fn compatible(&mut self, id: ElementId, widget: &Widget) -> bool {
        match self.elements.get(id.0) {
            Some(element) => {
                self.diagnostics.widget_type_comparisons += 1;
                element.widget.type_() == widget.type_() && {
                    self.diagnostics.key_comparisons += 1;
                    element.widget.key() == widget.key()
                }
            }
            None => false,
        }
    }
    pub(super) fn reconcile_children(
        &mut self,
        parent: ElementId,
        previous: Vec<ElementId>,
        desired: &[&Widget],
    ) -> Result<Vec<ElementId>, TreeError> {
        self.check_keys_borrowed(Some(parent), desired.iter().copied())?;
        self.diagnostics.child_list_scans += 1;
        let mut start = 0;
        let mut old_end = previous.len();
        let mut new_end = desired.len();
        let mut next = Vec::with_capacity(new_end);
        while start < old_end && start < new_end && self.compatible(previous[start], desired[start])
        {
            let id = previous[start];
            self.update_existing(id, desired[start])?;
            next.push(id);
            start += 1;
            self.diagnostics.reconciliation_fast_paths += 1;
            self.diagnostics.elements_reused += 1;
        }
        while start < old_end
            && start < new_end
            && self.compatible(previous[old_end - 1], desired[new_end - 1])
        {
            old_end -= 1;
            new_end -= 1;
            self.diagnostics.reconciliation_fast_paths += 1;
            self.diagnostics.elements_reused += 1;
        }
        let old_middle = &previous[start..old_end];
        let desired_middle = &desired[start..new_end];
        // Track original positions to detect genuine moves (Task 15 counter).
        let mut keyed: HashMap<Key, (ElementId, usize)> = HashMap::new();
        let mut unkeyed_ids: Vec<(ElementId, usize)> = Vec::new();
        let any_keys = old_middle.iter().any(|id| {
            self.elements
                .get(id.0)
                .is_some_and(|e| e.widget.key().is_some())
        }) || desired_middle.iter().any(|w| w.key().is_some());
        if any_keys {
            self.diagnostics.key_maps_built += 1;
            for (position, id) in old_middle.iter().enumerate() {
                if let Some(key) = self
                    .elements
                    .get(id.0)
                    .and_then(|e| e.widget.key().cloned())
                {
                    keyed.insert(key, (*id, position));
                    self.diagnostics.key_map_entries += 1;
                } else {
                    unkeyed_ids.push((*id, position));
                }
            }
        } else {
            for (position, id) in old_middle.iter().enumerate() {
                unkeyed_ids.push((*id, position));
            }
        }
        let mut unkeyed = unkeyed_ids.into_iter();
        let mut used = std::collections::HashSet::new();
        for widget in desired_middle {
            let candidate = if let Some(key) = widget.key() {
                self.diagnostics.key_lookups += 1;
                keyed.get(key).copied()
            } else {
                unkeyed.next()
            };
            self.diagnostics.widget_type_comparisons += 1;
            if let Some((id, old_position)) = candidate.filter(|(id, _)| {
                self.elements
                    .get(id.0)
                    .is_some_and(|e| e.widget.type_() == widget.type_())
            }) {
                self.update_existing(id, widget)?;
                used.insert(id);
                // A reused child is "moved" when its previous middle position
                // differs from the position it is emitted at now.
                let emitted_index = next.len();
                let expected_index = start + old_position;
                if emitted_index != expected_index {
                    self.diagnostics.elements_moved += 1;
                }
                next.push(id);
                self.diagnostics.elements_reused += 1;
            } else {
                self.diagnostics.elements_created += 1;
                next.push(self.mount_element(Some(parent), (*widget).clone())?);
            }
        }
        for id in old_middle {
            if !used.contains(id) {
                self.unmount_element(*id);
                self.diagnostics.elements_removed += 1;
            }
        }
        let mut suffix = Vec::new();
        for index in new_end..desired.len() {
            let id = previous[old_end + (index - new_end)];
            self.update_existing(id, desired[index])?;
            suffix.push(id);
            self.diagnostics.elements_reused += 1;
        }
        next.extend(suffix);
        Ok(next)
    }
    /// Reconciles the indexed child window produced by the retained sliver
    /// protocol. Unlike a box list, each child carries a viewport-scoped
    /// identity and an explicit sliver placement/constraint record.
    pub(super) fn reconcile_sliver_children(
        &mut self,
        id: RenderObjectId,
        config: &SliverViewportConfig,
        layout: &SliverViewportLayout,
    ) -> Result<(), TreeError> {
        let element_id = self.element_for_render(id).unwrap_or_else(|| {
            self.panic_invariant(
                InvariantCategory::Ownership,
                None,
                Some(id),
                "sliver viewport render must have a live element owner",
            )
        });
        let old_ids = {
            let element = self.element_live(element_id, "sliver viewport element must remain live");
            element.sliver_child_ids.clone()
        };
        let desired = layout
            .children
            .iter()
            .map(|child| DesiredDynamicChild {
                key: child.id,
                widget: child.widget.clone(),
            })
            .collect::<Vec<_>>();
        let reconciled =
            self.reconcile_dynamic_children(element_id, &old_ids, desired, true, |key| {
                GeneratedChildIdentity::Sliver(format!("{key:?}"))
            })?;
        let mut next_semantic_indices = Vec::with_capacity(layout.children.len());
        let mut overlays = HashSet::new();
        for child in &layout.children {
            let child_id = child.id;
            if child.placement != crate::scrolling::SliverChildPlacement::Flow {
                overlays.insert(child_id);
            }
            next_semantic_indices.push(
                child
                    .semantic_index
                    .or_else(|| child.widget.semantic_index())
                    .or_else(|| child_id.item_index()),
            );
        }
        let element = self.element_live_mut(element_id, "sliver viewport element must remain live");
        element.children = reconciled.children;
        element.sliver_child_ids = reconciled.keys;
        element.sliver_child_semantic_indices = next_semantic_indices;
        element.sliver_overlay_ids = overlays;
        element.advanced_child_keys.clear();
        element.sliver_delegate_revision = config.delegate.revision();
        element.sliver_scroll_revision = config.controller.revision();
        self.sync_render_children(element_id);
        Ok(())
    }

    /// Reconciles children materialized by one of the renderer-independent
    /// advanced scrolling algorithms. The algorithm-owned key is kept
    /// separately from the widget key because a lazy child can move in and
    /// out of the cache window without changing its declarative identity.
    pub(super) fn reconcile_advanced_children(
        &mut self,
        element_id: ElementId,
        desired: Vec<(AdvancedChildKey, Widget)>,
    ) -> Result<(), TreeError> {
        let old_keys = {
            let element =
                self.element_live(element_id, "advanced scrolling element must remain live");
            element.advanced_child_keys.clone()
        };
        let desired = desired
            .into_iter()
            .map(|(key, widget)| DesiredDynamicChild { key, widget })
            .collect();
        let reconciled =
            self.reconcile_dynamic_children(element_id, &old_keys, desired, true, |key| {
                GeneratedChildIdentity::Advanced(format!("{key:?}"))
            })?;

        let element =
            self.element_live_mut(element_id, "advanced scrolling element must remain live");
        element.children = reconciled.children;
        element.advanced_child_keys = reconciled.keys;
        element.sliver_child_ids.clear();
        element.sliver_child_semantic_indices.clear();
        element.sliver_overlay_ids.clear();
        self.sync_render_children(element_id);
        Ok(())
    }

    fn reconcile_dynamic_children<K>(
        &mut self,
        owner: ElementId,
        old_keys: &[K],
        desired: Vec<DesiredDynamicChild<K>>,
        account_items: bool,
        identity: impl Fn(&K) -> GeneratedChildIdentity,
    ) -> Result<DynamicChildResult<K>, TreeError>
    where
        K: Clone + Eq + std::hash::Hash,
    {
        let mut seen = HashSet::with_capacity(desired.len());
        for child in &desired {
            if !seen.insert(child.key.clone()) {
                return Err(TreeError::InvalidGeneratedChild {
                    owner,
                    child: identity(&child.key),
                    source: Box::new(TreeError::InvalidWidgetConfiguration {
                        widget: "dynamic child set",
                        reason: "duplicate generated child key".into(),
                    }),
                });
            }
            self.validate_widget_subtree(&child.widget, Some(owner))
                .map_err(|source| TreeError::InvalidGeneratedChild {
                    owner,
                    child: identity(&child.key),
                    source: Box::new(source),
                })?;
        }

        let old_children = self
            .element_live(owner, "dynamic child owner must remain live")
            .children
            .clone();
        debug_assert_eq!(old_keys.len(), old_children.len());
        let existing = old_keys
            .iter()
            .cloned()
            .zip(old_children.iter().copied())
            .collect::<HashMap<_, _>>();
        let mut retained = HashSet::with_capacity(desired.len());
        let mut next_keys = Vec::with_capacity(desired.len());
        let mut next_children = Vec::with_capacity(desired.len());

        for DesiredDynamicChild { key, widget } in desired {
            let child = if let Some(existing) = existing
                .get(&key)
                .copied()
                .filter(|existing| self.compatible(*existing, &widget))
            {
                self.update_existing(existing, &widget).map_err(|source| {
                    TreeError::InvalidGeneratedChild {
                        owner,
                        child: identity(&key),
                        source: Box::new(source),
                    }
                })?;
                existing
            } else {
                if account_items {
                    self.diagnostics.items_built += 1;
                }
                let child = self.mount_element(Some(owner), widget).map_err(|source| {
                    TreeError::InvalidGeneratedChild {
                        owner,
                        child: identity(&key),
                        source: Box::new(source),
                    }
                })?;
                if account_items {
                    self.diagnostics.items_mounted += 1;
                }
                child
            };
            retained.insert(child);
            next_keys.push(key);
            next_children.push(child);
        }

        for child in old_children {
            if !retained.contains(&child) {
                self.unmount_element(child);
                if account_items {
                    self.diagnostics.items_unmounted += 1;
                }
            } else if account_items {
                self.diagnostics.items_reused += 1;
            }
        }

        Ok(DynamicChildResult {
            keys: next_keys,
            children: next_children,
        })
    }

    pub(super) fn materialize_layout_builder(
        &mut self,
        id: RenderObjectId,
        constraints: Constraints,
    ) -> Result<(), TreeError> {
        with_recursive_tree_stack(|| self.materialize_layout_builder_inner(id, constraints))
    }

    /// Invalidates the retained measurement enclosing a rebuilt builder
    /// child after its content changed. Walks up past the rebuilt element
    /// itself to the nearest enclosing sliver viewport and notifies exactly
    /// the sliver that owns the rebuilt descendant by viewport-scoped child
    /// identity — sibling slivers are untouched. Controller-driven content
    /// changes (text-field content revisions observed in the layout
    /// preamble) enter through this same channel rather than a second walk.
    ///
    /// Only genuine content changes call this: stateful-builder revision
    /// changes, descriptor replacements, inherited invalidations, and
    /// controller content revisions. Constraints-driven rematerializations
    /// from scrolling, stretch presentation, or measurement passes never call
    /// it (they carry no content signal), so invalidation cannot recurse or
    /// loop: demotion is idempotent, learning passes perform layout only,
    /// and every demotion converges within the viewport's bounded re-layout
    /// passes.
    pub(super) fn invalidate_enclosing_sliver_measurement(&mut self, id: ElementId) {
        let mut child_below = id;
        let mut current = self.elements.get(id.0).and_then(|element| element.parent);
        while let Some(ancestor) = current {
            let (is_viewport, children, sliver_ids, render, parent) =
                match self.elements.get(ancestor.0) {
                    Some(element) => (
                        matches!(element.widget.kind(), WidgetKind::SliverViewport { .. }),
                        element.children.clone(),
                        element.sliver_child_ids.clone(),
                        element.render,
                        element.parent,
                    ),
                    None => return,
                };
            if is_viewport {
                let Some(position) = children.iter().position(|child| *child == child_below) else {
                    return;
                };
                let Some(scoped) = sliver_ids.get(position).copied() else {
                    return;
                };
                if let Some(render) = self.renders.get(render.0)
                    && let RenderKind::SliverViewport { config } = &render.object.kind
                {
                    config.delegate.invalidate_sliver_child(scoped);
                }
                return;
            }
            child_below = ancestor;
            current = parent;
        }
    }

    pub(super) fn materialize_layout_builder_inner(
        &mut self,
        id: RenderObjectId,
        constraints: Constraints,
    ) -> Result<(), TreeError> {
        let element_id = self.element_for_render(id).unwrap_or_else(|| {
            self.panic_invariant(
                InvariantCategory::Ownership,
                None,
                Some(id),
                "layout-builder render must have a live element owner",
            )
        });
        let _build_guard = self.guard_element(FramePhase::Build, element_id);
        let (
            builder,
            build_context,
            revision,
            previous_constraints,
            previous_revision,
            previous_children,
        ) = {
            let element = self.element_live(element_id, "layout-builder element must remain live");
            let WidgetKind::LayoutBuilder {
                builder, revision, ..
            } = element.widget.kind()
            else {
                return Ok(());
            };
            (
                builder.clone(),
                element.build_context.clone(),
                revision.clone(),
                element.layout_builder_constraints,
                element.layout_builder_revision,
                element.children.clone(),
            )
        };
        let revision_value = revision.as_ref().map_or(0, |revision| revision.get());
        if previous_constraints == Some(constraints)
            && previous_revision == revision_value
            && previous_children.len() == 1
        {
            return Ok(());
        }
        if revision_value != previous_revision {
            // A revision-driven rebuild carries new content (not new
            // constraints): invalidate the enclosing sliver measurement so it
            // revalidates unbounded. Constraints-driven rebuilds from
            // scrolling, stretch presentation, or measurement passes take the
            // early return above or skip this branch, which is what keeps
            // invalidation from ever looping.
            self.invalidate_enclosing_sliver_measurement(element_id);
        }
        let child = build_context.build(|context| {
            let context = BuildContext::new(context);
            builder(&context, constraints)
        });
        let old_keys = vec![(); previous_children.len()];
        let reconciled = self.reconcile_dynamic_children(
            element_id,
            &old_keys,
            vec![DesiredDynamicChild {
                key: (),
                widget: child,
            }],
            false,
            |_| GeneratedChildIdentity::LayoutBuilder,
        )?;
        let element = self.element_live_mut(element_id, "layout-builder element must remain live");
        element.children = reconciled.children;
        element.layout_builder_constraints = Some(constraints);
        element.layout_builder_revision = revision_value;
        self.sync_render_children(element_id);
        Ok(())
    }
    /// Returns whether the current retained sliver children still cover the
    /// viewport's requested cache window. A small scroll can therefore stay a
    /// compositor-only transform; layout is needed only when a cache boundary
    /// is crossed or the delegate itself changed.
    pub(super) fn sliver_cache_window_is_covered(
        &self,
        render_id: RenderObjectId,
        element: &Element,
        config: &SliverViewportConfig,
    ) -> bool {
        let Some(render) = self.renders.get(render_id.0) else {
            return false;
        };
        let viewport = config.axis.main_extent(render.size).max(0.);
        let cache = config.cache_extent.max(0.);
        let content_end = (config.controller.max_offset() + viewport).max(0.);
        let physical =
            physical_scroll_offset(&config.controller, config.reverse).clamp(0., content_end);
        if !physical.is_finite()
            || !viewport.is_finite()
            || !cache.is_finite()
            || !content_end.is_finite()
        {
            return false;
        }
        let wanted_start = (physical - cache).max(0.);
        let wanted_end = (physical + viewport + cache)
            .min(content_end)
            .max(wanted_start);
        let mut materialized_start = f32::INFINITY;
        let mut materialized_end = f32::NEG_INFINITY;
        for child in &element.children {
            let Some(child_element) = self.elements.get(child.0) else {
                return false;
            };
            let Some(child_render) = self.renders.get(child_element.render.0) else {
                return false;
            };
            let child_start = match config.axis {
                Axis::Horizontal => child_render.offset.x,
                Axis::Vertical => child_render.offset.y,
            };
            let child_extent = config.axis.main_extent(child_render.size).max(0.);
            if !child_start.is_finite() || !child_extent.is_finite() {
                return false;
            }
            materialized_start = materialized_start.min(child_start);
            materialized_end = materialized_end.max(child_start + child_extent);
        }
        if !materialized_start.is_finite() || !materialized_end.is_finite() {
            return content_end == 0.;
        }
        const RANGE_EPSILON: f32 = 0.001;
        materialized_start <= wanted_start + RANGE_EPSILON
            && materialized_end + RANGE_EPSILON >= wanted_end
    }

    /// A controller or sliver delegate can change independently of widget
    /// BUILD. Mark the retained viewport dirty only when its sliver protocol
    /// needs a new cache window or its delegate changed.
    pub(super) fn refresh_sliver_ranges(&mut self) {
        let pending = self
            .renders
            .iter()
            .filter_map(|(raw, render)| {
                let RenderKind::SliverViewport { config } = &render.object.kind else {
                    return None;
                };
                let element = self.element_for_render(RenderObjectId(raw))?;
                let element = self.elements.get(element.0)?;
                let delegate_changed =
                    element.sliver_delegate_revision != config.delegate.revision();
                let scroll_changed = element.sliver_scroll_revision != config.controller.revision();
                (delegate_changed
                    || (scroll_changed
                        && (config.delegate.scroll_layout_dependency()
                            == crate::scrolling::SliverScrollDependency::ScrollOffset
                            || !self.sliver_cache_window_is_covered(
                                RenderObjectId(raw),
                                element,
                                config,
                            ))))
                .then_some(RenderObjectId(raw))
            })
            .collect::<Vec<_>>();
        for render in pending {
            self.mark_render_dirty(render, DirtyFlags::LAYOUT, true);
        }
    }

    /// Rebuilds the retained notification subscriptions after layout. A
    /// sliver viewport owns its materialized children, so installing these
    /// subscriptions after materialization lets an ancestor listener observe
    /// both ordinary and newly visible nested viewports without rebuilding the
    /// application widget description.
    pub(super) fn refresh_notification_listeners(&mut self) {
        let mut listeners = self
            .elements
            .iter()
            .filter_map(|(raw, element)| {
                let WidgetKind::NotificationListener {
                    callback: Some(callback),
                    ..
                } = element.widget.kind()
                else {
                    return None;
                };
                let id = ElementId(raw);
                Some((id, callback.clone(), self.element_depth(id)))
            })
            .collect::<Vec<_>>();

        // Drop old registrations first. This also removes listeners for
        // children that left a lazy cache window.
        for (_, element) in self.elements.iter_mut() {
            element.notification_subscriptions.clear();
        }

        // Controller dispatch is stop-on-true. Register the deepest wrapper
        // first so an inner listener gets first refusal, matching bubbling up
        // through Flutter's notification tree.
        listeners.sort_by_key(|(_, _, depth)| std::cmp::Reverse(*depth));
        for (listener, callback, _) in listeners {
            let mut sources = Vec::new();
            self.collect_scroll_sources(listener, 0, &mut sources);
            let subscriptions = sources
                .into_iter()
                .map(|(controller, depth)| {
                    let callback = callback.clone();
                    controller.add_listener(move |mut notification| {
                        notification.depth = depth;
                        callback(notification)
                    })
                })
                .collect::<Vec<_>>();
            if let Some(element) = self.elements.get_mut(listener.0) {
                element.notification_subscriptions = subscriptions;
            }
        }
    }

    pub(super) fn element_depth(&self, mut id: ElementId) -> usize {
        let mut depth = 0;
        while let Some(parent) = self.elements.get(id.0).and_then(|element| element.parent) {
            depth += 1;
            id = parent;
        }
        depth
    }

    pub(super) fn scroll_controller_for_element(&self, id: ElementId) -> Option<ScrollController> {
        let render = self.elements.get(id.0)?.render;
        match &self.renders.get(render.0)?.object.kind {
            RenderKind::Scroll { controller, .. } => Some(controller.clone()),
            RenderKind::SliverViewport { config } => Some(config.controller.clone()),
            _ => None,
        }
    }

    pub(super) fn collect_scroll_sources(
        &self,
        id: ElementId,
        viewport_depth: usize,
        out: &mut Vec<(ScrollController, usize)>,
    ) {
        let mut work = vec![(id, viewport_depth)];
        while let Some((current, depth)) = work.pop() {
            let Some(element) = self.elements.get(current.0) else {
                continue;
            };
            let next_depth = if let Some(controller) = self.scroll_controller_for_element(current) {
                if !out.iter().any(|(existing, _)| *existing == controller) {
                    out.push((controller, depth));
                }
                depth + 1
            } else {
                depth
            };
            work.extend(
                element
                    .children
                    .iter()
                    .rev()
                    .copied()
                    .map(|child| (child, next_depth)),
            );
        }
    }

    pub(super) fn unmount(&mut self, id: ElementId) -> Result<(), TreeError> {
        if !self.elements.contains(id.0) {
            return Err(TreeError::MissingElement(id));
        }
        self.unmount_element(id);
        if self.root == Some(id) {
            self.root = None;
        }
        self.refresh_selection_states();
        Ok(())
    }
    pub(super) fn unmount_element(&mut self, id: ElementId) {
        enum UnmountWork {
            Enter(ElementId),
            Exit(ElementId, RenderObjectId),
        }

        let mut work = vec![UnmountWork::Enter(id)];
        while let Some(next) = work.pop() {
            match next {
                UnmountWork::Enter(id) => {
                    self.raw_input_unmounted(id);
                    self.external_drop_target_unmounted(id);
                    let Some(element) = self.elements.remove(id.0) else {
                        continue;
                    };
                    if let Some(semantic) = self.semantic_ids.remove(&id) {
                        let _ = self.semantics.remove(semantic);
                    }
                    let build_consumer = element.build_context.consumer_id();
                    let render_consumer = element.render_context.consumer_id();
                    element.build_context.clear_dependencies();
                    element.render_context.clear_dependencies();
                    self.inherited_consumers.remove(&build_consumer);
                    self.inherited_consumers.remove(&render_consumer);
                    match element.widget.kind() {
                        WidgetKind::SelectionListener {
                            notifier, delegate, ..
                        } => {
                            notifier.unregister();
                            self.static_selections.remove(&id);
                            let controller = delegate.controller();
                            controller.set_registered_child_count(0);
                            controller.clear_selection();
                        }
                        WidgetKind::SelectionArea { controller, .. } => {
                            self.static_selections.remove(&id);
                            controller.set_registered_child_count(0);
                            controller.clear_selection();
                        }
                        WidgetKind::SelectionContainer { delegate, .. } => {
                            self.static_selections.remove(&id);
                            let controller = delegate.controller();
                            controller.set_registered_child_count(0);
                            controller.clear_selection();
                        }
                        _ => {}
                    }
                    self.active_gestures.retain(|_, active| {
                        active
                            .members
                            .iter()
                            .all(|candidate| candidate.element != id)
                    });
                    let stale_trackpad_streams = self
                        .active_trackpad_gestures
                        .iter()
                        .filter_map(|(key, active)| (active.element == id).then_some(*key))
                        .collect::<Vec<_>>();
                    for key in stale_trackpad_streams {
                        let _ = self.gesture_arena.cancel(key);
                        self.active_trackpad_gestures.remove(&key);
                    }
                    self.pointer_captures.retain(|_, target| *target != id);
                    self.scale_gestures.remove(&id);

                    let render = element.render;
                    let children = element.children.clone();
                    work.push(UnmountWork::Exit(id, render));
                    work.extend(children.into_iter().rev().map(UnmountWork::Enter));
                }
                UnmountWork::Exit(id, render_id) => {
                    if let Some(render) = self.renders.remove(render_id.0) {
                        render.object.layers.remove(&mut self.compositor);
                    }
                    self.unmounted.push(id);
                    self.diagnostics.unmounts += 1;
                }
            }
        }
    }
    pub(super) fn check_keys_borrowed<'a>(
        &self,
        parent: Option<ElementId>,
        widgets: impl IntoIterator<Item = &'a Widget>,
    ) -> Result<(), TreeError> {
        let mut keys: HashSet<&Key> = HashSet::new();
        for key in widgets.into_iter().filter_map(Widget::key) {
            if !keys.insert(key) {
                return Err(TreeError::DuplicateKey {
                    key: key.clone(),
                    parent,
                });
            }
        }
        Ok(())
    }

    /// Validates immutable widget topology without allocating retained state.
    /// Generated builders use this before reconciliation so malformed output
    /// cannot leave half-mounted elements or compositor layers behind.
    pub(super) fn validate_widget_subtree(
        &self,
        widget: &Widget,
        parent: Option<ElementId>,
    ) -> Result<(), TreeError> {
        let mut stack = vec![(widget, parent)];
        while let Some((current, owner)) = stack.pop() {
            let children = current.children_refs().into_iter().collect::<Vec<_>>();
            self.check_keys_borrowed(owner, children.iter().copied())?;
            // Descendants do not have retained IDs yet. Their duplicate-key
            // error remains structured; the generated-child wrapper supplies
            // the live owner at the materialization boundary.
            stack.extend(children.into_iter().map(|child| (child, None)));
        }
        Ok(())
    }
    pub(super) fn sync_render_children(&mut self, id: ElementId) {
        let (render, children) = {
            let e = self.element_live(id, "mounted element must remain live");
            (e.render, e.children.clone())
        };
        let render_children: Vec<_> = children
            .into_iter()
            .filter_map(|child| self.render_id(child))
            .collect();
        let children_changed = {
            let node = self.render_live_mut(render, "mounted render must remain live");
            let changed = node.children != render_children;
            node.children = render_children.clone();
            if changed {
                node.dirty.insert(DirtyFlags::LAYOUT | DirtyFlags::PAINT);
            }
            changed
        };
        let (layers, kind) = {
            let node = self.render_live(render, "mounted render must remain live");
            (node.object.layers.clone(), node.object.kind.clone())
        };
        let mut child_layers = render_children
            .iter()
            .map(|child| {
                self.render_live(*child, "mounted child render must remain live")
                    .object
                    .layers
                    .root
            })
            .collect::<Vec<_>>();
        if let RenderKind::IndexedStack { index, .. } = kind {
            child_layers = child_layers.get(index).copied().into_iter().collect();
        } else if matches!(kind, RenderKind::SliverViewport { .. }) {
            let overlays = self
                .element_for_render(render)
                .and_then(|element| self.elements.get(element.0))
                .map(|element| element.sliver_overlay_ids.clone())
                .unwrap_or_default();
            let mut order = (0..render_children.len()).collect::<Vec<_>>();
            order.sort_by_key(|index| {
                let child_id = self
                    .element_for_render(render)
                    .and_then(|element| self.elements.get(element.0))
                    .and_then(|element| element.sliver_child_ids.get(*index))
                    .copied();
                usize::from(child_id.is_some_and(|id| overlays.contains(&id)))
            });
            child_layers = order
                .into_iter()
                .filter_map(|index| child_layers.get(index).copied())
                .collect();
        }
        layers.set_render_children(&mut self.compositor, &kind, child_layers);
        for child in render_children {
            self.render_live_mut(child, "mounted child render must remain live")
                .parent = Some(render);
        }
        if children_changed {
            self.mark_render_dirty(render, DirtyFlags::LAYOUT | DirtyFlags::PAINT, true);
        }
    }
}
