//! Focused, renderer-neutral implementations for the advanced scrolling
//! widgets that still need a retained-tree adapter.
//!
//! The modules in this directory deliberately stop at the same boundary as
//! `incular-scroll`: they own controller state, viewport geometry, child
//! virtualization, transforms, and hit-test decisions, while the tree adapter
//! is kept as a small integration patch for the master agent. This keeps the
//! algorithms testable without adding a second widget or sliver protocol.

pub mod draggable;
pub mod raw_scrollbar;
pub mod two_dimensional;
pub mod wheel;

#[allow(unused_imports)]
pub use draggable::{
    DraggableNotificationSubscription, DraggableScrollableActuator, DraggableScrollableController,
    DraggableScrollableNotification, DraggableScrollableSheet, DraggableScrollableState,
    DraggableSheetDelta, DraggableSheetExtent, DraggableSizeAnimation, DraggableSnap,
    DraggableSnapTarget,
};
#[allow(unused_imports)]
pub use raw_scrollbar::{
    RawScrollbar, RawScrollbarGeometry, RawScrollbarOrientation, RawScrollbarStyle,
};
#[allow(unused_imports)]
pub use two_dimensional::{
    CacheExtentStyle, ChildVicinity, DiagonalDragBehavior, TwoDimensionalChildDelegate,
    TwoDimensionalChildLayout, TwoDimensionalConstraints, TwoDimensionalScrollDelta,
    TwoDimensionalScrollView, TwoDimensionalScrollable, TwoDimensionalViewport,
    TwoDimensionalViewportLayout,
};
#[allow(unused_imports)]
pub use wheel::{
    ChangeReportingBehavior, FixedExtentScrollController, ListWheelScrollView, ListWheelViewport,
    WheelChildDelegate, WheelChildLayout, WheelLayout, WheelMatrix, WheelProjection,
};
