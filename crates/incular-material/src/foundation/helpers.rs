//! Internal color helpers shared by Material foundation modules.

use incular_core::{Color, Lerp};

pub(in crate::foundation) fn alpha(color: Color, opacity: f32) -> Color {
    Color::rgba(
        color.red,
        color.green,
        color.blue,
        (color.alpha as f32 * opacity.clamp(0.0, 1.0)).round() as u8,
    )
}

pub(in crate::foundation) fn mix(a: Color, b: Color, amount: f32) -> Color {
    a.lerp(&b, amount.clamp(0.0, 1.0))
}
