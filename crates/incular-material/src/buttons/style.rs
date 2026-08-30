//! Material button style factories and variant defaults.

use incular_config::{Alignment, EdgeInsets};
use incular_controls::{
    ButtonLayerBuilder, ButtonStyle, ButtonVariant, ControlDensity, ControlState, SplashFactory,
    StateColor, StateTable, StateValue, TapTargetSize,
};
use incular_core::{Color, Size};
use incular_text::TextStyle;
use incular_widgets::{Border, BorderRadius, Widget};
use std::time::Duration;
use typed_builder::TypedBuilder;

/// Sparse values accepted by the Material `style_from` helpers.
///
/// This is intentionally separate from [`ButtonStyle`]. A style factory
/// supplies a component variant and leaves all unspecified values unset;
/// resolution can then apply component theme values and explicit overrides in
/// the same order as Flutter's `ButtonStyleButton`.
#[derive(Clone, Debug, Default, PartialEq, TypedBuilder)]
pub struct ButtonStyleConfig {
    #[builder(default, setter(strip_option, into))]
    pub background_color: Option<StateValue<Color>>,
    #[builder(default, setter(strip_option, into))]
    pub foreground_color: Option<StateValue<Color>>,
    #[builder(default, setter(strip_option, into))]
    pub overlay_color: Option<StateValue<Color>>,
    #[builder(default, setter(strip_option, into))]
    pub shadow_color: Option<StateValue<Color>>,
    #[builder(default, setter(strip_option, into))]
    pub surface_tint_color: Option<StateValue<Color>>,
    #[builder(default, setter(strip_option, into))]
    pub elevation: Option<StateValue<f32>>,
    #[builder(default, setter(strip_option, into))]
    pub padding: Option<EdgeInsets>,
    #[builder(default, setter(strip_option, into))]
    pub minimum_size: Option<Size>,
    #[builder(default, setter(strip_option, into))]
    pub fixed_size: Option<Size>,
    #[builder(default, setter(strip_option, into))]
    pub maximum_size: Option<Size>,
    #[builder(default, setter(strip_option, into))]
    pub icon_color: Option<StateValue<Color>>,
    #[builder(default, setter(strip_option, into))]
    pub icon_size: Option<StateValue<f32>>,
    #[builder(default, setter(strip_option, into))]
    pub icon_alignment: Option<incular_controls::IconAlignment>,
    #[builder(default, setter(strip_option, into))]
    pub side: Option<StateValue<Border>>,
    #[builder(default, setter(strip_option, into))]
    pub shape: Option<StateValue<BorderRadius>>,
    #[builder(default, setter(strip_option, into))]
    pub mouse_cursor: Option<String>,
    #[builder(default, setter(strip_option, into))]
    pub visual_density: Option<ControlDensity>,
    #[builder(default, setter(strip_option, into))]
    pub tap_target_size: Option<TapTargetSize>,
    #[builder(default, setter(strip_option, into))]
    pub animation_duration: Option<Duration>,
    #[builder(default, setter(strip_option, into))]
    pub enable_feedback: Option<bool>,
    #[builder(default, setter(strip_option, into))]
    pub alignment: Option<Alignment>,
    #[builder(default, setter(strip_option, into))]
    pub splash_factory: Option<SplashFactory>,
    #[builder(
        default,
        setter(
            fn transform<F>(value: F) -> Option<ButtonLayerBuilder>
            where
                F: Fn(Widget, ControlState) -> Widget + 'static,
            {
                Some(ButtonLayerBuilder::new(value))
            }
        )
    )]
    pub background_builder: Option<ButtonLayerBuilder>,
    #[builder(
        default,
        setter(
            fn transform<F>(value: F) -> Option<ButtonLayerBuilder>
            where
                F: Fn(Widget, ControlState) -> Widget + 'static,
            {
                Some(ButtonLayerBuilder::new(value))
            }
        )
    )]
    pub foreground_builder: Option<ButtonLayerBuilder>,
    #[builder(default, setter(strip_option, into))]
    pub text_style: Option<TextStyle>,
}

impl ButtonStyleConfig {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn background_color(mut self, value: impl Into<StateValue<Color>>) -> Self {
        self.background_color = Some(value.into());
        self
    }

    #[must_use]
    pub fn foreground_color(mut self, value: impl Into<StateValue<Color>>) -> Self {
        self.foreground_color = Some(value.into());
        self
    }

    #[must_use]
    pub fn overlay_color(mut self, value: impl Into<StateValue<Color>>) -> Self {
        self.overlay_color = Some(value.into());
        self
    }

    #[must_use]
    pub fn shadow_color(mut self, value: impl Into<StateValue<Color>>) -> Self {
        self.shadow_color = Some(value.into());
        self
    }

    #[must_use]
    pub fn surface_tint_color(mut self, value: impl Into<StateValue<Color>>) -> Self {
        self.surface_tint_color = Some(value.into());
        self
    }

    #[must_use]
    pub fn elevation(mut self, value: impl Into<StateValue<f32>>) -> Self {
        self.elevation = Some(value.into());
        self
    }

    #[must_use]
    pub fn padding(mut self, value: EdgeInsets) -> Self {
        self.padding = Some(value);
        self
    }

    #[must_use]
    pub fn minimum_size(mut self, value: Size) -> Self {
        self.minimum_size = Some(value);
        self
    }

    #[must_use]
    pub fn fixed_size(mut self, value: Size) -> Self {
        self.fixed_size = Some(value);
        self
    }

    #[must_use]
    pub fn maximum_size(mut self, value: Size) -> Self {
        self.maximum_size = Some(value);
        self
    }

    #[must_use]
    pub fn icon_color(mut self, value: impl Into<StateValue<Color>>) -> Self {
        self.icon_color = Some(value.into());
        self
    }

    #[must_use]
    pub fn icon_size(mut self, value: impl Into<StateValue<f32>>) -> Self {
        self.icon_size = Some(value.into());
        self
    }

    #[must_use]
    pub fn icon_alignment(mut self, value: incular_controls::IconAlignment) -> Self {
        self.icon_alignment = Some(value);
        self
    }

    #[must_use]
    pub fn side(mut self, value: impl Into<StateValue<Border>>) -> Self {
        self.side = Some(value.into());
        self
    }

    #[must_use]
    pub fn shape(mut self, value: impl Into<StateValue<BorderRadius>>) -> Self {
        self.shape = Some(value.into());
        self
    }

    #[must_use]
    pub fn mouse_cursor(mut self, value: impl Into<String>) -> Self {
        self.mouse_cursor = Some(value.into());
        self
    }

    #[must_use]
    pub fn visual_density(mut self, value: ControlDensity) -> Self {
        self.visual_density = Some(value);
        self
    }

    #[must_use]
    pub fn tap_target_size(mut self, value: TapTargetSize) -> Self {
        self.tap_target_size = Some(value);
        self
    }

    #[must_use]
    pub fn animation_duration(mut self, value: Duration) -> Self {
        self.animation_duration = Some(value);
        self
    }

    #[must_use]
    pub fn enable_feedback(mut self, value: bool) -> Self {
        self.enable_feedback = Some(value);
        self
    }

    #[must_use]
    pub fn alignment(mut self, value: Alignment) -> Self {
        self.alignment = Some(value);
        self
    }

    #[must_use]
    pub fn splash_factory(mut self, value: SplashFactory) -> Self {
        self.splash_factory = Some(value);
        self
    }

    #[must_use]
    pub fn background_builder(
        mut self,
        value: impl Fn(Widget, ControlState) -> Widget + 'static,
    ) -> Self {
        self.background_builder = Some(ButtonLayerBuilder::new(value));
        self
    }

    #[must_use]
    pub fn foreground_builder(
        mut self,
        value: impl Fn(Widget, ControlState) -> Widget + 'static,
    ) -> Self {
        self.foreground_builder = Some(ButtonLayerBuilder::new(value));
        self
    }

    #[must_use]
    pub fn text_style(mut self, value: TextStyle) -> Self {
        self.text_style = Some(value);
        self
    }

    /// Converts this sparse factory input into the controls style used by a
    /// concrete Material button family.
    #[must_use]
    pub fn into_style(self, variant: ButtonVariant) -> ButtonStyle {
        let mut style = ButtonStyle::new().variant(variant);

        if let Some(value) = self.background_color {
            apply_color_property(&mut style, value, true);
        }
        if let Some(value) = self.foreground_color {
            apply_color_property(&mut style, value, false);
        }
        if let Some(value) = self.overlay_color {
            style = style.overlay_color(value);
        }
        if let Some(value) = self.shadow_color {
            style = style.shadow_color(value);
        }
        if let Some(value) = self.surface_tint_color {
            style = style.surface_tint_color(value);
        }
        if let Some(value) = self.elevation {
            style = style.elevation(value);
        }
        if let Some(value) = self.padding {
            style = style.padding(value);
        }
        if let Some(value) = self.minimum_size {
            style = style.minimum_size(value);
        }
        if let Some(value) = self.fixed_size {
            style = style.fixed_size(value);
        }
        if let Some(value) = self.maximum_size {
            style = style.maximum_size(value);
        }
        if let Some(value) = self.icon_color {
            style = style.icon_color(value);
        }
        if let Some(value) = self.icon_size {
            style = style.icon_size(value);
        }
        if let Some(value) = self.icon_alignment {
            style = style.icon_alignment(value);
        }
        if let Some(value) = self.side {
            style = style.side(value);
        }
        if let Some(value) = self.shape {
            style = style.shape(value);
        }
        if let Some(value) = self.mouse_cursor {
            style = style.mouse_cursor(value);
        }
        if let Some(value) = self.visual_density {
            style = style.visual_density(value);
        }
        if let Some(value) = self.tap_target_size {
            style = style.tap_target_size(value);
        }
        if let Some(value) = self.animation_duration {
            style = style.animation_duration(value);
        }
        if let Some(value) = self.enable_feedback {
            style = style.enable_feedback(value);
        }
        if let Some(value) = self.alignment {
            style = style.alignment(value);
        }
        if let Some(value) = self.splash_factory {
            style = style.splash_factory(value);
        }
        if let Some(value) = self.background_builder {
            style.background_builder = Some(value);
        }
        if let Some(value) = self.foreground_builder {
            style.foreground_builder = Some(value);
        }
        if let Some(value) = self.text_style {
            style = style.text_style(value);
        }
        style
    }
}

/// Alias emphasizing that this is the argument object for `style_from`.
pub type StyleFrom = ButtonStyleConfig;

/// Converts a sparse style factory object using the neutral button variant.
#[must_use]
pub fn style_from(config: ButtonStyleConfig) -> ButtonStyle {
    config.into_style(ButtonVariant::Default)
}

fn apply_color_property(style: &mut ButtonStyle, value: StateValue<Color>, background: bool) {
    match value {
        StateValue::Value(value) => {
            if background {
                style.background = Some(value);
            } else {
                style.foreground = Some(value);
            }
        }
        StateValue::States(table) => {
            let colors = StateColorFromTable::from(table);
            if background {
                style.background_states = Some(colors.0);
            } else {
                style.foreground_states = Some(colors.0);
            }
        }
        StateValue::Resolver(resolver) => {
            if background {
                style.background_property = Some(StateValue::Resolver(resolver));
            } else {
                style.foreground_property = Some(StateValue::Resolver(resolver));
            }
        }
    }
}

struct StateColorFromTable(StateColor);

impl From<StateTable<Color>> for StateColorFromTable {
    fn from(table: StateTable<Color>) -> Self {
        Self(incular_controls::StateColor {
            normal: table.normal,
            hovered: table.hovered,
            pressed: table.pressed,
            focused: table.focused,
            disabled: table.disabled,
            checked: table.checked,
            selected: table.selected,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ButtonKind {
    Elevated,
    Filled,
    FilledTonal,
    Outlined,
    Text,
    Icon,
    Floating,
}

impl ButtonKind {
    pub(super) const fn variant(self) -> ButtonVariant {
        match self {
            Self::Outlined => ButtonVariant::Default,
            Self::Text | Self::Icon => ButtonVariant::Ghost,
            Self::Elevated | Self::Filled | Self::FilledTonal | Self::Floating => {
                ButtonVariant::Primary
            }
        }
    }

    pub(super) fn default_style(self) -> ButtonStyle {
        let mut style = ButtonStyle::new()
            .variant(self.variant())
            .minimum_size(Size::new(64.0, 36.0))
            .padding(EdgeInsets::symmetric(24.0, 8.0))
            .tap_target_size(TapTargetSize::Padded)
            .animation_duration(Duration::from_millis(200))
            .enable_feedback(true)
            .alignment(Alignment::CENTER)
            .splash_factory(SplashFactory::Ripple);

        match self {
            Self::Outlined => {}
            Self::Text => {
                style = style.padding(EdgeInsets::symmetric(12.0, 8.0));
            }
            Self::Icon => {
                style = style
                    .minimum_size(Size::new(48.0, 48.0))
                    .padding(EdgeInsets::all(8.0))
                    .border_radius(24.0);
            }
            Self::Floating => {
                style = style
                    .minimum_size(Size::new(56.0, 56.0))
                    .padding(EdgeInsets::all(16.0))
                    .border_radius(28.0);
            }
            Self::Elevated | Self::Filled | Self::FilledTonal => {}
        }
        style
    }
}
