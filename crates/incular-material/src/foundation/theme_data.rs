//! Material theme aggregation, inheritance, and transition data.

use incular_config::Brightness;
use incular_controls::{ButtonStyle, ControlTheme, SplashFactory};
use incular_core::Color;
use incular_widgets::{BorderRadius, Widget};
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

/// Central Material theme. Component fields are present even when a component
/// has no explicit override, matching Flutter's resolved-theme shape.
#[derive(Clone, Debug, PartialEq)]
pub struct ThemeData {
    pub brightness: Brightness,
    /// Optional Cupertino bridge payload. The Cupertino crate is deliberately
    /// not a dependency of Material, so applications can carry an opaque
    /// extension value here and let the platform adapter interpret it.
    pub cupertino_override_theme: Option<ThemeExtensionValue>,
    /// Application-defined adaptive policies keyed by their Rust type name.
    pub adaptation_map: BTreeMap<String, ThemeExtensionValue>,
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
    pub icon_theme: IconThemeData,
    pub primary_icon_theme: IconThemeData,
    pub input_decoration_theme: InputDecorationThemeData,
    pub action_icon_theme: Option<ActionIconThemeData>,
    pub app_bar_theme: AppBarThemeData,
    pub badge_theme: BadgeThemeData,
    pub banner_theme: BannerThemeData,
    pub bottom_app_bar_theme: BottomAppBarThemeData,
    pub bottom_navigation_bar_theme: BottomNavigationBarThemeData,
    pub bottom_sheet_theme: BottomSheetThemeData,
    pub button_theme: ButtonThemeData,
    pub card_theme: CardThemeData,
    pub carousel_view_theme: CarouselViewThemeData,
    pub checkbox_theme: CheckboxThemeData,
    pub chip_theme: ChipThemeData,
    pub data_table_theme: DataTableThemeData,
    pub date_picker_theme: DatePickerThemeData,
    pub dialog_theme: DialogThemeData,
    pub divider_theme: DividerThemeData,
    pub drawer_theme: DrawerThemeData,
    pub dropdown_menu_theme: crate::menus::DropdownMenuThemeData,
    pub elevated_button_theme: ElevatedButtonThemeData,
    pub expansion_tile_theme: ExpansionTileThemeData,
    pub filled_button_theme: FilledButtonThemeData,
    pub floating_action_button_theme: FloatingActionButtonThemeData,
    pub icon_button_theme: IconButtonThemeData,
    pub list_tile_theme: ListTileThemeData,
    pub menu_bar_theme: MenuBarThemeData,
    pub menu_button_theme: MenuButtonThemeData,
    pub menu_theme: crate::menus::MenuThemeData,
    pub navigation_bar_theme: NavigationBarThemeData,
    pub navigation_drawer_theme: NavigationDrawerThemeData,
    pub navigation_rail_theme: NavigationRailThemeData,
    pub outlined_button_theme: OutlinedButtonThemeData,
    pub popup_menu_theme: crate::menus::PopupMenuThemeData,
    pub progress_indicator_theme: crate::feedback::ProgressIndicatorThemeData,
    pub radio_theme: RadioThemeData,
    pub search_bar_theme: SearchBarThemeData,
    pub search_view_theme: SearchViewThemeData,
    pub segmented_button_theme: SegmentedButtonThemeData,
    pub slider_theme: crate::p0_controls::SliderThemeData,
    pub snack_bar_theme: SnackBarThemeData,
    pub switch_theme: SwitchThemeData,
    pub tab_bar_theme: crate::p0_controls::TabBarThemeData,
    pub text_button_theme: TextButtonThemeData,
    pub text_selection_theme: TextSelectionThemeData,
    pub time_picker_theme: TimePickerThemeData,
    pub toggle_buttons_theme: ToggleButtonsThemeData,
    pub tooltip_theme: TooltipThemeData,
    pub button_bar_theme: Option<ButtonBarThemeData>,
    pub extensions: BTreeMap<String, ThemeExtensionValue>,
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

    /// Creates a light theme directly in shared heap storage. ThemeData is a
    /// deliberately rich compatibility descriptor; keeping this constructor
    /// shared avoids moving its large value through the native UI stack.
    #[must_use]
    pub fn light_shared() -> Rc<Self> {
        Self::from_color_scheme_shared(ColorScheme::light())
    }

    /// Creates a dark theme directly in shared heap storage.
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

    /// Creates a seeded theme directly in shared heap storage.
    #[must_use]
    pub fn from_seed_shared(seed_color: Color) -> Rc<Self> {
        Self::from_color_scheme_shared(ColorScheme::from_seed(seed_color))
    }

    /// Heap-backed counterpart to [`Self::from_color_scheme`]. Each field is
    /// written into the allocation so no full ThemeData temporary is needed
    /// on the caller's stack.
    #[must_use]
    pub fn from_color_scheme_shared(color_scheme: ColorScheme) -> Rc<Self> {
        let brightness = color_scheme.brightness;
        let text_theme = match brightness {
            Brightness::Light => TextTheme::light(),
            Brightness::Dark => TextTheme::dark(),
        };
        let control_theme = ControlTheme::default();
        let mut shared = Rc::<Self>::new_uninit();
        let ptr = Rc::get_mut(&mut shared)
            .expect("new theme allocation has one owner")
            .as_mut_ptr();
        macro_rules! write_field {
            ($field:ident, $value:expr) => {
                // `shared` is a unique, uninitialized allocation and every
                // field is written exactly once before assume_init below.
                unsafe { std::ptr::addr_of_mut!((*ptr).$field).write($value) };
            };
        }
        write_field!(brightness, brightness);
        write_field!(cupertino_override_theme, None);
        write_field!(adaptation_map, BTreeMap::new());
        write_field!(color_scheme, color_scheme);
        write_field!(text_theme, text_theme.clone());
        write_field!(primary_text_theme, text_theme);
        write_field!(typography, Typography::default());
        write_field!(use_material3, true);
        write_field!(visual_density, VisualDensity::default());
        write_field!(material_tap_target_size, MaterialTapTargetSize::default());
        write_field!(platform, TargetPlatform::default());
        write_field!(page_transitions_theme, PageTransitionsTheme::default());
        write_field!(splash_factory, SplashFactory::Ripple);
        write_field!(
            apply_elevation_overlay_color,
            brightness == Brightness::Dark
        );
        write_field!(canvas_color, color_scheme.surface);
        write_field!(card_color, color_scheme.surface_container_low);
        write_field!(disabled_color, alpha(color_scheme.on_surface, 0.38));
        write_field!(divider_color, color_scheme.outline_variant);
        write_field!(focus_color, alpha(color_scheme.primary, 0.12));
        write_field!(highlight_color, alpha(color_scheme.primary, 0.12));
        write_field!(hint_color, color_scheme.on_surface_variant);
        write_field!(hover_color, alpha(color_scheme.primary, 0.08));
        write_field!(primary_color, color_scheme.primary);
        write_field!(primary_color_dark, color_scheme.primary_container);
        write_field!(primary_color_light, color_scheme.primary_fixed);
        write_field!(scaffold_background_color, color_scheme.surface);
        write_field!(secondary_header_color, color_scheme.secondary_container);
        write_field!(shadow_color, color_scheme.shadow);
        write_field!(splash_color, alpha(color_scheme.primary, 0.16));
        write_field!(unselected_widget_color, color_scheme.on_surface_variant);
        write_field!(dialog_background_color, color_scheme.surface_container_high);
        write_field!(indicator_color, color_scheme.primary);
        write_field!(icon_theme, ComponentThemeData::default());
        write_field!(primary_icon_theme, ComponentThemeData::default());
        write_field!(input_decoration_theme, InputDecorationThemeData::default());
        write_field!(action_icon_theme, None);
        write_field!(app_bar_theme, ComponentThemeData::default());
        write_field!(badge_theme, ComponentThemeData::default());
        write_field!(banner_theme, ComponentThemeData::default());
        write_field!(bottom_app_bar_theme, ComponentThemeData::default());
        write_field!(bottom_navigation_bar_theme, ComponentThemeData::default());
        write_field!(bottom_sheet_theme, ComponentThemeData::default());
        write_field!(button_theme, ButtonThemeData::default());
        write_field!(card_theme, ComponentThemeData::default());
        write_field!(carousel_view_theme, ComponentThemeData::default());
        write_field!(checkbox_theme, ComponentThemeData::default());
        write_field!(chip_theme, ComponentThemeData::default());
        write_field!(data_table_theme, ComponentThemeData::default());
        write_field!(date_picker_theme, ComponentThemeData::default());
        write_field!(dialog_theme, ComponentThemeData::default());
        write_field!(divider_theme, ComponentThemeData::default());
        write_field!(drawer_theme, ComponentThemeData::default());
        write_field!(
            dropdown_menu_theme,
            crate::menus::DropdownMenuThemeData::default()
        );
        write_field!(elevated_button_theme, ComponentThemeData::default());
        write_field!(expansion_tile_theme, ComponentThemeData::default());
        write_field!(filled_button_theme, ComponentThemeData::default());
        write_field!(floating_action_button_theme, ComponentThemeData::default());
        write_field!(icon_button_theme, ComponentThemeData::default());
        write_field!(list_tile_theme, ComponentThemeData::default());
        write_field!(menu_bar_theme, ComponentThemeData::default());
        write_field!(menu_button_theme, ComponentThemeData::default());
        write_field!(menu_theme, crate::menus::MenuThemeData::default());
        write_field!(navigation_bar_theme, ComponentThemeData::default());
        write_field!(navigation_drawer_theme, ComponentThemeData::default());
        write_field!(navigation_rail_theme, ComponentThemeData::default());
        write_field!(outlined_button_theme, ComponentThemeData::default());
        write_field!(
            popup_menu_theme,
            crate::menus::PopupMenuThemeData::default()
        );
        write_field!(
            progress_indicator_theme,
            crate::feedback::ProgressIndicatorThemeData::default()
        );
        write_field!(radio_theme, ComponentThemeData::default());
        write_field!(search_bar_theme, ComponentThemeData::default());
        write_field!(search_view_theme, ComponentThemeData::default());
        write_field!(segmented_button_theme, ComponentThemeData::default());
        write_field!(slider_theme, crate::p0_controls::SliderThemeData::default());
        write_field!(snack_bar_theme, ComponentThemeData::default());
        write_field!(switch_theme, ComponentThemeData::default());
        write_field!(
            tab_bar_theme,
            crate::p0_controls::TabBarThemeData::default()
        );
        write_field!(text_button_theme, ComponentThemeData::default());
        write_field!(text_selection_theme, ComponentThemeData::default());
        write_field!(time_picker_theme, ComponentThemeData::default());
        write_field!(toggle_buttons_theme, ComponentThemeData::default());
        write_field!(tooltip_theme, ComponentThemeData::default());
        write_field!(button_bar_theme, None);
        write_field!(extensions, BTreeMap::new());
        let mut shared = unsafe { shared.assume_init() };
        Rc::get_mut(&mut shared)
            .expect("new theme allocation remains uniquely owned")
            .with_control_defaults(control_theme);
        shared
    }

    #[must_use]
    pub fn from_color_scheme(color_scheme: ColorScheme) -> Self {
        let brightness = color_scheme.brightness;
        let text_theme = match brightness {
            Brightness::Light => TextTheme::light(),
            Brightness::Dark => TextTheme::dark(),
        };
        let control_theme = ControlTheme::default();
        let mut theme = Self {
            brightness,
            cupertino_override_theme: None,
            adaptation_map: BTreeMap::new(),
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
            color_scheme,
            icon_theme: ComponentThemeData::default(),
            primary_icon_theme: ComponentThemeData::default(),
            input_decoration_theme: InputDecorationThemeData::default(),
            action_icon_theme: None,
            app_bar_theme: ComponentThemeData::default(),
            badge_theme: ComponentThemeData::default(),
            banner_theme: ComponentThemeData::default(),
            bottom_app_bar_theme: ComponentThemeData::default(),
            bottom_navigation_bar_theme: ComponentThemeData::default(),
            bottom_sheet_theme: ComponentThemeData::default(),
            button_theme: ButtonThemeData::default(),
            card_theme: ComponentThemeData::default(),
            carousel_view_theme: ComponentThemeData::default(),
            checkbox_theme: ComponentThemeData::default(),
            chip_theme: ComponentThemeData::default(),
            data_table_theme: ComponentThemeData::default(),
            date_picker_theme: ComponentThemeData::default(),
            dialog_theme: ComponentThemeData::default(),
            divider_theme: ComponentThemeData::default(),
            drawer_theme: ComponentThemeData::default(),
            dropdown_menu_theme: crate::menus::DropdownMenuThemeData::default(),
            elevated_button_theme: ComponentThemeData::default(),
            expansion_tile_theme: ComponentThemeData::default(),
            filled_button_theme: ComponentThemeData::default(),
            floating_action_button_theme: ComponentThemeData::default(),
            icon_button_theme: ComponentThemeData::default(),
            list_tile_theme: ComponentThemeData::default(),
            menu_bar_theme: ComponentThemeData::default(),
            menu_button_theme: ComponentThemeData::default(),
            menu_theme: crate::menus::MenuThemeData::default(),
            navigation_bar_theme: ComponentThemeData::default(),
            navigation_drawer_theme: ComponentThemeData::default(),
            navigation_rail_theme: ComponentThemeData::default(),
            outlined_button_theme: ComponentThemeData::default(),
            popup_menu_theme: crate::menus::PopupMenuThemeData::default(),
            progress_indicator_theme: crate::feedback::ProgressIndicatorThemeData::default(),
            radio_theme: ComponentThemeData::default(),
            search_bar_theme: ComponentThemeData::default(),
            search_view_theme: ComponentThemeData::default(),
            segmented_button_theme: ComponentThemeData::default(),
            slider_theme: crate::p0_controls::SliderThemeData::default(),
            snack_bar_theme: ComponentThemeData::default(),
            switch_theme: ComponentThemeData::default(),
            tab_bar_theme: crate::p0_controls::TabBarThemeData::default(),
            text_button_theme: ComponentThemeData::default(),
            text_selection_theme: ComponentThemeData::default(),
            time_picker_theme: ComponentThemeData::default(),
            toggle_buttons_theme: ComponentThemeData::default(),
            tooltip_theme: ComponentThemeData::default(),
            button_bar_theme: None,
            extensions: BTreeMap::new(),
        };
        theme.with_control_defaults(control_theme);
        theme
    }

    fn with_control_defaults(&mut self, control_theme: ControlTheme) {
        self.elevated_button_theme.elevation = Some(control_theme.elevation.popup.min(6.0));
        self.card_theme.elevation = Some(control_theme.elevation.popup.min(1.0));
    }

    #[must_use]
    pub fn copy_with(mut self, patch: ThemeDataPatch) -> Self {
        if let Some(value) = patch.brightness {
            self.brightness = value;
        }
        if let Some(value) = patch.cupertino_override_theme {
            self.cupertino_override_theme = Some(value);
        }
        if let Some(value) = patch.adaptation_map {
            self.adaptation_map = value;
        }
        if let Some(value) = patch.color_scheme {
            self.brightness = value.brightness;
            self.color_scheme = value;
        }
        if let Some(value) = patch.text_theme {
            self.text_theme = value;
        }
        if let Some(value) = patch.primary_text_theme {
            self.primary_text_theme = value;
        }
        if let Some(value) = patch.icon_theme {
            self.icon_theme = value;
        }
        if let Some(value) = patch.primary_icon_theme {
            self.primary_icon_theme = value;
        }
        if let Some(value) = patch.text_selection_theme {
            self.text_selection_theme = value;
        }
        if let Some(value) = patch.use_material3 {
            self.use_material3 = value;
        }
        if let Some(value) = patch.visual_density {
            self.visual_density = value;
        }
        if let Some(value) = patch.material_tap_target_size {
            self.material_tap_target_size = value;
        }
        if let Some(value) = patch.platform {
            self.platform = value;
        }
        if let Some(value) = patch.page_transitions_theme {
            self.page_transitions_theme = value;
        }
        if let Some(value) = patch.splash_factory {
            self.splash_factory = value;
        }
        if let Some(value) = patch.input_decoration_theme {
            self.input_decoration_theme = value;
        }
        macro_rules! component_patch {
            ($field:ident) => {
                if let Some(value) = patch.$field {
                    self.$field = value;
                }
            };
        }
        component_patch!(badge_theme);
        component_patch!(app_bar_theme);
        component_patch!(bottom_app_bar_theme);
        component_patch!(bottom_navigation_bar_theme);
        component_patch!(card_theme);
        component_patch!(checkbox_theme);
        component_patch!(dialog_theme);
        component_patch!(divider_theme);
        component_patch!(drawer_theme);
        component_patch!(dropdown_menu_theme);
        component_patch!(elevated_button_theme);
        component_patch!(filled_button_theme);
        component_patch!(floating_action_button_theme);
        component_patch!(icon_button_theme);
        component_patch!(list_tile_theme);
        component_patch!(menu_bar_theme);
        component_patch!(menu_button_theme);
        component_patch!(menu_theme);
        component_patch!(navigation_bar_theme);
        component_patch!(navigation_drawer_theme);
        component_patch!(navigation_rail_theme);
        component_patch!(outlined_button_theme);
        component_patch!(popup_menu_theme);
        component_patch!(progress_indicator_theme);
        component_patch!(radio_theme);
        component_patch!(slider_theme);
        component_patch!(snack_bar_theme);
        component_patch!(switch_theme);
        component_patch!(tab_bar_theme);
        component_patch!(text_button_theme);
        component_patch!(tooltip_theme);
        if let Some(value) = patch.button_bar_theme {
            self.button_bar_theme = value;
        }
        self
    }

    #[must_use]
    pub fn with_color_scheme(mut self, value: ColorScheme) -> Self {
        self.brightness = value.brightness;
        self.color_scheme = value;
        self
    }
    #[must_use]
    pub fn with_use_material3(mut self, value: bool) -> Self {
        self.use_material3 = value;
        self
    }
    #[must_use]
    pub fn with_visual_density(mut self, value: VisualDensity) -> Self {
        self.visual_density = value;
        self
    }
    #[must_use]
    pub fn with_tap_target_size(mut self, value: MaterialTapTargetSize) -> Self {
        self.material_tap_target_size = value;
        self
    }
    #[must_use]
    pub fn with_platform(mut self, value: TargetPlatform) -> Self {
        self.platform = value;
        self
    }
    #[must_use]
    pub fn with_page_transitions_theme(mut self, value: PageTransitionsTheme) -> Self {
        self.page_transitions_theme = value;
        self
    }
    #[must_use]
    pub fn with_splash_factory(mut self, value: SplashFactory) -> Self {
        self.splash_factory = value;
        self
    }
    #[must_use]
    pub fn with_input_decoration_theme(mut self, value: InputDecorationThemeData) -> Self {
        self.input_decoration_theme = value;
        self
    }
    #[must_use]
    pub fn with_button_bar_theme(mut self, value: Option<ButtonBarThemeData>) -> Self {
        self.button_bar_theme = value;
        self
    }
    #[must_use]
    pub fn with_extension(mut self, key: impl Into<String>, value: ThemeExtensionValue) -> Self {
        self.extensions.insert(key.into(), value);
        self
    }

    #[must_use]
    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let mut result = if t < 0.5 { self.clone() } else { other.clone() };
        result.brightness = if t < 0.5 {
            self.brightness
        } else {
            other.brightness
        };
        result.color_scheme = self.color_scheme.lerp(other.color_scheme, t);
        result.text_theme = self.text_theme.lerp(&other.text_theme, t);
        result.primary_text_theme = self.primary_text_theme.lerp(&other.primary_text_theme, t);
        result.visual_density = self.visual_density.lerp(other.visual_density, t);
        result
    }

    /// Converts the Material theme into the existing controls theme. This is
    /// the only bridge from Material defaults to headless control behavior.
    #[must_use]
    pub fn control_theme(&self) -> ControlTheme {
        let mut controls = if self.brightness == Brightness::Light {
            ControlTheme::light()
        } else {
            ControlTheme::dark()
        };
        controls = controls.with_palette(incular_controls::ControlColors {
            background: self.scaffold_background_color,
            surface: self.color_scheme.surface,
            surface_variant: self.color_scheme.surface_container,
            surface_elevated: self.color_scheme.surface_container_high,
            surface_active: self.color_scheme.surface_container_highest,
            foreground: self.color_scheme.on_surface,
            foreground_muted: self.color_scheme.on_surface_variant,
            foreground_disabled: self.disabled_color,
            accent: self.color_scheme.primary,
            accent_hover: alpha(self.color_scheme.primary, 0.92),
            accent_active: self.color_scheme.primary_container,
            accent_foreground: self.color_scheme.on_primary,
            border: self.color_scheme.outline,
            border_subtle: self.color_scheme.outline_variant,
            border_strong: self.color_scheme.outline,
            hover_overlay: self.hover_color,
            pressed_overlay: self.splash_color,
            focus_ring: self.focus_color,
            selection: alpha(self.color_scheme.primary, 0.24),
            disabled_surface: self.color_scheme.surface_container_highest,
            disabled_foreground: self.disabled_color,
            error: self.color_scheme.error,
            warning: Color::rgba(160, 100, 0, 255),
            success: Color::rgba(30, 120, 60, 255),
            info: self.color_scheme.primary,
        });
        controls.density = if self.visual_density == VisualDensity::COMPACT {
            incular_controls::ControlDensity::Compact
        } else if self.visual_density == VisualDensity::COMFORTABLE {
            incular_controls::ControlDensity::Comfortable
        } else {
            incular_controls::ControlDensity::Standard
        };
        // Slider mechanics stay in `incular-controls`; Material's sparse
        // slider theme supplies the dimensions and common state colors used
        // by that shared renderer.
        if let Some(height) = self.slider_theme.track_height {
            controls.slider.track_height = height.max(1.0);
        }
        if let Some(size) = self.slider_theme.thumb_size {
            controls.slider.thumb_size = size.max(1.0);
        }
        if let Some(color) = self
            .slider_theme
            .active_track_color
            .as_ref()
            .map(|property| property.resolve(WidgetStates::default()))
        {
            controls.colors.accent = color;
        }
        if let Some(color) = self
            .slider_theme
            .inactive_track_color
            .as_ref()
            .map(|property| property.resolve(WidgetStates::default()))
        {
            controls.colors.border_strong = color;
        }
        if let Some(color) = self.slider_theme.disabled_active_track_color {
            controls.colors.disabled_foreground = color;
        }
        if let Some(size) = self.checkbox_theme.icon_size {
            controls.checkbox.indicator_size = size.max(1.0);
        }
        if let Some(size) = self.radio_theme.icon_size {
            controls.radio.indicator_size = size.max(1.0);
        }
        if let Some(width) = self.switch_theme.minimum_size.map(|size| size.width) {
            controls.switch.width = width.max(1.0);
        }
        if let Some(height) = self.switch_theme.minimum_size.map(|size| size.height) {
            controls.switch.height = height.max(1.0);
        }
        if let Some(size) = self.switch_theme.thumb_size {
            controls.switch.thumb_size = size.max(1.0);
        }
        if let Some(fill) = self
            .checkbox_theme
            .fill_color
            .as_ref()
            .map(|property| property.resolve(incular_controls::ControlState::empty()))
        {
            controls.colors.accent = fill;
        }
        if let Some(check) = self
            .checkbox_theme
            .check_color
            .as_ref()
            .map(|property| property.resolve(incular_controls::ControlState::empty()))
        {
            controls.colors.accent_foreground = check;
        }
        controls
    }
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
    pub fn of() -> Option<ThemeData> {
        Self::of_shared().map(|theme| (*theme).clone()).or_else(|| {
            incular_widgets::internal::current_build_environment_boxed::<ThemeData>()
                .map(|theme| *theme)
        })
    }

    /// Reads the ambient theme without copying the large theme descriptor onto
    /// the native UI stack. Material application roots use this shared form.
    #[must_use]
    pub(crate) fn of_shared() -> Option<Rc<ThemeData>> {
        incular_widgets::internal::current_build_environment::<Rc<ThemeData>>()
    }

    /// Installs one shared theme descriptor plus the derived control scopes.
    pub(crate) fn scope_shared(data: Rc<ThemeData>, child: Widget) -> Widget {
        let controls = data.control_theme();
        let input_decoration = data.input_decoration_theme.clone();
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
