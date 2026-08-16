//! Timing curves used by explicit and implicit animations.

/// A normalized timing function. Curves clamp input to `[0, 1]` and always
/// return a finite value, which keeps animation graphs stable when a frame is
/// late or an external progress value is invalid.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Curve {
    #[default]
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    EaseInCubic,
    EaseOutCubic,
    EaseInOutCubic,
    EaseInBack,
    EaseOutBack,
    EaseInOutBack,
    CubicBezier {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
    },
    StepStart,
    StepEnd,
}

impl Curve {
    #[must_use]
    pub fn apply(self, value: f32) -> f32 {
        let t = if value.is_finite() {
            value.clamp(0.0, 1.0)
        } else if value.is_sign_negative() {
            0.0
        } else {
            1.0
        };
        let result = match self {
            Self::Linear => t,
            Self::EaseIn => t * t,
            Self::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            Self::EaseInOut => t * t * (3.0 - 2.0 * t),
            Self::EaseInCubic => t * t * t,
            Self::EaseOutCubic => 1.0 - (1.0 - t).powi(3),
            Self::EaseInOutCubic => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
            Self::EaseInBack => {
                let c = 1.70158;
                (c + 1.0) * t * t * t - c * t * t
            }
            Self::EaseOutBack => {
                let c = 1.70158;
                let p = t - 1.0;
                1.0 + (c + 1.0) * p * p * p + c * p * p
            }
            Self::EaseInOutBack => {
                let c = 1.70158 * 1.525;
                let p = 2.0 * t;
                if p < 1.0 {
                    p * p * ((c + 1.0) * p - c) / 2.0
                } else {
                    let q = p - 2.0;
                    (q * q * ((c + 1.0) * q + c) + 2.0) / 2.0
                }
            }
            Self::CubicBezier { x1, y1, x2, y2 } => cubic_bezier(x1, y1, x2, y2, t),
            Self::StepStart => {
                if t <= 0.0 {
                    0.0
                } else {
                    1.0
                }
            }
            Self::StepEnd => {
                if t >= 1.0 {
                    1.0
                } else {
                    0.0
                }
            }
        };
        result.clamp(0.0, 1.0)
    }

    #[must_use]
    pub const fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Self::CubicBezier { x1, y1, x2, y2 }
    }

    #[must_use]
    pub const fn reverse(self) -> Self {
        match self {
            Self::Linear => Self::Linear,
            Self::EaseIn => Self::EaseOut,
            Self::EaseOut => Self::EaseIn,
            Self::EaseInOut => Self::EaseInOut,
            Self::EaseInCubic => Self::EaseOutCubic,
            Self::EaseOutCubic => Self::EaseInCubic,
            Self::EaseInOutCubic => Self::EaseInOutCubic,
            Self::EaseInBack => Self::EaseOutBack,
            Self::EaseOutBack => Self::EaseInBack,
            Self::EaseInOutBack => Self::EaseInOutBack,
            Self::CubicBezier { x1, y1, x2, y2 } => Self::CubicBezier {
                x1: 1.0 - x2,
                y1: 1.0 - y2,
                x2: 1.0 - x1,
                y2: 1.0 - y1,
            },
            Self::StepStart => Self::StepEnd,
            Self::StepEnd => Self::StepStart,
        }
    }

    #[must_use]
    pub const fn then(self, next: Self) -> CurveChain {
        CurveChain { first: self, next }
    }
}

/// Sequential curve composition. The first curve consumes the first half of
/// the timeline and the second curve consumes the second half.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveChain {
    first: Curve,
    next: Curve,
}

impl CurveChain {
    #[must_use]
    pub fn apply(self, value: f32) -> f32 {
        let value = if value.is_finite() {
            value.clamp(0.0, 1.0)
        } else if value.is_sign_negative() {
            0.0
        } else {
            1.0
        };
        if value < 0.5 {
            self.first.apply(value * 2.0) * 0.5
        } else {
            0.5 + self.next.apply((value - 0.5) * 2.0) * 0.5
        }
    }
}

/// Named curve constants, mirroring Flutter's `Curves` namespace while
/// keeping the actual curve a small value type.
pub struct Curves;

impl Curves {
    pub const LINEAR: Curve = Curve::Linear;
    pub const EASE_IN: Curve = Curve::EaseIn;
    pub const EASE_OUT: Curve = Curve::EaseOut;
    pub const EASE_IN_OUT: Curve = Curve::EaseInOut;
    pub const EASE_IN_CUBIC: Curve = Curve::EaseInCubic;
    pub const EASE_OUT_CUBIC: Curve = Curve::EaseOutCubic;
    pub const EASE_IN_OUT_CUBIC: Curve = Curve::EaseInOutCubic;
    pub const EASE_IN_BACK: Curve = Curve::EaseInBack;
    pub const EASE_OUT_BACK: Curve = Curve::EaseOutBack;
    pub const EASE_IN_OUT_BACK: Curve = Curve::EaseInOutBack;
}

fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, target: f32) -> f32 {
    let x1 = x1.clamp(0.0, 1.0);
    let x2 = x2.clamp(0.0, 1.0);
    let y1 = y1.clamp(-2.0, 2.0);
    let y2 = y2.clamp(-2.0, 2.0);
    let mut parameter = target;
    for _ in 0..8 {
        let x = bezier(parameter, x1, x2);
        let dx = bezier_derivative(parameter, x1, x2);
        if dx.abs() < 1.0e-5 {
            break;
        }
        parameter = (parameter - (x - target) / dx).clamp(0.0, 1.0);
    }
    bezier(parameter, y1, y2)
}

fn bezier(t: f32, first: f32, second: f32) -> f32 {
    let inverse = 1.0 - t;
    3.0 * inverse * inverse * t * first + 3.0 * inverse * t * t * second + t * t * t
}

fn bezier_derivative(t: f32, first: f32, second: f32) -> f32 {
    let inverse = 1.0 - t;
    3.0 * inverse * inverse * first
        + 6.0 * inverse * t * (second - first)
        + 3.0 * t * t * (1.0 - second)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curves_are_clamped_and_reversible() {
        assert_eq!(Curve::Linear.apply(-1.0), 0.0);
        assert_eq!(Curve::Linear.apply(2.0), 1.0);
        assert!((Curve::EaseIn.reverse().apply(0.25) - Curve::EaseOut.apply(0.25)).abs() < 1e-6);
    }

    #[test]
    fn chained_curves_preserve_endpoints() {
        let curve = Curve::EaseIn.then(Curve::EaseOut);
        assert_eq!(curve.apply(0.0), 0.0);
        assert_eq!(curve.apply(1.0), 1.0);
    }
}
