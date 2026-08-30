//! Backwards-compatible layout re-exports.
//!
//! Canonical ownership moved to `incular-config`; layout algorithms retain
//! these names in their historical module path so downstream code can migrate
//! without a type split.

pub use incular_config::{
    Alignment, AlignmentDirectional, Axis, AxisDirection, CrossAxisAlignment, FlexFit,
    MainAxisAlignment, MainAxisSize, TextDirection, VerticalDirection, WrapAlignment,
    WrapCrossAlignment,
};
