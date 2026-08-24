//! Flutter-style gesture detection, pointer routing, and hover widgets.

use std::rc::Rc;

pub use incular_core::{Offset, PointerPhase};
pub use incular_gestures::{
    Actions, Command, DragCallbacks, DragEndDetails, DragGestureDetector, DragStartDetails,
    DragUpdateDetails, GestureAction, GestureArena, GestureArenaEntry, GestureArenaKey,
    GestureArenaMember, GestureCallbacks, GestureDecision, GestureDisposition, GestureRecognizer,
    MouseRegion as RawMouseRegion, PointerDeviceKind, PointerEvent, PointerGestureRecognizer,
    ScaleEndDetails, ScaleGestureDetector, ScaleStartDetails, ScaleUpdateDetails, ShortcutKey,
    Shortcuts, TapDownDetails,
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

    /// Sets the on_double_tap callback.
    #[must_use]
    pub fn on_double_tap(mut self, callback: impl Fn() + 'static) -> Self {
        self.callbacks.on_double_tap = Some(Rc::new(callback));
        self
    }

    /// Sets the on_long_press callback.
    #[must_use]
    pub fn on_long_press(mut self, callback: impl Fn() + 'static) -> Self {
        self.callbacks.on_long_press = Some(Rc::new(callback));
        self
    }

    /// Sets the on_pan_update callback.
    #[must_use]
    pub fn on_pan_update(mut self, callback: impl Fn(Offset) + 'static) -> Self {
        self.callbacks.on_pan_update = Some(Rc::new(callback));
        self
    }

    /// Sets the on_horizontal_drag_update callback.
    #[must_use]
    pub fn on_horizontal_drag_update(mut self, callback: impl Fn(Offset) + 'static) -> Self {
        self.callbacks.on_horizontal_drag_update = Some(Rc::new(callback));
        self
    }

    /// Sets the on_vertical_drag_update callback.
    #[must_use]
    pub fn on_vertical_drag_update(mut self, callback: impl Fn(Offset) + 'static) -> Self {
        self.callbacks.on_vertical_drag_update = Some(Rc::new(callback));
        self
    }

    /// Sets the on_scale_update callback.
    #[must_use]
    pub fn on_scale_update(mut self, callback: impl Fn(ScaleUpdateDetails) + 'static) -> Self {
        self.callbacks.on_scale_update = Some(Rc::new(callback));
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

/// Compatibility alias for [`GestureDetector`].
pub type GestureRegion = GestureDetector;

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
