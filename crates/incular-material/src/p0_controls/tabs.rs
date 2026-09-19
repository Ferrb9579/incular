use crate::foundation::{StateProperty, Theme, ThemeData};
use crate::{TabAlignment, TabBarIndicatorSize};
use incular_config::{CrossAxisAlignment, MainAxisAlignment};
use incular_core::Color;
use incular_text::TextStyle;
use incular_widgets::{
    Column, Container, ExcludeSemantics, IgnorePointer, PageView, Positioned, Row, Stack, Text,
    Widget,
};
use std::cell::Cell;
use std::rc::Rc;
use typed_builder::TypedBuilder;

/// A Material tab descriptor. `text` and `icon` are convenience constructors;
/// a tab may also contain an arbitrary retained widget.
#[derive(Clone, TypedBuilder)]
pub struct Tab {
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    icon: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    text: Option<String>,
    #[builder(default = true)]
    enabled: bool,
}

impl Tab {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: Some(child.into()),
            icon: None,
            text: None,
            enabled: true,
        }
    }

    #[must_use]
    pub fn text(value: impl Into<String>) -> Self {
        let text = value.into();
        Self {
            child: None,
            icon: None,
            text: Some(text),
            enabled: true,
        }
    }

    #[must_use]
    pub fn icon(icon: impl Into<Widget>) -> Self {
        let icon = icon.into();
        Self {
            child: None,
            icon: Some(icon),
            text: None,
            enabled: true,
        }
    }

    #[must_use]
    pub fn icon_and_text(icon: impl Into<Widget>, text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            child: None,
            icon: Some(icon.into()),
            text: Some(text),
            enabled: true,
        }
    }

    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self
    }

    #[must_use]
    pub fn enabled(mut self, value: bool) -> Self {
        self.enabled = value;
        self
    }

    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn content(&self, style: Option<TextStyle>, color: Option<Color>) -> Widget {
        if let Some(child) = self.child.as_ref() {
            return child.clone();
        }

        let label = self.text.as_ref().map(|text| {
            let mut text = Text::new(text.clone());
            if let Some(style) = style.clone() {
                text = text.style(match color {
                    Some(color) => style.color(color),
                    None => style,
                });
            } else if let Some(color) = color {
                text = text.color(color);
            }
            Widget::from(text)
        });

        match (self.icon.as_ref(), label) {
            (Some(icon), Some(label)) => Row::new([icon.clone(), label])
                .spacing(8.0)
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .into(),
            (Some(icon), None) => icon.clone(),
            (None, Some(label)) => label,
            (None, None) => incular_widgets::SizedBox::shrink().into(),
        }
    }
}

impl From<Tab> for Widget {
    fn from(value: Tab) -> Self {
        let child = value.content(None, None);
        if value.enabled {
            child
        } else {
            IgnorePointer::new(ExcludeSemantics::new(child)).into()
        }
    }
}

/// Retained controller shared by `TabBar` and `TabBarView`.
#[derive(Clone, Debug)]
pub struct TabController {
    length: usize,
    index: Rc<Cell<usize>>,
    revision: Rc<Cell<u64>>,
    page_controller: incular_scroll::ScrollController,
    page_extent: Rc<Cell<f32>>,
}

impl TabController {
    #[must_use]
    pub fn new(length: usize) -> Self {
        Self {
            length,
            index: Rc::new(Cell::new(0)),
            revision: Rc::new(Cell::new(0)),
            page_controller: incular_scroll::ScrollController::new(),
            page_extent: Rc::new(Cell::new(1.0)),
        }
    }

    #[must_use]
    pub fn length(&self) -> usize {
        self.length
    }

    #[must_use]
    pub fn index(&self) -> usize {
        self.index.get().min(self.length.saturating_sub(1))
    }

    pub fn set_index(&self, value: usize) {
        let index = value.min(self.length.saturating_sub(1));
        if index == self.index() {
            return;
        }
        self.index.set(index);
        let offset = index as f32 * self.page_extent.get();
        if self.page_controller.max_offset() > 0.0 {
            self.page_controller.jump_to(offset);
        } else {
            self.page_controller.deferred_jump_to(offset);
        }
        self.revision.set(self.revision.get().wrapping_add(1));
    }

    pub fn animate_to(&self, value: usize) {
        self.set_index(value);
    }

    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision.get()
    }

    fn revision_cell(&self) -> Rc<Cell<u64>> {
        self.revision.clone()
    }

    /// Returns the retained page position shared by `TabBarView`.
    #[must_use]
    pub fn page_controller(&self) -> incular_scroll::ScrollController {
        self.page_controller.clone()
    }

    fn set_page_extent(&self, extent: f32) {
        let extent = if extent.is_finite() {
            extent.max(1.0)
        } else {
            1.0
        };
        if (self.page_extent.get() - extent).abs() <= f32::EPSILON {
            return;
        }
        self.page_extent.set(extent);
        let offset = self.index() as f32 * self.page_extent.get();
        // LayoutBuilder runs before the PageView publishes its new metrics.
        // Always defer the extent-derived target so a resize cannot clamp the
        // new page offset against the previous viewport's max extent.
        self.page_controller.deferred_jump_to(offset);
    }
}

impl Default for TabController {
    fn default() -> Self {
        Self::new(0)
    }
}

/// Material tab bar. It deliberately uses ordinary buttons and a retained
/// controller; no second tab navigation engine is introduced.
#[derive(Clone, TypedBuilder)]
pub struct TabBar {
    #[builder(
        default,
        setter(transform = |tabs: impl IntoIterator<Item = Tab>| {
            tabs.into_iter().collect::<Vec<Tab>>()
        })
    )]
    tabs: Vec<Tab>,
    #[builder(default, setter(strip_option))]
    controller: Option<TabController>,
    #[builder(default)]
    selected_index: usize,
    #[builder(default)]
    scrollable: bool,
    #[builder(default, setter(strip_option))]
    alignment: Option<TabAlignment>,
    #[builder(default = TabBarIndicatorSize::Tab)]
    indicator_size: TabBarIndicatorSize,
    #[builder(default, setter(strip_option))]
    indicator_color: Option<Color>,
    #[builder(
        default,
        setter(
            fn transform<F>(callback: F) -> Option<Rc<dyn Fn(usize) + 'static>>
            where
                F: Fn(usize) + 'static,
            {
                Some(Rc::new(callback))
            }
        )
    )]
    on_tap: Option<Rc<dyn Fn(usize) + 'static>>,
}

impl Default for TabBar {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl TabBar {
    #[must_use]
    pub fn new(tabs: impl IntoIterator<Item = Tab>) -> Self {
        Self {
            tabs: tabs.into_iter().collect(),
            controller: None,
            selected_index: 0,
            scrollable: false,
            alignment: None,
            indicator_size: TabBarIndicatorSize::Tab,
            indicator_color: None,
            on_tap: None,
        }
    }

    #[must_use]
    pub fn controller(mut self, value: TabController) -> Self {
        self.controller = Some(value);
        self
    }

    #[must_use]
    pub fn selected_index(mut self, value: usize) -> Self {
        self.selected_index = value;
        self
    }

    #[must_use]
    pub fn is_scrollable(mut self, value: bool) -> Self {
        self.scrollable = value;
        self
    }

    #[must_use]
    pub fn tab_alignment(mut self, value: TabAlignment) -> Self {
        self.alignment = Some(value);
        self
    }

    #[must_use]
    pub fn indicator_size(mut self, value: TabBarIndicatorSize) -> Self {
        self.indicator_size = value;
        self
    }

    #[must_use]
    pub fn indicator_color(mut self, value: Color) -> Self {
        self.indicator_color = Some(value);
        self
    }

    #[must_use]
    pub fn on_tap(mut self, callback: impl Fn(usize) + 'static) -> Self {
        self.on_tap = Some(Rc::new(callback));
        self
    }
}

impl TabBar {
    fn build(&self, context: &incular_widgets::BuildContext<'_>) -> Widget {
        let theme = Theme::of_shared(context).unwrap_or_else(ThemeData::light_shared);
        let tab_theme = &theme.navigation().tab_bar_theme;
        let selected = self
            .controller
            .as_ref()
            .map_or(self.selected_index, TabController::index);
        let callback = self.on_tap.clone();
        let controller = self.controller.clone();
        let indicator = self
            .indicator_color
            .or(tab_theme.indicator_color)
            .unwrap_or(theme.core().color_scheme.primary);
        let indicator_weight = tab_theme.indicator_weight.unwrap_or(2.0).max(0.0);
        let scrollable = self.scrollable;
        let tab_alignment = self
            .alignment
            .or(tab_theme.tab_alignment)
            .unwrap_or(TabAlignment::Center);
        let alignment = match tab_alignment {
            TabAlignment::Start | TabAlignment::StartOffset => MainAxisAlignment::Start,
            TabAlignment::Center => MainAxisAlignment::Center,
            TabAlignment::Fill => MainAxisAlignment::SpaceEvenly,
        };
        let indicator_size = self.indicator_size;
        let children = self.tabs.iter().cloned().enumerate().map(|(index, tab)| {
            let enabled = tab.enabled;
            let is_selected = index == selected;
            let label_style = if is_selected {
                tab_theme.label_style.clone()
            } else {
                tab_theme.unselected_label_style.clone()
            };
            let label_color = if is_selected {
                tab_theme.label_color
            } else {
                tab_theme.unselected_label_color
            };
            let mut content = tab.content(label_style, label_color);
            if is_selected && indicator_size == TabBarIndicatorSize::Label && indicator_weight > 0.0
            {
                content = Column::new([
                    content,
                    Container::new()
                        .height(indicator_weight)
                        .color(indicator)
                        .into(),
                ])
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .into();
            }
            let mut button = crate::TextButton::with_child(content);
            button = button.enabled(enabled);
            if enabled {
                let controller = controller.clone();
                if let Some(callback) = callback.clone() {
                    button = button.on_click(move || {
                        if let Some(controller) = controller.as_ref() {
                            controller.set_index(index);
                        }
                        callback(index);
                    });
                } else if let Some(controller) = controller {
                    button = button.on_click(move || controller.set_index(index));
                }
            }
            let visual: Widget = button.into();
            if is_selected && indicator_size == TabBarIndicatorSize::Tab && indicator_weight > 0.0 {
                Stack::new([
                    visual,
                    Positioned::new(Container::new().color(indicator))
                        .left(0.0)
                        .right(0.0)
                        .bottom(0.0)
                        .height(indicator_weight)
                        .into(),
                ])
                .into()
            } else {
                visual
            }
        });
        let row = Row::new(children)
            .main_axis_alignment(alignment)
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .into();
        if scrollable {
            incular_widgets::SingleChildScrollView::new(row)
                .scroll_direction(incular_config::Axis::Horizontal)
                .into()
        } else {
            row
        }
    }
}

impl From<TabBar> for Widget {
    fn from(value: TabBar) -> Self {
        let value = Rc::new(value);
        let revision = value
            .controller
            .as_ref()
            .map(TabController::revision_cell)
            .unwrap_or_else(|| Rc::new(Cell::new(0)));
        Widget::stateful_layout_builder(revision, move |context, _| value.build(context))
    }
}

/// Tab content backed by the core `PageView` implementation.
#[derive(Clone, TypedBuilder)]
pub struct TabBarView {
    #[builder(
        default,
        setter(transform = |children: impl IntoIterator<Item = impl Into<Widget>>| {
            children.into_iter().map(Into::into).collect::<Vec<Widget>>()
        })
    )]
    children: Vec<Widget>,
    #[builder(default, setter(strip_option))]
    controller: Option<TabController>,
    #[builder(default = 1.0, setter(transform = |value: f32| value.max(0.01)))]
    viewport_fraction: f32,
    #[builder(default, setter(strip_option))]
    physics: Option<incular_scroll::ScrollPhysics>,
}

impl Default for TabBarView {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl TabBarView {
    #[must_use]
    pub fn new(children: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        Self {
            children: children.into_iter().map(Into::into).collect(),
            controller: None,
            viewport_fraction: 1.0,
            physics: None,
        }
    }

    #[must_use]
    pub fn controller(mut self, value: TabController) -> Self {
        self.controller = Some(value);
        self
    }

    #[must_use]
    pub fn viewport_fraction(mut self, value: f32) -> Self {
        self.viewport_fraction = value.max(0.01);
        self
    }

    #[must_use]
    pub fn physics(mut self, value: incular_scroll::ScrollPhysics) -> Self {
        self.physics = Some(value);
        self
    }
}

impl From<TabBarView> for Widget {
    fn from(value: TabBarView) -> Self {
        let controller = value
            .controller
            .unwrap_or_else(|| TabController::new(value.children.len()));
        let children = value.children;
        let viewport_fraction = value.viewport_fraction.max(0.01);
        let physics = value.physics;
        Widget::from(incular_widgets::LayoutBuilder::new(
            move |_, constraints| {
                let viewport_width = if constraints.is_width_bounded() {
                    constraints.max_width()
                } else {
                    constraints.min_width()
                };
                controller.set_page_extent(viewport_width * viewport_fraction);
                let mut page_view = PageView::new(children.clone())
                    .controller(controller.page_controller())
                    .viewport_fraction(viewport_fraction);
                if let Some(physics) = physics {
                    page_view = page_view.physics(physics);
                }
                page_view.into()
            },
        ))
    }
}

/// A small theme descriptor for tab bars. The full component theme is kept in
/// the Material foundation and this value is useful for explicit overrides.
#[derive(Clone, Debug, Default, PartialEq, TypedBuilder)]
pub struct TabBarThemeData {
    #[builder(default, setter(strip_option))]
    pub label_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    pub unselected_label_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    pub indicator_color: Option<Color>,
    #[builder(default, setter(transform = |value: f32| Some(value.max(0.0))))]
    pub indicator_weight: Option<f32>,
    #[builder(default, setter(strip_option))]
    pub divider_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    pub label_style: Option<TextStyle>,
    #[builder(default, setter(strip_option))]
    pub unselected_label_style: Option<TextStyle>,
    #[builder(default, setter(strip_option, into))]
    pub overlay_color: Option<StateProperty<Color>>,
    #[builder(default, setter(strip_option))]
    pub tab_alignment: Option<TabAlignment>,
}
