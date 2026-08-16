//! Typography values shared by plain and rich text.

use incular_core::Color;
use std::sync::Arc;

/// A logical font family. Named families are resolved by [`super::TextEngine`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FontFamily {
    SansSerif,
    Serif,
    Monospace,
    Cursive,
    SystemUi,
    Named(String),
}

impl FontFamily {
    #[must_use]
    pub fn named(name: impl Into<String>) -> Self {
        Self::Named(name.into())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FontWeight(pub u16);

impl FontWeight {
    pub const THIN: Self = Self(100);
    pub const EXTRA_LIGHT: Self = Self(200);
    pub const LIGHT: Self = Self(300);
    pub const NORMAL: Self = Self(400);
    pub const MEDIUM: Self = Self(500);
    pub const SEMI_BOLD: Self = Self(600);
    pub const BOLD: Self = Self(700);
    pub const EXTRA_BOLD: Self = Self(800);
    pub const BLACK: Self = Self(900);

    #[must_use]
    pub const fn new(value: u16) -> Self {
        Self(if value < 1 {
            1
        } else if value > 1000 {
            1000
        } else {
            value
        })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FontStyle {
    #[default]
    Normal,
    Italic,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextAlign {
    #[default]
    Start,
    Center,
    End,
}

/// Text scaling policy applied after a style's logical font size is chosen.
/// Keeping this separate from [`TextStyle`] lets the same style be rendered at
/// different accessibility scales without rebuilding a span tree.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextScaler {
    kind: TextScalerKind,
    factor: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextScalerKind {
    Linear,
    NoScaling,
}

impl TextScaler {
    pub const NO_SCALING: Self = Self {
        kind: TextScalerKind::NoScaling,
        factor: 1.0,
    };

    #[must_use]
    pub const fn linear(factor: f32) -> Self {
        // A const constructor cannot panic in a useful way on all supported
        // compilers; `scale` clamps invalid values defensively at use time.
        Self {
            kind: TextScalerKind::Linear,
            factor,
        }
    }

    #[must_use]
    pub const fn no_scaling() -> Self {
        Self::NO_SCALING
    }

    #[must_use]
    pub const fn factor(self) -> f32 {
        self.factor
    }

    #[must_use]
    pub const fn kind(self) -> TextScalerKind {
        self.kind
    }

    #[must_use]
    pub fn scale(self, font_size: f32) -> f32 {
        let factor = if self.factor.is_finite() && self.factor >= 0.0 {
            self.factor
        } else {
            1.0
        };
        (font_size.max(0.0) * factor).max(0.0)
    }

    #[must_use]
    pub fn is_no_scaling(self) -> bool {
        self.kind == TextScalerKind::NoScaling
    }
}

impl Default for TextScaler {
    fn default() -> Self {
        Self::NO_SCALING
    }
}

/// Font and paragraph-independent visual properties. The existing public
/// fields intentionally remain unchanged so current widget and renderer code
/// can continue constructing styles with struct literals.
#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    pub family: FontFamily,
    pub fallback_families: Arc<[FontFamily]>,
    pub size: f32,
    pub weight: FontWeight,
    pub style: FontStyle,
    pub color: Color,
    pub line_height: Option<f32>,
    pub letter_spacing: f32,
}

impl TextStyle {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn font_family(mut self, family: impl Into<String>) -> Self {
        self.family = FontFamily::Named(family.into());
        self
    }

    #[must_use]
    pub fn family(mut self, family: FontFamily) -> Self {
        self.family = family;
        self
    }

    #[must_use]
    pub fn fallback_families<I, S>(mut self, families: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.fallback_families = families
            .into_iter()
            .map(|name| FontFamily::Named(name.into()))
            .collect();
        self
    }

    #[must_use]
    pub fn fallback_family(mut self, family: FontFamily) -> Self {
        let mut families = self.fallback_families.to_vec();
        families.push(family);
        self.fallback_families = families.into();
        self
    }

    #[must_use]
    pub fn font_size(mut self, size: f32) -> Self {
        self.size = size.max(0.0);
        self
    }

    #[must_use]
    pub fn font_weight(mut self, weight: FontWeight) -> Self {
        self.weight = weight;
        self
    }

    #[must_use]
    pub fn bold(mut self) -> Self {
        self.weight = FontWeight::BOLD;
        self
    }

    #[must_use]
    pub fn italic(mut self) -> Self {
        self.style = FontStyle::Italic;
        self
    }

    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    #[must_use]
    pub fn line_height(mut self, line_height: Option<f32>) -> Self {
        self.line_height = line_height.map(|value| value.max(0.0));
        self
    }

    #[must_use]
    pub fn letter_spacing(mut self, letter_spacing: f32) -> Self {
        self.letter_spacing = letter_spacing;
        self
    }

    /// Returns a style with values from `other` replacing this style where
    /// `other` has a meaningful value. This is useful for inherited spans.
    /// Since the historical `TextStyle` stores concrete values, zero/`None`
    /// are treated as the absence of an override for metric fields.
    #[must_use]
    pub fn merge(&self, other: &Self) -> Self {
        let mut merged = self.clone();
        merged.family = other.family.clone();
        if !other.fallback_families.is_empty() {
            merged.fallback_families = other.fallback_families.clone();
        }
        if other.size != 0.0 {
            merged.size = other.size;
        }
        merged.weight = other.weight;
        merged.style = other.style;
        merged.color = other.color;
        if other.line_height.is_some() {
            merged.line_height = other.line_height;
        }
        if other.letter_spacing != 0.0 {
            merged.letter_spacing = other.letter_spacing;
        }
        merged
    }

    #[must_use]
    pub fn scaled(&self, scaler: TextScaler) -> Self {
        let mut scaled = self.clone();
        scaled.size = scaler.scale(self.size);
        scaled
    }

    #[must_use]
    pub fn lerp(a: &Self, b: &Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let mix = |left: f32, right: f32| left + (right - left) * t;
        let color = Color::rgba(
            mix(a.color.red as f32, b.color.red as f32).round() as u8,
            mix(a.color.green as f32, b.color.green as f32).round() as u8,
            mix(a.color.blue as f32, b.color.blue as f32).round() as u8,
            mix(a.color.alpha as f32, b.color.alpha as f32).round() as u8,
        );
        Self {
            family: if t < 0.5 {
                a.family.clone()
            } else {
                b.family.clone()
            },
            fallback_families: if t < 0.5 {
                a.fallback_families.clone()
            } else {
                b.fallback_families.clone()
            },
            size: mix(a.size, b.size),
            weight: if t < 0.5 { a.weight } else { b.weight },
            style: if t < 0.5 { a.style } else { b.style },
            color,
            line_height: match (a.line_height, b.line_height) {
                (Some(left), Some(right)) => Some(mix(left, right)),
                (Some(left), None) => Some(mix(left, 0.0)),
                (None, Some(right)) => Some(mix(0.0, right)),
                (None, None) => None,
            },
            letter_spacing: mix(a.letter_spacing, b.letter_spacing),
        }
    }
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            family: FontFamily::SystemUi,
            fallback_families: Arc::new([]),
            size: 16.0,
            weight: FontWeight::NORMAL,
            style: FontStyle::Normal,
            color: Color::WHITE,
            line_height: None,
            letter_spacing: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaler_is_deterministic_and_defensive() {
        assert_eq!(TextScaler::linear(1.5).scale(16.0), 24.0);
        assert_eq!(TextScaler::no_scaling().scale(16.0), 16.0);
        assert_eq!(TextScaler::linear(-1.0).scale(16.0), 16.0);
    }

    #[test]
    fn style_lerp_interpolates_metrics_and_color() {
        let first = TextStyle::default().font_size(10.0).color(Color::BLACK);
        let second = TextStyle::default().font_size(20.0).color(Color::WHITE);
        let middle = TextStyle::lerp(&first, &second, 0.5);
        assert_eq!(middle.size, 15.0);
        assert_eq!(middle.color, Color::rgba(128, 128, 128, 255));
    }
}
