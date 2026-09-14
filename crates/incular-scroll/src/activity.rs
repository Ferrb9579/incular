use crate::{
    controller::{ScrollController, ScrollState},
    notifications::ScrollNotificationType,
    restoration::persist_scroll_offset,
};

/// Advances the activity identity without reuse: exhaustion panics
/// explicitly rather than wrapping, so a generation can never alias an
/// earlier activity — the same policy as attachment identities. The
/// 64-bit space makes exhaustion unreachable in practice; the panic
/// exists so wraparound can never silently break stale-token safety.
fn next_activity_generation(state: &mut ScrollState) -> u64 {
    match state.activity_generation.checked_add(1) {
        Some(next) => {
            state.activity_generation = next;
            next
        }
        None => {
            panic!("scroll activity identity space exhausted: refusing to reuse a generation")
        }
    }
}

/// Which driver owns an activity token. Metric-attachment ownership
/// stays separate: this names the input/animation driver holding the
/// bracket, never who may publish geometry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ActivityOrigin {
    /// A scrollbar thumb drag owns the bracket.
    Scrollbar,
    /// One wheel/trackpad sample owns its complete bracket.
    Wheel,
    /// An accepted touch drag owns the bracket for its stream.
    Drag,
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
                next_activity_generation(&mut state);
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
            let generation = next_activity_generation(&mut state);
            (started, generation)
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

/// Snapshot of one owner-checked drive: the offset and revision the
/// driver itself committed, captured under the state lock before any
/// notification ran. Drivers record this — never a post-callback read —
/// as the expected position, so application changes made by `Update`
/// listeners cannot be mistaken for driver motion on the next step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OwnedDrive {
    /// Committed offset immediately after this drive's own write.
    pub offset: f32,
    /// State revision immediately after this drive's own write.
    pub revision: u64,
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

    /// Applies `delta` to the bracketed controller's offset — clamped to
    /// the current content bounds with the same commit semantics as
    /// [`ScrollController::jump_to`] — but only while this token still
    /// owns the open bracket.
    ///
    /// The commit (offset plus revision) is captured under the state
    /// lock and returned as an [`OwnedDrive`] snapshot before the
    /// `Update` notification runs. Callers verify tenure after the
    /// callbacks return: the token still current, and offset plus
    /// revision still equal to the snapshot. A listener jump, bounds
    /// change, or takeover is then visible as a post-callback
    /// difference — never absorbed into the driver's expected position.
    ///
    /// Returns `None` without touching anything when this token is
    /// stale (a newer activity took over, or the bracket closed). A
    /// same-value drive is a no-op like [`ScrollController::jump_to`]:
    /// the unchanged snapshot, no revision bump, no notification — so a
    /// listener jumping to the already-current position never disturbs
    /// the tenure. Non-finite deltas likewise drive nothing.
    pub fn drive(&self, delta: f32) -> Option<OwnedDrive> {
        let (committed, persistence, applied) = {
            let mut state = self.controller.state.borrow_mut();
            if !(state.activity_active && state.activity_generation == self.generation) {
                return None;
            }
            let snapshot = || OwnedDrive {
                offset: state.offset,
                revision: state.revision,
            };
            if !delta.is_finite() {
                return Some(snapshot());
            }
            let value = (state.offset + delta).clamp(0., state.max_offset);
            if value == state.offset {
                return Some(snapshot());
            }
            let previous = state.offset;
            state.offset = value;
            state.pending_restored_offset = None;
            state.revision += 1;
            (
                OwnedDrive {
                    offset: value,
                    revision: state.revision,
                },
                state.restoration.clone(),
                value - previous,
            )
        };
        persist_scroll_offset(persistence, committed.offset);
        self.controller
            .dispatch_notification(ScrollNotificationType::Update, applied, 0.);
        Some(committed)
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
