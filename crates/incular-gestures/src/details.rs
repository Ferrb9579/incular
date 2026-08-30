use std::rc::Rc;

use std::time::Instant;

use incular_core::{KeyboardEvent, Offset, PointerPhase};

use crate::focus::{FocusBehavior, FocusNode};
use crate::keyboard::{
    ActionInvocationSubscription, ActionListenerSubscription, ErasedActionScope,
    ErasedShortcutScope,
};

use crate::scale::{ScaleEndDetails, ScaleStartDetails, ScaleUpdateDetails};

/// A platform-neutral pointer event.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerEvent {
    pub pointer: u64,
    pub position: Offset,
    pub phase: PointerPhase,
    pub time: Instant,
}

/// A pointer event carrying the metadata needed by raw input widgets.
///
/// [`PointerEvent`] predates device-aware routing and remains intentionally
/// small for source compatibility. Raw widgets use this value so a retained
/// route can preserve the pointer stream, physical device, pressed buttons,
/// phase, and event timestamp without coupling the widget crate to a native
/// event type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawPointerEvent {
    /// Stable for the duration of one pointer sequence.
    pub pointer: u64,
    /// Platform device identifier. Zero means that the source did not supply
    /// a distinct device identifier.
    pub device: u64,
    /// Physical category of the device producing the event.
    pub kind: PointerDeviceKind,
    /// Flutter-compatible button bit mask. Zero is used for hover and for
    /// adapters that do not expose button state.
    pub buttons: u32,
    pub position: Offset,
    pub phase: PointerPhase,
    pub time: Instant,
}

impl RawPointerEvent {
    #[must_use]
    pub const fn new(
        pointer: u64,
        device: u64,
        kind: PointerDeviceKind,
        buttons: u32,
        position: Offset,
        phase: PointerPhase,
        time: Instant,
    ) -> Self {
        Self {
            pointer,
            device,
            kind,
            buttons,
            position,
            phase,
            time,
        }
    }

    /// Converts to the legacy gesture event while retaining the raw event as
    /// the source of truth for listeners and tap regions.
    #[must_use]
    pub const fn legacy(self) -> PointerEvent {
        PointerEvent {
            pointer: self.pointer,
            position: self.position,
            phase: self.phase,
            time: self.time,
        }
    }
}

impl From<PointerEvent> for RawPointerEvent {
    fn from(event: PointerEvent) -> Self {
        Self {
            pointer: event.pointer,
            device: 0,
            kind: PointerDeviceKind::Mouse,
            buttons: 0,
            position: event.position,
            phase: event.phase,
            time: event.time,
        }
    }
}

/// A renderer-independent cursor preference for a [`MouseRegion`].
///
/// `Defer` asks the retained hit-test route to continue behind the current
/// region. Concrete platform adapters map the remaining values to native
/// cursors; keeping the vocabulary here avoids putting platform handles in
/// the widget or painting crates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum MouseCursor {
    #[default]
    Defer,
    Basic,
    Click,
    Text,
    Crosshair,
    Grab,
    Grabbing,
    NotAllowed,
    ResizeHorizontal,
    ResizeVertical,
}

pub use incular_core::PointerDeviceKind;

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
    /// A typed shortcut scope retained separately from the callback above so
    /// the widget tree can resolve an intent against action ancestors.
    pub shortcut_scope: Option<Rc<ErasedShortcutScope>>,
    /// A typed action scope used by descendant shortcut scopes.
    pub action_scope: Option<Rc<ErasedActionScope>>,
    /// Lifetime token for an [`ActionListener`] registration.
    pub action_listener: Option<Rc<ActionListenerSubscription>>,
    /// Lifetime token for an action invocation listener installed by a
    /// retained `ActionListener` widget.
    pub action_invocation_listener: Option<Rc<ActionInvocationSubscription>>,
    /// Focus metadata associated with a keyboard listener. Keeping this on the
    /// platform-neutral callback value lets the retained tree route keyboard
    /// input without introducing a runtime dependency into gestures.
    pub focus_node: Option<FocusNode>,
    /// Combined retained focus, hover, and focus-highlight behavior.
    pub focus_behavior: Option<Rc<FocusBehavior>>,
    pub autofocus: bool,
    pub include_semantics: bool,
    /// Renderer-independent cursor preference for hover-capable widgets.
    pub mouse_cursor: MouseCursor,
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
