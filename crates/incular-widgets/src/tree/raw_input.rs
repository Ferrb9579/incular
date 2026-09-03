//! Retained routing for raw pointer widgets.

use std::collections::HashSet;

use crate::gestures::HitTestBehavior;
use incular_core::PointerPhase;
use incular_gestures::{
    ErasedGestureRecognizerFactory, GestureDecision, MouseCursor, PointerDeviceKind,
    RawPointerEvent,
};

use crate::raw_input::{
    ListenerCallbacks, MouseRegionCallbacks, RawInputKind, TapRegionCallbacks, TapRegionGroupId,
    WindowInteraction,
};

use super::*;

#[derive(Clone)]
struct TapRegistration {
    id: ElementId,
    group_id: Option<TapRegionGroupId>,
    callbacks: TapRegionCallbacks,
    consume_outside_taps: bool,
}

impl WidgetTree {
    pub(super) fn dispatch_raw_trackpad_gesture(&self, position: Offset, event: TrackpadGesture) {
        let callbacks = self
            .listener_ids(&self.raw_hit_elements(position))
            .into_iter()
            .filter_map(|id| self.listener_callbacks(id))
            .filter_map(|callbacks| callbacks.on_trackpad_gesture)
            .collect::<Vec<_>>();
        for callback in callbacks {
            callback(event);
        }
    }

    /// Dispatches one metadata-rich pointer event through the standalone
    /// retained window.
    pub fn dispatch_raw_pointer(&mut self, event: RawPointerEvent) -> Option<ElementId> {
        self.dispatch_raw_pointer_in_window(0, event)
    }

    /// Dispatches raw listeners, mouse annotations, tap-region boundaries,
    /// and raw recognizers in one retained window. The return value is only a
    /// target that intentionally consumes ordinary runtime pointer fallback;
    /// Listener and MouseRegion callbacks never suppress buttons or text.
    pub fn dispatch_raw_pointer_in_window(
        &mut self,
        window: u64,
        event: RawPointerEvent,
    ) -> Option<ElementId> {
        let key = GestureArenaKey::pointer_device(window, event.device, event.pointer);
        self.dispatch_mouse_regions(window, event);
        if matches!(event.phase, PointerPhase::Enter | PointerPhase::Exit) {
            return None;
        }
        self.dispatch_listeners(key, event);

        let tap_target = self.dispatch_tap_regions(event);
        if let Some(target) = tap_target {
            self.cancel_raw_gesture_stream(key, true);
            self.consumed_tap_pointers.insert(key);
            return Some(target);
        }
        if self.consumed_tap_pointers.contains(&key) {
            let primary_up = event.phase == PointerPhase::Up
                && (event.button.is_none()
                    || event.button == Some(incular_core::PRIMARY_POINTER_BUTTON));
            if event.phase == PointerPhase::Cancel || primary_up {
                self.consumed_tap_pointers.remove(&key);
            }
            return None;
        }
        self.dispatch_raw_gesture_stream(key, event)
    }

    /// Returns the cursor selected by the current raw hit route. A deferred
    /// cursor continues behind translucent/deferred regions, just as Flutter
    /// does for `MouseRegion` annotations.
    #[must_use]
    pub fn mouse_cursor_at(&self, point: Offset) -> MouseCursor {
        self.raw_hit_elements(point)
            .into_iter()
            .filter_map(|id| self.cursor_for_raw_input(id))
            .find(|cursor| *cursor != MouseCursor::Defer)
            .unwrap_or(MouseCursor::Defer)
    }

    fn cursor_for_raw_input(&self, element: ElementId) -> Option<MouseCursor> {
        match self.raw_input_kind(element)? {
            RawInputKind::MouseRegion { cursor, .. } => Some(*cursor),
            RawInputKind::WindowResizeRegion { direction } => Some(match direction {
                incular_core::WindowResizeDirection::East
                | incular_core::WindowResizeDirection::West => MouseCursor::ResizeHorizontal,
                incular_core::WindowResizeDirection::North
                | incular_core::WindowResizeDirection::South => MouseCursor::ResizeVertical,
                incular_core::WindowResizeDirection::NorthWest
                | incular_core::WindowResizeDirection::SouthEast => {
                    MouseCursor::ResizeUpLeftDownRight
                }
                incular_core::WindowResizeDirection::NorthEast
                | incular_core::WindowResizeDirection::SouthWest => {
                    MouseCursor::ResizeUpRightDownLeft
                }
            }),
            _ => None,
        }
    }

    #[must_use]
    pub fn cursor_at(&self, point: Offset) -> MouseCursor {
        self.mouse_cursor_at(point)
    }

    /// Whether a tap at `point` belongs to an enabled text-field tap-region
    /// group. Runtime focus fallback uses this to keep the current editor
    /// focused while a spinner, clear button, or other grouped control is
    /// pressed.
    #[must_use]
    pub fn preserves_text_field_focus(&self, point: Offset) -> bool {
        self.raw_hit_elements(point).into_iter().any(|id| {
            matches!(
                self.raw_input_kind(id),
                Some(RawInputKind::TextFieldTapRegion { enabled: true, .. })
            )
        })
    }

    /// Returns the deepest/frontmost custom-chrome interaction annotation at
    /// `point`. Interactive descendants are resolved separately by Runtime and
    /// take precedence before this annotation is acted upon.
    #[doc(hidden)]
    #[must_use]
    pub fn window_interaction_at(&self, point: Offset) -> Option<(ElementId, WindowInteraction)> {
        self.raw_hit_elements(point).into_iter().find_map(|id| {
            let interaction = match self.raw_input_kind(id)? {
                RawInputKind::WindowDragRegion => WindowInteraction::Move,
                RawInputKind::WindowResizeRegion { direction } => {
                    WindowInteraction::Resize(*direction)
                }
                _ => return None,
            };
            (!self.window_interaction_blocked_by_descendant(id, point)).then_some((id, interaction))
        })
    }

    fn window_interaction_blocked_by_descendant(&self, region: ElementId, point: Offset) -> bool {
        let Some(mut current) = self
            .hit_test(point)
            .and_then(|render| self.element_for_render(render))
        else {
            return true;
        };
        loop {
            if current == region {
                return false;
            }
            let Some(element) = self.elements.get(current.0) else {
                return true;
            };
            if window_chrome_blocks_at(element.widget.kind()) {
                return true;
            }
            let Some(parent) = element.parent else {
                // The raw annotation is not an ancestor of the visual target;
                // another frontmost branch owns the press.
                return true;
            };
            current = parent;
        }
    }

    pub(super) fn sync_raw_input_state(&mut self, element: ElementId) {
        let factories = self
            .raw_input_kind(element)
            .and_then(RawInputKind::factory_map)
            .map(ToOwned::to_owned);
        let Some(factories) = factories else {
            self.cancel_raw_streams_for_element(element);
            if let Some(mut recognizers) = self.raw_recognizers.remove(&element) {
                for recognizer in recognizers.values_mut() {
                    recognizer.dispose();
                }
            }
            return;
        };

        let next_types = factories
            .iter()
            .map(ErasedGestureRecognizerFactory::type_id)
            .collect::<HashSet<_>>();
        let streams_with_removed_recognizers = self
            .raw_gesture_streams
            .iter()
            .filter(|(_, active)| {
                active.members.iter().any(|member| {
                    member.element == element && !next_types.contains(&member.type_id)
                })
            })
            .map(|(key, _)| *key)
            .collect::<Vec<_>>();
        for key in streams_with_removed_recognizers {
            self.cancel_raw_gesture_stream(key, true);
        }

        let mut previous = self.raw_recognizers.remove(&element).unwrap_or_default();
        let mut next = std::collections::HashMap::new();
        let mut seen = HashSet::new();
        for factory in factories {
            if !seen.insert(factory.type_id()) {
                continue;
            }
            let type_id = factory.type_id();
            let mut recognizer = previous
                .remove(&type_id)
                .unwrap_or_else(|| factory.create());
            if factory.initialize(recognizer.as_mut()).is_ok() {
                next.insert(type_id, recognizer);
            } else {
                recognizer.dispose();
            }
        }
        for recognizer in previous.values_mut() {
            recognizer.dispose();
        }
        self.raw_recognizers.insert(element, next);
    }

    fn raw_input_kind(&self, element: ElementId) -> Option<&RawInputKind> {
        match self.elements.get(element.0)?.widget.kind() {
            WidgetKind::RawInput { kind, .. } => Some(kind),
            _ => None,
        }
    }

    fn listener_callbacks(&self, element: ElementId) -> Option<ListenerCallbacks> {
        match self.raw_input_kind(element)? {
            RawInputKind::Listener { callbacks, .. } => Some(callbacks.clone()),
            _ => None,
        }
    }

    fn mouse_region_config(
        &self,
        element: ElementId,
    ) -> Option<(
        MouseRegionCallbacks,
        MouseCursor,
        bool,
        Option<HitTestBehavior>,
    )> {
        match self.raw_input_kind(element)? {
            RawInputKind::MouseRegion {
                callbacks,
                cursor,
                opaque,
                behavior,
            } => Some((callbacks.clone(), *cursor, *opaque, *behavior)),
            _ => None,
        }
    }

    fn listener_ids(&self, elements: &[ElementId]) -> Vec<ElementId> {
        let mut seen = HashSet::new();
        elements
            .iter()
            .copied()
            .filter(|id| {
                seen.insert(*id)
                    && matches!(
                        self.raw_input_kind(*id),
                        Some(RawInputKind::Listener { .. })
                    )
            })
            .collect()
    }

    fn dispatch_listeners(&mut self, key: GestureArenaKey, event: RawPointerEvent) {
        let current = self.listener_ids(&self.raw_hit_elements(event.position));
        let hover_event = event.phase == PointerPhase::Move
            && event.buttons == 0
            && !self.raw_pointer_routes.contains_key(&key);
        let route = match event.phase {
            PointerPhase::Down => {
                if let Some(route) = self.raw_pointer_routes.get(&key) {
                    route.clone()
                } else {
                    self.raw_pointer_routes.insert(key, current.clone());
                    current.clone()
                }
            }
            PointerPhase::Move => self
                .raw_pointer_routes
                .get(&key)
                .cloned()
                .unwrap_or_default(),
            PointerPhase::Up if event.buttons != 0 => self
                .raw_pointer_routes
                .get(&key)
                .cloned()
                .unwrap_or_default(),
            PointerPhase::Up | PointerPhase::Cancel => {
                self.raw_pointer_routes.remove(&key).unwrap_or_default()
            }
            PointerPhase::Enter | PointerPhase::Exit => Vec::new(),
        };
        let callbacks = route
            .iter()
            .filter_map(|id| self.listener_callbacks(*id))
            .collect::<Vec<_>>();
        for callback in callbacks {
            match event.phase {
                PointerPhase::Down => {
                    if let Some(callback) = callback.on_pointer_down {
                        callback(event);
                    }
                }
                PointerPhase::Move => {
                    if let Some(callback) = callback.on_pointer_move {
                        callback(event);
                    }
                }
                PointerPhase::Up => {
                    if let Some(callback) = callback.on_pointer_up {
                        callback(event);
                    }
                }
                PointerPhase::Cancel => {
                    if let Some(callback) = callback.on_pointer_cancel {
                        callback(event);
                    }
                }
                PointerPhase::Enter | PointerPhase::Exit => {}
            }
        }
        if hover_event {
            for callback in current.iter().filter_map(|id| self.listener_callbacks(*id)) {
                if let Some(callback) = callback.on_pointer_hover {
                    callback(event);
                }
            }
        }
    }

    fn dispatch_mouse_regions(&mut self, window: u64, event: RawPointerEvent) {
        if !matches!(
            event.kind,
            PointerDeviceKind::Mouse
                | PointerDeviceKind::Trackpad
                | PointerDeviceKind::Stylus
                | PointerDeviceKind::InvertedStylus
        ) {
            return;
        }
        let key = GestureArenaKey::pointer_device(window, event.device, event.pointer);
        if event.phase == PointerPhase::Enter {
            // Winit's CursorEntered carries no position. Waiting for the first
            // CursorMoved avoids firing an enter callback for stale coordinates
            // from a previous surface visit.
            return;
        }
        if event.phase == PointerPhase::Exit {
            let previous = self.mouse_hover.remove(&key).unwrap_or_default();
            let exit_callbacks = previous
                .into_iter()
                .filter(|id| self.elements.contains(id.0))
                .filter_map(|id| {
                    self.mouse_region_config(id)
                        .and_then(|(callbacks, ..)| callbacks.on_exit)
                })
                .collect::<Vec<_>>();
            for callback in exit_callbacks {
                callback(event);
            }
            return;
        }
        let next = self
            .raw_hit_elements(event.position)
            .into_iter()
            .filter(|id| self.mouse_region_config(*id).is_some())
            .collect::<Vec<_>>();
        let previous = self
            .mouse_hover
            .insert(key, next.clone())
            .unwrap_or_default();
        let previous_live = previous
            .into_iter()
            .filter(|id| self.elements.contains(id.0))
            .collect::<Vec<_>>();
        let exits = previous_live
            .iter()
            .copied()
            .filter(|id| !next.contains(id))
            .collect::<Vec<_>>();
        let enters = next
            .iter()
            .copied()
            .filter(|id| !previous_live.contains(id))
            .collect::<Vec<_>>();
        let exit_callbacks = exits
            .iter()
            .filter_map(|id| {
                self.mouse_region_config(*id)
                    .and_then(|(callbacks, ..)| callbacks.on_exit)
            })
            .collect::<Vec<_>>();
        let enter_callbacks = enters
            .iter()
            .filter_map(|id| {
                self.mouse_region_config(*id)
                    .and_then(|(callbacks, ..)| callbacks.on_enter)
            })
            .collect::<Vec<_>>();
        for callback in exit_callbacks {
            callback(event);
        }
        for callback in enter_callbacks {
            callback(event);
        }
        if event.phase == PointerPhase::Move && event.buttons == 0 {
            let hover_callbacks = next
                .iter()
                .filter_map(|id| {
                    self.mouse_region_config(*id)
                        .and_then(|(callbacks, ..)| callbacks.on_hover)
                })
                .collect::<Vec<_>>();
            for callback in hover_callbacks {
                callback(event);
            }
        }
    }

    fn tap_region_registrations(&self, surface: ElementId) -> Vec<TapRegistration> {
        self.elements
            .iter()
            .filter_map(|(raw, element)| {
                let id = ElementId(raw);
                if self.nearest_tap_surface(id) != Some(surface) {
                    return None;
                }
                let (callbacks, group_id, consume_outside_taps) = match element.widget.kind() {
                    WidgetKind::RawInput {
                        kind:
                            RawInputKind::TapRegion {
                                callbacks,
                                enabled: true,
                                group_id,
                                consume_outside_taps,
                                ..
                            },
                        ..
                    } => (callbacks.clone(), group_id.clone(), *consume_outside_taps),
                    WidgetKind::RawInput {
                        kind:
                            RawInputKind::TextFieldTapRegion {
                                callbacks,
                                enabled: true,
                                group_id,
                                consume_outside_taps,
                                ..
                            },
                        ..
                    } => (
                        callbacks.clone(),
                        Some(group_id.clone()),
                        *consume_outside_taps,
                    ),
                    _ => return None,
                };
                Some(TapRegistration {
                    id,
                    group_id,
                    callbacks,
                    consume_outside_taps,
                })
            })
            .collect()
    }

    fn nearest_tap_surface(&self, element: ElementId) -> Option<ElementId> {
        let mut current = self.parent(element);
        while let Some(id) = current {
            if matches!(
                self.raw_input_kind(id),
                Some(RawInputKind::TapRegionSurface)
            ) {
                return Some(id);
            }
            current = self.parent(id);
        }
        None
    }

    fn dispatch_tap_regions(&self, event: RawPointerEvent) -> Option<ElementId> {
        if !matches!(event.phase, PointerPhase::Down | PointerPhase::Up) {
            return None;
        }
        // TapRegion is an ordinary primary-tap boundary, not a raw all-button
        // listener. Secondary/middle/extra transitions stay available through
        // Listener/RawGestureDetector and context-menu APIs without becoming a
        // synthetic primary tap. Legacy adapters without changed-button
        // metadata retain their historical primary semantics.
        if event.button.is_some() && event.button != Some(incular_core::PRIMARY_POINTER_BUTTON) {
            return None;
        }
        let hit = self.raw_hit_elements(event.position);
        let surfaces = hit
            .iter()
            .copied()
            .filter(|id| {
                matches!(
                    self.raw_input_kind(*id),
                    Some(RawInputKind::TapRegionSurface)
                )
            })
            .collect::<Vec<_>>();
        let mut consumed = None;
        for surface in surfaces {
            let registrations = self.tap_region_registrations(surface);
            if registrations.is_empty() {
                continue;
            }
            let hit_groups = registrations
                .iter()
                .filter(|registration| hit.contains(&registration.id))
                .filter_map(|registration| registration.group_id.clone())
                .collect::<HashSet<_>>();
            let inside = registrations
                .iter()
                .filter(|registration| {
                    hit.contains(&registration.id)
                        || registration
                            .group_id
                            .as_ref()
                            .is_some_and(|group| hit_groups.contains(group))
                })
                .collect::<Vec<_>>();
            let inside_ids = inside
                .iter()
                .map(|registration| registration.id)
                .collect::<HashSet<_>>();
            let outside = registrations
                .iter()
                .filter(|registration| !inside_ids.contains(&registration.id))
                .collect::<Vec<_>>();
            if event.phase == PointerPhase::Down {
                for registration in &inside {
                    if let Some(callback) = &registration.callbacks.on_tap_inside {
                        callback(event);
                    }
                }
                for registration in &outside {
                    if let Some(callback) = &registration.callbacks.on_tap_outside {
                        callback(event);
                    }
                    if consumed.is_none() && registration.consume_outside_taps {
                        consumed = Some(registration.id);
                    }
                }
            } else {
                for registration in &inside {
                    if let Some(callback) = &registration.callbacks.on_tap_up_inside {
                        callback(event);
                    }
                }
                for registration in &outside {
                    if let Some(callback) = &registration.callbacks.on_tap_up_outside {
                        callback(event);
                    }
                }
            }
        }
        consumed
    }

    fn raw_gesture_factories(&self, element: ElementId) -> Vec<ErasedGestureRecognizerFactory> {
        self.raw_input_kind(element)
            .and_then(RawInputKind::factory_map)
            .map_or_else(Vec::new, ToOwned::to_owned)
    }

    fn raw_gesture_elements(&self, point: Offset) -> Vec<ElementId> {
        let mut seen = HashSet::new();
        self.raw_hit_elements(point)
            .into_iter()
            .filter(|id| {
                seen.insert(*id)
                    && self
                        .raw_input_kind(*id)
                        .and_then(RawInputKind::factory_map)
                        .is_some_and(|factories| !factories.is_empty())
            })
            .collect()
    }

    fn dispatch_raw_gesture_stream(
        &mut self,
        key: GestureArenaKey,
        event: RawPointerEvent,
    ) -> Option<ElementId> {
        if event.phase == PointerPhase::Down && !self.raw_gesture_streams.contains_key(&key) {
            self.cancel_raw_gesture_stream(key, true);
            let elements = self.raw_gesture_elements(event.position);
            let element = elements.first().copied()?;
            let mut active = ActiveRawGesture {
                element,
                members: Vec::new(),
            };
            let mut rejected = Vec::new();
            for element in elements {
                let factories = self.raw_gesture_factories(element);
                for factory in factories {
                    let type_id = factory.type_id();
                    let Some(recognizer) = self
                        .raw_recognizers
                        .get_mut(&element)
                        .and_then(|recognizers| recognizers.get_mut(&type_id))
                    else {
                        continue;
                    };
                    let member = self.gesture_arena.add(key, false);
                    let decision = recognizer.observe_raw(event);
                    active.members.push(ActiveRawGestureMember {
                        element,
                        member,
                        type_id,
                        cancelled: false,
                    });
                    if matches!(
                        decision,
                        GestureDecision::Reject | GestureDecision::Cancelled
                    ) {
                        rejected.push(member);
                    }
                }
            }
            if active.members.is_empty() {
                let _ = self.gesture_arena.cancel(key);
                return None;
            }
            self.raw_gesture_streams.insert(key, active);
            self.pointer_captures.insert(key, element);
            for member in rejected {
                self.gesture_arena.reject(key, member);
            }
            return Some(element);
        }

        let active_members = self.raw_gesture_streams.get(&key).map(|active| {
            active
                .members
                .iter()
                .map(|member| (member.element, member.member, member.type_id))
                .collect::<Vec<_>>()
        })?;
        let dispositions = self.gesture_arena.entries(key);
        let mut accepts = Vec::new();
        let mut rejects = Vec::new();
        for (element, member, type_id) in active_members {
            if disposition_for(&dispositions, member) != GestureDisposition::Pending {
                continue;
            }
            let Some(recognizer) = self
                .raw_recognizers
                .get_mut(&element)
                .and_then(|recognizers| recognizers.get_mut(&type_id))
            else {
                rejects.push(member);
                continue;
            };
            match recognizer.observe_raw(event) {
                GestureDecision::Accept(action) => accepts.push((member, action)),
                GestureDecision::Reject | GestureDecision::Cancelled => rejects.push(member),
                GestureDecision::Pending => {}
            }
        }
        for member in rejects {
            self.gesture_arena.reject(key, member);
            self.apply_raw_arena_entries(key, self.gesture_arena.entries(key));
        }
        for (member, action) in accepts {
            let entries = self.gesture_arena.accept(key, member);
            self.apply_raw_arena_entries(key, entries);
            if disposition_for(&self.gesture_arena.entries(key), member)
                == GestureDisposition::Accepted
            {
                self.dispatch_raw_gesture_action(key, member, action);
            }
        }
        let element = self.raw_gesture_streams.get(&key)?.element;
        if event.phase == PointerPhase::Cancel
            || (event.phase == PointerPhase::Up && event.buttons == 0)
        {
            if event.phase == PointerPhase::Cancel {
                self.cancel_raw_gesture_stream(key, true);
            } else {
                self.remove_raw_gesture_stream(key);
            }
        }
        Some(element)
    }

    fn dispatch_raw_gesture_action(
        &mut self,
        key: GestureArenaKey,
        member: GestureArenaMember,
        action: GestureAction,
    ) {
        let Some((element, type_id)) = self.raw_gesture_streams.get(&key).and_then(|active| {
            active
                .members
                .iter()
                .find(|candidate| candidate.member == member)
                .map(|candidate| (candidate.element, candidate.type_id))
        }) else {
            return;
        };
        if let Some(recognizer) = self
            .raw_recognizers
            .get_mut(&element)
            .and_then(|recognizers| recognizers.get_mut(&type_id))
        {
            recognizer.dispatch(action);
        }
    }

    fn apply_raw_arena_entries(&mut self, key: GestureArenaKey, entries: Vec<GestureArenaEntry>) {
        let to_cancel = {
            let Some(active) = self.raw_gesture_streams.get_mut(&key) else {
                return;
            };
            entries
                .into_iter()
                .filter(|entry| {
                    matches!(
                        entry.disposition,
                        GestureDisposition::Rejected | GestureDisposition::Cancelled
                    )
                })
                .filter_map(|entry| {
                    let member = active
                        .members
                        .iter_mut()
                        .find(|member| member.member == entry.member)?;
                    if member.cancelled {
                        return None;
                    }
                    member.cancelled = true;
                    Some((member.element, member.type_id))
                })
                .collect::<Vec<_>>()
        };
        for (element, type_id) in to_cancel {
            if let Some(recognizer) = self
                .raw_recognizers
                .get_mut(&element)
                .and_then(|recognizers| recognizers.get_mut(&type_id))
            {
                recognizer.cancel();
            }
        }
    }

    pub(super) fn cancel_raw_gesture_stream(&mut self, key: GestureArenaKey, notify: bool) {
        let Some(active) = self.raw_gesture_streams.get(&key) else {
            return;
        };
        let members = active
            .members
            .iter()
            .map(|member| (member.element, member.type_id))
            .collect::<Vec<_>>();
        let element = active.element;
        let _ = self.gesture_arena.cancel(key);
        if notify {
            for (member_element, type_id) in members {
                if let Some(recognizer) = self
                    .raw_recognizers
                    .get_mut(&member_element)
                    .and_then(|recognizers| recognizers.get_mut(&type_id))
                {
                    recognizer.cancel();
                }
            }
        }
        self.raw_gesture_streams.remove(&key);
        if self.pointer_captures.get(&key) == Some(&element) {
            self.pointer_captures.remove(&key);
        }
    }

    fn cancel_raw_streams_for_element(&mut self, element: ElementId) {
        let streams = self
            .raw_gesture_streams
            .iter()
            .filter(|(_, active)| {
                active
                    .members
                    .iter()
                    .any(|member| member.element == element)
            })
            .map(|(key, _)| *key)
            .collect::<Vec<_>>();
        for key in streams {
            self.cancel_raw_gesture_stream(key, true);
        }
    }

    fn remove_raw_gesture_stream(&mut self, key: GestureArenaKey) {
        let Some(active) = self.raw_gesture_streams.remove(&key) else {
            return;
        };
        let _ = self.gesture_arena.cancel(key);
        if self.pointer_captures.get(&key) == Some(&active.element) {
            self.pointer_captures.remove(&key);
        }
    }

    pub(super) fn raw_input_unmounted(&mut self, element: ElementId) {
        self.cancel_raw_streams_for_element(element);
        self.raw_pointer_routes.retain(|_, route| {
            route.retain(|candidate| *candidate != element);
            !route.is_empty()
        });
        self.mouse_hover.retain(|_, hover| {
            hover.retain(|candidate| *candidate != element);
            !hover.is_empty()
        });
        if let Some(mut recognizers) = self.raw_recognizers.remove(&element) {
            for recognizer in recognizers.values_mut() {
                recognizer.dispose();
            }
        }
    }
}

fn window_chrome_blocks_at(kind: &WidgetKind) -> bool {
    match kind {
        WidgetKind::Button(_)
        | WidgetKind::SelectableText { .. }
        | WidgetKind::SelectionArea { .. }
        | WidgetKind::SelectionContainer { .. }
        | WidgetKind::TextField(_)
        | WidgetKind::Gesture { .. }
        | WidgetKind::Draggable { .. } => true,
        WidgetKind::AbsorbPointer { absorbing, .. } => *absorbing,
        WidgetKind::RawInput { kind, .. } => matches!(
            kind,
            RawInputKind::Listener { .. }
                | RawInputKind::RawGestureDetector { .. }
                | RawInputKind::TapRegion { .. }
                | RawInputKind::TextFieldTapRegion { .. }
        ),
        _ => false,
    }
}
