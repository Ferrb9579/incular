//! Renderer-independent layout policy and configuration.
//!
//! This crate deliberately contains no widget or render-object types. It
//! provides the values a layout engine needs (constraints, alignment, insets)
//! together with deterministic algorithms that operate on measured child
//! sizes. A widget tree can use these algorithms without making layout depend
//! on a particular renderer or widget representation.

mod algorithms;
mod alignment;
mod constraints;
mod descriptors;
mod geometry;
mod insets;

pub use algorithms::{
    AlignmentChild, BaselineChild, FlexChild, StackChild, StackPosition, TableChild, WrapChild,
    layout_align, layout_aspect_ratio, layout_baseline, layout_center, layout_constrained_box,
    layout_flex, layout_fractionally_sized, layout_padding, layout_sized_box, layout_stack,
    layout_table, layout_unconstrained_box, layout_wrap,
};
pub use alignment::{
    Alignment, AlignmentDirectional, Axis, AxisDirection, CrossAxisAlignment, FlexFit,
    MainAxisAlignment, MainAxisSize, TextDirection, VerticalDirection, WrapAlignment,
    WrapCrossAlignment,
};
pub use constraints::{ConstraintError, Constraints};
pub use descriptors::{
    Align, AspectRatio, Baseline, BaselineType, Center, Clip, Column, ConstrainedBox, Flex,
    FractionallySizedBox, LayoutOptions, Offstage, Padding, Positioned, Row, SizedBox, Stack,
    StackFit, Table, UnconstrainedBox, Visibility, Wrap,
};
pub use geometry::{ChildLayout, LayoutResult};
pub use insets::EdgeInsets;

// Re-exporting the small geometry values makes the layout API convenient while
// retaining `incular-core` as their canonical owner.
pub use incular_core::{Offset, Rect, Size};
