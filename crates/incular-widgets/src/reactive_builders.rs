//! Reactive, async, and animation builder widgets.

#![allow(clippy::type_complexity)]

use crate::{BuildContext, Widget};
use std::rc::Rc;
use std::time::Duration;

/// Rebuilds its child when an animation controller notifies of a tick.
#[derive(Clone)]
pub struct AnimatedBuilder {
    builder: Rc<dyn Fn(&BuildContext, Option<Widget>) -> Widget>,
    child: Option<Widget>,
}

impl AnimatedBuilder {
    #[must_use]
    pub fn new<W>(builder: impl Fn(&BuildContext, Option<Widget>) -> W + 'static) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self {
            builder: Rc::new(move |ctx, ch| builder(ctx, ch).into()),
            child: None,
        }
    }

    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self
    }
}

impl From<AnimatedBuilder> for Widget {
    fn from(value: AnimatedBuilder) -> Self {
        let dummy_ctx = BuildContext;
        (value.builder)(&dummy_ctx, value.child)
    }
}

/// General-purpose builder that rebuilds when a listenable change occurs.
#[derive(Clone)]
pub struct ListenableBuilder {
    builder: Rc<dyn Fn(&BuildContext, Option<Widget>) -> Widget>,
    child: Option<Widget>,
}

impl ListenableBuilder {
    #[must_use]
    pub fn new<W>(builder: impl Fn(&BuildContext, Option<Widget>) -> W + 'static) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self {
            builder: Rc::new(move |ctx, ch| builder(ctx, ch).into()),
            child: None,
        }
    }

    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self
    }
}

impl From<ListenableBuilder> for Widget {
    fn from(value: ListenableBuilder) -> Self {
        let dummy_ctx = BuildContext;
        (value.builder)(&dummy_ctx, value.child)
    }
}

/// Generic builder reacting to typed value notifications.
#[derive(Clone)]
pub struct ValueListenableBuilder<T: Clone + 'static> {
    value: T,
    builder: Rc<dyn Fn(&BuildContext, &T, Option<Widget>) -> Widget>,
    child: Option<Widget>,
}

impl<T: Clone + 'static> ValueListenableBuilder<T> {
    #[must_use]
    pub fn new<W>(
        value: T,
        builder: impl Fn(&BuildContext, &T, Option<Widget>) -> W + 'static,
    ) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self {
            value,
            builder: Rc::new(move |ctx, val, ch| builder(ctx, val, ch).into()),
            child: None,
        }
    }

    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self
    }
}

impl<T: Clone + 'static> From<ValueListenableBuilder<T>> for Widget {
    fn from(value: ValueListenableBuilder<T>) -> Self {
        let dummy_ctx = BuildContext;
        (value.builder)(&dummy_ctx, &value.value, value.child)
    }
}

/// Connection state for asynchronous operations.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ConnectionState {
    None,
    Waiting,
    Active,
    Done,
}

/// Snapshot of an asynchronous computation.
#[derive(Clone, Debug, PartialEq)]
pub struct AsyncSnapshot<T: Clone> {
    pub connection_state: ConnectionState,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T: Clone> AsyncSnapshot<T> {
    #[must_use]
    pub fn has_data(&self) -> bool {
        self.data.is_some()
    }

    #[must_use]
    pub fn has_error(&self) -> bool {
        self.error.is_some()
    }
}

/// Asynchronous builder for Futures / Tokio Tasks.
#[derive(Clone)]
pub struct FutureBuilder<T: Clone + 'static> {
    initial_data: Option<T>,
    builder: Rc<dyn Fn(&BuildContext, &AsyncSnapshot<T>) -> Widget>,
}

impl<T: Clone + 'static> FutureBuilder<T> {
    #[must_use]
    pub fn new<W>(builder: impl Fn(&BuildContext, &AsyncSnapshot<T>) -> W + 'static) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self {
            initial_data: None,
            builder: Rc::new(move |ctx, snap| builder(ctx, snap).into()),
        }
    }

    #[must_use]
    pub fn initial_data(mut self, data: T) -> Self {
        self.initial_data = Some(data);
        self
    }
}

impl<T: Clone + 'static> From<FutureBuilder<T>> for Widget {
    fn from(value: FutureBuilder<T>) -> Self {
        let snapshot = AsyncSnapshot {
            connection_state: ConnectionState::Done,
            data: value.initial_data,
            error: None,
        };
        let dummy_ctx = BuildContext;
        (value.builder)(&dummy_ctx, &snapshot)
    }
}

/// Asynchronous builder for Streams / Channels.
#[derive(Clone)]
pub struct StreamBuilder<T: Clone + 'static> {
    initial_data: Option<T>,
    builder: Rc<dyn Fn(&BuildContext, &AsyncSnapshot<T>) -> Widget>,
}

impl<T: Clone + 'static> StreamBuilder<T> {
    #[must_use]
    pub fn new<W>(builder: impl Fn(&BuildContext, &AsyncSnapshot<T>) -> W + 'static) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self {
            initial_data: None,
            builder: Rc::new(move |ctx, snap| builder(ctx, snap).into()),
        }
    }

    #[must_use]
    pub fn initial_data(mut self, data: T) -> Self {
        self.initial_data = Some(data);
        self
    }
}

impl<T: Clone + 'static> From<StreamBuilder<T>> for Widget {
    fn from(value: StreamBuilder<T>) -> Self {
        let snapshot = AsyncSnapshot {
            connection_state: ConnectionState::Done,
            data: value.initial_data,
            error: None,
        };
        let dummy_ctx = BuildContext;
        (value.builder)(&dummy_ctx, &snapshot)
    }
}

/// Builder with local mutable state and rebuild capability.
#[derive(Clone)]
pub struct StatefulBuilder {
    builder: Rc<dyn Fn(&BuildContext, &dyn Fn()) -> Widget>,
}

impl StatefulBuilder {
    #[must_use]
    pub fn new<W>(builder: impl Fn(&BuildContext, &dyn Fn()) -> W + 'static) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self {
            builder: Rc::new(move |ctx, set_state| builder(ctx, set_state).into()),
        }
    }
}

impl From<StatefulBuilder> for Widget {
    fn from(value: StatefulBuilder) -> Self {
        let dummy_ctx = BuildContext;
        let set_state = || {};
        (value.builder)(&dummy_ctx, &set_state)
    }
}

/// Implicit animation builder that interpolates values without manual controller boilerplate.
#[derive(Clone)]
pub struct TweenAnimationBuilder<T: Clone + 'static> {
    tween_end: T,
    duration: Duration,
    builder: Rc<dyn Fn(&BuildContext, &T, Option<Widget>) -> Widget>,
    child: Option<Widget>,
}

impl<T: Clone + 'static> TweenAnimationBuilder<T> {
    #[must_use]
    pub fn new<W>(
        tween_end: T,
        duration: Duration,
        builder: impl Fn(&BuildContext, &T, Option<Widget>) -> W + 'static,
    ) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self {
            tween_end,
            duration,
            builder: Rc::new(move |ctx, val, ch| builder(ctx, val, ch).into()),
            child: None,
        }
    }

    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self
    }

    #[must_use]
    pub fn duration(&self) -> Duration {
        self.duration
    }
}

impl<T: Clone + 'static> From<TweenAnimationBuilder<T>> for Widget {
    fn from(value: TweenAnimationBuilder<T>) -> Self {
        let dummy_ctx = BuildContext;
        (value.builder)(&dummy_ctx, &value.tween_end, value.child)
    }
}

/// Repeating animation builder ticking indefinitely.
#[derive(Clone)]
pub struct RepeatingAnimationBuilder {
    duration: Duration,
    builder: Rc<dyn Fn(&BuildContext, f32) -> Widget>,
}

impl RepeatingAnimationBuilder {
    #[must_use]
    pub fn new<W>(duration: Duration, builder: impl Fn(&BuildContext, f32) -> W + 'static) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self {
            duration,
            builder: Rc::new(move |ctx, progress| builder(ctx, progress).into()),
        }
    }

    #[must_use]
    pub fn duration(&self) -> Duration {
        self.duration
    }
}

impl From<RepeatingAnimationBuilder> for Widget {
    fn from(value: RepeatingAnimationBuilder) -> Self {
        let dummy_ctx = BuildContext;
        (value.builder)(&dummy_ctx, 1.0)
    }
}
