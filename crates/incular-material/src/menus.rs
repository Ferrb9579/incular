//! Material menu and dropdown components.
//!
//! This module owns the Material vocabulary and default composition for menus.
//! Hit testing, retained overlays, buttons, text editing, and type-ahead state
//! remain in the lower-level crates.  The types intentionally accept ordinary
//! [`Widget`] values so an application can use custom menu rows and icons
//! without opting into a second menu renderer.

use std::cell::Cell;
use std::rc::Rc;

use crate::foundation::{InputDecoration, InputDecorationThemeData, Material};
use crate::{DropdownMenuCloseBehavior, PopupMenuPosition, TextField};
use incular_config::{Alignment, Clip, Constraints, CrossAxisAlignment, EdgeInsets};
use incular_controls::{
    Button as ControlButton, ButtonStyle, ButtonVariant, ControlIcon, ControlState, StateValue,
    current_control_theme,
};
use incular_core::{Color, Offset, Size};
use incular_semantics::{Role as SemanticRole, SemanticActionKind, SemanticState};
use incular_text::{TextEditingController, TextStyle};
use incular_widgets::internal::{ActionSurface, ExplicitSemantics};
use incular_widgets::{
    Border, BorderRadius, BoxDecoration, Column, Container, GestureDetector, HitTestBehavior,
    ListView, Positioned, Row, SizedBox, Stack, Text, Widget,
};
use typed_builder::TypedBuilder;

type DropdownValidator<T> = Rc<dyn Fn(Option<&T>) -> Option<String> + 'static>;
type DropdownSaved<T> = Rc<dyn Fn(Option<&T>) + 'static>;

/// State-aware visual properties used by Material menus.
///
/// This mirrors Flutter's `MenuStyle` property surface while using the shared
/// `StateValue` resolver from `incular-controls`.  The fields remain sparse so
/// component themes can be merged without replacing unrelated defaults.
#[derive(Clone, Debug, Default, PartialEq, TypedBuilder)]
pub struct MenuStyle {
    #[builder(default, setter(strip_option, into))]
    pub background_color: Option<StateValue<Color>>,
    #[builder(default, setter(strip_option, into))]
    pub shadow_color: Option<StateValue<Color>>,
    #[builder(default, setter(strip_option, into))]
    pub surface_tint_color: Option<StateValue<Color>>,
    #[builder(default, setter(transform = |value: impl Into<StateValue<f32>>| Some(value.into())))]
    pub elevation: Option<StateValue<f32>>,
    #[builder(default, setter(strip_option))]
    pub padding: Option<EdgeInsets>,
    #[builder(default, setter(strip_option))]
    pub minimum_size: Option<Size>,
    #[builder(default, setter(strip_option))]
    pub fixed_size: Option<Size>,
    #[builder(default, setter(strip_option))]
    pub maximum_size: Option<Size>,
    #[builder(default, setter(strip_option, into))]
    pub side: Option<StateValue<Border>>,
    #[builder(default, setter(strip_option, into))]
    pub shape: Option<StateValue<BorderRadius>>,
    #[builder(default, setter(strip_option, into))]
    pub mouse_cursor: Option<String>,
    #[builder(default, setter(strip_option))]
    pub alignment: Option<Alignment>,
}

impl MenuStyle {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn background_color(mut self, value: impl Into<StateValue<Color>>) -> Self {
        self.background_color = Some(value.into());
        self
    }

    #[must_use]
    pub fn shadow_color(mut self, value: impl Into<StateValue<Color>>) -> Self {
        self.shadow_color = Some(value.into());
        self
    }

    #[must_use]
    pub fn surface_tint_color(mut self, value: impl Into<StateValue<Color>>) -> Self {
        self.surface_tint_color = Some(value.into());
        self
    }

    #[must_use]
    pub fn elevation(mut self, value: impl Into<StateValue<f32>>) -> Self {
        self.elevation = Some(value.into());
        self
    }

    #[must_use]
    pub fn padding(mut self, value: EdgeInsets) -> Self {
        self.padding = Some(value);
        self
    }

    #[must_use]
    pub fn minimum_size(mut self, value: Size) -> Self {
        self.minimum_size = Some(value);
        self
    }

    #[must_use]
    pub fn fixed_size(mut self, value: Size) -> Self {
        self.fixed_size = Some(value);
        self
    }

    #[must_use]
    pub fn maximum_size(mut self, value: Size) -> Self {
        self.maximum_size = Some(value);
        self
    }

    #[must_use]
    pub fn side(mut self, value: impl Into<StateValue<Border>>) -> Self {
        self.side = Some(value.into());
        self
    }

    #[must_use]
    pub fn shape(mut self, value: impl Into<StateValue<BorderRadius>>) -> Self {
        self.shape = Some(value.into());
        self
    }

    #[must_use]
    pub fn mouse_cursor(mut self, value: impl Into<String>) -> Self {
        self.mouse_cursor = Some(value.into());
        self
    }

    #[must_use]
    pub fn alignment(mut self, value: Alignment) -> Self {
        self.alignment = Some(value);
        self
    }

    /// Fills only fields that are not already set on this style.
    #[must_use]
    pub fn merge(mut self, other: &Self) -> Self {
        macro_rules! fill {
            ($field:ident) => {
                if self.$field.is_none() {
                    self.$field = other.$field.clone();
                }
            };
        }
        fill!(background_color);
        fill!(shadow_color);
        fill!(surface_tint_color);
        fill!(elevation);
        fill!(padding);
        fill!(minimum_size);
        fill!(fixed_size);
        fill!(maximum_size);
        fill!(side);
        fill!(shape);
        fill!(mouse_cursor);
        fill!(alignment);
        self
    }

    #[must_use]
    fn resolve_background(
        &self,
        state: ControlState,
        theme: &incular_controls::ControlTheme,
    ) -> Color {
        self.background_color
            .as_ref()
            .map_or(theme.colors.surface_elevated, |value| value.resolve(state))
    }

    #[must_use]
    fn resolve_shadow(
        &self,
        state: ControlState,
        _theme: &incular_controls::ControlTheme,
    ) -> Color {
        self.shadow_color
            .as_ref()
            .map_or(Color::BLACK, |value| value.resolve(state))
    }

    #[must_use]
    fn resolve_tint(&self, state: ControlState) -> Option<Color> {
        self.surface_tint_color
            .as_ref()
            .map(|value| value.resolve(state))
    }

    #[must_use]
    fn resolve_elevation(&self, state: ControlState) -> f32 {
        self.elevation
            .as_ref()
            .map_or(8.0, |value| value.resolve(state).max(0.0))
    }

    #[must_use]
    fn resolve_shape(&self, state: ControlState) -> BorderRadius {
        self.shape
            .as_ref()
            .map_or_else(|| BorderRadius::circular(4.0), |value| value.resolve(state))
    }

    #[must_use]
    fn constrained(&self, widget: Widget) -> Widget {
        if let Some(fixed) = self.fixed_size {
            return Container::with_child(widget)
                .constraints(Constraints::tight(fixed))
                .into();
        }
        let min = self.minimum_size.unwrap_or(Size::ZERO);
        let max_width = self
            .maximum_size
            .map_or(f32::INFINITY, |size| size.width)
            .max(min.width);
        let max_height = self
            .maximum_size
            .map_or(f32::INFINITY, |size| size.height)
            .max(min.height);
        if min == Size::ZERO && !max_width.is_finite() && !max_height.is_finite() {
            widget
        } else {
            Container::with_child(widget)
                .constraints(Constraints::new(
                    min.width.max(0.0),
                    max_width,
                    min.height.max(0.0),
                    max_height,
                ))
                .into()
        }
    }
}

/// Theme data for `MenuAnchor`, `MenuItemButton`, and `SubmenuButton`.
#[derive(Clone, Debug, Default, PartialEq, TypedBuilder)]
pub struct MenuThemeData {
    #[builder(default, setter(strip_option))]
    pub style: Option<MenuStyle>,
    #[builder(default, setter(strip_option, into))]
    pub submenu_icon: Option<StateValue<Widget>>,
}

impl MenuThemeData {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn style(mut self, value: MenuStyle) -> Self {
        self.style = Some(value);
        self
    }

    #[must_use]
    pub fn submenu_icon(mut self, value: impl Into<StateValue<Widget>>) -> Self {
        self.submenu_icon = Some(value.into());
        self
    }

    #[must_use]
    pub fn copy_with(
        &self,
        style: Option<MenuStyle>,
        submenu_icon: Option<StateValue<Widget>>,
    ) -> Self {
        Self {
            style: style.or_else(|| self.style.clone()),
            submenu_icon: submenu_icon.or_else(|| self.submenu_icon.clone()),
        }
    }
}

/// Theme data for `PopupMenuButton` and popup menu entries.
#[derive(Clone, Debug, Default, PartialEq, TypedBuilder)]
pub struct PopupMenuThemeData {
    #[builder(default, setter(strip_option))]
    pub color: Option<Color>,
    #[builder(default, setter(strip_option))]
    pub shape: Option<BorderRadius>,
    #[builder(default, setter(strip_option))]
    pub menu_padding: Option<EdgeInsets>,
    #[builder(default, setter(transform = |value: f32| Some(value.max(0.0))))]
    pub elevation: Option<f32>,
    #[builder(default, setter(strip_option))]
    pub shadow_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    pub surface_tint_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    pub text_style: Option<TextStyle>,
    #[builder(default, setter(strip_option, into))]
    pub label_text_style: Option<StateValue<TextStyle>>,
    #[builder(default, setter(strip_option))]
    pub enable_feedback: Option<bool>,
    #[builder(default, setter(strip_option, into))]
    pub mouse_cursor: Option<String>,
    #[builder(default, setter(strip_option))]
    pub position: Option<PopupMenuPosition>,
    #[builder(default, setter(strip_option))]
    pub icon_color: Option<Color>,
    #[builder(default, setter(transform = |value: f32| Some(value.max(0.0))))]
    pub icon_size: Option<f32>,
}

impl PopupMenuThemeData {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn color(mut self, value: Color) -> Self {
        self.color = Some(value);
        self
    }

    #[must_use]
    pub fn shape(mut self, value: BorderRadius) -> Self {
        self.shape = Some(value);
        self
    }

    #[must_use]
    pub fn menu_padding(mut self, value: EdgeInsets) -> Self {
        self.menu_padding = Some(value);
        self
    }

    #[must_use]
    pub fn elevation(mut self, value: f32) -> Self {
        self.elevation = Some(value.max(0.0));
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
    pub fn enable_feedback(mut self, value: bool) -> Self {
        self.enable_feedback = Some(value);
        self
    }

    #[must_use]
    pub fn mouse_cursor(mut self, value: impl Into<String>) -> Self {
        self.mouse_cursor = Some(value.into());
        self
    }

    #[must_use]
    pub fn position(mut self, value: PopupMenuPosition) -> Self {
        self.position = Some(value);
        self
    }

    #[must_use]
    pub fn icon_color(mut self, value: Color) -> Self {
        self.icon_color = Some(value);
        self
    }

    #[must_use]
    pub fn icon_size(mut self, value: f32) -> Self {
        self.icon_size = Some(value.max(0.0));
        self
    }
}

/// Theme data for `DropdownMenu`.
#[derive(Clone, Debug, Default, PartialEq, TypedBuilder)]
pub struct DropdownMenuThemeData {
    #[builder(default, setter(strip_option))]
    pub text_style: Option<TextStyle>,
    #[builder(default, setter(strip_option))]
    pub input_decoration_theme: Option<InputDecorationThemeData>,
    #[builder(default, setter(strip_option))]
    pub menu_style: Option<MenuStyle>,
    #[builder(default, setter(strip_option))]
    pub disabled_color: Option<Color>,
}

impl DropdownMenuThemeData {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn text_style(mut self, value: TextStyle) -> Self {
        self.text_style = Some(value);
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
    pub fn disabled_color(mut self, value: Color) -> Self {
        self.disabled_color = Some(value);
        self
    }
}

/// Retained controller shared by menu anchors and popup buttons.
#[derive(Clone, Default)]
pub struct MenuController {
    open: Rc<Cell<bool>>,
    revision: Rc<Cell<u64>>,
}

impl MenuController {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open.get()
    }

    pub fn open(&self) {
        self.set_open(true);
    }

    pub fn close(&self) {
        self.set_open(false);
    }

    pub fn toggle(&self) {
        self.set_open(!self.is_open());
    }

    pub fn set_open(&self, value: bool) {
        let previous = self.open.replace(value);
        if previous != value {
            self.revision.set(self.revision.get().wrapping_add(1));
        }
    }

    fn revision(&self) -> Rc<Cell<u64>> {
        self.revision.clone()
    }
}

/// A horizontal Material menu bar.
///
/// `MenuBar` is a Material composition widget, not a renderer primitive. Its
/// children are normally [`MenuItemButton`] and [`SubmenuButton`] values, but
/// ordinary widgets are accepted as well so applications can provide custom
/// menu entries. The retained row/surface is shared with the menu anchor
/// implementation and keeps the menu API independent of a platform window
/// menu implementation.
#[derive(Clone, TypedBuilder)]
pub struct MenuBar {
    #[builder(
        default,
        setter(transform = |children: impl IntoIterator<Item = impl Into<Widget>>| {
            children.into_iter().map(Into::into).collect::<Vec<Widget>>()
        })
    )]
    children: Vec<Widget>,
    #[builder(default, setter(strip_option))]
    style: Option<MenuStyle>,
    #[builder(default, setter(strip_option))]
    item_style: Option<ButtonStyle>,
    #[builder(default, setter(transform = |value: f32| value.max(0.0)))]
    spacing: f32,
    #[builder(default = EdgeInsets::symmetric(4.0, 4.0))]
    padding: EdgeInsets,
    #[builder(default = Alignment::TOP_LEFT)]
    alignment: Alignment,
    #[builder(default = Clip::HardEdge)]
    clip_behavior: Clip,
    #[builder(default = true)]
    enabled: bool,
}

impl Default for MenuBar {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl MenuBar {
    #[must_use]
    pub fn new(children: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        Self {
            children: children.into_iter().map(Into::into).collect(),
            style: None,
            item_style: None,
            spacing: 0.0,
            padding: EdgeInsets::symmetric(4.0, 4.0),
            alignment: Alignment::TOP_LEFT,
            clip_behavior: Clip::HardEdge,
            enabled: true,
        }
    }

    #[must_use]
    pub fn children(mut self, value: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        self.children = value.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn style(mut self, value: MenuStyle) -> Self {
        self.style = Some(value);
        self
    }

    /// Applies a common button style to direct children which are plain
    /// widgets. Existing `MenuItemButton`/`SubmenuButton` descriptors retain
    /// their own behavior; this value is a layout-level fallback for custom
    /// menu rows.
    #[must_use]
    pub fn item_style(mut self, value: ButtonStyle) -> Self {
        self.item_style = Some(value);
        self
    }

    #[must_use]
    pub fn spacing(mut self, value: f32) -> Self {
        self.spacing = value.max(0.0);
        self
    }

    #[must_use]
    pub fn padding(mut self, value: EdgeInsets) -> Self {
        self.padding = value;
        self
    }

    #[must_use]
    pub fn alignment(mut self, value: Alignment) -> Self {
        self.alignment = value;
        self
    }

    #[must_use]
    pub fn clip_behavior(mut self, value: Clip) -> Self {
        self.clip_behavior = value;
        self
    }

    #[must_use]
    pub fn enabled(mut self, value: bool) -> Self {
        self.enabled = value;
        self
    }
}

impl From<MenuBar> for Widget {
    fn from(value: MenuBar) -> Self {
        let theme = current_control_theme();
        let children = if let Some(style) = value.item_style {
            value
                .children
                .into_iter()
                .map(|child| ControlButton::with_child(child).style(style.clone()).into())
                .collect::<Vec<Widget>>()
        } else {
            value.children
        };
        let row: Widget = Row::new(children)
            .spacing(value.spacing)
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .into();
        let style = value.style.unwrap_or_default();
        let surface = Container::with_child(row)
            .padding(style.padding.unwrap_or(value.padding))
            .alignment(value.alignment)
            .decoration(
                BoxDecoration::new()
                    .border_radius(style.resolve_shape(ControlState::empty()))
                    .border(
                        style
                            .side
                            .as_ref()
                            .map(|side| side.resolve(ControlState::empty()))
                            .unwrap_or_else(|| Border::new(0.0, Color::TRANSPARENT)),
                    ),
            )
            .clip_behavior(value.clip_behavior);
        // Keep the menu bar's surface state-aware in the same way as popup
        // menus. `enabled` controls the semantic state and custom child
        // controls remain responsible for their own activation policy.
        let state = ControlState::from_enabled(value.enabled);
        let material: Widget = Material::new(surface)
            .color(style.resolve_background(state, &theme))
            .shadow_color(style.resolve_shadow(state, &theme))
            .elevation(style.resolve_elevation(state))
            .border_radius(style.resolve_shape(state))
            .clip_behavior(value.clip_behavior)
            .into();
        material.semantics(
            ExplicitSemantics::new(SemanticRole::Menu).state(SemanticState {
                enabled: value.enabled,
                ..SemanticState::default()
            }),
        )
    }
}

/// A leaf menu item used by menu anchors and menu bars.
#[derive(Clone, TypedBuilder)]
pub struct MenuItemButton {
    #[builder(setter(into))]
    child: Widget,
    #[builder(default, setter(strip_option, into))]
    leading_icon: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    trailing_icon: Option<Widget>,
    #[builder(default = true)]
    enabled: bool,
    #[builder(default = true)]
    request_focus_on_hover: bool,
    #[builder(default = true)]
    close_on_activate: bool,
    #[builder(default, setter(transform = |value: f32| Some(value.max(0.0))))]
    height: Option<f32>,
    #[builder(default, setter(strip_option, into))]
    semantics_label: Option<String>,
    #[builder(default, setter(strip_option))]
    style: Option<ButtonStyle>,
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
    on_hover: Option<Rc<dyn Fn(bool) + 'static>>,
}

impl MenuItemButton {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            leading_icon: None,
            trailing_icon: None,
            enabled: true,
            request_focus_on_hover: true,
            close_on_activate: true,
            height: None,
            semantics_label: None,
            style: None,
            on_pressed: None,
            on_hover: None,
        }
    }

    #[must_use]
    pub fn label(value: impl Into<String>) -> Self {
        Self::new(Text::new(value.into()))
    }

    #[must_use]
    pub fn child(mut self, value: impl Into<Widget>) -> Self {
        self.child = value.into();
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
    pub fn request_focus_on_hover(mut self, value: bool) -> Self {
        self.request_focus_on_hover = value;
        self
    }

    #[must_use]
    pub fn close_on_activate(mut self, value: bool) -> Self {
        self.close_on_activate = value;
        self
    }

    #[must_use]
    pub fn height(mut self, value: f32) -> Self {
        self.height = Some(value.max(0.0));
        self
    }

    #[must_use]
    pub fn semantics_label(mut self, value: impl Into<String>) -> Self {
        self.semantics_label = Some(value.into());
        self
    }

    #[must_use]
    pub fn style(mut self, value: ButtonStyle) -> Self {
        self.style = Some(value);
        self
    }

    #[must_use]
    pub fn on_pressed(mut self, value: impl Fn() + 'static) -> Self {
        self.on_pressed = Some(Rc::new(value));
        self
    }

    #[must_use]
    pub fn on_hover(mut self, value: impl Fn(bool) + 'static) -> Self {
        self.on_hover = Some(Rc::new(value));
        self
    }

    fn build_with_close(&self, close: Option<Rc<dyn Fn() + 'static>>) -> Widget {
        let mut children = Vec::with_capacity(3);
        if let Some(icon) = &self.leading_icon {
            children.push(icon.clone());
        }
        children.push(self.child.clone());
        if let Some(icon) = &self.trailing_icon {
            children.push(icon.clone());
        }
        let content: Widget = if self.leading_icon.is_none() && self.trailing_icon.is_none() {
            self.child.clone()
        } else {
            Row::new(children)
                .spacing(8.0)
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .into()
        };
        let semantic_label = self
            .semantics_label
            .clone()
            .filter(|label| !label.trim().is_empty())
            .or_else(|| content.semantic_text())
            .unwrap_or_default();

        let mut style = self
            .style
            .clone()
            .unwrap_or_else(|| ButtonStyle::new().variant(ButtonVariant::Ghost));
        if let Some(height) = self.height {
            style = style.height(height);
        }
        if style.padding.is_none() {
            style = style.padding(EdgeInsets::symmetric(12.0, 6.0));
        }
        let mut button = ControlButton::with_child(content)
            .style(style)
            .enabled(self.enabled);

        let on_pressed = self.on_pressed.clone();
        let close_on_activate = self.close_on_activate;
        if on_pressed.is_some() || (close_on_activate && close.is_some()) {
            button = button.on_click(move || {
                if close_on_activate {
                    if let Some(close) = close.as_ref() {
                        close();
                    }
                }
                if let Some(callback) = on_pressed.as_ref() {
                    callback();
                }
            });
        }

        // Menu rows are materialized by an already-deferred menu builder. Build
        // the inner control eagerly here so opening a menu does not introduce
        // another layout-builder boundary into a lazy sliver child.
        let mut result: Widget = button.build(&current_control_theme());
        if let Some(hover) = self.on_hover.clone() {
            let enter = hover.clone();
            let exit = hover;
            result = ActionSurface::with_child(result)
                .color(Color::TRANSPARENT)
                .enabled(self.enabled)
                .on_hover(move || enter(true))
                .on_exit(move || exit(false))
                .into();
        }

        result.semantics(
            ExplicitSemantics::new(SemanticRole::Button)
                .label(semantic_label)
                .state(SemanticState {
                    enabled: self.enabled,
                    focusable: self.enabled,
                    ..SemanticState::default()
                })
                .actions(if self.enabled {
                    vec![SemanticActionKind::Focus, SemanticActionKind::Activate]
                } else {
                    Vec::new()
                }),
        )
    }
}

impl From<MenuItemButton> for Widget {
    fn from(value: MenuItemButton) -> Self {
        value.build_with_close(None)
    }
}

/// An item that owns a cascading submenu.
#[derive(Clone, TypedBuilder)]
pub struct SubmenuButton {
    #[builder(setter(into))]
    child: Widget,
    #[builder(
        setter(transform = |menu_children: impl IntoIterator<Item = impl Into<Widget>>| {
            menu_children
                .into_iter()
                .map(Into::into)
                .collect::<Vec<Widget>>()
        })
    )]
    menu_children: Vec<Widget>,
    #[builder(default, setter(strip_option, into))]
    leading_icon: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    trailing_icon: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    submenu_icon: Option<Widget>,
    #[builder(default = true)]
    enabled: bool,
    #[builder(default, setter(strip_option))]
    style: Option<ButtonStyle>,
    #[builder(default, setter(strip_option))]
    menu_style: Option<MenuStyle>,
    #[builder(default = Offset::new(4.0, 0.0))]
    alignment_offset: Offset,
    #[builder(default)]
    controller: MenuController,
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
    on_open: Option<Rc<dyn Fn() + 'static>>,
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
    on_close: Option<Rc<dyn Fn() + 'static>>,
}

impl SubmenuButton {
    #[must_use]
    pub fn new(
        child: impl Into<Widget>,
        menu_children: impl IntoIterator<Item = impl Into<Widget>>,
    ) -> Self {
        Self {
            child: child.into(),
            menu_children: menu_children.into_iter().map(Into::into).collect(),
            leading_icon: None,
            trailing_icon: None,
            submenu_icon: None,
            enabled: true,
            style: None,
            menu_style: None,
            alignment_offset: Offset::new(4.0, 0.0),
            controller: MenuController::new(),
            on_open: None,
            on_close: None,
        }
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
    pub fn submenu_icon(mut self, value: impl Into<Widget>) -> Self {
        self.submenu_icon = Some(value.into());
        self
    }

    #[must_use]
    pub fn enabled(mut self, value: bool) -> Self {
        self.enabled = value;
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
    pub fn alignment_offset(mut self, value: Offset) -> Self {
        self.alignment_offset = value;
        self
    }

    #[must_use]
    pub fn controller(mut self, value: MenuController) -> Self {
        self.controller = value;
        self
    }

    #[must_use]
    pub fn on_open(mut self, value: impl Fn() + 'static) -> Self {
        self.on_open = Some(Rc::new(value));
        self
    }

    #[must_use]
    pub fn on_close(mut self, value: impl Fn() + 'static) -> Self {
        self.on_close = Some(Rc::new(value));
        self
    }
}

impl From<SubmenuButton> for Widget {
    fn from(value: SubmenuButton) -> Self {
        let mut row = Row::new([value.child.clone()]).spacing(8.0);
        if let Some(icon) = value.leading_icon.clone() {
            row = Row::new([icon, value.child.clone()]).spacing(8.0);
        }
        if let Some(icon) = value.trailing_icon.clone().or(value.submenu_icon.clone()) {
            row = Row::new([row.into(), icon]).spacing(8.0);
        }
        let mut anchor = MenuAnchor::new(value.menu_children)
            .child(row)
            .enabled(value.enabled)
            .alignment_offset(value.alignment_offset)
            .controller(value.controller.clone());
        if let Some(style) = value.style {
            anchor = anchor.item_style(style);
        }
        if let Some(style) = value.menu_style {
            anchor = anchor.style(style);
        }
        if let Some(on_open) = value.on_open {
            anchor = anchor.on_open(move || on_open());
        }
        if let Some(on_close) = value.on_close {
            anchor = anchor.on_close(move || on_close());
        }
        anchor.into()
    }
}

/// An anchor and a retained popup menu.
#[derive(Clone, TypedBuilder)]
#[builder(builder_method(name = typed_builder))]
pub struct MenuAnchor {
    #[builder(
        setter(transform = |menu_children: impl IntoIterator<Item = impl Into<Widget>>| {
            menu_children
                .into_iter()
                .map(Into::into)
                .collect::<Vec<Widget>>()
        })
    )]
    menu_children: Vec<Widget>,
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
    #[builder(
        default,
        setter(
            fn transform<F>(builder: F) -> Option<Rc<dyn Fn(bool) -> Widget + 'static>>
            where
                F: Fn(bool) -> Widget + 'static,
            {
                Some(Rc::new(builder))
            }
        )
    )]
    builder: Option<Rc<dyn Fn(bool) -> Widget + 'static>>,
    #[builder(default, setter(strip_option))]
    style: Option<MenuStyle>,
    #[builder(default, setter(strip_option))]
    item_style: Option<ButtonStyle>,
    #[builder(default)]
    alignment_offset: Offset,
    #[builder(default = Clip::HardEdge)]
    clip_behavior: Clip,
    #[builder(default)]
    consume_outside_tap: bool,
    #[builder(default = true)]
    cross_axis_unconstrained: bool,
    #[builder(default)]
    use_root_overlay: bool,
    #[builder(default)]
    animated: bool,
    #[builder(default = true)]
    enabled: bool,
    #[builder(default)]
    controller: MenuController,
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
    on_open: Option<Rc<dyn Fn() + 'static>>,
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
    on_close: Option<Rc<dyn Fn() + 'static>>,
}

impl Default for MenuAnchor {
    fn default() -> Self {
        Self::typed_builder()
            .menu_children(Vec::<Widget>::new())
            .build()
    }
}

impl MenuAnchor {
    #[must_use]
    pub fn new(menu_children: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        Self {
            menu_children: menu_children.into_iter().map(Into::into).collect(),
            child: None,
            builder: None,
            style: None,
            item_style: None,
            alignment_offset: Offset::ZERO,
            clip_behavior: Clip::HardEdge,
            consume_outside_tap: false,
            cross_axis_unconstrained: true,
            use_root_overlay: false,
            animated: false,
            enabled: true,
            controller: MenuController::new(),
            on_open: None,
            on_close: None,
        }
    }

    #[must_use]
    pub fn child(mut self, value: impl Into<Widget>) -> Self {
        self.child = Some(value.into());
        self
    }

    /// Supplies a builder that receives the current open state.
    #[must_use]
    pub fn builder(mut self, value: impl Fn(bool) -> Widget + 'static) -> Self {
        self.builder = Some(Rc::new(value));
        self
    }

    #[must_use]
    pub fn style(mut self, value: MenuStyle) -> Self {
        self.style = Some(value);
        self
    }

    #[must_use]
    pub fn item_style(mut self, value: ButtonStyle) -> Self {
        self.item_style = Some(value);
        self
    }

    #[must_use]
    pub fn alignment_offset(mut self, value: Offset) -> Self {
        self.alignment_offset = value;
        self
    }

    #[must_use]
    pub fn clip_behavior(mut self, value: Clip) -> Self {
        self.clip_behavior = value;
        self
    }

    #[must_use]
    pub fn consume_outside_tap(mut self, value: bool) -> Self {
        self.consume_outside_tap = value;
        self
    }

    #[must_use]
    pub fn cross_axis_unconstrained(mut self, value: bool) -> Self {
        self.cross_axis_unconstrained = value;
        self
    }

    #[must_use]
    pub fn use_root_overlay(mut self, value: bool) -> Self {
        self.use_root_overlay = value;
        self
    }

    #[must_use]
    pub fn animated(mut self, value: bool) -> Self {
        self.animated = value;
        self
    }

    #[must_use]
    pub fn enabled(mut self, value: bool) -> Self {
        self.enabled = value;
        self
    }

    #[must_use]
    pub fn open(self, value: bool) -> Self {
        self.controller.set_open(value);
        self
    }

    #[must_use]
    pub fn controller(mut self, value: MenuController) -> Self {
        self.controller = value;
        self
    }

    #[must_use]
    pub fn on_open(mut self, value: impl Fn() + 'static) -> Self {
        self.on_open = Some(Rc::new(value));
        self
    }

    #[must_use]
    pub fn on_close(mut self, value: impl Fn() + 'static) -> Self {
        self.on_close = Some(Rc::new(value));
        self
    }

    fn build(&self) -> Widget {
        let open = self.controller.is_open();
        let anchor_child = self.builder.as_ref().map_or_else(
            || {
                self.child
                    .clone()
                    .unwrap_or_else(|| Text::new("Menu").into())
            },
            |builder| builder(open),
        );
        let controller = self.controller.clone();
        let on_open = self.on_open.clone();
        let on_close = self.on_close.clone();
        let anchor_label = anchor_child.semantic_text();
        let mut anchor = ActionSurface::with_child(anchor_child)
            .color(Color::TRANSPARENT)
            .enabled(self.enabled);
        if self.enabled {
            anchor = anchor.on_click(move || {
                let next = !controller.is_open();
                controller.set_open(next);
                if next {
                    if let Some(callback) = on_open.as_ref() {
                        callback();
                    }
                } else if let Some(callback) = on_close.as_ref() {
                    callback();
                }
            });
        }
        let anchor: Widget = anchor.into();
        let mut semantics = ExplicitSemantics::new(SemanticRole::Button).state(SemanticState {
            enabled: self.enabled,
            focusable: self.enabled,
            expanded: Some(open),
            ..SemanticState::default()
        });
        if let Some(label) = anchor_label {
            semantics = semantics.label(label);
        }
        let anchor = anchor.semantics(semantics.actions(if self.enabled {
            vec![SemanticActionKind::Focus, SemanticActionKind::Activate]
        } else {
            Vec::new()
        }));

        if !open {
            return anchor;
        }

        let theme = current_control_theme();
        let style = self.style.clone().unwrap_or_default();
        let items = if let Some(item_style) = self.item_style.clone() {
            self.menu_children
                .iter()
                .cloned()
                .map(|item| {
                    ControlButton::with_child(item)
                        .style(item_style.clone())
                        .into()
                })
                .collect::<Vec<Widget>>()
        } else {
            self.menu_children.clone()
        };
        let (panel, panel_height) = menu_panel(items, &style, &theme);
        let panel: Widget = Positioned::new(panel)
            .left(self.alignment_offset.x)
            .top(self.alignment_offset.y)
            .height(panel_height)
            .into();
        // The lower-level overlay portal is retained here.  When requested,
        // add a transparent, full-bounds barrier beneath the anchor/panel so
        // an outside activation closes the menu while the visible rows remain
        // the top hit-test targets.
        let controller = self.controller.clone();
        let on_close = self.on_close.clone();
        let barrier_behavior = if self.consume_outside_tap {
            HitTestBehavior::Opaque
        } else {
            HitTestBehavior::Translucent
        };
        let barrier = GestureDetector::new(Container::new().color(Color::TRANSPARENT))
            .behavior(barrier_behavior)
            .on_tap(move || {
                controller.close();
                if let Some(callback) = on_close.as_ref() {
                    callback();
                }
            });
        let overlay: Widget = Stack::new([barrier.into(), anchor, panel]).into();
        let _ = (
            self.clip_behavior,
            self.cross_axis_unconstrained,
            self.use_root_overlay,
            self.animated,
        );
        overlay
    }
}

impl From<MenuAnchor> for Widget {
    fn from(value: MenuAnchor) -> Self {
        let value = Rc::new(value);
        let revision = value.controller.revision();
        Widget::stateful_layout_builder(revision, move |_| value.build())
    }
}

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
        item.build_with_close(close)
    }

    fn build_with_selection(&self, on_selected: Option<Rc<dyn Fn(Option<T>) + 'static>>) -> Widget
    where
        T: Clone + 'static,
    {
        self.build_with_selection_and_close(on_selected, None)
    }
}

impl<T: Clone + 'static> From<PopupMenuItem<T>> for Widget {
    fn from(value: PopupMenuItem<T>) -> Self {
        value.build_with_selection(None)
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
        on_selected: Option<Rc<dyn Fn(Option<T>) + 'static>>,
        close: Option<Rc<dyn Fn() + 'static>>,
    ) -> Widget
    where
        T: Clone + 'static,
    {
        let mark: Widget = if self.checked {
            ControlIcon::Check.widget(16.0, current_control_theme().colors.accent)
        } else {
            SizedBox::new().width(16.0).into()
        };
        let child = Row::new([mark, self.child.clone()]).spacing(8.0);
        self.popup_item(child)
            .build_with_selection_and_close(on_selected, close)
    }

    fn build_with_selection(&self, on_selected: Option<Rc<dyn Fn(Option<T>) + 'static>>) -> Widget
    where
        T: Clone + 'static,
    {
        self.build_with_selection_and_close(on_selected, None)
    }
}

impl<T: Clone + 'static> From<CheckedPopupMenuItem<T>> for Widget {
    fn from(value: CheckedPopupMenuItem<T>) -> Self {
        value.build_with_selection(None)
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
        on_selected: Option<Rc<dyn Fn(Option<T>) + 'static>>,
        close: Option<Rc<dyn Fn() + 'static>>,
    ) -> Widget {
        match self {
            Self::Item(item) => item.build_with_selection_and_close(on_selected, close),
            Self::Checked(item) => item.build_with_selection_and_close(on_selected, close),
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
    fn build(&self) -> Widget {
        let theme = current_control_theme();
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
            if let Some(value) = value {
                if let Some(callback) = selected.as_ref() {
                    callback(value);
                }
            }
        }) as Rc<dyn Fn(Option<T>) + 'static>;
        let children = (self.item_builder)()
            .into_iter()
            .map(|item| item.build_with_selection(Some(select.clone()), Some(close.clone())))
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
        let overlay_child = if self.barrier_dismissible {
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
            Stack::new([Positioned::fill(barrier).into(), panel]).into()
        } else {
            panel
        };
        incular_widgets::OverlayPortal::new(anchor)
            .overlay_child(overlay_child)
            .show(true)
            .into()
    }
}

impl<T: Clone + 'static> From<PopupMenuButton<T>> for Widget {
    fn from(value: PopupMenuButton<T>) -> Self {
        let value = Rc::new(value);
        let revision = value.controller.revision();
        Widget::stateful_layout_builder(revision, move |_| value.build())
    }
}

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
        let mut selected_widget = Container::with_child(selected_widget).alignment(value.alignment);
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
                ControlIcon::ChevronDown.widget(20.0, current_control_theme().colors.foreground),
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
        if let Some(decoration) = value.decoration.as_ref() {
            if let Some(label) = decoration.label_text.clone() {
                children.push(Text::new(label).into());
            }
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
        } else if let Some(decoration) = value.decoration.as_ref() {
            if let Some(helper) = decoration.helper_text.clone() {
                children.push(Text::new(helper).into());
            }
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
        Widget::stateful_layout_builder(value.revision.clone(), move |_| value.build())
    }
}

impl<T: Clone + PartialEq + 'static> DropdownMenu<T> {
    fn build(&self) -> Widget {
        let theme = current_control_theme();
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

fn menu_panel(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AppBar, Card, FilledButton, FloatingActionButton, MaterialApp, Scaffold, ScaffoldMessenger,
    };

    #[test]
    fn menu_style_merge_keeps_explicit_values() {
        let base = MenuStyle::new()
            .elevation(4.0)
            .padding(EdgeInsets::all(3.0));
        let override_style = MenuStyle::new()
            .elevation(9.0)
            .background_color(Color::WHITE);
        let merged = base.clone().merge(&override_style);
        assert_eq!(merged.elevation, base.elevation);
        assert_eq!(merged.padding, base.padding);
        assert_eq!(merged.background_color, override_style.background_color);
    }

    #[test]
    fn menu_controller_notifies_retained_revision_on_transitions() {
        let controller = MenuController::new();
        let revision = controller.revision.get();
        controller.open();
        assert!(controller.is_open());
        assert!(controller.revision.get() > revision);
        let next = controller.revision.get();
        controller.open();
        assert_eq!(controller.revision.get(), next);
        controller.close();
        assert!(!controller.is_open());
        assert!(controller.revision.get() > next);
    }

    #[test]
    fn dropdown_entry_preserves_value_and_disabled_state() {
        let entry = DropdownMenuEntry::new(7_u32, "Seven").disabled(true);
        assert_eq!(entry.value, 7);
        assert_eq!(entry.label, "Seven");
        assert!(!entry.enabled);
    }

    #[test]
    fn menu_bar_builds_a_retained_material_surface() {
        let children: Vec<Widget> = vec![
            MenuItemButton::label("File").into(),
            SubmenuButton::new(Text::new("Edit"), [MenuItemButton::label("Undo")]).into(),
        ];
        let _: Widget = MenuBar::new(children).spacing(4.0).into();
    }

    #[test]
    fn menu_anchor_opens_in_a_retained_tree() {
        let controller = MenuController::new();
        let widget: Widget = MenuAnchor::new([MenuItemButton::label("New")])
            .controller(controller.clone())
            .child(FilledButton::tonal("Menu"))
            .into();
        let mut tree = incular_widgets::internal::WidgetTree::new();
        tree.mount(ListView::new([widget]).into())
            .expect("mount menu anchor");
        tree.layout(Constraints::tight(Size::new(320.0, 240.0)));
        controller.open();
        tree.layout(Constraints::tight(Size::new(320.0, 240.0)));
        tree.update_semantics();
        assert!(
            tree.semantics()
                .iter()
                .any(|(_, node)| node.label.as_deref() == Some("New"))
        );
    }

    #[test]
    fn menu_anchor_opens_inside_material_scaffold() {
        let controller = MenuController::new();
        let body: Widget = Container::new()
            .color(Color::WHITE)
            .padding(EdgeInsets::all(24.0))
            .child(ListView::new([Card::new(
                Column::new([
                    Widget::from(Text::new("Button families")),
                    MenuAnchor::new([MenuItemButton::label("New document")])
                        .controller(controller.clone())
                        .child(FilledButton::tonal("Menu"))
                        .into(),
                ])
                .spacing(12.0),
            )]))
            .into();
        let root = MaterialApp::new(ScaffoldMessenger::new(
            Scaffold::new(body)
                .app_bar(AppBar::new(Text::new("Material Workbench")))
                .floating_action_button(FloatingActionButton::extended("Create")),
        ))
        .build();
        let mut tree = incular_widgets::internal::WidgetTree::new();
        tree.mount(root).expect("mount material scaffold");
        tree.layout(Constraints::tight(Size::new(1180.0, 820.0)));
        controller.open();
        tree.layout(Constraints::tight(Size::new(1180.0, 820.0)));
        tree.layout(Constraints::tight(Size::new(1180.0, 820.0)));
        let _ = tree.paint();
        tree.update_semantics();
        assert!(
            tree.semantics()
                .iter()
                .any(|(_, node)| node.label.as_deref() == Some("New document"))
        );
    }

    #[test]
    fn typed_builders_keep_menu_defaults_and_widget_composition() {
        let style = MenuStyle::builder()
            .background_color(Color::WHITE)
            .padding(EdgeInsets::all(4.0))
            .build();
        assert_eq!(style.padding, Some(EdgeInsets::all(4.0)));

        let item = MenuItemButton::builder()
            .child(Text::new("Open"))
            .leading_icon(Text::new("+"))
            .enabled(false)
            .build();
        assert!(!item.enabled);
        let _: Widget = item.into();

        let submenu = SubmenuButton::builder()
            .child(Text::new("Edit"))
            .menu_children([MenuItemButton::label("Undo")])
            .build();
        let _: Widget = submenu.into();

        let anchor = MenuAnchor::typed_builder()
            .menu_children([MenuItemButton::label("Close")])
            .child(Text::new("Menu"))
            .build();
        let _: Widget = anchor.into();

        let popup_item = PopupMenuItem::<u32>::builder()
            .child(Text::new("One"))
            .value(1)
            .height(-1.0)
            .build();
        assert_eq!(popup_item.height, 0.0);
        let checked = CheckedPopupMenuItem::<u32>::builder()
            .child(Text::new("Checked"))
            .checked(true)
            .value(2)
            .build();
        let _: Widget = checked.into();

        let popup = PopupMenuButton::<u32>::builder()
            .item_builder(|| vec![PopupMenuItem::label("One").value(1)])
            .child(Text::new("More"))
            .build();
        let _: Widget = popup.into();

        let dropdown = DropdownButton::<u32>::builder()
            .items([DropdownMenuItem::label("One").value(1)])
            .hint(Text::new("Choose"))
            .build();
        let _: Widget = dropdown.into();

        let entry = DropdownMenuEntry::<u32>::builder()
            .value(1_u32)
            .label("One")
            .label_widget(Text::new("Custom"))
            .build();
        let menu = DropdownMenu::builder()
            .entries([entry])
            .leading_icon(Text::new("+"))
            .build();
        let _: Widget = menu.into();
    }
}
