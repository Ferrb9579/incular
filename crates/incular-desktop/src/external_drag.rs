use incular_core::Offset;
use incular_platform::{ExternalDragEvent, ExternalDragPhase};
use std::path::PathBuf;

/// Winit emits one path per hover/drop event. This state normalizes those
/// samples into one ordered multi-file transfer and delays drop completion
/// until the native event queue reaches `about_to_wait`.
#[doc(hidden)]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExternalFileDragState {
    hovered: Vec<PathBuf>,
    pending_drop: Vec<PathBuf>,
    active: bool,
    last_position: Offset,
}

impl ExternalFileDragState {
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active
    }

    #[must_use]
    pub fn current_position(&self) -> Option<Offset> {
        self.active.then_some(self.last_position)
    }

    #[must_use]
    pub fn hover(&mut self, path: PathBuf, position: Offset) -> ExternalDragEvent {
        let phase = if self.active {
            ExternalDragPhase::Over
        } else {
            ExternalDragPhase::Enter
        };
        self.active = true;
        self.last_position = position;
        push_unique(&mut self.hovered, path);
        ExternalDragEvent::files(phase, self.hovered.iter().cloned(), position)
    }

    #[must_use]
    pub fn over(&mut self, position: Offset) -> Option<ExternalDragEvent> {
        if !self.active || self.hovered.is_empty() {
            return None;
        }
        self.last_position = position;
        Some(ExternalDragEvent::files(
            ExternalDragPhase::Over,
            self.hovered.iter().cloned(),
            position,
        ))
    }

    pub fn queue_drop(&mut self, path: PathBuf, position: Offset) {
        self.active = true;
        self.last_position = position;
        push_unique(&mut self.pending_drop, path);
    }

    #[must_use]
    pub fn take_drop(&mut self) -> Option<ExternalDragEvent> {
        if self.pending_drop.is_empty() {
            return None;
        }
        let mut files = self.hovered.clone();
        for path in self.pending_drop.drain(..) {
            push_unique(&mut files, path);
        }
        let position = self.last_position;
        self.hovered.clear();
        self.active = false;
        Some(ExternalDragEvent::files(
            ExternalDragPhase::Drop,
            files,
            position,
        ))
    }

    /// Cancels an active hover. If native drop events are already queued, drop
    /// wins over a late hover-cancel notification and is finalized later.
    #[must_use]
    pub fn cancel(&mut self, position: Offset) -> Option<ExternalDragEvent> {
        if !self.pending_drop.is_empty() || !self.active {
            return None;
        }
        self.last_position = position;
        let files = std::mem::take(&mut self.hovered);
        self.active = false;
        (!files.is_empty())
            .then(|| ExternalDragEvent::files(ExternalDragPhase::Cancel, files, self.last_position))
    }
}

fn push_unique(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.contains(&path) {
        paths.push(path);
    }
}
