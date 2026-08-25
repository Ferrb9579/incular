//! Theme models and color palettes for Incular Studio.

use incular_core::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
    HighContrast,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct StudioTheme {
    pub mode: ThemeMode,
    pub background: Color,
    pub surface: Color,
    pub surface_elevated: Color,
    pub surface_hover: Color,
    pub surface_active: Color,
    pub border: Color,
    pub border_focus: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_muted: Color,
    pub accent: Color,
    pub accent_hover: Color,
    pub accent_active: Color,
    pub selection: Color,
    pub error: Color,
    pub warning: Color,
    pub info: Color,
    pub success: Color,
    pub line_number: Color,
    pub line_number_active: Color,
    pub divider: Color,
}

impl StudioTheme {
    #[must_use]
    pub fn for_mode(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Dark => Self::dark(),
            ThemeMode::Light => Self::light(),
            ThemeMode::HighContrast => Self::high_contrast(),
        }
    }

    #[must_use]
    pub fn dark() -> Self {
        Self {
            mode: ThemeMode::Dark,
            background: Color::rgba(24, 24, 28, 255),
            surface: Color::rgba(32, 33, 38, 255),
            surface_elevated: Color::rgba(42, 44, 52, 255),
            surface_hover: Color::rgba(50, 52, 62, 255),
            surface_active: Color::rgba(60, 63, 76, 255),
            border: Color::rgba(54, 57, 68, 255),
            border_focus: Color::rgba(100, 149, 237, 255),
            text_primary: Color::rgba(235, 238, 245, 255),
            text_secondary: Color::rgba(180, 184, 195, 255),
            text_muted: Color::rgba(120, 125, 138, 255),
            accent: Color::rgba(80, 130, 240, 255),
            accent_hover: Color::rgba(100, 150, 255, 255),
            accent_active: Color::rgba(60, 110, 220, 255),
            selection: Color::rgba(50, 80, 140, 180),
            error: Color::rgba(235, 87, 87, 255),
            warning: Color::rgba(242, 153, 74, 255),
            info: Color::rgba(45, 156, 219, 255),
            success: Color::rgba(39, 174, 96, 255),
            line_number: Color::rgba(90, 94, 105, 255),
            line_number_active: Color::rgba(200, 205, 220, 255),
            divider: Color::rgba(45, 48, 58, 255),
        }
    }

    #[must_use]
    pub fn light() -> Self {
        Self {
            mode: ThemeMode::Light,
            background: Color::rgba(245, 247, 250, 255),
            surface: Color::rgba(255, 255, 255, 255),
            surface_elevated: Color::rgba(240, 243, 248, 255),
            surface_hover: Color::rgba(230, 235, 242, 255),
            surface_active: Color::rgba(220, 226, 235, 255),
            border: Color::rgba(215, 220, 230, 255),
            border_focus: Color::rgba(65, 115, 225, 255),
            text_primary: Color::rgba(30, 35, 45, 255),
            text_secondary: Color::rgba(85, 95, 110, 255),
            text_muted: Color::rgba(140, 150, 165, 255),
            accent: Color::rgba(50, 100, 220, 255),
            accent_hover: Color::rgba(65, 115, 235, 255),
            accent_active: Color::rgba(40, 85, 200, 255),
            selection: Color::rgba(180, 210, 255, 180),
            error: Color::rgba(220, 50, 50, 255),
            warning: Color::rgba(230, 130, 30, 255),
            info: Color::rgba(30, 140, 205, 255),
            success: Color::rgba(30, 155, 80, 255),
            line_number: Color::rgba(165, 175, 190, 255),
            line_number_active: Color::rgba(50, 55, 65, 255),
            divider: Color::rgba(225, 230, 238, 255),
        }
    }

    #[must_use]
    pub fn high_contrast() -> Self {
        Self {
            mode: ThemeMode::HighContrast,
            background: Color::rgba(0, 0, 0, 255),
            surface: Color::rgba(15, 15, 15, 255),
            surface_elevated: Color::rgba(30, 30, 30, 255),
            surface_hover: Color::rgba(45, 45, 45, 255),
            surface_active: Color::rgba(60, 60, 60, 255),
            border: Color::rgba(255, 255, 255, 255),
            border_focus: Color::rgba(255, 255, 0, 255),
            text_primary: Color::rgba(255, 255, 255, 255),
            text_secondary: Color::rgba(230, 230, 230, 255),
            text_muted: Color::rgba(180, 180, 180, 255),
            accent: Color::rgba(255, 255, 0, 255),
            accent_hover: Color::rgba(255, 255, 100, 255),
            accent_active: Color::rgba(220, 220, 0, 255),
            selection: Color::rgba(255, 255, 255, 120),
            error: Color::rgba(255, 50, 50, 255),
            warning: Color::rgba(255, 180, 0, 255),
            info: Color::rgba(100, 200, 255, 255),
            success: Color::rgba(50, 255, 50, 255),
            line_number: Color::rgba(200, 200, 200, 255),
            line_number_active: Color::rgba(255, 255, 255, 255),
            divider: Color::rgba(200, 200, 200, 255),
        }
    }
}
