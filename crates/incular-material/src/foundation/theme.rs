//! Material color, typography, and component theme values.

use incular_config::{Brightness, Clip, EdgeInsets};
use incular_controls::{ButtonStyle, StateValue};
use incular_core::{Color, HslColor, Lerp, Size};
use incular_text::TextStyle;
use incular_widgets::{Border, BorderRadius};
use std::time::Duration;

use super::helpers::mix;

/// Material theme extension payload for application-defined values.
#[derive(Clone, Debug, PartialEq)]
pub enum ThemeExtensionValue {
    Color(Color),
    Number(f32),
    Text(String),
    Bool(bool),
}

/// The complete Material 3 colour-role set.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorScheme {
    pub brightness: Brightness,
    pub primary: Color,
    pub on_primary: Color,
    pub primary_container: Color,
    pub on_primary_container: Color,
    pub primary_fixed: Color,
    pub primary_fixed_dim: Color,
    pub on_primary_fixed: Color,
    pub on_primary_fixed_variant: Color,
    pub secondary: Color,
    pub on_secondary: Color,
    pub secondary_container: Color,
    pub on_secondary_container: Color,
    pub secondary_fixed: Color,
    pub secondary_fixed_dim: Color,
    pub on_secondary_fixed: Color,
    pub on_secondary_fixed_variant: Color,
    pub tertiary: Color,
    pub on_tertiary: Color,
    pub tertiary_container: Color,
    pub on_tertiary_container: Color,
    pub tertiary_fixed: Color,
    pub tertiary_fixed_dim: Color,
    pub on_tertiary_fixed: Color,
    pub on_tertiary_fixed_variant: Color,
    pub error: Color,
    pub on_error: Color,
    pub error_container: Color,
    pub on_error_container: Color,
    pub surface: Color,
    pub on_surface: Color,
    pub surface_dim: Color,
    pub surface_bright: Color,
    pub surface_container_lowest: Color,
    pub surface_container_low: Color,
    pub surface_container: Color,
    pub surface_container_high: Color,
    pub surface_container_highest: Color,
    pub on_surface_variant: Color,
    pub outline: Color,
    pub outline_variant: Color,
    pub shadow: Color,
    pub scrim: Color,
    pub inverse_surface: Color,
    pub on_inverse_surface: Color,
    pub inverse_primary: Color,
    pub surface_tint: Color,
    /// Deprecated Flutter aliases retained for source compatibility.
    pub background: Color,
    pub on_background: Color,
    pub surface_variant: Color,
}

impl ColorScheme {
    #[must_use]
    pub fn light() -> Self {
        Self::from_seed_with_brightness(Color::rgba(103, 80, 164, 255), Brightness::Light)
    }

    #[must_use]
    pub fn dark() -> Self {
        Self::from_seed_with_brightness(Color::rgba(208, 188, 255, 255), Brightness::Dark)
    }

    #[must_use]
    pub fn from_seed(seed_color: Color) -> Self {
        // Flutter's `ColorScheme.fromSeed` defaults to light brightness.
        // Brightness is an explicit theme choice; inferring it from a seed
        // makes the standard purple seed unexpectedly produce a dark theme.
        Self::from_seed_with_brightness(seed_color, Brightness::Light)
    }

    #[must_use]
    pub fn from_seed_with_brightness(seed: Color, brightness: Brightness) -> Self {
        let hsl = seed.to_hsl();
        let primary = seed;
        let secondary = HslColor::new(
            hsl.hue + 60.0,
            (hsl.saturation * 0.7).min(1.0),
            hsl.lightness,
            1.0,
        )
        .to_color();
        let tertiary = HslColor::new(
            hsl.hue + 120.0,
            (hsl.saturation * 0.65).min(1.0),
            hsl.lightness,
            1.0,
        )
        .to_color();
        let (surface, on_surface, container, on_container) = match brightness {
            Brightness::Light => (
                Color::rgba(255, 251, 255, 255),
                Color::rgba(29, 27, 32, 255),
                HslColor::new(hsl.hue, hsl.saturation * 0.35, 0.92, 1.0).to_color(),
                Color::rgba(33, 0, 93, 255),
            ),
            Brightness::Dark => (
                Color::rgba(20, 18, 24, 255),
                Color::rgba(231, 225, 229, 255),
                HslColor::new(hsl.hue, hsl.saturation * 0.25, 0.28, 1.0).to_color(),
                Color::rgba(234, 221, 255, 255),
            ),
        };
        let on_primary = contrasting(primary);
        let on_secondary = contrasting(secondary);
        let on_tertiary = contrasting(tertiary);
        let error = match brightness {
            Brightness::Light => Color::rgba(186, 26, 26, 255),
            Brightness::Dark => Color::rgba(255, 180, 171, 255),
        };
        let on_error = contrasting(error);
        let error_container = match brightness {
            Brightness::Light => Color::rgba(255, 218, 214, 255),
            Brightness::Dark => Color::rgba(147, 0, 10, 255),
        };
        let on_error_container = contrasting(error_container);
        let surface_dim = if brightness == Brightness::Light {
            Color::rgba(222, 216, 225, 255)
        } else {
            Color::rgba(20, 18, 24, 255)
        };
        let surface_bright = if brightness == Brightness::Light {
            Color::rgba(255, 251, 255, 255)
        } else {
            Color::rgba(59, 56, 62, 255)
        };
        let outline = if brightness == Brightness::Light {
            Color::rgba(121, 116, 126, 255)
        } else {
            Color::rgba(147, 143, 153, 255)
        };
        let inverse_surface = if brightness == Brightness::Light {
            Color::rgba(50, 47, 53, 255)
        } else {
            Color::rgba(231, 225, 229, 255)
        };
        let on_inverse_surface = contrasting(inverse_surface);
        Self {
            brightness,
            primary,
            on_primary,
            primary_container: container,
            on_primary_container: on_container,
            primary_fixed: Color::rgba(234, 221, 255, 255),
            primary_fixed_dim: Color::rgba(217, 194, 255, 255),
            on_primary_fixed: Color::rgba(33, 0, 93, 255),
            on_primary_fixed_variant: Color::rgba(79, 55, 139, 255),
            secondary,
            on_secondary,
            secondary_container: mix(surface, secondary, 0.18),
            on_secondary_container: contrasting(mix(surface, secondary, 0.18)),
            secondary_fixed: Color::rgba(232, 222, 248, 255),
            secondary_fixed_dim: Color::rgba(203, 191, 219, 255),
            on_secondary_fixed: Color::rgba(30, 25, 34, 255),
            on_secondary_fixed_variant: Color::rgba(74, 68, 82, 255),
            tertiary,
            on_tertiary,
            tertiary_container: mix(surface, tertiary, 0.18),
            on_tertiary_container: contrasting(mix(surface, tertiary, 0.18)),
            tertiary_fixed: Color::rgba(255, 216, 227, 255),
            tertiary_fixed_dim: Color::rgba(239, 184, 201, 255),
            on_tertiary_fixed: Color::rgba(49, 17, 32, 255),
            on_tertiary_fixed_variant: Color::rgba(116, 53, 76, 255),
            error,
            on_error,
            error_container,
            on_error_container,
            surface,
            on_surface,
            surface_dim,
            surface_bright,
            surface_container_lowest: surface,
            surface_container_low: mix(surface, on_surface, 0.03),
            surface_container: mix(surface, on_surface, 0.06),
            surface_container_high: mix(surface, on_surface, 0.10),
            surface_container_highest: mix(surface, on_surface, 0.14),
            on_surface_variant: if brightness == Brightness::Light {
                Color::rgba(73, 69, 79, 255)
            } else {
                Color::rgba(202, 196, 208, 255)
            },
            outline,
            outline_variant: mix(outline, surface, 0.45),
            shadow: Color::BLACK,
            scrim: Color::BLACK,
            inverse_surface,
            on_inverse_surface,
            inverse_primary: if brightness == Brightness::Light {
                Color::rgba(217, 194, 255, 255)
            } else {
                Color::rgba(103, 80, 164, 255)
            },
            surface_tint: primary,
            background: surface,
            on_background: on_surface,
            surface_variant: mix(surface, on_surface, 0.10),
        }
    }

    #[must_use]
    pub fn with_primary(mut self, value: Color) -> Self {
        self.primary = value;
        self.surface_tint = value;
        self
    }

    #[must_use]
    pub fn with_surface(mut self, value: Color) -> Self {
        self.surface = value;
        self.background = value;
        self
    }

    #[must_use]
    pub fn with_on_surface(mut self, value: Color) -> Self {
        self.on_surface = value;
        self.on_background = value;
        self
    }

    #[must_use]
    pub fn lerp(self, other: Self, t: f32) -> Self {
        macro_rules! c {
            ($field:ident) => {
                self.$field.lerp(&other.$field, t)
            };
        }
        Self {
            brightness: if t < 0.5 {
                self.brightness
            } else {
                other.brightness
            },
            primary: c!(primary),
            on_primary: c!(on_primary),
            primary_container: c!(primary_container),
            on_primary_container: c!(on_primary_container),
            primary_fixed: c!(primary_fixed),
            primary_fixed_dim: c!(primary_fixed_dim),
            on_primary_fixed: c!(on_primary_fixed),
            on_primary_fixed_variant: c!(on_primary_fixed_variant),
            secondary: c!(secondary),
            on_secondary: c!(on_secondary),
            secondary_container: c!(secondary_container),
            on_secondary_container: c!(on_secondary_container),
            secondary_fixed: c!(secondary_fixed),
            secondary_fixed_dim: c!(secondary_fixed_dim),
            on_secondary_fixed: c!(on_secondary_fixed),
            on_secondary_fixed_variant: c!(on_secondary_fixed_variant),
            tertiary: c!(tertiary),
            on_tertiary: c!(on_tertiary),
            tertiary_container: c!(tertiary_container),
            on_tertiary_container: c!(on_tertiary_container),
            tertiary_fixed: c!(tertiary_fixed),
            tertiary_fixed_dim: c!(tertiary_fixed_dim),
            on_tertiary_fixed: c!(on_tertiary_fixed),
            on_tertiary_fixed_variant: c!(on_tertiary_fixed_variant),
            error: c!(error),
            on_error: c!(on_error),
            error_container: c!(error_container),
            on_error_container: c!(on_error_container),
            surface: c!(surface),
            on_surface: c!(on_surface),
            surface_dim: c!(surface_dim),
            surface_bright: c!(surface_bright),
            surface_container_lowest: c!(surface_container_lowest),
            surface_container_low: c!(surface_container_low),
            surface_container: c!(surface_container),
            surface_container_high: c!(surface_container_high),
            surface_container_highest: c!(surface_container_highest),
            on_surface_variant: c!(on_surface_variant),
            outline: c!(outline),
            outline_variant: c!(outline_variant),
            shadow: c!(shadow),
            scrim: c!(scrim),
            inverse_surface: c!(inverse_surface),
            on_inverse_surface: c!(on_inverse_surface),
            inverse_primary: c!(inverse_primary),
            surface_tint: c!(surface_tint),
            background: c!(background),
            on_background: c!(on_background),
            surface_variant: c!(surface_variant),
        }
    }
}

impl Default for ColorScheme {
    fn default() -> Self {
        Self::light()
    }
}

fn relative_luminance(color: Color) -> f32 {
    fn channel(value: u8) -> f32 {
        let value = value as f32 / 255.0;
        if value <= 0.03928 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * channel(color.red) + 0.7152 * channel(color.green) + 0.0722 * channel(color.blue)
}

fn contrasting(color: Color) -> Color {
    if relative_luminance(color) > 0.45 {
        Color::BLACK
    } else {
        Color::WHITE
    }
}

/// Material typography roles.  The text engine remains owned by
/// `incular-text`; this is only the Material role mapping.
#[derive(Clone, Debug, PartialEq)]
pub struct TextTheme {
    pub display_large: TextStyle,
    pub display_medium: TextStyle,
    pub display_small: TextStyle,
    pub headline_large: TextStyle,
    pub headline_medium: TextStyle,
    pub headline_small: TextStyle,
    pub title_large: TextStyle,
    pub title_medium: TextStyle,
    pub title_small: TextStyle,
    pub body_large: TextStyle,
    pub body_medium: TextStyle,
    pub body_small: TextStyle,
    pub label_large: TextStyle,
    pub label_medium: TextStyle,
    pub label_small: TextStyle,
}

impl Default for TextTheme {
    fn default() -> Self {
        Self::light()
    }
}

impl TextTheme {
    #[must_use]
    pub fn light() -> Self {
        Self::with_color(Color::rgba(29, 27, 32, 255))
    }

    #[must_use]
    pub fn dark() -> Self {
        Self::with_color(Color::rgba(231, 225, 229, 255))
    }

    #[must_use]
    pub fn with_color(color: Color) -> Self {
        let role = |size: f32| TextStyle::new().font_size(size).color(color);
        Self {
            display_large: role(57.0),
            display_medium: role(45.0),
            display_small: role(36.0),
            headline_large: role(32.0),
            headline_medium: role(28.0),
            headline_small: role(24.0),
            title_large: role(22.0),
            title_medium: role(16.0),
            title_small: role(14.0),
            body_large: role(16.0),
            body_medium: role(14.0),
            body_small: role(12.0),
            label_large: role(14.0),
            label_medium: role(12.0),
            label_small: role(11.0),
        }
    }

    #[must_use]
    pub fn merge(mut self, other: &Self) -> Self {
        self.display_large = self.display_large.clone().merge(&other.display_large);
        self.display_medium = self.display_medium.clone().merge(&other.display_medium);
        self.display_small = self.display_small.clone().merge(&other.display_small);
        self.headline_large = self.headline_large.clone().merge(&other.headline_large);
        self.headline_medium = self.headline_medium.clone().merge(&other.headline_medium);
        self.headline_small = self.headline_small.clone().merge(&other.headline_small);
        self.title_large = self.title_large.clone().merge(&other.title_large);
        self.title_medium = self.title_medium.clone().merge(&other.title_medium);
        self.title_small = self.title_small.clone().merge(&other.title_small);
        self.body_large = self.body_large.clone().merge(&other.body_large);
        self.body_medium = self.body_medium.clone().merge(&other.body_medium);
        self.body_small = self.body_small.clone().merge(&other.body_small);
        self.label_large = self.label_large.clone().merge(&other.label_large);
        self.label_medium = self.label_medium.clone().merge(&other.label_medium);
        self.label_small = self.label_small.clone().merge(&other.label_small);
        self
    }

    #[must_use]
    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        macro_rules! style {
            ($field:ident) => {
                self.$field.lerp(&other.$field, t)
            };
        }
        Self {
            display_large: style!(display_large),
            display_medium: style!(display_medium),
            display_small: style!(display_small),
            headline_large: style!(headline_large),
            headline_medium: style!(headline_medium),
            headline_small: style!(headline_small),
            title_large: style!(title_large),
            title_medium: style!(title_medium),
            title_small: style!(title_small),
            body_large: style!(body_large),
            body_medium: style!(body_medium),
            body_small: style!(body_small),
            label_large: style!(label_large),
            label_medium: style!(label_medium),
            label_small: style!(label_small),
        }
    }

    #[must_use]
    pub fn with_body_medium(mut self, style: TextStyle) -> Self {
        self.body_medium = style;
        self
    }
}

/// Material's typography family metadata.
#[derive(Clone, Debug, PartialEq)]
pub struct Typography {
    pub black: TextTheme,
    pub white: TextTheme,
    pub english_like: TextTheme,
    pub dense: TextTheme,
    pub tall: TextTheme,
}

impl Default for Typography {
    fn default() -> Self {
        Self::material2021()
    }
}

impl Typography {
    #[must_use]
    pub fn material2021() -> Self {
        Self {
            black: TextTheme::light(),
            white: TextTheme::dark(),
            english_like: TextTheme::light(),
            dense: TextTheme::light(),
            tall: TextTheme::light(),
        }
    }
}

/// Shared fields common to Material component theme data.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ComponentThemeData {
    /// Optional complete style override used by button component themes.
    /// Non-button themes simply leave this unset; keeping it here lets the
    /// generated Flutter-shaped theme aliases share one sparse merge model.
    pub style: Option<ButtonStyle>,
    pub background_color: Option<Color>,
    /// Flutter calls this `color` for surfaces such as Card and Divider.
    /// `background_color` remains the shared spelling used by button themes.
    pub color: Option<Color>,
    pub foreground_color: Option<Color>,
    pub overlay_color: Option<Color>,
    pub shadow_color: Option<Color>,
    pub surface_tint_color: Option<Color>,
    pub elevation: Option<f32>,
    pub padding: Option<EdgeInsets>,
    pub margin: Option<EdgeInsets>,
    pub minimum_size: Option<Size>,
    pub maximum_size: Option<Size>,
    pub shape: Option<BorderRadius>,
    pub side: Option<Border>,
    pub clip_behavior: Option<Clip>,
    pub border_on_foreground: Option<bool>,
    pub center_title: Option<bool>,
    pub toolbar_height: Option<f32>,
    pub leading_width: Option<f32>,
    pub title_spacing: Option<f32>,
    pub indicator_size: Option<f32>,
    pub track_height: Option<f32>,
    pub thumb_size: Option<f32>,
    pub divider_color: Option<Color>,
    pub label_color: Option<Color>,
    pub unselected_label_color: Option<Color>,
    pub icon_color: Option<Color>,
    pub selected_icon_color: Option<Color>,
    /// State-aware colors used by selection controls and indicators. Keeping
    /// these on the shared sparse theme record lets the typed
    /// `CheckboxThemeData`/`RadioThemeData`/`SwitchThemeData` aliases expose
    /// Flutter's common property names without forking a second resolver.
    pub fill_color: Option<StateValue<Color>>,
    pub check_color: Option<StateValue<Color>>,
    pub thumb_color: Option<StateValue<Color>>,
    pub track_color: Option<StateValue<Color>>,
    pub outline_color: Option<StateValue<Color>>,
    pub active_track_color: Option<StateValue<Color>>,
    pub inactive_track_color: Option<StateValue<Color>>,
    pub disabled_color: Option<Color>,
    pub icon_size: Option<f32>,
    pub opacity: Option<f32>,
    pub mouse_cursor: Option<String>,
    pub selected_item_color: Option<Color>,
    pub unselected_item_color: Option<Color>,
    pub surface_color: Option<Color>,
    pub min_size: Option<Size>,
    pub text_style: Option<TextStyle>,
    pub animation_duration: Option<Duration>,
}

impl ComponentThemeData {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn style(mut self, value: ButtonStyle) -> Self {
        self.style = Some(value);
        self
    }

    #[must_use]
    pub fn background_color(mut self, value: Color) -> Self {
        self.background_color = Some(value);
        self
    }

    #[must_use]
    pub fn foreground_color(mut self, value: Color) -> Self {
        self.foreground_color = Some(value);
        self
    }

    #[must_use]
    pub fn color(mut self, value: Color) -> Self {
        self.color = Some(value);
        self
    }

    #[must_use]
    pub fn margin(mut self, value: EdgeInsets) -> Self {
        self.margin = Some(value);
        self
    }

    #[must_use]
    pub fn overlay_color(mut self, value: Color) -> Self {
        self.overlay_color = Some(value);
        self
    }

    #[must_use]
    pub fn fill_color(mut self, value: impl Into<StateValue<Color>>) -> Self {
        self.fill_color = Some(value.into());
        self
    }

    #[must_use]
    pub fn check_color(mut self, value: impl Into<StateValue<Color>>) -> Self {
        self.check_color = Some(value.into());
        self
    }

    #[must_use]
    pub fn thumb_color(mut self, value: impl Into<StateValue<Color>>) -> Self {
        self.thumb_color = Some(value.into());
        self
    }

    #[must_use]
    pub fn track_color(mut self, value: impl Into<StateValue<Color>>) -> Self {
        self.track_color = Some(value.into());
        self
    }

    #[must_use]
    pub fn outline_color(mut self, value: impl Into<StateValue<Color>>) -> Self {
        self.outline_color = Some(value.into());
        self
    }

    #[must_use]
    pub fn active_track_color(mut self, value: impl Into<StateValue<Color>>) -> Self {
        self.active_track_color = Some(value.into());
        self
    }

    #[must_use]
    pub fn inactive_track_color(mut self, value: impl Into<StateValue<Color>>) -> Self {
        self.inactive_track_color = Some(value.into());
        self
    }

    #[must_use]
    pub fn disabled_color(mut self, value: Color) -> Self {
        self.disabled_color = Some(value);
        self
    }

    #[must_use]
    pub fn icon_size(mut self, value: f32) -> Self {
        self.icon_size = Some(value.max(0.0));
        self
    }

    /// Flutter-shaped spelling for `IconThemeData.size`.
    #[must_use]
    pub fn size(self, value: f32) -> Self {
        self.icon_size(value)
    }

    #[must_use]
    pub fn opacity(mut self, value: f32) -> Self {
        self.opacity = Some(value.clamp(0.0, 1.0));
        self
    }

    #[must_use]
    pub fn mouse_cursor(mut self, value: impl Into<String>) -> Self {
        self.mouse_cursor = Some(value.into());
        self
    }

    #[must_use]
    pub fn elevation(mut self, value: f32) -> Self {
        self.elevation = Some(value.max(0.0));
        self
    }

    #[must_use]
    pub fn padding(mut self, value: EdgeInsets) -> Self {
        self.padding = Some(value);
        self
    }

    #[must_use]
    pub fn shape(mut self, value: BorderRadius) -> Self {
        self.shape = Some(value);
        self
    }
}

macro_rules! component_theme_aliases {
    ($($name:ident),+ $(,)?) => { $(pub type $name = ComponentThemeData;)+ };
}

component_theme_aliases!(
    ActionIconThemeData,
    AppBarThemeData,
    BadgeThemeData,
    BannerThemeData,
    MaterialBannerThemeData,
    IconThemeData,
    BottomAppBarThemeData,
    BottomNavigationBarThemeData,
    BottomSheetThemeData,
    CardThemeData,
    CarouselViewThemeData,
    CheckboxThemeData,
    ChipThemeData,
    DataTableThemeData,
    DatePickerThemeData,
    DialogThemeData,
    DividerThemeData,
    DrawerThemeData,
    ElevatedButtonThemeData,
    ExpansionTileThemeData,
    FilledButtonThemeData,
    FloatingActionButtonThemeData,
    IconButtonThemeData,
    ListTileThemeData,
    MenuBarThemeData,
    MenuButtonThemeData,
    NavigationBarThemeData,
    NavigationDrawerThemeData,
    NavigationRailThemeData,
    OutlinedButtonThemeData,
    RadioThemeData,
    SearchBarThemeData,
    SearchViewThemeData,
    SegmentedButtonThemeData,
    SnackBarThemeData,
    SwitchThemeData,
    TextButtonThemeData,
    TextSelectionThemeData,
    TimePickerThemeData,
    ToggleButtonsThemeData,
    TooltipThemeData,
    ButtonBarThemeData,
);

// Flutter exposes both `FooTheme` inherited wrappers and `FooThemeData`
// records. The retained implementation uses one sparse record per family;
// these aliases preserve the canonical entry points without creating a
// second theme-resolution system.
macro_rules! component_theme_wrappers {
    ($($name:ident = $data:ident),+ $(,)?) => { $(#[allow(dead_code)] pub type $name = $data;)+ };
}

component_theme_wrappers!(
    ActionIconTheme = ActionIconThemeData,
    AppBarTheme = AppBarThemeData,
    BadgeTheme = BadgeThemeData,
    BottomAppBarTheme = BottomAppBarThemeData,
    BottomNavigationBarTheme = BottomNavigationBarThemeData,
    BottomSheetTheme = BottomSheetThemeData,
    CardTheme = CardThemeData,
    CheckboxTheme = CheckboxThemeData,
    ChipTheme = ChipThemeData,
    DialogTheme = DialogThemeData,
    DividerTheme = DividerThemeData,
    DrawerTheme = DrawerThemeData,
    ElevatedButtonTheme = ElevatedButtonThemeData,
    FilledButtonTheme = FilledButtonThemeData,
    FloatingActionButtonTheme = FloatingActionButtonThemeData,
    IconButtonTheme = IconButtonThemeData,
    ListTileTheme = ListTileThemeData,
    MenuBarTheme = MenuBarThemeData,
    MenuButtonTheme = MenuButtonThemeData,
    NavigationBarTheme = NavigationBarThemeData,
    NavigationDrawerTheme = NavigationDrawerThemeData,
    NavigationRailTheme = NavigationRailThemeData,
    OutlinedButtonTheme = OutlinedButtonThemeData,
    RadioTheme = RadioThemeData,
    SnackBarTheme = SnackBarThemeData,
    SwitchTheme = SwitchThemeData,
    TextButtonTheme = TextButtonThemeData,
    TextSelectionTheme = TextSelectionThemeData,
    TooltipTheme = TooltipThemeData,
);

#[allow(dead_code)]
pub type PopupMenuTheme = crate::menus::PopupMenuThemeData;
#[allow(dead_code)]
pub type TabBarTheme = crate::p0_controls::TabBarThemeData;
