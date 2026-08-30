use super::*;

/// A first-class scrollable box that scrolls a single child.
#[derive(Clone, TypedBuilder)]
pub struct SingleChildScrollView {
    #[builder(setter(into))]
    child: Widget,
    #[builder(default = WidgetDefaults::DEFAULT.scroll_direction)]
    scroll_direction: Axis,
    #[builder(default = false)]
    reverse: bool,
    #[builder(default, setter(strip_option))]
    padding: Option<EdgeInsets>,
    #[builder(default, setter(strip_option))]
    controller: Option<ScrollController>,
    #[builder(default, setter(strip_option))]
    physics: Option<ScrollPhysics>,
    #[builder(default = WidgetDefaults::DEFAULT.scroll_clip_behavior)]
    clip_behavior: Clip,
}

impl SingleChildScrollView {
    /// Creates a scrollable single-child viewport.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            scroll_direction: WidgetDefaults::DEFAULT.scroll_direction,
            reverse: false,
            padding: None,
            controller: None,
            physics: None,
            clip_behavior: WidgetDefaults::DEFAULT.scroll_clip_behavior,
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

/// Delegate governing two-dimensional grid layout.
///
/// This is the Rust representation of Flutter's `SliverGridDelegate`. The
/// delegate owns grid policy; `GridView` and `SliverGrid` only provide the
/// child source and viewport. Keeping those responsibilities separate avoids
/// the old `cross_axis_count`/`row_extent` constructor workaround.
#[derive(Clone, Debug, PartialEq)]
pub enum SliverGridDelegate {
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

impl SliverGridDelegate {
    /// Creates a delegate with a fixed number of children on the cross axis.
    #[must_use]
    pub fn fixed_cross_axis_count(cross_axis_count: usize) -> Self {
        Self::FixedCrossAxisCount {
            cross_axis_count: cross_axis_count.max(1),
            main_axis_spacing: 0.,
            cross_axis_spacing: 0.,
            child_aspect_ratio: 1.,
            main_axis_extent: None,
        }
    }

    /// Creates a delegate that derives the cross-axis count from the
    /// available extent and a maximum tile extent.
    #[must_use]
    pub fn max_cross_axis_extent(max_cross_axis_extent: f32) -> Self {
        Self::MaxCrossAxisExtent {
            max_cross_axis_extent: if max_cross_axis_extent.is_finite() {
                max_cross_axis_extent.max(1.)
            } else {
                1.
            },
            main_axis_spacing: 0.,
            cross_axis_spacing: 0.,
            child_aspect_ratio: 1.,
            main_axis_extent: None,
        }
    }

    #[must_use]
    pub fn main_axis_spacing(mut self, spacing: f32) -> Self {
        let spacing = if spacing.is_finite() {
            spacing.max(0.)
        } else {
            0.
        };
        match &mut self {
            Self::FixedCrossAxisCount {
                main_axis_spacing, ..
            }
            | Self::MaxCrossAxisExtent {
                main_axis_spacing, ..
            } => *main_axis_spacing = spacing,
        }
        self
    }

    #[must_use]
    pub fn cross_axis_spacing(mut self, spacing: f32) -> Self {
        let spacing = if spacing.is_finite() {
            spacing.max(0.)
        } else {
            0.
        };
        match &mut self {
            Self::FixedCrossAxisCount {
                cross_axis_spacing, ..
            }
            | Self::MaxCrossAxisExtent {
                cross_axis_spacing, ..
            } => *cross_axis_spacing = spacing,
        }
        self
    }

    #[must_use]
    pub fn child_aspect_ratio(mut self, ratio: f32) -> Self {
        let ratio = if ratio.is_finite() {
            ratio.max(f32::EPSILON)
        } else {
            1.
        };
        match &mut self {
            Self::FixedCrossAxisCount {
                child_aspect_ratio, ..
            }
            | Self::MaxCrossAxisExtent {
                child_aspect_ratio, ..
            } => *child_aspect_ratio = ratio,
        }
        self
    }

    #[must_use]
    pub fn main_axis_extent(mut self, extent: f32) -> Self {
        let extent = extent.is_finite().then_some(extent.max(1.));
        match &mut self {
            Self::FixedCrossAxisCount {
                main_axis_extent, ..
            }
            | Self::MaxCrossAxisExtent {
                main_axis_extent, ..
            } => *main_axis_extent = extent,
        }
        self
    }

    pub(super) fn cross_axis_count(&self, available_extent: f32) -> usize {
        match *self {
            Self::FixedCrossAxisCount {
                cross_axis_count, ..
            } => cross_axis_count.max(1),
            Self::MaxCrossAxisExtent {
                max_cross_axis_extent,
                cross_axis_spacing,
                ..
            } => ((available_extent.max(0.) + cross_axis_spacing)
                / (max_cross_axis_extent + cross_axis_spacing))
                .floor()
                .max(1.) as usize,
        }
    }

    pub(super) fn resolved_cross_axis_spacing(&self) -> f32 {
        match *self {
            Self::FixedCrossAxisCount {
                cross_axis_spacing, ..
            }
            | Self::MaxCrossAxisExtent {
                cross_axis_spacing, ..
            } => cross_axis_spacing,
        }
    }

    pub(super) fn resolved_main_axis_spacing(&self) -> f32 {
        match *self {
            Self::FixedCrossAxisCount {
                main_axis_spacing, ..
            }
            | Self::MaxCrossAxisExtent {
                main_axis_spacing, ..
            } => main_axis_spacing,
        }
    }

    pub(super) fn resolve_main_axis_extent(&self, cross_axis_extent: f32, columns: usize) -> f32 {
        let (main_axis_extent, child_aspect_ratio) = match *self {
            Self::FixedCrossAxisCount {
                main_axis_extent,
                child_aspect_ratio,
                ..
            }
            | Self::MaxCrossAxisExtent {
                main_axis_extent,
                child_aspect_ratio,
                ..
            } => (main_axis_extent, child_aspect_ratio),
        };
        if let Some(extent) = main_axis_extent {
            return extent.max(1.);
        }
        let spacing = self.resolved_cross_axis_spacing() * columns.saturating_sub(1) as f32;
        ((cross_axis_extent - spacing).max(0.)
            / columns.max(1) as f32
            / child_aspect_ratio.max(f32::EPSILON))
        .max(1.)
    }
}

impl Default for SliverGridDelegate {
    fn default() -> Self {
        Self::fixed_cross_axis_count(WidgetDefaults::DEFAULT.grid_cross_axis_count)
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
    #[builder(default, setter(skip))]
    item_extent: Option<f32>,
    #[builder(default, setter(skip))]
    item_extent_builder: Option<Rc<dyn Fn(usize) -> f32>>,
    #[builder(default, setter(skip))]
    prototype_item: Option<Widget>,
    #[builder(default = WidgetDefaults::DEFAULT.scroll_direction)]
    scroll_direction: Axis,
    #[builder(default = false)]
    reverse: bool,
    #[builder(default = false)]
    shrink_wrap: bool,
    #[builder(default, setter(strip_option))]
    controller: Option<ScrollController>,
    #[builder(default, setter(strip_option))]
    padding: Option<EdgeInsets>,
    #[builder(default, setter(strip_option))]
    cache_extent: Option<f32>,
    #[builder(default, setter(strip_option))]
    physics: Option<ScrollPhysics>,
    #[builder(default = WidgetDefaults::DEFAULT.scroll_clip_behavior)]
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
            item_extent: None,
            item_extent_builder: None,
            prototype_item: None,
            scroll_direction: WidgetDefaults::DEFAULT.scroll_direction,
            reverse: false,
            shrink_wrap: false,
            controller: None,
            padding: None,
            cache_extent: None,
            physics: None,
            clip_behavior: WidgetDefaults::DEFAULT.scroll_clip_behavior,
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
            item_extent: None,
            item_extent_builder: None,
            prototype_item: None,
            scroll_direction: WidgetDefaults::DEFAULT.scroll_direction,
            reverse: false,
            shrink_wrap: false,
            controller: None,
            padding: None,
            cache_extent: None,
            physics: None,
            clip_behavior: WidgetDefaults::DEFAULT.scroll_clip_behavior,
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
            item_extent: None,
            item_extent_builder: None,
            prototype_item: None,
            scroll_direction: WidgetDefaults::DEFAULT.scroll_direction,
            reverse: false,
            shrink_wrap: false,
            controller: None,
            padding: None,
            cache_extent: None,
            physics: None,
            clip_behavior: WidgetDefaults::DEFAULT.scroll_clip_behavior,
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

    /// Sets whether the list sizes itself to its contents instead of
    /// requiring a bounded viewport extent.
    #[must_use]
    pub fn shrink_wrap(mut self, shrink_wrap: bool) -> Self {
        self.shrink_wrap = shrink_wrap;
        self
    }

    /// Sets cache extent in logical pixels.
    #[must_use]
    pub fn cache_extent(mut self, cache_extent: f32) -> Self {
        self.cache_extent = Some(if cache_extent.is_finite() {
            cache_extent.max(0.0)
        } else {
            0.0
        });
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
        self.item_extent = extent.is_finite().then_some(extent.max(1.));
        self.item_extent_builder = None;
        self.prototype_item = None;
        self
    }

    /// Sets a prototype child item for measuring extent.
    #[must_use]
    pub fn prototype_item(mut self, prototype: impl Into<Widget>) -> Self {
        self.prototype_item = Some(prototype.into());
        self.item_extent = None;
        self.item_extent_builder = None;
        self
    }

    /// Sets the extent of each child using the same index as the item
    /// builder. The measured index remains internal to the retained sliver.
    #[must_use]
    pub fn item_extent_builder(mut self, builder: impl Fn(usize) -> f32 + 'static) -> Self {
        self.item_extent_builder = Some(Rc::new(builder));
        self.item_extent = None;
        self.prototype_item = None;
        self
    }
}

impl From<ListView> for Widget {
    fn from(value: ListView) -> Self {
        let controller = value.controller.unwrap_or_default();
        let item_extent = value.item_extent;
        let item_extent_builder = value.item_extent_builder;
        let prototype_item = value.prototype_item;
        let strategy = value
            .strategy
            .unwrap_or_else(|| ListViewStrategy::Children(value.children));
        let mut sliver: Box<dyn RenderSliver> = match strategy {
            ListViewStrategy::Children(children) => {
                let children = Rc::new(children);
                build_list_render_sliver(
                    children.len(),
                    Rc::new(move |index| children[index].clone()),
                    value.scroll_direction,
                    item_extent,
                    item_extent_builder,
                    prototype_item,
                )
            }
            ListViewStrategy::Builder {
                item_count,
                builder,
            } => build_list_render_sliver(
                item_count,
                builder,
                value.scroll_direction,
                item_extent,
                item_extent_builder,
                prototype_item,
            ),
        };

        if let Some(padding) = value.padding {
            sliver = Box::new(PaddingRenderSliver {
                inner: RefCell::new(sliver),
                padding,
            });
        }
        single_sliver_viewport_with_options(
            SliverViewportOptions {
                controller,
                axis: value.scroll_direction,
                reverse: value.reverse,
                physics: value.physics.unwrap_or_default(),
                cache_extent: value.cache_extent.unwrap_or(DEFAULT_SLIVER_CACHE_EXTENT),
                clip_behavior: value.clip_behavior,
                shrink_wrap: value.shrink_wrap,
            },
            sliver,
        )
    }
}

fn build_list_render_sliver(
    item_count: usize,
    builder: Rc<dyn Fn(usize) -> Widget>,
    axis: Axis,
    item_extent: Option<f32>,
    item_extent_builder: Option<Rc<dyn Fn(usize) -> f32>>,
    prototype_item: Option<Widget>,
) -> Box<dyn RenderSliver> {
    if let Some(extent) = item_extent {
        return Box::new(FixedExtentRenderSliver::new(item_count, extent, builder));
    }
    if let Some(prototype) = prototype_item {
        let extent = widget_main_extent_hint(&prototype, axis)
            .unwrap_or(DEFAULT_LAZY_ITEM_EXTENT)
            .max(1.);
        return Box::new(FixedExtentRenderSliver::new(item_count, extent, builder));
    }
    let index = if let Some(extent_builder) = item_extent_builder {
        MeasuredExtentIndex::with_estimates(item_count, DEFAULT_LAZY_ITEM_EXTENT, move |index| {
            extent_builder(index)
        })
    } else {
        MeasuredExtentIndex::new(item_count, DEFAULT_LAZY_ITEM_EXTENT)
    };
    Box::new(VariableExtentRenderSliver::new(index, builder))
}

/// Content strategy for [`GridView`].
#[derive(Clone)]
enum GridViewStrategy {
    Count {
        delegate: SliverGridDelegate,
        children: Vec<Widget>,
    },
    Builder {
        item_count: usize,
        delegate: SliverGridDelegate,
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
    #[builder(default)]
    grid_delegate: SliverGridDelegate,
    #[builder(default = WidgetDefaults::DEFAULT.scroll_direction)]
    scroll_direction: Axis,
    #[builder(default = false)]
    reverse: bool,
    #[builder(default = false)]
    shrink_wrap: bool,
    #[builder(default, setter(strip_option))]
    controller: Option<ScrollController>,
    #[builder(default, setter(strip_option))]
    padding: Option<EdgeInsets>,
    #[builder(default, setter(strip_option))]
    cache_extent: Option<f32>,
    #[builder(default, setter(strip_option))]
    physics: Option<ScrollPhysics>,
    #[builder(default = WidgetDefaults::DEFAULT.scroll_clip_behavior)]
    clip_behavior: Clip,
}

impl Default for GridView {
    fn default() -> Self {
        Self::typed_builder().build()
    }
}

impl GridView {
    /// Creates a grid from a Flutter-style sliver grid delegate and static
    /// children.
    #[must_use]
    pub fn new(
        grid_delegate: SliverGridDelegate,
        children: impl IntoIterator<Item = impl Into<Widget>>,
    ) -> Self {
        Self {
            children: children.into_iter().map(Into::into).collect(),
            strategy: None,
            grid_delegate,
            scroll_direction: WidgetDefaults::DEFAULT.scroll_direction,
            reverse: false,
            shrink_wrap: false,
            controller: None,
            padding: None,
            cache_extent: None,
            physics: None,
            clip_behavior: WidgetDefaults::DEFAULT.scroll_clip_behavior,
        }
    }

    /// Creates a grid whose column count is derived from the available
    /// cross-axis extent.
    #[must_use]
    pub fn extent(
        max_cross_axis_extent: f32,
        children: impl IntoIterator<Item = impl Into<Widget>>,
    ) -> Self {
        Self::new(
            SliverGridDelegate::max_cross_axis_extent(max_cross_axis_extent),
            children,
        )
    }

    /// Creates a fixed cross-axis count grid from static children.
    #[must_use]
    pub fn count(
        cross_axis_count: usize,
        children: impl IntoIterator<Item = impl Into<Widget>>,
    ) -> Self {
        Self::new(
            SliverGridDelegate::fixed_cross_axis_count(cross_axis_count),
            children,
        )
    }

    /// Creates a lazily virtualized grid with a row-major builder and a
    /// Flutter-style sliver grid delegate.
    #[must_use]
    pub fn builder<W>(
        item_count: usize,
        grid_delegate: SliverGridDelegate,
        builder: impl Fn(usize) -> W + 'static,
    ) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self {
            children: Vec::new(),
            strategy: Some(GridViewStrategy::Builder {
                item_count,
                delegate: grid_delegate,
                builder: Rc::new(move |i| builder(i).into()),
            }),
            grid_delegate: SliverGridDelegate::default(),
            scroll_direction: WidgetDefaults::DEFAULT.scroll_direction,
            reverse: false,
            shrink_wrap: false,
            controller: None,
            padding: None,
            cache_extent: None,
            physics: None,
            clip_behavior: WidgetDefaults::DEFAULT.scroll_clip_behavior,
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

    /// Sets the sliver grid delegate for a static-child grid.
    #[must_use]
    pub fn grid_delegate(mut self, delegate: SliverGridDelegate) -> Self {
        self.grid_delegate = delegate;
        self
    }

    /// Sets whether the grid sizes itself to its contents.
    #[must_use]
    pub fn shrink_wrap(mut self, shrink_wrap: bool) -> Self {
        self.shrink_wrap = shrink_wrap;
        self
    }

    /// Sets the lazy child cache in logical pixels.
    #[must_use]
    pub fn cache_extent(mut self, cache_extent: f32) -> Self {
        self.cache_extent = Some(if cache_extent.is_finite() {
            cache_extent.max(0.)
        } else {
            0.
        });
        self
    }

    /// Sets clipping behavior at the viewport boundary.
    #[must_use]
    pub fn clip_behavior(mut self, clip: Clip) -> Self {
        self.clip_behavior = clip;
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
        let mut sliver: Box<dyn RenderSliver> =
            match value.strategy.unwrap_or_else(|| GridViewStrategy::Count {
                delegate: value.grid_delegate,
                children: value.children,
            }) {
                GridViewStrategy::Count { delegate, children } => {
                    let children = Rc::new(children);
                    Box::new(GridRenderSliver::new(
                        children.len(),
                        delegate,
                        Rc::new(move |index| children[index].clone()),
                        value.scroll_direction,
                    ))
                }
                GridViewStrategy::Builder {
                    item_count,
                    delegate,
                    builder,
                } => Box::new(GridRenderSliver::new(
                    item_count,
                    delegate,
                    builder,
                    value.scroll_direction,
                )),
            };

        if let Some(padding) = value.padding {
            sliver = Box::new(PaddingRenderSliver {
                inner: RefCell::new(sliver),
                padding,
            });
        }
        single_sliver_viewport_with_options(
            SliverViewportOptions {
                controller,
                axis: value.scroll_direction,
                reverse: value.reverse,
                physics: value.physics.unwrap_or_default(),
                cache_extent: value.cache_extent.unwrap_or(DEFAULT_SLIVER_CACHE_EXTENT),
                clip_behavior: value.clip_behavior,
                shrink_wrap: value.shrink_wrap,
            },
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
    #[builder(default = WidgetDefaults::DEFAULT.page_scroll_direction)]
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
            scroll_direction: WidgetDefaults::DEFAULT.page_scroll_direction,
            reverse: false,
            page_snapping: true,
            viewport_fraction: 1.0,
            physics: None,
        }
    }

    /// Creates a lazily virtualized PageView with a page builder closure.
    #[must_use]
    pub fn builder<W>(page_count: usize, builder: impl Fn(usize) -> W + 'static) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self {
            children: Vec::new(),
            strategy: Some(PageViewStrategy::Builder {
                page_count,
                builder: Rc::new(move |i| builder(i).into()),
            }),
            controller: None,
            scroll_direction: WidgetDefaults::DEFAULT.page_scroll_direction,
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
        let physics_for_extent = || {
            if let Some(physics) = value.physics {
                physics
            } else if value.page_snapping {
                ScrollPhysics::clamping().page()
            } else {
                ScrollPhysics::clamping()
            }
        };
        match value
            .strategy
            .unwrap_or_else(|| PageViewStrategy::Children(value.children))
        {
            PageViewStrategy::Children(children) => {
                let sliver = SliverFillViewport::new(children)
                    .viewport_fraction(fraction)
                    .fallback_extent(WidgetDefaults::DEFAULT.sliver_fill_viewport_extent);
                let render =
                    sliver.create_render_sliver(&controller, value.scroll_direction, value.reverse);
                single_sliver_viewport(
                    controller,
                    value.scroll_direction,
                    value.reverse,
                    physics_for_extent(),
                    DEFAULT_SLIVER_CACHE_EXTENT,
                    WidgetDefaults::DEFAULT.scroll_clip_behavior,
                    render,
                )
            }
            PageViewStrategy::Builder {
                page_count,
                builder,
            } => {
                let sliver = SliverFillViewport::builder(page_count, move |index| builder(index))
                    .viewport_fraction(fraction)
                    .fallback_extent(WidgetDefaults::DEFAULT.sliver_fill_viewport_extent);
                let render =
                    sliver.create_render_sliver(&controller, value.scroll_direction, value.reverse);
                single_sliver_viewport(
                    controller,
                    value.scroll_direction,
                    value.reverse,
                    physics_for_extent(),
                    DEFAULT_SLIVER_CACHE_EXTENT,
                    WidgetDefaults::DEFAULT.scroll_clip_behavior,
                    render,
                )
            }
        }
    }
}
