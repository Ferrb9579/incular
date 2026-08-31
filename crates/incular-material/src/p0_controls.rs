//! Material P0 adapters for stateful controls, sliders, and tabs.
//!
//! The modules in `incular-controls` own the retained interaction mechanics.
//! These descriptors only add Material vocabulary, defaults, theme slots, and
//! the public Rust API expected by an ordinary Material application.

mod checkbox_radio;
mod icons;
mod slider_range;
mod switch;
mod tabs;

pub use checkbox_radio::*;
pub use icons::*;
pub use slider_range::*;
pub use switch::*;
pub use tabs::*;
