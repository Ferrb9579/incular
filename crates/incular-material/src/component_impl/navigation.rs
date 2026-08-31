use std::rc::Rc;

use crate::{
    BottomNavigationBarType, K_BOTTOM_NAVIGATION_BAR_HEIGHT, NavigationDestinationLabelBehavior,
    TextButton,
};
use incular_config::{CrossAxisAlignment, MainAxisAlignment};
use incular_controls::{ControlTheme, current_control_theme};
use incular_core::Color;
use incular_widgets::{Column, Container, Row, Text, Widget};
use typed_builder::TypedBuilder;

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
        Widget::from(incular_widgets::LayoutBuilder::new(move |context, _| {
            value.build(&current_control_theme(context))
        }))
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

    fn build(&self, context: &incular_widgets::BuildContext<'_>) -> Widget {
        let selected = self
            .selected_index
            .min(self.destinations.len().saturating_sub(1));
        let behavior = self.label_behavior;
        let callback = self.on_destination_selected.clone();
        let children = self
            .destinations
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, item)| {
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
                if item.enabled
                    && let Some(callback) = callback.clone()
                {
                    button = button.on_click(move || callback(index));
                }
                Widget::from(button)
            });
        Container::new()
            .height(80.0)
            .color(current_control_theme(context).colors.surface)
            .child(Row::new(children).main_axis_alignment(MainAxisAlignment::SpaceEvenly))
            .into()
    }
}

impl From<NavigationBar> for Widget {
    fn from(value: NavigationBar) -> Self {
        let value = Rc::new(value);
        Widget::from(incular_widgets::LayoutBuilder::new(move |context, _| {
            value.build(context)
        }))
    }
}
