//! Platform-owned window data and normalized native events.
//!
//! Layout stays in logical pixels. This crate is the single conversion boundary
//! between those coordinates and physical surface/window coordinates.

use directories::ProjectDirs;
use incular_config::ApplicationDefaults;
use incular_core::{
    Code, Color, ImeEvent, InputEvent, KeyState, KeyboardEvent, KeyboardKey, Location, Modifiers,
    NamedKey, NormalizedPressure, Offset, PointerPhase, PointerSampleMetadata, Rect, Size,
    TrackpadGesture, TrackpadGesturePhase,
};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use std::{fmt, path::PathBuf};

mod application_activation;
mod application_shell;
mod capabilities;
mod content_sensitivity;
mod data_transfer;
mod display;
mod file_dialog;
mod operation;
mod pointer;
mod system_environment;
mod window_control;

pub use application_activation::{
    ApplicationActivation, GlobalShortcutChord, GlobalShortcutChordError, GlobalShortcutError,
    GlobalShortcutId, LaunchActivation, SingleInstancePolicy, SingleInstancePolicyError,
    UrlActivation,
};
pub use application_shell::{
    ApplicationBadge, ApplicationShellError, ApplicationShellFeature, NotificationAction,
    NotificationActionId, NotificationId, NotificationPresentation, TaskbarDockState,
    TaskbarProgress, TaskbarProgressError, TrayItemId, TrayItemPresentation,
};
pub use capabilities::{
    AdvancedInputCapabilities, ApplicationServiceCapabilities, CapabilitySupport,
    DataTransferCapabilities, DisplayPlacementCapabilities, NativeMenuCapabilities,
    PlatformCapabilities, TransientSurfaceCapabilities, WindowControlCapabilities,
};
pub use content_sensitivity::{
    ContentSensitivityBackend, ContentSensitivityCapability, ContentSensitivityNoOpReason,
    ContentSensitivityOutcome, MemoryContentSensitivityBackend, NoopContentSensitivityBackend,
};
pub use data_transfer::{
    Clipboard, ClipboardCapabilities, ClipboardError, ClipboardWriteReport, DataTransfer,
    ExternalDragEvent, ExternalDragPhase, ExternalDragResponse, MediaType, MemoryClipboard,
    TransferData, TransferDataError, TransferFormat, TransferFormatCapabilities, TransferImage,
    TransferItem, TransferOperation, TransferOperations, TransferReadRequest,
    TransferRepresentation,
};
pub use display::{
    DisplayId, DisplayPlacementArea, DisplaySnapshot, LogicalDisplayPosition,
    LogicalScreenPosition, LogicalScreenRect, PhysicalDisplayPosition, PhysicalScreenPosition,
    PhysicalScreenRect,
};
pub use file_dialog::{
    DocumentActivation, DocumentDescriptor, FileContentType, FileContentTypeKind,
    FileDialogCapabilities, FileDialogError, FileDialogFilter, FileDialogFilterId, FileDialogKind,
    FileDialogOption, FileDialogOptions, FileDialogOutcome, FileDialogRequest, FileDialogRequestId,
    FileDialogSelection, FileDialogValidationError, FileExtension,
};
pub use incular_config::{TransparencyMode, WindowSizePolicy};
pub use operation::{
    NativeOperationCompletion, NativeRequestId, PlatformOperationError, PlatformOperationErrorKind,
    PlatformOperationResult,
};
pub use pointer::{
    CursorGrabMode, LogicalWindowPosition, LogicalWindowPositionError, NativePointerSample,
    PointerMetadata,
};
pub use system_environment::{
    MemorySystemEnvironmentProvider, SystemEnvironmentPreferences, SystemEnvironmentProvider,
    canonicalize_system_locales,
};
pub use window_control::{
    LogicalSizeLimits, LogicalSizeLimitsError, UserAttentionType, WindowIcon, WindowIconError,
    WindowLevel, WindowObservedState, WindowRequestedState,
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
/// All sizes are logical pixels. A transparent window is a strict presentation
/// request: a backend must either configure compatible compositor alpha or
/// report that the request is unsupported rather than silently presenting an
/// opaque/black surface.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowOptions {
    pub title: String,
    pub initial_logical_size: Size,
    /// Selects whether native viewport metrics or retained content own size.
    ///
    /// In [`WindowSizePolicy::Content`] mode `initial_logical_size` is also the
    /// minimum content viewport. The framework follows the retained root's
    /// layout size and can shrink back when primary content becomes smaller.
    /// Paint-only overflow such as shadows and transient overlays does not
    /// resize the native host. This is deliberately independent of window
    /// decorations.
    pub size_policy: WindowSizePolicy,
    pub minimum_logical_size: Option<Size>,
    pub maximum_logical_size: Option<Size>,
    pub resizable: bool,
    pub visible: bool,
    pub decorations: bool,
    pub transparency_mode: TransparencyMode,
    /// Base scene color. This does not change native compositor transparency.
    pub background_color: Color,
    pub maximized: bool,
    pub fullscreen: Option<Fullscreen>,
    pub window_level: WindowLevel,
    pub window_icon: Option<WindowIcon>,
}

impl Default for WindowOptions {
    fn default() -> Self {
        let defaults = ApplicationDefaults::DEFAULT;
        Self {
            title: defaults.window_title.to_owned(),
            initial_logical_size: defaults.initial_window_size,
            size_policy: defaults.window_size_policy,
            minimum_logical_size: None,
            maximum_logical_size: None,
            resizable: defaults.resizable,
            visible: defaults.visible,
            decorations: defaults.decorations,
            transparency_mode: defaults.transparency_mode,
            background_color: defaults.background_color,
            maximized: defaults.maximized,
            fullscreen: None,
            window_level: WindowLevel::Normal,
            window_icon: None,
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

    pub fn requested_state(&self) -> Result<WindowRequestedState, LogicalSizeLimitsError> {
        let size_limits =
            LogicalSizeLimits::new(self.minimum_logical_size, self.maximum_logical_size)?;
        Ok(WindowRequestedState {
            visible: self.visible,
            minimized: false,
            maximized: self.maximized,
            fullscreen: self.fullscreen,
            resizable: self.resizable,
            decorations: self.decorations,
            size_limits,
            level: self.window_level,
        })
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
    pub request_id: Option<NativeRequestId>,
}

impl WindowCommand {
    #[must_use]
    pub const fn new(window_id: WindowId, operation: WindowOperation) -> Self {
        Self {
            window_id,
            operation,
            request_id: None,
        }
    }

    /// Associates this command with one result-bearing runtime request.
    #[must_use]
    pub const fn with_request(
        window_id: WindowId,
        request_id: NativeRequestId,
        operation: WindowOperation,
    ) -> Self {
        Self {
            window_id,
            operation,
            request_id: Some(request_id),
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
    SetOuterPosition(PhysicalScreenPosition),
    BeginMoveDrag,
    BeginResizeDrag(incular_core::WindowResizeDirection),
    SetMinimized(bool),
    SetMaximized(bool),
    SetFullscreen(Option<Fullscreen>),
    SetResizable(bool),
    SetDecorations(bool),
    SetLogicalSizeLimits(LogicalSizeLimits),
    SetWindowLevel(WindowLevel),
    SetWindowIcon(Option<WindowIcon>),
    RequestUserAttention(Option<UserAttentionType>),
    SetCursorGrab(CursorGrabMode),
    SetCursorVisible(bool),
    SetCursorPosition(LogicalWindowPosition),
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

    #[must_use]
    pub const fn state_changed(window_id: WindowId, state: WindowObservedState) -> Self {
        Self::new(window_id, WindowEventKind::StateChanged(state))
    }

    /// Returns the wrapped legacy event when this is a platform event.
    #[must_use]
    pub fn platform_event(&self) -> Option<&PlatformEvent> {
        match &self.kind {
            WindowEventKind::Platform(event) => Some(event),
            WindowEventKind::Lifecycle(_)
            | WindowEventKind::RedrawRequested
            | WindowEventKind::StateChanged(_) => None,
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
    /// Snapshot of native state that the active backend can actually observe.
    StateChanged(WindowObservedState),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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
    /// Complete normalized environment replacement produced by the native
    /// shell. Runtime dependency tracking decides which consumers invalidate.
    Environment(incular_config::RuntimeEnvironment),
    Lifecycle(PlatformLifecycle),
    ExternalDrag(ExternalDragEvent),
    CloseRequested,
}

/// Lifecycle states native adapters can report without depending on the
/// runtime crate. Adapters may emit only the states their OS exposes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlatformLifecycle {
    Active,
    Inactive,
    Suspended,
    /// The OS reports that a previously suspended application is runnable
    /// again. Runtime maps this to `ApplicationLifecycle::Active` while keeping
    /// the native transition explicit at the platform boundary.
    Resumed,
    Stopping,
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

/// Native desktop window-system family for capability refinement.
///
/// This is a backend diagnostic/capability value, not a public native handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NativeWindowSystem {
    Win32,
    AppKit,
    X11,
    Wayland,
    Other,
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
