use std::rc::Rc;

use incular_config::{Axis, Clip, EdgeInsets};
use incular_scroll::{
    DragStartBehavior, MeasuredExtentIndex, ScrollCacheExtent, ScrollController, ScrollPhysics,
    ScrollViewKeyboardDismissBehavior,
};

use crate::{Column, DecoratedBox, Padding, Row, SizedBox, VirtualList, Widget};

/// A first-class scrollable box that scrolls a single child.
#[derive(Clone)]
pub struct SingleChildScrollView {
    child: Widget,
    scroll_direction: Axis,
    reverse: bool,
    padding: Option<EdgeInsets>,
    controller: Option<ScrollController>,
    physics: Option<ScrollPhysics>,
    clip_behavior: Clip,
}

impl SingleChildScrollView {
    /// Creates a scrollable single-child viewport.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            scroll_direction: Axis::Vertical,
            reverse: false,
            padding: None,
            controller: None,
            physics: None,
            clip_behavior: Clip::HardEdge,
        }
    }

    /// Sets the scroll axis.
    #[must_use]
    pub fn scroll_direction(mut self, direction: Axis) -> Self {
        self.scroll_direction = direction;
        self
    }

    /// Returns the configured scroll axis.
    #[must_use]
    pub fn get_scroll_direction(&self) -> Axis {
        self.scroll_direction
    }

    /// Sets whether the scroll view scrolls in reverse.
    #[must_use]
    pub fn reverse(mut self, reverse: bool) -> Self {
        self.reverse = reverse;
        self
    }

    /// Sets inner padding around the scrollable child.
    #[must_use]
    pub fn padding(mut self, padding: EdgeInsets) -> Self {
        self.padding = Some(padding);
        self
    }

    /// Attaches an external [`ScrollController`].
    #[must_use]
    pub fn controller(mut self, controller: ScrollController) -> Self {
        self.controller = Some(controller);
        self
    }

    /// Sets scroll physics.
    #[must_use]
    pub fn physics(mut self, physics: ScrollPhysics) -> Self {
        self.physics = Some(physics);
        self
    }

    /// Sets clipping behavior.
    #[must_use]
    pub fn clip_behavior(mut self, clip: Clip) -> Self {
        self.clip_behavior = clip;
        self
    }

    /// Sets drag start behavior.
    #[must_use]
    pub const fn drag_start_behavior(self, _behavior: DragStartBehavior) -> Self {
        self
    }

    /// Sets keyboard dismiss behavior.
    #[must_use]
    pub const fn keyboard_dismiss_behavior(
        self,
        _behavior: ScrollViewKeyboardDismissBehavior,
    ) -> Self {
        self
    }

    /// Sets whether this is the primary scroll view.
    #[must_use]
    pub const fn primary(self, _primary: bool) -> Self {
        self
    }

    /// Sets restoration ID.
    #[must_use]
    pub fn restoration_id(self, _id: impl Into<String>) -> Self {
        self
    }
}

impl From<SingleChildScrollView> for Widget {
    fn from(value: SingleChildScrollView) -> Self {
        let child = if let Some(padding) = value.padding {
            Padding::new(padding, value.child).into()
        } else {
            value.child
        };
        let controller = value.controller.unwrap_or_default();
        Widget::scroll_view_with_config(
            controller,
            child,
            value.scroll_direction,
            value.reverse,
            value.physics.unwrap_or_default(),
        )
    }
}

/// Strategy for determining item main-axis extents in a scrollable list.
#[derive(Clone)]
#[allow(clippy::large_enum_variant)]
pub enum ItemExtentStrategy {
    Measured,
    Fixed(f32),
    Builder(Rc<dyn Fn(usize) -> f32>),
    Prototype(Widget),
}

/// Retention policy for offscreen lazy list item state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum KeepAlivePolicy {
    #[default]
    Automatic,
    Manual,
    Disabled,
}

/// Repaint isolation policy for lazy list items.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum RepaintBoundaryPolicy {
    #[default]
    Automatic,
    Manual,
    Disabled,
}

/// Semantic node indexing policy for lazy list items.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SemanticIndexPolicy {
    #[default]
    Automatic,
    Manual,
    Disabled,
}

/// Delegate governing 2D grid cell layout.
#[derive(Clone)]
pub enum GridDelegate {
    FixedCrossAxisCount {
        cross_axis_count: usize,
        main_axis_spacing: f32,
        cross_axis_spacing: f32,
        child_aspect_ratio: f32,
        main_axis_extent: Option<f32>,
    },
    MaxCrossAxisExtent {
        max_cross_axis_extent: f32,
        main_axis_spacing: f32,
        cross_axis_spacing: f32,
        child_aspect_ratio: f32,
        main_axis_extent: Option<f32>,
    },
}

/// Content strategies for [`ListView`].
#[derive(Clone)]
enum ListViewStrategy {
    Children(Vec<Widget>),
    Builder {
        item_count: usize,
        builder: Rc<dyn Fn(usize) -> Widget>,
    },
    FixedExtent {
        item_count: usize,
        item_extent: f32,
        builder: Rc<dyn Fn(usize) -> Widget>,
    },
    VariableExtent {
        item_count: usize,
        estimated_extent: f32,
        index: Option<MeasuredExtentIndex>,
        builder: Rc<dyn Fn(usize) -> Widget>,
    },
}

/// A first-class list descriptor supporting static children and lazy virtualized item builders.
#[derive(Clone)]
pub struct ListView {
    strategy: ListViewStrategy,
    scroll_direction: Axis,
    reverse: bool,
    controller: Option<ScrollController>,
    padding: Option<EdgeInsets>,
    cache_extent: Option<f32>,
    physics: Option<ScrollPhysics>,
    clip_behavior: Clip,
}

impl ListView {
    /// Creates a ListView from an explicit collection of child widgets.
    #[must_use]
    pub fn new(children: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        Self {
            strategy: ListViewStrategy::Children(children.into_iter().map(Into::into).collect()),
            scroll_direction: Axis::Vertical,
            reverse: false,
            controller: None,
            padding: None,
            cache_extent: None,
            physics: None,
            clip_behavior: Clip::HardEdge,
        }
    }

    /// Creates a lazily virtualized ListView with a child builder closure.
    #[must_use]
    pub fn builder<W>(item_count: usize, builder: impl Fn(usize) -> W + 'static) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self {
            strategy: ListViewStrategy::Builder {
                item_count,
                builder: Rc::new(move |i| builder(i).into()),
            },
            scroll_direction: Axis::Vertical,
            reverse: false,
            controller: None,
            padding: None,
            cache_extent: None,
            physics: None,
            clip_behavior: Clip::HardEdge,
        }
    }

    /// Creates a lazily virtualized ListView with alternating separator items.
    #[must_use]
    pub fn separated<W, S>(
        item_count: usize,
        item_builder: impl Fn(usize) -> W + 'static,
        separator_builder: impl Fn(usize) -> S + 'static,
    ) -> Self
    where
        W: Into<Widget> + 'static,
        S: Into<Widget> + 'static,
    {
        let total_count = if item_count == 0 {
            0
        } else {
            item_count * 2 - 1
        };
        let item_builder = Rc::new(move |i| item_builder(i).into());
        let separator_builder = Rc::new(move |i| separator_builder(i).into());
        Self {
            strategy: ListViewStrategy::Builder {
                item_count: total_count,
                builder: Rc::new(move |index| {
                    if index % 2 == 0 {
                        item_builder(index / 2)
                    } else {
                        separator_builder(index / 2)
                    }
                }),
            },
            scroll_direction: Axis::Vertical,
            reverse: false,
            controller: None,
            padding: None,
            cache_extent: None,
            physics: None,
            clip_behavior: Clip::HardEdge,
        }
    }

    /// Creates a fixed-extent virtualized ListView.
    #[must_use]
    pub fn fixed_extent<W>(
        item_count: usize,
        item_extent: f32,
        builder: impl Fn(usize) -> W + 'static,
    ) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self {
            strategy: ListViewStrategy::FixedExtent {
                item_count,
                item_extent: item_extent.max(1.0),
                builder: Rc::new(move |i| builder(i).into()),
            },
            scroll_direction: Axis::Vertical,
            reverse: false,
            controller: None,
            padding: None,
            cache_extent: None,
            physics: None,
            clip_behavior: Clip::HardEdge,
        }
    }

    /// Creates a variable-extent virtualized ListView.
    #[must_use]
    pub fn variable_extent<W>(
        item_count: usize,
        estimated_extent: f32,
        builder: impl Fn(usize) -> W + 'static,
    ) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self {
            strategy: ListViewStrategy::VariableExtent {
                item_count,
                estimated_extent: estimated_extent.max(1.0),
                index: None,
                builder: Rc::new(move |i| builder(i).into()),
            },
            scroll_direction: Axis::Vertical,
            reverse: false,
            controller: None,
            padding: None,
            cache_extent: None,
            physics: None,
            clip_behavior: Clip::HardEdge,
        }
    }

    /// Sets the scroll controller.
    #[must_use]
    pub fn controller(mut self, controller: ScrollController) -> Self {
        self.controller = Some(controller);
        self
    }

    /// Sets inner padding.
    #[must_use]
    pub fn padding(mut self, padding: EdgeInsets) -> Self {
        self.padding = Some(padding);
        self
    }

    /// Sets scroll direction.
    #[must_use]
    pub fn scroll_direction(mut self, direction: Axis) -> Self {
        self.scroll_direction = direction;
        self
    }

    /// Returns the configured scroll axis.
    #[must_use]
    pub fn get_scroll_direction(&self) -> Axis {
        self.scroll_direction
    }

    /// Sets reverse scrolling.
    #[must_use]
    pub fn reverse(mut self, reverse: bool) -> Self {
        self.reverse = reverse;
        self
    }

    /// Sets cache extent in logical pixels.
    #[must_use]
    pub fn cache_extent(mut self, cache_extent: f32) -> Self {
        self.cache_extent = Some(cache_extent.max(0.0));
        self
    }

    /// Sets cache extent using a typed policy.
    #[must_use]
    pub fn scroll_cache_extent(mut self, cache_extent: ScrollCacheExtent) -> Self {
        self.cache_extent = Some(cache_extent.to_pixels(600.0));
        self
    }

    /// Sets scroll physics.
    #[must_use]
    pub fn physics(mut self, physics: ScrollPhysics) -> Self {
        self.physics = Some(physics);
        self
    }

    /// Sets clipping behavior.
    #[must_use]
    pub fn clip_behavior(mut self, clip: Clip) -> Self {
        self.clip_behavior = clip;
        self
    }

    /// Sets fixed item extent.
    #[must_use]
    pub fn item_extent(mut self, extent: f32) -> Self {
        if let ListViewStrategy::Builder {
            item_count,
            builder,
        } = self.strategy
        {
            self.strategy = ListViewStrategy::FixedExtent {
                item_count,
                item_extent: extent.max(1.0),
                builder,
            };
        }
        self
    }

    /// Sets a prototype child item for measuring extent.
    #[must_use]
    pub fn prototype_item(self, _prototype: impl Into<Widget>) -> Self {
        self
    }

    /// Sets keep-alive policy.
    #[must_use]
    pub const fn keep_alive_policy(self, _policy: KeepAlivePolicy) -> Self {
        self
    }

    /// Sets repaint boundary policy.
    #[must_use]
    pub const fn repaint_boundary_policy(self, _policy: RepaintBoundaryPolicy) -> Self {
        self
    }

    /// Sets semantic index policy.
    #[must_use]
    pub const fn semantic_index_policy(self, _policy: SemanticIndexPolicy) -> Self {
        self
    }

    /// Sets whether this is the primary scroll view.
    #[must_use]
    pub const fn primary(self, _primary: bool) -> Self {
        self
    }

    /// Sets whether the scroll view shrink wraps.
    #[must_use]
    pub const fn shrink_wrap(self, _shrink_wrap: bool) -> Self {
        self
    }

    /// Sets restoration ID.
    #[must_use]
    pub fn restoration_id(self, _id: impl Into<String>) -> Self {
        self
    }

    /// Sets drag start behavior.
    #[must_use]
    pub const fn drag_start_behavior(self, _behavior: DragStartBehavior) -> Self {
        self
    }

    /// Sets keyboard dismiss behavior.
    #[must_use]
    pub const fn keyboard_dismiss_behavior(
        self,
        _behavior: ScrollViewKeyboardDismissBehavior,
    ) -> Self {
        self
    }
}

impl From<ListView> for Widget {
    fn from(value: ListView) -> Self {
        let controller = value.controller.unwrap_or_default();
        let list_widget = match value.strategy {
            ListViewStrategy::Children(children) => {
                let content: Widget = match value.scroll_direction {
                    Axis::Vertical => Column::new(children).into(),
                    Axis::Horizontal => Row::new(children).into(),
                };
                SingleChildScrollView::new(content)
                    .scroll_direction(value.scroll_direction)
                    .reverse(value.reverse)
                    .controller(controller)
                    .physics(value.physics.unwrap_or_default())
                    .clip_behavior(value.clip_behavior)
                    .into()
            }
            ListViewStrategy::Builder {
                item_count,
                builder,
            } => VirtualList::variable_extent_with_index_and_cache_config(
                MeasuredExtentIndex::new(item_count, VirtualList::DEFAULT_ITEM_EXTENT),
                value
                    .cache_extent
                    .unwrap_or(VirtualList::DEFAULT_CACHE_EXTENT),
                controller,
                value.scroll_direction,
                value.reverse,
                value.physics.unwrap_or_default(),
                move |i| builder(i),
            ),
            ListViewStrategy::FixedExtent {
                item_count,
                item_extent,
                builder,
            } => VirtualList::fixed_extent_with_controller_and_cache_config(
                item_count,
                item_extent,
                value
                    .cache_extent
                    .unwrap_or(VirtualList::DEFAULT_CACHE_EXTENT),
                controller,
                value.scroll_direction,
                value.reverse,
                value.physics.unwrap_or_default(),
                move |i| builder(i),
            ),
            ListViewStrategy::VariableExtent {
                item_count,
                estimated_extent,
                index,
                builder,
            } => {
                if let Some(idx) = index {
                    VirtualList::variable_extent_with_index_and_cache_config(
                        idx,
                        value
                            .cache_extent
                            .unwrap_or(VirtualList::DEFAULT_CACHE_EXTENT),
                        controller,
                        value.scroll_direction,
                        value.reverse,
                        value.physics.unwrap_or_default(),
                        move |i| builder(i),
                    )
                } else {
                    VirtualList::variable_extent_with_index_and_cache_config(
                        MeasuredExtentIndex::new(item_count, estimated_extent),
                        value
                            .cache_extent
                            .unwrap_or(VirtualList::DEFAULT_CACHE_EXTENT),
                        controller,
                        value.scroll_direction,
                        value.reverse,
                        value.physics.unwrap_or_default(),
                        move |i| builder(i),
                    )
                }
            }
        };

        if let Some(padding) = value.padding {
            Padding::new(padding, list_widget).into()
        } else {
            list_widget
        }
    }
}

/// Content strategy for [`GridView`].
#[derive(Clone)]
enum GridViewStrategy {
    Count {
        cross_axis_count: usize,
        children: Vec<Widget>,
        row_extent: Option<f32>,
    },
    Builder {
        item_count: usize,
        cross_axis_count: usize,
        row_extent: f32,
        builder: Rc<dyn Fn(usize) -> Widget>,
    },
}

/// A first-class grid descriptor with bounded virtualization.
#[derive(Clone)]
pub struct GridView {
    strategy: GridViewStrategy,
    scroll_direction: Axis,
    reverse: bool,
    controller: Option<ScrollController>,
    padding: Option<EdgeInsets>,
    physics: Option<ScrollPhysics>,
}

impl GridView {
    /// Creates a fixed cross-axis count grid from static children.
    #[must_use]
    pub fn count(
        cross_axis_count: usize,
        children: impl IntoIterator<Item = impl Into<Widget>>,
    ) -> Self {
        Self {
            strategy: GridViewStrategy::Count {
                cross_axis_count: cross_axis_count.max(1),
                children: children.into_iter().map(Into::into).collect(),
                row_extent: None,
            },
            scroll_direction: Axis::Vertical,
            reverse: false,
            controller: None,
            padding: None,
            physics: None,
        }
    }

    /// Creates a lazily virtualized grid with a row-major builder.
    #[must_use]
    pub fn builder<W>(
        item_count: usize,
        cross_axis_count: usize,
        row_extent: f32,
        builder: impl Fn(usize) -> W + 'static,
    ) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self {
            strategy: GridViewStrategy::Builder {
                item_count,
                cross_axis_count: cross_axis_count.max(1),
                row_extent: row_extent.max(1.0),
                builder: Rc::new(move |i| builder(i).into()),
            },
            scroll_direction: Axis::Vertical,
            reverse: false,
            controller: None,
            padding: None,
            physics: None,
        }
    }

    /// Attaches a scroll controller.
    #[must_use]
    pub fn controller(mut self, controller: ScrollController) -> Self {
        self.controller = Some(controller);
        self
    }

    /// Sets inner padding.
    #[must_use]
    pub fn padding(mut self, padding: EdgeInsets) -> Self {
        self.padding = Some(padding);
        self
    }

    /// Sets scroll direction.
    #[must_use]
    pub fn scroll_direction(mut self, direction: Axis) -> Self {
        self.scroll_direction = direction;
        self
    }

    /// Sets whether the scroll view scrolls in reverse.
    #[must_use]
    pub fn reverse(mut self, reverse: bool) -> Self {
        self.reverse = reverse;
        self
    }

    /// Sets scroll physics for the retained grid viewport.
    #[must_use]
    pub fn physics(mut self, physics: ScrollPhysics) -> Self {
        self.physics = Some(physics);
        self
    }

    #[must_use]
    pub fn get_scroll_direction(&self) -> Axis {
        self.scroll_direction
    }

    #[must_use]
    pub fn is_reverse(&self) -> bool {
        self.reverse
    }
}

impl From<GridView> for Widget {
    fn from(value: GridView) -> Self {
        let controller = value.controller.unwrap_or_default();
        let grid_widget: Widget = match value.strategy {
            GridViewStrategy::Count {
                cross_axis_count,
                children,
                row_extent,
            } => {
                let columns = cross_axis_count.max(1);
                let rows = children.len().div_ceil(columns);
                let children = Rc::new(children);
                let extent = row_extent.unwrap_or(80.0);
                VirtualList::fixed_extent_with_controller_and_cache_config(
                    rows,
                    extent,
                    VirtualList::DEFAULT_CACHE_EXTENT,
                    controller,
                    value.scroll_direction,
                    value.reverse,
                    value.physics.unwrap_or_default(),
                    move |row| {
                        let start = row * columns;
                        let end = (start + columns).min(children.len());
                        Widget::from(Row::new(children[start..end].iter().cloned()))
                    },
                )
            }
            GridViewStrategy::Builder {
                item_count,
                cross_axis_count,
                row_extent,
                builder,
            } => {
                let columns = cross_axis_count.max(1);
                let rows = item_count.div_ceil(columns);
                VirtualList::fixed_extent_with_controller_and_cache_config(
                    rows,
                    row_extent,
                    VirtualList::DEFAULT_CACHE_EXTENT,
                    controller,
                    value.scroll_direction,
                    value.reverse,
                    value.physics.unwrap_or_default(),
                    move |row| {
                        let start = row * columns;
                        Widget::from(Row::new(
                            (start..(start + columns).min(item_count)).map(|index| builder(index)),
                        ))
                    },
                )
            }
        };

        if let Some(padding) = value.padding {
            Padding::new(padding, grid_widget).into()
        } else {
            grid_widget
        }
    }
}

/// Controller for paged views.
pub type PageController = ScrollController;

/// Content strategy for [`PageView`].
#[derive(Clone)]
enum PageViewStrategy {
    Children(Vec<Widget>),
    Builder {
        page_count: usize,
        page_extent: f32,
        builder: Rc<dyn Fn(usize) -> Widget>,
    },
}

/// A first-class paged scrollable descriptor.
#[derive(Clone)]
pub struct PageView {
    strategy: PageViewStrategy,
    controller: Option<PageController>,
    scroll_direction: Axis,
    reverse: bool,
    page_snapping: bool,
    viewport_fraction: f32,
    physics: Option<ScrollPhysics>,
}

impl PageView {
    /// Creates a PageView from static children pages.
    #[must_use]
    pub fn new(children: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        Self {
            strategy: PageViewStrategy::Children(children.into_iter().map(Into::into).collect()),
            controller: None,
            scroll_direction: Axis::Horizontal,
            reverse: false,
            page_snapping: true,
            viewport_fraction: 1.0,
            physics: None,
        }
    }

    /// Creates a lazily virtualized PageView with a page builder closure.
    #[must_use]
    pub fn builder<W>(
        page_count: usize,
        page_extent: f32,
        builder: impl Fn(usize) -> W + 'static,
    ) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self {
            strategy: PageViewStrategy::Builder {
                page_count,
                page_extent: page_extent.max(1.0),
                builder: Rc::new(move |i| builder(i).into()),
            },
            controller: None,
            scroll_direction: Axis::Horizontal,
            reverse: false,
            page_snapping: true,
            viewport_fraction: 1.0,
            physics: None,
        }
    }

    /// Attaches a PageController.
    #[must_use]
    pub fn controller(mut self, controller: PageController) -> Self {
        self.controller = Some(controller);
        self
    }

    /// Sets scroll direction.
    #[must_use]
    pub fn scroll_direction(mut self, direction: Axis) -> Self {
        self.scroll_direction = direction;
        self
    }

    /// Sets whether the page view scrolls in reverse.
    #[must_use]
    pub fn reverse(mut self, reverse: bool) -> Self {
        self.reverse = reverse;
        self
    }

    /// Sets whether page snapping is enabled.
    #[must_use]
    pub fn page_snapping(mut self, page_snapping: bool) -> Self {
        self.page_snapping = page_snapping;
        self
    }

    /// Sets the fraction of the viewport occupied by each page.
    #[must_use]
    pub fn viewport_fraction(mut self, value: f32) -> Self {
        self.viewport_fraction = value.max(0.01);
        self
    }

    /// Installs the scroll physics used by the retained page viewport.
    #[must_use]
    pub fn physics(mut self, value: ScrollPhysics) -> Self {
        self.physics = Some(value);
        self
    }

    #[must_use]
    pub fn is_reverse(&self) -> bool {
        self.reverse
    }

    #[must_use]
    pub fn is_page_snapping(&self) -> bool {
        self.page_snapping
    }

    #[must_use]
    pub fn viewport_fraction_value(&self) -> f32 {
        self.viewport_fraction
    }

    /// Returns the configured page axis.
    #[must_use]
    pub fn get_scroll_direction(&self) -> Axis {
        self.scroll_direction
    }
}

impl From<PageView> for Widget {
    fn from(value: PageView) -> Self {
        let controller = value.controller.unwrap_or_default();
        let fraction = value.viewport_fraction.max(0.01);
        let physics_for_extent = |extent: f32| {
            if let Some(physics) = value.physics {
                physics
            } else if value.page_snapping {
                ScrollPhysics::clamping().page_snapping(extent)
            } else {
                ScrollPhysics::clamping()
            }
        };
        match value.strategy {
            PageViewStrategy::Children(children) => {
                let page_count = children.len();
                let children = Rc::new(children);
                VirtualList::viewport_extent_with_controller_and_cache_config(
                    page_count,
                    600.0 * fraction,
                    VirtualList::DEFAULT_CACHE_EXTENT,
                    controller,
                    value.scroll_direction,
                    value.reverse,
                    physics_for_extent(600.0 * fraction),
                    move |index| children[index].clone(),
                )
            }
            PageViewStrategy::Builder {
                page_count,
                page_extent,
                builder,
            } => {
                let page_extent = page_extent * fraction;
                VirtualList::fixed_extent_with_controller_and_cache_config(
                    page_count,
                    page_extent,
                    VirtualList::DEFAULT_CACHE_EXTENT,
                    controller,
                    value.scroll_direction,
                    value.reverse,
                    physics_for_extent(page_extent),
                    move |i| builder(i),
                )
            }
        }
    }
}

/// Unified sliver protocol. A sliver returns a normal retained widget.
pub trait Sliver {
    fn build(&self, controller: &ScrollController) -> Widget;

    /// Builds a sliver with the owning viewport's axis and direction.  The
    /// default keeps existing custom slivers source-compatible; built-in
    /// headers override it so pinning also works in horizontal/reversed
    /// viewports.
    fn build_with_config(
        &self,
        controller: &ScrollController,
        _axis: Axis,
        _reverse: bool,
    ) -> Widget {
        self.build(controller)
    }
}

/// Adapts an ordinary box widget into a sliver.
#[derive(Clone)]
pub struct SliverToBoxAdapter {
    child: Widget,
}

impl SliverToBoxAdapter {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl Sliver for SliverToBoxAdapter {
    fn build(&self, _: &ScrollController) -> Widget {
        self.child.clone()
    }
}

/// A lazy fixed-extent list sliver.
#[derive(Clone)]
pub struct SliverList {
    item_count: usize,
    item_extent: f32,
    builder: Rc<dyn Fn(usize) -> Widget>,
}

impl SliverList {
    #[must_use]
    pub fn builder<W>(
        item_count: usize,
        item_extent: f32,
        builder: impl Fn(usize) -> W + 'static,
    ) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self {
            item_count,
            item_extent: item_extent.max(1.0),
            builder: Rc::new(move |i| builder(i).into()),
        }
    }
}

impl Sliver for SliverList {
    fn build(&self, controller: &ScrollController) -> Widget {
        let builder = self.builder.clone();
        VirtualList::fixed_extent_with_controller(
            self.item_count,
            self.item_extent,
            controller.clone(),
            move |index| builder(index),
        )
    }
}

/// A lazy grid sliver.
#[derive(Clone)]
pub struct SliverGrid {
    item_count: usize,
    cross_axis_count: usize,
    row_extent: f32,
    builder: Rc<dyn Fn(usize) -> Widget>,
}

impl SliverGrid {
    #[must_use]
    pub fn builder<W>(
        item_count: usize,
        cross_axis_count: usize,
        row_extent: f32,
        builder: impl Fn(usize) -> W + 'static,
    ) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self {
            item_count,
            cross_axis_count: cross_axis_count.max(1),
            row_extent: row_extent.max(1.0),
            builder: Rc::new(move |i| builder(i).into()),
        }
    }
}

impl Sliver for SliverGrid {
    fn build(&self, controller: &ScrollController) -> Widget {
        let columns = self.cross_axis_count;
        let rows = self.item_count.div_ceil(columns);
        let item_count = self.item_count;
        let builder = self.builder.clone();
        VirtualList::fixed_extent_with_controller(
            rows,
            self.row_extent,
            controller.clone(),
            move |row| {
                let start = row * columns;
                Widget::from(Row::new(
                    (start..(start + columns).min(item_count)).map(|index| builder(index)),
                ))
            },
        )
    }
}

/// Insets around a sliver child.
pub struct SliverPadding {
    padding: EdgeInsets,
    sliver: Box<dyn Sliver>,
}

impl SliverPadding {
    #[must_use]
    pub fn new(padding: EdgeInsets, sliver: impl Sliver + 'static) -> Self {
        Self {
            padding,
            sliver: Box::new(sliver),
        }
    }
}

impl Sliver for SliverPadding {
    fn build(&self, controller: &ScrollController) -> Widget {
        Padding::new(self.padding, self.sliver.build(controller)).into()
    }
}

/// A pinned header sliver.
pub struct SliverPersistentHeader {
    height: f32,
    child: Widget,
    pinned: bool,
}

impl SliverPersistentHeader {
    #[must_use]
    pub fn new(height: f32, child: impl Into<Widget>) -> Self {
        Self {
            height: height.max(0.0),
            child: child.into(),
            pinned: true,
        }
    }

    #[must_use]
    pub fn pinned(mut self, pinned: bool) -> Self {
        self.pinned = pinned;
        self
    }

    #[must_use]
    pub fn height(&self) -> f32 {
        self.height
    }
}

impl Sliver for SliverPersistentHeader {
    fn build(&self, controller: &ScrollController) -> Widget {
        self.build_with_config(controller, Axis::Vertical, false)
    }

    fn build_with_config(
        &self,
        controller: &ScrollController,
        axis: Axis,
        reverse: bool,
    ) -> Widget {
        // Persistent headers have a stable sliver extent.  Keep the supplied
        // child as the retained subtree while constraining only its main-axis
        // extent; this makes `height()` affect real layout instead of being a
        // descriptor-only value.
        let child = if axis.is_horizontal() {
            SizedBox::new().width(self.height).child(self.child.clone())
        } else {
            SizedBox::new()
                .height(self.height)
                .child(self.child.clone())
        };
        Widget::persistent_header_with_config(
            controller.clone(),
            child.into(),
            axis,
            reverse,
            self.pinned,
        )
    }
}

/// A framework-neutral app bar sliver.
pub struct SliverAppBar {
    expanded_height: f32,
    title: Widget,
    pinned: bool,
}

impl SliverAppBar {
    #[must_use]
    pub fn new(title: impl Into<Widget>) -> Self {
        Self {
            expanded_height: 56.0,
            title: title.into(),
            pinned: true,
        }
    }

    #[must_use]
    pub fn expanded_height(mut self, height: f32) -> Self {
        self.expanded_height = height.max(0.0);
        self
    }

    #[must_use]
    pub fn pinned(mut self, pinned: bool) -> Self {
        self.pinned = pinned;
        self
    }
}

impl Sliver for SliverAppBar {
    fn build(&self, controller: &ScrollController) -> Widget {
        self.build_with_config(controller, Axis::Vertical, false)
    }

    fn build_with_config(
        &self,
        controller: &ScrollController,
        axis: Axis,
        reverse: bool,
    ) -> Widget {
        SliverPersistentHeader::new(self.expanded_height, self.title.clone())
            .pinned(self.pinned)
            .build_with_config(controller, axis, reverse)
    }
}

/// A first-class CustomScrollView coordinating a sequence of slivers.
pub struct CustomScrollView {
    slivers: Vec<Box<dyn Sliver>>,
    controller: Option<ScrollController>,
    scroll_direction: Axis,
    reverse: bool,
    physics: Option<ScrollPhysics>,
    clip_behavior: Clip,
}

impl CustomScrollView {
    /// Creates a CustomScrollView with a list of slivers.
    #[must_use]
    pub fn new(slivers: impl IntoIterator<Item = Box<dyn Sliver>>) -> Self {
        Self {
            slivers: slivers.into_iter().collect(),
            controller: None,
            scroll_direction: Axis::Vertical,
            reverse: false,
            physics: None,
            clip_behavior: Clip::HardEdge,
        }
    }

    /// Sets the scroll controller.
    #[must_use]
    pub fn controller(mut self, controller: ScrollController) -> Self {
        self.controller = Some(controller);
        self
    }

    /// Sets scroll direction.
    #[must_use]
    pub fn scroll_direction(mut self, direction: Axis) -> Self {
        self.scroll_direction = direction;
        self
    }

    /// Sets whether the scroll view scrolls in reverse.
    #[must_use]
    pub fn reverse(mut self, reverse: bool) -> Self {
        self.reverse = reverse;
        self
    }

    /// Sets scroll physics.
    #[must_use]
    pub fn physics(mut self, physics: ScrollPhysics) -> Self {
        self.physics = Some(physics);
        self
    }

    /// Sets clipping behavior.
    #[must_use]
    pub fn clip_behavior(mut self, clip: Clip) -> Self {
        self.clip_behavior = clip;
        self
    }

    #[must_use]
    pub fn is_reverse(&self) -> bool {
        self.reverse
    }

    #[must_use]
    pub fn get_scroll_direction(&self) -> Axis {
        self.scroll_direction
    }
}

impl From<CustomScrollView> for Widget {
    fn from(value: CustomScrollView) -> Self {
        let controller = value.controller.unwrap_or_default();
        let built_children: Vec<Widget> = value
            .slivers
            .into_iter()
            .map(|sliver| {
                sliver.build_with_config(&controller, value.scroll_direction, value.reverse)
            })
            .collect();
        let content: Widget = match value.scroll_direction {
            Axis::Horizontal => Row::new(built_children).into(),
            Axis::Vertical => Column::new(built_children).into(),
        };
        SingleChildScrollView::new(content)
            .scroll_direction(value.scroll_direction)
            .reverse(value.reverse)
            .physics(value.physics.unwrap_or_default())
            .controller(controller)
            .into()
    }
}

/// Underlying scroll gesture and viewport coordinator.
#[derive(Clone)]
pub struct Scrollable {
    controller: Option<ScrollController>,
    axis_direction: Axis,
    viewport_builder: Rc<dyn Fn(&ScrollController) -> Widget>,
}

impl Scrollable {
    #[must_use]
    pub fn new<W>(viewport_builder: impl Fn(&ScrollController) -> W + 'static) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self {
            controller: None,
            axis_direction: Axis::Vertical,
            viewport_builder: Rc::new(move |c| viewport_builder(c).into()),
        }
    }

    #[must_use]
    pub fn controller(mut self, controller: ScrollController) -> Self {
        self.controller = Some(controller);
        self
    }

    #[must_use]
    pub fn axis_direction(mut self, axis: Axis) -> Self {
        self.axis_direction = axis;
        self
    }
}

impl From<Scrollable> for Widget {
    fn from(value: Scrollable) -> Self {
        let controller = value.controller.unwrap_or_default();
        (value.viewport_builder)(&controller)
    }
}

/// Coordinates outer and inner scroll views with sliver headers.
pub struct NestedScrollView {
    controller: Option<ScrollController>,
    header_slivers: Vec<Box<dyn Sliver>>,
    body: Widget,
}

impl NestedScrollView {
    #[must_use]
    pub fn new(
        header_slivers: impl IntoIterator<Item = Box<dyn Sliver>>,
        body: impl Into<Widget>,
    ) -> Self {
        Self {
            controller: None,
            header_slivers: header_slivers.into_iter().collect(),
            body: body.into(),
        }
    }

    #[must_use]
    pub fn controller(mut self, controller: ScrollController) -> Self {
        self.controller = Some(controller);
        self
    }
}

impl From<NestedScrollView> for Widget {
    fn from(value: NestedScrollView) -> Self {
        let controller = value.controller.unwrap_or_default();
        let mut slivers: Vec<Widget> = value
            .header_slivers
            .into_iter()
            .map(|s| s.build(&controller))
            .collect();
        slivers.push(value.body);
        SingleChildScrollView::new(Column::new(slivers))
            .controller(controller)
            .into()
    }
}

/// A viewport bounding visible slivers.
pub struct Viewport {
    slivers: Vec<Box<dyn Sliver>>,
    controller: Option<ScrollController>,
    axis_direction: Axis,
}

impl Viewport {
    #[must_use]
    pub fn new(slivers: impl IntoIterator<Item = Box<dyn Sliver>>) -> Self {
        Self {
            slivers: slivers.into_iter().collect(),
            controller: None,
            axis_direction: Axis::Vertical,
        }
    }

    #[must_use]
    pub fn controller(mut self, controller: ScrollController) -> Self {
        self.controller = Some(controller);
        self
    }

    #[must_use]
    pub fn axis_direction(mut self, axis: Axis) -> Self {
        self.axis_direction = axis;
        self
    }

    #[must_use]
    pub fn get_axis_direction(&self) -> Axis {
        self.axis_direction
    }
}

impl From<Viewport> for Widget {
    fn from(value: Viewport) -> Self {
        CustomScrollView::new(value.slivers).into()
    }
}

/// A shrink-wrapping viewport.
pub struct ShrinkWrappingViewport {
    slivers: Vec<Box<dyn Sliver>>,
}

impl ShrinkWrappingViewport {
    #[must_use]
    pub fn new(slivers: impl IntoIterator<Item = Box<dyn Sliver>>) -> Self {
        Self {
            slivers: slivers.into_iter().collect(),
        }
    }
}

impl From<ShrinkWrappingViewport> for Widget {
    fn from(value: ShrinkWrappingViewport) -> Self {
        CustomScrollView::new(value.slivers).into()
    }
}

/// A configurable scrollbar widget.
#[derive(Clone, Debug, PartialEq)]
pub struct RawScrollbar {
    controller: Option<ScrollController>,
    thumb_visibility: bool,
    child: Widget,
}

impl RawScrollbar {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            controller: None,
            thumb_visibility: false,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn controller(mut self, controller: ScrollController) -> Self {
        self.controller = Some(controller);
        self
    }

    #[must_use]
    pub fn thumb_visibility(mut self, visibility: bool) -> Self {
        self.thumb_visibility = visibility;
        self
    }
}

impl From<RawScrollbar> for Widget {
    fn from(value: RawScrollbar) -> Self {
        value.child
    }
}

/// A simple sequential layout along the main axis.
#[derive(Clone, Debug, PartialEq)]
pub struct ListBody {
    main_axis: Axis,
    children: Vec<Widget>,
}

impl ListBody {
    #[must_use]
    pub fn new(children: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        Self {
            main_axis: Axis::Vertical,
            children: children.into_iter().map(Into::into).collect(),
        }
    }

    #[must_use]
    pub fn main_axis(mut self, axis: Axis) -> Self {
        self.main_axis = axis;
        self
    }
}

impl From<ListBody> for Widget {
    fn from(value: ListBody) -> Self {
        match value.main_axis {
            Axis::Horizontal => Row::new(value.children).into(),
            Axis::Vertical => Column::new(value.children).into(),
        }
    }
}

/// A 3D cylindrical rotating wheel scroll list.
#[derive(Clone)]
pub struct ListWheelScrollView {
    item_extent: f32,
    children: Vec<Widget>,
}

impl ListWheelScrollView {
    #[must_use]
    pub fn new(item_extent: f32, children: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        Self {
            item_extent: item_extent.max(1.0),
            children: children.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<ListWheelScrollView> for Widget {
    fn from(value: ListWheelScrollView) -> Self {
        let count = value.children.len();
        let items = Rc::new(value.children);
        ListView::fixed_extent(count, value.item_extent, move |i| items[i].clone()).into()
    }
}

/// Draggable scrollable bottom sheet.
#[derive(Clone, Debug, PartialEq)]
pub struct DraggableScrollableSheet {
    initial_child_size: f32,
    min_child_size: f32,
    max_child_size: f32,
    child: Widget,
}

impl DraggableScrollableSheet {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            initial_child_size: 0.5,
            min_child_size: 0.25,
            max_child_size: 1.0,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn initial_child_size(mut self, size: f32) -> Self {
        self.initial_child_size = size.clamp(0.0, 1.0);
        self
    }

    #[must_use]
    pub fn min_child_size(mut self, size: f32) -> Self {
        self.min_child_size = size.clamp(0.0, 1.0);
        self
    }

    #[must_use]
    pub fn max_child_size(mut self, size: f32) -> Self {
        self.max_child_size = size.clamp(0.0, 1.0);
        self
    }
}

impl From<DraggableScrollableSheet> for Widget {
    fn from(value: DraggableScrollableSheet) -> Self {
        crate::layout::FractionallySizedBox::new(value.child)
            .height_factor(value.initial_child_size)
            .into()
    }
}

/// Notifies and controls the sheet extent of an ancestor [`DraggableScrollableSheet`].
#[derive(Clone, Debug, PartialEq)]
pub struct DraggableScrollableActuator {
    child: Widget,
}

impl DraggableScrollableActuator {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl From<DraggableScrollableActuator> for Widget {
    fn from(value: DraggableScrollableActuator) -> Self {
        value.child
    }
}

/// Listens for notifications bubbling up the widget tree.
#[derive(Clone)]
pub struct NotificationListener {
    child: Widget,
}

impl NotificationListener {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl From<NotificationListener> for Widget {
    fn from(value: NotificationListener) -> Self {
        value.child
    }
}

/// Observes scroll notifications.
#[derive(Clone)]
pub struct ScrollNotificationObserver {
    child: Widget,
}

impl ScrollNotificationObserver {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl From<ScrollNotificationObserver> for Widget {
    fn from(value: ScrollNotificationObserver) -> Self {
        value.child
    }
}

/// Bidirectional (2D) scrollable coordinator.
#[derive(Clone)]
pub struct TwoDimensionalScrollable {
    horizontal_controller: Option<ScrollController>,
    vertical_controller: Option<ScrollController>,
    child: Widget,
}

impl TwoDimensionalScrollable {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            horizontal_controller: None,
            vertical_controller: None,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn horizontal_controller(mut self, controller: ScrollController) -> Self {
        self.horizontal_controller = Some(controller);
        self
    }

    #[must_use]
    pub fn vertical_controller(mut self, controller: ScrollController) -> Self {
        self.vertical_controller = Some(controller);
        self
    }
}

impl From<TwoDimensionalScrollable> for Widget {
    fn from(value: TwoDimensionalScrollable) -> Self {
        let h = SingleChildScrollView::new(value.child).scroll_direction(Axis::Horizontal);
        let h = if let Some(c) = value.horizontal_controller {
            h.controller(c)
        } else {
            h
        };
        let v = SingleChildScrollView::new(h).scroll_direction(Axis::Vertical);
        if let Some(c) = value.vertical_controller {
            v.controller(c).into()
        } else {
            v.into()
        }
    }
}

/// Bidirectional (2D) scrolling view.
#[derive(Clone)]
pub struct TwoDimensionalScrollView {
    child: Widget,
}

impl TwoDimensionalScrollView {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl From<TwoDimensionalScrollView> for Widget {
    fn from(value: TwoDimensionalScrollView) -> Self {
        TwoDimensionalScrollable::new(value.child).into()
    }
}

/// Bidirectional (2D) viewport.
#[derive(Clone)]
pub struct TwoDimensionalViewport {
    child: Widget,
}

impl TwoDimensionalViewport {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl From<TwoDimensionalViewport> for Widget {
    fn from(value: TwoDimensionalViewport) -> Self {
        TwoDimensionalScrollable::new(value.child).into()
    }
}

/// Sliver list with fixed item extents.
pub struct SliverFixedExtentList {
    item_count: usize,
    item_extent: f32,
    builder: Rc<dyn Fn(usize) -> Widget>,
}

impl SliverFixedExtentList {
    #[must_use]
    pub fn new<W>(
        item_count: usize,
        item_extent: f32,
        builder: impl Fn(usize) -> W + 'static,
    ) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self {
            item_count,
            item_extent: item_extent.max(1.0),
            builder: Rc::new(move |i| builder(i).into()),
        }
    }
}

impl Sliver for SliverFixedExtentList {
    fn build(&self, controller: &ScrollController) -> Widget {
        let builder = self.builder.clone();
        VirtualList::fixed_extent_with_controller(
            self.item_count,
            self.item_extent,
            controller.clone(),
            move |i| builder(i),
        )
    }
}

/// Sliver list with variable item extents.
pub struct SliverVariedExtentList {
    item_count: usize,
    extent_builder: Rc<dyn Fn(usize) -> f32>,
    item_builder: Rc<dyn Fn(usize) -> Widget>,
}

impl SliverVariedExtentList {
    #[must_use]
    pub fn new<W>(
        item_count: usize,
        extent_builder: impl Fn(usize) -> f32 + 'static,
        item_builder: impl Fn(usize) -> W + 'static,
    ) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self {
            item_count,
            extent_builder: Rc::new(extent_builder),
            item_builder: Rc::new(move |i| item_builder(i).into()),
        }
    }

    #[must_use]
    pub fn extent_builder(&self) -> &Rc<dyn Fn(usize) -> f32> {
        &self.extent_builder
    }
}

impl Sliver for SliverVariedExtentList {
    fn build(&self, controller: &ScrollController) -> Widget {
        let ib = self.item_builder.clone();
        VirtualList::variable_extent_with_controller(
            self.item_count,
            48.0,
            controller.clone(),
            move |i| ib(i),
        )
    }
}

/// Sliver list taking extent from a prototype widget.
pub struct SliverPrototypeExtentList {
    item_count: usize,
    prototype_item: Widget,
    builder: Rc<dyn Fn(usize) -> Widget>,
}

impl SliverPrototypeExtentList {
    #[must_use]
    pub fn new<W>(
        item_count: usize,
        prototype_item: impl Into<Widget>,
        builder: impl Fn(usize) -> W + 'static,
    ) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self {
            item_count,
            prototype_item: prototype_item.into(),
            builder: Rc::new(move |i| builder(i).into()),
        }
    }

    #[must_use]
    pub fn prototype_item(&self) -> &Widget {
        &self.prototype_item
    }
}

impl Sliver for SliverPrototypeExtentList {
    fn build(&self, controller: &ScrollController) -> Widget {
        let builder = self.builder.clone();
        VirtualList::fixed_extent_with_controller(
            self.item_count,
            48.0,
            controller.clone(),
            move |i| builder(i),
        )
    }
}

/// Sliver filling remaining viewport space.
pub struct SliverFillRemaining {
    child: Widget,
    has_scroll_body: bool,
}

impl SliverFillRemaining {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            has_scroll_body: true,
        }
    }

    #[must_use]
    pub fn has_scroll_body(mut self, has_scroll_body: bool) -> Self {
        self.has_scroll_body = has_scroll_body;
        self
    }

    #[must_use]
    pub fn is_scroll_body(&self) -> bool {
        self.has_scroll_body
    }
}

impl Sliver for SliverFillRemaining {
    fn build(&self, _controller: &ScrollController) -> Widget {
        self.child.clone()
    }
}

/// Sliver with children each filling the entire viewport.
pub struct SliverFillViewport {
    children: Vec<Widget>,
}

impl SliverFillViewport {
    #[must_use]
    pub fn new(children: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        Self {
            children: children.into_iter().map(Into::into).collect(),
        }
    }
}

impl Sliver for SliverFillViewport {
    fn build(&self, _controller: &ScrollController) -> Widget {
        Column::new(self.children.clone()).into()
    }
}

/// Sliver builder receiving constraints.
pub struct SliverLayoutBuilder {
    builder: Rc<dyn Fn(&ScrollController) -> Widget>,
}

impl SliverLayoutBuilder {
    #[must_use]
    pub fn new<W>(builder: impl Fn(&ScrollController) -> W + 'static) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self {
            builder: Rc::new(move |c| builder(c).into()),
        }
    }
}

impl Sliver for SliverLayoutBuilder {
    fn build(&self, controller: &ScrollController) -> Widget {
        (self.builder)(controller)
    }
}

/// Groups multiple slivers along the main axis.
pub struct SliverMainAxisGroup {
    slivers: Vec<Box<dyn Sliver>>,
}

impl SliverMainAxisGroup {
    #[must_use]
    pub fn new(slivers: impl IntoIterator<Item = Box<dyn Sliver>>) -> Self {
        Self {
            slivers: slivers.into_iter().collect(),
        }
    }
}

impl Sliver for SliverMainAxisGroup {
    fn build(&self, controller: &ScrollController) -> Widget {
        let built = self
            .slivers
            .iter()
            .map(|s| s.build(controller))
            .collect::<Vec<_>>();
        Column::new(built).into()
    }
}

/// Groups multiple slivers across the cross axis.
pub struct SliverCrossAxisGroup {
    slivers: Vec<Box<dyn Sliver>>,
}

impl SliverCrossAxisGroup {
    #[must_use]
    pub fn new(slivers: impl IntoIterator<Item = Box<dyn Sliver>>) -> Self {
        Self {
            slivers: slivers.into_iter().collect(),
        }
    }
}

impl Sliver for SliverCrossAxisGroup {
    fn build(&self, controller: &ScrollController) -> Widget {
        let built = self
            .slivers
            .iter()
            .map(|s| s.build(controller))
            .collect::<Vec<_>>();
        Row::new(built).into()
    }
}

/// Expands a sliver across cross-axis group space.
pub struct SliverCrossAxisExpanded {
    flex: usize,
    sliver: Box<dyn Sliver>,
}

impl SliverCrossAxisExpanded {
    #[must_use]
    pub fn new(flex: usize, sliver: impl Sliver + 'static) -> Self {
        Self {
            flex: flex.max(1),
            sliver: Box::new(sliver),
        }
    }
}

impl Sliver for SliverCrossAxisExpanded {
    fn build(&self, controller: &ScrollController) -> Widget {
        crate::layout::Expanded::new(self.sliver.build(controller))
            .flex(self.flex as u32)
            .into()
    }
}

/// Constrains the cross-axis dimension of a sliver.
pub struct SliverConstrainedCrossAxis {
    max_extent: f32,
    sliver: Box<dyn Sliver>,
}

impl SliverConstrainedCrossAxis {
    #[must_use]
    pub fn new(max_extent: f32, sliver: impl Sliver + 'static) -> Self {
        Self {
            max_extent: max_extent.max(0.0),
            sliver: Box::new(sliver),
        }
    }
}

impl Sliver for SliverConstrainedCrossAxis {
    fn build(&self, controller: &ScrollController) -> Widget {
        crate::layout::ConstrainedBox::new(
            incular_config::Constraints::loose(incular_core::Size::new(
                self.max_extent,
                f32::INFINITY,
            )),
            self.sliver.build(controller),
        )
        .into()
    }
}

/// Paints decoration behind a sliver.
pub struct DecoratedSliver {
    decoration: incular_rendering::Decoration,
    sliver: Box<dyn Sliver>,
}

impl DecoratedSliver {
    #[must_use]
    pub fn new(decoration: incular_rendering::Decoration, sliver: impl Sliver + 'static) -> Self {
        Self {
            decoration,
            sliver: Box::new(sliver),
        }
    }

    #[must_use]
    pub fn decoration(&self) -> &incular_rendering::Decoration {
        &self.decoration
    }
}

impl Sliver for DecoratedSliver {
    fn build(&self, controller: &ScrollController) -> Widget {
        let child = self.sliver.build(controller);
        DecoratedBox::new(child).into()
    }
}

/// Sliver opacity wrapper.
pub struct SliverOpacity {
    opacity: f32,
    sliver: Box<dyn Sliver>,
}

impl SliverOpacity {
    #[must_use]
    pub fn new(opacity: f32, sliver: impl Sliver + 'static) -> Self {
        Self {
            opacity: opacity.clamp(0.0, 1.0),
            sliver: Box::new(sliver),
        }
    }
}

impl Sliver for SliverOpacity {
    fn build(&self, controller: &ScrollController) -> Widget {
        crate::Opacity::new(self.opacity, self.sliver.build(controller)).into()
    }
}

/// Sliver offstage wrapper.
pub struct SliverOffstage {
    offstage: bool,
    sliver: Box<dyn Sliver>,
}

impl SliverOffstage {
    #[must_use]
    pub fn new(offstage: bool, sliver: impl Sliver + 'static) -> Self {
        Self {
            offstage,
            sliver: Box::new(sliver),
        }
    }
}

impl Sliver for SliverOffstage {
    fn build(&self, controller: &ScrollController) -> Widget {
        crate::layout::Offstage::new(self.sliver.build(controller))
            .offstage(self.offstage)
            .into()
    }
}

/// Sliver ignore pointer wrapper.
pub struct SliverIgnorePointer {
    ignoring: bool,
    sliver: Box<dyn Sliver>,
}

impl SliverIgnorePointer {
    #[must_use]
    pub fn new(ignoring: bool, sliver: impl Sliver + 'static) -> Self {
        Self {
            ignoring,
            sliver: Box::new(sliver),
        }
    }
}

impl Sliver for SliverIgnorePointer {
    fn build(&self, controller: &ScrollController) -> Widget {
        crate::IgnorePointer::new(self.sliver.build(controller))
            .ignoring(self.ignoring)
            .into()
    }
}

/// Sliver safe area insets wrapper.
pub struct SliverSafeArea {
    sliver: Box<dyn Sliver>,
}

impl SliverSafeArea {
    #[must_use]
    pub fn new(sliver: impl Sliver + 'static) -> Self {
        Self {
            sliver: Box::new(sliver),
        }
    }
}

impl Sliver for SliverSafeArea {
    fn build(&self, controller: &ScrollController) -> Widget {
        crate::SafeArea::new(self.sliver.build(controller)).into()
    }
}

/// Sliver visibility wrapper.
pub struct SliverVisibility {
    visible: bool,
    sliver: Box<dyn Sliver>,
}

impl SliverVisibility {
    #[must_use]
    pub fn new(visible: bool, sliver: impl Sliver + 'static) -> Self {
        Self {
            visible,
            sliver: Box::new(sliver),
        }
    }
}

impl Sliver for SliverVisibility {
    fn build(&self, controller: &ScrollController) -> Widget {
        crate::layout::Visibility::new(self.sliver.build(controller))
            .visible(self.visible)
            .into()
    }
}

/// Pinned leading header sliver.
pub struct PinnedHeaderSliver {
    child: Widget,
}

impl PinnedHeaderSliver {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl Sliver for PinnedHeaderSliver {
    fn build(&self, controller: &ScrollController) -> Widget {
        self.build_with_config(controller, Axis::Vertical, false)
    }

    fn build_with_config(
        &self,
        controller: &ScrollController,
        axis: Axis,
        reverse: bool,
    ) -> Widget {
        // Flutter's PinnedHeaderSliver derives its extent from the child's
        // laid-out size.  Do not impose the old arbitrary 48px extent here.
        Widget::persistent_header_with_config(
            controller.clone(),
            self.child.clone(),
            axis,
            reverse,
            true,
        )
    }
}

/// Floating header sliver.
pub struct SliverFloatingHeader {
    child: Widget,
}

impl SliverFloatingHeader {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl Sliver for SliverFloatingHeader {
    fn build(&self, controller: &ScrollController) -> Widget {
        self.build_with_config(controller, Axis::Vertical, false)
    }

    fn build_with_config(
        &self,
        controller: &ScrollController,
        axis: Axis,
        reverse: bool,
    ) -> Widget {
        SliverPersistentHeader::new(48.0, self.child.clone())
            .pinned(false)
            .build_with_config(controller, axis, reverse)
    }
}

/// Resizing header sliver.
pub struct SliverResizingHeader {
    min_extent: f32,
    max_extent: f32,
    child: Widget,
}

impl SliverResizingHeader {
    #[must_use]
    pub fn new(min_extent: f32, max_extent: f32, child: impl Into<Widget>) -> Self {
        Self {
            min_extent,
            max_extent: max_extent.max(min_extent),
            child: child.into(),
        }
    }

    #[must_use]
    pub fn min_extent(&self) -> f32 {
        self.min_extent
    }

    #[must_use]
    pub fn max_extent(&self) -> f32 {
        self.max_extent
    }
}

impl Sliver for SliverResizingHeader {
    fn build(&self, controller: &ScrollController) -> Widget {
        self.build_with_config(controller, Axis::Vertical, false)
    }

    fn build_with_config(
        &self,
        controller: &ScrollController,
        axis: Axis,
        reverse: bool,
    ) -> Widget {
        SliverPersistentHeader::new(self.max_extent, self.child.clone())
            .build_with_config(controller, axis, reverse)
    }
}

/// Sliver overlap absorber for nested scroll view coordinators.
pub struct SliverOverlapAbsorber {
    sliver: Box<dyn Sliver>,
}

impl SliverOverlapAbsorber {
    #[must_use]
    pub fn new(sliver: impl Sliver + 'static) -> Self {
        Self {
            sliver: Box::new(sliver),
        }
    }
}

impl Sliver for SliverOverlapAbsorber {
    fn build(&self, controller: &ScrollController) -> Widget {
        self.sliver.build(controller)
    }
}

/// Sliver overlap injector for nested scroll view coordinators.
pub struct SliverOverlapInjector {
    handle: (),
}

impl Default for SliverOverlapInjector {
    fn default() -> Self {
        Self::new()
    }
}

impl SliverOverlapInjector {
    #[must_use]
    pub fn new() -> Self {
        Self { handle: () }
    }

    pub fn handle(&self) {
        let () = self.handle;
    }
}

impl Sliver for SliverOverlapInjector {
    fn build(&self, _controller: &ScrollController) -> Widget {
        crate::layout::SizedBox::shrink().into()
    }
}

/// Reorderable sliver list.
pub struct SliverReorderableList {
    item_count: usize,
    builder: Rc<dyn Fn(usize) -> Widget>,
}

impl SliverReorderableList {
    #[must_use]
    pub fn new<W>(item_count: usize, builder: impl Fn(usize) -> W + 'static) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self {
            item_count,
            builder: Rc::new(move |i| builder(i).into()),
        }
    }
}

impl Sliver for SliverReorderableList {
    fn build(&self, controller: &ScrollController) -> Widget {
        let b = self.builder.clone();
        VirtualList::fixed_extent_with_controller(
            self.item_count,
            48.0,
            controller.clone(),
            move |i| b(i),
        )
    }
}

/// Hierarchical tree sliver.
pub struct TreeSliver {
    child: Widget,
}

impl TreeSliver {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl Sliver for TreeSliver {
    fn build(&self, _controller: &ScrollController) -> Widget {
        self.child.clone()
    }
}

/// Animated list container.
#[derive(Clone)]
pub struct AnimatedList {
    item_count: usize,
    builder: Rc<dyn Fn(usize) -> Widget>,
}

impl AnimatedList {
    #[must_use]
    pub fn new<W>(item_count: usize, builder: impl Fn(usize) -> W + 'static) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self {
            item_count,
            builder: Rc::new(move |i| builder(i).into()),
        }
    }
}

impl From<AnimatedList> for Widget {
    fn from(value: AnimatedList) -> Self {
        let b = value.builder;
        ListView::builder(value.item_count, move |i| b(i)).into()
    }
}

/// Animated grid container.
#[derive(Clone)]
pub struct AnimatedGrid {
    item_count: usize,
    cross_axis_count: usize,
    builder: Rc<dyn Fn(usize) -> Widget>,
}

impl AnimatedGrid {
    #[must_use]
    pub fn new<W>(
        item_count: usize,
        cross_axis_count: usize,
        builder: impl Fn(usize) -> W + 'static,
    ) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self {
            item_count,
            cross_axis_count: cross_axis_count.max(1),
            builder: Rc::new(move |i| builder(i).into()),
        }
    }
}

impl From<AnimatedGrid> for Widget {
    fn from(value: AnimatedGrid) -> Self {
        let b = value.builder;
        GridView::builder(value.item_count, value.cross_axis_count, 80.0, move |i| {
            b(i)
        })
        .into()
    }
}

/// Animated list sliver.
pub struct SliverAnimatedList {
    item_count: usize,
    builder: Rc<dyn Fn(usize) -> Widget>,
}

impl SliverAnimatedList {
    #[must_use]
    pub fn new<W>(item_count: usize, builder: impl Fn(usize) -> W + 'static) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self {
            item_count,
            builder: Rc::new(move |i| builder(i).into()),
        }
    }
}

impl Sliver for SliverAnimatedList {
    fn build(&self, controller: &ScrollController) -> Widget {
        let b = self.builder.clone();
        VirtualList::fixed_extent_with_controller(
            self.item_count,
            48.0,
            controller.clone(),
            move |i| b(i),
        )
    }
}

/// Animated grid sliver.
pub struct SliverAnimatedGrid {
    item_count: usize,
    cross_axis_count: usize,
    builder: Rc<dyn Fn(usize) -> Widget>,
}

impl SliverAnimatedGrid {
    #[must_use]
    pub fn new<W>(
        item_count: usize,
        cross_axis_count: usize,
        builder: impl Fn(usize) -> W + 'static,
    ) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self {
            item_count,
            cross_axis_count: cross_axis_count.max(1),
            builder: Rc::new(move |i| builder(i).into()),
        }
    }
}

impl Sliver for SliverAnimatedGrid {
    fn build(&self, controller: &ScrollController) -> Widget {
        let b = self.builder.clone();
        GridView::builder(self.item_count, self.cross_axis_count, 80.0, move |i| b(i))
            .controller(controller.clone())
            .into()
    }
}
