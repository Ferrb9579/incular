use super::*;

/// Listens for notifications bubbling up the widget tree.
#[derive(Clone, TypedBuilder)]
pub struct NotificationListener {
    #[builder(default)]
    callback: Option<Rc<dyn Fn(ScrollNotification) -> bool>>,
    #[builder(setter(into))]
    child: Widget,
}

impl NotificationListener {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            callback: None,
            child: child.into(),
        }
    }

    /// Receives scroll notifications from descendant viewports. Returning
    /// `true` stops the notification from reaching an outer listener.
    #[must_use]
    pub fn on_notification(
        mut self,
        callback: impl Fn(ScrollNotification) -> bool + 'static,
    ) -> Self {
        self.callback = Some(Rc::new(callback));
        self
    }
}

impl From<NotificationListener> for Widget {
    fn from(value: NotificationListener) -> Self {
        Widget::notification_listener(value.callback, value.child)
    }
}

/// Observes scroll notifications.
#[derive(Clone, TypedBuilder)]
pub struct ScrollNotificationObserver {
    #[builder(setter(into))]
    child: Widget,
}

impl ScrollNotificationObserver {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl From<ScrollNotificationObserver> for Widget {
    fn from(value: ScrollNotificationObserver) -> Self {
        value.child
    }
}
