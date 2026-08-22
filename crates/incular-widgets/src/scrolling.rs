//! High-level scroll and unified-sliver descriptors over Incular's retained
//! vertical viewport.

use std::rc::Rc;

use crate::{MeasuredExtentIndex, ScrollController, ScrollView, VirtualList, Widget};

/// Flutter-compatible list API backed by the lazy fixed-extent viewport.
pub struct ListView;
impl ListView {
    #[must_use]
    pub fn builder<W>(item_count: usize, builder: impl Fn(usize) -> W + 'static) -> Widget
    where
        W: Into<Widget> + 'static,
    {
        VirtualList::builder(item_count, builder)
    }
    #[must_use]
    pub fn fixed_extent<W>(
        item_count: usize,
        item_extent: f32,
        builder: impl Fn(usize) -> W + 'static,
    ) -> Widget
    where
        W: Into<Widget> + 'static,
    {
        VirtualList::fixed_extent(item_count, item_extent, builder)
    }
    #[must_use]
    pub fn with_controller<W>(
        item_count: usize,
        item_extent: f32,
        controller: ScrollController,
        builder: impl Fn(usize) -> W + 'static,
    ) -> Widget
    where
        W: Into<Widget> + 'static,
    {
        VirtualList::fixed_extent_with_controller(item_count, item_extent, controller, builder)
    }

    /// Lazy rows measured after layout, backed by the same retained viewport
    /// as `fixed_extent` rather than a second list implementation.
    #[must_use]
    pub fn variable_extent<W>(
        item_count: usize,
        estimated_extent: f32,
        builder: impl Fn(usize) -> W + 'static,
    ) -> Widget
    where
        W: Into<Widget> + 'static,
    {
        VirtualList::variable_extent(item_count, estimated_extent, builder)
    }

    /// Variable rows with a persistent scroll controller.
    #[must_use]
    pub fn variable_extent_with_controller<W>(
        item_count: usize,
        estimated_extent: f32,
        controller: ScrollController,
        builder: impl Fn(usize) -> W + 'static,
    ) -> Widget
    where
        W: Into<Widget> + 'static,
    {
        VirtualList::variable_extent_with_controller(
            item_count,
            estimated_extent,
            controller,
            builder,
        )
    }

    /// Variable rows with an application-owned index for insert/remove/move
    /// invalidation without rebuilding non-visible widgets.
    #[must_use]
    pub fn variable_extent_with_index<W>(
        index: MeasuredExtentIndex,
        controller: ScrollController,
        builder: impl Fn(usize) -> W + 'static,
    ) -> Widget
    where
        W: Into<Widget> + 'static,
    {
        VirtualList::variable_extent_with_index(index, controller, builder)
    }
}

/// A lazy grid expressed as fixed-height rows, avoiding a separate render
/// hierarchy while retaining bounded materialization.
pub struct GridView;
impl GridView {
    #[must_use]
    pub fn builder<W>(
        item_count: usize,
        cross_axis_count: usize,
        row_extent: f32,
        builder: impl Fn(usize) -> W + 'static,
    ) -> Widget
    where
        W: Into<Widget> + 'static,
    {
        Self::with_controller(
            item_count,
            cross_axis_count,
            row_extent,
            ScrollController::new(),
            builder,
        )
    }
    #[must_use]
    pub fn with_controller<W>(
        item_count: usize,
        cross_axis_count: usize,
        row_extent: f32,
        controller: ScrollController,
        builder: impl Fn(usize) -> W + 'static,
    ) -> Widget
    where
        W: Into<Widget> + 'static,
    {
        let columns = cross_axis_count.max(1);
        let rows = item_count.div_ceil(columns);
        let builder = Rc::new(builder);
        VirtualList::fixed_extent_with_controller(rows, row_extent, controller, move |row| {
            let start = row * columns;
            Widget::row(
                (start..(start + columns).min(item_count))
                    .map(|index| builder(index).into())
                    .collect::<Vec<Widget>>(),
            )
        })
    }
}

/// Controller for paged views. Its offset is expressed in logical page
/// extents; the retained viewport clamps it to valid content bounds.
pub type PageController = ScrollController;

/// Paged content using the retained lazy viewport. `page_extent` is the
/// logical extent of one page on the current vertical backend.
pub struct PageView;
impl PageView {
    #[must_use]
    pub fn builder<W>(
        page_count: usize,
        page_extent: f32,
        controller: PageController,
        builder: impl Fn(usize) -> W + 'static,
    ) -> Widget
    where
        W: Into<Widget> + 'static,
    {
        VirtualList::fixed_extent_with_controller(page_count, page_extent, controller, builder)
    }
}

/// Unified sliver protocol. A sliver returns a normal retained widget, making
/// headers, padding, and lazy content composable without a class explosion.
pub trait Sliver {
    fn build(&self, controller: &ScrollController) -> Widget;
}

#[derive(Clone)]
pub struct SliverBox {
    child: Widget,
}
impl SliverBox {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}
impl Sliver for SliverBox {
    fn build(&self, _: &ScrollController) -> Widget {
        self.child.clone()
    }
}

/// A lazy fixed-extent list usable through the unified [`Sliver`] protocol.
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
            item_extent,
            builder: Rc::new(move |index| builder(index).into()),
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

/// A lazy fixed-extent row grid usable through the unified [`Sliver`]
/// protocol.
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
            row_extent,
            builder: Rc::new(move |index| builder(index).into()),
        }
    }
}
impl Sliver for SliverGrid {
    fn build(&self, controller: &ScrollController) -> Widget {
        let columns = self.cross_axis_count;
        let item_count = self.item_count;
        let builder = self.builder.clone();
        VirtualList::fixed_extent_with_controller(
            item_count.div_ceil(columns),
            self.row_extent,
            controller.clone(),
            move |row| {
                let start = row * columns;
                Widget::row(
                    (start..(start + columns).min(item_count))
                        .map(|index| builder(index))
                        .collect::<Vec<_>>(),
                )
            },
        )
    }
}

#[derive(Clone)]
pub struct SliverPadding {
    padding: incular_config::EdgeInsets,
    child: Widget,
}
impl SliverPadding {
    #[must_use]
    pub fn new(padding: incular_config::EdgeInsets, child: impl Into<Widget>) -> Self {
        Self {
            padding,
            child: child.into(),
        }
    }
}
impl Sliver for SliverPadding {
    fn build(&self, _: &ScrollController) -> Widget {
        Widget::padding(self.padding, self.child.clone())
    }
}

/// A fixed-extent flow sliver that remains visible at the leading edge while
/// its [`ScrollController`] moves past it. A later persistent header pushes it
/// away, matching the non-overlapping pinned-header behavior of a
/// `SliverAppBar`/`SliverPersistentHeader` sequence.
#[derive(Clone)]
pub struct SliverPersistentHeader {
    extent: f32,
    child: Widget,
}
impl SliverPersistentHeader {
    #[must_use]
    pub fn new(extent: f32, child: impl Into<Widget>) -> Self {
        assert!(
            extent.is_finite() && extent >= 0.,
            "persistent header extent must be finite and non-negative"
        );
        Self {
            extent,
            child: child.into(),
        }
    }
    #[must_use]
    pub const fn extent(&self) -> f32 {
        self.extent
    }
}
impl Sliver for SliverPersistentHeader {
    fn build(&self, controller: &ScrollController) -> Widget {
        Widget::persistent_header(
            controller.clone(),
            Widget::constrained(
                incular_config::Constraints::new(0., f32::INFINITY, self.extent, self.extent),
                self.child.clone(),
            ),
        )
    }
}

/// A compact, composable SliverAppBar-like descriptor. The framework does not
/// prescribe chrome; callers supply the bar child and choose whether it pins.
#[derive(Clone)]
pub struct SliverAppBar {
    extent: f32,
    pinned: bool,
    child: Widget,
}
impl SliverAppBar {
    #[must_use]
    pub fn new(extent: f32, child: impl Into<Widget>) -> Self {
        assert!(
            extent.is_finite() && extent >= 0.,
            "app bar extent must be finite and non-negative"
        );
        Self {
            extent,
            pinned: true,
            child: child.into(),
        }
    }
    #[must_use]
    pub fn pinned(mut self, pinned: bool) -> Self {
        self.pinned = pinned;
        self
    }
    #[must_use]
    pub const fn is_pinned(&self) -> bool {
        self.pinned
    }
}
impl Sliver for SliverAppBar {
    fn build(&self, controller: &ScrollController) -> Widget {
        let child = Widget::constrained(
            incular_config::Constraints::new(0., f32::INFINITY, self.extent, self.extent),
            self.child.clone(),
        );
        if self.pinned {
            Widget::persistent_header(controller.clone(), child)
        } else {
            child
        }
    }
}

/// A custom scroll view builds all its protocol slivers into one retained
/// scroll child. Lazy list/grid slivers can still be added as their own
/// viewport where appropriate.
pub struct CustomScrollView;
impl CustomScrollView {
    #[must_use]
    pub fn build(
        controller: ScrollController,
        slivers: impl IntoIterator<Item = Box<dyn Sliver>>,
    ) -> Widget {
        let children = slivers
            .into_iter()
            .map(|sliver| sliver.build(&controller))
            .collect::<Vec<_>>();
        ScrollView::vertical(controller, Widget::column(children))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use incular_core::{Color, Size};

    #[test]
    fn grid_and_page_builders_create_retained_widgets() {
        let _: Widget = GridView::builder(5, 2, 40., |_| {
            Widget::fixed_box(Size::new(10., 10.), Color::WHITE)
        });
        let _: Widget = PageView::builder(2, 100., PageController::new(), |_| {
            Widget::fixed_box(Size::new(10., 10.), Color::WHITE)
        });
    }
    #[test]
    fn list_and_grid_are_usable_as_sliver_protocol_entries() {
        let controller = ScrollController::new();
        let list = SliverList::builder(4, 20., |_| {
            Widget::fixed_box(Size::new(10., 10.), Color::WHITE)
        });
        let grid = SliverGrid::builder(4, 2, 20., |_| {
            Widget::fixed_box(Size::new(10., 10.), Color::WHITE)
        });
        let _: Widget = list.build(&controller);
        let _: Widget = grid.build(&controller);
    }

    #[test]
    fn persistent_headers_have_a_fixed_flow_extent() {
        let controller = ScrollController::new();
        let header =
            SliverPersistentHeader::new(32., Widget::fixed_box(Size::new(10., 4.), Color::WHITE));
        assert_eq!(header.extent(), 32.);
        let _: Widget = header.build(&controller);
        assert!(SliverAppBar::new(24., Widget::text("Title")).is_pinned());
        assert!(
            !SliverAppBar::new(24., Widget::text("Title"))
                .pinned(false)
                .is_pinned()
        );
    }
}
