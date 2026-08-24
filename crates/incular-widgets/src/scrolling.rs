//! High-level scroll and unified-sliver descriptors over Incular's retained viewport.

use std::rc::Rc;

use incular_config::{Axis, Clip, EdgeInsets};
use incular_scroll::{MeasuredExtentIndex, ScrollController, ScrollPhysics};

use crate::{Column, Padding, Row, VirtualList, Widget, WidgetKind};

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
}

impl From<SingleChildScrollView> for Widget {
    fn from(value: SingleChildScrollView) -> Self {
        let child = if let Some(padding) = value.padding {
            Padding::new(padding, value.child).into()
        } else {
            value.child
        };
        let controller = value.controller.unwrap_or_default();
        Widget::from_kind(WidgetKind::Scroll {
            controller,
            child: Box::new(child),
        })
    }
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
                    .controller(controller)
                    .into()
            }
            ListViewStrategy::Builder {
                item_count,
                builder,
            } => VirtualList::fixed_extent_with_controller(
                item_count,
                VirtualList::DEFAULT_ITEM_EXTENT,
                controller,
                move |i| builder(i),
            ),
            ListViewStrategy::FixedExtent {
                item_count,
                item_extent,
                builder,
            } => VirtualList::fixed_extent_with_controller(
                item_count,
                item_extent,
                controller,
                move |i| builder(i),
            ),
            ListViewStrategy::VariableExtent {
                item_count,
                estimated_extent,
                index,
                builder,
            } => {
                if let Some(idx) = index {
                    VirtualList::variable_extent_with_index(idx, controller, move |i| builder(i))
                } else {
                    VirtualList::variable_extent_with_controller(
                        item_count,
                        estimated_extent,
                        controller,
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
                VirtualList::fixed_extent_with_controller(rows, extent, controller, move |row| {
                    let start = row * columns;
                    let end = (start + columns).min(children.len());
                    Widget::from(Row::new(children[start..end].iter().cloned()))
                })
            }
            GridViewStrategy::Builder {
                item_count,
                cross_axis_count,
                row_extent,
                builder,
            } => {
                let columns = cross_axis_count.max(1);
                let rows = item_count.div_ceil(columns);
                VirtualList::fixed_extent_with_controller(
                    rows,
                    row_extent,
                    controller,
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
}

impl PageView {
    /// Creates a PageView from static children pages.
    #[must_use]
    pub fn new(children: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        Self {
            strategy: PageViewStrategy::Children(children.into_iter().map(Into::into).collect()),
            controller: None,
            scroll_direction: Axis::Vertical,
            reverse: false,
            page_snapping: true,
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
            scroll_direction: Axis::Vertical,
            reverse: false,
            page_snapping: true,
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

    #[must_use]
    pub fn is_reverse(&self) -> bool {
        self.reverse
    }

    #[must_use]
    pub fn is_page_snapping(&self) -> bool {
        self.page_snapping
    }
}

impl From<PageView> for Widget {
    fn from(value: PageView) -> Self {
        let controller = value.controller.unwrap_or_default();
        match value.strategy {
            PageViewStrategy::Children(children) => {
                let page_count = children.len();
                let children = Rc::new(children);
                VirtualList::fixed_extent_with_controller(
                    page_count,
                    600.0,
                    controller,
                    move |index| children[index].clone(),
                )
            }
            PageViewStrategy::Builder {
                page_count,
                page_extent,
                builder,
            } => VirtualList::fixed_extent_with_controller(
                page_count,
                page_extent,
                controller,
                move |i| builder(i),
            ),
        }
    }
}

/// Unified sliver protocol. A sliver returns a normal retained widget.
pub trait Sliver {
    fn build(&self, controller: &ScrollController) -> Widget;
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

/// Legacy alias for [`SliverToBoxAdapter`].
pub type SliverBox = SliverToBoxAdapter;

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
        Widget::from_kind(WidgetKind::PersistentHeader {
            controller: controller.clone(),
            child: Box::new(self.child.clone()),
        })
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
        SliverPersistentHeader::new(self.expanded_height, self.title.clone())
            .pinned(self.pinned)
            .build(controller)
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
            .map(|sliver| sliver.build(&controller))
            .collect();
        SingleChildScrollView::new(Column::new(built_children))
            .scroll_direction(value.scroll_direction)
            .controller(controller)
            .into()
    }
}
