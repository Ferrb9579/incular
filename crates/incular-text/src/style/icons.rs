use super::fonts::{FontFamily, FontVariation};
use super::text_style::TextStyle;
use std::sync::Arc;

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
