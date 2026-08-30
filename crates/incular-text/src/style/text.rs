use super::fonts::{FontFamily, FontWeight};
use incular_rendering::Shadow;
use std::sync::Arc;

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
