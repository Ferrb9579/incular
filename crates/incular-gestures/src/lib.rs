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
mod raw;
mod scale;

pub use arena::{
    GestureArena, GestureArenaEntry, GestureArenaKey, GestureArenaMember, GestureArenaSource,
    GestureDisposition,
};
pub use details::{
    DragCallbacks, DragDownDetails, DragEndDetails, DragStartDetails, DragUpdateDetails,
    GestureAction, GestureCallbacks, GestureDecision, LongPressEndDetails,
    LongPressMoveUpdateDetails, LongPressStartDetails, MouseCursor, PointerDeviceKind,
    PointerEvent, RawPointerEvent, TapDownDetails, TapUpDetails, Velocity,
};
pub use focus::{
    FocusBehavior, FocusHighlightManager, FocusHighlightMode, FocusHighlightStrategy,
    FocusHighlightSubscription, FocusManager, FocusNode, FocusNodeSubscription, FocusScopeNode,
    FocusScopeSubscription, FocusTraversalPolicy, FocusTraversalPolicyKind, OrderedTraversalPolicy,
    ReadingOrderTraversalPolicy, WidgetOrderTraversalPolicy,
};
pub use keyboard::{
    Action, ActionInvocationPhase, ActionInvocationSubscription, ActionListenerSubscription,
    ActionResult, Actions, Command, CommandId, ErasedActionScope, ErasedIntent,
    ErasedShortcutScope, Intent, LogicalShortcutKey, ShortcutActivator, ShortcutKey, ShortcutMatch,
    ShortcutTrigger, Shortcuts, SingleActivator,
};
pub use mouse::MouseRegion;
pub use pointer::{DragGestureDetector, GestureDetector, PointerGestureRecognizer};
pub use raw::{
    ErasedGestureRecognizerFactory, GestureRecognizer, GestureRecognizerFactory,
    GestureRecognizerFactoryError, GestureRecognizerFactoryWithHandlers,
};
pub use scale::{ScaleEndDetails, ScaleGestureDetector, ScaleStartDetails, ScaleUpdateDetails};
