use crate::styles::{ButtonStyle, ButtonVariant, ControlState};
use crate::theme::ControlTheme;
use incular_config::EdgeInsets;
use incular_core::Color;
use incular_semantics::{Role as SemanticRole, SemanticActionKind, SemanticState};
use incular_widgets::{
    Border, BorderRadius, BoxDecoration, Button as RawButton, Container, ExplicitSemantics, Text,
    Widget,
};
use std::rc::Rc;

/// Platform-neutral styled push button.
#[derive(Clone)]
pub struct Button {
    label: Option<String>,
    child: Option<Widget>,
    style: ButtonStyle,
    enabled: bool,
    focusable_when_disabled: bool,
    loading: bool,
    on_click: Option<Rc<dyn Fn() + 'static>>,
}

impl Button {
    /// Creates a button with a text label.
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: Some(label.into()),
            child: None,
            style: ButtonStyle::new(),
            enabled: true,
            focusable_when_disabled: false,
            loading: false,
            on_click: None,
        }
    }

    /// Creates a button with a custom child widget.
    #[must_use]
    pub fn with_child(child: impl Into<Widget>) -> Self {
        Self {
            label: None,
            child: Some(child.into()),
            style: ButtonStyle::new(),
            enabled: true,
            focusable_when_disabled: false,
            loading: false,
            on_click: None,
        }
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
        let state = ControlState::from_enabled(effective_enabled);

        let bg = self.style.resolve_background(state, theme);
        let hover_bg = self
            .style
            .resolve_background(state.with(ControlState::HOVERED), theme);
        let pressed_bg = self
            .style
            .resolve_background(state.with(ControlState::PRESSED), theme);
        let focused_bg = self
            .style
            .resolve_background(state.with(ControlState::FOCUSED), theme);
        let disabled_bg = self.style.resolve_background(ControlState::DISABLED, theme);
        let fg = self.style.resolve_foreground(state, theme);
        let radius = self.style.border_radius.unwrap_or(theme.button.radius);
        let padding = self
            .style
            .padding
            .unwrap_or_else(|| theme.density.padding());
        let height = self.style.height.unwrap_or(theme.button.height);

        let content: Widget = if let Some(label) = self.label.as_ref() {
            Text::new(label.clone())
                .style(
                    self.style
                        .text_style
                        .clone()
                        .unwrap_or_else(|| theme.typography.body.clone())
                        .color(fg),
                )
                .into()
        } else if let Some(child) = self.child.as_ref() {
            child.clone()
        } else {
            incular_widgets::SizedBox::shrink().into()
        };

        let border = self.style.border.unwrap_or_else(|| {
            if self.style.variant == ButtonVariant::Ghost {
                Border::new(0.0, Color::TRANSPARENT)
            } else {
                Border::new(theme.button.border_width, theme.colors.border)
            }
        });

        let decorated = Container::new()
            .height(height)
            .padding(padding)
            .decoration(
                BoxDecoration::new()
                    // The retained raw button owns the state-aware surface;
                    // keeping this wrapper transparent lets hover/pressed
                    // paint updates remain visible instead of being covered
                    // by a second opaque decoration.
                    .color(Color::TRANSPARENT)
                    .border(border)
                    .border_radius(BorderRadius::circular(radius)),
            )
            .child(content);

        let mut raw = RawButton::with_child(decorated)
            .color(bg)
            .hover_color(hover_bg)
            .pressed_color(pressed_bg)
            .focused_color(focused_bg)
            .disabled_color(disabled_bg)
            .enabled(effective_enabled)
            .focusable_when_disabled(self.focusable_when_disabled || self.loading);
        if let Some(cb) = self.on_click.clone() {
            if effective_enabled {
                raw = raw.on_click(move || cb());
            }
        }
        let mut semantics = ExplicitSemantics::new(SemanticRole::Button)
            .label(self.label.clone().unwrap_or_default())
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
        let raw: Widget = raw.into();
        raw.semantics(semantics)
    }
}

impl From<Button> for Widget {
    fn from(value: Button) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| {
            let theme =
                incular_widgets::current_build_environment::<ControlTheme>().unwrap_or_default();
            value.build(&theme)
        })
    }
}

/// Primary high-emphasis action button.
#[derive(Clone)]
pub struct PrimaryButton {
    inner: Button,
}

impl PrimaryButton {
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            inner: Button::new(label).variant(ButtonVariant::Primary),
        }
    }

    #[must_use]
    pub fn with_child(child: impl Into<Widget>) -> Self {
        Self {
            inner: Button::with_child(child).variant(ButtonVariant::Primary),
        }
    }

    #[must_use]
    pub fn style(mut self, style: ButtonStyle) -> Self {
        self.inner = self.inner.style(style);
        self
    }
    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.inner = self.inner.enabled(enabled);
        self
    }
    #[must_use]
    pub fn on_click(mut self, callback: impl Fn() + 'static) -> Self {
        self.inner = self.inner.on_click(callback);
        self
    }
    #[must_use]
    pub fn focusable_when_disabled(mut self, value: bool) -> Self {
        self.inner = self.inner.focusable_when_disabled(value);
        self
    }
    #[must_use]
    pub fn loading(mut self, value: bool) -> Self {
        self.inner = self.inner.loading(value);
        self
    }
}

impl From<PrimaryButton> for Widget {
    fn from(value: PrimaryButton) -> Self {
        value.inner.into()
    }
}

/// Ghost/flat button for toolbars and lightweight actions.
#[derive(Clone)]
pub struct GhostButton {
    inner: Button,
}

impl GhostButton {
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            inner: Button::new(label).variant(ButtonVariant::Ghost),
        }
    }

    #[must_use]
    pub fn with_child(child: impl Into<Widget>) -> Self {
        Self {
            inner: Button::with_child(child).variant(ButtonVariant::Ghost),
        }
    }

    #[must_use]
    pub fn style(mut self, style: ButtonStyle) -> Self {
        self.inner = self.inner.style(style);
        self
    }
    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.inner = self.inner.enabled(enabled);
        self
    }
    #[must_use]
    pub fn on_click(mut self, callback: impl Fn() + 'static) -> Self {
        self.inner = self.inner.on_click(callback);
        self
    }
    #[must_use]
    pub fn focusable_when_disabled(mut self, value: bool) -> Self {
        self.inner = self.inner.focusable_when_disabled(value);
        self
    }
    #[must_use]
    pub fn loading(mut self, value: bool) -> Self {
        self.inner = self.inner.loading(value);
        self
    }
}

impl From<GhostButton> for Widget {
    fn from(value: GhostButton) -> Self {
        value.inner.into()
    }
}

/// Compact square icon button.
#[derive(Clone)]
pub struct IconButton {
    inner: Button,
}

impl IconButton {
    #[must_use]
    pub fn new(icon: impl Into<String>) -> Self {
        Self {
            inner: Button::new(icon).variant(ButtonVariant::Ghost).style(
                ButtonStyle::new()
                    .padding(EdgeInsets::all(4.0))
                    .height(28.0),
            ),
        }
    }

    #[must_use]
    pub fn with_child(child: impl Into<Widget>) -> Self {
        Self {
            inner: Button::with_child(child)
                .variant(ButtonVariant::Ghost)
                .style(
                    ButtonStyle::new()
                        .padding(EdgeInsets::all(4.0))
                        .height(28.0),
                ),
        }
    }

    #[must_use]
    pub fn style(mut self, style: ButtonStyle) -> Self {
        self.inner = self.inner.style(style);
        self
    }
    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.inner = self.inner.enabled(enabled);
        self
    }
    #[must_use]
    pub fn on_click(mut self, callback: impl Fn() + 'static) -> Self {
        self.inner = self.inner.on_click(callback);
        self
    }
    #[must_use]
    pub fn focusable_when_disabled(mut self, value: bool) -> Self {
        self.inner = self.inner.focusable_when_disabled(value);
        self
    }
    #[must_use]
    pub fn loading(mut self, value: bool) -> Self {
        self.inner = self.inner.loading(value);
        self
    }
}

impl From<IconButton> for Widget {
    fn from(value: IconButton) -> Self {
        value.inner.into()
    }
}
