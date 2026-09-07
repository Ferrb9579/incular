use std::rc::Rc;

use super::common::finite_non_negative;
use crate::ListTileTitleAlignment;
use incular_config::{Clip, Constraints, CrossAxisAlignment, EdgeInsets, MainAxisSize};
use incular_controls::{
    Checkbox, CheckedState, ControlTheme, Radio, Switch, current_control_theme,
};
use incular_core::Color;
use incular_semantics::SemanticAction;
use incular_widgets::internal::{ActionSurface, Expanded};
use incular_widgets::{
    BorderRadius, BoxDecoration, Column, ConstrainedBox, Container, FocusScope, GestureDetector,
    HitTestBehavior, Positioned, Row, Semantics, SizedBox, Stack, Text, Widget,
};
use typed_builder::TypedBuilder;

/// Material badge, optionally overlaid on a child.
#[derive(Clone, TypedBuilder)]
pub struct Badge {
    #[builder(setter(into))]
    label: String,
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
    #[builder(default, setter(strip_option))]
    background_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    foreground_color: Option<Color>,
    #[builder(default = EdgeInsets::symmetric(4.0, 2.0))]
    padding: EdgeInsets,
}

impl Badge {
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            child: None,
            background_color: None,
            foreground_color: None,
            padding: EdgeInsets::symmetric(4.0, 2.0),
        }
    }

    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self
    }

    #[must_use]
    pub fn background_color(mut self, color: Color) -> Self {
        self.background_color = Some(color);
        self
    }

    #[must_use]
    pub fn foreground_color(mut self, color: Color) -> Self {
        self.foreground_color = Some(color);
        self
    }

    #[must_use]
    pub fn padding(mut self, padding: EdgeInsets) -> Self {
        self.padding = padding;
        self
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        let badge: Widget = Container::new()
            .padding(self.padding)
            .decoration(
                BoxDecoration::new()
                    .color(self.background_color.unwrap_or(theme.colors.error))
                    .border_radius(BorderRadius::circular(999.0)),
            )
            .child(
                Text::new(self.label.clone())
                    .style(theme.typography.caption.clone())
                    .color(
                        self.foreground_color
                            .unwrap_or(theme.colors.accent_foreground),
                    ),
            )
            .into();

        match self.child.clone() {
            Some(child) => Stack::new([child, Positioned::new(badge).top(0.0).right(0.0).into()])
                .clip_behavior(Clip::None)
                .into(),
            None => badge,
        }
    }
}

impl From<Badge> for Widget {
    fn from(value: Badge) -> Self {
        let value = Rc::new(value);
        Widget::from(incular_widgets::LayoutBuilder::new(move |context, _| {
            value.build(&current_control_theme(context))
        }))
    }
}

/// A Material list tile with optional leading, subtitle, and trailing slots.
#[derive(Clone, TypedBuilder)]
pub struct ListTile {
    #[builder(setter(into))]
    title: Widget,
    #[builder(default, setter(strip_option, into))]
    subtitle: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    leading: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    trailing: Option<Widget>,
    #[builder(default = true)]
    enabled: bool,
    #[builder(default)]
    selected: bool,
    #[builder(default)]
    dense: bool,
    #[builder(default, setter(strip_option))]
    content_padding: Option<EdgeInsets>,
    #[builder(default, setter(strip_option))]
    tile_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    selected_tile_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    shape: Option<BorderRadius>,
    #[builder(default, setter(transform = |value: f32| Some(finite_non_negative(value))))]
    min_tile_height: Option<f32>,
    #[builder(default, setter(transform = |value: f32| Some(finite_non_negative(value))))]
    min_leading_width: Option<f32>,
    #[builder(default, setter(transform = |value: f32| Some(finite_non_negative(value))))]
    horizontal_title_gap: Option<f32>,
    #[builder(default, setter(transform = |value: f32| Some(finite_non_negative(value))))]
    min_vertical_padding: Option<f32>,
    #[builder(default = ListTileTitleAlignment::TitleHeight)]
    title_alignment: ListTileTitleAlignment,
    #[builder(default)]
    autofocus: bool,
    #[builder(default, setter(strip_option, into))]
    semantic_label: Option<String>,
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
    on_tap: Option<Rc<dyn Fn() + 'static>>,
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
    on_long_press: Option<Rc<dyn Fn() + 'static>>,
}

impl ListTile {
    #[must_use]
    pub fn new(title: impl Into<Widget>) -> Self {
        Self {
            title: title.into(),
            subtitle: None,
            leading: None,
            trailing: None,
            enabled: true,
            selected: false,
            dense: false,
            content_padding: None,
            tile_color: None,
            selected_tile_color: None,
            shape: None,
            min_tile_height: None,
            min_leading_width: None,
            horizontal_title_gap: None,
            min_vertical_padding: None,
            title_alignment: ListTileTitleAlignment::TitleHeight,
            autofocus: false,
            semantic_label: None,
            on_tap: None,
            on_long_press: None,
        }
    }

    #[must_use]
    pub fn subtitle(mut self, subtitle: impl Into<Widget>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    #[must_use]
    pub fn leading(mut self, leading: impl Into<Widget>) -> Self {
        self.leading = Some(leading.into());
        self
    }

    #[must_use]
    pub fn trailing(mut self, trailing: impl Into<Widget>) -> Self {
        self.trailing = Some(trailing.into());
        self
    }

    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    #[must_use]
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    #[must_use]
    pub fn dense(mut self, dense: bool) -> Self {
        self.dense = dense;
        self
    }

    #[must_use]
    pub fn content_padding(mut self, content_padding: EdgeInsets) -> Self {
        self.content_padding = Some(content_padding);
        self
    }

    #[must_use]
    pub fn tile_color(mut self, color: Color) -> Self {
        self.tile_color = Some(color);
        self
    }

    #[must_use]
    pub fn selected_tile_color(mut self, color: Color) -> Self {
        self.selected_tile_color = Some(color);
        self
    }

    #[must_use]
    pub fn shape(mut self, shape: BorderRadius) -> Self {
        self.shape = Some(shape);
        self
    }

    #[must_use]
    pub fn min_tile_height(mut self, value: f32) -> Self {
        self.min_tile_height = Some(finite_non_negative(value));
        self
    }

    #[must_use]
    pub fn min_leading_width(mut self, value: f32) -> Self {
        self.min_leading_width = Some(finite_non_negative(value));
        self
    }

    #[must_use]
    pub fn horizontal_title_gap(mut self, value: f32) -> Self {
        self.horizontal_title_gap = Some(finite_non_negative(value));
        self
    }

    #[must_use]
    pub fn min_vertical_padding(mut self, value: f32) -> Self {
        self.min_vertical_padding = Some(finite_non_negative(value));
        self
    }

    #[must_use]
    pub fn title_alignment(mut self, value: ListTileTitleAlignment) -> Self {
        self.title_alignment = value;
        self
    }

    /// Requests initial keyboard focus for the enabled tile's existing action.
    #[must_use]
    pub fn autofocus(mut self, value: bool) -> Self {
        self.autofocus = value;
        self
    }

    #[must_use]
    pub fn semantic_label(mut self, label: impl Into<String>) -> Self {
        self.semantic_label = Some(label.into());
        self
    }

    #[must_use]
    pub fn on_tap(mut self, callback: impl Fn() + 'static) -> Self {
        self.on_tap = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn on_long_press(mut self, callback: impl Fn() + 'static) -> Self {
        self.on_long_press = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        let mut padding = self
            .content_padding
            .unwrap_or_else(|| EdgeInsets::symmetric(16.0, if self.dense { 8.0 } else { 12.0 }));
        if let Some(min_vertical) = self.min_vertical_padding {
            padding.top = padding.top.max(min_vertical);
            padding.bottom = padding.bottom.max(min_vertical);
        }
        let mut row = tile_row_configured(
            self.leading.clone(),
            self.title.clone(),
            self.subtitle.clone(),
            self.trailing.clone(),
            self.dense,
            padding,
            self.selected,
            self.enabled,
            theme,
            self.min_leading_width,
            self.horizontal_title_gap,
            self.title_alignment,
        );
        let tile_color = if self.selected {
            self.selected_tile_color.unwrap_or(theme.colors.selection)
        } else {
            self.tile_color.unwrap_or(Color::TRANSPARENT)
        };
        let mut tile = Container::with_child(row).color(tile_color);
        if let Some(shape) = self.shape {
            tile = tile.decoration(BoxDecoration::new().border_radius(shape));
        }
        let default_height = if self.subtitle.is_some() {
            if self.dense { 64.0 } else { 72.0 }
        } else if self.dense {
            48.0
        } else {
            56.0
        };
        let min_height = self.min_tile_height.unwrap_or(default_height);
        tile = tile.constraints(Constraints::new(
            0.0,
            f32::INFINITY,
            min_height,
            f32::INFINITY,
        ));
        row = tile.into();
        let has_action = self.on_tap.is_some();
        let mut surface = ActionSurface::with_child(row)
            .color(Color::TRANSPARENT)
            .hover_color(Color::TRANSPARENT)
            .pressed_color(Color::TRANSPARENT)
            .disabled_color(theme.colors.disabled_surface)
            .enabled(self.enabled);
        if let Some(callback) = self.on_tap.clone() {
            surface = surface.on_click(move || callback());
        }
        let raw: Widget = surface.into();
        let raw = if self.autofocus && self.enabled {
            FocusScope::new(raw).autofocus(true).into()
        } else {
            raw
        };
        let raw = if let Some(callback) = self.on_long_press.clone() {
            GestureDetector::new(raw)
                .behavior(HitTestBehavior::Opaque)
                .on_long_press(move || callback())
                .into()
        } else {
            raw
        };
        let mut semantics = Semantics::new(raw).button(has_action).enabled(self.enabled);
        if has_action {
            semantics = semantics.action(SemanticAction::Activate);
        }
        let semantic_label = self
            .semantic_label
            .clone()
            .filter(|label| !label.trim().is_empty())
            .or_else(|| self.title.semantic_text());
        match semantic_label {
            Some(label) => semantics.label(label).into(),
            None => semantics.into(),
        }
    }
}

impl From<ListTile> for Widget {
    fn from(value: ListTile) -> Self {
        let value = Rc::new(value);
        Widget::from(incular_widgets::LayoutBuilder::new(move |context, _| {
            value.build(&current_control_theme(context))
        }))
    }
}

/// A full-row checkbox tile.  The controls crate owns the retained checkbox
/// state and semantic role; this component only composes its visual row.
#[derive(Clone, TypedBuilder)]
pub struct CheckboxListTile {
    #[builder(setter(into))]
    value: bool,
    #[builder(setter(into))]
    title: Widget,
    #[builder(default, setter(strip_option, into))]
    subtitle: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    secondary: Option<Widget>,
    #[builder(default = true)]
    enabled: bool,
    #[builder(default)]
    selected: bool,
    #[builder(default)]
    dense: bool,
    #[builder(default, setter(strip_option))]
    content_padding: Option<EdgeInsets>,
    #[builder(default, setter(strip_option, into))]
    semantic_label: Option<String>,
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
    on_changed: Option<Rc<dyn Fn(bool) + 'static>>,
}

impl CheckboxListTile {
    #[must_use]
    pub fn new(value: bool, title: impl Into<Widget>) -> Self {
        Self {
            value,
            title: title.into(),
            subtitle: None,
            secondary: None,
            enabled: true,
            selected: false,
            dense: false,
            content_padding: None,
            semantic_label: None,
            on_changed: None,
        }
    }

    #[must_use]
    pub fn value(mut self, value: bool) -> Self {
        self.value = value;
        self
    }

    #[must_use]
    pub fn subtitle(mut self, subtitle: impl Into<Widget>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    #[must_use]
    pub fn secondary(mut self, secondary: impl Into<Widget>) -> Self {
        self.secondary = Some(secondary.into());
        self
    }

    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    #[must_use]
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    #[must_use]
    pub fn dense(mut self, dense: bool) -> Self {
        self.dense = dense;
        self
    }

    #[must_use]
    pub fn content_padding(mut self, content_padding: EdgeInsets) -> Self {
        self.content_padding = Some(content_padding);
        self
    }

    #[must_use]
    pub fn semantic_label(mut self, label: impl Into<String>) -> Self {
        self.semantic_label = Some(label.into());
        self
    }

    #[must_use]
    pub fn on_changed(mut self, callback: impl Fn(bool) + 'static) -> Self {
        self.on_changed = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        let indicator = Checkbox::new(self.value)
            .enabled(false)
            .build(theme)
            .exclude_semantics();
        let row = tile_row(
            Some(indicator),
            self.title.clone(),
            self.subtitle.clone(),
            self.secondary.clone(),
            self.dense,
            self.content_padding.unwrap_or_else(|| {
                EdgeInsets::symmetric(16.0, if self.dense { 8.0 } else { 12.0 })
            }),
            self.selected,
            self.enabled,
            theme,
        );
        let mut root = incular_controls::checkbox::Root::new()
            .checked(self.value)
            .enabled(self.enabled)
            .child(row);
        if let Some(callback) = self.on_changed.clone() {
            root = root.on_checked_change(move |state| {
                callback(state == CheckedState::Checked);
            });
        }
        let visual = root.build(theme);
        match self.semantic_label.clone() {
            Some(label) => Semantics::new(visual).label(label).into(),
            None => visual,
        }
    }
}

impl From<CheckboxListTile> for Widget {
    fn from(value: CheckboxListTile) -> Self {
        let value = Rc::new(value);
        Widget::from(incular_widgets::LayoutBuilder::new(move |context, _| {
            value.build(&current_control_theme(context))
        }))
    }
}

/// A full-row radio tile.  `selected` is supplied explicitly so the wrapper
/// remains usable with any group state model without introducing a separate
/// selection controller into the Material crate.
#[derive(Clone, TypedBuilder)]
pub struct RadioListTile<T: PartialEq + Clone + 'static> {
    #[builder(setter(into))]
    value: T,
    #[builder(default, setter(strip_option))]
    group_value: Option<T>,
    #[builder(setter(into))]
    title: Widget,
    #[builder(default, setter(strip_option, into))]
    subtitle: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    secondary: Option<Widget>,
    #[builder(default = true)]
    enabled: bool,
    #[builder(default = false)]
    selected: bool,
    #[builder(default)]
    dense: bool,
    #[builder(default, setter(strip_option))]
    content_padding: Option<EdgeInsets>,
    #[builder(default, setter(strip_option, into))]
    semantic_label: Option<String>,
    #[builder(
        default,
        setter(
            fn transform<F>(callback: F) -> Option<Rc<dyn Fn(T) + 'static>>
            where
                F: Fn(T) + 'static,
            {
                Some(Rc::new(callback))
            }
        )
    )]
    on_changed: Option<Rc<dyn Fn(T) + 'static>>,
}

impl<T: PartialEq + Clone + 'static> RadioListTile<T> {
    #[must_use]
    pub fn new(value: T, group_value: Option<T>, title: impl Into<Widget>) -> Self {
        let selected = group_value.as_ref() == Some(&value);
        Self {
            value,
            group_value,
            title: title.into(),
            subtitle: None,
            secondary: None,
            enabled: true,
            selected,
            dense: false,
            content_padding: None,
            semantic_label: None,
            on_changed: None,
        }
    }

    #[must_use]
    pub fn group_value(mut self, group_value: Option<T>) -> Self {
        self.selected = group_value.as_ref() == Some(&self.value);
        self.group_value = group_value;
        self
    }

    #[must_use]
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    #[must_use]
    pub fn subtitle(mut self, subtitle: impl Into<Widget>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    #[must_use]
    pub fn secondary(mut self, secondary: impl Into<Widget>) -> Self {
        self.secondary = Some(secondary.into());
        self
    }

    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    #[must_use]
    pub fn dense(mut self, dense: bool) -> Self {
        self.dense = dense;
        self
    }

    #[must_use]
    pub fn content_padding(mut self, content_padding: EdgeInsets) -> Self {
        self.content_padding = Some(content_padding);
        self
    }

    #[must_use]
    pub fn semantic_label(mut self, label: impl Into<String>) -> Self {
        self.semantic_label = Some(label.into());
        self
    }

    #[must_use]
    pub fn on_changed(mut self, callback: impl Fn(T) + 'static) -> Self {
        self.on_changed = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        let indicator = Radio::new(self.value.clone(), self.group_value.clone())
            .build(theme)
            .exclude_semantics();
        let row = tile_row(
            Some(indicator),
            self.title.clone(),
            self.subtitle.clone(),
            self.secondary.clone(),
            self.dense,
            self.content_padding.unwrap_or_else(|| {
                EdgeInsets::symmetric(16.0, if self.dense { 8.0 } else { 12.0 })
            }),
            self.selected,
            self.enabled,
            theme,
        );
        let mut surface = ActionSurface::with_child(row)
            .color(Color::TRANSPARENT)
            .hover_color(Color::TRANSPARENT)
            .pressed_color(Color::TRANSPARENT)
            .disabled_color(theme.colors.disabled_surface)
            .enabled(self.enabled);
        if let Some(callback) = self.on_changed.clone() {
            let value = self.value.clone();
            surface = surface.on_click(move || callback(value.clone()));
        }
        let raw: Widget = surface.into();
        let has_action = self.enabled && self.on_changed.is_some();
        let mut semantics = Semantics::new(raw).button(has_action).enabled(self.enabled);
        if has_action {
            semantics = semantics.action(SemanticAction::Activate);
        }
        match self.semantic_label.clone() {
            Some(label) => semantics.label(label).into(),
            None => semantics.into(),
        }
    }
}

impl<T: PartialEq + Clone + 'static> From<RadioListTile<T>> for Widget {
    fn from(value: RadioListTile<T>) -> Self {
        let value = Rc::new(value);
        Widget::from(incular_widgets::LayoutBuilder::new(move |context, _| {
            value.build(&current_control_theme(context))
        }))
    }
}

/// A full-row switch tile.  The controls crate's compound switch retains the
/// checked semantics while this wrapper supplies the Material tile anatomy.
#[derive(Clone, TypedBuilder)]
pub struct SwitchListTile {
    #[builder(setter(into))]
    value: bool,
    #[builder(setter(into))]
    title: Widget,
    #[builder(default, setter(strip_option, into))]
    subtitle: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    secondary: Option<Widget>,
    #[builder(default = true)]
    enabled: bool,
    #[builder(default)]
    selected: bool,
    #[builder(default)]
    dense: bool,
    #[builder(default, setter(strip_option))]
    content_padding: Option<EdgeInsets>,
    #[builder(default, setter(strip_option, into))]
    semantic_label: Option<String>,
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
    on_changed: Option<Rc<dyn Fn(bool) + 'static>>,
}

impl SwitchListTile {
    #[must_use]
    pub fn new(value: bool, title: impl Into<Widget>) -> Self {
        Self {
            value,
            title: title.into(),
            subtitle: None,
            secondary: None,
            enabled: true,
            selected: false,
            dense: false,
            content_padding: None,
            semantic_label: None,
            on_changed: None,
        }
    }

    #[must_use]
    pub fn value(mut self, value: bool) -> Self {
        self.value = value;
        self
    }

    #[must_use]
    pub fn subtitle(mut self, subtitle: impl Into<Widget>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    #[must_use]
    pub fn secondary(mut self, secondary: impl Into<Widget>) -> Self {
        self.secondary = Some(secondary.into());
        self
    }

    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    #[must_use]
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    #[must_use]
    pub fn dense(mut self, dense: bool) -> Self {
        self.dense = dense;
        self
    }

    #[must_use]
    pub fn content_padding(mut self, content_padding: EdgeInsets) -> Self {
        self.content_padding = Some(content_padding);
        self
    }

    #[must_use]
    pub fn semantic_label(mut self, label: impl Into<String>) -> Self {
        self.semantic_label = Some(label.into());
        self
    }

    #[must_use]
    pub fn on_changed(mut self, callback: impl Fn(bool) + 'static) -> Self {
        self.on_changed = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        let indicator = Switch::new(self.value)
            .enabled(false)
            .build(theme)
            .exclude_semantics();
        let row = tile_row(
            self.secondary.clone(),
            self.title.clone(),
            self.subtitle.clone(),
            Some(indicator),
            self.dense,
            self.content_padding.unwrap_or_else(|| {
                EdgeInsets::symmetric(16.0, if self.dense { 8.0 } else { 12.0 })
            }),
            self.selected,
            self.enabled,
            theme,
        );
        let mut root = incular_controls::switch::Root::new()
            .checked(self.value)
            .enabled(self.enabled)
            .child(row);
        if let Some(callback) = self.on_changed.clone() {
            root = root.on_checked_change(move |value| callback(value));
        }
        let visual = root.build(theme);
        match self.semantic_label.clone() {
            Some(label) => Semantics::new(visual).label(label).into(),
            None => visual,
        }
    }
}

impl From<SwitchListTile> for Widget {
    fn from(value: SwitchListTile) -> Self {
        let value = Rc::new(value);
        Widget::from(incular_widgets::LayoutBuilder::new(move |context, _| {
            value.build(&current_control_theme(context))
        }))
    }
}

#[allow(clippy::too_many_arguments)]
fn tile_row(
    leading: Option<Widget>,
    title: Widget,
    subtitle: Option<Widget>,
    trailing: Option<Widget>,
    dense: bool,
    padding: EdgeInsets,
    selected: bool,
    enabled: bool,
    theme: &ControlTheme,
) -> Widget {
    tile_row_configured(
        leading,
        title,
        subtitle,
        trailing,
        dense,
        padding,
        selected,
        enabled,
        theme,
        None,
        None,
        ListTileTitleAlignment::TitleHeight,
    )
}

#[allow(clippy::too_many_arguments)]
fn tile_row_configured(
    leading: Option<Widget>,
    title: Widget,
    subtitle: Option<Widget>,
    trailing: Option<Widget>,
    dense: bool,
    padding: EdgeInsets,
    selected: bool,
    enabled: bool,
    theme: &ControlTheme,
    min_leading_width: Option<f32>,
    horizontal_title_gap: Option<f32>,
    title_alignment: ListTileTitleAlignment,
) -> Widget {
    let title = if let Some(subtitle) = subtitle {
        Column::new([title, subtitle])
            .main_axis_size(MainAxisSize::Min)
            .alignment(CrossAxisAlignment::Start)
            .spacing(if dense { 2.0 } else { 4.0 })
            .into()
    } else {
        title
    };
    let title_alignment = match title_alignment {
        ListTileTitleAlignment::Top => CrossAxisAlignment::Start,
        ListTileTitleAlignment::Bottom => CrossAxisAlignment::End,
        ListTileTitleAlignment::Center | ListTileTitleAlignment::TitleHeight => {
            CrossAxisAlignment::Center
        }
        // A three-line tile keeps the title block vertically centered in the
        // available tile, matching the Material list's title-height policy.
        ListTileTitleAlignment::ThreeLine => CrossAxisAlignment::Center,
    };
    let title_gap = horizontal_title_gap.unwrap_or(16.0);
    let mut children = Vec::with_capacity(5);
    if let Some(leading) = leading {
        let minimum = min_leading_width.unwrap_or(56.0);
        let leading: Widget = ConstrainedBox::new(
            Constraints::new(minimum, f32::INFINITY, 0.0, f32::INFINITY),
            leading,
        )
        .into();
        children.push(leading);
        children.push(SizedBox::new().width(title_gap).into());
    }
    children.push(Expanded::new(title).into());
    if let Some(trailing) = trailing {
        children.push(SizedBox::new().width(title_gap).into());
        children.push(trailing);
    }
    let row: Widget = Row::new(children)
        .main_axis_size(MainAxisSize::Max)
        .alignment(title_alignment)
        .into();
    Container::new()
        .padding(padding)
        .color(if selected {
            theme.colors.selection
        } else if !enabled {
            theme.colors.disabled_surface
        } else {
            Color::TRANSPARENT
        })
        .child(row)
        .into()
}
