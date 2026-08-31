use std::rc::Rc;

use super::common::finite_non_negative;
use incular_config::{CrossAxisAlignment, EdgeInsets, MainAxisSize};
use incular_controls::{ControlIcon, ControlTheme, current_control_theme};
use incular_core::{Color, Offset};
use incular_semantics::SemanticAction;
use incular_text::TextStyle;
use incular_widgets::internal::ActionSurface;
use incular_widgets::{
    Border, BorderRadius, BoxDecoration, Container, Row, Semantics, SizedBox, Text, Widget,
};
use typed_builder::TypedBuilder;

/// The configurable Material chip implementation.
///
/// Flutter's public chip family shares one retained visual anatomy while
/// varying which callbacks and selection affordances are enabled.  `RawChip`
/// is that common implementation.  The named chip types below are small
/// typed wrappers that expose the corresponding Flutter-shaped constructors
/// without duplicating the retained rendering and hit target.
#[derive(Clone, TypedBuilder)]
pub struct RawChip {
    #[builder(setter(into))]
    label: String,
    #[builder(default, setter(strip_option, into))]
    avatar: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    delete_icon: Option<Widget>,
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
    on_pressed: Option<Rc<dyn Fn() + 'static>>,
    #[builder(
        default,
        setter(
            fn transform<F>(callback: F) -> Option<Rc<dyn Fn(bool) + 'static>>
            where
                F: Fn(bool) + 'static,
            {
                Some(Rc::new(callback))
            }
        )
    )]
    on_selected: Option<Rc<dyn Fn(bool) + 'static>>,
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
    on_deleted: Option<Rc<dyn Fn() + 'static>>,
    #[builder(default)]
    selected: bool,
    #[builder(default = true)]
    enabled: bool,
    #[builder(default)]
    show_checkmark: bool,
    #[builder(default, setter(strip_option))]
    color: Option<Color>,
    #[builder(default, setter(strip_option))]
    selected_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    disabled_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    label_style: Option<TextStyle>,
    #[builder(default = EdgeInsets::symmetric(12.0, 6.0))]
    padding: EdgeInsets,
    #[builder(default = 0.0, setter(transform = |value: f32| finite_non_negative(value)))]
    elevation: f32,
    #[builder(default, setter(strip_option, into))]
    semantic_label: Option<String>,
}

impl Default for RawChip {
    fn default() -> Self {
        Self::new("")
    }
}

impl RawChip {
    /// Creates a non-selected, enabled chip with the Material default padding.
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            avatar: None,
            delete_icon: None,
            on_pressed: None,
            on_selected: None,
            on_deleted: None,
            selected: false,
            enabled: true,
            show_checkmark: false,
            color: None,
            selected_color: None,
            disabled_color: None,
            label_style: None,
            padding: EdgeInsets::symmetric(12.0, 6.0),
            elevation: 0.0,
            semantic_label: None,
        }
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    fn set_label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    #[must_use]
    pub fn is_selected(&self) -> bool {
        self.selected
    }

    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    #[must_use]
    pub fn elevation_value(&self) -> f32 {
        self.elevation
    }

    #[must_use]
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    #[must_use]
    pub fn avatar(mut self, avatar: impl Into<Widget>) -> Self {
        self.avatar = Some(avatar.into());
        self
    }

    #[must_use]
    pub fn delete_icon(mut self, delete_icon: impl Into<Widget>) -> Self {
        self.delete_icon = Some(delete_icon.into());
        self
    }

    #[must_use]
    pub fn on_pressed(mut self, callback: impl Fn() + 'static) -> Self {
        self.on_pressed = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn on_selected(mut self, callback: impl Fn(bool) + 'static) -> Self {
        self.on_selected = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn on_deleted(mut self, callback: impl Fn() + 'static) -> Self {
        self.on_deleted = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn show_checkmark(mut self, show_checkmark: bool) -> Self {
        self.show_checkmark = show_checkmark;
        self
    }

    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    #[must_use]
    pub fn selected_color(mut self, color: Color) -> Self {
        self.selected_color = Some(color);
        self
    }

    #[must_use]
    pub fn disabled_color(mut self, color: Color) -> Self {
        self.disabled_color = Some(color);
        self
    }

    #[must_use]
    pub fn label_style(mut self, style: TextStyle) -> Self {
        self.label_style = Some(style);
        self
    }

    #[must_use]
    pub fn padding(mut self, padding: EdgeInsets) -> Self {
        self.padding = padding;
        self
    }

    #[must_use]
    pub fn elevation(mut self, elevation: f32) -> Self {
        self.elevation = finite_non_negative(elevation);
        self
    }

    #[must_use]
    pub fn semantic_label(mut self, label: impl Into<String>) -> Self {
        self.semantic_label = Some(label.into());
        self
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        let background = if !self.enabled {
            self.disabled_color.unwrap_or(theme.colors.disabled_surface)
        } else if self.selected {
            self.selected_color.unwrap_or(theme.colors.accent)
        } else {
            self.color.unwrap_or(theme.colors.surface_variant)
        };
        let foreground = if !self.enabled {
            theme.colors.disabled_foreground
        } else if self.selected {
            theme.colors.accent_foreground
        } else {
            theme.colors.foreground
        };

        let mut children = Vec::with_capacity(5);
        if self.show_checkmark && self.selected {
            children.push(ControlIcon::Check.widget(14.0, foreground));
        }
        if let Some(avatar) = self.avatar.clone() {
            children.push(avatar);
        }
        let label_style = self
            .label_style
            .clone()
            .unwrap_or_else(|| theme.typography.body.clone())
            .color(foreground);
        children.push(Text::new(self.label.clone()).style(label_style).into());

        if self.delete_icon.is_some() || self.on_deleted.is_some() {
            children.push(SizedBox::new().width(4.0).into());
            let delete_icon = self.delete_icon.clone().unwrap_or_else(|| {
                ControlIcon::Close
                    .widget(14.0, foreground)
                    .exclude_semantics()
            });
            let delete: Widget = if let Some(callback) = self.on_deleted.clone() {
                let mut action = ActionSurface::with_child(delete_icon)
                    .color(Color::TRANSPARENT)
                    .hover_color(Color::TRANSPARENT)
                    .pressed_color(Color::TRANSPARENT)
                    .disabled_color(Color::TRANSPARENT)
                    .enabled(self.enabled);
                if self.enabled {
                    action = action.on_click(move || callback());
                }
                action.into()
            } else {
                delete_icon
            };
            children.push(delete);
        }

        let content: Widget = Row::new(children)
            .main_axis_size(MainAxisSize::Min)
            .alignment(CrossAxisAlignment::Center)
            .spacing(6.0)
            .into();
        let decorated: Widget = Container::new()
            .padding(self.padding)
            .decoration(
                BoxDecoration::new()
                    .color(background)
                    .border(Border::new(1.0, theme.colors.border_subtle))
                    .border_radius(BorderRadius::circular(999.0)),
            )
            .child(content)
            .into();
        let decorated = if self.elevation > 0.0 {
            Widget::from(incular_widgets::internal::DropShadow::new(
                Offset::new(0.0, (self.elevation * 0.2).min(8.0)),
                (self.elevation * 0.45).clamp(1.0, 16.0),
                Color::rgba(0, 0, 0, 64),
                decorated,
            ))
        } else {
            decorated
        };

        let mut surface = ActionSurface::with_child(decorated)
            .color(Color::TRANSPARENT)
            .hover_color(Color::TRANSPARENT)
            .pressed_color(Color::TRANSPARENT)
            .focused_color(Color::TRANSPARENT)
            .disabled_color(Color::TRANSPARENT)
            .enabled(self.enabled);
        if self.enabled {
            if let Some(callback) = self.on_pressed.clone() {
                surface = surface.on_click(move || callback());
            } else if let Some(callback) = self.on_selected.clone() {
                let next = !self.selected;
                surface = surface.on_click(move || callback(next));
            }
        }
        let raw: Widget = surface.into();
        let has_action = self.enabled
            && (self.on_pressed.is_some()
                || self.on_selected.is_some()
                || self.on_deleted.is_some());
        let mut semantics = Semantics::new(raw).button(has_action).enabled(self.enabled);
        if has_action {
            semantics = semantics.action(SemanticAction::Activate);
        }
        semantics
            .label(
                self.semantic_label
                    .clone()
                    .unwrap_or_else(|| self.label.clone()),
            )
            .into()
    }
}

impl From<RawChip> for Widget {
    fn from(value: RawChip) -> Self {
        let value = Rc::new(value);
        Widget::from(incular_widgets::LayoutBuilder::new(move |context, _| {
            value.build(&current_control_theme(context))
        }))
    }
}

/// Flutter's base `Chip` vocabulary is the unconstrained raw chip anatomy.
pub type Chip = RawChip;

/// A tappable chip with no built-in selection semantics.
#[derive(Clone, TypedBuilder)]
#[builder(mutators(
    pub fn label(self, label: impl Into<String>) {
        self.raw = self.raw.clone().set_label(label);
    }
    pub fn on_pressed<F>(self, callback: F)
    where
        F: Fn() + 'static,
    {
        self.raw = self.raw.clone().on_pressed(callback);
    }
    pub fn avatar(self, avatar: impl Into<Widget>) {
        self.raw = self.raw.clone().avatar(avatar);
    }
    pub fn enabled(self, enabled: bool) {
        self.raw = self.raw.clone().enabled(enabled);
    }
    pub fn color(self, color: Color) {
        self.raw = self.raw.clone().color(color);
    }
    pub fn elevation(self, elevation: f32) {
        self.raw = self.raw.clone().elevation(elevation);
    }
    pub fn padding(self, padding: EdgeInsets) {
        self.raw = self.raw.clone().padding(padding);
    }
    pub fn semantic_label(self, label: impl Into<String>) {
        self.raw = self.raw.clone().semantic_label(label);
    }
))]
pub struct ActionChip {
    #[builder(via_mutators = RawChip::default())]
    raw: RawChip,
}

impl Default for ActionChip {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl ActionChip {
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            raw: RawChip::new(label),
        }
    }

    #[must_use]
    pub fn on_pressed(mut self, callback: impl Fn() + 'static) -> Self {
        self.raw = self.raw.clone().on_pressed(callback);
        self
    }

    #[must_use]
    pub fn avatar(mut self, avatar: impl Into<Widget>) -> Self {
        self.raw = self.raw.clone().avatar(avatar);
        self
    }

    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.raw = self.raw.clone().enabled(enabled);
        self
    }

    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.raw = self.raw.clone().color(color);
        self
    }

    #[must_use]
    pub fn elevation(mut self, elevation: f32) -> Self {
        self.raw = self.raw.clone().elevation(elevation);
        self
    }

    #[must_use]
    pub fn padding(mut self, padding: EdgeInsets) -> Self {
        self.raw = self.raw.clone().padding(padding);
        self
    }

    #[must_use]
    pub fn semantic_label(mut self, label: impl Into<String>) -> Self {
        self.raw = self.raw.clone().semantic_label(label);
        self
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        self.raw.build(theme)
    }
}

impl From<ActionChip> for Widget {
    fn from(value: ActionChip) -> Self {
        value.raw.into()
    }
}

/// A selectable chip for one choice in a mutually exclusive set.
#[derive(Clone, TypedBuilder)]
#[builder(mutators(
    pub fn label(self, label: impl Into<String>) {
        self.raw = self.raw.clone().set_label(label);
    }
    pub fn selected(self, selected: bool) {
        self.raw = self.raw.clone().selected(selected);
    }
    pub fn on_selected<F>(self, callback: F)
    where
        F: Fn(bool) + 'static,
    {
        self.raw = self.raw.clone().on_selected(callback);
    }
    pub fn avatar(self, avatar: impl Into<Widget>) {
        self.raw = self.raw.clone().avatar(avatar);
    }
    pub fn enabled(self, enabled: bool) {
        self.raw = self.raw.clone().enabled(enabled);
    }
    pub fn selected_color(self, color: Color) {
        self.raw = self.raw.clone().selected_color(color);
    }
    pub fn show_checkmark(self, show_checkmark: bool) {
        self.raw = self.raw.clone().show_checkmark(show_checkmark);
    }
    pub fn semantic_label(self, label: impl Into<String>) {
        self.raw = self.raw.clone().semantic_label(label);
    }
))]
pub struct ChoiceChip {
    #[builder(via_mutators = RawChip::default().show_checkmark(true))]
    raw: RawChip,
}

impl Default for ChoiceChip {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl ChoiceChip {
    #[must_use]
    pub fn new(label: impl Into<String>, selected: bool) -> Self {
        Self {
            raw: RawChip::new(label).selected(selected).show_checkmark(true),
        }
    }

    #[must_use]
    pub fn selected(mut self, selected: bool) -> Self {
        self.raw = self.raw.clone().selected(selected);
        self
    }

    #[must_use]
    pub fn on_selected(mut self, callback: impl Fn(bool) + 'static) -> Self {
        self.raw = self.raw.clone().on_selected(callback);
        self
    }

    #[must_use]
    pub fn avatar(mut self, avatar: impl Into<Widget>) -> Self {
        self.raw = self.raw.clone().avatar(avatar);
        self
    }

    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.raw = self.raw.clone().enabled(enabled);
        self
    }

    #[must_use]
    pub fn selected_color(mut self, color: Color) -> Self {
        self.raw = self.raw.clone().selected_color(color);
        self
    }

    #[must_use]
    pub fn show_checkmark(mut self, show_checkmark: bool) -> Self {
        self.raw = self.raw.clone().show_checkmark(show_checkmark);
        self
    }

    #[must_use]
    pub fn semantic_label(mut self, label: impl Into<String>) -> Self {
        self.raw = self.raw.clone().semantic_label(label);
        self
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        self.raw.build(theme)
    }
}

impl From<ChoiceChip> for Widget {
    fn from(value: ChoiceChip) -> Self {
        value.raw.into()
    }
}

/// A selectable chip suitable for filtering, optionally with deletion.
#[derive(Clone, TypedBuilder)]
#[builder(mutators(
    pub fn label(self, label: impl Into<String>) {
        self.raw = self.raw.clone().set_label(label);
    }
    pub fn selected(self, selected: bool) {
        self.raw = self.raw.clone().selected(selected);
    }
    pub fn on_selected<F>(self, callback: F)
    where
        F: Fn(bool) + 'static,
    {
        self.raw = self.raw.clone().on_selected(callback);
    }
    pub fn on_deleted<F>(self, callback: F)
    where
        F: Fn() + 'static,
    {
        self.raw = self.raw.clone().on_deleted(callback);
    }
    pub fn delete_icon(self, icon: impl Into<Widget>) {
        self.raw = self.raw.clone().delete_icon(icon);
    }
    pub fn enabled(self, enabled: bool) {
        self.raw = self.raw.clone().enabled(enabled);
    }
    pub fn selected_color(self, color: Color) {
        self.raw = self.raw.clone().selected_color(color);
    }
    pub fn semantic_label(self, label: impl Into<String>) {
        self.raw = self.raw.clone().semantic_label(label);
    }
))]
pub struct FilterChip {
    #[builder(via_mutators = RawChip::default().show_checkmark(true))]
    raw: RawChip,
}

impl Default for FilterChip {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl FilterChip {
    #[must_use]
    pub fn new(label: impl Into<String>, selected: bool) -> Self {
        Self {
            raw: RawChip::new(label).selected(selected).show_checkmark(true),
        }
    }

    #[must_use]
    pub fn selected(mut self, selected: bool) -> Self {
        self.raw = self.raw.clone().selected(selected);
        self
    }

    #[must_use]
    pub fn on_selected(mut self, callback: impl Fn(bool) + 'static) -> Self {
        self.raw = self.raw.clone().on_selected(callback);
        self
    }

    #[must_use]
    pub fn on_deleted(mut self, callback: impl Fn() + 'static) -> Self {
        self.raw = self.raw.clone().on_deleted(callback);
        self
    }

    #[must_use]
    pub fn delete_icon(mut self, icon: impl Into<Widget>) -> Self {
        self.raw = self.raw.clone().delete_icon(icon);
        self
    }

    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.raw = self.raw.clone().enabled(enabled);
        self
    }

    #[must_use]
    pub fn selected_color(mut self, color: Color) -> Self {
        self.raw = self.raw.clone().selected_color(color);
        self
    }

    #[must_use]
    pub fn semantic_label(mut self, label: impl Into<String>) -> Self {
        self.raw = self.raw.clone().semantic_label(label);
        self
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        self.raw.build(theme)
    }
}

impl From<FilterChip> for Widget {
    fn from(value: FilterChip) -> Self {
        value.raw.into()
    }
}

/// A chip representing an input token, with optional selection and deletion.
#[derive(Clone, TypedBuilder)]
#[builder(mutators(
    pub fn label(self, label: impl Into<String>) {
        self.raw = self.raw.clone().set_label(label);
    }
    pub fn selected(self, selected: bool) {
        self.raw = self.raw.clone().selected(selected);
    }
    pub fn on_pressed<F>(self, callback: F)
    where
        F: Fn() + 'static,
    {
        self.raw = self.raw.clone().on_pressed(callback);
    }
    pub fn on_selected<F>(self, callback: F)
    where
        F: Fn(bool) + 'static,
    {
        self.raw = self.raw.clone().on_selected(callback);
    }
    pub fn on_deleted<F>(self, callback: F)
    where
        F: Fn() + 'static,
    {
        self.raw = self.raw.clone().on_deleted(callback);
    }
    pub fn avatar(self, avatar: impl Into<Widget>) {
        self.raw = self.raw.clone().avatar(avatar);
    }
    pub fn delete_icon(self, icon: impl Into<Widget>) {
        self.raw = self.raw.clone().delete_icon(icon);
    }
    pub fn enabled(self, enabled: bool) {
        self.raw = self.raw.clone().enabled(enabled);
    }
    pub fn semantic_label(self, label: impl Into<String>) {
        self.raw = self.raw.clone().semantic_label(label);
    }
))]
pub struct InputChip {
    #[builder(via_mutators = RawChip::default())]
    raw: RawChip,
}

impl Default for InputChip {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl InputChip {
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            raw: RawChip::new(label),
        }
    }

    #[must_use]
    pub fn selected(mut self, selected: bool) -> Self {
        self.raw = self.raw.clone().selected(selected);
        self
    }

    #[must_use]
    pub fn on_pressed(mut self, callback: impl Fn() + 'static) -> Self {
        self.raw = self.raw.clone().on_pressed(callback);
        self
    }

    #[must_use]
    pub fn on_selected(mut self, callback: impl Fn(bool) + 'static) -> Self {
        self.raw = self.raw.clone().on_selected(callback);
        self
    }

    #[must_use]
    pub fn on_deleted(mut self, callback: impl Fn() + 'static) -> Self {
        self.raw = self.raw.clone().on_deleted(callback);
        self
    }

    #[must_use]
    pub fn avatar(mut self, avatar: impl Into<Widget>) -> Self {
        self.raw = self.raw.clone().avatar(avatar);
        self
    }

    #[must_use]
    pub fn delete_icon(mut self, icon: impl Into<Widget>) -> Self {
        self.raw = self.raw.clone().delete_icon(icon);
        self
    }

    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.raw = self.raw.clone().enabled(enabled);
        self
    }

    #[must_use]
    pub fn semantic_label(mut self, label: impl Into<String>) -> Self {
        self.raw = self.raw.clone().semantic_label(label);
        self
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        self.raw.build(theme)
    }
}

impl From<InputChip> for Widget {
    fn from(value: InputChip) -> Self {
        value.raw.into()
    }
}
