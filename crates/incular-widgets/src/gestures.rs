//! Flutter-style gesture detection, pointer routing, and hover widgets.

use std::rc::Rc;

pub use incular_core::{Offset, PointerPhase};
pub use incular_gestures::{
    Actions, Command, DragCallbacks, DragDownDetails, DragEndDetails, DragGestureDetector,
    DragStartDetails, DragUpdateDetails, GestureAction, GestureArena, GestureArenaEntry,
    GestureArenaKey, GestureArenaMember, GestureCallbacks, GestureDecision, GestureDisposition,
    GestureRecognizer, LongPressEndDetails, LongPressMoveUpdateDetails, LongPressStartDetails,
    PointerDeviceKind, PointerEvent, PointerGestureRecognizer, ScaleEndDetails,
    ScaleGestureDetector, ScaleStartDetails, ScaleUpdateDetails, ShortcutKey, Shortcuts,
    TapDownDetails, TapUpDetails, Velocity,
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
    child: Option<Widget>,
}

impl GestureDetector {
    /// Creates a GestureDetector wrapping a child.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            callbacks: GestureCallbacks::default(),
            behavior: HitTestBehavior::DeferToChild,
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
    pub fn on_tap_down(mut self, callback: impl Fn(TapDownDetails) + 'static) -> Self {
        self.callbacks.on_tap_down = Some(Rc::new(callback));
        self
    }

    /// Sets the on_tap_up callback.
    #[must_use]
    pub fn on_tap_up(mut self, callback: impl Fn(TapUpDetails) + 'static) -> Self {
        self.callbacks.on_tap_up = Some(Rc::new(callback));
        self
    }

    /// Sets the on_tap_cancel callback.
    #[must_use]
    pub fn on_tap_cancel(mut self, callback: impl Fn() + 'static) -> Self {
        self.callbacks.on_tap_cancel = Some(Rc::new(callback));
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
    pub fn on_double_tap_down(mut self, callback: impl Fn(TapDownDetails) + 'static) -> Self {
        self.callbacks.on_double_tap_down = Some(Rc::new(callback));
        self
    }

    /// Sets the on_double_tap_cancel callback.
    #[must_use]
    pub fn on_double_tap_cancel(mut self, callback: impl Fn() + 'static) -> Self {
        self.callbacks.on_double_tap_cancel = Some(Rc::new(callback));
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
    pub fn on_long_press_start(
        mut self,
        callback: impl Fn(LongPressStartDetails) + 'static,
    ) -> Self {
        self.callbacks.on_long_press_start = Some(Rc::new(callback));
        self
    }

    /// Sets the on_long_press_move_update callback.
    #[must_use]
    pub fn on_long_press_move_update(
        mut self,
        callback: impl Fn(LongPressMoveUpdateDetails) + 'static,
    ) -> Self {
        self.callbacks.on_long_press_move_update = Some(Rc::new(callback));
        self
    }

    /// Sets the on_long_press_up callback.
    #[must_use]
    pub fn on_long_press_up(mut self, callback: impl Fn() + 'static) -> Self {
        self.callbacks.on_long_press_up = Some(Rc::new(callback));
        self
    }

    /// Sets the on_long_press_end callback.
    #[must_use]
    pub fn on_long_press_end(mut self, callback: impl Fn(LongPressEndDetails) + 'static) -> Self {
        self.callbacks.on_long_press_end = Some(Rc::new(callback));
        self
    }

    /// Sets the on_pan_down callback.
    #[must_use]
    pub fn on_pan_down(mut self, callback: impl Fn(DragDownDetails) + 'static) -> Self {
        self.callbacks.on_pan_down = Some(Rc::new(callback));
        self
    }

    /// Sets the on_pan_start callback.
    #[must_use]
    pub fn on_pan_start(mut self, callback: impl Fn(DragStartDetails) + 'static) -> Self {
        self.callbacks.on_pan_start = Some(Rc::new(callback));
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
    pub fn on_pan_end(mut self, callback: impl Fn(DragEndDetails) + 'static) -> Self {
        self.callbacks.on_pan_end = Some(Rc::new(callback));
        self
    }

    /// Sets the on_pan_cancel callback.
    #[must_use]
    pub fn on_pan_cancel(mut self, callback: impl Fn() + 'static) -> Self {
        self.callbacks.on_pan_cancel = Some(Rc::new(callback));
        self
    }

    /// Sets the on_horizontal_drag_down callback.
    #[must_use]
    pub fn on_horizontal_drag_down(mut self, callback: impl Fn(DragDownDetails) + 'static) -> Self {
        self.callbacks.on_horizontal_drag_down = Some(Rc::new(callback));
        self
    }

    /// Sets the on_horizontal_drag_start callback.
    #[must_use]
    pub fn on_horizontal_drag_start(
        mut self,
        callback: impl Fn(DragStartDetails) + 'static,
    ) -> Self {
        self.callbacks.on_horizontal_drag_start = Some(Rc::new(callback));
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
    pub fn on_horizontal_drag_end(mut self, callback: impl Fn(DragEndDetails) + 'static) -> Self {
        self.callbacks.on_horizontal_drag_end = Some(Rc::new(callback));
        self
    }

    /// Sets the on_horizontal_drag_cancel callback.
    #[must_use]
    pub fn on_horizontal_drag_cancel(mut self, callback: impl Fn() + 'static) -> Self {
        self.callbacks.on_horizontal_drag_cancel = Some(Rc::new(callback));
        self
    }

    /// Sets the on_vertical_drag_down callback.
    #[must_use]
    pub fn on_vertical_drag_down(mut self, callback: impl Fn(DragDownDetails) + 'static) -> Self {
        self.callbacks.on_vertical_drag_down = Some(Rc::new(callback));
        self
    }

    /// Sets the on_vertical_drag_start callback.
    #[must_use]
    pub fn on_vertical_drag_start(mut self, callback: impl Fn(DragStartDetails) + 'static) -> Self {
        self.callbacks.on_vertical_drag_start = Some(Rc::new(callback));
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
    pub fn on_vertical_drag_end(mut self, callback: impl Fn(DragEndDetails) + 'static) -> Self {
        self.callbacks.on_vertical_drag_end = Some(Rc::new(callback));
        self
    }

    /// Sets the on_vertical_drag_cancel callback.
    #[must_use]
    pub fn on_vertical_drag_cancel(mut self, callback: impl Fn() + 'static) -> Self {
        self.callbacks.on_vertical_drag_cancel = Some(Rc::new(callback));
        self
    }

    /// Sets the on_scale_start callback.
    #[must_use]
    pub fn on_scale_start(mut self, callback: impl Fn(ScaleStartDetails) + 'static) -> Self {
        self.callbacks.on_scale_start = Some(Rc::new(callback));
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
    pub fn on_scale_end(mut self, callback: impl Fn(ScaleEndDetails) + 'static) -> Self {
        self.callbacks.on_scale_end = Some(Rc::new(callback));
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

    /// Sets the callbacks configuration directly.
    #[must_use]
    pub fn callbacks(mut self, callbacks: GestureCallbacks) -> Self {
        self.callbacks = callbacks;
        self
    }

    /// Associates a retained focus node with this detector. Focus metadata is
    /// kept on the gesture callback record so the widget tree can include the
    /// detector in traversal without introducing a second focus subsystem.
    #[must_use]
    pub fn focus_node(mut self, node: crate::FocusNode) -> Self {
        self.callbacks.focus_node = Some(node);
        self
    }

    /// Requests the associated focus node when the retained tree mounts.
    #[must_use]
    pub fn autofocus(mut self, autofocus: bool) -> Self {
        self.callbacks.autofocus = autofocus;
        self
    }
}

impl From<GestureDetector> for Widget {
    fn from(value: GestureDetector) -> Self {
        let child = value
            .child
            .unwrap_or_else(|| crate::SizedBox::shrink().into());
        Widget::from_kind(WidgetKind::Gesture {
            behavior: value.behavior,
            callbacks: Box::new(value.callbacks),
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
