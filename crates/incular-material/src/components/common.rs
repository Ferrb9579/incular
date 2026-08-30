use incular_core::Color;

pub(super) fn finite_non_negative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

pub(super) fn with_alpha(color: Color, alpha: u8) -> Color {
    Color::rgba(color.red, color.green, color.blue, alpha)
}

pub(super) fn normalized(value: Option<f32>, min: f32, max: f32) -> Option<f32> {
    let value = value?;
    if !value.is_finite() {
        return None;
    }
    let span = (max - min).max(f32::EPSILON);
    Some(((value - min) / span).clamp(0.0, 1.0))
}
