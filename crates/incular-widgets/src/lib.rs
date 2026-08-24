//! Declarative widgets and the retained element/render tree.
//!
//! The public crate root provides canonical first-class Flutter-style widget descriptors
//! while internal retained tree execution remains Rust-native.

mod animation_transitions;
#[cfg(feature = "devtools")]
pub mod devtools;
#[cfg(feature = "devtools")]
pub mod devtools_props;
mod drag_drop;
mod environment;
mod focus_keyboard;
mod forms;
mod gestures;
mod hero;
mod implicit_animations;
pub mod layout;
mod navigation_scopes;
mod painting_effects;
mod radio_selection;
mod reactive_builders;
mod safe_area;
mod scrolling;
mod selection;
mod semantics;
mod tree;
mod utilities;

pub use animation_transitions::*;
pub use drag_drop::*;
pub use environment::*;
pub use focus_keyboard::*;
pub use forms::*;
pub use gestures::*;
pub use hero::*;
pub use implicit_animations::*;
pub use incular_scroll::{
    BoundaryPhysics, ClampingScrollPhysics, DragStartBehavior, MeasuredExtentIndex,
    NestedScrollCoordinator, ScrollCacheExtent, ScrollController, ScrollDelta, ScrollMetrics,
    ScrollPhysics, ScrollSpringStep, ScrollViewConfig, ScrollViewKeyboardDismissBehavior,
    Scrollability, ScrollbarGeometry, ScrollbarStyle, SnapPhysics,
};
pub use incular_text::{FontFamily, TextStyle};
pub use layout::*;
pub use navigation_scopes::*;
pub use painting_effects::*;
pub use radio_selection::*;
pub use reactive_builders::*;
pub use safe_area::*;
pub use scrolling::*;
pub use selection::*;
pub use semantics::*;
pub use tree::*;
pub use utilities::*;

/// Convenience macro to construct a heterogeneous `Vec<Widget>` from items implementing `Into<Widget>`.
#[macro_export]
macro_rules! children {
    ($($child:expr),* $(,)?) => {
        vec![$($crate::Widget::from($child)),*]
    };
}

/// A trait for types that can convert into an erased [`Widget`].
pub trait IntoWidget {
    fn into_widget(self) -> Widget;
}

impl<T: Into<Widget>> IntoWidget for T {
    fn into_widget(self) -> Widget {
        self.into()
    }
}
