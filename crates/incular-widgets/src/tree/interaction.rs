//! Gesture, button, compositor, scrolling, and scrollbar interaction.

use super::rendering::{fitted_transform, transform_around, transform_around_alignment};
use super::*;

impl WidgetTree {
    /// Allocates an opaque callback action. The runtime owns dispatch, while
    /// the tree uses this shared sequence for lazy children mounted during
    /// layout so IDs can never collide with eagerly prepared buttons.
    pub fn allocate_action(&mut self) -> ActionId {
        let action = ActionId(self.next_action);
        self.next_action += 1;
        action
    }
    pub fn take_pending_handlers(&mut self) -> Vec<(ActionId, Rc<dyn Fn()>)> {
        std::mem::take(&mut self.pending_handlers)
    }
    #[must_use]
    pub fn action_ancestor(&self, mut id: ElementId) -> Option<(ElementId, ActionId)> {
        loop {
            if let Some(action) = self.action_for_element(id) {
                return (action.0 != 0).then_some((id, action));
            }
            id = self.parent(id)?;
        }
    }
    #[must_use]
    pub fn button_ancestor(&self, mut id: ElementId) -> Option<(ElementId, Option<ActionId>)> {
        loop {
            if let Some(WidgetKind::Button {
                action, enabled, ..
            }) = self.elements.get(id.0).map(|element| &element.widget.kind)
            {
                if !*enabled {
                    return None;
                }
                return Some((id, (action.0 != 0).then_some(*action)));
            }
            id = self.parent(id)?;
        }
    }
    /// Requests retained pointer capture for an already active gesture member.
    /// The returned token is window-local and is released automatically on up,
    /// cancellation, or unmount. This is the portable guarantee; platform
    /// adapters may additionally request native OS capture.
    pub fn request_pointer_capture(
        &mut self,
        window: u64,
        pointer: u64,
        element: ElementId,
    ) -> Option<PointerCapture> {
        let key = GestureArenaKey { window, pointer };
        let legacy_member = self.active_gestures.get(&key).is_some_and(|active| {
            active
                .members
                .iter()
                .any(|candidate| candidate.element == element)
        });
        let raw_member = self.raw_gesture_streams.get(&key).is_some_and(|active| {
            active
                .members
                .iter()
                .any(|candidate| candidate.element == element)
        });
        if !legacy_member && !raw_member {
            return None;
        }
        self.pointer_captures.insert(key, element);
        Some(PointerCapture { key })
    }
    /// Returns the retained target currently captured for a window/pointer.
    #[must_use]
    pub fn pointer_capture_target(&self, window: u64, pointer: u64) -> Option<ElementId> {
        self.pointer_captures
            .get(&GestureArenaKey { window, pointer })
            .copied()
    }
    /// Releases a capture token. Releasing a token from another window or a
    /// stale pointer sequence is harmless and returns `false`.
    pub fn release_pointer_capture(&mut self, capture: PointerCapture) -> bool {
        self.pointer_captures.remove(&capture.key).is_some()
    }
    /// Dispatches a pointer event in the standalone window (identity zero).
    /// Multi-window runtimes should use [`Self::dispatch_gesture_in_window`].
    pub fn dispatch_gesture(&mut self, event: PointerEvent) -> Option<ElementId> {
        self.dispatch_gesture_in_window(0, event)
    }
    /// Dispatches a pointer event through the retained gesture arena for one
    /// window. Every hit-tested gesture ancestor joins the stream pending;
    /// callbacks run only after its recognizer wins (or is explicitly
    /// compatible with) that stream's arena.
    pub fn dispatch_gesture_in_window(
        &mut self,
        window: u64,
        event: PointerEvent,
    ) -> Option<ElementId> {
        use incular_core::PointerPhase;

        let key = GestureArenaKey {
            window,
            pointer: event.pointer,
        };
        if matches!(event.phase, PointerPhase::Down) {
            self.cancel_gesture_stream(key, true);
            let hit = self
                .hit_test(event.position)
                .and_then(|render| self.element_for_render(render))?;
            let elements = self.gesture_ancestors(hit);
            let element = *elements.first()?;
            let mut active = ActiveGesture {
                element,
                members: Vec::new(),
                cancelled_elements: HashSet::new(),
                start: event.position,
            };
            for candidate in elements {
                let Some(callbacks) = self.gesture_callbacks(candidate) else {
                    continue;
                };
                if callbacks.has_pointer_recognizer() {
                    let member = self.gesture_arena.add(key, false);
                    let mut recognizer = PointerGestureRecognizer::new(callbacks.clone());
                    let _ = recognizer.observe(event);
                    active.members.push(ActiveGestureMember {
                        element: candidate,
                        member,
                        kind: RetainedGestureKind::Pointer,
                        recognizer: Some(recognizer),
                        on_cancel: callbacks.on_cancel.clone(),
                    });
                }
                if callbacks.on_scale_update.is_some()
                    || callbacks.on_scale_start.is_some()
                    || callbacks.on_scale_end.is_some()
                {
                    let on_update = callbacks.on_scale_update.clone();
                    let on_start = callbacks.on_scale_start.clone();
                    let on_end = callbacks.on_scale_end.clone();
                    let member = self.gesture_arena.add(key, true);
                    let mut scale = self.scale_gestures.remove(&candidate).unwrap_or_else(|| {
                        ScaleGestureDetector::with_callbacks(on_update, on_start, on_end)
                    });
                    let _ = scale.observe(event);
                    self.scale_gestures.insert(candidate, scale);
                    active.members.push(ActiveGestureMember {
                        element: candidate,
                        member,
                        kind: RetainedGestureKind::Scale,
                        recognizer: None,
                        on_cancel: callbacks.on_cancel.clone(),
                    });
                }
            }
            // A plain hit-tested label must continue through the runtime's
            // ordinary pointer route (for buttons, editable fields, and
            // read-only text selection). Only actual recognizers create an
            // arena stream or retain pointer capture.
            if active.members.is_empty() {
                return None;
            }
            self.active_gestures.insert(key, active);
            self.pointer_captures.insert(key, element);
            let scale_elements = self
                .active_gestures
                .get(&key)
                .map(|active| {
                    active
                        .members
                        .iter()
                        .filter(|candidate| candidate.kind == RetainedGestureKind::Scale)
                        .map(|candidate| candidate.element)
                        .collect()
                })
                .unwrap_or_default();
            self.activate_scale_pairs(key.window, scale_elements);
            return Some(element);
        }

        let element = self.active_gestures.get(&key)?.element;
        if !self.elements.contains(element.0) {
            self.cancel_gesture_stream(key, true);
            return None;
        }
        let dispositions = self.gesture_arena.entries(key);
        let mut accepts = Vec::new();
        let mut rejects = Vec::new();
        let mut callbacks = Vec::new();
        let mut scale_updates = Vec::new();
        if let Some(active) = self.active_gestures.get_mut(&key) {
            for candidate in &mut active.members {
                let disposition = disposition_for(&dispositions, candidate.member);
                match candidate.kind {
                    RetainedGestureKind::Pointer => {
                        let Some(recognizer) = candidate.recognizer.as_mut() else {
                            continue;
                        };
                        match recognizer.observe(event) {
                            GestureDecision::Accept(action) => match disposition {
                                GestureDisposition::Accepted => {
                                    callbacks.push((candidate.member, action))
                                }
                                GestureDisposition::Pending => {
                                    accepts.push((candidate.member, action))
                                }
                                GestureDisposition::Rejected | GestureDisposition::Cancelled => {}
                            },
                            GestureDecision::Reject | GestureDecision::Cancelled
                                if disposition == GestureDisposition::Pending =>
                            {
                                rejects.push(candidate.member);
                            }
                            GestureDecision::Pending
                            | GestureDecision::Reject
                            | GestureDecision::Cancelled => {}
                        }
                    }
                    RetainedGestureKind::Scale => {
                        if let Some(scale) = self.scale_gestures.get_mut(&candidate.element) {
                            if let Some(details) = scale.observe(event)
                                && disposition == GestureDisposition::Accepted
                            {
                                scale_updates.push((candidate.element, details));
                            }
                        }
                    }
                }
            }
        }
        for member in rejects {
            self.gesture_arena.reject(key, member);
            self.apply_arena_entries(key, self.gesture_arena.entries(key));
        }
        for (member, action) in accepts {
            let entries = self.gesture_arena.accept(key, member);
            self.apply_arena_entries(key, entries);
            if disposition_for(&self.gesture_arena.entries(key), member)
                == GestureDisposition::Accepted
            {
                callbacks.push((member, action));
            }
        }
        for (member, action) in callbacks {
            self.dispatch_gesture_action(key, member, action);
        }
        for (scale_element, details) in scale_updates {
            if let Some(scale) = self.scale_gestures.get(&scale_element) {
                scale.dispatch(details);
            }
        }
        let handled = !self.gesture_arena.entries(key).is_empty();
        if matches!(event.phase, PointerPhase::Up | PointerPhase::Cancel) {
            self.finish_drag(
                key,
                matches!(event.phase, PointerPhase::Cancel),
                event.position,
            );
            self.cancel_gesture_stream(key, matches!(event.phase, PointerPhase::Cancel));
        } else {
            self.remove_scale_recognizers_when_idle();
        }
        handled.then_some(element)
    }

    pub(super) fn gesture_callbacks(&self, element: ElementId) -> Option<GestureCallbacks> {
        match &self.elements.get(element.0)?.widget.kind {
            WidgetKind::Gesture { callbacks, .. } => Some((**callbacks).clone()),
            WidgetKind::Draggable { .. } => Some(GestureCallbacks {
                on_pan_update: Some(Rc::new(|_| {})),
                ..GestureCallbacks::default()
            }),
            _ => None,
        }
    }
    pub(super) fn gesture_ancestors(&self, mut id: ElementId) -> Vec<ElementId> {
        let mut ancestors = Vec::new();
        loop {
            if self.elements.get(id.0).is_none() {
                return ancestors;
            }
            if self.elements.get(id.0).is_some_and(|element| {
                matches!(
                    element.widget.kind,
                    WidgetKind::Gesture { .. } | WidgetKind::Draggable { .. }
                )
            }) {
                ancestors.push(id);
            }
            let Some(parent) = self.parent(id) else {
                return ancestors;
            };
            id = parent;
        }
    }
    pub(super) fn activate_scale_pairs(&mut self, window: u64, elements: Vec<ElementId>) {
        for element in elements {
            let members: Vec<_> = self
                .active_gestures
                .iter()
                .filter(|(key, _)| key.window == window)
                .flat_map(|(key, active)| {
                    active.members.iter().filter_map(move |candidate| {
                        (candidate.element == element
                            && candidate.kind == RetainedGestureKind::Scale)
                            .then_some((*key, candidate.member))
                    })
                })
                .collect();
            if members.len() < 2 {
                continue;
            }
            for (key, member) in members {
                let entries = self.gesture_arena.accept(key, member);
                self.apply_arena_entries(key, entries);
            }
        }
    }
    pub(super) fn apply_arena_entries(
        &mut self,
        key: GestureArenaKey,
        entries: Vec<GestureArenaEntry>,
    ) {
        let mut callbacks = Vec::new();
        if let Some(active) = self.active_gestures.get_mut(&key) {
            for entry in entries {
                if !matches!(
                    entry.disposition,
                    GestureDisposition::Rejected | GestureDisposition::Cancelled
                ) {
                    continue;
                }
                let Some(candidate) = active
                    .members
                    .iter()
                    .find(|candidate| candidate.member == entry.member)
                else {
                    continue;
                };
                if active.cancelled_elements.insert(candidate.element) {
                    if let Some(callback) = &candidate.on_cancel {
                        callbacks.push(callback.clone());
                    }
                }
            }
        }
        for callback in callbacks {
            callback();
        }
    }
    pub(super) fn dispatch_gesture_action(
        &mut self,
        key: GestureArenaKey,
        member: GestureArenaMember,
        action: GestureAction,
    ) {
        self.update_drag_from_action(key, member, action);
        if let Some(recognizer) = self.active_gestures.get_mut(&key).and_then(|active| {
            active
                .members
                .iter_mut()
                .find(|candidate| candidate.member == member)
                .and_then(|candidate| candidate.recognizer.as_mut())
        }) {
            recognizer.dispatch(action);
        }
    }
    pub(super) fn update_drag_from_action(
        &mut self,
        key: GestureArenaKey,
        member: GestureArenaMember,
        action: GestureAction,
    ) {
        let (GestureAction::Pan(delta)
        | GestureAction::HorizontalDrag(delta)
        | GestureAction::VerticalDrag(delta)) = action
        else {
            return;
        };
        let Some((source, start)) = self.active_gestures.get(&key).and_then(|active| {
            active
                .members
                .iter()
                .find(|candidate| candidate.member == member)
                .and_then(|candidate| self.drag_source(candidate.element))
                .map(|source| (source, active.start))
        }) else {
            return;
        };
        let position = start + delta;
        self.active_drags.entry(key).or_insert_with(|| {
            source.start(start);
            ActiveDrag {
                source: source.clone(),
                target: None,
                start,
            }
        });
        source.update(position);
        self.update_drag_target(key, position);
    }
    pub(super) fn drag_source(&self, element: ElementId) -> Option<Rc<dyn RetainedDragSource>> {
        match &self.elements.get(element.0)?.widget.kind {
            WidgetKind::Draggable { source, .. } => Some(source.clone()),
            _ => None,
        }
    }
    pub(super) fn drag_target(&self, element: ElementId) -> Option<Rc<dyn RetainedDragTarget>> {
        match &self.elements.get(element.0)?.widget.kind {
            WidgetKind::DragTarget { target, .. } => Some(target.clone()),
            _ => None,
        }
    }
    pub(super) fn drag_target_ancestor(
        &self,
        mut element: ElementId,
    ) -> Option<(ElementId, Rc<dyn RetainedDragTarget>)> {
        loop {
            if let Some(target) = self.drag_target(element) {
                return Some((element, target));
            }
            element = self.parent(element)?;
        }
    }
    pub(super) fn update_drag_target(&mut self, key: GestureArenaKey, position: Offset) {
        let next = self
            .hit_test(position)
            .and_then(|render| self.element_for_render(render))
            .and_then(|element| self.drag_target_ancestor(element));
        let Some(active) = self.active_drags.get_mut(&key) else {
            return;
        };
        let source_context = active.source.context_id();
        let next = next.filter(|(_, target)| target.context_id() == source_context);
        if active.target.as_ref().map(|(element, _)| *element)
            == next.as_ref().map(|(element, _)| *element)
        {
            if let Some((_, target)) = &active.target {
                target.update(position);
            }
            return;
        }
        if let Some((_, target)) = active.target.take() {
            target.leave();
        }
        if let Some((element, target)) = next {
            if target.enter() {
                target.update(position);
                active.target = Some((element, target));
            }
        }
    }
    pub(super) fn finish_drag(&mut self, key: GestureArenaKey, cancelled: bool, position: Offset) {
        self.update_drag_target(key, position);
        let Some(active) = self.active_drags.remove(&key) else {
            return;
        };
        if cancelled {
            if let Some((_, target)) = active.target {
                target.leave();
            }
            active.source.cancel();
        } else {
            if let Some((_, target)) = active.target {
                target.drop_payload();
            }
            active.source.finish();
        }
    }
    pub(super) fn cancel_gesture_stream(&mut self, key: GestureArenaKey, notify: bool) {
        let position = self
            .active_drags
            .get(&key)
            .map_or(Offset::ZERO, |drag| drag.start);
        self.finish_drag(key, true, position);
        let entries = self.gesture_arena.cancel(key);
        if notify {
            self.apply_arena_entries(key, entries);
        }
        self.active_gestures.remove(&key);
        self.pointer_captures.remove(&key);
        self.remove_scale_recognizers_when_idle();
    }
    pub(super) fn remove_scale_recognizers_when_idle(&mut self) {
        self.scale_gestures.retain(|element, _| {
            self.active_gestures.values().any(|active| {
                active.members.iter().any(|candidate| {
                    candidate.element == *element && candidate.kind == RetainedGestureKind::Scale
                })
            })
        });
    }
    #[must_use]
    pub fn render_size(&self, id: RenderObjectId) -> Option<Size> {
        self.renders.get(id.0).map(|r| r.size)
    }
    /// Baseline in this render object's local logical coordinate system.
    #[must_use]
    pub fn baseline(&self, id: RenderObjectId) -> Option<f32> {
        self.renders.get(id.0).and_then(|render| render.baseline)
    }
    #[must_use]
    pub fn text_diagnostics(&self) -> TextDiagnostics {
        self.text_engine.diagnostics()
    }
    #[must_use]
    pub fn compositor_debug_tree(&self) -> String {
        self.compositor.debug_tree()
    }
    #[must_use]
    pub fn compositor_diagnostics(&self) -> incular_painting::CompositorDiagnostics {
        self.compositor.diagnostics()
    }
    #[must_use]
    pub fn button_state(&self, id: ElementId) -> Option<ButtonState> {
        self.render_id(id)
            .and_then(|render| self.renders.get(render.0))
            .map(|render| render.button_state)
    }
    pub fn set_button_state(&mut self, id: ElementId, state: ButtonState) -> Result<(), TreeError> {
        let render = self.render_id(id).ok_or(TreeError::MissingElement(id))?;
        let node = self.renders.get_mut(render.0).expect("live render");
        if matches!(node.kind, RenderKind::Button { .. }) && node.button_state != state {
            node.button_state = state;
            node.button_hovered = matches!(state, ButtonState::Hovered);
            node.button_pressed = matches!(state, ButtonState::Pressed);
            node.button_focused = matches!(state, ButtonState::Focused);
            node.dirty.insert(DirtyFlags::PAINT);
        }
        Ok(())
    }

    /// Updates retained hover, press, and focus bits independently. The
    /// legacy [`set_button_state`](Self::set_button_state) API remains a
    /// compatibility setter for callers that want an exclusive state.
    pub fn set_button_interaction(
        &mut self,
        id: ElementId,
        hovered: Option<bool>,
        pressed: Option<bool>,
        focused: Option<bool>,
    ) -> Result<(), TreeError> {
        let render = self.render_id(id).ok_or(TreeError::MissingElement(id))?;
        let node = self.renders.get_mut(render.0).expect("live render");
        if !matches!(node.kind, RenderKind::Button { .. }) {
            return Ok(());
        }
        if let Some(value) = hovered {
            node.button_hovered = value;
        }
        if let Some(value) = pressed {
            node.button_pressed = value;
        }
        if let Some(value) = focused {
            node.button_focused = value;
        }
        node.button_state = if node.button_pressed {
            ButtonState::Pressed
        } else if node.button_focused {
            ButtonState::Focused
        } else if node.button_hovered {
            ButtonState::Hovered
        } else {
            ButtonState::Normal
        };
        node.dirty.insert(DirtyFlags::PAINT);
        Ok(())
    }
    /// Changes only retained animation progression. The accumulated logical
    /// timestamp is preserved, so pausing or slowing an already-running
    /// controller never jumps it forward to wall-clock time.
    pub fn set_animation_time_scale(&mut self, scale: f32) {
        self.animation_time_scale = if scale.is_finite() {
            scale.clamp(0., 1.)
        } else {
            1.
        };
    }
    #[must_use]
    pub const fn animation_time_scale(&self) -> f32 {
        self.animation_time_scale
    }
    pub(super) fn animation_now(&mut self, real_now: Instant) -> Instant {
        let (last_real, last_animation) = self.animation_clock.unwrap_or((real_now, real_now));
        let elapsed = real_now.saturating_duration_since(last_real);
        let scaled = elapsed.mul_f32(self.animation_time_scale);
        let animation_now = last_animation.checked_add(scaled).unwrap_or(last_animation);
        self.animation_clock = Some((real_now, animation_now));
        animation_now
    }
    /// Applies only retained compositor properties. It never marks a render
    /// object for build, layout, or paint.
    pub fn update_compositor(&mut self, now: Instant) -> (bool, bool) {
        let _phase_guard = self.guard_phase_root(FramePhase::Compositor);
        #[cfg(feature = "devtools")]
        let trace = self
            .root
            .and_then(|root| self.devtools_trace_begin_element(root, TracePhase::Composite));
        let now = self.animation_now(now);
        let nodes = self
            .renders
            .iter()
            .map(|(id, node)| {
                (
                    RenderObjectId(id),
                    node.kind.clone(),
                    node.content_layer,
                    node.opacity_layer,
                    node.blur_layer,
                    node.shadow_layer,
                    node.color_filter_layer,
                    node.blend_layer,
                    node.shader_mask_layer,
                    node.backdrop_filter_layer,
                    node.annotation_layer,
                    node.leader_layer,
                    node.follower_layer,
                )
            })
            .collect::<Vec<_>>();
        let mut changed = false;
        let mut active = false;
        for (
            _render,
            kind,
            content_layer,
            opacity_layer,
            blur_layer,
            shadow_layer,
            color_filter_layer,
            blend_layer,
            shader_mask_layer,
            backdrop_filter_layer,
            _annotation_layer,
            _leader_layer,
            _follower_layer,
        ) in nodes
        {
            let constraints = self
                .renders
                .get(_render.0)
                .and_then(|render| render.constraints);
            let _node_guard = self.guard_render(FramePhase::Compositor, _render, constraints);
            #[cfg(feature = "devtools")]
            let changed_before_node = changed;
            match kind {
                RenderKind::Scroll {
                    controller,
                    axis,
                    reverse,
                    ..
                } => {
                    if let Some(content) = content_layer
                        && self.compositor.update_transform(
                            content,
                            CoreTransform::translation(scroll_translation(
                                &controller,
                                axis,
                                reverse,
                            )),
                        )
                    {
                        changed = true;
                        self.diagnostics.compositor_only_updates += 1;
                        self.diagnostics.scroll_offset_updates += 1;
                        // Only this viewport's overlay picture changes; the
                        // retained content subtree remains compositor-only.
                        self.renders
                            .get_mut(_render.0)
                            .expect("live")
                            .dirty
                            .insert(DirtyFlags::PAINT);
                    }
                }
                RenderKind::SliverViewport { config } => {
                    if config.delegate.tick(now) {
                        changed = true;
                        self.diagnostics.animation_ticks += 1;
                        self.mark_render_dirty(
                            _render,
                            DirtyFlags::LAYOUT | DirtyFlags::PAINT,
                            true,
                        );
                    }
                    active |= config.delegate.is_animating();
                    if let Some(content) = content_layer
                        && self.compositor.update_transform(
                            content,
                            CoreTransform::translation(scroll_translation(
                                &config.controller,
                                config.axis,
                                config.reverse,
                            )),
                        )
                    {
                        changed = true;
                        self.diagnostics.compositor_only_updates += 1;
                        self.diagnostics.scroll_offset_updates += 1;
                        self.renders
                            .get_mut(_render.0)
                            .expect("live")
                            .dirty
                            .insert(DirtyFlags::PAINT);
                        let has_pinned_children = self
                            .element_for_render(_render)
                            .and_then(|element| self.elements.get(element.0))
                            .is_some_and(|element| !element.sliver_pinned_ids.is_empty());
                        if has_pinned_children {
                            // Pinned placement is part of sliver layout rather
                            // than the generic box transform. Queue one
                            // retained layout refresh so its push-away
                            // geometry follows the new scroll offset.
                            self.mark_render_dirty(_render, DirtyFlags::LAYOUT, true);
                            if let Some(constraints) = self
                                .renders
                                .get(_render.0)
                                .and_then(|render| render.constraints)
                            {
                                // Standalone WidgetTree users do not have a
                                // runtime layout phase between a controller
                                // jump and this compositor call. Run the
                                // already-known viewport layout now so pinned
                                // placement is observable immediately; the
                                // runtime reuses the cached result on its next
                                // layout.
                                self.layout_render(_render, constraints);
                            }
                        }
                    }
                }
                RenderKind::PersistentHeader {
                    controller,
                    axis,
                    reverse,
                    pinned,
                } => {
                    let offset = self.persistent_header_translation(
                        _render,
                        &controller,
                        axis,
                        reverse,
                        pinned,
                    );
                    if let Some(content) = content_layer
                        && self
                            .compositor
                            .update_transform(content, CoreTransform::translation(offset))
                    {
                        changed = true;
                        self.diagnostics.compositor_only_updates += 1;
                        self.diagnostics.scroll_offset_updates += 1;
                    }
                }
                RenderKind::Translate { controller } => {
                    if controller.tick(now) {
                        self.diagnostics.animation_ticks += 1;
                    }
                    active |= controller.is_active();
                    // Layout placement lives on `layer`; the inner retained
                    // transform carries only the compositor-only movement.
                    if let Some(content) = content_layer
                        && self.compositor.update_transform(
                            content,
                            CoreTransform::translation(controller.offset()),
                        )
                    {
                        changed = true;
                        self.diagnostics.compositor_only_updates += 1;
                    }
                }
                RenderKind::Transform { transform, origin } => {
                    if let Some(content) = content_layer {
                        let size = self.renders.get(_render.0).expect("live").size;
                        if self
                            .compositor
                            .update_transform(content, transform_around(transform, origin, size))
                        {
                            changed = true;
                            self.diagnostics.compositor_only_updates += 1;
                        }
                    }
                }
                RenderKind::Scale { controller, origin } => {
                    if controller.tick(now) {
                        self.diagnostics.animation_ticks += 1;
                    }
                    active |= controller.is_active();
                    if let Some(content) = content_layer {
                        let size = self.renders.get(_render.0).expect("live").size;
                        if self.compositor.update_transform(
                            content,
                            transform_around(
                                CoreTransform::scale(controller.scale()),
                                origin,
                                size,
                            ),
                        ) {
                            changed = true;
                            self.diagnostics.compositor_only_updates += 1;
                        }
                    }
                }
                RenderKind::Rotation {
                    controller,
                    origin,
                    alignment,
                } => {
                    if controller.tick(now) {
                        self.diagnostics.animation_ticks += 1;
                    }
                    active |= controller.is_active();
                    if let Some(content) = content_layer {
                        let size = self.renders.get(_render.0).expect("live").size;
                        if self.compositor.update_transform(
                            content,
                            transform_around_alignment(
                                CoreTransform::rotation(controller.radians()),
                                origin,
                                alignment,
                                size,
                            ),
                        ) {
                            changed = true;
                            self.diagnostics.compositor_only_updates += 1;
                        }
                    }
                }
                RenderKind::FittedBox { fit, alignment } => {
                    if let (Some(content), Some(&child)) = (
                        content_layer,
                        self.renders.get(_render.0).expect("live").children.first(),
                    ) {
                        let size = self.renders.get(_render.0).expect("live").size;
                        let child_size = self.renders.get(child.0).expect("live").size;
                        if self.compositor.update_transform(
                            content,
                            fitted_transform(child_size, size, fit, alignment),
                        ) {
                            changed = true;
                            self.diagnostics.compositor_only_updates += 1;
                        }
                    }
                }
                RenderKind::Opacity { alpha, controller } => {
                    if let Some(controller) = controller {
                        if controller.tick(now) {
                            self.diagnostics.animation_ticks += 1;
                        }
                        active |= controller.is_active();
                        if let Some(opacity) = opacity_layer
                            && self
                                .compositor
                                .update_opacity(opacity, controller.opacity())
                        {
                            changed = true;
                            self.diagnostics.compositor_only_updates += 1;
                        }
                    } else if let Some(opacity) = opacity_layer
                        && self.compositor.update_opacity(opacity, alpha)
                    {
                        changed = true;
                        self.diagnostics.compositor_only_updates += 1;
                    }
                }
                RenderKind::Blur {
                    sigma_x,
                    sigma_y,
                    controller,
                } => {
                    let mut sigma_x = sigma_x;
                    let mut sigma_y = sigma_y;
                    if let Some(controller) = controller {
                        if controller.tick(now) {
                            self.diagnostics.animation_ticks += 1;
                        }
                        active |= controller.is_active();
                        sigma_x = controller.sigma();
                        sigma_y = sigma_x;
                    }
                    if let Some(layer) = blur_layer
                        && self
                            .compositor
                            .update_blur(layer, GaussianBlur::new(sigma_x, sigma_y))
                    {
                        changed = true;
                        self.diagnostics.compositor_only_updates += 1;
                    }
                }
                RenderKind::DropShadow {
                    offset,
                    sigma_x,
                    sigma_y,
                    color,
                    controller,
                } => {
                    let mut shadow = DropShadowEffect::asymmetric(offset, sigma_x, sigma_y, color);
                    if let Some(controller) = controller {
                        if controller.tick(now) {
                            self.diagnostics.animation_ticks += 1;
                        }
                        active |= controller.is_active();
                        shadow = DropShadowEffect::new(
                            controller.offset(),
                            controller.sigma(),
                            controller.color(),
                        );
                    }
                    if let Some(layer) = shadow_layer
                        && self.compositor.update_drop_shadow(layer, shadow)
                    {
                        changed = true;
                        self.diagnostics.compositor_only_updates += 1;
                    }
                }
                RenderKind::ColorFiltered { filter, controller } => {
                    let mut filter = filter;
                    if let Some(controller) = controller {
                        if controller.tick(now) {
                            self.diagnostics.animation_ticks += 1;
                        }
                        active |= controller.is_active();
                        filter = controller.filter();
                    }
                    if let Some(layer) = color_filter_layer
                        && self.compositor.update_color_filter(layer, filter)
                    {
                        changed = true;
                        self.diagnostics.compositor_only_updates += 1;
                    }
                }
                RenderKind::Blend { mode } => {
                    if let Some(layer) = blend_layer
                        && self.compositor.update_blend(layer, mode)
                    {
                        changed = true;
                        self.diagnostics.compositor_only_updates += 1;
                    }
                }
                RenderKind::ShaderMask { blend_mode, .. } => {
                    if let Some(layer) = shader_mask_layer
                        && self.compositor.update_shader_mask_blend(layer, blend_mode)
                    {
                        changed = true;
                        self.diagnostics.compositor_only_updates += 1;
                    }
                }
                RenderKind::BackdropFilter {
                    blur,
                    blend_mode,
                    enabled,
                } => {
                    if let Some(layer) = backdrop_filter_layer
                        && self
                            .compositor
                            .update_backdrop_filter(layer, blur, blend_mode, enabled)
                    {
                        changed = true;
                        self.diagnostics.compositor_only_updates += 1;
                    }
                }
                _ => {}
            }
            #[cfg(feature = "devtools")]
            if changed != changed_before_node
                && let Some(element) = self.element_for_render(_render)
                && let Some(element) = self.elements.get_mut(element.0)
            {
                element.dev.composites += 1;
                element.dev.composite_reason =
                    Some("retained compositor property changed; paint reused".into());
            }
        }
        if !self.compositor_initialized {
            changed = true;
            self.diagnostics.compositor_only_updates += 1;
            self.compositor_initialized = true;
        }
        if changed {
            self.diagnostics.composites += 1;
        }
        #[cfg(feature = "devtools")]
        self.devtools_trace_end(trace);
        (changed, active)
    }
    pub fn scroll_at(&mut self, point: Offset, delta: Offset) -> bool {
        let Some(mut element) = self
            .hit_test(point)
            .and_then(|render| self.element_for_render(render))
        else {
            return false;
        };
        // The hit-tested leaf walks outward through its retained ancestors.
        // Passing the resulting innermost-first chain to the coordinator
        // transfers only boundary remainder to an outer scroll viewport.
        let mut viewports = Vec::new();
        loop {
            let render = self.elements.get(element.0).expect("live element").render;
            match &self.renders.get(render.0).expect("live render").kind {
                RenderKind::Scroll {
                    controller,
                    axis,
                    reverse,
                    physics,
                } => viewports.push((controller.clone(), *axis, *reverse, *physics)),
                RenderKind::SliverViewport { config } => viewports.push((
                    config.controller.clone(),
                    config.axis,
                    config.reverse,
                    config.physics,
                )),
                _ => {}
            }
            let Some(parent) = self.parent(element) else {
                break;
            };
            element = parent;
        }
        let Some((_, first_axis, _, _)) = viewports.first() else {
            return false;
        };
        let mut physical_remaining = scroll_delta_for_axis(*first_axis, delta);
        let mut consumed = 0.;
        for (controller, _axis, reverse, physics) in viewports {
            // Nested viewports normally share an axis. If an application
            // nests orthogonal viewports, the innermost hit-tested axis owns
            // the physical input and outer viewports receive its remainder.
            let logical = if reverse {
                -physical_remaining
            } else {
                physical_remaining
            };
            controller.notify_user_scroll(logical);
            let result = controller.apply_physics(physics, logical);
            // Pointer-wheel samples are complete user activities in this
            // high-level adapter. Drag recognizers can keep an activity open
            // by calling the controller's begin/end methods directly.
            controller.end_activity();
            let physical_consumed = if reverse {
                -result.consumed
            } else {
                result.consumed
            };
            consumed += physical_consumed;
            physical_remaining = if reverse {
                -result.unconsumed
            } else {
                result.unconsumed
            };
            if physical_remaining == 0. {
                break;
            }
        }
        if consumed != 0. {
            self.diagnostics.scroll_events += 1;
            true
        } else {
            false
        }
    }
    /// Applies a semantic page action to the controller owned by this viewport.
    pub fn semantic_scroll(&mut self, id: ElementId, forward: bool) -> bool {
        let Some(render) = self.render_id(id) else {
            return false;
        };
        let (controller, reverse, physics) = match &self.renders.get(render.0).expect("live").kind {
            RenderKind::Scroll {
                controller,
                axis: _,
                reverse,
                physics,
            } => (controller.clone(), *reverse, *physics),
            RenderKind::SliverViewport { config } => {
                (config.controller.clone(), config.reverse, config.physics)
            }
            _ => return false,
        };
        let page_extent = physics
            .snap_extent(controller.viewport_extent())
            .unwrap_or_else(|| controller.viewport_extent());
        let mut delta = if forward { page_extent } else { -page_extent };
        if reverse {
            delta = -delta;
        }
        let changed = controller.apply_physics(physics, delta).consumed != 0.;
        if changed {
            self.diagnostics.scroll_events += 1;
        }
        changed
    }
    /// Handles overlay scrollbar hit testing and capture. A thumb drag maps
    /// directly to the same controller used by wheel input; a track click
    /// pages one viewport toward the pointer.
    pub fn scrollbar_pointer(&mut self, phase: incular_core::PointerPhase, point: Offset) -> bool {
        match phase {
            incular_core::PointerPhase::Down => {
                let Some(render) = self.scrollbar_at(point) else {
                    return false;
                };
                let (controller, geometry) = self
                    .scrollbar_local_geometry(render)
                    .expect("scrollbar render");
                let point = self
                    .scrollbar_local_point(render, point)
                    .expect("invertible scrollbar");
                if geometry.thumb.contains(point) {
                    let grab = point.y - geometry.thumb.origin.y;
                    self.scrollbar_drag = Some(ScrollbarDrag {
                        render,
                        grab_offset: grab.clamp(0., geometry.thumb.size.height),
                    });
                    self.renders
                        .get_mut(render.0)
                        .expect("live")
                        .scrollbar_dragging = true;
                } else {
                    let delta = if point.y < geometry.thumb.origin.y {
                        -controller.viewport_extent()
                    } else {
                        controller.viewport_extent()
                    };
                    let _ = controller.scroll_by(delta);
                }
                self.renders
                    .get_mut(render.0)
                    .expect("live")
                    .dirty
                    .insert(DirtyFlags::PAINT);
                true
            }
            incular_core::PointerPhase::Move => {
                if let Some(drag) = self.scrollbar_drag {
                    let (controller, geometry) = self
                        .scrollbar_local_geometry(drag.render)
                        .expect("live drag");
                    let point = self
                        .scrollbar_local_point(drag.render, point)
                        .expect("invertible scrollbar");
                    let thumb_top = point.y - drag.grab_offset;
                    let offset = geometry.offset_for_thumb_top(thumb_top);
                    let changed = controller.jump_to(offset);
                    if changed {
                        self.diagnostics.scroll_events += 1;
                    }
                    self.renders
                        .get_mut(drag.render.0)
                        .expect("live")
                        .dirty
                        .insert(DirtyFlags::PAINT);
                    return true;
                }
                let hovered = self.scrollbar_at(point);
                let mut changed = false;
                let ids = self
                    .renders
                    .iter()
                    .map(|(raw, _)| RenderObjectId(raw))
                    .collect::<Vec<_>>();
                for render in ids {
                    let node = self.renders.get_mut(render.0).expect("live");
                    let is_hovered = hovered == Some(render);
                    if node.scrollbar_hovered != is_hovered {
                        node.scrollbar_hovered = is_hovered;
                        node.dirty.insert(DirtyFlags::PAINT);
                        changed = true;
                    }
                }
                changed || hovered.is_some()
            }
            incular_core::PointerPhase::Up | incular_core::PointerPhase::Cancel => {
                let Some(drag) = self.scrollbar_drag.take() else {
                    return false;
                };
                let node = self.renders.get_mut(drag.render.0).expect("live");
                node.scrollbar_dragging = false;
                node.dirty.insert(DirtyFlags::PAINT);
                true
            }
        }
    }
    #[must_use]
    pub fn scrollbar_diagnostics(&self) -> Vec<ScrollbarGeometry> {
        self.renders
            .iter()
            .filter_map(|(raw, _)| {
                self.scrollbar_controller_and_geometry(RenderObjectId(raw))
                    .map(|(_, geometry)| geometry)
            })
            .collect()
    }
    #[must_use]
    pub fn scrollbar_drag_diagnostics(&self) -> ScrollbarDragDiagnostics {
        let Some(drag) = self.scrollbar_drag else {
            return ScrollbarDragDiagnostics::default();
        };
        let Some((controller, geometry)) = self.scrollbar_controller_and_geometry(drag.render)
        else {
            return ScrollbarDragDiagnostics::default();
        };
        ScrollbarDragDiagnostics {
            active: true,
            grab_offset: drag.grab_offset,
            track_extent: geometry.track.size.height,
            thumb_extent: geometry.thumb.size.height,
            thumb_travel: geometry.thumb_travel,
            thumb_top: geometry.thumb.origin.y,
            scroll_offset: controller.offset(),
        }
    }
}
