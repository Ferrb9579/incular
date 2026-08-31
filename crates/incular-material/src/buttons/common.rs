//! Shared button construction, theme resolution, and layout policy.

use super::style::ButtonKind;
use crate::foundation::{ComponentThemeData, Theme, ThemeData};
use incular_config::{Constraints, CrossAxisAlignment};
use incular_controls::{Button as ControlButton, ControlTheme};
use incular_core::{Color, Size};
use incular_widgets::{Border, Container, Row, SizedBox, Widget};
use std::rc::Rc;

#[derive(Clone)]
pub(super) struct ButtonSpec {
    pub(super) kind: ButtonKind,
    pub(super) label: Option<String>,
    pub(super) child: Option<Widget>,
    pub(super) semantic_label: Option<String>,
    pub(super) style: incular_controls::ButtonStyle,
    pub(super) enabled: bool,
    pub(super) focusable_when_disabled: bool,
    pub(super) loading: bool,
    pub(super) on_click: Option<Rc<dyn Fn() + 'static>>,
}

impl ButtonSpec {
    pub(super) fn label(kind: ButtonKind, label: impl Into<String>) -> Self {
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

    pub(super) fn child(kind: ButtonKind, child: impl Into<Widget>) -> Self {
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

    pub(super) fn set_label(mut self, label: impl Into<String>) -> Self {
        let label = label.into();
        self.label = Some(label.clone());
        self.child = None;
        self.semantic_label = Some(label);
        self
    }

    pub(super) fn set_child(mut self, child: impl Into<Widget>) -> Self {
        self.label = None;
        self.child = Some(child.into());
        self.semantic_label = None;
        self
    }

    pub(super) fn icon_label(
        kind: ButtonKind,
        icon: impl Into<Widget>,
        label: impl Into<String>,
    ) -> Self {
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

    pub(super) fn style(mut self, style: incular_controls::ButtonStyle) -> Self {
        self.style = self.style.merge(&style);
        self
    }

    pub(super) fn enabled(mut self, value: bool) -> Self {
        self.enabled = value;
        self
    }

    pub(super) fn focusable_when_disabled(mut self, value: bool) -> Self {
        self.focusable_when_disabled = value;
        self
    }

    pub(super) fn loading(mut self, value: bool) -> Self {
        self.loading = value;
        self
    }

    pub(super) fn on_click(mut self, callback: impl Fn() + 'static) -> Self {
        self.on_click = Some(Rc::new(callback));
        self
    }

    fn resolve_style(&self, theme: &ThemeData) -> incular_controls::ButtonStyle {
        let mut resolved = self.kind.default_style();

        // The legacy ButtonTheme style is the lowest explicit Material theme
        // layer. Component theme values then override it, and the widget's
        // sparse style is applied last.
        if let Some(style) = &theme.buttons().button_theme.style {
            resolved = resolved.merge(style);
        }
        let component = match self.kind {
            ButtonKind::Elevated => &theme.buttons().elevated_button_theme,
            ButtonKind::Filled | ButtonKind::FilledTonal => &theme.buttons().filled_button_theme,
            ButtonKind::Outlined => &theme.buttons().outlined_button_theme,
            ButtonKind::Text => &theme.buttons().text_button_theme,
            ButtonKind::Icon => &theme.buttons().icon_button_theme,
            ButtonKind::Floating => &theme.buttons().floating_action_button_theme,
        };
        apply_component_theme(&mut resolved, component);

        if self.kind == ButtonKind::Outlined
            && resolved.background.is_none()
            && resolved.background_states.is_none()
        {
            resolved.background_states = Some(
                incular_controls::StateColor::new(Color::TRANSPARENT)
                    .hovered(theme.core().color_scheme.surface_container_high)
                    .pressed(theme.core().color_scheme.surface_container_highest)
                    .focused(theme.core().color_scheme.surface_container_high),
            );
            if resolved.border.is_none() && resolved.side.is_none() {
                resolved.border = Some(Border::new(1.0, theme.core().color_scheme.outline));
            }
        }

        if self.kind == ButtonKind::FilledTonal {
            if resolved.background.is_none() {
                resolved.background = Some(theme.core().color_scheme.secondary_container);
            }
            if resolved.foreground.is_none() {
                resolved.foreground = Some(theme.core().color_scheme.on_secondary_container);
            }
        }

        if resolved.text_style.is_none() {
            resolved.text_style = Some(theme.core().text_theme.label_large.clone());
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

    pub(super) fn into_widget(self) -> Widget {
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

fn apply_component_theme(style: &mut incular_controls::ButtonStyle, theme: &ComponentThemeData) {
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
        style.overlay_color = Some(incular_controls::StateValue::from(value));
    }
    if let Some(value) = theme.shadow_color {
        style.shadow_color = Some(incular_controls::StateValue::from(value));
    }
    if let Some(value) = theme.surface_tint_color {
        style.surface_tint_color = Some(incular_controls::StateValue::from(value));
    }
    if let Some(value) = theme.elevation {
        style.elevation = Some(incular_controls::StateValue::from(value));
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
        style.shape = Some(incular_controls::StateValue::from(value));
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
        style.side = Some(incular_controls::StateValue::from(value));
        style.border = Some(value);
    }
    if let Some(value) = theme.text_style.clone() {
        style.text_style = Some(value);
    }
    if let Some(value) = theme.animation_duration {
        style.animation_duration = Some(value);
    }
}

fn apply_constraints(
    widget: Widget,
    style: &incular_controls::ButtonStyle,
    theme: &ThemeData,
) -> Widget {
    let density = style.visual_density.unwrap_or_else(|| {
        if theme.core().visual_density == crate::foundation::VisualDensity::COMPACT {
            incular_controls::ControlDensity::Compact
        } else if theme.core().visual_density == crate::foundation::VisualDensity::COMFORTABLE {
            incular_controls::ControlDensity::Comfortable
        } else {
            incular_controls::ControlDensity::Standard
        }
    });
    let density_adjustment = match density {
        incular_controls::ControlDensity::Compact => -8.0,
        incular_controls::ControlDensity::Standard => 0.0,
        incular_controls::ControlDensity::Comfortable => 8.0,
    };
    let target = style.tap_target_size.unwrap_or_else(|| {
        if theme.core().material_tap_target_size
            == crate::foundation::MaterialTapTargetSize::ShrinkWrap
        {
            incular_controls::TapTargetSize::ShrinkWrap
        } else {
            incular_controls::TapTargetSize::Padded
        }
    });
    let target_extent = match target {
        incular_controls::TapTargetSize::Padded => 48.0,
        incular_controls::TapTargetSize::ShrinkWrap => 0.0,
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
