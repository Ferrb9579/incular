//! Platform-owned window data and normalized native events.
//!
//! Layout stays in logical pixels. This crate is the single conversion boundary
//! between those coordinates and physical surface/window coordinates.

use incular_core::{
    ImeEvent, InputEvent, KeyCode, KeyEvent, Modifiers, Offset, PointerPhase, Size,
};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhysicalSize {
    pub width: u32,
    pub height: u32,
}
impl PhysicalSize {
    #[must_use]
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.width == 0 || self.height == 0
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowMetrics {
    pub physical_size: PhysicalSize,
    pub scale_factor: f64,
}
impl WindowMetrics {
    #[must_use]
    pub fn new(physical_size: PhysicalSize, scale_factor: f64) -> Self {
        assert!(
            scale_factor.is_finite() && scale_factor > 0.0,
            "scale factor must be positive"
        );
        Self {
            physical_size,
            scale_factor,
        }
    }
    #[must_use]
    pub fn logical_size(self) -> Size {
        Size::new(
            self.physical_size.width as f32 / self.scale_factor as f32,
            self.physical_size.height as f32 / self.scale_factor as f32,
        )
    }
    #[must_use]
    pub fn logical_to_physical(self, offset: Offset) -> Offset {
        Offset::new(
            offset.x * self.scale_factor as f32,
            offset.y * self.scale_factor as f32,
        )
    }
    #[must_use]
    pub fn physical_to_logical(self, offset: Offset) -> Offset {
        Offset::new(
            offset.x / self.scale_factor as f32,
            offset.y / self.scale_factor as f32,
        )
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum PlatformEvent {
    Input(InputEvent),
    CloseRequested,
}
/// Platform clipboard boundary. Backends replace this in-memory implementation
/// with their native clipboard bridge without leaking native types into widgets.
pub trait Clipboard {
    fn get_text(&mut self) -> Option<String>;
    fn set_text(&mut self, text: String);
}
#[derive(Default)]
pub struct MemoryClipboard {
    text: String,
}
impl Clipboard for MemoryClipboard {
    fn get_text(&mut self) -> Option<String> {
        (!self.text.is_empty()).then(|| self.text.clone())
    }
    fn set_text(&mut self, text: String) {
        self.text = text;
    }
}
/// Raw handles are passed only to the GPU backend. The Linux window owner must
/// outlive the created surface; this is documented at the unsafe GPU boundary.
#[derive(Clone, Copy, Debug)]
pub struct RawWindowHandles {
    pub window: RawWindowHandle,
    pub display: Option<RawDisplayHandle>,
}
#[must_use]
pub fn raw_window_handles(window: &winit::window::Window) -> RawWindowHandles {
    RawWindowHandles {
        window: window
            .window_handle()
            .expect("window exposes a raw handle")
            .as_raw(),
        display: window.display_handle().ok().map(|handle| handle.as_raw()),
    }
}
#[must_use]
pub fn normalize_cursor(
    position: winit::dpi::PhysicalPosition<f64>,
    metrics: WindowMetrics,
) -> Offset {
    metrics.physical_to_logical(Offset::new(position.x as f32, position.y as f32))
}
#[must_use]
pub fn pointer_event(
    phase: PointerPhase,
    position: winit::dpi::PhysicalPosition<f64>,
    metrics: WindowMetrics,
) -> PlatformEvent {
    PlatformEvent::Input(InputEvent::Pointer {
        phase,
        position: normalize_cursor(position, metrics),
    })
}
/// Converts an identified platform contact into logical coordinates. Native
/// touch/pen adapters should call this rather than collapsing contacts into
/// the mouse-compatible pointer `0` path above.
#[must_use]
pub fn identified_pointer_event(
    pointer: u64,
    phase: PointerPhase,
    position: winit::dpi::PhysicalPosition<f64>,
    metrics: WindowMetrics,
) -> PlatformEvent {
    PlatformEvent::Input(InputEvent::PointerWithId {
        pointer,
        phase,
        position: normalize_cursor(position, metrics),
    })
}
/// Converts a Winit touch contact to the identified pointer pipeline. This
/// keeps native contact IDs intact for retained multi-pointer gestures.
#[must_use]
pub fn touch_event(touch: winit::event::Touch, metrics: WindowMetrics) -> PlatformEvent {
    let phase = match touch.phase {
        winit::event::TouchPhase::Started => PointerPhase::Down,
        winit::event::TouchPhase::Moved => PointerPhase::Move,
        winit::event::TouchPhase::Ended => PointerPhase::Up,
        winit::event::TouchPhase::Cancelled => PointerPhase::Cancel,
    };
    identified_pointer_event(touch.id, phase, touch.location, metrics)
}
/// Incular's scroll convention is positive Y increasing a vertical
/// [`incular_widgets::ScrollController`] offset (content moves upward). Winit
/// already reports both wheel forms in the platform's intended direction, so
/// neither path is inverted here. A line is a documented 40 logical-pixel
/// convenience step; pixel deltas deliberately retain their fractional value.
#[must_use]
pub fn wheel_event(delta: winit::event::MouseScrollDelta, metrics: WindowMetrics) -> PlatformEvent {
    let delta = match delta {
        winit::event::MouseScrollDelta::LineDelta(x, y) => Offset::new(x * 40.0, y * 40.0),
        winit::event::MouseScrollDelta::PixelDelta(position) => {
            metrics.physical_to_logical(Offset::new(position.x as f32, position.y as f32))
        }
    };
    PlatformEvent::Input(InputEvent::Scroll { delta })
}
/// Normalizes physical navigation/shortcut keys. Printable Unicode remains a
/// separate text event so keyboard layout and IME behaviour stay native.
#[must_use]
pub fn key_event(
    event: &winit::event::KeyEvent,
    modifiers: winit::keyboard::ModifiersState,
) -> PlatformEvent {
    use winit::keyboard::{Key, KeyCode as NativeKeyCode, PhysicalKey};
    let code = match event.physical_key {
        PhysicalKey::Code(NativeKeyCode::Tab) => KeyCode::Tab,
        PhysicalKey::Code(NativeKeyCode::Enter) => KeyCode::Enter,
        PhysicalKey::Code(NativeKeyCode::Escape) => KeyCode::Escape,
        PhysicalKey::Code(NativeKeyCode::Backspace) => KeyCode::Backspace,
        PhysicalKey::Code(NativeKeyCode::Delete) => KeyCode::Delete,
        PhysicalKey::Code(NativeKeyCode::ArrowLeft) => KeyCode::ArrowLeft,
        PhysicalKey::Code(NativeKeyCode::ArrowRight) => KeyCode::ArrowRight,
        PhysicalKey::Code(NativeKeyCode::ArrowUp) => KeyCode::ArrowUp,
        PhysicalKey::Code(NativeKeyCode::ArrowDown) => KeyCode::ArrowDown,
        PhysicalKey::Code(NativeKeyCode::Home) => KeyCode::Home,
        PhysicalKey::Code(NativeKeyCode::End) => KeyCode::End,
        PhysicalKey::Code(NativeKeyCode::PageUp) => KeyCode::PageUp,
        PhysicalKey::Code(NativeKeyCode::PageDown) => KeyCode::PageDown,
        PhysicalKey::Code(NativeKeyCode::KeyA) => KeyCode::KeyA,
        PhysicalKey::Code(NativeKeyCode::KeyC) => KeyCode::KeyC,
        PhysicalKey::Code(NativeKeyCode::KeyV) => KeyCode::KeyV,
        PhysicalKey::Code(NativeKeyCode::KeyX) => KeyCode::KeyX,
        _ => match &event.logical_key {
            Key::Named(named) => named_key_code(*named),
            _ => KeyCode::Other,
        },
    };
    PlatformEvent::Input(InputEvent::Key(KeyEvent {
        code,
        pressed: event.state.is_pressed(),
        repeat: event.repeat,
        modifiers: Modifiers {
            shift: modifiers.shift_key(),
            control: modifiers.control_key(),
            alt: modifiers.alt_key(),
            super_key: modifiers.super_key(),
            // This Linux backend treats Control as the desktop shortcut key.
            command: modifiers.control_key(),
        },
    }))
}
fn named_key_code(key: winit::keyboard::NamedKey) -> KeyCode {
    use winit::keyboard::NamedKey;
    match key {
        NamedKey::Tab => KeyCode::Tab,
        NamedKey::Enter => KeyCode::Enter,
        NamedKey::Escape => KeyCode::Escape,
        NamedKey::Backspace => KeyCode::Backspace,
        NamedKey::Delete => KeyCode::Delete,
        NamedKey::ArrowLeft => KeyCode::ArrowLeft,
        NamedKey::ArrowRight => KeyCode::ArrowRight,
        NamedKey::ArrowUp => KeyCode::ArrowUp,
        NamedKey::ArrowDown => KeyCode::ArrowDown,
        NamedKey::Home => KeyCode::Home,
        NamedKey::End => KeyCode::End,
        NamedKey::PageUp => KeyCode::PageUp,
        NamedKey::PageDown => KeyCode::PageDown,
        _ => KeyCode::Other,
    }
}
#[must_use]
pub fn text_event(event: &winit::event::KeyEvent) -> Option<PlatformEvent> {
    (event.state.is_pressed())
        .then_some(event.text.as_ref())
        .flatten()
        // Winit can supply a control character (notably U+0008) as the text
        // representation of a named editing key. Commands travel through
        // `key_event`; forwarding this payload would reinsert it immediately
        // after Backspace/Delete handling. Text input is therefore printable
        // Unicode only. Newline remains a command and is handled by Runtime.
        .filter(|text| is_committed_text(text))
        .map(|text| PlatformEvent::Input(InputEvent::Text(text.to_string())))
}
fn is_committed_text(text: &str) -> bool {
    !text.is_empty() && !text.chars().any(char::is_control)
}
#[must_use]
pub fn ime_event(event: winit::event::Ime) -> Option<PlatformEvent> {
    let event = match event {
        winit::event::Ime::Preedit(text, selection) => ImeEvent::Preedit { text, selection },
        winit::event::Ime::Commit(text) => ImeEvent::Commit(text),
        winit::event::Ime::Disabled => ImeEvent::End,
        winit::event::Ime::Enabled => return None,
    };
    Some(PlatformEvent::Input(InputEvent::Ime(event)))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dpi_round_trip_supports_fractional_scales() {
        let m = WindowMetrics::new(PhysicalSize::new(225, 150), 1.5);
        assert_eq!(m.logical_size(), Size::new(150., 100.));
        assert_eq!(
            m.physical_to_logical(Offset::new(75., 30.)),
            Offset::new(50., 20.)
        );
        assert_eq!(
            m.logical_to_physical(Offset::new(50., 20.)),
            Offset::new(75., 30.)
        );
    }
    #[test]
    fn identified_pointer_conversion_preserves_contact_identity() {
        let metrics = WindowMetrics::new(PhysicalSize::new(200, 100), 2.0);
        let PlatformEvent::Input(InputEvent::PointerWithId {
            pointer,
            phase,
            position,
        }) = identified_pointer_event(
            42,
            PointerPhase::Down,
            winit::dpi::PhysicalPosition::new(50., 30.),
            metrics,
        )
        else {
            unreachable!()
        };
        assert_eq!(pointer, 42);
        assert_eq!(phase, PointerPhase::Down);
        assert_eq!(position, Offset::new(25., 15.));
    }
    #[test]
    fn scroll_forms_preserve_direction_and_fractional_pixels() {
        let metrics = WindowMetrics::new(PhysicalSize::new(200, 100), 2.0);
        let PlatformEvent::Input(InputEvent::Scroll { delta: line }) =
            wheel_event(winit::event::MouseScrollDelta::LineDelta(0., 1.), metrics)
        else {
            unreachable!()
        };
        let PlatformEvent::Input(InputEvent::Scroll { delta: pixel }) = wheel_event(
            winit::event::MouseScrollDelta::PixelDelta(winit::dpi::PhysicalPosition::new(0., 80.)),
            metrics,
        ) else {
            unreachable!()
        };
        assert_eq!(line.y, pixel.y);
        let PlatformEvent::Input(InputEvent::Scroll { delta }) = wheel_event(
            winit::event::MouseScrollDelta::PixelDelta(winit::dpi::PhysicalPosition::new(0., 2.5)),
            metrics,
        ) else {
            unreachable!()
        };
        assert_eq!(delta.y, 1.25);
    }
    #[test]
    fn command_control_text_is_not_a_text_commit() {
        assert!(!is_committed_text("\u{8}")); // winit's possible Backspace text payload
        assert!(!is_committed_text("\n"));
        assert!(is_committed_text("é"));
    }
    #[test]
    fn named_backspace_falls_back_when_no_physical_code_is_available() {
        assert_eq!(
            named_key_code(winit::keyboard::NamedKey::Backspace),
            KeyCode::Backspace
        );
    }
}
