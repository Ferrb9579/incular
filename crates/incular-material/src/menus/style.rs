use crate::PopupMenuPosition;
use crate::foundation::InputDecorationThemeData;
use incular_config::{Alignment, Constraints, EdgeInsets};
use incular_controls::{ControlState, StateValue};
use incular_core::{Color, Size};
use incular_text::TextStyle;
use incular_widgets::{Border, BorderRadius, Container, Widget};
use typed_builder::TypedBuilder;

/// State-aware visual properties used by Material menus.
///
/// This mirrors Flutter's `MenuStyle` property surface while using the shared
/// `StateValue` resolver from `incular-controls`.  The fields remain sparse so
/// component themes can be merged without replacing unrelated defaults.
#[derive(Clone, Debug, Default, PartialEq, TypedBuilder)]
pub struct MenuStyle {
    #[builder(default, setter(strip_option, into))]
    pub background_color: Option<StateValue<Color>>,
    #[builder(default, setter(strip_option, into))]
    pub shadow_color: Option<StateValue<Color>>,
    #[builder(default, setter(strip_option, into))]
    pub surface_tint_color: Option<StateValue<Color>>,
    #[builder(default, setter(transform = |value: impl Into<StateValue<f32>>| Some(value.into())))]
    pub elevation: Option<StateValue<f32>>,
    #[builder(default, setter(strip_option))]
    pub padding: Option<EdgeInsets>,
    #[builder(default, setter(strip_option))]
    pub minimum_size: Option<Size>,
    #[builder(default, setter(strip_option))]
    pub fixed_size: Option<Size>,
    #[builder(default, setter(strip_option))]
    pub maximum_size: Option<Size>,
    #[builder(default, setter(strip_option, into))]
    pub side: Option<StateValue<Border>>,
    #[builder(default, setter(strip_option, into))]
    pub shape: Option<StateValue<BorderRadius>>,
    #[builder(default, setter(strip_option, into))]
    pub mouse_cursor: Option<String>,
    #[builder(default, setter(strip_option))]
    pub alignment: Option<Alignment>,
}

impl MenuStyle {
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
    pub fn alignment(mut self, value: Alignment) -> Self {
        self.alignment = Some(value);
        self
    }

    /// Fills only fields that are not already set on this style.
    #[must_use]
    pub fn merge(mut self, other: &Self) -> Self {
        macro_rules! fill {
            ($field:ident) => {
                if self.$field.is_none() {
                    self.$field = other.$field.clone();
                }
            };
        }
        fill!(background_color);
        fill!(shadow_color);
        fill!(surface_tint_color);
        fill!(elevation);
        fill!(padding);
        fill!(minimum_size);
        fill!(fixed_size);
        fill!(maximum_size);
        fill!(side);
        fill!(shape);
        fill!(mouse_cursor);
        fill!(alignment);
        self
    }

    #[must_use]
    pub(super) fn resolve_background(
        &self,
        state: ControlState,
        theme: &incular_controls::ControlTheme,
    ) -> Color {
        self.background_color
            .as_ref()
            .map_or(theme.colors.surface_elevated, |value| value.resolve(state))
    }

    #[must_use]
    pub(super) fn resolve_shadow(
        &self,
        state: ControlState,
        _theme: &incular_controls::ControlTheme,
    ) -> Color {
        self.shadow_color
            .as_ref()
            .map_or(Color::BLACK, |value| value.resolve(state))
    }

    #[must_use]
    pub(super) fn resolve_tint(&self, state: ControlState) -> Option<Color> {
        self.surface_tint_color
            .as_ref()
            .map(|value| value.resolve(state))
    }

    #[must_use]
    pub(super) fn resolve_elevation(&self, state: ControlState) -> f32 {
        self.elevation
            .as_ref()
            .map_or(8.0, |value| value.resolve(state).max(0.0))
    }

    #[must_use]
    pub(super) fn resolve_shape(&self, state: ControlState) -> BorderRadius {
        self.shape
            .as_ref()
            .map_or_else(|| BorderRadius::circular(4.0), |value| value.resolve(state))
    }

    #[must_use]
    pub(super) fn constrained(&self, widget: Widget) -> Widget {
        if let Some(fixed) = self.fixed_size {
            return Container::with_child(widget)
                .constraints(Constraints::tight(fixed))
                .into();
        }
        let min = self.minimum_size.unwrap_or(Size::ZERO);
        let max_width = self
            .maximum_size
            .map_or(f32::INFINITY, |size| size.width)
            .max(min.width);
        let max_height = self
            .maximum_size
            .map_or(f32::INFINITY, |size| size.height)
            .max(min.height);
        if min == Size::ZERO && !max_width.is_finite() && !max_height.is_finite() {
            widget
        } else {
            Container::with_child(widget)
                .constraints(Constraints::new(
                    min.width.max(0.0),
                    max_width,
                    min.height.max(0.0),
                    max_height,
                ))
                .into()
        }
    }
}

/// Theme data for `MenuAnchor`, `MenuItemButton`, and `SubmenuButton`.
#[derive(Clone, Debug, Default, PartialEq, TypedBuilder)]
pub struct MenuThemeData {
    #[builder(default, setter(strip_option))]
    pub style: Option<MenuStyle>,
    #[builder(default, setter(strip_option, into))]
    pub submenu_icon: Option<StateValue<Widget>>,
}

impl MenuThemeData {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn style(mut self, value: MenuStyle) -> Self {
        self.style = Some(value);
        self
    }

    #[must_use]
    pub fn submenu_icon(mut self, value: impl Into<StateValue<Widget>>) -> Self {
        self.submenu_icon = Some(value.into());
        self
    }

    #[must_use]
    pub fn copy_with(
        &self,
        style: Option<MenuStyle>,
        submenu_icon: Option<StateValue<Widget>>,
    ) -> Self {
        Self {
            style: style.or_else(|| self.style.clone()),
            submenu_icon: submenu_icon.or_else(|| self.submenu_icon.clone()),
        }
    }
}

/// Theme data for `PopupMenuButton` and popup menu entries.
#[derive(Clone, Debug, Default, PartialEq, TypedBuilder)]
pub struct PopupMenuThemeData {
    #[builder(default, setter(strip_option))]
    pub color: Option<Color>,
    #[builder(default, setter(strip_option))]
    pub shape: Option<BorderRadius>,
    #[builder(default, setter(strip_option))]
    pub menu_padding: Option<EdgeInsets>,
    #[builder(default, setter(transform = |value: f32| Some(value.max(0.0))))]
    pub elevation: Option<f32>,
    #[builder(default, setter(strip_option))]
    pub shadow_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    pub surface_tint_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    pub text_style: Option<TextStyle>,
    #[builder(default, setter(strip_option, into))]
    pub label_text_style: Option<StateValue<TextStyle>>,
    #[builder(default, setter(strip_option))]
    pub enable_feedback: Option<bool>,
    #[builder(default, setter(strip_option, into))]
    pub mouse_cursor: Option<String>,
    #[builder(default, setter(strip_option))]
    pub position: Option<PopupMenuPosition>,
    #[builder(default, setter(strip_option))]
    pub icon_color: Option<Color>,
    #[builder(default, setter(transform = |value: f32| Some(value.max(0.0))))]
    pub icon_size: Option<f32>,
}

impl PopupMenuThemeData {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn color(mut self, value: Color) -> Self {
        self.color = Some(value);
        self
    }

    #[must_use]
    pub fn shape(mut self, value: BorderRadius) -> Self {
        self.shape = Some(value);
        self
    }

    #[must_use]
    pub fn menu_padding(mut self, value: EdgeInsets) -> Self {
        self.menu_padding = Some(value);
        self
    }

    #[must_use]
    pub fn elevation(mut self, value: f32) -> Self {
        self.elevation = Some(value.max(0.0));
        self
    }

    #[must_use]
    pub fn shadow_color(mut self, value: Color) -> Self {
        self.shadow_color = Some(value);
        self
    }

    #[must_use]
    pub fn surface_tint_color(mut self, value: Color) -> Self {
        self.surface_tint_color = Some(value);
        self
    }

    #[must_use]
    pub fn text_style(mut self, value: TextStyle) -> Self {
        self.text_style = Some(value);
        self
    }

    #[must_use]
    pub fn label_text_style(mut self, value: impl Into<StateValue<TextStyle>>) -> Self {
        self.label_text_style = Some(value.into());
        self
    }

    #[must_use]
    pub fn enable_feedback(mut self, value: bool) -> Self {
        self.enable_feedback = Some(value);
        self
    }

    #[must_use]
    pub fn mouse_cursor(mut self, value: impl Into<String>) -> Self {
        self.mouse_cursor = Some(value.into());
        self
    }

    #[must_use]
    pub fn position(mut self, value: PopupMenuPosition) -> Self {
        self.position = Some(value);
        self
    }

    #[must_use]
    pub fn icon_color(mut self, value: Color) -> Self {
        self.icon_color = Some(value);
        self
    }

    #[must_use]
    pub fn icon_size(mut self, value: f32) -> Self {
        self.icon_size = Some(value.max(0.0));
        self
    }
}

/// Theme data for `DropdownMenu`.
#[derive(Clone, Debug, Default, PartialEq, TypedBuilder)]
pub struct DropdownMenuThemeData {
    #[builder(default, setter(strip_option))]
    pub text_style: Option<TextStyle>,
    #[builder(default, setter(strip_option))]
    pub input_decoration_theme: Option<InputDecorationThemeData>,
    #[builder(default, setter(strip_option))]
    pub menu_style: Option<MenuStyle>,
    #[builder(default, setter(strip_option))]
    pub disabled_color: Option<Color>,
}

impl DropdownMenuThemeData {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn text_style(mut self, value: TextStyle) -> Self {
        self.text_style = Some(value);
        self
    }

    #[must_use]
    pub fn input_decoration_theme(mut self, value: InputDecorationThemeData) -> Self {
        self.input_decoration_theme = Some(value);
        self
    }

    #[must_use]
    pub fn menu_style(mut self, value: MenuStyle) -> Self {
        self.menu_style = Some(value);
        self
    }

    #[must_use]
    pub fn disabled_color(mut self, value: Color) -> Self {
        self.disabled_color = Some(value);
        self
    }
}
