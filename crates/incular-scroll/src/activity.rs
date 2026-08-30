use crate::{controller::ScrollController, notifications::ScrollNotificationType};

impl ScrollController {
    /// Begins a user-driven scroll activity and emits one `Start` event.
    /// Repeated pointer/wheel samples in the same gesture do not emit
    /// duplicate starts.
    pub fn begin_activity(&self) -> bool {
        let started = {
            let mut state = self.state.borrow_mut();
            if state.activity_active {
                false
            } else {
                state.activity_active = true;
                true
            }
        };
        if started {
            self.dispatch_notification(ScrollNotificationType::Start, 0., 0.);
        }
        started
    }

    /// Ends a user-driven scroll activity and emits one `End` event.
    pub fn end_activity(&self) -> bool {
        let ended = {
            let mut state = self.state.borrow_mut();
            if !state.activity_active {
                false
            } else {
                state.activity_active = false;
                true
            }
        };
        if ended {
            self.dispatch_notification(ScrollNotificationType::End, 0., 0.);
        }
        ended
    }

    /// Emits a user-scroll direction sample. The first sample implicitly
    /// starts the activity, matching Flutter's start/update/user-scroll event
    /// ordering while keeping input adapters small.
    pub fn notify_user_scroll(&self, delta: f32) -> bool {
        if !delta.is_finite() || delta.abs() <= f32::EPSILON {
            return false;
        }
        self.begin_activity();
        self.dispatch_notification(ScrollNotificationType::UserScroll, delta, 0.);
        true
    }
}

/// How a scroll view dismisses the virtual keyboard.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ScrollViewKeyboardDismissBehavior {
    #[default]
    Manual,
    OnDrag,
}

/// Determines when a drag gesture begins recognizing motion.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DragStartBehavior {
    #[default]
    Start,
    Down,
}
