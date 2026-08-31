//! Animation builder widgets.

#![allow(clippy::type_complexity)]

use crate::{BuildContext, Widget};
use std::rc::Rc;
use std::time::Duration;

/// Rebuilds its child when an animation controller notifies of a tick.
#[derive(Clone)]
pub struct AnimatedBuilder {
    builder: Rc<dyn for<'a> Fn(&BuildContext<'a>, Option<Widget>) -> Widget>,
    child: Option<Widget>,
}

impl AnimatedBuilder {
    #[must_use]
    pub fn new<W>(
        builder: impl for<'a> Fn(&BuildContext<'a>, Option<Widget>) -> W + 'static,
    ) -> Self
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
        let builder = value.builder;
        let child = value.child;
        Widget::layout_builder(move |context, _| builder(context, child.clone()))
    }
}

/// Implicit animation builder that interpolates values without manual controller boilerplate.
#[derive(Clone)]
pub struct TweenAnimationBuilder<T: Clone + 'static> {
    tween_end: T,
    duration: Duration,
    builder: Rc<dyn for<'a> Fn(&BuildContext<'a>, &T, Option<Widget>) -> Widget>,
    child: Option<Widget>,
}

impl<T: Clone + 'static> TweenAnimationBuilder<T> {
    #[must_use]
    pub fn new<W>(
        tween_end: T,
        duration: Duration,
        builder: impl for<'a> Fn(&BuildContext<'a>, &T, Option<Widget>) -> W + 'static,
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
        let builder = value.builder;
        let tween_end = value.tween_end;
        let child = value.child;
        Widget::layout_builder(move |context, _| builder(context, &tween_end, child.clone()))
    }
}

/// Repeating animation builder ticking indefinitely.
#[derive(Clone)]
pub struct RepeatingAnimationBuilder {
    duration: Duration,
    builder: Rc<dyn for<'a> Fn(&BuildContext<'a>, f32) -> Widget>,
}

impl RepeatingAnimationBuilder {
    #[must_use]
    pub fn new<W>(
        duration: Duration,
        builder: impl for<'a> Fn(&BuildContext<'a>, f32) -> W + 'static,
    ) -> Self
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
        let builder = value.builder;
        Widget::layout_builder(move |context, _| builder(context, 1.0))
    }
}
