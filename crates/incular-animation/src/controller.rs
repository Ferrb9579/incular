//! Runtime-driven animation controllers.

use crate::{Curve, Tween};
use std::{
    cell::RefCell,
    fmt,
    rc::Rc,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AnimationStatus {
    #[default]
    Idle,
    Forward,
    Reverse,
    Completed,
    Dismissed,
    Stopped,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct RepeatState {
    reverse: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct State {
    value: f32,
    status: AnimationStatus,
    duration: Duration,
    lower_bound: f32,
    upper_bound: f32,
    started_at: Option<Instant>,
    from: f32,
    to: f32,
    curve: Curve,
    repeat: Option<RepeatState>,
    last_elapsed: Duration,
}

impl Default for State {
    fn default() -> Self {
        Self {
            value: 0.0,
            status: AnimationStatus::Idle,
            duration: Duration::ZERO,
            lower_bound: 0.0,
            upper_bound: 1.0,
            started_at: None,
            from: 0.0,
            to: 1.0,
            curve: Curve::Linear,
            repeat: None,
            last_elapsed: Duration::ZERO,
        }
    }
}

type Listener = Rc<dyn Fn(f32)>;

#[derive(Default)]
struct ControllerState {
    state: State,
    listeners: Vec<(usize, Listener)>,
    next_listener: usize,
}

/// A cloneable controller advanced by timestamps supplied by the runtime.
///
/// It intentionally owns no timer or event loop. `tick` is deterministic and
/// can be driven by a frame scheduler, a test clock, or a native animation
/// callback. Values are always kept inside the configured bounds.
#[derive(Clone, Default)]
pub struct AnimationController {
    inner: Rc<RefCell<ControllerState>>,
}

impl fmt::Debug for AnimationController {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = self.inner.borrow().state;
        formatter
            .debug_struct("AnimationController")
            .field("value", &state.value)
            .field("status", &state.status)
            .field("duration", &state.duration)
            .field("lower_bound", &state.lower_bound)
            .field("upper_bound", &state.upper_bound)
            .finish()
    }
}

impl AnimationController {
    #[must_use]
    pub fn new(duration: Duration) -> Self {
        Self::with_bounds(duration, 0.0, 1.0)
    }

    #[must_use]
    pub fn with_bounds(duration: Duration, lower_bound: f32, upper_bound: f32) -> Self {
        let (lower_bound, upper_bound) = normalize_bounds(lower_bound, upper_bound);
        let state = State {
            duration,
            lower_bound,
            upper_bound,
            value: lower_bound,
            from: lower_bound,
            to: upper_bound,
            ..State::default()
        };
        Self {
            inner: Rc::new(RefCell::new(ControllerState {
                state,
                ..ControllerState::default()
            })),
        }
    }

    #[must_use]
    pub fn duration(&self) -> Duration {
        self.inner.borrow().state.duration
    }

    pub fn set_duration(&self, duration: Duration) {
        self.inner.borrow_mut().state.duration = duration;
    }

    #[must_use]
    pub fn bounds(&self) -> (f32, f32) {
        let state = self.inner.borrow().state;
        (state.lower_bound, state.upper_bound)
    }

    pub fn set_bounds(&self, lower_bound: f32, upper_bound: f32) {
        let (lower_bound, upper_bound) = normalize_bounds(lower_bound, upper_bound);
        let (changed, listeners) = {
            let mut controller = self.inner.borrow_mut();
            let old = controller.state.value;
            controller.state.lower_bound = lower_bound;
            controller.state.upper_bound = upper_bound;
            controller.state.value = old.clamp(lower_bound, upper_bound);
            controller.state.from = controller.state.from.clamp(lower_bound, upper_bound);
            controller.state.to = controller.state.to.clamp(lower_bound, upper_bound);
            collect_listeners(&controller, controller.state.value, old)
        };
        notify(listeners, changed);
    }

    pub fn forward(&self, now: Instant) {
        let (from, to) = {
            let state = self.inner.borrow().state;
            (state.value, state.upper_bound)
        };
        self.start(now, from, to, AnimationStatus::Forward);
    }

    pub fn reverse(&self, now: Instant) {
        let (from, to) = {
            let state = self.inner.borrow().state;
            (state.value, state.lower_bound)
        };
        self.start(now, from, to, AnimationStatus::Reverse);
    }

    /// Drives to an arbitrary point in the configured bounds.
    pub fn animate_to(&self, now: Instant, target: f32) {
        let state = self.inner.borrow().state;
        let target = target.clamp(state.lower_bound, state.upper_bound);
        let direction = if target >= state.value {
            AnimationStatus::Forward
        } else {
            AnimationStatus::Reverse
        };
        let from = state.value;
        self.start(now, from, target, direction);
    }

    /// Alias for `animate_to`, useful when a caller semantically reverses to a
    /// custom point rather than all the way to the lower bound.
    pub fn animate_back(&self, now: Instant, target: f32) {
        self.animate_to(now, target);
    }

    /// Drives from the current value to `target`; this is a concise alias for
    /// [`Self::animate_to`] for callers building animation graphs.
    pub fn drive(&self, now: Instant, target: f32) {
        self.animate_to(now, target);
    }

    /// Drives an explicit interval, resetting the current value to `from`.
    pub fn drive_range(&self, now: Instant, from: f32, to: f32) {
        self.set_value(from);
        self.animate_to(now, to);
    }

    /// Starts a repeating lower-bound-to-upper-bound animation. When
    /// `reverse` is true, every other cycle runs backwards.
    pub fn repeat(&self, now: Instant, reverse: bool) {
        let (lower, upper) = self.bounds();
        {
            let mut controller = self.inner.borrow_mut();
            controller.state.value = lower;
            controller.state.from = lower;
            controller.state.to = upper;
            controller.state.started_at = Some(now);
            controller.state.last_elapsed = Duration::ZERO;
            controller.state.status = AnimationStatus::Forward;
            controller.state.repeat = Some(RepeatState { reverse });
        }
        self.notify_current();
    }

    /// Alias documenting that `repeat` remains active until stopped.
    pub fn repeat_forever(&self, now: Instant, reverse: bool) {
        self.repeat(now, reverse);
    }

    pub fn stop_repeat(&self) {
        self.inner.borrow_mut().state.repeat = None;
    }

    pub fn stop(&self) {
        let changed = {
            let mut controller = self.inner.borrow_mut();
            let changed = controller.state.status != AnimationStatus::Stopped;
            controller.state.status = AnimationStatus::Stopped;
            controller.state.started_at = None;
            controller.state.repeat = None;
            changed
        };
        if changed {
            self.notify_current();
        }
    }

    pub fn reset(&self) {
        let (value, listeners) = {
            let mut controller = self.inner.borrow_mut();
            let state = &mut controller.state;
            state.value = state.lower_bound;
            state.from = state.lower_bound;
            state.to = state.upper_bound;
            state.status = AnimationStatus::Dismissed;
            state.started_at = None;
            state.repeat = None;
            state.last_elapsed = Duration::ZERO;
            (state.value, clone_listeners(&controller))
        };
        notify(listeners, Some(value));
    }

    pub fn set_value(&self, value: f32) {
        let (changed, listeners) = {
            let mut controller = self.inner.borrow_mut();
            let state = &mut controller.state;
            let old_value = state.value;
            let next = if value.is_finite() {
                value.clamp(state.lower_bound, state.upper_bound)
            } else {
                state.lower_bound
            };
            let old_status = state.status;
            state.value = next;
            state.status = if next <= state.lower_bound {
                AnimationStatus::Dismissed
            } else if next >= state.upper_bound {
                AnimationStatus::Completed
            } else {
                AnimationStatus::Idle
            };
            state.started_at = None;
            state.repeat = None;
            let status_changed = old_status != state.status;
            let value_changed = next != old_value;
            (
                (value_changed || status_changed).then_some(next),
                clone_listeners(&controller),
            )
        };
        notify(listeners, changed);
    }

    /// Advances the controller and returns whether value or status changed.
    pub fn tick(&self, now: Instant) -> bool {
        let (changed, listeners, value) = {
            let mut controller = self.inner.borrow_mut();
            let (changed, value) = {
                let state = &mut controller.state;
                if !matches!(
                    state.status,
                    AnimationStatus::Forward | AnimationStatus::Reverse
                ) {
                    return false;
                }
                let started = state.started_at.unwrap_or(now);
                let elapsed = now.saturating_duration_since(started);
                state.last_elapsed = elapsed;
                let duration = state.duration;
                let old_value = state.value;
                let old_status = state.status;
                let raw_progress = if duration.is_zero() {
                    1.0
                } else {
                    elapsed.as_secs_f32() / duration.as_secs_f32()
                };

                if let Some(repeat) = state.repeat {
                    if duration.is_zero() {
                        state.value = if repeat.reverse {
                            state.lower_bound
                        } else {
                            state.upper_bound
                        };
                    } else {
                        let cycle = raw_progress.floor() as u64;
                        let mut local = raw_progress.fract();
                        let backwards = repeat.reverse && cycle % 2 == 1;
                        if backwards {
                            local = 1.0 - local;
                        }
                        let (from, to) = if backwards {
                            (state.upper_bound, state.lower_bound)
                        } else {
                            (state.lower_bound, state.upper_bound)
                        };
                        state.value = Tween::interpolate(from, to, state.curve.apply(local));
                        state.status = if backwards {
                            AnimationStatus::Reverse
                        } else {
                            AnimationStatus::Forward
                        };
                    }
                } else {
                    let progress = raw_progress.min(1.0);
                    state.value =
                        Tween::interpolate(state.from, state.to, state.curve.apply(progress));
                    if raw_progress >= 1.0 {
                        state.status = if state.to <= state.lower_bound {
                            AnimationStatus::Dismissed
                        } else {
                            AnimationStatus::Completed
                        };
                        state.started_at = None;
                    }
                }
                let changed = state.value != old_value || state.status != old_status;
                (changed, state.value)
            };
            (changed, clone_listeners(&controller), value)
        };
        if changed {
            notify(listeners, Some(value));
        }
        changed
    }
    #[must_use]
    pub fn value(&self) -> f32 {
        self.inner.borrow().state.value
    }

    #[must_use]
    pub fn status(&self) -> AnimationStatus {
        self.inner.borrow().state.status
    }

    #[must_use]
    pub fn is_active(&self) -> bool {
        matches!(
            self.status(),
            AnimationStatus::Forward | AnimationStatus::Reverse
        )
    }

    #[must_use]
    pub fn is_completed(&self) -> bool {
        self.status() == AnimationStatus::Completed
    }

    #[must_use]
    pub fn elapsed(&self) -> Duration {
        self.inner.borrow().state.last_elapsed
    }

    pub fn set_curve(&self, curve: Curve) {
        self.inner.borrow_mut().state.curve = curve;
    }

    #[must_use]
    pub fn curve(&self) -> Curve {
        self.inner.borrow().state.curve
    }

    pub fn add_listener(&self, listener: impl Fn(f32) + 'static) -> usize {
        let mut controller = self.inner.borrow_mut();
        let id = controller.next_listener;
        controller.next_listener = controller.next_listener.wrapping_add(1);
        controller.listeners.push((id, Rc::new(listener)));
        id
    }

    pub fn remove_listener(&self, id: usize) -> bool {
        let mut controller = self.inner.borrow_mut();
        let before = controller.listeners.len();
        controller
            .listeners
            .retain(|(listener_id, _)| *listener_id != id);
        before != controller.listeners.len()
    }

    fn start(&self, now: Instant, from: f32, to: f32, status: AnimationStatus) {
        let (from, to) = {
            let state = self.inner.borrow().state;
            (
                from.clamp(state.lower_bound, state.upper_bound),
                to.clamp(state.lower_bound, state.upper_bound),
            )
        };
        {
            let mut controller = self.inner.borrow_mut();
            let state = &mut controller.state;
            state.from = from;
            state.to = to;
            state.status = status;
            state.started_at = Some(now);
            state.last_elapsed = Duration::ZERO;
            state.repeat = None;
        }
        self.notify_current();
    }

    fn notify_current(&self) {
        let listeners = clone_listeners(&self.inner.borrow());
        notify(listeners, Some(self.value()));
    }
}

fn normalize_bounds(lower: f32, upper: f32) -> (f32, f32) {
    let lower = if lower.is_finite() { lower } else { 0.0 };
    let upper = if upper.is_finite() { upper } else { 1.0 };
    if lower <= upper {
        (lower, upper)
    } else {
        (upper, lower)
    }
}

fn collect_listeners(
    controller: &ControllerState,
    next: f32,
    old: f32,
) -> (Option<f32>, Vec<Listener>) {
    ((next != old).then_some(next), clone_listeners(controller))
}

fn clone_listeners(controller: &ControllerState) -> Vec<Listener> {
    controller
        .listeners
        .iter()
        .map(|(_, listener)| listener.clone())
        .collect()
}

fn notify(listeners: Vec<Listener>, value: Option<f32>) {
    if let Some(value) = value {
        for listener in listeners {
            listener(value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn bounds_and_drive_apis_are_deterministic() {
        let origin = Instant::now();
        let controller = AnimationController::with_bounds(Duration::from_secs(1), 10.0, 20.0);
        controller.animate_to(origin, 15.0);
        assert!(controller.tick(origin + Duration::from_millis(500)));
        assert_eq!(controller.value(), 12.5);
        assert!(controller.tick(origin + Duration::from_secs(1)));
        assert_eq!(controller.value(), 15.0);
        assert_eq!(controller.status(), AnimationStatus::Completed);
    }

    #[test]
    fn repeat_can_run_in_reverse_and_notifies() {
        let origin = Instant::now();
        let controller = AnimationController::new(Duration::from_secs(1));
        let calls = Rc::new(Cell::new(0));
        let calls_in_listener = calls.clone();
        let token = controller.add_listener(move |_| {
            calls_in_listener.set(calls_in_listener.get() + 1);
        });
        controller.repeat(origin, true);
        controller.tick(origin + Duration::from_millis(500));
        assert_eq!(controller.value(), 0.5);
        controller.tick(origin + Duration::from_millis(1500));
        assert_eq!(controller.value(), 0.5);
        assert!(calls.get() >= 2);
        assert!(controller.remove_listener(token));
    }

    #[test]
    fn zero_duration_completes_on_first_tick() {
        let origin = Instant::now();
        let controller = AnimationController::new(Duration::ZERO);
        controller.forward(origin);
        assert!(controller.tick(origin));
        assert_eq!(controller.value(), 1.0);
        assert_eq!(controller.status(), AnimationStatus::Completed);
    }
}
