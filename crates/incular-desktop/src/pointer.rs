use incular_core::PointerPhase;
use incular_platform::{NativePointerSample, mouse_button_mask};
use incular_widgets::MouseCursor;
use std::collections::HashMap;
use winit::{
    event::{DeviceId, ElementState, MouseButton},
    window::CursorIcon,
};

/// Stable process-local IDs for Winit pointer devices. Zero remains reserved
/// for adapters that do not expose native device identity.
#[derive(Default)]
pub(crate) struct PointerDeviceRegistry {
    ids: HashMap<DeviceId, u64>,
    native_ids: HashMap<u64, u64>,
    next: u64,
}

impl PointerDeviceRegistry {
    pub(crate) fn id(&mut self, device: DeviceId) -> u64 {
        if let Some(id) = self.ids.get(&device).copied() {
            return id;
        }
        let id = self
            .next
            .checked_add(1)
            .expect("pointer device id space exhausted");
        self.next = id;
        self.ids.insert(device, id);
        id
    }

    /// Resolves a backend-local opaque device token into the same public ID
    /// namespace as Winit devices. Keeping separate source maps prevents a
    /// native token such as `1` from aliasing Winit's first allocated device.
    fn native_id(&mut self, device: u64) -> u64 {
        if let Some(id) = self.native_ids.get(&device).copied() {
            return id;
        }
        let id = self
            .next
            .checked_add(1)
            .expect("pointer device id space exhausted");
        self.next = id;
        self.native_ids.insert(device, id);
        id
    }

    /// Resolves the fallback Winit device and, when present, the OS adapter's
    /// richer device token in one step. Native sample metadata remains intact;
    /// only its backend-local identity is rewritten to the portable process
    /// namespace.
    pub(crate) fn resolve_native_sample(
        &mut self,
        fallback: DeviceId,
        mut native: Option<NativePointerSample>,
    ) -> (u64, Option<NativePointerSample>) {
        let fallback = self.id(fallback);
        if let Some(sample) = native.as_mut()
            && let Some(device) = sample.device
        {
            sample.device = Some(self.native_id(device));
        }
        (fallback, native)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MouseButtonTransition {
    pub(crate) phase: PointerPhase,
    pub(crate) button: u32,
    pub(crate) buttons: u32,
}

#[derive(Default)]
pub(crate) struct MouseButtonState {
    pressed: u32,
}

impl MouseButtonState {
    #[must_use]
    pub(crate) const fn pressed(&self) -> u32 {
        self.pressed
    }

    /// Clears every pressed button after native focus/capture loss and returns
    /// whether a pointer sequence was active. Callers use this to synthesize
    /// one retained Cancel before clearing hover with Exit.
    pub(crate) fn cancel(&mut self) -> bool {
        let active = self.pressed != 0;
        self.pressed = 0;
        active
    }

    /// Applies one native button transition. Buttons outside Incular's portable
    /// mask capacity are ignored rather than aliased to an existing button.
    pub(crate) fn transition(
        &mut self,
        state: ElementState,
        button: MouseButton,
    ) -> Option<MouseButtonTransition> {
        let button = mouse_button_mask(button)?;
        let phase = match state {
            ElementState::Pressed => {
                self.pressed |= button;
                PointerPhase::Down
            }
            ElementState::Released => {
                self.pressed &= !button;
                PointerPhase::Up
            }
        };
        Some(MouseButtonTransition {
            phase,
            button,
            buttons: self.pressed,
        })
    }
}

#[derive(Default)]
pub(crate) struct NativeCursorCoordinator {
    applied: Option<MouseCursor>,
}

impl NativeCursorCoordinator {
    /// Returns a native cursor only when the effective portable cursor changed.
    pub(crate) fn update(&mut self, cursor: MouseCursor) -> Option<CursorIcon> {
        let cursor = if cursor == MouseCursor::Defer {
            MouseCursor::Basic
        } else {
            cursor
        };
        if self.applied == Some(cursor) {
            return None;
        }
        self.applied = Some(cursor);
        Some(native_cursor_icon(cursor))
    }
}

#[must_use]
pub(crate) const fn native_cursor_icon(cursor: MouseCursor) -> CursorIcon {
    match cursor {
        MouseCursor::Defer | MouseCursor::Basic => CursorIcon::Default,
        MouseCursor::Click => CursorIcon::Pointer,
        MouseCursor::Text => CursorIcon::Text,
        MouseCursor::Crosshair => CursorIcon::Crosshair,
        MouseCursor::Grab => CursorIcon::Grab,
        MouseCursor::Grabbing => CursorIcon::Grabbing,
        MouseCursor::NotAllowed => CursorIcon::NotAllowed,
        MouseCursor::ResizeHorizontal => CursorIcon::EwResize,
        MouseCursor::ResizeVertical => CursorIcon::NsResize,
        MouseCursor::ResizeUpLeftDownRight => CursorIcon::NwseResize,
        MouseCursor::ResizeUpRightDownLeft => CursorIcon::NeswResize,
    }
}
