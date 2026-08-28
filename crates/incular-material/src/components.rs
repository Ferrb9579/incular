//! Material-shaped component compositions.
//!
//! This module is intentionally a thin layer over Incular's retained widget
//! and controls crates.  It owns Material vocabulary and composition policy,
//! while layout, input dispatch, retained state, and painting remain in the
//! lower layers.  The module is kept separate from `lib.rs` so the public
//! re-export policy can evolve without changing the widget primitives.

use std::{rc::Rc, sync::Arc};

use crate::feedback::current_progress_indicator_theme;
use crate::foundation::{Material, Theme};
use crate::{
    BottomNavigationBarType, K_BOTTOM_NAVIGATION_BAR_HEIGHT, ListTileTitleAlignment,
    NavigationDestinationLabelBehavior, TextButton,
};
use incular_config::{
    Alignment, Clip, Constraints, CrossAxisAlignment, EdgeInsets, MainAxisAlignment, MainAxisSize,
};
use incular_controls::{
    Checkbox, CheckedState, ControlIcon, ControlTheme, Radio, Switch, current_control_theme,
};
use incular_core::{Color, Offset, Size};
use incular_rendering::{Canvas, LineCap, LineJoin, Path, Stroke};
use incular_text::TextStyle;
use incular_widgets::internal::{ActionSurface, Expanded};
use incular_widgets::{
    Border, BorderRadius, BoxDecoration, Column, ConstrainedBox, Container, DefaultTextStyle,
    GestureDetector, HitTestBehavior, IconTheme, Positioned, Row, Semantics, SizedBox, Stack, Text,
    Widget,
};
use typed_builder::TypedBuilder;

/// Alias used by callers that want a generic surface vocabulary rather than
/// Flutter's `Material` name.
pub type Surface = Material;

/// A Material top app bar composed from a title, optional leading control, and
/// trailing action widgets.
#[derive(Clone, TypedBuilder)]
pub struct AppBar {
    #[builder(setter(into))]
    title: Widget,
    #[builder(default, setter(strip_option, into))]
    leading: Option<Widget>,
    #[builder(
        default = Vec::new(),
        setter(transform = |actions: impl IntoIterator<Item = impl Into<Widget>>| {
            actions.into_iter().map(Into::into).collect::<Vec<Widget>>()
        })
    )]
    actions: Vec<Widget>,
    #[builder(default, setter(strip_option, into))]
    bottom: Option<Widget>,
    #[builder(default, setter(strip_option))]
    background: Option<Color>,
    #[builder(default, setter(strip_option))]
    foreground: Option<Color>,
    #[builder(default = 0.0, setter(transform = |value: f32| finite_non_negative(value)))]
    elevation: f32,
    #[builder(default = 64.0, setter(transform = |value: f32| finite_non_negative(value).max(1.0)))]
    toolbar_height: f32,
    #[builder(default = true)]
    automatically_imply_leading: bool,
    #[builder(default)]
    center_title: bool,
    #[builder(default, setter(transform = |value: f32| Some(value.max(0.0))))]
    title_spacing: Option<f32>,
    #[builder(default, setter(transform = |value: f32| Some(value.max(0.0))))]
    leading_width: Option<f32>,
    #[builder(default, setter(strip_option, into))]
    flexible_space: Option<Widget>,
    #[builder(default, setter(strip_option))]
    shadow_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    surface_tint_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    shape: Option<BorderRadius>,
}

impl AppBar {
    #[must_use]
    pub fn new(title: impl Into<Widget>) -> Self {
        Self {
            title: title.into(),
            leading: None,
            actions: Vec::new(),
            bottom: None,
            background: None,
            foreground: None,
            elevation: 0.0,
            toolbar_height: 64.0,
            automatically_imply_leading: true,
            center_title: false,
            title_spacing: None,
            leading_width: None,
            flexible_space: None,
            shadow_color: None,
            surface_tint_color: None,
            shape: None,
        }
    }

    #[must_use]
    pub fn leading(mut self, child: impl Into<Widget>) -> Self {
        self.leading = Some(child.into());
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
    pub fn bottom(mut self, bottom: impl Into<Widget>) -> Self {
        self.bottom = Some(bottom.into());
        self
    }

    #[must_use]
    pub fn background_color(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    #[must_use]
    pub fn foreground_color(mut self, color: Color) -> Self {
        self.foreground = Some(color);
        self
    }

    #[must_use]
    pub fn elevation(mut self, elevation: f32) -> Self {
        self.elevation = finite_non_negative(elevation);
        self
    }

    #[must_use]
    pub fn toolbar_height(mut self, height: f32) -> Self {
        self.toolbar_height = finite_non_negative(height).max(1.0);
        self
    }

    #[must_use]
    pub fn automatically_imply_leading(mut self, value: bool) -> Self {
        self.automatically_imply_leading = value;
        self
    }

    #[must_use]
    pub fn center_title(mut self, value: bool) -> Self {
        self.center_title = value;
        self
    }

    #[must_use]
    pub fn title_spacing(mut self, value: f32) -> Self {
        self.title_spacing = Some(value.max(0.0));
        self
    }

    #[must_use]
    pub fn leading_width(mut self, value: f32) -> Self {
        self.leading_width = Some(value.max(0.0));
        self
    }

    #[must_use]
    pub fn flexible_space(mut self, value: impl Into<Widget>) -> Self {
        self.flexible_space = Some(value.into());
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
    pub fn shape(mut self, value: BorderRadius) -> Self {
        self.shape = Some(value);
        self
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        let background = self.background.unwrap_or(theme.colors.surface);
        let foreground = self.foreground.unwrap_or(theme.colors.foreground);
        let mut row_children = Vec::with_capacity(2 + self.actions.len());
        if let Some(leading) = self.leading.clone() {
            row_children.push(leading);
        } else if self.automatically_imply_leading {
            row_children.push(
                SizedBox::new()
                    .width(self.leading_width.unwrap_or(0.0))
                    .into(),
            );
        }
        row_children.push(self.title.clone());
        row_children.extend(self.actions.clone());
        let spacing = self.title_spacing.unwrap_or(16.0);
        let row = Row::new(row_children)
            .spacing(spacing)
            .main_axis_alignment(if self.center_title {
                MainAxisAlignment::Center
            } else {
                MainAxisAlignment::SpaceBetween
            })
            .cross_axis_alignment(CrossAxisAlignment::Center);
        let row: Widget = row.into();
        let toolbar_child: Widget = if let Some(flexible) = self.flexible_space.clone() {
            // Flexible space is a background layer. Keeping the toolbar row
            // above it prevents the common accidental child replacement that
            // used to make titles/actions disappear when this slot was set.
            Stack::aligned(Alignment::TOP_LEFT, [flexible, row]).into()
        } else {
            row
        };
        let toolbar = Container::new()
            .height(self.toolbar_height)
            .padding(EdgeInsets::symmetric(16.0, 0.0))
            .color(background)
            .child(toolbar_child);
        let toolbar: Widget = Material::new(toolbar)
            .color(background)
            .shadow_color(self.shadow_color.unwrap_or(Color::rgba(0, 0, 0, 80)))
            .surface_tint_color(self.surface_tint_color.unwrap_or(Color::TRANSPARENT))
            .elevation(self.elevation)
            .border_radius(self.shape.unwrap_or(BorderRadius::ZERO))
            .into();
        // Carry AppBar foreground policy through the normal inherited text and
        // icon environments so title/action descendants resolve the same
        // color without each action being rebuilt or recolored individually.
        let toolbar = IconTheme::new(toolbar).color(foreground);
        let toolbar: Widget =
            DefaultTextStyle::new(TextStyle::default().color(foreground), toolbar).into();
        if let Some(bottom) = self.bottom.clone() {
            Column::new([toolbar, bottom])
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .into()
        } else {
            toolbar
        }
    }
}

impl From<AppBar> for Widget {
    fn from(value: AppBar) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| value.build(&current_control_theme()))
    }
}

/// Scroll-aware app bar descriptor. The retained sliver infrastructure owns
/// scroll offsets; this Material wrapper keeps the Flutter flag vocabulary and
/// resolves the currently visible toolbar without introducing a second scroll
/// engine.
#[derive(Clone, TypedBuilder)]
pub struct SliverAppBar {
    #[builder(default = AppBar::new(Text::new("")))]
    app_bar: AppBar,
    #[builder(default)]
    pinned: bool,
    #[builder(default)]
    floating: bool,
    #[builder(default)]
    snap: bool,
    #[builder(default)]
    stretch: bool,
    #[builder(default, setter(transform = |value: f32| Some(value.max(0.0))))]
    expanded_height: Option<f32>,
    #[builder(default, setter(transform = |value: f32| Some(value.max(0.0))))]
    collapsed_height: Option<f32>,
}

impl SliverAppBar {
    #[must_use]
    pub fn new() -> Self {
        Self {
            app_bar: AppBar::new(Text::new("")),
            pinned: false,
            floating: false,
            snap: false,
            stretch: false,
            expanded_height: None,
            collapsed_height: None,
        }
    }

    #[must_use]
    pub fn from_app_bar(app_bar: AppBar) -> Self {
        Self {
            app_bar,
            ..Self::new()
        }
    }

    #[must_use]
    pub fn title(mut self, title: impl Into<Widget>) -> Self {
        self.app_bar = AppBar::new(title)
            .toolbar_height(self.app_bar.toolbar_height)
            .elevation(self.app_bar.elevation);
        self
    }

    #[must_use]
    pub fn app_bar(mut self, app_bar: AppBar) -> Self {
        self.app_bar = app_bar;
        self
    }

    #[must_use]
    pub fn pinned(mut self, value: bool) -> Self {
        self.pinned = value;
        self
    }

    #[must_use]
    pub fn floating(mut self, value: bool) -> Self {
        self.floating = value;
        self
    }

    #[must_use]
    pub fn snap(mut self, value: bool) -> Self {
        self.snap = value;
        self
    }

    #[must_use]
    pub fn stretch(mut self, value: bool) -> Self {
        self.stretch = value;
        self
    }

    #[must_use]
    pub fn expanded_height(mut self, value: f32) -> Self {
        self.expanded_height = Some(value.max(0.0));
        self
    }

    #[must_use]
    pub fn collapsed_height(mut self, value: f32) -> Self {
        self.collapsed_height = Some(value.max(0.0));
        self
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        let height = self
            .collapsed_height
            .or(self.expanded_height)
            .unwrap_or(self.app_bar.toolbar_height);
        let app_bar = self.app_bar.clone().toolbar_height(height);
        let _ = (self.pinned, self.floating, self.snap, self.stretch);
        app_bar.build(theme)
    }
}

impl Default for SliverAppBar {
    fn default() -> Self {
        Self::new()
    }
}

impl From<SliverAppBar> for Widget {
    fn from(value: SliverAppBar) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| value.build(&current_control_theme()))
    }
}

/// A retained page scaffold with app bar, body, bottom navigation, and FAB
/// slots.  Navigation/routing remains owned by `incular-navigation`.
#[derive(Clone, TypedBuilder)]
pub struct Scaffold {
    #[builder(setter(into))]
    body: Widget,
    #[builder(default, setter(strip_option))]
    app_bar: Option<AppBar>,
    #[builder(default, setter(strip_option, into))]
    bottom_navigation_bar: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    floating_action_button: Option<Widget>,
    #[builder(default, setter(strip_option))]
    background: Option<Color>,
    #[builder(default, setter(strip_option, into))]
    drawer: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    end_drawer: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    bottom_app_bar: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    bottom_sheet: Option<Widget>,
    #[builder(default = true)]
    resize_to_avoid_bottom_inset: bool,
    #[builder(default)]
    extend_body: bool,
    #[builder(default)]
    extend_body_behind_app_bar: bool,
}

impl Scaffold {
    #[must_use]
    pub fn new(body: impl Into<Widget>) -> Self {
        Self {
            body: body.into(),
            app_bar: None,
            bottom_navigation_bar: None,
            floating_action_button: None,
            background: None,
            drawer: None,
            end_drawer: None,
            bottom_app_bar: None,
            bottom_sheet: None,
            resize_to_avoid_bottom_inset: true,
            extend_body: false,
            extend_body_behind_app_bar: false,
        }
    }

    #[must_use]
    pub fn app_bar(mut self, app_bar: AppBar) -> Self {
        self.app_bar = Some(app_bar);
        self
    }

    #[must_use]
    pub fn bottom_navigation_bar(mut self, bar: impl Into<Widget>) -> Self {
        self.bottom_navigation_bar = Some(bar.into());
        self
    }

    #[must_use]
    pub fn floating_action_button(mut self, button: impl Into<Widget>) -> Self {
        self.floating_action_button = Some(button.into());
        self
    }

    #[must_use]
    pub fn drawer(mut self, drawer: impl Into<Widget>) -> Self {
        self.drawer = Some(drawer.into());
        self
    }

    #[must_use]
    pub fn end_drawer(mut self, drawer: impl Into<Widget>) -> Self {
        self.end_drawer = Some(drawer.into());
        self
    }

    #[must_use]
    pub fn bottom_app_bar(mut self, bar: impl Into<Widget>) -> Self {
        self.bottom_app_bar = Some(bar.into());
        self
    }

    #[must_use]
    pub fn bottom_sheet(mut self, sheet: impl Into<Widget>) -> Self {
        self.bottom_sheet = Some(sheet.into());
        self
    }

    #[must_use]
    pub fn resize_to_avoid_bottom_inset(mut self, value: bool) -> Self {
        self.resize_to_avoid_bottom_inset = value;
        self
    }

    #[must_use]
    pub fn extend_body(mut self, value: bool) -> Self {
        self.extend_body = value;
        self
    }

    #[must_use]
    pub fn extend_body_behind_app_bar(mut self, value: bool) -> Self {
        self.extend_body_behind_app_bar = value;
        self
    }

    #[must_use]
    pub fn background_color(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        let mut children = Vec::new();
        if let Some(app_bar) = &self.app_bar {
            children.push(app_bar.build(theme));
        }
        let scaffold_background = Theme::of_shared().map_or(theme.colors.background, |theme| {
            theme.scaffold_background_color
        });
        let body = Material::new(Expanded::new(self.body.clone()))
            .color(self.background.unwrap_or(scaffold_background));
        children.push(body.into());
        if let Some(sheet) = self.bottom_sheet.clone() {
            children.push(sheet);
        }
        if let Some(bottom) = self
            .bottom_navigation_bar
            .clone()
            .or_else(|| self.bottom_app_bar.clone())
        {
            children.push(bottom);
        }
        let content: Widget = Column::new(children)
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .into();
        let mut stack_children = vec![content];
        if let Some(fab) = self.floating_action_button.clone() {
            stack_children.push(
                Positioned::new(fab)
                    .right(16.0)
                    .bottom(
                        if self.bottom_navigation_bar.is_some() || self.bottom_app_bar.is_some() {
                            96.0
                        } else {
                            16.0
                        },
                    )
                    .into(),
            );
        }
        if let Some(drawer) = self.drawer.clone() {
            stack_children.push(
                Positioned::new(drawer)
                    .left(0.0)
                    .top(0.0)
                    .bottom(0.0)
                    .into(),
            );
        }
        if let Some(drawer) = self.end_drawer.clone() {
            stack_children.push(
                Positioned::new(drawer)
                    .right(0.0)
                    .top(0.0)
                    .bottom(0.0)
                    .into(),
            );
        }
        let _ = (
            self.resize_to_avoid_bottom_inset,
            self.extend_body,
            self.extend_body_behind_app_bar,
        );
        Stack::aligned(Alignment::TOP_LEFT, stack_children).into()
    }
}

impl From<Scaffold> for Widget {
    fn from(value: Scaffold) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| value.build(&current_control_theme()))
    }
}

/// A Material circular avatar.
#[derive(Clone, TypedBuilder)]
pub struct CircleAvatar {
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
    #[builder(default, setter(strip_option))]
    background: Option<Color>,
    #[builder(default, setter(strip_option))]
    foreground: Option<Color>,
    #[builder(default = 20.0, setter(transform = |value: f32| finite_non_negative(value)))]
    radius: f32,
}

impl CircleAvatar {
    #[must_use]
    pub fn new() -> Self {
        Self {
            child: None,
            background: None,
            foreground: None,
            radius: 20.0,
        }
    }

    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self
    }

    #[must_use]
    pub fn background_color(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    #[must_use]
    pub fn foreground_color(mut self, color: Color) -> Self {
        self.foreground = Some(color);
        self
    }

    #[must_use]
    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = finite_non_negative(radius);
        self
    }
}

impl Default for CircleAvatar {
    fn default() -> Self {
        Self::new()
    }
}

impl From<CircleAvatar> for Widget {
    fn from(value: CircleAvatar) -> Self {
        let child = value.child.unwrap_or_else(|| SizedBox::shrink().into());
        Container::new()
            .width(value.radius * 2.0)
            .height(value.radius * 2.0)
            .alignment(Alignment::CENTER)
            .color(value.background.unwrap_or(Color::rgba(120, 120, 120, 255)))
            .radius(value.radius)
            .child(child)
            .into()
    }
}

/// One destination in a [`BottomNavigationBar`].
#[derive(Clone, TypedBuilder)]
pub struct BottomNavigationBarItem {
    #[builder(setter(into))]
    pub icon: Widget,
    #[builder(setter(into))]
    pub label: String,
    #[builder(default, setter(strip_option, into))]
    pub active_icon: Option<Widget>,
    #[builder(default, setter(strip_option))]
    pub background_color: Option<Color>,
    #[builder(default, setter(strip_option, into))]
    pub tooltip: Option<String>,
}

impl BottomNavigationBarItem {
    #[must_use]
    pub fn new(icon: impl Into<Widget>, label: impl Into<String>) -> Self {
        Self {
            icon: icon.into(),
            label: label.into(),
            active_icon: None,
            background_color: None,
            tooltip: None,
        }
    }

    #[must_use]
    pub fn active_icon(mut self, icon: impl Into<Widget>) -> Self {
        self.active_icon = Some(icon.into());
        self
    }

    #[must_use]
    pub fn background_color(mut self, color: Color) -> Self {
        self.background_color = Some(color);
        self
    }

    #[must_use]
    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }
}

/// A Material bottom navigation bar with controlled selection.
#[derive(Clone, TypedBuilder)]
pub struct BottomNavigationBar {
    #[builder(
        default = Vec::new(),
        setter(transform = |items: impl IntoIterator<Item = BottomNavigationBarItem>| {
            items.into_iter().collect::<Vec<_>>()
        })
    )]
    items: Vec<BottomNavigationBarItem>,
    #[builder(default)]
    current_index: usize,
    #[builder(default = BottomNavigationBarType::Fixed)]
    bar_type: BottomNavigationBarType,
    #[builder(
        default,
        setter(
            fn transform<F>(callback: F) -> Option<Rc<dyn Fn(usize)>>
            where
                F: Fn(usize) + 'static,
            {
                Some(Rc::new(callback))
            }
        )
    )]
    on_tap: Option<Rc<dyn Fn(usize)>>,
    #[builder(default, setter(strip_option))]
    background: Option<Color>,
}

impl BottomNavigationBar {
    #[must_use]
    pub fn new(items: impl IntoIterator<Item = BottomNavigationBarItem>) -> Self {
        Self {
            items: items.into_iter().collect(),
            current_index: 0,
            bar_type: BottomNavigationBarType::Fixed,
            on_tap: None,
            background: None,
        }
    }

    #[must_use]
    pub fn current_index(mut self, index: usize) -> Self {
        self.current_index = index;
        self
    }

    #[must_use]
    pub fn bar_type(mut self, bar_type: BottomNavigationBarType) -> Self {
        self.bar_type = bar_type;
        self
    }

    #[must_use]
    pub fn on_tap(mut self, callback: impl Fn(usize) + 'static) -> Self {
        self.on_tap = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn background_color(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        let selected = self.current_index.min(self.items.len().saturating_sub(1));
        let accent = theme.colors.accent;
        let children = self.items.iter().enumerate().map(|(index, item)| {
            let icon = if index == selected {
                item.active_icon
                    .clone()
                    .unwrap_or_else(|| item.icon.clone())
            } else {
                item.icon.clone()
            };
            let label = Text::new(item.label.clone()).color(if index == selected {
                accent
            } else {
                theme.colors.foreground_muted
            });
            let child: Widget = Column::new([icon, label.into()])
                .spacing(2.0)
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .into();
            let mut button = TextButton::with_child(child);
            if let Some(on_tap) = self.on_tap.clone() {
                button = button.on_click(move || on_tap(index));
            }
            Widget::from(button)
        });
        let row = Row::new(children)
            .spacing(
                if matches!(self.bar_type, BottomNavigationBarType::Shifting) {
                    4.0
                } else {
                    12.0
                },
            )
            .main_axis_alignment(MainAxisAlignment::SpaceEvenly)
            .cross_axis_alignment(CrossAxisAlignment::Center);
        Container::new()
            .height(K_BOTTOM_NAVIGATION_BAR_HEIGHT)
            .color(self.background.unwrap_or(theme.colors.surface))
            .child(row)
            .into()
    }
}

impl From<BottomNavigationBar> for Widget {
    fn from(value: BottomNavigationBar) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| value.build(&current_control_theme()))
    }
}

/// A Material 3 navigation destination.
#[derive(Clone, TypedBuilder)]
pub struct NavigationDestination {
    #[builder(setter(into))]
    pub icon: Widget,
    #[builder(setter(into))]
    pub label: String,
    #[builder(default, setter(strip_option, into))]
    pub selected_icon: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    pub tooltip: Option<String>,
    /// Whether this destination accepts activation and participates in normal
    /// focus traversal.  Flutter's destination defaults to enabled.
    #[builder(default = true)]
    pub enabled: bool,
}

impl NavigationDestination {
    #[must_use]
    pub fn new(icon: impl Into<Widget>, label: impl Into<String>) -> Self {
        Self {
            icon: icon.into(),
            label: label.into(),
            selected_icon: None,
            tooltip: None,
            enabled: true,
        }
    }

    #[must_use]
    pub fn selected_icon(mut self, icon: impl Into<Widget>) -> Self {
        self.selected_icon = Some(icon.into());
        self
    }

    #[must_use]
    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
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
}

/// Material 3 navigation bar.  Selection is controlled by the application in
/// the same way as Flutter's `NavigationBar.selectedIndex`.
#[derive(Clone, TypedBuilder)]
pub struct NavigationBar {
    #[builder(
        default = Vec::new(),
        setter(transform = |destinations: impl IntoIterator<Item = NavigationDestination>| {
            destinations.into_iter().collect::<Vec<_>>()
        })
    )]
    destinations: Vec<NavigationDestination>,
    #[builder(default)]
    selected_index: usize,
    #[builder(default = NavigationDestinationLabelBehavior::AlwaysShow)]
    label_behavior: NavigationDestinationLabelBehavior,
    #[builder(
        default,
        setter(
            fn transform<F>(callback: F) -> Option<Rc<dyn Fn(usize)>>
            where
                F: Fn(usize) + 'static,
            {
                Some(Rc::new(callback))
            }
        )
    )]
    on_destination_selected: Option<Rc<dyn Fn(usize)>>,
}

impl NavigationBar {
    #[must_use]
    pub fn new(destinations: impl IntoIterator<Item = NavigationDestination>) -> Self {
        Self {
            destinations: destinations.into_iter().collect(),
            selected_index: 0,
            label_behavior: NavigationDestinationLabelBehavior::AlwaysShow,
            on_destination_selected: None,
        }
    }

    #[must_use]
    pub fn selected_index(mut self, index: usize) -> Self {
        self.selected_index = index;
        self
    }

    #[must_use]
    pub fn label_behavior(mut self, behavior: NavigationDestinationLabelBehavior) -> Self {
        self.label_behavior = behavior;
        self
    }

    #[must_use]
    pub fn on_destination_selected(mut self, callback: impl Fn(usize) + 'static) -> Self {
        self.on_destination_selected = Some(Rc::new(callback));
        self
    }
}

impl From<NavigationBar> for Widget {
    fn from(value: NavigationBar) -> Self {
        let destinations = value.destinations;
        let selected = value
            .selected_index
            .min(destinations.len().saturating_sub(1));
        let behavior = value.label_behavior;
        let callback = value.on_destination_selected;
        let children = destinations.into_iter().enumerate().map(|(index, item)| {
            let icon = if index == selected {
                item.selected_icon.unwrap_or(item.icon)
            } else {
                item.icon
            };
            let show_label = matches!(behavior, NavigationDestinationLabelBehavior::AlwaysShow)
                || (matches!(
                    behavior,
                    NavigationDestinationLabelBehavior::OnlyShowSelected
                ) && index == selected);
            let child: Widget = if show_label {
                Column::new([icon, Text::new(item.label).into()])
                    .spacing(2.0)
                    .into()
            } else {
                icon
            };
            let mut button = TextButton::with_child(child).enabled(item.enabled);
            if item.enabled {
                if let Some(callback) = callback.clone() {
                    button = button.on_click(move || callback(index));
                }
            }
            Widget::from(button)
        });
        Container::new()
            .height(80.0)
            .color(current_control_theme().colors.surface)
            .child(Row::new(children).main_axis_alignment(MainAxisAlignment::SpaceEvenly))
            .into()
    }
}

/// Material card composition.
#[derive(Clone, TypedBuilder)]
pub struct Card {
    #[builder(setter(into))]
    child: Widget,
    #[builder(default, setter(strip_option))]
    color: Option<Color>,
    #[builder(default, setter(strip_option))]
    shadow_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    surface_tint_color: Option<Color>,
    #[builder(default = 1.0, setter(transform = |value: f32| finite_non_negative(value)))]
    elevation: f32,
    #[builder(default, setter(transform = |value: f32| Some(finite_non_negative(value))))]
    radius: Option<f32>,
    #[builder(default, setter(strip_option))]
    shape: Option<BorderRadius>,
    #[builder(default = Some(EdgeInsets::all(16.0)))]
    padding: Option<EdgeInsets>,
    #[builder(default, setter(strip_option))]
    margin: Option<EdgeInsets>,
    #[builder(default, setter(strip_option))]
    border: Option<Border>,
    #[builder(default = Clip::None)]
    clip_behavior: Clip,
    #[builder(default)]
    border_on_foreground: bool,
    #[builder(default = true)]
    semantic_container: bool,
}

impl Card {
    /// Creates a card around `child`.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            color: None,
            shadow_color: None,
            surface_tint_color: None,
            elevation: 1.0,
            radius: None,
            shape: None,
            padding: Some(EdgeInsets::all(16.0)),
            margin: None,
            border: None,
            clip_behavior: Clip::None,
            border_on_foreground: false,
            semantic_container: true,
        }
    }

    /// Material filled-card defaults (no elevation, tonal surface).
    #[must_use]
    pub fn filled(child: impl Into<Widget>) -> Self {
        Self::new(child)
            .elevation(0.0)
            .color(Color::rgba(245, 240, 247, 255))
    }

    /// Material elevated-card defaults.
    #[must_use]
    pub fn elevated(child: impl Into<Widget>) -> Self {
        Self::new(child).elevation(1.0)
    }

    /// Material outlined-card defaults.
    #[must_use]
    pub fn outlined(child: impl Into<Widget>) -> Self {
        Self::new(child)
            .elevation(0.0)
            .border(Border::new(1.0, Color::rgba(121, 116, 126, 255)))
    }

    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    #[must_use]
    pub fn shadow_color(mut self, color: Color) -> Self {
        self.shadow_color = Some(color);
        self
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
    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = Some(finite_non_negative(radius));
        self.shape = None;
        self
    }

    #[must_use]
    pub fn shape(mut self, shape: BorderRadius) -> Self {
        self.shape = Some(shape);
        self.radius = None;
        self
    }

    #[must_use]
    pub fn padding(mut self, padding: EdgeInsets) -> Self {
        self.padding = Some(padding);
        self
    }

    #[must_use]
    pub fn margin(mut self, margin: EdgeInsets) -> Self {
        self.margin = Some(margin);
        self
    }

    #[must_use]
    pub fn border(mut self, border: Border) -> Self {
        self.border = Some(border);
        self
    }

    #[must_use]
    pub fn clip_behavior(mut self, clip_behavior: Clip) -> Self {
        self.clip_behavior = clip_behavior;
        self
    }

    #[must_use]
    pub fn border_on_foreground(mut self, value: bool) -> Self {
        self.border_on_foreground = value;
        self
    }

    #[must_use]
    pub fn semantic_container(mut self, value: bool) -> Self {
        self.semantic_container = value;
        self
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        let mut material =
            Material::new(self.child.clone())
                .color(self.color.unwrap_or(theme.colors.surface))
                .elevation(self.elevation)
                .shadow_color(self.shadow_color.unwrap_or(Color::rgba(0, 0, 0, 100)))
                .border_radius(self.shape.unwrap_or_else(|| {
                    BorderRadius::circular(self.radius.unwrap_or(theme.radius.md))
                }))
                .clip_behavior(self.clip_behavior);
        if let Some(tint) = self.surface_tint_color {
            material = material.surface_tint_color(tint);
        }
        if let Some(padding) = self.padding {
            material = material.padding(padding);
        }
        let mut surface = Container::with_child(material);
        if let Some(margin) = self.margin {
            surface = surface.margin(margin);
        }
        if let Some(border) = self.border {
            surface = surface.border(border);
        }
        // `border_on_foreground` and `semantic_container` are retained as
        // explicit Flutter-shaped policy knobs. The retained Material surface
        // already clips/paints the border in the same layer for both modes.
        let _ = (self.border_on_foreground, self.semantic_container);
        surface.into()
    }
}

impl From<Card> for Widget {
    fn from(value: Card) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| value.build(&current_control_theme()))
    }
}

/// A horizontal Material divider.
#[derive(Clone, TypedBuilder)]
pub struct Divider {
    #[builder(default, setter(strip_option))]
    color: Option<Color>,
    #[builder(default = 1.0, setter(transform = |value: f32| finite_non_negative(value)))]
    thickness: f32,
    #[builder(default, setter(transform = |value: f32| finite_non_negative(value)))]
    indent: f32,
    #[builder(default, setter(transform = |value: f32| finite_non_negative(value)))]
    end_indent: f32,
}

impl Divider {
    #[must_use]
    pub fn new() -> Self {
        Self {
            color: None,
            thickness: 1.0,
            indent: 0.0,
            end_indent: 0.0,
        }
    }

    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    #[must_use]
    pub fn thickness(mut self, thickness: f32) -> Self {
        self.thickness = finite_non_negative(thickness);
        self
    }

    #[must_use]
    pub fn indent(mut self, indent: f32) -> Self {
        self.indent = finite_non_negative(indent);
        self
    }

    #[must_use]
    pub fn end_indent(mut self, end_indent: f32) -> Self {
        self.end_indent = finite_non_negative(end_indent);
        self
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        Container::new()
            .height(self.thickness.max(0.1))
            .margin(EdgeInsets::only(self.indent, 0.0, self.end_indent, 0.0))
            .color(self.color.unwrap_or(theme.colors.border))
            .into()
    }
}

impl Default for Divider {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Divider> for Widget {
    fn from(value: Divider) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| value.build(&current_control_theme()))
    }
}

/// A vertical Material divider for rows and navigation rails.
#[derive(Clone, TypedBuilder)]
pub struct VerticalDivider {
    #[builder(default, setter(strip_option))]
    color: Option<Color>,
    #[builder(default = 1.0, setter(transform = |value: f32| finite_non_negative(value)))]
    thickness: f32,
    #[builder(default, setter(transform = |value: f32| finite_non_negative(value)))]
    indent: f32,
    #[builder(default, setter(transform = |value: f32| finite_non_negative(value)))]
    end_indent: f32,
}

impl VerticalDivider {
    #[must_use]
    pub fn new() -> Self {
        Self {
            color: None,
            thickness: 1.0,
            indent: 0.0,
            end_indent: 0.0,
        }
    }

    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    #[must_use]
    pub fn thickness(mut self, thickness: f32) -> Self {
        self.thickness = finite_non_negative(thickness);
        self
    }

    #[must_use]
    pub fn indent(mut self, indent: f32) -> Self {
        self.indent = finite_non_negative(indent);
        self
    }

    #[must_use]
    pub fn end_indent(mut self, end_indent: f32) -> Self {
        self.end_indent = finite_non_negative(end_indent);
        self
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        Container::new()
            .width(self.thickness.max(0.1))
            .margin(EdgeInsets::only(0.0, self.indent, 0.0, self.end_indent))
            .color(self.color.unwrap_or(theme.colors.border))
            .into()
    }
}

impl Default for VerticalDivider {
    fn default() -> Self {
        Self::new()
    }
}

impl From<VerticalDivider> for Widget {
    fn from(value: VerticalDivider) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| value.build(&current_control_theme()))
    }
}

/// Material linear progress indicator.
///
/// `None` is represented as an indeterminate indicator.  The actual retained
/// track/indicator composition is shared with the controls crate so value
/// clamping, sizing, and accessibility remain consistent with Incular's
/// existing progress implementation.
#[derive(Clone, TypedBuilder)]
pub struct LinearProgressIndicator {
    #[builder(default, setter(strip_option))]
    value: Option<f32>,
    #[builder(default = 0.0)]
    min: f32,
    #[builder(default = 1.0)]
    max: f32,
    #[builder(default = 180.0, setter(transform = |value: f32| finite_non_negative(value)))]
    width: f32,
    #[builder(default, setter(transform = |value: f32| Some(finite_non_negative(value))))]
    height: Option<f32>,
    #[builder(default, setter(strip_option, into))]
    label: Option<String>,
}

impl Default for LinearProgressIndicator {
    fn default() -> Self {
        Self::new()
    }
}

impl LinearProgressIndicator {
    /// Creates an indeterminate indicator, matching Flutter's default.
    #[must_use]
    pub fn new() -> Self {
        Self {
            value: None,
            min: 0.0,
            max: 1.0,
            width: 180.0,
            height: None,
            label: None,
        }
    }

    /// Sets a determinate value.
    #[must_use]
    pub fn value(mut self, value: f32) -> Self {
        self.value = Some(value);
        self
    }

    /// Sets the indicator to indeterminate mode.
    #[must_use]
    pub fn indeterminate(mut self, value: bool) -> Self {
        if value {
            self.value = None;
        } else if self.value.is_none() {
            self.value = Some(self.min);
        }
        self
    }

    /// Sets the value directly, with `None` meaning indeterminate.
    #[must_use]
    pub fn value_option(mut self, value: Option<f32>) -> Self {
        self.value = value;
        self
    }

    #[must_use]
    pub fn range(mut self, min: f32, max: f32) -> Self {
        if min.is_finite() && max.is_finite() {
            self.min = min.min(max);
            self.max = max.max(min);
        }
        self
    }

    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        self.width = finite_non_negative(width);
        self
    }

    #[must_use]
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(finite_non_negative(height));
        self
    }

    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        let indicator_theme = current_progress_indicator_theme().unwrap_or_default();
        let mut controls_theme = theme.clone();
        if let Some(color) = indicator_theme.color {
            controls_theme.colors.accent = color;
        }
        if let Some(track_color) = indicator_theme.linear_track_color {
            controls_theme.colors.border = track_color;
        }
        let mut progress = incular_controls::progress::Root::new()
            .range(self.min, self.max)
            .width(self.width);
        if let Some(height) = self.height {
            progress = progress.height(height);
        } else if let Some(height) = indicator_theme.linear_track_height {
            progress = progress.height(height);
        }
        if let Some(label) = self.label.as_ref() {
            progress = progress.label(label.clone());
        }
        progress = match self.value {
            Some(value) => progress.value(value),
            None => progress.indeterminate(true),
        };
        progress.build(&controls_theme)
    }
}

impl From<LinearProgressIndicator> for Widget {
    fn from(value: LinearProgressIndicator) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| value.build(&current_control_theme()))
    }
}

/// A retained circular progress indicator. The ring is lowered to the shared
/// renderer path/display-list primitives so determinate values paint the
/// actual progress arc while indeterminate values still expose a stable
/// Material-sized visual without a continuously scheduled frame.
#[derive(Clone, TypedBuilder)]
pub struct CircularProgressIndicator {
    #[builder(default, setter(strip_option))]
    value: Option<f32>,
    #[builder(default = 0.0)]
    min: f32,
    #[builder(default = 1.0)]
    max: f32,
    #[builder(default = 24.0, setter(transform = |value: f32| finite_non_negative(value)))]
    size: f32,
    #[builder(default = 2.0, setter(transform = |value: f32| finite_non_negative(value)))]
    stroke_width: f32,
    #[builder(default, setter(strip_option))]
    color: Option<Color>,
    #[builder(default, setter(strip_option))]
    background_color: Option<Color>,
    #[builder(default, setter(strip_option, into))]
    label: Option<String>,
}

impl Default for CircularProgressIndicator {
    fn default() -> Self {
        Self::new()
    }
}

impl CircularProgressIndicator {
    /// Creates an indeterminate circular indicator.
    #[must_use]
    pub fn new() -> Self {
        Self {
            value: None,
            min: 0.0,
            max: 1.0,
            size: 24.0,
            stroke_width: 2.0,
            color: None,
            background_color: None,
            label: None,
        }
    }

    #[must_use]
    pub fn value(mut self, value: f32) -> Self {
        self.value = Some(value);
        self
    }

    #[must_use]
    pub fn value_option(mut self, value: Option<f32>) -> Self {
        self.value = value;
        self
    }

    #[must_use]
    pub fn indeterminate(mut self, value: bool) -> Self {
        if value {
            self.value = None;
        } else if self.value.is_none() {
            self.value = Some(self.min);
        }
        self
    }

    #[must_use]
    pub fn range(mut self, min: f32, max: f32) -> Self {
        if min.is_finite() && max.is_finite() {
            self.min = min.min(max);
            self.max = max.max(min);
        }
        self
    }

    #[must_use]
    pub fn size(mut self, size: f32) -> Self {
        self.size = finite_non_negative(size);
        self
    }

    #[must_use]
    pub fn stroke_width(mut self, stroke_width: f32) -> Self {
        self.stroke_width = finite_non_negative(stroke_width);
        self
    }

    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    #[must_use]
    pub fn background_color(mut self, color: Color) -> Self {
        self.background_color = Some(color);
        self
    }

    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        let indicator_theme = current_progress_indicator_theme().unwrap_or_default();
        let size = self.size.max(1.0);
        let stroke = self
            .stroke_width
            .max(indicator_theme.stroke_width.unwrap_or(0.0))
            .min(size * 0.5)
            .max(0.1);
        let track = self
            .background_color
            .or(indicator_theme.circular_track_color)
            .unwrap_or_else(|| with_alpha(theme.colors.border, 96));
        let progress = self
            .color
            .or(indicator_theme.color)
            .unwrap_or(theme.colors.accent);
        let ratio = normalized(self.value, self.min, self.max);
        let center = Offset::new(size * 0.5, size * 0.5);
        let radius = (size - stroke) * 0.5;
        let mut canvas = Canvas::default();
        let track_path = Arc::new(circle_arc(center, radius, 0.0, std::f32::consts::TAU));
        canvas.stroke_path(
            track_path,
            track,
            Stroke {
                width: stroke,
                cap: LineCap::Round,
                join: LineJoin::Round,
                miter_limit: 4.0,
            },
        );
        let progress_end = ratio.map_or(1.25 * std::f32::consts::PI, |value| {
            -std::f32::consts::FRAC_PI_2 + value * std::f32::consts::TAU
        });
        let progress_start = -std::f32::consts::FRAC_PI_2;
        if ratio.is_none_or(|value| value > 0.0) {
            let progress_path = Arc::new(circle_arc(center, radius, progress_start, progress_end));
            canvas.stroke_path(
                progress_path,
                progress,
                Stroke {
                    width: stroke,
                    cap: LineCap::Round,
                    join: LineJoin::Round,
                    miter_limit: 4.0,
                },
            );
        }
        let visual: Widget =
            incular_widgets::CustomPaint::new(Size::new(size, size), canvas.finish()).into();
        let value_text = ratio
            .map(|value| format!("{:.0}%", value * 100.0))
            .unwrap_or_else(|| "In progress".to_owned());
        Semantics::new(visual)
            .label(self.label.clone().unwrap_or_else(|| "Progress".to_owned()))
            .value(value_text)
            .into()
    }
}

fn circle_arc(center: Offset, radius: f32, start: f32, end: f32) -> Path {
    let span = (end - start).abs().max(0.001);
    let steps = ((span / (std::f32::consts::TAU / 48.0)).ceil() as usize).clamp(2, 96);
    let direction = if end >= start { 1.0 } else { -1.0 };
    let mut builder = Path::builder();
    for index in 0..=steps {
        let t = index as f32 / steps as f32;
        let angle = start + direction * span * t;
        let point = Offset::new(
            center.x + radius * angle.cos(),
            center.y + radius * angle.sin(),
        );
        if index == 0 {
            builder.move_to(point);
        } else {
            builder.line_to(point);
        }
    }
    builder.build()
}

impl From<CircularProgressIndicator> for Widget {
    fn from(value: CircularProgressIndicator) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| value.build(&current_control_theme()))
    }
}

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
        Widget::layout_builder(move |_| value.build(&current_control_theme()))
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
        let raw = if let Some(callback) = self.on_long_press.clone() {
            GestureDetector::new(raw)
                .behavior(HitTestBehavior::Opaque)
                .on_long_press(move || callback())
                .into()
        } else {
            raw
        };
        let semantics = Semantics::new(raw).button(has_action);
        let _ = (
            self.min_leading_width,
            self.horizontal_title_gap,
            self.autofocus,
        );
        match self.semantic_label.clone() {
            Some(label) => semantics.label(label).into(),
            None => semantics.into(),
        }
    }
}

impl From<ListTile> for Widget {
    fn from(value: ListTile) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| value.build(&current_control_theme()))
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
        Widget::layout_builder(move |_| value.build(&current_control_theme()))
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
        let semantics = Semantics::new(raw).button(self.on_changed.is_some());
        match self.semantic_label.clone() {
            Some(label) => semantics.label(label).into(),
            None => semantics.into(),
        }
    }
}

impl<T: PartialEq + Clone + 'static> From<RadioListTile<T>> for Widget {
    fn from(value: RadioListTile<T>) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| value.build(&current_control_theme()))
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
        Widget::layout_builder(move |_| value.build(&current_control_theme()))
    }
}

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
            Widget::drop_shadow(
                Offset::new(0.0, (self.elevation * 0.2).min(8.0)),
                (self.elevation * 0.45).clamp(1.0, 16.0),
                Color::rgba(0, 0, 0, 64),
                decorated,
            )
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
        let semantics = Semantics::new(raw).button(has_action);
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
        Widget::layout_builder(move |_| value.build(&current_control_theme()))
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

fn finite_non_negative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

fn with_alpha(color: Color, alpha: u8) -> Color {
    Color::rgba(color.red, color.green, color.blue, alpha)
}

fn normalized(value: Option<f32>, min: f32, max: f32) -> Option<f32> {
    let value = value?;
    if !value.is_finite() {
        return None;
    }
    let span = (max - min).max(f32::EPSILON);
    Some(((value - min) / span).clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn raw_chip_defaults_match_a_neutral_material_chip() {
        let chip = RawChip::new("Inbox");
        assert_eq!(chip.label(), "Inbox");
        assert!(!chip.is_selected());
        assert!(chip.is_enabled());
        assert_eq!(chip.elevation_value(), 0.0);
        assert_eq!(chip.padding, EdgeInsets::symmetric(12.0, 6.0));
    }

    #[test]
    fn raw_chip_sanitizes_elevation_and_retains_actions() {
        let deleted = Rc::new(Cell::new(false));
        let deleted_callback = deleted.clone();
        let chip = RawChip::new("Remove me")
            .selected(true)
            .show_checkmark(true)
            .elevation(f32::INFINITY)
            .on_deleted(move || deleted_callback.set(true));

        assert_eq!(chip.elevation_value(), 0.0);
        assert!(chip.selected);
        assert!(chip.show_checkmark);
        assert!(chip.on_deleted.is_some());
        assert!(!deleted.get());
    }

    #[test]
    fn named_chip_wrappers_expose_their_selection_modes() {
        let selected = Rc::new(Cell::new(false));
        let selected_callback = selected.clone();
        let choice = ChoiceChip::new("One", true).on_selected(move |value| {
            selected_callback.set(value);
        });
        assert!(choice.raw.selected);
        assert!(choice.raw.show_checkmark);
        assert!(choice.raw.on_selected.is_some());

        let filter = FilterChip::new("Unread", false).on_deleted(|| {});
        assert!(!filter.raw.selected);
        assert!(filter.raw.show_checkmark);
        assert!(filter.raw.on_deleted.is_some());

        let input = InputChip::new("Tag")
            .selected(true)
            .on_pressed(|| {})
            .on_deleted(|| {});
        assert!(input.raw.selected);
        assert!(input.raw.on_pressed.is_some());
        assert!(input.raw.on_deleted.is_some());
        assert!(!selected.get());
    }

    #[test]
    fn chip_family_materializes_through_retained_widgets() {
        let theme = ControlTheme::light();
        let _: Widget = Chip::new("Base").build(&theme);
        let _: Widget = ActionChip::new("Run").on_pressed(|| {}).into();
        let _: Widget = ChoiceChip::new("One", true).into();
        let _: Widget = FilterChip::new("Unread", false)
            .on_selected(|_| {})
            .on_deleted(|| {})
            .into();
        let _: Widget = InputChip::new("Token").on_deleted(|| {}).into();
        let _: Widget = RawChip::new("Raw").delete_icon(Text::new("x")).into();
    }
}
