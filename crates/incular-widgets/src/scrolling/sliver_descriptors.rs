use super::*;

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

/// A lazy variable-extent list sliver.
///
/// Use [`SliverFixedExtentList`] when every child has a known extent, or
/// [`SliverVariedExtentList`] when the extent comes from an index builder.
#[derive(Clone)]
pub struct SliverList {
    item_count: usize,
    builder: Rc<dyn Fn(usize) -> Widget>,
}

impl SliverList {
    #[must_use]
    pub fn builder<W>(item_count: usize, builder: impl Fn(usize) -> W + 'static) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self {
            item_count,
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
            MeasuredExtentIndex::new(self.item_count, DEFAULT_LAZY_ITEM_EXTENT),
            self.builder.clone(),
        ))
    }
}

/// A lazy grid sliver driven by a [`SliverGridDelegate`].
#[derive(Clone)]
pub struct SliverGrid {
    item_count: usize,
    delegate: SliverGridDelegate,
    builder: Rc<dyn Fn(usize) -> Widget>,
}

impl SliverGrid {
    #[must_use]
    pub fn builder<W>(
        item_count: usize,
        delegate: SliverGridDelegate,
        builder: impl Fn(usize) -> W + 'static,
    ) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self {
            item_count,
            delegate,
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
        axis: Axis,
        _reverse: bool,
    ) -> Box<dyn RenderSliver> {
        Box::new(GridRenderSliver::new(
            self.item_count,
            self.delegate.clone(),
            self.builder.clone(),
            axis,
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
    #[builder(default = WidgetDefaults::DEFAULT.scroll_direction)]
    scroll_direction: Axis,
    #[builder(default = false)]
    reverse: bool,
    #[builder(default, setter(strip_option))]
    physics: Option<ScrollPhysics>,
    #[builder(default = WidgetDefaults::DEFAULT.sliver_cache_extent, setter(transform = |extent: f32| extent.max(0.0)))]
    cache_extent: f32,
    #[builder(default = WidgetDefaults::DEFAULT.scroll_clip_behavior)]
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
            scroll_direction: WidgetDefaults::DEFAULT.scroll_direction,
            reverse: false,
            physics: None,
            cache_extent: WidgetDefaults::DEFAULT.sliver_cache_extent,
            clip_behavior: WidgetDefaults::DEFAULT.scroll_clip_behavior,
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
    #[builder(default = WidgetDefaults::DEFAULT.scroll_direction)]
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
            axis_direction: WidgetDefaults::DEFAULT.scroll_direction,
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
            WidgetDefaults::DEFAULT.scroll_direction,
            false,
            ScrollPhysics::default(),
            DEFAULT_SLIVER_CACHE_EXTENT,
            true,
            WidgetDefaults::DEFAULT.scroll_clip_behavior,
            Rc::new(SequenceViewportDelegate::new(render_slivers)),
        )
    }
}

/// A simple sequential layout along the main axis.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct ListBody {
    #[builder(default = WidgetDefaults::DEFAULT.scroll_direction)]
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
            main_axis: WidgetDefaults::DEFAULT.scroll_direction,
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
        let index = MeasuredExtentIndex::with_estimates(
            self.item_count,
            DEFAULT_LAZY_ITEM_EXTENT,
            |index| (self.extent_builder)(index),
        );
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
            .unwrap_or(DEFAULT_LAZY_ITEM_EXTENT)
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
    #[builder(default = WidgetDefaults::DEFAULT.sliver_fill_viewport_extent, setter(transform = |extent: f32| extent.max(1.0)))]
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
            fallback_extent: WidgetDefaults::DEFAULT.sliver_fill_viewport_extent,
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
            fallback_extent: WidgetDefaults::DEFAULT.sliver_fill_viewport_extent,
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

/// A sliver builder that receives the current sliver constraints during
/// layout, matching Flutter's `SliverLayoutBuilder` contract.
pub struct SliverLayoutBuilder {
    builder: SliverLayoutBuilderFn,
}

impl SliverLayoutBuilder {
    #[must_use]
    pub fn new<W>(builder: impl Fn(SliverConstraints) -> W + 'static) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self {
            builder: Rc::new(move |constraints| builder(constraints).into()),
        }
    }
}

impl Sliver for SliverLayoutBuilder {
    fn build(&self, _controller: &ScrollController) -> Widget {
        // The callback belongs to the render-sliver layout phase. Calling it
        // here would fabricate constraints and run application code twice:
        // once while constructing the widget description and again when the
        // viewport has real geometry. This placeholder is replaced by
        // `LayoutBuilderRenderSliver::perform_layout`.
        SizedBox::shrink().into()
    }

    fn create_render_sliver(
        &self,
        _controller: &ScrollController,
        _axis: Axis,
        _reverse: bool,
    ) -> Box<dyn RenderSliver> {
        Box::new(LayoutBuilderRenderSliver {
            builder: self.builder.clone(),
            child: SizedBox::shrink().into(),
            extent: Cell::new(DEFAULT_LAZY_ITEM_EXTENT),
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
    fn scroll_layout_dependency(&self) -> SliverScrollDependency {
        self.children
            .iter()
            .map(|(child, _)| child.borrow().scroll_layout_dependency())
            .max()
            .unwrap_or(SliverScrollDependency::CacheWindow)
    }

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
                .unwrap_or(DEFAULT_LAZY_ITEM_EXTENT)
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
                    .unwrap_or(DEFAULT_LAZY_ITEM_EXTENT)
                    .max(1.),
            ),
            scroll_state: HeaderScrollState::default(),
        })
    }
}

/// Scroll behavior of a resizing header.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SliverHeaderScrollBehavior {
    /// Collapse, then scroll the minimum extent out of view.
    Scroll,
    /// Keep the collapsed header at the viewport edge.
    #[default]
    Pinned,
    /// Reveal the header immediately when scrolling reverses.
    Floating,
    /// Keep the minimum extent visible and expand on scroll reversal.
    FloatingPinned,
}

/// Response of a resizing header to leading overscroll.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SliverHeaderOverscrollBehavior {
    /// Preserve the header extent and translate with the viewport.
    #[default]
    Translate,
    /// Expand into leading overscroll without increasing logical scroll extent.
    Stretch,
}

/// Header that shrinks from its maximum to minimum extent while scrolling.
/// The default scroll behavior pins the collapsed header.
/// Negative and non-finite bounds become zero; the effective maximum is at
/// least the minimum. Constructor and typed-builder paths use the same policy.
#[derive(TypedBuilder)]
pub struct SliverResizingHeader {
    #[builder(setter(transform = |extent: f32| SliverResizingHeader::normalize_extent(extent)))]
    min_extent: f32,
    #[builder(setter(transform = |extent: f32| SliverResizingHeader::normalize_extent(extent)))]
    max_extent: f32,
    #[builder(setter(into))]
    child: Widget,
    #[builder(default)]
    scroll_behavior: SliverHeaderScrollBehavior,
    #[builder(default)]
    overscroll_behavior: SliverHeaderOverscrollBehavior,
}

impl SliverResizingHeader {
    fn normalize_extent(extent: f32) -> f32 {
        if extent.is_finite() {
            extent.max(0.)
        } else {
            0.
        }
    }

    #[must_use]
    pub fn new(min_extent: f32, max_extent: f32, child: impl Into<Widget>) -> Self {
        Self {
            min_extent: Self::normalize_extent(min_extent),
            max_extent: Self::normalize_extent(max_extent),
            child: child.into(),
            scroll_behavior: SliverHeaderScrollBehavior::Pinned,
            overscroll_behavior: SliverHeaderOverscrollBehavior::Translate,
        }
    }

    #[must_use]
    pub fn scroll_behavior(mut self, behavior: SliverHeaderScrollBehavior) -> Self {
        self.scroll_behavior = behavior;
        self
    }

    #[must_use]
    pub fn overscroll_behavior(mut self, behavior: SliverHeaderOverscrollBehavior) -> Self {
        self.overscroll_behavior = behavior;
        self
    }

    #[must_use]
    pub fn min_extent(&self) -> f32 {
        self.min_extent
    }

    #[must_use]
    pub fn max_extent(&self) -> f32 {
        self.max_extent.max(self.min_extent)
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
        SliverPersistentHeader::new(self.max_extent(), self.child.clone())
            .pinned(matches!(
                self.scroll_behavior,
                SliverHeaderScrollBehavior::Pinned | SliverHeaderScrollBehavior::FloatingPinned
            ))
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
            min_extent: self.min_extent(),
            max_extent: self.max_extent(),
            scroll_behavior: self.scroll_behavior,
            overscroll_behavior: self.overscroll_behavior,
            scroll_state: HeaderScrollState::default(),
        })
    }
}

/// Naturally measured header with typed scroll and overscroll policies.
///
/// The logical scroll extent is the child's measured main-axis extent; it is
/// learned from unstretched layouts and reused while stretched. Stretched
/// layouts present `natural + leading overlap` without growing the scroll
/// range and return to the current measurement when overscroll ends. The
/// default scroll behavior pins and the default overscroll translates.
#[derive(TypedBuilder)]
pub struct SliverNaturalHeader {
    #[builder(setter(into))]
    child: Widget,
    #[builder(default)]
    scroll_behavior: SliverHeaderScrollBehavior,
    #[builder(default)]
    overscroll_behavior: SliverHeaderOverscrollBehavior,
}

impl SliverNaturalHeader {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            scroll_behavior: SliverHeaderScrollBehavior::Pinned,
            overscroll_behavior: SliverHeaderOverscrollBehavior::Translate,
        }
    }

    #[must_use]
    pub fn scroll_behavior(mut self, behavior: SliverHeaderScrollBehavior) -> Self {
        self.scroll_behavior = behavior;
        self
    }

    #[must_use]
    pub fn overscroll_behavior(mut self, behavior: SliverHeaderOverscrollBehavior) -> Self {
        self.overscroll_behavior = behavior;
        self
    }
}

impl Sliver for SliverNaturalHeader {
    fn build(&self, controller: &ScrollController) -> Widget {
        self.build_with_config(controller, Axis::Vertical, false)
    }

    fn build_with_config(
        &self,
        controller: &ScrollController,
        axis: Axis,
        reverse: bool,
    ) -> Widget {
        match self.scroll_behavior {
            SliverHeaderScrollBehavior::Scroll => SliverToBoxAdapter::new(self.child.clone())
                .build_with_config(controller, axis, reverse),
            SliverHeaderScrollBehavior::Floating => SliverPersistentHeader::new(
                widget_main_extent_hint(&self.child, axis)
                    .unwrap_or(DEFAULT_LAZY_ITEM_EXTENT)
                    .max(1.),
                self.child.clone(),
            )
            .pinned(false)
            .build_with_config(controller, axis, reverse),
            SliverHeaderScrollBehavior::Pinned | SliverHeaderScrollBehavior::FloatingPinned => {
                Widget::persistent_header_with_config(
                    controller.clone(),
                    self.child.clone(),
                    axis,
                    reverse,
                    true,
                )
            }
        }
    }

    fn create_render_sliver(
        &self,
        _controller: &ScrollController,
        axis: Axis,
        _reverse: bool,
    ) -> Box<dyn RenderSliver> {
        Box::new(NaturalHeaderRenderSliver {
            child: self.child.clone(),
            scroll_behavior: self.scroll_behavior,
            overscroll_behavior: self.overscroll_behavior,
            natural: Cell::new(
                widget_main_extent_hint(&self.child, axis)
                    .unwrap_or(DEFAULT_LAZY_ITEM_EXTENT)
                    .max(0.),
            ),
            stretched: Cell::new(false),
            scroll_state: HeaderScrollState::default(),
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
