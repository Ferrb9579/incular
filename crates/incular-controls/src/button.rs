use crate::styles::{
    ButtonStyle, ButtonVariant, ControlState, IconAlignment, SplashFactory, TapTargetSize,
    mouse_cursor_from_name,
};
use crate::theme::ControlTheme;
use incular_config::{Alignment, Constraints, CrossAxisAlignment, EdgeInsets};
use incular_core::Color;
use incular_core::Offset;
use incular_semantics::{Role as SemanticRole, SemanticActionKind, SemanticState};
use incular_widgets::internal::{
    ActionInteractionController, ActionInteractionState, ActionPolicy, ActionSplashPolicy,
    ActionSurface, DropShadow, ExplicitSemantics,
};
use incular_widgets::{
    Align, Border, BorderRadius, BoxDecoration, Container, IconTheme, MouseCursor, Row, SizedBox,
    Text, Widget,
};
use std::{rc::Rc, time::Duration};
use typed_builder::TypedBuilder;

/// Platform-neutral styled push button.
#[derive(Clone, TypedBuilder)]
pub struct Button {
    #[builder(default, setter(strip_option, into))]
    label: Option<String>,
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    icon: Option<Widget>,
    #[builder(default)]
    style: ButtonStyle,
    #[builder(default = true)]
    enabled: bool,
    #[builder(default)]
    focusable_when_disabled: bool,
    #[builder(default)]
    loading: bool,
    #[builder(
        default,
        setter(
            fn transform<F>(callback: F) -> Option<Rc<dyn Fn() + 'static>>
            where
                F: Fn() + 'static,
            {
                Some(Rc::new(callback))
            }
        )
    )]
    on_click: Option<Rc<dyn Fn() + 'static>>,
}

impl Default for Button {
    fn default() -> Self {
        Self {
            label: None,
            child: None,
            icon: None,
            style: ButtonStyle::default(),
            enabled: true,
            focusable_when_disabled: false,
            loading: false,
            on_click: None,
        }
    }
}

impl Button {
    /// Creates a button with a text label.
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self::builder().label(label).build()
    }

    /// Creates a button with a custom child widget.
    #[must_use]
    pub fn with_child(child: impl Into<Widget>) -> Self {
        Self::builder().child(child).build()
    }

    /// Creates a button whose icon and label are composed by the live retained
    /// presentation root, allowing state-aware icon styling and alignment.
    #[must_use]
    pub fn with_icon_label(icon: impl Into<Widget>, label: impl Into<String>) -> Self {
        Self::builder().icon(icon).label(label).build()
    }

    /// Replaces the button's text label and clears any custom child.
    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self.child = None;
        self.icon = None;
        self
    }

    /// Replaces the button's content with an arbitrary widget.
    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self.label = None;
        self.icon = None;
        self
    }

    /// Content-oriented alias for [`Button::child`].
    #[must_use]
    pub fn content(self, content: impl Into<Widget>) -> Self {
        self.child(content)
    }

    /// Sets the button visual variant.
    #[must_use]
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.style.variant = variant;
        self
    }

    /// Overrides specific button styling properties.
    #[must_use]
    pub fn style(mut self, style: ButtonStyle) -> Self {
        self.style = style;
        self
    }

    /// Sets whether the button is enabled for user interaction.
    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Keeps a disabled button in the focus order (useful for loading or
    /// explanatory states) while still suppressing activation.
    #[must_use]
    pub fn focusable_when_disabled(mut self, focusable: bool) -> Self {
        self.focusable_when_disabled = focusable;
        self
    }

    /// Shows a busy state while retaining focus and suppressing activation.
    #[must_use]
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// Sets click/activation callback.
    #[must_use]
    pub fn on_click(mut self, on_click: impl Fn() + 'static) -> Self {
        self.on_click = Some(Rc::new(on_click));
        self
    }

    /// Builds the styled widget tree using the provided or default theme.
    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        let effective_enabled = self.enabled && !self.loading;
        let interaction = ActionInteractionController::new();
        let presentation = {
            let button = self.clone();
            let theme = theme.clone();
            let interaction = interaction.clone();
            Widget::stateful_layout_builder(interaction.revision(), move |_, _| {
                let state = button.control_state(effective_enabled, interaction.state());
                button.build_presentation(&theme, state)
            })
        };

        let mut raw = ActionSurface::with_child(presentation)
            .color(Color::TRANSPARENT)
            .hover_color(Color::TRANSPARENT)
            .pressed_color(Color::TRANSPARENT)
            .focused_color(theme.colors.focus_ring)
            .disabled_color(Color::TRANSPARENT)
            .enabled(effective_enabled)
            .focusable_when_disabled(self.focusable_when_disabled || self.loading)
            .interaction_controller(interaction)
            .policy(ActionPolicy {
                feedback_enabled: self.style.enable_feedback.unwrap_or(true),
                splash: match self.style.splash_factory.unwrap_or_default() {
                    SplashFactory::Ripple => ActionSplashPolicy::Ripple,
                    SplashFactory::Splash => ActionSplashPolicy::Splash,
                    SplashFactory::Sparkle => ActionSplashPolicy::Sparkle,
                    SplashFactory::NoSplash => ActionSplashPolicy::None,
                },
                transition_duration: self.style.animation_duration.unwrap_or_else(|| {
                    Duration::from_millis(u64::from(theme.motion.pressed_duration_ms))
                }),
            })
            .mouse_cursor(
                self.style
                    .mouse_cursor
                    .as_deref()
                    .map(mouse_cursor_from_name)
                    .unwrap_or(if effective_enabled {
                        MouseCursor::Click
                    } else {
                        MouseCursor::Basic
                    }),
            );
        if let Some(cb) = self.on_click.clone()
            && effective_enabled
        {
            raw = raw.on_click(move || cb());
        }
        let semantic_label = self
            .label
            .clone()
            .or_else(|| self.child.as_ref().and_then(Widget::semantic_text))
            .unwrap_or_default();
        let mut semantics = ExplicitSemantics::new(SemanticRole::Button)
            .label(semantic_label)
            .state(SemanticState {
                enabled: effective_enabled,
                focusable: self.enabled || self.focusable_when_disabled || self.loading,
                ..SemanticState::default()
            });
        if effective_enabled {
            semantics =
                semantics.actions([SemanticActionKind::Focus, SemanticActionKind::Activate]);
        } else if self.focusable_when_disabled || self.loading {
            semantics = semantics.actions([SemanticActionKind::Focus]);
        }
        Widget::from(raw).semantics(semantics)
    }

    fn control_state(
        &self,
        effective_enabled: bool,
        interaction: ActionInteractionState,
    ) -> ControlState {
        let mut state = ControlState::from_enabled(effective_enabled);
        if interaction.hovered {
            state.insert(ControlState::HOVERED);
        }
        if interaction.pressed {
            state.insert(ControlState::PRESSED);
        }
        if interaction.focused {
            state.insert(ControlState::FOCUSED);
            state.insert(ControlState::FOCUS_VISIBLE);
        }
        state
    }

    fn build_presentation(&self, theme: &ControlTheme, state: ControlState) -> Widget {
        let elevation = self.style.resolve_elevation(state, theme);
        let splash = self.style.splash_factory.unwrap_or_default();
        let overlay = if state.is_pressed() && splash == SplashFactory::NoSplash {
            Color::TRANSPARENT
        } else {
            self.style.resolve_overlay_color(state, theme)
        };
        let mut background = blend_overlay(self.style.resolve_background(state, theme), overlay);
        if self.style.surface_tint_color.is_some() && elevation > 0.0 {
            let tint = self.style.resolve_surface_tint_color(state, theme);
            let alpha = (elevation * 0.04).clamp(0.0, 0.16);
            background = blend_overlay(
                background,
                Color::rgba(
                    tint.red,
                    tint.green,
                    tint.blue,
                    (alpha * 255.0).round() as u8,
                ),
            );
        }
        let fg = self.style.resolve_foreground(state, theme);
        let radius = self
            .style
            .shape
            .as_ref()
            .map(|shape| shape.resolve(state))
            .unwrap_or_else(|| {
                BorderRadius::circular(self.style.border_radius.unwrap_or(theme.button.radius))
            });
        let density = self.style.visual_density.unwrap_or(theme.density);
        let padding = self.style.padding.unwrap_or_else(|| density.padding());
        let fixed_size = self.style.fixed_size;
        let mut height = fixed_size
            .map(|size| size.height)
            .or(self.style.height)
            .unwrap_or_else(|| {
                if self.style.visual_density.is_some() {
                    density.control_height()
                } else {
                    theme.button.height
                }
            });
        if fixed_size.is_none() {
            let mut minimum_height = self
                .style
                .minimum_size
                .map_or(0.0, |size| size.height.max(0.0));
            if self.style.tap_target_size == Some(TapTargetSize::Padded) {
                minimum_height = minimum_height.max(48.0);
            }
            height = height.max(minimum_height);
            if let Some(maximum) = self.style.maximum_size {
                height = height.min(maximum.height.max(minimum_height));
            }
        }

        let label: Option<Widget> = self.label.as_ref().map(|label| {
            Widget::from(
                Text::new(label.clone()).style(
                    self.style
                        .text_style
                        .clone()
                        .unwrap_or_else(|| theme.typography.body.clone())
                        .color(fg),
                ),
            )
        });
        let icon: Option<Widget> = self.icon.as_ref().map(|icon| {
            let icon_color = self
                .style
                .icon_color
                .as_ref()
                .map_or(fg, |value| value.resolve(state));
            let icon_size = self
                .style
                .icon_size
                .as_ref()
                .map_or(theme.metrics.icon_size, |value| {
                    value.resolve(state).max(0.0)
                });
            IconTheme::new(icon.clone())
                .color(icon_color)
                .size(icon_size)
                .into()
        });
        let mut content: Widget = match (icon, label) {
            (Some(icon), Some(label)) => {
                let spacing: Widget = SizedBox::new().width(theme.metrics.small_spacing).into();
                let children = match self.style.icon_alignment.unwrap_or_default() {
                    IconAlignment::Start => [icon, spacing, label],
                    IconAlignment::End => [label, spacing, icon],
                };
                Row::new(children)
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .into()
            }
            (Some(icon), None) => icon,
            (None, Some(label)) => label,
            (None, None) => self
                .child
                .clone()
                .unwrap_or_else(|| incular_widgets::SizedBox::shrink().into()),
        };

        if let Some(builder) = self.style.foreground_builder.as_ref() {
            content = builder.build(content, state);
        }

        let border = self
            .style
            .side
            .as_ref()
            .map(|side| side.resolve(state))
            .or(self.style.border)
            .unwrap_or_else(|| match self.style.variant {
                ButtonVariant::Primary | ButtonVariant::Danger | ButtonVariant::Destructive => {
                    Border::new(0.0, Color::TRANSPARENT)
                }
                ButtonVariant::Default | ButtonVariant::Standard => {
                    Border::new(theme.button.border_width, theme.colors.border)
                }
                ButtonVariant::Ghost => Border::new(0.0, Color::TRANSPARENT),
            });

        let shrink_wrap = self.style.fixed_size.is_none();
        let content = if shrink_wrap {
            Align::new(self.style.alignment.unwrap_or(Alignment::CENTER), content)
                .width_factor(1.0)
                .into()
        } else {
            content
        };

        let mut decoration = BoxDecoration::new().color(background).border_radius(radius);
        if has_visible_border(&border) {
            decoration = decoration.border(border);
        }
        let mut decorated = Container::new()
            .height(height)
            .padding(padding)
            .decoration(decoration)
            .child(content);
        if let Some(size) = fixed_size {
            decorated = decorated.width(size.width);
        }
        if !shrink_wrap {
            decorated = decorated.alignment(self.style.alignment.unwrap_or(Alignment::CENTER));
        }

        let mut min_width = self
            .style
            .minimum_size
            .map_or(0.0, |size| size.width.max(0.0));
        let mut min_height = self
            .style
            .minimum_size
            .map_or(0.0, |size| size.height.max(0.0));
        if self.style.tap_target_size == Some(TapTargetSize::Padded) {
            min_width = min_width.max(48.0);
            min_height = min_height.max(48.0);
        }
        let mut max_width = self
            .style
            .maximum_size
            .map_or(f32::INFINITY, |size| size.width.max(min_width));
        let mut max_height = self
            .style
            .maximum_size
            .map_or(f32::INFINITY, |size| size.height.max(min_height));
        if let Some(size) = fixed_size {
            min_width = size.width.max(0.0);
            max_width = min_width;
            min_height = size.height.max(0.0);
            max_height = min_height;
        }
        if min_width > 0.0 || min_height > 0.0 || max_width.is_finite() || max_height.is_finite() {
            decorated = decorated.constraints(Constraints::new(
                min_width, max_width, min_height, max_height,
            ));
        }

        let mut decorated: Widget = decorated.into();
        if let Some(builder) = self.style.background_builder.as_ref() {
            decorated = builder.build(decorated, state);
        }

        if elevation > 0.0 {
            DropShadow::new(
                Offset::new(0.0, elevation * 0.18),
                (elevation * 0.55).max(1.0),
                self.style.resolve_shadow_color(state, theme),
                decorated,
            )
            .into()
        } else {
            decorated
        }
    }
}

fn blend_overlay(base: Color, overlay: Color) -> Color {
    let source_alpha = overlay.alpha as f32 / 255.0;
    if source_alpha <= 0.0 {
        return base;
    }
    let destination_alpha = base.alpha as f32 / 255.0;
    let output_alpha = source_alpha + destination_alpha * (1.0 - source_alpha);
    if output_alpha <= f32::EPSILON {
        return Color::TRANSPARENT;
    }
    let channel = |source: u8, destination: u8| {
        ((source as f32 * source_alpha
            + destination as f32 * destination_alpha * (1.0 - source_alpha))
            / output_alpha)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    Color::rgba(
        channel(overlay.red, base.red),
        channel(overlay.green, base.green),
        channel(overlay.blue, base.blue),
        (output_alpha * 255.0).round().clamp(0.0, 255.0) as u8,
    )
}

fn has_visible_border(border: &Border) -> bool {
    [border.top, border.right, border.bottom, border.left]
        .into_iter()
        .any(|side| side.width > 0.0 && side.color.alpha > 0)
}

impl From<Button> for Widget {
    fn from(value: Button) -> Self {
        let value = Rc::new(value);
        let semantic_label = value
            .label
            .clone()
            .or_else(|| value.child.as_ref().and_then(Widget::semantic_text));
        let widget = Widget::from(incular_widgets::LayoutBuilder::new(move |context, _| {
            let theme = crate::theme::current_control_theme(context);
            value.build(&theme)
        }));
        semantic_label.map_or(widget.clone(), |label| widget.accessibility_label(label))
    }
}

/// Primary high-emphasis action button.
#[derive(Clone, TypedBuilder)]
pub struct PrimaryButton {
    #[builder(default, setter(strip_option, into))]
    label: Option<String>,
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
    #[builder(default = ButtonStyle::new().variant(ButtonVariant::Primary))]
    style: ButtonStyle,
    #[builder(default = true)]
    enabled: bool,
    #[builder(default)]
    focusable_when_disabled: bool,
    #[builder(default)]
    loading: bool,
    #[builder(
        default,
        setter(
            fn transform<F>(callback: F) -> Option<Rc<dyn Fn() + 'static>>
            where
                F: Fn() + 'static,
            {
                Some(Rc::new(callback))
            }
        )
    )]
    on_click: Option<Rc<dyn Fn() + 'static>>,
}

impl Default for PrimaryButton {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl PrimaryButton {
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self::builder().label(label).build()
    }

    #[must_use]
    pub fn with_child(child: impl Into<Widget>) -> Self {
        Self::builder().child(child).build()
    }

    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self.child = None;
        self
    }

    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self.label = None;
        self
    }

    #[must_use]
    pub fn content(self, content: impl Into<Widget>) -> Self {
        self.child(content)
    }

    #[must_use]
    pub fn style(mut self, style: ButtonStyle) -> Self {
        self.style = style;
        self
    }
    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    #[must_use]
    pub fn on_click(mut self, callback: impl Fn() + 'static) -> Self {
        self.on_click = Some(Rc::new(callback));
        self
    }
    #[must_use]
    pub fn focusable_when_disabled(mut self, value: bool) -> Self {
        self.focusable_when_disabled = value;
        self
    }
    #[must_use]
    pub fn loading(mut self, value: bool) -> Self {
        self.loading = value;
        self
    }

    fn into_button(self) -> Button {
        Button {
            label: self.label,
            child: self.child,
            icon: None,
            style: self.style,
            enabled: self.enabled,
            focusable_when_disabled: self.focusable_when_disabled,
            loading: self.loading,
            on_click: self.on_click,
        }
    }
}

impl From<PrimaryButton> for Widget {
    fn from(value: PrimaryButton) -> Self {
        value.into_button().into()
    }
}

/// Ghost/flat button for toolbars and lightweight actions.
#[derive(Clone, TypedBuilder)]
pub struct GhostButton {
    #[builder(default, setter(strip_option, into))]
    label: Option<String>,
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
    #[builder(default = ButtonStyle::new().variant(ButtonVariant::Ghost))]
    style: ButtonStyle,
    #[builder(default = true)]
    enabled: bool,
    #[builder(default)]
    focusable_when_disabled: bool,
    #[builder(default)]
    loading: bool,
    #[builder(
        default,
        setter(
            fn transform<F>(callback: F) -> Option<Rc<dyn Fn() + 'static>>
            where
                F: Fn() + 'static,
            {
                Some(Rc::new(callback))
            }
        )
    )]
    on_click: Option<Rc<dyn Fn() + 'static>>,
}

impl Default for GhostButton {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl GhostButton {
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self::builder().label(label).build()
    }

    #[must_use]
    pub fn with_child(child: impl Into<Widget>) -> Self {
        Self::builder().child(child).build()
    }

    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self.child = None;
        self
    }

    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self.label = None;
        self
    }

    #[must_use]
    pub fn content(self, content: impl Into<Widget>) -> Self {
        self.child(content)
    }

    #[must_use]
    pub fn style(mut self, style: ButtonStyle) -> Self {
        self.style = style;
        self
    }
    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    #[must_use]
    pub fn on_click(mut self, callback: impl Fn() + 'static) -> Self {
        self.on_click = Some(Rc::new(callback));
        self
    }
    #[must_use]
    pub fn focusable_when_disabled(mut self, value: bool) -> Self {
        self.focusable_when_disabled = value;
        self
    }
    #[must_use]
    pub fn loading(mut self, value: bool) -> Self {
        self.loading = value;
        self
    }

    fn into_button(self) -> Button {
        Button {
            label: self.label,
            child: self.child,
            icon: None,
            style: self.style,
            enabled: self.enabled,
            focusable_when_disabled: self.focusable_when_disabled,
            loading: self.loading,
            on_click: self.on_click,
        }
    }
}

impl From<GhostButton> for Widget {
    fn from(value: GhostButton) -> Self {
        value.into_button().into()
    }
}

/// Compact square icon button.
#[derive(Clone, TypedBuilder)]
pub struct IconButton {
    #[builder(default, setter(strip_option, into))]
    label: Option<String>,
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
    #[builder(
        default = ButtonStyle::new()
            .variant(ButtonVariant::Ghost)
            .padding(EdgeInsets::all(4.0))
            .height(28.0)
    )]
    style: ButtonStyle,
    #[builder(default = true)]
    enabled: bool,
    #[builder(default)]
    focusable_when_disabled: bool,
    #[builder(default)]
    loading: bool,
    #[builder(
        default,
        setter(
            fn transform<F>(callback: F) -> Option<Rc<dyn Fn() + 'static>>
            where
                F: Fn() + 'static,
            {
                Some(Rc::new(callback))
            }
        )
    )]
    on_click: Option<Rc<dyn Fn() + 'static>>,
}

impl Default for IconButton {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl IconButton {
    #[must_use]
    pub fn new(icon: impl Into<String>) -> Self {
        Self::builder().label(icon).build()
    }

    #[must_use]
    pub fn with_child(child: impl Into<Widget>) -> Self {
        Self::builder().child(child).build()
    }

    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self.child = None;
        self
    }

    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self.label = None;
        self
    }

    #[must_use]
    pub fn content(self, content: impl Into<Widget>) -> Self {
        self.child(content)
    }

    #[must_use]
    pub fn style(mut self, style: ButtonStyle) -> Self {
        self.style = style;
        self
    }
    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    #[must_use]
    pub fn on_click(mut self, callback: impl Fn() + 'static) -> Self {
        self.on_click = Some(Rc::new(callback));
        self
    }
    #[must_use]
    pub fn focusable_when_disabled(mut self, value: bool) -> Self {
        self.focusable_when_disabled = value;
        self
    }
    #[must_use]
    pub fn loading(mut self, value: bool) -> Self {
        self.loading = value;
        self
    }

    fn into_button(self) -> Button {
        Button {
            label: self.label,
            child: self.child,
            icon: None,
            style: self.style,
            enabled: self.enabled,
            focusable_when_disabled: self.focusable_when_disabled,
            loading: self.loading,
            on_click: self.on_click,
        }
    }
}

impl From<IconButton> for Widget {
    fn from(value: IconButton) -> Self {
        value.into_button().into()
    }
}
