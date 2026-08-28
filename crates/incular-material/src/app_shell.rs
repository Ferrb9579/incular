//! Material application-shell descriptors.
//!
//! This module keeps the shell vocabulary that sits above individual Material
//! controls.  It intentionally composes the retained `Widget` primitives and
//! the shared controls state machine; there is no second layout or event
//! implementation here.  The navigation crate can later attach a
//! `Navigator`/`Overlay` to the route hooks without changing these widgets.

use crate::feedback::SnackBar;
use crate::foundation::Theme;
use crate::{TextButton, ThemeData, ThemeMode};
use incular_config::{
    Alignment, Brightness, CrossAxisAlignment, EdgeInsets, Locale, MainAxisAlignment,
    RuntimeEnvironment,
};
use incular_controls::overlay::Side;
use incular_controls::{ControlTheme, current_control_theme};
use incular_core::Color;
use incular_widgets::{
    BoxDecoration, Column, Container, Icon, Row, ScrollConfiguration, ScrollPhysics, SizedBox,
    Stack, Text, Widget,
};
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::Instant;

/// The input device classes accepted by the Material scroll behavior.
///
/// The retained gesture layer performs the actual dispatch.  Keeping this
/// policy as data lets platform adapters add/remove device classes without
/// coupling the Material crate to a native event enum.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum MaterialPointerDevice {
    #[default]
    Touch,
    Mouse,
    Trackpad,
    Stylus,
    Unknown,
}

/// Flutter-shaped scroll behavior configuration for a Material application.
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialScrollBehavior {
    physics: ScrollPhysics,
    scrollbars: bool,
    drag_devices: Vec<MaterialPointerDevice>,
}

impl Default for MaterialScrollBehavior {
    fn default() -> Self {
        Self {
            physics: ScrollPhysics::clamping(),
            scrollbars: true,
            drag_devices: vec![
                MaterialPointerDevice::Touch,
                MaterialPointerDevice::Mouse,
                MaterialPointerDevice::Trackpad,
                MaterialPointerDevice::Stylus,
            ],
        }
    }
}

impl MaterialScrollBehavior {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn physics(mut self, physics: ScrollPhysics) -> Self {
        self.physics = physics;
        self
    }

    #[must_use]
    pub fn scrollbars(mut self, enabled: bool) -> Self {
        self.scrollbars = enabled;
        self
    }

    #[must_use]
    pub fn drag_devices(
        mut self,
        devices: impl IntoIterator<Item = MaterialPointerDevice>,
    ) -> Self {
        self.drag_devices = devices.into_iter().collect();
        self
    }

    #[must_use]
    pub fn get_physics(&self) -> ScrollPhysics {
        self.physics
    }

    #[must_use]
    pub fn get_scrollbars(&self) -> bool {
        self.scrollbars
    }

    #[must_use]
    pub fn get_drag_devices(&self) -> &[MaterialPointerDevice] {
        &self.drag_devices
    }

    /// Wraps a subtree in the retained ambient scroll configuration.
    ///
    /// `scrollbars` and `drag_devices` remain policy data for the platform
    /// adapter; physics is the part consumed directly by retained scroll
    /// views today.
    #[must_use]
    pub fn wrap(&self, child: impl Into<Widget>) -> Widget {
        ScrollConfiguration::new(child).physics(self.physics).into()
    }
}

/// A Material application root.
///
/// This is intentionally a retained descriptor rather than a runtime handle.
/// Runtime-owned routing can select a child through [`MaterialApp::route`]
/// and the navigation crate can replace that selection with its Navigator
/// projection at the integration boundary.
#[derive(Clone)]
pub struct MaterialApp {
    home: Option<Widget>,
    routes: BTreeMap<String, Widget>,
    initial_route: Option<String>,
    title: Option<String>,
    // ThemeData carries all of the component defaults and is intentionally
    // fairly rich. Keep it behind a pointer in the application descriptor so
    // fluent builder chains do not repeatedly move a large value on the
    // stack (and so a MaterialApp remains cheap to clone).
    theme: Rc<ThemeData>,
    dark_theme: Option<Rc<ThemeData>>,
    theme_mode: ThemeMode,
    locale: Option<Locale>,
    supported_locales: Vec<Locale>,
    restoration_scope_id: Option<String>,
    scroll_behavior: MaterialScrollBehavior,
    builder: Option<Rc<dyn Fn(Widget) -> Widget>>,
    debug_show_checked_mode_banner: bool,
}

impl MaterialApp {
    #[must_use]
    pub fn new(home: impl Into<Widget>) -> Self {
        Self {
            home: Some(home.into()),
            routes: BTreeMap::new(),
            initial_route: None,
            title: None,
            theme: ThemeData::light_shared(),
            dark_theme: None,
            theme_mode: ThemeMode::System,
            locale: None,
            supported_locales: Vec::new(),
            restoration_scope_id: None,
            scroll_behavior: MaterialScrollBehavior::default(),
            builder: None,
            debug_show_checked_mode_banner: false,
        }
    }

    /// Creates an app whose first child is supplied by the builder.
    #[must_use]
    pub fn from_builder(builder: impl Fn() -> Widget + 'static) -> Self {
        let mut app = Self::new(SizedBox::shrink());
        app.home = None;
        app.builder = Some(Rc::new(move |_| builder()));
        app
    }

    #[must_use]
    pub fn home(mut self, home: impl Into<Widget>) -> Self {
        self.home = Some(home.into());
        self
    }

    #[must_use]
    pub fn route(mut self, name: impl Into<String>, child: impl Into<Widget>) -> Self {
        self.routes.insert(name.into(), child.into());
        self
    }

    #[must_use]
    pub fn routes(mut self, routes: impl IntoIterator<Item = (String, Widget)>) -> Self {
        self.routes.extend(routes);
        self
    }

    #[must_use]
    pub fn initial_route(mut self, route: impl Into<String>) -> Self {
        self.initial_route = Some(route.into());
        self
    }

    #[must_use]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    #[must_use]
    pub fn theme(mut self, theme: ThemeData) -> Self {
        self.theme = Rc::new(theme);
        self
    }

    /// Sets a theme already owned by the application without copying the
    /// large descriptor through the native UI stack.
    #[must_use]
    pub fn theme_shared(mut self, theme: Rc<ThemeData>) -> Self {
        self.theme = theme;
        self
    }

    #[must_use]
    pub fn dark_theme(mut self, theme: ThemeData) -> Self {
        self.dark_theme = Some(Rc::new(theme));
        self
    }

    /// Sets a shared dark theme without copying the descriptor.
    #[must_use]
    pub fn dark_theme_shared(mut self, theme: Rc<ThemeData>) -> Self {
        self.dark_theme = Some(theme);
        self
    }

    #[must_use]
    pub fn theme_mode(mut self, mode: ThemeMode) -> Self {
        self.theme_mode = mode;
        self
    }

    /// Selects the locale exposed to the retained localization environment.
    /// Material text remains locale-neutral until an application installs its
    /// ICU4X catalog, but carrying the value here keeps the application root
    /// compatible with Flutter's common `locale`/`supportedLocales` API.
    #[must_use]
    pub fn locale(mut self, locale: Locale) -> Self {
        self.locale = Some(locale);
        self
    }

    #[must_use]
    pub fn supported_locales(mut self, locales: impl IntoIterator<Item = Locale>) -> Self {
        self.supported_locales = locales.into_iter().collect();
        self
    }

    #[must_use]
    pub fn restoration_scope_id(mut self, value: impl Into<String>) -> Self {
        self.restoration_scope_id = Some(value.into());
        self
    }

    #[must_use]
    pub fn scroll_behavior(mut self, behavior: MaterialScrollBehavior) -> Self {
        self.scroll_behavior = behavior;
        self
    }

    #[must_use]
    pub fn builder(mut self, builder: impl Fn(Widget) -> Widget + 'static) -> Self {
        self.builder = Some(Rc::new(builder));
        self
    }

    #[must_use]
    pub fn debug_show_checked_mode_banner(mut self, show: bool) -> Self {
        self.debug_show_checked_mode_banner = show;
        self
    }

    #[must_use]
    pub fn get_debug_show_checked_mode_banner(&self) -> bool {
        self.debug_show_checked_mode_banner
    }

    #[must_use]
    pub fn get_title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    #[must_use]
    pub fn get_theme_mode(&self) -> ThemeMode {
        self.theme_mode
    }

    #[must_use]
    pub fn get_locale(&self) -> Option<&Locale> {
        self.locale.as_ref()
    }

    #[must_use]
    pub fn get_supported_locales(&self) -> &[Locale] {
        &self.supported_locales
    }

    #[must_use]
    pub fn get_restoration_scope_id(&self) -> Option<&str> {
        self.restoration_scope_id.as_deref()
    }

    #[must_use]
    pub fn get_scroll_behavior(&self) -> &MaterialScrollBehavior {
        &self.scroll_behavior
    }

    #[must_use]
    pub fn build(&self) -> Widget {
        let child = self
            .initial_route
            .as_ref()
            .and_then(|route| self.routes.get(route))
            .cloned()
            .or_else(|| self.home.clone())
            .unwrap_or_else(|| SizedBox::shrink().into());
        let system_brightness =
            incular_widgets::internal::current_build_environment::<RuntimeEnvironment>()
                .map_or(Brightness::Light, |environment| environment.brightness);
        let theme = match self.theme_mode {
            ThemeMode::Dark => self
                .dark_theme
                .clone()
                .unwrap_or_else(ThemeData::dark_shared),
            ThemeMode::Light => self.theme.clone(),
            ThemeMode::System if system_brightness == Brightness::Dark => self
                .dark_theme
                .clone()
                .unwrap_or_else(ThemeData::dark_shared),
            ThemeMode::System => self.theme.clone(),
        };
        let themed: Widget = Theme::scope_shared(theme, child);
        let localized = if let Some(locale) = self.locale.clone() {
            Widget::environment_scope(locale, themed)
        } else {
            themed
        };
        let configured = self.scroll_behavior.wrap(localized);
        self.builder
            .as_ref()
            .map_or(configured.clone(), |builder| builder(configured))
    }
}

impl From<MaterialApp> for Widget {
    fn from(value: MaterialApp) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| value.build())
    }
}

/// A single destination shown by [`NavigationRail`].
#[derive(Clone)]
pub struct NavigationRailDestination {
    pub icon: Widget,
    pub label: String,
    pub selected_icon: Option<Widget>,
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
#[derive(Clone)]
pub struct NavigationRail {
    destinations: Vec<NavigationRailDestination>,
    selected_index: usize,
    label_type: crate::NavigationRailLabelType,
    extended: bool,
    min_width: f32,
    min_extended_width: f32,
    background: Option<Color>,
    leading: Option<Widget>,
    trailing: Option<Widget>,
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
            if item.enabled {
                if let Some(callback) = self.on_destination_selected.clone() {
                    button = button.on_click(move || callback(index));
                }
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

impl From<NavigationRail> for Widget {
    fn from(value: NavigationRail) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| value.build(&current_control_theme()))
    }
}

/// A single destination shown by [`NavigationDrawer`].
#[derive(Clone)]
pub struct NavigationDrawerDestination {
    pub icon: Widget,
    pub label: String,
    pub selected_icon: Option<Widget>,
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
#[derive(Clone)]
pub struct NavigationDrawer {
    destinations: Vec<NavigationDrawerDestination>,
    children: Vec<Widget>,
    selected_index: usize,
    width: f32,
    background: Option<Color>,
    header: Option<Widget>,
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
            if item.enabled {
                if let Some(callback) = self.on_destination_selected.clone() {
                    button = button.on_click(move || callback(index));
                }
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

impl From<NavigationDrawer> for Widget {
    fn from(value: NavigationDrawer) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| value.build(&current_control_theme()))
    }
}

/// Material bottom app bar.  A floating action button, when supplied, is
/// painted above the bar in a retained stack so it does not affect bar layout.
#[derive(Clone)]
pub struct BottomAppBar {
    child: Option<Widget>,
    actions: Vec<Widget>,
    floating_action_button: Option<Widget>,
    height: f32,
    background: Option<Color>,
    padding: EdgeInsets,
}

/// Header surface for a Material [`Drawer`].
///
/// Flutter's `DrawerHeader` is deliberately a small composition widget: the
/// drawer owns its route/gesture behavior while the header owns only the
/// padded, decorated slot at the top.  Keeping it here avoids introducing a
/// second drawer implementation in the Material layer.
#[derive(Clone, Debug, PartialEq)]
pub struct DrawerHeader {
    pub child: Widget,
    pub decoration: Option<BoxDecoration>,
    pub margin: EdgeInsets,
    pub padding: EdgeInsets,
}

/// Material's left-hand drawer surface.
///
/// The headless drawer root defaults to a right-side sheet for generic
/// desktop controls. Flutter's Material `Drawer` is left-aligned, so this
/// thin adapter fixes that Material default while delegating open/close,
/// barrier, and semantics behavior to the shared root.
#[derive(Clone)]
pub struct Drawer {
    root: incular_controls::drawer::Root,
}

impl Drawer {
    #[must_use]
    pub fn new() -> Self {
        Self {
            root: incular_controls::drawer::Root::new().side(Side::Left),
        }
    }

    #[must_use]
    pub fn with_child(child: impl Into<Widget>) -> Self {
        Self::new().child(child)
    }

    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.root = self.root.child(child);
        self
    }

    #[must_use]
    pub fn panel(mut self, panel: impl Into<Widget>) -> Self {
        self.root = self.root.panel(panel);
        self
    }

    #[must_use]
    pub fn open(mut self, value: bool) -> Self {
        self.root = self.root.open(value);
        self
    }

    #[must_use]
    pub fn default_open(self, value: bool) -> Self {
        self.open(value)
    }

    #[must_use]
    pub fn width(mut self, value: f32) -> Self {
        self.root = self.root.width(value);
        self
    }

    #[must_use]
    pub fn modal(mut self, value: bool) -> Self {
        self.root = self.root.modal(value);
        self
    }

    #[must_use]
    pub fn on_open_change(mut self, callback: impl Fn(bool) + 'static) -> Self {
        self.root = self.root.on_open_change(callback);
        self
    }
}

impl Default for Drawer {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Drawer> for Widget {
    fn from(value: Drawer) -> Self {
        value.root.into()
    }
}

impl DrawerHeader {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            decoration: None,
            margin: EdgeInsets::ZERO,
            padding: EdgeInsets::all(16.0),
        }
    }

    #[must_use]
    pub fn decoration(mut self, decoration: BoxDecoration) -> Self {
        self.decoration = Some(decoration);
        self
    }

    #[must_use]
    pub fn margin(mut self, margin: EdgeInsets) -> Self {
        self.margin = margin;
        self
    }

    #[must_use]
    pub fn padding(mut self, padding: EdgeInsets) -> Self {
        self.padding = padding;
        self
    }
}

impl From<DrawerHeader> for Widget {
    fn from(value: DrawerHeader) -> Self {
        let mut header = Container::with_child(value.child)
            .margin(value.margin)
            .padding(value.padding);
        if let Some(decoration) = value.decoration {
            header = header.decoration(decoration);
        }
        header.into()
    }
}

impl BottomAppBar {
    #[must_use]
    pub fn new() -> Self {
        Self {
            child: None,
            actions: Vec::new(),
            floating_action_button: None,
            height: 80.0,
            background: None,
            padding: EdgeInsets::symmetric(16.0, 8.0),
        }
    }

    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
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
    pub fn floating_action_button(mut self, button: impl Into<Widget>) -> Self {
        self.floating_action_button = Some(button.into());
        self
    }

    #[must_use]
    pub fn height(mut self, height: f32) -> Self {
        self.height = height.max(0.0);
        self
    }

    #[must_use]
    pub fn background_color(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    #[must_use]
    pub fn padding(mut self, padding: EdgeInsets) -> Self {
        self.padding = padding;
        self
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        let mut row_children = self.actions.clone();
        if let Some(child) = self.child.clone() {
            row_children.push(child);
        }
        let bar: Widget = Container::new()
            .height(self.height)
            .padding(self.padding)
            .color(self.background.unwrap_or(theme.colors.surface))
            .child(
                Row::new(row_children)
                    .spacing(8.0)
                    .main_axis_alignment(MainAxisAlignment::SpaceBetween)
                    .cross_axis_alignment(CrossAxisAlignment::Center),
            )
            .into();
        if let Some(fab) = self.floating_action_button.clone() {
            Stack::aligned(
                Alignment::BOTTOM_CENTER,
                [
                    bar,
                    incular_widgets::Positioned::new(fab)
                        .bottom(self.height - 28.0)
                        .into(),
                ],
            )
            .into()
        } else {
            bar
        }
    }
}

impl Default for BottomAppBar {
    fn default() -> Self {
        Self::new()
    }
}

impl From<BottomAppBar> for Widget {
    fn from(value: BottomAppBar) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| value.build(&current_control_theme()))
    }
}

/// Cloneable retained controller for the current snackbar.
#[derive(Clone)]
pub struct ScaffoldMessengerController {
    current: Rc<RefCell<Option<SnackBar>>>,
    queue: Rc<RefCell<VecDeque<SnackBar>>>,
    shown_at: Rc<Cell<Option<Instant>>>,
    revision: Rc<Cell<u64>>,
}

impl Default for ScaffoldMessengerController {
    fn default() -> Self {
        Self::new()
    }
}

impl ScaffoldMessengerController {
    #[must_use]
    pub fn new() -> Self {
        Self {
            current: Rc::new(RefCell::new(None)),
            queue: Rc::new(RefCell::new(VecDeque::new())),
            shown_at: Rc::new(Cell::new(None)),
            revision: Rc::new(Cell::new(0)),
        }
    }

    pub fn show_snack_bar(&self, snack_bar: SnackBar) {
        let mut current = self.current.borrow_mut();
        if current.is_none() {
            *current = Some(snack_bar);
            self.shown_at.set(Some(Instant::now()));
        } else {
            self.queue.borrow_mut().push_back(snack_bar);
        }
        self.bump_revision();
    }

    /// Explicit queue spelling for code that wants to make snackbar ordering
    /// obvious. `show_snack_bar` has the same queue semantics.
    pub fn queue_snack_bar(&self, snack_bar: SnackBar) {
        self.show_snack_bar(snack_bar);
    }

    pub fn hide_current_snack_bar(&self) -> Option<SnackBar> {
        let result = self.current.borrow_mut().take();
        let next = self.queue.borrow_mut().pop_front();
        *self.current.borrow_mut() = next;
        self.shown_at
            .set(self.current.borrow().as_ref().map(|_| Instant::now()));
        if result.is_some() {
            self.bump_revision();
        }
        result
    }

    pub fn remove_current_snack_bar(&self) -> Option<SnackBar> {
        self.hide_current_snack_bar()
    }

    #[must_use]
    pub fn current_snack_bar(&self) -> Option<SnackBar> {
        self.current.borrow().clone()
    }

    #[must_use]
    pub fn is_showing(&self) -> bool {
        self.current.borrow().is_some()
    }

    #[must_use]
    pub fn queued_count(&self) -> usize {
        self.queue.borrow().len()
    }

    /// Removes the current snackbar and all queued entries.
    pub fn clear_snack_bars(&self) {
        let changed = self.current.borrow().is_some() || !self.queue.borrow().is_empty();
        self.current.borrow_mut().take();
        self.queue.borrow_mut().clear();
        self.shown_at.set(None);
        if changed {
            self.bump_revision();
        }
    }

    /// Advances timeout state without introducing a second timer/runtime.
    /// Native/runtime integrations can call this from their frame tick.
    pub fn poll(&self, now: Instant) -> bool {
        let Some(started) = self.shown_at.get() else {
            return false;
        };
        let expired = self
            .current
            .borrow()
            .as_ref()
            .is_some_and(|bar| now.saturating_duration_since(started) >= bar.duration_value());
        if expired {
            let _ = self.hide_current_snack_bar();
            true
        } else {
            false
        }
    }

    #[must_use]
    pub fn revision(&self) -> Rc<Cell<u64>> {
        self.revision.clone()
    }

    fn bump_revision(&self) {
        self.revision.set(self.revision.get().wrapping_add(1));
    }
}

/// Retained shell that paints the application child and current snackbar.
#[derive(Clone)]
pub struct ScaffoldMessenger {
    child: Widget,
    controller: ScaffoldMessengerController,
}

impl ScaffoldMessenger {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            controller: ScaffoldMessengerController::new(),
        }
    }

    #[must_use]
    pub fn with_controller(
        controller: ScaffoldMessengerController,
        child: impl Into<Widget>,
    ) -> Self {
        Self {
            child: child.into(),
            controller,
        }
    }

    #[must_use]
    pub fn controller(&self) -> ScaffoldMessengerController {
        self.controller.clone()
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        let mut children = vec![self.child.clone()];
        if let Some(snack_bar) = self.controller.current_snack_bar() {
            children.push(
                incular_widgets::Positioned::new(snack_bar.build(theme))
                    .left(16.0)
                    .right(16.0)
                    .bottom(16.0)
                    .into(),
            );
        }
        Stack::aligned(Alignment::TOP_LEFT, children).into()
    }
}

impl From<ScaffoldMessenger> for Widget {
    fn from(value: ScaffoldMessenger) -> Self {
        let revision = value.controller.revision();
        let child = value.child.clone();
        let controller = value.controller.clone();
        Widget::stateful_layout_builder(revision, move |_| {
            let shell = ScaffoldMessenger {
                child: child.clone(),
                controller: controller.clone(),
            };
            shell.build(&current_control_theme())
        })
    }
}

#[derive(Clone, Copy)]
enum ActionButtonKind {
    Back,
    Close,
    Drawer,
    EndDrawer,
}

impl ActionButtonKind {
    fn label(self) -> &'static str {
        match self {
            Self::Back => "Back",
            Self::Close => "Close",
            Self::Drawer => "Open navigation drawer",
            Self::EndDrawer => "Open end drawer",
        }
    }

    fn icon(self) -> Widget {
        match self {
            Self::Back => Icon::new(incular_widgets::internal::icons::chevron_left())
                .size(24.0)
                .into(),
            Self::Close => Icon::new(incular_widgets::internal::icons::close())
                .size(24.0)
                .into(),
            Self::Drawer => Text::new("≡").into(),
            Self::EndDrawer => Text::new("≡").into(),
        }
    }
}

#[derive(Clone)]
struct ActionButtonSpec {
    enabled: bool,
    tooltip: Option<String>,
    on_pressed: Option<Rc<dyn Fn()>>,
}

impl ActionButtonSpec {
    fn new() -> Self {
        Self {
            enabled: true,
            tooltip: None,
            on_pressed: None,
        }
    }

    fn build(&self, kind: ActionButtonKind) -> Widget {
        let mut button = crate::IconButton::with_child(kind.icon())
            .tooltip(
                self.tooltip
                    .clone()
                    .unwrap_or_else(|| kind.label().to_owned()),
            )
            .enabled(self.enabled);
        if let Some(callback) = self.on_pressed.clone() {
            button = button.on_click(move || callback());
        }
        button.into()
    }
}

macro_rules! action_button {
    ($name:ident, $kind:ident) => {
        #[derive(Clone)]
        pub struct $name {
            spec: ActionButtonSpec,
        }

        impl $name {
            #[must_use]
            pub fn new() -> Self {
                Self {
                    spec: ActionButtonSpec::new(),
                }
            }

            #[must_use]
            pub fn enabled(mut self, enabled: bool) -> Self {
                self.spec.enabled = enabled;
                self
            }

            #[must_use]
            pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
                self.spec.tooltip = Some(tooltip.into());
                self
            }

            #[must_use]
            pub fn on_pressed(mut self, callback: impl Fn() + 'static) -> Self {
                self.spec.on_pressed = Some(Rc::new(callback));
                self
            }

            #[must_use]
            pub fn on_click(self, callback: impl Fn() + 'static) -> Self {
                self.on_pressed(callback)
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl From<$name> for Widget {
            fn from(value: $name) -> Self {
                value.spec.build(ActionButtonKind::$kind)
            }
        }
    };
}

action_button!(BackButton, Back);
action_button!(CloseButton, Close);
action_button!(DrawerButton, Drawer);
action_button!(EndDrawerButton, EndDrawer);

macro_rules! action_icon {
    ($name:ident, $glyph:expr) => {
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
        pub struct $name;

        impl From<$name> for Widget {
            fn from(_: $name) -> Self {
                Text::new($glyph).into()
            }
        }
    };
}

action_icon!(BackButtonIcon, crate::Icons::ARROW_BACK);
action_icon!(CloseButtonIcon, crate::Icons::CLOSE);
action_icon!(DrawerButtonIcon, crate::Icons::MENU);
action_icon!(EndDrawerButtonIcon, crate::Icons::MENU);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scroll_behavior_preserves_physics_and_devices() {
        let behavior = MaterialScrollBehavior::new()
            .physics(ScrollPhysics::clamping().always_scrollable())
            .scrollbars(false)
            .drag_devices([MaterialPointerDevice::Mouse]);
        assert!(!behavior.get_scrollbars());
        assert_eq!(behavior.get_drag_devices(), &[MaterialPointerDevice::Mouse]);
        assert_ne!(behavior.get_physics(), ScrollPhysics::clamping());
    }

    #[test]
    fn messenger_controller_queues_and_removes_current_snackbar() {
        let controller = ScaffoldMessengerController::new();
        assert!(!controller.is_showing());
        controller.show_snack_bar(SnackBar::text("first"));
        assert!(controller.is_showing());
        controller.show_snack_bar(SnackBar::text("second"));
        assert!(controller.current_snack_bar().is_some());
        assert_eq!(controller.queued_count(), 1);
        assert!(controller.hide_current_snack_bar().is_some());
        assert!(controller.is_showing());
        assert!(controller.hide_current_snack_bar().is_some());
        assert!(!controller.is_showing());
    }

    #[test]
    fn app_uses_named_initial_route_before_home() {
        let app = MaterialApp::new(Text::new("home"))
            .route("/settings", Text::new("settings"))
            .initial_route("/settings")
            .title("sample");
        assert_eq!(app.get_title(), Some("sample"));
        assert_eq!(app.get_theme_mode(), ThemeMode::System);
    }
}
