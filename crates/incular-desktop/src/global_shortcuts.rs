//! Native application-level global shortcut backend.

use global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
    hotkey::{Code as NativeCode, HotKey, Modifiers as NativeModifiers},
};
use incular_platform::{
    CapabilitySupport, GlobalShortcutChord, GlobalShortcutError, GlobalShortcutId,
    NativeWindowSystem,
};
use std::{
    collections::HashMap,
    str::FromStr,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};
use winit::{event::KeyEvent, keyboard::PhysicalKey};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct NativeGlobalShortcutEvent {
    pub(crate) native_id: u32,
}

type EventSink = Arc<dyn Fn(NativeGlobalShortcutEvent) + Send + Sync>;
type InstalledSink = (u64, EventSink);

static EVENT_HANDLER_INSTALLED: OnceLock<()> = OnceLock::new();
static EVENT_SINK: OnceLock<Mutex<Option<InstalledSink>>> = OnceLock::new();
static NEXT_SINK_GENERATION: AtomicU64 = AtomicU64::new(1);

pub(crate) struct GlobalShortcutEventSinkGuard {
    generation: u64,
}

impl Drop for GlobalShortcutEventSinkGuard {
    fn drop(&mut self) {
        let Some(sink) = EVENT_SINK.get() else {
            return;
        };
        let mut sink = sink.lock().expect("global shortcut event sink lock");
        if sink
            .as_ref()
            .is_some_and(|(generation, _)| *generation == self.generation)
        {
            sink.take();
        }
    }
}

pub(crate) fn install_event_sink(sink: EventSink) -> GlobalShortcutEventSinkGuard {
    EVENT_HANDLER_INSTALLED.get_or_init(|| {
        GlobalHotKeyEvent::set_event_handler(Some(|event: GlobalHotKeyEvent| {
            if event.state != HotKeyState::Pressed {
                return;
            }
            let sink = EVENT_SINK
                .get_or_init(|| Mutex::new(None))
                .lock()
                .expect("global shortcut event sink lock")
                .as_ref()
                .map(|(_, sink)| sink.clone());
            if let Some(sink) = sink {
                sink(NativeGlobalShortcutEvent {
                    native_id: event.id,
                });
            }
        }));
    });
    let generation = NEXT_SINK_GENERATION.fetch_add(1, Ordering::Relaxed);
    *EVENT_SINK
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("global shortcut event sink lock") = Some((generation, sink));
    GlobalShortcutEventSinkGuard { generation }
}

struct NativeRegistration {
    hotkey: HotKey,
}

pub(crate) struct DesktopGlobalShortcuts {
    manager: Option<GlobalHotKeyManager>,
    initialization_error: Option<String>,
    support: CapabilitySupport,
    registrations: HashMap<GlobalShortcutId, NativeRegistration>,
    native_ids: HashMap<u32, GlobalShortcutId>,
}

impl DesktopGlobalShortcuts {
    pub(crate) fn new(system: NativeWindowSystem) -> Self {
        let support = support_for(system);
        if support != CapabilitySupport::Supported {
            return Self {
                manager: None,
                initialization_error: None,
                support,
                registrations: HashMap::new(),
                native_ids: HashMap::new(),
            };
        }
        match GlobalHotKeyManager::new() {
            Ok(manager) => Self {
                manager: Some(manager),
                initialization_error: None,
                support,
                registrations: HashMap::new(),
                native_ids: HashMap::new(),
            },
            Err(error) => Self {
                manager: None,
                initialization_error: Some(error.to_string()),
                support: CapabilitySupport::Unknown,
                registrations: HashMap::new(),
                native_ids: HashMap::new(),
            },
        }
    }

    #[must_use]
    pub(crate) const fn support(&self) -> CapabilitySupport {
        self.support
    }

    pub(crate) fn register(
        &mut self,
        id: GlobalShortcutId,
        chord: GlobalShortcutChord,
    ) -> Result<(), GlobalShortcutError> {
        if self.support == CapabilitySupport::Unsupported {
            return Err(GlobalShortcutError::Unsupported);
        }
        let Some(manager) = self.manager.as_ref() else {
            return Err(GlobalShortcutError::Backend(
                self.initialization_error
                    .clone()
                    .unwrap_or_else(|| "native global shortcut manager is unavailable".to_owned()),
            ));
        };
        if self.registrations.contains_key(&id) {
            return Err(GlobalShortcutError::AlreadyRegistered(id));
        }
        let hotkey = native_hotkey(chord)?;
        if self.native_ids.contains_key(&hotkey.id()) {
            return Err(GlobalShortcutError::Conflict(chord));
        }
        manager
            .register(hotkey)
            .map_err(|error| map_register_error(error, chord))?;
        self.native_ids.insert(hotkey.id(), id);
        self.registrations.insert(id, NativeRegistration { hotkey });
        Ok(())
    }

    pub(crate) fn unregister(&mut self, id: GlobalShortcutId) -> Result<(), GlobalShortcutError> {
        let Some(registration) = self.registrations.get(&id) else {
            return Err(GlobalShortcutError::NotRegistered(id));
        };
        let Some(manager) = self.manager.as_ref() else {
            return Err(GlobalShortcutError::Backend(
                self.initialization_error
                    .clone()
                    .unwrap_or_else(|| "native global shortcut manager is unavailable".to_owned()),
            ));
        };
        manager
            .unregister(registration.hotkey)
            .map_err(|error| GlobalShortcutError::Backend(error.to_string()))?;
        let native_id = registration.hotkey.id();
        self.registrations.remove(&id);
        self.native_ids.remove(&native_id);
        Ok(())
    }

    #[must_use]
    pub(crate) fn stable_id(&self, native_id: u32) -> Option<GlobalShortcutId> {
        self.native_ids.get(&native_id).copied()
    }

    /// Returns true when a Winit key event is the focused-window copy of a
    /// globally registered chord. The desktop runner suppresses that local
    /// delivery so one physical press cannot also trigger focused `Shortcuts`.
    #[must_use]
    pub(crate) fn suppress_local_key_event(
        &self,
        event: &KeyEvent,
        modifiers: winit::keyboard::ModifiersState,
    ) -> bool {
        let PhysicalKey::Code(code) = event.physical_key else {
            return false;
        };
        let Ok(native_code) = NativeCode::from_str(&format!("{code:?}")) else {
            return false;
        };
        let native_modifiers = native_modifiers_from_winit(modifiers);
        self.registrations.values().any(|registration| {
            registration.hotkey.key == native_code && registration.hotkey.mods == native_modifiers
        })
    }
}

pub(crate) const fn support_for(system: NativeWindowSystem) -> CapabilitySupport {
    match system {
        NativeWindowSystem::Win32 | NativeWindowSystem::AppKit | NativeWindowSystem::X11 => {
            CapabilitySupport::Supported
        }
        NativeWindowSystem::Wayland | NativeWindowSystem::Other => CapabilitySupport::Unsupported,
    }
}

fn native_hotkey(chord: GlobalShortcutChord) -> Result<HotKey, GlobalShortcutError> {
    let key = NativeCode::from_str(&chord.key().to_string())
        .map_err(|error| GlobalShortcutError::Backend(error.to_string()))?;
    let modifiers = native_modifiers(chord.modifiers());
    Ok(HotKey::new(
        (!modifiers.is_empty()).then_some(modifiers),
        key,
    ))
}

fn native_modifiers(modifiers: incular_core::Modifiers) -> NativeModifiers {
    let mut native = NativeModifiers::empty();
    if modifiers.contains(incular_core::Modifiers::ALT) {
        native |= NativeModifiers::ALT;
    }
    if modifiers.contains(incular_core::Modifiers::CONTROL) {
        native |= NativeModifiers::CONTROL;
    }
    if modifiers.contains(incular_core::Modifiers::META) {
        native |= NativeModifiers::SUPER;
    }
    if modifiers.contains(incular_core::Modifiers::SHIFT) {
        native |= NativeModifiers::SHIFT;
    }
    native
}

fn native_modifiers_from_winit(modifiers: winit::keyboard::ModifiersState) -> NativeModifiers {
    let mut native = NativeModifiers::empty();
    if modifiers.alt_key() {
        native |= NativeModifiers::ALT;
    }
    if modifiers.control_key() {
        native |= NativeModifiers::CONTROL;
    }
    if modifiers.super_key() {
        native |= NativeModifiers::SUPER;
    }
    if modifiers.shift_key() {
        native |= NativeModifiers::SHIFT;
    }
    native
}

fn map_register_error(
    error: global_hotkey::Error,
    chord: GlobalShortcutChord,
) -> GlobalShortcutError {
    match error {
        global_hotkey::Error::AlreadyRegistered(_) => GlobalShortcutError::Conflict(chord),
        error => GlobalShortcutError::Backend(error.to_string()),
    }
}
