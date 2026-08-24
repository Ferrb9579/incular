//! Time-based UI physics simulations.

use std::time::Duration;

/// Tolerances used to decide when a physical simulation has settled.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tolerance {
    pub distance: f32,
    pub time: Duration,
    pub velocity: f32,
}

impl Tolerance {
    pub const DEFAULT: Self = Self {
        distance: 0.001,
        time: Duration::from_millis(1),
        velocity: 0.001,
    };

    #[must_use]
    pub const fn new(distance: f32, time: Duration, velocity: f32) -> Self {
        Self {
            distance,
            time,
            velocity,
        }
    }
}

impl Default for Tolerance {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Damping classification of a harmonic spring.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SpringType {
    CriticallyDamped,
    UnderDamped,
    OverDamped,
}

/// Physical parameters for a damped harmonic oscillator.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpringDescription {
    pub mass: f32,
    pub stiffness: f32,
    pub damping: f32,
}

impl SpringDescription {
    #[must_use]
    pub const fn new(mass: f32, stiffness: f32, damping: f32) -> Self {
        Self {
            mass: if mass > 0.0 { mass } else { 1.0 },
            stiffness: if stiffness > 0.0 { stiffness } else { 100.0 },
            damping: if damping >= 0.0 { damping } else { 10.0 },
        }
    }

    #[must_use]
    pub fn with_damping_ratio(mass: f32, stiffness: f32, ratio: f32) -> Self {
        let m = if mass > 0.0 { mass } else { 1.0 };
        let k = if stiffness > 0.0 { stiffness } else { 100.0 };
        let r = if ratio >= 0.0 { ratio } else { 1.0 };
        let damping = 2.0 * r * (m * k).sqrt();
        Self {
            mass: m,
            stiffness: k,
            damping,
        }
    }

    #[must_use]
    pub fn with_duration_and_bounce(duration: Duration, bounce: f32) -> Self {
        let duration_sec = duration.as_secs_f32().max(0.001);
        let mass = 1.0;
        let stiffness = (2.0 * std::f32::consts::PI / duration_sec).powi(2);
        let ratio = 1.0 - bounce.clamp(0.0, 1.0);
        Self::with_damping_ratio(mass, stiffness, ratio)
    }

    #[must_use]
    pub fn spring_type(&self) -> SpringType {
        let critical = 2.0 * (self.mass * self.stiffness).sqrt();
        let diff = self.damping - critical;
        if diff.abs() < 1e-4 {
            SpringType::CriticallyDamped
        } else if diff < 0.0 {
            SpringType::UnderDamped
        } else {
            SpringType::OverDamped
        }
    }
}

/// Abstract time-based scalar simulation.
pub trait Simulation {
    /// Computes position at elapsed time `t`.
    fn position(&self, t: Duration) -> f32;
    /// Computes velocity at elapsed time `t`.
    fn velocity(&self, t: Duration) -> f32;
    /// Checks whether the simulation has settled at elapsed time `t`.
    fn is_done(&self, t: Duration) -> bool;
}

/// A damped harmonic spring simulation.
#[derive(Clone, Debug, PartialEq)]
pub struct SpringSimulation {
    spring: SpringDescription,
    start: f32,
    end: f32,
    initial_velocity: f32,
    tolerance: Tolerance,
}

impl SpringSimulation {
    #[must_use]
    pub fn new(
        spring: SpringDescription,
        start: f32,
        end: f32,
        initial_velocity: f32,
        tolerance: Tolerance,
    ) -> Self {
        Self {
            spring,
            start,
            end,
            initial_velocity,
            tolerance,
        }
    }
}

impl Simulation for SpringSimulation {
    fn position(&self, t: Duration) -> f32 {
        let sec = t.as_secs_f32();
        let m = self.spring.mass;
        let k = self.spring.stiffness;
        let c = self.spring.damping;
        let x0 = self.start - self.end;
        let v0 = self.initial_velocity;

        let discr = c * c - 4.0 * m * k;
        let x = if discr.abs() < 1e-4 {
            // Critically damped
            let r = -c / (2.0 * m);
            (x0 + (v0 - r * x0) * sec) * (r * sec).exp()
        } else if discr < 0.0 {
            // Underdamped
            let alpha = -c / (2.0 * m);
            let omega = (-discr).sqrt() / (2.0 * m);
            let c1 = x0;
            let c2 = (v0 - alpha * x0) / omega;
            (alpha * sec).exp() * (c1 * (omega * sec).cos() + c2 * (omega * sec).sin())
        } else {
            // Overdamped
            let r1 = (-c - discr.sqrt()) / (2.0 * m);
            let r2 = (-c + discr.sqrt()) / (2.0 * m);
            let c2 = (v0 - r1 * x0) / (r2 - r1);
            let c1 = x0 - c2;
            c1 * (r1 * sec).exp() + c2 * (r2 * sec).exp()
        };

        self.end + x
    }

    fn velocity(&self, t: Duration) -> f32 {
        let dt = 0.001;
        let pos1 = self.position(t);
        let pos2 = self.position(t + Duration::from_secs_f32(dt));
        (pos2 - pos1) / dt
    }

    fn is_done(&self, t: Duration) -> bool {
        let pos = self.position(t);
        let vel = self.velocity(t);
        (pos - self.end).abs() <= self.tolerance.distance && vel.abs() <= self.tolerance.velocity
    }
}

/// Spring simulation tailored for scroll boundaries and overscroll recovery.
#[derive(Clone, Debug, PartialEq)]
pub struct ScrollSpringSimulation {
    inner: SpringSimulation,
}

impl ScrollSpringSimulation {
    #[must_use]
    pub fn new(
        spring: SpringDescription,
        start: f32,
        end: f32,
        initial_velocity: f32,
        tolerance: Tolerance,
    ) -> Self {
        Self {
            inner: SpringSimulation::new(spring, start, end, initial_velocity, tolerance),
        }
    }
}

impl Simulation for ScrollSpringSimulation {
    fn position(&self, t: Duration) -> f32 {
        self.inner.position(t)
    }

    fn velocity(&self, t: Duration) -> f32 {
        self.inner.velocity(t)
    }

    fn is_done(&self, t: Duration) -> bool {
        self.inner.is_done(t)
    }
}

/// Kinetic friction decay simulation.
#[derive(Clone, Debug, PartialEq)]
pub struct FrictionSimulation {
    drag: f32,
    start: f32,
    initial_velocity: f32,
    tolerance: Tolerance,
}

impl FrictionSimulation {
    #[must_use]
    pub fn new(drag: f32, start: f32, initial_velocity: f32, tolerance: Tolerance) -> Self {
        let drag = if drag > 0.0 { drag } else { 0.135 };
        Self {
            drag,
            start,
            initial_velocity,
            tolerance,
        }
    }
}

impl Simulation for FrictionSimulation {
    fn position(&self, t: Duration) -> f32 {
        let sec = t.as_secs_f32();
        self.start + self.initial_velocity * (self.drag.powf(sec) - 1.0) / self.drag.ln()
    }

    fn velocity(&self, t: Duration) -> f32 {
        let sec = t.as_secs_f32();
        self.initial_velocity * self.drag.powf(sec)
    }

    fn is_done(&self, t: Duration) -> bool {
        self.velocity(t).abs() <= self.tolerance.velocity
    }
}

/// Kinetic friction simulation clamped within `[min, max]`.
#[derive(Clone, Debug, PartialEq)]
pub struct BoundedFrictionSimulation {
    friction: FrictionSimulation,
    min: f32,
    max: f32,
}

impl BoundedFrictionSimulation {
    #[must_use]
    pub fn new(drag: f32, start: f32, initial_velocity: f32, min: f32, max: f32) -> Self {
        Self {
            friction: FrictionSimulation::new(drag, start, initial_velocity, Tolerance::DEFAULT),
            min,
            max,
        }
    }
}

impl Simulation for BoundedFrictionSimulation {
    fn position(&self, t: Duration) -> f32 {
        self.friction.position(t).clamp(self.min, self.max)
    }

    fn velocity(&self, t: Duration) -> f32 {
        let pos = self.friction.position(t);
        if pos <= self.min || pos >= self.max {
            0.0
        } else {
            self.friction.velocity(t)
        }
    }

    fn is_done(&self, t: Duration) -> bool {
        let pos = self.friction.position(t);
        pos <= self.min || pos >= self.max || self.friction.is_done(t)
    }
}

/// Constant gravitational acceleration simulation.
#[derive(Clone, Debug, PartialEq)]
pub struct GravitySimulation {
    acceleration: f32,
    start: f32,
    end: f32,
    initial_velocity: f32,
}

impl GravitySimulation {
    #[must_use]
    pub fn new(acceleration: f32, start: f32, end: f32, initial_velocity: f32) -> Self {
        Self {
            acceleration,
            start,
            end,
            initial_velocity,
        }
    }
}

impl Simulation for GravitySimulation {
    fn position(&self, t: Duration) -> f32 {
        let sec = t.as_secs_f32();
        self.start + self.initial_velocity * sec + 0.5 * self.acceleration * sec * sec
    }

    fn velocity(&self, t: Duration) -> f32 {
        let sec = t.as_secs_f32();
        self.initial_velocity + self.acceleration * sec
    }

    fn is_done(&self, t: Duration) -> bool {
        let pos = self.position(t);
        if self.acceleration >= 0.0 {
            pos >= self.end
        } else {
            pos <= self.end
        }
    }
}

/// Clamped simulation decorator bounding positions within `[min, max]`.
#[derive(Clone, Debug, PartialEq)]
pub struct ClampedSimulation<S: Simulation> {
    inner: S,
    min: f32,
    max: f32,
}

impl<S: Simulation> ClampedSimulation<S> {
    #[must_use]
    pub fn new(inner: S, min: f32, max: f32) -> Self {
        Self { inner, min, max }
    }
}

impl<S: Simulation> Simulation for ClampedSimulation<S> {
    fn position(&self, t: Duration) -> f32 {
        self.inner.position(t).clamp(self.min, self.max)
    }

    fn velocity(&self, t: Duration) -> f32 {
        let pos = self.inner.position(t);
        if pos <= self.min || pos >= self.max {
            0.0
        } else {
            self.inner.velocity(t)
        }
    }

    fn is_done(&self, t: Duration) -> bool {
        let pos = self.inner.position(t);
        pos <= self.min || pos >= self.max || self.inner.is_done(t)
    }
}
