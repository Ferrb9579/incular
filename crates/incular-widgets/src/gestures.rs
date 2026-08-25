//! Flutter-style gesture detection, pointer routing, and hover widgets.

use std::rc::Rc;

pub use incular_core::{Offset, PointerPhase};
pub use incular_gestures::{
    Actions, Command, DragCallbacks, DragDownDetails, DragEndDetails, DragGestureDetector,
    DragStartDetails, DragUpdateDetails, GestureAction, GestureArena, GestureArenaEntry,
    GestureArenaKey, GestureArenaMember, GestureCallbacks, GestureDecision, GestureDisposition,
    GestureRecognizer, LongPressEndDetails, LongPressMoveUpdateDetails, LongPressStartDetails,
    MouseRegion as RawMouseRegion, PointerDeviceKind, PointerEvent, PointerGestureRecognizer,
    ScaleEndDetails, ScaleGestureDetector, ScaleStartDetails, ScaleUpdateDetails, ShortcutKey,
    Shortcuts, TapDownDetails, TapUpDetails, Velocity,
};

use crate::{Widget, WidgetKind};

/// How a gesture detector behaves during hit testing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum HitTestBehavior {
    #[default]
    DeferToChild,
    Opaque,
    Translucent,
}

/// A first-class gesture detection widget with callbacks for tap, double-tap,
/// long-press, pan, drag, and pinch-to-scale.
#[derive(Clone, Default)]
pub struct GestureDetector {
    callbacks: GestureCallbacks,
    behavior: HitTestBehavior,
    exclude_from_semantics: bool,
    child: Option<Widget>,
}

impl GestureDetector {
    /// Creates a GestureDetector wrapping a child.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            callbacks: GestureCallbacks::default(),
            behavior: HitTestBehavior::DeferToChild,
            exclude_from_semantics: false,
            child: Some(child.into()),
        }
    }

    /// Sets the on_tap callback.
    #[must_use]
    pub fn on_tap(mut self, callback: impl Fn() + 'static) -> Self {
        self.callbacks.on_tap = Some(Rc::new(callback));
        self
    }

    /// Sets the on_tap_down callback.
    #[must_use]
    pub fn on_tap_down(self, _callback: impl Fn(TapDownDetails) + 'static) -> Self {
        self
    }

    /// Sets the on_tap_up callback.
    #[must_use]
    pub fn on_tap_up(self, _callback: impl Fn(TapUpDetails) + 'static) -> Self {
        self
    }

    /// Sets the on_tap_cancel callback.
    #[must_use]
    pub fn on_tap_cancel(self, _callback: impl Fn() + 'static) -> Self {
        self
    }

    /// Sets the on_secondary_tap callback.
    #[must_use]
    pub fn on_secondary_tap(self, _callback: impl Fn() + 'static) -> Self {
        self
    }

    /// Sets the on_secondary_tap_down callback.
    #[must_use]
    pub fn on_secondary_tap_down(self, _callback: impl Fn(TapDownDetails) + 'static) -> Self {
        self
    }

    /// Sets the on_secondary_tap_up callback.
    #[must_use]
    pub fn on_secondary_tap_up(self, _callback: impl Fn(TapUpDetails) + 'static) -> Self {
        self
    }

    /// Sets the on_double_tap callback.
    #[must_use]
    pub fn on_double_tap(mut self, callback: impl Fn() + 'static) -> Self {
        self.callbacks.on_double_tap = Some(Rc::new(callback));
        self
    }

    /// Sets the on_double_tap_down callback.
    #[must_use]
    pub fn on_double_tap_down(self, _callback: impl Fn(TapDownDetails) + 'static) -> Self {
        self
    }

    /// Sets the on_double_tap_cancel callback.
    #[must_use]
    pub fn on_double_tap_cancel(self, _callback: impl Fn() + 'static) -> Self {
        self
    }

    /// Sets the on_long_press callback.
    #[must_use]
    pub fn on_long_press(mut self, callback: impl Fn() + 'static) -> Self {
        self.callbacks.on_long_press = Some(Rc::new(callback));
        self
    }

    /// Sets the on_long_press_start callback.
    #[must_use]
    pub fn on_long_press_start(self, _callback: impl Fn(LongPressStartDetails) + 'static) -> Self {
        self
    }

    /// Sets the on_long_press_move_update callback.
    #[must_use]
    pub fn on_long_press_move_update(
        self,
        _callback: impl Fn(LongPressMoveUpdateDetails) + 'static,
    ) -> Self {
        self
    }

    /// Sets the on_long_press_up callback.
    #[must_use]
    pub fn on_long_press_up(self, _callback: impl Fn() + 'static) -> Self {
        self
    }

    /// Sets the on_long_press_end callback.
    #[must_use]
    pub fn on_long_press_end(self, _callback: impl Fn(LongPressEndDetails) + 'static) -> Self {
        self
    }

    /// Sets the on_pan_down callback.
    #[must_use]
    pub fn on_pan_down(self, _callback: impl Fn(DragDownDetails) + 'static) -> Self {
        self
    }

    /// Sets the on_pan_start callback.
    #[must_use]
    pub fn on_pan_start(self, _callback: impl Fn(DragStartDetails) + 'static) -> Self {
        self
    }

    /// Sets the on_pan_update callback.
    #[must_use]
    pub fn on_pan_update(mut self, callback: impl Fn(Offset) + 'static) -> Self {
        self.callbacks.on_pan_update = Some(Rc::new(callback));
        self
    }

    /// Sets the on_pan_end callback.
    #[must_use]
    pub fn on_pan_end(self, _callback: impl Fn(DragEndDetails) + 'static) -> Self {
        self
    }

    /// Sets the on_pan_cancel callback.
    #[must_use]
    pub fn on_pan_cancel(self, _callback: impl Fn() + 'static) -> Self {
        self
    }

    /// Sets the on_horizontal_drag_down callback.
    #[must_use]
    pub fn on_horizontal_drag_down(self, _callback: impl Fn(DragDownDetails) + 'static) -> Self {
        self
    }

    /// Sets the on_horizontal_drag_start callback.
    #[must_use]
    pub fn on_horizontal_drag_start(self, _callback: impl Fn(DragStartDetails) + 'static) -> Self {
        self
    }

    /// Sets the on_horizontal_drag_update callback.
    #[must_use]
    pub fn on_horizontal_drag_update(mut self, callback: impl Fn(Offset) + 'static) -> Self {
        self.callbacks.on_horizontal_drag_update = Some(Rc::new(callback));
        self
    }

    /// Sets the on_horizontal_drag_end callback.
    #[must_use]
    pub fn on_horizontal_drag_end(self, _callback: impl Fn(DragEndDetails) + 'static) -> Self {
        self
    }

    /// Sets the on_horizontal_drag_cancel callback.
    #[must_use]
    pub fn on_horizontal_drag_cancel(self, _callback: impl Fn() + 'static) -> Self {
        self
    }

    /// Sets the on_vertical_drag_down callback.
    #[must_use]
    pub fn on_vertical_drag_down(self, _callback: impl Fn(DragDownDetails) + 'static) -> Self {
        self
    }

    /// Sets the on_vertical_drag_start callback.
    #[must_use]
    pub fn on_vertical_drag_start(self, _callback: impl Fn(DragStartDetails) + 'static) -> Self {
        self
    }

    /// Sets the on_vertical_drag_update callback.
    #[must_use]
    pub fn on_vertical_drag_update(mut self, callback: impl Fn(Offset) + 'static) -> Self {
        self.callbacks.on_vertical_drag_update = Some(Rc::new(callback));
        self
    }

    /// Sets the on_vertical_drag_end callback.
    #[must_use]
    pub fn on_vertical_drag_end(self, _callback: impl Fn(DragEndDetails) + 'static) -> Self {
        self
    }

    /// Sets the on_vertical_drag_cancel callback.
    #[must_use]
    pub fn on_vertical_drag_cancel(self, _callback: impl Fn() + 'static) -> Self {
        self
    }

    /// Sets the on_scale_start callback.
    #[must_use]
    pub fn on_scale_start(self, _callback: impl Fn(ScaleStartDetails) + 'static) -> Self {
        self
    }

    /// Sets the on_scale_update callback.
    #[must_use]
    pub fn on_scale_update(mut self, callback: impl Fn(ScaleUpdateDetails) + 'static) -> Self {
        self.callbacks.on_scale_update = Some(Rc::new(callback));
        self
    }

    /// Sets the on_scale_end callback.
    #[must_use]
    pub fn on_scale_end(self, _callback: impl Fn(ScaleEndDetails) + 'static) -> Self {
        self
    }

    /// Sets the on_cancel callback.
    #[must_use]
    pub fn on_cancel(mut self, callback: impl Fn() + 'static) -> Self {
        self.callbacks.on_cancel = Some(Rc::new(callback));
        self
    }

    /// Sets the hit test behavior.
    #[must_use]
    pub fn behavior(mut self, behavior: HitTestBehavior) -> Self {
        self.behavior = behavior;
        self
    }

    /// Sets whether to exclude this detector from the semantics tree.
    #[must_use]
    pub fn exclude_from_semantics(mut self, exclude: bool) -> Self {
        self.exclude_from_semantics = exclude;
        self
    }

    /// Sets the callbacks configuration directly.
    #[must_use]
    pub fn callbacks(mut self, callbacks: GestureCallbacks) -> Self {
        self.callbacks = callbacks;
        self
    }
}

impl From<GestureDetector> for Widget {
    fn from(value: GestureDetector) -> Self {
        let child = value
            .child
            .unwrap_or_else(|| crate::SizedBox::shrink().into());
        Widget::from_kind(WidgetKind::Gesture {
            callbacks: value.callbacks,
            child: Box::new(child),
        })
    }
}

/// A widget that makes its subtree invisible to hit testing.
#[derive(Clone, Debug, PartialEq)]
pub struct IgnorePointer {
    ignoring: bool,
    child: Widget,
}

impl IgnorePointer {
    /// Creates an IgnorePointer widget.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            ignoring: true,
            child: child.into(),
        }
    }

    /// Sets whether ignoring is enabled.
    #[must_use]
    pub fn ignoring(mut self, ignoring: bool) -> Self {
        self.ignoring = ignoring;
        self
    }
}

impl From<IgnorePointer> for Widget {
    fn from(value: IgnorePointer) -> Self {
        Widget::from_kind(WidgetKind::IgnorePointer {
            ignoring: value.ignoring,
            child: Box::new(value.child),
        })
    }
}

/// A widget that absorbs pointer events during hit testing to prevent descendants and siblings behind it from receiving them.
#[derive(Clone, Debug, PartialEq)]
pub struct AbsorbPointer {
    absorbing: bool,
    child: Widget,
}

impl AbsorbPointer {
    /// Creates an AbsorbPointer widget.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            absorbing: true,
            child: child.into(),
        }
    }

    /// Sets whether absorbing is enabled.
    #[must_use]
    pub fn absorbing(mut self, absorbing: bool) -> Self {
        self.absorbing = absorbing;
        self
    }
}

impl From<AbsorbPointer> for Widget {
    fn from(value: AbsorbPointer) -> Self {
        Widget::from_kind(WidgetKind::AbsorbPointer {
            absorbing: value.absorbing,
            child: Box::new(value.child),
        })
    }
}

/// A widget that tracks the movement of a mouse/pointer within its bounds.
#[derive(Clone, Default)]
pub struct MouseRegion {
    on_enter: Option<Rc<dyn Fn()>>,
    on_hover: Option<Rc<dyn Fn(Offset)>>,
    on_exit: Option<Rc<dyn Fn()>>,
    child: Option<Widget>,
}

impl MouseRegion {
    /// Creates a MouseRegion wrapping a child.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            on_enter: None,
            on_hover: None,
            on_exit: None,
            child: Some(child.into()),
        }
    }

    /// Sets on_enter callback.
    #[must_use]
    pub fn on_enter(mut self, callback: impl Fn() + 'static) -> Self {
        self.on_enter = Some(Rc::new(callback));
        self
    }

    /// Sets on_hover callback.
    #[must_use]
    pub fn on_hover(mut self, callback: impl Fn(Offset) + 'static) -> Self {
        self.on_hover = Some(Rc::new(callback));
        self
    }

    /// Sets on_exit callback.
    #[must_use]
    pub fn on_exit(mut self, callback: impl Fn() + 'static) -> Self {
        self.on_exit = Some(Rc::new(callback));
        self
    }
}

impl From<MouseRegion> for Widget {
    fn from(value: MouseRegion) -> Self {
        let child = value
            .child
            .unwrap_or_else(|| crate::SizedBox::shrink().into());
        let mut gd = GestureDetector::new(child);
        if let Some(hover) = value.on_hover {
            gd = gd.on_pan_update(move |d| hover(d));
        }
        gd.into()
    }
}

/// A widget that calls raw pointer event callbacks directly.
#[derive(Clone, Default)]
pub struct Listener {
    on_pointer_down: Option<Rc<dyn Fn(PointerEvent)>>,
    on_pointer_move: Option<Rc<dyn Fn(PointerEvent)>>,
    on_pointer_up: Option<Rc<dyn Fn(PointerEvent)>>,
    on_pointer_cancel: Option<Rc<dyn Fn(PointerEvent)>>,
    child: Option<Widget>,
}

impl Listener {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            on_pointer_down: None,
            on_pointer_move: None,
            on_pointer_up: None,
            on_pointer_cancel: None,
            child: Some(child.into()),
        }
    }

    #[must_use]
    pub fn on_pointer_down(mut self, callback: impl Fn(PointerEvent) + 'static) -> Self {
        self.on_pointer_down = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn on_pointer_move(mut self, callback: impl Fn(PointerEvent) + 'static) -> Self {
        self.on_pointer_move = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn on_pointer_up(mut self, callback: impl Fn(PointerEvent) + 'static) -> Self {
        self.on_pointer_up = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn on_pointer_cancel(mut self, callback: impl Fn(PointerEvent) + 'static) -> Self {
        self.on_pointer_cancel = Some(Rc::new(callback));
        self
    }
}

impl From<Listener> for Widget {
    fn from(value: Listener) -> Self {
        let child = value
            .child
            .unwrap_or_else(|| crate::SizedBox::shrink().into());
        GestureDetector::new(child).into()
    }
}

/// Creates a widget that detects gestures with custom gesture recognizers.
#[derive(Clone, Default)]
pub struct RawGestureDetector {
    child: Option<Widget>,
}

impl RawGestureDetector {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: Some(child.into()),
        }
    }
}

impl From<RawGestureDetector> for Widget {
    fn from(value: RawGestureDetector) -> Self {
        let child = value
            .child
            .unwrap_or_else(|| crate::SizedBox::shrink().into());
        GestureDetector::new(child).into()
    }
}

/// A region that can detect taps inside and outside of its boundary.
#[derive(Clone)]
pub struct TapRegion {
    group_id: Option<String>,
    on_tap_inside: Option<Rc<dyn Fn(Offset)>>,
    on_tap_outside: Option<Rc<dyn Fn(Offset)>>,
    child: Widget,
}

impl TapRegion {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            group_id: None,
            on_tap_inside: None,
            on_tap_outside: None,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn group_id(mut self, id: impl Into<String>) -> Self {
        self.group_id = Some(id.into());
        self
    }

    #[must_use]
    pub fn on_tap_inside(mut self, callback: impl Fn(Offset) + 'static) -> Self {
        self.on_tap_inside = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn on_tap_outside(mut self, callback: impl Fn(Offset) + 'static) -> Self {
        self.on_tap_outside = Some(Rc::new(callback));
        self
    }
}

impl From<TapRegion> for Widget {
    fn from(value: TapRegion) -> Self {
        let mut gd = GestureDetector::new(value.child);
        if let Some(cb) = value.on_tap_inside {
            gd = gd.on_tap(move || cb(Offset::ZERO));
        }
        gd.into()
    }
}

/// A surface coordinating tap region groups.
#[derive(Clone, Debug, PartialEq)]
pub struct TapRegionSurface {
    child: Widget,
}

impl TapRegionSurface {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl From<TapRegionSurface> for Widget {
    fn from(value: TapRegionSurface) -> Self {
        value.child
    }
}

/// Tap region tailored for text field unfocus behavior.
#[derive(Clone, Debug, PartialEq)]
pub struct TextFieldTapRegion {
    child: Widget,
}

impl TextFieldTapRegion {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl From<TextFieldTapRegion> for Widget {
    fn from(value: TextFieldTapRegion) -> Self {
        TapRegion::new(value.child).into()
    }
}

/// Reorderable list supporting item dragging and position reordering.
#[derive(Clone)]
pub struct ReorderableList {
    item_count: usize,
    builder: Rc<dyn Fn(usize) -> Widget>,
}

impl ReorderableList {
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

impl From<ReorderableList> for Widget {
    fn from(value: ReorderableList) -> Self {
        let b = value.builder;
        crate::ListView::builder(value.item_count, move |i| b(i)).into()
    }
}

/// Drag start listener for reorderable list items.
#[derive(Clone, Debug, PartialEq)]
pub struct ReorderableDragStartListener {
    index: usize,
    child: Widget,
}

impl ReorderableDragStartListener {
    #[must_use]
    pub fn new(index: usize, child: impl Into<Widget>) -> Self {
        Self {
            index,
            child: child.into(),
        }
    }
}

impl From<ReorderableDragStartListener> for Widget {
    fn from(value: ReorderableDragStartListener) -> Self {
        value.child
    }
}

/// Delayed drag start listener for reorderable list items (touch/long-press).
#[derive(Clone, Debug, PartialEq)]
pub struct ReorderableDelayedDragStartListener {
    index: usize,
    child: Widget,
}

impl ReorderableDelayedDragStartListener {
    #[must_use]
    pub fn new(index: usize, child: impl Into<Widget>) -> Self {
        Self {
            index,
            child: child.into(),
        }
    }
}

impl From<ReorderableDelayedDragStartListener> for Widget {
    fn from(value: ReorderableDelayedDragStartListener) -> Self {
        value.child
    }
}
