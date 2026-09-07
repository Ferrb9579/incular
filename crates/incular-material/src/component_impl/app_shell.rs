use std::rc::Rc;

use super::common::finite_non_negative;
use crate::foundation::{Material, Theme};
use incular_config::{Alignment, CrossAxisAlignment, EdgeInsets, MainAxisAlignment, MainAxisSize};
use incular_controls::{ControlTheme, current_control_theme};
use incular_core::Color;
use incular_text::TextStyle;
use incular_widgets::internal::Expanded;
use incular_widgets::{
    BorderRadius, Column, Container, DefaultTextStyle, IconTheme, Padding, Positioned, Row,
    SizedBox, Stack, Text, Widget,
};
use typed_builder::TypedBuilder;

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
                .main_axis_size(MainAxisSize::Min)
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
        Widget::from(incular_widgets::LayoutBuilder::new(move |context, _| {
            value.build(&current_control_theme(context))
        }))
    }
}

/// Material app bar for a [`incular_widgets::CustomScrollView`]. Pass this
/// descriptor as a [`incular_widgets::Sliver`] to enable retained pinning,
/// floating or stretching. A pinned fixed-height toolbar remains visible even
/// when floating is also enabled.
/// Conversion to an ordinary [`Widget`] supplies only the static box
/// presentation; stretching applies only to the retained sliver because it
/// consumes the viewport's leading overscroll.
/// Explicit expanded/collapsed heights select retained resizing and describe
/// the entire header, including its bottom slot. The collapsed default is the
/// configured toolbar height; the expanded default is the collapsed height.
/// Without explicit heights, the header measures its natural content and
/// stretches that measurement.
/// Snapping remains pending.
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
    #[builder(default, setter(transform = |value: f32| Some(finite_non_negative(value))))]
    expanded_height: Option<f32>,
    #[builder(default, setter(transform = |value: f32| Some(finite_non_negative(value))))]
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
        self.app_bar.title = title.into();
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
        self.expanded_height = Some(finite_non_negative(value));
        self
    }

    #[must_use]
    pub fn collapsed_height(mut self, value: f32) -> Self {
        self.collapsed_height = Some(finite_non_negative(value));
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

impl incular_widgets::Sliver for SliverAppBar {
    fn build(&self, controller: &incular_scroll::ScrollController) -> Widget {
        incular_widgets::Sliver::build_with_config(
            self,
            controller,
            incular_config::Axis::Vertical,
            false,
        )
    }

    fn build_with_config(
        &self,
        controller: &incular_scroll::ScrollController,
        axis: incular_config::Axis,
        reverse: bool,
    ) -> Widget {
        self.retained_header()
            .build_with_config(controller, axis, reverse)
    }

    fn create_render_sliver(
        &self,
        controller: &incular_scroll::ScrollController,
        axis: incular_config::Axis,
        reverse: bool,
    ) -> Box<dyn incular_widgets::internal::RenderSliver> {
        self.retained_header()
            .create_render_sliver(controller, axis, reverse)
    }
}

impl SliverAppBar {
    fn retained_header(&self) -> Box<dyn incular_widgets::Sliver> {
        // Keep theme lookup in the mounted subtree. The neutral header owns
        // scroll geometry; Material supplies only its deferred presentation.
        if self.expanded_height.is_some() || self.collapsed_height.is_some() {
            use incular_widgets::{
                SliverHeaderOverscrollBehavior, SliverHeaderScrollBehavior, SliverResizingHeader,
            };

            let min = self.collapsed_height.unwrap_or(self.app_bar.toolbar_height);
            let max = self.expanded_height.unwrap_or(min).max(min);
            let behavior = match (self.pinned, self.floating) {
                (false, false) => SliverHeaderScrollBehavior::Scroll,
                (true, false) => SliverHeaderScrollBehavior::Pinned,
                (false, true) => SliverHeaderScrollBehavior::Floating,
                (true, true) => SliverHeaderScrollBehavior::FloatingPinned,
            };
            let overscroll = if self.stretch {
                SliverHeaderOverscrollBehavior::Stretch
            } else {
                SliverHeaderOverscrollBehavior::Translate
            };
            return Box::new(
                SliverResizingHeader::new(min, max, self.resizing_child())
                    .scroll_behavior(behavior)
                    .overscroll_behavior(overscroll),
            );
        }
        if self.stretch {
            use incular_widgets::{SliverHeaderScrollBehavior, SliverNaturalHeader};

            let behavior = match (self.pinned, self.floating) {
                (false, false) => SliverHeaderScrollBehavior::Scroll,
                (true, false) => SliverHeaderScrollBehavior::Pinned,
                (false, true) => SliverHeaderScrollBehavior::Floating,
                (true, true) => SliverHeaderScrollBehavior::FloatingPinned,
            };
            return Box::new(
                SliverNaturalHeader::new(self.natural_stretch_child())
                    .scroll_behavior(behavior)
                    .overscroll_behavior(incular_widgets::SliverHeaderOverscrollBehavior::Stretch),
            );
        }
        let child = Widget::from(self.clone());
        if self.pinned {
            Box::new(incular_widgets::PinnedHeaderSliver::new(child))
        } else if self.floating {
            Box::new(incular_widgets::SliverFloatingHeader::new(child))
        } else {
            Box::new(incular_widgets::SliverToBoxAdapter::new(child))
        }
    }

    fn resizing_child(&self) -> Widget {
        let mut app_bar = self.app_bar.clone();
        let bottom = app_bar.bottom.take();
        // Flex measures the bottom first. Resolve toolbar presentation against
        // the remaining tight height, retaining the same slot structure.
        let toolbar = Widget::from(incular_widgets::LayoutBuilder::new(
            move |context, constraints| {
                app_bar
                    .clone()
                    .toolbar_height(constraints.max_height())
                    .build(&current_control_theme(context))
            },
        ));
        let child = if let Some(bottom) = bottom {
            Column::new([Expanded::new(toolbar).into(), bottom])
                .main_axis_size(MainAxisSize::Max)
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .into()
        } else {
            toolbar
        };
        incular_widgets::ClipRect::new(child).into()
    }

    fn natural_stretch_child(&self) -> Widget {
        let app_bar = self.app_bar.clone();
        // The neutral header measures this child unbounded while settled so
        // later content changes are learned, and tight while stretched. Both
        // branches build the same AppBar structure and differ only in the
        // resolved toolbar height, so slot identity survives the switch and
        // stretched samples never become the cached natural measurement.
        Widget::from(incular_widgets::LayoutBuilder::new(
            move |context, constraints| {
                let theme = current_control_theme(context);
                if constraints.max_height().is_finite() {
                    let current = constraints.max_height();
                    // Bottom keeps its measured height; the toolbar fills the
                    // remainder. The hint covers fixed bottoms (the common
                    // PreferredSize case); without one fall back to natural
                    // presentation instead of guessing a height.
                    if let Some(bottom) = app_bar.bottom.clone()
                        && let Some(bottom_height) =
                            incular_widgets::internal::widget_main_extent_hint(
                                &bottom,
                                incular_config::Axis::Vertical,
                            )
                    {
                        let toolbar = (current - bottom_height).max(0.);
                        return app_bar.clone().toolbar_height(toolbar).build(&theme);
                    }
                    if app_bar.bottom.is_none() {
                        return app_bar.clone().toolbar_height(current).build(&theme);
                    }
                }
                app_bar.clone().build(&theme)
            },
        ))
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
        Widget::from(incular_widgets::LayoutBuilder::new(move |context, _| {
            value.build(&current_control_theme(context))
        }))
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

    /// Excludes the bottom view inset from this scaffold's usable layout area.
    /// All slots, including floating controls and bottom bars, reflow above it.
    /// Disable this on a nested scaffold when its parent already avoids the inset.
    #[must_use]
    pub fn resize_to_avoid_bottom_inset(mut self, value: bool) -> Self {
        self.resize_to_avoid_bottom_inset = value;
        self
    }

    /// Extends the body beneath the bottom region (sheet and bottom bar).
    /// That region stays bottom-aligned and paints above the body.
    #[must_use]
    pub fn extend_body(mut self, value: bool) -> Self {
        self.extend_body = value;
        self
    }

    /// Extends the body beneath the app bar, including its bottom content.
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
    pub fn build(
        &self,
        context: &incular_widgets::BuildContext<'_>,
        theme: &ControlTheme,
    ) -> Widget {
        let mut children = Vec::new();
        if let Some(app_bar) = &self.app_bar
            && !self.extend_body_behind_app_bar
        {
            children.push(app_bar.build(theme).with_key("scaffold-app-bar"));
        }
        let scaffold_background = Theme::of_shared(context)
            .map_or(theme.colors.background, |theme| {
                theme.colors().scaffold_background_color
            });
        let body = Expanded::new(
            Material::new(self.body.clone()).color(self.background.unwrap_or(scaffold_background)),
        );
        // A private slot key keeps the body mounted when reserved regions move
        // into the overlay layer. Application keys remain on the child itself.
        children.push(Widget::from(body).with_key("scaffold-body"));
        let mut bottom_children = Vec::new();
        if let Some(sheet) = self.bottom_sheet.clone() {
            bottom_children.push(sheet);
        }
        if let Some(bottom) = self
            .bottom_navigation_bar
            .clone()
            .or_else(|| self.bottom_app_bar.clone())
        {
            bottom_children.push(bottom);
        }
        let bottom_region: Widget = Column::new(bottom_children)
            .main_axis_size(MainAxisSize::Min)
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .into();
        if !self.extend_body {
            children.push(bottom_region.clone().with_key("scaffold-bottom-region"));
        }
        let content: Widget = Column::new(children)
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .into();
        let mut stack_children = vec![content];
        if self.extend_body_behind_app_bar
            && let Some(app_bar) = &self.app_bar
        {
            stack_children.push(
                Widget::from(
                    Positioned::new(app_bar.build(theme))
                        .left(0.0)
                        .right(0.0)
                        .top(0.0),
                )
                .with_key("scaffold-app-bar"),
            );
        }
        if self.extend_body {
            stack_children.push(
                Widget::from(
                    Positioned::new(bottom_region)
                        .left(0.0)
                        .right(0.0)
                        .bottom(0.0),
                )
                .with_key("scaffold-bottom-region"),
            );
        }
        if let Some(fab) = self.floating_action_button.clone() {
            stack_children.push(
                Widget::from(Positioned::new(fab).right(16.0).bottom(
                    if self.bottom_navigation_bar.is_some() || self.bottom_app_bar.is_some() {
                        96.0
                    } else {
                        16.0
                    },
                ))
                .with_key("scaffold-fab"),
            );
        }
        if let Some(drawer) = self.drawer.clone() {
            stack_children.push(
                Widget::from(Positioned::new(drawer).left(0.0).top(0.0).bottom(0.0))
                    .with_key("scaffold-drawer"),
            );
        }
        if let Some(drawer) = self.end_drawer.clone() {
            stack_children.push(
                Widget::from(Positioned::new(drawer).right(0.0).top(0.0).bottom(0.0))
                    .with_key("scaffold-end-drawer"),
            );
        }
        let bottom_inset = if self.resize_to_avoid_bottom_inset {
            context.view_insets().bottom
        } else {
            0.0
        };
        Padding::new(
            EdgeInsets::only(0.0, 0.0, 0.0, bottom_inset),
            Stack::aligned(Alignment::TOP_LEFT, stack_children),
        )
        .into()
    }
}

impl From<Scaffold> for Widget {
    fn from(value: Scaffold) -> Self {
        let value = Rc::new(value);
        Widget::from(incular_widgets::LayoutBuilder::new(move |context, _| {
            value.build(context, &current_control_theme(context))
        }))
    }
}
