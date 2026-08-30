//! Advanced retained slivers.
//!
//! This module is intentionally kept separate from the first-generation
//! scrolling descriptors.  It contains the state machines needed by
//! Flutter's tree and animated collection slivers while sharing the public
//! retained `RenderSliver` protocol.

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::{Rc, Weak},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

use incular_animation::AnimationController;
use incular_config::{Axis, Clip, Constraints, WidgetDefaults};
use incular_scroll::{
    MeasuredExtentIndex, ScrollController, ScrollPhysics, SliverConstraints, SliverGeometry,
};

use super::internal::{
    CustomScrollView, RenderSliver, SizedBox, Sliver, SliverChildId, SliverChildLayout,
    SliverGridDelegate, SliverLayout, Widget,
};

mod animated;
mod keep_alive;
mod tree_sliver;

#[allow(unused_imports)]
pub use animated::{
    AnimatedGrid, AnimatedGridController, AnimatedItem, AnimatedItemBuilder, AnimatedItemPhase,
    AnimatedList, AnimatedListController, AnimatedRemovedItemBuilder, SliverAnimatedGrid,
    SliverAnimatedGridController,
};
#[allow(unused_imports)]
pub use keep_alive::{
    AutomaticKeepAlive, KeepAlive, KeepAliveHandle, KeepAliveNotification, KeepAliveRegistry,
};
#[allow(unused_imports)]
pub use tree_sliver::{
    TreeRowAnimation, TreeSliver, TreeSliverController, TreeSliverIndentation, TreeSliverNode,
    TreeSliverNodeId,
};
