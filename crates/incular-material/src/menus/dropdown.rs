use std::cell::Cell;
use std::rc::Rc;

use super::{
    controls::MenuItemButton,
    popup::{PopupMenuButton, PopupMenuItem, menu_panel},
    style::MenuStyle,
};
use crate::foundation::{InputDecoration, InputDecorationThemeData};
use crate::{DropdownMenuCloseBehavior, TextField};
use incular_config::{Alignment, CrossAxisAlignment, EdgeInsets};
use incular_controls::{ButtonStyle, ControlIcon, current_control_theme};
use incular_core::{Color, Size};
use incular_text::TextEditingController;
use incular_widgets::internal::ActionSurface;
use incular_widgets::{Column, Container, Row, SizedBox, Text, Widget};
use typed_builder::TypedBuilder;

type DropdownValidator<T> = Rc<dyn Fn(Option<&T>) -> Option<String> + 'static>;
type DropdownSaved<T> = Rc<dyn Fn(Option<&T>) + 'static>;

/// A typed item used by `DropdownButton` and `DropdownButtonFormField`.
pub type DropdownMenuItem<T> = PopupMenuItem<T>;

/// Builder callback vocabulary retained for source-compatible dropdown code.
/// The retained implementation accepts ordinary widgets and does not require
/// a separate platform menu renderer.
pub type DropdownButtonBuilder = Rc<dyn Fn(Widget) -> Widget + 'static>;
pub type DropdownMenuDecorationBuilder<T> =
    Rc<dyn Fn(&DropdownMenuEntry<T>) -> InputDecoration + 'static>;
pub type FilterCallback<T> = Rc<dyn Fn(&str, &DropdownMenuEntry<T>) -> bool + 'static>;
pub type SearchCallback<T> = Rc<dyn Fn(&str, &[DropdownMenuEntry<T>]) -> Option<usize> + 'static>;

/// Material wrapper that suppresses the legacy dropdown underline.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct DropdownButtonHideUnderline {
    #[builder(setter(into))]
    child: Widget,
}

impl DropdownButtonHideUnderline {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self::builder().child(child).build()
    }

    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = child.into();
        self
    }
}

impl From<DropdownButtonHideUnderline> for Widget {
    fn from(value: DropdownButtonHideUnderline) -> Self {
        value.child
    }
}

/// The original Material dropdown button API.
#[derive(Clone, TypedBuilder)]
#[builder(builder_type(name = DropdownButtonTypedBuilder))]
pub struct DropdownButton<T> {
    #[builder(
        setter(transform = |items: impl IntoIterator<Item = DropdownMenuItem<T>>| {
            items.into_iter().collect::<Vec<DropdownMenuItem<T>>>()
        })
    )]
    items: Vec<DropdownMenuItem<T>>,
    #[builder(default, setter(strip_option))]
    value: Option<T>,
    #[builder(default, setter(strip_option, into))]
    hint: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    disabled_hint: Option<Widget>,
    #[builder(
        default,
        setter(
            fn transform<F>(callback: F) -> Option<Rc<dyn Fn(Option<T>) + 'static>>
            where
                F: Fn(Option<T>) + 'static,
            {
                Some(Rc::new(callback))
            }
        )
    )]
    on_changed: Option<Rc<dyn Fn(Option<T>) + 'static>>,
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
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    icon: Option<Widget>,
    #[builder(default)]
    is_dense: bool,
    #[builder(default)]
    is_expanded: bool,
    #[builder(default = Some(48.0), setter(transform = |value: f32| Some(value.max(48.0))))]
    item_height: Option<f32>,
    #[builder(default, setter(transform = |value: f32| Some(value.max(0.0))))]
    menu_width: Option<f32>,
    #[builder(default, setter(transform = |value: f32| Some(value.max(0.0))))]
    menu_max_height: Option<f32>,
    #[builder(default, setter(strip_option))]
    dropdown_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    style: Option<ButtonStyle>,
    #[builder(default, setter(strip_option))]
    menu_style: Option<MenuStyle>,
    #[builder(default, setter(strip_option))]
    padding: Option<EdgeInsets>,
    #[builder(default = Alignment::TOP_LEFT)]
    alignment: Alignment,
    #[builder(default)]
    autofocus: bool,
    #[builder(default = true)]
    barrier_dismissible: bool,
}

impl<T> DropdownButton<T> {
    #[must_use]
    pub fn new(items: impl IntoIterator<Item = DropdownMenuItem<T>>) -> Self {
        Self::builder().items(items).build()
    }

    #[must_use]
    pub fn value(mut self, value: T) -> Self {
        self.value = Some(value);
        self
    }

    #[must_use]
    pub fn initial_value(self, value: T) -> Self {
        self.value(value)
    }

    #[must_use]
    pub fn hint(mut self, value: impl Into<Widget>) -> Self {
        self.hint = Some(value.into());
        self
    }

    #[must_use]
    pub fn disabled_hint(mut self, value: impl Into<Widget>) -> Self {
        self.disabled_hint = Some(value.into());
        self
    }

    #[must_use]
    pub fn on_changed(mut self, value: impl Fn(Option<T>) + 'static) -> Self {
        self.on_changed = Some(Rc::new(value));
        self
    }

    #[must_use]
    pub fn on_tap(mut self, value: impl Fn() + 'static) -> Self {
        self.on_tap = Some(Rc::new(value));
        self
    }

    #[must_use]
    pub fn child(mut self, value: impl Into<Widget>) -> Self {
        self.child = Some(value.into());
        self
    }

    #[must_use]
    pub fn icon(mut self, value: impl Into<Widget>) -> Self {
        self.icon = Some(value.into());
        self
    }

    #[must_use]
    pub fn is_dense(mut self, value: bool) -> Self {
        self.is_dense = value;
        self
    }

    #[must_use]
    pub fn is_expanded(mut self, value: bool) -> Self {
        self.is_expanded = value;
        self
    }

    #[must_use]
    pub fn item_height(mut self, value: Option<f32>) -> Self {
        self.item_height = value.map(|value| value.max(48.0));
        self
    }

    #[must_use]
    pub fn menu_width(mut self, value: f32) -> Self {
        self.menu_width = Some(value.max(0.0));
        self
    }

    #[must_use]
    pub fn menu_max_height(mut self, value: f32) -> Self {
        self.menu_max_height = Some(value.max(0.0));
        self
    }

    #[must_use]
    pub fn dropdown_color(mut self, value: Color) -> Self {
        self.dropdown_color = Some(value);
        self
    }

    #[must_use]
    pub fn style(mut self, value: ButtonStyle) -> Self {
        self.style = Some(value);
        self
    }

    #[must_use]
    pub fn menu_style(mut self, value: MenuStyle) -> Self {
        self.menu_style = Some(value);
        self
    }

    #[must_use]
    pub fn padding(mut self, value: EdgeInsets) -> Self {
        self.padding = Some(value);
        self
    }

    #[must_use]
    pub fn alignment(mut self, value: Alignment) -> Self {
        self.alignment = value;
        self
    }

    #[must_use]
    pub fn autofocus(mut self, value: bool) -> Self {
        self.autofocus = value;
        self
    }

    #[must_use]
    pub fn barrier_dismissible(mut self, value: bool) -> Self {
        self.barrier_dismissible = value;
        self
    }
}

impl<T: Clone + PartialEq + 'static> From<DropdownButton<T>> for Widget {
    fn from(value: DropdownButton<T>) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |context, _| {
            let value = value.as_ref().clone();
            let enabled = value.on_changed.is_some() && !value.items.is_empty();
            let selected = value.value.as_ref().and_then(|selected| {
                value
                    .items
                    .iter()
                    .find(|item| item.item_value() == Some(selected))
            });
            let selected_widget = selected
                .map(|item| item.item_child().clone())
                .or_else(|| value.hint.clone())
                .or_else(|| value.disabled_hint.clone())
                .unwrap_or_else(|| Text::new("Select").into());
            let mut selected_widget =
                Container::with_child(selected_widget).alignment(value.alignment);
            if value.is_expanded {
                // An expanded dropdown consumes the width offered by its parent;
                // the menu itself keeps its independent width/height constraints.
                selected_widget = selected_widget.width(f32::INFINITY);
            }
            let mut content =
                Row::new([selected_widget]).cross_axis_alignment(CrossAxisAlignment::Center);
            if let Some(icon) = value.icon.clone() {
                content = Row::new([content.into(), icon]).spacing(8.0);
            } else {
                content = Row::new([
                    content.into(),
                    ControlIcon::ChevronDown
                        .widget(20.0, current_control_theme(context).colors.foreground),
                ])
                .spacing(8.0);
            }
            let mut popup = PopupMenuButton::from_items(value.items.clone())
                .child(content)
                .enabled(enabled)
                .barrier_dismissible(value.barrier_dismissible)
                .on_tap({
                    let callback = value.on_tap.clone();
                    move || {
                        if let Some(callback) = callback.as_ref() {
                            callback();
                        }
                    }
                });
            if let Some(callback) = value.on_changed.clone() {
                popup = popup.on_selected(move |selected| callback(Some(selected)));
            }
            if let Some(style) = value.style {
                popup = popup.style(style);
            }
            if let Some(padding) = value.padding {
                popup = popup.padding(padding);
            }
            if let Some(color) = value.dropdown_color {
                popup = popup.color(color);
            }
            let mut menu_style = value.menu_style.unwrap_or_default();
            if let Some(width) = value.menu_width {
                menu_style = menu_style.fixed_size(Size::new(width, f32::INFINITY));
            }
            if let Some(max_height) = value.menu_max_height {
                let current = menu_style
                    .maximum_size
                    .unwrap_or(Size::new(f32::INFINITY, f32::INFINITY));
                menu_style = menu_style.maximum_size(Size::new(current.width, max_height));
            }
            if let Some(height) = value.item_height {
                let mut item_style = ButtonStyle::new().height(height);
                if value.is_dense {
                    item_style = item_style.padding(EdgeInsets::symmetric(8.0, 2.0));
                }
                popup = popup.menu_style(menu_style).style(item_style);
            } else {
                popup = popup.menu_style(menu_style);
            }
            let popup: Widget = popup.into();
            if value.autofocus {
                incular_widgets::Focus::new(popup).autofocus(true).into()
            } else {
                popup
            }
        })
    }
}

/// Form-aware wrapper around `DropdownButton`.
#[derive(Clone, TypedBuilder)]
pub struct DropdownButtonFormField<T> {
    #[builder(setter(into))]
    dropdown: DropdownButton<T>,
    #[builder(default, setter(strip_option))]
    decoration: Option<InputDecoration>,
    #[builder(default, setter(strip_option, into))]
    error_text: Option<String>,
    #[builder(
        default,
        setter(
            fn transform<F>(callback: F) -> Option<DropdownValidator<T>>
            where
                F: Fn(Option<&T>) -> Option<String> + 'static,
            {
                Some(Rc::new(callback))
            }
        )
    )]
    validator: Option<DropdownValidator<T>>,
    #[builder(
        default,
        setter(
            fn transform<F>(callback: F) -> Option<DropdownSaved<T>>
            where
                F: Fn(Option<&T>) + 'static,
            {
                Some(Rc::new(callback))
            }
        )
    )]
    on_saved: Option<DropdownSaved<T>>,
}

impl<T> DropdownButtonFormField<T> {
    #[must_use]
    pub fn new(items: impl IntoIterator<Item = DropdownMenuItem<T>>) -> Self {
        Self::builder().dropdown(DropdownButton::new(items)).build()
    }

    #[must_use]
    pub fn value(mut self, value: T) -> Self {
        self.dropdown = self.dropdown.value(value);
        self
    }

    #[must_use]
    pub fn initial_value(self, value: T) -> Self {
        self.value(value)
    }

    #[must_use]
    pub fn on_changed(mut self, value: impl Fn(Option<T>) + 'static) -> Self {
        self.dropdown = self.dropdown.on_changed(value);
        self
    }

    #[must_use]
    pub fn decoration(mut self, value: InputDecoration) -> Self {
        self.decoration = Some(value);
        self
    }

    #[must_use]
    pub fn error_text(mut self, value: impl Into<String>) -> Self {
        self.error_text = Some(value.into());
        self
    }

    #[must_use]
    pub fn validator(mut self, value: impl Fn(Option<&T>) -> Option<String> + 'static) -> Self {
        self.validator = Some(Rc::new(value));
        self
    }

    #[must_use]
    pub fn on_saved(mut self, value: impl Fn(Option<&T>) + 'static) -> Self {
        self.on_saved = Some(Rc::new(value));
        self
    }
}

impl<T: Clone + PartialEq + 'static> From<DropdownButtonFormField<T>> for Widget {
    fn from(value: DropdownButtonFormField<T>) -> Self {
        let mut children = Vec::new();
        if let Some(decoration) = value.decoration.as_ref()
            && let Some(label) = decoration.label_text.clone()
        {
            children.push(Text::new(label).into());
        }
        children.push(Widget::from(value.dropdown.clone()));
        let error = value.error_text.clone().or_else(|| {
            value
                .validator
                .as_ref()
                .and_then(|validator| validator(value.dropdown.value.as_ref()))
        });
        if let Some(error) = error {
            children.push(Text::new(error).into());
        } else if let Some(decoration) = value.decoration.as_ref()
            && let Some(helper) = decoration.helper_text.clone()
        {
            children.push(Text::new(helper).into());
        }
        let _ = value.on_saved;
        Column::new(children)
            .spacing(4.0)
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .into()
    }
}

/// Form-aware naming retained for applications that use the Material 3
/// `DropdownMenu` API. Validation can be layered with the ordinary form
/// controller while selection/search behavior remains in `DropdownMenu`.
pub type DropdownMenuFormField<T> = DropdownMenu<T>;

/// A searchable/filterable Material 3 dropdown menu.
#[derive(Clone, TypedBuilder)]
pub struct DropdownMenu<T> {
    #[builder(
        setter(transform = |entries: impl IntoIterator<Item = DropdownMenuEntry<T>>| {
            entries.into_iter().collect::<Vec<DropdownMenuEntry<T>>>()
        })
    )]
    entries: Vec<DropdownMenuEntry<T>>,
    #[builder(default = TextEditingController::new())]
    controller: TextEditingController,
    #[builder(default = Rc::new(Cell::new(0)), setter(skip))]
    revision: Rc<Cell<u64>>,
    #[builder(default = Rc::new(Cell::new(false)), setter(skip))]
    open: Rc<Cell<bool>>,
    #[builder(default = Rc::new(Cell::new(None)), setter(skip))]
    selected: Rc<Cell<Option<usize>>>,
    #[builder(default = Cell::new(false), setter(skip))]
    controller_listener_attached: Cell<bool>,
    #[builder(default = true)]
    enabled: bool,
    #[builder(default, setter(transform = |value: f32| Some(value.max(0.0))))]
    width: Option<f32>,
    #[builder(default, setter(transform = |value: f32| Some(value.max(0.0))))]
    menu_height: Option<f32>,
    #[builder(default, setter(strip_option, into))]
    leading_icon: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    trailing_icon: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    selected_trailing_icon: Option<Widget>,
    #[builder(default = true)]
    show_trailing_icon: bool,
    #[builder(default, setter(strip_option, into))]
    label: Option<String>,
    #[builder(default, setter(strip_option, into))]
    hint_text: Option<String>,
    #[builder(default, setter(strip_option, into))]
    helper_text: Option<String>,
    #[builder(default, setter(strip_option, into))]
    error_text: Option<String>,
    #[builder(default)]
    enable_filter: bool,
    #[builder(default = true)]
    enable_search: bool,
    #[builder(default)]
    select_only: bool,
    #[builder(default, setter(strip_option))]
    request_focus_on_tap: Option<bool>,
    #[builder(default, setter(strip_option))]
    input_decoration_theme: Option<InputDecorationThemeData>,
    #[builder(default, setter(strip_option))]
    menu_style: Option<MenuStyle>,
    #[builder(default = DropdownMenuCloseBehavior::All)]
    close_behavior: DropdownMenuCloseBehavior,
    #[builder(
        default,
        setter(
            fn transform<F>(callback: F) -> Option<Rc<dyn Fn(Option<T>) + 'static>>
            where
                F: Fn(Option<T>) + 'static,
            {
                Some(Rc::new(callback))
            }
        )
    )]
    on_selected: Option<Rc<dyn Fn(Option<T>) + 'static>>,
}

impl<T> DropdownMenu<T> {
    #[must_use]
    pub fn new(entries: impl IntoIterator<Item = DropdownMenuEntry<T>>) -> Self {
        let controller = TextEditingController::new();
        let revision: Rc<Cell<u64>> = Rc::new(Cell::new(0));
        let observed = revision.clone();
        controller.add_listener(move |_| {
            observed.set(observed.get().wrapping_add(1));
        });
        let menu = Self {
            entries: entries.into_iter().collect(),
            controller,
            revision,
            open: Rc::new(Cell::new(false)),
            selected: Rc::new(Cell::new(None)),
            controller_listener_attached: Cell::new(false),
            enabled: true,
            width: None,
            menu_height: None,
            leading_icon: None,
            trailing_icon: None,
            selected_trailing_icon: None,
            show_trailing_icon: true,
            label: None,
            hint_text: None,
            helper_text: None,
            error_text: None,
            enable_filter: false,
            enable_search: true,
            select_only: false,
            request_focus_on_tap: None,
            input_decoration_theme: None,
            menu_style: None,
            close_behavior: DropdownMenuCloseBehavior::All,
            on_selected: None,
        };
        menu.controller_listener_attached.set(true);
        menu
    }

    #[must_use]
    pub fn controller(mut self, value: TextEditingController) -> Self {
        self.controller = value;
        self.controller_listener_attached.set(false);
        self.attach_controller_listener();
        self
    }

    fn attach_controller_listener(&self) {
        if self.controller_listener_attached.get() {
            return;
        }
        let revision = self.revision.clone();
        self.controller.add_listener(move |_| {
            revision.set(revision.get().wrapping_add(1));
        });
        self.controller_listener_attached.set(true);
    }

    #[must_use]
    pub fn initial_selection(self, value: T) -> Self
    where
        T: PartialEq,
    {
        if let Some(index) = self.entries.iter().position(|entry| entry.value == value) {
            self.selected.set(Some(index));
            self.controller.set_text(self.entries[index].label.clone());
        }
        self
    }

    #[must_use]
    pub fn enabled(mut self, value: bool) -> Self {
        self.enabled = value;
        self
    }

    #[must_use]
    pub fn width(mut self, value: f32) -> Self {
        self.width = Some(value.max(0.0));
        self
    }

    #[must_use]
    pub fn menu_height(mut self, value: f32) -> Self {
        self.menu_height = Some(value.max(0.0));
        self
    }

    #[must_use]
    pub fn leading_icon(mut self, value: impl Into<Widget>) -> Self {
        self.leading_icon = Some(value.into());
        self
    }

    #[must_use]
    pub fn trailing_icon(mut self, value: impl Into<Widget>) -> Self {
        self.trailing_icon = Some(value.into());
        self
    }

    #[must_use]
    pub fn selected_trailing_icon(mut self, value: impl Into<Widget>) -> Self {
        self.selected_trailing_icon = Some(value.into());
        self
    }

    #[must_use]
    pub fn show_trailing_icon(mut self, value: bool) -> Self {
        self.show_trailing_icon = value;
        self
    }

    #[must_use]
    pub fn label(mut self, value: impl Into<String>) -> Self {
        self.label = Some(value.into());
        self
    }

    #[must_use]
    pub fn hint_text(mut self, value: impl Into<String>) -> Self {
        self.hint_text = Some(value.into());
        self
    }

    #[must_use]
    pub fn helper_text(mut self, value: impl Into<String>) -> Self {
        self.helper_text = Some(value.into());
        self
    }

    #[must_use]
    pub fn error_text(mut self, value: impl Into<String>) -> Self {
        self.error_text = Some(value.into());
        self
    }

    #[must_use]
    pub fn enable_filter(mut self, value: bool) -> Self {
        self.enable_filter = value;
        self
    }

    #[must_use]
    pub fn enable_search(mut self, value: bool) -> Self {
        self.enable_search = value;
        self
    }

    #[must_use]
    pub fn select_only(mut self, value: bool) -> Self {
        self.select_only = value;
        self
    }

    #[must_use]
    pub fn request_focus_on_tap(mut self, value: bool) -> Self {
        self.request_focus_on_tap = Some(value);
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
    pub fn close_behavior(mut self, value: DropdownMenuCloseBehavior) -> Self {
        self.close_behavior = value;
        self
    }

    #[must_use]
    pub fn on_selected(mut self, value: impl Fn(Option<T>) + 'static) -> Self {
        self.on_selected = Some(Rc::new(value));
        self
    }

    #[must_use]
    pub fn text(&self) -> String {
        self.controller.text()
    }
}

impl<T: Clone + PartialEq + 'static> From<DropdownMenu<T>> for Widget {
    fn from(value: DropdownMenu<T>) -> Self {
        value.attach_controller_listener();
        let value = Rc::new(value);
        Widget::stateful_layout_builder(value.revision.clone(), move |context, _| {
            value.build(context)
        })
    }
}

impl<T: Clone + PartialEq + 'static> DropdownMenu<T> {
    fn build(&self, context: &incular_widgets::BuildContext<'_>) -> Widget {
        let theme = current_control_theme(context);
        let query = self.controller.text().to_lowercase();
        let visible = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                !self.enable_filter
                    || query.is_empty()
                    || entry.label.to_lowercase().contains(&query)
            })
            .collect::<Vec<_>>();
        let open = self.open.clone();
        let revision = self.revision.clone();
        let controller = self.controller.clone();
        let selected = self.selected.clone();
        let on_selected = self.on_selected.clone();
        let close_behavior = self.close_behavior;
        let item_widgets = visible
            .into_iter()
            .map(|(index, entry)| {
                let label = entry.label.clone();
                let entry_enabled = entry.enabled;
                let child = entry
                    .label_widget
                    .clone()
                    .unwrap_or_else(|| Text::new(label.clone()).into());
                let value = entry.value.clone();
                let controller = controller.clone();
                let selected = selected.clone();
                let on_selected = on_selected.clone();
                let open = open.clone();
                let revision = revision.clone();
                let mut item = MenuItemButton::new(child).enabled(entry_enabled);
                if let Some(icon) = entry.leading_icon.clone() {
                    item = item.leading_icon(icon);
                }
                if let Some(icon) = entry.trailing_icon.clone() {
                    item = item.trailing_icon(icon);
                }
                if let Some(style) = entry.style.clone() {
                    item = item.style(style);
                }
                item = item.on_pressed(move || {
                    if entry_enabled {
                        selected.set(Some(index));
                        controller.set_text(label.clone());
                        if let Some(callback) = on_selected.as_ref() {
                            callback(Some(value.clone()));
                        }
                        if !matches!(close_behavior, DropdownMenuCloseBehavior::None) {
                            open.set(false);
                            revision.set(revision.get().wrapping_add(1));
                        }
                    }
                });
                item.into()
            })
            .collect::<Vec<Widget>>();
        let mut style = self.menu_style.clone().unwrap_or_default();
        if let Some(width) = self.width {
            style = style.fixed_size(Size::new(width, f32::INFINITY));
        }
        if let Some(height) = self.menu_height {
            style = style.maximum_size(Size::new(f32::INFINITY, height));
        }
        let (panel, panel_height) = menu_panel(item_widgets, &style, &theme);
        let panel: Widget = SizedBox::new().height(panel_height).child(panel).into();

        let mut decoration = InputDecoration::new();
        if let Some(label) = self.label.clone() {
            decoration = decoration.label(label);
        }
        if let Some(hint) = self.hint_text.clone() {
            decoration = decoration.hint(hint);
        }
        if let Some(helper) = self.helper_text.clone() {
            decoration = decoration.helper(helper);
        }
        if let Some(error) = self.error_text.clone() {
            decoration = decoration.error(error);
        }
        if let Some(theme_data) = self.input_decoration_theme.as_ref() {
            decoration = decoration.merge(&InputDecoration::default());
            if let Some(filled) = theme_data.filled {
                decoration = decoration.filled(filled);
            }
            if let Some(padding) = theme_data.content_padding {
                decoration = decoration.content_padding(padding);
            }
        }
        if let Some(icon) = self.leading_icon.clone() {
            decoration = decoration.prefix_icon(icon);
        }
        let trailing = if open.get() {
            self.selected_trailing_icon
                .clone()
                .or_else(|| self.trailing_icon.clone())
                .unwrap_or_else(|| ControlIcon::ChevronUp.widget(20.0, theme.colors.foreground))
        } else {
            self.trailing_icon
                .clone()
                .unwrap_or_else(|| ControlIcon::ChevronDown.widget(20.0, theme.colors.foreground))
        };
        if self.show_trailing_icon {
            decoration = decoration.suffix_icon(trailing);
        }
        let request_focus = self
            .request_focus_on_tap
            .unwrap_or(self.enable_search && !self.select_only);
        let mut field = TextField::new(self.controller.clone())
            .input_decoration(decoration)
            // A select-only or explicitly non-searchable dropdown still uses
            // the retained TextField for decoration, but must not accept text.
            .read_only(self.select_only || !self.enable_search)
            .can_request_focus(request_focus)
            .enabled(self.enabled);
        if let Some(width) = self.width {
            field = field.size(Size::new(width, 0.0));
        }
        let field: Widget = field.into();
        let controller = open.clone();
        let revision = self.revision.clone();
        let anchor = ActionSurface::with_child(field)
            .color(Color::TRANSPARENT)
            .enabled(self.enabled)
            .on_click(move || {
                controller.set(!controller.get());
                revision.set(revision.get().wrapping_add(1));
            });
        if open.get() {
            incular_widgets::OverlayPortal::new(anchor)
                .overlay_child(panel)
                .show(true)
                .into()
        } else {
            anchor.into()
        }
    }
}

/// A declarative entry for `DropdownMenu`.
#[derive(Clone, TypedBuilder)]
pub struct DropdownMenuEntry<T> {
    #[builder(setter(into))]
    pub value: T,
    #[builder(setter(into))]
    pub label: String,
    #[builder(default, setter(strip_option, into))]
    pub label_widget: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    pub leading_icon: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    pub trailing_icon: Option<Widget>,
    #[builder(default = true)]
    pub enabled: bool,
    #[builder(default, setter(strip_option))]
    pub style: Option<ButtonStyle>,
}

impl<T> DropdownMenuEntry<T> {
    #[must_use]
    pub fn new(value: T, label: impl Into<String>) -> Self {
        Self::builder().value(value).label(label).build()
    }

    #[must_use]
    pub fn label_widget(mut self, value: impl Into<Widget>) -> Self {
        self.label_widget = Some(value.into());
        self
    }

    #[must_use]
    pub fn leading_icon(mut self, value: impl Into<Widget>) -> Self {
        self.leading_icon = Some(value.into());
        self
    }

    #[must_use]
    pub fn trailing_icon(mut self, value: impl Into<Widget>) -> Self {
        self.trailing_icon = Some(value.into());
        self
    }

    #[must_use]
    pub fn enabled(mut self, value: bool) -> Self {
        self.enabled = value;
        self
    }

    #[must_use]
    pub fn disabled(self, value: bool) -> Self {
        self.enabled(!value)
    }

    #[must_use]
    pub fn style(mut self, value: ButtonStyle) -> Self {
        self.style = Some(value);
        self
    }
}
