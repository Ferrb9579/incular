//! Stack navigation, deep-link routing, route transitions, and overlays.
//!
//! The subsystem depends one-way on widget descriptions and retained
//! transition layers. It owns no widget tree or renderer state.

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use incular_core::Color;
use incular_widgets::{
    FadeTransition, OpacityController, SlideTransition, TranslationController, Widget,
};

/// Stable identity for a route in a [`Navigator`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RouteId(u64);

/// A reusable description of a navigable screen.
///
/// Unlike [`Route`], a page has no runtime identity, allowing the same page
/// description to be placed in multiple navigators.
#[derive(Clone)]
pub struct Page {
    pub name: String,
    pub child: Widget,
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
#[derive(Clone)]
pub struct Route {
    pub id: RouteId,
    pub name: String,
    pub child: Widget,
    pub transition: RouteTransition,
}
impl Route {
    #[must_use]
    pub fn new(name: impl Into<String>, child: impl Into<Widget>) -> Self {
        Self {
            id: RouteId(0),
            name: name.into(),
            child: child.into(),
            transition: RouteTransition::None,
        }
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

/// Maps external locations to declarative [`Page`] builders without coupling
/// navigation to URL, file-association, or platform APIs.
type PageBuilder = Rc<dyn Fn() -> Page>;
type RouteBuilders = Rc<RefCell<HashMap<String, PageBuilder>>>;

#[derive(Clone, Default)]
pub struct RouteRegistry {
    builders: RouteBuilders,
}
impl RouteRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    pub fn register(&self, location: impl Into<String>, builder: impl Fn() -> Page + 'static) {
        let location = location.into();
        self.builders
            .borrow_mut()
            .insert(normalize_location(&location), Rc::new(builder));
    }
    #[must_use]
    pub fn resolve(&self, location: &str) -> Option<Page> {
        self.builders
            .borrow()
            .get(&normalize_location(location))
            .map(|builder| builder())
    }
    pub fn navigate(&self, navigator: &Navigator, location: &str) -> Option<RouteId> {
        self.resolve(location).map(|page| navigator.push_page(page))
    }
}

fn normalize_location(location: &str) -> String {
    let location = location.trim();
    if location.is_empty() || location == "/" {
        "/".to_owned()
    } else if location.starts_with('/') {
        location.trim_end_matches('/').to_owned()
    } else {
        format!("/{}", location.trim_end_matches('/'))
    }
}

#[derive(Default)]
struct NavigatorState {
    next_id: u64,
    routes: Vec<Route>,
}

/// Cloneable stack navigator. Applications may drive it imperatively or
/// reconcile it from declarative page state.
#[derive(Clone, Default)]
pub struct Navigator {
    state: Rc<RefCell<NavigatorState>>,
}
impl Navigator {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    pub fn push(&self, mut route: Route) -> RouteId {
        let mut state = self.state.borrow_mut();
        state.next_id = state.next_id.wrapping_add(1).max(1);
        route.id = RouteId(state.next_id);
        let id = route.id;
        state.routes.push(route);
        id
    }
    pub fn push_page(&self, page: Page) -> RouteId {
        self.push(page.into())
    }
    /// Reconciles the stack to declarative pages. Names identify retained
    /// route positions, preserving their IDs while replacing child widgets.
    pub fn set_pages(&self, pages: impl IntoIterator<Item = Page>) {
        let mut state = self.state.borrow_mut();
        let mut previous = std::mem::take(&mut state.routes);
        let mut next = Vec::new();
        for page in pages {
            if let Some(index) = previous.iter().position(|route| route.name == page.name) {
                let mut route = previous.remove(index);
                route.child = page.child;
                next.push(route);
            } else {
                state.next_id = state.next_id.wrapping_add(1).max(1);
                next.push(Route {
                    id: RouteId(state.next_id),
                    name: page.name,
                    child: page.child,
                    transition: RouteTransition::None,
                });
            }
        }
        state.routes = next;
    }
    pub fn pop(&self) -> Option<Route> {
        self.state.borrow_mut().routes.pop()
    }
    pub fn replace(&self, route: Route) -> Option<Route> {
        let previous = self.pop();
        self.push(route);
        previous
    }
    #[must_use]
    pub fn current(&self) -> Option<Route> {
        self.state.borrow().routes.last().cloned()
    }
    #[must_use]
    pub fn routes(&self) -> Vec<Route> {
        self.state.borrow().routes.clone()
    }
    #[must_use]
    pub fn can_pop(&self) -> bool {
        self.state.borrow().routes.len() > 1
    }
}

/// Modal overlay configuration. The barrier is input-blocking by default.
#[derive(Clone)]
pub struct ModalBarrier {
    pub color: Color,
    pub dismissible: bool,
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
#[derive(Clone)]
pub struct OverlayEntry {
    pub child: Widget,
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
}

/// A modal dialog presentation owned by application overlay state.
#[derive(Clone)]
pub struct Dialog {
    pub child: Widget,
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
#[derive(Clone)]
pub struct BottomSheet {
    pub child: Widget,
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
    #[must_use]
    pub fn entries(&self) -> Vec<OverlayEntry> {
        self.entries.borrow().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use incular_core::Size;

    fn page() -> Widget {
        Widget::fixed_box(Size::new(1., 1.), Color::WHITE)
    }
    #[test]
    fn navigator_is_a_lifo_stack() {
        let navigator = Navigator::new();
        navigator.push(Route::new("home", page()));
        navigator.push(Route::new("details", page()));
        assert_eq!(navigator.current().unwrap().name, "details");
        assert_eq!(navigator.pop().unwrap().name, "details");
    }
    #[test]
    fn declarative_pages_preserve_named_route_identity() {
        let navigator = Navigator::new();
        navigator.set_pages([Page::new("home", page()), Page::new("settings", page())]);
        let id = navigator.routes()[0].id;
        navigator.set_pages([Page::new("home", page())]);
        assert_eq!(navigator.routes()[0].id, id);
    }
    #[test]
    fn registry_resolves_a_normalized_location() {
        let registry = RouteRegistry::new();
        registry.register("/settings", || Page::new("settings", page()));
        let navigator = Navigator::new();
        assert!(registry.navigate(&navigator, "settings/").is_some());
    }
    #[test]
    fn transitions_wrap_route_children_in_retained_layers() {
        let route = Route::new("details", page()).transition(RouteTransition::FadeSlide {
            opacity: OpacityController::new(),
            translation: TranslationController::new(),
        });
        let _: Widget = route.presented_child();
    }
    #[test]
    fn dialogs_and_bottom_sheets_are_modal_entries() {
        let overlay = Overlay::new();
        overlay.show_dialog(Dialog::new(page()));
        overlay.show_bottom_sheet(BottomSheet::new(page()).barrier(ModalBarrier {
            dismissible: false,
            ..ModalBarrier::default()
        }));
        let entries = overlay.entries();
        assert!(entries[0].barrier.as_ref().unwrap().dismissible);
        assert!(!entries[1].barrier.as_ref().unwrap().dismissible);
    }
}
