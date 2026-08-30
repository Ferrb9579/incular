use std::rc::Rc;

use std::time::Instant;

use incular_core::{KeyboardEvent, Offset, PointerPhase};

use crate::focus::FocusNode;

use crate::scale::{ScaleEndDetails, ScaleStartDetails, ScaleUpdateDetails};

/// A platform-neutral pointer event.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerEvent {
    pub pointer: u64,
    pub position: Offset,
    pub phase: PointerPhase,
    pub time: Instant,
}

/// The physical device category generating pointer events.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PointerDeviceKind {
    #[default]
    Mouse,
    Touch,
    Stylus,
    InvertedStylus,
    Trackpad,
    Unknown,
}

/// Physical pointer velocity in logical pixels per second.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Velocity {
    pub pixels_per_second: Offset,
}

impl Velocity {
    pub const ZERO: Self = Self {
        pixels_per_second: Offset::ZERO,
    };

    #[must_use]
    pub const fn new(pixels_per_second: Offset) -> Self {
        Self { pixels_per_second }
    }
}

/// Position and timing metadata for pointer tap-down events.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TapDownDetails {
    pub global_position: Offset,
    pub local_position: Offset,
    pub kind: PointerDeviceKind,
}

/// Position and timing metadata for pointer tap-up events.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TapUpDetails {
    pub global_position: Offset,
    pub local_position: Offset,
    pub kind: PointerDeviceKind,
}

/// Metadata for drag start events.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DragStartDetails {
    pub global_position: Offset,
    pub local_position: Offset,
}

/// Metadata for drag down events.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DragDownDetails {
    pub global_position: Offset,
    pub local_position: Offset,
}

/// Metadata for long-press start events.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LongPressStartDetails {
    pub global_position: Offset,
    pub local_position: Offset,
}

/// Metadata for long-press movement update events.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LongPressMoveUpdateDetails {
    pub global_position: Offset,
    pub local_position: Offset,
    pub offset_from_origin: Offset,
    pub local_offset_from_origin: Offset,
}

/// Metadata for long-press end events.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LongPressEndDetails {
    pub global_position: Offset,
    pub local_position: Offset,
    pub velocity: Velocity,
}

/// Callbacks understood by [`PointerGestureRecognizer`].
#[derive(Clone, Default)]
pub struct GestureCallbacks {
    pub on_tap: Option<Rc<dyn Fn()>>,
    pub on_tap_down: Option<Rc<dyn Fn(TapDownDetails)>>,
    pub on_tap_up: Option<Rc<dyn Fn(TapUpDetails)>>,
    pub on_tap_cancel: Option<Rc<dyn Fn()>>,
    pub on_double_tap: Option<Rc<dyn Fn()>>,
    pub on_double_tap_down: Option<Rc<dyn Fn(TapDownDetails)>>,
    pub on_double_tap_cancel: Option<Rc<dyn Fn()>>,
    pub on_long_press: Option<Rc<dyn Fn()>>,
    pub on_long_press_start: Option<Rc<dyn Fn(LongPressStartDetails)>>,
    pub on_long_press_move_update: Option<Rc<dyn Fn(LongPressMoveUpdateDetails)>>,
    pub on_long_press_up: Option<Rc<dyn Fn()>>,
    pub on_long_press_end: Option<Rc<dyn Fn(LongPressEndDetails)>>,
    pub on_pan_down: Option<Rc<dyn Fn(DragDownDetails)>>,
    pub on_pan_start: Option<Rc<dyn Fn(DragStartDetails)>>,
    pub on_pan_update: Option<Rc<dyn Fn(Offset)>>,
    /// Called once when a pan ends normally. The callback receives the total
    /// displacement, velocity, and cancellation state for the pointer stream.
    pub on_pan_end: Option<Rc<dyn Fn(DragEndDetails)>>,
    pub on_pan_cancel: Option<Rc<dyn Fn()>>,
    pub on_horizontal_drag_down: Option<Rc<dyn Fn(DragDownDetails)>>,
    pub on_horizontal_drag_start: Option<Rc<dyn Fn(DragStartDetails)>>,
    /// Receives a pan whose first slop-exceeding movement was horizontal.
    /// It competes with vertical drags in a retained `GestureDetector`.
    pub on_horizontal_drag_update: Option<Rc<dyn Fn(Offset)>>,
    /// Called once when a horizontal drag ends normally.
    pub on_horizontal_drag_end: Option<Rc<dyn Fn(DragEndDetails)>>,
    pub on_horizontal_drag_cancel: Option<Rc<dyn Fn()>>,
    pub on_vertical_drag_down: Option<Rc<dyn Fn(DragDownDetails)>>,
    pub on_vertical_drag_start: Option<Rc<dyn Fn(DragStartDetails)>>,
    /// Receives a pan whose first slop-exceeding movement was vertical.
    /// It competes with horizontal drags in a retained `GestureDetector`.
    pub on_vertical_drag_update: Option<Rc<dyn Fn(Offset)>>,
    /// Called once when a vertical drag ends normally.
    pub on_vertical_drag_end: Option<Rc<dyn Fn(DragEndDetails)>>,
    pub on_vertical_drag_cancel: Option<Rc<dyn Fn()>>,
    pub on_scale_start: Option<Rc<dyn Fn(ScaleStartDetails)>>,
    /// Receives the active focal point and relative distance for a retained
    /// multi-pointer region. Single-pointer recognizers ignore this callback.
    pub on_scale_update: Option<Rc<dyn Fn(ScaleUpdateDetails)>>,
    pub on_scale_end: Option<Rc<dyn Fn(ScaleEndDetails)>>,
    /// Called once for a retained pointer sequence when this region loses its
    /// arena claim or the platform cancels the sequence.
    pub on_cancel: Option<Rc<dyn Fn()>>,
    /// Receives a keyboard event after the retained runtime resolves the
    /// focused element. Returning `true` consumes the event and prevents
    /// outer keyboard scopes and text editing from seeing it.
    pub on_key: Option<Rc<dyn Fn(KeyboardEvent) -> bool>>,
    /// Receives key-down events that were not consumed by [`Self::on_key`].
    pub on_key_down: Option<Rc<dyn Fn(KeyboardEvent)>>,
    /// Receives repeated key-down events in addition to [`Self::on_key_down`].
    pub on_key_repeat: Option<Rc<dyn Fn(KeyboardEvent)>>,
    /// Receives key-up events that were not consumed by [`Self::on_key`].
    pub on_key_up: Option<Rc<dyn Fn(KeyboardEvent)>>,
    /// An optional typed shortcut dispatcher. Widgets owns the closure so
    /// this crate remains independent from the retained runtime.
    pub on_shortcut: Option<Rc<dyn Fn(KeyboardEvent) -> bool>>,
    /// Focus metadata associated with a keyboard listener. Keeping this on the
    /// platform-neutral callback value lets the retained tree route keyboard
    /// input without introducing a runtime dependency into gestures.
    pub focus_node: Option<FocusNode>,
    pub autofocus: bool,
    pub include_semantics: bool,
}

/// The callback action selected after a recognizer has claimed its arena.
/// This is public so retained adapters can defer callbacks until arbitration
/// has completed while [`GestureDetector::handle`] remains source-compatible.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GestureAction {
    Tap,
    DoubleTap,
    LongPress,
    Pan(Offset),
    PanEnd(DragEndDetails),
    HorizontalDrag(Offset),
    HorizontalDragEnd(DragEndDetails),
    VerticalDrag(Offset),
    VerticalDragEnd(DragEndDetails),
}

/// A recognizer's claim for one pointer event.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GestureDecision {
    Pending,
    Accept(GestureAction),
    Reject,
    Cancelled,
}

/// Details emitted by a single-pointer drag recognizer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DragUpdateDetails {
    pub global_position: Offset,
    pub local_position: Offset,
    pub delta: Offset,
    pub total_delta: Offset,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DragEndDetails {
    pub total_delta: Offset,
    pub velocity: Offset,
    pub cancelled: bool,
}
#[derive(Clone, Default)]
pub struct DragCallbacks {
    pub on_start: Option<Rc<dyn Fn(Offset)>>,
    pub on_update: Option<Rc<dyn Fn(DragUpdateDetails)>>,
    pub on_end: Option<Rc<dyn Fn(DragEndDetails)>>,
}
