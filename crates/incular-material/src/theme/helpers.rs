//! Internal Material color and normalization helpers shared by owner modules.

use incular_core::{Color, Lerp};

pub(crate) fn alpha(color: Color, opacity: f32) -> Color {
    Color::rgba(
        color.red,
        color.green,
        color.blue,
        (color.alpha as f32 * opacity.clamp(0.0, 1.0)).round() as u8,
    )
}

pub(crate) fn mix(a: Color, b: Color, amount: f32) -> Color {
    a.lerp(&b, amount.clamp(0.0, 1.0))
}

pub(crate) fn finite_non_negative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

pub(crate) fn with_alpha(color: Color, alpha: u8) -> Color {
    Color::rgba(color.red, color.green, color.blue, alpha)
}

pub(crate) fn normalized(value: Option<f32>, min: f32, max: f32) -> Option<f32> {
    let value = value?;
    if !value.is_finite() {
        return None;
    }
    let span = (max - min).max(f32::EPSILON);
    Some(((value - min) / span).clamp(0.0, 1.0))
}
