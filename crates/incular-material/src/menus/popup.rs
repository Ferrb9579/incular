use std::rc::Rc;

use super::{
    controls::{MenuController, MenuItemButton},
    style::MenuStyle,
};
use crate::foundation::Material;
use incular_config::{Clip, EdgeInsets};
use incular_controls::{
    Button as ControlButton, ButtonStyle, ButtonVariant, ControlIcon, ControlState, StateValue,
    current_control_theme,
};
use incular_core::{Color, Offset};
use incular_text::TextStyle;
use incular_widgets::{
    Border, BoxDecoration, Container, GestureDetector, HitTestBehavior, ListView, Positioned, Row,
    SizedBox, Text, TransientRole, Widget,
};
use typed_builder::TypedBuilder;

/// A popup menu entry which returns an optional typed value.
#[derive(Clone, TypedBuilder)]
pub struct PopupMenuItem<T> {
    #[builder(default, setter(strip_option))]
    value: Option<T>,
    #[builder(setter(into))]
    child: Widget,
    #[builder(default = true)]
    enabled: bool,
    #[builder(default = 48.0, setter(transform = |value: f32| value.max(0.0)))]
    height: f32,
    #[builder(default, setter(strip_option))]
    padding: Option<EdgeInsets>,
    #[builder(default, setter(strip_option))]
    text_style: Option<TextStyle>,
    #[builder(default, setter(strip_option, into))]
    label_text_style: Option<StateValue<TextStyle>>,
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
}

impl<T> PopupMenuItem<T> {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            value: None,
            child: child.into(),
            enabled: true,
            height: 48.0,
            padding: None,
            text_style: None,
            label_text_style: None,
            on_tap: None,
        }
    }

    #[must_use]
    pub fn label(value: impl Into<String>) -> Self {
        Self::new(Text::new(value.into()))
    }

    #[must_use]
    pub fn value(mut self, value: T) -> Self {
        self.value = Some(value);
        self
    }

    #[must_use]
    pub fn child(mut self, value: impl Into<Widget>) -> Self {
        self.child = value.into();
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
    pub fn height(mut self, value: f32) -> Self {
        self.height = value.max(0.0);
        self
    }

    #[must_use]
    pub fn padding(mut self, value: EdgeInsets) -> Self {
        self.padding = Some(value);
        self
    }

    #[must_use]
    pub fn text_style(mut self, value: TextStyle) -> Self {
        self.text_style = Some(value);
        self
    }

    #[must_use]
    pub fn label_text_style(mut self, value: impl Into<StateValue<TextStyle>>) -> Self {
        self.label_text_style = Some(value.into());
        self
    }

    #[must_use]
    pub fn on_tap(mut self, value: impl Fn() + 'static) -> Self {
        self.on_tap = Some(Rc::new(value));
        self
    }

    #[must_use]
    pub fn item_value(&self) -> Option<&T> {
        self.value.as_ref()
    }

    #[must_use]
    pub fn item_child(&self) -> &Widget {
        &self.child
    }

    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn build_with_selection_and_close(
        &self,
        context: &incular_widgets::BuildContext<'_>,
        on_selected: Option<Rc<dyn Fn(Option<T>) + 'static>>,
        close: Option<Rc<dyn Fn() + 'static>>,
    ) -> Widget
    where
        T: Clone + 'static,
    {
        let value = self.value.clone();
        let on_tap = self.on_tap.clone();
        let callback = move || {
            if let Some(on_tap) = on_tap.as_ref() {
                on_tap();
            }
            if let Some(on_selected) = on_selected.as_ref() {
                on_selected(value.clone());
            }
        };
        let mut item = MenuItemButton::new(self.child.clone())
            .enabled(self.enabled)
            .height(self.height)
            .on_pressed(callback);
        if let Some(padding) = self.padding {
            item = item.style(
                ButtonStyle::new()
                    .variant(ButtonVariant::Ghost)
                    .padding(padding),
            );
        }
        if let Some(style) = self.text_style.clone() {
            item = item.style(
                ButtonStyle::new()
                    .variant(ButtonVariant::Ghost)
                    .text_style(style),
            );
        }
        if let Some(style) = self.label_text_style.as_ref() {
            item = item.style(
                ButtonStyle::new()
                    .variant(ButtonVariant::Ghost)
                    .text_style(style.resolve(ControlState::empty())),
            );
        }
        item.build_with_close(context, close)
    }
}

impl<T: Clone + 'static> From<PopupMenuItem<T>> for Widget {
    fn from(value: PopupMenuItem<T>) -> Self {
        let value = Rc::new(value);
        Widget::from(incular_widgets::LayoutBuilder::new(move |context, _| {
            value.build_with_selection_and_close(context, None, None)
        }))
    }
}

/// A popup entry with an animated-compatible checkmark slot.
#[derive(Clone, TypedBuilder)]
pub struct CheckedPopupMenuItem<T> {
    #[builder(setter(into))]
    child: Widget,
    #[builder(default, setter(strip_option))]
    value: Option<T>,
    #[builder(default = true)]
    enabled: bool,
    #[builder(default = 48.0, setter(transform = |value: f32| value.max(0.0)))]
    height: f32,
    #[builder(default, setter(strip_option))]
    padding: Option<EdgeInsets>,
    #[builder(default, setter(strip_option))]
    text_style: Option<TextStyle>,
    #[builder(default, setter(strip_option, into))]
    label_text_style: Option<StateValue<TextStyle>>,
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
    #[builder(default)]
    checked: bool,
}

impl<T> CheckedPopupMenuItem<T> {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self::builder().child(child).build()
    }

    #[must_use]
    pub fn label(value: impl Into<String>) -> Self {
        Self::new(Text::new(value.into()))
    }

    #[must_use]
    pub fn checked(mut self, value: bool) -> Self {
        self.checked = value;
        self
    }

    #[must_use]
    pub fn value(mut self, value: T) -> Self {
        self.value = Some(value);
        self
    }

    #[must_use]
    pub fn enabled(mut self, value: bool) -> Self {
        self.enabled = value;
        self
    }

    #[must_use]
    pub fn on_tap(mut self, value: impl Fn() + 'static) -> Self {
        self.on_tap = Some(Rc::new(value));
        self
    }

    fn popup_item(&self, child: impl Into<Widget>) -> PopupMenuItem<T>
    where
        T: Clone,
    {
        PopupMenuItem {
            value: self.value.clone(),
            child: child.into(),
            enabled: self.enabled,
            height: self.height,
            padding: self.padding,
            text_style: self.text_style.clone(),
            label_text_style: self.label_text_style.clone(),
            on_tap: self.on_tap.clone(),
        }
    }

    fn build_with_selection_and_close(
        &self,
        context: &incular_widgets::BuildContext<'_>,
        on_selected: Option<Rc<dyn Fn(Option<T>) + 'static>>,
        close: Option<Rc<dyn Fn() + 'static>>,
    ) -> Widget
    where
        T: Clone + 'static,
    {
        let mark: Widget = if self.checked {
            ControlIcon::Check.widget(16.0, current_control_theme(context).colors.accent)
        } else {
            SizedBox::new().width(16.0).into()
        };
        let child = Row::new([mark, self.child.clone()]).spacing(8.0);
        self.popup_item(child)
            .build_with_selection_and_close(context, on_selected, close)
    }
}

impl<T: Clone + 'static> From<CheckedPopupMenuItem<T>> for Widget {
    fn from(value: CheckedPopupMenuItem<T>) -> Self {
        let value = Rc::new(value);
        Widget::from(incular_widgets::LayoutBuilder::new(move |context, _| {
            value.build_with_selection_and_close(context, None, None)
        }))
    }
}

/// A horizontal separator entry for popup menus.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct PopupMenuDivider {
    #[builder(default = 16.0, setter(transform = |value: f32| value.max(0.0)))]
    height: f32,
    #[builder(default = 0.0, setter(transform = |value: f32| value.max(0.0)))]
    indent: f32,
    #[builder(default = 0.0, setter(transform = |value: f32| value.max(0.0)))]
    end_indent: f32,
    #[builder(default, setter(strip_option))]
    color: Option<Color>,
    #[builder(default = 1.0, setter(transform = |value: f32| value.max(0.0)))]
    thickness: f32,
}

impl Default for PopupMenuDivider {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl PopupMenuDivider {
    #[must_use]
    pub fn new() -> Self {
        Self::builder().build()
    }

    #[must_use]
    pub fn height(mut self, value: f32) -> Self {
        self.height = value.max(0.0);
        self
    }

    #[must_use]
    pub fn indent(mut self, value: f32) -> Self {
        self.indent = value.max(0.0);
        self
    }

    #[must_use]
    pub fn end_indent(mut self, value: f32) -> Self {
        self.end_indent = value.max(0.0);
        self
    }

    #[must_use]
    pub fn color(mut self, value: Color) -> Self {
        self.color = Some(value);
        self
    }

    #[must_use]
    pub fn thickness(mut self, value: f32) -> Self {
        self.thickness = value.max(0.0);
        self
    }
}

impl From<PopupMenuDivider> for Widget {
    fn from(value: PopupMenuDivider) -> Self {
        Container::new()
            .height(value.height)
            .margin(EdgeInsets::only(value.indent, 0.0, value.end_indent, 0.0))
            .color(value.color.unwrap_or(Color::rgba(128, 128, 128, 90)))
            .child(SizedBox::new().height(value.thickness))
            .into()
    }
}

/// A typed entry accepted by `PopupMenuButton`.
///
/// Keeping items and separators in one enum mirrors Flutter's
/// `PopupMenuEntry<T>` while retaining an ordinary Rust `Vec` API.  The
/// `From` implementations make the common `vec![item.into(), divider.into()]`
/// form concise.
#[derive(Clone)]
pub enum PopupMenuEntry<T> {
    Item(PopupMenuItem<T>),
    Checked(CheckedPopupMenuItem<T>),
    Divider(PopupMenuDivider),
}

impl<T> PopupMenuEntry<T> {
    #[must_use]
    pub fn item(value: PopupMenuItem<T>) -> Self {
        Self::Item(value)
    }

    #[must_use]
    pub fn checked(value: CheckedPopupMenuItem<T>) -> Self {
        Self::Checked(value)
    }

    #[must_use]
    pub fn divider() -> Self {
        Self::Divider(PopupMenuDivider::new())
    }
}

impl<T> From<PopupMenuItem<T>> for PopupMenuEntry<T> {
    fn from(value: PopupMenuItem<T>) -> Self {
        Self::Item(value)
    }
}

impl<T> From<CheckedPopupMenuItem<T>> for PopupMenuEntry<T> {
    fn from(value: CheckedPopupMenuItem<T>) -> Self {
        Self::Checked(value)
    }
}

impl<T> From<PopupMenuDivider> for PopupMenuEntry<T> {
    fn from(value: PopupMenuDivider) -> Self {
        Self::Divider(value)
    }
}

impl<T: Clone + 'static> PopupMenuEntry<T> {
    fn build_with_selection(
        &self,
        context: &incular_widgets::BuildContext<'_>,
        on_selected: Option<Rc<dyn Fn(Option<T>) + 'static>>,
        close: Option<Rc<dyn Fn() + 'static>>,
    ) -> Widget {
        match self {
            Self::Item(item) => item.build_with_selection_and_close(context, on_selected, close),
            Self::Checked(item) => item.build_with_selection_and_close(context, on_selected, close),
            Self::Divider(divider) => divider.clone().into(),
        }
    }
}

/// A button which opens a typed popup menu.
#[derive(Clone, TypedBuilder)]
pub struct PopupMenuButton<T> {
    #[builder(
        setter(
            fn transform<F, E>(item_builder: F) -> Rc<dyn Fn() -> Vec<PopupMenuEntry<T>> + 'static>
            where
                F: Fn() -> Vec<E> + 'static,
                E: Into<PopupMenuEntry<T>> + 'static,
            {
                Rc::new(move || {
                    item_builder()
                        .into_iter()
                        .map(Into::into)
                        .collect::<Vec<_>>()
                })
            }
        )
    )]
    item_builder: Rc<dyn Fn() -> Vec<PopupMenuEntry<T>> + 'static>,
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    icon: Option<Widget>,
    #[builder(default, setter(strip_option))]
    initial_value: Option<T>,
    #[builder(default = true)]
    enabled: bool,
    #[builder(default, setter(strip_option, into))]
    tooltip: Option<String>,
    #[builder(default = EdgeInsets::all(8.0))]
    padding: EdgeInsets,
    #[builder(default, setter(strip_option))]
    menu_padding: Option<EdgeInsets>,
    #[builder(default)]
    offset: Offset,
    #[builder(default, setter(transform = |value: f32| Some(value.max(0.0))))]
    elevation: Option<f32>,
    #[builder(default, setter(strip_option))]
    color: Option<Color>,
    #[builder(default, setter(strip_option))]
    shadow_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    surface_tint_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    menu_style: Option<MenuStyle>,
    #[builder(default, setter(strip_option))]
    button_style: Option<ButtonStyle>,
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
    on_opened: Option<Rc<dyn Fn() + 'static>>,
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
    on_selected: Option<Rc<dyn Fn(T) + 'static>>,
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
    on_canceled: Option<Rc<dyn Fn() + 'static>>,
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
    #[builder(default)]
    controller: MenuController,
    #[builder(default = true)]
    barrier_dismissible: bool,
}

impl<T> PopupMenuButton<T> {
    #[must_use]
    pub fn new<E>(item_builder: impl Fn() -> Vec<E> + 'static) -> Self
    where
        E: Into<PopupMenuEntry<T>> + 'static,
    {
        Self::builder().item_builder(item_builder).build()
    }

    #[must_use]
    pub fn from_items(items: impl IntoIterator<Item = PopupMenuItem<T>>) -> Self
    where
        T: Clone + 'static,
    {
        let items: Vec<_> = items.into_iter().collect();
        Self::new(move || items.clone())
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
    pub fn initial_value(mut self, value: T) -> Self {
        self.initial_value = Some(value);
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
    pub fn tooltip(mut self, value: impl Into<String>) -> Self {
        self.tooltip = Some(value.into());
        self
    }

    #[must_use]
    pub fn padding(mut self, value: EdgeInsets) -> Self {
        self.padding = value;
        self
    }

    #[must_use]
    pub fn menu_padding(mut self, value: EdgeInsets) -> Self {
        self.menu_padding = Some(value);
        self
    }

    #[must_use]
    pub fn offset(mut self, value: Offset) -> Self {
        self.offset = value;
        self
    }

    #[must_use]
    pub fn elevation(mut self, value: f32) -> Self {
        self.elevation = Some(value.max(0.0));
        self
    }

    #[must_use]
    pub fn color(mut self, value: Color) -> Self {
        self.color = Some(value);
        self
    }

    #[must_use]
    pub fn shadow_color(mut self, value: Color) -> Self {
        self.shadow_color = Some(value);
        self
    }

    #[must_use]
    pub fn surface_tint_color(mut self, value: Color) -> Self {
        self.surface_tint_color = Some(value);
        self
    }

    #[must_use]
    pub fn menu_style(mut self, value: MenuStyle) -> Self {
        self.menu_style = Some(value);
        self
    }

    #[must_use]
    pub fn style(mut self, value: ButtonStyle) -> Self {
        self.button_style = Some(value);
        self
    }

    #[must_use]
    pub fn on_opened(mut self, value: impl Fn() + 'static) -> Self {
        self.on_opened = Some(Rc::new(value));
        self
    }

    #[must_use]
    pub fn on_selected(mut self, value: impl Fn(T) + 'static) -> Self {
        self.on_selected = Some(Rc::new(value));
        self
    }

    #[must_use]
    pub fn on_canceled(mut self, value: impl Fn() + 'static) -> Self {
        self.on_canceled = Some(Rc::new(value));
        self
    }

    #[must_use]
    pub fn on_tap(mut self, value: impl Fn() + 'static) -> Self {
        self.on_tap = Some(Rc::new(value));
        self
    }

    /// Controls whether an outside pointer activation dismisses the open
    /// popup. The barrier is retained in the same overlay stack as the menu,
    /// so disabling it does not alter menu item hit testing.
    #[must_use]
    pub fn barrier_dismissible(mut self, value: bool) -> Self {
        self.barrier_dismissible = value;
        self
    }

    #[must_use]
    pub fn controller(mut self, value: MenuController) -> Self {
        self.controller = value;
        self
    }
}

impl<T: Clone + 'static> PopupMenuButton<T> {
    fn build(&self, context: &incular_widgets::BuildContext<'_>) -> Widget {
        let theme = current_control_theme(context);
        let controller = self.controller.clone();
        let on_opened = self.on_opened.clone();
        let on_tap = self.on_tap.clone();
        let on_canceled = self.on_canceled.clone();
        let anchor_child = self
            .child
            .clone()
            .or_else(|| self.icon.clone())
            .unwrap_or_else(|| Text::new("More").into());
        let mut anchor_style = self
            .button_style
            .clone()
            .unwrap_or_else(|| ButtonStyle::new().variant(ButtonVariant::Ghost));
        anchor_style = anchor_style.padding(self.padding);
        let mut anchor = ControlButton::with_child(anchor_child)
            .style(anchor_style)
            .enabled(self.enabled);
        if self.enabled {
            anchor = anchor.on_click(move || {
                if let Some(callback) = on_tap.as_ref() {
                    callback();
                }
                let next = !controller.is_open();
                controller.set_open(next);
                if next {
                    if let Some(callback) = on_opened.as_ref() {
                        callback();
                    }
                } else if let Some(callback) = on_canceled.as_ref() {
                    callback();
                }
            });
        }
        let anchor: Widget = anchor.into();
        if !self.controller.is_open() {
            return anchor;
        }

        let close = {
            let controller = self.controller.clone();
            Rc::new(move || controller.close()) as Rc<dyn Fn() + 'static>
        };
        let selected = self.on_selected.clone();
        let select = Rc::new(move |value: Option<T>| {
            if let Some(value) = value
                && let Some(callback) = selected.as_ref()
            {
                callback(value);
            }
        }) as Rc<dyn Fn(Option<T>) + 'static>;
        let children = (self.item_builder)()
            .into_iter()
            .map(|item| {
                item.build_with_selection(context, Some(select.clone()), Some(close.clone()))
            })
            .collect::<Vec<Widget>>();
        let mut style = self.menu_style.clone().unwrap_or_default();
        if let Some(padding) = self.menu_padding {
            style = style.padding(padding);
        }
        if let Some(value) = self.elevation {
            style = style.elevation(value);
        }
        if let Some(value) = self.color {
            style = style.background_color(value);
        }
        if let Some(value) = self.shadow_color {
            style = style.shadow_color(value);
        }
        if let Some(value) = self.surface_tint_color {
            style = style.surface_tint_color(value);
        }
        let (panel, panel_height) = menu_panel(children, &style, &theme);
        let panel: Widget = Positioned::new(panel)
            .left(self.offset.x)
            .top(self.offset.y)
            .height(panel_height)
            .into();
        let barrier = if self.barrier_dismissible {
            let controller = self.controller.clone();
            let on_canceled = self.on_canceled.clone();
            let barrier = GestureDetector::new(Container::new().color(Color::TRANSPARENT))
                .behavior(HitTestBehavior::Opaque)
                .on_tap(move || {
                    controller.close();
                    if let Some(callback) = on_canceled.as_ref() {
                        callback();
                    }
                });
            Some(Widget::from(Positioned::fill(barrier)))
        } else {
            None
        };
        let mut portal = incular_widgets::OverlayPortal::new(anchor)
            .overlay_child(panel)
            .role(TransientRole::Menu)
            .show(true);
        if let Some(barrier) = barrier {
            portal = portal.barrier_child(barrier);
        }
        portal.into()
    }
}

impl<T: Clone + 'static> From<PopupMenuButton<T>> for Widget {
    fn from(value: PopupMenuButton<T>) -> Self {
        let value = Rc::new(value);
        let revision = value.controller.revision();
        Widget::stateful_layout_builder(revision, move |context, _| value.build(context))
    }
}

pub(super) fn menu_panel(
    children: Vec<Widget>,
    style: &MenuStyle,
    theme: &incular_controls::ControlTheme,
) -> (Widget, f32) {
    const DEFAULT_MENU_ITEM_HEIGHT: f32 = 48.0;
    let item_count = children.len() as f32;
    let state = ControlState::empty();
    let item_list: Widget = ListView::new(children).padding(EdgeInsets::ZERO).into();
    let padding = style
        .padding
        .unwrap_or_else(|| EdgeInsets::symmetric(0.0, 8.0));
    let natural_height = item_count * DEFAULT_MENU_ITEM_HEIGHT + padding.top + padding.bottom;
    let minimum_height = style.minimum_size.map_or(0.0, |size| size.height);
    let maximum_height = style.maximum_size.map_or(f32::INFINITY, |size| size.height);
    let panel_height = style
        .fixed_size
        .map(|size| size.height)
        .filter(|height| height.is_finite())
        .unwrap_or(natural_height)
        .max(minimum_height)
        .min(maximum_height)
        .max(1.0);
    let shape = style.resolve_shape(state);
    let border = style.side.as_ref().map(|value| value.resolve(state));
    let surface: Widget = Container::with_child(item_list)
        .padding(padding)
        .decoration(
            BoxDecoration::new()
                .border_radius(shape)
                .border(border.unwrap_or_else(|| Border::new(0.0, Color::TRANSPARENT))),
        )
        .into();
    let material = Material::new(surface)
        .color(style.resolve_background(state, theme))
        .shadow_color(style.resolve_shadow(state, theme))
        .elevation(style.resolve_elevation(state))
        .border_radius(shape)
        .clip_behavior(Clip::HardEdge);
    let material: Widget = if let Some(tint) = style.resolve_tint(state) {
        material.surface_tint_color(tint).into()
    } else {
        material.into()
    };
    (style.constrained(material), panel_height)
}
