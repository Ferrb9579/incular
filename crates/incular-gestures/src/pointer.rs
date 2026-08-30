use std::time::{Duration, Instant};

use incular_core::{Offset, PointerPhase};

use crate::details::{
    DragCallbacks, DragDownDetails, DragEndDetails, DragStartDetails, DragUpdateDetails,
    GestureAction, GestureCallbacks, GestureDecision, LongPressEndDetails,
    LongPressMoveUpdateDetails, LongPressStartDetails, PointerDeviceKind, PointerEvent,
    TapDownDetails, TapUpDetails, Velocity,
};
use crate::raw::GestureRecognizer;

impl GestureCallbacks {
    /// Whether this callback set contributes an exclusive single-pointer
    /// recognizer to a gesture arena. Scale is intentionally separate so a
    /// pending pinch can coexist until one recognizer claims the stream.
    #[must_use]
    pub fn has_pointer_recognizer(&self) -> bool {
        self.on_tap.is_some()
            || self.on_tap_down.is_some()
            || self.on_tap_up.is_some()
            || self.on_tap_cancel.is_some()
            || self.on_double_tap.is_some()
            || self.on_double_tap_down.is_some()
            || self.on_double_tap_cancel.is_some()
            || self.on_long_press.is_some()
            || self.on_long_press_start.is_some()
            || self.on_long_press_move_update.is_some()
            || self.on_long_press_up.is_some()
            || self.on_long_press_end.is_some()
            || self.on_pan_down.is_some()
            || self.on_pan_start.is_some()
            || self.on_pan_update.is_some()
            || self.on_pan_end.is_some()
            || self.on_pan_cancel.is_some()
            || self.on_horizontal_drag_down.is_some()
            || self.on_horizontal_drag_start.is_some()
            || self.on_horizontal_drag_update.is_some()
            || self.on_horizontal_drag_end.is_some()
            || self.on_horizontal_drag_cancel.is_some()
            || self.on_vertical_drag_down.is_some()
            || self.on_vertical_drag_start.is_some()
            || self.on_vertical_drag_update.is_some()
            || self.on_vertical_drag_end.is_some()
            || self.on_vertical_drag_cancel.is_some()
    }
}

/// A reusable recognizer for tap, double-tap, long-press, and pan.
pub struct PointerGestureRecognizer {
    callbacks: GestureCallbacks,
    kind: PointerDeviceKind,
    down: Option<PointerEvent>,
    last_event: Option<PointerEvent>,
    last_tap: Option<Instant>,
    pan_started: bool,
    pan_action: Option<PanAction>,
    long_press_started: bool,
}

/// Legacy direct-use gesture detector.
///
/// The retained widget path uses [`PointerGestureRecognizer`] directly. This
/// small owning wrapper keeps the old convenience API concrete while
/// `GestureRecognizer` itself is now a real object-safe trait for raw factory
/// erasure.
pub struct GestureDetector {
    recognizer: PointerGestureRecognizer,
}

#[derive(Clone, Copy)]
enum PanAction {
    Pan,
    Horizontal,
    Vertical,
}
impl PointerGestureRecognizer {
    pub const DOUBLE_TAP_TIMEOUT: Duration = Duration::from_millis(300);
    pub const LONG_PRESS_TIMEOUT: Duration = Duration::from_millis(500);
    pub const PAN_SLOP: f32 = 18.;
    #[must_use]
    pub fn new(callbacks: GestureCallbacks) -> Self {
        Self {
            callbacks,
            kind: PointerDeviceKind::Mouse,
            down: None,
            last_event: None,
            last_tap: None,
            pan_started: false,
            pan_action: None,
            long_press_started: false,
        }
    }
    /// Observes an event without invoking user callbacks. This lets a
    /// retained arena delay observable behavior until it has accepted this
    /// recognizer. Direct users should normally keep using [`Self::handle`].
    pub fn observe(&mut self, event: PointerEvent) -> GestureDecision {
        match event.phase {
            PointerPhase::Down => {
                let is_double_tap = self.last_tap.is_some_and(|tap| {
                    event.time.saturating_duration_since(tap) <= Self::DOUBLE_TAP_TIMEOUT
                });
                if !is_double_tap {
                    self.last_tap = None;
                }
                self.down = Some(event);
                self.last_event = Some(event);
                self.pan_started = false;
                self.pan_action = None;
                self.long_press_started = false;
                if let Some(callback) = &self.callbacks.on_tap_down {
                    callback(TapDownDetails {
                        global_position: event.position,
                        local_position: event.position,
                        kind: self.kind,
                    });
                }
                if is_double_tap {
                    if let Some(callback) = &self.callbacks.on_double_tap_down {
                        callback(TapDownDetails {
                            global_position: event.position,
                            local_position: event.position,
                            kind: self.kind,
                        });
                    }
                }
                let pan_down = DragDownDetails {
                    global_position: event.position,
                    local_position: event.position,
                };
                if let Some(callback) = &self.callbacks.on_pan_down {
                    callback(pan_down);
                }
                if let Some(callback) = &self.callbacks.on_horizontal_drag_down {
                    callback(pan_down);
                }
                if let Some(callback) = &self.callbacks.on_vertical_drag_down {
                    callback(pan_down);
                }
                GestureDecision::Pending
            }
            PointerPhase::Move => {
                let Some(down) = self.down else {
                    return GestureDecision::Reject;
                };
                let delta = event.position - down.position;
                self.last_event = Some(event);
                if !self.pan_started
                    && !self.long_press_started
                    && event.time.saturating_duration_since(down.time) >= Self::LONG_PRESS_TIMEOUT
                {
                    self.long_press_started = true;
                    if let Some(callback) = &self.callbacks.on_long_press_start {
                        callback(LongPressStartDetails {
                            global_position: down.position,
                            local_position: down.position,
                        });
                    }
                }
                if self.long_press_started && !self.pan_started {
                    if let Some(callback) = &self.callbacks.on_long_press_move_update {
                        callback(LongPressMoveUpdateDetails {
                            global_position: event.position,
                            local_position: event.position,
                            offset_from_origin: delta,
                            local_offset_from_origin: delta,
                        });
                    }
                }
                if !self.pan_started && delta.x.hypot(delta.y) >= Self::PAN_SLOP {
                    self.pan_action = if delta.x.abs() >= delta.y.abs() {
                        (self.callbacks.on_horizontal_drag_down.is_some()
                            || self.callbacks.on_horizontal_drag_start.is_some()
                            || self.callbacks.on_horizontal_drag_update.is_some()
                            || self.callbacks.on_horizontal_drag_end.is_some()
                            || self.callbacks.on_horizontal_drag_cancel.is_some())
                        .then_some(PanAction::Horizontal)
                        .or_else(|| {
                            (self.callbacks.on_pan_down.is_some()
                                || self.callbacks.on_pan_start.is_some()
                                || self.callbacks.on_pan_update.is_some()
                                || self.callbacks.on_pan_end.is_some()
                                || self.callbacks.on_pan_cancel.is_some())
                            .then_some(PanAction::Pan)
                        })
                    } else {
                        (self.callbacks.on_vertical_drag_down.is_some()
                            || self.callbacks.on_vertical_drag_start.is_some()
                            || self.callbacks.on_vertical_drag_update.is_some()
                            || self.callbacks.on_vertical_drag_end.is_some()
                            || self.callbacks.on_vertical_drag_cancel.is_some())
                        .then_some(PanAction::Vertical)
                        .or_else(|| {
                            (self.callbacks.on_pan_down.is_some()
                                || self.callbacks.on_pan_start.is_some()
                                || self.callbacks.on_pan_update.is_some()
                                || self.callbacks.on_pan_end.is_some()
                                || self.callbacks.on_pan_cancel.is_some())
                            .then_some(PanAction::Pan)
                        })
                    };
                    let start = DragStartDetails {
                        global_position: down.position,
                        local_position: down.position,
                    };
                    if let Some(callback) = &self.callbacks.on_pan_start {
                        callback(start);
                    }
                    match self.pan_action {
                        Some(PanAction::Horizontal) => {
                            if let Some(callback) = &self.callbacks.on_horizontal_drag_start {
                                callback(start);
                            }
                        }
                        Some(PanAction::Vertical) => {
                            if let Some(callback) = &self.callbacks.on_vertical_drag_start {
                                callback(start);
                            }
                        }
                        Some(PanAction::Pan) | None => {}
                    }
                    self.pan_started = self.pan_action.is_some();
                    if self.pan_action.is_none() {
                        return GestureDecision::Reject;
                    }
                }
                match self.pan_action {
                    Some(PanAction::Pan) => GestureDecision::Accept(GestureAction::Pan(delta)),
                    Some(PanAction::Horizontal) => {
                        GestureDecision::Accept(GestureAction::HorizontalDrag(delta))
                    }
                    Some(PanAction::Vertical) => {
                        GestureDecision::Accept(GestureAction::VerticalDrag(delta))
                    }
                    None => GestureDecision::Pending,
                }
            }
            PointerPhase::Up => {
                let Some(down) = self.down.take() else {
                    return GestureDecision::Reject;
                };
                if self.pan_started {
                    let previous = self.last_event.take().unwrap_or(down);
                    let elapsed = event
                        .time
                        .saturating_duration_since(previous.time)
                        .as_secs_f32();
                    let delta = event.position - previous.position;
                    let details = DragEndDetails {
                        total_delta: event.position - down.position,
                        velocity: if elapsed > 0.0 {
                            Offset::new(delta.x / elapsed, delta.y / elapsed)
                        } else {
                            Offset::ZERO
                        },
                        cancelled: false,
                    };
                    let action = match self.pan_action.take() {
                        Some(PanAction::Pan) => self
                            .callbacks
                            .on_pan_end
                            .as_ref()
                            .map(|_| GestureAction::PanEnd(details)),
                        Some(PanAction::Horizontal) => self
                            .callbacks
                            .on_horizontal_drag_end
                            .as_ref()
                            .map(|_| GestureAction::HorizontalDragEnd(details)),
                        Some(PanAction::Vertical) => self
                            .callbacks
                            .on_vertical_drag_end
                            .as_ref()
                            .map(|_| GestureAction::VerticalDragEnd(details)),
                        None => None,
                    };
                    self.pan_started = false;
                    return action.map_or(GestureDecision::Pending, GestureDecision::Accept);
                }
                self.last_event = None;
                let elapsed = event.time.saturating_duration_since(down.time);
                if elapsed >= Self::LONG_PRESS_TIMEOUT {
                    if !self.long_press_started {
                        self.long_press_started = true;
                        if let Some(callback) = &self.callbacks.on_long_press_start {
                            callback(LongPressStartDetails {
                                global_position: down.position,
                                local_position: down.position,
                            });
                        }
                    }
                    if let Some(callback) = &self.callbacks.on_long_press_up {
                        callback();
                    }
                    if let Some(callback) = &self.callbacks.on_long_press_end {
                        callback(LongPressEndDetails {
                            global_position: event.position,
                            local_position: event.position,
                            velocity: Velocity::ZERO,
                        });
                    }
                    self.long_press_started = false;
                    if self.callbacks.on_long_press.is_some()
                        || self.callbacks.on_long_press_start.is_some()
                        || self.callbacks.on_long_press_move_update.is_some()
                        || self.callbacks.on_long_press_up.is_some()
                        || self.callbacks.on_long_press_end.is_some()
                    {
                        GestureDecision::Accept(GestureAction::LongPress)
                    } else {
                        GestureDecision::Reject
                    }
                } else if self.last_tap.is_some_and(|tap| {
                    event.time.saturating_duration_since(tap) <= Self::DOUBLE_TAP_TIMEOUT
                }) && self.callbacks.on_double_tap.is_some()
                {
                    if let Some(callback) = &self.callbacks.on_tap_up {
                        callback(TapUpDetails {
                            global_position: event.position,
                            local_position: event.position,
                            kind: self.kind,
                        });
                    }
                    self.last_tap = None;
                    GestureDecision::Accept(GestureAction::DoubleTap)
                } else if self.callbacks.on_tap.is_some()
                    || self.callbacks.on_tap_up.is_some()
                    || self.callbacks.on_tap_cancel.is_some()
                {
                    if let Some(callback) = &self.callbacks.on_tap_up {
                        callback(TapUpDetails {
                            global_position: event.position,
                            local_position: event.position,
                            kind: self.kind,
                        });
                    }
                    self.last_tap = Some(event.time);
                    GestureDecision::Accept(GestureAction::Tap)
                } else {
                    GestureDecision::Reject
                }
            }
            PointerPhase::Cancel => {
                if self.down.take().is_some() {
                    self.last_event = None;
                    self.pan_started = false;
                    self.pan_action = None;
                    self.long_press_started = false;
                    GestureDecision::Cancelled
                } else {
                    GestureDecision::Reject
                }
            }
        }
    }
    /// Invokes a callback selected by [`Self::observe`] after the caller has
    /// accepted this recognizer.
    pub fn dispatch(&self, action: GestureAction) {
        match action {
            GestureAction::Tap => self.callbacks.on_tap.as_ref().map(|callback| callback()),
            GestureAction::DoubleTap => self
                .callbacks
                .on_double_tap
                .as_ref()
                .map(|callback| callback()),
            GestureAction::LongPress => self
                .callbacks
                .on_long_press
                .as_ref()
                .map(|callback| callback()),
            GestureAction::Pan(delta) => self
                .callbacks
                .on_pan_update
                .as_ref()
                .map(|callback| callback(delta)),
            GestureAction::PanEnd(details) => self
                .callbacks
                .on_pan_end
                .as_ref()
                .map(|callback| callback(details)),
            GestureAction::HorizontalDrag(delta) => self
                .callbacks
                .on_horizontal_drag_update
                .as_ref()
                .map(|callback| callback(delta)),
            GestureAction::HorizontalDragEnd(details) => self
                .callbacks
                .on_horizontal_drag_end
                .as_ref()
                .map(|callback| callback(details)),
            GestureAction::VerticalDrag(delta) => self
                .callbacks
                .on_vertical_drag_update
                .as_ref()
                .map(|callback| callback(delta)),
            GestureAction::VerticalDragEnd(details) => self
                .callbacks
                .on_vertical_drag_end
                .as_ref()
                .map(|callback| callback(details)),
        };
    }
    pub fn cancel(&self) {
        if let Some(callback) = &self.callbacks.on_tap_cancel {
            callback();
        }
        if let Some(callback) = &self.callbacks.on_double_tap_cancel {
            callback();
        }
        if let Some(callback) = &self.callbacks.on_pan_cancel {
            callback();
        }
        if let Some(callback) = &self.callbacks.on_horizontal_drag_cancel {
            callback();
        }
        if let Some(callback) = &self.callbacks.on_vertical_drag_cancel {
            callback();
        }
        if let Some(callback) = &self.callbacks.on_cancel {
            callback();
        }
    }
    pub fn handle(&mut self, event: PointerEvent) -> bool {
        match self.observe(event) {
            GestureDecision::Accept(action) => {
                self.dispatch(action);
                true
            }
            GestureDecision::Pending => self.down.is_some(),
            GestureDecision::Reject => false,
            GestureDecision::Cancelled => {
                self.cancel();
                true
            }
        }
    }
}

impl GestureRecognizer for PointerGestureRecognizer {
    fn observe(&mut self, event: PointerEvent) -> GestureDecision {
        Self::observe(self, event)
    }

    fn dispatch(&mut self, action: GestureAction) {
        Self::dispatch(self, action);
    }

    fn observe_raw(&mut self, event: crate::details::RawPointerEvent) -> GestureDecision {
        self.kind = event.kind;
        Self::observe(self, event.legacy())
    }

    fn cancel(&mut self) {
        Self::cancel(self);
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl GestureDetector {
    #[must_use]
    pub fn new(callbacks: GestureCallbacks) -> Self {
        Self {
            recognizer: PointerGestureRecognizer::new(callbacks),
        }
    }

    pub fn observe(&mut self, event: PointerEvent) -> GestureDecision {
        self.recognizer.observe(event)
    }

    pub fn dispatch(&mut self, action: GestureAction) {
        self.recognizer.dispatch(action);
    }

    pub fn cancel(&mut self) {
        self.recognizer.cancel();
    }

    pub fn handle(&mut self, event: PointerEvent) -> bool {
        self.recognizer.handle(event)
    }
}

/// A platform-neutral drag recognizer with start, update, end, and cancellation.
pub struct DragGestureDetector {
    callbacks: DragCallbacks,
    active: Option<PointerEvent>,
    previous: Option<PointerEvent>,
    started: bool,
}
impl DragGestureDetector {
    pub const SLOP: f32 = 18.;
    #[must_use]
    pub fn new(callbacks: DragCallbacks) -> Self {
        Self {
            callbacks,
            active: None,
            previous: None,
            started: false,
        }
    }
    pub fn handle(&mut self, event: PointerEvent) -> bool {
        match event.phase {
            PointerPhase::Down => {
                self.active = Some(event);
                self.previous = Some(event);
                self.started = false;
                true
            }
            PointerPhase::Move => {
                let Some(start) = self.active else {
                    return false;
                };
                let previous = self.previous.replace(event).unwrap_or(start);
                let total_delta = event.position - start.position;
                if !self.started && total_delta.x.hypot(total_delta.y) >= Self::SLOP {
                    self.started = true;
                    if let Some(callback) = &self.callbacks.on_start {
                        callback(start.position);
                    }
                }
                if self.started {
                    if let Some(callback) = &self.callbacks.on_update {
                        callback(DragUpdateDetails {
                            global_position: event.position,
                            local_position: event.position,
                            delta: event.position - previous.position,
                            total_delta,
                        });
                    }
                }
                true
            }
            PointerPhase::Up | PointerPhase::Cancel => {
                let Some(start) = self.active.take() else {
                    return false;
                };
                let previous = self.previous.take().unwrap_or(start);
                let started = std::mem::take(&mut self.started);
                if started {
                    let elapsed = event
                        .time
                        .saturating_duration_since(previous.time)
                        .as_secs_f32();
                    let delta = event.position - previous.position;
                    if let Some(callback) = &self.callbacks.on_end {
                        callback(DragEndDetails {
                            total_delta: event.position - start.position,
                            velocity: if elapsed > 0. {
                                Offset::new(delta.x / elapsed, delta.y / elapsed)
                            } else {
                                Offset::ZERO
                            },
                            cancelled: matches!(event.phase, PointerPhase::Cancel),
                        });
                    }
                }
                true
            }
        }
    }
}
