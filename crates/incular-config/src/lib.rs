//! Renderer-independent configuration values shared by Incular layers.
//!
//! The types in this crate describe policy and bounds only. Layout algorithms
//! live in `incular-layout`; widgets, runtime, and platform adapters can use
//! these values without depending on each other.

mod alignment;
mod constraints;
mod insets;

pub use alignment::{
    Alignment, AlignmentDirectional, Axis, AxisDirection, CrossAxisAlignment, FlexFit,
    MainAxisAlignment, MainAxisSize, TextDirection, VerticalDirection, WrapAlignment,
    WrapCrossAlignment,
};
pub use constraints::{ConstraintError, Constraints};
pub use insets::EdgeInsets;
