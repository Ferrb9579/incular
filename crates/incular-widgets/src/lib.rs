//! Declarative widgets and the retained element/render tree.
//!
//! The public crate root intentionally stays small.  Widget construction and
//! the retained tree live in focused modules so applications depend on a
//! stable API rather than the renderer's internal implementation details.

mod gestures;
mod parity;
mod scrolling;
mod tree;

pub use gestures::*;
pub use incular_scroll::{
    ClampingScrollPhysics, ScrollController, ScrollbarGeometry, ScrollbarStyle,
};
pub use parity::*;
pub use scrolling::*;
pub use tree::*;
