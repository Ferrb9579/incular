//! Material adapters for retained selection controls and sliders.

#[path = "selection/checkbox_radio.rs"]
mod checkbox_radio;
#[path = "selection/slider_range.rs"]
mod slider_range;
#[path = "selection/switch.rs"]
mod switch;
#[path = "selection/vocabulary.rs"]
mod vocabulary;

pub use checkbox_radio::*;
pub use slider_range::*;
pub use switch::*;
pub use vocabulary::*;
