//! Monotonic, runtime-driven animation primitives.
//!
//! Controllers carry no platform timer. The runtime supplies an [`Instant`]
//! once per requested frame, which makes deterministic tests straightforward.

use incular_core::Offset;
use std::{
    cell::RefCell,
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
pub enum Curve {
    Linear,
    EaseInOut,
}
impl Curve {
    #[must_use]
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::EaseInOut => t * t * (3.0 - 2.0 * t),
        }
    }
}

pub trait Tween: Copy {
    fn interpolate(from: Self, to: Self, t: f32) -> Self;
}
impl Tween for f32 {
    fn interpolate(from: Self, to: Self, t: f32) -> Self {
        from + (to - from) * t
    }
}
impl Tween for Offset {
    fn interpolate(from: Self, to: Self, t: f32) -> Self {
        Offset::new(
            f32::interpolate(from.x, to.x, t),
            f32::interpolate(from.y, to.y, t),
        )
    }
}

#[derive(Clone, Copy, Debug)]
struct State {
    value: f32,
    status: AnimationStatus,
    duration: Duration,
    started_at: Option<Instant>,
    from: f32,
    to: f32,
    curve: Curve,
}
impl Default for State {
    fn default() -> Self {
        Self {
            value: 0.0,
            status: AnimationStatus::Idle,
            duration: Duration::ZERO,
            started_at: None,
            from: 0.0,
            to: 1.0,
            curve: Curve::Linear,
        }
    }
}

/// Small cloneable controller. `forward`/`reverse` schedule values; callers do
/// not request frames themselves—the owning runtime observes `is_active`.
#[derive(Clone, Default)]
pub struct AnimationController {
    inner: Rc<RefCell<State>>,
}
impl AnimationController {
    #[must_use]
    pub fn new(duration: Duration) -> Self {
        let this = Self::default();
        this.inner.borrow_mut().duration = duration;
        this
    }
    pub fn forward(&self, now: Instant) {
        self.start(now, 1.0, AnimationStatus::Forward);
    }
    pub fn reverse(&self, now: Instant) {
        self.start(now, 0.0, AnimationStatus::Reverse);
    }
    pub fn stop(&self) {
        self.inner.borrow_mut().status = AnimationStatus::Stopped;
    }
    pub fn reset(&self) {
        let mut s = self.inner.borrow_mut();
        s.value = 0.;
        s.status = AnimationStatus::Dismissed;
        s.started_at = None;
    }
    fn start(&self, now: Instant, to: f32, status: AnimationStatus) {
        let mut s = self.inner.borrow_mut();
        s.from = s.value;
        s.to = to;
        s.status = status;
        s.started_at = Some(now);
    }
    /// Advances from an injected monotonic timestamp. Returns true if value or
    /// status changed and thus compositor state may need submission.
    pub fn tick(&self, now: Instant) -> bool {
        let mut s = self.inner.borrow_mut();
        if !matches!(
            s.status,
            AnimationStatus::Forward | AnimationStatus::Reverse
        ) {
            return false;
        }
        let started = s.started_at.expect("active animation has start time");
        let progress = if s.duration.is_zero() {
            1.0
        } else {
            now.saturating_duration_since(started).as_secs_f32() / s.duration.as_secs_f32()
        };
        let next = f32::interpolate(s.from, s.to, s.curve.apply(progress));
        let changed = next != s.value;
        s.value = next;
        if progress >= 1.0 {
            s.status = if s.to == 0.0 {
                AnimationStatus::Dismissed
            } else {
                AnimationStatus::Completed
            };
            s.started_at = None;
        }
        changed
    }
    #[must_use]
    pub fn value(&self) -> f32 {
        self.inner.borrow().value
    }
    #[must_use]
    pub fn status(&self) -> AnimationStatus {
        self.inner.borrow().status
    }
    #[must_use]
    pub fn is_active(&self) -> bool {
        matches!(
            self.status(),
            AnimationStatus::Forward | AnimationStatus::Reverse
        )
    }
    pub fn set_curve(&self, curve: Curve) {
        self.inner.borrow_mut().curve = curve;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deterministic_controller_ticks_without_sleeping() {
        let origin = Instant::now();
        let controller = AnimationController::new(Duration::from_millis(1000));
        controller.forward(origin);
        assert_eq!(controller.value(), 0.0);
        assert!(controller.tick(origin + Duration::from_millis(500)));
        assert_eq!(controller.value(), 0.5);
        assert!(controller.tick(origin + Duration::from_millis(1000)));
        assert_eq!(controller.value(), 1.0);
        assert!(!controller.is_active());
    }
}
