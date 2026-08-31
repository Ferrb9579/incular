//! Platform-owned window data and normalized native events.
//!
//! Layout stays in logical pixels. This crate is the single conversion boundary
//! between those coordinates and physical surface/window coordinates.

use directories::ProjectDirs;
use incular_config::ApplicationDefaults;
use incular_core::{
    Code, ImeEvent, InputEvent, KeyState, KeyboardEvent, KeyboardKey, Location, Modifiers,
    NamedKey, Offset, PointerPhase, Rect, Size,
};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use std::{fmt, path::PathBuf};

mod content_sensitivity;

pub use content_sensitivity::{
    ContentSensitivityBackend, ContentSensitivityCapability, ContentSensitivityNoOpReason,
    ContentSensitivityOutcome, MemoryContentSensitivityBackend, NoopContentSensitivityBackend,
};

/// Resolves the persistent, local application-data directory through the
/// operating system's standard project-directory conventions.
///
/// The caller supplies a stable application identifier (normally a reverse
/// domain ID).  Incular intentionally never derives it from a window title,
/// executable path, current directory, or transient window ID.  Restoration
/// snapshots belong in local data rather than the regenerable cache directory.
#[must_use]
pub fn application_data_local_directory(application_id: &str) -> Option<PathBuf> {
    (!application_id.trim().is_empty())
        .then(|| ProjectDirs::from("org", "Incular", application_id))
        .flatten()
        .map(|directories| directories.data_local_dir().to_path_buf())
}

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
        let defaults = ApplicationDefaults::DEFAULT;
        Self {
            title: defaults.window_title.to_owned(),
            initial_logical_size: defaults.initial_window_size,
            minimum_logical_size: None,
            maximum_logical_size: None,
            resizable: defaults.resizable,
            visible: defaults.visible,
            decorations: defaults.decorations,
            transparent: defaults.transparent,
            maximized: defaults.maximized,
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
            && (minimum.width > maximum.width || minimum.height > maximum.height)
        {
            return Err(WindowOptionsError::MinimumExceedsMaximum);
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
    /// Applies the retained tree's effective capture-protection policy.
    SetContentSensitivity(incular_config::ContentSensitivity),
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

/// Stable identity for the text-input client currently attached to a window.
/// It is generated by the retained tree and is independent of native handles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextInputClientId(pub u64);

impl TextInputClientId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Hints supplied to a native IME or soft-keyboard adapter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextInputType {
    #[default]
    Text,
    Multiline,
    Number,
    Phone,
    Email,
    Url,
    Password,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextInputAction {
    #[default]
    Unspecified,
    None,
    Done,
    Go,
    Search,
    Send,
    Next,
    Previous,
    Newline,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextInputConfiguration {
    pub client: TextInputClientId,
    pub input_type: TextInputType,
    pub action: TextInputAction,
    pub multiline: bool,
    pub enabled: bool,
    pub read_only: bool,
    pub obscure_text: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TextInputState {
    pub text: String,
    pub selection_start: usize,
    pub selection_end: usize,
    pub composing: Option<(usize, usize)>,
}

/// Commands emitted by the runtime and consumed by a native text-input
/// adapter. They contain no window handles and can be queued safely at the
/// runtime boundary.
#[derive(Clone, Debug, PartialEq)]
pub enum TextInputCommand {
    SetClient {
        configuration: TextInputConfiguration,
        state: TextInputState,
        caret_rect: Rect,
    },
    Update {
        client: TextInputClientId,
        state: TextInputState,
    },
    SetCaretRect {
        client: TextInputClientId,
        rect: Rect,
    },
    Hide {
        client: TextInputClientId,
    },
    Clear {
        client: TextInputClientId,
    },
}

/// Minimal adapter seam for embedders that own their own text-input bridge.
pub trait TextInputAdapter {
    fn apply(&mut self, command: &TextInputCommand);
}

/// In-memory adapter useful for tests and headless hosts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MemoryTextInputAdapter {
    commands: Vec<TextInputCommand>,
}

impl MemoryTextInputAdapter {
    #[must_use]
    pub fn commands(&self) -> &[TextInputCommand] {
        &self.commands
    }

    pub fn clear(&mut self) {
        self.commands.clear();
    }
}

impl TextInputAdapter for MemoryTextInputAdapter {
    fn apply(&mut self, command: &TextInputCommand) {
        self.commands.push(command.clone());
    }
}

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

#[derive(Clone, Debug, PartialEq)]
pub enum PlatformEvent {
    Input(InputEvent),
    /// Action chosen by a native software keyboard (for example Done, Next,
    /// or Search). Desktop backends normally express the same behavior as a
    /// physical Enter key, while mobile hosts can forward the IME action
    /// without manufacturing a keyboard event.
    TextInputAction(TextInputAction),
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
