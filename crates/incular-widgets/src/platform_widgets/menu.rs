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
    rc::Rc,
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

/// A serialized menu node.  `separator` distinguishes `Divider` entries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformMenuSnapshotNode {
    pub id: MenuItemId,
    pub label: String,
    pub tooltip: Option<String>,
    pub shortcut: Option<PlatformMenuShortcut>,
    pub enabled: bool,
    pub separator: bool,
    pub children: Vec<Self>,
}

/// A command emitted to a native menu adapter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlatformMenuCommand {
    SetMenus {
        owner: MenuOwnerId,
        snapshot: PlatformMenuSnapshot,
    },
    ClearMenus {
        owner: MenuOwnerId,
    },
    Invoke {
        owner: MenuOwnerId,
        id: MenuItemId,
    },
}

/// Capability and ownership result from a menu delegate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlatformMenuUpdate {
    Applied,
    Unchanged,
    NoOpUnsupported,
    RejectedOwnedByOther,
}

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

    /// Forwards a command invocation when a backend exposes an explicit
    /// command channel.  Menu bars that deliver callbacks directly may keep
    /// the deterministic default.
    fn invoke(&self, _owner: MenuOwnerId, _id: MenuItemId) -> PlatformMenuUpdate {
        PlatformMenuUpdate::NoOpUnsupported
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

    fn invoke(&self, _owner: MenuOwnerId, _id: MenuItemId) -> PlatformMenuUpdate {
        PlatformMenuUpdate::NoOpUnsupported
    }
}

struct MemoryMenuState {
    supported: bool,
    owner: Option<MenuOwnerId>,
    snapshot: Option<PlatformMenuSnapshot>,
    commands: Vec<PlatformMenuCommand>,
}

/// Deterministic in-memory delegate useful for tests and embedded adapters.
#[derive(Clone)]
pub struct MemoryPlatformMenuDelegate {
    state: Rc<RefCell<MemoryMenuState>>,
}

impl fmt::Debug for MemoryPlatformMenuDelegate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = self.state.borrow();
        formatter
            .debug_struct("MemoryPlatformMenuDelegate")
            .field("supported", &state.supported)
            .field("owner", &state.owner)
            .field("command_count", &state.commands.len())
            .finish()
    }
}

impl MemoryPlatformMenuDelegate {
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Rc::new(RefCell::new(MemoryMenuState {
                supported: true,
                owner: None,
                snapshot: None,
                commands: Vec::new(),
            })),
        }
    }

    /// Creates a delegate that reports the unsupported capability.
    #[must_use]
    pub fn unsupported() -> Self {
        let delegate = Self::new();
        delegate.state.borrow_mut().supported = false;
        delegate
    }

    #[must_use]
    pub fn owner(&self) -> Option<MenuOwnerId> {
        self.state.borrow().owner
    }

    #[must_use]
    pub fn snapshot(&self) -> Option<PlatformMenuSnapshot> {
        self.state.borrow().snapshot.clone()
    }

    #[must_use]
    pub fn commands(&self) -> Vec<PlatformMenuCommand> {
        self.state.borrow().commands.clone()
    }

    pub fn clear_commands(&self) {
        self.state.borrow_mut().commands.clear();
    }
}

impl Default for MemoryPlatformMenuDelegate {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformMenuDelegate for MemoryPlatformMenuDelegate {
    fn acquire(&self, owner: MenuOwnerId) -> PlatformMenuUpdate {
        let mut state = self.state.borrow_mut();
        if !state.supported {
            return PlatformMenuUpdate::NoOpUnsupported;
        }
        match state.owner {
            None => {
                state.owner = Some(owner);
                PlatformMenuUpdate::Applied
            }
            Some(current) if current == owner => {
                state.owner = Some(owner);
                PlatformMenuUpdate::Applied
            }
            Some(_) => PlatformMenuUpdate::RejectedOwnedByOther,
        }
    }

    fn set_menus(&self, owner: MenuOwnerId, snapshot: PlatformMenuSnapshot) -> PlatformMenuUpdate {
        let mut state = self.state.borrow_mut();
        if !state.supported {
            return PlatformMenuUpdate::NoOpUnsupported;
        }
        if state.owner != Some(owner) {
            return PlatformMenuUpdate::RejectedOwnedByOther;
        }
        if state.snapshot.as_ref() == Some(&snapshot) {
            return PlatformMenuUpdate::Unchanged;
        }
        state.snapshot = Some(snapshot.clone());
        state
            .commands
            .push(PlatformMenuCommand::SetMenus { owner, snapshot });
        PlatformMenuUpdate::Applied
    }

    fn clear_menus(&self, owner: MenuOwnerId) -> PlatformMenuUpdate {
        let mut state = self.state.borrow_mut();
        if !state.supported {
            return PlatformMenuUpdate::NoOpUnsupported;
        }
        if state.owner != Some(owner) {
            return PlatformMenuUpdate::RejectedOwnedByOther;
        }
        if state.snapshot.take().is_some() {
            state
                .commands
                .push(PlatformMenuCommand::ClearMenus { owner });
        }
        PlatformMenuUpdate::Applied
    }

    fn release(&self, owner: MenuOwnerId) -> PlatformMenuUpdate {
        let mut state = self.state.borrow_mut();
        if !state.supported {
            return PlatformMenuUpdate::NoOpUnsupported;
        }
        if state.owner == Some(owner) {
            state.owner = None;
            PlatformMenuUpdate::Applied
        } else {
            PlatformMenuUpdate::RejectedOwnedByOther
        }
    }

    fn invoke(&self, owner: MenuOwnerId, id: MenuItemId) -> PlatformMenuUpdate {
        let mut state = self.state.borrow_mut();
        if !state.supported {
            return PlatformMenuUpdate::NoOpUnsupported;
        }
        if state.owner != Some(owner) {
            return PlatformMenuUpdate::RejectedOwnedByOther;
        }
        state
            .commands
            .push(PlatformMenuCommand::Invoke { owner, id });
        PlatformMenuUpdate::Applied
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

#[derive(Clone)]
struct MenuCallbackSet {
    selected: BTreeMap<MenuItemId, Rc<dyn Fn()>>,
    opened: BTreeMap<MenuItemId, Rc<dyn Fn()>>,
    closed: BTreeMap<MenuItemId, Rc<dyn Fn()>>,
}

impl Default for MenuCallbackSet {
    fn default() -> Self {
        Self {
            selected: BTreeMap::new(),
            opened: BTreeMap::new(),
            closed: BTreeMap::new(),
        }
    }
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
        let _ = self.delegate.clear_menus(self.owner);
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
        let mut state = self.inner.state.borrow_mut();
        state.callbacks = callbacks;
        if state.installed.as_ref() == Some(&snapshot) {
            state.last_update = Some(PlatformMenuUpdate::Unchanged);
            return Ok(PlatformMenuUpdate::Unchanged);
        }
        drop(state);
        let acquire = self.inner.delegate.acquire(self.inner.owner);
        if matches!(acquire, PlatformMenuUpdate::NoOpUnsupported) {
            self.inner.state.borrow_mut().last_update = Some(acquire);
            return Ok(acquire);
        }
        if matches!(acquire, PlatformMenuUpdate::RejectedOwnedByOther) {
            self.inner.state.borrow_mut().last_update = Some(acquire);
            return Ok(acquire);
        }
        let update = self
            .inner
            .delegate
            .set_menus(self.inner.owner, snapshot.clone());
        if matches!(
            update,
            PlatformMenuUpdate::Applied | PlatformMenuUpdate::Unchanged
        ) {
            self.inner.state.borrow_mut().installed = Some(snapshot);
        }
        self.inner.state.borrow_mut().last_update = Some(update);
        Ok(update)
    }

    /// Clears native menus while keeping this controller reusable.
    pub fn detach(&self) -> PlatformMenuUpdate {
        let update = self.inner.delegate.clear_menus(self.inner.owner);
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
        match callback {
            Some(callback) => {
                let _ = self.inner.delegate.invoke(self.inner.owner, id.clone());
                callback();
                MenuDispatchResult::Handled
            }
            None => MenuDispatchResult::Unknown,
        }
    }

    /// Runs the retained submenu-open callback.
    #[must_use]
    pub fn open(&self, id: &MenuItemId) -> MenuDispatchResult {
        dispatch_callback(&self.inner.state.borrow().callbacks.opened, id)
    }

    /// Runs the retained submenu-close callback.
    #[must_use]
    pub fn close(&self, id: &MenuItemId) -> MenuDispatchResult {
        dispatch_callback(&self.inner.state.borrow().callbacks.closed, id)
    }
}

fn dispatch_callback(
    callbacks: &BTreeMap<MenuItemId, Rc<dyn Fn()>>,
    id: &MenuItemId,
) -> MenuDispatchResult {
    callbacks
        .get(id)
        .map_or(MenuDispatchResult::Unknown, |callback| {
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
    /// Creates a menu bar using the deterministic unsupported delegate.
    #[must_use]
    pub fn new(menus: impl Into<Vec<PlatformMenu>>, child: impl Into<Widget>) -> Self {
        Self::with_delegate(menus, child, Rc::new(NoopPlatformMenuDelegate))
    }

    /// Creates a menu bar connected to a native delegate.
    #[must_use]
    pub fn with_delegate(
        menus: impl Into<Vec<PlatformMenu>>,
        child: impl Into<Widget>,
        delegate: Rc<dyn PlatformMenuDelegate>,
    ) -> Self {
        Self {
            menus: menus.into(),
            child: Some(child.into()),
            controller: PlatformMenuBarController::new(delegate),
        }
    }

    /// Creates a menu bar with no visual child.
    #[must_use]
    pub fn without_child(menus: impl Into<Vec<PlatformMenu>>) -> Self {
        Self {
            menus: menus.into(),
            child: None,
            controller: PlatformMenuBarController::new(Rc::new(NoopPlatformMenuDelegate)),
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
        &self,
        menus: impl Into<Vec<PlatformMenu>>,
    ) -> Result<PlatformMenuUpdate, PlatformMenuBuildError> {
        let menus = menus.into();
        self.controller.install(&menus)
    }

    /// Installs the menu tree and returns the visual child unchanged.
    #[must_use]
    pub fn into_widget(self) -> Widget {
        let _ = self.controller.install(&self.menus);
        self.child.unwrap_or_else(|| SizedBox::shrink().into())
    }

    /// Alias for [`Self::into_widget`].
    #[must_use]
    pub fn widget(self) -> Widget {
        self.into_widget()
    }
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
    if item.enabled {
        if let Some(callback) = item.on_selected.clone() {
            callbacks.selected.insert(id.clone(), callback);
        }
    }
    Ok(PlatformMenuSnapshotNode {
        id,
        label: item.label.clone(),
        tooltip: item.tooltip.clone(),
        shortcut: item.shortcut.clone(),
        enabled: item.enabled && item.on_selected.is_some(),
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
