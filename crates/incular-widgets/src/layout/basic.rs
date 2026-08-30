//! Canonical Flutter-style basic layout widget descriptors.
//!
//! The module is kept as the public facade for the basic layout widgets. The
//! implementations are grouped by layout responsibility while preserving the
//! existing API paths and behavior.
//!
//! Provides [`Padding`], [`Align`], [`Center`], [`SizedBox`], [`ColoredBox`],
//! [`ConstrainedBox`], [`LimitedBox`], [`OverflowBox`], [`UnconstrainedBox`],
//! [`FractionallySizedBox`], [`Baseline`], [`AspectRatio`], [`FittedBox`],
//! [`Visibility`], [`Offstage`], [`LayoutBuilder`], [`RepaintBoundary`],
//! [`CustomPaint`], and clipping widgets ([`ClipRect`], [`ClipRRect`],
//! [`ClipOval`], [`ClipPath`]), along with the remaining basic utility
//! descriptors.

mod baseline_aspect_fractional;
mod clipping;
mod padding_alignment;
mod painting;
mod sizing_constraints;
mod split_view;
mod transforms;
mod utility;
mod visibility;

pub use baseline_aspect_fractional::{
    AspectRatio, AspectRatioBuilder, Baseline, BaselineBuilder, FittedBox, FittedBoxBuilder,
    FractionallySizedBox, FractionallySizedBoxBuilder,
};
pub use clipping::{
    ClipOval, ClipOvalBuilder, ClipPath, ClipPathBuilder, ClipRRect, ClipRRectBuilder, ClipRect,
    ClipRectBuilder,
};
pub use padding_alignment::{Align, AlignBuilder, Center, CenterBuilder, Padding, PaddingBuilder};
pub use painting::{
    ColoredBox, ColoredBoxBuilder, CustomPaint, CustomPaintBuilder, Placeholder,
    PlaceholderBuilder, RepaintBoundary, RepaintBoundaryBuilder,
};
pub use sizing_constraints::{
    ConstrainedBox, ConstrainedBoxBuilder, ConstraintsTransformBox, IntrinsicHeight,
    IntrinsicHeightBuilder, IntrinsicWidth, IntrinsicWidthBuilder, LayoutBuilder, LimitedBox,
    LimitedBoxBuilder, OverflowBox, OverflowBoxBuilder, PreferredSize, PreferredSizeBuilder,
    SizedBox, SizedBoxBuilder, SizedOverflowBox, SizedOverflowBoxBuilder, UnconstrainedBox,
    UnconstrainedBoxBuilder,
};
pub use split_view::{SplitPosition, SplitView, SplitViewBuilder};
pub use transforms::{FractionalTranslation, FractionalTranslationBuilder, RotatedBox};
pub use utility::{
    InteractiveViewer, InteractiveViewerBuilder, KeyedSubtree, KeyedSubtreeBuilder,
    NavigationToolbar, NavigationToolbarBuilder, OverflowBar, OverflowBarBuilder, TableCell,
    TableCellBuilder,
};
pub use visibility::{Offstage, OffstageBuilder, Visibility, VisibilityBuilder};
