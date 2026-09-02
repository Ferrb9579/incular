//! Retained routing for data dragged in from the native platform.

use super::*;
use incular_platform::{ExternalDragEvent, ExternalDragPhase, ExternalDragResponse};

impl WidgetTree {
    fn external_drop_binding(&self, element: ElementId) -> Option<ExternalDropTargetBinding> {
        self.elements
            .get(element.0)?
            .environment_override
            .as_ref()?
            .value
            .downcast_ref::<ExternalDropTargetMarker>()
            .map(|marker| marker.binding.clone())
    }

    fn external_drop_target_ancestor(
        &self,
        mut element: ElementId,
    ) -> Option<(ElementId, ExternalDropTargetBinding)> {
        loop {
            if let Some(binding) = self.external_drop_binding(element) {
                return Some((element, binding));
            }
            element = self.parent(element)?;
        }
    }

    fn external_drop_target_at(
        &self,
        position: Offset,
    ) -> Option<(ElementId, ExternalDropTargetBinding)> {
        self.hit_test(position)
            .and_then(|render| self.element_for_render(render))
            .and_then(|element| self.external_drop_target_ancestor(element))
    }

    /// Routes one normalized external drag sample through hit testing without
    /// entering the local typed gesture/drag arena.
    ///
    /// Only operations explicitly advertised by the source can be returned.
    /// The active target stores retained identity rather than callbacks, so a
    /// compatible widget rebuild immediately replaces callback behavior while
    /// an unmounted target can never receive a later drop.
    #[must_use]
    pub fn dispatch_external_drag(&mut self, event: ExternalDragEvent) -> ExternalDragResponse {
        self.clear_stale_external_drop_target();

        if matches!(
            event.phase,
            ExternalDragPhase::Leave | ExternalDragPhase::Cancel
        ) {
            self.finish_external_drop_without_drop(&event);
            return ExternalDragResponse::default();
        }

        let candidate =
            self.external_drop_target_at(event.position)
                .and_then(|(element, binding)| {
                    binding
                        .negotiate(&event)
                        .map(|operation| (element, binding, operation))
                });

        let same_target = self.active_external_drop.as_ref().is_some_and(|active| {
            candidate
                .as_ref()
                .is_some_and(|(element, _, _)| active.element == *element)
        });

        if !same_target
            && let Some(active) = self.active_external_drop.take()
            && let Some(binding) = self.external_drop_binding(active.element)
        {
            let mut leave = event.clone();
            leave.phase = ExternalDragPhase::Leave;
            binding.leave(&leave);
        }

        let Some((element, binding, operation)) = candidate else {
            return ExternalDragResponse::default();
        };

        if !same_target {
            let mut enter = event.clone();
            enter.phase = ExternalDragPhase::Enter;
            binding.enter(&enter, operation);
            self.active_external_drop = Some(ActiveExternalDrop {
                element,
                operation,
                last_event: event.clone(),
            });
        } else if let Some(active) = self.active_external_drop.as_mut() {
            active.operation = operation;
            active.last_event = event.clone();
        }

        if matches!(event.phase, ExternalDragPhase::Over) {
            binding.update(&event, operation);
        }

        if matches!(event.phase, ExternalDragPhase::Drop) {
            self.active_external_drop = None;
            binding.drop_data(&event, operation);
        }

        ExternalDragResponse {
            requested_operation: Some(operation),
        }
    }

    fn finish_external_drop_without_drop(&mut self, event: &ExternalDragEvent) {
        let Some(active) = self.active_external_drop.take() else {
            return;
        };
        let Some(binding) = self.external_drop_binding(active.element) else {
            return;
        };
        match event.phase {
            ExternalDragPhase::Cancel => binding.cancel(event),
            ExternalDragPhase::Leave => binding.leave(event),
            ExternalDragPhase::Enter | ExternalDragPhase::Over | ExternalDragPhase::Drop => {}
        }
    }

    fn clear_stale_external_drop_target(&mut self) {
        if self
            .active_external_drop
            .as_ref()
            .is_some_and(|active| !self.elements.contains(active.element.0))
        {
            self.active_external_drop = None;
        }
    }

    pub(super) fn external_drop_target_unmounted(&mut self, element: ElementId) {
        if self
            .active_external_drop
            .as_ref()
            .is_none_or(|active| active.element != element)
        {
            return;
        }
        let Some(active) = self.active_external_drop.take() else {
            return;
        };
        let Some(binding) = self.external_drop_binding(element) else {
            return;
        };
        let mut cancel = active.last_event;
        cancel.phase = ExternalDragPhase::Cancel;
        binding.cancel(&cancel);
    }
}
