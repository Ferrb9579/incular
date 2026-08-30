//! Platform-neutral gesture recognition and interaction primitives.
//!
//! This crate deliberately contains no widget-tree, runtime, or native-window
//! types. Retained widget regions and platform adapters share it as an input
//! boundary.

mod arena;
mod details;
mod focus;
mod keyboard;
mod mouse;
mod pointer;
mod scale;

#[cfg(test)]
mod tests;

pub use arena::{
    GestureArena, GestureArenaEntry, GestureArenaKey, GestureArenaMember, GestureDisposition,
};
pub use details::{
    DragCallbacks, DragDownDetails, DragEndDetails, DragStartDetails, DragUpdateDetails,
    GestureAction, GestureCallbacks, GestureDecision, LongPressEndDetails,
    LongPressMoveUpdateDetails, LongPressStartDetails, PointerDeviceKind, PointerEvent,
    TapDownDetails, TapUpDetails, Velocity,
};
pub use focus::{
    FocusManager, FocusNode, FocusScopeNode, FocusScopeSubscription, FocusTraversalPolicy,
    FocusTraversalPolicyKind, OrderedTraversalPolicy, ReadingOrderTraversalPolicy,
    WidgetOrderTraversalPolicy,
};
pub use keyboard::{
    Action, ActionResult, Actions, Command, CommandId, Intent, LogicalShortcutKey, ShortcutKey,
    ShortcutTrigger, Shortcuts,
};
pub use mouse::MouseRegion;
pub use pointer::{
    DragGestureDetector, GestureDetector, GestureRecognizer, PointerGestureRecognizer,
};
pub use scale::{ScaleEndDetails, ScaleGestureDetector, ScaleStartDetails, ScaleUpdateDetails};
