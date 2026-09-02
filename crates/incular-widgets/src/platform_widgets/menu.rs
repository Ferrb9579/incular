//! Platform menu descriptions, ownership, and deterministic backend bridges.
//!
//! Flutter's `PlatformMenuBar` is a retained top-level menu owner: one
//! delegate receives the current menu tree, owns the platform command IDs,
//! and is cleared when the widget leaves the tree.  Incular keeps that
//! lifecycle independent of any one native menu API.  A platform crate can
//! implement [`PlatformMenuDelegate`] by translating the portable snapshot to
//! its native commands; [`NoopPlatformMenuDelegate`] explicitly reports an
//! unsupported capability without pretending that menus were installed.

use super::{SizedBox, Widget};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    fmt,
    rc::{Rc, Weak},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_MENU_OWNER: AtomicU64 = AtomicU64::new(1);

/// Stable identity for a platform menu command.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MenuItemId(String);

impl MenuItemId {
    /// Creates an application-owned stable command ID.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the stable command string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn generated(path: &str) -> Self {
        Self(format!("incular.path.{path}"))
    }
}

impl From<&str> for MenuItemId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for MenuItemId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// Identity of the current menu-bar owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MenuOwnerId(u64);

impl MenuOwnerId {
    #[must_use]
    pub fn new() -> Self {
        Self(NEXT_MENU_OWNER.fetch_add(1, Ordering::Relaxed))
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl Default for MenuOwnerId {
    fn default() -> Self {
        Self::new()
    }
}

/// Portable modifier flags for a platform menu shortcut.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ShortcutModifiers(u8);

impl ShortcutModifiers {
    pub const ALT: Self = Self(1 << 0);
    pub const CONTROL: Self = Self(1 << 1);
    pub const META: Self = Self(1 << 2);
    pub const SHIFT: Self = Self(1 << 3);

    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0
    }

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// A portable keyboard shortcut attached to a menu item.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PlatformMenuShortcut {
    pub key: String,
    pub modifiers: ShortcutModifiers,
}

impl PlatformMenuShortcut {
    #[must_use]
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            modifiers: ShortcutModifiers::empty(),
        }
    }

    #[must_use]
    pub fn modifiers(mut self, modifiers: ShortcutModifiers) -> Self {
        self.modifiers = modifiers;
        self
    }
}

/// A selectable platform menu item.
pub struct PlatformMenuItem {
    id: Option<MenuItemId>,
    label: String,
    tooltip: Option<String>,
    shortcut: Option<PlatformMenuShortcut>,
    enabled: bool,
    on_selected: Option<Rc<dyn Fn()>>,
}

impl Clone for PlatformMenuItem {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            label: self.label.clone(),
            tooltip: self.tooltip.clone(),
            shortcut: self.shortcut.clone(),
            enabled: self.enabled,
            on_selected: self.on_selected.clone(),
        }
    }
}

impl fmt::Debug for PlatformMenuItem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PlatformMenuItem")
            .field("id", &self.id)
            .field("label", &self.label)
            .field("tooltip", &self.tooltip)
            .field("shortcut", &self.shortcut)
            .field("enabled", &self.enabled)
            .field("has_callback", &self.on_selected.is_some())
            .finish()
    }
}

impl PlatformMenuItem {
    /// Creates an enabled item with no callback.  A callback can be attached
    /// with [`Self::on_selected`]; no callback is intentionally disabled.
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            id: None,
            label: label.into(),
            tooltip: None,
            shortcut: None,
            enabled: true,
            on_selected: None,
        }
    }

    #[must_use]
    pub fn id(mut self, id: impl Into<MenuItemId>) -> Self {
        self.id = Some(id.into());
        self
    }

    #[must_use]
    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    #[must_use]
    pub fn shortcut(mut self, shortcut: PlatformMenuShortcut) -> Self {
        self.shortcut = Some(shortcut);
        self
    }

    #[must_use]
    pub fn on_selected(mut self, callback: impl Fn() + 'static) -> Self {
        self.on_selected = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    #[must_use]
    pub fn disabled(self) -> Self {
        self.enabled(false)
    }

    #[must_use]
    pub fn item_id(&self) -> Option<&MenuItemId> {
        self.id.as_ref()
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }
}

/// A group of leaf menu items.
#[derive(Clone, Debug)]
pub struct PlatformMenuItemGroup {
    items: Vec<PlatformMenuItem>,
}

impl PlatformMenuItemGroup {
    #[must_use]
    pub fn new(items: impl Into<Vec<PlatformMenuItem>>) -> Self {
        Self {
            items: items.into(),
        }
    }

    #[must_use]
    pub fn items(&self) -> &[PlatformMenuItem] {
        &self.items
    }
}

/// One entry below a platform menu.
#[derive(Clone, Debug)]
pub enum PlatformMenuEntry {
    Menu(PlatformMenu),
    Item(PlatformMenuItem),
    Group(PlatformMenuItemGroup),
    Divider,
}

impl From<PlatformMenu> for PlatformMenuEntry {
    fn from(value: PlatformMenu) -> Self {
        Self::Menu(value)
    }
}

impl From<PlatformMenuItem> for PlatformMenuEntry {
    fn from(value: PlatformMenuItem) -> Self {
        Self::Item(value)
    }
}

impl From<PlatformMenuItemGroup> for PlatformMenuEntry {
    fn from(value: PlatformMenuItemGroup) -> Self {
        Self::Group(value)
    }
}

/// A top-level or nested platform menu.
pub struct PlatformMenu {
    id: Option<MenuItemId>,
    label: String,
    tooltip: Option<String>,
    entries: Vec<PlatformMenuEntry>,
    on_open: Option<Rc<dyn Fn()>>,
    on_close: Option<Rc<dyn Fn()>>,
}

impl Clone for PlatformMenu {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            label: self.label.clone(),
            tooltip: self.tooltip.clone(),
            entries: self.entries.clone(),
            on_open: self.on_open.clone(),
            on_close: self.on_close.clone(),
        }
    }
}

impl fmt::Debug for PlatformMenu {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PlatformMenu")
            .field("id", &self.id)
            .field("label", &self.label)
            .field("tooltip", &self.tooltip)
            .field("entries", &self.entries)
            .field("has_on_open", &self.on_open.is_some())
            .field("has_on_close", &self.on_close.is_some())
            .finish()
    }
}

impl PlatformMenu {
    /// Creates a menu with its nested entries.
    #[must_use]
    pub fn new(label: impl Into<String>, entries: impl Into<Vec<PlatformMenuEntry>>) -> Self {
        Self {
            id: None,
            label: label.into(),
            tooltip: None,
            entries: entries.into(),
            on_open: None,
            on_close: None,
        }
    }

    /// Creates a menu from leaf items.
    #[must_use]
    pub fn with_items(label: impl Into<String>, items: impl Into<Vec<PlatformMenuItem>>) -> Self {
        Self::new(
            label,
            items
                .into()
                .into_iter()
                .map(PlatformMenuEntry::Item)
                .collect::<Vec<_>>(),
        )
    }

    #[must_use]
    pub fn id(mut self, id: impl Into<MenuItemId>) -> Self {
        self.id = Some(id.into());
        self
    }

    #[must_use]
    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    #[must_use]
    pub fn on_open(mut self, callback: impl Fn() + 'static) -> Self {
        self.on_open = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn on_close(mut self, callback: impl Fn() + 'static) -> Self {
        self.on_close = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn entries(&self) -> &[PlatformMenuEntry] {
        &self.entries
    }

    #[must_use]
    pub fn menu_id(&self) -> Option<&MenuItemId> {
        self.id.as_ref()
    }
}

/// A callback-free menu snapshot sent across the platform boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformMenuSnapshot {
    pub menus: Vec<PlatformMenuSnapshotNode>,
}

/// A serialized menu node. `selectable` distinguishes command items from menu
/// containers even when a menu is empty; `separator` distinguishes dividers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformMenuSnapshotNode {
    pub id: MenuItemId,
    pub label: String,
    pub tooltip: Option<String>,
    pub shortcut: Option<PlatformMenuShortcut>,
    pub enabled: bool,
    pub selectable: bool,
    pub separator: bool,
    pub children: Vec<Self>,
}

/// Capability and ownership result from a menu delegate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlatformMenuUpdate {
    Applied,
    Unchanged,
    NoOpUnsupported,
    RejectedOwnedByOther,
    /// The backend owns the requested platform but could not safely apply the
    /// native mutation (for example native command-space exhaustion or an OS
    /// menu construction failure). Existing installed state remains current.
    RejectedByPlatform,
}

/// One event produced by a native application-menu backend.
///
/// Native adapters report retained IDs rather than labels, positions, or
/// backend command integers. The controller resolves the event against its
/// current callback generation, so a command removed by an update cannot
/// accidentally call a replacement callback.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlatformMenuEvent {
    Selected(MenuItemId),
    Opened(MenuItemId),
    Closed(MenuItemId),
}

type PlatformMenuEventHandler = Rc<dyn Fn(PlatformMenuEvent)>;

/// Native menu bridge implemented by a platform crate.
pub trait PlatformMenuDelegate: 'static {
    /// Acquires exclusive top-level menu ownership.
    fn acquire(&self, owner: MenuOwnerId) -> PlatformMenuUpdate;

    /// Sends the current callback-free menu tree.
    fn set_menus(&self, owner: MenuOwnerId, snapshot: PlatformMenuSnapshot) -> PlatformMenuUpdate;

    /// Clears menus if this owner is still active.
    fn clear_menus(&self, owner: MenuOwnerId) -> PlatformMenuUpdate;

    /// Releases ownership after clear.
    fn release(&self, owner: MenuOwnerId) -> PlatformMenuUpdate;

    /// Installs or removes the retained event sink for this owner.
    ///
    /// Backends that are driven only through explicit controller dispatch may
    /// keep the default. Production native backends override this and emit
    /// [`PlatformMenuEvent`] after resolving their private native command IDs.
    fn set_event_handler(
        &self,
        _owner: MenuOwnerId,
        _handler: Option<PlatformMenuEventHandler>,
    ) -> PlatformMenuUpdate {
        PlatformMenuUpdate::Applied
    }

    /// Handles one desktop keyboard accelerator before ordinary retained key
    /// dispatch. Returning `true` makes the application-menu shortcut the
    /// authority for that key event, preventing a second in-tree shortcut from
    /// firing the same command.
    fn handle_shortcut(&self, _key: &str, _modifiers: ShortcutModifiers) -> bool {
        false
    }
}

/// Delegate that consistently no-ops when a backend has no menu capability.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoopPlatformMenuDelegate;

impl PlatformMenuDelegate for NoopPlatformMenuDelegate {
    fn acquire(&self, _owner: MenuOwnerId) -> PlatformMenuUpdate {
        PlatformMenuUpdate::NoOpUnsupported
    }

    fn set_menus(
        &self,
        _owner: MenuOwnerId,
        _snapshot: PlatformMenuSnapshot,
    ) -> PlatformMenuUpdate {
        PlatformMenuUpdate::NoOpUnsupported
    }

    fn clear_menus(&self, _owner: MenuOwnerId) -> PlatformMenuUpdate {
        PlatformMenuUpdate::NoOpUnsupported
    }

    fn release(&self, _owner: MenuOwnerId) -> PlatformMenuUpdate {
        PlatformMenuUpdate::NoOpUnsupported
    }

    fn set_event_handler(
        &self,
        _owner: MenuOwnerId,
        _handler: Option<PlatformMenuEventHandler>,
    ) -> PlatformMenuUpdate {
        PlatformMenuUpdate::NoOpUnsupported
    }
}

/// Late-bound per-menu bridge used by the default `PlatformMenuBar` path.
///
/// The application builds and mounts retained roots before a desktop event
/// loop owns native windows. Keeping this bridge in the retained tree lets the
/// desktop adapter attach the correct OS delegate later without a process
/// global or any application-supplied native handle.
#[derive(Default)]
struct DeferredPlatformMenuDelegate {
    target: RefCell<Option<Rc<dyn PlatformMenuDelegate>>>,
}

impl DeferredPlatformMenuDelegate {
    fn bind(&self, delegate: Rc<dyn PlatformMenuDelegate>) -> bool {
        let mut target = self.target.borrow_mut();
        if target
            .as_ref()
            .is_some_and(|current| Rc::ptr_eq(current, &delegate))
        {
            return false;
        }
        *target = Some(delegate);
        true
    }

    fn unbind(&self) -> bool {
        self.target.borrow_mut().take().is_some()
    }

    fn is_bound(&self) -> bool {
        self.target.borrow().is_some()
    }

    fn is_bound_to(&self, delegate: &Rc<dyn PlatformMenuDelegate>) -> bool {
        self.target
            .borrow()
            .as_ref()
            .is_some_and(|current| Rc::ptr_eq(current, delegate))
    }

    fn forward(
        &self,
        call: impl FnOnce(&dyn PlatformMenuDelegate) -> PlatformMenuUpdate,
    ) -> PlatformMenuUpdate {
        self.target
            .borrow()
            .as_ref()
            .map_or(PlatformMenuUpdate::NoOpUnsupported, |delegate| {
                call(delegate.as_ref())
            })
    }
}

impl PlatformMenuDelegate for DeferredPlatformMenuDelegate {
    fn acquire(&self, owner: MenuOwnerId) -> PlatformMenuUpdate {
        self.forward(|delegate| delegate.acquire(owner))
    }

    fn set_menus(&self, owner: MenuOwnerId, snapshot: PlatformMenuSnapshot) -> PlatformMenuUpdate {
        self.forward(|delegate| delegate.set_menus(owner, snapshot))
    }

    fn clear_menus(&self, owner: MenuOwnerId) -> PlatformMenuUpdate {
        self.forward(|delegate| delegate.clear_menus(owner))
    }

    fn release(&self, owner: MenuOwnerId) -> PlatformMenuUpdate {
        self.forward(|delegate| delegate.release(owner))
    }

    fn set_event_handler(
        &self,
        owner: MenuOwnerId,
        handler: Option<PlatformMenuEventHandler>,
    ) -> PlatformMenuUpdate {
        self.forward(|delegate| delegate.set_event_handler(owner, handler))
    }

    fn handle_shortcut(&self, key: &str, modifiers: ShortcutModifiers) -> bool {
        self.target
            .borrow()
            .as_ref()
            .is_some_and(|delegate| delegate.handle_shortcut(key, modifiers))
    }
}

/// Failure while flattening a menu tree into stable command IDs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlatformMenuBuildError {
    DuplicateId(MenuItemId),
}

impl fmt::Display for PlatformMenuBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateId(id) => {
                write!(formatter, "duplicate platform menu id {}", id.as_str())
            }
        }
    }
}

impl std::error::Error for PlatformMenuBuildError {}

#[derive(Clone, Default)]
struct MenuCallbackSet {
    selected: BTreeMap<MenuItemId, Rc<dyn Fn()>>,
    opened: BTreeMap<MenuItemId, Rc<dyn Fn()>>,
    closed: BTreeMap<MenuItemId, Rc<dyn Fn()>>,
}

struct PlatformMenuBarState {
    installed: Option<PlatformMenuSnapshot>,
    callbacks: MenuCallbackSet,
    last_update: Option<PlatformMenuUpdate>,
}

struct PlatformMenuBarInner {
    owner: MenuOwnerId,
    delegate: Rc<dyn PlatformMenuDelegate>,
    state: RefCell<PlatformMenuBarState>,
}

impl Drop for PlatformMenuBarInner {
    fn drop(&mut self) {
        if self.state.get_mut().installed.is_none() {
            return;
        }
        let _ = self.delegate.clear_menus(self.owner);
        let _ = self.delegate.set_event_handler(self.owner, None);
        let _ = self.delegate.release(self.owner);
    }
}

/// Retained ownership of one platform menu bar.
#[derive(Clone)]
pub struct PlatformMenuBarController {
    inner: Rc<PlatformMenuBarInner>,
}

impl fmt::Debug for PlatformMenuBarController {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PlatformMenuBarController")
            .field("owner", &self.owner())
            .field("installed", &self.snapshot().is_some())
            .finish()
    }
}

impl PlatformMenuBarController {
    #[must_use]
    pub fn new(delegate: Rc<dyn PlatformMenuDelegate>) -> Self {
        Self {
            inner: Rc::new(PlatformMenuBarInner {
                owner: MenuOwnerId::new(),
                delegate,
                state: RefCell::new(PlatformMenuBarState {
                    installed: None,
                    callbacks: MenuCallbackSet::default(),
                    last_update: None,
                }),
            }),
        }
    }

    #[must_use]
    pub fn owner(&self) -> MenuOwnerId {
        self.inner.owner
    }

    #[must_use]
    pub fn snapshot(&self) -> Option<PlatformMenuSnapshot> {
        self.inner.state.borrow().installed.clone()
    }

    #[must_use]
    pub fn last_update(&self) -> Option<PlatformMenuUpdate> {
        self.inner.state.borrow().last_update
    }

    /// Installs or updates menus, retaining callbacks locally for command
    /// dispatch even when the native delegate only sees a serializable tree.
    pub fn install(
        &self,
        menus: &[PlatformMenu],
    ) -> Result<PlatformMenuUpdate, PlatformMenuBuildError> {
        let (snapshot, callbacks) = build_snapshot(menus)?;
        let had_installed = self.inner.state.borrow().installed.is_some();
        if self.inner.state.borrow().installed.as_ref() == Some(&snapshot) {
            // Callback-only changes are intentionally committed even when the
            // native snapshot is unchanged. Native command identity remains
            // stable while future events resolve against the newest closure.
            let mut state = self.inner.state.borrow_mut();
            state.callbacks = callbacks;
            state.last_update = Some(PlatformMenuUpdate::Unchanged);
            return Ok(PlatformMenuUpdate::Unchanged);
        }
        let acquire = self.inner.delegate.acquire(self.inner.owner);
        if matches!(acquire, PlatformMenuUpdate::NoOpUnsupported) {
            self.inner.state.borrow_mut().last_update = Some(acquire);
            return Ok(acquire);
        }
        if matches!(acquire, PlatformMenuUpdate::RejectedOwnedByOther) {
            self.inner.state.borrow_mut().last_update = Some(acquire);
            return Ok(acquire);
        }
        let handler_update = self.inner.delegate.set_event_handler(
            self.inner.owner,
            Some(menu_event_handler(Rc::downgrade(&self.inner))),
        );
        if matches!(
            handler_update,
            PlatformMenuUpdate::NoOpUnsupported
                | PlatformMenuUpdate::RejectedOwnedByOther
                | PlatformMenuUpdate::RejectedByPlatform
        ) {
            if !had_installed {
                let _ = self.inner.delegate.release(self.inner.owner);
            }
            self.inner.state.borrow_mut().last_update = Some(handler_update);
            return Ok(handler_update);
        }
        let update = self
            .inner
            .delegate
            .set_menus(self.inner.owner, snapshot.clone());
        if matches!(
            update,
            PlatformMenuUpdate::Applied | PlatformMenuUpdate::Unchanged
        ) {
            let mut state = self.inner.state.borrow_mut();
            state.installed = Some(snapshot);
            state.callbacks = callbacks;
        } else if !had_installed {
            // Backend mutation is transactional from the controller's point of
            // view. When first installation fails, remove the provisional sink
            // and ownership. A rejected replacement keeps the previous sink and
            // callback generation because the previous native tree is still
            // authoritative.
            let _ = self
                .inner
                .delegate
                .set_event_handler(self.inner.owner, None);
            let _ = self.inner.delegate.release(self.inner.owner);
        }
        self.inner.state.borrow_mut().last_update = Some(update);
        Ok(update)
    }

    /// Clears native menus while keeping this controller reusable.
    pub fn detach(&self) -> PlatformMenuUpdate {
        let update = self.inner.delegate.clear_menus(self.inner.owner);
        let _ = self
            .inner
            .delegate
            .set_event_handler(self.inner.owner, None);
        let _ = self.inner.delegate.release(self.inner.owner);
        let mut state = self.inner.state.borrow_mut();
        state.installed = None;
        state.callbacks = MenuCallbackSet::default();
        state.last_update = Some(update);
        update
    }

    /// Dispatches a native command to its retained Dart-like callback.
    #[must_use]
    pub fn dispatch(&self, id: &MenuItemId) -> MenuDispatchResult {
        let callback = self
            .inner
            .state
            .borrow()
            .callbacks
            .selected
            .get(id)
            .cloned();
        dispatch_owned_callback(callback)
    }

    /// Runs the retained submenu-open callback.
    #[must_use]
    pub fn open(&self, id: &MenuItemId) -> MenuDispatchResult {
        let callback = self.inner.state.borrow().callbacks.opened.get(id).cloned();
        dispatch_owned_callback(callback)
    }

    /// Runs the retained submenu-close callback.
    #[must_use]
    pub fn close(&self, id: &MenuItemId) -> MenuDispatchResult {
        let callback = self.inner.state.borrow().callbacks.closed.get(id).cloned();
        dispatch_owned_callback(callback)
    }
}

fn menu_event_handler(inner: Weak<PlatformMenuBarInner>) -> PlatformMenuEventHandler {
    Rc::new(move |event| {
        let Some(inner) = inner.upgrade() else {
            return;
        };
        let callback = {
            let state = inner.state.borrow();
            match event {
                PlatformMenuEvent::Selected(id) => state.callbacks.selected.get(&id).cloned(),
                PlatformMenuEvent::Opened(id) => state.callbacks.opened.get(&id).cloned(),
                PlatformMenuEvent::Closed(id) => state.callbacks.closed.get(&id).cloned(),
            }
        };
        let _ = dispatch_owned_callback(callback);
    })
}

fn dispatch_owned_callback(callback: Option<Rc<dyn Fn()>>) -> MenuDispatchResult {
    callback.map_or(MenuDispatchResult::Unknown, |callback| {
        callback();
        MenuDispatchResult::Handled
    })
}

/// Result of dispatching a platform command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuDispatchResult {
    Handled,
    Unknown,
}

/// A top-level retained menu-bar widget.
#[derive(Clone)]
pub struct PlatformMenuBar {
    menus: Vec<PlatformMenu>,
    child: Option<Widget>,
    controller: PlatformMenuBarController,
    deferred_delegate: Rc<DeferredPlatformMenuDelegate>,
    auto_connect_native_delegate: bool,
}

impl fmt::Debug for PlatformMenuBar {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PlatformMenuBar")
            .field("menus", &self.menus)
            .field("has_child", &self.child.is_some())
            .field("controller", &self.controller)
            .finish()
    }
}

impl PlatformMenuBar {
    /// Creates a menu bar that is late-bound to the active platform menu
    /// backend when mounted by a native runner. Unsupported hosts retain the
    /// model and report [`PlatformMenuUpdate::NoOpUnsupported`].
    #[must_use]
    pub fn new(menus: impl Into<Vec<PlatformMenu>>, child: impl Into<Widget>) -> Self {
        let deferred_delegate = Rc::new(DeferredPlatformMenuDelegate::default());
        let controller = PlatformMenuBarController::new(deferred_delegate.clone());
        Self {
            menus: menus.into(),
            child: Some(child.into()),
            controller,
            deferred_delegate,
            auto_connect_native_delegate: true,
        }
    }

    /// Creates a menu bar connected to a native delegate.
    #[must_use]
    pub fn with_delegate(
        menus: impl Into<Vec<PlatformMenu>>,
        child: impl Into<Widget>,
        delegate: Rc<dyn PlatformMenuDelegate>,
    ) -> Self {
        let deferred_delegate = Rc::new(DeferredPlatformMenuDelegate::default());
        let _ = deferred_delegate.bind(delegate);
        Self {
            menus: menus.into(),
            child: Some(child.into()),
            controller: PlatformMenuBarController::new(deferred_delegate.clone()),
            deferred_delegate,
            auto_connect_native_delegate: false,
        }
    }

    /// Creates a menu bar with no visual child.
    #[must_use]
    pub fn without_child(menus: impl Into<Vec<PlatformMenu>>) -> Self {
        let deferred_delegate = Rc::new(DeferredPlatformMenuDelegate::default());
        Self {
            menus: menus.into(),
            child: None,
            controller: PlatformMenuBarController::new(deferred_delegate.clone()),
            deferred_delegate,
            auto_connect_native_delegate: true,
        }
    }

    #[must_use]
    pub fn controller(&self) -> PlatformMenuBarController {
        self.controller.clone()
    }

    #[must_use]
    pub fn menus(&self) -> &[PlatformMenu] {
        &self.menus
    }

    /// Replaces the menu tree through the retained owner.
    pub fn update(
        &mut self,
        menus: impl Into<Vec<PlatformMenu>>,
    ) -> Result<PlatformMenuUpdate, PlatformMenuBuildError> {
        self.menus = menus.into();
        self.controller.install(&self.menus)
    }

    /// Installs the menu tree and returns the visual child unchanged.
    #[must_use]
    pub fn into_widget(self) -> Widget {
        let child = self.child.unwrap_or_else(|| SizedBox::shrink().into());
        let binding = PlatformMenuBinding::new(
            self.controller,
            self.deferred_delegate,
            self.menus,
            self.auto_connect_native_delegate,
        );
        Widget::environment_scope(PlatformMenuRetainedMarker { binding }, child)
    }

    /// Alias for [`Self::into_widget`].
    #[must_use]
    pub fn widget(self) -> Widget {
        self.into_widget()
    }
}

struct PlatformMenuMountLease {
    controller: PlatformMenuBarController,
}

impl Drop for PlatformMenuMountLease {
    fn drop(&mut self) {
        let _ = self.controller.detach();
    }
}

/// Retained bridge discovered by desktop runners after the native window has
/// been created. This is an implementation seam, not application API.
#[doc(hidden)]
#[derive(Clone)]
pub struct PlatformMenuBinding {
    controller: PlatformMenuBarController,
    delegate: Rc<DeferredPlatformMenuDelegate>,
    state: Rc<RefCell<PlatformMenuBindingState>>,
    _lease: Rc<PlatformMenuMountLease>,
}

struct PlatformMenuBindingState {
    menus: Vec<PlatformMenu>,
    auto_connect_native_delegate: bool,
    explicit_delegate: Option<Rc<dyn PlatformMenuDelegate>>,
}

impl PlatformMenuBinding {
    fn new(
        controller: PlatformMenuBarController,
        delegate: Rc<DeferredPlatformMenuDelegate>,
        menus: Vec<PlatformMenu>,
        auto_connect_native_delegate: bool,
    ) -> Self {
        let lease = Rc::new(PlatformMenuMountLease {
            controller: controller.clone(),
        });
        let explicit_delegate = (!auto_connect_native_delegate)
            .then(|| delegate.target.borrow().clone())
            .flatten();
        Self {
            controller,
            delegate,
            state: Rc::new(RefCell::new(PlatformMenuBindingState {
                menus,
                auto_connect_native_delegate,
                explicit_delegate,
            })),
            _lease: lease,
        }
    }

    /// Keeps the retained owner/delegate while replacing the declarative menu
    /// model from a compatible widget rebuild. An already-bound delegate is
    /// synchronized immediately; a not-yet-bound native delegate is applied at
    /// the desktop adapter's next synchronization point.
    pub(crate) fn reconcile_from(&self, incoming: &Self) {
        let (menus, auto_connect_native_delegate, explicit_delegate) = {
            let incoming = incoming.state.borrow();
            (
                incoming.menus.clone(),
                incoming.auto_connect_native_delegate,
                incoming.explicit_delegate.clone(),
            )
        };
        let (mode_changed, explicit_target_changed) = {
            let current = self.state.borrow();
            (
                current.auto_connect_native_delegate != auto_connect_native_delegate,
                !auto_connect_native_delegate
                    && match (&current.explicit_delegate, &explicit_delegate) {
                        (Some(current), Some(incoming)) => !Rc::ptr_eq(current, incoming),
                        (None, None) => false,
                        _ => true,
                    },
            )
        };

        if mode_changed || explicit_target_changed {
            if self.delegate.is_bound() {
                let _ = self.controller.detach();
            }
            if auto_connect_native_delegate {
                let _ = self.delegate.unbind();
            } else if let Some(delegate) = explicit_delegate.clone() {
                let _ = self.delegate.bind(delegate);
            }
        }

        {
            let mut current = self.state.borrow_mut();
            current.menus = menus;
            current.auto_connect_native_delegate = auto_connect_native_delegate;
            current.explicit_delegate = explicit_delegate;
        }
        self.install_if_bound();
    }

    pub(crate) fn install_if_bound(&self) {
        if self.delegate.is_bound() {
            let _ = self
                .controller
                .install(self.state.borrow().menus.as_slice());
        }
    }

    #[must_use]
    pub(crate) fn auto_connect_native_delegate(&self) -> bool {
        self.state.borrow().auto_connect_native_delegate
    }

    #[must_use]
    pub fn owner(&self) -> MenuOwnerId {
        self.controller.owner()
    }

    /// Connects the retained owner to one application-scoped native delegate.
    /// Repeated synchronization is cheap and retries owners that were
    /// previously rejected while another window held the application menu.
    pub fn connect(
        &self,
        delegate: Rc<dyn PlatformMenuDelegate>,
    ) -> Result<PlatformMenuUpdate, PlatformMenuBuildError> {
        if !self.auto_connect_native_delegate() {
            return Ok(self
                .controller
                .last_update()
                .unwrap_or(PlatformMenuUpdate::NoOpUnsupported));
        }
        if self.delegate.is_bound() && !self.delegate.is_bound_to(&delegate) {
            let _ = self.controller.detach();
        }
        let _ = self.delegate.bind(delegate);
        self.controller
            .install(self.state.borrow().menus.as_slice())
    }
}

#[derive(Clone)]
pub(crate) struct PlatformMenuRetainedMarker {
    pub binding: PlatformMenuBinding,
}

impl From<PlatformMenuBar> for Widget {
    fn from(value: PlatformMenuBar) -> Self {
        value.into_widget()
    }
}

fn build_snapshot(
    menus: &[PlatformMenu],
) -> Result<(PlatformMenuSnapshot, MenuCallbackSet), PlatformMenuBuildError> {
    let mut ids = BTreeSet::new();
    let mut callbacks = MenuCallbackSet::default();
    let nodes = menus
        .iter()
        .enumerate()
        .map(|(index, menu)| {
            build_menu_node(menu, &format!("menu.{index}"), &mut ids, &mut callbacks)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((PlatformMenuSnapshot { menus: nodes }, callbacks))
}

fn build_menu_node(
    menu: &PlatformMenu,
    path: &str,
    ids: &mut BTreeSet<MenuItemId>,
    callbacks: &mut MenuCallbackSet,
) -> Result<PlatformMenuSnapshotNode, PlatformMenuBuildError> {
    let id = menu
        .id
        .clone()
        .unwrap_or_else(|| MenuItemId::generated(path));
    insert_id(ids, id.clone())?;
    if let Some(callback) = menu.on_open.clone() {
        callbacks.opened.insert(id.clone(), callback);
    }
    if let Some(callback) = menu.on_close.clone() {
        callbacks.closed.insert(id.clone(), callback);
    }
    let children = menu
        .entries
        .iter()
        .enumerate()
        .flat_map(|(index, entry)| match entry {
            PlatformMenuEntry::Menu(menu) => vec![build_menu_node(
                menu,
                &format!("{path}.menu.{index}"),
                ids,
                callbacks,
            )],
            PlatformMenuEntry::Item(item) => vec![build_item_node(
                item,
                &format!("{path}.item.{index}"),
                ids,
                callbacks,
            )],
            PlatformMenuEntry::Group(group) => group
                .items
                .iter()
                .enumerate()
                .map(|(group_index, item)| {
                    build_item_node(
                        item,
                        &format!("{path}.group.{index}.item.{group_index}"),
                        ids,
                        callbacks,
                    )
                })
                .collect::<Vec<_>>(),
            PlatformMenuEntry::Divider => vec![Ok(PlatformMenuSnapshotNode {
                id: MenuItemId::generated(&format!("{path}.divider.{index}")),
                label: String::new(),
                tooltip: None,
                shortcut: None,
                enabled: false,
                selectable: false,
                separator: true,
                children: Vec::new(),
            })],
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(PlatformMenuSnapshotNode {
        id,
        label: menu.label.clone(),
        tooltip: menu.tooltip.clone(),
        shortcut: None,
        enabled: true,
        selectable: false,
        separator: false,
        children,
    })
}

fn build_item_node(
    item: &PlatformMenuItem,
    path: &str,
    ids: &mut BTreeSet<MenuItemId>,
    callbacks: &mut MenuCallbackSet,
) -> Result<PlatformMenuSnapshotNode, PlatformMenuBuildError> {
    let id = item
        .id
        .clone()
        .unwrap_or_else(|| MenuItemId::generated(path));
    insert_id(ids, id.clone())?;
    if item.enabled
        && let Some(callback) = item.on_selected.clone()
    {
        callbacks.selected.insert(id.clone(), callback);
    }
    Ok(PlatformMenuSnapshotNode {
        id,
        label: item.label.clone(),
        tooltip: item.tooltip.clone(),
        shortcut: item.shortcut.clone(),
        enabled: item.enabled && item.on_selected.is_some(),
        selectable: true,
        separator: false,
        children: Vec::new(),
    })
}

fn insert_id(ids: &mut BTreeSet<MenuItemId>, id: MenuItemId) -> Result<(), PlatformMenuBuildError> {
    if ids.insert(id.clone()) {
        Ok(())
    } else {
        Err(PlatformMenuBuildError::DuplicateId(id))
    }
}
