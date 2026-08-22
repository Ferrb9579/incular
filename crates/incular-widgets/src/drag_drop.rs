//! Typed, retained-local drag/drop bindings.
//!
//! A [`DragDropContext`] is generic over the payload and shared explicitly by
//! its [`Draggable`] sources and [`DragTarget`]s. There is no process-global
//! payload registry or untyped public `Any`: a target can only observe a
//! payload from the same typed context. A context exposes an optional
//! [`DragFeedback`] snapshot so an application can present it through its
//! existing `Overlay` while this crate remains navigation-independent.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use incular_core::Offset;

use crate::{GestureCallbacks, GestureRegion, Widget};

#[derive(Clone)]
pub struct DragFeedback {
    pub position: Offset,
    pub child: Widget,
}

struct DragState<T> {
    payload: Option<T>,
    position: Offset,
    feedback: Option<Widget>,
}

/// Typed local state shared by related drag sources and targets.
#[derive(Clone)]
pub struct DragDropContext<T> {
    state: Rc<RefCell<DragState<T>>>,
}
impl<T> Default for DragDropContext<T> {
    fn default() -> Self {
        Self::new()
    }
}
impl<T> DragDropContext<T> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Rc::new(RefCell::new(DragState {
                payload: None,
                position: Offset::ZERO,
                feedback: None,
            })),
        }
    }
    /// Returns an overlay-ready feedback snapshot during an active local drag.
    #[must_use]
    pub fn feedback(&self) -> Option<DragFeedback> {
        let state = self.state.borrow();
        state.feedback.clone().map(|child| DragFeedback {
            position: state.position,
            child,
        })
    }
    #[must_use]
    pub fn is_dragging(&self) -> bool {
        self.state.borrow().payload.is_some()
    }
    fn id(&self) -> usize {
        Rc::as_ptr(&self.state) as usize
    }
}

pub(crate) trait RetainedDragSource {
    fn context_id(&self) -> usize;
    fn start(&self, position: Offset);
    fn update(&self, position: Offset);
    fn finish(&self);
    fn cancel(&self);
}
pub(crate) trait RetainedDragTarget {
    fn context_id(&self) -> usize;
    fn enter(&self) -> bool;
    fn update(&self, position: Offset);
    fn leave(&self);
    fn drop_payload(&self);
}

/// Typed retained drag source. Its payload stays within its supplied
/// [`DragDropContext`], and may be previewed by an application overlay through
/// [`DragDropContext::feedback`].
#[derive(Clone)]
pub struct Draggable<T: Clone + 'static> {
    context: DragDropContext<T>,
    payload: T,
    child: Widget,
    feedback: Option<Rc<dyn Fn(T) -> Widget>>,
    on_start: Option<Rc<dyn Fn(T)>>,
    on_end: Option<Rc<dyn Fn(T)>>,
    on_cancel: Option<Rc<dyn Fn(T)>>,
}
impl<T: Clone + 'static> Draggable<T> {
    #[must_use]
    pub fn new(context: DragDropContext<T>, payload: T, child: impl Into<Widget>) -> Self {
        Self {
            context,
            payload,
            child: child.into(),
            feedback: None,
            on_start: None,
            on_end: None,
            on_cancel: None,
        }
    }
    #[must_use]
    pub fn feedback(mut self, builder: impl Fn(T) -> Widget + 'static) -> Self {
        self.feedback = Some(Rc::new(builder));
        self
    }
    #[must_use]
    pub fn on_start(mut self, callback: impl Fn(T) + 'static) -> Self {
        self.on_start = Some(Rc::new(callback));
        self
    }
    #[must_use]
    pub fn on_end(mut self, callback: impl Fn(T) + 'static) -> Self {
        self.on_end = Some(Rc::new(callback));
        self
    }
    #[must_use]
    pub fn on_cancel(mut self, callback: impl Fn(T) + 'static) -> Self {
        self.on_cancel = Some(Rc::new(callback));
        self
    }
}
struct DragSourceBinding<T: Clone + 'static> {
    context: DragDropContext<T>,
    payload: T,
    feedback: Option<Rc<dyn Fn(T) -> Widget>>,
    on_start: Option<Rc<dyn Fn(T)>>,
    on_end: Option<Rc<dyn Fn(T)>>,
    on_cancel: Option<Rc<dyn Fn(T)>>,
}
impl<T: Clone + 'static> RetainedDragSource for DragSourceBinding<T> {
    fn context_id(&self) -> usize {
        self.context.id()
    }
    fn start(&self, position: Offset) {
        let feedback = self
            .feedback
            .as_ref()
            .map(|builder| builder(self.payload.clone()));
        let mut state = self.context.state.borrow_mut();
        state.payload = Some(self.payload.clone());
        state.position = position;
        state.feedback = feedback;
        drop(state);
        if let Some(callback) = &self.on_start {
            callback(self.payload.clone());
        }
    }
    fn update(&self, position: Offset) {
        self.context.state.borrow_mut().position = position;
    }
    fn finish(&self) {
        let payload = self.context.state.borrow_mut().payload.take();
        self.context.state.borrow_mut().feedback = None;
        if let (Some(payload), Some(callback)) = (payload, &self.on_end) {
            callback(payload);
        }
    }
    fn cancel(&self) {
        let payload = self.context.state.borrow_mut().payload.take();
        self.context.state.borrow_mut().feedback = None;
        if let (Some(payload), Some(callback)) = (payload, &self.on_cancel) {
            callback(payload);
        }
    }
}
impl<T: Clone + 'static> From<Draggable<T>> for Widget {
    fn from(value: Draggable<T>) -> Self {
        Widget::draggable(
            Rc::new(DragSourceBinding {
                context: value.context,
                payload: value.payload,
                feedback: value.feedback,
                on_start: value.on_start,
                on_end: value.on_end,
                on_cancel: value.on_cancel,
            }),
            value.child,
        )
    }
}

/// Typed drop target for the same local [`DragDropContext`] as its sources.
#[derive(Clone)]
pub struct DragTarget<T: Clone + 'static> {
    context: DragDropContext<T>,
    child: Widget,
    on_will_accept: Option<Rc<dyn Fn(T) -> bool>>,
    on_enter: Option<Rc<dyn Fn(T)>>,
    on_leave: Option<Rc<dyn Fn(T)>>,
    on_update: Option<Rc<dyn Fn(T, Offset)>>,
    on_drop: Option<Rc<dyn Fn(T)>>,
}
impl<T: Clone + 'static> DragTarget<T> {
    #[must_use]
    pub fn new(context: DragDropContext<T>, child: impl Into<Widget>) -> Self {
        Self {
            context,
            child: child.into(),
            on_will_accept: None,
            on_enter: None,
            on_leave: None,
            on_update: None,
            on_drop: None,
        }
    }
    #[must_use]
    pub fn on_will_accept(mut self, callback: impl Fn(T) -> bool + 'static) -> Self {
        self.on_will_accept = Some(Rc::new(callback));
        self
    }
    #[must_use]
    pub fn on_enter(mut self, callback: impl Fn(T) + 'static) -> Self {
        self.on_enter = Some(Rc::new(callback));
        self
    }
    #[must_use]
    pub fn on_leave(mut self, callback: impl Fn(T) + 'static) -> Self {
        self.on_leave = Some(Rc::new(callback));
        self
    }
    #[must_use]
    pub fn on_update(mut self, callback: impl Fn(T, Offset) + 'static) -> Self {
        self.on_update = Some(Rc::new(callback));
        self
    }
    #[must_use]
    pub fn on_drop(mut self, callback: impl Fn(T) + 'static) -> Self {
        self.on_drop = Some(Rc::new(callback));
        self
    }
}
struct DragTargetBinding<T: Clone + 'static> {
    context: DragDropContext<T>,
    on_will_accept: Option<Rc<dyn Fn(T) -> bool>>,
    on_enter: Option<Rc<dyn Fn(T)>>,
    on_leave: Option<Rc<dyn Fn(T)>>,
    on_update: Option<Rc<dyn Fn(T, Offset)>>,
    on_drop: Option<Rc<dyn Fn(T)>>,
}
impl<T: Clone + 'static> DragTargetBinding<T> {
    fn payload(&self) -> Option<T> {
        self.context.state.borrow().payload.clone()
    }
}
impl<T: Clone + 'static> RetainedDragTarget for DragTargetBinding<T> {
    fn context_id(&self) -> usize {
        self.context.id()
    }
    fn enter(&self) -> bool {
        let Some(payload) = self.payload() else {
            return false;
        };
        if !self
            .on_will_accept
            .as_ref()
            .is_none_or(|callback| callback(payload.clone()))
        {
            return false;
        }
        if let Some(callback) = &self.on_enter {
            callback(payload);
        }
        true
    }
    fn update(&self, position: Offset) {
        if let (Some(payload), Some(callback)) = (self.payload(), &self.on_update) {
            callback(payload, position);
        }
    }
    fn leave(&self) {
        if let (Some(payload), Some(callback)) = (self.payload(), &self.on_leave) {
            callback(payload);
        }
    }
    fn drop_payload(&self) {
        if let (Some(payload), Some(callback)) = (self.payload(), &self.on_drop) {
            callback(payload);
        }
    }
}
impl<T: Clone + 'static> From<DragTarget<T>> for Widget {
    fn from(value: DragTarget<T>) -> Self {
        Widget::drag_target(
            Rc::new(DragTargetBinding {
                context: value.context,
                on_will_accept: value.on_will_accept,
                on_enter: value.on_enter,
                on_leave: value.on_leave,
                on_update: value.on_update,
                on_drop: value.on_drop,
            }),
            value.child,
        )
    }
}

/// Axis on which a [`Dismissible`] can claim a drag sequence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DismissDirection {
    Horizontal,
    Vertical,
}

/// A one-shot, arena-backed dismiss interaction. Once its threshold is crossed
/// it invokes the callback; the declarative owner then removes or replaces the
/// widget. No fake imperative removal is performed by the retained tree.
#[derive(Clone)]
pub struct Dismissible {
    direction: DismissDirection,
    threshold: f32,
    child: Widget,
    on_dismiss: Rc<dyn Fn(DismissDirection)>,
}
impl Dismissible {
    #[must_use]
    pub fn new(
        direction: DismissDirection,
        child: impl Into<Widget>,
        on_dismiss: impl Fn(DismissDirection) + 'static,
    ) -> Self {
        Self {
            direction,
            threshold: 80.,
            child: child.into(),
            on_dismiss: Rc::new(on_dismiss),
        }
    }
    #[must_use]
    pub fn threshold(mut self, threshold: f32) -> Self {
        assert!(
            threshold.is_finite() && threshold > 0.,
            "dismiss threshold must be positive"
        );
        self.threshold = threshold;
        self
    }
}
impl From<Dismissible> for Widget {
    fn from(value: Dismissible) -> Self {
        let fired = Rc::new(Cell::new(false));
        let callback = value.on_dismiss.clone();
        let threshold = value.threshold;
        let direction = value.direction;
        let update = Rc::new(move |delta: Offset| {
            let distance = match direction {
                DismissDirection::Horizontal => delta.x.abs(),
                DismissDirection::Vertical => delta.y.abs(),
            };
            if distance >= threshold && !fired.replace(true) {
                callback(direction);
            }
        });
        let mut callbacks = GestureCallbacks::default();
        match value.direction {
            DismissDirection::Horizontal => callbacks.on_horizontal_drag_update = Some(update),
            DismissDirection::Vertical => callbacks.on_vertical_drag_update = Some(update),
        }
        GestureRegion::new(callbacks, value.child).into()
    }
}
