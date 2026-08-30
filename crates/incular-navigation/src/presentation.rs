use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

use incular_core::Color;
use incular_widgets::internal::{OpacityController, TranslationController};
use incular_widgets::{FadeTransition, SlideTransition, Widget};
use typed_builder::TypedBuilder;

use super::{RouteId, RouteSettings};

/// A reusable description of a navigable screen.
///
/// Unlike [`Route`], a page has no runtime identity, allowing the same page
/// description to be placed in multiple navigators.
#[derive(Clone, TypedBuilder)]
#[builder(builder_type(name = PageDescriptorBuilder))]
pub struct Page {
    #[builder(setter(into))]
    pub name: String,
    #[builder(setter(into))]
    pub child: Widget,
}

/// Rust-native route presentation composition.
///
/// These variants replace Flutter's `Route`/`ModalRoute`/`PopupRoute` class
/// hierarchy. Core owns the mechanics and Material can supply appearance.
#[derive(Clone)]
pub enum RoutePresentation {
    /// A normal page in the navigator's content area.
    Page {
        opaque: bool,
        maintain_state: bool,
        fullscreen_dialog: bool,
    },
    /// A non-fullscreen overlay presentation.
    Popup {
        barrier: Option<ModalBarrier>,
        maintain_state: bool,
    },
    /// A modal presentation that isolates background input.
    Modal {
        barrier: ModalBarrier,
        focus_trap: bool,
    },
    /// A route backed by one or more existing overlay entries.
    Overlay { entries: Vec<OverlayEntry> },
}

impl Default for RoutePresentation {
    fn default() -> Self {
        Self::Page {
            opaque: true,
            maintain_state: true,
            fullscreen_dialog: false,
        }
    }
}

impl RoutePresentation {
    #[must_use]
    pub const fn page() -> Self {
        Self::Page {
            opaque: true,
            maintain_state: true,
            fullscreen_dialog: false,
        }
    }

    #[must_use]
    pub fn popup(barrier: Option<ModalBarrier>) -> Self {
        Self::Popup {
            barrier,
            maintain_state: true,
        }
    }

    #[must_use]
    pub fn modal(barrier: ModalBarrier) -> Self {
        Self::Modal {
            barrier,
            focus_trap: true,
        }
    }

    #[must_use]
    pub fn overlay(entries: impl IntoIterator<Item = OverlayEntry>) -> Self {
        Self::Overlay {
            entries: entries.into_iter().collect(),
        }
    }

    #[must_use]
    pub const fn is_opaque(&self) -> bool {
        matches!(self, Self::Page { opaque: true, .. })
    }

    #[must_use]
    pub const fn blocks_background_input(&self) -> bool {
        matches!(self, Self::Modal { .. })
    }
}

/// A typed, retained completion channel for a route result.
///
/// The navigator remains synchronous and renderer-independent; applications
/// retain this handle and complete it from their route action. No `Any` or
/// untyped Dart `Object?` is needed.
#[derive(Clone)]
pub struct RouteResult<T> {
    state: Rc<RefCell<Option<T>>>,
}

impl<T> Default for RouteResult<T> {
    fn default() -> Self {
        Self {
            state: Rc::new(RefCell::new(None)),
        }
    }
}

impl<T> RouteResult<T> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Completes the result once. Repeated completion is rejected.
    pub fn complete(&self, value: T) -> Result<(), T> {
        let mut slot = self.state.borrow_mut();
        if slot.is_some() {
            return Err(value);
        }
        *slot = Some(value);
        Ok(())
    }

    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.state.borrow().is_some()
    }

    #[must_use]
    pub fn take(&self) -> Option<T> {
        self.state.borrow_mut().take()
    }
}
impl Page {
    #[must_use]
    pub fn new(name: impl Into<String>, child: impl Into<Widget>) -> Self {
        Self {
            name: name.into(),
            child: child.into(),
        }
    }
}

/// A declarative route: an application name and the widget it displays.
#[derive(Clone, TypedBuilder)]
pub struct Route {
    /// Session-local identity assigned by [`Navigator::push`].
    #[builder(default = RouteId(0), setter(skip))]
    pub id: RouteId,
    #[builder(setter(into))]
    pub name: String,
    #[builder(setter(into))]
    pub child: Widget,
    #[builder(default = RouteTransition::None)]
    pub transition: RouteTransition,
    /// Route metadata is initialized from `name` and can be changed through
    /// [`Route::settings`], which keeps the legacy name mirror synchronized.
    #[builder(default = RouteSettings::new((*name).clone()), setter(skip))]
    pub settings: RouteSettings,
    #[builder(default)]
    pub presentation: RoutePresentation,
}

impl Route {
    #[must_use]
    pub fn new(name: impl Into<String>, child: impl Into<Widget>) -> Self {
        let name = name.into();
        Self {
            id: RouteId(0),
            settings: RouteSettings::new(name.clone()),
            name,
            child: child.into(),
            transition: RouteTransition::None,
            presentation: RoutePresentation::default(),
        }
    }
    /// Replaces the route metadata and keeps the legacy `name` field in sync.
    #[must_use]
    pub fn settings(mut self, settings: RouteSettings) -> Self {
        self.name = settings.name().to_owned();
        self.settings = settings;
        self
    }

    #[must_use]
    pub fn presentation(mut self, presentation: RoutePresentation) -> Self {
        self.presentation = presentation;
        self
    }

    #[must_use]
    pub fn popup(self, barrier: Option<ModalBarrier>) -> Self {
        self.presentation(RoutePresentation::popup(barrier))
    }

    #[must_use]
    pub fn modal(self, barrier: ModalBarrier) -> Self {
        self.presentation(RoutePresentation::modal(barrier))
    }

    #[must_use]
    pub fn dialog(self) -> Self {
        self.modal(ModalBarrier::default())
    }

    #[must_use]
    pub fn overlay(self, entries: impl IntoIterator<Item = OverlayEntry>) -> Self {
        self.presentation(RoutePresentation::overlay(entries))
    }
    #[must_use]
    pub fn transition(mut self, transition: RouteTransition) -> Self {
        self.transition = transition;
        self
    }
    /// Returns the retained presentation with its transition layer applied.
    #[must_use]
    pub fn presented_child(&self) -> Widget {
        self.transition.apply(self.child.clone())
    }
}

/// Ergonomic builder for a page route with a custom transition.
#[derive(Clone, TypedBuilder)]
#[builder(builder_type(name = PageRouteDescriptorBuilder))]
pub struct PageRouteBuilder {
    #[builder(setter(into))]
    name: String,
    #[builder(setter(into))]
    child: Widget,
    #[builder(default = RouteTransition::None)]
    transition: RouteTransition,
    #[builder(default)]
    presentation: RoutePresentation,
    #[builder(default = RouteSettings::new((*name).clone()), setter(skip))]
    settings: RouteSettings,
}

impl PageRouteBuilder {
    #[must_use]
    pub fn new(name: impl Into<String>, child: impl Into<Widget>) -> Self {
        let name = name.into();
        Self {
            settings: RouteSettings::new(name.clone()),
            name,
            child: child.into(),
            transition: RouteTransition::None,
            presentation: RoutePresentation::default(),
        }
    }

    #[must_use]
    pub fn settings(mut self, settings: RouteSettings) -> Self {
        self.name = settings.name().to_owned();
        self.settings = settings;
        self
    }

    #[must_use]
    pub fn presentation(mut self, presentation: RoutePresentation) -> Self {
        self.presentation = presentation;
        self
    }

    #[must_use]
    pub fn transition(mut self, transition: RouteTransition) -> Self {
        self.transition = transition;
        self
    }

    #[must_use]
    pub fn build(self) -> Route {
        Route::new(self.name, self.child)
            .settings(self.settings)
            .transition(self.transition)
            .presentation(self.presentation)
    }
}
impl From<Page> for Route {
    fn from(page: Page) -> Self {
        Self::new(page.name, page.child)
    }
}

/// Route-level presentation transitions backed by retained controller layers.
#[derive(Clone)]
pub enum RouteTransition {
    None,
    Fade(OpacityController),
    Slide(TranslationController),
    FadeSlide {
        opacity: OpacityController,
        translation: TranslationController,
    },
}
impl RouteTransition {
    /// Creates a neutral compositor fade-in without exposing the retained
    /// opacity controller. The route owns the animation and the runtime ticks
    /// it along with the rest of the retained tree.
    #[must_use]
    pub fn fade_in(duration: Duration, now: Instant) -> Self {
        let opacity = OpacityController::new();
        opacity.set_opacity(0.);
        opacity.animate_to(1., duration, now);
        Self::Fade(opacity)
    }

    #[must_use]
    pub fn apply(&self, child: Widget) -> Widget {
        match self {
            Self::None => child,
            Self::Fade(controller) => FadeTransition::new(controller.clone(), child).into(),
            Self::Slide(controller) => SlideTransition::new(controller.clone(), child).into(),
            Self::FadeSlide {
                opacity,
                translation,
            } => FadeTransition::new(
                opacity.clone(),
                SlideTransition::new(translation.clone(), child),
            )
            .into(),
        }
    }
}

/// Modal overlay configuration. The barrier is input-blocking by default.
#[derive(Clone, Debug, PartialEq, Eq, TypedBuilder)]
pub struct ModalBarrier {
    #[builder(default = Color::rgba(0, 0, 0, 128))]
    pub color: Color,
    #[builder(default = true)]
    pub dismissible: bool,
    #[builder(default, setter(strip_option, into))]
    pub label: Option<String>,
}
impl Default for ModalBarrier {
    fn default() -> Self {
        Self {
            color: Color::rgba(0, 0, 0, 128),
            dismissible: true,
            label: None,
        }
    }
}

/// An entry in painter-ordered overlay state.
#[derive(Clone, TypedBuilder)]
pub struct OverlayEntry {
    #[builder(setter(into))]
    pub child: Widget,
    #[builder(default, setter(strip_option))]
    pub barrier: Option<ModalBarrier>,
}
impl OverlayEntry {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            barrier: None,
        }
    }
    #[must_use]
    pub fn modal(mut self, barrier: ModalBarrier) -> Self {
        self.barrier = Some(barrier);
        self
    }

    #[must_use]
    pub fn blocks_background_input(&self) -> bool {
        self.barrier.is_some()
    }

    #[must_use]
    pub fn dismissible(&self) -> bool {
        self.barrier
            .as_ref()
            .is_none_or(|barrier| barrier.dismissible)
    }
}

/// A modal dialog presentation owned by application overlay state.
#[derive(Clone, TypedBuilder)]
pub struct Dialog {
    #[builder(setter(into))]
    pub child: Widget,
    #[builder(default)]
    pub barrier: ModalBarrier,
}
impl Dialog {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            barrier: ModalBarrier::default(),
        }
    }
    #[must_use]
    pub fn barrier(mut self, barrier: ModalBarrier) -> Self {
        self.barrier = barrier;
        self
    }
    #[must_use]
    pub fn into_entry(self) -> OverlayEntry {
        OverlayEntry::new(self.child).modal(self.barrier)
    }
}

/// A modal bottom-sheet presentation. Positioning remains a widget concern.
#[derive(Clone, TypedBuilder)]
pub struct BottomSheet {
    #[builder(setter(into))]
    pub child: Widget,
    #[builder(default)]
    pub barrier: ModalBarrier,
}
impl BottomSheet {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            barrier: ModalBarrier::default(),
        }
    }
    #[must_use]
    pub fn barrier(mut self, barrier: ModalBarrier) -> Self {
        self.barrier = barrier;
        self
    }
    #[must_use]
    pub fn into_entry(self) -> OverlayEntry {
        OverlayEntry::new(self.child).modal(self.barrier)
    }
}

/// Cloneable painter-ordered overlay state.
#[derive(Clone, Default)]
pub struct Overlay {
    entries: Rc<RefCell<Vec<OverlayEntry>>>,
}
impl Overlay {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    pub fn insert(&self, entry: OverlayEntry) {
        self.entries.borrow_mut().push(entry);
    }
    pub fn show_dialog(&self, dialog: Dialog) {
        self.insert(dialog.into_entry());
    }
    pub fn show_bottom_sheet(&self, sheet: BottomSheet) {
        self.insert(sheet.into_entry());
    }
    pub fn remove_top(&self) -> Option<OverlayEntry> {
        self.entries.borrow_mut().pop()
    }

    /// Removes the top entry only when it has a dismissible barrier.
    pub fn dismiss_top(&self) -> Option<OverlayEntry> {
        let dismissible = self
            .entries
            .borrow()
            .last()
            .is_some_and(OverlayEntry::dismissible);
        dismissible.then(|| self.remove_top()).flatten()
    }

    /// Whether the current overlay stack must consume pointer/keyboard input
    /// before it reaches the route underneath it.
    #[must_use]
    pub fn blocks_background_input(&self) -> bool {
        self.entries
            .borrow()
            .iter()
            .rev()
            .any(OverlayEntry::blocks_background_input)
    }
    #[must_use]
    pub fn entries(&self) -> Vec<OverlayEntry> {
        self.entries.borrow().clone()
    }
}
