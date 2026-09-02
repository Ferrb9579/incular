//! Retained targets for data dragged in from other applications.
//!
//! This is intentionally separate from [`crate::DragDropContext`]. Local typed
//! drags retain Rust payloads inside one widget tree; external drops carry the
//! platform-neutral [`incular_platform::DataTransfer`] model and participate in
//! operating-system operation negotiation.

use incular_platform::{ExternalDragEvent, TransferOperation};
use std::rc::Rc;

use crate::Widget;

type AcceptCallback = Rc<dyn Fn(&ExternalDragEvent) -> Option<TransferOperation>>;
type OperationCallback = Rc<dyn Fn(&ExternalDragEvent, TransferOperation)>;
type EventCallback = Rc<dyn Fn(&ExternalDragEvent)>;

/// A hit-tested target for clipboard-like data dragged from another
/// application or native shell.
///
/// The target never sees the local typed drag payload registry. Returning an
/// operation that the source did not advertise rejects the target rather than
/// silently coercing it to another operation.
#[derive(Clone)]
pub struct ExternalDropTarget {
    child: Widget,
    on_will_accept: Option<AcceptCallback>,
    on_enter: Option<OperationCallback>,
    on_update: Option<OperationCallback>,
    on_leave: Option<EventCallback>,
    on_cancel: Option<EventCallback>,
    on_drop: Option<OperationCallback>,
}

impl ExternalDropTarget {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            on_will_accept: None,
            on_enter: None,
            on_update: None,
            on_leave: None,
            on_cancel: None,
            on_drop: None,
        }
    }

    /// Chooses the operation this target requests. `None` rejects the drag.
    /// Without a callback Incular requests the source's preferred advertised
    /// operation (copy, then move, then link).
    #[must_use]
    pub fn on_will_accept(
        mut self,
        callback: impl Fn(&ExternalDragEvent) -> Option<TransferOperation> + 'static,
    ) -> Self {
        self.on_will_accept = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn on_enter(
        mut self,
        callback: impl Fn(&ExternalDragEvent, TransferOperation) + 'static,
    ) -> Self {
        self.on_enter = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn on_update(
        mut self,
        callback: impl Fn(&ExternalDragEvent, TransferOperation) + 'static,
    ) -> Self {
        self.on_update = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn on_leave(mut self, callback: impl Fn(&ExternalDragEvent) + 'static) -> Self {
        self.on_leave = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn on_cancel(mut self, callback: impl Fn(&ExternalDragEvent) + 'static) -> Self {
        self.on_cancel = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn on_drop(
        mut self,
        callback: impl Fn(&ExternalDragEvent, TransferOperation) + 'static,
    ) -> Self {
        self.on_drop = Some(Rc::new(callback));
        self
    }
}

impl From<ExternalDropTarget> for Widget {
    fn from(value: ExternalDropTarget) -> Self {
        let binding = ExternalDropTargetBinding {
            on_will_accept: value.on_will_accept,
            on_enter: value.on_enter,
            on_update: value.on_update,
            on_leave: value.on_leave,
            on_cancel: value.on_cancel,
            on_drop: value.on_drop,
        };
        Widget::environment_scope(ExternalDropTargetMarker { binding }, value.child)
    }
}

#[derive(Clone)]
pub(crate) struct ExternalDropTargetMarker {
    pub(crate) binding: ExternalDropTargetBinding,
}

#[derive(Clone)]
pub(crate) struct ExternalDropTargetBinding {
    on_will_accept: Option<AcceptCallback>,
    on_enter: Option<OperationCallback>,
    on_update: Option<OperationCallback>,
    on_leave: Option<EventCallback>,
    on_cancel: Option<EventCallback>,
    on_drop: Option<OperationCallback>,
}

impl ExternalDropTargetBinding {
    pub(crate) fn negotiate(&self, event: &ExternalDragEvent) -> Option<TransferOperation> {
        let requested = self.on_will_accept.as_ref().map_or_else(
            || event.allowed_operations.preferred(),
            |callback| callback(event),
        );
        requested.filter(|operation| event.allowed_operations.contains(*operation))
    }

    pub(crate) fn enter(&self, event: &ExternalDragEvent, operation: TransferOperation) {
        if let Some(callback) = &self.on_enter {
            callback(event, operation);
        }
    }

    pub(crate) fn update(&self, event: &ExternalDragEvent, operation: TransferOperation) {
        if let Some(callback) = &self.on_update {
            callback(event, operation);
        }
    }

    pub(crate) fn leave(&self, event: &ExternalDragEvent) {
        if let Some(callback) = &self.on_leave {
            callback(event);
        }
    }

    pub(crate) fn cancel(&self, event: &ExternalDragEvent) {
        if let Some(callback) = &self.on_cancel {
            callback(event);
        }
    }

    pub(crate) fn drop_data(&self, event: &ExternalDragEvent, operation: TransferOperation) {
        if let Some(callback) = &self.on_drop {
            callback(event, operation);
        }
    }
}
