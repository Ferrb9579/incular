//! Winit conversions and native access for desktop backend integrations.
//!
//! Runtime and widgets consume the portable values produced here. Raw handle
//! values borrow native ownership; callers must retain the window while using
//! them. All window/IME operations run on the host event-loop thread.
use incular_core::{
    Code, ImeEvent, InputEvent, KeyState, KeyboardEvent, KeyboardKey, Location, Modifiers,
    NamedKey, NormalizedPressure, Offset, PointerPhase, PointerSampleMetadata, TrackpadGesture,
    TrackpadGesturePhase,
};
use incular_platform::{
    NativePointerSample, NativeWindowSystem, PlatformEvent, PointerMetadata, TextInputCommand,
    WindowMetrics,
};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
/// Applies a portable text-input command to a Winit window. Winit accepts
/// logical positions and sizes here and performs native scale conversion.
pub fn apply_text_input_command(window: &winit::window::Window, command: &TextInputCommand) {
    match command {
        TextInputCommand::SetClient {
            configuration,
            caret_rect,
            ..
        } => {
            window.set_ime_purpose(if configuration.obscure_text {
                winit::window::ImePurpose::Password
            } else {
                winit::window::ImePurpose::Normal
            });
            window.set_ime_allowed(configuration.enabled && !configuration.read_only);
            window.set_ime_cursor_area(
                winit::dpi::LogicalPosition::new(
                    f64::from(caret_rect.origin.x),
                    f64::from(caret_rect.origin.y),
                ),
                winit::dpi::LogicalSize::new(
                    f64::from(caret_rect.size.width.max(1.0)),
                    f64::from(caret_rect.size.height.max(1.0)),
                ),
            );
        }
        TextInputCommand::Update { .. } => {}
        TextInputCommand::SetCaretRect { rect, .. } => {
            window.set_ime_cursor_area(
                winit::dpi::LogicalPosition::new(
                    f64::from(rect.origin.x),
                    f64::from(rect.origin.y),
                ),
                winit::dpi::LogicalSize::new(
                    f64::from(rect.size.width.max(1.0)),
                    f64::from(rect.size.height.max(1.0)),
                ),
            );
        }
        TextInputCommand::Hide { .. } | TextInputCommand::Clear { .. } => {
            window.set_ime_allowed(false);
        }
    }
}

/// Detached native handles used by platform adapters. These values do not
/// retain their window or display; adapters must keep their owners alive while
/// using them. GPU construction uses an owned surface target instead.
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
pub fn native_window_system(window: &winit::window::Window) -> NativeWindowSystem {
    match raw_window_handles(window).window {
        RawWindowHandle::Win32(_) => NativeWindowSystem::Win32,
        RawWindowHandle::AppKit(_) => NativeWindowSystem::AppKit,
        RawWindowHandle::Xlib(_) | RawWindowHandle::Xcb(_) => NativeWindowSystem::X11,
        RawWindowHandle::Wayland(_) => NativeWindowSystem::Wayland,
        _ => NativeWindowSystem::Other,
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

/// Converts one native pointer sample into the metadata-rich Incular path.
///
/// `buttons` is the complete chord after this native event has been applied;
/// `button` identifies the button that changed on Down/Up. Keeping both values
/// prevents a secondary press during a primary drag from masquerading as
/// another primary press.
#[must_use]
pub fn pointer_event_with_metadata(
    metadata: PointerMetadata,
    position: winit::dpi::PhysicalPosition<f64>,
    metrics: WindowMetrics,
) -> PlatformEvent {
    PlatformEvent::Input(InputEvent::PointerWithMetadata {
        pointer: metadata.pointer,
        device: metadata.device,
        kind: metadata.kind,
        buttons: metadata.buttons,
        button: metadata.button,
        sample: metadata.sample,
        phase: metadata.phase,
        position: normalize_cursor(position, metrics),
    })
}

/// Portable bit assigned to one Winit mouse button.
#[must_use]
pub const fn mouse_button_mask(button: winit::event::MouseButton) -> Option<u32> {
    match button {
        winit::event::MouseButton::Left => Some(incular_core::PRIMARY_POINTER_BUTTON),
        winit::event::MouseButton::Right => Some(incular_core::SECONDARY_POINTER_BUTTON),
        winit::event::MouseButton::Middle => Some(incular_core::TERTIARY_POINTER_BUTTON),
        winit::event::MouseButton::Back => Some(incular_core::BACK_POINTER_BUTTON),
        winit::event::MouseButton::Forward => Some(incular_core::FORWARD_POINTER_BUTTON),
        winit::event::MouseButton::Other(index) => {
            incular_core::additional_pointer_button_mask(index)
        }
    }
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

/// Converts a Winit touch contact to the metadata-rich pointer path while a
/// caller-supplied stable device ID preserves source identity.
#[must_use]
pub fn touch_event_with_device(
    touch: winit::event::Touch,
    device: u64,
    metrics: WindowMetrics,
) -> PlatformEvent {
    touch_event_with_native_sample(touch, device, None, metrics)
}

/// Converts one Winit touch/pen contact while allowing an OS facade to restore
/// native device semantics that Winit does not expose. The native sample may
/// enrich pressure, but never replaces a genuine Winit pressure value with a
/// guess when the OS field is absent.
#[must_use]
pub fn touch_event_with_native_sample(
    touch: winit::event::Touch,
    fallback_device: u64,
    native: Option<NativePointerSample>,
    metrics: WindowMetrics,
) -> PlatformEvent {
    let (phase, mut buttons, button) = match touch.phase {
        winit::event::TouchPhase::Started => (
            PointerPhase::Down,
            incular_core::PRIMARY_POINTER_BUTTON,
            Some(incular_core::PRIMARY_POINTER_BUTTON),
        ),
        winit::event::TouchPhase::Moved => (
            PointerPhase::Move,
            incular_core::PRIMARY_POINTER_BUTTON,
            None,
        ),
        winit::event::TouchPhase::Ended => (
            PointerPhase::Up,
            0,
            Some(incular_core::PRIMARY_POINTER_BUTTON),
        ),
        winit::event::TouchPhase::Cancelled => (PointerPhase::Cancel, 0, None),
    };
    if native.is_some_and(|native| native.in_contact == Some(false)) && phase == PointerPhase::Move
    {
        buttons = 0;
    }
    if phase != PointerPhase::Cancel
        && native.is_some_and(|native| {
            native
                .sample
                .stylus
                .is_some_and(|stylus| stylus.barrel_button)
        })
    {
        buttons |= incular_core::SECONDARY_POINTER_BUTTON;
    }
    let fallback_pressure = normalize_touch_force(touch.force);
    let (device, kind, sample) = native.map_or_else(
        || {
            (
                fallback_device,
                incular_core::PointerDeviceKind::Touch,
                PointerSampleMetadata {
                    pressure: fallback_pressure,
                    stylus: None,
                },
            )
        },
        |native| {
            let mut sample = native.sample;
            sample.pressure = sample.pressure.or(fallback_pressure);
            (
                native.device.unwrap_or(fallback_device),
                native.kind,
                sample,
            )
        },
    );
    pointer_event_with_metadata(
        PointerMetadata {
            pointer: touch.id,
            device,
            kind,
            buttons,
            button,
            sample,
            phase,
        },
        touch.location,
        metrics,
    )
}

fn normalize_touch_force(force: Option<winit::event::Force>) -> Option<NormalizedPressure> {
    match force? {
        winit::event::Force::Normalized(value) => NormalizedPressure::new(value),
        winit::event::Force::Calibrated {
            force,
            max_possible_force,
            altitude_angle,
        } => {
            let adjusted = altitude_angle
                .filter(|angle| angle.is_finite() && angle.sin() > 0.0)
                .map_or(force, |angle| force / angle.sin());
            NormalizedPressure::from_range(adjusted, max_possible_force)
        }
    }
}

#[must_use]
pub fn trackpad_pinch_event(
    device: u64,
    delta: f64,
    phase: winit::event::TouchPhase,
) -> Option<PlatformEvent> {
    delta.is_finite().then(|| {
        PlatformEvent::Input(InputEvent::TrackpadGesture(TrackpadGesture::Pinch {
            device,
            phase: trackpad_phase(phase),
            magnification_delta: delta as f32,
        }))
    })
}

#[must_use]
pub fn trackpad_rotation_event(
    device: u64,
    delta_degrees: f32,
    phase: winit::event::TouchPhase,
) -> Option<PlatformEvent> {
    delta_degrees.is_finite().then(|| {
        PlatformEvent::Input(InputEvent::TrackpadGesture(TrackpadGesture::Rotation {
            device,
            phase: trackpad_phase(phase),
            delta_radians: delta_degrees.to_radians(),
        }))
    })
}

#[must_use]
pub fn trackpad_pan_event(
    device: u64,
    delta: winit::dpi::PhysicalPosition<f32>,
    phase: winit::event::TouchPhase,
    metrics: WindowMetrics,
) -> Option<PlatformEvent> {
    if !delta.x.is_finite() || !delta.y.is_finite() {
        return None;
    }
    let logical = metrics.physical_to_logical(Offset::new(delta.x, delta.y));
    Some(PlatformEvent::Input(InputEvent::TrackpadGesture(
        TrackpadGesture::Pan {
            device,
            phase: trackpad_phase(phase),
            delta: logical,
        },
    )))
}

#[must_use]
pub fn trackpad_smart_magnify_event(device: u64) -> PlatformEvent {
    PlatformEvent::Input(InputEvent::TrackpadGesture(TrackpadGesture::SmartMagnify {
        device,
    }))
}

#[must_use]
pub fn trackpad_pressure_event(device: u64, pressure: f32, stage: i64) -> Option<PlatformEvent> {
    NormalizedPressure::new(f64::from(pressure)).map(|pressure| {
        PlatformEvent::Input(InputEvent::TrackpadGesture(TrackpadGesture::Pressure {
            device,
            pressure,
            stage,
        }))
    })
}

const fn trackpad_phase(phase: winit::event::TouchPhase) -> TrackpadGesturePhase {
    match phase {
        winit::event::TouchPhase::Started => TrackpadGesturePhase::Started,
        winit::event::TouchPhase::Moved => TrackpadGesturePhase::Updated,
        winit::event::TouchPhase::Ended => TrackpadGesturePhase::Ended,
        winit::event::TouchPhase::Cancelled => TrackpadGesturePhase::Cancelled,
    }
}
/// Converts wheel motion into Incular's content-offset convention. Winit's
/// positive wheel Y denotes upward wheel motion, whereas a positive vertical
/// `ScrollController` offset moves content upward. Inverting
/// the raw motion makes content follow the gesture (natural scrolling) for
/// wheel and trackpad input. A line is a documented 40 logical-pixel
/// convenience step; pixel deltas deliberately retain their fractional value.
#[must_use]
pub fn wheel_event(delta: winit::event::MouseScrollDelta, metrics: WindowMetrics) -> PlatformEvent {
    let delta = match delta {
        winit::event::MouseScrollDelta::LineDelta(x, y) => Offset::new(-x * 40.0, -y * 40.0),
        winit::event::MouseScrollDelta::PixelDelta(position) => {
            let logical =
                metrics.physical_to_logical(Offset::new(position.x as f32, position.y as f32));
            Offset::new(-logical.x, -logical.y)
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
    PlatformEvent::Input(InputEvent::Key(KeyboardEvent {
        code: physical_code(event.physical_key),
        key: logical_key(&event.logical_key),
        state: if event.state.is_pressed() {
            KeyState::Down
        } else {
            KeyState::Up
        },
        location: match event.location {
            winit::keyboard::KeyLocation::Standard => Location::Standard,
            winit::keyboard::KeyLocation::Left => Location::Left,
            winit::keyboard::KeyLocation::Right => Location::Right,
            winit::keyboard::KeyLocation::Numpad => Location::Numpad,
        },
        repeat: event.repeat,
        modifiers: normalized_modifiers(modifiers),
        // Winit emits preedit/commit separately and has no per-key composing flag.
        is_composing: false,
    }))
}
fn physical_code(key: winit::keyboard::PhysicalKey) -> Code {
    match key {
        winit::keyboard::PhysicalKey::Code(code) => {
            format!("{code:?}").parse().unwrap_or(Code::Unidentified)
        }
        winit::keyboard::PhysicalKey::Unidentified(_) => Code::Unidentified,
    }
}
fn logical_key(key: &winit::keyboard::Key) -> KeyboardKey {
    match key {
        winit::keyboard::Key::Named(named) => format!("{named:?}")
            .parse::<NamedKey>()
            .map(KeyboardKey::Named)
            .unwrap_or(KeyboardKey::Named(NamedKey::Unidentified)),
        winit::keyboard::Key::Character(text) => KeyboardKey::Character(text.to_string()),
        winit::keyboard::Key::Dead(_) => KeyboardKey::Named(NamedKey::Dead),
        winit::keyboard::Key::Unidentified(_) => KeyboardKey::Named(NamedKey::Unidentified),
    }
}
fn normalized_modifiers(state: winit::keyboard::ModifiersState) -> Modifiers {
    let mut modifiers = Modifiers::default();
    modifiers.set(Modifiers::SHIFT, state.shift_key());
    modifiers.set(Modifiers::CONTROL, state.control_key());
    modifiers.set(Modifiers::ALT, state.alt_key());
    modifiers.set(Modifiers::META, state.super_key());
    modifiers
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
