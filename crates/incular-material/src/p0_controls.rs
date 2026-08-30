//! Material P0 adapters for stateful controls, sliders, and tabs.
//!
//! The modules in `incular-controls` own the retained interaction mechanics.
//! These descriptors only add Material vocabulary, defaults, theme slots, and
//! the public Rust API expected by an ordinary Material application.

#[path = "p0_controls/checkbox_radio.rs"]
mod checkbox_radio;
#[path = "p0_controls/icons.rs"]
mod icons;
#[path = "p0_controls/slider_range.rs"]
mod slider_range;
#[path = "p0_controls/switch.rs"]
mod switch;
#[path = "p0_controls/tabs.rs"]
mod tabs;

pub use checkbox_radio::*;
pub use icons::*;
pub use slider_range::*;
pub use switch::*;
pub use tabs::*;
