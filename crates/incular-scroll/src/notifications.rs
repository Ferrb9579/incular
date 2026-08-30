use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

use crate::{
    controller::{ScrollController, ScrollState},
    metrics::ScrollMetrics,
};

pub(crate) type ScrollNotificationListener = Rc<dyn Fn(ScrollNotification) -> bool>;

/// Kind of normalized notification emitted by a [`ScrollController`].
///
/// The payload deliberately stays renderer-independent, so the same stream
/// can feed widgets, accessibility, tests, or an inspector panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ScrollNotificationType {
    Start,
    Update,
    Overscroll,
    End,
    UserScroll,
    Metrics,
}

/// A scroll event paired with the complete current viewport metrics.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollNotification {
    pub kind: ScrollNotificationType,
    pub metrics: ScrollMetrics,
    pub delta: f32,
    pub overscroll: f32,
    /// Nested viewport depth. Direct controller subscriptions observe depth
    /// zero; a widget bubbling adapter can increase it while forwarding.
    pub depth: usize,
}

/// RAII subscription returned by [`ScrollController::add_listener`].
pub struct ScrollNotificationSubscription {
    state: Weak<RefCell<ScrollState>>,
    id: u64,
}

impl Drop for ScrollNotificationSubscription {
    fn drop(&mut self) {
        if let Some(state) = self.state.upgrade() {
            state
                .borrow()
                .notification_listeners
                .borrow_mut()
                .retain(|(id, _)| *id != self.id);
        }
    }
}

impl ScrollController {
    /// Subscribes to normalized scroll notifications. The returned handle
    /// removes the listener when dropped. Returning `true` stops dispatch to
    /// later listeners for the same scroll activity, matching Flutter's
    /// `NotificationListener` contract.
    #[must_use]
    pub fn add_notification_listener(
        &self,
        listener: impl Fn(ScrollNotification) -> bool + 'static,
    ) -> ScrollNotificationSubscription {
        let mut state = self.state.borrow_mut();
        let id = state.next_notification_listener;
        state.next_notification_listener = id.wrapping_add(1);
        state
            .notification_listeners
            .borrow_mut()
            .push((id, Rc::new(listener)));
        ScrollNotificationSubscription {
            state: Rc::downgrade(&self.state),
            id,
        }
    }

    /// Alias matching the usual controller listener spelling.
    #[must_use]
    pub fn add_listener(
        &self,
        listener: impl Fn(ScrollNotification) -> bool + 'static,
    ) -> ScrollNotificationSubscription {
        self.add_notification_listener(listener)
    }
}

impl ScrollController {
    pub(crate) fn dispatch_notification(
        &self,
        kind: ScrollNotificationType,
        delta: f32,
        overscroll: f32,
    ) {
        let notification = ScrollNotification {
            kind,
            metrics: self.metrics(),
            delta,
            overscroll,
            depth: 0,
        };
        let listeners = self
            .state
            .borrow()
            .notification_listeners
            .borrow()
            .iter()
            .map(|(_, listener)| listener.clone())
            .collect::<Vec<_>>();
        for listener in listeners {
            if listener(notification) {
                break;
            }
        }
    }
}
