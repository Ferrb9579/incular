use incular_config::EdgeInsets;
use incular_core::Color;
use incular_text::{FontWeight, TextStyle};

/// Desktop UI density levels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ControlDensity {
    /// Compact density for dense data grids, toolbars, and inspector panels.
    Compact,
    /// Standard density for desktop productivity software (default).
    #[default]
    Standard,
    /// Comfortable density with larger hit targets.
    Comfortable,
}

impl ControlDensity {
    /// Returns the effective control height in logical pixels.
    #[must_use]
    pub const fn control_height(self) -> f32 {
        match self {
            Self::Compact => 26.0,
            Self::Standard => 32.0,
            Self::Comfortable => 38.0,
        }
    }

    /// Returns the effective horizontal/vertical padding.
    #[must_use]
    pub const fn padding(self) -> EdgeInsets {
        match self {
            Self::Compact => EdgeInsets::symmetric(8.0, 4.0),
            Self::Standard => EdgeInsets::symmetric(12.0, 6.0),
            Self::Comfortable => EdgeInsets::symmetric(16.0, 8.0),
        }
    }
}

/// Semantic color palette for generic controls.
#[derive(Clone, Debug, PartialEq)]
pub struct ControlColors {
    pub background: Color,
    pub surface: Color,
    pub surface_variant: Color,
    pub surface_elevated: Color,
    pub surface_active: Color,

    pub foreground: Color,
    pub foreground_muted: Color,
    pub foreground_disabled: Color,

    pub accent: Color,
    pub accent_hover: Color,
    pub accent_active: Color,
    pub accent_foreground: Color,

    pub border: Color,
    pub border_subtle: Color,
    pub border_strong: Color,

    pub hover_overlay: Color,
    pub pressed_overlay: Color,
    pub focus_ring: Color,
    pub selection: Color,

    pub disabled_surface: Color,
    pub disabled_foreground: Color,

    pub error: Color,
    pub warning: Color,
    pub success: Color,
    pub info: Color,
}

impl ControlColors {
    /// Light theme color palette with verified contrast ratios.
    #[must_use]
    pub fn light() -> Self {
        Self {
            background: Color::rgba(248, 249, 251, 255),
            surface: Color::rgba(255, 255, 255, 255),
            surface_variant: Color::rgba(240, 242, 246, 255),
            surface_elevated: Color::rgba(245, 247, 250, 255),
            surface_active: Color::rgba(230, 234, 242, 255),

            foreground: Color::rgba(28, 32, 40, 255),
            foreground_muted: Color::rgba(100, 108, 125, 255),
            foreground_disabled: Color::rgba(165, 172, 185, 255),

            accent: Color::rgba(40, 110, 230, 255),
            accent_hover: Color::rgba(55, 125, 245, 255),
            accent_active: Color::rgba(30, 95, 210, 255),
            accent_foreground: Color::rgba(255, 255, 255, 255),

            border: Color::rgba(220, 225, 235, 255),
            border_subtle: Color::rgba(235, 238, 245, 255),
            border_strong: Color::rgba(180, 188, 202, 255),

            hover_overlay: Color::rgba(0, 0, 0, 12),
            pressed_overlay: Color::rgba(0, 0, 0, 24),
            focus_ring: Color::rgba(40, 110, 230, 180),
            selection: Color::rgba(40, 110, 230, 60),

            disabled_surface: Color::rgba(240, 242, 246, 180),
            disabled_foreground: Color::rgba(165, 172, 185, 255),

            error: Color::rgba(220, 50, 50, 255),
            warning: Color::rgba(225, 140, 20, 255),
            success: Color::rgba(40, 165, 75, 255),
            info: Color::rgba(40, 120, 220, 255),
        }
    }

    /// Dark theme color palette with verified neutral contrast.
    #[must_use]
    pub fn dark() -> Self {
        Self {
            background: Color::rgba(18, 20, 24, 255),
            surface: Color::rgba(26, 28, 34, 255),
            surface_variant: Color::rgba(34, 37, 45, 255),
            surface_elevated: Color::rgba(42, 45, 56, 255),
            surface_active: Color::rgba(48, 52, 65, 255),

            foreground: Color::rgba(235, 238, 245, 255),
            foreground_muted: Color::rgba(150, 156, 172, 255),
            foreground_disabled: Color::rgba(90, 95, 110, 255),

            accent: Color::rgba(65, 135, 245, 255),
            accent_hover: Color::rgba(85, 150, 255, 255),
            accent_active: Color::rgba(50, 115, 225, 255),
            accent_foreground: Color::rgba(255, 255, 255, 255),

            border: Color::rgba(50, 54, 68, 255),
            border_subtle: Color::rgba(38, 41, 52, 255),
            border_strong: Color::rgba(75, 82, 102, 255),

            hover_overlay: Color::rgba(255, 255, 255, 16),
            pressed_overlay: Color::rgba(255, 255, 255, 28),
            focus_ring: Color::rgba(85, 150, 255, 200),
            selection: Color::rgba(65, 135, 245, 75),

            disabled_surface: Color::rgba(30, 32, 40, 180),
            disabled_foreground: Color::rgba(90, 95, 110, 255),

            error: Color::rgba(240, 80, 80, 255),
            warning: Color::rgba(245, 165, 45, 255),
            success: Color::rgba(60, 190, 95, 255),
            info: Color::rgba(75, 150, 245, 255),
        }
    }
}

/// Generic desktop typography hierarchy.
#[derive(Clone, Debug, PartialEq)]
pub struct ControlTypography {
    pub caption: TextStyle,
    pub small: TextStyle,
    pub body: TextStyle,
    pub body_emphasis: TextStyle,
    pub title: TextStyle,
    pub heading: TextStyle,
    pub monospace: TextStyle,
}

impl ControlTypography {
    #[must_use]
    pub fn new(fg: Color, muted: Color) -> Self {
        Self {
            caption: TextStyle::new()
                .font_size(10.0)
                .line_height_multiplier(1.3)
                .color(muted),
            small: TextStyle::new()
                .font_size(11.0)
                .line_height_multiplier(1.35)
                .color(muted),
            body: TextStyle::new()
                .font_size(13.0)
                .line_height_multiplier(1.4)
                .color(fg),
            body_emphasis: TextStyle::new()
                .font_size(13.0)
                .font_weight(FontWeight::BOLD)
                .line_height_multiplier(1.4)
                .color(fg),
            title: TextStyle::new()
                .font_size(15.0)
                .font_weight(FontWeight::BOLD)
                .line_height_multiplier(1.3)
                .color(fg),
            heading: TextStyle::new()
                .font_size(18.0)
                .font_weight(FontWeight::BOLD)
                .line_height_multiplier(1.25)
                .color(fg),
            monospace: TextStyle::new()
                .font_size(12.0)
                .line_height_multiplier(1.45)
                .color(fg),
        }
    }
}

/// Generic control dimensions, radii, and spacing tokens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ControlMetrics {
    pub control_height: f32,
    pub horizontal_padding: f32,
    pub vertical_padding: f32,
    pub border_radius: f32,
    pub border_width: f32,
    pub focus_ring_width: f32,
    pub icon_size: f32,
    pub scrollbar_thickness: f32,
    pub small_spacing: f32,
    pub medium_spacing: f32,
    pub large_spacing: f32,
}

impl Default for ControlMetrics {
    fn default() -> Self {
        Self {
            control_height: 32.0,
            horizontal_padding: 12.0,
            vertical_padding: 6.0,
            border_radius: 4.0,
            border_width: 1.0,
            focus_ring_width: 2.0,
            icon_size: 16.0,
            scrollbar_thickness: 8.0,
            small_spacing: 6.0,
            medium_spacing: 12.0,
            large_spacing: 24.0,
        }
    }
}

/// Animation and transition durations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ControlMotion {
    pub hover_duration_ms: u32,
    pub pressed_duration_ms: u32,
    pub focus_duration_ms: u32,
    pub reduced_motion: bool,
}

impl Default for ControlMotion {
    fn default() -> Self {
        Self {
            hover_duration_ms: 100,
            pressed_duration_ms: 50,
            focus_duration_ms: 120,
            reduced_motion: false,
        }
    }
}

/// Public semantic-token names used by the Base UI-inspired preset. The
/// aliases keep older `colors`/`typography` code source compatible while
/// giving new controls a vocabulary that is independent of a renderer.
pub type PaletteTokens = ControlColors;
pub type TypographyTokens = ControlTypography;
pub type MotionTokens = ControlMotion;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpacingTokens {
    pub xs: f32,
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
    pub xl: f32,
}

impl Default for SpacingTokens {
    fn default() -> Self {
        Self {
            xs: 4.,
            sm: 8.,
            md: 12.,
            lg: 16.,
            xl: 24.,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RadiusTokens {
    pub none: f32,
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
    pub pill: f32,
}

impl Default for RadiusTokens {
    fn default() -> Self {
        Self {
            none: 0.,
            sm: 3.,
            md: 5.,
            lg: 8.,
            pill: 999.,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ElevationTokens {
    pub none: f32,
    pub popup: f32,
    pub dialog: f32,
}

impl Default for ElevationTokens {
    fn default() -> Self {
        Self {
            none: 0.,
            popup: 8.,
            dialog: 24.,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ButtonTheme {
    pub height: f32,
    pub radius: f32,
    pub border_width: f32,
    pub focus_ring_width: f32,
}

impl Default for ButtonTheme {
    fn default() -> Self {
        Self {
            height: 32.,
            radius: 5.,
            border_width: 1.,
            focus_ring_width: 2.,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InputTheme {
    pub height: f32,
    pub radius: f32,
    pub border_width: f32,
    pub focus_ring_width: f32,
}

impl Default for InputTheme {
    fn default() -> Self {
        Self {
            height: 32.,
            radius: 5.,
            border_width: 1.,
            focus_ring_width: 2.,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CheckboxTheme {
    pub indicator_size: f32,
    pub radius: f32,
}

impl Default for CheckboxTheme {
    fn default() -> Self {
        Self {
            indicator_size: 18.,
            radius: 4.,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RadioTheme {
    pub indicator_size: f32,
}

impl Default for RadioTheme {
    fn default() -> Self {
        Self {
            indicator_size: 18.,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SwitchTheme {
    pub width: f32,
    pub height: f32,
    pub thumb_size: f32,
}

impl Default for SwitchTheme {
    fn default() -> Self {
        Self {
            width: 36.,
            height: 20.,
            thumb_size: 16.,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PopupTheme {
    pub radius: f32,
    pub elevation: f32,
    pub padding: f32,
}

impl Default for PopupTheme {
    fn default() -> Self {
        Self {
            radius: 6.,
            elevation: 8.,
            padding: 6.,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollbarTheme {
    pub width: f32,
    pub min_thumb_extent: f32,
}

impl Default for ScrollbarTheme {
    fn default() -> Self {
        Self {
            width: 8.,
            min_thumb_extent: 24.,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SliderTheme {
    pub track_height: f32,
    pub thumb_size: f32,
    pub radius: f32,
}

impl Default for SliderTheme {
    fn default() -> Self {
        Self {
            track_height: 6.,
            thumb_size: 16.,
            radius: 3.,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MenuTheme {
    pub item_height: f32,
    pub popup_padding: f32,
    pub radius: f32,
}

impl Default for MenuTheme {
    fn default() -> Self {
        Self {
            item_height: 32.,
            popup_padding: 4.,
            radius: 6.,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TooltipTheme {
    pub radius: f32,
    pub padding: f32,
    pub show_delay_ms: u32,
    pub hide_delay_ms: u32,
}

impl Default for TooltipTheme {
    fn default() -> Self {
        Self {
            radius: 5.,
            padding: 8.,
            show_delay_ms: 500,
            hide_delay_ms: 100,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TabsTheme {
    pub tab_height: f32,
    pub indicator_height: f32,
    pub gap: f32,
}

impl Default for TabsTheme {
    fn default() -> Self {
        Self {
            tab_height: 32.,
            indicator_height: 2.,
            gap: 4.,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollAreaTheme {
    pub scrollbar_gap: f32,
    pub corner_size: f32,
}

impl Default for ScrollAreaTheme {
    fn default() -> Self {
        Self {
            scrollbar_gap: 2.,
            corner_size: 8.,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProgressTheme {
    pub height: f32,
    pub radius: f32,
    pub track_alpha: u8,
}

impl Default for ProgressTheme {
    fn default() -> Self {
        Self {
            height: 8.,
            radius: 4.,
            track_alpha: 32,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AvatarTheme {
    pub size: f32,
    pub radius: f32,
    pub border_width: f32,
}

impl Default for AvatarTheme {
    fn default() -> Self {
        Self {
            size: 32.,
            radius: 16.,
            border_width: 1.,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToastTheme {
    pub width: f32,
    pub radius: f32,
    pub padding: f32,
}

impl Default for ToastTheme {
    fn default() -> Self {
        Self {
            width: 320.,
            radius: 6.,
            padding: 12.,
        }
    }
}

/// Meter uses the same geometry as Progress but has distinct semantic intent.
pub type MeterTheme = ProgressTheme;

/// Toast and popup-family controls share the same surface geometry. Toast
/// additionally exposes a width and content padding for the default desktop
/// presentation.
pub type DrawerTheme = PopupTheme;
pub type AlertDialogTheme = PopupTheme;
pub type AutocompleteTheme = PopupTheme;
pub type ContextMenuTheme = PopupTheme;
pub type MenubarTheme = PopupTheme;
pub type NavigationMenuTheme = PopupTheme;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroupTheme {
    pub spacing: f32,
    pub radius: f32,
}

impl Default for GroupTheme {
    fn default() -> Self {
        Self {
            spacing: 6.,
            radius: 5.,
        }
    }
}

pub type CheckboxGroupTheme = GroupTheme;
pub type ToggleGroupTheme = GroupTheme;

/// Full generic design token theme.
#[derive(Clone, Debug, PartialEq)]
pub struct ControlTheme {
    /// Base UI-inspired semantic tokens. `colors` remains as a compatibility
    /// alias for applications written against the first controls release.
    pub palette: PaletteTokens,
    pub colors: ControlColors,
    pub typography: ControlTypography,
    pub spacing: SpacingTokens,
    pub radius: RadiusTokens,
    pub elevation: ElevationTokens,
    pub metrics: ControlMetrics,
    pub motion: ControlMotion,
    pub density: ControlDensity,
    pub button: ButtonTheme,
    pub input: InputTheme,
    pub checkbox: CheckboxTheme,
    pub radio: RadioTheme,
    pub switch: SwitchTheme,
    pub popup: PopupTheme,
    pub scrollbar: ScrollbarTheme,
    pub slider: SliderTheme,
    pub menu: MenuTheme,
    pub tooltip: TooltipTheme,
    pub tabs: TabsTheme,
    pub scroll_area: ScrollAreaTheme,
    pub progress: ProgressTheme,
    pub avatar: AvatarTheme,
    pub toast: ToastTheme,
    pub meter: MeterTheme,
    pub drawer: DrawerTheme,
    pub alert_dialog: AlertDialogTheme,
    pub autocomplete: AutocompleteTheme,
    pub context_menu: ContextMenuTheme,
    pub menubar: MenubarTheme,
    pub navigation_menu: NavigationMenuTheme,
    pub checkbox_group: CheckboxGroupTheme,
    pub toggle_group: ToggleGroupTheme,
}

impl ControlTheme {
    /// Constructs the standard generic light control theme.
    #[must_use]
    pub fn light() -> Self {
        let colors = ControlColors::light();
        let typography = ControlTypography::new(colors.foreground, colors.foreground_muted);
        Self {
            palette: colors.clone(),
            colors,
            typography,
            spacing: SpacingTokens::default(),
            radius: RadiusTokens::default(),
            elevation: ElevationTokens::default(),
            metrics: ControlMetrics::default(),
            motion: ControlMotion::default(),
            density: ControlDensity::Standard,
            button: ButtonTheme::default(),
            input: InputTheme::default(),
            checkbox: CheckboxTheme::default(),
            radio: RadioTheme::default(),
            switch: SwitchTheme::default(),
            popup: PopupTheme::default(),
            scrollbar: ScrollbarTheme::default(),
            slider: SliderTheme::default(),
            menu: MenuTheme::default(),
            tooltip: TooltipTheme::default(),
            tabs: TabsTheme::default(),
            scroll_area: ScrollAreaTheme::default(),
            progress: ProgressTheme::default(),
            avatar: AvatarTheme::default(),
            toast: ToastTheme::default(),
            meter: MeterTheme::default(),
            drawer: DrawerTheme::default(),
            alert_dialog: AlertDialogTheme::default(),
            autocomplete: AutocompleteTheme::default(),
            context_menu: ContextMenuTheme::default(),
            menubar: MenubarTheme::default(),
            navigation_menu: NavigationMenuTheme::default(),
            checkbox_group: CheckboxGroupTheme::default(),
            toggle_group: ToggleGroupTheme::default(),
        }
    }

    /// Constructs the standard generic dark control theme.
    #[must_use]
    pub fn dark() -> Self {
        let colors = ControlColors::dark();
        let typography = ControlTypography::new(colors.foreground, colors.foreground_muted);
        Self {
            palette: colors.clone(),
            colors,
            typography,
            spacing: SpacingTokens::default(),
            radius: RadiusTokens::default(),
            elevation: ElevationTokens::default(),
            metrics: ControlMetrics::default(),
            motion: ControlMotion::default(),
            density: ControlDensity::Standard,
            button: ButtonTheme::default(),
            input: InputTheme::default(),
            checkbox: CheckboxTheme::default(),
            radio: RadioTheme::default(),
            switch: SwitchTheme::default(),
            popup: PopupTheme::default(),
            scrollbar: ScrollbarTheme::default(),
            slider: SliderTheme::default(),
            menu: MenuTheme::default(),
            tooltip: TooltipTheme::default(),
            tabs: TabsTheme::default(),
            scroll_area: ScrollAreaTheme::default(),
            progress: ProgressTheme::default(),
            avatar: AvatarTheme::default(),
            toast: ToastTheme::default(),
            meter: MeterTheme::default(),
            drawer: DrawerTheme::default(),
            alert_dialog: AlertDialogTheme::default(),
            autocomplete: AutocompleteTheme::default(),
            context_menu: ContextMenuTheme::default(),
            menubar: MenubarTheme::default(),
            navigation_menu: NavigationMenuTheme::default(),
            checkbox_group: CheckboxGroupTheme::default(),
            toggle_group: ToggleGroupTheme::default(),
        }
    }

    /// Sets the density for this theme.
    #[must_use]
    pub fn density(mut self, density: ControlDensity) -> Self {
        self.density = density;
        self.metrics.control_height = density.control_height();
        self.button.height = density.control_height();
        self.input.height = density.control_height();
        self
    }

    /// Replaces the semantic palette while keeping the compatibility
    /// `colors` view and default typography in sync.
    #[must_use]
    pub fn with_palette(mut self, palette: PaletteTokens) -> Self {
        self.typography = ControlTypography::new(palette.foreground, palette.foreground_muted);
        self.colors = palette.clone();
        self.palette = palette;
        self
    }
}

impl Default for ControlTheme {
    fn default() -> Self {
        Self::dark()
    }
}

/// Reads the nearest retained control theme and registers the caller as a
/// dependent of that exact inherited scope.
#[must_use]
pub fn current_control_theme(context: &incular_widgets::BuildContext<'_>) -> ControlTheme {
    context.depend_on::<ControlTheme>().unwrap_or_default()
}
