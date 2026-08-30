use std::rc::Rc;

use super::helpers::{blend_color, finite_non_negative, semantic_state};
use incular_config::{
    Alignment, Clip, CrossAxisAlignment, EdgeInsets, MainAxisAlignment, MainAxisSize,
};
use incular_controls::{ControlTheme, current_control_theme};
use incular_core::{Color, Offset};
use incular_semantics::{Role as SemanticRole, SemanticAction};
use incular_widgets::internal::ActionSurface;
use incular_widgets::{
    Align, BorderRadius, BoxDecoration, Column, Container, Padding, Row, Semantics, SizedBox, Text,
    Widget,
};
use typed_builder::TypedBuilder;

/// A Material dialog surface.
///
/// `Dialog` only describes the surface itself.  Use [`show_dialog`] and
/// [`DialogHandle::present`] when a retained overlay presentation is desired;
/// navigation and application-shell policy remain outside this type.
#[derive(Clone, TypedBuilder)]
pub struct Dialog {
    #[builder(setter(into))]
    child: Widget,
    #[builder(default, setter(strip_option))]
    background_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    surface_tint_color: Option<Color>,
    #[builder(default = 24.0, setter(transform = |value: f32| finite_non_negative(value)))]
    elevation: f32,
    #[builder(default = EdgeInsets::symmetric(40.0, 24.0))]
    inset_padding: EdgeInsets,
    #[builder(default = BorderRadius::circular(4.0))]
    shape: BorderRadius,
    #[builder(default = Alignment::CENTER)]
    alignment: Alignment,
    #[builder(default = Clip::None)]
    clip_behavior: Clip,
    #[builder(default, setter(strip_option, into))]
    semantic_label: Option<String>,
}

/// A Material alert dialog with the common title/content/actions slots.
///
/// The modal mechanics are intentionally provided by [`DialogHandle`]; this
/// type only composes the Material surface.  Keeping the slots as ordinary
/// widgets makes it possible to use custom controls without introducing a
/// second route or focus implementation.
#[derive(Clone, TypedBuilder)]
pub struct AlertDialog {
    #[builder(default, setter(strip_option, into))]
    title: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    content: Option<Widget>,
    #[builder(
        default = Vec::new(),
        setter(transform = |actions: impl IntoIterator<Item = impl Into<Widget>>| {
            actions.into_iter().map(Into::into).collect::<Vec<Widget>>()
        })
    )]
    actions: Vec<Widget>,
    #[builder(default)]
    scrollable: bool,
    #[builder(default, setter(strip_option))]
    background_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    surface_tint_color: Option<Color>,
    #[builder(default = 24.0, setter(transform = |value: f32| finite_non_negative(value)))]
    elevation: f32,
    #[builder(default = EdgeInsets::symmetric(40.0, 24.0))]
    inset_padding: EdgeInsets,
    #[builder(default, setter(strip_option))]
    title_padding: Option<EdgeInsets>,
    #[builder(default, setter(strip_option))]
    content_padding: Option<EdgeInsets>,
    #[builder(default = EdgeInsets::symmetric(24.0, 8.0))]
    actions_padding: EdgeInsets,
    #[builder(default = BorderRadius::circular(4.0))]
    shape: BorderRadius,
    #[builder(default = Alignment::CENTER)]
    alignment: Alignment,
    #[builder(default, setter(strip_option, into))]
    semantic_label: Option<String>,
}

impl Default for AlertDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl AlertDialog {
    #[must_use]
    pub fn new() -> Self {
        Self {
            title: None,
            content: None,
            actions: Vec::new(),
            scrollable: false,
            background_color: None,
            surface_tint_color: None,
            elevation: 24.0,
            inset_padding: EdgeInsets::symmetric(40.0, 24.0),
            title_padding: None,
            content_padding: None,
            actions_padding: EdgeInsets::symmetric(24.0, 8.0),
            shape: BorderRadius::circular(4.0),
            alignment: Alignment::CENTER,
            semantic_label: None,
        }
    }

    #[must_use]
    pub fn title(mut self, title: impl Into<Widget>) -> Self {
        self.title = Some(title.into());
        self
    }

    #[must_use]
    pub fn title_text(self, title: impl Into<String>) -> Self {
        self.title(Text::new(title))
    }

    #[must_use]
    pub fn content(mut self, content: impl Into<Widget>) -> Self {
        self.content = Some(content.into());
        self
    }

    #[must_use]
    pub fn actions(mut self, actions: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        self.actions = actions.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn action(mut self, action: impl Into<Widget>) -> Self {
        self.actions.push(action.into());
        self
    }

    #[must_use]
    pub fn scrollable(mut self, value: bool) -> Self {
        self.scrollable = value;
        self
    }

    #[must_use]
    pub fn background_color(mut self, value: Color) -> Self {
        self.background_color = Some(value);
        self
    }

    #[must_use]
    pub fn surface_tint_color(mut self, value: Color) -> Self {
        self.surface_tint_color = Some(value);
        self
    }

    #[must_use]
    pub fn elevation(mut self, value: f32) -> Self {
        self.elevation = finite_non_negative(value);
        self
    }

    #[must_use]
    pub fn inset_padding(mut self, value: EdgeInsets) -> Self {
        self.inset_padding = value;
        self
    }

    #[must_use]
    pub fn title_padding(mut self, value: EdgeInsets) -> Self {
        self.title_padding = Some(value);
        self
    }

    #[must_use]
    pub fn content_padding(mut self, value: EdgeInsets) -> Self {
        self.content_padding = Some(value);
        self
    }

    #[must_use]
    pub fn actions_padding(mut self, value: EdgeInsets) -> Self {
        self.actions_padding = value;
        self
    }

    #[must_use]
    pub fn shape(mut self, value: BorderRadius) -> Self {
        self.shape = value;
        self
    }

    #[must_use]
    pub fn alignment(mut self, value: Alignment) -> Self {
        self.alignment = value;
        self
    }

    #[must_use]
    pub fn semantic_label(mut self, value: impl Into<String>) -> Self {
        self.semantic_label = Some(value.into());
        self
    }

    fn build(&self, theme: &ControlTheme) -> Widget {
        let mut children: Vec<Widget> = Vec::with_capacity(3);
        if let Some(title) = self.title.clone() {
            children.push(
                Container::with_child(title)
                    .padding(
                        self.title_padding
                            .unwrap_or_else(|| EdgeInsets::only(24.0, 24.0, 24.0, 16.0)),
                    )
                    .into(),
            );
        }
        if let Some(content) = self.content.clone() {
            let content = if self.scrollable {
                incular_widgets::SingleChildScrollView::new(content).into()
            } else {
                content
            };
            children.push(
                Container::with_child(content)
                    .padding(
                        self.content_padding
                            .unwrap_or_else(|| EdgeInsets::only(24.0, 0.0, 24.0, 20.0)),
                    )
                    .into(),
            );
        }
        if !self.actions.is_empty() {
            children.push(
                Container::with_child(
                    Row::new(self.actions.clone())
                        .spacing(8.0)
                        .main_axis_alignment(MainAxisAlignment::End)
                        .cross_axis_alignment(CrossAxisAlignment::Center),
                )
                .padding(self.actions_padding)
                .into(),
            );
        }
        let content: Widget = Column::new(children)
            .main_axis_size(MainAxisSize::Min)
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .into();
        let mut dialog = Dialog::new(content)
            .background_color(
                self.background_color
                    .unwrap_or(theme.colors.surface_elevated),
            )
            .elevation(self.elevation)
            .inset_padding(self.inset_padding)
            .shape(self.shape)
            .alignment(self.alignment)
            .semantic_label(
                self.semantic_label
                    .clone()
                    .unwrap_or_else(|| "Alert dialog".to_owned()),
            );
        if let Some(tint) = self.surface_tint_color {
            dialog = dialog.surface_tint_color(tint);
        }
        dialog.build(theme)
    }
}

impl From<AlertDialog> for Widget {
    fn from(value: AlertDialog) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| value.build(&current_control_theme()))
    }
}

impl Dialog {
    /// Creates a dialog around `child`.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            background_color: None,
            surface_tint_color: None,
            elevation: 24.0,
            inset_padding: EdgeInsets::symmetric(40.0, 24.0),
            shape: BorderRadius::circular(4.0),
            alignment: Alignment::CENTER,
            clip_behavior: Clip::None,
            semantic_label: None,
        }
    }

    /// Creates a dialog with an empty child for incremental composition.
    #[must_use]
    pub fn empty() -> Self {
        Self::new(SizedBox::shrink())
    }

    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = child.into();
        self
    }

    #[must_use]
    pub fn background_color(mut self, color: Color) -> Self {
        self.background_color = Some(color);
        self
    }

    /// Alias for [`Dialog::background_color`].
    #[must_use]
    pub fn color(self, color: Color) -> Self {
        self.background_color(color)
    }

    #[must_use]
    pub fn surface_tint_color(mut self, color: Color) -> Self {
        self.surface_tint_color = Some(color);
        self
    }

    #[must_use]
    pub fn elevation(mut self, elevation: f32) -> Self {
        self.elevation = finite_non_negative(elevation);
        self
    }

    #[must_use]
    pub fn inset_padding(mut self, padding: EdgeInsets) -> Self {
        self.inset_padding = padding;
        self
    }

    #[must_use]
    pub fn shape(mut self, shape: BorderRadius) -> Self {
        self.shape = shape;
        self
    }

    #[must_use]
    pub fn radius(mut self, radius: f32) -> Self {
        self.shape = BorderRadius::circular(finite_non_negative(radius));
        self
    }

    #[must_use]
    pub fn alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }

    #[must_use]
    pub fn clip_behavior(mut self, clip_behavior: Clip) -> Self {
        self.clip_behavior = clip_behavior;
        self
    }

    #[must_use]
    pub fn semantic_label(mut self, label: impl Into<String>) -> Self {
        self.semantic_label = Some(label.into());
        self
    }

    /// Materializes this dialog using the current control theme.
    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        let mut surface_color = self
            .background_color
            .unwrap_or(theme.colors.surface_elevated);
        if let Some(tint) = self.surface_tint_color {
            // Material surface tint is intentionally subtle at the default
            // elevation. The blend stays renderer-independent and keeps the
            // descriptor useful to non-GPU hosts.
            surface_color = blend_color(surface_color, tint, (self.elevation / 24.0) * 0.12);
        }

        let surface: Widget = Container::with_child(self.child.clone())
            .color(surface_color)
            .decoration(BoxDecoration::new().border_radius(self.shape))
            .clip_behavior(self.clip_behavior)
            .into();
        let surface = if self.elevation > 0.0 {
            Widget::drop_shadow(
                Offset::new(0.0, self.elevation * 0.16),
                (self.elevation * 0.45).max(1.0),
                Color::rgba(0, 0, 0, 90),
                surface,
            )
        } else {
            surface
        };
        let surface = Padding::new(self.inset_padding, surface);
        let mut semantics = Semantics::new(surface)
            .role(SemanticRole::Dialog)
            .state(semantic_state(true))
            .action(SemanticAction::Focus);
        if let Some(label) = self.semantic_label.clone() {
            semantics = semantics.label(label);
        }
        Align::new(self.alignment, semantics).into()
    }
}

impl From<Dialog> for Widget {
    fn from(value: Dialog) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| value.build(&current_control_theme()))
    }
}

/// A selectable option used by [`SimpleDialog`].
#[derive(Clone, TypedBuilder)]
pub struct SimpleDialogOption {
    #[builder(setter(into))]
    child: Widget,
    #[builder(
        default,
        setter(
            fn transform<F>(callback: F) -> Option<Rc<dyn Fn()>>
            where
                F: Fn() + 'static,
            {
                Some(Rc::new(callback))
            }
        )
    )]
    on_pressed: Option<Rc<dyn Fn()>>,
    #[builder(default = true)]
    enabled: bool,
    #[builder(default, setter(strip_option, into))]
    semantic_label: Option<String>,
}

impl SimpleDialogOption {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            on_pressed: None,
            enabled: true,
            semantic_label: None,
        }
    }

    #[must_use]
    pub fn text(label: impl Into<String>) -> Self {
        Self::new(Text::new(label))
    }

    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = child.into();
        self
    }

    #[must_use]
    pub fn on_pressed(mut self, callback: impl Fn() + 'static) -> Self {
        self.on_pressed = Some(Rc::new(callback));
        self
    }

    /// Alias matching the naming used by the Flutter API.
    #[must_use]
    pub fn on_click(self, callback: impl Fn() + 'static) -> Self {
        self.on_pressed(callback)
    }

    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    #[must_use]
    pub fn semantic_label(mut self, label: impl Into<String>) -> Self {
        self.semantic_label = Some(label.into());
        self
    }

    fn build(&self, theme: &ControlTheme) -> Widget {
        let content: Widget = Container::with_child(self.child.clone())
            .height(48.0)
            .padding(EdgeInsets::symmetric(24.0, 0.0))
            .alignment(Alignment::CENTER_LEFT)
            .into();
        let mut surface = ActionSurface::with_child(content)
            .color(Color::TRANSPARENT)
            .hover_color(theme.colors.hover_overlay)
            .pressed_color(theme.colors.pressed_overlay)
            .focused_color(Color::TRANSPARENT)
            .disabled_color(Color::TRANSPARENT)
            .enabled(self.enabled);
        if let Some(callback) = self.on_pressed.clone() {
            if self.enabled {
                surface = surface.on_click(move || callback());
            }
        }
        let mut semantics = Semantics::new(surface)
            .role(SemanticRole::Button)
            .enabled(self.enabled)
            .action(SemanticAction::Activate);
        if let Some(label) = self.semantic_label.clone() {
            semantics = semantics.label(label);
        }
        semantics.into()
    }
}

impl From<SimpleDialogOption> for Widget {
    fn from(value: SimpleDialogOption) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| value.build(&current_control_theme()))
    }
}

/// A Material simple dialog made from a title and selectable options.
#[derive(Clone, TypedBuilder)]
pub struct SimpleDialog {
    #[builder(default, setter(strip_option, into))]
    title: Option<Widget>,
    #[builder(
        default = Vec::new(),
        setter(transform = |children: impl IntoIterator<Item = impl Into<Widget>>| {
            children.into_iter().map(Into::into).collect::<Vec<Widget>>()
        })
    )]
    children: Vec<Widget>,
    #[builder(default, setter(strip_option))]
    background_color: Option<Color>,
    #[builder(default = 24.0, setter(transform = |value: f32| finite_non_negative(value)))]
    elevation: f32,
    #[builder(default = EdgeInsets::symmetric(40.0, 24.0))]
    inset_padding: EdgeInsets,
    #[builder(default = EdgeInsets::symmetric(8.0, 12.0))]
    content_padding: EdgeInsets,
    #[builder(default = EdgeInsets::symmetric(24.0, 16.0))]
    title_padding: EdgeInsets,
    #[builder(default = BorderRadius::circular(4.0))]
    shape: BorderRadius,
    #[builder(default, setter(strip_option, into))]
    semantic_label: Option<String>,
}

impl Default for SimpleDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl SimpleDialog {
    #[must_use]
    pub fn new() -> Self {
        Self {
            title: None,
            children: Vec::new(),
            background_color: None,
            elevation: 24.0,
            inset_padding: EdgeInsets::symmetric(40.0, 24.0),
            content_padding: EdgeInsets::symmetric(8.0, 12.0),
            title_padding: EdgeInsets::symmetric(24.0, 16.0),
            shape: BorderRadius::circular(4.0),
            semantic_label: None,
        }
    }

    #[must_use]
    pub fn with_children(children: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        Self::new().children(children)
    }

    #[must_use]
    pub fn title(mut self, title: impl Into<Widget>) -> Self {
        self.title = Some(title.into());
        self
    }

    #[must_use]
    pub fn title_text(self, title: impl Into<String>) -> Self {
        self.title(Text::new(title))
    }

    #[must_use]
    pub fn children(mut self, children: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        self.children = children.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.children.push(child.into());
        self
    }

    #[must_use]
    pub fn option(self, option: SimpleDialogOption) -> Self {
        self.child(option)
    }

    #[must_use]
    pub fn background_color(mut self, color: Color) -> Self {
        self.background_color = Some(color);
        self
    }

    #[must_use]
    pub fn elevation(mut self, elevation: f32) -> Self {
        self.elevation = finite_non_negative(elevation);
        self
    }

    #[must_use]
    pub fn inset_padding(mut self, padding: EdgeInsets) -> Self {
        self.inset_padding = padding;
        self
    }

    #[must_use]
    pub fn content_padding(mut self, padding: EdgeInsets) -> Self {
        self.content_padding = padding;
        self
    }

    #[must_use]
    pub fn title_padding(mut self, padding: EdgeInsets) -> Self {
        self.title_padding = padding;
        self
    }

    #[must_use]
    pub fn shape(mut self, shape: BorderRadius) -> Self {
        self.shape = shape;
        self
    }

    #[must_use]
    pub fn semantic_label(mut self, label: impl Into<String>) -> Self {
        self.semantic_label = Some(label.into());
        self
    }

    fn build(&self, theme: &ControlTheme) -> Widget {
        let mut items: Vec<Widget> =
            Vec::with_capacity(self.children.len() + usize::from(self.title.is_some()));
        if let Some(title) = self.title.clone() {
            items.push(
                Container::with_child(title)
                    .padding(self.title_padding)
                    .into(),
            );
        }
        items.extend(self.children.iter().cloned());
        let content: Widget = Container::with_child(
            Column::new(items)
                .main_axis_size(MainAxisSize::Min)
                .cross_axis_alignment(CrossAxisAlignment::Stretch),
        )
        .padding(self.content_padding)
        .into();
        Dialog::new(content)
            .background_color(
                self.background_color
                    .unwrap_or(theme.colors.surface_elevated),
            )
            .elevation(self.elevation)
            .inset_padding(self.inset_padding)
            .shape(self.shape)
            .semantic_label(
                self.semantic_label
                    .clone()
                    .unwrap_or_else(|| "Simple dialog".to_owned()),
            )
            .build(theme)
    }
}

impl From<SimpleDialog> for Widget {
    fn from(value: SimpleDialog) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| value.build(&current_control_theme()))
    }
}
