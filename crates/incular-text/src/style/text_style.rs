use super::fonts::{FontFamily, FontFeature, FontVariation, FontWeight};
use super::text::{
    FontStyle, LineHeight, TextBaseline, TextDecoration, TextDecorationStyle,
    TextLeadingDistribution, TextOverflow, TextScaler, TextShadow,
};
use incular_config::WidgetDefaults;
use incular_core::{ChangeImpact, Color, Lerp};
use std::sync::Arc;

/// An immutable, Flutter-style text style description.
#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    pub inherit: bool,
    pub has_explicit_color: bool,
    pub has_explicit_size: bool,
    pub has_explicit_weight: bool,
    pub has_explicit_style: bool,
    pub has_explicit_family: bool,
    pub family: FontFamily,
    pub fallback_families: Arc<[FontFamily]>,
    pub size: f32,
    pub weight: FontWeight,
    pub style: FontStyle,
    pub color: Color,
    pub background_color: Option<Color>,
    pub line_height: Option<LineHeight>,
    pub letter_spacing: f32,
    pub word_spacing: Option<f32>,
    pub text_baseline: Option<TextBaseline>,
    pub leading_distribution: Option<TextLeadingDistribution>,
    pub locale: Option<String>,
    pub foreground: Option<Color>,
    pub background: Option<Color>,
    pub shadows: Arc<[TextShadow]>,
    pub font_variations: Option<Arc<str>>,
    pub font_features: Option<Arc<str>>,
    pub decoration: Option<TextDecoration>,
    pub decoration_color: Option<Color>,
    pub decoration_style: Option<TextDecorationStyle>,
    pub decoration_thickness: Option<f32>,
    pub debug_label: Option<String>,
    pub overflow: Option<TextOverflow>,
}

impl TextStyle {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn inherit(mut self, inherit: bool) -> Self {
        self.inherit = inherit;
        self
    }

    #[must_use]
    pub fn font_family(mut self, family: impl Into<String>) -> Self {
        self.family = FontFamily::Named(family.into());
        self.has_explicit_family = true;
        self
    }

    #[must_use]
    pub fn family(mut self, family: FontFamily) -> Self {
        self.family = family;
        self.has_explicit_family = true;
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
        self.has_explicit_size = true;
        self
    }

    #[must_use]
    pub fn size(self, size: f32) -> Self {
        self.font_size(size)
    }

    #[must_use]
    pub fn font_weight(mut self, weight: FontWeight) -> Self {
        self.weight = weight;
        self.has_explicit_weight = true;
        self
    }

    #[must_use]
    pub fn bold(self) -> Self {
        self.font_weight(FontWeight::BOLD)
    }

    #[must_use]
    pub fn font_style(mut self, style: FontStyle) -> Self {
        self.style = style;
        self.has_explicit_style = true;
        self
    }

    #[must_use]
    pub fn italic(self) -> Self {
        self.font_style(FontStyle::Italic)
    }

    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self.foreground = None;
        self.has_explicit_color = true;
        self
    }

    #[must_use]
    pub fn background_color(mut self, color: Color) -> Self {
        self.background_color = Some(color);
        self.background = None;
        self
    }

    #[must_use]
    pub fn line_height(mut self, line_height: Option<f32>) -> Self {
        self.line_height = line_height.map(|value| {
            if value <= 4.0 {
                LineHeight::Multiplier(value.max(0.0))
            } else {
                LineHeight::Absolute(value.max(0.0))
            }
        });
        self
    }

    #[must_use]
    pub fn height(self, height: Option<f32>) -> Self {
        self.line_height(height)
    }

    #[must_use]
    pub fn line_height_multiplier(mut self, factor: f32) -> Self {
        self.line_height = Some(LineHeight::Multiplier(factor.max(0.0)));
        self
    }

    #[must_use]
    pub fn line_height_absolute(mut self, pixels: f32) -> Self {
        self.line_height = Some(LineHeight::Absolute(pixels.max(0.0)));
        self
    }

    #[must_use]
    pub fn line_height_mode(mut self, mode: LineHeight) -> Self {
        self.line_height = Some(mode);
        self
    }

    #[must_use]
    pub fn letter_spacing(mut self, letter_spacing: f32) -> Self {
        self.letter_spacing = letter_spacing;
        self
    }

    #[must_use]
    pub fn word_spacing(mut self, word_spacing: Option<f32>) -> Self {
        self.word_spacing = word_spacing;
        self
    }

    #[must_use]
    pub fn text_baseline(mut self, baseline: TextBaseline) -> Self {
        self.text_baseline = Some(baseline);
        self
    }

    #[must_use]
    pub fn leading_distribution(mut self, distribution: TextLeadingDistribution) -> Self {
        self.leading_distribution = Some(distribution);
        self
    }

    #[must_use]
    pub fn locale(mut self, locale: impl Into<String>) -> Self {
        self.locale = Some(locale.into());
        self
    }

    #[must_use]
    pub fn foreground(mut self, color: Color) -> Self {
        self.foreground = Some(color);
        self
    }

    #[must_use]
    pub fn background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self.background_color = None;
        self
    }

    #[must_use]
    pub fn shadows<I>(mut self, shadows: I) -> Self
    where
        I: IntoIterator<Item = TextShadow>,
    {
        self.shadows = shadows.into_iter().collect::<Vec<_>>().into();
        self
    }

    #[must_use]
    pub fn shadow(mut self, shadow: TextShadow) -> Self {
        let mut shadows = self.shadows.to_vec();
        shadows.push(shadow);
        self.shadows = shadows.into();
        self
    }

    #[must_use]
    pub fn font_variations(mut self, settings: impl Into<Arc<str>>) -> Self {
        self.font_variations = Some(settings.into());
        self
    }

    #[must_use]
    pub fn font_variation_settings(mut self, variations: &[FontVariation]) -> Self {
        let formatted = variations
            .iter()
            .map(FontVariation::to_css_setting)
            .collect::<Vec<_>>()
            .join(", ");
        self.font_variations = Some(formatted.into());
        self
    }

    #[must_use]
    pub fn font_features(mut self, settings: impl Into<Arc<str>>) -> Self {
        self.font_features = Some(settings.into());
        self
    }

    #[must_use]
    pub fn font_feature_settings(mut self, features: &[FontFeature]) -> Self {
        let formatted = features
            .iter()
            .map(FontFeature::to_css_setting)
            .collect::<Vec<_>>()
            .join(", ");
        self.font_features = Some(formatted.into());
        self
    }

    #[must_use]
    pub fn decoration(mut self, decoration: TextDecoration) -> Self {
        self.decoration = Some(decoration);
        self
    }

    #[must_use]
    pub fn decoration_color(mut self, color: Color) -> Self {
        self.decoration_color = Some(color);
        self
    }

    #[must_use]
    pub fn decoration_style(mut self, style: TextDecorationStyle) -> Self {
        self.decoration_style = Some(style);
        self
    }

    #[must_use]
    pub fn decoration_thickness(mut self, thickness: f32) -> Self {
        self.decoration_thickness = Some(thickness.max(0.0));
        self
    }

    #[must_use]
    pub fn debug_label(mut self, label: impl Into<String>) -> Self {
        self.debug_label = Some(label.into());
        self
    }

    #[must_use]
    pub fn overflow(mut self, overflow: TextOverflow) -> Self {
        self.overflow = Some(overflow);
        self
    }

    // Immutable derivation with_* methods
    #[must_use]
    pub fn with_font_size(&self, size: f32) -> Self {
        let mut s = self.clone();
        s.size = size.max(0.0);
        s
    }

    #[must_use]
    pub fn with_font_weight(&self, weight: FontWeight) -> Self {
        let mut s = self.clone();
        s.weight = weight;
        s
    }

    #[must_use]
    pub fn with_color(&self, color: Color) -> Self {
        let mut s = self.clone();
        s.color = color;
        s.has_explicit_color = true;
        s
    }

    #[must_use]
    pub fn with_letter_spacing(&self, letter_spacing: f32) -> Self {
        let mut s = self.clone();
        s.letter_spacing = letter_spacing;
        s
    }

    #[must_use]
    pub fn with_line_height(&self, line_height: Option<f32>) -> Self {
        let mut s = self.clone();
        s.line_height = line_height.map(|v| {
            if v <= 4.0 {
                LineHeight::Multiplier(v.max(0.0))
            } else {
                LineHeight::Absolute(v.max(0.0))
            }
        });
        s
    }

    #[must_use]
    pub fn with_decoration(&self, decoration: Option<TextDecoration>) -> Self {
        let mut s = self.clone();
        s.decoration = decoration;
        s
    }

    #[must_use]
    pub fn merge(&self, other: &Self) -> Self {
        if !other.inherit {
            return other.clone();
        }
        let mut merged = self.clone();
        if other.has_explicit_family {
            merged.family = other.family.clone();
            merged.has_explicit_family = true;
        }
        if !other.fallback_families.is_empty() {
            merged.fallback_families = other.fallback_families.clone();
        }
        if other.has_explicit_size {
            merged.size = other.size;
            merged.has_explicit_size = true;
        }
        if other.has_explicit_weight {
            merged.weight = other.weight;
            merged.has_explicit_weight = true;
        }
        if other.has_explicit_style {
            merged.style = other.style;
            merged.has_explicit_style = true;
        }
        if other.has_explicit_color || other.foreground.is_some() {
            merged.color = other.color;
            merged.has_explicit_color = true;
        }
        if other.background_color.is_some() {
            merged.background_color = other.background_color;
        }
        if other.line_height.is_some() {
            merged.line_height = other.line_height;
        }
        if other.letter_spacing != 0.0 {
            merged.letter_spacing = other.letter_spacing;
        }
        if other.word_spacing.is_some() {
            merged.word_spacing = other.word_spacing;
        }
        if other.text_baseline.is_some() {
            merged.text_baseline = other.text_baseline;
        }
        if other.leading_distribution.is_some() {
            merged.leading_distribution = other.leading_distribution;
        }
        if other.locale.is_some() {
            merged.locale = other.locale.clone();
        }
        if other.foreground.is_some() {
            merged.foreground = other.foreground;
        }
        if other.background.is_some() {
            merged.background = other.background;
        }
        if !other.shadows.is_empty() {
            merged.shadows = other.shadows.clone();
        }
        if other.font_variations.is_some() {
            merged.font_variations = other.font_variations.clone();
        }
        if other.font_features.is_some() {
            merged.font_features = other.font_features.clone();
        }
        if other.decoration.is_some() {
            merged.decoration = other.decoration;
        }
        if other.decoration_color.is_some() {
            merged.decoration_color = other.decoration_color;
        }
        if other.decoration_style.is_some() {
            merged.decoration_style = other.decoration_style;
        }
        if other.decoration_thickness.is_some() {
            merged.decoration_thickness = other.decoration_thickness;
        }
        if other.overflow.is_some() {
            merged.overflow = other.overflow;
        }
        merged
    }

    #[must_use]
    pub fn apply(
        &self,
        color: Option<Color>,
        background_color: Option<Color>,
        font_size_factor: Option<f32>,
        font_size_delta: Option<f32>,
        letter_spacing_factor: Option<f32>,
        letter_spacing_delta: Option<f32>,
    ) -> Self {
        let mut res = self.clone();
        if let Some(c) = color {
            res.color = c;
        }
        if let Some(bg) = background_color {
            res.background_color = Some(bg);
        }
        if let Some(factor) = font_size_factor {
            res.size = (res.size * factor).max(0.0);
        }
        if let Some(delta) = font_size_delta {
            res.size = (res.size + delta).max(0.0);
        }
        if let Some(factor) = letter_spacing_factor {
            res.letter_spacing *= factor;
        }
        if let Some(delta) = letter_spacing_delta {
            res.letter_spacing += delta;
        }
        res
    }

    #[must_use]
    pub fn scaled(&self, scaler: TextScaler) -> Self {
        let mut scaled = self.clone();
        scaled.size = scaler.scale(self.size);
        scaled
    }

    #[must_use]
    pub fn change_impact(&self, other: &Self) -> ChangeImpact {
        if self == other {
            return ChangeImpact::None;
        }
        if self.family != other.family
            || self.fallback_families != other.fallback_families
            || self.size != other.size
            || self.weight != other.weight
            || self.style != other.style
            || self.line_height != other.line_height
            || self.letter_spacing != other.letter_spacing
            || self.word_spacing != other.word_spacing
            || self.text_baseline != other.text_baseline
            || self.leading_distribution != other.leading_distribution
            || self.font_variations != other.font_variations
            || self.font_features != other.font_features
            || self.overflow != other.overflow
        {
            ChangeImpact::Layout
        } else {
            ChangeImpact::Paint
        }
    }

    #[must_use]
    pub fn invalidation(&self, other: &Self) -> incular_core::Invalidation {
        if self == other {
            return incular_core::Invalidation::NONE;
        }
        if self.family != other.family
            || self.fallback_families != other.fallback_families
            || self.size != other.size
            || self.weight != other.weight
            || self.style != other.style
            || self.line_height != other.line_height
            || self.letter_spacing != other.letter_spacing
            || self.word_spacing != other.word_spacing
            || self.text_baseline != other.text_baseline
            || self.leading_distribution != other.leading_distribution
            || self.font_variations != other.font_variations
            || self.font_features != other.font_features
            || self.overflow != other.overflow
        {
            incular_core::Invalidation::LAYOUT | incular_core::Invalidation::PAINT
        } else {
            incular_core::Invalidation::PAINT
        }
    }
}

impl Lerp for TextStyle {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let mix = |left: f32, right: f32| left + (right - left) * t;
        Self {
            inherit: if t < 0.5 { self.inherit } else { other.inherit },
            has_explicit_color: self.has_explicit_color || other.has_explicit_color,
            has_explicit_size: self.has_explicit_size || other.has_explicit_size,
            has_explicit_weight: self.has_explicit_weight || other.has_explicit_weight,
            has_explicit_style: self.has_explicit_style || other.has_explicit_style,
            has_explicit_family: self.has_explicit_family || other.has_explicit_family,
            family: if t < 0.5 {
                self.family.clone()
            } else {
                other.family.clone()
            },
            fallback_families: if t < 0.5 {
                self.fallback_families.clone()
            } else {
                other.fallback_families.clone()
            },
            size: mix(self.size, other.size),
            weight: self.weight.lerp(&other.weight, t),
            style: if t < 0.5 { self.style } else { other.style },
            color: self.color.lerp(&other.color, t),
            background_color: match (self.background_color, other.background_color) {
                (Some(a), Some(b)) => Some(a.lerp(&b, t)),
                (Some(a), None) => {
                    if t < 0.5 {
                        Some(a)
                    } else {
                        None
                    }
                }
                (None, Some(b)) => {
                    if t >= 0.5 {
                        Some(b)
                    } else {
                        None
                    }
                }
                (None, None) => None,
            },
            line_height: match (self.line_height, other.line_height) {
                (Some(LineHeight::Multiplier(a)), Some(LineHeight::Multiplier(b))) => {
                    Some(LineHeight::Multiplier(mix(a, b)))
                }
                (Some(LineHeight::Absolute(a)), Some(LineHeight::Absolute(b))) => {
                    Some(LineHeight::Absolute(mix(a, b)))
                }
                (Some(a), Some(b)) => {
                    if t < 0.5 {
                        Some(a)
                    } else {
                        Some(b)
                    }
                }
                (Some(a), None) => {
                    if t < 0.5 {
                        Some(a)
                    } else {
                        None
                    }
                }
                (None, Some(b)) => {
                    if t >= 0.5 {
                        Some(b)
                    } else {
                        None
                    }
                }
                (None, None) => None,
            },
            letter_spacing: mix(self.letter_spacing, other.letter_spacing),
            word_spacing: match (self.word_spacing, other.word_spacing) {
                (Some(left), Some(right)) => Some(mix(left, right)),
                (Some(left), None) => Some(mix(left, 0.0)),
                (None, Some(right)) => Some(mix(0.0, right)),
                (None, None) => None,
            },
            text_baseline: if t < 0.5 {
                self.text_baseline
            } else {
                other.text_baseline
            },
            leading_distribution: if t < 0.5 {
                self.leading_distribution
            } else {
                other.leading_distribution
            },
            locale: if t < 0.5 {
                self.locale.clone()
            } else {
                other.locale.clone()
            },
            foreground: match (self.foreground, other.foreground) {
                (Some(a), Some(b)) => Some(a.lerp(&b, t)),
                (Some(a), None) => {
                    if t < 0.5 {
                        Some(a)
                    } else {
                        None
                    }
                }
                (None, Some(b)) => {
                    if t >= 0.5 {
                        Some(b)
                    } else {
                        None
                    }
                }
                (None, None) => None,
            },
            background: match (self.background, other.background) {
                (Some(a), Some(b)) => Some(a.lerp(&b, t)),
                (Some(a), None) => {
                    if t < 0.5 {
                        Some(a)
                    } else {
                        None
                    }
                }
                (None, Some(b)) => {
                    if t >= 0.5 {
                        Some(b)
                    } else {
                        None
                    }
                }
                (None, None) => None,
            },
            shadows: if t < 0.5 {
                self.shadows.clone()
            } else {
                other.shadows.clone()
            },
            font_variations: if t < 0.5 {
                self.font_variations.clone()
            } else {
                other.font_variations.clone()
            },
            font_features: if t < 0.5 {
                self.font_features.clone()
            } else {
                other.font_features.clone()
            },
            decoration: if t < 0.5 {
                self.decoration
            } else {
                other.decoration
            },
            decoration_color: match (self.decoration_color, other.decoration_color) {
                (Some(a), Some(b)) => Some(a.lerp(&b, t)),
                (Some(a), None) => {
                    if t < 0.5 {
                        Some(a)
                    } else {
                        None
                    }
                }
                (None, Some(b)) => {
                    if t >= 0.5 {
                        Some(b)
                    } else {
                        None
                    }
                }
                (None, None) => None,
            },
            decoration_style: if t < 0.5 {
                self.decoration_style
            } else {
                other.decoration_style
            },
            decoration_thickness: match (self.decoration_thickness, other.decoration_thickness) {
                (Some(a), Some(b)) => Some(mix(a, b)),
                (Some(a), None) => Some(mix(a, 0.0)),
                (None, Some(b)) => Some(mix(0.0, b)),
                (None, None) => None,
            },
            debug_label: if t < 0.5 {
                self.debug_label.clone()
            } else {
                other.debug_label.clone()
            },
            overflow: if t < 0.5 {
                self.overflow
            } else {
                other.overflow
            },
        }
    }
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            inherit: true,
            has_explicit_color: false,
            has_explicit_size: false,
            has_explicit_weight: false,
            has_explicit_style: false,
            has_explicit_family: false,
            family: FontFamily::SystemUi,
            fallback_families: Arc::new([]),
            size: WidgetDefaults::DEFAULT.text_size,
            weight: FontWeight::NORMAL,
            style: FontStyle::Normal,
            color: Color::WHITE,
            background_color: None,
            line_height: None,
            letter_spacing: 0.0,
            word_spacing: None,
            text_baseline: None,
            leading_distribution: None,
            locale: None,
            foreground: None,
            background: None,
            shadows: Arc::new([]),
            font_variations: None,
            font_features: None,
            decoration: None,
            decoration_color: None,
            decoration_style: None,
            decoration_thickness: None,
            debug_label: None,
            overflow: None,
        }
    }
}
