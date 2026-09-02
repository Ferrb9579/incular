use incular_desktop::{
    NativeMenuCommandRegistry, matching_menu_shortcut, native_menu_structure_equal,
};
use incular_widgets::{
    MenuItemId, MenuOwnerId, PlatformMenuDelegate, PlatformMenuEvent, PlatformMenuSnapshot,
    PlatformMenuSnapshotNode, PlatformMenuUpdate, ShortcutModifiers,
};
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{ClassType, DeclaredClass, declare_class, msg_send_id, mutability, sel};
use objc2_app_kit::{NSApplication, NSEventModifierFlags, NSMenu, NSMenuDelegate, NSMenuItem};
use objc2_foundation::{MainThreadMarker, NSObject, NSObjectProtocol, NSString};
use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    rc::Rc,
};

#[derive(Clone, Copy, Debug)]
enum NativeMenuMessage {
    Command(u32),
    Opened(usize),
    Closed(usize),
}

struct DispatcherIvars {
    messages: Rc<RefCell<VecDeque<NativeMenuMessage>>>,
}

declare_class!(
    struct PlatformMenuDispatcher;

    // SAFETY:
    // - NSObject has no additional subclassing requirements.
    // - AppKit targets/delegates are main-thread-only objects.
    // - The dispatcher owns only ordinary Rust ivars and implements no custom
    //   deallocation contract.
    unsafe impl ClassType for PlatformMenuDispatcher {
        type Super = NSObject;
        type Mutability = mutability::MainThreadOnly;
        const NAME: &'static str = "IncularPlatformMenuDispatcher";
    }

    impl DeclaredClass for PlatformMenuDispatcher {
        type Ivars = DispatcherIvars;
    }

    unsafe impl NSObjectProtocol for PlatformMenuDispatcher {}

    unsafe impl PlatformMenuDispatcher {
        #[method(incularMenuItemSelected:)]
        fn selected(&self, sender: &NSMenuItem) {
            // SAFETY: AppKit invokes this action with the NSMenuItem whose tag
            // was assigned from the generation-safe native command registry.
            let tag = unsafe { sender.tag() };
            if let Ok(command) = u32::try_from(tag) {
                self.ivars()
                    .messages
                    .borrow_mut()
                    .push_back(NativeMenuMessage::Command(command));
            }
        }
    }

    unsafe impl NSMenuDelegate for PlatformMenuDispatcher {
        #[method(menuWillOpen:)]
        fn menu_will_open(&self, menu: &NSMenu) {
            self.ivars()
                .messages
                .borrow_mut()
                .push_back(NativeMenuMessage::Opened(menu as *const NSMenu as usize));
        }

        #[method(menuDidClose:)]
        fn menu_did_close(&self, menu: &NSMenu) {
            self.ivars()
                .messages
                .borrow_mut()
                .push_back(NativeMenuMessage::Closed(menu as *const NSMenu as usize));
        }
    }
);

impl PlatformMenuDispatcher {
    fn new(
        messages: Rc<RefCell<VecDeque<NativeMenuMessage>>>,
        mtm: MainThreadMarker,
    ) -> Retained<Self> {
        let this = mtm.alloc().set_ivars(DispatcherIvars { messages });
        // SAFETY: NSObject's designated init is valid for this subclass after
        // the Rust ivars have been initialized.
        unsafe { msg_send_id![super(this), init] }
    }
}

#[derive(Default)]
struct MacosMenuState {
    owner: Option<MenuOwnerId>,
    snapshot: Option<PlatformMenuSnapshot>,
    event_handler: Option<Rc<dyn Fn(PlatformMenuEvent)>>,
    registry: NativeMenuCommandRegistry,
    baseline: Option<BaselineMainMenu>,
    menu_ids: HashMap<usize, MenuItemId>,
    dispatcher: Option<Retained<PlatformMenuDispatcher>>,
}

struct BaselineMainMenu {
    /// `None` only when the embedding NSApplication had no main menu before
    /// Incular attached. Winit normally installs one with About/Services/Hide/
    /// Quit, but embedders remain supported without inventing one as baseline.
    original: Option<Retained<NSMenu>>,
    /// Root menu Incular appends its owned top-level entries to. When an
    /// original menu exists this is the exact same NSMenu, preserving all
    /// platform/application items and their native identity.
    root: Retained<NSMenu>,
    original_item_count: usize,
}

/// Application-global AppKit main-menu backend. Native menu actions never run
/// retained Rust callbacks from the Objective-C stack; the dispatcher queues
/// stable native command/menu identities and the desktop loop flushes them.
pub(crate) struct MacosPlatformMenuDelegate {
    state: RefCell<MacosMenuState>,
    native_messages: Rc<RefCell<VecDeque<NativeMenuMessage>>>,
}

impl Default for MacosPlatformMenuDelegate {
    fn default() -> Self {
        Self {
            state: RefCell::new(MacosMenuState::default()),
            native_messages: Rc::new(RefCell::new(VecDeque::new())),
        }
    }
}

impl MacosPlatformMenuDelegate {
    pub(crate) fn flush_events(&self) {
        let messages = self
            .native_messages
            .borrow_mut()
            .drain(..)
            .collect::<Vec<_>>();
        if messages.is_empty() {
            return;
        }
        let (handler, events) = {
            let state = self.state.borrow();
            let Some(handler) = state.event_handler.clone() else {
                return;
            };
            let events = messages
                .into_iter()
                .filter_map(|message| match message {
                    NativeMenuMessage::Command(raw) => state
                        .registry
                        .resolve_raw(raw)
                        .cloned()
                        .map(PlatformMenuEvent::Selected),
                    NativeMenuMessage::Opened(menu) => state
                        .menu_ids
                        .get(&menu)
                        .cloned()
                        .map(PlatformMenuEvent::Opened),
                    NativeMenuMessage::Closed(menu) => state
                        .menu_ids
                        .get(&menu)
                        .cloned()
                        .map(PlatformMenuEvent::Closed),
                })
                .collect::<Vec<_>>();
            (handler, events)
        };
        for event in events {
            handler(event);
        }
    }
}

impl PlatformMenuDelegate for MacosPlatformMenuDelegate {
    fn acquire(&self, owner: MenuOwnerId) -> PlatformMenuUpdate {
        let mut state = self.state.borrow_mut();
        match state.owner {
            None => {
                state.owner = Some(owner);
                PlatformMenuUpdate::Applied
            }
            Some(active) if active == owner => PlatformMenuUpdate::Applied,
            Some(_) => PlatformMenuUpdate::RejectedOwnedByOther,
        }
    }

    fn set_menus(&self, owner: MenuOwnerId, snapshot: PlatformMenuSnapshot) -> PlatformMenuUpdate {
        let Some(mtm) = MainThreadMarker::new() else {
            return PlatformMenuUpdate::RejectedByPlatform;
        };
        let mut state = self.state.borrow_mut();
        if state.owner != Some(owner) {
            return PlatformMenuUpdate::RejectedOwnedByOther;
        }
        if state.snapshot.as_ref() == Some(&snapshot) {
            return PlatformMenuUpdate::Unchanged;
        }

        let mut next_registry = state.registry.clone();
        if next_registry.synchronize(&snapshot).is_err() {
            return PlatformMenuUpdate::RejectedByPlatform;
        }
        let dispatcher = state
            .dispatcher
            .get_or_insert_with(|| PlatformMenuDispatcher::new(self.native_messages.clone(), mtm))
            .clone();
        let application = NSApplication::sharedApplication(mtm);
        if state.baseline.is_none() {
            state.baseline = Some(capture_baseline_main_menu(&application, mtm));
        }

        if state
            .snapshot
            .as_ref()
            .is_some_and(|current| native_menu_structure_equal(current, &snapshot))
        {
            let baseline = state
                .baseline
                .as_ref()
                .expect("AppKit baseline captured before update");
            if !update_menu_nodes(
                &baseline.root,
                baseline.original_item_count,
                &snapshot.menus,
                &next_registry,
            ) {
                if let Some(previous) = state.snapshot.as_ref() {
                    let _ = update_menu_nodes(
                        &baseline.root,
                        baseline.original_item_count,
                        &previous.menus,
                        &state.registry,
                    );
                }
                return PlatformMenuUpdate::RejectedByPlatform;
            }
        } else {
            let mut menu_ids = HashMap::new();
            let items = build_menu_items(
                &snapshot.menus,
                &next_registry,
                &dispatcher,
                mtm,
                &mut menu_ids,
            );
            let baseline = state
                .baseline
                .as_ref()
                .expect("AppKit baseline captured before replacement");
            remove_owned_top_level_items(&baseline.root, baseline.original_item_count);
            for item in items {
                baseline.root.addItem(&item);
            }
            application.setMainMenu(Some(&baseline.root));
            state.menu_ids = menu_ids;
        }

        state.registry = next_registry;
        state.snapshot = Some(snapshot);
        PlatformMenuUpdate::Applied
    }

    fn clear_menus(&self, owner: MenuOwnerId) -> PlatformMenuUpdate {
        let mut state = self.state.borrow_mut();
        if state.owner != Some(owner) {
            return PlatformMenuUpdate::RejectedOwnedByOther;
        }
        if let (Some(mtm), Some(baseline)) = (MainThreadMarker::new(), state.baseline.as_ref()) {
            remove_owned_top_level_items(&baseline.root, baseline.original_item_count);
            NSApplication::sharedApplication(mtm).setMainMenu(baseline.original.as_deref());
        }
        state.menu_ids.clear();
        let _ = state
            .registry
            .synchronize(&PlatformMenuSnapshot { menus: Vec::new() });
        state.snapshot = None;
        PlatformMenuUpdate::Applied
    }

    fn release(&self, owner: MenuOwnerId) -> PlatformMenuUpdate {
        let mut state = self.state.borrow_mut();
        if state.owner != Some(owner) {
            return PlatformMenuUpdate::RejectedOwnedByOther;
        }
        state.owner = None;
        state.event_handler = None;
        PlatformMenuUpdate::Applied
    }

    fn set_event_handler(
        &self,
        owner: MenuOwnerId,
        handler: Option<Rc<dyn Fn(PlatformMenuEvent)>>,
    ) -> PlatformMenuUpdate {
        let mut state = self.state.borrow_mut();
        if state.owner != Some(owner) {
            return PlatformMenuUpdate::RejectedOwnedByOther;
        }
        state.event_handler = handler;
        PlatformMenuUpdate::Applied
    }

    fn handle_shortcut(&self, key: &str, modifiers: ShortcutModifiers) -> bool {
        // AppKit's main menu is the shortcut authority. If Winit surfaces the
        // same key event after native handling, consume it without invoking the
        // retained callback a second time.
        self.state
            .borrow()
            .snapshot
            .as_ref()
            .is_some_and(|snapshot| matching_menu_shortcut(snapshot, key, modifiers).is_some())
    }
}

fn capture_baseline_main_menu(
    application: &NSApplication,
    mtm: MainThreadMarker,
) -> BaselineMainMenu {
    // SAFETY: queried synchronously on AppKit's main thread. Winit normally
    // initializes the standard application menu before native windows mount.
    let original = unsafe { application.mainMenu() };
    let root = original.clone().unwrap_or_else(|| {
        let title = NSString::from_str("");
        // SAFETY: creating a detached NSMenu on the main thread is valid.
        unsafe { NSMenu::initWithTitle(mtm.alloc(), &title) }
    });
    // SAFETY: `root` is a live retained NSMenu on the main thread.
    let original_item_count = usize::try_from(unsafe { root.numberOfItems() }).unwrap_or(0);
    BaselineMainMenu {
        original,
        root,
        original_item_count,
    }
}

fn remove_owned_top_level_items(root: &NSMenu, original_item_count: usize) {
    // Remove from the end so every original item keeps its exact index and
    // identity. Incular owns only entries appended after the captured baseline.
    loop {
        // SAFETY: main-thread-only menu query.
        let count = usize::try_from(unsafe { root.numberOfItems() }).unwrap_or(0);
        if count <= original_item_count {
            break;
        }
        // SAFETY: `count - 1` is an existing Incular-owned item index.
        unsafe { root.removeItemAtIndex(isize::try_from(count - 1).expect("NSMenu index fits")) };
    }
}

fn build_menu_items(
    nodes: &[PlatformMenuSnapshotNode],
    registry: &NativeMenuCommandRegistry,
    dispatcher: &PlatformMenuDispatcher,
    mtm: MainThreadMarker,
    menu_ids: &mut HashMap<usize, MenuItemId>,
) -> Vec<Retained<NSMenuItem>> {
    nodes
        .iter()
        .map(|node| build_menu_item(node, registry, dispatcher, mtm, menu_ids))
        .collect()
}

fn append_menu_nodes(
    parent: &NSMenu,
    nodes: &[PlatformMenuSnapshotNode],
    registry: &NativeMenuCommandRegistry,
    dispatcher: &PlatformMenuDispatcher,
    mtm: MainThreadMarker,
    menu_ids: &mut HashMap<usize, MenuItemId>,
) {
    for item in build_menu_items(nodes, registry, dispatcher, mtm, menu_ids) {
        parent.addItem(&item);
    }
}

fn build_menu_item(
    node: &PlatformMenuSnapshotNode,
    registry: &NativeMenuCommandRegistry,
    dispatcher: &PlatformMenuDispatcher,
    mtm: MainThreadMarker,
    menu_ids: &mut HashMap<usize, MenuItemId>,
) -> Retained<NSMenuItem> {
    if node.separator {
        return NSMenuItem::separatorItem(mtm);
    }

    let title = NSString::from_str(&node.label);
    let key = NSString::from_str(
        node.shortcut
            .as_ref()
            .filter(|_| node.selectable)
            .map_or("", |shortcut| shortcut.key.as_str()),
    );
    let action = node.selectable.then_some(sel!(incularMenuItemSelected:));
    // SAFETY: the selector belongs to PlatformMenuDispatcher and AppKit menu
    // construction occurs on the main thread.
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(mtm.alloc(), &title, action, &key)
    };
    unsafe {
        item.setEnabled(node.enabled);
        if let Some(tooltip) = &node.tooltip {
            item.setToolTip(Some(&NSString::from_str(tooltip)));
        }
    }

    if node.selectable {
        let command = registry
            .command_for(&node.id)
            .expect("selectable AppKit menu node has native command ID");
        unsafe {
            item.setTag(isize::try_from(command.get()).expect("AppKit NSInteger fits u32"));
            item.setTarget(Some(dispatcher.as_super().as_super()));
        }
        item.setKeyEquivalentModifierMask(appkit_modifiers(
            node.shortcut
                .as_ref()
                .map_or(ShortcutModifiers::empty(), |shortcut| shortcut.modifiers),
        ));
    } else {
        let submenu_title = NSString::from_str(&node.label);
        // SAFETY: construction and delegate assignment are main-thread-only.
        let submenu = unsafe { NSMenu::initWithTitle(mtm.alloc(), &submenu_title) };
        unsafe {
            submenu.setAutoenablesItems(false);
            submenu.setDelegate(Some(ProtocolObject::from_ref(dispatcher)));
        }
        append_menu_nodes(
            &submenu,
            &node.children,
            registry,
            dispatcher,
            mtm,
            menu_ids,
        );
        menu_ids.insert(&*submenu as *const NSMenu as usize, node.id.clone());
        item.setSubmenu(Some(&submenu));
    }
    item
}

fn update_menu_nodes(
    parent: &NSMenu,
    start_index: usize,
    nodes: &[PlatformMenuSnapshotNode],
    registry: &NativeMenuCommandRegistry,
) -> bool {
    for (index, node) in nodes.iter().enumerate() {
        let Some(index) = start_index.checked_add(index) else {
            return false;
        };
        let Ok(index) = isize::try_from(index) else {
            return false;
        };
        // SAFETY: native_menu_structure_equal guarantees an item at every
        // corresponding retained position.
        let Some(item) = (unsafe { parent.itemAtIndex(index) }) else {
            return false;
        };
        if node.separator {
            // SAFETY: structure equality guarantees the existing item is also a
            // separator, so it has no mutable command attributes to apply.
            if !unsafe { item.isSeparatorItem() } {
                return false;
            }
            continue;
        }
        unsafe {
            item.setTitle(&NSString::from_str(&node.label));
            item.setEnabled(node.enabled);
            item.setToolTip(
                node.tooltip
                    .as_ref()
                    .map(|value| NSString::from_str(value))
                    .as_deref(),
            );
        }
        if node.selectable {
            let Some(command) = registry.command_for(&node.id) else {
                return false;
            };
            let shortcut = node.shortcut.as_ref();
            unsafe {
                item.setTag(isize::try_from(command.get()).expect("AppKit NSInteger fits u32"));
                item.setKeyEquivalent(&NSString::from_str(
                    shortcut.map_or("", |value| value.key.as_str()),
                ));
            }
            item.setKeyEquivalentModifierMask(appkit_modifiers(
                shortcut.map_or(ShortcutModifiers::empty(), |value| value.modifiers),
            ));
        } else {
            // SAFETY: structure equality guarantees a retained submenu at this
            // position; reusing it preserves native menu/item identity.
            let Some(submenu) = (unsafe { item.submenu() }) else {
                return false;
            };
            if !update_menu_nodes(&submenu, 0, &node.children, registry) {
                return false;
            }
        }
    }
    true
}

fn appkit_modifiers(modifiers: ShortcutModifiers) -> NSEventModifierFlags {
    let mut native = NSEventModifierFlags::empty();
    if modifiers.contains(ShortcutModifiers::SHIFT) {
        native |= NSEventModifierFlags::NSEventModifierFlagShift;
    }
    if modifiers.contains(ShortcutModifiers::CONTROL) {
        native |= NSEventModifierFlags::NSEventModifierFlagControl;
    }
    if modifiers.contains(ShortcutModifiers::ALT) {
        native |= NSEventModifierFlags::NSEventModifierFlagOption;
    }
    if modifiers.contains(ShortcutModifiers::META) {
        native |= NSEventModifierFlags::NSEventModifierFlagCommand;
    }
    native
}
