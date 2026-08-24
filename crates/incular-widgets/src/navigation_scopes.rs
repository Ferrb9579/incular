//! Declarative navigation, restoration scopes, application roots, and multi-view widgets.

use crate::Widget;
use std::rc::Rc;

/// Declarative router managing top-level page routes.
#[derive(Clone)]
pub struct Router {
    child: Widget,
}

impl Router {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl From<Router> for Widget {
    fn from(value: Router) -> Self {
        value.child
    }
}

/// Intercepts back navigation and page pop requests.
#[derive(Clone)]
pub struct PopScope {
    can_pop: bool,
    on_pop_invoked: Option<Rc<dyn Fn(bool)>>,
    child: Widget,
}

impl PopScope {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            can_pop: true,
            on_pop_invoked: None,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn can_pop(mut self, can_pop: bool) -> Self {
        self.can_pop = can_pop;
        self
    }

    #[must_use]
    pub fn on_pop_invoked(mut self, callback: impl Fn(bool) + 'static) -> Self {
        self.on_pop_invoked = Some(Rc::new(callback));
        self
    }
}

impl From<PopScope> for Widget {
    fn from(value: PopScope) -> Self {
        value.child
    }
}

/// Handles navigator pop gestures and requests.
#[derive(Clone)]
pub struct NavigatorPopHandler {
    child: Widget,
}

impl NavigatorPopHandler {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl From<NavigatorPopHandler> for Widget {
    fn from(value: NavigatorPopHandler) -> Self {
        value.child
    }
}

/// Listens for platform hardware / system back button presses.
#[derive(Clone)]
pub struct BackButtonListener {
    child: Widget,
}

impl BackButtonListener {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl From<BackButtonListener> for Widget {
    fn from(value: BackButtonListener) -> Self {
        value.child
    }
}

/// An overlay portal that renders its overlay child in an ancestor `Overlay`.
#[derive(Clone)]
pub struct OverlayPortal {
    show_overlay: bool,
    overlay_child: Option<Widget>,
    child: Widget,
}

impl OverlayPortal {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            show_overlay: false,
            overlay_child: None,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn overlay_child(mut self, overlay: impl Into<Widget>) -> Self {
        self.overlay_child = Some(overlay.into());
        self
    }

    #[must_use]
    pub fn show(mut self, show: bool) -> Self {
        self.show_overlay = show;
        self
    }
}

impl From<OverlayPortal> for Widget {
    fn from(value: OverlayPortal) -> Self {
        value.child
    }
}

/// An animated modal barrier for dialog and popup dismissal.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimatedModalBarrier {
    dismissible: bool,
    child: Option<Widget>,
}

impl Default for AnimatedModalBarrier {
    fn default() -> Self {
        Self::new()
    }
}

impl AnimatedModalBarrier {
    #[must_use]
    pub fn new() -> Self {
        Self {
            dismissible: true,
            child: None,
        }
    }

    #[must_use]
    pub fn dismissible(mut self, dismissible: bool) -> Self {
        self.dismissible = dismissible;
        self
    }
}

impl From<AnimatedModalBarrier> for Widget {
    fn from(value: AnimatedModalBarrier) -> Self {
        value
            .child
            .unwrap_or_else(|| crate::SizedBox::shrink().into())
    }
}

/// Stores and restores the scroll offsets and state of unmounted pages.
#[derive(Clone, Debug, PartialEq)]
pub struct PageStorage {
    child: Widget,
}

impl PageStorage {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl From<PageStorage> for Widget {
    fn from(value: PageStorage) -> Self {
        value.child
    }
}

/// Establishes the root restoration scope for the application tree.
#[derive(Clone, Debug, PartialEq)]
pub struct RootRestorationScope {
    restoration_id: String,
    child: Widget,
}

impl RootRestorationScope {
    #[must_use]
    pub fn new(restoration_id: impl Into<String>, child: impl Into<Widget>) -> Self {
        Self {
            restoration_id: restoration_id.into(),
            child: child.into(),
        }
    }
}

impl From<RootRestorationScope> for Widget {
    fn from(value: RootRestorationScope) -> Self {
        value.child
    }
}

/// Establishes an unmanaged restoration scope without automatic synchronization.
#[derive(Clone, Debug, PartialEq)]
pub struct UnmanagedRestorationScope {
    child: Widget,
}

impl UnmanagedRestorationScope {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl From<UnmanagedRestorationScope> for Widget {
    fn from(value: UnmanagedRestorationScope) -> Self {
        value.child
    }
}

/// Top-level application bootstrap widget.
#[derive(Clone)]
pub struct WidgetsApp {
    home: Option<Widget>,
    title: String,
}

impl WidgetsApp {
    #[must_use]
    pub fn new(home: impl Into<Widget>) -> Self {
        Self {
            home: Some(home.into()),
            title: String::new(),
        }
    }

    #[must_use]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }
}

impl From<WidgetsApp> for Widget {
    fn from(value: WidgetsApp) -> Self {
        value
            .home
            .unwrap_or_else(|| crate::SizedBox::shrink().into())
    }
}

/// Multi-view top-level view container.
#[derive(Clone, Debug, PartialEq)]
pub struct View {
    child: Widget,
}

impl View {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl From<View> for Widget {
    fn from(value: View) -> Self {
        value.child
    }
}

/// Anchor for placing auxiliary views and windows.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewAnchor {
    child: Widget,
}

impl ViewAnchor {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl From<ViewAnchor> for Widget {
    fn from(value: ViewAnchor) -> Self {
        value.child
    }
}

/// Window / application title metadata widget.
#[derive(Clone, Debug, PartialEq)]
pub struct Title {
    title: String,
    child: Widget,
}

impl Title {
    #[must_use]
    pub fn new(title: impl Into<String>, child: impl Into<Widget>) -> Self {
        Self {
            title: title.into(),
            child: child.into(),
        }
    }
}

impl From<Title> for Widget {
    fn from(value: Title) -> Self {
        value.child
    }
}

/// Platform-native desktop top menu bar.
#[derive(Clone, Debug, PartialEq)]
pub struct PlatformMenuBar {
    child: Widget,
}

impl PlatformMenuBar {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl From<PlatformMenuBar> for Widget {
    fn from(value: PlatformMenuBar) -> Self {
        value.child
    }
}
