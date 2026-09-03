//! Shared desktop event-loop bridge for Incular.
mod clipboard;
mod display;
mod display_identity;
mod environment;
mod external_drag;
mod file_dialogs;
mod global_shortcuts;
mod platform_menus;
mod platform_services;
mod pointer;
mod single_instance;
mod transients;

#[doc(hidden)]
pub use external_drag::ExternalFileDragState;
#[doc(hidden)]
pub use platform_menus::{
    NativeMenuCommandId, NativeMenuCommandRegistry, NativeMenuRegistryError, format_menu_shortcut,
    matching_menu_shortcut, native_menu_structure_equal,
};
pub use platform_services::{DefaultDesktopPlatformServices, DesktopPlatformServices};
#[doc(hidden)]
pub use single_instance::{
    SingleInstanceError, SingleInstancePrimary, SingleInstanceRole, acquire_single_instance,
    acquire_single_instance_in,
};

use accesskit_winit::{
    Adapter as AccessKitAdapter, Event as AccessKitEvent, WindowEvent as AccessKitWindowEvent,
};
use incular_accessibility::AccessKitProjection;
use incular_config::{Constraints, TransientPresentation, TransientRole};
#[cfg(feature = "devtools")]
use incular_core::PRIMARY_POINTER_BUTTON;
use incular_core::{Color, InputEvent, Offset};
use incular_core::{PointerDeviceKind, PointerPhase, WindowResizeDirection};
use incular_platform::{
    ApplicationActivation, CapabilitySupport, ContentSensitivityBackend,
    ContentSensitivityNoOpReason, ContentSensitivityOutcome, CursorGrabMode,
    NativeOperationCompletion, NativeWindowSystem, NoopContentSensitivityBackend,
    PhysicalScreenPosition, PhysicalSize, PlatformCapabilities, PlatformEvent, PlatformLifecycle,
    PlatformOperationError, PlatformOperationErrorKind, PointerMetadata, TransparencyMode,
    UserAttentionType, WindowCommand, WindowEvent as IncularWindowEvent, WindowIcon,
    WindowId as IncularWindowId, WindowLevel, WindowLifecycle, WindowMetrics, WindowObservedState,
    WindowOperation, WindowOptions, apply_text_input_command, ime_event, key_event,
    native_window_system, pointer_event_with_metadata, raw_window_handles, text_event,
    touch_event_with_device, wheel_event,
};
use incular_rendering::DisplayList;
use incular_runtime::{
    Application, GlobalShortcutCompletionStatus, GpuSample, NativeFileDialogCompletion,
    NativeGlobalShortcutCompletion, NativeGlobalShortcutOperation, NativeWindowCommand,
    RenderFrameMetrics, ResolvedTransientPresentation, Runtime, RuntimeWake, Screenshot,
    TransientFallbackReason, TransientPresentationResolution,
};
use incular_wgpu::{RenderStats, RendererError, SharedGpuContext, WgpuRenderer};
use incular_widgets::{
    PlatformMenuDelegate, ShortcutModifiers, TransientSurfaceId, TransientSurfaceSnapshot,
    internal::ActionId,
};
use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::Arc,
    time::Instant,
};
#[cfg(feature = "devtools")]
use std::{path::PathBuf, process::Command};
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalPosition, PhysicalPosition},
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{
        CursorGrabMode as NativeCursorGrabMode, Icon as NativeWindowIcon,
        ResizeDirection as NativeResizeDirection, UserAttentionType as NativeAttentionType, Window,
        WindowAttributes, WindowId as NativeWindowId, WindowLevel as NativeWindowLevel,
    },
};

use crate::clipboard::DesktopClipboard;
#[doc(hidden)]
pub use crate::clipboard::desktop_clipboard_write_report;
use crate::display::DesktopDisplayRegistry;
use crate::environment::DesktopEnvironmentProvider;
#[doc(hidden)]
pub use crate::file_dialogs::desktop_file_dialog_capabilities;
use crate::global_shortcuts::{
    DesktopGlobalShortcuts, GlobalShortcutEventSinkGuard, NativeGlobalShortcutEvent,
};
use crate::pointer::{MouseButtonState, NativeCursorCoordinator, PointerDeviceRegistry};
use crate::transients::{NativeTransientState, TransientHostKey, TransientNativeRejection};

#[derive(Debug)]
pub enum RunError {
    EventLoop(winit::error::EventLoopError),
    SingleInstance(SingleInstanceError),
}
impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EventLoop(e) => write!(f, "event-loop setup failed: {e}"),
            Self::SingleInstance(error) => write!(f, "single-instance setup failed: {error}"),
        }
    }
}
impl std::error::Error for RunError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::EventLoop(error) => Some(error),
            Self::SingleInstance(error) => Some(error),
        }
    }
}

fn runtime_render_metrics(stats: &RenderStats) -> RenderFrameMetrics {
    RenderFrameMetrics {
        draw_calls: stats.draw_calls,
        instances: stats.total_instances(),
        render_passes: stats.render_passes,
        path_triangles: stats.path_triangles,
        upload_bytes: stats.upload_bytes,
        texture_upload_bytes: stats.texture_upload_bytes,
        pipelines_created: stats.pipelines_created,
        queue_submissions: stats.queue_submissions,
        prepare_us: stats.prepare_us,
        encode_us: stats.encode_us,
        submit_us: stats.submit_us,
        ..RenderFrameMetrics::default()
    }
}
/// Runs a native desktop window until close. The native window stays alive for the
/// complete lifetime of the renderer's unsafe raw-handle surface.
pub fn run_window(
    runtime: Runtime,
    on_action: impl FnMut(ActionId) + 'static,
) -> Result<(), RunError> {
    run_window_with_services(runtime, on_action, DefaultDesktopPlatformServices)
}

/// Runs the standalone-runtime desktop path with the same OS environment and
/// lifecycle facade used by [`run_application_with_services`]. Platform crates
/// call this so `run_window` and `run_application` never disagree about native
/// theme, locale, accessibility, input, or lifecycle state.
#[doc(hidden)]
pub fn run_window_with_services(
    mut runtime: Runtime,
    on_action: impl FnMut(ActionId) + 'static,
    platform_services: impl DesktopPlatformServices + 'static,
) -> Result<(), RunError> {
    runtime.set_clipboard(Box::new(DesktopClipboard::new()));
    let mut event_loop_builder = EventLoop::<RuntimeWakeEvent>::with_user_event();
    #[cfg(target_os = "windows")]
    if let Some(hook) = platform_services.windows_message_hook() {
        use winit::platform::windows::EventLoopBuilderExtWindows;
        event_loop_builder.with_msg_hook(hook);
    }
    let event_loop = event_loop_builder.build().map_err(RunError::EventLoop)?;
    let proxy = event_loop.create_proxy();
    let environment_proxy = proxy.clone();
    platform_services.start_system_environment_watch(
        runtime.tokio_handle(),
        Arc::new(move || {
            let _ = environment_proxy.send_event(RuntimeWakeEvent::SystemEnvironmentChanged);
        }),
    );
    if let Some(active) = platform_services.application_active() {
        let _ = runtime.handle_platform_event(PlatformEvent::Lifecycle(if active {
            PlatformLifecycle::Active
        } else {
            PlatformLifecycle::Inactive
        }));
    }
    runtime.set_wake_handler(Arc::new(DesktopWake(proxy.clone())));
    let mut app = App {
        window: None,
        runtime: Some(runtime),
        renderer: None,
        metrics: None,
        cursor: PhysicalPosition::new(0., 0.),
        mouse_buttons: MouseButtonState::default(),
        pointer_devices: PointerDeviceRegistry::default(),
        native_cursor: NativeCursorCoordinator::default(),
        environment: None,
        modifiers: winit::keyboard::ModifiersState::default(),
        on_action,
        accessibility_proxy: proxy,
        accessibility: None,
        platform_services: Box::new(platform_services),
    };
    event_loop.run_app(&mut app).map_err(RunError::EventLoop)
}

/// A Tokio completion wake has no payload: `Runtime` owns the bounded UI
/// message drain and runs it on the UI thread when this user event reaches
/// Winit.
enum RuntimeWakeEvent {
    Runtime,
    Accessibility(AccessKitEvent),
    FileDialog(NativeFileDialogCompletion),
    ApplicationActivation(ApplicationActivation),
    GlobalShortcut(NativeGlobalShortcutEvent),
    SingleInstanceActivation,
    SystemEnvironmentChanged,
}

impl From<AccessKitEvent> for RuntimeWakeEvent {
    fn from(event: AccessKitEvent) -> Self {
        Self::Accessibility(event)
    }
}

struct DesktopWake(winit::event_loop::EventLoopProxy<RuntimeWakeEvent>);
impl RuntimeWake for DesktopWake {
    fn wake(&self) {
        let _ = self.0.send_event(RuntimeWakeEvent::Runtime);
    }
}
/// Runs an application built with Incular's declarative root API. Button
/// callbacks are dispatched by `Runtime`; the legacy action callback is empty.
#[cfg(feature = "devtools")]
pub mod devtools_runner;

#[cfg(feature = "devtools")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DevToolsLaunchMode {
    Disabled,
    AgentOnly,
    OpenUi,
}

#[cfg(feature = "devtools")]
pub fn devtools_launch_mode_from(
    args: impl IntoIterator<Item = impl AsRef<std::ffi::OsStr>>,
    environment_enabled: bool,
) -> DevToolsLaunchMode {
    if args.into_iter().any(|argument| {
        matches!(
            argument.as_ref().to_str(),
            Some("--devtools" | "--incular-devtools")
        )
    }) {
        DevToolsLaunchMode::OpenUi
    } else if environment_enabled {
        DevToolsLaunchMode::AgentOnly
    } else {
        DevToolsLaunchMode::Disabled
    }
}

#[cfg(feature = "devtools")]
fn devtools_launch_mode() -> DevToolsLaunchMode {
    devtools_launch_mode_from(
        std::env::args_os(),
        std::env::var_os("INCULAR_DEVTOOLS").is_some(),
    )
}

#[cfg(feature = "devtools")]
pub fn devtools_ui_candidates(current_exe: Option<PathBuf>) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let ui_binary = if cfg!(windows) {
        "incular-devtools.exe"
    } else {
        "incular-devtools"
    };
    if let Some(explicit) = std::env::var_os("INCULAR_DEVTOOLS_UI") {
        candidates.push(PathBuf::from(explicit));
    }
    if let Some(executable) = current_exe
        && let Some(directory) = executable.parent()
    {
        candidates.push(directory.join(ui_binary));
        // Cargo examples live under target/{profile}/examples while the
        // DevTools binary lives one directory above them.
        if directory.file_name().and_then(|name| name.to_str()) == Some("examples")
            && let Some(profile_directory) = directory.parent()
        {
            candidates.push(profile_directory.join(ui_binary));
        }
    }
    candidates
}

#[cfg(feature = "devtools")]
fn launch_devtools_ui() {
    let pid = std::process::id().to_string();
    let candidates = devtools_ui_candidates(std::env::current_exe().ok());
    for candidate in &candidates {
        if !candidate.is_file() {
            continue;
        }
        match Command::new(candidate)
            .arg("--target-pid")
            .arg(&pid)
            .stdin(std::process::Stdio::null())
            .spawn()
        {
            Ok(_) => return,
            Err(error) => eprintln!(
                "Incular DevTools: could not launch {}: {error}",
                candidate.display()
            ),
        }
    }
    let ui_command = if cfg!(windows) {
        "incular-devtools.exe"
    } else {
        "incular-devtools"
    };
    match Command::new(ui_command)
        .arg("--target-pid")
        .arg(pid)
        .stdin(std::process::Stdio::null())
        .spawn()
    {
        Ok(_) => {}
        Err(error) => eprintln!(
            "Incular DevTools: --devtools started the target agent, but the UI binary was not found ({error}). Build it with `cargo build -p incular-devtools-ui` or set INCULAR_DEVTOOLS_UI."
        ),
    }
}

pub fn run_application(application: Application) -> Result<(), RunError> {
    run_application_with_services(application, DefaultDesktopPlatformServices)
}

/// Runs an application with optional OS-specific desktop geometry services.
/// The shared shell remains Winit-owned; facade crates use this seam only for
/// data Winit does not expose portably, such as a taskbar/dock-adjusted work
/// area.
pub fn run_application_with_services(
    mut application: Application,
    platform_services: impl DesktopPlatformServices + 'static,
) -> Result<(), RunError> {
    application.set_platform_capabilities(desktop_platform_capabilities());
    let launch_activation = ApplicationActivation::Launch(application.launch_activation());
    let single_instance = if let Some(policy) = application.single_instance_policy().cloned() {
        match acquire_single_instance(&policy, launch_activation.clone())
            .map_err(RunError::SingleInstance)?
        {
            SingleInstanceRole::Primary(primary) => Some(primary),
            SingleInstanceRole::SecondaryForwarded => return Ok(()),
        }
    } else {
        None
    };
    // Launch is an application activation, not a window event. Publish it
    // before Winit creates the first native surface so pre-run subscribers can
    // observe it and later subscribers can drain it from the activation buffer.
    application.handle_application_activation(launch_activation);
    let platform_menu_delegate = platform_services.platform_menu_delegate();
    let mut event_loop_builder = EventLoop::<RuntimeWakeEvent>::with_user_event();
    #[cfg(target_os = "windows")]
    if let Some(hook) = platform_services.windows_message_hook() {
        use winit::platform::windows::EventLoopBuilderExtWindows;
        event_loop_builder.with_msg_hook(hook);
    }
    let event_loop = event_loop_builder.build().map_err(RunError::EventLoop)?;
    let proxy = event_loop.create_proxy();
    let wake = Arc::new(DesktopWake(proxy.clone()));
    let environment_proxy = proxy.clone();
    platform_services.start_system_environment_watch(
        application.tokio_handle(),
        Arc::new(move || {
            let _ = environment_proxy.send_event(RuntimeWakeEvent::SystemEnvironmentChanged);
        }),
    );
    let activation_proxy = proxy.clone();
    platform_services.start_application_activation_watch(Arc::new(move |activation| {
        let _ = activation_proxy.send_event(RuntimeWakeEvent::ApplicationActivation(activation));
    }));
    if let Some(primary) = single_instance.as_ref() {
        let instance_proxy = proxy.clone();
        primary.set_wake(Arc::new(move || {
            let _ = instance_proxy.send_event(RuntimeWakeEvent::SingleInstanceActivation);
        }));
    }
    let shortcut_proxy = proxy.clone();
    let global_shortcut_event_sink =
        crate::global_shortcuts::install_event_sink(Arc::new(move |event| {
            let _ = shortcut_proxy.send_event(RuntimeWakeEvent::GlobalShortcut(event));
        }));
    let initial_application_active = platform_services.application_active();
    #[cfg(feature = "devtools")]
    let devtools_mode = devtools_launch_mode();
    #[cfg(feature = "devtools")]
    let devtools_agent = if devtools_mode != DevToolsLaunchMode::Disabled {
        let config = incular_devtools::session::SessionConfig {
            app_name: std::env::current_exe()
                .ok()
                .and_then(|path| path.file_name().map(|n| n.to_string_lossy().into_owned()))
                .unwrap_or_else(|| "incular-app".into()),
        };
        incular_devtools::spawn(config, application.tokio_handle())
    } else {
        None
    };
    #[cfg(feature = "devtools")]
    if devtools_agent.is_some() && devtools_mode == DevToolsLaunchMode::OpenUi {
        launch_devtools_ui();
    }
    let mut app = MultiApp {
        application,
        windows: HashMap::new(),
        native_ids: HashMap::new(),
        transient_windows: HashMap::new(),
        transient_native_ids: HashMap::new(),
        transient_native_rejections: HashMap::new(),
        shared_gpu: None,
        frame_timestamp: Instant::now(),
        event_proxy: proxy,
        displays: DesktopDisplayRegistry::default(),
        pointer_devices: PointerDeviceRegistry::default(),
        platform_services: Box::new(platform_services),
        platform_menu_delegate,
        window_system: None,
        global_shortcuts: None,
        single_instance,
        _global_shortcut_event_sink: global_shortcut_event_sink,
        pending_native_destructions: HashSet::new(),
        #[cfg(feature = "devtools")]
        devtools_state: crate::devtools_runner::DevToolsState::new(devtools_agent),
    };
    app.application.set_wake_handler(wake);
    if let Some(active) = initial_application_active {
        app.application.handle_application_lifecycle(if active {
            PlatformLifecycle::Active
        } else {
            PlatformLifecycle::Inactive
        });
    }
    event_loop.run_app(&mut app).map_err(RunError::EventLoop)
}
struct App<F: FnMut(ActionId)> {
    runtime: Option<Runtime>,
    renderer: Option<WgpuRenderer>,
    metrics: Option<WindowMetrics>,
    cursor: PhysicalPosition<f64>,
    mouse_buttons: MouseButtonState,
    pointer_devices: PointerDeviceRegistry,
    native_cursor: NativeCursorCoordinator,
    environment: Option<DesktopEnvironmentProvider>,
    modifiers: winit::keyboard::ModifiersState,
    on_action: F,
    accessibility_proxy: winit::event_loop::EventLoopProxy<RuntimeWakeEvent>,
    accessibility: Option<NativeAccessibilityState>,
    platform_services: Box<dyn DesktopPlatformServices>,
    /// Declared last so renderer/accessibility raw-handle users are dropped
    /// before the native window itself.
    window: Option<Window>,
}

/// One AccessKit adapter/projection pair belongs to exactly one native window.
/// The projection is pure retained-tree state; `Adapter` owns the OS bridge.
struct NativeAccessibilityState {
    adapter: AccessKitAdapter,
    projection: AccessKitProjection,
    active: bool,
}
impl<F: FnMut(ActionId)> ApplicationHandler<RuntimeWakeEvent> for App<F> {
    fn resumed(&mut self, loop_target: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let defaults = incular_config::ApplicationDefaults::DEFAULT;
        let window = match loop_target.create_window(
            WindowAttributes::default()
                .with_title(defaults.window_title)
                .with_inner_size(winit::dpi::LogicalSize::new(
                    f64::from(defaults.initial_window_size.width),
                    f64::from(defaults.initial_window_size.height),
                ))
                .with_visible(false),
        ) {
            Ok(window) => window,
            Err(error) => {
                eprintln!("Incular window error: {error}");
                loop_target.exit();
                return;
            }
        };
        let metrics = WindowMetrics::new(
            PhysicalSize::new(window.inner_size().width, window.inner_size().height),
            window.scale_factor(),
        );
        let renderer = match pollster::block_on(WgpuRenderer::new(
            raw_window_handles(&window),
            metrics.physical_size,
            TransparencyMode::Opaque,
            defaults.background_color,
        )) {
            Ok(renderer) => renderer,
            Err(error) => {
                eprintln!("Incular renderer initialization failed: {error}");
                loop_target.exit();
                return;
            }
        };
        window.request_redraw();
        self.metrics = Some(metrics);
        let environment = DesktopEnvironmentProvider::new(
            &window,
            self.platform_services.system_environment_preferences(),
        );
        if let Some(runtime) = self.runtime.as_mut() {
            runtime.set_environment(environment.snapshot(metrics));
        }
        self.environment = Some(environment);
        self.renderer = Some(renderer);
        let mut accessibility = NativeAccessibilityState {
            adapter: AccessKitAdapter::with_event_loop_proxy(
                loop_target,
                &window,
                self.accessibility_proxy.clone(),
            ),
            projection: AccessKitProjection::new(),
            active: false,
        };
        accessibility.projection.note_adapter_created();
        window.set_visible(true);
        self.accessibility = Some(accessibility);
        self.window = Some(window);
        self.refresh_system_environment(true);
        self.drain_application_lifecycle();
    }
    fn window_event(
        &mut self,
        loop_target: &ActiveEventLoop,
        window_id: NativeWindowId,
        event: WindowEvent,
    ) {
        if self
            .window
            .as_ref()
            .is_none_or(|window| window.id() != window_id)
        {
            return;
        }
        if let (Some(window), Some(accessibility)) =
            (self.window.as_ref(), self.accessibility.as_mut())
        {
            accessibility.adapter.process_event(window, &event);
        }
        match event {
            WindowEvent::CloseRequested => {
                if let Some(mut accessibility) = self.accessibility.take() {
                    accessibility.projection.note_adapter_destroyed();
                }
                if let Some(runtime) = self.runtime.as_mut() {
                    runtime.shutdown();
                }
                loop_target.exit();
            }
            WindowEvent::Resized(size) => self.resize(PhysicalSize::new(size.width, size.height)),
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                let size = self.window.as_ref().expect("window exists").inner_size();
                self.metrics = Some(WindowMetrics::new(
                    PhysicalSize::new(size.width, size.height),
                    scale_factor,
                ));
                self.publish_environment();
                if !PhysicalSize::new(size.width, size.height).is_zero() {
                    self.renderer
                        .as_mut()
                        .expect("renderer exists")
                        .resize(PhysicalSize::new(size.width, size.height));
                    self.window
                        .as_ref()
                        .expect("window exists")
                        .request_redraw();
                }
            }
            WindowEvent::CursorMoved {
                device_id,
                position,
            } => {
                self.note_mouse_environment();
                self.cursor = position;
                self.route_mouse_pointer(device_id, PointerPhase::Move, None);
            }
            WindowEvent::CursorEntered { device_id } => {
                self.note_mouse_environment();
                self.route_mouse_pointer(device_id, PointerPhase::Enter, None);
            }
            WindowEvent::CursorLeft { device_id } => {
                self.note_mouse_environment();
                self.route_mouse_pointer(device_id, PointerPhase::Exit, None);
            }
            WindowEvent::MouseInput {
                device_id,
                state,
                button,
            } => {
                self.note_mouse_environment();
                if let Some(transition) = self.mouse_buttons.transition(state, button) {
                    self.route_mouse_pointer(device_id, transition.phase, Some(transition.button));
                }
            }
            WindowEvent::Focused(focused) => {
                if !focused {
                    self.cancel_mouse_pointer();
                }
                if self
                    .environment
                    .as_mut()
                    .is_some_and(|environment| environment.set_focused(focused))
                {
                    self.publish_environment();
                }
            }
            WindowEvent::ThemeChanged(theme) => {
                if self
                    .environment
                    .as_mut()
                    .is_some_and(|environment| environment.set_theme(theme))
                {
                    self.publish_environment();
                }
            }
            WindowEvent::Occluded(occluded) => {
                if self
                    .environment
                    .as_mut()
                    .is_some_and(|environment| environment.set_occluded(occluded))
                {
                    self.publish_environment();
                }
            }
            WindowEvent::Touch(touch) => {
                self.note_touch_environment();
                if let (Some(runtime), Some(metrics)) = (self.runtime.as_mut(), self.metrics) {
                    let device = self.pointer_devices.id(touch.device_id);
                    let event = match touch_event_with_device(touch, device, metrics) {
                        PlatformEvent::Input(event) => event,
                        _ => unreachable!(),
                    };
                    if let Some(action) =
                        runtime.handle_input(event).and_then(|target| target.action)
                    {
                        (self.on_action)(action);
                        self.window
                            .as_ref()
                            .expect("window exists")
                            .request_redraw();
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.note_mouse_environment();
                if let (Some(runtime), Some(metrics)) = (self.runtime.as_mut(), self.metrics) {
                    let PlatformEvent::Input(event) = wheel_event(delta, metrics) else {
                        unreachable!()
                    };
                    let _ = runtime.handle_input(event);
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::KeyboardInput { event, .. } => {
                self.note_keyboard_environment();
                if let Some(runtime) = self.runtime.as_mut() {
                    let PlatformEvent::Input(input) = key_event(&event, self.modifiers) else {
                        unreachable!()
                    };
                    let _ = runtime.handle_input(input);
                    if let Some(PlatformEvent::Input(input)) = text_event(&event) {
                        let _ = runtime.handle_input(input);
                    }
                }
            }
            WindowEvent::Ime(event) => {
                if let (Some(runtime), Some(PlatformEvent::Input(input))) =
                    (self.runtime.as_mut(), ime_event(event))
                {
                    let _ = runtime.handle_input(input);
                }
            }
            WindowEvent::RedrawRequested => self.redraw(),
            _ => {}
        }
        self.refresh_system_environment(false);
        self.drain_application_lifecycle();
    }
    fn about_to_wait(&mut self, loop_target: &ActiveEventLoop) {
        self.refresh_system_environment(false);
        self.drain_application_lifecycle();
        self.apply_text_input_commands();
        if self.runtime.as_ref().is_some_and(Runtime::frame_requested) && self.window_can_present()
        {
            self.window
                .as_ref()
                .expect("window exists")
                .request_redraw();
        }
        // Tokio owns application timers on its worker runtime and wakes Winit
        // only when UI-relevant messages arrive. Frame/animation scheduling is
        // still driven by redraw requests, so the native loop can sleep idle.
        loop_target.set_control_flow(winit::event_loop::ControlFlow::Wait);
    }
    fn user_event(&mut self, _: &ActiveEventLoop, event: RuntimeWakeEvent) {
        match event {
            RuntimeWakeEvent::Runtime => {
                if self.runtime.is_some() {
                    self.runtime
                        .as_mut()
                        .expect("runtime exists")
                        .process_runtime_work();
                    self.apply_text_input_commands();
                    if self.runtime.as_ref().is_some_and(Runtime::frame_requested)
                        && self.window_can_present()
                    {
                        self.window
                            .as_ref()
                            .expect("window exists")
                            .request_redraw();
                    }
                }
            }
            RuntimeWakeEvent::Accessibility(event) => self.handle_accesskit_event(event),
            RuntimeWakeEvent::FileDialog(_) => {
                // `run_window` owns a standalone Runtime, not an Application,
                // so it cannot create native file-dialog requests. The shared
                // event enum is used only to keep one Winit event-loop bridge.
            }
            RuntimeWakeEvent::ApplicationActivation(_)
            | RuntimeWakeEvent::GlobalShortcut(_)
            | RuntimeWakeEvent::SingleInstanceActivation => {
                // Activations and global shortcuts are application-scoped and
                // intentionally unavailable on the legacy standalone runner.
            }
            RuntimeWakeEvent::SystemEnvironmentChanged => self.refresh_system_environment(true),
        }
        self.refresh_system_environment(false);
        self.drain_application_lifecycle();
    }

    fn exiting(&mut self, _: &ActiveEventLoop) {
        if let Some(runtime) = self.runtime.as_mut()
            && !matches!(
                runtime.lifecycle(),
                incular_runtime::ApplicationLifecycle::Terminated
            )
        {
            runtime.shutdown();
        }
    }
}
impl<F: FnMut(ActionId)> App<F> {
    fn refresh_system_environment(&mut self, force: bool) {
        if self.environment.is_none()
            || (!force && !self.platform_services.take_system_environment_change())
        {
            return;
        }
        let preferences = self.platform_services.system_environment_preferences();
        if self
            .environment
            .as_mut()
            .is_some_and(|environment| environment.replace_preferences(preferences))
        {
            self.publish_environment();
        }
    }

    fn drain_application_lifecycle(&mut self) {
        for lifecycle in self.platform_services.take_application_lifecycle_events() {
            let Some(runtime) = self.runtime.as_mut() else {
                return;
            };
            if lifecycle == PlatformLifecycle::Stopping {
                runtime.shutdown();
            } else {
                let _ = runtime.handle_platform_event(PlatformEvent::Lifecycle(lifecycle));
            }
        }
    }

    fn publish_environment(&mut self) {
        let (Some(runtime), Some(environment), Some(metrics)) = (
            self.runtime.as_mut(),
            self.environment.as_ref(),
            self.metrics,
        ) else {
            return;
        };
        runtime.set_environment(environment.snapshot(metrics));
    }

    fn note_mouse_environment(&mut self) {
        if self
            .environment
            .as_mut()
            .is_some_and(DesktopEnvironmentProvider::note_mouse)
        {
            self.publish_environment();
        }
    }

    fn note_touch_environment(&mut self) {
        if self
            .environment
            .as_mut()
            .is_some_and(DesktopEnvironmentProvider::note_touch)
        {
            self.publish_environment();
        }
    }

    fn note_keyboard_environment(&mut self) {
        if self
            .environment
            .as_mut()
            .is_some_and(DesktopEnvironmentProvider::note_keyboard)
        {
            self.publish_environment();
        }
    }

    fn window_can_present(&self) -> bool {
        !self
            .environment
            .as_ref()
            .is_some_and(DesktopEnvironmentProvider::is_occluded)
            && self
                .window
                .as_ref()
                .and_then(Window::is_minimized)
                .is_none_or(|minimized| !minimized)
    }

    fn route_mouse_pointer(
        &mut self,
        device_id: winit::event::DeviceId,
        phase: PointerPhase,
        button: Option<u32>,
    ) {
        let Some(metrics) = self.metrics else {
            return;
        };
        let device = self.pointer_devices.id(device_id);
        let PlatformEvent::Input(event) = pointer_event_with_metadata(
            mouse_pointer_metadata(device, self.mouse_buttons.pressed(), button, phase),
            self.cursor,
            metrics,
        ) else {
            unreachable!()
        };
        if let Some(action) = self
            .runtime
            .as_mut()
            .and_then(|runtime| runtime.handle_input(event))
            .and_then(|target| target.action)
        {
            (self.on_action)(action);
            self.window
                .as_ref()
                .expect("window exists")
                .request_redraw();
        }
        self.sync_cursor();
    }

    fn cancel_mouse_pointer(&mut self) {
        let Some(metrics) = self.metrics else {
            return;
        };
        let had_pressed_buttons = self.mouse_buttons.cancel();
        let Some(runtime) = self.runtime.as_mut() else {
            return;
        };
        if had_pressed_buttons {
            let PlatformEvent::Input(cancel) = pointer_event_with_metadata(
                mouse_pointer_metadata(0, 0, None, PointerPhase::Cancel),
                self.cursor,
                metrics,
            ) else {
                unreachable!()
            };
            let _ = runtime.handle_input(cancel);
        }
        let PlatformEvent::Input(exit) = pointer_event_with_metadata(
            mouse_pointer_metadata(0, 0, None, PointerPhase::Exit),
            self.cursor,
            metrics,
        ) else {
            unreachable!()
        };
        let _ = runtime.handle_input(exit);
        self.sync_cursor();
    }

    fn sync_cursor(&mut self) {
        let Some(cursor) = self.runtime.as_ref().map(Runtime::effective_mouse_cursor) else {
            return;
        };
        let Some(native) = self.native_cursor.update(cursor) else {
            return;
        };
        if let Some(window) = self.window.as_ref() {
            window.set_cursor(native);
        }
    }

    fn resize(&mut self, size: PhysicalSize) {
        let scale = self.metrics.expect("metrics exist").scale_factor;
        self.metrics = Some(WindowMetrics::new(size, scale));
        self.publish_environment();
        if !size.is_zero() {
            self.renderer
                .as_mut()
                .expect("renderer exists")
                .resize(size);
            self.window
                .as_ref()
                .expect("window exists")
                .request_redraw();
        }
    }
    fn redraw(&mut self) {
        let Some(metrics) = self.metrics else {
            return;
        };
        if metrics.physical_size.is_zero() || !self.window_can_present() {
            return;
        }
        let result = self
            .runtime
            .as_mut()
            .expect("runtime exists")
            .run_frame(Constraints::tight(metrics.logical_size()));
        match result {
            Ok((list, _)) => match self
                .renderer
                .as_mut()
                .expect("renderer exists")
                .render(&list, metrics.scale_factor)
            {
                Ok(stats) if !stats.presented => self
                    .window
                    .as_ref()
                    .expect("window exists")
                    .request_redraw(),
                Ok(_) => {}
                Err(RendererError::OutOfMemory) => {
                    eprintln!("Incular renderer stopped: out of GPU memory")
                }
                Err(error) => eprintln!("Incular renderer error: {error}"),
            },
            Err(error) => eprintln!("Incular runtime error: {error:?}"),
        }
        if let (Some(runtime), Some(accessibility)) =
            (self.runtime.as_ref(), self.accessibility.as_mut())
            && accessibility.active
            && let Some(update) = accessibility
                .projection
                .sync(runtime.tree().semantics(), metrics.scale_factor)
        {
            accessibility
                .adapter
                .update_if_active(|| update.into_accesskit());
        }
        self.apply_text_input_commands();
        self.sync_cursor();
    }

    fn apply_text_input_commands(&mut self) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        let commands = self
            .runtime
            .as_mut()
            .map(Runtime::take_text_input_commands)
            .unwrap_or_default();
        for command in commands {
            apply_text_input_command(window, &command);
        }
    }

    fn handle_accesskit_event(&mut self, event: AccessKitEvent) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        if window.id() != event.window_id {
            return;
        }
        let Some(accessibility) = self.accessibility.as_mut() else {
            return;
        };
        match event.window_event {
            AccessKitWindowEvent::InitialTreeRequested => {
                accessibility.active = true;
                accessibility.projection.activate();
                window.request_redraw();
            }
            AccessKitWindowEvent::ActionRequested(request) => {
                if let Some(request) = accessibility.projection.translate_action(&request) {
                    let handled = self.runtime.as_mut().is_some_and(|runtime| {
                        runtime.dispatch_semantic_action(request.node, request.action)
                    });
                    if handled {
                        window.request_redraw();
                    } else {
                        accessibility.projection.note_runtime_stale_action();
                    }
                }
            }
            AccessKitWindowEvent::AccessibilityDeactivated => {
                accessibility.active = false;
                accessibility.projection.deactivate();
            }
        }
    }
}

struct NativeWindowState {
    id: IncularWindowId,
    renderer: WgpuRenderer,
    metrics: WindowMetrics,
    cursor: PhysicalPosition<f64>,
    mouse_buttons: MouseButtonState,
    native_cursor: NativeCursorCoordinator,
    environment: DesktopEnvironmentProvider,
    modifiers: winit::keyboard::ModifiersState,
    external_file_drag: ExternalFileDragState,
    accessibility: NativeAccessibilityState,
    content_sensitivity: NoopContentSensitivityBackend,
    /// Must outlive every field that was created from this window's raw
    /// handles. Struct fields drop in declaration order, so keep it last.
    window: Window,
}

/// Winit 0.30 adapter for an [`Application`] with many retained roots. Native
/// IDs stay in these two maps and are never exposed through Incular APIs.
struct MultiApp {
    application: Application,
    windows: HashMap<NativeWindowId, NativeWindowState>,
    native_ids: HashMap<IncularWindowId, NativeWindowId>,
    transient_windows: HashMap<NativeWindowId, NativeTransientState>,
    transient_native_ids: HashMap<TransientHostKey, NativeWindowId>,
    transient_native_rejections: HashMap<TransientHostKey, TransientNativeRejection>,
    shared_gpu: Option<SharedGpuContext>,
    frame_timestamp: Instant,
    event_proxy: winit::event_loop::EventLoopProxy<RuntimeWakeEvent>,
    displays: DesktopDisplayRegistry,
    pointer_devices: PointerDeviceRegistry,
    platform_services: Box<dyn DesktopPlatformServices>,
    platform_menu_delegate: Rc<dyn PlatformMenuDelegate>,
    window_system: Option<NativeWindowSystem>,
    global_shortcuts: Option<DesktopGlobalShortcuts>,
    single_instance: Option<SingleInstancePrimary>,
    _global_shortcut_event_sink: GlobalShortcutEventSinkGuard,
    /// Native windows whose platform service requires a post-drop
    /// `WindowEvent::Destroyed` acknowledgement. Empty on synchronous-drop
    /// backends. Keeping this portable leaves the lifecycle quirk in the OS
    /// service rather than scattering target conditionals through the shell.
    pending_native_destructions: HashSet<NativeWindowId>,
    /// Debug overlays, Select Widget mode, and the DevTools agent pump.
    #[cfg(feature = "devtools")]
    devtools_state: crate::devtools_runner::DevToolsState,
}

impl MultiApp {
    fn publish_window_environment(&mut self, native_id: NativeWindowId) {
        let Some((id, environment)) = self
            .windows
            .get(&native_id)
            .map(|state| (state.id, state.environment.snapshot(state.metrics)))
        else {
            return;
        };
        self.application
            .handle_window_event(IncularWindowEvent::platform(
                id,
                PlatformEvent::Environment(environment),
            ));
    }

    fn refresh_system_environment(&mut self, force: bool) {
        if !force && !self.platform_services.take_system_environment_change() {
            return;
        }
        let preferences = self.platform_services.system_environment_preferences();
        let changed = self
            .windows
            .iter_mut()
            .filter_map(|(native_id, state)| {
                state
                    .environment
                    .replace_preferences(preferences.clone())
                    .then_some(*native_id)
            })
            .collect::<Vec<_>>();
        for native_id in changed {
            self.publish_window_environment(native_id);
        }
    }

    fn drain_application_lifecycle(&mut self) {
        for lifecycle in self.platform_services.take_application_lifecycle_events() {
            self.application.handle_application_lifecycle(lifecycle);
        }
    }

    fn drain_single_instance_activations(&mut self) {
        let Some(primary) = self.single_instance.as_ref() else {
            return;
        };
        for activation in primary.take_activations() {
            self.application.handle_application_activation(activation);
        }
    }

    fn process_pending_global_shortcuts(&mut self) {
        let Some(shortcuts) = self.global_shortcuts.as_mut() else {
            return;
        };
        for request in self.application.take_native_global_shortcut_requests() {
            let result = match request.operation {
                NativeGlobalShortcutOperation::Register { id, chord } => {
                    shortcuts.register(id, chord)
                }
                NativeGlobalShortcutOperation::Unregister { id } => shortcuts.unregister(id),
            };
            let status =
                self.application
                    .complete_global_shortcut_request(NativeGlobalShortcutCompletion {
                        request_id: request.request_id,
                        result,
                    });
            if let GlobalShortcutCompletionStatus::AbandonedRegistration(id) = status {
                let _ = shortcuts.unregister(id);
            }
        }
    }

    fn handle_global_shortcut_event(&mut self, event: NativeGlobalShortcutEvent) {
        let Some(id) = self
            .global_shortcuts
            .as_ref()
            .and_then(|shortcuts| shortcuts.stable_id(event.native_id))
        else {
            return;
        };
        self.application
            .handle_application_activation(ApplicationActivation::GlobalShortcutInvoked(id));
    }

    fn note_window_mouse(&mut self, native_id: NativeWindowId) {
        if self
            .windows
            .get_mut(&native_id)
            .is_some_and(|state| state.environment.note_mouse())
        {
            self.publish_window_environment(native_id);
        }
    }

    fn note_window_touch(&mut self, native_id: NativeWindowId) {
        if self
            .windows
            .get_mut(&native_id)
            .is_some_and(|state| state.environment.note_touch())
        {
            self.publish_window_environment(native_id);
        }
    }

    fn note_window_keyboard(&mut self, native_id: NativeWindowId) {
        if self
            .windows
            .get_mut(&native_id)
            .is_some_and(|state| state.environment.note_keyboard())
        {
            self.publish_window_environment(native_id);
        }
    }

    fn start_pending_file_dialogs(&mut self) {
        let requests = self.application.take_native_file_dialog_requests();
        if requests.is_empty() {
            return;
        }
        let tokio = self.application.tokio_handle();
        for request in requests {
            let parent = self
                .native_ids
                .get(&request.window_id)
                .and_then(|native_id| self.windows.get(native_id))
                .map(|state| &state.window);
            if let Some(parent) = parent {
                crate::file_dialogs::start_native_file_dialog(
                    request,
                    parent,
                    tokio.clone(),
                    self.event_proxy.clone(),
                );
            } else {
                let _ = self.application.complete_file_dialog(
                    incular_runtime::NativeFileDialogCompletion {
                        request_id: request.request_id,
                        window_id: request.window_id,
                        result: Err(incular_platform::FileDialogError::ParentClosed),
                    },
                );
            }
        }
    }

    fn native_external_drag_position(&self, native_id: NativeWindowId) -> Option<Offset> {
        let system = self.window_system?;
        let state = self.windows.get(&native_id)?;
        self.platform_services
            .external_drag_position(system, &state.window)
            .map(|position| logical_cursor_position(position, state.metrics))
    }

    fn flush_external_file_drops(&mut self) {
        let drops = self
            .windows
            .values_mut()
            .filter_map(|state| {
                state
                    .external_file_drag
                    .take_drop()
                    .map(|event| (state.id, event))
            })
            .collect::<Vec<_>>();
        for (id, event) in drops {
            let _ = self.application.handle_external_drag_event(id, event);
        }
    }

    fn sync_platform_menus(&self) {
        for id in self.application.active_window_ids() {
            for binding in self.application.platform_menu_bindings(id) {
                let _ = binding.connect(self.platform_menu_delegate.clone());
            }
        }
    }

    fn refresh_displays(&mut self, target: &ActiveEventLoop) {
        let Some(system) = self.window_system else {
            return;
        };
        let sync = self.displays.synchronize(
            target.available_monitors().collect(),
            target.primary_monitor(),
            system,
            self.platform_services.as_ref(),
        );
        if !sync.changed {
            return;
        }
        self.application
            .publish_displays(sync.snapshots, sync.primary);
        // Current-monitor association can change when an output is removed even
        // if no window movement event is delivered for the surviving windows.
        let ids = self.application.active_window_ids();
        for id in ids {
            self.publish_window_state(id);
        }
    }

    fn apply_window_commands(&mut self, target: &ActiveEventLoop) {
        // Simulation requests arrive through the same event-loop wake as
        // runtime work. They are serviced before native operations so a
        // simulated callback can enqueue a title/visibility/redraw command in
        // this same turn.
        self.application.process_simulation_requests();
        for command in self.application.take_native_window_commands() {
            match command {
                NativeWindowCommand::Create { window_id, options } => {
                    self.create_window(target, window_id, options);
                }
                NativeWindowCommand::Operate(command) => self.operate_window(command),
            }
        }
        if self.application.should_exit() {
            if !self.pending_native_destructions.is_empty() {
                return;
            }
            target.exit();
        }
    }

    fn track_native_window_drop(&mut self, native_id: NativeWindowId) {
        let Some(system) = self.window_system else {
            return;
        };
        if self
            .platform_services
            .wait_for_destroyed_event_after_window_drop(system)
        {
            self.pending_native_destructions.insert(native_id);
        }
    }

    fn create_window(
        &mut self,
        target: &ActiveEventLoop,
        id: IncularWindowId,
        options: WindowOptions,
    ) {
        if self.native_ids.contains_key(&id) || !self.application.contains_window(id) {
            return;
        }
        let attributes = match window_attributes(&options) {
            Ok(attributes) => attributes.with_visible(false),
            Err(error) => {
                let _ = self.application.record_platform_operation_error(id, error);
                let _ = self.application.close_window(id);
                return;
            }
        };
        let window = match target.create_window(attributes) {
            Ok(window) => window,
            Err(error) => {
                eprintln!("Incular window error: {error}");
                let _ = self.application.close_window(id);
                return;
            }
        };
        let system = native_window_system(&window);
        if self.window_system.is_none() {
            self.window_system = Some(system);
            let global_shortcuts = DesktopGlobalShortcuts::new(system);
            let mut capabilities = refine_window_capabilities(
                desktop_platform_capabilities(),
                system,
                self.platform_services.as_ref(),
            );
            capabilities.application_services.global_shortcuts = global_shortcuts.support();
            self.application.set_platform_capabilities(capabilities);
            self.global_shortcuts = Some(global_shortcuts);
        }
        self.refresh_displays(target);
        let metrics = WindowMetrics::new(
            PhysicalSize::new(window.inner_size().width, window.inner_size().height),
            window.scale_factor(),
        );
        let handles = raw_window_handles(&window);
        let renderer = match self.shared_gpu.clone() {
            Some(shared) => pollster::block_on(shared.create_renderer(
                handles,
                metrics.physical_size,
                options.transparency_mode,
                options.background_color,
            )),
            None => {
                match pollster::block_on(SharedGpuContext::new(handles, options.transparency_mode))
                {
                    Ok(shared) => {
                        let renderer = pollster::block_on(shared.create_renderer(
                            handles,
                            metrics.physical_size,
                            options.transparency_mode,
                            options.background_color,
                        ));
                        if renderer.is_ok() {
                            self.shared_gpu = Some(shared);
                        }
                        renderer
                    }
                    Err(error) => Err(error),
                }
            }
        };
        let renderer = match renderer {
            Ok(renderer) => renderer,
            Err(error) => {
                eprintln!("Incular renderer initialization failed: {error}");
                self.track_native_window_drop(window.id());
                let _ = self.application.close_window(id);
                return;
            }
        };
        let mut accessibility = NativeAccessibilityState {
            adapter: AccessKitAdapter::with_event_loop_proxy(
                target,
                &window,
                self.event_proxy.clone(),
            ),
            projection: AccessKitProjection::new(),
            active: false,
        };
        accessibility.projection.note_adapter_created();
        let clipboard = DesktopClipboard::new();
        let clipboard_capabilities = clipboard.native_capabilities();
        let mut capabilities = refine_window_capabilities(
            desktop_platform_capabilities(),
            system,
            self.platform_services.as_ref(),
        );
        capabilities.data_transfer.clipboard_text = clipboard_format_bidirectional_support(
            clipboard_capabilities,
            &incular_platform::TransferFormat::PlainText,
        );
        capabilities.data_transfer.clipboard_rich = if [
            incular_platform::TransferFormat::Html,
            incular_platform::TransferFormat::Files,
            incular_platform::TransferFormat::Rgba8Image,
        ]
        .iter()
        .all(|format| {
            clipboard_format_bidirectional_support(clipboard_capabilities, format)
                == CapabilitySupport::Supported
        }) {
            CapabilitySupport::Supported
        } else {
            CapabilitySupport::Unsupported
        };
        capabilities.data_transfer.clipboard_custom = CapabilitySupport::Unsupported;
        let _ = self.application.set_window_capabilities(id, capabilities);
        if let Some(error) = unsupported_initial_window_policy(&options, &capabilities) {
            let _ = self.application.record_platform_operation_error(id, error);
        }
        self.application
            .set_window_clipboard(id, Box::new(clipboard));
        self.application
            .handle_window_event(IncularWindowEvent::platform(
                id,
                PlatformEvent::Metrics(metrics),
            ));
        self.application
            .handle_window_event(IncularWindowEvent::lifecycle(
                id,
                if options.visible {
                    WindowLifecycle::Visible
                } else {
                    WindowLifecycle::Hidden
                },
            ));
        // The retained roots already exist before the native runner starts.
        // Bind their deferred menu bridges before registering this window so a
        // backend can attach the current application menu before first show.
        self.sync_platform_menus();
        if let Err(error) = self
            .platform_services
            .register_platform_menu_window(system, &window)
        {
            let _ = self.application.record_platform_operation_error(id, error);
        }
        let native_id = window.id();
        let environment = DesktopEnvironmentProvider::new(
            &window,
            self.platform_services.system_environment_preferences(),
        );
        self.application
            .handle_window_event(IncularWindowEvent::platform(
                id,
                PlatformEvent::Environment(environment.snapshot(metrics)),
            ));
        window.set_visible(options.visible);
        let observed_state = observed_window_state(&window, &self.displays, system);
        self.native_ids.insert(id, native_id);
        self.windows.insert(
            native_id,
            NativeWindowState {
                id,
                window,
                renderer,
                metrics,
                cursor: PhysicalPosition::new(0., 0.),
                mouse_buttons: MouseButtonState::default(),
                native_cursor: NativeCursorCoordinator::default(),
                environment,
                modifiers: winit::keyboard::ModifiersState::default(),
                external_file_drag: ExternalFileDragState::default(),
                accessibility,
                content_sensitivity: NoopContentSensitivityBackend,
            },
        );
        self.process_pending_global_shortcuts();
        self.application
            .handle_window_event(IncularWindowEvent::state_changed(id, observed_state));
        self.request_frame_if_needed(id);
    }

    fn operate_window(&mut self, command: WindowCommand) {
        let window_id = command.window_id;
        let request_id = command.request_id;
        let Some(native_id) = self.native_ids.get(&command.window_id).copied() else {
            if let Some(request_id) = request_id {
                let error = if self.application.contains_window(window_id) {
                    PlatformOperationError::unavailable()
                } else {
                    PlatformOperationError::stale_resource()
                };
                let _ = self
                    .application
                    .complete_native_operation(NativeOperationCompletion::new(
                        window_id,
                        request_id,
                        Err(error),
                    ));
            }
            return;
        };
        if matches!(command.operation, WindowOperation::Close) {
            let external_position = self.native_external_drag_position(native_id).or_else(|| {
                self.windows
                    .get(&native_id)
                    .and_then(|state| state.external_file_drag.current_position())
            });
            let external_cancel = external_position.and_then(|position| {
                self.windows
                    .get_mut(&native_id)
                    .and_then(|state| state.external_file_drag.cancel(position))
            });
            if let Some(event) = external_cancel {
                let _ = self
                    .application
                    .handle_external_drag_event(command.window_id, event);
            }
            self.destroy_transient_hosts_for_owner(command.window_id);
            self.native_ids.remove(&command.window_id);
            if let Some(mut state) = self.windows.remove(&native_id) {
                if let Some(system) = self.window_system {
                    self.platform_services
                        .unregister_platform_menu_window(system, &state.window);
                }
                self.track_native_window_drop(native_id);
                state.accessibility.projection.note_adapter_destroyed();
            }
            return;
        }
        let publishes_state = matches!(
            command.operation,
            WindowOperation::SetVisible(_)
                | WindowOperation::SetOuterPosition(_)
                | WindowOperation::SetMinimized(_)
                | WindowOperation::SetMaximized(_)
                | WindowOperation::SetFullscreen(_)
                | WindowOperation::SetResizable(_)
                | WindowOperation::SetDecorations(_)
        );
        let mut synchronous_resize = None;
        let mut operation_result = Ok(());
        {
            let Some(state) = self.windows.get_mut(&native_id) else {
                if let Some(request_id) = request_id {
                    let _ =
                        self.application
                            .complete_native_operation(NativeOperationCompletion::new(
                                window_id,
                                request_id,
                                Err(PlatformOperationError::unavailable()),
                            ));
                }
                return;
            };
            let capabilities = self
                .application
                .window_capabilities(window_id)
                .unwrap_or_default();
            if operation_support(&capabilities, &command.operation)
                == CapabilitySupport::Unsupported
            {
                operation_result = Err(PlatformOperationError::unsupported());
            } else {
                match command.operation {
                    WindowOperation::SetTitle(title) => state.window.set_title(&title),
                    WindowOperation::SetVisible(visible) => state.window.set_visible(visible),
                    WindowOperation::SetLogicalSize(size) => {
                        synchronous_resize = state
                            .window
                            .request_inner_size(winit::dpi::LogicalSize::new(
                                size.width,
                                size.height,
                            ))
                            .map(|physical| {
                                (
                                    PhysicalSize::new(physical.width, physical.height),
                                    state.window.scale_factor(),
                                )
                            });
                    }
                    WindowOperation::SetOuterPosition(position) => {
                        state
                            .window
                            .set_outer_position(PhysicalPosition::new(position.x, position.y));
                    }
                    WindowOperation::BeginMoveDrag => {
                        operation_result = state.window.drag_window().map_err(map_external_error);
                    }
                    WindowOperation::BeginResizeDrag(direction) => {
                        operation_result = state
                            .window
                            .drag_resize_window(native_resize_direction(direction))
                            .map_err(map_external_error);
                    }
                    WindowOperation::SetMinimized(minimized) => {
                        state.window.set_minimized(minimized);
                    }
                    WindowOperation::SetMaximized(maximized) => {
                        state.window.set_maximized(maximized);
                    }
                    WindowOperation::SetFullscreen(fullscreen) => {
                        state.window.set_fullscreen(
                            fullscreen.map(|_| winit::window::Fullscreen::Borderless(None)),
                        );
                    }
                    WindowOperation::SetResizable(resizable) => {
                        state.window.set_resizable(resizable);
                    }
                    WindowOperation::SetDecorations(decorations) => {
                        state.window.set_decorations(decorations);
                    }
                    WindowOperation::SetLogicalSizeLimits(limits) => {
                        state
                            .window
                            .set_min_inner_size(limits.minimum().map(|size| {
                                winit::dpi::LogicalSize::new(
                                    f64::from(size.width),
                                    f64::from(size.height),
                                )
                            }));
                        state
                            .window
                            .set_max_inner_size(limits.maximum().map(|size| {
                                winit::dpi::LogicalSize::new(
                                    f64::from(size.width),
                                    f64::from(size.height),
                                )
                            }));
                    }
                    WindowOperation::SetWindowLevel(level) => {
                        state.window.set_window_level(native_window_level(level));
                    }
                    WindowOperation::SetWindowIcon(icon) => {
                        match icon.as_ref().map(native_window_icon).transpose() {
                            Ok(icon) => state.window.set_window_icon(icon),
                            Err(error) => operation_result = Err(error),
                        }
                    }
                    WindowOperation::RequestUserAttention(attention) => {
                        state
                            .window
                            .request_user_attention(attention.map(native_attention_type));
                    }
                    WindowOperation::SetCursorGrab(mode) => {
                        operation_result = state
                            .window
                            .set_cursor_grab(native_cursor_grab_mode(mode))
                            .map_err(map_external_error);
                    }
                    WindowOperation::SetCursorVisible(visible) => {
                        state.window.set_cursor_visible(visible);
                    }
                    WindowOperation::SetCursorPosition(position) => {
                        operation_result = state
                            .window
                            .set_cursor_position(LogicalPosition::new(position.x(), position.y()))
                            .map_err(map_external_error);
                    }
                    WindowOperation::SetContentSensitivity(sensitivity) => {
                        operation_result = match state.content_sensitivity.apply(sensitivity) {
                            ContentSensitivityOutcome::Applied { .. }
                            | ContentSensitivityOutcome::NoOp {
                                reason: ContentSensitivityNoOpReason::Unchanged,
                                ..
                            } => Ok(()),
                            ContentSensitivityOutcome::NoOp {
                                reason: ContentSensitivityNoOpReason::Unsupported,
                                ..
                            } => Err(PlatformOperationError::unsupported()),
                        };
                    }
                    WindowOperation::RequestFocus => state.window.focus_window(),
                    WindowOperation::RequestRedraw => state.window.request_redraw(),
                    WindowOperation::Close => unreachable!(),
                }
            }
        }
        // Some native backends, notably Wayland, can acknowledge a requested
        // inner size synchronously and are then not required to emit a later
        // `WindowEvent::Resized`. Treat that acknowledgment exactly like the
        // native event so metrics, WGPU, runtime viewport state, and capture
        // dimensions advance atomically while preserving this window's native
        // identity and compositor placement. Backends returning `None` remain
        // asynchronous and are handled by the ordinary `Resized` event path.
        if let Some((physical_size, scale_factor)) = synchronous_resize {
            self.resize_window(command.window_id, physical_size, scale_factor);
        }
        if publishes_state {
            self.publish_window_state(window_id);
        }
        if let Some(request_id) = request_id {
            let _ = self
                .application
                .complete_native_operation(NativeOperationCompletion::new(
                    window_id,
                    request_id,
                    operation_result,
                ));
        } else if let Err(error) = operation_result {
            let _ = self
                .application
                .record_platform_operation_error(window_id, error);
        }
    }

    fn publish_window_state(&mut self, id: IncularWindowId) {
        let Some(native_id) = self.native_ids.get(&id).copied() else {
            return;
        };
        let Some(state) = self.windows.get(&native_id) else {
            return;
        };
        let Some(system) = self.window_system else {
            return;
        };
        let observed = observed_window_state(&state.window, &self.displays, system);
        self.application
            .handle_window_event(IncularWindowEvent::state_changed(id, observed));
        self.publish_native_transient_bounds(id);
    }

    fn request_frame_if_needed(&mut self, id: IncularWindowId) {
        self.apply_text_input_commands(id);
        if self.application.frame_requested(id)
            && let Some(native_id) = self.native_ids.get(&id).copied()
            && let Some(state) = self.windows.get(&native_id)
            && !state.environment.is_occluded()
            && state
                .window
                .is_minimized()
                .is_none_or(|minimized| !minimized)
        {
            state.window.request_redraw();
            self.application.note_frame_requested(id);
        }
    }

    fn resize_window(&mut self, id: IncularWindowId, size: PhysicalSize, scale_factor: f64) {
        let Some(native_id) = self.native_ids.get(&id).copied() else {
            return;
        };
        let Some(state) = self.windows.get_mut(&native_id) else {
            return;
        };
        let metrics = WindowMetrics::new(size, scale_factor);
        let metrics_changed = state.metrics != metrics;
        let surface_size_changed = state.renderer.physical_size() != size;
        if !metrics_changed && !surface_size_changed {
            return;
        }
        state.metrics = metrics;
        if surface_size_changed {
            state.renderer.resize(size);
        }
        if metrics_changed {
            self.application
                .handle_window_event(IncularWindowEvent::platform(
                    id,
                    PlatformEvent::Metrics(metrics),
                ));
        }
        if !size.is_zero() {
            state.window.request_redraw();
            self.application.note_frame_requested(id);
        }
    }

    fn redraw_window(&mut self, target: &ActiveEventLoop, id: IncularWindowId) {
        #[cfg(feature = "devtools")]
        self.devtools_state.drain(&mut self.application);
        let Some(native_id) = self.native_ids.get(&id).copied() else {
            return;
        };
        if self.windows.get(&native_id).is_some_and(|state| {
            state.environment.is_occluded()
                || state
                    .window
                    .is_minimized()
                    .is_some_and(|minimized| minimized)
        }) {
            // Preserve Runtime::frame_requested while native presentation is
            // suppressed. The first unoccluded/restored event-loop turn will
            // request one frame containing every retained invalidation queued
            // while the window could not contribute visible pixels.
            return;
        }
        let Some(metrics) = self.windows.get(&native_id).map(|state| state.metrics) else {
            return;
        };
        if self.application.simulation_capture_pending(id)
            && let Some(state) = self.windows.get_mut(&native_id)
        {
            state.renderer.request_capture();
        }
        self.publish_native_transient_bounds(id);
        #[cfg(feature = "devtools")]
        self.devtools_state
            .begin_deep_frame(&mut self.application, id);
        let result = self.application.run_window_frame_at(
            id,
            Constraints::tight(metrics.logical_size()),
            self.frame_timestamp,
        );
        match result {
            Ok(Some((list, frame))) => {
                #[cfg(feature = "devtools")]
                let mut list = list;
                #[cfg(feature = "devtools")]
                let devtools_frame = frame;
                #[cfg(not(feature = "devtools"))]
                let _ = frame;
                #[cfg(feature = "devtools")]
                self.devtools_state.paint_overlay(id, &mut list);

                let snapshots = self.application.transient_surfaces(id);
                let selected = self.synchronize_transient_hosts(target, id, &snapshots);
                let (_, detached) = list.detach_surface_partitions(&selected);
                let mut failed = Vec::new();
                let mut ready = HashSet::new();
                for snapshot in snapshots.iter().copied() {
                    let partition = snapshot.id.surface_partition();
                    if !selected.contains(&partition) {
                        continue;
                    }
                    let Some(transient_list) = detached.get(&partition) else {
                        failed.push((snapshot, TransientFallbackReason::NativeHostUnavailable));
                        continue;
                    };
                    let key = TransientHostKey {
                        owner: id,
                        transient: snapshot.id,
                    };
                    match self.render_transient_partition(key, transient_list) {
                        Ok(true) => {
                            ready.insert(partition);
                        }
                        Ok(false) => {}
                        Err(reason) => failed.push((snapshot, reason)),
                    }
                }
                for (snapshot, reason) in &failed {
                    let key = TransientHostKey {
                        owner: id,
                        transient: snapshot.id,
                    };
                    self.destroy_transient_host(key);
                    self.transient_native_rejections.insert(
                        key,
                        TransientNativeRejection {
                            role: snapshot.role,
                            reason: *reason,
                        },
                    );
                }
                self.publish_transient_presentations(id, &snapshots);
                let (parent_list, _) = list.detach_surface_partitions(&ready);

                let Some(state) = self.windows.get_mut(&native_id) else {
                    return;
                };
                match state.renderer.render(&parent_list, metrics.scale_factor) {
                    Ok(stats) => {
                        self.application.note_presented(id, stats.presented);
                        let gpu = state.renderer.gpu_frame_timings().map(|timing| GpuSample {
                            supported: true,
                            frame: timing.frame,
                            main_pass_us: timing.main_pass_us,
                        });
                        self.application.note_render_metrics(
                            id,
                            runtime_render_metrics(&stats),
                            gpu,
                        );
                        let capture = state.renderer.take_capture().map(|result| {
                            result.and_then(|frame| {
                                Screenshot::from_rgba8(frame.width, frame.height, frame.rgba8)
                                    .map_err(|error| error.to_string())
                            })
                        });
                        self.application
                            .complete_simulation_frame(id, stats.presented, capture);
                        #[cfg(feature = "devtools")]
                        {
                            let frame = &devtools_frame;
                            let budget_us = state
                                .window
                                .current_monitor()
                                .and_then(|monitor| monitor.refresh_rate_millihertz())
                                .filter(|rate| *rate > 0)
                                .map(|rate| {
                                    u32::try_from(1_000_000_000_u64 / u64::from(rate))
                                        .unwrap_or(u32::MAX)
                                });
                            let cpu_total = frame
                                .timings
                                .cpu_total
                                .saturating_add(stats.prepare_us)
                                .saturating_add(stats.encode_us)
                                .saturating_add(stats.submit_us);
                            let frame_id = self.devtools_state.next_frame();
                            self.devtools_state.push_frame(
                                incular_devtools_protocol::TargetEvent::FrameRecord(
                                    incular_devtools_protocol::FrameRecordEvent {
                                        window: incular_devtools_protocol::DevWindowId::new(
                                            u64::from(id.index()) + 1,
                                            u64::from(id.generation()),
                                        ),
                                        frame: frame_id,
                                        timings: incular_devtools_protocol::FrameTimingsWire {
                                            event_processing: frame.timings.event_processing,
                                            runtime_messages: frame.timings.runtime_messages,
                                            build: frame.timings.build,
                                            layout: frame.timings.layout,
                                            composite: frame.timings.composite,
                                            semantics: frame.timings.semantics,
                                            paint: frame.timings.paint,
                                            cpu_total,
                                            prepare: stats.prepare_us,
                                            encode: stats.encode_us,
                                            submit: stats.submit_us,
                                            gpu_us: state
                                                .renderer
                                                .gpu_frame_timings()
                                                .map(|timing| timing.main_pass_us),
                                        },
                                        budget_us,
                                        over_budget: budget_us
                                            .is_some_and(|budget| cpu_total > budget),
                                        draw_calls: stats.draw_calls,
                                        instances: stats.total_instances(),
                                        upload_bytes: stats.upload_bytes,
                                        pipelines_created: stats.pipelines_created,
                                    },
                                ),
                            );
                            if self.devtools_state.note_recorded_frame()
                                && self.devtools_state.deep_recording()
                                && let Some(trace) =
                                    self.application.devtools_take_deep_trace(id, frame_id)
                            {
                                self.devtools_state.push_frame(
                                    incular_devtools_protocol::TargetEvent::DeepTrace(trace),
                                );
                            }
                        }
                        if !stats.presented {
                            state.window.request_redraw();
                        }
                    }
                    Err(RendererError::OutOfMemory) => {
                        eprintln!("Incular renderer stopped: out of GPU memory");
                        self.application
                            .fail_simulation_frame(id, "renderer stopped: out of GPU memory");
                        #[cfg(feature = "devtools")]
                        self.devtools_state.push_frame(
                            incular_devtools_protocol::TargetEvent::Log {
                                level: "error".into(),
                                target: "incular::renderer".into(),
                                message: "renderer stopped: out of GPU memory".into(),
                            },
                        );
                    }
                    Err(error) => {
                        eprintln!("Incular renderer error: {error}");
                        self.application
                            .fail_simulation_frame(id, error.to_string());
                        #[cfg(feature = "devtools")]
                        self.devtools_state.push_frame(
                            incular_devtools_protocol::TargetEvent::Log {
                                level: "error".into(),
                                target: "incular::renderer".into(),
                                message: error.to_string(),
                            },
                        );
                    }
                }
            }
            Ok(None) => self.application.note_presented(id, false),
            Err(error) => {
                eprintln!("Incular runtime error: {error:?}");
                self.application
                    .fail_simulation_frame(id, format!("{error:?}"));
                #[cfg(feature = "devtools")]
                self.devtools_state
                    .push_frame(incular_devtools_protocol::TargetEvent::Log {
                        level: "error".into(),
                        target: "incular::runtime".into(),
                        message: format!("{error:?}"),
                    });
            }
        }
        let Some(state) = self.windows.get_mut(&native_id) else {
            return;
        };
        if state.accessibility.active
            && let Some(update) = self
                .application
                .sync_accessibility(id, &mut state.accessibility.projection)
        {
            state
                .accessibility
                .adapter
                .update_if_active(|| update.into_accesskit());
        }
        self.application
            .set_accessibility_diagnostics(id, state.accessibility.projection.diagnostics());
        if let Some(cursor) = self.application.window_mouse_cursor(id)
            && let Some(native) = state.native_cursor.update(cursor)
        {
            state.window.set_cursor(native);
        }
        self.apply_text_input_commands(id);
        #[cfg(feature = "devtools")]
        self.devtools_state.stream_tree_updates(&self.application);
    }

    fn apply_text_input_commands(&mut self, id: IncularWindowId) {
        let Some(native_id) = self.native_ids.get(&id).copied() else {
            return;
        };
        let commands = self.application.take_window_text_input_commands(id);
        let Some(state) = self.windows.get(&native_id) else {
            return;
        };
        for command in commands {
            apply_text_input_command(&state.window, &command);
        }
    }

    fn route_window_event(&mut self, native_id: NativeWindowId, event: IncularWindowEvent) {
        if self.windows.contains_key(&native_id) {
            self.application.handle_window_event(event);
        }
    }

    fn sync_window_cursor(&mut self, id: IncularWindowId) {
        let Some(cursor) = self.application.window_mouse_cursor(id) else {
            return;
        };
        let Some(native_id) = self.native_ids.get(&id).copied() else {
            return;
        };
        let Some(state) = self.windows.get_mut(&native_id) else {
            return;
        };
        if let Some(native) = state.native_cursor.update(cursor) {
            state.window.set_cursor(native);
        }
    }

    fn cancel_window_mouse_pointer(&mut self, native_id: NativeWindowId, id: IncularWindowId) {
        let (had_pressed_buttons, cursor, metrics) = {
            let Some(state) = self.windows.get_mut(&native_id) else {
                return;
            };
            (state.mouse_buttons.cancel(), state.cursor, state.metrics)
        };
        if had_pressed_buttons {
            let cancel = pointer_event_with_metadata(
                mouse_pointer_metadata(0, 0, None, PointerPhase::Cancel),
                cursor,
                metrics,
            );
            self.route_window_event(native_id, IncularWindowEvent::platform(id, cancel));
        }
        let exit = pointer_event_with_metadata(
            mouse_pointer_metadata(0, 0, None, PointerPhase::Exit),
            cursor,
            metrics,
        );
        self.route_window_event(native_id, IncularWindowEvent::platform(id, exit));
        self.sync_window_cursor(id);
    }

    fn route_accesskit_event(&mut self, event: AccessKitEvent) {
        let Some(state) = self.windows.get_mut(&event.window_id) else {
            return;
        };
        let id = state.id;
        match event.window_event {
            AccessKitWindowEvent::InitialTreeRequested => {
                state.accessibility.active = true;
                state.accessibility.projection.activate();
                state.window.request_redraw();
            }
            AccessKitWindowEvent::ActionRequested(request) => {
                if let Some(request) = state.accessibility.projection.translate_action(&request) {
                    if self.application.dispatch_accessibility_action(id, request) {
                        state.window.request_redraw();
                    } else {
                        state.accessibility.projection.note_runtime_stale_action();
                    }
                }
            }
            AccessKitWindowEvent::AccessibilityDeactivated => {
                state.accessibility.active = false;
                state.accessibility.projection.deactivate();
            }
        }
        self.application
            .set_accessibility_diagnostics(id, state.accessibility.projection.diagnostics());
    }
}

fn desktop_platform_capabilities() -> PlatformCapabilities {
    let mut capabilities = PlatformCapabilities::unsupported();
    capabilities.window.set_title = CapabilitySupport::Supported;
    capabilities.window.set_visibility = CapabilitySupport::Supported;
    capabilities.window.set_logical_size = CapabilitySupport::Supported;
    capabilities.window.begin_move_drag = CapabilitySupport::Supported;
    capabilities.window.begin_resize_drag = if cfg!(target_os = "macos") {
        CapabilitySupport::Unsupported
    } else {
        CapabilitySupport::Supported
    };
    capabilities.window.minimize = CapabilitySupport::Supported;
    capabilities.window.maximize = CapabilitySupport::Supported;
    capabilities.window.fullscreen = CapabilitySupport::Supported;
    capabilities.window.set_resizable = CapabilitySupport::Supported;
    capabilities.window.set_decorations = CapabilitySupport::Supported;
    capabilities.window.set_size_limits = CapabilitySupport::Supported;
    capabilities.window.set_window_level = if cfg!(target_os = "linux") {
        CapabilitySupport::Unknown
    } else {
        CapabilitySupport::Supported
    };
    capabilities.window.set_window_icon = if cfg!(target_os = "windows") {
        CapabilitySupport::Supported
    } else if cfg!(target_os = "macos") {
        CapabilitySupport::Unsupported
    } else {
        CapabilitySupport::Unknown
    };
    capabilities.window.request_user_attention = if cfg!(target_os = "linux") {
        CapabilitySupport::Unknown
    } else {
        CapabilitySupport::Supported
    };
    capabilities.window.request_focus = CapabilitySupport::Supported;
    capabilities.window.request_redraw = CapabilitySupport::Supported;
    capabilities.window.close = CapabilitySupport::Supported;
    // The current shared desktop adapter deliberately uses the explicit no-op
    // sensitivity backend. OS-specific enforcement can publish Supported when
    // a real adapter is installed.
    capabilities.window.content_sensitivity = CapabilitySupport::Unsupported;
    capabilities.display.enumerate_displays = CapabilitySupport::Supported;
    capabilities.display.query_current_display = CapabilitySupport::Supported;
    // These depend on the concrete window system and are refined after the
    // first native window reveals whether this is Win32/AppKit/X11/Wayland.
    capabilities.display.display_bounds = CapabilitySupport::Unknown;
    capabilities.display.work_area = CapabilitySupport::Unknown;
    capabilities.display.query_window_position = CapabilitySupport::Unknown;
    capabilities.display.set_window_position = CapabilitySupport::Unknown;
    capabilities.transients.native_surface = CapabilitySupport::Unknown;
    capabilities.transients.popover = CapabilitySupport::Unknown;
    capabilities.transients.menu = CapabilitySupport::Unknown;
    capabilities.transients.context_menu = CapabilitySupport::Unknown;
    capabilities.transients.combo_box = CapabilitySupport::Unknown;
    capabilities.transients.tooltip = CapabilitySupport::Unknown;
    capabilities.advanced_input.pointer_metadata = CapabilitySupport::Supported;
    capabilities.advanced_input.cursor_icons = CapabilitySupport::Supported;
    capabilities.advanced_input.cursor_visibility = CapabilitySupport::Supported;
    capabilities.advanced_input.cursor_position = CapabilitySupport::Unknown;
    capabilities.advanced_input.cursor_confine = CapabilitySupport::Unknown;
    capabilities.advanced_input.cursor_lock = CapabilitySupport::Unknown;
    // Clipboard availability depends on whether the native clipboard service
    // can be opened for the concrete session/window, so creation refines it.
    capabilities.data_transfer.clipboard_text = CapabilitySupport::Unknown;
    capabilities.data_transfer.clipboard_rich = CapabilitySupport::Unknown;
    capabilities.data_transfer.clipboard_custom = CapabilitySupport::Unsupported;
    capabilities.data_transfer.external_drag_drop = CapabilitySupport::Unknown;
    capabilities.data_transfer.external_drag_drop_files = CapabilitySupport::Unknown;
    capabilities.data_transfer.external_drag_drop_rich = CapabilitySupport::Unsupported;
    capabilities.application_services.file_dialogs =
        crate::file_dialogs::desktop_file_dialog_capabilities(None);
    capabilities.application_services.global_shortcuts = CapabilitySupport::Unknown;
    capabilities.application_services.single_instance_activation = CapabilitySupport::Supported;
    capabilities
}

fn refine_window_capabilities(
    mut capabilities: PlatformCapabilities,
    system: NativeWindowSystem,
    services: &dyn DesktopPlatformServices,
) -> PlatformCapabilities {
    capabilities.display.enumerate_displays = CapabilitySupport::Supported;
    capabilities.display.query_current_display = CapabilitySupport::Supported;
    capabilities.display.work_area = services.work_area_support(system);
    capabilities.transients.popover =
        services.transient_support(system, incular_config::TransientRole::Popover);
    capabilities.transients.menu =
        services.transient_support(system, incular_config::TransientRole::Menu);
    capabilities.transients.context_menu =
        services.transient_support(system, incular_config::TransientRole::ContextMenu);
    capabilities.transients.combo_box =
        services.transient_support(system, incular_config::TransientRole::ComboBox);
    capabilities.transients.tooltip =
        services.transient_support(system, incular_config::TransientRole::Tooltip);
    let transient_support = [
        capabilities.transients.popover,
        capabilities.transients.menu,
        capabilities.transients.context_menu,
        capabilities.transients.combo_box,
        capabilities.transients.tooltip,
    ];
    capabilities.transients.native_surface =
        if transient_support.contains(&CapabilitySupport::Supported) {
            CapabilitySupport::Supported
        } else if transient_support.contains(&CapabilitySupport::Unknown) {
            CapabilitySupport::Unknown
        } else {
            CapabilitySupport::Unsupported
        };
    match system {
        NativeWindowSystem::Win32 => {
            capabilities.window.begin_resize_drag = CapabilitySupport::Supported;
            capabilities.window.set_window_level = CapabilitySupport::Supported;
            capabilities.window.set_window_icon = CapabilitySupport::Supported;
            capabilities.window.request_user_attention = CapabilitySupport::Supported;
            capabilities.display.display_bounds = CapabilitySupport::Supported;
            capabilities.display.query_window_position = CapabilitySupport::Supported;
            capabilities.display.set_window_position = CapabilitySupport::Supported;
            capabilities.advanced_input.cursor_position = CapabilitySupport::Supported;
            capabilities.advanced_input.cursor_confine = CapabilitySupport::Supported;
            capabilities.advanced_input.cursor_lock = CapabilitySupport::Supported;
        }
        NativeWindowSystem::AppKit => {
            capabilities.window.begin_resize_drag = CapabilitySupport::Unsupported;
            capabilities.window.set_window_level = CapabilitySupport::Supported;
            capabilities.window.set_window_icon = CapabilitySupport::Unsupported;
            capabilities.window.request_user_attention = CapabilitySupport::Supported;
            capabilities.display.display_bounds = CapabilitySupport::Supported;
            capabilities.display.query_window_position = CapabilitySupport::Supported;
            capabilities.display.set_window_position = CapabilitySupport::Supported;
            capabilities.advanced_input.cursor_position = CapabilitySupport::Supported;
            capabilities.advanced_input.cursor_confine = CapabilitySupport::Unsupported;
            capabilities.advanced_input.cursor_lock = CapabilitySupport::Supported;
        }
        NativeWindowSystem::X11 => {
            capabilities.window.begin_resize_drag = CapabilitySupport::Supported;
            capabilities.window.set_window_level = CapabilitySupport::Supported;
            capabilities.window.set_window_icon = CapabilitySupport::Supported;
            capabilities.window.request_user_attention = CapabilitySupport::Supported;
            capabilities.display.display_bounds = CapabilitySupport::Supported;
            capabilities.display.query_window_position = CapabilitySupport::Supported;
            capabilities.display.set_window_position = CapabilitySupport::Supported;
            capabilities.advanced_input.cursor_position = CapabilitySupport::Supported;
            capabilities.advanced_input.cursor_confine = CapabilitySupport::Supported;
            capabilities.advanced_input.cursor_lock = CapabilitySupport::Unsupported;
        }
        NativeWindowSystem::Wayland => {
            capabilities.window.begin_resize_drag = CapabilitySupport::Supported;
            capabilities.window.set_window_level = CapabilitySupport::Unsupported;
            capabilities.window.set_window_icon = CapabilitySupport::Unsupported;
            // Winit can issue an xdg-activation request, but protocol availability
            // is compositor/session dependent and is not queryable here.
            capabilities.window.request_user_attention = CapabilitySupport::Unknown;
            // A Wayland compositor owns top-level placement. Winit's output
            // metadata must not be repurposed into a fake desktop-global
            // coordinate system for application windows.
            capabilities.display.display_bounds = CapabilitySupport::Unsupported;
            capabilities.display.work_area = CapabilitySupport::Unsupported;
            capabilities.display.query_window_position = CapabilitySupport::Unsupported;
            capabilities.display.set_window_position = CapabilitySupport::Unsupported;
            // Pointer-constraints is an optional compositor protocol. Let the
            // actual Winit operation decide and return a typed native result.
            capabilities.advanced_input.cursor_position = CapabilitySupport::Unknown;
            capabilities.advanced_input.cursor_confine = CapabilitySupport::Unknown;
            capabilities.advanced_input.cursor_lock = CapabilitySupport::Unknown;
        }
        NativeWindowSystem::Other => {}
    }
    let external_file_drag = services.external_file_drag_support(system);
    capabilities.data_transfer.external_drag_drop = external_file_drag;
    capabilities.data_transfer.external_drag_drop_files = external_file_drag;
    capabilities.data_transfer.external_drag_drop_rich = CapabilitySupport::Unsupported;
    capabilities.data_transfer.clipboard_custom = CapabilitySupport::Unsupported;
    capabilities.application_services.file_dialogs =
        crate::file_dialogs::desktop_file_dialog_capabilities(Some(system));
    capabilities.application_services.global_shortcuts =
        crate::global_shortcuts::support_for(system);
    capabilities.application_services.single_instance_activation = CapabilitySupport::Supported;
    capabilities
}

fn clipboard_format_bidirectional_support(
    capabilities: incular_platform::ClipboardCapabilities,
    format: &incular_platform::TransferFormat,
) -> CapabilitySupport {
    if capabilities.read.support(format) == CapabilitySupport::Supported
        && capabilities.write.support(format) == CapabilitySupport::Supported
    {
        CapabilitySupport::Supported
    } else {
        CapabilitySupport::Unsupported
    }
}

fn operation_support(
    capabilities: &PlatformCapabilities,
    operation: &WindowOperation,
) -> CapabilitySupport {
    match operation {
        WindowOperation::SetTitle(_) => capabilities.window.set_title,
        WindowOperation::SetVisible(_) => capabilities.window.set_visibility,
        WindowOperation::SetLogicalSize(_) => capabilities.window.set_logical_size,
        WindowOperation::SetOuterPosition(_) => capabilities.display.set_window_position,
        WindowOperation::BeginMoveDrag => capabilities.window.begin_move_drag,
        WindowOperation::BeginResizeDrag(_) => capabilities.window.begin_resize_drag,
        WindowOperation::SetMinimized(_) => capabilities.window.minimize,
        WindowOperation::SetMaximized(_) => capabilities.window.maximize,
        WindowOperation::SetFullscreen(_) => capabilities.window.fullscreen,
        WindowOperation::SetResizable(_) => capabilities.window.set_resizable,
        WindowOperation::SetDecorations(_) => capabilities.window.set_decorations,
        WindowOperation::SetLogicalSizeLimits(_) => capabilities.window.set_size_limits,
        WindowOperation::SetWindowLevel(_) => capabilities.window.set_window_level,
        WindowOperation::SetWindowIcon(_) => capabilities.window.set_window_icon,
        WindowOperation::RequestUserAttention(_) => capabilities.window.request_user_attention,
        WindowOperation::SetCursorGrab(CursorGrabMode::None) => CapabilitySupport::Supported,
        WindowOperation::SetCursorGrab(CursorGrabMode::Confined) => {
            capabilities.advanced_input.cursor_confine
        }
        WindowOperation::SetCursorGrab(CursorGrabMode::Locked) => {
            capabilities.advanced_input.cursor_lock
        }
        WindowOperation::SetCursorVisible(_) => capabilities.advanced_input.cursor_visibility,
        WindowOperation::SetCursorPosition(_) => capabilities.advanced_input.cursor_position,
        WindowOperation::SetContentSensitivity(_) => capabilities.window.content_sensitivity,
        WindowOperation::RequestFocus => capabilities.window.request_focus,
        WindowOperation::RequestRedraw => capabilities.window.request_redraw,
        WindowOperation::Close => capabilities.window.close,
    }
}

fn unsupported_initial_window_policy(
    options: &WindowOptions,
    capabilities: &PlatformCapabilities,
) -> Option<PlatformOperationError> {
    let mut unsupported = Vec::new();
    if options.window_icon.is_some()
        && capabilities.window.set_window_icon == CapabilitySupport::Unsupported
    {
        unsupported.push("window icon");
    }
    if options.window_level != WindowLevel::Normal
        && capabilities.window.set_window_level == CapabilitySupport::Unsupported
    {
        unsupported.push("window level");
    }
    (!unsupported.is_empty()).then(|| {
        PlatformOperationError::with_context(
            PlatformOperationErrorKind::Unsupported,
            format!(
                "initial native window policy is unsupported on this window system: {}",
                unsupported.join(", ")
            ),
        )
    })
}

fn observed_window_state(
    window: &Window,
    displays: &DesktopDisplayRegistry,
    system: NativeWindowSystem,
) -> WindowObservedState {
    let outer_position = if matches!(system, NativeWindowSystem::Wayland) {
        None
    } else {
        window
            .outer_position()
            .ok()
            .map(|position| PhysicalScreenPosition::new(position.x, position.y))
    };
    let outer_size = window.outer_size();
    WindowObservedState {
        visible: window.is_visible(),
        minimized: window.is_minimized(),
        maximized: Some(window.is_maximized()),
        fullscreen: Some(window.fullscreen().is_some()),
        resizable: Some(window.is_resizable()),
        decorations: Some(window.is_decorated()),
        outer_position,
        outer_size: Some(PhysicalSize::new(outer_size.width, outer_size.height)),
        current_display: window
            .current_monitor()
            .as_ref()
            .and_then(|monitor| displays.id_for(monitor)),
    }
}

fn native_resize_direction(direction: WindowResizeDirection) -> NativeResizeDirection {
    match direction {
        WindowResizeDirection::East => NativeResizeDirection::East,
        WindowResizeDirection::North => NativeResizeDirection::North,
        WindowResizeDirection::NorthEast => NativeResizeDirection::NorthEast,
        WindowResizeDirection::NorthWest => NativeResizeDirection::NorthWest,
        WindowResizeDirection::South => NativeResizeDirection::South,
        WindowResizeDirection::SouthEast => NativeResizeDirection::SouthEast,
        WindowResizeDirection::SouthWest => NativeResizeDirection::SouthWest,
        WindowResizeDirection::West => NativeResizeDirection::West,
    }
}

fn native_window_level(level: WindowLevel) -> NativeWindowLevel {
    match level {
        WindowLevel::Normal => NativeWindowLevel::Normal,
        WindowLevel::AlwaysOnTop => NativeWindowLevel::AlwaysOnTop,
    }
}

fn native_attention_type(attention: UserAttentionType) -> NativeAttentionType {
    match attention {
        UserAttentionType::Informational => NativeAttentionType::Informational,
        UserAttentionType::Critical => NativeAttentionType::Critical,
    }
}

fn mouse_pointer_metadata(
    device: u64,
    buttons: u32,
    button: Option<u32>,
    phase: PointerPhase,
) -> PointerMetadata {
    PointerMetadata {
        pointer: 0,
        device,
        kind: PointerDeviceKind::Mouse,
        buttons,
        button,
        phase,
    }
}

fn native_cursor_grab_mode(mode: CursorGrabMode) -> NativeCursorGrabMode {
    match mode {
        CursorGrabMode::None => NativeCursorGrabMode::None,
        CursorGrabMode::Confined => NativeCursorGrabMode::Confined,
        CursorGrabMode::Locked => NativeCursorGrabMode::Locked,
    }
}

fn native_window_icon(icon: &WindowIcon) -> Result<NativeWindowIcon, PlatformOperationError> {
    NativeWindowIcon::from_rgba(icon.rgba().to_vec(), icon.width(), icon.height()).map_err(
        |error| {
            PlatformOperationError::with_context(
                PlatformOperationErrorKind::NativeFailure,
                error.to_string(),
            )
        },
    )
}

fn map_external_error(error: winit::error::ExternalError) -> PlatformOperationError {
    match error {
        winit::error::ExternalError::NotSupported(_) => PlatformOperationError::unsupported(),
        winit::error::ExternalError::Ignored => {
            PlatformOperationError::new(PlatformOperationErrorKind::RejectedByPlatform)
        }
        winit::error::ExternalError::Os(error) => PlatformOperationError::with_context(
            PlatformOperationErrorKind::NativeFailure,
            error.to_string(),
        ),
    }
}

impl Drop for MultiApp {
    fn drop(&mut self) {
        let Some(system) = self.window_system else {
            return;
        };
        // OS menu backends may subclass or associate state with the native
        // window itself. Detach those resources while the Winit Window values
        // are unquestionably still live; relying on struct field drop order
        // would destroy the windows before `platform_services` can clean up.
        for state in self.windows.values() {
            self.platform_services
                .unregister_platform_menu_window(system, &state.window);
        }
    }
}

impl ApplicationHandler<RuntimeWakeEvent> for MultiApp {
    fn resumed(&mut self, target: &ActiveEventLoop) {
        self.apply_window_commands(target);
        self.refresh_displays(target);
        self.refresh_system_environment(true);
        self.drain_application_lifecycle();
        self.drain_single_instance_activations();
        self.process_pending_global_shortcuts();
        self.start_pending_file_dialogs();
    }

    fn exiting(&mut self, _: &ActiveEventLoop) {
        if !self.application.should_exit() {
            self.application
                .handle_application_lifecycle(PlatformLifecycle::Stopping);
        }
    }

    fn window_event(
        &mut self,
        target: &ActiveEventLoop,
        native_id: NativeWindowId,
        event: WindowEvent,
    ) {
        if matches!(event, WindowEvent::Destroyed)
            && self.pending_native_destructions.remove(&native_id)
        {
            // The native handle is now actually gone. This acknowledgement is
            // required by services whose Winit `Window::drop` is asynchronous
            // (notably Win32); only now is last-window event-loop exit safe.
            self.apply_window_commands(target);
            return;
        }
        if self.handle_transient_window_event(native_id, &event) {
            self.apply_window_commands(target);
            return;
        }
        let Some(id) = self.windows.get(&native_id).map(|state| state.id) else {
            return;
        };
        if let Some(state) = self.windows.get_mut(&native_id) {
            state
                .accessibility
                .adapter
                .process_event(&state.window, &event);
        }
        match event {
            WindowEvent::CloseRequested => self.route_window_event(
                native_id,
                IncularWindowEvent::platform(id, PlatformEvent::CloseRequested),
            ),
            WindowEvent::Resized(size) => {
                let scale = self
                    .windows
                    .get(&native_id)
                    .expect("known native window")
                    .metrics
                    .scale_factor;
                self.resize_window(id, PhysicalSize::new(size.width, size.height), scale);
                self.publish_window_state(id);
            }
            WindowEvent::Moved(_) => {
                self.refresh_displays(target);
                self.publish_window_state(id);
                self.reposition_transient_hosts(id);
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                let size = self
                    .windows
                    .get(&native_id)
                    .expect("known native window")
                    .window
                    .inner_size();
                self.resize_window(id, PhysicalSize::new(size.width, size.height), scale_factor);
                self.refresh_displays(target);
                self.publish_window_state(id);
                self.reposition_transient_hosts(id);
            }
            WindowEvent::Focused(focused) => {
                let environment_changed = self
                    .windows
                    .get_mut(&native_id)
                    .is_some_and(|state| state.environment.set_focused(focused));
                if !focused {
                    self.cancel_window_mouse_pointer(native_id, id);
                }
                self.route_window_event(
                    native_id,
                    IncularWindowEvent::lifecycle(
                        id,
                        if focused {
                            WindowLifecycle::Focused
                        } else {
                            WindowLifecycle::Unfocused
                        },
                    ),
                );
                if environment_changed {
                    self.publish_window_environment(native_id);
                }
            }
            WindowEvent::ThemeChanged(theme) => {
                if self
                    .windows
                    .get_mut(&native_id)
                    .is_some_and(|state| state.environment.set_theme(theme))
                {
                    self.publish_window_environment(native_id);
                }
            }
            WindowEvent::Occluded(occluded) => {
                if self
                    .windows
                    .get_mut(&native_id)
                    .is_some_and(|state| state.environment.set_occluded(occluded))
                {
                    self.publish_window_environment(native_id);
                }
            }
            WindowEvent::CursorMoved {
                device_id,
                position,
            } => {
                self.note_window_mouse(native_id);
                let device = self.pointer_devices.id(device_id);
                let (event, external_over) = {
                    let state = self
                        .windows
                        .get_mut(&native_id)
                        .expect("known native window");
                    state.cursor = position;
                    (
                        pointer_event_with_metadata(
                            mouse_pointer_metadata(
                                device,
                                state.mouse_buttons.pressed(),
                                None,
                                PointerPhase::Move,
                            ),
                            position,
                            state.metrics,
                        ),
                        state
                            .external_file_drag
                            .over(logical_cursor_position(state.cursor, state.metrics)),
                    )
                };
                #[cfg(feature = "devtools")]
                if self.devtools_state.inspect_pointer(
                    &self.application,
                    id,
                    Offset::new(
                        (position.x / self.windows[&native_id].metrics.scale_factor) as f32,
                        (position.y / self.windows[&native_id].metrics.scale_factor) as f32,
                    ),
                    false,
                ) {
                    return;
                }
                self.route_window_event(native_id, IncularWindowEvent::platform(id, event));
                if let Some(event) = external_over {
                    let _ = self.application.handle_external_drag_event(id, event);
                }
                self.sync_window_cursor(id);
            }
            WindowEvent::HoveredFile(path) => {
                if let Some(position) = self.native_external_drag_position(native_id) {
                    let event = self
                        .windows
                        .get_mut(&native_id)
                        .expect("known native window")
                        .external_file_drag
                        .hover(path, position);
                    let _ = self.application.handle_external_drag_event(id, event);
                }
            }
            WindowEvent::DroppedFile(path) => {
                let position = self.native_external_drag_position(native_id).or_else(|| {
                    self.windows
                        .get(&native_id)
                        .and_then(|state| state.external_file_drag.current_position())
                });
                if let Some(position) = position {
                    self.windows
                        .get_mut(&native_id)
                        .expect("known native window")
                        .external_file_drag
                        .queue_drop(path, position);
                }
            }
            WindowEvent::HoveredFileCancelled => {
                let position = self.native_external_drag_position(native_id).or_else(|| {
                    self.windows
                        .get(&native_id)
                        .and_then(|state| state.external_file_drag.current_position())
                });
                let event = position.and_then(|position| {
                    self.windows
                        .get_mut(&native_id)
                        .and_then(|state| state.external_file_drag.cancel(position))
                });
                if let Some(event) = event {
                    let _ = self.application.handle_external_drag_event(id, event);
                }
            }
            WindowEvent::CursorEntered { device_id } | WindowEvent::CursorLeft { device_id } => {
                self.note_window_mouse(native_id);
                let phase = if matches!(event, WindowEvent::CursorEntered { .. }) {
                    PointerPhase::Enter
                } else {
                    PointerPhase::Exit
                };
                let device = self.pointer_devices.id(device_id);
                let event = {
                    let state = self.windows.get(&native_id).expect("known native window");
                    pointer_event_with_metadata(
                        mouse_pointer_metadata(device, state.mouse_buttons.pressed(), None, phase),
                        state.cursor,
                        state.metrics,
                    )
                };
                self.route_window_event(native_id, IncularWindowEvent::platform(id, event));
                self.sync_window_cursor(id);
            }
            WindowEvent::MouseInput {
                device_id,
                state,
                button,
            } => {
                self.note_window_mouse(native_id);
                let device = self.pointer_devices.id(device_id);
                let Some(transition) = self
                    .windows
                    .get_mut(&native_id)
                    .expect("known native window")
                    .mouse_buttons
                    .transition(state, button)
                else {
                    self.apply_window_commands(target);
                    return;
                };
                let event = {
                    let state_ref = self.windows.get(&native_id).expect("known native window");
                    pointer_event_with_metadata(
                        mouse_pointer_metadata(
                            device,
                            transition.buttons,
                            Some(transition.button),
                            transition.phase,
                        ),
                        state_ref.cursor,
                        state_ref.metrics,
                    )
                };
                #[cfg(feature = "devtools")]
                if transition.phase == PointerPhase::Down
                    && transition.button == PRIMARY_POINTER_BUTTON
                    && self.devtools_state.inspect_pointer(
                        &self.application,
                        id,
                        Offset::new(
                            (self.windows[&native_id].cursor.x
                                / self.windows[&native_id].metrics.scale_factor)
                                as f32,
                            (self.windows[&native_id].cursor.y
                                / self.windows[&native_id].metrics.scale_factor)
                                as f32,
                        ),
                        true,
                    )
                {
                    return;
                }
                self.route_window_event(native_id, IncularWindowEvent::platform(id, event));
                self.sync_window_cursor(id);
            }
            WindowEvent::Touch(touch) => {
                self.note_window_touch(native_id);
                let device = self.pointer_devices.id(touch.device_id);
                let metrics = self
                    .windows
                    .get(&native_id)
                    .expect("known native window")
                    .metrics;
                self.route_window_event(
                    native_id,
                    IncularWindowEvent::platform(
                        id,
                        touch_event_with_device(touch, device, metrics),
                    ),
                );
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.note_window_mouse(native_id);
                let metrics = self
                    .windows
                    .get(&native_id)
                    .expect("known native window")
                    .metrics;
                self.route_window_event(
                    native_id,
                    IncularWindowEvent::platform(id, wheel_event(delta, metrics)),
                );
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.windows
                    .get_mut(&native_id)
                    .expect("known native window")
                    .modifiers = modifiers.state();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                self.note_window_keyboard(native_id);
                let modifiers = self
                    .windows
                    .get(&native_id)
                    .expect("known native window")
                    .modifiers;
                let global_handled = self
                    .global_shortcuts
                    .as_ref()
                    .is_some_and(|shortcuts| shortcuts.suppress_local_key_event(&event, modifiers));
                let menu_handled = event.state == winit::event::ElementState::Pressed
                    && platform_menu_key(&event).is_some_and(|key| {
                        self.platform_menu_delegate
                            .handle_shortcut(&key, platform_menu_modifiers(modifiers))
                    });
                if !global_handled && !menu_handled {
                    self.route_window_event(
                        native_id,
                        IncularWindowEvent::platform(id, key_event(&event, modifiers)),
                    );
                    if let Some(text) = text_event(&event) {
                        self.route_window_event(native_id, IncularWindowEvent::platform(id, text));
                    }
                }
            }
            WindowEvent::Ime(event) => {
                if let Some(ime) = ime_event(event) {
                    self.route_window_event(native_id, IncularWindowEvent::platform(id, ime));
                }
            }
            WindowEvent::RedrawRequested => {
                self.route_window_event(native_id, IncularWindowEvent::redraw_requested(id));
                self.redraw_window(target, id);
            }
            _ => {}
        }
        self.refresh_system_environment(false);
        self.drain_application_lifecycle();
        self.drain_single_instance_activations();
        self.apply_window_commands(target);
        self.process_pending_global_shortcuts();
    }

    fn about_to_wait(&mut self, target: &ActiveEventLoop) {
        self.frame_timestamp = Instant::now();
        self.platform_services.flush_platform_menu_events();
        self.sync_platform_menus();
        self.refresh_displays(target);
        self.refresh_system_environment(false);
        self.drain_application_lifecycle();
        self.drain_single_instance_activations();
        self.flush_external_file_drops();
        self.apply_window_commands(target);
        self.process_pending_global_shortcuts();
        self.start_pending_file_dialogs();
        for id in self.application.active_window_ids() {
            self.request_frame_if_needed(id);
        }
        target.set_control_flow(winit::event_loop::ControlFlow::Wait);
    }

    fn user_event(&mut self, target: &ActiveEventLoop, event: RuntimeWakeEvent) {
        match event {
            RuntimeWakeEvent::Runtime => self.application.process_runtime_work(),
            RuntimeWakeEvent::Accessibility(event) => self.route_accesskit_event(event),
            RuntimeWakeEvent::FileDialog(completion) => {
                let _ = self.application.complete_file_dialog(completion);
            }
            RuntimeWakeEvent::ApplicationActivation(activation) => {
                self.application.handle_application_activation(activation);
            }
            RuntimeWakeEvent::GlobalShortcut(event) => self.handle_global_shortcut_event(event),
            RuntimeWakeEvent::SingleInstanceActivation => self.drain_single_instance_activations(),
            RuntimeWakeEvent::SystemEnvironmentChanged => self.refresh_system_environment(true),
        }
        self.refresh_system_environment(false);
        self.drain_application_lifecycle();
        self.drain_single_instance_activations();
        self.platform_services.flush_platform_menu_events();
        self.sync_platform_menus();
        self.apply_window_commands(target);
        self.process_pending_global_shortcuts();
        self.start_pending_file_dialogs();
        for id in self.application.active_window_ids() {
            self.request_frame_if_needed(id);
        }
    }
}

fn logical_cursor_position(cursor: PhysicalPosition<f64>, metrics: WindowMetrics) -> Offset {
    let scale = if metrics.scale_factor.is_finite() && metrics.scale_factor > 0.0 {
        metrics.scale_factor
    } else {
        1.0
    };
    Offset::new((cursor.x / scale) as f32, (cursor.y / scale) as f32)
}

fn platform_menu_key(event: &winit::event::KeyEvent) -> Option<String> {
    match &event.logical_key {
        winit::keyboard::Key::Character(value) if !value.is_empty() => Some(value.to_string()),
        winit::keyboard::Key::Named(value) => Some(format!("{value:?}")),
        _ => None,
    }
}

fn platform_menu_modifiers(modifiers: winit::keyboard::ModifiersState) -> ShortcutModifiers {
    let mut result = ShortcutModifiers::empty();
    if modifiers.alt_key() {
        result = result.union(ShortcutModifiers::ALT);
    }
    if modifiers.control_key() {
        result = result.union(ShortcutModifiers::CONTROL);
    }
    if modifiers.super_key() {
        result = result.union(ShortcutModifiers::META);
    }
    if modifiers.shift_key() {
        result = result.union(ShortcutModifiers::SHIFT);
    }
    result
}

fn window_attributes(options: &WindowOptions) -> Result<WindowAttributes, PlatformOperationError> {
    let initial = winit::dpi::LogicalSize::new(
        f64::from(options.initial_logical_size.width),
        f64::from(options.initial_logical_size.height),
    );
    let minimum = options
        .minimum_logical_size
        .map(|size| winit::dpi::LogicalSize::new(f64::from(size.width), f64::from(size.height)));
    let maximum = options
        .maximum_logical_size
        .map(|size| winit::dpi::LogicalSize::new(f64::from(size.width), f64::from(size.height)));
    let icon = options
        .window_icon
        .as_ref()
        .map(native_window_icon)
        .transpose()?;
    let mut attributes = WindowAttributes::default()
        .with_title(options.title.clone())
        .with_inner_size(initial)
        .with_resizable(options.resizable)
        .with_visible(options.visible)
        .with_decorations(options.decorations)
        .with_transparent(options.transparency_mode.is_transparent())
        .with_maximized(options.maximized)
        .with_window_level(native_window_level(options.window_level))
        .with_window_icon(icon)
        .with_fullscreen(
            options
                .fullscreen
                .map(|_| winit::window::Fullscreen::Borderless(None)),
        );
    attributes.min_inner_size = minimum.map(Into::into);
    attributes.max_inner_size = maximum.map(Into::into);
    Ok(attributes)
}
