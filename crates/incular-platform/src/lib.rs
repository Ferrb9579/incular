//! Platform-owned window data and normalized native events.
//!
//! Layout stays in logical pixels. This crate is the single conversion boundary
//! between those coordinates and physical surface/window coordinates.

use incular_core::{
    ImeEvent, InputEvent, KeyCode, KeyEvent, Modifiers, Offset, PointerPhase, Size,
};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use std::fmt;

/// Stable identity for an Incular window.
///
/// A [`WindowId`] is deliberately unrelated to a native window handle. Window
/// registries allocate an index and advance its generation whenever that slot
/// is reused, so a command retained for a closed window cannot affect the next
/// window occupying the same slot.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WindowId {
    index: u32,
    generation: u32,
}

impl WindowId {
    /// Creates an ID from a registry slot and its current generation.
    ///
    /// This is public for platform/runtime registries, not for representing a
    /// native ID. Application code normally obtains IDs from `WindowHandle`.
    #[must_use]
    pub const fn from_parts(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }

    /// Returns the stable slot index selected by the owning window registry.
    #[must_use]
    pub const fn index(self) -> u32 {
        self.index
    }

    /// Returns the slot generation used to reject stale handles and commands.
    #[must_use]
    pub const fn generation(self) -> u32 {
        self.generation
    }
}

impl fmt::Debug for WindowId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "WindowId({}, {})", self.index, self.generation)
    }
}

impl fmt::Display for WindowId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Window#{}@{}", self.index + 1, self.generation)
    }
}

/// The portable subset of fullscreen behaviour supported by Incular.
///
/// Borderless fullscreen intentionally avoids exposing platform monitor/video
/// mode handles. Native adapters may use the monitor containing the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fullscreen {
    Borderless,
}

/// Configuration used when creating an Incular desktop window.
///
/// All sizes are logical pixels. `transparent` is a request: native adapters
/// may fall back when transparent windows are unavailable on the host.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowOptions {
    pub title: String,
    pub initial_logical_size: Size,
    pub minimum_logical_size: Option<Size>,
    pub maximum_logical_size: Option<Size>,
    pub resizable: bool,
    pub visible: bool,
    pub decorations: bool,
    pub transparent: bool,
    pub maximized: bool,
    pub fullscreen: Option<Fullscreen>,
}

impl Default for WindowOptions {
    fn default() -> Self {
        Self {
            title: "Incular".to_owned(),
            initial_logical_size: Size::new(800.0, 600.0),
            minimum_logical_size: None,
            maximum_logical_size: None,
            resizable: true,
            visible: true,
            decorations: true,
            transparent: false,
            maximized: false,
            fullscreen: None,
        }
    }
}

impl WindowOptions {
    /// Creates options with the documented desktop defaults and a chosen title.
    #[must_use]
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            ..Self::default()
        }
    }

    /// Checks size constraints before a native adapter attempts window creation.
    pub fn validate(&self) -> Result<(), WindowOptionsError> {
        if !is_positive_size(self.initial_logical_size) {
            return Err(WindowOptionsError::InvalidInitialSize);
        }
        if self
            .minimum_logical_size
            .is_some_and(|size| !is_positive_size(size))
        {
            return Err(WindowOptionsError::InvalidMinimumSize);
        }
        if self
            .maximum_logical_size
            .is_some_and(|size| !is_positive_size(size))
        {
            return Err(WindowOptionsError::InvalidMaximumSize);
        }
        if let (Some(minimum), Some(maximum)) =
            (self.minimum_logical_size, self.maximum_logical_size)
        {
            if minimum.width > maximum.width || minimum.height > maximum.height {
                return Err(WindowOptionsError::MinimumExceedsMaximum);
            }
        }
        if self
            .minimum_logical_size
            .is_some_and(|minimum| !size_is_at_least(self.initial_logical_size, minimum))
        {
            return Err(WindowOptionsError::InitialSizeBelowMinimum);
        }
        if self
            .maximum_logical_size
            .is_some_and(|maximum| !size_is_at_least(maximum, self.initial_logical_size))
        {
            return Err(WindowOptionsError::InitialSizeAboveMaximum);
        }
        Ok(())
    }
}

fn is_positive_size(size: Size) -> bool {
    size.width.is_finite() && size.height.is_finite() && size.width > 0.0 && size.height > 0.0
}

fn size_is_at_least(left: Size, right: Size) -> bool {
    left.width >= right.width && left.height >= right.height
}

/// A contradiction in portable window-creation constraints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowOptionsError {
    InvalidInitialSize,
    InvalidMinimumSize,
    InvalidMaximumSize,
    MinimumExceedsMaximum,
    InitialSizeBelowMinimum,
    InitialSizeAboveMaximum,
}

impl fmt::Display for WindowOptionsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let description = match self {
            Self::InvalidInitialSize => "initial logical size must be finite and positive",
            Self::InvalidMinimumSize => "minimum logical size must be finite and positive",
            Self::InvalidMaximumSize => "maximum logical size must be finite and positive",
            Self::MinimumExceedsMaximum => {
                "minimum logical size cannot exceed maximum logical size"
            }
            Self::InitialSizeBelowMinimum => {
                "initial logical size cannot be smaller than the minimum logical size"
            }
            Self::InitialSizeAboveMaximum => {
                "initial logical size cannot be larger than the maximum logical size"
            }
        };
        formatter.write_str(description)
    }
}

impl std::error::Error for WindowOptionsError {}

/// A UI-thread operation requested for one Incular window.
///
/// Commands are data only: Tokio workers can enqueue them through the runtime
/// UI dispatcher, while native adapters apply them from their event-loop turn.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowCommand {
    pub window_id: WindowId,
    pub operation: WindowOperation,
}

impl WindowCommand {
    #[must_use]
    pub const fn new(window_id: WindowId, operation: WindowOperation) -> Self {
        Self {
            window_id,
            operation,
        }
    }
}

/// Operations available through an Incular `WindowHandle`.
///
/// `Close` is an accepted application close operation. A native close gesture
/// is reported first as [`PlatformEvent::CloseRequested`] in a [`WindowEvent`]
/// so applications can accept or cancel it.
#[derive(Clone, Debug, PartialEq)]
pub enum WindowOperation {
    SetTitle(String),
    SetVisible(bool),
    SetLogicalSize(Size),
    RequestFocus,
    RequestRedraw,
    Close,
}

/// Native lifecycle state for one window, independent of application lifecycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowLifecycle {
    Creating,
    Visible,
    Hidden,
    Focused,
    Unfocused,
    Closing,
    Closed,
}

/// A native event routed to one normalized Incular window.
///
/// Existing [`PlatformEvent`] values remain unchanged and are carried by the
/// `Platform` variant. This lets a single-window runtime keep using
/// `PlatformEvent` while a multi-window runtime associates every input,
/// metrics update, and close request with a [`WindowId`].
#[derive(Clone, Debug, PartialEq)]
pub struct WindowEvent {
    pub window_id: WindowId,
    pub kind: WindowEventKind,
}

impl WindowEvent {
    #[must_use]
    pub const fn new(window_id: WindowId, kind: WindowEventKind) -> Self {
        Self { window_id, kind }
    }

    #[must_use]
    pub fn platform(window_id: WindowId, event: PlatformEvent) -> Self {
        Self::new(window_id, WindowEventKind::Platform(event))
    }

    #[must_use]
    pub const fn lifecycle(window_id: WindowId, lifecycle: WindowLifecycle) -> Self {
        Self::new(window_id, WindowEventKind::Lifecycle(lifecycle))
    }

    #[must_use]
    pub const fn redraw_requested(window_id: WindowId) -> Self {
        Self::new(window_id, WindowEventKind::RedrawRequested)
    }

    /// Returns the wrapped legacy event when this is a platform event.
    #[must_use]
    pub fn platform_event(&self) -> Option<&PlatformEvent> {
        match &self.kind {
            WindowEventKind::Platform(event) => Some(event),
            WindowEventKind::Lifecycle(_) | WindowEventKind::RedrawRequested => None,
        }
    }
}

/// Window event data that is not part of the legacy [`PlatformEvent`] stream.
#[derive(Clone, Debug, PartialEq)]
pub enum WindowEventKind {
    /// Existing input, metrics, application lifecycle, or close-request data.
    Platform(PlatformEvent),
    /// A state transition of the native window itself.
    Lifecycle(WindowLifecycle),
    /// The native surface is ready for the target window to present a frame.
    RedrawRequested,
}

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
    Metrics(WindowMetrics),
    Lifecycle(PlatformLifecycle),
    CloseRequested,
}

/// Lifecycle states native adapters can report without depending on the
/// runtime crate. Adapters may emit only the states their OS exposes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlatformLifecycle {
    Active,
    Inactive,
    Suspended,
    Stopping,
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
/// Converts wheel motion into Incular's content-offset convention. Winit's
/// positive wheel Y denotes upward wheel motion, whereas a positive vertical
/// [`incular_scroll::ScrollController`] offset moves content upward. Inverting
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
    fn window_id_generation_distinguishes_reused_slots() {
        let closed = WindowId::from_parts(7, 2);
        let replacement = WindowId::from_parts(7, 3);

        assert_ne!(closed, replacement);
        assert_eq!(replacement.index(), 7);
        assert_eq!(replacement.generation(), 3);
        assert_eq!(replacement.to_string(), "Window#8@3");
    }

    #[test]
    fn window_options_validate_portable_size_constraints() {
        let mut options = WindowOptions::new("Inspector");
        options.initial_logical_size = Size::new(800.0, 600.0);
        options.minimum_logical_size = Some(Size::new(400.0, 300.0));
        options.maximum_logical_size = Some(Size::new(1_600.0, 1_200.0));
        assert_eq!(options.validate(), Ok(()));

        options.minimum_logical_size = Some(Size::new(1_800.0, 300.0));
        assert_eq!(
            options.validate(),
            Err(WindowOptionsError::MinimumExceedsMaximum)
        );

        options.minimum_logical_size = Some(Size::new(400.0, 300.0));
        options.initial_logical_size = Size::ZERO;
        assert_eq!(
            options.validate(),
            Err(WindowOptionsError::InvalidInitialSize)
        );
    }

    #[test]
    fn initial_size_must_respect_declared_bounds() {
        let options = WindowOptions {
            initial_logical_size: Size::new(300.0, 300.0),
            minimum_logical_size: Some(Size::new(400.0, 200.0)),
            ..WindowOptions::default()
        };
        assert_eq!(
            options.validate(),
            Err(WindowOptionsError::InitialSizeBelowMinimum)
        );

        let options = WindowOptions {
            initial_logical_size: Size::new(300.0, 300.0),
            maximum_logical_size: Some(Size::new(200.0, 400.0)),
            ..WindowOptions::default()
        };
        assert_eq!(
            options.validate(),
            Err(WindowOptionsError::InitialSizeAboveMaximum)
        );
    }

    #[test]
    fn command_preserves_target_and_operation_without_native_data() {
        let window_id = WindowId::from_parts(3, 1);
        let command = WindowCommand::new(
            window_id,
            WindowOperation::SetLogicalSize(Size::new(640.0, 480.0)),
        );

        assert_eq!(command.window_id, window_id);
        assert_eq!(
            command.operation,
            WindowOperation::SetLogicalSize(Size::new(640.0, 480.0))
        );
    }

    #[test]
    fn window_events_preserve_legacy_platform_events_with_window_identity() {
        let id = WindowId::from_parts(1, 0);
        let event = WindowEvent::platform(id, PlatformEvent::CloseRequested);
        assert_eq!(event.window_id, id);
        assert_eq!(event.platform_event(), Some(&PlatformEvent::CloseRequested));
        assert_eq!(
            WindowEvent::lifecycle(id, WindowLifecycle::Focused).kind,
            WindowEventKind::Lifecycle(WindowLifecycle::Focused)
        );
    }

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
    fn scroll_forms_use_natural_content_direction_and_fractional_pixels() {
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
        assert_eq!(line.y, -40.);
        let PlatformEvent::Input(InputEvent::Scroll { delta }) = wheel_event(
            winit::event::MouseScrollDelta::PixelDelta(winit::dpi::PhysicalPosition::new(0., 2.5)),
            metrics,
        ) else {
            unreachable!()
        };
        assert_eq!(delta.y, -1.25);
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
