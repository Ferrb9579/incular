//! Retained-tree construction, identity, inspection, and public overlay hooks.

use super::*;

use crate::environment::{ContentSensitivity, SensitiveContentHost};
use crate::platform_widgets::PlatformMenuRetainedMarker;
use crate::transient::{
    RetainedTransientPlacement, TransientPlacementInput, TransientPortalMarker, TransientSurfaceId,
    TransientSurfaceSnapshot, place_transient,
};

impl Default for WidgetTree {
    fn default() -> Self {
        Self::new()
    }
}

impl WidgetTree {
    #[must_use]
    pub fn new() -> Self {
        let dependency_root = DependencyContext::new();
        Self {
            elements: Arena::new(),
            renders: Arena::new(),
            root: None,
            diagnostics: Diagnostics::default(),
            unmounted: Vec::new(),
            text_engine: TextEngine::new(),
            compositor: LayerTree::new(),
            compositor_initialized: false,
            animation_time_scale: 1.,
            animation_clock: None,
            next_action: 1,
            pending_handlers: Vec::new(),
            gesture_arena: GestureArena::new(),
            active_gestures: HashMap::new(),
            raw_recognizers: HashMap::new(),
            raw_gesture_streams: HashMap::new(),
            raw_pointer_routes: HashMap::new(),
            mouse_hover: HashMap::new(),
            consumed_tap_pointers: HashSet::new(),
            pointer_captures: HashMap::new(),
            active_drags: HashMap::new(),
            active_external_drop: None,
            scale_gestures: HashMap::new(),
            scrollbar_drag: None,
            semantics: SemanticsTree::new(),
            semantic_ids: HashMap::new(),
            static_selections: HashMap::new(),
            dependency_root,
            inherited_consumers: HashMap::new(),
            environment: RuntimeEnvironment::default(),
            transient_placements: HashMap::new(),
            native_transient_bounds: None,
            native_transient_presentations: HashSet::new(),
            recursion_diagnostics: RecursionDiagnostics::new(),
            #[cfg(feature = "devtools")]
            deep_trace: None,
        }
    }

    /// Sets the ambient runtime environment (safe insets, scaling, etc.) and invalidates layout if needed.
    pub fn set_environment(&mut self, environment: RuntimeEnvironment) {
        let dirty_safe_area = self.environment.safe_insets != environment.safe_insets;
        self.environment = environment;
        if dirty_safe_area {
            for (_, render) in self.renders.iter_mut() {
                if matches!(render.object.kind, RenderKind::SafeArea { .. }) {
                    render.dirty.insert(DirtyFlags::LAYOUT);
                }
            }
        }
    }

    /// Returns a reference to the ambient runtime environment.
    #[must_use]
    pub fn environment(&self) -> &RuntimeEnvironment {
        &self.environment
    }

    /// Returns default platform-menu bridges retained by the current tree.
    /// Desktop adapters bind these to their application-scoped native delegate
    /// after the owning native window has been created.
    #[doc(hidden)]
    #[must_use]
    pub fn platform_menu_bindings(&self) -> Vec<crate::platform_widgets::PlatformMenuBinding> {
        self.elements
            .iter()
            .filter_map(|(_, element)| {
                element
                    .environment_override
                    .as_ref()?
                    .value
                    .downcast_ref::<PlatformMenuRetainedMarker>()
                    .filter(|marker| marker.binding.auto_connect_native_delegate())
                    .map(|marker| marker.binding.clone())
            })
            .collect()
    }

    /// Returns the highest-priority capture policy contributed by all mounted
    /// [`SensitiveContent`](crate::SensitiveContent) scopes. The calculation
    /// intentionally counts registrations rather than using nearest-ancestor
    /// lookup, matching the window-wide Flutter host policy.
    #[must_use]
    pub fn calculated_content_sensitivity(&self) -> Option<ContentSensitivity> {
        let mut host = SensitiveContentHost::new();
        for (_, element) in self.elements.iter() {
            if let Some(sensitivity) = element
                .environment_override
                .as_ref()
                .and_then(|value| value.value.downcast_ref::<ContentSensitivity>())
            {
                host.register(*sensitivity);
            }
        }
        host.calculated_content_sensitivity()
    }

    /// Returns the normalized policy emitted at the platform window boundary.
    /// An empty tree restores the neutral `NotSensitive` fallback.
    #[must_use]
    pub fn content_sensitivity(&self) -> ContentSensitivity {
        self.calculated_content_sensitivity()
            .unwrap_or(ContentSensitivity::NotSensitive)
    }

    /// Layout size of the retained root before paint-only overflow is applied.
    ///
    /// This is the framework's content-sizing boundary. Transforms, shadows,
    /// filters, and positioned/overlay paint overflow must not silently become
    /// native-window layout. Call [`Self::scene_bounds`] when visual bounds are
    /// actually required.
    #[must_use]
    pub fn root_layout_size(&self) -> Option<Size> {
        let root = self.root?;
        let render = self.render_id(root)?;
        self.renders.get(render.0).map(|node| node.size)
    }

    /// World-space visual bounds of the retained scene produced by this tree.
    ///
    /// The compositor is the authority for transforms and viewport clips, so
    /// callers do not need to reconstruct geometry from widget descriptors.
    #[must_use]
    pub fn scene_bounds(&self) -> Option<Rect> {
        self.compositor.scene_bounds()
    }

    /// Visible semantic transient portals after the latest retained layout.
    ///
    /// The first stack child is the anchor. Barrier children remain owned by
    /// the parent view; `popup_child_index` identifies only the transient
    /// content so native presentation never inherits full-window barrier
    /// bounds.
    #[must_use]
    pub fn transient_surfaces(&self) -> Vec<TransientSurfaceSnapshot> {
        self.transient_surface_entries()
            .into_iter()
            .map(|(snapshot, _)| snapshot)
            .collect()
    }

    /// Publishes the desktop work-area/full-display bounds available to native
    /// transient hosts, expressed in this view's logical coordinate space.
    /// `None` means the backend cannot provide a reliable native placement
    /// rectangle and retained transients stay viewport-constrained.
    #[doc(hidden)]
    pub fn set_native_transient_bounds(&mut self, bounds: Option<Rect>) -> bool {
        if self.native_transient_bounds == bounds {
            return false;
        }
        self.native_transient_bounds = bounds;
        true
    }

    /// Selects which retained transients have completed native-host negotiation.
    /// Until a host is confirmed, the popup remains at its overlay placement so
    /// native fallback never flashes/crops a work-area-positioned subtree in the
    /// owning view.
    #[doc(hidden)]
    pub fn set_native_transient_presentations(
        &mut self,
        ids: impl IntoIterator<Item = TransientSurfaceId>,
    ) -> bool {
        let next = ids.into_iter().collect::<HashSet<_>>();
        if self.native_transient_presentations == next {
            return false;
        }
        self.native_transient_presentations = next;
        true
    }

    pub(super) fn resolve_transient_placements(
        &mut self,
        constraints: Constraints,
    ) -> Result<(), TreeError> {
        let entries = self.transient_portal_entries();
        let active = entries.iter().map(|entry| entry.0).collect::<HashSet<_>>();
        self.transient_placements
            .retain(|id, _| active.contains(id));
        self.native_transient_presentations
            .retain(|id| active.contains(id));

        let viewport_size =
            if self.environment.viewport.width > 0.0 && self.environment.viewport.height > 0.0 {
                self.environment.viewport
            } else if constraints.max_width.is_finite() && constraints.max_height.is_finite() {
                Size::new(constraints.max_width, constraints.max_height)
            } else {
                self.root_layout_size().unwrap_or(Size::ZERO)
            };
        let viewport = Rect::from_origin_size(Offset::ZERO, viewport_size);
        let text_direction = self.environment.text_direction;

        for (id, marker, _parent, stack, anchor, popup) in entries {
            let Some(popup_render) = self.render_id(popup) else {
                continue;
            };
            // A positioned transient does not size its owning Stack. Measure it
            // independently first so a compact toolbar can host a much larger
            // native popup without growing the top-level window.
            self.layout_render(
                popup_render,
                Constraints::new(0.0, f32::INFINITY, 0.0, f32::INFINITY),
            )?;
            let desired_size = self
                .renders
                .get(popup_render.0)
                .map_or(Size::ZERO, |node| node.size);
            let Some(anchor_rect) = marker
                .anchor_override
                .or_else(|| self.element_bounds(anchor))
            else {
                continue;
            };
            let available_rect = if marker.presentation
                == incular_config::TransientPresentation::Auto
                && self.native_transient_presentations.contains(&id)
            {
                self.native_transient_bounds.unwrap_or(viewport)
            } else {
                viewport
            };
            let input = TransientPlacementInput {
                anchor_rect,
                desired_size,
                available_rect,
                role: marker.role,
                text_direction,
                placement: marker.placement,
            };
            let first = place_transient(input);
            self.layout_render(
                popup_render,
                Constraints::new(0.0, first.rect.size.width, 0.0, first.rect.size.height),
            )?;
            let actual_size = self
                .renders
                .get(popup_render.0)
                .map_or(Size::ZERO, |node| node.size);
            let mut result = if actual_size == first.rect.size {
                first
            } else {
                place_transient(TransientPlacementInput {
                    desired_size: actual_size,
                    ..input
                })
            };
            result.constrained |= first.constrained;

            let stack_origin = self
                .element_bounds(stack)
                .map_or(Offset::ZERO, |bounds| bounds.origin);
            let local_offset = result.rect.origin - stack_origin;
            let layer = {
                let node = self.render_live_mut(
                    popup_render,
                    "transient popup render must remain live during placement",
                );
                node.offset = local_offset;
                node.object.layers.root
            };
            self.compositor
                .update_transform(layer, CoreTransform::translation(local_offset));
            self.transient_placements.insert(
                id,
                RetainedTransientPlacement {
                    desired_size,
                    result,
                },
            );
        }
        Ok(())
    }

    /// Applies a semantic dismissal cause to every visible transient whose
    /// policy accepts it. Callbacks are collected before invocation so closing
    /// a controller cannot mutate retained traversal in the middle of a scan.
    #[doc(hidden)]
    pub fn dismiss_transients(&self, reason: crate::TransientDismissReason) -> usize {
        let callbacks = self
            .transient_portal_entries()
            .into_iter()
            .rev()
            .filter_map(|(_, marker, _, _, _, _)| {
                marker
                    .dismiss_policy
                    .allows(reason)
                    .then_some(marker.on_dismiss)
                    .flatten()
            })
            .collect::<Vec<_>>();
        let count = callbacks.len();
        for callback in callbacks {
            callback(reason);
        }
        count
    }

    /// Dismisses visible transient chains for which `point` is an outside
    /// activation. A popup containing the point and all of its retained parent
    /// transients are protected; unrelated sibling chains close.
    #[doc(hidden)]
    pub fn dismiss_transients_for_pointer(&self, point: Offset) -> usize {
        let entries = self.transient_portal_entries();
        let snapshots = self
            .transient_surface_entries()
            .into_iter()
            .map(|(snapshot, _)| (snapshot.id, snapshot))
            .collect::<HashMap<_, _>>();
        let mut protected = snapshots
            .values()
            .filter(|snapshot| {
                snapshot.content_rect.contains(point) || snapshot.anchor_rect.contains(point)
            })
            .map(|snapshot| snapshot.id)
            .collect::<HashSet<_>>();
        let mut frontier = protected.iter().copied().collect::<Vec<_>>();
        while let Some(id) = frontier.pop() {
            if let Some(parent) = snapshots.get(&id).and_then(|snapshot| snapshot.parent)
                && protected.insert(parent)
            {
                frontier.push(parent);
            }
        }
        let reason = crate::TransientDismissReason::OutsidePointer;
        let callbacks = entries
            .into_iter()
            .rev()
            .filter_map(|(id, marker, _, _, _, _)| {
                (!protected.contains(&id) && marker.dismiss_policy.outside_pointer)
                    .then_some(marker.on_dismiss)
                    .flatten()
            })
            .collect::<Vec<_>>();
        let count = callbacks.len();
        for callback in callbacks {
            callback(reason);
        }
        count
    }

    /// Dismisses interactive transient chains after logical focus moves. The
    /// chain containing the newly focused element (either its anchor or popup
    /// subtree) and all retained transient ancestors is preserved.
    #[doc(hidden)]
    pub fn dismiss_transients_for_focus(&self, focused: ElementId) -> usize {
        let entries = self.transient_portal_entries();
        let snapshots = self
            .transient_surface_entries()
            .into_iter()
            .map(|(snapshot, _)| (snapshot.id, snapshot))
            .collect::<HashMap<_, _>>();
        let mut protected = entries
            .iter()
            .filter(|(_, _, _, _, anchor, popup)| {
                self.is_descendant_or_self(focused, *anchor)
                    || self.is_descendant_or_self(focused, *popup)
            })
            .map(|(id, _, _, _, _, _)| *id)
            .collect::<HashSet<_>>();
        let mut frontier = protected.iter().copied().collect::<Vec<_>>();
        while let Some(id) = frontier.pop() {
            if let Some(parent) = snapshots.get(&id).and_then(|snapshot| snapshot.parent)
                && protected.insert(parent)
            {
                frontier.push(parent);
            }
        }
        // Focus on a menu-chain ancestor must not collapse a still-owned
        // submenu beneath it while Tab/arrow traversal is moving through the
        // chain. Once an ancestor is protected, protect all currently visible
        // retained transient descendants as part of the same ownership chain.
        loop {
            let mut changed = false;
            for snapshot in snapshots.values() {
                if snapshot
                    .parent
                    .is_some_and(|parent| protected.contains(&parent))
                    && protected.insert(snapshot.id)
                {
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        let reason = crate::TransientDismissReason::FocusLost;
        let callbacks = entries
            .into_iter()
            .rev()
            .filter_map(|(id, marker, _, _, _, _)| {
                (!protected.contains(&id) && marker.dismiss_policy.focus_loss)
                    .then_some(marker.on_dismiss)
                    .flatten()
            })
            .collect::<Vec<_>>();
        let count = callbacks.len();
        for callback in callbacks {
            callback(reason);
        }
        count
    }

    pub(super) fn sync_transient_surface_partitions(&mut self) {
        let entries = self.transient_surface_entries();
        self.compositor.clear_surface_partitions();
        for (snapshot, layer) in entries {
            self.compositor
                .set_surface_partition(layer, Some(snapshot.id.surface_partition()));
        }
    }

    fn transient_surface_entries(
        &self,
    ) -> Vec<(TransientSurfaceSnapshot, incular_rendering::LayerId)> {
        self.transient_portal_entries()
            .into_iter()
            .filter_map(|(id, marker, parent, _stack, anchor, popup)| {
                let popup_render = self.render_id(popup)?;
                let popup_layer = self.renders.get(popup_render.0)?.object.layers.root;
                let content_rect = self.element_bounds(popup)?;
                let placement = self.transient_placements.get(&id).copied().unwrap_or(
                    RetainedTransientPlacement {
                        desired_size: content_rect.size,
                        result: place_transient(TransientPlacementInput {
                            anchor_rect: marker
                                .anchor_override
                                .or_else(|| self.element_bounds(anchor))?,
                            desired_size: content_rect.size,
                            available_rect: Rect::from_origin_size(
                                Offset::ZERO,
                                self.root_layout_size().unwrap_or(Size::ZERO),
                            ),
                            role: marker.role,
                            text_direction: self.environment.text_direction,
                            placement: marker.placement,
                        }),
                    },
                );
                let snapshot = TransientSurfaceSnapshot {
                    id,
                    parent,
                    role: marker.role,
                    presentation: marker.presentation,
                    anchor_rect: marker
                        .anchor_override
                        .or_else(|| self.element_bounds(anchor))?,
                    desired_size: placement.desired_size,
                    placement: marker.placement,
                    text_direction: self.environment.text_direction,
                    placement_result: placement.result,
                    content_rect,
                };
                Some((snapshot, popup_layer))
            })
            .collect()
    }

    pub(super) fn transient_portal_entries(
        &self,
    ) -> Vec<(
        TransientSurfaceId,
        TransientPortalMarker,
        Option<TransientSurfaceId>,
        ElementId,
        ElementId,
        ElementId,
    )> {
        self.elements
            .iter()
            .filter_map(|(raw, element)| {
                let marker = element
                    .environment_override
                    .as_ref()?
                    .value
                    .downcast_ref::<TransientPortalMarker>()?
                    .clone();
                if !marker.show {
                    return None;
                }
                let mut parent = element.parent;
                let mut transient_parent = None;
                while let Some(ancestor) = parent {
                    let ancestor_element = self.elements.get(ancestor.0)?;
                    if ancestor_element
                        .environment_override
                        .as_ref()
                        .and_then(|scope| scope.value.downcast_ref::<TransientPortalMarker>())
                        .is_some_and(|ancestor_marker| ancestor_marker.show)
                    {
                        transient_parent = Some(TransientSurfaceId::from_parts(
                            ancestor.0.index(),
                            ancestor.0.generation(),
                        ));
                        break;
                    }
                    parent = ancestor_element.parent;
                }
                // Environment scopes materialize one transparent child. The
                // open OverlayPortal materializes a Stack beneath that scope.
                let stack = *element.children.first()?;
                let stack_element = self.elements.get(stack.0)?;
                let anchor = *stack_element.children.get(marker.anchor_child_index)?;
                let popup = *stack_element.children.get(marker.popup_child_index)?;
                Some((
                    TransientSurfaceId::from_parts(raw.index(), raw.generation()),
                    marker,
                    transient_parent,
                    stack,
                    anchor,
                    popup,
                ))
            })
            .collect()
    }

    /// Records the input or command responsible for subsequent frame work.
    /// The value is retained until another trigger replaces it so an input
    /// callback that merely schedules a frame remains attributable later.
    pub fn set_diagnostic_trigger(&self, trigger: impl Into<String>) {
        self.recursion_diagnostics.set_trigger(trigger);
    }

    /// Returns the most recent recursion report. Reports are also persisted to
    /// the platform crash-report directory before the guard panics.
    #[must_use]
    pub fn last_recursion_report(&self) -> Option<RecursionReport> {
        self.recursion_diagnostics.last_report()
    }

    /// Overrides the recursion crash-log directory, primarily for embedders
    /// and deterministic tests.
    pub fn set_recursion_report_directory(&self, path: impl Into<std::path::PathBuf>) {
        self.recursion_diagnostics.set_report_directory(path);
    }

    /// Sets the maximum nested pipeline depth. Debug and DevTools builds also
    /// detect exact retained-node re-entry before this fallback is reached.
    pub fn set_recursion_limit(&self, limit: usize) {
        self.recursion_diagnostics.set_limit(limit);
    }

    pub(super) fn guard_phase_root(&self, phase: FramePhase) -> crate::recursion::ActivePhaseGuard {
        self.recursion_diagnostics
            .enter(DiagnosticNode::phase_root(phase))
    }

    pub(super) fn guard_element(
        &self,
        phase: FramePhase,
        id: ElementId,
    ) -> crate::recursion::ActivePhaseGuard {
        let kind = self
            .elements
            .get(id.0)
            .map_or("UnmountedElement", |element| element.widget.type_().name());
        self.recursion_diagnostics.enter(DiagnosticNode {
            phase,
            id: Some(DiagnosticNodeId::Element(id)),
            kind,
            constraints: None,
        })
    }

    pub(super) fn guard_render(
        &self,
        phase: FramePhase,
        id: RenderObjectId,
        constraints: Option<Constraints>,
    ) -> crate::recursion::ActivePhaseGuard {
        let kind = self
            .element_for_render(id)
            .and_then(|element| self.elements.get(element.0))
            .map_or("DetachedRenderObject", |element| {
                element.widget.type_().name()
            });
        self.recursion_diagnostics.enter(DiagnosticNode {
            phase,
            id: Some(DiagnosticNodeId::Render(id)),
            kind,
            constraints,
        })
    }

    /// Starts one bounded Deep-profiler frame. Calling this again discards an
    /// unfinished capture, which keeps stale target sessions from retaining
    /// trace data indefinitely.
    #[cfg(feature = "devtools")]
    pub fn begin_deep_trace(&mut self, max_events: usize) {
        self.deep_trace = Some(DeepTraceCapture::new(max_events));
    }

    /// Finishes a Deep-profiler frame and transfers its bounded storage.
    #[cfg(feature = "devtools")]
    pub fn take_deep_trace(&mut self) -> Option<(Vec<TraceEvent>, u32)> {
        self.deep_trace
            .take()
            .map(|capture| (capture.events, capture.dropped_events))
    }

    #[cfg(feature = "devtools")]
    pub fn devtools_trace_begin_element(
        &mut self,
        id: ElementId,
        phase: TracePhase,
    ) -> Option<u32> {
        let raw = id.0;
        self.deep_trace.as_mut()?.begin(
            DevWidgetId::new(u64::from(raw.index()), u64::from(raw.generation())),
            phase,
        )
    }

    #[cfg(feature = "devtools")]
    pub fn devtools_trace_end(&mut self, token: Option<u32>) {
        if let Some(capture) = &mut self.deep_trace {
            capture.end(token);
        }
    }
    pub fn mount(&mut self, widget: Widget) -> Result<ElementId, TreeError> {
        let _phase_guard = self.guard_phase_root(FramePhase::Build);
        if let Some(root) = self.root {
            self.unmount(root)?;
        }
        let root = self.mount_element(None, widget)?;
        self.root = Some(root);
        self.refresh_notification_listeners();
        Ok(root)
    }
    #[must_use]
    pub fn root(&self) -> Option<ElementId> {
        self.root
    }
    /// Finds the first mounted element whose widget carries exactly `key`.
    #[must_use]
    pub fn element_with_key(&self, key: &Key) -> Option<ElementId> {
        self.elements.iter().find_map(|(raw, element)| {
            (element.widget.key() == Some(key)).then_some(ElementId(raw))
        })
    }
    #[must_use]
    pub fn element_count(&self) -> usize {
        self.elements.len()
    }
    #[must_use]
    pub fn render_object_count(&self) -> usize {
        self.renders.len()
    }
    #[must_use]
    pub fn sliver_viewport_diagnostics(&self) -> Option<SliverViewportDiagnostics> {
        self.elements.iter().find_map(|(_raw, element)| {
            let WidgetKind::SliverViewport { config } = element.widget.kind() else {
                return None;
            };
            let render = self.renders.get(element.render.0)?;
            let mut indices = element
                .sliver_child_ids
                .iter()
                .filter_map(|id| id.item_index());
            let first = indices.next();
            let (start, end) = first.map_or((0, 0), |first| {
                indices.fold((first, first + 1), |(min, max), index| {
                    (min.min(index), max.max(index + 1))
                })
            });
            Some(SliverViewportDiagnostics {
                logical_item_count: config
                    .delegate
                    .child_count()
                    .unwrap_or(element.sliver_child_ids.len()),
                materialized_item_count: element.sliver_child_ids.len(),
                materialized_range: start..end,
                scroll_offset: config.controller.offset(),
                viewport_extent: config.axis.main_extent(render.size),
                cache_extent: config.cache_extent,
                element_count: self.elements.len(),
                render_object_count: self.renders.len(),
                picture_layer_count: self.compositor.diagnostics().layers as usize,
            })
        })
    }
    #[must_use]
    pub fn diagnostics(&self) -> Diagnostics {
        self.diagnostics
    }
    /// Retained, renderer-independent semantic tree. It reflects meaningful
    /// controls, rather than paint commands or compositor pictures.
    #[must_use]
    pub fn semantics(&self) -> &SemanticsTree {
        &self.semantics
    }
    #[must_use]
    pub fn semantics_diagnostics(&self) -> SemanticsDiagnostics {
        self.semantics.diagnostics()
    }
    #[must_use]
    pub fn semantic_node_for_element(&self, element: ElementId) -> Option<SemanticNodeId> {
        self.semantic_ids.get(&element).copied()
    }
    #[must_use]
    pub fn element_for_semantic_node(&self, node: SemanticNodeId) -> Option<ElementId> {
        self.semantic_ids
            .iter()
            .find_map(|(element, current)| (*current == node).then_some(*element))
    }
    #[must_use]
    pub fn semantics_debug_dump(&self) -> String {
        self.semantics.debug_dump()
    }
    /// Bounded semantic diagnostics for in-app debuggers. The limit is
    /// applied before formatting labels or traversing descendants.
    #[must_use]
    pub fn semantics_debug_dump_bounded(&self, max_nodes: usize) -> String {
        self.semantics.debug_dump_bounded(max_nodes)
    }
    pub fn note_semantic_action(&mut self) {
        self.semantics.note_action();
    }
    pub fn take_unmounted(&mut self) -> Vec<ElementId> {
        std::mem::take(&mut self.unmounted)
    }
    #[must_use]
    pub fn element_exists(&self, id: ElementId) -> bool {
        self.elements.contains(id.0)
    }
    #[must_use]
    pub fn children(&self, id: ElementId) -> Option<&[ElementId]> {
        self.elements.get(id.0).map(|e| e.children.as_slice())
    }
    #[must_use]
    pub fn parent(&self, id: ElementId) -> Option<ElementId> {
        self.elements.get(id.0).and_then(|e| e.parent)
    }
    /// Records why an element is about to rebuild (DevTools builds only).
    #[cfg(feature = "devtools")]
    pub fn note_invalidation(&mut self, id: ElementId, cause: InvalidationCause) {
        if let Some(element) = self.elements.get_mut(id.0) {
            const MAX_CAUSES: usize = 8;
            if element.dev.invalidation_causes.len() == MAX_CAUSES {
                element.dev.invalidation_causes.remove(0);
            }
            element.dev.invalidation_causes.push(cause.clone());
            element.dev.last_cause = Some(cause);
        }
    }

    pub fn mark_build(&mut self, id: ElementId) -> Result<(), TreeError> {
        let element = self
            .elements
            .get_mut(id.0)
            .ok_or(TreeError::MissingElement(id))?;
        self.diagnostics.dirty_requests += 1;
        let was_dirty = element.dirty.contains(DirtyFlags::BUILD);
        element.dirty.insert(DirtyFlags::BUILD);
        if was_dirty {
            self.diagnostics.dirty_queue_deduplicated += 1;
        } else {
            self.diagnostics.dirty_queue_insertions += 1;
        }
        Ok(())
    }
    #[must_use]
    pub fn is_build_dirty(&self, id: ElementId) -> bool {
        self.elements
            .get(id.0)
            .is_some_and(|element| element.dirty.contains(DirtyFlags::BUILD))
    }
    #[must_use]
    pub fn render_id(&self, id: ElementId) -> Option<RenderObjectId> {
        self.elements.get(id.0).map(|e| e.render)
    }
    /// Returns the retained render kind for diagnostics and framework tooling.
    #[doc(hidden)]
    #[must_use]
    pub fn render_object_kind(&self, id: RenderObjectId) -> Option<&RenderKind> {
        self.renders.get(id.0).map(|render| &render.object.kind)
    }
    /// Returns the cached text layout for a retained text render object.
    #[doc(hidden)]
    #[must_use]
    pub fn text_layout(&self, id: RenderObjectId) -> Option<&TextLayout> {
        self.renders
            .get(id.0)
            .and_then(RenderNode::text_layout)
            .map(Arc::as_ref)
    }
    /// Current world-space bounds for a mounted element. This is useful for
    /// platform-neutral tooling and tests; it never exposes a render ID.
    #[must_use]
    pub fn element_bounds(&self, id: ElementId) -> Option<Rect> {
        let render = self.render_id(id)?;
        let size = self.renders.get(render.0)?.size;
        Some(
            self.render_world_transform(render)
                .transform_rect_bbox(Rect::from_origin_size(Offset::ZERO, size)),
        )
    }
    #[must_use]
    #[cfg(feature = "devtools")]
    pub(crate) fn dev_elements(&self) -> &Arena<Element> {
        &self.elements
    }

    /// Exact retained transforms for a live element, exposed only to the
    /// read-only DevTools snapshot adapter. Kurbo remains the geometry
    /// authority; this does not re-run layout or compositor work.
    #[must_use]
    #[cfg(feature = "devtools")]
    pub(crate) fn devtools_layout_transforms(
        &self,
        id: ElementId,
    ) -> Option<(CoreTransform, CoreTransform, CoreTransform)> {
        let render = self.render_id(id)?;
        let node = self.renders.get(render.0)?;
        Some((
            CoreTransform::translation(node.offset),
            self.render_world_transform(render),
            self.content_transform(render)
                .unwrap_or(CoreTransform::IDENTITY),
        ))
    }

    /// Line count from the existing Parley-backed retained result. It is
    /// intentionally an observation only: DevTools never asks the text
    /// engine to shape content for inspection.
    #[must_use]
    #[cfg(feature = "devtools")]
    pub(crate) fn devtools_text_line_count(&self, id: ElementId) -> Option<usize> {
        let render = self.render_id(id)?;
        Some(self.renders.get(render.0)?.text_layout()?.lines.len())
    }

    #[must_use]
    #[cfg(feature = "devtools")]
    pub(crate) fn devtools_is_layer_boundary(&self, id: ElementId) -> bool {
        self.render_id(id)
            .and_then(|render| self.renders.get(render.0))
            .is_some_and(|render| {
                render.object.layers.picture.is_some()
                    || render.object.layers.clip().is_some()
                    || render.object.layers.opacity().is_some()
                    || render.object.layers.blur().is_some()
                    || render.object.layers.shadow().is_some()
                    || render.object.layers.color_filter().is_some()
                    || render.object.layers.blend().is_some()
            })
    }

    /// Per-viewport variant of [`Self::sliver_viewport_diagnostics`], used by
    /// the selected-node Layout Explorer rather than a global first-match
    /// query. Values are retained by the live sliver viewport.
    #[must_use]
    #[cfg(feature = "devtools")]
    pub(crate) fn devtools_sliver_viewport_diagnostics(
        &self,
        id: ElementId,
    ) -> Option<SliverViewportDiagnostics> {
        let element = self.elements.get(id.0)?;
        let WidgetKind::SliverViewport { config } = element.widget.kind() else {
            return None;
        };
        let render = self.renders.get(element.render.0)?;
        let mut indices = element
            .sliver_child_ids
            .iter()
            .filter_map(|id| id.item_index());
        let first = indices.next();
        let (start, end) = first.map_or((0, 0), |first| {
            indices.fold((first, first + 1), |(min, max), index| {
                (min.min(index), max.max(index + 1))
            })
        });
        Some(SliverViewportDiagnostics {
            logical_item_count: config
                .delegate
                .child_count()
                .unwrap_or(element.sliver_child_ids.len()),
            materialized_item_count: element.sliver_child_ids.len(),
            materialized_range: start..end,
            scroll_offset: config.controller.offset(),
            viewport_extent: config.axis.main_extent(render.size),
            cache_extent: config.cache_extent,
            element_count: self.elements.len(),
            render_object_count: self.renders.len(),
            picture_layer_count: self.compositor.diagnostics().layers as usize,
        })
    }
    #[cfg(feature = "devtools")]
    pub(crate) fn dev_renders(&self) -> &Arena<RenderNode> {
        &self.renders
    }

    /// Applies a narrowly typed, temporary DevTools property override to a
    /// live retained element. Unsupported properties and stale IDs are
    /// rejected without mutating the tree.
    #[cfg(feature = "devtools")]
    pub fn devtools_edit_property(
        &mut self,
        target: DevWidgetId,
        name: &str,
        value: &DebugValue,
    ) -> bool {
        let Some(id) = self.devtools_resolve_id(target) else {
            return false;
        };
        let Some(value) = (match value {
            DebugValue::Float(value) if value.is_finite() => Some((*value as f32).clamp(0., 1.)),
            _ => None,
        }) else {
            return false;
        };
        let render = {
            let Some(element) = self.elements.get_mut(id.0) else {
                return false;
            };
            match (element.widget.kind_mut(), name) {
                (
                    WidgetKind::Opacity {
                        alpha,
                        controller: None,
                        ..
                    },
                    "opacity",
                ) => {
                    if *alpha == value {
                        return true;
                    }
                    *alpha = value;
                }
                _ => return false,
            }
            element.dev.revision = element.dev.revision.wrapping_add(1);
            element.dev.composite_reason = Some("DevTools opacity override".into());
            element.render
        };
        let opacity_layer = {
            let Some(render_node) = self.renders.get_mut(render.0) else {
                return false;
            };
            let RenderKind::Opacity {
                alpha,
                controller: None,
            } = &mut render_node.object.kind
            else {
                return false;
            };
            *alpha = value;
            render_node.object.layers.opacity()
        };
        if let Some(layer) = opacity_layer {
            let _ = self.compositor.update_opacity(layer, value);
        }
        self.diagnostics.compositor_only_updates =
            self.diagnostics.compositor_only_updates.wrapping_add(1);
        true
    }
    #[cfg(feature = "devtools")]
    pub(crate) fn dev_semantic_ids(&self) -> &HashMap<ElementId, SemanticNodeId> {
        &self.semantic_ids
    }
    #[cfg(feature = "devtools")]
    pub fn arena_index(&self, id: ElementId) -> u32 {
        id.0.index()
    }
    pub fn element_for_render(&self, render: RenderObjectId) -> Option<ElementId> {
        // Render IDs are opaque; a linear reverse lookup is only on input paths,
        // never layout/paint hot paths. A reverse arena index can be added when
        // profiling demonstrates it matters.
        self.elements
            .iter()
            .find_map(|(raw, element)| (element.render == render).then_some(ElementId(raw)))
    }
    #[must_use]
    pub fn action_for_element(&self, id: ElementId) -> Option<ActionId> {
        match self.elements.get(id.0)?.widget.kind() {
            WidgetKind::Button(spec) => Some(spec.action),
            _ => None,
        }
    }
    /// Returns an application-provided semantic action callback, if one was
    /// attached by [`Semantics`](crate::Semantics). The callback is cloned
    /// before the runtime invokes it, so it never runs while the tree is
    /// borrowed mutably.
    #[must_use]
    pub fn semantic_action_callback(
        &self,
        id: ElementId,
        action: SemanticActionKind,
    ) -> Option<Rc<dyn Fn() + 'static>> {
        self.elements.get(id.0).and_then(|element| {
            element
                .widget
                .semantic_properties()
                .callbacks
                .callback(action)
        })
    }
    #[must_use]
    pub fn action_ids(&self) -> HashSet<ActionId> {
        self.elements
            .iter()
            .flat_map(|(_, element)| match element.widget.kind() {
                WidgetKind::Button(spec) => [spec.action, spec.hover_action, spec.exit_action]
                    .map(|action| (action.0 != 0).then_some(action)),
                _ => [None; 3],
            })
            .flatten()
            .collect()
    }
    #[must_use]
    pub fn hover_actions_for_element(&self, id: ElementId) -> (Option<ActionId>, Option<ActionId>) {
        match &self.elements.get(id.0).map(|element| element.widget.kind()) {
            Some(WidgetKind::Button(spec)) => (
                (spec.hover_action.0 != 0).then_some(spec.hover_action),
                (spec.exit_action.0 != 0).then_some(spec.exit_action),
            ),
            _ => (None, None),
        }
    }
}

/// Widget key marking the performance-overlay mount point. Applications place
/// a placeholder with this key; `Runtime::install_performance_overlay`
/// (runtime crate) replaces it with the live overlay.
pub const PERFORMANCE_OVERLAY_KEY: &str = "incular-performance-overlay";

/// Mount-point placeholder for `Runtime::install_performance_overlay`
/// (runtime crate). Place this anywhere in an application tree; installing
/// swaps its contents for the live overlay while keeping the same top-level
/// widget kind so the retained update path stays compatible.
#[must_use]
pub fn performance_overlay_placeholder() -> Widget {
    // Keep the mount point itself non-interactive. The live overlay is
    // diagnostic chrome and must never prevent application widgets beneath it
    // from receiving pointer events.
    Widget::ignore_pointer(
        true,
        Widget::repaint_boundary(Widget::box_(Size::ZERO, incular_core::Color::TRANSPARENT)),
    )
    .with_key(Key::String(PERFORMANCE_OVERLAY_KEY.to_owned()))
}
