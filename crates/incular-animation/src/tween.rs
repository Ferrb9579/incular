//! Generic interpolation and tween composition.

use crate::Curve;
use incular_core::{Color, Offset, Size};

/// Values that can be interpolated by an animation. The trait remains named
/// `Tween` for compatibility with the original crate API.
pub trait Tween: Copy {
    fn interpolate(from: Self, to: Self, t: f32) -> Self;

    #[must_use]
    fn lerp(from: Self, to: Self, t: f32) -> Self {
        Self::interpolate(from, to, t.clamp(0.0, 1.0))
    }
}

impl Tween for f32 {
    fn interpolate(from: Self, to: Self, t: f32) -> Self {
        from + (to - from) * t
    }
}

impl Tween for f64 {
    fn interpolate(from: Self, to: Self, t: f32) -> Self {
        from + (to - from) * f64::from(t)
    }
}

impl Tween for i32 {
    fn interpolate(from: Self, to: Self, t: f32) -> Self {
        (from as f32 + (to - from) as f32 * t).round() as i32
    }
}

impl Tween for u32 {
    fn interpolate(from: Self, to: Self, t: f32) -> Self {
        (from as f32 + (to as f32 - from as f32) * t)
            .round()
            .max(0.0) as u32
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

impl Tween for Size {
    fn interpolate(from: Self, to: Self, t: f32) -> Self {
        Size::new(
            f32::interpolate(from.width, to.width, t).max(0.0),
            f32::interpolate(from.height, to.height, t).max(0.0),
        )
    }
}

impl Tween for Color {
    fn interpolate(from: Self, to: Self, t: f32) -> Self {
        let mix = |left: u8, right: u8| {
            (f32::from(left) + (f32::from(right) - f32::from(left)) * t).round() as u8
        };
        Color::rgba(
            mix(from.red, to.red),
            mix(from.green, to.green),
            mix(from.blue, to.blue),
            mix(from.alpha, to.alpha),
        )
    }
}

/// A typed interval with an optional timing curve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TweenValue<T: Tween> {
    pub begin: T,
    pub end: T,
    pub curve: Curve,
}

impl<T: Tween> TweenValue<T> {
    #[must_use]
    pub const fn new(begin: T, end: T) -> Self {
        Self {
            begin,
            end,
            curve: Curve::Linear,
        }
    }

    #[must_use]
    pub const fn curved(begin: T, end: T, curve: Curve) -> Self {
        Self { begin, end, curve }
    }

    #[must_use]
    pub const fn curve(mut self, curve: Curve) -> Self {
        self.curve = curve;
        self
    }

    #[must_use]
    pub fn evaluate(self, progress: f32) -> T {
        T::interpolate(self.begin, self.end, self.curve.apply(progress))
    }

    #[must_use]
    pub fn lerp(self, progress: f32) -> T {
        T::interpolate(self.begin, self.end, progress.clamp(0.0, 1.0))
    }

    #[must_use]
    pub fn reversed(self) -> Self {
        Self {
            begin: self.end,
            end: self.begin,
            curve: self.curve.reverse(),
        }
    }

    #[must_use]
    pub fn then(self, next: Self) -> TweenSequence<T> {
        TweenSequence::new().push(self, 1.0).push(next, 1.0)
    }
}

/// A normalized weighted sequence of typed tweens.
#[derive(Clone, Debug, PartialEq)]
pub struct TweenSequence<T: Tween> {
    segments: Vec<TweenSegment<T>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TweenSegment<T: Tween> {
    pub tween: TweenValue<T>,
    pub weight: f32,
}

impl<T: Tween> TweenSequence<T> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            segments: Vec::new(),
        }
    }

    #[must_use]
    pub fn push(mut self, tween: TweenValue<T>, weight: f32) -> Self {
        if weight.is_finite() && weight > 0.0 {
            self.segments.push(TweenSegment { tween, weight });
        }
        self
    }

    #[must_use]
    pub fn with_segment(&mut self, tween: TweenValue<T>, weight: f32) -> &mut Self {
        if weight.is_finite() && weight > 0.0 {
            self.segments.push(TweenSegment { tween, weight });
        }
        self
    }

    #[must_use]
    pub fn evaluate(&self, progress: f32) -> Option<T> {
        let first = self.segments.first()?;
        let total = self
            .segments
            .iter()
            .map(|segment| segment.weight)
            .sum::<f32>();
        if total <= 0.0 || !total.is_finite() {
            return Some(first.tween.evaluate(0.0));
        }
        let target = progress.clamp(0.0, 1.0) * total;
        let mut cursor = 0.0;
        for (index, segment) in self.segments.iter().enumerate() {
            let end = cursor + segment.weight;
            if target <= end || index + 1 == self.segments.len() {
                let local = if segment.weight == 0.0 {
                    0.0
                } else {
                    ((target - cursor) / segment.weight).clamp(0.0, 1.0)
                };
                return Some(segment.tween.evaluate(local));
            }
            cursor = end;
        }
        Some(first.tween.evaluate(0.0))
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.segments.len()
    }

    #[must_use]
    pub fn segments(&self) -> &[TweenSegment<T>] {
        &self.segments
    }
}

impl<T: Tween> Default for TweenSequence<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use incular_core::Offset;

    #[test]
    fn typed_tween_interpolates_scalars_and_offsets() {
        assert_eq!(TweenValue::new(0.0_f32, 10.0).evaluate(0.25), 2.5);
        assert_eq!(
            TweenValue::new(Offset::ZERO, Offset::new(10.0, 20.0)).evaluate(0.5),
            Offset::new(5.0, 10.0)
        );
    }

    #[test]
    fn weighted_sequence_uses_local_progress() {
        let sequence = TweenValue::new(0.0_f32, 10.0).then(TweenValue::new(10.0, 20.0));
        assert_eq!(sequence.evaluate(0.25), Some(5.0));
        assert_eq!(sequence.evaluate(0.75), Some(15.0));
    }
}
