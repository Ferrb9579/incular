//! Declarative widgets and the retained element/render tree.
//!
//! The public crate root provides canonical first-class Flutter-style widget descriptors
//! while internal retained tree execution remains Rust-native.

#[cfg(feature = "devtools")]
pub mod devtools;
#[cfg(feature = "devtools")]
pub mod devtools_props;
mod drag_drop;
mod focus_keyboard;
mod forms;
mod gestures;
pub mod layout;
mod safe_area;
mod scrolling;
mod selection;
mod semantics;
mod tree;

pub use drag_drop::*;
pub use focus_keyboard::*;
pub use forms::*;
pub use gestures::*;
pub use incular_scroll::{
    BoundaryPhysics, ClampingScrollPhysics, MeasuredExtentIndex, NestedScrollCoordinator,
    ScrollController, ScrollDelta, ScrollPhysics, ScrollSpringStep, Scrollability,
    ScrollbarGeometry, ScrollbarStyle, SnapPhysics,
};
pub use incular_text::{FontFamily, TextStyle};
pub use layout::*;
pub use safe_area::*;
pub use scrolling::*;
pub use selection::*;
pub use semantics::*;
pub use tree::*;

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
