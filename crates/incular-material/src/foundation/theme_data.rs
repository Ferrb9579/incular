//! Material theme aggregation, inheritance, and transition data.

use incular_config::Brightness;
use incular_controls::{ButtonStyle, ControlTheme, SplashFactory};
use incular_core::Color;
use incular_widgets::{BorderRadius, BuildContext, Widget};
use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;
use std::time::Duration;
use typed_builder::TypedBuilder;

use super::helpers::alpha;
use super::input::InputDecorationThemeData;
use super::state::WidgetStates;
use super::surfaces::{MaterialTapTargetSize, VisualDensity};
use super::theme::*;

/// Platform policy used by Material adaptive components.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TargetPlatform {
    #[default]
    Linux,
    Windows,
    MacOs,
    Android,
    Ios,
}

/// Marker trait for application-defined theme extensions.
pub trait ThemeExtension: Clone + fmt::Debug + PartialEq + 'static {
    fn lerp(&self, other: &Self, t: f32) -> Self;
}

/// Animation/page-transition policy. It is intentionally data-only; the
/// runtime owns controllers and frame scheduling.
#[derive(Clone, Debug, Default, PartialEq, TypedBuilder)]
pub struct PageTransitionsTheme {
    #[builder(default)]
    pub duration: Duration,
    #[builder(default)]
    pub reduced_motion: bool,
}

/// Core Material policy and typography.
#[derive(Clone, Debug, PartialEq)]
pub struct CoreThemeData {
    pub brightness: Brightness,
    pub color_scheme: ColorScheme,
    pub text_theme: TextTheme,
    pub primary_text_theme: TextTheme,
    pub typography: Typography,
    pub use_material3: bool,
    pub visual_density: VisualDensity,
    pub material_tap_target_size: MaterialTapTargetSize,
    pub platform: TargetPlatform,
    pub page_transitions_theme: PageTransitionsTheme,
    pub splash_factory: SplashFactory,
    pub apply_elevation_overlay_color: bool,
}

/// Legacy color aliases retained for Flutter-compatible Material APIs.
#[derive(Clone, Debug, PartialEq)]
pub struct LegacyThemeColors {
    pub canvas_color: Color,
    pub card_color: Color,
    pub disabled_color: Color,
    pub divider_color: Color,
    pub focus_color: Color,
    pub highlight_color: Color,
    pub hint_color: Color,
    pub hover_color: Color,
    pub primary_color: Color,
    pub primary_color_dark: Color,
    pub primary_color_light: Color,
    pub scaffold_background_color: Color,
    pub secondary_header_color: Color,
    pub shadow_color: Color,
    pub splash_color: Color,
    pub unselected_widget_color: Color,
    pub dialog_background_color: Color,
    pub indicator_color: Color,
}

/// Application-defined theme extension and adaptation payloads.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ThemeExtensions {
    /// Optional Cupertino bridge payload. The Cupertino crate is deliberately
    /// not a dependency of Material, so applications can carry an opaque
    /// extension value here and let the platform adapter interpret it.
    pub cupertino_override_theme: Option<ThemeExtensionValue>,
    /// Application-defined adaptive policies keyed by their Rust type name.
    pub adaptation_map: BTreeMap<String, ThemeExtensionValue>,
    pub extensions: BTreeMap<String, ThemeExtensionValue>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct IconAndInputThemes {
    pub icon_theme: IconThemeData,
    pub primary_icon_theme: IconThemeData,
    pub input_decoration_theme: InputDecorationThemeData,
    pub action_icon_theme: Option<ActionIconThemeData>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ButtonComponentThemes {
    pub button_theme: ButtonThemeData,
    pub elevated_button_theme: ElevatedButtonThemeData,
    pub filled_button_theme: FilledButtonThemeData,
    pub floating_action_button_theme: FloatingActionButtonThemeData,
    pub icon_button_theme: IconButtonThemeData,
    pub outlined_button_theme: OutlinedButtonThemeData,
    pub segmented_button_theme: SegmentedButtonThemeData,
    pub text_button_theme: TextButtonThemeData,
    pub toggle_buttons_theme: ToggleButtonsThemeData,
    pub button_bar_theme: Option<ButtonBarThemeData>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NavigationComponentThemes {
    pub app_bar_theme: AppBarThemeData,
    pub bottom_app_bar_theme: BottomAppBarThemeData,
    pub bottom_navigation_bar_theme: BottomNavigationBarThemeData,
    pub drawer_theme: DrawerThemeData,
    pub navigation_bar_theme: NavigationBarThemeData,
    pub navigation_drawer_theme: NavigationDrawerThemeData,
    pub navigation_rail_theme: NavigationRailThemeData,
    pub tab_bar_theme: crate::p0_controls::TabBarThemeData,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SelectionControlThemes {
    pub checkbox_theme: CheckboxThemeData,
    pub radio_theme: RadioThemeData,
    pub slider_theme: crate::p0_controls::SliderThemeData,
    pub switch_theme: SwitchThemeData,
    pub text_selection_theme: TextSelectionThemeData,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SurfaceComponentThemes {
    pub badge_theme: BadgeThemeData,
    pub banner_theme: BannerThemeData,
    pub bottom_sheet_theme: BottomSheetThemeData,
    pub card_theme: CardThemeData,
    pub carousel_view_theme: CarouselViewThemeData,
    pub chip_theme: ChipThemeData,
    pub dialog_theme: DialogThemeData,
    pub divider_theme: DividerThemeData,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ContentComponentThemes {
    pub data_table_theme: DataTableThemeData,
    pub date_picker_theme: DatePickerThemeData,
    pub expansion_tile_theme: ExpansionTileThemeData,
    pub list_tile_theme: ListTileThemeData,
    pub search_bar_theme: SearchBarThemeData,
    pub search_view_theme: SearchViewThemeData,
    pub time_picker_theme: TimePickerThemeData,
    pub tooltip_theme: TooltipThemeData,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MenuComponentThemes {
    pub dropdown_menu_theme: crate::menus::DropdownMenuThemeData,
    pub menu_bar_theme: MenuBarThemeData,
    pub menu_button_theme: MenuButtonThemeData,
    pub menu_theme: crate::menus::MenuThemeData,
    pub popup_menu_theme: crate::menus::PopupMenuThemeData,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FeedbackComponentThemes {
    pub progress_indicator_theme: crate::feedback::ProgressIndicatorThemeData,
    pub snack_bar_theme: SnackBarThemeData,
}

#[derive(Clone, Debug, PartialEq)]
struct ThemeDataInner {
    core: Rc<CoreThemeData>,
    colors: Rc<LegacyThemeColors>,
    extensions: Rc<ThemeExtensions>,
    icons_and_input: Rc<IconAndInputThemes>,
    buttons: Rc<ButtonComponentThemes>,
    navigation: Rc<NavigationComponentThemes>,
    selection_controls: Rc<SelectionControlThemes>,
    surfaces: Rc<SurfaceComponentThemes>,
    content: Rc<ContentComponentThemes>,
    menus: Rc<MenuComponentThemes>,
    feedback: Rc<FeedbackComponentThemes>,
}

/// Central Material theme. The public value is a cheap shared handle; coherent
/// subgroups use copy-on-write when deriving a modified theme.
#[derive(Clone, Debug, PartialEq)]
pub struct ThemeData {
    inner: Rc<ThemeDataInner>,
}

/// The fields that are most commonly changed when deriving a theme.
#[derive(Clone, Debug, Default, PartialEq, TypedBuilder)]
#[builder(field_defaults(default, setter(strip_option, into)))]
pub struct ThemeDataPatch {
    pub brightness: Option<Brightness>,
    pub cupertino_override_theme: Option<ThemeExtensionValue>,
    pub adaptation_map: Option<BTreeMap<String, ThemeExtensionValue>>,
    pub color_scheme: Option<ColorScheme>,
    pub text_theme: Option<TextTheme>,
    pub primary_text_theme: Option<TextTheme>,
    pub icon_theme: Option<IconThemeData>,
    pub primary_icon_theme: Option<IconThemeData>,
    pub text_selection_theme: Option<TextSelectionThemeData>,
    pub use_material3: Option<bool>,
    pub visual_density: Option<VisualDensity>,
    pub material_tap_target_size: Option<MaterialTapTargetSize>,
    pub platform: Option<TargetPlatform>,
    pub page_transitions_theme: Option<PageTransitionsTheme>,
    pub splash_factory: Option<SplashFactory>,
    pub input_decoration_theme: Option<InputDecorationThemeData>,
    pub badge_theme: Option<BadgeThemeData>,
    pub app_bar_theme: Option<AppBarThemeData>,
    pub bottom_app_bar_theme: Option<BottomAppBarThemeData>,
    pub bottom_navigation_bar_theme: Option<BottomNavigationBarThemeData>,
    pub card_theme: Option<CardThemeData>,
    pub checkbox_theme: Option<CheckboxThemeData>,
    pub dialog_theme: Option<DialogThemeData>,
    pub divider_theme: Option<DividerThemeData>,
    pub drawer_theme: Option<DrawerThemeData>,
    pub dropdown_menu_theme: Option<crate::menus::DropdownMenuThemeData>,
    pub elevated_button_theme: Option<ElevatedButtonThemeData>,
    pub filled_button_theme: Option<FilledButtonThemeData>,
    pub floating_action_button_theme: Option<FloatingActionButtonThemeData>,
    pub icon_button_theme: Option<IconButtonThemeData>,
    pub list_tile_theme: Option<ListTileThemeData>,
    pub menu_bar_theme: Option<MenuBarThemeData>,
    pub menu_button_theme: Option<MenuButtonThemeData>,
    pub menu_theme: Option<crate::menus::MenuThemeData>,
    pub navigation_bar_theme: Option<NavigationBarThemeData>,
    pub navigation_drawer_theme: Option<NavigationDrawerThemeData>,
    pub navigation_rail_theme: Option<NavigationRailThemeData>,
    pub outlined_button_theme: Option<OutlinedButtonThemeData>,
    pub popup_menu_theme: Option<crate::menus::PopupMenuThemeData>,
    pub progress_indicator_theme: Option<crate::feedback::ProgressIndicatorThemeData>,
    pub radio_theme: Option<RadioThemeData>,
    pub slider_theme: Option<crate::p0_controls::SliderThemeData>,
    pub snack_bar_theme: Option<SnackBarThemeData>,
    pub switch_theme: Option<SwitchThemeData>,
    pub tab_bar_theme: Option<crate::p0_controls::TabBarThemeData>,
    pub text_button_theme: Option<TextButtonThemeData>,
    pub tooltip_theme: Option<TooltipThemeData>,
    pub button_bar_theme: Option<Option<ButtonBarThemeData>>,
}

impl ThemeData {
    #[must_use]
    pub fn light() -> Self {
        Self::from_color_scheme(ColorScheme::light())
    }

    #[must_use]
    pub fn dark() -> Self {
        Self::from_color_scheme(ColorScheme::dark())
    }

    #[must_use]
    pub fn light_shared() -> Rc<Self> {
        Self::from_color_scheme_shared(ColorScheme::light())
    }

    #[must_use]
    pub fn dark_shared() -> Rc<Self> {
        Self::from_color_scheme_shared(ColorScheme::dark())
    }

    #[must_use]
    pub fn fallback() -> Self {
        Self::light()
    }

    #[must_use]
    pub fn from_seed(seed_color: Color) -> Self {
        Self::from_color_scheme(ColorScheme::from_seed(seed_color))
    }

    #[must_use]
    pub fn from_seed_shared(seed_color: Color) -> Rc<Self> {
        Self::from_color_scheme_shared(ColorScheme::from_seed(seed_color))
    }

    #[must_use]
    pub fn from_color_scheme_shared(color_scheme: ColorScheme) -> Rc<Self> {
        Rc::new(Self::from_color_scheme(color_scheme))
    }

    #[must_use]
    pub fn from_color_scheme(color_scheme: ColorScheme) -> Self {
        Self {
            inner: Rc::new(defaults_from_color_scheme(color_scheme)),
        }
    }

    #[must_use]
    pub fn core(&self) -> &CoreThemeData {
        &self.inner.core
    }

    #[must_use]
    pub fn colors(&self) -> &LegacyThemeColors {
        &self.inner.colors
    }

    #[must_use]
    pub fn extensions(&self) -> &ThemeExtensions {
        &self.inner.extensions
    }

    #[must_use]
    pub fn icons_and_input(&self) -> &IconAndInputThemes {
        &self.inner.icons_and_input
    }

    #[must_use]
    pub fn buttons(&self) -> &ButtonComponentThemes {
        &self.inner.buttons
    }

    #[must_use]
    pub fn navigation(&self) -> &NavigationComponentThemes {
        &self.inner.navigation
    }

    #[must_use]
    pub fn selection_controls(&self) -> &SelectionControlThemes {
        &self.inner.selection_controls
    }

    #[must_use]
    pub fn surfaces(&self) -> &SurfaceComponentThemes {
        &self.inner.surfaces
    }

    #[must_use]
    pub fn content(&self) -> &ContentComponentThemes {
        &self.inner.content
    }

    #[must_use]
    pub fn menus(&self) -> &MenuComponentThemes {
        &self.inner.menus
    }

    #[must_use]
    pub fn feedback(&self) -> &FeedbackComponentThemes {
        &self.inner.feedback
    }

    #[must_use]
    pub fn copy_with(mut self, patch: ThemeDataPatch) -> Self {
        let inner = Rc::make_mut(&mut self.inner);
        if patch.brightness.is_some()
            || patch.color_scheme.is_some()
            || patch.text_theme.is_some()
            || patch.primary_text_theme.is_some()
            || patch.use_material3.is_some()
            || patch.visual_density.is_some()
            || patch.material_tap_target_size.is_some()
            || patch.platform.is_some()
            || patch.page_transitions_theme.is_some()
            || patch.splash_factory.is_some()
        {
            let core = Rc::make_mut(&mut inner.core);
            if let Some(value) = patch.brightness {
                core.brightness = value;
            }
            if let Some(value) = patch.color_scheme {
                core.brightness = value.brightness;
                core.color_scheme = value;
            }
            if let Some(value) = patch.text_theme {
                core.text_theme = value;
            }
            if let Some(value) = patch.primary_text_theme {
                core.primary_text_theme = value;
            }
            if let Some(value) = patch.use_material3 {
                core.use_material3 = value;
            }
            if let Some(value) = patch.visual_density {
                core.visual_density = value;
            }
            if let Some(value) = patch.material_tap_target_size {
                core.material_tap_target_size = value;
            }
            if let Some(value) = patch.platform {
                core.platform = value;
            }
            if let Some(value) = patch.page_transitions_theme {
                core.page_transitions_theme = value;
            }
            if let Some(value) = patch.splash_factory {
                core.splash_factory = value;
            }
        }
        if patch.cupertino_override_theme.is_some() || patch.adaptation_map.is_some() {
            let extensions = Rc::make_mut(&mut inner.extensions);
            if let Some(value) = patch.cupertino_override_theme {
                extensions.cupertino_override_theme = Some(value);
            }
            if let Some(value) = patch.adaptation_map {
                extensions.adaptation_map = value;
            }
        }
        if patch.icon_theme.is_some()
            || patch.primary_icon_theme.is_some()
            || patch.input_decoration_theme.is_some()
        {
            let group = Rc::make_mut(&mut inner.icons_and_input);
            if let Some(value) = patch.icon_theme {
                group.icon_theme = value;
            }
            if let Some(value) = patch.primary_icon_theme {
                group.primary_icon_theme = value;
            }
            if let Some(value) = patch.input_decoration_theme {
                group.input_decoration_theme = value;
            }
        }
        if patch.elevated_button_theme.is_some()
            || patch.filled_button_theme.is_some()
            || patch.floating_action_button_theme.is_some()
            || patch.icon_button_theme.is_some()
            || patch.outlined_button_theme.is_some()
            || patch.text_button_theme.is_some()
            || patch.button_bar_theme.is_some()
        {
            let group = Rc::make_mut(&mut inner.buttons);
            macro_rules! patch_button {
                ($field:ident) => {
                    if let Some(value) = patch.$field {
                        group.$field = value;
                    }
                };
            }
            patch_button!(elevated_button_theme);
            patch_button!(filled_button_theme);
            patch_button!(floating_action_button_theme);
            patch_button!(icon_button_theme);
            patch_button!(outlined_button_theme);
            patch_button!(text_button_theme);
            if let Some(value) = patch.button_bar_theme {
                group.button_bar_theme = value;
            }
        }
        if patch.app_bar_theme.is_some()
            || patch.bottom_app_bar_theme.is_some()
            || patch.bottom_navigation_bar_theme.is_some()
            || patch.drawer_theme.is_some()
            || patch.navigation_bar_theme.is_some()
            || patch.navigation_drawer_theme.is_some()
            || patch.navigation_rail_theme.is_some()
            || patch.tab_bar_theme.is_some()
        {
            let group = Rc::make_mut(&mut inner.navigation);
            macro_rules! patch_navigation {
                ($field:ident) => {
                    if let Some(value) = patch.$field {
                        group.$field = value;
                    }
                };
            }
            patch_navigation!(app_bar_theme);
            patch_navigation!(bottom_app_bar_theme);
            patch_navigation!(bottom_navigation_bar_theme);
            patch_navigation!(drawer_theme);
            patch_navigation!(navigation_bar_theme);
            patch_navigation!(navigation_drawer_theme);
            patch_navigation!(navigation_rail_theme);
            patch_navigation!(tab_bar_theme);
        }
        if patch.checkbox_theme.is_some()
            || patch.radio_theme.is_some()
            || patch.slider_theme.is_some()
            || patch.switch_theme.is_some()
            || patch.text_selection_theme.is_some()
        {
            let group = Rc::make_mut(&mut inner.selection_controls);
            macro_rules! patch_selection {
                ($field:ident) => {
                    if let Some(value) = patch.$field {
                        group.$field = value;
                    }
                };
            }
            patch_selection!(checkbox_theme);
            patch_selection!(radio_theme);
            patch_selection!(slider_theme);
            patch_selection!(switch_theme);
            patch_selection!(text_selection_theme);
        }
        if patch.badge_theme.is_some()
            || patch.card_theme.is_some()
            || patch.dialog_theme.is_some()
            || patch.divider_theme.is_some()
        {
            let group = Rc::make_mut(&mut inner.surfaces);
            macro_rules! patch_surface {
                ($field:ident) => {
                    if let Some(value) = patch.$field {
                        group.$field = value;
                    }
                };
            }
            patch_surface!(badge_theme);
            patch_surface!(card_theme);
            patch_surface!(dialog_theme);
            patch_surface!(divider_theme);
        }
        if patch.list_tile_theme.is_some() || patch.tooltip_theme.is_some() {
            let group = Rc::make_mut(&mut inner.content);
            if let Some(value) = patch.list_tile_theme {
                group.list_tile_theme = value;
            }
            if let Some(value) = patch.tooltip_theme {
                group.tooltip_theme = value;
            }
        }
        if patch.dropdown_menu_theme.is_some()
            || patch.menu_bar_theme.is_some()
            || patch.menu_button_theme.is_some()
            || patch.menu_theme.is_some()
            || patch.popup_menu_theme.is_some()
        {
            let group = Rc::make_mut(&mut inner.menus);
            macro_rules! patch_menu {
                ($field:ident) => {
                    if let Some(value) = patch.$field {
                        group.$field = value;
                    }
                };
            }
            patch_menu!(dropdown_menu_theme);
            patch_menu!(menu_bar_theme);
            patch_menu!(menu_button_theme);
            patch_menu!(menu_theme);
            patch_menu!(popup_menu_theme);
        }
        if patch.progress_indicator_theme.is_some() || patch.snack_bar_theme.is_some() {
            let group = Rc::make_mut(&mut inner.feedback);
            if let Some(value) = patch.progress_indicator_theme {
                group.progress_indicator_theme = value;
            }
            if let Some(value) = patch.snack_bar_theme {
                group.snack_bar_theme = value;
            }
        }
        self
    }

    #[must_use]
    pub fn with_color_scheme(mut self, value: ColorScheme) -> Self {
        let inner = Rc::make_mut(&mut self.inner);
        let core = Rc::make_mut(&mut inner.core);
        core.brightness = value.brightness;
        core.color_scheme = value;
        self
    }

    #[must_use]
    pub fn with_use_material3(mut self, value: bool) -> Self {
        Rc::make_mut(&mut Rc::make_mut(&mut self.inner).core).use_material3 = value;
        self
    }

    #[must_use]
    pub fn with_visual_density(mut self, value: VisualDensity) -> Self {
        Rc::make_mut(&mut Rc::make_mut(&mut self.inner).core).visual_density = value;
        self
    }

    #[must_use]
    pub fn with_tap_target_size(mut self, value: MaterialTapTargetSize) -> Self {
        Rc::make_mut(&mut Rc::make_mut(&mut self.inner).core).material_tap_target_size = value;
        self
    }

    #[must_use]
    pub fn with_platform(mut self, value: TargetPlatform) -> Self {
        Rc::make_mut(&mut Rc::make_mut(&mut self.inner).core).platform = value;
        self
    }

    #[must_use]
    pub fn with_page_transitions_theme(mut self, value: PageTransitionsTheme) -> Self {
        Rc::make_mut(&mut Rc::make_mut(&mut self.inner).core).page_transitions_theme = value;
        self
    }

    #[must_use]
    pub fn with_splash_factory(mut self, value: SplashFactory) -> Self {
        Rc::make_mut(&mut Rc::make_mut(&mut self.inner).core).splash_factory = value;
        self
    }

    #[must_use]
    pub fn with_input_decoration_theme(mut self, value: InputDecorationThemeData) -> Self {
        Rc::make_mut(&mut Rc::make_mut(&mut self.inner).icons_and_input).input_decoration_theme =
            value;
        self
    }

    #[must_use]
    pub fn with_button_bar_theme(mut self, value: Option<ButtonBarThemeData>) -> Self {
        Rc::make_mut(&mut Rc::make_mut(&mut self.inner).buttons).button_bar_theme = value;
        self
    }

    #[must_use]
    pub fn with_extension(mut self, key: impl Into<String>, value: ThemeExtensionValue) -> Self {
        Rc::make_mut(&mut Rc::make_mut(&mut self.inner).extensions)
            .extensions
            .insert(key.into(), value);
        self
    }

    #[must_use]
    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let mut result = if t < 0.5 { self.clone() } else { other.clone() };
        let core = Rc::make_mut(&mut Rc::make_mut(&mut result.inner).core);
        core.brightness = if t < 0.5 {
            self.core().brightness
        } else {
            other.core().brightness
        };
        core.color_scheme = self.core().color_scheme.lerp(other.core().color_scheme, t);
        core.text_theme = self.core().text_theme.lerp(&other.core().text_theme, t);
        core.primary_text_theme = self
            .core()
            .primary_text_theme
            .lerp(&other.core().primary_text_theme, t);
        core.visual_density = self
            .core()
            .visual_density
            .lerp(other.core().visual_density, t);
        result
    }

    /// Converts the Material theme into the existing controls theme. This is
    /// the only bridge from Material defaults to headless control behavior.
    #[must_use]
    pub fn control_theme(&self) -> ControlTheme {
        let core = self.core();
        let colors = self.colors();
        let selection = self.selection_controls();
        let mut controls = if core.brightness == Brightness::Light {
            ControlTheme::light()
        } else {
            ControlTheme::dark()
        };
        controls = controls.with_palette(incular_controls::ControlColors {
            background: colors.scaffold_background_color,
            surface: core.color_scheme.surface,
            surface_variant: core.color_scheme.surface_container,
            surface_elevated: core.color_scheme.surface_container_high,
            surface_active: core.color_scheme.surface_container_highest,
            foreground: core.color_scheme.on_surface,
            foreground_muted: core.color_scheme.on_surface_variant,
            foreground_disabled: colors.disabled_color,
            accent: core.color_scheme.primary,
            accent_hover: alpha(core.color_scheme.primary, 0.92),
            accent_active: core.color_scheme.primary_container,
            accent_foreground: core.color_scheme.on_primary,
            border: core.color_scheme.outline,
            border_subtle: core.color_scheme.outline_variant,
            border_strong: core.color_scheme.outline,
            hover_overlay: colors.hover_color,
            pressed_overlay: colors.splash_color,
            focus_ring: colors.focus_color,
            selection: alpha(core.color_scheme.primary, 0.24),
            disabled_surface: core.color_scheme.surface_container_highest,
            disabled_foreground: colors.disabled_color,
            error: core.color_scheme.error,
            warning: Color::rgba(160, 100, 0, 255),
            success: Color::rgba(30, 120, 60, 255),
            info: core.color_scheme.primary,
        });
        controls.density = if core.visual_density == VisualDensity::COMPACT {
            incular_controls::ControlDensity::Compact
        } else if core.visual_density == VisualDensity::COMFORTABLE {
            incular_controls::ControlDensity::Comfortable
        } else {
            incular_controls::ControlDensity::Standard
        };
        if let Some(height) = selection.slider_theme.track_height {
            controls.slider.track_height = height.max(1.0);
        }
        if let Some(size) = selection.slider_theme.thumb_size {
            controls.slider.thumb_size = size.max(1.0);
        }
        if let Some(color) = selection
            .slider_theme
            .active_track_color
            .as_ref()
            .map(|property| property.resolve(WidgetStates::default()))
        {
            controls.colors.accent = color;
        }
        if let Some(color) = selection
            .slider_theme
            .inactive_track_color
            .as_ref()
            .map(|property| property.resolve(WidgetStates::default()))
        {
            controls.colors.border_strong = color;
        }
        if let Some(color) = selection.slider_theme.disabled_active_track_color {
            controls.colors.disabled_foreground = color;
        }
        if let Some(size) = selection.checkbox_theme.icon_size {
            controls.checkbox.indicator_size = size.max(1.0);
        }
        if let Some(size) = selection.radio_theme.icon_size {
            controls.radio.indicator_size = size.max(1.0);
        }
        if let Some(width) = selection.switch_theme.minimum_size.map(|size| size.width) {
            controls.switch.width = width.max(1.0);
        }
        if let Some(height) = selection.switch_theme.minimum_size.map(|size| size.height) {
            controls.switch.height = height.max(1.0);
        }
        if let Some(size) = selection.switch_theme.thumb_size {
            controls.switch.thumb_size = size.max(1.0);
        }
        if let Some(fill) = selection
            .checkbox_theme
            .fill_color
            .as_ref()
            .map(|property| property.resolve(incular_controls::ControlState::empty()))
        {
            controls.colors.accent = fill;
        }
        if let Some(check) = selection
            .checkbox_theme
            .check_color
            .as_ref()
            .map(|property| property.resolve(incular_controls::ControlState::empty()))
        {
            controls.colors.accent_foreground = check;
        }
        controls.motion.reduced_motion = core.page_transitions_theme.reduced_motion;
        controls
    }
}

fn defaults_from_color_scheme(color_scheme: ColorScheme) -> ThemeDataInner {
    ThemeDataInner {
        core: default_core_theme(color_scheme),
        colors: default_legacy_colors(color_scheme),
        extensions: Rc::new(ThemeExtensions::default()),
        icons_and_input: default_icons_and_input(),
        buttons: default_button_themes(),
        navigation: default_navigation_themes(),
        selection_controls: default_selection_control_themes(),
        surfaces: default_surface_themes(),
        content: default_content_themes(),
        menus: default_menu_themes(),
        feedback: default_feedback_themes(),
    }
}

fn default_core_theme(color_scheme: ColorScheme) -> Rc<CoreThemeData> {
    let brightness = color_scheme.brightness;
    let text_theme = match brightness {
        Brightness::Light => TextTheme::light(),
        Brightness::Dark => TextTheme::dark(),
    };
    Rc::new(CoreThemeData {
        brightness,
        color_scheme,
        primary_text_theme: text_theme.clone(),
        text_theme,
        typography: Typography::default(),
        use_material3: true,
        visual_density: VisualDensity::default(),
        material_tap_target_size: MaterialTapTargetSize::default(),
        platform: TargetPlatform::default(),
        page_transitions_theme: PageTransitionsTheme::default(),
        splash_factory: SplashFactory::Ripple,
        apply_elevation_overlay_color: brightness == Brightness::Dark,
    })
}

fn default_legacy_colors(color_scheme: ColorScheme) -> Rc<LegacyThemeColors> {
    Rc::new(LegacyThemeColors {
        canvas_color: color_scheme.surface,
        card_color: color_scheme.surface_container_low,
        disabled_color: alpha(color_scheme.on_surface, 0.38),
        divider_color: color_scheme.outline_variant,
        focus_color: alpha(color_scheme.primary, 0.12),
        highlight_color: alpha(color_scheme.primary, 0.12),
        hint_color: color_scheme.on_surface_variant,
        hover_color: alpha(color_scheme.primary, 0.08),
        primary_color: color_scheme.primary,
        primary_color_dark: color_scheme.primary_container,
        primary_color_light: color_scheme.primary_fixed,
        scaffold_background_color: color_scheme.surface,
        secondary_header_color: color_scheme.secondary_container,
        shadow_color: color_scheme.shadow,
        splash_color: alpha(color_scheme.primary, 0.16),
        unselected_widget_color: color_scheme.on_surface_variant,
        dialog_background_color: color_scheme.surface_container_high,
        indicator_color: color_scheme.primary,
    })
}

fn default_icons_and_input() -> Rc<IconAndInputThemes> {
    Rc::new(IconAndInputThemes::default())
}

fn default_button_themes() -> Rc<ButtonComponentThemes> {
    let mut themes = ButtonComponentThemes::default();
    themes.elevated_button_theme.elevation = Some(ControlTheme::default().elevation.popup.min(6.0));
    Rc::new(themes)
}

fn default_navigation_themes() -> Rc<NavigationComponentThemes> {
    Rc::new(NavigationComponentThemes::default())
}

fn default_selection_control_themes() -> Rc<SelectionControlThemes> {
    Rc::new(SelectionControlThemes::default())
}

fn default_surface_themes() -> Rc<SurfaceComponentThemes> {
    let mut themes = SurfaceComponentThemes::default();
    themes.card_theme.elevation = Some(ControlTheme::default().elevation.popup.min(1.0));
    Rc::new(themes)
}

fn default_content_themes() -> Rc<ContentComponentThemes> {
    Rc::new(ContentComponentThemes::default())
}

fn default_menu_themes() -> Rc<MenuComponentThemes> {
    Rc::new(MenuComponentThemes::default())
}

fn default_feedback_themes() -> Rc<FeedbackComponentThemes> {
    Rc::new(FeedbackComponentThemes::default())
}

impl Default for ThemeData {
    fn default() -> Self {
        Self::light()
    }
}

/// Inherited Material theme scope.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct Theme {
    #[builder(setter(into))]
    pub data: ThemeData,
    #[builder(setter(into))]
    pub child: Widget,
}

impl Theme {
    #[must_use]
    pub fn new(data: ThemeData, child: impl Into<Widget>) -> Self {
        Self {
            data,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn of(context: &BuildContext<'_>) -> Option<ThemeData> {
        Self::of_shared(context)
            .map(|theme| (*theme).clone())
            .or_else(|| context.depend_on::<ThemeData>())
    }

    /// Reads the ambient theme without copying the large theme descriptor onto
    /// the native UI stack. Material application roots use this shared form.
    #[must_use]
    pub(crate) fn of_shared(context: &BuildContext<'_>) -> Option<Rc<ThemeData>> {
        context.depend_on::<Rc<ThemeData>>()
    }

    /// Installs one shared theme descriptor plus the derived control scopes.
    pub(crate) fn scope_shared(data: Rc<ThemeData>, child: Widget) -> Widget {
        let controls = data.control_theme();
        let input_decoration = data.icons_and_input().input_decoration_theme.clone();
        Widget::environment_scope(
            data,
            Widget::environment_scope(input_decoration, Widget::environment_scope(controls, child)),
        )
    }
}

impl From<Theme> for Widget {
    fn from(value: Theme) -> Self {
        Theme::scope_shared(Rc::new(value.data), value.child)
    }
}

/// Data-only animated theme descriptor. The runtime can interpolate the
/// supplied endpoints without forcing components to own animation state.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct AnimatedTheme {
    #[builder(setter(into))]
    pub data: ThemeData,
    pub duration: Duration,
    #[builder(setter(into))]
    pub child: Widget,
}

impl AnimatedTheme {
    #[must_use]
    pub fn new(data: ThemeData, duration: Duration, child: impl Into<Widget>) -> Self {
        Self {
            data,
            duration,
            child: child.into(),
        }
    }
}

impl From<AnimatedTheme> for Widget {
    fn from(value: AnimatedTheme) -> Self {
        Theme::new(value.data, value.child).into()
    }
}

/// Theme data for the legacy `ButtonTheme` API.
#[derive(Clone, Debug, Default, PartialEq, TypedBuilder)]
#[builder(field_defaults(default, setter(strip_option, into)))]
pub struct ButtonThemeData {
    pub style: Option<ButtonStyle>,
    pub height: Option<f32>,
    pub min_width: Option<f32>,
    pub shape: Option<BorderRadius>,
}
