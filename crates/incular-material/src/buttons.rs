//! Flutter-shaped Material button components.
//!
//! The retained input, focus, semantics, and repaint behavior is provided by
//! [`incular_controls::Button`].  This module owns the Material vocabulary and
//! the default/configuration layer around that headless primitive.  Keeping the
//! two layers separate means a button can be themed with [`ThemeData`] without
//! moving Material policy into `incular-widgets` or duplicating the controls
//! event implementation.

use crate::foundation::{ComponentThemeData, Theme, ThemeData};
use incular_config::{Alignment, Constraints, CrossAxisAlignment, EdgeInsets};
use incular_controls::{Button as ControlButton, ControlTheme};
use incular_core::{Color, Size};
use incular_text::TextStyle;
use incular_widgets::{Border, BorderRadius, Container, Row, SizedBox, Widget};
use std::rc::Rc;
use std::time::Duration;
use typed_builder::TypedBuilder;

pub use incular_controls::{
    ButtonLayerBuilder, ButtonStyle, ButtonVariant, ControlDensity, ControlState, IconAlignment,
    SplashFactory, StateColor, StateTable, StateValue, TapTargetSize,
};

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
    pub icon_alignment: Option<IconAlignment>,
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
    pub fn icon_alignment(mut self, value: IconAlignment) -> Self {
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
enum ButtonKind {
    Elevated,
    Filled,
    FilledTonal,
    Outlined,
    Text,
    Icon,
    Floating,
}

impl ButtonKind {
    const fn variant(self) -> ButtonVariant {
        match self {
            Self::Outlined => ButtonVariant::Default,
            Self::Text | Self::Icon => ButtonVariant::Ghost,
            Self::Elevated | Self::Filled | Self::FilledTonal | Self::Floating => {
                ButtonVariant::Primary
            }
        }
    }

    fn default_style(self) -> ButtonStyle {
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

#[derive(Clone)]
struct ButtonSpec {
    kind: ButtonKind,
    label: Option<String>,
    child: Option<Widget>,
    semantic_label: Option<String>,
    style: ButtonStyle,
    enabled: bool,
    focusable_when_disabled: bool,
    loading: bool,
    on_click: Option<Rc<dyn Fn() + 'static>>,
}

impl ButtonSpec {
    fn label(kind: ButtonKind, label: impl Into<String>) -> Self {
        let label = label.into();
        Self {
            kind,
            semantic_label: Some(label.clone()),
            label: Some(label),
            child: None,
            style: kind.default_style(),
            enabled: true,
            focusable_when_disabled: false,
            loading: false,
            on_click: None,
        }
    }

    fn child(kind: ButtonKind, child: impl Into<Widget>) -> Self {
        Self {
            kind,
            label: None,
            child: Some(child.into()),
            semantic_label: None,
            style: kind.default_style(),
            enabled: true,
            focusable_when_disabled: false,
            loading: false,
            on_click: None,
        }
    }

    fn set_label(mut self, label: impl Into<String>) -> Self {
        let label = label.into();
        self.label = Some(label.clone());
        self.child = None;
        self.semantic_label = Some(label);
        self
    }

    fn set_child(mut self, child: impl Into<Widget>) -> Self {
        self.label = None;
        self.child = Some(child.into());
        self.semantic_label = None;
        self
    }

    fn icon_label(kind: ButtonKind, icon: impl Into<Widget>, label: impl Into<String>) -> Self {
        let label = label.into();
        let child: Widget = Row::new([
            icon.into(),
            SizedBox::new().width(8.0).into(),
            incular_widgets::Text::new(label.clone()).into(),
        ])
        .alignment(CrossAxisAlignment::Center)
        .into();
        let mut spec = Self::child(kind, child);
        spec.semantic_label = Some(label);
        spec
    }

    fn style(mut self, style: ButtonStyle) -> Self {
        self.style = self.style.merge(&style);
        self
    }

    fn enabled(mut self, value: bool) -> Self {
        self.enabled = value;
        self
    }

    fn focusable_when_disabled(mut self, value: bool) -> Self {
        self.focusable_when_disabled = value;
        self
    }

    fn loading(mut self, value: bool) -> Self {
        self.loading = value;
        self
    }

    fn on_click(mut self, callback: impl Fn() + 'static) -> Self {
        self.on_click = Some(Rc::new(callback));
        self
    }

    fn resolve_style(&self, theme: &ThemeData) -> ButtonStyle {
        let mut resolved = self.kind.default_style();

        // The legacy ButtonTheme style is the lowest explicit Material theme
        // layer. Component theme values then override it, and the widget's
        // sparse style is applied last.
        if let Some(style) = &theme.button_theme.style {
            resolved = resolved.merge(style);
        }
        let component = match self.kind {
            ButtonKind::Elevated => &theme.elevated_button_theme,
            ButtonKind::Filled | ButtonKind::FilledTonal => &theme.filled_button_theme,
            ButtonKind::Outlined => &theme.outlined_button_theme,
            ButtonKind::Text => &theme.text_button_theme,
            ButtonKind::Icon => &theme.icon_button_theme,
            ButtonKind::Floating => &theme.floating_action_button_theme,
        };
        apply_component_theme(&mut resolved, component);

        if self.kind == ButtonKind::Outlined
            && resolved.background.is_none()
            && resolved.background_states.is_none()
        {
            resolved.background_states = Some(
                StateColor::new(Color::TRANSPARENT)
                    .hovered(theme.color_scheme.surface_container_high)
                    .pressed(theme.color_scheme.surface_container_highest)
                    .focused(theme.color_scheme.surface_container_high),
            );
            if resolved.border.is_none() && resolved.side.is_none() {
                resolved.border = Some(Border::new(1.0, theme.color_scheme.outline));
            }
        }

        if self.kind == ButtonKind::FilledTonal {
            if resolved.background.is_none() {
                resolved.background = Some(theme.color_scheme.secondary_container);
            }
            if resolved.foreground.is_none() {
                resolved.foreground = Some(theme.color_scheme.on_secondary_container);
            }
        }

        if resolved.text_style.is_none() {
            resolved.text_style = Some(theme.text_theme.label_large.clone());
        }

        resolved.merge(&self.style)
    }

    fn build(&self, theme: &ThemeData, controls_theme: &ControlTheme) -> Widget {
        let style = self.resolve_style(theme);
        let mut button = if let Some(label) = self.label.as_ref() {
            ControlButton::new(label.clone())
        } else {
            ControlButton::with_child(
                self.child
                    .clone()
                    .unwrap_or_else(|| SizedBox::shrink().into()),
            )
        }
        .style(style.clone())
        .enabled(self.enabled)
        .focusable_when_disabled(self.focusable_when_disabled)
        .loading(self.loading);

        if let Some(callback) = self.on_click.clone() {
            button = button.on_click(move || callback());
        }

        let mut widget = button.build(controls_theme);
        if let Some(label) = self.semantic_label.as_ref() {
            widget = widget.accessibility_label(label.clone());
        }
        apply_constraints(widget, &style, theme)
    }

    fn into_widget(self) -> Widget {
        let spec = Rc::new(self);
        let semantic_label = spec
            .semantic_label
            .clone()
            .filter(|label| !label.trim().is_empty())
            .or_else(|| spec.child.as_ref().and_then(Widget::semantic_text));
        let widget = Widget::layout_builder(move |_| {
            let theme = Theme::of_shared().unwrap_or_else(ThemeData::light_shared);
            let controls_theme = theme.control_theme();
            spec.build(&theme, &controls_theme)
        });
        match semantic_label {
            Some(label) => widget.accessibility_label(label),
            None => widget,
        }
    }
}

fn apply_component_theme(style: &mut ButtonStyle, theme: &ComponentThemeData) {
    if let Some(value) = &theme.style {
        *style = style.clone().merge(value);
    }
    if let Some(value) = theme.background_color {
        style.background = Some(value);
    }
    if let Some(value) = theme.foreground_color {
        style.foreground = Some(value);
    }
    if let Some(value) = theme.overlay_color {
        style.overlay_color = Some(StateValue::from(value));
    }
    if let Some(value) = theme.shadow_color {
        style.shadow_color = Some(StateValue::from(value));
    }
    if let Some(value) = theme.surface_tint_color {
        style.surface_tint_color = Some(StateValue::from(value));
    }
    if let Some(value) = theme.elevation {
        style.elevation = Some(StateValue::from(value));
    }
    if let Some(value) = theme.padding {
        style.padding = Some(value);
    }
    if let Some(value) = theme.minimum_size {
        style.minimum_size = Some(value);
    }
    if let Some(value) = theme.maximum_size {
        style.maximum_size = Some(value);
    }
    if let Some(value) = theme.shape {
        style.shape = Some(StateValue::from(value));
        // The controls renderer currently consumes its uniform radius field;
        // retain the full state property above for future shape resolution.
        if value.top_left == value.top_right
            && value.top_left == value.bottom_right
            && value.top_left == value.bottom_left
        {
            style.border_radius = Some(value.top_left.x.max(value.top_left.y));
        }
    }
    if let Some(value) = theme.side {
        style.side = Some(StateValue::from(value));
        style.border = Some(value);
    }
    if let Some(value) = theme.text_style.clone() {
        style.text_style = Some(value);
    }
    if let Some(value) = theme.animation_duration {
        style.animation_duration = Some(value);
    }
}

fn apply_constraints(widget: Widget, style: &ButtonStyle, theme: &ThemeData) -> Widget {
    let density = style.visual_density.unwrap_or_else(|| {
        if theme.visual_density == crate::foundation::VisualDensity::COMPACT {
            ControlDensity::Compact
        } else if theme.visual_density == crate::foundation::VisualDensity::COMFORTABLE {
            ControlDensity::Comfortable
        } else {
            ControlDensity::Standard
        }
    });
    let density_adjustment = match density {
        ControlDensity::Compact => -8.0,
        ControlDensity::Standard => 0.0,
        ControlDensity::Comfortable => 8.0,
    };
    let target = style.tap_target_size.unwrap_or_else(|| {
        if theme.material_tap_target_size == crate::foundation::MaterialTapTargetSize::ShrinkWrap {
            TapTargetSize::ShrinkWrap
        } else {
            TapTargetSize::Padded
        }
    });
    let target_extent = match target {
        TapTargetSize::Padded => 48.0,
        TapTargetSize::ShrinkWrap => 0.0,
    };

    let minimum = style.minimum_size.unwrap_or(Size::ZERO);
    let fixed = style.fixed_size;
    let maximum = style.maximum_size;
    let (min_width, max_width, min_height, max_height) = if let Some(fixed) = fixed {
        (fixed.width, fixed.width, fixed.height, fixed.height)
    } else {
        let min_width = (minimum.width + density_adjustment)
            .max(target_extent)
            .max(0.0);
        let min_height = (minimum.height + density_adjustment)
            .max(target_extent)
            .max(0.0);
        let max_width = maximum.map_or(f32::INFINITY, |value| value.width.max(min_width));
        let max_height = maximum.map_or(f32::INFINITY, |value| value.height.max(min_height));
        (min_width, max_width, min_height, max_height)
    };

    if min_width == 0.0 && min_height == 0.0 && !max_width.is_finite() && !max_height.is_finite() {
        widget
    } else {
        Container::with_child(widget)
            .constraints(Constraints::new(
                min_width, max_width, min_height, max_height,
            ))
            .into()
    }
}

/// High-emphasis Material elevated button.
#[derive(Clone, TypedBuilder)]
#[builder(mutators(
    pub fn label(self, label: impl Into<String>) {
        self.spec = self.spec.clone().set_label(label);
    }
    pub fn child(self, child: impl Into<Widget>) {
        self.spec = self.spec.clone().set_child(child);
    }
    pub fn style(self, style: ButtonStyle) {
        self.spec = self.spec.clone().style(style);
    }
    pub fn enabled(self, value: bool) {
        self.spec = self.spec.clone().enabled(value);
    }
    pub fn on_click<F>(self, callback: F)
    where
        F: Fn() + 'static,
    {
        self.spec = self.spec.clone().on_click(callback);
    }
    pub fn on_pressed<F>(self, callback: F)
    where
        F: Fn() + 'static,
    {
        self.spec = self.spec.clone().on_click(callback);
    }
    pub fn focusable_when_disabled(self, value: bool) {
        self.spec = self.spec.clone().focusable_when_disabled(value);
    }
    pub fn loading(self, value: bool) {
        self.spec = self.spec.clone().loading(value);
    }
))]
pub struct ElevatedButton {
    #[builder(via_mutators = ButtonSpec::child(ButtonKind::Elevated, SizedBox::shrink()))]
    spec: ButtonSpec,
}

impl Default for ElevatedButton {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl ElevatedButton {
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            spec: ButtonSpec::label(ButtonKind::Elevated, label),
        }
    }

    #[must_use]
    pub fn icon(icon: impl Into<Widget>, label: impl Into<String>) -> Self {
        Self {
            spec: ButtonSpec::icon_label(ButtonKind::Elevated, icon, label),
        }
    }

    #[must_use]
    pub fn with_child(child: impl Into<Widget>) -> Self {
        Self {
            spec: ButtonSpec::child(ButtonKind::Elevated, child),
        }
    }

    #[must_use]
    pub fn style_from(config: ButtonStyleConfig) -> ButtonStyle {
        config.into_style(ButtonVariant::Primary)
    }

    #[must_use]
    pub fn style(mut self, value: ButtonStyle) -> Self {
        self.spec = self.spec.clone().style(value);
        self
    }

    #[must_use]
    pub fn enabled(mut self, value: bool) -> Self {
        self.spec = self.spec.clone().enabled(value);
        self
    }

    #[must_use]
    pub fn on_click(mut self, callback: impl Fn() + 'static) -> Self {
        self.spec = self.spec.clone().on_click(callback);
        self
    }

    #[must_use]
    pub fn on_pressed(self, callback: impl Fn() + 'static) -> Self {
        self.on_click(callback)
    }

    #[must_use]
    pub fn focusable_when_disabled(mut self, value: bool) -> Self {
        self.spec = self.spec.clone().focusable_when_disabled(value);
        self
    }

    #[must_use]
    pub fn loading(mut self, value: bool) -> Self {
        self.spec = self.spec.clone().loading(value);
        self
    }
}

impl From<ElevatedButton> for Widget {
    fn from(value: ElevatedButton) -> Self {
        value.spec.into_widget()
    }
}

/// Filled Material button, including the Material 3 tonal variant.
#[derive(Clone, TypedBuilder)]
#[builder(mutators(
    pub fn label(self, label: impl Into<String>) {
        self.spec = self.spec.clone().set_label(label);
    }
    pub fn child(self, child: impl Into<Widget>) {
        self.spec = self.spec.clone().set_child(child);
    }
    pub fn style(self, style: ButtonStyle) {
        self.spec = self.spec.clone().style(style);
    }
    pub fn enabled(self, value: bool) {
        self.spec = self.spec.clone().enabled(value);
    }
    pub fn on_click<F>(self, callback: F)
    where
        F: Fn() + 'static,
    {
        self.spec = self.spec.clone().on_click(callback);
    }
    pub fn on_pressed<F>(self, callback: F)
    where
        F: Fn() + 'static,
    {
        self.spec = self.spec.clone().on_click(callback);
    }
    pub fn focusable_when_disabled(self, value: bool) {
        self.spec = self.spec.clone().focusable_when_disabled(value);
    }
    pub fn loading(self, value: bool) {
        self.spec = self.spec.clone().loading(value);
    }
))]
pub struct FilledButton {
    #[builder(via_mutators = ButtonSpec::child(ButtonKind::Filled, SizedBox::shrink()))]
    spec: ButtonSpec,
}

impl Default for FilledButton {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl FilledButton {
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            spec: ButtonSpec::label(ButtonKind::Filled, label),
        }
    }

    #[must_use]
    pub fn icon(icon: impl Into<Widget>, label: impl Into<String>) -> Self {
        Self {
            spec: ButtonSpec::icon_label(ButtonKind::Filled, icon, label),
        }
    }

    #[must_use]
    pub fn tonal(label: impl Into<String>) -> Self {
        Self {
            spec: ButtonSpec::label(ButtonKind::FilledTonal, label),
        }
    }

    #[must_use]
    pub fn tonal_icon(icon: impl Into<Widget>, label: impl Into<String>) -> Self {
        Self {
            spec: ButtonSpec::icon_label(ButtonKind::FilledTonal, icon, label),
        }
    }

    #[must_use]
    pub fn with_child(child: impl Into<Widget>) -> Self {
        Self {
            spec: ButtonSpec::child(ButtonKind::Filled, child),
        }
    }

    #[must_use]
    pub fn tonal_with_child(child: impl Into<Widget>) -> Self {
        Self {
            spec: ButtonSpec::child(ButtonKind::FilledTonal, child),
        }
    }

    #[must_use]
    pub fn style_from(config: ButtonStyleConfig) -> ButtonStyle {
        config.into_style(ButtonVariant::Primary)
    }

    #[must_use]
    pub fn tonal_style_from(config: ButtonStyleConfig) -> ButtonStyle {
        config.into_style(ButtonVariant::Primary)
    }

    #[must_use]
    pub fn style(mut self, value: ButtonStyle) -> Self {
        self.spec = self.spec.clone().style(value);
        self
    }

    #[must_use]
    pub fn enabled(mut self, value: bool) -> Self {
        self.spec = self.spec.clone().enabled(value);
        self
    }

    #[must_use]
    pub fn on_click(mut self, callback: impl Fn() + 'static) -> Self {
        self.spec = self.spec.clone().on_click(callback);
        self
    }

    #[must_use]
    pub fn on_pressed(self, callback: impl Fn() + 'static) -> Self {
        self.on_click(callback)
    }

    #[must_use]
    pub fn focusable_when_disabled(mut self, value: bool) -> Self {
        self.spec = self.spec.clone().focusable_when_disabled(value);
        self
    }

    #[must_use]
    pub fn loading(mut self, value: bool) -> Self {
        self.spec = self.spec.clone().loading(value);
        self
    }
}

impl From<FilledButton> for Widget {
    fn from(value: FilledButton) -> Self {
        value.spec.into_widget()
    }
}

/// Medium-emphasis outlined Material button.
#[derive(Clone, TypedBuilder)]
#[builder(mutators(
    pub fn label(self, label: impl Into<String>) {
        self.spec = self.spec.clone().set_label(label);
    }
    pub fn child(self, child: impl Into<Widget>) {
        self.spec = self.spec.clone().set_child(child);
    }
    pub fn style(self, style: ButtonStyle) {
        self.spec = self.spec.clone().style(style);
    }
    pub fn enabled(self, value: bool) {
        self.spec = self.spec.clone().enabled(value);
    }
    pub fn on_click<F>(self, callback: F)
    where
        F: Fn() + 'static,
    {
        self.spec = self.spec.clone().on_click(callback);
    }
    pub fn on_pressed<F>(self, callback: F)
    where
        F: Fn() + 'static,
    {
        self.spec = self.spec.clone().on_click(callback);
    }
    pub fn focusable_when_disabled(self, value: bool) {
        self.spec = self.spec.clone().focusable_when_disabled(value);
    }
    pub fn loading(self, value: bool) {
        self.spec = self.spec.clone().loading(value);
    }
))]
pub struct OutlinedButton {
    #[builder(via_mutators = ButtonSpec::child(ButtonKind::Outlined, SizedBox::shrink()))]
    spec: ButtonSpec,
}

impl Default for OutlinedButton {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl OutlinedButton {
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            spec: ButtonSpec::label(ButtonKind::Outlined, label),
        }
    }

    #[must_use]
    pub fn icon(icon: impl Into<Widget>, label: impl Into<String>) -> Self {
        Self {
            spec: ButtonSpec::icon_label(ButtonKind::Outlined, icon, label),
        }
    }

    #[must_use]
    pub fn with_child(child: impl Into<Widget>) -> Self {
        Self {
            spec: ButtonSpec::child(ButtonKind::Outlined, child),
        }
    }

    #[must_use]
    pub fn style_from(config: ButtonStyleConfig) -> ButtonStyle {
        config.into_style(ButtonVariant::Default)
    }

    #[must_use]
    pub fn style(mut self, value: ButtonStyle) -> Self {
        self.spec = self.spec.clone().style(value);
        self
    }

    #[must_use]
    pub fn enabled(mut self, value: bool) -> Self {
        self.spec = self.spec.clone().enabled(value);
        self
    }

    #[must_use]
    pub fn on_click(mut self, callback: impl Fn() + 'static) -> Self {
        self.spec = self.spec.clone().on_click(callback);
        self
    }

    #[must_use]
    pub fn on_pressed(self, callback: impl Fn() + 'static) -> Self {
        self.on_click(callback)
    }

    #[must_use]
    pub fn focusable_when_disabled(mut self, value: bool) -> Self {
        self.spec = self.spec.clone().focusable_when_disabled(value);
        self
    }

    #[must_use]
    pub fn loading(mut self, value: bool) -> Self {
        self.spec = self.spec.clone().loading(value);
        self
    }
}

impl From<OutlinedButton> for Widget {
    fn from(value: OutlinedButton) -> Self {
        value.spec.into_widget()
    }
}

/// Low-emphasis text Material button.
#[derive(Clone, TypedBuilder)]
#[builder(mutators(
    pub fn label(self, label: impl Into<String>) {
        self.spec = self.spec.clone().set_label(label);
    }
    pub fn child(self, child: impl Into<Widget>) {
        self.spec = self.spec.clone().set_child(child);
    }
    pub fn style(self, style: ButtonStyle) {
        self.spec = self.spec.clone().style(style);
    }
    pub fn enabled(self, value: bool) {
        self.spec = self.spec.clone().enabled(value);
    }
    pub fn on_click<F>(self, callback: F)
    where
        F: Fn() + 'static,
    {
        self.spec = self.spec.clone().on_click(callback);
    }
    pub fn on_pressed<F>(self, callback: F)
    where
        F: Fn() + 'static,
    {
        self.spec = self.spec.clone().on_click(callback);
    }
    pub fn focusable_when_disabled(self, value: bool) {
        self.spec = self.spec.clone().focusable_when_disabled(value);
    }
    pub fn loading(self, value: bool) {
        self.spec = self.spec.clone().loading(value);
    }
))]
pub struct TextButton {
    #[builder(via_mutators = ButtonSpec::child(ButtonKind::Text, SizedBox::shrink()))]
    spec: ButtonSpec,
}

impl Default for TextButton {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl TextButton {
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            spec: ButtonSpec::label(ButtonKind::Text, label),
        }
    }

    #[must_use]
    pub fn icon(icon: impl Into<Widget>, label: impl Into<String>) -> Self {
        Self {
            spec: ButtonSpec::icon_label(ButtonKind::Text, icon, label),
        }
    }

    #[must_use]
    pub fn with_child(child: impl Into<Widget>) -> Self {
        Self {
            spec: ButtonSpec::child(ButtonKind::Text, child),
        }
    }

    #[must_use]
    pub fn style_from(config: ButtonStyleConfig) -> ButtonStyle {
        config.into_style(ButtonVariant::Ghost)
    }

    #[must_use]
    pub fn style(mut self, value: ButtonStyle) -> Self {
        self.spec = self.spec.clone().style(value);
        self
    }

    #[must_use]
    pub fn enabled(mut self, value: bool) -> Self {
        self.spec = self.spec.clone().enabled(value);
        self
    }

    #[must_use]
    pub fn on_click(mut self, callback: impl Fn() + 'static) -> Self {
        self.spec = self.spec.clone().on_click(callback);
        self
    }

    #[must_use]
    pub fn on_pressed(self, callback: impl Fn() + 'static) -> Self {
        self.on_click(callback)
    }

    #[must_use]
    pub fn focusable_when_disabled(mut self, value: bool) -> Self {
        self.spec = self.spec.clone().focusable_when_disabled(value);
        self
    }

    #[must_use]
    pub fn loading(mut self, value: bool) -> Self {
        self.spec = self.spec.clone().loading(value);
        self
    }
}

impl From<TextButton> for Widget {
    fn from(value: TextButton) -> Self {
        value.spec.into_widget()
    }
}

/// Material icon-only action button.
#[derive(Clone, TypedBuilder)]
#[builder(mutators(
    pub fn icon(self, icon: impl Into<String>) {
        self.spec = self.spec.clone().set_child(incular_widgets::Text::new(icon));
    }
    pub fn child(self, child: impl Into<Widget>) {
        self.spec = self.spec.clone().set_child(child);
    }
    pub fn style(self, style: ButtonStyle) {
        self.spec = self.spec.clone().style(style);
    }
    pub fn enabled(self, value: bool) {
        self.spec = self.spec.clone().enabled(value);
    }
    pub fn on_click<F>(self, callback: F)
    where
        F: Fn() + 'static,
    {
        self.spec = self.spec.clone().on_click(callback);
    }
    pub fn on_pressed<F>(self, callback: F)
    where
        F: Fn() + 'static,
    {
        self.spec = self.spec.clone().on_click(callback);
    }
    pub fn focusable_when_disabled(self, value: bool) {
        self.spec = self.spec.clone().focusable_when_disabled(value);
    }
    pub fn loading(self, value: bool) {
        self.spec = self.spec.clone().loading(value);
    }
))]
pub struct IconButton {
    #[builder(via_mutators = ButtonSpec::child(ButtonKind::Icon, SizedBox::shrink()))]
    spec: ButtonSpec,
    #[builder(default)]
    selected: bool,
    #[builder(default, setter(strip_option, into))]
    tooltip: Option<String>,
}

impl Default for IconButton {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl IconButton {
    #[must_use]
    pub fn new(icon: impl Into<String>) -> Self {
        Self {
            spec: ButtonSpec::child(ButtonKind::Icon, incular_widgets::Text::new(icon)),
            selected: false,
            tooltip: None,
        }
    }

    #[must_use]
    pub fn icon(icon: impl Into<Widget>) -> Self {
        Self::with_child(icon)
    }

    #[must_use]
    pub fn with_child(child: impl Into<Widget>) -> Self {
        Self {
            spec: ButtonSpec::child(ButtonKind::Icon, child),
            selected: false,
            tooltip: None,
        }
    }

    #[must_use]
    pub fn selected(mut self, value: bool) -> Self {
        self.selected = value;
        self
    }

    #[must_use]
    pub fn is_selected(&self) -> bool {
        self.selected
    }

    #[must_use]
    pub fn tooltip(mut self, value: impl Into<String>) -> Self {
        self.tooltip = Some(value.into());
        self
    }

    #[must_use]
    pub fn style_from(config: ButtonStyleConfig) -> ButtonStyle {
        config.into_style(ButtonVariant::Ghost)
    }

    #[must_use]
    pub fn style(mut self, value: ButtonStyle) -> Self {
        self.spec = self.spec.clone().style(value);
        self
    }

    #[must_use]
    pub fn enabled(mut self, value: bool) -> Self {
        self.spec = self.spec.clone().enabled(value);
        self
    }

    #[must_use]
    pub fn on_click(mut self, callback: impl Fn() + 'static) -> Self {
        self.spec = self.spec.clone().on_click(callback);
        self
    }

    #[must_use]
    pub fn on_pressed(self, callback: impl Fn() + 'static) -> Self {
        self.on_click(callback)
    }

    #[must_use]
    pub fn focusable_when_disabled(mut self, value: bool) -> Self {
        self.spec = self.spec.clone().focusable_when_disabled(value);
        self
    }

    #[must_use]
    pub fn loading(mut self, value: bool) -> Self {
        self.spec = self.spec.clone().loading(value);
        self
    }
}

impl From<IconButton> for Widget {
    fn from(mut value: IconButton) -> Self {
        if let Some(tooltip) = value.tooltip.take() {
            value.spec.semantic_label = Some(tooltip);
        } else {
            value.spec.semantic_label = Some("Icon button".to_owned());
        }
        value.spec.into_widget()
    }
}

/// Material floating action button.
#[derive(Clone, TypedBuilder)]
#[builder(mutators(
    pub fn child(self, child: impl Into<Widget>) {
        self.spec = self.spec.clone().set_child(child);
    }
    pub fn small(self) {
        self.size = FloatingActionButtonSize::Small;
    }
    pub fn large(self) {
        self.size = FloatingActionButtonSize::Large;
    }
    pub fn extended(self, label: impl Into<String>) {
        let label = label.into();
        self.size = FloatingActionButtonSize::Extended;
        self.spec = self
            .spec
            .clone()
            .set_child(incular_widgets::Text::new(label.clone()));
        self.spec.semantic_label = Some(label);
    }
    pub fn extended_with_icon(self, icon: impl Into<Widget>, label: impl Into<String>) {
        let label = label.into();
        self.size = FloatingActionButtonSize::Extended;
        self.spec = self.spec.clone().set_child(
            Row::new([
                icon.into(),
                SizedBox::new().width(8.0).into(),
                incular_widgets::Text::new(label.clone()).into(),
            ])
            .alignment(CrossAxisAlignment::Center),
        );
        self.spec.semantic_label = Some(label);
    }
    pub fn style(self, style: ButtonStyle) {
        self.spec = self.spec.clone().style(style);
    }
    pub fn on_click<F>(self, callback: F)
    where
        F: Fn() + 'static,
    {
        self.spec = self.spec.clone().on_click(callback);
    }
    pub fn on_pressed<F>(self, callback: F)
    where
        F: Fn() + 'static,
    {
        self.spec = self.spec.clone().on_click(callback);
    }
    pub fn enabled(self, value: bool) {
        self.spec = self.spec.clone().enabled(value);
    }
    pub fn focusable_when_disabled(self, value: bool) {
        self.spec = self.spec.clone().focusable_when_disabled(value);
    }
    pub fn loading(self, value: bool) {
        self.spec = self.spec.clone().loading(value);
    }
))]
pub struct FloatingActionButton {
    #[builder(via_mutators = ButtonSpec::child(ButtonKind::Floating, SizedBox::shrink()))]
    spec: ButtonSpec,
    #[builder(via_mutators = FloatingActionButtonSize::Regular)]
    size: FloatingActionButtonSize,
    #[builder(default, setter(strip_option, into))]
    tooltip: Option<String>,
    #[builder(default, setter(strip_option, into))]
    hero_tag: Option<String>,
}

impl Default for FloatingActionButton {
    fn default() -> Self {
        Self::builder().build()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FloatingActionButtonSize {
    Regular,
    Small,
    Large,
    Extended,
}

impl FloatingActionButton {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            spec: ButtonSpec::child(ButtonKind::Floating, child),
            size: FloatingActionButtonSize::Regular,
            tooltip: None,
            hero_tag: None,
        }
    }

    #[must_use]
    pub fn small(child: impl Into<Widget>) -> Self {
        Self {
            size: FloatingActionButtonSize::Small,
            ..Self::new(child)
        }
    }

    #[must_use]
    pub fn large(child: impl Into<Widget>) -> Self {
        Self {
            size: FloatingActionButtonSize::Large,
            ..Self::new(child)
        }
    }

    #[must_use]
    pub fn extended(label: impl Into<String>) -> Self {
        let label = label.into();
        let mut value = Self::new(incular_widgets::Text::new(label.clone()));
        value.size = FloatingActionButtonSize::Extended;
        value.spec.semantic_label = Some(label);
        value
    }

    #[must_use]
    pub fn extended_with_icon(icon: impl Into<Widget>, label: impl Into<String>) -> Self {
        let label = label.into();
        let mut value = Self::new(
            Row::new([
                icon.into(),
                SizedBox::new().width(8.0).into(),
                incular_widgets::Text::new(label.clone()).into(),
            ])
            .alignment(CrossAxisAlignment::Center),
        );
        value.size = FloatingActionButtonSize::Extended;
        value.spec.semantic_label = Some(label);
        value
    }

    #[must_use]
    pub fn style_from(config: ButtonStyleConfig) -> ButtonStyle {
        config.into_style(ButtonVariant::Primary)
    }

    #[must_use]
    pub fn style(mut self, value: ButtonStyle) -> Self {
        self.spec = self.spec.clone().style(value);
        self
    }

    #[must_use]
    pub fn tooltip(mut self, value: impl Into<String>) -> Self {
        self.tooltip = Some(value.into());
        self
    }

    #[must_use]
    pub fn hero_tag(mut self, value: impl Into<String>) -> Self {
        self.hero_tag = Some(value.into());
        self
    }

    #[must_use]
    pub fn enabled(mut self, value: bool) -> Self {
        self.spec = self.spec.clone().enabled(value);
        self
    }

    #[must_use]
    pub fn on_click(mut self, callback: impl Fn() + 'static) -> Self {
        self.spec = self.spec.clone().on_click(callback);
        self
    }

    #[must_use]
    pub fn on_pressed(self, callback: impl Fn() + 'static) -> Self {
        self.on_click(callback)
    }

    #[must_use]
    pub fn focusable_when_disabled(mut self, value: bool) -> Self {
        self.spec = self.spec.clone().focusable_when_disabled(value);
        self
    }

    #[must_use]
    pub fn loading(mut self, value: bool) -> Self {
        self.spec = self.spec.clone().loading(value);
        self
    }

    /// Returns the configured hero tag. The runtime owns actual hero-flight
    /// coordination; this accessor makes the retained configuration inspectable.
    #[must_use]
    pub fn hero_tag_value(&self) -> Option<&str> {
        self.hero_tag.as_deref()
    }
}

impl From<FloatingActionButton> for Widget {
    fn from(mut value: FloatingActionButton) -> Self {
        let size = match value.size {
            FloatingActionButtonSize::Regular => 56.0,
            FloatingActionButtonSize::Small => 40.0,
            FloatingActionButtonSize::Large => 96.0,
            FloatingActionButtonSize::Extended => 48.0,
        };
        let mut size_style = ButtonStyle::new().border_radius(size * 0.5);
        if value.size == FloatingActionButtonSize::Extended {
            size_style = size_style
                .minimum_size(Size::new(80.0, 48.0))
                .padding(EdgeInsets::symmetric(16.0, 8.0));
        } else {
            size_style = size_style
                .minimum_size(Size::new(size, size))
                .fixed_size(Size::new(size, size));
        }
        value.spec = value.spec.style(size_style);
        value.spec.semantic_label = value.tooltip.take().or(value.spec.semantic_label);
        if value.spec.semantic_label.is_none() {
            value.spec.semantic_label = Some("Floating action button".to_owned());
        }
        value.spec.into_widget()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_factory_preserves_sparse_state_properties() {
        let style = ElevatedButton::style_from(
            ButtonStyleConfig::new()
                .background_color(Color::rgba(1, 2, 3, 255))
                .elevation(StateValue::states(StateTable::new(1.0).pressed(4.0)))
                .minimum_size(Size::new(80.0, 40.0)),
        );
        assert_eq!(style.background, Some(Color::rgba(1, 2, 3, 255)));
        assert_eq!(style.minimum_size, Some(Size::new(80.0, 40.0)));
        assert_eq!(
            style
                .elevation
                .as_ref()
                .expect("elevation")
                .resolve(ControlState::PRESSED),
            4.0
        );
        assert_eq!(style.variant, ButtonVariant::Primary);
    }

    #[test]
    fn button_families_have_widget_conversions() {
        let _: Widget = ElevatedButton::new("Elevated").into();
        let _: Widget = FilledButton::tonal("Tonal").into();
        let _: Widget = OutlinedButton::new("Outlined").into();
        let _: Widget = TextButton::new("Text").into();
        let _: Widget = IconButton::new("⚙").into();
        let _: Widget = IconButton::icon(SizedBox::square(16.0))
            .tooltip("Settings")
            .into();
        let _: Widget = FloatingActionButton::small(SizedBox::square(20.0)).into();
        let _: Widget = FloatingActionButton::extended("Create").into();
    }

    #[test]
    fn material_default_constraints_follow_tap_target_policy() {
        let theme = ThemeData::light()
            .with_tap_target_size(crate::foundation::MaterialTapTargetSize::Padded);
        let style = ButtonKind::Text.default_style();
        let minimum = style.minimum_size.expect("default button minimum");
        assert!(minimum.height >= 36.0);
        assert_eq!(style.tap_target_size, Some(TapTargetSize::Padded));
        assert_eq!(
            theme.material_tap_target_size,
            crate::foundation::MaterialTapTargetSize::Padded
        );
    }
}
