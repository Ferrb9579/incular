use std::cell::Cell;
use std::rc::Rc;

use super::{anchor::MenuAnchor, style::MenuStyle};
use crate::foundation::Material;
use incular_config::{Alignment, Clip, CrossAxisAlignment, EdgeInsets};
use incular_controls::{
    Button as ControlButton, ButtonStyle, ButtonVariant, ControlState, current_control_theme,
};
use incular_core::{Color, Offset};
use incular_semantics::{Role as SemanticRole, SemanticActionKind, SemanticState};
use incular_widgets::internal::{ActionSurface, ExplicitSemantics};
use incular_widgets::{Border, BoxDecoration, Container, Row, Text, Widget};
use typed_builder::TypedBuilder;

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

    pub(super) fn revision(&self) -> Rc<Cell<u64>> {
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

    pub(super) fn build_with_close(&self, close: Option<Rc<dyn Fn() + 'static>>) -> Widget {
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
                if close_on_activate && let Some(close) = close.as_ref() {
                    close();
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
