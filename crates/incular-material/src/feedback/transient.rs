use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use super::helpers::{finite_non_negative, semantic_state};
use crate::extras::SnackBarBehavior;
use incular_config::{CrossAxisAlignment, EdgeInsets, MainAxisAlignment, MainAxisSize};
use incular_controls::{ControlTheme, current_control_theme};
use incular_core::{Color, Offset};
use incular_semantics::{Role as SemanticRole, SemanticAction, SemanticState};
use incular_text::TextStyle;
use incular_widgets::internal::ActionSurface;
use incular_widgets::{
    Align, BorderRadius, BoxDecoration, Container, Focus, FocusNode, OverlayPortal, Padding, Row,
    Semantics, Text, Widget,
};
use typed_builder::TypedBuilder;

/// A Material snackbar action.
#[derive(Clone, TypedBuilder)]
pub struct SnackBarAction {
    #[builder(setter(into))]
    label: String,
    #[builder(
        default,
        setter(
            fn transform<F>(callback: F) -> Option<Rc<dyn Fn()>>
            where
                F: Fn() + 'static,
            {
                Some(Rc::new(callback))
            }
        )
    )]
    on_pressed: Option<Rc<dyn Fn()>>,
    #[builder(default, setter(strip_option))]
    text_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    disabled_text_color: Option<Color>,
    #[builder(default = true)]
    enabled: bool,
    #[builder(default, setter(strip_option, into))]
    semantic_label: Option<String>,
}

impl SnackBarAction {
    #[must_use]
    pub fn new(label: impl Into<String>, on_pressed: impl Fn() + 'static) -> Self {
        Self {
            label: label.into(),
            on_pressed: Some(Rc::new(on_pressed)),
            text_color: None,
            disabled_text_color: None,
            enabled: true,
            semantic_label: None,
        }
    }

    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    #[must_use]
    pub fn on_pressed(mut self, callback: impl Fn() + 'static) -> Self {
        self.on_pressed = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    #[must_use]
    pub fn text_color(mut self, color: Color) -> Self {
        self.text_color = Some(color);
        self
    }

    #[must_use]
    pub fn disabled_text_color(mut self, color: Color) -> Self {
        self.disabled_text_color = Some(color);
        self
    }

    #[must_use]
    pub fn semantic_label(mut self, label: impl Into<String>) -> Self {
        self.semantic_label = Some(label.into());
        self
    }

    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    #[must_use]
    pub fn label_text(&self) -> &str {
        &self.label
    }

    fn build(&self, theme: &ControlTheme) -> Widget {
        let color = if self.enabled {
            self.text_color.unwrap_or(theme.colors.accent)
        } else {
            self.disabled_text_color
                .unwrap_or(theme.colors.foreground_disabled)
        };
        let mut surface = ActionSurface::new(self.label.clone())
            .padding(EdgeInsets::symmetric(12.0, 8.0))
            .label_style(TextStyle::new().font_size(14.0).bold().color(color))
            .color(Color::TRANSPARENT)
            .hover_color(theme.colors.hover_overlay)
            .pressed_color(theme.colors.pressed_overlay)
            .focused_color(Color::TRANSPARENT)
            .disabled_color(Color::TRANSPARENT)
            .enabled(self.enabled);
        if let Some(callback) = self.on_pressed.clone()
            && self.enabled
        {
            surface = surface.on_click(move || callback());
        }
        let mut semantics = Semantics::new(surface)
            .role(SemanticRole::Button)
            .label(
                self.semantic_label
                    .clone()
                    .unwrap_or_else(|| self.label.clone()),
            )
            .enabled(self.enabled)
            .action(SemanticAction::Activate);
        if self.enabled
            && let Some(callback) = self.on_pressed.clone()
        {
            semantics = semantics.on_tap(move || callback());
        }
        if !self.enabled {
            semantics = semantics.state(SemanticState {
                enabled: false,
                ..SemanticState::default()
            });
        }
        semantics.into()
    }
}

impl From<SnackBarAction> for Widget {
    fn from(value: SnackBarAction) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |context, _| value.build(&current_control_theme(context)))
    }
}

/// A Material snackbar surface.
#[derive(Clone, TypedBuilder)]
pub struct SnackBar {
    #[builder(setter(into))]
    content: Widget,
    #[builder(default, setter(strip_option))]
    action: Option<SnackBarAction>,
    #[builder(default, setter(strip_option))]
    background_color: Option<Color>,
    #[builder(default = 6.0, setter(transform = |value: f32| finite_non_negative(value)))]
    elevation: f32,
    #[builder(default, setter(strip_option))]
    shape: Option<BorderRadius>,
    #[builder(default = EdgeInsets::symmetric(16.0, 14.0))]
    padding: EdgeInsets,
    #[builder(default, setter(strip_option))]
    margin: Option<EdgeInsets>,
    #[builder(default, setter(transform = |value: f32| Some(finite_non_negative(value))))]
    width: Option<f32>,
    #[builder(default = SnackBarBehavior::Fixed)]
    behavior: SnackBarBehavior,
    #[builder(default = Duration::from_millis(4_000))]
    duration: Duration,
    #[builder(default, setter(strip_option, into))]
    semantic_label: Option<String>,
}

impl SnackBar {
    #[must_use]
    pub fn new(content: impl Into<Widget>) -> Self {
        Self {
            content: content.into(),
            action: None,
            background_color: None,
            elevation: 6.0,
            shape: None,
            padding: EdgeInsets::symmetric(16.0, 14.0),
            margin: None,
            width: None,
            behavior: SnackBarBehavior::Fixed,
            duration: Duration::from_millis(4_000),
            semantic_label: None,
        }
    }

    #[must_use]
    pub fn text(message: impl Into<String>) -> Self {
        Self::new(Text::new(message))
    }

    #[must_use]
    pub fn content(mut self, content: impl Into<Widget>) -> Self {
        self.content = content.into();
        self
    }

    #[must_use]
    pub fn action(mut self, action: SnackBarAction) -> Self {
        self.action = Some(action);
        self
    }

    #[must_use]
    pub fn background_color(mut self, color: Color) -> Self {
        self.background_color = Some(color);
        self
    }

    #[must_use]
    pub fn elevation(mut self, elevation: f32) -> Self {
        self.elevation = finite_non_negative(elevation);
        self
    }

    #[must_use]
    pub fn shape(mut self, shape: BorderRadius) -> Self {
        self.shape = Some(shape);
        self
    }

    #[must_use]
    pub fn radius(mut self, radius: f32) -> Self {
        self.shape = Some(BorderRadius::circular(finite_non_negative(radius)));
        self
    }

    #[must_use]
    pub fn padding(mut self, padding: EdgeInsets) -> Self {
        self.padding = padding;
        self
    }

    #[must_use]
    pub fn margin(mut self, margin: EdgeInsets) -> Self {
        self.margin = Some(margin);
        self
    }

    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(finite_non_negative(width));
        self
    }

    #[must_use]
    pub fn behavior(mut self, behavior: SnackBarBehavior) -> Self {
        self.behavior = behavior;
        self
    }

    #[must_use]
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    #[must_use]
    pub fn semantic_label(mut self, label: impl Into<String>) -> Self {
        self.semantic_label = Some(label.into());
        self
    }

    #[must_use]
    pub fn action_ref(&self) -> Option<&SnackBarAction> {
        self.action.as_ref()
    }

    #[must_use]
    pub fn behavior_value(&self) -> SnackBarBehavior {
        self.behavior
    }

    #[must_use]
    pub fn duration_value(&self) -> Duration {
        self.duration
    }

    pub(crate) fn build(&self, theme: &ControlTheme) -> Widget {
        let mut children = vec![self.content.clone()];
        if let Some(action) = self.action.as_ref() {
            children.push(action.build(theme));
        }
        let content: Widget = Row::new(children)
            .spacing(8.0)
            .main_axis_size(MainAxisSize::Min)
            .main_axis_alignment(MainAxisAlignment::SpaceBetween)
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .into();
        let shape = self.shape.unwrap_or_else(|| {
            if self.behavior == SnackBarBehavior::Floating {
                BorderRadius::circular(theme.toast.radius)
            } else {
                BorderRadius::ZERO
            }
        });
        let mut surface = Container::with_child(content)
            .padding(self.padding)
            .color(
                self.background_color
                    .unwrap_or(theme.colors.surface_elevated),
            )
            .decoration(BoxDecoration::new().border_radius(shape));
        if let Some(width) = self.width {
            surface = surface.width(width);
        }
        if let Some(margin) = self.margin {
            surface = surface.margin(margin);
        }
        let visual: Widget = if self.elevation > 0.0 {
            Widget::drop_shadow(
                Offset::new(0.0, self.elevation * 0.2),
                (self.elevation * 0.5).max(1.0),
                Color::rgba(0, 0, 0, 90),
                surface.into(),
            )
        } else {
            surface.into()
        };
        let mut semantics = Semantics::new(visual)
            .role(SemanticRole::GenericContainer)
            .state(semantic_state(true));
        if let Some(label) = self.semantic_label.clone() {
            semantics = semantics.label(label);
        }
        semantics.into()
    }
}

impl From<SnackBar> for Widget {
    fn from(value: SnackBar) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |context, _| value.build(&current_control_theme(context)))
    }
}

/// Controls whether a tooltip is currently visible.
#[derive(Clone, Default)]
pub struct TooltipController {
    visible: Rc<Cell<bool>>,
    revision: Rc<Cell<u64>>,
    hide_at: Rc<Cell<Option<Instant>>>,
}

impl TooltipController {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn show(&self) {
        self.hide_at.set(None);
        self.set_visible(true);
    }

    pub fn hide(&self) {
        self.hide_at.set(None);
        self.set_visible(false);
    }

    /// Shows the tooltip for a bounded lifetime. The runtime can call
    /// [`Self::poll`] from its normal frame tick without allocating a timer
    /// task for every tooltip.
    pub fn show_for(&self, duration: Duration, now: Instant) {
        self.hide_at
            .set((duration > Duration::ZERO).then_some(now + duration));
        self.set_visible(true);
    }

    /// Applies a scheduled hide, returning whether visibility changed.
    pub fn poll(&self, now: Instant) -> bool {
        if self.hide_at.get().is_some_and(|deadline| now >= deadline) {
            self.hide_at.set(None);
            self.set_visible(false);
            true
        } else {
            false
        }
    }

    pub fn toggle(&self) {
        self.set_visible(!self.is_visible());
    }

    #[must_use]
    pub fn is_visible(&self) -> bool {
        self.visible.get()
    }

    fn set_visible(&self, visible: bool) {
        if self.visible.replace(visible) != visible {
            self.revision.set(self.revision.get().wrapping_add(1));
        }
    }
}

/// Input policy for a [`Tooltip`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TooltipTriggerMode {
    #[default]
    Hover,
    Focus,
    Tap,
    Manual,
}

/// A Material tooltip attached to a child widget.
#[derive(Clone, TypedBuilder)]
pub struct Tooltip {
    #[builder(setter(into))]
    message: String,
    #[builder(setter(into))]
    child: Widget,
    #[builder(default = TooltipController::new())]
    controller: TooltipController,
    #[builder(default = FocusNode::new())]
    focus_node: FocusNode,
    #[builder(default = TooltipTriggerMode::Hover)]
    trigger_mode: TooltipTriggerMode,
    #[builder(default = true)]
    enabled: bool,
    #[builder(default = EdgeInsets::symmetric(16.0, 8.0))]
    padding: EdgeInsets,
    #[builder(default = EdgeInsets::all(0.0))]
    margin: EdgeInsets,
    #[builder(default, setter(strip_option))]
    text_style: Option<TextStyle>,
    #[builder(default, setter(strip_option))]
    decoration_color: Option<Color>,
    #[builder(default = true)]
    prefer_below: bool,
    #[builder(default = 14.0, setter(transform = |value: f32| finite_non_negative(value)))]
    vertical_offset: f32,
    #[builder(default = Duration::from_millis(500))]
    wait_duration: Duration,
    #[builder(default = Duration::from_millis(0))]
    show_duration: Duration,
    #[builder(default = Duration::from_millis(100))]
    exit_duration: Duration,
    #[builder(default)]
    exclude_from_semantics: bool,
}

impl Tooltip {
    #[must_use]
    pub fn new(message: impl Into<String>, child: impl Into<Widget>) -> Self {
        Self {
            message: message.into(),
            child: child.into(),
            controller: TooltipController::new(),
            focus_node: FocusNode::new(),
            trigger_mode: TooltipTriggerMode::Hover,
            enabled: true,
            padding: EdgeInsets::symmetric(16.0, 8.0),
            margin: EdgeInsets::all(0.0),
            text_style: None,
            decoration_color: None,
            prefer_below: true,
            vertical_offset: 14.0,
            wait_duration: Duration::from_millis(500),
            show_duration: Duration::from_millis(0),
            exit_duration: Duration::from_millis(100),
            exclude_from_semantics: false,
        }
    }

    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = child.into();
        self
    }

    #[must_use]
    pub fn controller(mut self, controller: TooltipController) -> Self {
        self.controller = controller;
        self
    }

    #[must_use]
    pub fn focus_node(mut self, node: FocusNode) -> Self {
        self.focus_node = node;
        self
    }

    #[must_use]
    pub fn controller_ref(&self) -> TooltipController {
        self.controller.clone()
    }

    #[must_use]
    pub fn trigger_mode(mut self, mode: TooltipTriggerMode) -> Self {
        self.trigger_mode = mode;
        self
    }

    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    #[must_use]
    pub fn padding(mut self, padding: EdgeInsets) -> Self {
        self.padding = padding;
        self
    }

    #[must_use]
    pub fn margin(mut self, margin: EdgeInsets) -> Self {
        self.margin = margin;
        self
    }

    #[must_use]
    pub fn text_style(mut self, style: TextStyle) -> Self {
        self.text_style = Some(style);
        self
    }

    #[must_use]
    pub fn decoration_color(mut self, color: Color) -> Self {
        self.decoration_color = Some(color);
        self
    }

    #[must_use]
    pub fn prefer_below(mut self, prefer_below: bool) -> Self {
        self.prefer_below = prefer_below;
        self
    }

    #[must_use]
    pub fn vertical_offset(mut self, offset: f32) -> Self {
        self.vertical_offset = finite_non_negative(offset);
        self
    }

    #[must_use]
    pub fn wait_duration(mut self, duration: Duration) -> Self {
        self.wait_duration = duration;
        self
    }

    #[must_use]
    pub fn show_duration(mut self, duration: Duration) -> Self {
        self.show_duration = duration;
        self
    }

    #[must_use]
    pub fn exit_duration(mut self, duration: Duration) -> Self {
        self.exit_duration = duration;
        self
    }

    #[must_use]
    pub fn exclude_from_semantics(mut self, exclude: bool) -> Self {
        self.exclude_from_semantics = exclude;
        self
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    #[must_use]
    pub fn wait_duration_value(&self) -> Duration {
        self.wait_duration
    }

    #[must_use]
    pub fn show_duration_value(&self) -> Duration {
        self.show_duration
    }

    #[must_use]
    pub fn exit_duration_value(&self) -> Duration {
        self.exit_duration
    }

    fn build(&self, theme: &ControlTheme) -> Widget {
        let controller = self.controller.clone();
        let mut anchor = ActionSurface::with_child(self.child.clone())
            .color(Color::TRANSPARENT)
            .hover_color(Color::TRANSPARENT)
            .pressed_color(Color::TRANSPARENT)
            .focused_color(Color::TRANSPARENT)
            .disabled_color(Color::TRANSPARENT)
            .enabled(self.enabled);
        if self.enabled {
            match self.trigger_mode {
                TooltipTriggerMode::Hover => {
                    let show = controller.clone();
                    let hide = controller.clone();
                    anchor = anchor
                        .on_hover(move || show.show())
                        .on_exit(move || hide.hide());
                }
                TooltipTriggerMode::Tap => {
                    let toggle = controller.clone();
                    anchor = anchor.on_click(move || toggle.toggle());
                }
                TooltipTriggerMode::Focus => {
                    if self.focus_node.has_focus() {
                        controller.show();
                    } else {
                        controller.hide();
                    }
                }
                TooltipTriggerMode::Manual => {}
            }
        }
        let anchor: Widget = if self.exclude_from_semantics {
            anchor.into()
        } else {
            Semantics::new(anchor).tooltip(self.message.clone()).into()
        };

        let style = self
            .text_style
            .clone()
            .unwrap_or_else(|| theme.typography.small.clone())
            .color(theme.colors.accent_foreground);
        let popup: Widget = Container::with_child(Text::new(self.message.clone()).style(style))
            .padding(self.padding)
            .margin(self.margin)
            .decoration(
                BoxDecoration::new()
                    .color(self.decoration_color.unwrap_or(theme.colors.surface_active))
                    .border_radius(BorderRadius::circular(theme.tooltip.radius)),
            )
            .into();
        let popup =
            Widget::drop_shadow(Offset::new(0.0, 2.0), 3.0, Color::rgba(0, 0, 0, 70), popup);
        let popup = if self.prefer_below {
            Align::new(
                incular_config::Alignment::TOP_CENTER,
                Padding::new(EdgeInsets::only(0.0, self.vertical_offset, 0.0, 0.0), popup),
            )
        } else {
            Align::new(
                incular_config::Alignment::BOTTOM_CENTER,
                Padding::new(EdgeInsets::only(0.0, 0.0, 0.0, self.vertical_offset), popup),
            )
        };
        let anchor = if self.trigger_mode == TooltipTriggerMode::Focus {
            Focus::new(anchor).node(self.focus_node.clone()).into()
        } else {
            anchor
        };
        OverlayPortal::new(anchor)
            .overlay_child(popup)
            .show(self.enabled && self.controller.is_visible())
            .into()
    }
}

impl From<Tooltip> for Widget {
    fn from(value: Tooltip) -> Self {
        let value = Rc::new(value);
        Widget::stateful_layout_builder(value.controller.revision.clone(), move |context, _| {
            value.build(&current_control_theme(context))
        })
    }
}
