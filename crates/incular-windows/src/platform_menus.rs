use incular_desktop::{
    NativeMenuCommandRegistry, format_menu_shortcut, matching_menu_shortcut,
    native_menu_structure_equal,
};
use incular_platform::{
    PlatformOperationError, PlatformOperationErrorKind, PlatformOperationResult,
};
use incular_widgets::{
    MenuItemId, MenuOwnerId, PlatformMenuDelegate, PlatformMenuEvent, PlatformMenuSnapshot,
    PlatformMenuSnapshotNode, PlatformMenuUpdate, ShortcutModifiers,
};
use raw_window_handle::RawWindowHandle;
use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    rc::Rc,
    sync::OnceLock,
};
use windows_sys::Win32::{
    Foundation::{GetLastError, HWND, LPARAM, LRESULT, SetLastError, WPARAM},
    UI::WindowsAndMessaging::{
        AppendMenuW, CallWindowProcW, CreateMenu, CreatePopupMenu, DefWindowProcW, DestroyMenu,
        DrawMenuBar, GWLP_WNDPROC, GetPropW, GetSubMenu, GetWindowLongPtrW, HMENU, MF_BYPOSITION,
        MF_GRAYED, MF_POPUP, MF_SEPARATOR, MF_STRING, ModifyMenuW, RemovePropW, SetMenu, SetPropW,
        SetWindowLongPtrW, WM_COMMAND, WM_INITMENUPOPUP, WM_UNINITMENUPOPUP, WNDPROC,
    },
};

const MAX_WIN32_COMMAND_ID: u32 = 0xEFFF;

#[derive(Clone, Debug)]
enum NativeMenuMessage {
    Command(u32),
    Opened(HMENU),
    Closed(HMENU),
}

struct WindowSubclassData {
    previous: WNDPROC,
    queue: Rc<RefCell<VecDeque<NativeMenuMessage>>>,
}

struct WindowMenu {
    root: HMENU,
    popup_handles: Vec<HMENU>,
    subclass_data: *mut WindowSubclassData,
}

impl WindowMenu {
    fn empty(subclass_data: *mut WindowSubclassData) -> Self {
        Self {
            root: 0,
            popup_handles: Vec::new(),
            subclass_data,
        }
    }
}

struct PreparedMenu {
    root: HMENU,
    popup_ids: Vec<(HMENU, MenuItemId)>,
}

impl PreparedMenu {
    fn empty() -> Self {
        Self {
            root: 0,
            popup_ids: Vec::new(),
        }
    }

    fn take_root(&mut self) -> HMENU {
        let root = self.root;
        self.root = 0;
        root
    }
}

impl Drop for PreparedMenu {
    fn drop(&mut self) {
        if self.root != 0 {
            // SAFETY: `root` is an unattached menu created by this backend and
            // still owned by this RAII guard.
            unsafe {
                DestroyMenu(self.root);
            }
        }
    }
}

struct WindowsMenuState {
    owner: Option<MenuOwnerId>,
    snapshot: Option<PlatformMenuSnapshot>,
    event_handler: Option<Rc<dyn Fn(PlatformMenuEvent)>>,
    registry: NativeMenuCommandRegistry,
    windows: HashMap<HWND, WindowMenu>,
    popup_ids: HashMap<HMENU, MenuItemId>,
}

impl Default for WindowsMenuState {
    fn default() -> Self {
        Self {
            owner: None,
            snapshot: None,
            event_handler: None,
            registry: NativeMenuCommandRegistry::with_max_command_id(MAX_WIN32_COMMAND_ID),
            windows: HashMap::new(),
            popup_ids: HashMap::new(),
        }
    }
}

/// Application-scoped Win32 menu backend shared by every top-level Incular
/// window in one desktop runner.
pub(crate) struct WindowsPlatformMenuDelegate {
    state: RefCell<WindowsMenuState>,
    native_messages: Rc<RefCell<VecDeque<NativeMenuMessage>>>,
}

impl Default for WindowsPlatformMenuDelegate {
    fn default() -> Self {
        Self {
            state: RefCell::new(WindowsMenuState::default()),
            native_messages: Rc::new(RefCell::new(VecDeque::new())),
        }
    }
}

impl WindowsPlatformMenuDelegate {
    pub(crate) fn register_window(
        &self,
        window: &winit::window::Window,
    ) -> PlatformOperationResult {
        let hwnd = win32_hwnd(window)?;
        if self.state.borrow().windows.contains_key(&hwnd) {
            return Ok(());
        }

        let property = subclass_property_name();
        // SAFETY: `hwnd` belongs to the live Winit window on this UI thread.
        // We replace only the WNDPROC slot and retain the previous procedure for
        // exact chain restoration on unregister.
        let previous_raw = unsafe { GetWindowLongPtrW(hwnd, GWLP_WNDPROC) };
        if previous_raw == 0 {
            return Err(native_failure("Win32 window has no WNDPROC"));
        }
        let previous = unsafe { raw_to_wndproc(previous_raw) };
        let subclass_data = Box::into_raw(Box::new(WindowSubclassData {
            previous,
            queue: self.native_messages.clone(),
        }));
        // SAFETY: the property stores a stable Box pointer until unregister.
        if unsafe { SetPropW(hwnd, property.as_ptr(), subclass_data as isize) } == 0 {
            // SAFETY: ownership was not transferred to the HWND property.
            unsafe {
                drop(Box::from_raw(subclass_data));
            }
            return Err(native_failure("Win32 could not attach menu subclass state"));
        }
        // SAFETY: clear the thread's last-error value so a zero return can be
        // distinguished from a valid previous value as required by Win32.
        unsafe { SetLastError(0) };
        // SAFETY: `menu_wnd_proc` has the exact Win32 WNDPROC ABI and the old
        // procedure remains stored in `subclass_data` for CallWindowProcW.
        let replaced = unsafe {
            SetWindowLongPtrW(
                hwnd,
                GWLP_WNDPROC,
                menu_wnd_proc as *const () as usize as isize,
            )
        };
        if replaced == 0 && unsafe { GetLastError() } != 0 {
            // SAFETY: the new WNDPROC was not installed, so the Box/property can
            // be removed synchronously.
            unsafe {
                RemovePropW(hwnd, property.as_ptr());
                drop(Box::from_raw(subclass_data));
            }
            return Err(native_failure(
                "Win32 could not install menu WNDPROC subclass",
            ));
        }

        let mut state = self.state.borrow_mut();
        state.windows.insert(hwnd, WindowMenu::empty(subclass_data));
        if let Some(snapshot) = state.snapshot.clone()
            && let Err(error) = install_snapshot_for_window(&mut state, hwnd, &snapshot)
        {
            drop(state);
            self.unregister_window(window);
            return Err(error);
        }
        Ok(())
    }

    pub(crate) fn unregister_window(&self, window: &winit::window::Window) {
        let Ok(hwnd) = win32_hwnd(window) else {
            return;
        };
        let Some(binding) = self.state.borrow_mut().windows.remove(&hwnd) else {
            return;
        };
        {
            let mut state = self.state.borrow_mut();
            for popup in &binding.popup_handles {
                state.popup_ids.remove(popup);
            }
        }
        // SAFETY: the HWND is still live; restoring the original WNDPROC before
        // removing the property guarantees no callback can observe freed state.
        unsafe {
            SetMenu(hwnd, 0);
            DrawMenuBar(hwnd);
            let data = &*binding.subclass_data;
            SetWindowLongPtrW(hwnd, GWLP_WNDPROC, wndproc_to_raw(data.previous));
            RemovePropW(hwnd, subclass_property_name().as_ptr());
            drop(Box::from_raw(binding.subclass_data));
            if binding.root != 0 {
                DestroyMenu(binding.root);
            }
        }
    }

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
                        .popup_ids
                        .get(&menu)
                        .cloned()
                        .map(PlatformMenuEvent::Opened),
                    NativeMenuMessage::Closed(menu) => state
                        .popup_ids
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

impl PlatformMenuDelegate for WindowsPlatformMenuDelegate {
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

        let update_result = if state
            .snapshot
            .as_ref()
            .is_some_and(|current| native_menu_structure_equal(current, &snapshot))
        {
            update_snapshot_in_place(&mut state, &snapshot, &next_registry)
        } else {
            install_structural_snapshot_with_registry(&mut state, &snapshot, &next_registry)
        };
        if update_result.is_err() {
            return PlatformMenuUpdate::RejectedByPlatform;
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
        for (hwnd, window) in &mut state.windows {
            // SAFETY: every HWND/menu pair is owned by this delegate and still
            // registered with the live Winit window.
            unsafe {
                SetMenu(*hwnd, 0);
                DrawMenuBar(*hwnd);
                if window.root != 0 {
                    DestroyMenu(window.root);
                }
            }
            window.root = 0;
            window.popup_handles.clear();
        }
        state.popup_ids.clear();
        let empty = PlatformMenuSnapshot { menus: Vec::new() };
        let _ = state.registry.synchronize(&empty);
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
        let command = {
            let state = self.state.borrow();
            let Some(snapshot) = state.snapshot.as_ref() else {
                return false;
            };
            let Some(id) = matching_menu_shortcut(snapshot, key, modifiers) else {
                return false;
            };
            let Some(command) = state.registry.command_for(&id) else {
                return false;
            };
            command.get()
        };
        self.native_messages
            .borrow_mut()
            .push_back(NativeMenuMessage::Command(command));
        true
    }
}

impl Drop for WindowsPlatformMenuDelegate {
    fn drop(&mut self) {
        // Normal desktop teardown unregisters live Winit windows before this
        // delegate drops. Avoid touching HWNDs here because Winit may already
        // have destroyed them during panic/unwind teardown.
        debug_assert!(
            self.state.get_mut().windows.is_empty(),
            "Windows platform-menu windows must be unregistered before delegate drop"
        );
    }
}

fn install_snapshot_for_window(
    state: &mut WindowsMenuState,
    hwnd: HWND,
    snapshot: &PlatformMenuSnapshot,
) -> PlatformOperationResult {
    let mut prepared = build_native_menu(snapshot, &state.registry)?;
    // SAFETY: the HWND is registered and this menu was built specifically for
    // that still-live Winit window.
    if unsafe { SetMenu(hwnd, prepared.root) } == 0 {
        return Err(native_failure("Win32 SetMenu rejected application menu"));
    }
    unsafe {
        DrawMenuBar(hwnd);
    }
    let new_root = prepared.take_root();
    let popup_handles = prepared
        .popup_ids
        .iter()
        .map(|(handle, _)| *handle)
        .collect::<Vec<_>>();
    for (handle, id) in prepared.popup_ids.drain(..) {
        state.popup_ids.insert(handle, id);
    }
    let window = state
        .windows
        .get_mut(&hwnd)
        .expect("registered Win32 window");
    debug_assert_eq!(window.root, 0, "newly registered window must have no menu");
    window.root = new_root;
    window.popup_handles = popup_handles;
    Ok(())
}

fn install_structural_snapshot_with_registry(
    state: &mut WindowsMenuState,
    snapshot: &PlatformMenuSnapshot,
    registry: &NativeMenuCommandRegistry,
) -> PlatformOperationResult {
    let hwnds = state.windows.keys().copied().collect::<Vec<_>>();
    let mut prepared = Vec::with_capacity(hwnds.len());
    for hwnd in &hwnds {
        prepared.push((*hwnd, build_native_menu(snapshot, registry)?));
    }

    let old_roots = hwnds
        .iter()
        .map(|hwnd| (*hwnd, state.windows[hwnd].root))
        .collect::<Vec<_>>();
    let mut attached = Vec::new();
    for (hwnd, menu) in &prepared {
        // SAFETY: the HWND is registered and `menu.root` is either a fresh
        // unattached HMENU or zero for an empty application menu.
        if unsafe { SetMenu(*hwnd, menu.root) } == 0 {
            for (restore_hwnd, old_root) in old_roots.iter().copied() {
                if attached.contains(&restore_hwnd) {
                    // SAFETY: restore each previously attached old root before
                    // any prepared root guard destroys its menu.
                    unsafe {
                        SetMenu(restore_hwnd, old_root);
                        DrawMenuBar(restore_hwnd);
                    }
                }
            }
            return Err(native_failure("Win32 SetMenu rejected application menu"));
        }
        // SAFETY: redraw only updates this live window's non-client menu bar.
        unsafe {
            DrawMenuBar(*hwnd);
        }
        attached.push(*hwnd);
    }

    state.popup_ids.clear();
    for (hwnd, mut menu) in prepared {
        let new_root = menu.take_root();
        let popup_handles = menu
            .popup_ids
            .iter()
            .map(|(handle, _)| *handle)
            .collect::<Vec<_>>();
        for (handle, id) in menu.popup_ids.drain(..) {
            state.popup_ids.insert(handle, id);
        }
        let window = state
            .windows
            .get_mut(&hwnd)
            .expect("registered Win32 window");
        let old_root = std::mem::replace(&mut window.root, new_root);
        window.popup_handles = popup_handles;
        if old_root != 0 {
            // SAFETY: SetMenu detached the old root above; the backend retained
            // sole ownership and may now destroy the entire old submenu tree.
            unsafe {
                DestroyMenu(old_root);
            }
        }
    }
    Ok(())
}

fn update_snapshot_in_place(
    state: &mut WindowsMenuState,
    snapshot: &PlatformMenuSnapshot,
    registry: &NativeMenuCommandRegistry,
) -> PlatformOperationResult {
    let previous = state.snapshot.clone();
    for window in state.windows.values() {
        if window.root == 0 && snapshot.menus.is_empty() {
            continue;
        }
        if !unsafe { update_menu_nodes(window.root, &snapshot.menus, registry) } {
            if let Some(previous) = previous.as_ref() {
                for rollback in state.windows.values() {
                    // SAFETY: structure equality means all retained HMENU/
                    // submenu handles still match the old snapshot positions.
                    let _ = unsafe {
                        update_menu_nodes(rollback.root, &previous.menus, &state.registry)
                    };
                }
            }
            return Err(native_failure(
                "Win32 could not update application menu items",
            ));
        }
    }
    for hwnd in state.windows.keys() {
        // SAFETY: HWND is registered and menu contents were updated in place.
        unsafe {
            DrawMenuBar(*hwnd);
        }
    }
    Ok(())
}

fn build_native_menu(
    snapshot: &PlatformMenuSnapshot,
    registry: &NativeMenuCommandRegistry,
) -> Result<PreparedMenu, PlatformOperationError> {
    if snapshot.menus.is_empty() {
        return Ok(PreparedMenu::empty());
    }
    // SAFETY: CreateMenu returns a backend-owned detached HMENU.
    let root = unsafe { CreateMenu() };
    if root == 0 {
        return Err(native_failure("Win32 CreateMenu failed"));
    }
    let mut menu = PreparedMenu {
        root,
        popup_ids: Vec::new(),
    };
    if !unsafe { append_menu_nodes(root, &snapshot.menus, registry, &mut menu.popup_ids) } {
        return Err(native_failure("Win32 could not construct application menu"));
    }
    Ok(menu)
}

unsafe fn append_menu_nodes(
    parent: HMENU,
    nodes: &[PlatformMenuSnapshotNode],
    registry: &NativeMenuCommandRegistry,
    popup_ids: &mut Vec<(HMENU, MenuItemId)>,
) -> bool {
    for node in nodes {
        if node.separator {
            // SAFETY: parent is a live detached menu built by this backend.
            if unsafe { AppendMenuW(parent, MF_SEPARATOR, 0, std::ptr::null()) } == 0 {
                return false;
            }
            continue;
        }
        let label = native_label(node);
        let wide = wide(&label);
        let disabled = if node.enabled { 0 } else { MF_GRAYED };
        if node.selectable {
            let Some(command) = registry.command_for(&node.id) else {
                return false;
            };
            // SAFETY: command IDs are constrained to Win32's application range
            // and the string buffer remains live for this synchronous call.
            if unsafe {
                AppendMenuW(
                    parent,
                    MF_STRING | disabled,
                    command.get() as usize,
                    wide.as_ptr(),
                )
            } == 0
            {
                return false;
            }
        } else {
            // SAFETY: CreatePopupMenu returns a detached submenu owned by parent
            // after successful AppendMenuW(MF_POPUP).
            let submenu = unsafe { CreatePopupMenu() };
            if submenu == 0 {
                return false;
            }
            if !unsafe { append_menu_nodes(submenu, &node.children, registry, popup_ids) } {
                // SAFETY: submenu has not yet been attached to parent.
                unsafe {
                    DestroyMenu(submenu);
                }
                return false;
            }
            if unsafe {
                AppendMenuW(
                    parent,
                    MF_POPUP | MF_STRING | disabled,
                    submenu as usize,
                    wide.as_ptr(),
                )
            } == 0
            {
                // SAFETY: failed AppendMenuW leaves submenu unattached.
                unsafe {
                    DestroyMenu(submenu);
                }
                return false;
            }
            popup_ids.push((submenu, node.id.clone()));
        }
    }
    true
}

unsafe fn update_menu_nodes(
    parent: HMENU,
    nodes: &[PlatformMenuSnapshotNode],
    registry: &NativeMenuCommandRegistry,
) -> bool {
    for (position, node) in nodes.iter().enumerate() {
        let Ok(position) = u32::try_from(position) else {
            return false;
        };
        if node.separator {
            // SAFETY: structure equality guarantees this position remains a
            // separator in the existing menu.
            if unsafe {
                ModifyMenuW(
                    parent,
                    position,
                    MF_BYPOSITION | MF_SEPARATOR,
                    0,
                    std::ptr::null(),
                )
            } == 0
            {
                return false;
            }
            continue;
        }
        let wide = wide(&native_label(node));
        let disabled = if node.enabled { 0 } else { MF_GRAYED };
        if node.selectable {
            let Some(command) = registry.command_for(&node.id) else {
                return false;
            };
            if unsafe {
                ModifyMenuW(
                    parent,
                    position,
                    MF_BYPOSITION | MF_STRING | disabled,
                    command.get() as usize,
                    wide.as_ptr(),
                )
            } == 0
            {
                return false;
            }
        } else {
            // SAFETY: structure equality guarantees a submenu at this exact
            // position. Reusing the HMENU preserves native submenu identity.
            let submenu = unsafe { GetSubMenu(parent, position as i32) };
            if submenu == 0 {
                return false;
            }
            if unsafe {
                ModifyMenuW(
                    parent,
                    position,
                    MF_BYPOSITION | MF_POPUP | MF_STRING | disabled,
                    submenu as usize,
                    wide.as_ptr(),
                )
            } == 0
                || !unsafe { update_menu_nodes(submenu, &node.children, registry) }
            {
                return false;
            }
        }
    }
    true
}

fn native_label(node: &PlatformMenuSnapshotNode) -> String {
    // Win32 interprets '&' as an access-key marker. Incular's portable model
    // has no mnemonic semantic yet, so preserve literal application labels.
    let mut label = node.label.replace('&', "&&");
    if node.selectable
        && let Some(shortcut) = &node.shortcut
    {
        label.push('\t');
        label.push_str(&format_menu_shortcut(shortcut));
    }
    label
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn subclass_property_name() -> &'static [u16] {
    static NAME: OnceLock<Vec<u16>> = OnceLock::new();
    NAME.get_or_init(|| wide("Incular.PlatformMenu.Subclass"))
}

fn win32_hwnd(window: &winit::window::Window) -> Result<HWND, PlatformOperationError> {
    let RawWindowHandle::Win32(handle) =
        incular_desktop::winit_adapter::raw_window_handles(window).window
    else {
        return Err(PlatformOperationError::unavailable());
    };
    Ok(handle.hwnd.get())
}

fn native_failure(context: &'static str) -> PlatformOperationError {
    PlatformOperationError::with_context(PlatformOperationErrorKind::NativeFailure, context)
}

unsafe fn raw_to_wndproc(raw: isize) -> WNDPROC {
    // SAFETY: GWLP_WNDPROC is specified by Win32 to contain a WNDPROC pointer.
    unsafe { std::mem::transmute::<isize, WNDPROC>(raw) }
}

fn wndproc_to_raw(proc: WNDPROC) -> isize {
    proc.map_or(0, |callback| callback as usize as isize)
}

unsafe extern "system" fn menu_wnd_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: while our subclass is installed, the property points to a live
    // WindowSubclassData Box. Teardown restores the old WNDPROC before freeing
    // and removing this property.
    let data_ptr = unsafe { GetPropW(hwnd, subclass_property_name().as_ptr()) };
    if data_ptr == 0 {
        // SAFETY: no subclass state remains, so fall back to default handling.
        return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
    }
    // SAFETY: property lifetime is tied to the installed subclass as above.
    let data = unsafe { &*(data_ptr as *const WindowSubclassData) };
    match message {
        WM_COMMAND if lparam == 0 && ((wparam >> 16) & 0xFFFF) == 0 => {
            let command = (wparam & 0xFFFF) as u32;
            data.queue
                .borrow_mut()
                .push_back(NativeMenuMessage::Command(command));
        }
        WM_INITMENUPOPUP => data
            .queue
            .borrow_mut()
            .push_back(NativeMenuMessage::Opened(wparam as HMENU)),
        WM_UNINITMENUPOPUP => data
            .queue
            .borrow_mut()
            .push_back(NativeMenuMessage::Closed(wparam as HMENU)),
        _ => {}
    }
    // SAFETY: `previous` is the exact WNDPROC captured from this HWND before
    // installing our subclass, preserving Winit's native message semantics.
    unsafe { CallWindowProcW(data.previous, hwnd, message, wparam, lparam) }
}
