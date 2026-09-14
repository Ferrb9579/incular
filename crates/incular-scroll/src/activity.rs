use crate::{controller::ScrollController, notifications::ScrollNotificationType};

/// Which driver owns an activity token. Metric-attachment ownership
/// stays separate: this names the input/animation driver holding the
/// bracket, never who may publish geometry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ActivityOrigin {
    /// A scrollbar thumb drag owns the bracket.
    Scrollbar,
}

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
                state.activity_generation = state.activity_generation.wrapping_add(1);
                true
            }
        };
        if started {
            self.dispatch_notification(ScrollNotificationType::Start, 0., 0.);
        }
        started
    }

    /// Starts — or takes over — the activity bracket, returning a token
    /// identifying exactly this ownership. From idle this behaves like
    /// [`begin_activity`](Self::begin_activity) with one `Start`. When
    /// another activity is already running, ownership passes to the new
    /// token with no duplicate `Start`: the bracket continues under new
    /// ownership, and only the current token's
    /// [`finish`](OwnedActivity::finish) may close it. A boolean saying
    /// whether `Start` was emitted cannot express this — the token's
    /// generation distinguishes a later activity on the same controller.
    pub fn start_owned_activity(&self, origin: ActivityOrigin) -> OwnedActivity {
        // Commit the identity first, then notify: the token below owns
        // the open bracket before any listener runs, so a panic
        // unwinding through `Start` releases its claim instead of
        // stranding an open flag no token owns. A reentrant takeover
        // bumps past this generation, making this token stale — its
        // eventual drop then cannot cancel the newer activity.
        let (started, generation) = {
            let mut state = self.state.borrow_mut();
            let started = !state.activity_active;
            state.activity_active = true;
            state.activity_generation = state.activity_generation.wrapping_add(1);
            (started, state.activity_generation)
        };
        let token = OwnedActivity {
            controller: self.clone(),
            generation,
            origin,
        };
        if started {
            self.dispatch_notification(ScrollNotificationType::Start, 0., 0.);
        }
        token
    }

    /// The identity of the currently open bracket, if any. A read-only
    /// handle for observers: comparing identities distinguishes a later
    /// activity on the same controller without owning cleanup.
    #[must_use]
    pub fn current_activity_id(&self) -> Option<ActivityId> {
        let state = self.state.borrow();
        state.activity_active.then(|| ActivityId {
            generation: state.activity_generation,
        })
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

/// A token identifying one activity ownership on a controller.
///
/// Created only by
/// [`ScrollController::start_owned_activity`]; only the still-current
/// token may close the bracket. Stale tokens — a newer activity took
/// over, or the bracket already closed — finish silently without
/// touching the newer activity. Dropping a live token aborts its
/// bracket silently when still current (teardown policy, never a
/// notification from `Drop`); dropping a stale token changes nothing.
/// Read-only identity of one activity ownership: comparable and
/// copyable, owning no cleanup. Observers inspect activity identity
/// through this value — never through another RAII owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ActivityId {
    generation: u64,
}

/// Non-cloneable proof of activity ownership: exactly one driver owns
/// the cleanup authority. Cloning is refused structurally so a copy can
/// never become a second cleanup owner; observers use [`ActivityId`].
#[derive(Debug, PartialEq)]
pub struct OwnedActivity {
    controller: ScrollController,
    generation: u64,
    origin: ActivityOrigin,
}

impl OwnedActivity {
    /// The bracketed controller.
    #[must_use]
    pub fn controller(&self) -> &ScrollController {
        &self.controller
    }

    /// This token's generation. Increasing generations mark takeovers;
    /// only the latest generation on an open bracket is current.
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// This token's read-only identity for observers.
    #[must_use]
    pub fn id(&self) -> ActivityId {
        ActivityId {
            generation: self.generation,
        }
    }

    /// Which driver owns this token. Diagnostic only.
    #[must_use]
    pub fn origin(&self) -> ActivityOrigin {
        self.origin
    }

    /// Whether this token still owns the open bracket.
    #[must_use]
    pub fn is_current(&self) -> bool {
        let state = self.controller.state.borrow();
        state.activity_active && state.activity_generation == self.generation
    }

    /// Closes the bracket with one `End`, but only while this token is
    /// still current. Returns whether the `End` was emitted; stale
    /// tokens return `false` without touching a newer activity or an
    /// already-closed bracket. Consuming: each token finishes at most
    /// once by construction.
    pub fn finish(self) -> bool {
        let ended = {
            let mut state = self.controller.state.borrow_mut();
            if state.activity_active && state.activity_generation == self.generation {
                state.activity_active = false;
                true
            } else {
                false
            }
        };
        // The implicit drop afterwards finds a closed bracket and changes
        // nothing — explicit finish and silent teardown compose safely.
        if ended {
            self.controller
                .dispatch_notification(ScrollNotificationType::End, 0., 0.);
        }
        ended
    }
}

impl Drop for OwnedActivity {
    /// Silent teardown for an unfinished token: clears the bracket
    /// without notifying, but only while this token is still current —
    /// a stale drop can never cancel a newer activity.
    fn drop(&mut self) {
        let mut state = self.controller.state.borrow_mut();
        if state.activity_active && state.activity_generation == self.generation {
            state.activity_active = false;
        }
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
