use crate::feedback::SnackBar;
use incular_config::{Alignment, CrossAxisAlignment, EdgeInsets, MainAxisAlignment};
use incular_controls::overlay::Side;
use incular_controls::{ControlTheme, current_control_theme};
use incular_core::Color;
use incular_widgets::{BoxDecoration, Container, Icon, Row, Stack, Text, Widget};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::Instant;
use typed_builder::TypedBuilder;

/// Material bottom app bar.  A floating action button, when supplied, is
/// painted above the bar in a retained stack so it does not affect bar layout.
#[derive(Clone, TypedBuilder)]
pub struct BottomAppBar {
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
    #[builder(
        default = Vec::new(),
        setter(transform = |actions: impl IntoIterator<Item = impl Into<Widget>>| {
            actions.into_iter().map(Into::into).collect::<Vec<Widget>>()
        })
    )]
    actions: Vec<Widget>,
    #[builder(default, setter(strip_option, into))]
    floating_action_button: Option<Widget>,
    #[builder(default = 80.0, setter(transform = |height: f32| height.max(0.0)))]
    height: f32,
    #[builder(default, setter(strip_option))]
    background: Option<Color>,
    #[builder(default = EdgeInsets::symmetric(16.0, 8.0))]
    padding: EdgeInsets,
}

/// Header surface for a Material [`Drawer`].
///
/// Flutter's `DrawerHeader` is deliberately a small composition widget: the
/// drawer owns its route/gesture behavior while the header owns only the
/// padded, decorated slot at the top.  Keeping it here avoids introducing a
/// second drawer implementation in the Material layer.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct DrawerHeader {
    #[builder(setter(into))]
    pub child: Widget,
    #[builder(default, setter(strip_option))]
    pub decoration: Option<BoxDecoration>,
    #[builder(default = EdgeInsets::ZERO)]
    pub margin: EdgeInsets,
    #[builder(default = EdgeInsets::all(16.0))]
    pub padding: EdgeInsets,
}

/// Material's left-hand drawer surface.
///
/// The headless drawer root defaults to a right-side sheet for generic
/// desktop controls. Flutter's Material `Drawer` is left-aligned, so this
/// thin adapter fixes that Material default while delegating open/close,
/// barrier, and semantics behavior to the shared root.
#[derive(Clone, TypedBuilder)]
#[builder(mutators(
    pub fn child(self, child: impl Into<Widget>) {
        self.root = self.root.clone().child(child);
    }
    pub fn panel(self, panel: impl Into<Widget>) {
        self.root = self.root.clone().panel(panel);
    }
    pub fn open(self, value: bool) {
        self.root = self.root.clone().open(value);
    }
    pub fn default_open(self, value: bool) {
        self.root = self.root.clone().open(value);
    }
    pub fn width(self, value: f32) {
        self.root = self.root.clone().width(value);
    }
    pub fn modal(self, value: bool) {
        self.root = self.root.clone().modal(value);
    }
    pub fn on_open_change<F>(self, callback: F)
    where
        F: Fn(bool) + 'static,
    {
        self.root = self.root.clone().on_open_change(callback);
    }
))]
pub struct Drawer {
    #[builder(via_mutators = incular_controls::drawer::Root::new().side(Side::Left))]
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
        Widget::from(incular_widgets::LayoutBuilder::new(move |context, _| {
            value.build(&current_control_theme(context))
        }))
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
#[derive(Clone, TypedBuilder)]
pub struct ScaffoldMessenger {
    #[builder(setter(into))]
    child: Widget,
    #[builder(default = ScaffoldMessengerController::new(), setter(skip))]
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
        Widget::stateful_layout_builder(revision, move |context, _| {
            let shell = ScaffoldMessenger {
                child: child.clone(),
                controller: controller.clone(),
            };
            shell.build(&current_control_theme(context))
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

#[rustfmt::skip]
macro_rules! action_button {
    ($name:ident, $kind:ident) => {
        #[derive(Clone, TypedBuilder)]
        pub struct $name {
            #[builder(default = true)]
            enabled: bool,
            #[builder(default, setter(strip_option, into))]
            tooltip: Option<String>,
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
        }

        impl $name {
            #[must_use]
            pub fn new() -> Self {
                Self::builder().build()
            }

            #[must_use]
            pub fn enabled(mut self, enabled: bool) -> Self {
                self.enabled = enabled;
                self
            }

            #[must_use]
            pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
                self.tooltip = Some(tooltip.into());
                self
            }

            #[must_use]
            pub fn on_pressed(mut self, callback: impl Fn() + 'static) -> Self {
                self.on_pressed = Some(Rc::new(callback));
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
                ActionButtonSpec {
                    enabled: value.enabled,
                    tooltip: value.tooltip,
                    on_pressed: value.on_pressed,
                }
                .build(ActionButtonKind::$kind)
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
