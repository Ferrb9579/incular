use crate::TextButton;
use incular_config::{CrossAxisAlignment, EdgeInsets, MainAxisAlignment};
use incular_controls::{ControlTheme, current_control_theme};
use incular_core::Color;
use incular_widgets::{Column, Container, Row, Text, Widget};
use std::rc::Rc;
use typed_builder::TypedBuilder;

/// A single destination shown by [`NavigationRail`].
#[derive(Clone, TypedBuilder)]
pub struct NavigationRailDestination {
    #[builder(setter(into))]
    pub icon: Widget,
    #[builder(setter(into))]
    pub label: String,
    #[builder(default, setter(strip_option, into))]
    pub selected_icon: Option<Widget>,
    #[builder(default = true)]
    pub enabled: bool,
}

impl NavigationRailDestination {
    #[must_use]
    pub fn new(icon: impl Into<Widget>, label: impl Into<String>) -> Self {
        Self {
            icon: icon.into(),
            label: label.into(),
            selected_icon: None,
            enabled: true,
        }
    }

    #[must_use]
    pub fn selected_icon(mut self, icon: impl Into<Widget>) -> Self {
        self.selected_icon = Some(icon.into());
        self
    }

    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

/// Material navigation rail with retained destination activation callbacks.
#[derive(Clone, TypedBuilder)]
pub struct NavigationRail {
    #[builder(
        default = Vec::new(),
        setter(transform = |destinations: impl IntoIterator<Item = NavigationRailDestination>| {
            destinations.into_iter().collect::<Vec<_>>()
        })
    )]
    destinations: Vec<NavigationRailDestination>,
    #[builder(default)]
    selected_index: usize,
    #[builder(default = crate::NavigationRailLabelType::None)]
    label_type: crate::NavigationRailLabelType,
    #[builder(default)]
    extended: bool,
    #[builder(default = 80.0, setter(transform = |width: f32| width.max(0.0)))]
    min_width: f32,
    #[builder(default = 256.0, setter(transform = |width: f32| width.max(0.0)))]
    min_extended_width: f32,
    #[builder(default, setter(strip_option))]
    background: Option<Color>,
    #[builder(default, setter(strip_option, into))]
    leading: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    trailing: Option<Widget>,
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

impl NavigationRail {
    #[must_use]
    pub fn new(destinations: impl IntoIterator<Item = NavigationRailDestination>) -> Self {
        Self {
            destinations: destinations.into_iter().collect(),
            selected_index: 0,
            label_type: crate::NavigationRailLabelType::None,
            extended: false,
            min_width: 80.0,
            min_extended_width: 256.0,
            background: None,
            leading: None,
            trailing: None,
            on_destination_selected: None,
        }
    }

    #[must_use]
    pub fn selected_index(mut self, index: usize) -> Self {
        self.selected_index = index;
        self
    }

    #[must_use]
    pub fn label_type(mut self, label_type: crate::NavigationRailLabelType) -> Self {
        self.label_type = label_type;
        self
    }

    #[must_use]
    pub fn extended(mut self, extended: bool) -> Self {
        self.extended = extended;
        self
    }

    #[must_use]
    pub fn min_width(mut self, width: f32) -> Self {
        self.min_width = width.max(0.0);
        self
    }

    #[must_use]
    pub fn min_extended_width(mut self, width: f32) -> Self {
        self.min_extended_width = width.max(self.min_width);
        self
    }

    #[must_use]
    pub fn background_color(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    #[must_use]
    pub fn leading(mut self, child: impl Into<Widget>) -> Self {
        self.leading = Some(child.into());
        self
    }

    #[must_use]
    pub fn trailing(mut self, child: impl Into<Widget>) -> Self {
        self.trailing = Some(child.into());
        self
    }

    #[must_use]
    pub fn on_destination_selected(mut self, callback: impl Fn(usize) + 'static) -> Self {
        self.on_destination_selected = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        let selected = self
            .selected_index
            .min(self.destinations.len().saturating_sub(1));
        let show_all =
            self.extended || matches!(self.label_type, crate::NavigationRailLabelType::All);
        let show_selected = matches!(self.label_type, crate::NavigationRailLabelType::Selected);
        let destinations = self.destinations.iter().enumerate().map(|(index, item)| {
            let icon = if index == selected {
                item.selected_icon
                    .clone()
                    .unwrap_or_else(|| item.icon.clone())
            } else {
                item.icon.clone()
            };
            let show_label = show_all || (show_selected && index == selected);
            let content: Widget = if show_label {
                Row::new([icon, Text::new(item.label.clone()).into()])
                    .spacing(10.0)
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .into()
            } else {
                icon
            };
            let mut button = TextButton::with_child(content).enabled(item.enabled);
            if item.enabled
                && let Some(callback) = self.on_destination_selected.clone()
            {
                button = button.on_click(move || callback(index));
            }
            button.into()
        });
        let mut children = Vec::new();
        if let Some(leading) = self.leading.clone() {
            children.push(leading);
        }
        children.extend(destinations);
        if let Some(trailing) = self.trailing.clone() {
            children.push(trailing);
        }
        Container::new()
            .width(if self.extended {
                self.min_extended_width
            } else {
                self.min_width
            })
            .padding(EdgeInsets::symmetric(8.0, 12.0))
            .color(self.background.unwrap_or(theme.colors.surface))
            .child(
                Column::new(children)
                    .spacing(8.0)
                    .main_axis_alignment(MainAxisAlignment::Start)
                    .cross_axis_alignment(CrossAxisAlignment::Stretch),
            )
            .into()
    }
}

impl Default for NavigationRail {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl From<NavigationRail> for Widget {
    fn from(value: NavigationRail) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| value.build(&current_control_theme()))
    }
}

/// A single destination shown by [`NavigationDrawer`].
#[derive(Clone, TypedBuilder)]
pub struct NavigationDrawerDestination {
    #[builder(setter(into))]
    pub icon: Widget,
    #[builder(setter(into))]
    pub label: String,
    #[builder(default, setter(strip_option, into))]
    pub selected_icon: Option<Widget>,
    #[builder(default = true)]
    pub enabled: bool,
}

impl NavigationDrawerDestination {
    #[must_use]
    pub fn new(icon: impl Into<Widget>, label: impl Into<String>) -> Self {
        Self {
            icon: icon.into(),
            label: label.into(),
            selected_icon: None,
            enabled: true,
        }
    }

    #[must_use]
    pub fn selected_icon(mut self, icon: impl Into<Widget>) -> Self {
        self.selected_icon = Some(icon.into());
        self
    }

    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

/// Material navigation drawer with destination selection and arbitrary header
/// or extra children.
#[derive(Clone, TypedBuilder)]
pub struct NavigationDrawer {
    #[builder(
        default = Vec::new(),
        setter(transform = |destinations: impl IntoIterator<Item = NavigationDrawerDestination>| {
            destinations.into_iter().collect::<Vec<_>>()
        })
    )]
    destinations: Vec<NavigationDrawerDestination>,
    #[builder(
        default = Vec::new(),
        setter(transform = |children: impl IntoIterator<Item = impl Into<Widget>>| {
            children.into_iter().map(Into::into).collect::<Vec<Widget>>()
        })
    )]
    children: Vec<Widget>,
    #[builder(default)]
    selected_index: usize,
    #[builder(default = 360.0, setter(transform = |width: f32| width.max(0.0)))]
    width: f32,
    #[builder(default, setter(strip_option))]
    background: Option<Color>,
    #[builder(default, setter(strip_option, into))]
    header: Option<Widget>,
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

impl NavigationDrawer {
    #[must_use]
    pub fn new(destinations: impl IntoIterator<Item = NavigationDrawerDestination>) -> Self {
        Self {
            destinations: destinations.into_iter().collect(),
            children: Vec::new(),
            selected_index: 0,
            width: 360.0,
            background: None,
            header: None,
            on_destination_selected: None,
        }
    }

    #[must_use]
    pub fn children(mut self, children: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        self.children = children.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn header(mut self, header: impl Into<Widget>) -> Self {
        self.header = Some(header.into());
        self
    }

    #[must_use]
    pub fn selected_index(mut self, index: usize) -> Self {
        self.selected_index = index;
        self
    }

    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        self.width = width.max(0.0);
        self
    }

    #[must_use]
    pub fn background_color(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    #[must_use]
    pub fn on_destination_selected(mut self, callback: impl Fn(usize) + 'static) -> Self {
        self.on_destination_selected = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        let selected = self
            .selected_index
            .min(self.destinations.len().saturating_sub(1));
        let mut children = Vec::new();
        if let Some(header) = self.header.clone() {
            children.push(header);
        }
        children.extend(self.children.clone());
        children.extend(self.destinations.iter().enumerate().map(|(index, item)| {
            let icon = if index == selected {
                item.selected_icon
                    .clone()
                    .unwrap_or_else(|| item.icon.clone())
            } else {
                item.icon.clone()
            };
            let content: Widget = Row::new([icon, Text::new(item.label.clone()).into()])
                .spacing(12.0)
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .into();
            let mut button = TextButton::with_child(content).enabled(item.enabled);
            if item.enabled
                && let Some(callback) = self.on_destination_selected.clone()
            {
                button = button.on_click(move || callback(index));
            }
            button.into()
        }));
        Container::new()
            .width(self.width)
            .padding(EdgeInsets::symmetric(12.0, 16.0))
            .color(self.background.unwrap_or(theme.colors.surface))
            .child(
                Column::new(children)
                    .spacing(8.0)
                    .cross_axis_alignment(CrossAxisAlignment::Stretch),
            )
            .into()
    }
}

impl Default for NavigationDrawer {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl From<NavigationDrawer> for Widget {
    fn from(value: NavigationDrawer) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| value.build(&current_control_theme()))
    }
}
