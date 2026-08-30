//! Retained adapters for Flutter's raw pointer widgets.
//!
//! The public descriptors in this module are immutable widget values. Their
//! mutable state (hover sets, pointer routes, tap-region membership, and raw
//! recognizers) lives in `WidgetTree`, which gives updates and unmounts the
//! same lifecycle boundary as every other retained element.

use std::{fmt, rc::Rc};

pub use incular_gestures::{
    ErasedGestureRecognizerFactory, GestureRecognizer, GestureRecognizerFactory,
    GestureRecognizerFactoryError, GestureRecognizerFactoryWithHandlers, MouseCursor,
    PointerDeviceKind, RawPointerEvent,
};

use crate::{Widget, WidgetKind};
use crate::{gestures::HitTestBehavior, tree::WidgetType};

/// Callback set used by [`Listener`].
#[derive(Clone, Default)]
pub struct ListenerCallbacks {
    pub on_pointer_down: Option<Rc<dyn Fn(RawPointerEvent)>>,
    pub on_pointer_move: Option<Rc<dyn Fn(RawPointerEvent)>>,
    pub on_pointer_up: Option<Rc<dyn Fn(RawPointerEvent)>>,
    pub on_pointer_hover: Option<Rc<dyn Fn(RawPointerEvent)>>,
    pub on_pointer_cancel: Option<Rc<dyn Fn(RawPointerEvent)>>,
    pub on_pointer_signal: Option<Rc<dyn Fn(RawPointerEvent)>>,
}

/// Callback set used by [`MouseRegion`].
#[derive(Clone, Default)]
pub struct MouseRegionCallbacks {
    pub on_enter: Option<Rc<dyn Fn(RawPointerEvent)>>,
    pub on_exit: Option<Rc<dyn Fn(RawPointerEvent)>>,
    pub on_hover: Option<Rc<dyn Fn(RawPointerEvent)>>,
}

/// Callback set used by [`TapRegion`] and [`TextFieldTapRegion`].
#[derive(Clone, Default)]
pub struct TapRegionCallbacks {
    pub on_tap_inside: Option<Rc<dyn Fn(RawPointerEvent)>>,
    pub on_tap_outside: Option<Rc<dyn Fn(RawPointerEvent)>>,
    pub on_tap_up_inside: Option<Rc<dyn Fn(RawPointerEvent)>>,
    pub on_tap_up_outside: Option<Rc<dyn Fn(RawPointerEvent)>>,
}

/// Identity used to group tap regions. Flutter accepts arbitrary object
/// identity; Rust callers get explicit, hashable identities that can be
/// shared by value across rebuilt widget descriptions.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub enum TapRegionGroupId {
    Named(String),
    Numeric(u64),
    /// The default group used by [`TextFieldTapRegion`].
    #[default]
    TextField,
}

impl TapRegionGroupId {
    #[must_use]
    pub fn named(value: impl Into<String>) -> Self {
        Self::Named(value.into())
    }

    #[must_use]
    pub const fn numeric(value: u64) -> Self {
        Self::Numeric(value)
    }

    #[must_use]
    pub const fn text_field() -> Self {
        Self::TextField
    }
}

impl From<&str> for TapRegionGroupId {
    fn from(value: &str) -> Self {
        Self::Named(value.to_owned())
    }
}

impl From<String> for TapRegionGroupId {
    fn from(value: String) -> Self {
        Self::Named(value)
    }
}

impl From<u64> for TapRegionGroupId {
    fn from(value: u64) -> Self {
        Self::Numeric(value)
    }
}

/// Internal immutable descriptor stored in a `WidgetKind`.
#[derive(Clone)]
pub enum RawInputKind {
    Listener {
        callbacks: ListenerCallbacks,
        behavior: HitTestBehavior,
    },
    RawGestureDetector {
        factories: Vec<ErasedGestureRecognizerFactory>,
        behavior: HitTestBehavior,
        exclude_from_semantics: bool,
    },
    MouseRegion {
        callbacks: MouseRegionCallbacks,
        cursor: MouseCursor,
        opaque: bool,
        behavior: Option<HitTestBehavior>,
    },
    TapRegion {
        callbacks: TapRegionCallbacks,
        enabled: bool,
        behavior: HitTestBehavior,
        group_id: Option<TapRegionGroupId>,
        consume_outside_taps: bool,
    },
    TapRegionSurface,
    TextFieldTapRegion {
        callbacks: TapRegionCallbacks,
        enabled: bool,
        behavior: HitTestBehavior,
        group_id: TapRegionGroupId,
        consume_outside_taps: bool,
    },
}

impl RawInputKind {
    pub(crate) const fn type_(&self) -> WidgetType {
        match self {
            Self::Listener { .. } => WidgetType::Listener,
            Self::RawGestureDetector { .. } => WidgetType::RawGestureDetector,
            Self::MouseRegion { .. } => WidgetType::MouseRegion,
            Self::TapRegion { .. } => WidgetType::TapRegion,
            Self::TapRegionSurface => WidgetType::TapRegionSurface,
            Self::TextFieldTapRegion { .. } => WidgetType::TextFieldTapRegion,
        }
    }

    pub(crate) const fn hit_test_behavior(&self) -> HitTestBehavior {
        match self {
            Self::Listener { behavior, .. }
            | Self::RawGestureDetector { behavior, .. }
            | Self::TapRegion { behavior, .. }
            | Self::TextFieldTapRegion { behavior, .. } => *behavior,
            Self::MouseRegion {
                behavior: Some(behavior),
                ..
            } => *behavior,
            Self::MouseRegion {
                behavior: None,
                opaque,
                ..
            } => {
                if *opaque {
                    HitTestBehavior::Opaque
                } else {
                    HitTestBehavior::Translucent
                }
            }
            Self::TapRegionSurface => HitTestBehavior::DeferToChild,
        }
    }

    pub(crate) const fn is_enabled(&self) -> bool {
        match self {
            Self::TapRegion { enabled, .. } | Self::TextFieldTapRegion { enabled, .. } => *enabled,
            _ => true,
        }
    }

    pub(crate) fn factory_map(&self) -> Option<&[ErasedGestureRecognizerFactory]> {
        match self {
            Self::RawGestureDetector { factories, .. } => Some(factories),
            _ => None,
        }
    }
}

impl fmt::Debug for ListenerCallbacks {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ListenerCallbacks")
            .field("on_pointer_down", &self.on_pointer_down.is_some())
            .field("on_pointer_move", &self.on_pointer_move.is_some())
            .field("on_pointer_up", &self.on_pointer_up.is_some())
            .field("on_pointer_hover", &self.on_pointer_hover.is_some())
            .field("on_pointer_cancel", &self.on_pointer_cancel.is_some())
            .field("on_pointer_signal", &self.on_pointer_signal.is_some())
            .finish()
    }
}

impl fmt::Debug for MouseRegionCallbacks {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MouseRegionCallbacks")
            .field("on_enter", &self.on_enter.is_some())
            .field("on_exit", &self.on_exit.is_some())
            .field("on_hover", &self.on_hover.is_some())
            .finish()
    }
}

impl fmt::Debug for TapRegionCallbacks {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TapRegionCallbacks")
            .field("on_tap_inside", &self.on_tap_inside.is_some())
            .field("on_tap_outside", &self.on_tap_outside.is_some())
            .field("on_tap_up_inside", &self.on_tap_up_inside.is_some())
            .field("on_tap_up_outside", &self.on_tap_up_outside.is_some())
            .finish()
    }
}

fn same_callback<T: ?Sized>(left: &Option<Rc<T>>, right: &Option<Rc<T>>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => Rc::ptr_eq(left, right),
        (None, None) => true,
        _ => false,
    }
}

fn listener_callbacks_eq(left: &ListenerCallbacks, right: &ListenerCallbacks) -> bool {
    same_callback(&left.on_pointer_down, &right.on_pointer_down)
        && same_callback(&left.on_pointer_move, &right.on_pointer_move)
        && same_callback(&left.on_pointer_up, &right.on_pointer_up)
        && same_callback(&left.on_pointer_hover, &right.on_pointer_hover)
        && same_callback(&left.on_pointer_cancel, &right.on_pointer_cancel)
        && same_callback(&left.on_pointer_signal, &right.on_pointer_signal)
}

fn mouse_callbacks_eq(left: &MouseRegionCallbacks, right: &MouseRegionCallbacks) -> bool {
    same_callback(&left.on_enter, &right.on_enter)
        && same_callback(&left.on_exit, &right.on_exit)
        && same_callback(&left.on_hover, &right.on_hover)
}

fn tap_callbacks_eq(left: &TapRegionCallbacks, right: &TapRegionCallbacks) -> bool {
    same_callback(&left.on_tap_inside, &right.on_tap_inside)
        && same_callback(&left.on_tap_outside, &right.on_tap_outside)
        && same_callback(&left.on_tap_up_inside, &right.on_tap_up_inside)
        && same_callback(&left.on_tap_up_outside, &right.on_tap_up_outside)
}

impl PartialEq for RawInputKind {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Listener {
                    callbacks: left,
                    behavior: left_behavior,
                },
                Self::Listener {
                    callbacks: right,
                    behavior: right_behavior,
                },
            ) => left_behavior == right_behavior && listener_callbacks_eq(left, right),
            (
                Self::RawGestureDetector {
                    factories: left,
                    behavior: left_behavior,
                    exclude_from_semantics: left_exclude,
                },
                Self::RawGestureDetector {
                    factories: right,
                    behavior: right_behavior,
                    exclude_from_semantics: right_exclude,
                },
            ) => {
                left_behavior == right_behavior
                    && left_exclude == right_exclude
                    && left.len() == right.len()
                    && left.iter().zip(right).all(|(left, right)| {
                        left.type_id() == right.type_id() && left.ptr_eq(right)
                    })
            }
            (
                Self::MouseRegion {
                    callbacks: left,
                    cursor: left_cursor,
                    opaque: left_opaque,
                    behavior: left_behavior,
                },
                Self::MouseRegion {
                    callbacks: right,
                    cursor: right_cursor,
                    opaque: right_opaque,
                    behavior: right_behavior,
                },
            ) => {
                left_cursor == right_cursor
                    && left_opaque == right_opaque
                    && left_behavior == right_behavior
                    && mouse_callbacks_eq(left, right)
            }
            (
                Self::TapRegion {
                    callbacks: left,
                    enabled: left_enabled,
                    behavior: left_behavior,
                    group_id: left_group,
                    consume_outside_taps: left_consume,
                },
                Self::TapRegion {
                    callbacks: right,
                    enabled: right_enabled,
                    behavior: right_behavior,
                    group_id: right_group,
                    consume_outside_taps: right_consume,
                },
            ) => {
                left_enabled == right_enabled
                    && left_behavior == right_behavior
                    && left_group == right_group
                    && left_consume == right_consume
                    && tap_callbacks_eq(left, right)
            }
            (Self::TapRegionSurface, Self::TapRegionSurface) => true,
            (
                Self::TextFieldTapRegion {
                    callbacks: left,
                    enabled: left_enabled,
                    behavior: left_behavior,
                    group_id: left_group,
                    consume_outside_taps: left_consume,
                },
                Self::TextFieldTapRegion {
                    callbacks: right,
                    enabled: right_enabled,
                    behavior: right_behavior,
                    group_id: right_group,
                    consume_outside_taps: right_consume,
                },
            ) => {
                left_enabled == right_enabled
                    && left_behavior == right_behavior
                    && left_group == right_group
                    && left_consume == right_consume
                    && tap_callbacks_eq(left, right)
            }
            _ => false,
        }
    }
}

impl fmt::Debug for RawInputKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Listener { .. } => "Listener",
            Self::RawGestureDetector { .. } => "RawGestureDetector",
            Self::MouseRegion { .. } => "MouseRegion",
            Self::TapRegion { .. } => "TapRegion",
            Self::TapRegionSurface => "TapRegionSurface",
            Self::TextFieldTapRegion { .. } => "TextFieldTapRegion",
        };
        formatter.write_str(name)
    }
}

/// Raw pointer callbacks without a gesture arena.
#[derive(Clone, Default)]
pub struct Listener {
    callbacks: ListenerCallbacks,
    behavior: HitTestBehavior,
    child: Option<Widget>,
}

impl Listener {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: Some(child.into()),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn without_child() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self
    }

    #[must_use]
    pub fn behavior(mut self, behavior: HitTestBehavior) -> Self {
        self.behavior = behavior;
        self
    }

    #[must_use]
    pub fn on_pointer_down(mut self, callback: impl Fn(RawPointerEvent) + 'static) -> Self {
        self.callbacks.on_pointer_down = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn on_pointer_move(mut self, callback: impl Fn(RawPointerEvent) + 'static) -> Self {
        self.callbacks.on_pointer_move = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn on_pointer_up(mut self, callback: impl Fn(RawPointerEvent) + 'static) -> Self {
        self.callbacks.on_pointer_up = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn on_pointer_hover(mut self, callback: impl Fn(RawPointerEvent) + 'static) -> Self {
        self.callbacks.on_pointer_hover = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn on_pointer_cancel(mut self, callback: impl Fn(RawPointerEvent) + 'static) -> Self {
        self.callbacks.on_pointer_cancel = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn on_pointer_signal(mut self, callback: impl Fn(RawPointerEvent) + 'static) -> Self {
        self.callbacks.on_pointer_signal = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn callbacks(mut self, callbacks: ListenerCallbacks) -> Self {
        self.callbacks = callbacks;
        self
    }
}

impl From<Listener> for Widget {
    fn from(value: Listener) -> Self {
        Widget::from_kind(WidgetKind::RawInput {
            kind: RawInputKind::Listener {
                callbacks: value.callbacks,
                behavior: value.behavior,
            },
            child: value.child.map(Box::new),
        })
    }
}

/// A gesture detector whose recognizers are created, updated, and disposed
/// by type-erased factories at the retained element boundary.
#[derive(Clone, Default)]
pub struct RawGestureDetector {
    factories: Vec<ErasedGestureRecognizerFactory>,
    behavior: Option<HitTestBehavior>,
    exclude_from_semantics: bool,
    child: Option<Widget>,
}

impl RawGestureDetector {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: Some(child.into()),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn without_child() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self
    }

    /// Replaces the complete factory set. Duplicate concrete types are
    /// rejected by the retained synchronizer; the first factory wins.
    #[must_use]
    pub fn gestures<I, F>(mut self, factories: I) -> Self
    where
        I: IntoIterator<Item = F>,
        F: Into<ErasedGestureRecognizerFactory>,
    {
        self.factories = factories.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn gesture<F>(mut self, factory: F) -> Self
    where
        F: Into<ErasedGestureRecognizerFactory>,
    {
        self.factories.push(factory.into());
        self
    }

    #[must_use]
    pub fn behavior(mut self, behavior: HitTestBehavior) -> Self {
        self.behavior = Some(behavior);
        self
    }

    #[must_use]
    pub fn exclude_from_semantics(mut self, exclude: bool) -> Self {
        self.exclude_from_semantics = exclude;
        self
    }
}

impl From<RawGestureDetector> for Widget {
    fn from(value: RawGestureDetector) -> Self {
        let behavior = value.behavior.unwrap_or(if value.child.is_some() {
            HitTestBehavior::DeferToChild
        } else {
            HitTestBehavior::Translucent
        });
        Widget::from_kind(WidgetKind::RawInput {
            kind: RawInputKind::RawGestureDetector {
                factories: value.factories,
                behavior,
                exclude_from_semantics: value.exclude_from_semantics,
            },
            child: value.child.map(Box::new),
        })
    }
}

/// Tracks pointer entry, hover, exit, cursor preference, and opaque hit-test
/// behavior for a retained region.
#[derive(Clone, Default)]
pub struct MouseRegion {
    callbacks: MouseRegionCallbacks,
    cursor: MouseCursor,
    opaque: bool,
    behavior: Option<HitTestBehavior>,
    child: Option<Widget>,
}

impl MouseRegion {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: Some(child.into()),
            opaque: true,
            ..Self::default()
        }
    }

    #[must_use]
    pub fn without_child() -> Self {
        Self {
            opaque: true,
            ..Self::default()
        }
    }

    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self
    }

    #[must_use]
    pub fn on_enter(mut self, callback: impl Fn(RawPointerEvent) + 'static) -> Self {
        self.callbacks.on_enter = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn on_exit(mut self, callback: impl Fn(RawPointerEvent) + 'static) -> Self {
        self.callbacks.on_exit = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn on_hover(mut self, callback: impl Fn(RawPointerEvent) + 'static) -> Self {
        self.callbacks.on_hover = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn cursor(mut self, cursor: MouseCursor) -> Self {
        self.cursor = cursor;
        self
    }

    #[must_use]
    pub fn opaque(mut self, opaque: bool) -> Self {
        self.opaque = opaque;
        self
    }

    #[must_use]
    pub fn hit_test_behavior(mut self, behavior: HitTestBehavior) -> Self {
        self.behavior = Some(behavior);
        self
    }

    #[must_use]
    pub fn callbacks(mut self, callbacks: MouseRegionCallbacks) -> Self {
        self.callbacks = callbacks;
        self
    }
}

impl From<MouseRegion> for Widget {
    fn from(value: MouseRegion) -> Self {
        Widget::from_kind(WidgetKind::RawInput {
            kind: RawInputKind::MouseRegion {
                callbacks: value.callbacks,
                cursor: value.cursor,
                opaque: value.opaque,
                behavior: value.behavior,
            },
            child: value.child.map(Box::new),
        })
    }
}

/// A group-aware pointer boundary for inside/outside tap callbacks.
#[derive(Clone, Default)]
pub struct TapRegion {
    callbacks: TapRegionCallbacks,
    enabled: bool,
    behavior: HitTestBehavior,
    group_id: Option<TapRegionGroupId>,
    consume_outside_taps: bool,
    child: Option<Widget>,
}

impl TapRegion {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: Some(child.into()),
            enabled: true,
            ..Self::default()
        }
    }

    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    #[must_use]
    pub fn behavior(mut self, behavior: HitTestBehavior) -> Self {
        self.behavior = behavior;
        self
    }

    #[must_use]
    pub fn group_id(mut self, group_id: impl Into<TapRegionGroupId>) -> Self {
        self.group_id = Some(group_id.into());
        self
    }

    #[must_use]
    pub fn clear_group_id(mut self) -> Self {
        self.group_id = None;
        self
    }

    #[must_use]
    pub fn consume_outside_taps(mut self, consume: bool) -> Self {
        self.consume_outside_taps = consume;
        self
    }

    #[must_use]
    pub fn on_tap_inside(mut self, callback: impl Fn(RawPointerEvent) + 'static) -> Self {
        self.callbacks.on_tap_inside = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn on_tap_outside(mut self, callback: impl Fn(RawPointerEvent) + 'static) -> Self {
        self.callbacks.on_tap_outside = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn on_tap_up_inside(mut self, callback: impl Fn(RawPointerEvent) + 'static) -> Self {
        self.callbacks.on_tap_up_inside = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn on_tap_up_outside(mut self, callback: impl Fn(RawPointerEvent) + 'static) -> Self {
        self.callbacks.on_tap_up_outside = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn callbacks(mut self, callbacks: TapRegionCallbacks) -> Self {
        self.callbacks = callbacks;
        self
    }
}

impl From<TapRegion> for Widget {
    fn from(value: TapRegion) -> Self {
        Widget::from_kind(WidgetKind::RawInput {
            kind: RawInputKind::TapRegion {
                callbacks: value.callbacks,
                enabled: value.enabled,
                behavior: value.behavior,
                group_id: value.group_id,
                consume_outside_taps: value.consume_outside_taps,
            },
            child: Some(Box::new(
                value
                    .child
                    .unwrap_or_else(|| crate::SizedBox::shrink().into()),
            )),
        })
    }
}

/// Establishes the nearest retained tap-region registry for its subtree.
#[derive(Clone)]
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
        Widget::from_kind(WidgetKind::RawInput {
            kind: RawInputKind::TapRegionSurface,
            child: Some(Box::new(value.child)),
        })
    }
}

/// A tap region that joins the default text-field group so controls such as
/// increment/decrement buttons do not dismiss the associated editor.
#[derive(Clone, Default)]
pub struct TextFieldTapRegion {
    callbacks: TapRegionCallbacks,
    enabled: bool,
    behavior: HitTestBehavior,
    group_id: TapRegionGroupId,
    consume_outside_taps: bool,
    child: Option<Widget>,
}

impl TextFieldTapRegion {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: Some(child.into()),
            enabled: true,
            group_id: TapRegionGroupId::text_field(),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    #[must_use]
    pub fn behavior(mut self, behavior: HitTestBehavior) -> Self {
        self.behavior = behavior;
        self
    }

    #[must_use]
    pub fn group_id(mut self, group_id: impl Into<TapRegionGroupId>) -> Self {
        self.group_id = group_id.into();
        self
    }

    #[must_use]
    pub fn consume_outside_taps(mut self, consume: bool) -> Self {
        self.consume_outside_taps = consume;
        self
    }

    #[must_use]
    pub fn on_tap_inside(mut self, callback: impl Fn(RawPointerEvent) + 'static) -> Self {
        self.callbacks.on_tap_inside = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn on_tap_outside(mut self, callback: impl Fn(RawPointerEvent) + 'static) -> Self {
        self.callbacks.on_tap_outside = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn on_tap_up_inside(mut self, callback: impl Fn(RawPointerEvent) + 'static) -> Self {
        self.callbacks.on_tap_up_inside = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn on_tap_up_outside(mut self, callback: impl Fn(RawPointerEvent) + 'static) -> Self {
        self.callbacks.on_tap_up_outside = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn callbacks(mut self, callbacks: TapRegionCallbacks) -> Self {
        self.callbacks = callbacks;
        self
    }
}

impl From<TextFieldTapRegion> for Widget {
    fn from(value: TextFieldTapRegion) -> Self {
        Widget::from_kind(WidgetKind::RawInput {
            kind: RawInputKind::TextFieldTapRegion {
                callbacks: value.callbacks,
                enabled: value.enabled,
                behavior: value.behavior,
                group_id: value.group_id,
                consume_outside_taps: value.consume_outside_taps,
            },
            child: Some(Box::new(
                value
                    .child
                    .unwrap_or_else(|| crate::SizedBox::shrink().into()),
            )),
        })
    }
}
