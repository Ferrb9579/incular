//! Typography values shared by plain and rich text.

use incular_core::{ChangeImpact, Color, Lerp};
use incular_rendering::Shadow;
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

/// Font weight values (1..1000).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FontWeight(pub u16);

impl FontWeight {
    pub const W100: Self = Self(100);
    pub const W200: Self = Self(200);
    pub const W300: Self = Self(300);
    pub const W400: Self = Self(400);
    pub const W500: Self = Self(500);
    pub const W600: Self = Self(600);
    pub const W700: Self = Self(700);
    pub const W800: Self = Self(800);
    pub const W900: Self = Self(900);

    pub const THIN: Self = Self::W100;
    pub const EXTRA_LIGHT: Self = Self::W200;
    pub const LIGHT: Self = Self::W300;
    pub const NORMAL: Self = Self::W400;
    pub const MEDIUM: Self = Self::W500;
    pub const SEMI_BOLD: Self = Self::W600;
    pub const BOLD: Self = Self::W700;
    pub const EXTRA_BOLD: Self = Self::W800;
    pub const BLACK: Self = Self::W900;

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

    #[must_use]
    pub const fn value(self) -> u16 {
        self.0
    }
}

impl Lerp for FontWeight {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let left = self.0 as f32;
        let right = other.0 as f32;
        Self::new((left + (right - left) * t).round() as u16)
    }
}

/// OpenType font feature setting with a 4-byte ASCII tag and an integer value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FontFeature {
    pub tag: [u8; 4],
    pub value: i32,
}

impl FontFeature {
    #[must_use]
    pub const fn new(tag: [u8; 4], value: i32) -> Self {
        Self { tag, value }
    }

    #[must_use]
    pub const fn enable(tag: [u8; 4]) -> Self {
        Self::new(tag, 1)
    }

    #[must_use]
    pub const fn disable(tag: [u8; 4]) -> Self {
        Self::new(tag, 0)
    }

    #[must_use]
    pub const fn alternative(value: i32) -> Self {
        Self::new(*b"aalt", value)
    }

    #[must_use]
    pub const fn contextual_alternates() -> Self {
        Self::enable(*b"calt")
    }

    #[must_use]
    pub const fn tabular_figures() -> Self {
        Self::enable(*b"tnum")
    }

    #[must_use]
    pub const fn proportional_figures() -> Self {
        Self::enable(*b"pnum")
    }

    #[must_use]
    pub const fn slashed_zero() -> Self {
        Self::enable(*b"zero")
    }

    #[must_use]
    pub fn to_css_setting(&self) -> String {
        let tag_str = std::str::from_utf8(&self.tag).unwrap_or("????");
        format!("\"{}\" {}", tag_str, self.value)
    }
}

/// OpenType font variation axis setting with a 4-byte ASCII tag and floating-point value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FontVariation {
    pub axis: [u8; 4],
    pub value: f32,
}

impl FontVariation {
    #[must_use]
    pub const fn new(axis: [u8; 4], value: f32) -> Self {
        Self { axis, value }
    }

    #[must_use]
    pub const fn weight(value: f32) -> Self {
        Self::new(*b"wght", value)
    }

    #[must_use]
    pub const fn italic(value: f32) -> Self {
        Self::new(*b"ital", value)
    }

    #[must_use]
    pub const fn slant(value: f32) -> Self {
        Self::new(*b"slnt", value)
    }

    #[must_use]
    pub const fn width(value: f32) -> Self {
        Self::new(*b"wdth", value)
    }

    #[must_use]
    pub const fn optical_size(value: f32) -> Self {
        Self::new(*b"opsz", value)
    }

    #[must_use]
    pub fn to_css_setting(&self) -> String {
        let axis_str = std::str::from_utf8(&self.axis).unwrap_or("????");
        format!("\"{}\" {:.1}", axis_str, self.value)
    }
}

impl Lerp for FontVariation {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        Self::new(self.axis, self.value.lerp(&other.value, t))
    }
}

/// Immutable icon font metadata.
///
/// Icon glyphs use the normal text shaping pipeline: this value only carries
/// the code point and font selection metadata, then [`Self::text_style`]
/// produces the same [`TextStyle`] used by ordinary text. There is no second
/// icon renderer or path model to keep synchronized with text.
#[derive(Clone, Debug, PartialEq)]
pub struct IconData {
    code_point: u32,
    font_family: Option<FontFamily>,
    font_package: Option<String>,
    match_text_direction: bool,
    font_variations: Arc<[FontVariation]>,
}

impl IconData {
    #[must_use]
    pub fn new(code_point: u32) -> Self {
        Self {
            code_point,
            font_family: None,
            font_package: None,
            match_text_direction: false,
            font_variations: Arc::from([]),
        }
    }

    #[must_use]
    pub const fn code_point(&self) -> u32 {
        self.code_point
    }

    #[must_use]
    pub fn glyph(&self) -> Option<char> {
        char::from_u32(self.code_point)
    }

    #[must_use]
    pub fn glyph_text(&self) -> Option<String> {
        self.glyph().map(|glyph| glyph.to_string())
    }

    #[must_use]
    pub fn font_family_value(&self) -> Option<&FontFamily> {
        self.font_family.as_ref()
    }

    #[must_use]
    pub fn font_package_value(&self) -> Option<&str> {
        self.font_package.as_deref()
    }

    #[must_use]
    pub const fn match_text_direction_value(&self) -> bool {
        self.match_text_direction
    }

    #[must_use]
    pub fn font_variations(&self) -> &[FontVariation] {
        &self.font_variations
    }

    #[must_use]
    pub fn family(mut self, family: FontFamily) -> Self {
        self.font_family = Some(family);
        self
    }

    #[must_use]
    pub fn font_family(mut self, family: impl Into<String>) -> Self {
        self.font_family = Some(FontFamily::Named(family.into()));
        self
    }

    #[must_use]
    pub fn font_package(mut self, package: impl Into<String>) -> Self {
        self.font_package = Some(package.into());
        self
    }

    #[must_use]
    pub fn match_text_direction(mut self, match_text_direction: bool) -> Self {
        self.match_text_direction = match_text_direction;
        self
    }

    #[must_use]
    pub fn variations<I>(mut self, variations: I) -> Self
    where
        I: IntoIterator<Item = FontVariation>,
    {
        self.font_variations = variations.into_iter().collect::<Vec<_>>().into();
        self
    }

    /// Builds the ordinary text style used to shape this glyph.
    #[must_use]
    pub fn text_style(&self, size: f32) -> TextStyle {
        let mut style = TextStyle::default().font_size(size);
        if let Some(family) = &self.font_family {
            style = style.family(family.clone());
        }
        if !self.font_variations.is_empty() {
            style = style.font_variation_settings(&self.font_variations);
        }
        style
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
    Justify,
}

/// The baseline alignment policy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextBaseline {
    #[default]
    Alphabetic,
    Ideographic,
}

/// The leading distribution policy between line boxes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextLeadingDistribution {
    #[default]
    Proportional,
    Even,
}

/// Strategy for measuring paragraph width bounds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextWidthBasis {
    #[default]
    Parent,
    LongestLine,
}

/// Controls line-box height calculations for first ascent and last descent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextHeightBehavior {
    pub apply_height_to_first_ascent: bool,
    pub apply_height_to_last_descent: bool,
    pub leading_distribution: TextLeadingDistribution,
}

impl TextHeightBehavior {
    #[must_use]
    pub const fn new(
        apply_height_to_first_ascent: bool,
        apply_height_to_last_descent: bool,
        leading_distribution: TextLeadingDistribution,
    ) -> Self {
        Self {
            apply_height_to_first_ascent,
            apply_height_to_last_descent,
            leading_distribution,
        }
    }
}

/// Visual line decorations for rendered text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextDecoration(u8);

impl TextDecoration {
    pub const NONE: Self = Self(0);
    pub const UNDERLINE: Self = Self(1);
    pub const OVERLINE: Self = Self(2);
    pub const LINE_THROUGH: Self = Self(4);

    #[must_use]
    pub const fn combine(decorations: &[Self]) -> Self {
        let mut bits = 0;
        let mut i = 0;
        while i < decorations.len() {
            bits |= decorations[i].0;
            i += 1;
        }
        Self(bits)
    }

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl std::ops::BitOr for TextDecoration {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitAnd for TextDecoration {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

/// Visual stroke pattern for text decorations.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextDecorationStyle {
    #[default]
    Solid,
    Double,
    Dotted,
    Dashed,
    Wavy,
}

/// Compatibility spelling for the renderer-owned canonical shadow value.
/// Text must not maintain a second color/offset/blur representation.
pub type TextShadow = Shadow;

/// Baseline strut configuration defining minimum vertical line spacing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StrutStyle {
    pub font_family: Option<FontFamily>,
    pub font_family_fallback: Arc<[FontFamily]>,
    pub font_size: Option<f32>,
    pub font_weight: Option<FontWeight>,
    pub font_style: Option<FontStyle>,
    pub height: Option<f32>,
    pub leading_distribution: Option<TextLeadingDistribution>,
    pub force_strut_height: Option<bool>,
}

impl StrutStyle {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn font_family(mut self, family: impl Into<String>) -> Self {
        self.font_family = Some(FontFamily::Named(family.into()));
        self
    }

    #[must_use]
    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = Some(size.max(0.0));
        self
    }

    #[must_use]
    pub fn font_weight(mut self, weight: FontWeight) -> Self {
        self.font_weight = Some(weight);
        self
    }

    #[must_use]
    pub fn font_style(mut self, style: FontStyle) -> Self {
        self.font_style = Some(style);
        self
    }

    #[must_use]
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height.max(0.0));
        self
    }

    #[must_use]
    pub fn leading_distribution(mut self, distribution: TextLeadingDistribution) -> Self {
        self.leading_distribution = Some(distribution);
        self
    }

    #[must_use]
    pub fn force_strut_height(mut self, force: bool) -> Self {
        self.force_strut_height = Some(force);
        self
    }
}

/// What to do when a paragraph cannot fit its configured line or width limit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextOverflow {
    #[default]
    Clip,
    Ellipsis,
    Visible,
    Fade,
}

/// Text scaling policy applied after a style's logical font size is chosen.
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

/// Explicit text line height representation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LineHeight {
    /// Natural line height derived from font metrics.
    Normal,
    /// Line height as a proportional multiplier of font size (e.g. `1.4` = 1.4 * font_size).
    Multiplier(f32),
    /// Line height in absolute logical pixels.
    Absolute(f32),
}

impl LineHeight {
    #[must_use]
    pub const fn multiplier(factor: f32) -> Self {
        Self::Multiplier(factor)
    }

    #[must_use]
    pub const fn absolute(pixels: f32) -> Self {
        Self::Absolute(pixels)
    }

    #[must_use]
    pub fn to_parley(self, font_size: f32) -> Option<parley::style::LineHeight> {
        match self {
            Self::Normal => None,
            Self::Multiplier(factor) => Some(parley::style::LineHeight::Absolute(
                (factor * font_size).max(0.0),
            )),
            Self::Absolute(pixels) => Some(parley::style::LineHeight::Absolute(pixels.max(0.0))),
        }
    }
}

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
            size: 16.0,
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
        let middle = first.lerp(&second, 0.5);
        assert_eq!(middle.size, 15.0);
        assert_eq!(middle.color, Color::rgba(128, 128, 128, 255));
    }

    #[test]
    fn change_impact_distinguishes_layout_from_paint() {
        let base = TextStyle::default().font_size(16.0).color(Color::WHITE);
        let paint_change = base.clone().color(Color::BLACK);
        assert_eq!(base.change_impact(&paint_change), ChangeImpact::Paint);
        let layout_change = base.clone().font_size(18.0);
        assert_eq!(base.change_impact(&layout_change), ChangeImpact::Layout);
    }

    #[test]
    fn font_weight_maps_to_text_stack() {
        let style = TextStyle::default().font_weight(FontWeight::W700);
        assert_eq!(style.weight, FontWeight::W700);
        assert_eq!(style.weight.value(), 700);
    }

    #[test]
    fn font_feature_tag_round_trip() {
        let feature = FontFeature::new(*b"liga", 0);
        assert_eq!(feature.tag, *b"liga");
        assert_eq!(feature.value, 0);
        assert_eq!(feature.to_css_setting(), "\"liga\" 0");
    }

    #[test]
    fn font_variation_axis_round_trip() {
        let variation = FontVariation::new(*b"wght", 650.);
        assert_eq!(variation.axis, *b"wght");
        assert_eq!(variation.value, 650.);
        assert_eq!(variation.to_css_setting(), "\"wght\" 650.0");
    }
}
