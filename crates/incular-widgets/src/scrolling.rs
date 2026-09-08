use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    time::{Duration, Instant},
};

use incular_animation::{AnimationController, Curve, Tween};
use incular_config::{Axis, Clip, Constraints, EdgeInsets, WidgetDefaults};
use incular_core::Size;
use incular_scroll::{
    MeasuredExtentIndex, ScrollController, ScrollNotification, ScrollNotificationSubscription,
    ScrollNotificationType, ScrollPhysics, SliverConstraints, SliverGeometry,
};
use typed_builder::TypedBuilder;

use crate::drag_drop::DragDropContext;
use crate::tree::WidgetKind;
use crate::{Column, DecoratedBox, DragTarget, Draggable, Padding, Row, SizedBox, Widget};

type SliverLayoutBuilderFn = Rc<dyn Fn(SliverConstraints) -> Widget>;

const DEFAULT_LAZY_ITEM_EXTENT: f32 = WidgetDefaults::DEFAULT.lazy_item_extent;
const DEFAULT_SLIVER_CACHE_EXTENT: f32 = WidgetDefaults::DEFAULT.sliver_cache_extent;

mod geometry;
mod layout;
mod notifications;
mod reorderable;
mod scroll_views;
mod sliver_descriptors;
mod sliver_lists;

pub use geometry::{
    RenderSliver, SliverChildId, SliverChildLayout, SliverChildPlacement, SliverLayout,
    SliverScrollDependency, SliverViewportConfig,
};
pub(crate) use geometry::{SliverViewportDelegate, SliverViewportLayout};

pub use layout::SliverOverlapHandle;
use layout::{
    BoxRenderSliver, FillRemainingRenderSliver, FixedExtentRenderSliver,
    FloatingHeaderRenderSliver, GridRenderSliver, HeaderPresentation, HeaderRenderSliver,
    HeaderScrollState, HeaderSnapFrame, LayoutBuilderRenderSliver, NaturalHeaderExtent,
    NaturalHeaderRenderSliver, OverlapAbsorberRenderSliver, OverlapInjectorRenderSliver,
    PaddingRenderSliver, ResizingHeaderRenderSliver, SequenceRenderSliver,
    SequenceViewportDelegate, SliverViewportOptions, VariableExtentRenderSliver,
    ViewportExtentRenderSliver, WidgetWrapRenderSliver, single_sliver_viewport,
    single_sliver_viewport_with_options, sliver_child_constraints, snap_trigger_subscription,
    widget_main_extent_hint,
};

pub use notifications::*;
pub use reorderable::{
    ReorderableDelayedDragStartListener, ReorderableDragStartListener, ReorderableList,
};
pub use scroll_views::*;
pub use sliver_descriptors::*;
pub use sliver_lists::*;
