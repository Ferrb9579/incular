use crate::styles::{ButtonStyle, ButtonVariant, ControlState};
use crate::theme::ControlTheme;
use incular_config::{Alignment, EdgeInsets};
use incular_core::Color;
use incular_core::Offset;
use incular_semantics::{Role as SemanticRole, SemanticActionKind, SemanticState};
use incular_widgets::internal::{ActionSurface, DropShadow, ExplicitSemantics};
use incular_widgets::{Align, Border, BorderRadius, BoxDecoration, Container, Text, Widget};
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
        let hover_state = state.with(ControlState::HOVERED);
        let pressed_state = state.with(ControlState::PRESSED);
        let focused_state = state.with(ControlState::FOCUSED);
        let hover_bg = blend_overlay(
            self.style.resolve_background(hover_state, theme),
            self.style.resolve_overlay_color(hover_state, theme),
        );
        let pressed_bg = blend_overlay(
            self.style.resolve_background(pressed_state, theme),
            self.style.resolve_overlay_color(pressed_state, theme),
        );
        let focused_bg = blend_overlay(
            self.style.resolve_background(focused_state, theme),
            self.style.resolve_overlay_color(focused_state, theme),
        );
        let disabled_bg = self.style.resolve_background(ControlState::DISABLED, theme);
        let fg = self.style.resolve_foreground(state, theme);
        let radius = self
            .style
            .shape
            .as_ref()
            .map(|shape| {
                shape
                    .resolve(state)
                    .top_left
                    .x
                    .max(shape.resolve(state).top_left.y)
            })
            .or(self.style.border_radius)
            .unwrap_or(theme.button.radius);
        let padding = self
            .style
            .padding
            .unwrap_or_else(|| theme.density.padding());
        let height = self.style.height.unwrap_or(theme.button.height);
        let elevation = self.style.resolve_elevation(state, theme);

        let mut content: Widget = if let Some(label) = self.label.as_ref() {
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
                // Filled actions do not need a second outline around their
                // surface. Callers can still opt into one with `border` or
                // `side`, including state-aware sides.
                ButtonVariant::Primary | ButtonVariant::Danger | ButtonVariant::Destructive => {
                    Border::new(0.0, Color::TRANSPARENT)
                }
                ButtonVariant::Default | ButtonVariant::Standard => {
                    Border::new(theme.button.border_width, theme.colors.border)
                }
                ButtonVariant::Ghost => Border::new(0.0, Color::TRANSPARENT),
            });

        // `Container::alignment` is intentionally expanding, which is useful
        // for panels but wrong for a button in a loose row/column: it makes
        // the button consume the parent's entire bounded width. A normal
        // button should size to its label plus padding. Explicit fixed sizes
        // keep the expanding alignment behavior so custom-width buttons still
        // honor their requested alignment.
        let shrink_wrap = self.style.fixed_size.is_none();
        let content = if shrink_wrap {
            Align::new(self.style.alignment.unwrap_or(Alignment::CENTER), content)
                .width_factor(1.0)
                .into()
        } else {
            content
        };

        let mut decorated = Container::new()
            .height(height)
            .padding(padding)
            .decoration(
                BoxDecoration::new()
                    // The retained action surface owns the state-aware surface;
                    // keeping this wrapper transparent lets hover/pressed
                    // paint updates remain visible instead of being covered
                    // by a second opaque decoration.
                    .color(Color::TRANSPARENT)
                    .border(border)
                    .border_radius(BorderRadius::circular(radius)),
            )
            .child(content);
        if !shrink_wrap {
            decorated = decorated.alignment(self.style.alignment.unwrap_or(Alignment::CENTER));
        }

        if let Some(builder) = self.style.background_builder.as_ref() {
            decorated = Container::with_child(builder.build(decorated.into(), state));
        }

        // Elevation is a paint/composite concern. Keep it outside the action
        // surface so hover/press state can change without rebuilding the
        // button's content, while still reaching the retained drop-shadow
        // renderer instead of remaining metadata-only.
        let decorated: Widget = if elevation > 0.0 {
            DropShadow::new(
                Offset::new(0.0, elevation * 0.18),
                (elevation * 0.55).max(1.0),
                self.style.resolve_shadow_color(state, theme),
                decorated,
            )
            .into()
        } else {
            decorated.into()
        };

        let mut raw = ActionSurface::with_child(decorated)
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

impl From<Button> for Widget {
    fn from(value: Button) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| {
            let theme = incular_widgets::internal::current_build_environment::<ControlTheme>()
                .unwrap_or_default();
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
