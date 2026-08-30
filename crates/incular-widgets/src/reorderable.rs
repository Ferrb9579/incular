//! Box-level reorderable widgets and the indexed drag-start wrappers.
//!
//! The retained sliver owns the reorder operation.  These descriptors only
//! add the box scroll-view configuration and leave a small marker around a
//! listener child; the sliver consumes that marker while it materializes the
//! row, which keeps the listener useful even when it wraps only a drag handle.

use std::{cell::RefCell, rc::Rc};

use incular_config::{Axis, Clip, EdgeInsets, WidgetDefaults};
use incular_scroll::{ScrollController, ScrollPhysics};

use super::Sliver;
use crate::{Listener, SliverReorderController, SliverReorderableList, Widget};
use crate::{tree::Key, tree::WidgetKind};

const LISTENER_MARKER_PREFIX: &str = "\u{1f}incular-reorderable-listener:";

/// Internal description left by a reorderable drag-start listener until its
/// containing reorderable sliver materializes the row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ReorderableListenerSpec {
    pub(crate) index: usize,
    pub(crate) enabled: bool,
    pub(crate) delayed: bool,
}

fn marker_key(spec: ReorderableListenerSpec) -> Key {
    Key::String(format!(
        "{LISTENER_MARKER_PREFIX}{}:{}:{}",
        if spec.delayed { 'd' } else { 'i' },
        if spec.enabled { 'e' } else { 'x' },
        spec.index
    ))
}

fn marker_spec(widget: &Widget) -> Option<ReorderableListenerSpec> {
    let Key::String(value) = widget.key.as_ref()? else {
        return None;
    };
    let value = value.strip_prefix(LISTENER_MARKER_PREFIX)?;
    let mut fields = value.split(':');
    let delayed = match fields.next()? {
        "d" => true,
        "i" => false,
        _ => return None,
    };
    let enabled = match fields.next()? {
        "e" => true,
        "x" => false,
        _ => return None,
    };
    let index = fields.next()?.parse().ok()?;
    fields.next().is_none().then_some(ReorderableListenerSpec {
        index,
        enabled,
        delayed,
    })
}

fn empty_widget() -> Widget {
    Widget::box_(
        incular_core::Size::ZERO,
        incular_core::Color::rgba(0, 0, 0, 0),
    )
}

/// Removes one listener marker and replaces it with the widget made by
/// `wrap`.  Traversal covers every retained single-child and collection
/// widget, so a listener can be placed on an entire item or on an internal
/// drag handle without changing the visual tree.
pub(crate) fn install_listener(
    widget: &mut Widget,
    wrap: &mut impl FnMut(Widget, ReorderableListenerSpec) -> Widget,
) -> Option<ReorderableListenerSpec> {
    if let Some(spec) = marker_spec(widget) {
        let mut marked = std::mem::replace(widget, empty_widget());
        let kind = std::mem::replace(&mut marked.kind, empty_widget().kind);
        let child = match kind {
            WidgetKind::RawInput {
                child: Some(child), ..
            } => *child,
            other => {
                marked.kind = other;
                marked.key = None;
                marked
            }
        };
        *widget = if spec.enabled {
            wrap(child, spec)
        } else {
            child
        };
        return Some(spec);
    }

    match &mut widget.kind {
        WidgetKind::Banner { child, .. } | WidgetKind::Button { child, .. } => child
            .as_deref_mut()
            .and_then(|child| install_listener(child, wrap)),
        WidgetKind::RawInput { child, .. } => child
            .as_deref_mut()
            .and_then(|child| install_listener(child, wrap)),
        WidgetKind::SelectionArea { child, .. }
        | WidgetKind::SelectionContainer { child, .. }
        | WidgetKind::SelectionListener { child, .. }
        | WidgetKind::IndexedSemantics { child, .. }
        | WidgetKind::SemanticsDebugger { child, .. }
        | WidgetKind::Decorated { child, .. }
        | WidgetKind::Padding { child, .. }
        | WidgetKind::Constrained { child, .. }
        | WidgetKind::Limited { child, .. }
        | WidgetKind::Overflow { child, .. }
        | WidgetKind::Unconstrained { child, .. }
        | WidgetKind::Fractional { child, .. }
        | WidgetKind::Baseline { child, .. }
        | WidgetKind::RepaintBoundary { child, .. }
        | WidgetKind::Gesture { child, .. }
        | WidgetKind::Draggable { child, .. }
        | WidgetKind::DragTarget { child, .. }
        | WidgetKind::IgnorePointer { child, .. }
        | WidgetKind::AbsorbPointer { child, .. }
        | WidgetKind::Align { child, .. }
        | WidgetKind::Flexible { child, .. }
        | WidgetKind::Positioned { child, .. }
        | WidgetKind::SafeArea { child, .. }
        | WidgetKind::ClipRect { child, .. }
        | WidgetKind::ClipRRect { child, .. }
        | WidgetKind::ClipOval { child, .. }
        | WidgetKind::ClipPath { child, .. }
        | WidgetKind::Visibility { child, .. }
        | WidgetKind::AspectRatio { child, .. }
        | WidgetKind::Scroll { child, .. }
        | WidgetKind::RawScrollbar { child, .. }
        | WidgetKind::DraggableScrollableActuator { child, .. }
        | WidgetKind::PersistentHeader { child, .. }
        | WidgetKind::NotificationListener { child, .. }
        | WidgetKind::Translate { child, .. }
        | WidgetKind::Transform { child, .. }
        | WidgetKind::Scale { child, .. }
        | WidgetKind::Rotation { child, .. }
        | WidgetKind::FittedBox { child, .. }
        | WidgetKind::Opacity { child, .. }
        | WidgetKind::Blur { child, .. }
        | WidgetKind::DropShadow { child, .. }
        | WidgetKind::ColorFiltered { child, .. }
        | WidgetKind::Blend { child, .. }
        | WidgetKind::ShaderMask { child, .. }
        | WidgetKind::BackdropFilter { child, .. }
        | WidgetKind::AnnotatedRegion { child, .. }
        | WidgetKind::CompositedTransformTarget { child, .. }
        | WidgetKind::CompositedTransformFollower { child, .. } => install_listener(child, wrap),
        WidgetKind::Flex { children, .. }
        | WidgetKind::Wrap { children, .. }
        | WidgetKind::Table { children, .. }
        | WidgetKind::Stack { children, .. }
        | WidgetKind::IndexedStack { children, .. } => children
            .iter_mut()
            .find_map(|child| install_listener(child, wrap)),
        WidgetKind::Box { .. }
        | WidgetKind::Shape { .. }
        | WidgetKind::CustomPaint { .. }
        | WidgetKind::Text { .. }
        | WidgetKind::SelectableText { .. }
        | WidgetKind::Image { .. }
        | WidgetKind::TextField { .. }
        | WidgetKind::ListWheelScrollView { .. }
        | WidgetKind::ListWheelViewport { .. }
        | WidgetKind::DraggableScrollableSheet { .. }
        | WidgetKind::TwoDimensionalScrollView { .. }
        | WidgetKind::TwoDimensionalViewport { .. }
        | WidgetKind::LayoutBuilder { .. }
        | WidgetKind::SliverViewport { .. } => None,
    }
}

fn listener_widget(child: Widget, spec: ReorderableListenerSpec) -> Widget {
    let mut listener: Widget = Listener::new(child).into();
    listener.key = Some(marker_key(spec));
    listener
}

/// A wrapper that starts a reorder drag as soon as its pointer sequence
/// becomes an accepted drag.  In a reorderable list this is normally used for
/// a small drag handle.
#[derive(Clone)]
pub struct ReorderableDragStartListener {
    child: Widget,
    index: usize,
    enabled: bool,
}

impl ReorderableDragStartListener {
    /// Creates an enabled immediate drag-start listener for `index`.
    #[must_use]
    pub fn new(index: usize, child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            index,
            enabled: true,
        }
    }

    /// Alternative child-first spelling for callers mirroring Flutter's
    /// named `child`/`index` constructor arguments.
    #[must_use]
    pub fn with_child(child: impl Into<Widget>, index: usize) -> Self {
        Self::new(index, child)
    }

    /// Enables or disables this listener without removing its child.
    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }

    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl From<ReorderableDragStartListener> for Widget {
    fn from(value: ReorderableDragStartListener) -> Self {
        listener_widget(
            value.child,
            ReorderableListenerSpec {
                index: value.index,
                enabled: value.enabled,
                delayed: false,
            },
        )
    }
}

/// A wrapper that starts a reorder drag only after a long press has been
/// recognized.  It is normally used around an entire reorderable item on
/// touch-oriented platforms.
#[derive(Clone)]
pub struct ReorderableDelayedDragStartListener {
    child: Widget,
    index: usize,
    enabled: bool,
}

impl ReorderableDelayedDragStartListener {
    /// Creates an enabled delayed drag-start listener for `index`.
    #[must_use]
    pub fn new(index: usize, child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            index,
            enabled: true,
        }
    }

    /// Alternative child-first spelling for callers mirroring Flutter's
    /// named `child`/`index` constructor arguments.
    #[must_use]
    pub fn with_child(child: impl Into<Widget>, index: usize) -> Self {
        Self::new(index, child)
    }

    /// Enables or disables this listener without removing its child.
    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }

    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl From<ReorderableDelayedDragStartListener> for Widget {
    fn from(value: ReorderableDelayedDragStartListener) -> Self {
        listener_widget(
            value.child,
            ReorderableListenerSpec {
                index: value.index,
                enabled: value.enabled,
                delayed: true,
            },
        )
    }
}

/// A box scrolling container that allows its indexed children to be
/// interactively reordered.
#[derive(Clone)]
pub struct ReorderableList {
    item_count: usize,
    item_builder: Rc<dyn Fn(usize) -> Widget>,
    order_controller: SliverReorderController,
    on_reorder: Option<Rc<dyn Fn(usize, usize)>>,
    on_reorder_item: Option<Rc<dyn Fn(usize, usize)>>,
    on_reorder_start: Option<Rc<dyn Fn(usize)>>,
    on_reorder_end: Option<Rc<dyn Fn(usize)>>,
    item_extent: Option<f32>,
    item_extent_builder: Option<Rc<dyn Fn(usize) -> f32>>,
    prototype_item: Option<Widget>,
    scroll_direction: Axis,
    reverse: bool,
    shrink_wrap: bool,
    controller: Option<ScrollController>,
    padding: Option<EdgeInsets>,
    cache_extent: f32,
    physics: Option<ScrollPhysics>,
    clip_behavior: Clip,
}

impl ReorderableList {
    /// Creates a lazy reorderable list with `item_count` logical children.
    #[must_use]
    pub fn new<W>(item_count: usize, builder: impl Fn(usize) -> W + 'static) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self {
            item_count,
            item_builder: Rc::new(move |index| builder(index).into()),
            order_controller: SliverReorderController::new(item_count),
            on_reorder: None,
            on_reorder_item: None,
            on_reorder_start: None,
            on_reorder_end: None,
            item_extent: None,
            item_extent_builder: None,
            prototype_item: None,
            scroll_direction: WidgetDefaults::DEFAULT.scroll_direction,
            reverse: false,
            shrink_wrap: false,
            controller: None,
            padding: None,
            cache_extent: WidgetDefaults::DEFAULT.sliver_cache_extent,
            physics: None,
            clip_behavior: WidgetDefaults::DEFAULT.scroll_clip_behavior,
        }
    }

    /// Builder-named alias matching Flutter's `ReorderableList.builder`
    /// spelling while retaining the crate's ordinary Rust constructor.
    #[must_use]
    pub fn builder<W>(item_count: usize, builder: impl Fn(usize) -> W + 'static) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self::new(item_count, builder)
    }

    #[must_use]
    pub const fn item_count(&self) -> usize {
        self.item_count
    }

    /// Supplies the retained logical-order controller used by the list.
    #[must_use]
    pub fn with_reorder_controller(mut self, controller: SliverReorderController) -> Self {
        self.order_controller = controller;
        self
    }

    #[must_use]
    pub fn reorder_controller(&self) -> SliverReorderController {
        self.order_controller.clone()
    }

    /// Receives Flutter's legacy `(old_index, insertion_index)` callback.  An
    /// insertion index after the old item is intentionally one larger than
    /// the final post-removal slot.
    #[must_use]
    pub fn on_reorder(mut self, callback: impl Fn(usize, usize) + 'static) -> Self {
        self.on_reorder = Some(Rc::new(callback));
        self.on_reorder_item = None;
        self
    }

    /// Receives `(old_index, final_index)` after the old item is removed.
    #[must_use]
    pub fn on_reorder_item(mut self, callback: impl Fn(usize, usize) + 'static) -> Self {
        self.on_reorder_item = Some(Rc::new(callback));
        self.on_reorder = None;
        self
    }

    #[must_use]
    pub fn on_reorder_start(mut self, callback: impl Fn(usize) + 'static) -> Self {
        self.on_reorder_start = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn on_reorder_end(mut self, callback: impl Fn(usize) + 'static) -> Self {
        self.on_reorder_end = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn item_extent(mut self, extent: f32) -> Self {
        self.item_extent = extent.is_finite().then_some(extent.max(1.));
        self.item_extent_builder = None;
        self.prototype_item = None;
        self
    }

    #[must_use]
    pub fn item_extent_builder(mut self, builder: impl Fn(usize) -> f32 + 'static) -> Self {
        self.item_extent_builder = Some(Rc::new(builder));
        self.item_extent = None;
        self.prototype_item = None;
        self
    }

    #[must_use]
    pub fn prototype_item(mut self, prototype: impl Into<Widget>) -> Self {
        self.prototype_item = Some(prototype.into());
        self.item_extent = None;
        self.item_extent_builder = None;
        self
    }

    #[must_use]
    pub fn scroll_direction(mut self, direction: Axis) -> Self {
        self.scroll_direction = direction;
        self
    }

    #[must_use]
    pub fn reverse(mut self, reverse: bool) -> Self {
        self.reverse = reverse;
        self
    }

    #[must_use]
    pub fn shrink_wrap(mut self, shrink_wrap: bool) -> Self {
        self.shrink_wrap = shrink_wrap;
        self
    }

    #[must_use]
    pub fn controller(mut self, controller: ScrollController) -> Self {
        self.controller = Some(controller);
        self
    }

    #[must_use]
    pub fn padding(mut self, padding: EdgeInsets) -> Self {
        self.padding = Some(padding);
        self
    }

    #[must_use]
    pub fn cache_extent(mut self, extent: f32) -> Self {
        self.cache_extent = if extent.is_finite() {
            extent.max(0.)
        } else {
            0.
        };
        self
    }

    #[must_use]
    pub fn physics(mut self, physics: ScrollPhysics) -> Self {
        self.physics = Some(physics);
        self
    }

    #[must_use]
    pub fn clip_behavior(mut self, clip_behavior: Clip) -> Self {
        self.clip_behavior = clip_behavior;
        self
    }
}

impl From<ReorderableList> for Widget {
    fn from(value: ReorderableList) -> Self {
        let controller = value.controller.unwrap_or_default();
        let order_controller = value.order_controller;
        order_controller.set_item_count(value.item_count);

        let builder = value.item_builder;
        let item_controller = order_controller.clone();
        let mut sliver = SliverReorderableList::new(value.item_count, move |item| {
            let index = item_controller.position_of(item).unwrap_or(item);
            builder(index)
        })
        .controller(order_controller)
        .require_drag_listener(true);
        if let Some(item_extent) = value.item_extent {
            sliver = sliver.item_extent(item_extent);
        } else if let Some(item_extent_builder) = value.item_extent_builder {
            sliver = sliver.item_extent_builder(move |index| item_extent_builder(index));
        } else if let Some(prototype_item) = value.prototype_item {
            sliver = sliver.prototype_item(prototype_item);
        }
        if let Some(callback) = value.on_reorder {
            sliver = sliver.on_reorder(move |old, new| callback(old, new));
        }
        if let Some(callback) = value.on_reorder_item {
            sliver = sliver.on_reorder_item(move |old, new| callback(old, new));
        }
        if let Some(callback) = value.on_reorder_start {
            sliver = sliver.on_reorder_start(move |index| callback(index));
        }
        if let Some(callback) = value.on_reorder_end {
            sliver = sliver.on_reorder_end(move |index| callback(index));
        }

        let mut render_sliver =
            sliver.create_render_sliver(&controller, value.scroll_direction, value.reverse);
        if let Some(padding) = value.padding {
            render_sliver = Box::new(super::PaddingRenderSliver {
                inner: RefCell::new(render_sliver),
                padding,
            });
        }
        super::single_sliver_viewport_with_options(
            super::SliverViewportOptions {
                controller,
                axis: value.scroll_direction,
                reverse: value.reverse,
                physics: value.physics.unwrap_or_default(),
                cache_extent: value.cache_extent,
                clip_behavior: value.clip_behavior,
                shrink_wrap: value.shrink_wrap,
            },
            render_sliver,
        )
    }
}
