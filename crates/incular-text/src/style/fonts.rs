use incular_core::Lerp;

/// A logical font family. Named families are resolved by [`crate::TextEngine`].
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
