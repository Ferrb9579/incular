//! Timing curves used by explicit and implicit animations.

/// A normalized timing function. Curves clamp input to `[0, 1]` and always
/// return a finite value, which keeps animation graphs stable when a frame is
/// late or an external progress value is invalid.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Curve {
    #[default]
    Linear,
    Ease,
    EaseIn,
    EaseOut,
    EaseInOut,
    EaseInSine,
    EaseOutSine,
    EaseInOutSine,
    EaseInQuad,
    EaseOutQuad,
    EaseInOutQuad,
    EaseInCubic,
    EaseOutCubic,
    EaseInOutCubic,
    EaseInQuart,
    EaseOutQuart,
    EaseInOutQuart,
    EaseInQuint,
    EaseOutQuint,
    EaseInOutQuint,
    EaseInExpo,
    EaseOutExpo,
    EaseInOutExpo,
    EaseInCirc,
    EaseOutCirc,
    EaseInOutCirc,
    EaseInBack,
    EaseOutBack,
    EaseInOutBack,
    EaseInElastic,
    EaseOutElastic,
    EaseInOutElastic,
    EaseInBounce,
    EaseOutBounce,
    EaseInOutBounce,
    FastOutSlowIn,
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
        let pi = std::f32::consts::PI;
        let result = match self {
            Self::Linear => t,
            Self::Ease => cubic_bezier(0.25, 0.1, 0.25, 1.0, t),
            Self::EaseIn => t * t,
            Self::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            Self::EaseInOut => t * t * (3.0 - 2.0 * t),
            Self::EaseInSine => 1.0 - (t * pi / 2.0).cos(),
            Self::EaseOutSine => (t * pi / 2.0).sin(),
            Self::EaseInOutSine => -((pi * t).cos() - 1.0) / 2.0,
            Self::EaseInQuad => t * t,
            Self::EaseOutQuad => 1.0 - (1.0 - t) * (1.0 - t),
            Self::EaseInOutQuad => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
                }
            }
            Self::EaseInCubic => t * t * t,
            Self::EaseOutCubic => 1.0 - (1.0 - t).powi(3),
            Self::EaseInOutCubic => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
            Self::EaseInQuart => t * t * t * t,
            Self::EaseOutQuart => 1.0 - (1.0 - t).powi(4),
            Self::EaseInOutQuart => {
                if t < 0.5 {
                    8.0 * t * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(4) / 2.0
                }
            }
            Self::EaseInQuint => t.powi(5),
            Self::EaseOutQuint => 1.0 - (1.0 - t).powi(5),
            Self::EaseInOutQuint => {
                if t < 0.5 {
                    16.0 * t.powi(5)
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(5) / 2.0
                }
            }
            Self::EaseInExpo => {
                if t == 0.0 {
                    0.0
                } else {
                    (2.0f32).powf(10.0 * t - 10.0)
                }
            }
            Self::EaseOutExpo => {
                if t == 1.0 {
                    1.0
                } else {
                    1.0 - (2.0f32).powf(-10.0 * t)
                }
            }
            Self::EaseInOutExpo => {
                if t == 0.0 {
                    0.0
                } else if t == 1.0 {
                    1.0
                } else if t < 0.5 {
                    (2.0f32).powf(20.0 * t - 10.0) / 2.0
                } else {
                    (2.0 - (2.0f32).powf(-20.0 * t + 10.0)) / 2.0
                }
            }
            Self::EaseInCirc => 1.0 - (1.0 - t * t).max(0.0).sqrt(),
            Self::EaseOutCirc => (1.0 - (t - 1.0).powi(2)).max(0.0).sqrt(),
            Self::EaseInOutCirc => {
                if t < 0.5 {
                    (1.0 - (1.0 - 4.0 * t * t).max(0.0).sqrt()) / 2.0
                } else {
                    ((1.0 - (-2.0 * t + 2.0).powi(2)).max(0.0).sqrt() + 1.0) / 2.0
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
            Self::EaseInElastic => {
                let c = (2.0 * pi) / 3.0;
                if t == 0.0 {
                    0.0
                } else if t == 1.0 {
                    1.0
                } else {
                    -(2.0f32).powf(10.0 * t - 10.0) * ((t * 10.0 - 10.75) * c).sin()
                }
            }
            Self::EaseOutElastic => {
                let c = (2.0 * pi) / 3.0;
                if t == 0.0 {
                    0.0
                } else if t == 1.0 {
                    1.0
                } else {
                    (2.0f32).powf(-10.0 * t) * ((t * 10.0 - 0.75) * c).sin() + 1.0
                }
            }
            Self::EaseInOutElastic => {
                let c = (2.0 * pi) / 4.5;
                if t == 0.0 {
                    0.0
                } else if t == 1.0 {
                    1.0
                } else if t < 0.5 {
                    -((2.0f32).powf(20.0 * t - 10.0) * ((20.0 * t - 11.125) * c).sin()) / 2.0
                } else {
                    ((2.0f32).powf(-20.0 * t + 10.0) * ((20.0 * t - 11.125) * c).sin()) / 2.0 + 1.0
                }
            }
            Self::EaseInBounce => 1.0 - bounce_out(1.0 - t),
            Self::EaseOutBounce => bounce_out(t),
            Self::EaseInOutBounce => {
                if t < 0.5 {
                    (1.0 - bounce_out(1.0 - 2.0 * t)) / 2.0
                } else {
                    (1.0 + bounce_out(2.0 * t - 1.0)) / 2.0
                }
            }
            Self::FastOutSlowIn => cubic_bezier(0.4, 0.0, 0.2, 1.0, t),
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
            Self::Ease => Self::Ease,
            Self::EaseIn => Self::EaseOut,
            Self::EaseOut => Self::EaseIn,
            Self::EaseInOut => Self::EaseInOut,
            Self::EaseInSine => Self::EaseOutSine,
            Self::EaseOutSine => Self::EaseInSine,
            Self::EaseInOutSine => Self::EaseInOutSine,
            Self::EaseInQuad => Self::EaseOutQuad,
            Self::EaseOutQuad => Self::EaseInQuad,
            Self::EaseInOutQuad => Self::EaseInOutQuad,
            Self::EaseInCubic => Self::EaseOutCubic,
            Self::EaseOutCubic => Self::EaseInCubic,
            Self::EaseInOutCubic => Self::EaseInOutCubic,
            Self::EaseInQuart => Self::EaseOutQuart,
            Self::EaseOutQuart => Self::EaseInQuart,
            Self::EaseInOutQuart => Self::EaseInOutQuart,
            Self::EaseInQuint => Self::EaseOutQuint,
            Self::EaseOutQuint => Self::EaseInQuint,
            Self::EaseInOutQuint => Self::EaseInOutQuint,
            Self::EaseInExpo => Self::EaseOutExpo,
            Self::EaseOutExpo => Self::EaseInExpo,
            Self::EaseInOutExpo => Self::EaseInOutExpo,
            Self::EaseInCirc => Self::EaseOutCirc,
            Self::EaseOutCirc => Self::EaseInCirc,
            Self::EaseInOutCirc => Self::EaseInOutCirc,
            Self::EaseInBack => Self::EaseOutBack,
            Self::EaseOutBack => Self::EaseInBack,
            Self::EaseInOutBack => Self::EaseInOutBack,
            Self::EaseInElastic => Self::EaseOutElastic,
            Self::EaseOutElastic => Self::EaseInElastic,
            Self::EaseInOutElastic => Self::EaseInOutElastic,
            Self::EaseInBounce => Self::EaseOutBounce,
            Self::EaseOutBounce => Self::EaseInBounce,
            Self::EaseInOutBounce => Self::EaseInOutBounce,
            Self::FastOutSlowIn => Self::FastOutSlowIn,
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

fn bounce_out(mut t: f32) -> f32 {
    let n1 = 7.5625;
    let d1 = 2.75;
    if t < 1.0 / d1 {
        n1 * t * t
    } else if t < 2.0 / d1 {
        t -= 1.5 / d1;
        n1 * t * t + 0.75
    } else if t < 2.5 / d1 {
        t -= 2.25 / d1;
        n1 * t * t + 0.9375
    } else {
        t -= 2.625 / d1;
        n1 * t * t + 0.984375
    }
}

/// Sequential curve composition.
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

/// Named curve constants, mirroring Flutter's `Curves` namespace.
pub struct Curves;

impl Curves {
    pub const LINEAR: Curve = Curve::Linear;
    pub const EASE: Curve = Curve::Ease;
    pub const EASE_IN: Curve = Curve::EaseIn;
    pub const EASE_OUT: Curve = Curve::EaseOut;
    pub const EASE_IN_OUT: Curve = Curve::EaseInOut;
    pub const EASE_IN_SINE: Curve = Curve::EaseInSine;
    pub const EASE_OUT_SINE: Curve = Curve::EaseOutSine;
    pub const EASE_IN_OUT_SINE: Curve = Curve::EaseInOutSine;
    pub const EASE_IN_QUAD: Curve = Curve::EaseInQuad;
    pub const EASE_OUT_QUAD: Curve = Curve::EaseOutQuad;
    pub const EASE_IN_OUT_QUAD: Curve = Curve::EaseInOutQuad;
    pub const EASE_IN_CUBIC: Curve = Curve::EaseInCubic;
    pub const EASE_OUT_CUBIC: Curve = Curve::EaseOutCubic;
    pub const EASE_IN_OUT_CUBIC: Curve = Curve::EaseInOutCubic;
    pub const EASE_IN_QUART: Curve = Curve::EaseInQuart;
    pub const EASE_OUT_QUART: Curve = Curve::EaseOutQuart;
    pub const EASE_IN_OUT_QUART: Curve = Curve::EaseInOutQuart;
    pub const EASE_IN_QUINT: Curve = Curve::EaseInQuint;
    pub const EASE_OUT_QUINT: Curve = Curve::EaseOutQuint;
    pub const EASE_IN_OUT_QUINT: Curve = Curve::EaseInOutQuint;
    pub const EASE_IN_EXPO: Curve = Curve::EaseInExpo;
    pub const EASE_OUT_EXPO: Curve = Curve::EaseOutExpo;
    pub const EASE_IN_OUT_EXPO: Curve = Curve::EaseInOutExpo;
    pub const EASE_IN_CIRC: Curve = Curve::EaseInCirc;
    pub const EASE_OUT_CIRC: Curve = Curve::EaseOutCirc;
    pub const EASE_IN_OUT_CIRC: Curve = Curve::EaseInOutCirc;
    pub const EASE_IN_BACK: Curve = Curve::EaseInBack;
    pub const EASE_OUT_BACK: Curve = Curve::EaseOutBack;
    pub const EASE_IN_OUT_BACK: Curve = Curve::EaseInOutBack;
    pub const EASE_IN_ELASTIC: Curve = Curve::EaseInElastic;
    pub const EASE_OUT_ELASTIC: Curve = Curve::EaseOutElastic;
    pub const EASE_IN_OUT_ELASTIC: Curve = Curve::EaseInOutElastic;
    pub const EASE_IN_BOUNCE: Curve = Curve::EaseInBounce;
    pub const EASE_OUT_BOUNCE: Curve = Curve::EaseOutBounce;
    pub const EASE_IN_OUT_BOUNCE: Curve = Curve::EaseInOutBounce;
    pub const BOUNCE_OUT: Curve = Curve::EaseOutBounce;
    pub const BOUNCE_IN: Curve = Curve::EaseInBounce;
    pub const BOUNCE_IN_OUT: Curve = Curve::EaseInOutBounce;
    pub const ELASTIC_OUT: Curve = Curve::EaseOutElastic;
    pub const ELASTIC_IN: Curve = Curve::EaseInElastic;
    pub const ELASTIC_IN_OUT: Curve = Curve::EaseInOutElastic;
    pub const FAST_OUT_SLOW_IN: Curve = Curve::FastOutSlowIn;
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
