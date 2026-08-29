use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    time::{Duration, Instant},
};

use incular_animation::AnimationController;
use incular_config::{Axis, Clip, Constraints, EdgeInsets};
use incular_core::Size;
use incular_scroll::{
    DragStartBehavior, MeasuredExtentIndex, ScrollCacheExtent, ScrollController,
    ScrollNotification, ScrollPhysics, ScrollViewKeyboardDismissBehavior, SliverConstraints,
    SliverGeometry,
};
use typed_builder::TypedBuilder;

use crate::drag_drop::DragDropContext;
use crate::tree::WidgetKind;
use crate::{Column, DecoratedBox, DragTarget, Draggable, Padding, Row, SizedBox, Widget};

type SliverLayoutBuilderFn = Rc<dyn Fn(&ScrollController, SliverConstraints) -> Widget>;

const DEFAULT_LAZY_ITEM_EXTENT: f32 = 48.0;
const DEFAULT_SLIVER_CACHE_EXTENT: f32 = 250.0;

/// A first-class scrollable box that scrolls a single child.
#[derive(Clone, TypedBuilder)]
pub struct SingleChildScrollView {
    #[builder(setter(into))]
    child: Widget,
    #[builder(default = Axis::Vertical)]
    scroll_direction: Axis,
    #[builder(default = false)]
    reverse: bool,
    #[builder(default, setter(strip_option))]
    padding: Option<EdgeInsets>,
    #[builder(default, setter(strip_option))]
    controller: Option<ScrollController>,
    #[builder(default, setter(strip_option))]
    physics: Option<ScrollPhysics>,
    #[builder(default = Clip::HardEdge)]
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
#[derive(Clone, TypedBuilder)]
#[builder(builder_method(name = typed_builder))]
pub struct ListView {
    #[builder(default, setter(transform = |children: impl IntoIterator<Item = impl Into<Widget>>| {
        children.into_iter().map(Into::into).collect::<Vec<Widget>>()
    }))]
    children: Vec<Widget>,
    #[builder(default, setter(skip))]
    strategy: Option<ListViewStrategy>,
    #[builder(default = Axis::Vertical)]
    scroll_direction: Axis,
    #[builder(default = false)]
    reverse: bool,
    #[builder(default, setter(strip_option))]
    controller: Option<ScrollController>,
    #[builder(default, setter(strip_option))]
    padding: Option<EdgeInsets>,
    #[builder(default, setter(strip_option))]
    cache_extent: Option<f32>,
    #[builder(default, setter(strip_option))]
    physics: Option<ScrollPhysics>,
    #[builder(default = Clip::HardEdge)]
    clip_behavior: Clip,
}

impl Default for ListView {
    fn default() -> Self {
        Self::typed_builder().build()
    }
}

impl ListView {
    /// Creates a ListView from an explicit collection of child widgets.
    #[must_use]
    pub fn new(children: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        Self {
            children: children.into_iter().map(Into::into).collect(),
            strategy: None,
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
            children: Vec::new(),
            strategy: Some(ListViewStrategy::Builder {
                item_count,
                builder: Rc::new(move |i| builder(i).into()),
            }),
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
            children: Vec::new(),
            strategy: Some(ListViewStrategy::Builder {
                item_count: total_count,
                builder: Rc::new(move |index| {
                    if index % 2 == 0 {
                        item_builder(index / 2)
                    } else {
                        separator_builder(index / 2)
                    }
                }),
            }),
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
            children: Vec::new(),
            strategy: Some(ListViewStrategy::FixedExtent {
                item_count,
                item_extent: item_extent.max(1.0),
                builder: Rc::new(move |i| builder(i).into()),
            }),
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
            children: Vec::new(),
            strategy: Some(ListViewStrategy::VariableExtent {
                item_count,
                estimated_extent: estimated_extent.max(1.0),
                index: None,
                builder: Rc::new(move |i| builder(i).into()),
            }),
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
        if let Some(ListViewStrategy::Builder {
            item_count,
            builder,
        }) = self.strategy
        {
            self.strategy = Some(ListViewStrategy::FixedExtent {
                item_count,
                item_extent: extent.max(1.0),
                builder,
            });
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
        let mut sliver: Box<dyn RenderSliver> = match value
            .strategy
            .unwrap_or_else(|| ListViewStrategy::Children(value.children))
        {
            ListViewStrategy::Children(children) => {
                let children = Rc::new(children);
                Box::new(VariableExtentRenderSliver::new(
                    MeasuredExtentIndex::new(children.len(), DEFAULT_LAZY_ITEM_EXTENT),
                    Rc::new(move |index| children[index].clone()),
                ))
            }
            ListViewStrategy::Builder {
                item_count,
                builder,
            } => Box::new(VariableExtentRenderSliver::new(
                MeasuredExtentIndex::new(item_count, DEFAULT_LAZY_ITEM_EXTENT),
                builder,
            )),
            ListViewStrategy::FixedExtent {
                item_count,
                item_extent,
                builder,
            } => Box::new(FixedExtentRenderSliver::new(
                item_count,
                item_extent,
                builder,
            )),
            ListViewStrategy::VariableExtent {
                item_count,
                estimated_extent,
                index,
                builder,
            } => Box::new(VariableExtentRenderSliver::new(
                index.unwrap_or_else(|| MeasuredExtentIndex::new(item_count, estimated_extent)),
                builder,
            )),
        };

        if let Some(padding) = value.padding {
            sliver = Box::new(PaddingRenderSliver {
                inner: RefCell::new(sliver),
                padding,
            });
        }
        single_sliver_viewport(
            controller,
            value.scroll_direction,
            value.reverse,
            value.physics.unwrap_or_default(),
            value.cache_extent.unwrap_or(DEFAULT_SLIVER_CACHE_EXTENT),
            value.clip_behavior,
            sliver,
        )
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
#[derive(Clone, TypedBuilder)]
#[builder(builder_method(name = typed_builder))]
pub struct GridView {
    #[builder(default, setter(transform = |children: impl IntoIterator<Item = impl Into<Widget>>| {
        children.into_iter().map(Into::into).collect::<Vec<Widget>>()
    }))]
    children: Vec<Widget>,
    #[builder(default, setter(skip))]
    strategy: Option<GridViewStrategy>,
    #[builder(default = 1, setter(transform = |count: usize| count.max(1)))]
    cross_axis_count: usize,
    #[builder(default, setter(transform = |extent: f32| Some(extent.max(1.0))))]
    row_extent: Option<f32>,
    #[builder(default = Axis::Vertical)]
    scroll_direction: Axis,
    #[builder(default = false)]
    reverse: bool,
    #[builder(default, setter(strip_option))]
    controller: Option<ScrollController>,
    #[builder(default, setter(strip_option))]
    padding: Option<EdgeInsets>,
    #[builder(default, setter(strip_option))]
    physics: Option<ScrollPhysics>,
}

impl Default for GridView {
    fn default() -> Self {
        Self::typed_builder().build()
    }
}

impl GridView {
    /// Creates a fixed cross-axis count grid from static children.
    #[must_use]
    pub fn count(
        cross_axis_count: usize,
        children: impl IntoIterator<Item = impl Into<Widget>>,
    ) -> Self {
        Self {
            children: children.into_iter().map(Into::into).collect(),
            strategy: None,
            cross_axis_count: cross_axis_count.max(1),
            row_extent: None,
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
            children: Vec::new(),
            strategy: Some(GridViewStrategy::Builder {
                item_count,
                cross_axis_count: cross_axis_count.max(1),
                row_extent: row_extent.max(1.0),
                builder: Rc::new(move |i| builder(i).into()),
            }),
            cross_axis_count: 1,
            row_extent: None,
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
        let mut sliver: Box<dyn RenderSliver> = match value.strategy.unwrap_or_else(|| {
            GridViewStrategy::Count {
                cross_axis_count: value.cross_axis_count,
                children: value.children,
                row_extent: value.row_extent,
            }
        }) {
            GridViewStrategy::Count {
                cross_axis_count,
                children,
                row_extent,
            } => {
                let columns = cross_axis_count.max(1);
                let children = Rc::new(children);
                let extent = row_extent.unwrap_or(80.0);
                Box::new(FixedExtentRenderSliver::new(
                    children.len().div_ceil(columns),
                    extent,
                    Rc::new(move |row| {
                        let start = row * columns;
                        let end = (start + columns).min(children.len());
                        Widget::from(Row::new(children[start..end].iter().cloned()))
                    }),
                ))
            }
            GridViewStrategy::Builder {
                item_count,
                cross_axis_count,
                row_extent,
                builder,
            } => {
                let columns = cross_axis_count.max(1);
                let rows = item_count.div_ceil(columns);
                Box::new(FixedExtentRenderSliver::new(
                    rows,
                    row_extent,
                    Rc::new(move |row| {
                        let start = row * columns;
                        Widget::from(Row::new(
                            (start..(start + columns).min(item_count)).map(|index| builder(index)),
                        ))
                    }),
                ))
            }
        };

        if let Some(padding) = value.padding {
            sliver = Box::new(PaddingRenderSliver {
                inner: RefCell::new(sliver),
                padding,
            });
        }
        single_sliver_viewport(
            controller,
            value.scroll_direction,
            value.reverse,
            value.physics.unwrap_or_default(),
            DEFAULT_SLIVER_CACHE_EXTENT,
            Clip::HardEdge,
            sliver,
        )
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
#[derive(Clone, TypedBuilder)]
#[builder(builder_method(name = typed_builder))]
pub struct PageView {
    #[builder(default, setter(transform = |children: impl IntoIterator<Item = impl Into<Widget>>| {
        children.into_iter().map(Into::into).collect::<Vec<Widget>>()
    }))]
    children: Vec<Widget>,
    #[builder(default, setter(skip))]
    strategy: Option<PageViewStrategy>,
    #[builder(default, setter(strip_option))]
    controller: Option<PageController>,
    #[builder(default = Axis::Horizontal)]
    scroll_direction: Axis,
    #[builder(default = false)]
    reverse: bool,
    #[builder(default = true)]
    page_snapping: bool,
    #[builder(default = 1.0, setter(transform = |value: f32| value.max(0.01)))]
    viewport_fraction: f32,
    #[builder(default, setter(strip_option))]
    physics: Option<ScrollPhysics>,
}

impl Default for PageView {
    fn default() -> Self {
        Self::typed_builder().build()
    }
}

impl PageView {
    /// Creates a PageView from static children pages.
    #[must_use]
    pub fn new(children: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        Self {
            children: children.into_iter().map(Into::into).collect(),
            strategy: None,
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
            children: Vec::new(),
            strategy: Some(PageViewStrategy::Builder {
                page_count,
                page_extent: page_extent.max(1.0),
                builder: Rc::new(move |i| builder(i).into()),
            }),
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
        match value
            .strategy
            .unwrap_or_else(|| PageViewStrategy::Children(value.children))
        {
            PageViewStrategy::Children(children) => {
                let snap_extent = 600.0 * fraction;
                let sliver = SliverFillViewport::new(children)
                    .viewport_fraction(fraction)
                    .fallback_extent(600.0);
                let render =
                    sliver.create_render_sliver(&controller, value.scroll_direction, value.reverse);
                single_sliver_viewport(
                    controller,
                    value.scroll_direction,
                    value.reverse,
                    physics_for_extent(snap_extent),
                    DEFAULT_SLIVER_CACHE_EXTENT,
                    Clip::HardEdge,
                    render,
                )
            }
            PageViewStrategy::Builder {
                page_count,
                page_extent,
                builder,
            } => {
                let snap_extent = page_extent * fraction;
                let sliver = SliverFillViewport::builder(page_count, move |index| builder(index))
                    .viewport_fraction(fraction)
                    .fallback_extent(page_extent);
                let render =
                    sliver.create_render_sliver(&controller, value.scroll_direction, value.reverse);
                single_sliver_viewport(
                    controller,
                    value.scroll_direction,
                    value.reverse,
                    physics_for_extent(snap_extent),
                    DEFAULT_SLIVER_CACHE_EXTENT,
                    Clip::HardEdge,
                    render,
                )
            }
        }
    }
}

/// Stable identity for a child materialized by a sliver.
///
/// The low 32 bits are owned by the leaf sliver. The high 32 bits contain a
/// compact stack of eight-bit group scopes, with the innermost scope in the
/// least-significant byte. Keeping the scope path in the ID means nested main
/// and cross-axis groups cannot accidentally reuse each other's child state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SliverChildId(pub u64);

impl SliverChildId {
    const fn list_item(index: usize) -> Self {
        Self(index as u64 + 1)
    }

    pub(crate) fn item_index(self) -> Option<usize> {
        self.0
            .checked_sub(1)
            .and_then(|index| usize::try_from(index & u32::MAX as u64).ok())
    }

    fn scoped(sliver: usize, child: Self) -> Self {
        let path = child.0 >> 32;
        let scope = (sliver as u64).saturating_add(1).min(u8::MAX as u64);
        let path = (((path & 0x00ff_ffff) << 8) | scope) & u32::MAX as u64;
        Self((path << 32) | (child.0 & u32::MAX as u64))
    }

    fn scope(self) -> usize {
        let scope = (self.0 >> 32) & u8::MAX as u64;
        scope
            .checked_sub(1)
            .map_or(usize::MAX, |value| value as usize)
    }

    fn local(self) -> Self {
        Self((((self.0 >> 32) >> 8) << 32) | (self.0 & u32::MAX as u64))
    }
}

/// One child placement returned by a render sliver.
#[derive(Clone, Debug)]
pub struct SliverChildLayout {
    pub id: SliverChildId,
    pub widget: Widget,
    /// Main-axis content offset before the viewport scroll transform.
    pub offset: f32,
    /// Cross-axis content offset.
    pub cross_offset: f32,
    /// Box constraints used when the retained child is laid out.
    pub constraints: Constraints,
    /// Measured/estimated main-axis extent used for anchor and pinning math.
    pub extent: f32,
    /// Pinned children are painted above normal flowing children.
    pub pinned: bool,
}

/// Result of laying out one render sliver.
#[derive(Clone, Debug)]
pub struct SliverLayout {
    pub geometry: SliverGeometry,
    pub children: Vec<SliverChildLayout>,
    /// Overlap absorbed for a following nested viewport.
    pub absorbed_overlap: f32,
}

impl SliverLayout {
    fn empty(geometry: SliverGeometry) -> Self {
        Self {
            geometry,
            children: Vec::new(),
            absorbed_overlap: 0.,
        }
    }
}

/// Retained sliver protocol. Implementations receive viewport constraints and
/// return geometry plus only the children needed for the current paint/cache
/// interval. This is the analogue of Flutter's `RenderSliver` contract.
pub trait RenderSliver {
    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout;

    /// Returns the logical child count when this sliver is backed by an
    /// indexed child delegate. Non-indexed slivers leave it unknown.
    fn child_count(&self) -> Option<usize> {
        None
    }

    /// Records the exact extent measured by the retained child tree.
    fn set_child_extent(&mut self, _child: SliverChildId, _extent: f32) -> bool {
        false
    }

    /// Structural/measurement revision used to invalidate a viewport's
    /// materialized child range without rebuilding the application.
    fn revision(&self) -> u64 {
        0
    }

    /// Advances retained sliver-local animation state without rebuilding the
    /// application widget description. The viewport calls this from the
    /// compositor phase, and schedules layout only when geometry changed.
    fn tick(&mut self, _now: Instant) -> bool {
        false
    }

    /// Whether another frame is required for sliver-local animation.
    fn is_animating(&self) -> bool {
        false
    }
}

/// Private bridge consumed by the retained widget tree. The public sliver
/// protocol remains renderer-neutral; this bridge adds widget materialization
/// and stable viewport-scoped child IDs.
pub(crate) trait SliverViewportDelegate {
    fn perform_layout(&self, constraints: SliverConstraints) -> SliverViewportLayout;
    fn set_child_extent(&self, child: SliverChildId, extent: f32) -> bool;
    fn revision(&self) -> u64;
    fn sliver_count(&self) -> usize;
    fn child_count(&self) -> Option<usize>;
    fn tick(&self, now: Instant) -> bool;
    fn is_animating(&self) -> bool;
}

pub struct SliverViewportConfig {
    pub(crate) controller: ScrollController,
    pub(crate) axis: Axis,
    pub(crate) reverse: bool,
    pub(crate) physics: ScrollPhysics,
    pub(crate) cache_extent: f32,
    pub(crate) shrink_wrap: bool,
    pub(crate) clip_behavior: Clip,
    pub(crate) delegate: Rc<dyn SliverViewportDelegate>,
}

impl std::fmt::Debug for SliverViewportConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SliverViewportConfig")
            .field("axis", &self.axis)
            .field("reverse", &self.reverse)
            .field("physics", &self.physics)
            .field("cache_extent", &self.cache_extent)
            .field("shrink_wrap", &self.shrink_wrap)
            .field("clip_behavior", &self.clip_behavior)
            .field("sliver_count", &self.delegate.sliver_count())
            .finish()
    }
}

impl PartialEq for SliverViewportConfig {
    fn eq(&self, other: &Self) -> bool {
        self.controller == other.controller
            && self.axis == other.axis
            && self.reverse == other.reverse
            && self.physics == other.physics
            && self.cache_extent == other.cache_extent
            && self.shrink_wrap == other.shrink_wrap
            && self.clip_behavior == other.clip_behavior
            && Rc::ptr_eq(&self.delegate, &other.delegate)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SliverViewportLayout {
    pub geometry: SliverGeometry,
    pub children: Vec<SliverChildLayout>,
}

struct BoxRenderSliver {
    child: Widget,
    extent: Cell<f32>,
    pinned: bool,
}

/// Geometry shared by the pinned persistent-header variants.
///
/// A pinned header has a normal scroll extent, but its paint extent continues
/// at the leading edge after its layout extent has collapsed.  Keeping that
/// distinction in the sliver geometry is what lets the viewport compute the
/// same obstruction/overlap values as Flutter's persistent-header render
/// objects instead of relying only on a post-layout position adjustment.
fn pinned_geometry(
    constraints: SliverConstraints,
    scroll_extent: f32,
    child_extent: f32,
) -> SliverGeometry {
    let scroll_extent = scroll_extent.max(0.);
    let child_extent = child_extent.max(0.);
    let effective_remaining_paint_extent =
        (constraints.remaining_paint_extent - constraints.overlap).max(0.);
    let paint_extent = child_extent.min(effective_remaining_paint_extent);
    let layout_extent =
        (scroll_extent - constraints.scroll_offset).clamp(0., effective_remaining_paint_extent);
    let cache_extent = if layout_extent > 0. {
        (-constraints.cache_origin + layout_extent).max(0.)
    } else {
        layout_extent
    };
    SliverGeometry {
        scroll_extent,
        paint_extent,
        layout_extent,
        max_paint_extent: scroll_extent,
        hit_test_extent: paint_extent,
        paint_origin: constraints.overlap,
        cache_extent,
        visible: paint_extent > 0.,
        has_visual_overflow: true,
        scroll_offset_correction: None,
    }
}

impl BoxRenderSliver {
    fn new(child: Widget) -> Self {
        Self {
            child,
            extent: Cell::new(48.),
            pinned: false,
        }
    }

    fn pinned(child: Widget) -> Self {
        Self {
            pinned: true,
            ..Self::new(child)
        }
    }
}

impl RenderSliver for BoxRenderSliver {
    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        let extent = self.extent.get().max(0.);
        let geometry = if self.pinned {
            pinned_geometry(constraints, extent, extent)
        } else {
            SliverGeometry::from_scroll_extent(constraints, extent)
        };
        SliverLayout {
            geometry,
            children: vec![SliverChildLayout {
                id: SliverChildId(0),
                widget: self.child.clone(),
                offset: 0.,
                cross_offset: 0.,
                constraints: sliver_child_constraints(
                    constraints.axis,
                    constraints.cross_axis_extent,
                    None,
                ),
                extent,
                pinned: self.pinned,
            }],
            absorbed_overlap: (geometry.paint_extent - geometry.layout_extent).max(0.),
        }
    }

    fn set_child_extent(&mut self, child: SliverChildId, extent: f32) -> bool {
        if child.0 != 0 || !extent.is_finite() || extent < 0. {
            return false;
        }
        let extent = extent.max(0.);
        if (self.extent.get() - extent).abs() <= f32::EPSILON {
            return false;
        }
        self.extent.set(extent);
        true
    }
}

struct FixedExtentRenderSliver {
    item_count: usize,
    item_extent: f32,
    builder: Rc<dyn Fn(usize) -> Widget>,
    widgets: HashMap<usize, Widget>,
}

impl FixedExtentRenderSliver {
    fn new(item_count: usize, item_extent: f32, builder: Rc<dyn Fn(usize) -> Widget>) -> Self {
        Self {
            item_count,
            item_extent: item_extent.max(1.),
            builder,
            widgets: HashMap::new(),
        }
    }

    fn child_widget(&mut self, index: usize) -> Widget {
        if let Some(widget) = self.widgets.get(&index) {
            return widget.clone();
        }
        let widget = (self.builder)(index);
        self.widgets.insert(index, widget.clone());
        widget
    }
}

impl RenderSliver for FixedExtentRenderSliver {
    fn child_count(&self) -> Option<usize> {
        Some(self.item_count)
    }

    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        let total = self.item_count as f32 * self.item_extent;
        let cache_start = (constraints.scroll_offset + constraints.cache_origin).max(0.);
        let cache_end = (cache_start + constraints.remaining_cache_extent).max(cache_start);
        let start = (cache_start / self.item_extent).floor() as usize;
        let end = (cache_end / self.item_extent).ceil() as usize;
        let range =
            start.min(self.item_count)..end.min(self.item_count).max(start.min(self.item_count));
        self.widgets.retain(|index, _| range.contains(index));
        let children = range
            .map(|index| {
                let widget = self.child_widget(index);
                SliverChildLayout {
                    id: SliverChildId::list_item(index),
                    widget,
                    offset: index as f32 * self.item_extent,
                    cross_offset: 0.,
                    constraints: sliver_child_constraints(
                        constraints.axis,
                        constraints.cross_axis_extent,
                        Some(self.item_extent),
                    ),
                    extent: self.item_extent,
                    pinned: false,
                }
            })
            .collect();
        SliverLayout {
            geometry: SliverGeometry::from_scroll_extent(constraints, total),
            children,
            absorbed_overlap: 0.,
        }
    }
}

struct VariableExtentRenderSliver {
    index: MeasuredExtentIndex,
    builder: Rc<dyn Fn(usize) -> Widget>,
    widgets: HashMap<usize, Widget>,
}

impl VariableExtentRenderSliver {
    fn new(index: MeasuredExtentIndex, builder: Rc<dyn Fn(usize) -> Widget>) -> Self {
        Self {
            index,
            builder,
            widgets: HashMap::new(),
        }
    }

    fn child_widget(&mut self, index: usize) -> Widget {
        if let Some(widget) = self.widgets.get(&index) {
            return widget.clone();
        }
        let widget = (self.builder)(index);
        self.widgets.insert(index, widget.clone());
        widget
    }
}

impl RenderSliver for VariableExtentRenderSliver {
    fn child_count(&self) -> Option<usize> {
        Some(self.index.len())
    }

    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        let cache_start = (constraints.scroll_offset + constraints.cache_origin).max(0.);
        let cache_end = (cache_start + constraints.remaining_cache_extent).max(cache_start);
        let range =
            self.index
                .materialized_range(cache_start, (cache_end - cache_start).max(0.), 0.);
        self.widgets.retain(|index, _| range.contains(index));
        let children = range
            .map(|index| {
                let widget = self.child_widget(index);
                SliverChildLayout {
                    id: SliverChildId::list_item(index),
                    widget,
                    offset: self.index.offset_for_index(index),
                    cross_offset: 0.,
                    constraints: sliver_child_constraints(
                        constraints.axis,
                        constraints.cross_axis_extent,
                        None,
                    ),
                    extent: self.index.offset_for_index(index + 1)
                        - self.index.offset_for_index(index),
                    pinned: false,
                }
            })
            .collect();
        SliverLayout {
            geometry: SliverGeometry::from_scroll_extent(constraints, self.index.total_extent()),
            children,
            absorbed_overlap: 0.,
        }
    }

    fn set_child_extent(&mut self, child: SliverChildId, extent: f32) -> bool {
        child
            .0
            .checked_sub(1)
            .and_then(|index| usize::try_from(index).ok())
            .is_some_and(|index| self.index.set_measured_extent(index, extent))
    }

    fn revision(&self) -> u64 {
        self.index.revision()
    }
}

struct HeaderRenderSliver {
    child: Widget,
    extent: f32,
    pinned: bool,
}

impl HeaderRenderSliver {
    fn new(child: Widget, extent: f32, pinned: bool) -> Self {
        Self {
            child,
            extent: extent.max(0.),
            pinned,
        }
    }
}

/// Retained floating-header state. A floating header follows the normal
/// scroll offset while moving forward, but reveals by the same delta when the
/// viewport starts moving back toward the leading edge.
struct FloatingHeaderRenderSliver {
    child: Widget,
    extent: Cell<f32>,
    last_scroll_offset: Option<f32>,
    effective_scroll_offset: f32,
}

impl RenderSliver for FloatingHeaderRenderSliver {
    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        let extent = self.extent.get().max(0.);
        let scroll_offset = constraints.scroll_offset.max(0.);
        if let Some(previous) = self.last_scroll_offset {
            self.effective_scroll_offset = (self.effective_scroll_offset
                + (scroll_offset - previous))
                .clamp(0., scroll_offset.max(extent));
        } else {
            self.effective_scroll_offset = scroll_offset;
        }
        self.last_scroll_offset = Some(scroll_offset);

        let effective_remaining_paint_extent =
            (constraints.remaining_paint_extent - constraints.overlap).max(0.);
        let paint_extent = (extent - self.effective_scroll_offset)
            .max(0.)
            .min(effective_remaining_paint_extent);
        let layout_extent = (extent - scroll_offset)
            .clamp(0., effective_remaining_paint_extent)
            .min(paint_extent);
        let geometry = SliverGeometry {
            scroll_extent: extent,
            paint_extent,
            layout_extent,
            max_paint_extent: extent,
            hit_test_extent: paint_extent,
            paint_origin: constraints.overlap.min(0.),
            cache_extent: if layout_extent > 0. {
                (-constraints.cache_origin + layout_extent).max(0.)
            } else {
                0.
            },
            visible: paint_extent > 0.,
            has_visual_overflow: true,
            scroll_offset_correction: None,
        };
        SliverLayout {
            geometry,
            // The sequence converts this back through the viewport transform.
            // `extent - effective` is not a normal flow offset: it exposes
            // the child at the leading edge while the header is floating.
            children: vec![SliverChildLayout {
                id: SliverChildId(0),
                widget: self.child.clone(),
                offset: scroll_offset - self.effective_scroll_offset,
                cross_offset: 0.,
                constraints: sliver_child_constraints(
                    constraints.axis,
                    constraints.cross_axis_extent,
                    Some(extent),
                ),
                extent,
                pinned: false,
            }],
            absorbed_overlap: (paint_extent - layout_extent).max(0.),
        }
    }

    fn set_child_extent(&mut self, child: SliverChildId, extent: f32) -> bool {
        if child.0 != 0 || !extent.is_finite() || extent < 0. {
            return false;
        }
        let extent = extent.max(0.);
        if (self.extent.get() - extent).abs() <= f32::EPSILON {
            return false;
        }
        self.extent.set(extent);
        true
    }
}

impl RenderSliver for HeaderRenderSliver {
    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        let geometry = if self.pinned {
            pinned_geometry(constraints, self.extent, self.extent)
        } else {
            SliverGeometry::from_scroll_extent(constraints, self.extent)
        };
        SliverLayout {
            geometry,
            children: vec![SliverChildLayout {
                id: SliverChildId(0),
                widget: self.child.clone(),
                offset: 0.,
                cross_offset: 0.,
                constraints: sliver_child_constraints(
                    constraints.axis,
                    constraints.cross_axis_extent,
                    Some(self.extent),
                ),
                extent: self.extent,
                pinned: self.pinned,
            }],
            absorbed_overlap: (geometry.paint_extent - geometry.layout_extent).max(0.),
        }
    }
}

struct ResizingHeaderRenderSliver {
    child: Widget,
    min_extent: f32,
    max_extent: f32,
}

struct FillRemainingRenderSliver {
    child: Widget,
    has_scroll_body: bool,
    extent: Cell<f32>,
}

impl RenderSliver for FillRemainingRenderSliver {
    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        let child_hint = widget_main_extent_hint(&self.child, constraints.axis).unwrap_or(0.);
        let remaining_extent =
            (constraints.viewport_main_axis_extent - constraints.preceding_scroll_extent).max(0.);
        let extent = if self.has_scroll_body {
            (constraints.remaining_paint_extent - constraints.overlap.min(0.)).max(0.)
        } else {
            self.extent.get().max(remaining_extent).max(child_hint)
        };
        self.extent.set(extent);
        let scroll_extent = if self.has_scroll_body {
            constraints.viewport_main_axis_extent.max(0.)
        } else {
            extent
        };
        SliverLayout {
            geometry: SliverGeometry::from_scroll_extent(constraints, scroll_extent),
            children: vec![SliverChildLayout {
                id: SliverChildId(0),
                widget: self.child.clone(),
                // The parent sequence contributes the preceding scroll
                // extent; a sliver child is always positioned in this
                // sliver's local coordinate space.
                offset: 0.,
                cross_offset: 0.,
                constraints: sliver_child_constraints(
                    constraints.axis,
                    constraints.cross_axis_extent,
                    Some(extent),
                ),
                extent,
                pinned: false,
            }],
            absorbed_overlap: 0.,
        }
    }

    fn set_child_extent(&mut self, child: SliverChildId, extent: f32) -> bool {
        if child.0 != 0 || !extent.is_finite() || extent < 0. {
            return false;
        }
        let changed = (self.extent.get() - extent).abs() > f32::EPSILON;
        self.extent.set(extent);
        changed
    }
}

struct ViewportExtentRenderSliver {
    item_count: usize,
    children: Option<Rc<Vec<Widget>>>,
    builder: Option<Rc<dyn Fn(usize) -> Widget>>,
    widgets: HashMap<usize, Widget>,
    viewport_fraction: f32,
    fallback_extent: f32,
}

impl ViewportExtentRenderSliver {
    fn child_widget(&mut self, index: usize) -> Widget {
        if let Some(widget) = self.widgets.get(&index) {
            return widget.clone();
        }
        let widget = self
            .children
            .as_ref()
            .and_then(|children| children.get(index).cloned())
            .or_else(|| self.builder.as_ref().map(|builder| builder(index)))
            .unwrap_or_else(|| Widget::from(SizedBox::shrink()));
        self.widgets.insert(index, widget.clone());
        widget
    }
}

impl RenderSliver for ViewportExtentRenderSliver {
    fn child_count(&self) -> Option<usize> {
        Some(self.item_count)
    }

    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        let viewport_extent = if constraints.viewport_main_axis_extent.is_finite()
            && constraints.viewport_main_axis_extent > 0.
        {
            constraints.viewport_main_axis_extent
        } else {
            self.fallback_extent
        };
        let extent = (viewport_extent * self.viewport_fraction).max(0.);
        let total = self.item_count as f32 * extent;
        let cache_start = (constraints.scroll_offset + constraints.cache_origin).max(0.);
        let cache_end = (cache_start + constraints.remaining_cache_extent).max(cache_start);
        let start = if extent > 0. {
            (cache_start / extent).floor() as usize
        } else {
            0
        };
        let end = if extent > 0. {
            (cache_end / extent).ceil() as usize
        } else {
            0
        };
        let range =
            start.min(self.item_count)..end.min(self.item_count).max(start.min(self.item_count));
        self.widgets.retain(|index, _| range.contains(index));
        let children = range
            .map(|index| {
                let widget = self.child_widget(index);
                SliverChildLayout {
                    id: SliverChildId::list_item(index),
                    widget,
                    offset: index as f32 * extent,
                    cross_offset: 0.,
                    constraints: sliver_child_constraints(
                        constraints.axis,
                        constraints.cross_axis_extent,
                        Some(extent),
                    ),
                    extent,
                    pinned: false,
                }
            })
            .collect();
        SliverLayout {
            geometry: SliverGeometry::from_scroll_extent(constraints, total),
            children,
            absorbed_overlap: 0.,
        }
    }
}

impl RenderSliver for ResizingHeaderRenderSliver {
    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        let current =
            (self.max_extent - constraints.scroll_offset).clamp(self.min_extent, self.max_extent);
        let geometry = pinned_geometry(constraints, self.max_extent, current);
        SliverLayout {
            geometry,
            children: vec![SliverChildLayout {
                id: SliverChildId(0),
                widget: self.child.clone(),
                offset: 0.,
                cross_offset: 0.,
                constraints: sliver_child_constraints(
                    constraints.axis,
                    constraints.cross_axis_extent,
                    Some(current),
                ),
                extent: current,
                pinned: true,
            }],
            absorbed_overlap: (geometry.paint_extent - geometry.layout_extent).max(0.),
        }
    }
}

struct PaddingRenderSliver {
    inner: RefCell<Box<dyn RenderSliver>>,
    padding: EdgeInsets,
}

impl RenderSliver for PaddingRenderSliver {
    fn child_count(&self) -> Option<usize> {
        self.inner.borrow().child_count()
    }

    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        let before = main_before(constraints.axis, self.padding);
        let after = main_after(constraints.axis, self.padding);
        let total_padding = before + after;
        let paint_offset = |from: f32, to: f32| {
            let start = from.max(constraints.scroll_offset);
            let end = to.min(constraints.scroll_offset + constraints.remaining_paint_extent);
            (end - start).max(0.)
        };
        let cache_offset = |from: f32, to: f32| {
            let cache_start = (constraints.scroll_offset + constraints.cache_origin).max(0.);
            let start = from.max(cache_start);
            let end = to.min(cache_start + constraints.remaining_cache_extent);
            (end - start).max(0.)
        };
        let reduced_cross = (constraints.cross_axis_extent
            - cross_before(constraints.axis, self.padding)
            - cross_after(constraints.axis, self.padding))
        .max(0.);
        let before_paint_extent = paint_offset(0., before);
        let before_cache_extent = cache_offset(0., before);
        let inner_scroll = (constraints.scroll_offset - before).max(0.);
        let inner_constraints = SliverConstraints::new(
            constraints.axis,
            constraints.reverse,
            inner_scroll,
            constraints.preceding_scroll_extent + before,
            if constraints.overlap > 0. {
                (constraints.overlap - before_paint_extent).max(0.)
            } else {
                constraints.overlap
            },
            (constraints.remaining_paint_extent - before_paint_extent).max(0.),
            reduced_cross,
            constraints.viewport_main_axis_extent,
            (constraints.remaining_cache_extent - before_cache_extent).max(0.),
            (constraints.cache_origin + before).min(0.),
        );
        let mut layout = self.inner.borrow_mut().perform_layout(inner_constraints);
        let inner_geometry = layout.geometry.normalized();
        if let Some(correction) = inner_geometry.scroll_offset_correction {
            let mut geometry = SliverGeometry::ZERO;
            geometry.scroll_offset_correction = Some(correction);
            return SliverLayout::empty(geometry);
        }
        for child in &mut layout.children {
            child.offset += before;
            child.cross_offset += cross_before(constraints.axis, self.padding);
        }
        let scroll_extent = total_padding + inner_geometry.scroll_extent;
        let after_paint_extent = paint_offset(
            before + inner_geometry.scroll_extent,
            total_padding + inner_geometry.scroll_extent,
        );
        let after_cache_extent = cache_offset(
            before + inner_geometry.scroll_extent,
            total_padding + inner_geometry.scroll_extent,
        );
        let paint_extent = (before_paint_extent
            + inner_geometry
                .paint_extent
                .max(inner_geometry.layout_extent + after_paint_extent))
        .min(constraints.remaining_paint_extent)
        .max(0.);
        let layout_extent =
            (before_paint_extent + after_paint_extent + inner_geometry.layout_extent)
                .min(paint_extent)
                .max(0.);
        layout.geometry = SliverGeometry {
            paint_origin: inner_geometry.paint_origin,
            scroll_extent,
            paint_extent,
            layout_extent,
            cache_extent: (before_cache_extent + after_cache_extent + inner_geometry.cache_extent)
                .min(constraints.remaining_cache_extent)
                .max(0.),
            max_paint_extent: total_padding + inner_geometry.max_paint_extent,
            hit_test_extent: (before_paint_extent
                + after_paint_extent
                + inner_geometry.paint_extent)
                .max(before_paint_extent + inner_geometry.hit_test_extent),
            visible: paint_extent > 0.,
            has_visual_overflow: inner_geometry.has_visual_overflow,
            scroll_offset_correction: None,
        };
        layout
    }

    fn set_child_extent(&mut self, child: SliverChildId, extent: f32) -> bool {
        self.inner.borrow_mut().set_child_extent(child, extent)
    }

    fn revision(&self) -> u64 {
        self.inner.borrow().revision()
    }

    fn tick(&mut self, now: Instant) -> bool {
        self.inner.borrow_mut().tick(now)
    }

    fn is_animating(&self) -> bool {
        self.inner.borrow().is_animating()
    }
}

struct WidgetWrapRenderSliver {
    inner: RefCell<Box<dyn RenderSliver>>,
    wrap: Rc<dyn Fn(Widget) -> Widget>,
}

struct LayoutBuilderRenderSliver {
    controller: ScrollController,
    builder: SliverLayoutBuilderFn,
    child: Widget,
    extent: Cell<f32>,
    last_constraints: Option<SliverConstraints>,
    revision: u64,
}

impl RenderSliver for LayoutBuilderRenderSliver {
    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        if self.last_constraints != Some(constraints) {
            self.child = (self.builder)(&self.controller, constraints);
            self.last_constraints = Some(constraints);
            self.revision = self.revision.wrapping_add(1);
        }
        let extent = self.extent.get().max(0.);
        SliverLayout {
            geometry: SliverGeometry::from_scroll_extent(constraints, extent),
            children: vec![SliverChildLayout {
                id: SliverChildId(0),
                widget: self.child.clone(),
                offset: 0.,
                cross_offset: 0.,
                constraints: sliver_child_constraints(
                    constraints.axis,
                    constraints.cross_axis_extent,
                    None,
                ),
                extent,
                pinned: false,
            }],
            absorbed_overlap: 0.,
        }
    }

    fn set_child_extent(&mut self, child: SliverChildId, extent: f32) -> bool {
        if child.0 != 0 || !extent.is_finite() || extent < 0. {
            return false;
        }
        let changed = (self.extent.get() - extent).abs() > f32::EPSILON;
        self.extent.set(extent.max(0.));
        changed
    }

    fn revision(&self) -> u64 {
        self.revision
    }
}

/// Shared overlap state between an outer absorber and an inner injector.
#[derive(Clone, Default)]
pub struct SliverOverlapHandle {
    extent: Rc<Cell<f32>>,
}

impl SliverOverlapHandle {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn extent(&self) -> f32 {
        self.extent.get()
    }

    fn set_extent(&self, extent: f32) {
        self.extent.set(extent.max(0.));
    }
}

struct OverlapAbsorberRenderSliver {
    inner: RefCell<Box<dyn RenderSliver>>,
    handle: SliverOverlapHandle,
}

impl RenderSliver for OverlapAbsorberRenderSliver {
    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        let layout = self.inner.borrow_mut().perform_layout(constraints);
        self.handle.set_extent(layout.absorbed_overlap);
        layout
    }

    fn set_child_extent(&mut self, child: SliverChildId, extent: f32) -> bool {
        self.inner.borrow_mut().set_child_extent(child, extent)
    }

    fn revision(&self) -> u64 {
        self.inner.borrow().revision()
    }

    fn tick(&mut self, now: Instant) -> bool {
        self.inner.borrow_mut().tick(now)
    }

    fn is_animating(&self) -> bool {
        self.inner.borrow().is_animating()
    }
}

struct OverlapInjectorRenderSliver {
    handle: SliverOverlapHandle,
}

impl RenderSliver for OverlapInjectorRenderSliver {
    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        let extent = self.handle.extent();
        SliverLayout::empty(SliverGeometry::from_scroll_extent(constraints, extent))
    }
}

impl RenderSliver for WidgetWrapRenderSliver {
    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        let mut layout = self.inner.borrow_mut().perform_layout(constraints);
        for child in &mut layout.children {
            child.widget = (self.wrap)(child.widget.clone());
        }
        layout
    }

    fn set_child_extent(&mut self, child: SliverChildId, extent: f32) -> bool {
        self.inner.borrow_mut().set_child_extent(child, extent)
    }

    fn revision(&self) -> u64 {
        self.inner.borrow().revision()
    }

    fn tick(&mut self, now: Instant) -> bool {
        self.inner.borrow_mut().tick(now)
    }

    fn is_animating(&self) -> bool {
        self.inner.borrow().is_animating()
    }
}

/// Sequential viewport implementation shared by `CustomScrollView` and
/// sliver groups. It computes the same per-sliver constraint fields used by a
/// real viewport and scopes child identity at each boundary.
struct SequenceRenderSliver {
    children: Vec<RefCell<Box<dyn RenderSliver>>>,
}

impl SequenceRenderSliver {
    fn new(children: Vec<Box<dyn RenderSliver>>) -> Self {
        Self {
            children: children.into_iter().map(RefCell::new).collect(),
        }
    }

    fn layout_sequence(&self, constraints: SliverConstraints) -> SliverLayout {
        // These are the same independent cursors used by Flutter's
        // RenderViewport.layoutChildSequence. `preceding_scroll_extent` is
        // the logical scroll range consumed by previous slivers, while
        // `layout_offset` is the paint/layout cursor. They intentionally
        // diverge when a sliver is pinned, overlaps content, or has already
        // moved past the trailing edge of the viewport.
        let initial_layout_offset = 0.;
        let mut layout_offset = initial_layout_offset;
        let mut remaining_cache_extent = constraints.remaining_cache_extent;
        let mut cache_origin = constraints.cache_origin;
        let mut scroll_offset = constraints.scroll_offset;
        let mut max_paint_offset = layout_offset + constraints.overlap;
        let mut preceding = 0.;
        let mut children = Vec::new();
        let mut total_correction = None;
        for (sliver_index, sliver) in self.children.iter().enumerate() {
            let sliver_scroll_offset = scroll_offset.max(0.);
            // A sliver must not be asked to cache content before its local
            // scroll offset. This is the same corrected cache-origin rule
            // used by Flutter's viewport and is important when a viewport is
            // partially scrolled into a preceding sliver.
            let corrected_cache_origin = cache_origin.max(-sliver_scroll_offset);
            let cache_extent_correction = cache_origin - corrected_cache_origin;
            let sliver_constraints = SliverConstraints::new(
                constraints.axis,
                constraints.reverse,
                sliver_scroll_offset,
                preceding,
                max_paint_offset - layout_offset,
                (constraints.remaining_paint_extent - layout_offset + initial_layout_offset)
                    .max(0.),
                constraints.cross_axis_extent,
                constraints.viewport_main_axis_extent,
                (remaining_cache_extent + cache_extent_correction).max(0.),
                corrected_cache_origin,
            );
            let layout = sliver.borrow_mut().perform_layout(sliver_constraints);
            let geometry = layout.geometry.normalized();
            if total_correction.is_none() {
                total_correction = geometry.scroll_offset_correction;
            }

            // Flutter restarts the sequence at the first correction. Keep the
            // already laid out prefix so the retained tree remains coherent;
            // the viewport will apply the correction and run this sequence
            // again before painting.
            if geometry.scroll_offset_correction.is_some() {
                break;
            }

            let effective_layout_offset = layout_offset + geometry.paint_origin;
            // Once a sliver is past the trailing edge its effective paint
            // offset is no longer meaningful. Its increasing scroll cursor is
            // still useful for retaining a stable content ordering, matching
            // RenderViewport's fallback placement for invisible slivers.
            let sliver_paint_offset = if geometry.visible || scroll_offset > 0. {
                effective_layout_offset
            } else {
                -scroll_offset + initial_layout_offset
            };
            for mut child in layout.children {
                child.id = SliverChildId::scoped(sliver_index, child.id);
                // The retained tree applies one viewport-level transform.
                // Convert Flutter's paint-space child position back into the
                // sequence's content space so that transform produces the
                // same result for normal, overlapping, and pinned slivers.
                child.offset = sliver_paint_offset + child.offset - sliver_scroll_offset
                    + constraints.scroll_offset;
                children.push(child);
            }

            max_paint_offset =
                max_paint_offset.max(effective_layout_offset + geometry.paint_extent);
            preceding += geometry.scroll_extent;
            scroll_offset -= geometry.scroll_extent;
            layout_offset += geometry.layout_extent;
            if geometry.cache_extent != 0. {
                remaining_cache_extent -= geometry.cache_extent - cache_extent_correction;
                cache_origin = (corrected_cache_origin + geometry.cache_extent).min(0.);
            }
            // Custom slivers may expose an overlap that is not represented by
            // their paint extent. Preserve it as an explicit obstruction;
            // built-in pinned headers report exactly paintExtent-layoutExtent
            // here, so this does not double-count them.
            max_paint_offset =
                max_paint_offset.max(layout_offset + layout.absorbed_overlap.max(0.));
        }
        apply_pinned_offsets_with_direction(
            &mut children,
            constraints.scroll_offset,
            constraints.viewport_main_axis_extent,
            constraints.reverse,
        );
        let mut geometry = SliverGeometry::from_scroll_extent(constraints, preceding);
        geometry.scroll_offset_correction = total_correction;
        SliverLayout {
            geometry,
            children,
            absorbed_overlap: (max_paint_offset - layout_offset).max(0.),
        }
    }
}

impl RenderSliver for SequenceRenderSliver {
    fn child_count(&self) -> Option<usize> {
        self.children.iter().try_fold(0usize, |count, child| {
            child
                .borrow()
                .child_count()
                .map(|child_count| count + child_count)
        })
    }

    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        self.layout_sequence(constraints)
    }

    fn set_child_extent(&mut self, child: SliverChildId, extent: f32) -> bool {
        let index = child.scope();
        self.children
            .get(index)
            .is_some_and(|sliver| sliver.borrow_mut().set_child_extent(child.local(), extent))
    }

    fn revision(&self) -> u64 {
        self.children
            .iter()
            .map(|child| child.borrow().revision())
            .fold(0, u64::wrapping_add)
    }

    fn tick(&mut self, now: Instant) -> bool {
        self.children
            .iter()
            .any(|child| child.borrow_mut().tick(now))
    }

    fn is_animating(&self) -> bool {
        self.children
            .iter()
            .any(|child| child.borrow().is_animating())
    }
}

struct SequenceViewportDelegate {
    sequence: RefCell<SequenceRenderSliver>,
}

impl SequenceViewportDelegate {
    fn new(slivers: Vec<Box<dyn RenderSliver>>) -> Self {
        Self {
            sequence: RefCell::new(SequenceRenderSliver::new(slivers)),
        }
    }
}

impl SliverViewportDelegate for SequenceViewportDelegate {
    fn perform_layout(&self, constraints: SliverConstraints) -> SliverViewportLayout {
        let layout = self.sequence.borrow_mut().perform_layout(constraints);
        SliverViewportLayout {
            geometry: layout.geometry,
            children: layout.children,
        }
    }

    fn set_child_extent(&self, child: SliverChildId, extent: f32) -> bool {
        self.sequence.borrow_mut().set_child_extent(child, extent)
    }

    fn revision(&self) -> u64 {
        self.sequence.borrow().revision()
    }

    fn sliver_count(&self) -> usize {
        self.sequence.borrow().children.len()
    }

    fn child_count(&self) -> Option<usize> {
        self.sequence.borrow().child_count()
    }

    fn tick(&self, now: Instant) -> bool {
        self.sequence.borrow_mut().tick(now)
    }

    fn is_animating(&self) -> bool {
        self.sequence.borrow().is_animating()
    }
}

/// Builds a viewport for one sliver. Box scrollables such as `ListView` and
/// `PageView` use this same retained sliver protocol as `CustomScrollView`
/// instead of maintaining a second lazy-list implementation.
fn single_sliver_viewport(
    controller: ScrollController,
    axis: Axis,
    reverse: bool,
    physics: ScrollPhysics,
    cache_extent: f32,
    clip_behavior: Clip,
    sliver: Box<dyn RenderSliver>,
) -> Widget {
    Widget::sliver_viewport_with_delegate_options(
        controller,
        axis,
        reverse,
        physics,
        cache_extent,
        false,
        clip_behavior,
        Rc::new(SequenceViewportDelegate::new(vec![sliver])),
    )
}

fn sliver_child_constraints(axis: Axis, cross: f32, extent: Option<f32>) -> Constraints {
    let cross = cross.max(0.);
    match (axis, extent) {
        (Axis::Vertical, Some(extent)) => Constraints::new(cross, cross, extent, extent),
        (Axis::Horizontal, Some(extent)) => Constraints::new(extent, extent, cross, cross),
        (Axis::Vertical, None) => Constraints::new(cross, cross, 0., f32::INFINITY),
        (Axis::Horizontal, None) => Constraints::new(0., f32::INFINITY, cross, cross),
    }
}

/// Returns a conservative main-axis size for a widget that is used as a
/// sliver prototype. This is deliberately limited to dimensions that are
/// independent of the eventual viewport; widgets whose size depends on
/// ambient constraints return `None` and are measured by the retained child
/// pass instead.
fn widget_main_extent_hint(widget: &Widget, axis: Axis) -> Option<f32> {
    fn finite(value: f32) -> Option<f32> {
        value.is_finite().then_some(value.max(0.))
    }

    fn dimension(size: Size, axis: Axis) -> Option<f32> {
        finite(axis.main_extent(size))
    }

    fn constrained(child: &Widget, axis: Axis, min: f32, max: f32) -> Option<f32> {
        let min = finite(min).unwrap_or(0.);
        let child = widget_main_extent_hint(child, axis);
        if max.is_finite() {
            let max = max.max(min);
            child
                .map(|value| value.clamp(min, max))
                .or_else(|| finite(max))
        } else {
            child.or_else(|| (min > 0.).then_some(min))
        }
    }

    let result = match &widget.kind {
        WidgetKind::Box { size, .. } => dimension(*size, axis),
        WidgetKind::Shape { size, path, .. } => {
            size.and_then(|size| dimension(size, axis)).or_else(|| {
                path.bounds()
                    .and_then(|bounds| dimension(bounds.size, axis))
            })
        }
        WidgetKind::CustomPaint { size, .. } => dimension(*size, axis),
        WidgetKind::Decorated { size, child, .. } => size
            .and_then(|size| dimension(size, axis))
            .or_else(|| widget_main_extent_hint(child, axis)),
        WidgetKind::Button { size, child, .. } => dimension(*size, axis)
            .filter(|extent| *extent > 0.)
            .or_else(|| {
                child
                    .as_deref()
                    .and_then(|child| widget_main_extent_hint(child, axis))
            })
            .or_else(|| dimension(*size, axis)),
        WidgetKind::Text { style, .. } | WidgetKind::SelectableText { style, .. } => {
            let font_size = style.size.max(1.);
            let line_height = style
                .line_height
                .map_or(font_size * 1.2, |height| match height {
                    incular_text::LineHeight::Normal => font_size * 1.2,
                    incular_text::LineHeight::Multiplier(multiplier) => {
                        font_size * multiplier.max(0.)
                    }
                    incular_text::LineHeight::Absolute(pixels) => pixels.max(0.),
                });
            finite(line_height)
        }
        WidgetKind::Image { width, height, .. } => finite(match axis {
            Axis::Horizontal => width.unwrap_or(0.),
            Axis::Vertical => height.unwrap_or(0.),
        })
        .filter(|extent| *extent > 0.),
        WidgetKind::TextField { size, .. } => dimension(*size, axis),
        WidgetKind::Padding { padding, child } => {
            widget_main_extent_hint(child, axis).map(|extent| {
                extent
                    + if axis.is_vertical() {
                        padding.top + padding.bottom
                    } else {
                        padding.left + padding.right
                    }
            })
        }
        WidgetKind::Constrained { constraints, child } => constrained(
            child,
            axis,
            if axis.is_vertical() {
                constraints.min_height
            } else {
                constraints.min_width
            },
            if axis.is_vertical() {
                constraints.max_height
            } else {
                constraints.max_width
            },
        ),
        WidgetKind::Limited {
            max_width,
            max_height,
            child,
        } => {
            let max = if axis.is_vertical() {
                *max_height
            } else {
                *max_width
            };
            widget_main_extent_hint(child, axis)
                .map(|extent| extent.min(max))
                .or_else(|| finite(max))
        }
        WidgetKind::Overflow {
            min_width,
            max_width,
            min_height,
            max_height,
            child,
        } => {
            let min = if axis.is_vertical() {
                min_height.unwrap_or(0.)
            } else {
                min_width.unwrap_or(0.)
            };
            let max = if axis.is_vertical() {
                max_height.unwrap_or(f32::INFINITY)
            } else {
                max_width.unwrap_or(f32::INFINITY)
            };
            constrained(child, axis, min, max)
        }
        WidgetKind::Positioned {
            width,
            height,
            child,
            ..
        } => {
            let explicit = if axis.is_vertical() { *height } else { *width };
            explicit
                .and_then(finite)
                .or_else(|| widget_main_extent_hint(child, axis))
        }
        WidgetKind::Visibility { visible, child } => {
            if *visible {
                widget_main_extent_hint(child, axis)
            } else {
                Some(0.)
            }
        }
        WidgetKind::Align { child, .. }
        | WidgetKind::SafeArea { child, .. }
        | WidgetKind::ClipRect { child, .. }
        | WidgetKind::ClipRRect { child, .. }
        | WidgetKind::ClipOval { child, .. }
        | WidgetKind::ClipPath { child, .. }
        | WidgetKind::Gesture { child, .. }
        | WidgetKind::Draggable { child, .. }
        | WidgetKind::DragTarget { child, .. }
        | WidgetKind::IgnorePointer { child, .. }
        | WidgetKind::AbsorbPointer { child, .. }
        | WidgetKind::Unconstrained { child, .. }
        | WidgetKind::RepaintBoundary { child }
        | WidgetKind::FittedBox { child, .. }
        | WidgetKind::Opacity { child, .. }
        | WidgetKind::Blur { child, .. }
        | WidgetKind::DropShadow { child, .. }
        | WidgetKind::ColorFiltered { child, .. }
        | WidgetKind::Blend { child, .. }
        | WidgetKind::Translate { child, .. }
        | WidgetKind::Transform { child, .. }
        | WidgetKind::Scale { child, .. }
        | WidgetKind::Rotation { child, .. } => widget_main_extent_hint(child, axis),
        WidgetKind::Baseline { child, .. } => widget_main_extent_hint(child, axis),
        WidgetKind::Flexible { child, .. } => widget_main_extent_hint(child, axis),
        WidgetKind::Flex {
            axis: flex_axis,
            children,
            spacing,
            ..
        } => {
            let hints = children
                .iter()
                .map(|child| widget_main_extent_hint(child, axis))
                .collect::<Option<Vec<_>>>()?;
            if *flex_axis == axis {
                let spacing = spacing.max(0.) * children.len().saturating_sub(1) as f32;
                finite(hints.into_iter().sum::<f32>() + spacing)
            } else {
                hints.into_iter().reduce(f32::max).or(Some(0.))
            }
        }
        WidgetKind::Stack { children, .. } | WidgetKind::IndexedStack { children, .. } => children
            .iter()
            .filter_map(|child| widget_main_extent_hint(child, axis))
            .reduce(f32::max),
        WidgetKind::SelectionArea { child, .. } => widget_main_extent_hint(child, axis),
        WidgetKind::PersistentHeader { child, .. } => widget_main_extent_hint(child, axis),
        WidgetKind::NotificationListener { child, .. } => widget_main_extent_hint(child, axis),
        // A scrollable or a layout builder obtains its main-axis extent from
        // its parent; guessing it from the child would make a prototype list
        // report a different extent from the actual viewport.
        WidgetKind::Scroll { .. }
        | WidgetKind::SliverViewport { .. }
        | WidgetKind::LayoutBuilder { .. }
        | WidgetKind::AspectRatio { .. }
        | WidgetKind::Fractional { .. }
        | WidgetKind::Wrap { .. }
        | WidgetKind::Table { .. } => None,
    };
    result.and_then(finite)
}

fn main_before(axis: Axis, padding: EdgeInsets) -> f32 {
    if axis.is_vertical() {
        padding.top
    } else {
        padding.left
    }
}

fn main_after(axis: Axis, padding: EdgeInsets) -> f32 {
    if axis.is_vertical() {
        padding.bottom
    } else {
        padding.right
    }
}

fn cross_before(axis: Axis, padding: EdgeInsets) -> f32 {
    if axis.is_vertical() {
        padding.left
    } else {
        padding.top
    }
}

fn cross_after(axis: Axis, padding: EdgeInsets) -> f32 {
    if axis.is_vertical() {
        padding.right
    } else {
        padding.bottom
    }
}

fn apply_pinned_offsets_with_direction(
    children: &mut [SliverChildLayout],
    scroll: f32,
    viewport: f32,
    reverse: bool,
) {
    let viewport = viewport.max(0.);
    let pinned = children
        .iter()
        .enumerate()
        .filter_map(|(index, child)| child.pinned.then_some(index))
        .collect::<Vec<_>>();
    if reverse {
        // In a reversed viewport the leading edge is the physical trailing
        // edge. Walk backwards so multiple pinned headers stack from right to
        // left (or bottom to top) in the same way Flutter's viewport does.
        let mut stack = viewport;
        for child_index in pinned.into_iter().rev() {
            let normal = children[child_index].offset;
            let extent = children[child_index].extent.max(0.).min(viewport);
            let current = normal - scroll;
            let target = stack - extent;
            if current <= target {
                children[child_index].offset = normal + target - current;
                stack = target;
            }
        }
    } else {
        // Pinned headers reserve a slot at the leading edge once their normal
        // position reaches that slot. Every subsequent pinned header uses the
        // end of the previous slot, preventing overlap while preserving the
        // normal flow position before it reaches the stack.
        let mut stack = 0.;
        for child_index in pinned {
            let normal = children[child_index].offset;
            let extent = children[child_index].extent.max(0.).min(viewport);
            let current = normal - scroll;
            if current <= stack {
                children[child_index].offset = normal + stack - current;
                stack += extent;
            }
        }
    }
}

/// Unified sliver protocol. A sliver creates a retained render-sliver node.
pub trait Sliver {
    fn build(&self, controller: &ScrollController) -> Widget;

    /// Cross-axis groups use this flex when the sliver is wrapped in
    /// [`SliverCrossAxisExpanded`]. Ordinary slivers occupy one equal lane.
    fn cross_axis_flex(&self) -> Option<usize> {
        None
    }

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

    /// Creates the retained sliver protocol implementation. The default is a
    /// true `SliverToBoxAdapter`, which preserves source compatibility for
    /// custom slivers while built-ins override it with lazy geometry-aware
    /// implementations.
    fn create_render_sliver(
        &self,
        controller: &ScrollController,
        axis: Axis,
        reverse: bool,
    ) -> Box<dyn RenderSliver> {
        Box::new(BoxRenderSliver::new(
            self.build_with_config(controller, axis, reverse),
        ))
    }
}

/// Adapts an ordinary box widget into a sliver.
#[derive(Clone, TypedBuilder)]
pub struct SliverToBoxAdapter {
    #[builder(setter(into))]
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

    fn create_render_sliver(
        &self,
        _controller: &ScrollController,
        _axis: Axis,
        _reverse: bool,
    ) -> Box<dyn RenderSliver> {
        Box::new(BoxRenderSliver::new(self.child.clone()))
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
        single_sliver_viewport(
            controller.clone(),
            Axis::Vertical,
            false,
            ScrollPhysics::default(),
            DEFAULT_SLIVER_CACHE_EXTENT,
            Clip::HardEdge,
            self.create_render_sliver(controller, Axis::Vertical, false),
        )
    }

    fn create_render_sliver(
        &self,
        _controller: &ScrollController,
        _axis: Axis,
        _reverse: bool,
    ) -> Box<dyn RenderSliver> {
        Box::new(VariableExtentRenderSliver::new(
            MeasuredExtentIndex::new(self.item_count, self.item_extent),
            self.builder.clone(),
        ))
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
        single_sliver_viewport(
            controller.clone(),
            Axis::Vertical,
            false,
            ScrollPhysics::default(),
            DEFAULT_SLIVER_CACHE_EXTENT,
            Clip::HardEdge,
            self.create_render_sliver(controller, Axis::Vertical, false),
        )
    }

    fn create_render_sliver(
        &self,
        _controller: &ScrollController,
        _axis: Axis,
        _reverse: bool,
    ) -> Box<dyn RenderSliver> {
        let columns = self.cross_axis_count;
        let item_count = self.item_count;
        let builder = self.builder.clone();
        Box::new(FixedExtentRenderSliver::new(
            item_count.div_ceil(columns),
            self.row_extent,
            Rc::new(move |row| {
                let start = row * columns;
                Widget::from(Row::new(
                    (start..(start + columns).min(item_count)).map(|index| builder(index)),
                ))
            }),
        ))
    }
}

/// Insets around a sliver child.
#[derive(TypedBuilder)]
pub struct SliverPadding {
    #[builder(setter(into))]
    padding: EdgeInsets,
    #[builder(setter(transform = |sliver: impl Sliver + 'static| {
        Box::new(sliver) as Box<dyn Sliver>
    }))]
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

    fn create_render_sliver(
        &self,
        controller: &ScrollController,
        axis: Axis,
        reverse: bool,
    ) -> Box<dyn RenderSliver> {
        Box::new(PaddingRenderSliver {
            inner: RefCell::new(self.sliver.create_render_sliver(controller, axis, reverse)),
            padding: self.padding,
        })
    }
}

/// A pinned header sliver.
#[derive(TypedBuilder)]
pub struct SliverPersistentHeader {
    #[builder(setter(transform = |height: f32| height.max(0.0)))]
    height: f32,
    #[builder(setter(into))]
    child: Widget,
    #[builder(default = true)]
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

    fn create_render_sliver(
        &self,
        _controller: &ScrollController,
        _axis: Axis,
        _reverse: bool,
    ) -> Box<dyn RenderSliver> {
        Box::new(HeaderRenderSliver::new(
            self.child.clone(),
            self.height,
            self.pinned,
        ))
    }
}

/// A framework-neutral app bar sliver.
#[derive(TypedBuilder)]
pub struct SliverAppBar {
    #[builder(default = 56.0, setter(transform = |height: f32| height.max(0.0)))]
    expanded_height: f32,
    #[builder(setter(into))]
    title: Widget,
    #[builder(default = true)]
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

    fn create_render_sliver(
        &self,
        _controller: &ScrollController,
        _axis: Axis,
        _reverse: bool,
    ) -> Box<dyn RenderSliver> {
        Box::new(HeaderRenderSliver::new(
            self.title.clone(),
            self.expanded_height,
            self.pinned,
        ))
    }
}

/// A first-class CustomScrollView coordinating a sequence of slivers.
#[derive(TypedBuilder)]
pub struct CustomScrollView {
    #[builder(
        default,
        setter(transform = |slivers: impl IntoIterator<Item = Box<dyn Sliver>>| {
            slivers.into_iter().collect::<Vec<Box<dyn Sliver>>>()
        })
    )]
    slivers: Vec<Box<dyn Sliver>>,
    #[builder(default, setter(strip_option))]
    controller: Option<ScrollController>,
    #[builder(default = Axis::Vertical)]
    scroll_direction: Axis,
    #[builder(default = false)]
    reverse: bool,
    #[builder(default, setter(strip_option))]
    physics: Option<ScrollPhysics>,
    #[builder(default = 250.0, setter(transform = |extent: f32| extent.max(0.0)))]
    cache_extent: f32,
    #[builder(default = Clip::HardEdge)]
    clip_behavior: Clip,
}

impl Default for CustomScrollView {
    fn default() -> Self {
        Self::builder().build()
    }
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
            cache_extent: 250.,
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

    /// Sets the amount of logical content laid out before and after the
    /// visible viewport. A nonzero cache keeps scrolling from synchronously
    /// constructing the next child at the edge.
    #[must_use]
    pub fn cache_extent(mut self, extent: f32) -> Self {
        self.cache_extent = extent.max(0.);
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
        let render_slivers = value
            .slivers
            .iter()
            .map(|sliver| {
                sliver.create_render_sliver(&controller, value.scroll_direction, value.reverse)
            })
            .collect();
        Widget::sliver_viewport_with_delegate_options(
            controller,
            value.scroll_direction,
            value.reverse,
            value.physics.unwrap_or_default(),
            value.cache_extent,
            false,
            value.clip_behavior,
            Rc::new(SequenceViewportDelegate::new(render_slivers)),
        )
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
#[derive(TypedBuilder)]
pub struct NestedScrollView {
    #[builder(default, setter(strip_option))]
    controller: Option<ScrollController>,
    #[builder(
        default,
        setter(transform = |slivers: impl IntoIterator<Item = Box<dyn Sliver>>| {
            slivers.into_iter().collect::<Vec<Box<dyn Sliver>>>()
        })
    )]
    header_slivers: Vec<Box<dyn Sliver>>,
    #[builder(setter(into))]
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
        let mut slivers = value.header_slivers;
        slivers.push(Box::new(SliverToBoxAdapter::new(value.body)) as Box<dyn Sliver>);
        CustomScrollView::new(slivers).controller(controller).into()
    }
}

/// A viewport bounding visible slivers.
#[derive(TypedBuilder)]
pub struct Viewport {
    #[builder(
        default,
        setter(transform = |slivers: impl IntoIterator<Item = Box<dyn Sliver>>| {
            slivers.into_iter().collect::<Vec<Box<dyn Sliver>>>()
        })
    )]
    slivers: Vec<Box<dyn Sliver>>,
    #[builder(default, setter(strip_option))]
    controller: Option<ScrollController>,
    #[builder(default = Axis::Vertical)]
    axis_direction: Axis,
}

impl Default for Viewport {
    fn default() -> Self {
        Self::builder().build()
    }
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
        CustomScrollView::new(value.slivers)
            .scroll_direction(value.axis_direction)
            .controller(value.controller.unwrap_or_default())
            .cache_extent(0.)
            .into()
    }
}

/// A shrink-wrapping viewport.
#[derive(TypedBuilder)]
pub struct ShrinkWrappingViewport {
    #[builder(
        default,
        setter(transform = |slivers: impl IntoIterator<Item = Box<dyn Sliver>>| {
            slivers.into_iter().collect::<Vec<Box<dyn Sliver>>>()
        })
    )]
    slivers: Vec<Box<dyn Sliver>>,
}

impl Default for ShrinkWrappingViewport {
    fn default() -> Self {
        Self::builder().build()
    }
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
        let controller = ScrollController::new();
        let render_slivers = value
            .slivers
            .iter()
            .map(|sliver| sliver.create_render_sliver(&controller, Axis::Vertical, false))
            .collect();
        Widget::sliver_viewport_with_delegate_options(
            controller,
            Axis::Vertical,
            false,
            ScrollPhysics::default(),
            250.,
            true,
            Clip::HardEdge,
            Rc::new(SequenceViewportDelegate::new(render_slivers)),
        )
    }
}

/// A configurable scrollbar widget.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct RawScrollbar {
    #[builder(default, setter(strip_option))]
    controller: Option<ScrollController>,
    #[builder(default = false)]
    thumb_visibility: bool,
    #[builder(setter(into))]
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
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct ListBody {
    #[builder(default = Axis::Vertical)]
    main_axis: Axis,
    #[builder(default, setter(transform = |children: impl IntoIterator<Item = impl Into<Widget>>| {
        children.into_iter().map(Into::into).collect::<Vec<Widget>>()
    }))]
    children: Vec<Widget>,
}

impl Default for ListBody {
    fn default() -> Self {
        Self::builder().build()
    }
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
#[derive(Clone, TypedBuilder)]
pub struct ListWheelScrollView {
    #[builder(setter(transform = |extent: f32| extent.max(1.0)))]
    item_extent: f32,
    #[builder(default, setter(transform = |children: impl IntoIterator<Item = impl Into<Widget>>| {
        children.into_iter().map(Into::into).collect::<Vec<Widget>>()
    }))]
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
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct DraggableScrollableSheet {
    #[builder(default = 0.5, setter(transform = |size: f32| size.clamp(0.0, 1.0)))]
    initial_child_size: f32,
    #[builder(default = 0.25, setter(transform = |size: f32| size.clamp(0.0, 1.0)))]
    min_child_size: f32,
    #[builder(default = 1.0, setter(transform = |size: f32| size.clamp(0.0, 1.0)))]
    max_child_size: f32,
    #[builder(setter(into))]
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
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct DraggableScrollableActuator {
    #[builder(setter(into))]
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
#[derive(Clone, TypedBuilder)]
pub struct NotificationListener {
    #[builder(default)]
    callback: Option<Rc<dyn Fn(ScrollNotification) -> bool>>,
    #[builder(setter(into))]
    child: Widget,
}

impl NotificationListener {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            callback: None,
            child: child.into(),
        }
    }

    /// Receives scroll notifications from descendant viewports. Returning
    /// `true` stops the notification from reaching an outer listener.
    #[must_use]
    pub fn on_notification(
        mut self,
        callback: impl Fn(ScrollNotification) -> bool + 'static,
    ) -> Self {
        self.callback = Some(Rc::new(callback));
        self
    }
}

impl From<NotificationListener> for Widget {
    fn from(value: NotificationListener) -> Self {
        Widget::notification_listener(value.callback, value.child)
    }
}

/// Observes scroll notifications.
#[derive(Clone, TypedBuilder)]
pub struct ScrollNotificationObserver {
    #[builder(setter(into))]
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
#[derive(Clone, TypedBuilder)]
pub struct TwoDimensionalScrollable {
    #[builder(default, setter(strip_option))]
    horizontal_controller: Option<ScrollController>,
    #[builder(default, setter(strip_option))]
    vertical_controller: Option<ScrollController>,
    #[builder(setter(into))]
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
#[derive(Clone, TypedBuilder)]
pub struct TwoDimensionalScrollView {
    #[builder(setter(into))]
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
#[derive(Clone, TypedBuilder)]
pub struct TwoDimensionalViewport {
    #[builder(setter(into))]
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
        single_sliver_viewport(
            controller.clone(),
            Axis::Vertical,
            false,
            ScrollPhysics::default(),
            DEFAULT_SLIVER_CACHE_EXTENT,
            Clip::HardEdge,
            self.create_render_sliver(controller, Axis::Vertical, false),
        )
    }

    fn create_render_sliver(
        &self,
        _controller: &ScrollController,
        _axis: Axis,
        _reverse: bool,
    ) -> Box<dyn RenderSliver> {
        Box::new(FixedExtentRenderSliver::new(
            self.item_count,
            self.item_extent,
            self.builder.clone(),
        ))
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
        single_sliver_viewport(
            controller.clone(),
            Axis::Vertical,
            false,
            ScrollPhysics::default(),
            DEFAULT_SLIVER_CACHE_EXTENT,
            Clip::HardEdge,
            self.create_render_sliver(controller, Axis::Vertical, false),
        )
    }

    fn create_render_sliver(
        &self,
        _controller: &ScrollController,
        _axis: Axis,
        _reverse: bool,
    ) -> Box<dyn RenderSliver> {
        let index = MeasuredExtentIndex::with_estimates(self.item_count, 48., |index| {
            (self.extent_builder)(index)
        });
        Box::new(VariableExtentRenderSliver::new(
            index,
            self.item_builder.clone(),
        ))
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
        single_sliver_viewport(
            controller.clone(),
            Axis::Vertical,
            false,
            ScrollPhysics::default(),
            DEFAULT_SLIVER_CACHE_EXTENT,
            Clip::HardEdge,
            self.create_render_sliver(controller, Axis::Vertical, false),
        )
    }

    fn create_render_sliver(
        &self,
        _controller: &ScrollController,
        axis: Axis,
        _reverse: bool,
    ) -> Box<dyn RenderSliver> {
        // Flutter's prototype sliver lays out one prototype and forces every
        // child to that main-axis extent. Use explicit widget dimensions when
        // available; constraint-dependent prototypes fall back to the normal
        // warm-up estimate and are corrected by the retained child pass.
        let prototype_extent = widget_main_extent_hint(&self.prototype_item, axis)
            .unwrap_or(48.)
            .max(1.);
        Box::new(FixedExtentRenderSliver::new(
            self.item_count,
            prototype_extent,
            self.builder.clone(),
        ))
    }
}

/// Sliver filling remaining viewport space.
#[derive(TypedBuilder)]
pub struct SliverFillRemaining {
    #[builder(setter(into))]
    child: Widget,
    #[builder(default = true)]
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

    fn create_render_sliver(
        &self,
        _controller: &ScrollController,
        _axis: Axis,
        _reverse: bool,
    ) -> Box<dyn RenderSliver> {
        Box::new(FillRemainingRenderSliver {
            child: self.child.clone(),
            has_scroll_body: self.has_scroll_body,
            extent: Cell::new(0.),
        })
    }
}

/// Sliver with children each filling the entire viewport.
#[derive(TypedBuilder)]
#[builder(builder_method(name = typed_builder))]
pub struct SliverFillViewport {
    #[builder(default, setter(transform = |children: impl IntoIterator<Item = impl Into<Widget>>| {
        children.into_iter().map(Into::into).collect::<Vec<Widget>>()
    }))]
    children: Vec<Widget>,
    #[builder(default, setter(skip))]
    builder: Option<Rc<dyn Fn(usize) -> Widget>>,
    #[builder(default, setter(skip))]
    item_count: Option<usize>,
    #[builder(default = 1.0, setter(transform = |fraction: f32| fraction.max(0.01)))]
    viewport_fraction: f32,
    #[builder(default = 600.0, setter(transform = |extent: f32| extent.max(1.0)))]
    fallback_extent: f32,
}

impl Default for SliverFillViewport {
    fn default() -> Self {
        Self::typed_builder().build()
    }
}

impl SliverFillViewport {
    #[must_use]
    pub fn new(children: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        Self {
            children: children.into_iter().map(Into::into).collect(),
            builder: None,
            item_count: None,
            viewport_fraction: 1.0,
            fallback_extent: 600.0,
        }
    }

    /// Creates a lazy viewport-filling sliver from an indexed child builder.
    #[must_use]
    pub fn builder<W>(item_count: usize, builder: impl Fn(usize) -> W + 'static) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self {
            children: Vec::new(),
            builder: Some(Rc::new(move |index| builder(index).into())),
            item_count: Some(item_count),
            viewport_fraction: 1.0,
            fallback_extent: 600.0,
        }
    }

    #[must_use]
    pub fn viewport_fraction(mut self, fraction: f32) -> Self {
        self.viewport_fraction = fraction.max(0.01);
        self
    }

    /// Sets the extent used when the sliver receives unbounded viewport
    /// constraints. Bounded viewports always derive the page extent from the
    /// viewport itself, matching Flutter's `SliverFillViewport`.
    #[must_use]
    pub fn fallback_extent(mut self, extent: f32) -> Self {
        self.fallback_extent = extent.max(1.0);
        self
    }

    #[must_use]
    pub fn viewport_fraction_value(&self) -> f32 {
        self.viewport_fraction
    }
}

impl Sliver for SliverFillViewport {
    fn build(&self, controller: &ScrollController) -> Widget {
        let render = self.create_render_sliver(controller, Axis::Vertical, false);
        single_sliver_viewport(
            controller.clone(),
            Axis::Vertical,
            false,
            ScrollPhysics::default(),
            DEFAULT_SLIVER_CACHE_EXTENT,
            Clip::HardEdge,
            render,
        )
    }

    fn create_render_sliver(
        &self,
        _controller: &ScrollController,
        _axis: Axis,
        _reverse: bool,
    ) -> Box<dyn RenderSliver> {
        let item_count = self.item_count.unwrap_or(self.children.len());
        Box::new(ViewportExtentRenderSliver {
            item_count,
            children: (!self.children.is_empty()).then(|| Rc::new(self.children.clone())),
            builder: self.builder.clone(),
            widgets: HashMap::new(),
            viewport_fraction: self.viewport_fraction,
            fallback_extent: self.fallback_extent,
        })
    }
}

/// Sliver builder receiving the owning scroll controller.
pub struct SliverLayoutBuilder {
    builder: SliverLayoutBuilderFn,
}

impl SliverLayoutBuilder {
    #[must_use]
    pub fn new<W>(builder: impl Fn(&ScrollController) -> W + 'static) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self {
            builder: Rc::new(move |c, _| builder(c).into()),
        }
    }

    /// Creates a layout builder that observes the complete sliver protocol
    /// constraints. It is rebuilt only when those constraints change, just as
    /// Flutter's `SliverLayoutBuilder` is driven by sliver layout rather than
    /// by ordinary box constraints.
    #[must_use]
    pub fn new_with_sliver_constraints<W>(
        builder: impl Fn(SliverConstraints) -> W + 'static,
    ) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self {
            builder: Rc::new(move |_, constraints| builder(constraints).into()),
        }
    }
}

impl Sliver for SliverLayoutBuilder {
    fn build(&self, controller: &ScrollController) -> Widget {
        (self.builder)(
            controller,
            SliverConstraints::new(Axis::Vertical, false, 0., 0., 0., 0., 0., 0., 0., 0.),
        )
    }

    fn create_render_sliver(
        &self,
        controller: &ScrollController,
        _axis: Axis,
        _reverse: bool,
    ) -> Box<dyn RenderSliver> {
        Box::new(LayoutBuilderRenderSliver {
            controller: controller.clone(),
            builder: self.builder.clone(),
            child: SizedBox::shrink().into(),
            extent: Cell::new(48.),
            last_constraints: None,
            revision: 0,
        })
    }
}

/// Groups multiple slivers along the main axis.
#[derive(TypedBuilder)]
pub struct SliverMainAxisGroup {
    #[builder(
        default,
        setter(transform = |slivers: impl IntoIterator<Item = Box<dyn Sliver>>| {
            slivers.into_iter().collect::<Vec<Box<dyn Sliver>>>()
        })
    )]
    slivers: Vec<Box<dyn Sliver>>,
}

impl Default for SliverMainAxisGroup {
    fn default() -> Self {
        Self::builder().build()
    }
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

    fn create_render_sliver(
        &self,
        controller: &ScrollController,
        axis: Axis,
        reverse: bool,
    ) -> Box<dyn RenderSliver> {
        Box::new(SequenceRenderSliver::new(
            self.slivers
                .iter()
                .map(|sliver| sliver.create_render_sliver(controller, axis, reverse))
                .collect(),
        ))
    }
}

/// Groups multiple slivers across the cross axis.
#[derive(TypedBuilder)]
pub struct SliverCrossAxisGroup {
    #[builder(
        default,
        setter(transform = |slivers: impl IntoIterator<Item = Box<dyn Sliver>>| {
            slivers.into_iter().collect::<Vec<Box<dyn Sliver>>>()
        })
    )]
    slivers: Vec<Box<dyn Sliver>>,
}

impl Default for SliverCrossAxisGroup {
    fn default() -> Self {
        Self::builder().build()
    }
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

    fn create_render_sliver(
        &self,
        controller: &ScrollController,
        axis: Axis,
        reverse: bool,
    ) -> Box<dyn RenderSliver> {
        Box::new(CrossAxisGroupRenderSliver::new(
            self.slivers
                .iter()
                .map(|sliver| {
                    (
                        sliver.create_render_sliver(controller, axis, reverse),
                        sliver.cross_axis_flex().unwrap_or(1),
                    )
                })
                .collect(),
        ))
    }
}

/// Lays out several slivers against the same main-axis scroll position while
/// dividing the cross axis into flex lanes. Each lane has independent sliver
/// geometry, but the group consumes the maximum lane scroll extent.
struct CrossAxisGroupRenderSliver {
    children: Vec<(RefCell<Box<dyn RenderSliver>>, usize)>,
}

impl CrossAxisGroupRenderSliver {
    fn new(children: Vec<(Box<dyn RenderSliver>, usize)>) -> Self {
        Self {
            children: children
                .into_iter()
                .map(|(child, flex)| (RefCell::new(child), flex.max(1)))
                .collect(),
        }
    }
}

impl RenderSliver for CrossAxisGroupRenderSliver {
    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        let total_flex = self
            .children
            .iter()
            .map(|(_, flex)| *flex as f32)
            .sum::<f32>()
            .max(1.);
        let mut cross_offset = 0.;
        let mut scroll_extent: f32 = 0.;
        let mut absorbed_overlap: f32 = 0.;
        let mut children = Vec::new();
        for (lane, (sliver, flex)) in self.children.iter().enumerate() {
            let lane_extent = constraints.cross_axis_extent * (*flex as f32 / total_flex);
            let lane_constraints = SliverConstraints::new(
                constraints.axis,
                constraints.reverse,
                constraints.scroll_offset,
                constraints.preceding_scroll_extent,
                constraints.overlap,
                constraints.remaining_paint_extent,
                lane_extent,
                constraints.viewport_main_axis_extent,
                constraints.remaining_cache_extent,
                constraints.cache_origin,
            );
            let layout = sliver.borrow_mut().perform_layout(lane_constraints);
            let geometry = layout.geometry.normalized();
            scroll_extent = scroll_extent.max(geometry.scroll_extent);
            absorbed_overlap = absorbed_overlap.max(layout.absorbed_overlap);
            for mut child in layout.children {
                child.id = SliverChildId::scoped(lane, child.id);
                child.cross_offset += cross_offset;
                children.push(child);
            }
            cross_offset += lane_extent;
        }
        SliverLayout {
            geometry: SliverGeometry::from_scroll_extent(constraints, scroll_extent),
            children,
            absorbed_overlap,
        }
    }

    fn set_child_extent(&mut self, child: SliverChildId, extent: f32) -> bool {
        let lane = child.scope();
        self.children
            .get(lane)
            .is_some_and(|(sliver, _)| sliver.borrow_mut().set_child_extent(child.local(), extent))
    }

    fn revision(&self) -> u64 {
        self.children
            .iter()
            .map(|(sliver, _)| sliver.borrow().revision())
            .fold(0, u64::wrapping_add)
    }

    fn tick(&mut self, now: Instant) -> bool {
        self.children
            .iter()
            .any(|(sliver, _)| sliver.borrow_mut().tick(now))
    }

    fn is_animating(&self) -> bool {
        self.children
            .iter()
            .any(|(sliver, _)| sliver.borrow().is_animating())
    }
}

/// Expands a sliver across cross-axis group space.
#[derive(TypedBuilder)]
pub struct SliverCrossAxisExpanded {
    #[builder(default = 1, setter(transform = |flex: usize| flex.max(1)))]
    flex: usize,
    #[builder(setter(transform = |sliver: impl Sliver + 'static| {
        Box::new(sliver) as Box<dyn Sliver>
    }))]
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

    fn create_render_sliver(
        &self,
        controller: &ScrollController,
        axis: Axis,
        reverse: bool,
    ) -> Box<dyn RenderSliver> {
        self.sliver.create_render_sliver(controller, axis, reverse)
    }

    fn cross_axis_flex(&self) -> Option<usize> {
        Some(self.flex)
    }
}

/// Constrains the cross-axis dimension of a sliver.
#[derive(TypedBuilder)]
pub struct SliverConstrainedCrossAxis {
    #[builder(setter(transform = |extent: f32| extent.max(0.0)))]
    max_extent: f32,
    #[builder(setter(transform = |sliver: impl Sliver + 'static| {
        Box::new(sliver) as Box<dyn Sliver>
    }))]
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

    fn create_render_sliver(
        &self,
        controller: &ScrollController,
        axis: Axis,
        reverse: bool,
    ) -> Box<dyn RenderSliver> {
        let max_extent = self.max_extent;
        let inner = self.sliver.create_render_sliver(controller, axis, reverse);
        Box::new(WidgetWrapRenderSliver {
            inner: RefCell::new(inner),
            wrap: Rc::new(move |widget| {
                crate::layout::ConstrainedBox::new(
                    if axis.is_vertical() {
                        Constraints::new(0., max_extent, 0., f32::INFINITY)
                    } else {
                        Constraints::new(0., f32::INFINITY, 0., max_extent)
                    },
                    widget,
                )
                .into()
            }),
        })
    }
}

/// Paints decoration behind a sliver.
#[derive(TypedBuilder)]
pub struct DecoratedSliver {
    decoration: incular_rendering::Decoration,
    #[builder(setter(transform = |sliver: impl Sliver + 'static| {
        Box::new(sliver) as Box<dyn Sliver>
    }))]
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

    fn create_render_sliver(
        &self,
        controller: &ScrollController,
        axis: Axis,
        reverse: bool,
    ) -> Box<dyn RenderSliver> {
        let decoration = self.decoration.clone();
        let inner = self.sliver.create_render_sliver(controller, axis, reverse);
        Box::new(WidgetWrapRenderSliver {
            inner: RefCell::new(inner),
            // `Decoration` is renderer-owned and the existing DecoratedBox
            // widget accepts BoxDecoration. Preserve the sliver geometry and
            // retain the child while that richer decoration bridge is added.
            wrap: Rc::new(move |widget| {
                let _ = &decoration;
                DecoratedBox::new(widget).into()
            }),
        })
    }
}

/// Sliver opacity wrapper.
#[derive(TypedBuilder)]
pub struct SliverOpacity {
    #[builder(setter(transform = |opacity: f32| opacity.clamp(0.0, 1.0)))]
    opacity: f32,
    #[builder(setter(transform = |sliver: impl Sliver + 'static| {
        Box::new(sliver) as Box<dyn Sliver>
    }))]
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

    fn create_render_sliver(
        &self,
        controller: &ScrollController,
        axis: Axis,
        reverse: bool,
    ) -> Box<dyn RenderSliver> {
        let opacity = self.opacity;
        let inner = self.sliver.create_render_sliver(controller, axis, reverse);
        Box::new(WidgetWrapRenderSliver {
            inner: RefCell::new(inner),
            wrap: Rc::new(move |widget| crate::Opacity::new(opacity, widget).into()),
        })
    }
}

/// Sliver offstage wrapper.
#[derive(TypedBuilder)]
pub struct SliverOffstage {
    offstage: bool,
    #[builder(setter(transform = |sliver: impl Sliver + 'static| {
        Box::new(sliver) as Box<dyn Sliver>
    }))]
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

    fn create_render_sliver(
        &self,
        controller: &ScrollController,
        axis: Axis,
        reverse: bool,
    ) -> Box<dyn RenderSliver> {
        let offstage = self.offstage;
        let inner = self.sliver.create_render_sliver(controller, axis, reverse);
        Box::new(WidgetWrapRenderSliver {
            inner: RefCell::new(inner),
            wrap: Rc::new(move |widget| {
                crate::layout::Offstage::new(widget)
                    .offstage(offstage)
                    .into()
            }),
        })
    }
}

/// Sliver ignore pointer wrapper.
#[derive(TypedBuilder)]
pub struct SliverIgnorePointer {
    ignoring: bool,
    #[builder(setter(transform = |sliver: impl Sliver + 'static| {
        Box::new(sliver) as Box<dyn Sliver>
    }))]
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

    fn create_render_sliver(
        &self,
        controller: &ScrollController,
        axis: Axis,
        reverse: bool,
    ) -> Box<dyn RenderSliver> {
        let ignoring = self.ignoring;
        let inner = self.sliver.create_render_sliver(controller, axis, reverse);
        Box::new(WidgetWrapRenderSliver {
            inner: RefCell::new(inner),
            wrap: Rc::new(move |widget| {
                crate::IgnorePointer::new(widget).ignoring(ignoring).into()
            }),
        })
    }
}

/// Sliver safe area insets wrapper.
#[derive(TypedBuilder)]
pub struct SliverSafeArea {
    #[builder(setter(transform = |sliver: impl Sliver + 'static| {
        Box::new(sliver) as Box<dyn Sliver>
    }))]
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

    fn create_render_sliver(
        &self,
        controller: &ScrollController,
        axis: Axis,
        reverse: bool,
    ) -> Box<dyn RenderSliver> {
        let inner = self.sliver.create_render_sliver(controller, axis, reverse);
        Box::new(WidgetWrapRenderSliver {
            inner: RefCell::new(inner),
            wrap: Rc::new(|widget| crate::SafeArea::new(widget).into()),
        })
    }
}

/// Sliver visibility wrapper.
#[derive(TypedBuilder)]
pub struct SliverVisibility {
    visible: bool,
    #[builder(setter(transform = |sliver: impl Sliver + 'static| {
        Box::new(sliver) as Box<dyn Sliver>
    }))]
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

    fn create_render_sliver(
        &self,
        controller: &ScrollController,
        axis: Axis,
        reverse: bool,
    ) -> Box<dyn RenderSliver> {
        let visible = self.visible;
        let inner = self.sliver.create_render_sliver(controller, axis, reverse);
        Box::new(WidgetWrapRenderSliver {
            inner: RefCell::new(inner),
            wrap: Rc::new(move |widget| {
                crate::layout::Visibility::new(widget)
                    .visible(visible)
                    .into()
            }),
        })
    }
}

/// Pinned leading header sliver.
#[derive(TypedBuilder)]
pub struct PinnedHeaderSliver {
    #[builder(setter(into))]
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

    fn create_render_sliver(
        &self,
        _controller: &ScrollController,
        _axis: Axis,
        _reverse: bool,
    ) -> Box<dyn RenderSliver> {
        Box::new(BoxRenderSliver::pinned(self.child.clone()))
    }
}

/// Floating header sliver.
#[derive(TypedBuilder)]
pub struct SliverFloatingHeader {
    #[builder(setter(into))]
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
        SliverPersistentHeader::new(
            widget_main_extent_hint(&self.child, axis)
                .unwrap_or(48.0)
                .max(1.0),
            self.child.clone(),
        )
        .pinned(false)
        .build_with_config(controller, axis, reverse)
    }

    fn create_render_sliver(
        &self,
        _controller: &ScrollController,
        axis: Axis,
        _reverse: bool,
    ) -> Box<dyn RenderSliver> {
        Box::new(FloatingHeaderRenderSliver {
            child: self.child.clone(),
            extent: Cell::new(
                widget_main_extent_hint(&self.child, axis)
                    .unwrap_or(48.)
                    .max(1.),
            ),
            last_scroll_offset: None,
            effective_scroll_offset: 0.,
        })
    }
}

/// Resizing header sliver.
#[derive(TypedBuilder)]
pub struct SliverResizingHeader {
    #[builder(setter(transform = |extent: f32| extent.max(0.0)))]
    min_extent: f32,
    #[builder(setter(transform = |extent: f32| extent.max(0.0)))]
    max_extent: f32,
    #[builder(setter(into))]
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

    fn create_render_sliver(
        &self,
        _controller: &ScrollController,
        _axis: Axis,
        _reverse: bool,
    ) -> Box<dyn RenderSliver> {
        Box::new(ResizingHeaderRenderSliver {
            child: self.child.clone(),
            min_extent: self.min_extent,
            max_extent: self.max_extent,
        })
    }
}

/// Sliver overlap absorber for nested scroll view coordinators.
#[derive(TypedBuilder)]
pub struct SliverOverlapAbsorber {
    #[builder(setter(transform = |sliver: impl Sliver + 'static| {
        Box::new(sliver) as Box<dyn Sliver>
    }))]
    sliver: Box<dyn Sliver>,
    #[builder(default)]
    handle: SliverOverlapHandle,
}

impl SliverOverlapAbsorber {
    #[must_use]
    pub fn new(sliver: impl Sliver + 'static) -> Self {
        Self {
            sliver: Box::new(sliver),
            handle: SliverOverlapHandle::new(),
        }
    }

    /// Returns the handle consumed by an inner `SliverOverlapInjector`.
    #[must_use]
    pub fn handle(&self) -> SliverOverlapHandle {
        self.handle.clone()
    }
}

impl Sliver for SliverOverlapAbsorber {
    fn build(&self, controller: &ScrollController) -> Widget {
        self.sliver.build(controller)
    }

    fn create_render_sliver(
        &self,
        controller: &ScrollController,
        axis: Axis,
        reverse: bool,
    ) -> Box<dyn RenderSliver> {
        Box::new(OverlapAbsorberRenderSliver {
            inner: RefCell::new(self.sliver.create_render_sliver(controller, axis, reverse)),
            handle: self.handle.clone(),
        })
    }
}

/// Sliver overlap injector for nested scroll view coordinators.
pub struct SliverOverlapInjector {
    handle: SliverOverlapHandle,
}

impl Default for SliverOverlapInjector {
    fn default() -> Self {
        Self::new()
    }
}

impl SliverOverlapInjector {
    #[must_use]
    pub fn new() -> Self {
        Self {
            handle: SliverOverlapHandle::new(),
        }
    }

    /// Creates an injector backed by an absorber's shared handle.
    #[must_use]
    pub fn with_handle(handle: SliverOverlapHandle) -> Self {
        Self { handle }
    }

    /// Naming-compatible constructor for code that mirrors Flutter's
    /// `SliverOverlapInjector` factory style.
    #[must_use]
    pub fn new_with_handle(handle: SliverOverlapHandle) -> Self {
        Self::with_handle(handle)
    }

    #[must_use]
    pub fn overlap_extent(&self) -> f32 {
        self.handle.extent()
    }
}

impl Sliver for SliverOverlapInjector {
    fn build(&self, _controller: &ScrollController) -> Widget {
        crate::layout::SizedBox::shrink().into()
    }

    fn create_render_sliver(
        &self,
        _controller: &ScrollController,
        _axis: Axis,
        _reverse: bool,
    ) -> Box<dyn RenderSliver> {
        Box::new(OverlapInjectorRenderSliver {
            handle: self.handle.clone(),
        })
    }
}

/// Retained state for a reorderable sliver. The order stores logical item
/// identities rather than visible slots, so moving an item preserves its
/// element state and measured extent.
#[derive(Clone)]
pub struct SliverReorderController {
    state: Rc<RefCell<SliverReorderState>>,
}

struct SliverReorderState {
    order: Vec<usize>,
    next_item: usize,
    revision: u64,
}

impl SliverReorderController {
    #[must_use]
    pub fn new(item_count: usize) -> Self {
        Self {
            state: Rc::new(RefCell::new(SliverReorderState {
                order: (0..item_count).collect(),
                next_item: item_count,
                revision: 0,
            })),
        }
    }

    #[must_use]
    pub fn item_count(&self) -> usize {
        self.state.borrow().order.len()
    }

    /// Returns the logical item IDs in their current visual order.
    #[must_use]
    pub fn order(&self) -> Vec<usize> {
        self.state.borrow().order.clone()
    }

    #[must_use]
    pub fn item_at(&self, position: usize) -> Option<usize> {
        self.state.borrow().order.get(position).copied()
    }

    #[must_use]
    pub fn position_of(&self, item: usize) -> Option<usize> {
        self.state
            .borrow()
            .order
            .iter()
            .position(|candidate| *candidate == item)
    }

    #[must_use]
    pub fn revision(&self) -> u64 {
        self.state.borrow().revision
    }

    /// Moves `from` to `to`, where `to` is the final position after removal.
    /// This is the same operation used by the retained sliver drop target.
    pub fn move_item(&self, from: usize, to: usize) -> bool {
        let mut state = self.state.borrow_mut();
        if from >= state.order.len() || to >= state.order.len() || from == to {
            return false;
        }
        let item = state.order.remove(from);
        state.order.insert(to, item);
        state.revision = state.revision.wrapping_add(1);
        true
    }

    /// Applies Flutter's `onReorder(oldIndex, newIndex)` convention. Flutter
    /// reports `newIndex` before the old item is removed, so destinations after
    /// the source are shifted back by one.
    pub fn reorder(&self, old_index: usize, new_index: usize) -> bool {
        let len = self.item_count();
        if old_index >= len || new_index > len {
            return false;
        }
        let destination = if old_index < new_index {
            new_index.saturating_sub(1)
        } else {
            new_index
        };
        self.move_item(old_index, destination.min(len.saturating_sub(1)))
    }

    /// Changes the number of logical items. Newly appended items receive
    /// fresh identities; existing identities and their retained state remain.
    pub fn set_item_count(&self, item_count: usize) {
        let mut state = self.state.borrow_mut();
        if item_count == state.order.len() {
            return;
        }
        if item_count < state.order.len() {
            state.order.truncate(item_count);
        } else {
            while state.order.len() < item_count {
                let item = state.next_item;
                state.next_item = state.next_item.wrapping_add(1);
                state.order.push(item);
            }
        }
        state.revision = state.revision.wrapping_add(1);
    }
}

struct ReorderableRenderSliver {
    index: MeasuredExtentIndex,
    builder: Rc<dyn Fn(usize) -> Widget>,
    controller: SliverReorderController,
    drag_context: DragDropContext<usize>,
    on_reorder: Option<Rc<dyn Fn(usize, usize)>>,
    widgets: HashMap<usize, Widget>,
    order: Vec<usize>,
    controller_revision: u64,
}

impl ReorderableRenderSliver {
    fn new(
        controller: SliverReorderController,
        builder: Rc<dyn Fn(usize) -> Widget>,
        drag_context: DragDropContext<usize>,
        on_reorder: Option<Rc<dyn Fn(usize, usize)>>,
    ) -> Self {
        let order = controller.order();
        Self {
            index: MeasuredExtentIndex::new(order.len(), 48.),
            builder,
            controller_revision: controller.revision(),
            controller,
            drag_context,
            on_reorder,
            widgets: HashMap::new(),
            order,
        }
    }

    fn sync_controller(&mut self) {
        let revision = self.controller.revision();
        if revision == self.controller_revision {
            return;
        }
        let next_order = self.controller.order();
        if self.order.len() == next_order.len() {
            let mut current = self.order.clone();
            for (destination, item) in next_order.iter().copied().enumerate() {
                let Some(source) = current.iter().position(|candidate| *candidate == item) else {
                    continue;
                };
                if source != destination {
                    let _ = self.index.move_item(source, destination);
                    let moved = current.remove(source);
                    current.insert(destination, moved);
                }
            }
        } else {
            self.index.set_len(next_order.len());
        }
        // A cached target captures its old destination position. Recreate the
        // lightweight drag wrappers after an order mutation while retaining
        // the underlying element by its stable logical item ID.
        if self.order != next_order {
            self.widgets.clear();
        }
        self.widgets
            .retain(|item, _| next_order.iter().any(|candidate| candidate == item));
        self.order = next_order;
        self.controller_revision = revision;
    }

    fn child_widget(&mut self, position: usize, item: usize) -> Widget {
        if let Some(widget) = self.widgets.get(&item) {
            return widget.clone();
        }
        let child = (self.builder)(item);
        let context = self.drag_context.clone();
        let controller = self.controller.clone();
        let on_reorder = self.on_reorder.clone();
        let draggable: Widget = Draggable::new(context.clone(), item, child).into();
        let target = DragTarget::new(context, draggable).on_drop(move |source_item| {
            let Some(source_position) = controller.position_of(source_item) else {
                return;
            };
            if controller.move_item(source_position, position)
                && let Some(callback) = &on_reorder
            {
                callback(source_position, position);
            }
        });
        let widget: Widget = target.into();
        self.widgets.insert(item, widget.clone());
        widget
    }
}

impl RenderSliver for ReorderableRenderSliver {
    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        self.sync_controller();
        let cache_start = (constraints.scroll_offset + constraints.cache_origin).max(0.);
        let cache_end = (cache_start + constraints.remaining_cache_extent).max(cache_start);
        let range =
            self.index
                .materialized_range(cache_start, (cache_end - cache_start).max(0.), 0.);
        self.widgets
            .retain(|item, _| self.order.iter().any(|candidate| candidate == item));
        let order = self.order.clone();
        let children = range
            .map(|position| {
                let item = order[position];
                let offset = self.index.offset_for_index(position);
                let extent = self.index.offset_for_index(position + 1) - offset;
                SliverChildLayout {
                    id: SliverChildId::list_item(item),
                    widget: self.child_widget(position, item),
                    offset,
                    cross_offset: 0.,
                    constraints: sliver_child_constraints(
                        constraints.axis,
                        constraints.cross_axis_extent,
                        Some(extent),
                    ),
                    extent,
                    pinned: false,
                }
            })
            .collect();
        SliverLayout {
            geometry: SliverGeometry::from_scroll_extent(constraints, self.index.total_extent()),
            children,
            absorbed_overlap: 0.,
        }
    }

    fn set_child_extent(&mut self, child: SliverChildId, extent: f32) -> bool {
        let Some(item) = child
            .0
            .checked_sub(1)
            .and_then(|id| usize::try_from(id).ok())
        else {
            return false;
        };
        self.order
            .iter()
            .position(|candidate| *candidate == item)
            .is_some_and(|position| self.index.set_measured_extent(position, extent))
    }

    fn revision(&self) -> u64 {
        self.controller
            .revision()
            .wrapping_add(self.index.revision())
    }
}

/// Reorderable sliver list. Items are wrapped in retained drag sources and
/// targets, so a normal pointer drag performs the same logical operation as a
/// Flutter reorderable list without OS-level input injection.
pub struct SliverReorderableList {
    controller: SliverReorderController,
    builder: Rc<dyn Fn(usize) -> Widget>,
    drag_context: DragDropContext<usize>,
    on_reorder: Option<Rc<dyn Fn(usize, usize)>>,
}

impl SliverReorderableList {
    #[must_use]
    pub fn new<W>(item_count: usize, builder: impl Fn(usize) -> W + 'static) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self {
            controller: SliverReorderController::new(item_count),
            builder: Rc::new(move |i| builder(i).into()),
            drag_context: DragDropContext::new(),
            on_reorder: None,
        }
    }

    /// Replaces the retained order controller.
    #[must_use]
    pub fn controller(mut self, controller: SliverReorderController) -> Self {
        self.controller = controller;
        self
    }

    #[must_use]
    pub fn reorder_controller(&self) -> SliverReorderController {
        self.controller.clone()
    }

    /// Receives `(old_position, new_position)` after a successful drop.
    #[must_use]
    pub fn on_reorder(mut self, callback: impl Fn(usize, usize) + 'static) -> Self {
        self.on_reorder = Some(Rc::new(callback));
        self
    }
}

impl Sliver for SliverReorderableList {
    fn build(&self, controller: &ScrollController) -> Widget {
        single_sliver_viewport(
            controller.clone(),
            Axis::Vertical,
            false,
            ScrollPhysics::default(),
            DEFAULT_SLIVER_CACHE_EXTENT,
            Clip::HardEdge,
            self.create_render_sliver(controller, Axis::Vertical, false),
        )
    }

    fn create_render_sliver(
        &self,
        _controller: &ScrollController,
        _axis: Axis,
        _reverse: bool,
    ) -> Box<dyn RenderSliver> {
        Box::new(ReorderableRenderSliver::new(
            self.controller.clone(),
            self.builder.clone(),
            self.drag_context.clone(),
            self.on_reorder.clone(),
        ))
    }
}

/// Hierarchical tree sliver.
#[derive(TypedBuilder)]
pub struct TreeSliver {
    #[builder(setter(into))]
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

struct AnimatedListEntry {
    id: u64,
    animation: AnimationController,
    removing: bool,
}

#[derive(Clone, Copy)]
struct AnimatedListEntrySnapshot {
    id: u64,
    value: f32,
    removing: bool,
}

struct SliverAnimatedListState {
    entries: Vec<AnimatedListEntry>,
    next_id: u64,
    duration: Duration,
    revision: u64,
    structure_revision: u64,
}

/// Retained insertion/removal state for [`SliverAnimatedList`]. Calling
/// `insert` or `remove` changes only this small state object; the viewport
/// then drives the size transition from its normal frame clock.
#[derive(Clone)]
pub struct SliverAnimatedListController {
    state: Rc<RefCell<SliverAnimatedListState>>,
}

impl SliverAnimatedListController {
    #[must_use]
    pub fn new(item_count: usize) -> Self {
        Self::with_duration(item_count, Duration::from_millis(250))
    }

    #[must_use]
    pub fn with_duration(item_count: usize, duration: Duration) -> Self {
        let entries = (0..item_count)
            .map(|id| {
                let animation = AnimationController::new(duration);
                animation.set_value(1.);
                AnimatedListEntry {
                    id: id as u64,
                    animation,
                    removing: false,
                }
            })
            .collect();
        Self {
            state: Rc::new(RefCell::new(SliverAnimatedListState {
                entries,
                next_id: item_count as u64,
                duration,
                revision: 0,
                structure_revision: 0,
            })),
        }
    }

    #[must_use]
    pub fn item_count(&self) -> usize {
        self.state.borrow().entries.len()
    }

    #[must_use]
    pub fn duration(&self) -> Duration {
        self.state.borrow().duration
    }

    #[must_use]
    pub fn revision(&self) -> u64 {
        self.state.borrow().revision
    }

    /// Inserts one logical item and starts its expansion from zero extent.
    pub fn insert(&self, index: usize) -> bool {
        self.insert_at(index, Instant::now())
    }

    /// Deterministic-clock form of [`Self::insert`], useful for tests.
    pub fn insert_at(&self, index: usize, now: Instant) -> bool {
        let mut state = self.state.borrow_mut();
        let duration = state.duration;
        let id = state.next_id;
        state.next_id = state.next_id.wrapping_add(1);
        let animation = AnimationController::new(duration);
        animation.forward(now);
        let index = index.min(state.entries.len());
        state.entries.insert(
            index,
            AnimatedListEntry {
                id,
                animation,
                removing: false,
            },
        );
        state.revision = state.revision.wrapping_add(1);
        state.structure_revision = state.structure_revision.wrapping_add(1);
        true
    }

    /// Starts shrinking the item at `index`. It remains in the sliver until
    /// the reverse animation reaches zero, so the retained element and drag
    /// state are not destroyed halfway through the transition.
    pub fn remove(&self, index: usize) -> bool {
        self.remove_at(index, Instant::now())
    }

    /// Deterministic-clock form of [`Self::remove`], useful for tests.
    pub fn remove_at(&self, index: usize, now: Instant) -> bool {
        let state = self.state.borrow_mut();
        let Some(entry) = state.entries.get(index) else {
            return false;
        };
        if entry.removing {
            return false;
        }
        entry.animation.reverse(now);
        drop(state);
        let mut state = self.state.borrow_mut();
        if let Some(entry) = state.entries.get_mut(index) {
            entry.removing = true;
            state.revision = state.revision.wrapping_add(1);
            state.structure_revision = state.structure_revision.wrapping_add(1);
            true
        } else {
            false
        }
    }

    /// Advances all active insert/remove transitions and removes entries that
    /// have completed their reverse animation.
    pub fn tick(&self, now: Instant) -> bool {
        let mut state = self.state.borrow_mut();
        let mut changed = false;
        for entry in &state.entries {
            changed |= entry.animation.tick(now);
        }
        let before = state.entries.len();
        state.entries.retain(|entry| {
            !(entry.removing && !entry.animation.is_active() && entry.animation.value() <= 0.)
        });
        if state.entries.len() != before {
            changed = true;
            state.structure_revision = state.structure_revision.wrapping_add(1);
        }
        if changed {
            state.revision = state.revision.wrapping_add(1);
        }
        changed
    }

    #[must_use]
    pub fn is_animating(&self) -> bool {
        self.state
            .borrow()
            .entries
            .iter()
            .any(|entry| entry.animation.is_active())
    }

    fn structure_revision(&self) -> u64 {
        self.state.borrow().structure_revision
    }

    fn snapshot(&self) -> Vec<AnimatedListEntrySnapshot> {
        self.state
            .borrow()
            .entries
            .iter()
            .map(|entry| AnimatedListEntrySnapshot {
                id: entry.id,
                value: entry.animation.value().clamp(0., 1.),
                removing: entry.removing,
            })
            .collect()
    }
}

struct AnimatedExtentRenderSliver {
    controller: SliverAnimatedListController,
    builder: Rc<dyn Fn(usize) -> Widget>,
    index: MeasuredExtentIndex,
    entries: Vec<AnimatedListEntrySnapshot>,
    base_extents: HashMap<u64, f32>,
    widgets: HashMap<u64, Widget>,
    structure_revision: u64,
    estimated_extent: f32,
}

impl AnimatedExtentRenderSliver {
    fn new(
        controller: SliverAnimatedListController,
        builder: Rc<dyn Fn(usize) -> Widget>,
        estimated_extent: f32,
    ) -> Self {
        let entries = controller.snapshot();
        Self {
            index: MeasuredExtentIndex::new(entries.len(), estimated_extent),
            controller,
            builder,
            entries,
            base_extents: HashMap::new(),
            widgets: HashMap::new(),
            structure_revision: u64::MAX,
            estimated_extent: estimated_extent.max(1.),
        }
    }

    fn sync_entries(&mut self) {
        let next = self.controller.snapshot();
        let structure_revision = self.controller.structure_revision();
        if structure_revision != self.structure_revision {
            self.index = MeasuredExtentIndex::new(next.len(), self.estimated_extent);
            let live = next.iter().map(|entry| entry.id).collect::<Vec<_>>();
            self.base_extents
                .retain(|id, _| live.iter().any(|candidate| candidate == id));
            self.widgets
                .retain(|id, _| live.iter().any(|candidate| candidate == id));
            for (position, entry) in next.iter().enumerate() {
                let base = self
                    .base_extents
                    .get(&entry.id)
                    .copied()
                    .unwrap_or(self.estimated_extent);
                let _ = self.index.set_measured_extent(position, base * entry.value);
            }
            self.structure_revision = structure_revision;
        } else {
            for (position, (old, next)) in self.entries.iter().zip(&next).enumerate() {
                if (old.value - next.value).abs() > f32::EPSILON {
                    let base = self
                        .base_extents
                        .get(&next.id)
                        .copied()
                        .unwrap_or(self.estimated_extent);
                    let _ = self.index.set_measured_extent(position, base * next.value);
                }
            }
        }
        self.entries = next;
    }

    fn child_widget(&mut self, position: usize, id: u64) -> Widget {
        if let Some(widget) = self.widgets.get(&id) {
            return widget.clone();
        }
        let widget = (self.builder)(position);
        self.widgets.insert(id, widget.clone());
        widget
    }
}

impl RenderSliver for AnimatedExtentRenderSliver {
    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        self.sync_entries();
        let cache_start = (constraints.scroll_offset + constraints.cache_origin).max(0.);
        let cache_end = (cache_start + constraints.remaining_cache_extent).max(cache_start);
        let mut positions = self
            .index
            .materialized_range(cache_start, (cache_end - cache_start).max(0.), 0.)
            .collect::<Vec<_>>();
        // A freshly inserted zero-size entry does not intersect an offset
        // interval mathematically, but it must still be retained so its size
        // transition can paint on the next frame.
        positions.extend(
            self.entries
                .iter()
                .enumerate()
                .filter(|(_, entry)| entry.value < 1. || entry.removing)
                .map(|(position, _)| position),
        );
        positions.sort_unstable();
        positions.dedup();
        let children = positions
            .into_iter()
            .map(|position| {
                let entry = self.entries[position];
                let offset = self.index.offset_for_index(position);
                let extent = (self.index.offset_for_index(position + 1) - offset).max(0.);
                SliverChildLayout {
                    id: SliverChildId::list_item(entry.id as usize),
                    widget: self.child_widget(position, entry.id),
                    offset,
                    cross_offset: 0.,
                    constraints: sliver_child_constraints(
                        constraints.axis,
                        constraints.cross_axis_extent,
                        Some(extent),
                    ),
                    extent,
                    pinned: false,
                }
            })
            .collect();
        SliverLayout {
            geometry: SliverGeometry::from_scroll_extent(constraints, self.index.total_extent()),
            children,
            absorbed_overlap: 0.,
        }
    }

    fn set_child_extent(&mut self, child: SliverChildId, extent: f32) -> bool {
        let Some(id) = child.0.checked_sub(1) else {
            return false;
        };
        let Some((position, entry)) = self
            .entries
            .iter()
            .enumerate()
            .find(|(_, entry)| entry.id == id)
        else {
            return false;
        };
        if entry.value <= f32::EPSILON || !extent.is_finite() || extent < 0. {
            return false;
        }
        let base = (extent / entry.value).max(0.);
        self.base_extents.insert(id, base);
        self.index.set_measured_extent(position, extent)
    }

    fn revision(&self) -> u64 {
        self.controller
            .revision()
            .wrapping_add(self.index.revision())
    }

    fn tick(&mut self, now: Instant) -> bool {
        let changed = self.controller.tick(now);
        if changed {
            self.sync_entries();
        }
        changed
    }

    fn is_animating(&self) -> bool {
        self.controller.is_animating()
    }
}

/// Animated list sliver with retained insertion/removal transitions.
pub struct SliverAnimatedList {
    builder: Rc<dyn Fn(usize) -> Widget>,
    controller: SliverAnimatedListController,
}

impl SliverAnimatedList {
    #[must_use]
    pub fn new<W>(item_count: usize, builder: impl Fn(usize) -> W + 'static) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self {
            controller: SliverAnimatedListController::new(item_count),
            builder: Rc::new(move |i| builder(i).into()),
        }
    }

    #[must_use]
    pub fn controller(mut self, controller: SliverAnimatedListController) -> Self {
        self.controller = controller;
        self
    }

    #[must_use]
    pub fn animation_controller(&self) -> SliverAnimatedListController {
        self.controller.clone()
    }
}

impl Sliver for SliverAnimatedList {
    fn build(&self, controller: &ScrollController) -> Widget {
        single_sliver_viewport(
            controller.clone(),
            Axis::Vertical,
            false,
            ScrollPhysics::default(),
            DEFAULT_SLIVER_CACHE_EXTENT,
            Clip::HardEdge,
            self.create_render_sliver(controller, Axis::Vertical, false),
        )
    }

    fn create_render_sliver(
        &self,
        _controller: &ScrollController,
        _axis: Axis,
        _reverse: bool,
    ) -> Box<dyn RenderSliver> {
        Box::new(AnimatedExtentRenderSliver::new(
            self.controller.clone(),
            self.builder.clone(),
            48.,
        ))
    }
}

/// Animated grid sliver. Rows use the same retained size-transition engine;
/// each row builder still receives the original cell indices.
pub struct SliverAnimatedGrid {
    item_count: usize,
    cross_axis_count: usize,
    builder: Rc<dyn Fn(usize) -> Widget>,
    controller: SliverAnimatedListController,
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
        let cross_axis_count = cross_axis_count.max(1);
        Self {
            item_count,
            cross_axis_count,
            controller: SliverAnimatedListController::new(item_count.div_ceil(cross_axis_count)),
            builder: Rc::new(move |i| builder(i).into()),
        }
    }

    #[must_use]
    pub fn controller(mut self, controller: SliverAnimatedListController) -> Self {
        self.controller = controller;
        self
    }

    #[must_use]
    pub fn animation_controller(&self) -> SliverAnimatedListController {
        self.controller.clone()
    }
}

impl Sliver for SliverAnimatedGrid {
    fn build(&self, controller: &ScrollController) -> Widget {
        let b = self.builder.clone();
        GridView::builder(self.item_count, self.cross_axis_count, 80.0, move |i| b(i))
            .controller(controller.clone())
            .into()
    }

    fn create_render_sliver(
        &self,
        _controller: &ScrollController,
        _axis: Axis,
        _reverse: bool,
    ) -> Box<dyn RenderSliver> {
        let columns = self.cross_axis_count;
        let count = self.item_count;
        let builder = self.builder.clone();
        Box::new(AnimatedExtentRenderSliver::new(
            self.controller.clone(),
            Rc::new(move |row| {
                let start = row * columns;
                Widget::from(Row::new(
                    (start..(start + columns).min(count)).map(|index| builder(index)),
                ))
            }),
            80.,
        ))
    }
}
