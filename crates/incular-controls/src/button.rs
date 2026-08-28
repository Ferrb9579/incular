use crate::styles::{ButtonStyle, ButtonVariant, ControlState};
use crate::theme::ControlTheme;
use incular_config::{Alignment, EdgeInsets};
use incular_core::Color;
use incular_core::Offset;
use incular_semantics::{Role as SemanticRole, SemanticActionKind, SemanticState};
use incular_widgets::internal::{ActionSurface, DropShadow, ExplicitSemantics};
use incular_widgets::{Align, Border, BorderRadius, BoxDecoration, Container, Text, Widget};
use std::rc::Rc;
use typed_builder::TypedBuilder;

/// Platform-neutral styled push button.
#[derive(Clone, TypedBuilder)]
pub struct Button {
    #[builder(default, setter(strip_option, into))]
    label: Option<String>,
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
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

    /// Replaces the button's text label and clears any custom child.
    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self.child = None;
        self
    }

    /// Replaces the button's content with an arbitrary widget.
    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self.label = None;
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
        let fixed_size = self.style.fixed_size;
        let height = fixed_size
            .map(|size| size.height)
            .or(self.style.height)
            .unwrap_or(theme.button.height);
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

        let mut decoration = BoxDecoration::new().border_radius(BorderRadius::circular(radius));
        if has_visible_border(&border) {
            decoration = decoration.border(border);
        }
        // The retained action surface owns the state-aware surface. Keep this
        // wrapper free of a second background/border so focus rings remain a
        // separate interaction layer instead of becoming decoration.
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

fn has_visible_border(border: &Border) -> bool {
    [border.top, border.right, border.bottom, border.left]
        .into_iter()
        .any(|side| side.width > 0.0 && side.color.alpha > 0)
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

#[cfg(test)]
mod tests {
    use super::*;
    use incular_config::Constraints;
    use incular_core::Size;
    use incular_rendering::PaintCommand;
    use incular_widgets::internal::WidgetTree;

    #[test]
    fn button_builder_defaults_and_accepts_generic_children() {
        let default = Button::default();
        let built = Button::builder().build();

        assert!(default.label.is_none());
        assert!(default.child.is_none());
        assert_eq!(default.style, built.style);
        assert!(built.enabled);
        assert!(!built.focusable_when_disabled);
        assert!(!built.loading);
        assert!(built.on_click.is_none());

        let button = Button::builder()
            .child(Text::new("Save"))
            .enabled(false)
            .on_click(|| ())
            .build();
        assert!(button.label.is_none());
        assert!(button.child.is_some());
        assert!(!button.enabled);
        assert!(button.on_click.is_some());
    }

    #[test]
    fn specialized_button_builders_keep_variant_presets() {
        let primary = PrimaryButton::builder()
            .child(Text::new("Save"))
            .on_click(|| ())
            .build();
        assert_eq!(primary.style.variant, ButtonVariant::Primary);
        assert!(primary.child.is_some());

        let ghost = GhostButton::builder().label("Cancel").build();
        assert_eq!(ghost.style.variant, ButtonVariant::Ghost);
        assert_eq!(ghost.label.as_deref(), Some("Cancel"));

        let icon = IconButton::builder().build();
        assert_eq!(icon.style.variant, ButtonVariant::Ghost);
        assert_eq!(icon.style.padding, Some(EdgeInsets::all(4.0)));
        assert_eq!(icon.style.height, Some(28.0));

        let _: Widget = PrimaryButton::builder().label("Save").build().into();
        let _: Widget = GhostButton::builder().label("Cancel").build().into();
        let _: Widget = IconButton::builder().label("Close").build().into();
    }

    #[test]
    fn button_defaults_shrink_wrap_and_fixed_size_is_honored() {
        let mut tree = WidgetTree::new();
        let root = tree.mount(Button::new("OK").into()).unwrap();
        tree.layout(Constraints::loose(Size::new(400.0, 300.0)));
        let size = tree.render_size(tree.render_id(root).unwrap()).unwrap();
        assert!(size.width > 0.0 && size.width < 400.0);
        assert_eq!(size.height, 32.0);

        let mut tree = WidgetTree::new();
        let root = tree
            .mount(
                Button::new("OK")
                    .style(ButtonStyle::new().fixed_size(Size::new(180.0, 48.0)))
                    .into(),
            )
            .unwrap();
        tree.layout(Constraints::loose(Size::new(400.0, 300.0)));
        let size = tree.render_size(tree.render_id(root).unwrap()).unwrap();
        assert_eq!(size, Size::new(180.0, 48.0));
    }

    #[test]
    fn primary_button_has_no_decorative_default_border() {
        let mut tree = WidgetTree::new();
        let root = tree.mount(PrimaryButton::new("Save").into()).unwrap();
        tree.layout(Constraints::loose(Size::new(240.0, 80.0)));
        let _ = tree.render_id(root).unwrap();
        let list = tree.paint();

        assert!(
            !list
                .commands()
                .iter()
                .any(|command| matches!(command, PaintCommand::Border { .. }))
        );
        assert!(!has_visible_border(&Border::new(0.0, Color::TRANSPARENT)));
        assert!(has_visible_border(&Border::new(1.0, Color::WHITE)));
    }
}
