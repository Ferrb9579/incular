//! Shared desktop event-loop bridge for Incular.
use accesskit_winit::{
    Adapter as AccessKitAdapter, Event as AccessKitEvent, WindowEvent as AccessKitWindowEvent,
};
use incular_accessibility::AccessKitProjection;
use incular_config::Constraints;
#[cfg(feature = "devtools")]
use incular_core::Offset;
use incular_core::PointerPhase;
use incular_platform::{
    Clipboard, ContentSensitivityBackend, NoopContentSensitivityBackend, PhysicalSize,
    PlatformEvent, TransparencyMode, WindowCommand, WindowEvent as IncularWindowEvent,
    WindowId as IncularWindowId, WindowLifecycle, WindowMetrics, WindowOperation, WindowOptions,
    apply_text_input_command, ime_event, key_event, pointer_event, raw_window_handles, text_event,
    touch_event, wheel_event,
};
use incular_runtime::{
    Application, ApplicationLifecycle, GpuSample, NativeWindowCommand, RenderFrameMetrics, Runtime,
    RuntimeWake, Screenshot,
};
use incular_wgpu::{RenderStats, RendererError, SharedGpuContext, WgpuRenderer};
use incular_widgets::internal::ActionId;
use std::{collections::HashMap, sync::Arc, time::Instant};
#[cfg(feature = "devtools")]
use std::{path::PathBuf, process::Command};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowAttributes, WindowId as NativeWindowId},
};

#[derive(Debug)]
pub enum RunError {
    EventLoop(winit::error::EventLoopError),
}
impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EventLoop(e) => write!(f, "event-loop setup failed: {e}"),
        }
    }
}
impl std::error::Error for RunError {}

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
    mut runtime: Runtime,
    on_action: impl FnMut(ActionId) + 'static,
) -> Result<(), RunError> {
    runtime.set_clipboard(Box::new(DesktopClipboard::new()));
    let event_loop = EventLoop::<RuntimeWakeEvent>::with_user_event()
        .build()
        .map_err(RunError::EventLoop)?;
    let proxy = event_loop.create_proxy();
    runtime.set_wake_handler(Arc::new(DesktopWake(proxy.clone())));
    let mut app = App {
        window: None,
        runtime: Some(runtime),
        renderer: None,
        metrics: None,
        cursor: PhysicalPosition::new(0., 0.),
        modifiers: winit::keyboard::ModifiersState::default(),
        on_action,
        accessibility_proxy: proxy,
        accessibility: None,
    };
    event_loop.run_app(&mut app).map_err(RunError::EventLoop)
}

/// A Tokio completion wake has no payload: `Runtime` owns the bounded UI
/// message drain and runs it on the UI thread when this user event reaches
/// Winit.
enum RuntimeWakeEvent {
    Runtime,
    Accessibility(AccessKitEvent),
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
/// Native desktop clipboard with a local fallback for headless sessions or a
/// temporarily unavailable system clipboard service.
struct DesktopClipboard {
    native: Option<arboard::Clipboard>,
    fallback: String,
}
impl DesktopClipboard {
    fn new() -> Self {
        Self {
            native: arboard::Clipboard::new().ok(),
            fallback: String::new(),
        }
    }
}
impl Clipboard for DesktopClipboard {
    fn get_text(&mut self) -> Option<String> {
        self.native
            .as_mut()
            .and_then(|clipboard| clipboard.get_text().ok())
            .or_else(|| (!self.fallback.is_empty()).then(|| self.fallback.clone()))
    }
    fn set_text(&mut self, text: String) {
        self.fallback = text.clone();
        if let Some(clipboard) = self.native.as_mut() {
            let _ = clipboard.set_text(text);
        }
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
    let event_loop = EventLoop::<RuntimeWakeEvent>::with_user_event()
        .build()
        .map_err(RunError::EventLoop)?;
    let proxy = event_loop.create_proxy();
    let wake = Arc::new(DesktopWake(proxy.clone()));
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
        shared_gpu: None,
        frame_timestamp: Instant::now(),
        accessibility_proxy: proxy,
        #[cfg(feature = "devtools")]
        devtools_state: crate::devtools_runner::DevToolsState::new(devtools_agent),
    };
    app.application.set_wake_handler(wake);
    event_loop.run_app(&mut app).map_err(RunError::EventLoop)
}
struct App<F: FnMut(ActionId)> {
    window: Option<Window>,
    runtime: Option<Runtime>,
    renderer: Option<WgpuRenderer>,
    metrics: Option<WindowMetrics>,
    cursor: PhysicalPosition<f64>,
    modifiers: winit::keyboard::ModifiersState,
    on_action: F,
    accessibility_proxy: winit::event_loop::EventLoopProxy<RuntimeWakeEvent>,
    accessibility: Option<NativeAccessibilityState>,
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
        if let Some(runtime) = self.runtime.as_mut() {
            runtime.set_environment(environment_for(metrics));
            runtime.transition_lifecycle(ApplicationLifecycle::Active);
        }
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
                if let Some(runtime) = self.runtime.as_mut() {
                    runtime.set_environment(environment_for(self.metrics.expect("metrics exist")));
                }
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
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = position;
                if let (Some(runtime), Some(metrics)) = (self.runtime.as_mut(), self.metrics) {
                    let event = match pointer_event(PointerPhase::Move, position, metrics) {
                        PlatformEvent::Input(event) => event,
                        _ => unreachable!(),
                    };
                    let _ = runtime.handle_input(event);
                }
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                let phase = match state {
                    ElementState::Pressed => PointerPhase::Down,
                    ElementState::Released => PointerPhase::Up,
                };
                if let (Some(runtime), Some(metrics)) = (self.runtime.as_mut(), self.metrics) {
                    let event = match pointer_event(phase, self.cursor, metrics) {
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
            WindowEvent::Touch(touch) => {
                if let (Some(runtime), Some(metrics)) = (self.runtime.as_mut(), self.metrics) {
                    let event = match touch_event(touch, metrics) {
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
                if let (Some(runtime), Some(metrics)) = (self.runtime.as_mut(), self.metrics) {
                    let PlatformEvent::Input(event) = wheel_event(delta, metrics) else {
                        unreachable!()
                    };
                    let _ = runtime.handle_input(event);
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::KeyboardInput { event, .. } => {
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
    }
    fn about_to_wait(&mut self, loop_target: &ActiveEventLoop) {
        self.apply_text_input_commands();
        if self.runtime.as_ref().is_some_and(Runtime::frame_requested) {
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
                    if self.runtime.as_ref().is_some_and(Runtime::frame_requested) {
                        self.window
                            .as_ref()
                            .expect("window exists")
                            .request_redraw();
                    }
                }
            }
            RuntimeWakeEvent::Accessibility(event) => self.handle_accesskit_event(event),
        }
    }
}
impl<F: FnMut(ActionId)> App<F> {
    fn resize(&mut self, size: PhysicalSize) {
        let scale = self.metrics.expect("metrics exist").scale_factor;
        self.metrics = Some(WindowMetrics::new(size, scale));
        if let Some(runtime) = self.runtime.as_mut() {
            runtime.set_environment(environment_for(self.metrics.expect("metrics exist")));
        }
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
        if metrics.physical_size.is_zero() {
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
    window: Window,
    renderer: WgpuRenderer,
    metrics: WindowMetrics,
    cursor: PhysicalPosition<f64>,
    modifiers: winit::keyboard::ModifiersState,
    accessibility: NativeAccessibilityState,
    content_sensitivity: NoopContentSensitivityBackend,
}

/// Winit 0.30 adapter for an [`Application`] with many retained roots. Native
/// IDs stay in these two maps and are never exposed through Incular APIs.
struct MultiApp {
    application: Application,
    windows: HashMap<NativeWindowId, NativeWindowState>,
    native_ids: HashMap<IncularWindowId, NativeWindowId>,
    shared_gpu: Option<SharedGpuContext>,
    frame_timestamp: Instant,
    accessibility_proxy: winit::event_loop::EventLoopProxy<RuntimeWakeEvent>,
    /// Debug overlays, Select Widget mode, and the DevTools agent pump.
    #[cfg(feature = "devtools")]
    devtools_state: crate::devtools_runner::DevToolsState,
}

impl MultiApp {
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
            target.exit();
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
        let attributes = window_attributes(&options).with_visible(false);
        let window = match target.create_window(attributes) {
            Ok(window) => window,
            Err(error) => {
                eprintln!("Incular window error: {error}");
                let _ = self.application.close_window(id);
                return;
            }
        };
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
                let _ = self.application.close_window(id);
                return;
            }
        };
        let mut accessibility = NativeAccessibilityState {
            adapter: AccessKitAdapter::with_event_loop_proxy(
                target,
                &window,
                self.accessibility_proxy.clone(),
            ),
            projection: AccessKitProjection::new(),
            active: false,
        };
        accessibility.projection.note_adapter_created();
        self.application
            .set_window_clipboard(id, Box::new(DesktopClipboard::new()));
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
        let native_id = window.id();
        window.set_visible(options.visible);
        self.native_ids.insert(id, native_id);
        self.windows.insert(
            native_id,
            NativeWindowState {
                id,
                window,
                renderer,
                metrics,
                cursor: PhysicalPosition::new(0., 0.),
                modifiers: winit::keyboard::ModifiersState::default(),
                accessibility,
                content_sensitivity: NoopContentSensitivityBackend,
            },
        );
        self.request_frame_if_needed(id);
    }

    fn operate_window(&mut self, command: WindowCommand) {
        let Some(native_id) = self.native_ids.get(&command.window_id).copied() else {
            return;
        };
        if matches!(command.operation, WindowOperation::Close) {
            self.native_ids.remove(&command.window_id);
            if let Some(mut state) = self.windows.remove(&native_id) {
                state.accessibility.projection.note_adapter_destroyed();
            }
            return;
        }
        let mut synchronous_resize = None;
        {
            let Some(state) = self.windows.get_mut(&native_id) else {
                return;
            };
            match command.operation {
                WindowOperation::SetTitle(title) => state.window.set_title(&title),
                WindowOperation::SetVisible(visible) => state.window.set_visible(visible),
                WindowOperation::SetLogicalSize(size) => {
                    synchronous_resize = state
                        .window
                        .request_inner_size(winit::dpi::LogicalSize::new(size.width, size.height))
                        .map(|physical| {
                            (
                                PhysicalSize::new(physical.width, physical.height),
                                state.window.scale_factor(),
                            )
                        });
                }
                WindowOperation::SetContentSensitivity(sensitivity) => {
                    let _ = state.content_sensitivity.apply(sensitivity);
                }
                WindowOperation::RequestFocus => state.window.focus_window(),
                WindowOperation::RequestRedraw => state.window.request_redraw(),
                WindowOperation::Close => unreachable!(),
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
    }

    fn request_frame_if_needed(&mut self, id: IncularWindowId) {
        self.apply_text_input_commands(id);
        if self.application.frame_requested(id)
            && let Some(native_id) = self.native_ids.get(&id).copied()
            && let Some(state) = self.windows.get(&native_id)
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

    fn redraw_window(&mut self, id: IncularWindowId) {
        #[cfg(feature = "devtools")]
        self.devtools_state.drain(&mut self.application);
        let Some(native_id) = self.native_ids.get(&id).copied() else {
            return;
        };
        let Some(state) = self.windows.get_mut(&native_id) else {
            return;
        };
        let metrics = state.metrics;
        if self.application.simulation_capture_pending(id) {
            state.renderer.request_capture();
        }
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
                match state.renderer.render(&list, metrics.scale_factor) {
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

impl ApplicationHandler<RuntimeWakeEvent> for MultiApp {
    fn resumed(&mut self, target: &ActiveEventLoop) {
        self.apply_window_commands(target);
    }

    fn window_event(
        &mut self,
        target: &ActiveEventLoop,
        native_id: NativeWindowId,
        event: WindowEvent,
    ) {
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
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                let size = self
                    .windows
                    .get(&native_id)
                    .expect("known native window")
                    .window
                    .inner_size();
                self.resize_window(id, PhysicalSize::new(size.width, size.height), scale_factor);
            }
            WindowEvent::Focused(focused) => self.route_window_event(
                native_id,
                IncularWindowEvent::lifecycle(
                    id,
                    if focused {
                        WindowLifecycle::Focused
                    } else {
                        WindowLifecycle::Unfocused
                    },
                ),
            ),
            WindowEvent::CursorMoved { position, .. } => {
                let event = {
                    let state = self
                        .windows
                        .get_mut(&native_id)
                        .expect("known native window");
                    state.cursor = position;
                    pointer_event(PointerPhase::Move, position, state.metrics)
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
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                let event = {
                    let state_ref = self.windows.get(&native_id).expect("known native window");
                    pointer_event(
                        if state == ElementState::Pressed {
                            PointerPhase::Down
                        } else {
                            PointerPhase::Up
                        },
                        state_ref.cursor,
                        state_ref.metrics,
                    )
                };
                #[cfg(feature = "devtools")]
                if state == ElementState::Pressed
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
            }
            WindowEvent::Touch(touch) => {
                let metrics = self
                    .windows
                    .get(&native_id)
                    .expect("known native window")
                    .metrics;
                self.route_window_event(
                    native_id,
                    IncularWindowEvent::platform(id, touch_event(touch, metrics)),
                );
            }
            WindowEvent::MouseWheel { delta, .. } => {
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
                let modifiers = self
                    .windows
                    .get(&native_id)
                    .expect("known native window")
                    .modifiers;
                self.route_window_event(
                    native_id,
                    IncularWindowEvent::platform(id, key_event(&event, modifiers)),
                );
                if let Some(text) = text_event(&event) {
                    self.route_window_event(native_id, IncularWindowEvent::platform(id, text));
                }
            }
            WindowEvent::Ime(event) => {
                if let Some(ime) = ime_event(event) {
                    self.route_window_event(native_id, IncularWindowEvent::platform(id, ime));
                }
            }
            WindowEvent::RedrawRequested => {
                self.route_window_event(native_id, IncularWindowEvent::redraw_requested(id));
                self.redraw_window(id);
            }
            _ => {}
        }
        self.apply_window_commands(target);
    }

    fn about_to_wait(&mut self, target: &ActiveEventLoop) {
        self.frame_timestamp = Instant::now();
        self.apply_window_commands(target);
        for id in self.application.active_window_ids() {
            self.request_frame_if_needed(id);
        }
        target.set_control_flow(winit::event_loop::ControlFlow::Wait);
    }

    fn user_event(&mut self, target: &ActiveEventLoop, event: RuntimeWakeEvent) {
        match event {
            RuntimeWakeEvent::Runtime => self.application.process_runtime_work(),
            RuntimeWakeEvent::Accessibility(event) => self.route_accesskit_event(event),
        }
        self.apply_window_commands(target);
        for id in self.application.active_window_ids() {
            self.request_frame_if_needed(id);
        }
    }
}

fn window_attributes(options: &WindowOptions) -> WindowAttributes {
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
    let mut attributes = WindowAttributes::default()
        .with_title(options.title.clone())
        .with_inner_size(initial)
        .with_resizable(options.resizable)
        .with_visible(options.visible)
        .with_decorations(options.decorations)
        .with_transparent(options.transparency_mode.is_transparent())
        .with_maximized(options.maximized)
        .with_fullscreen(
            options
                .fullscreen
                .map(|_| winit::window::Fullscreen::Borderless(None)),
        );
    attributes.min_inner_size = minimum.map(Into::into);
    attributes.max_inner_size = maximum.map(Into::into);
    attributes
}

fn environment_for(metrics: WindowMetrics) -> incular_config::RuntimeEnvironment {
    incular_config::RuntimeEnvironment {
        viewport: metrics.logical_size(),
        physical_width: metrics.physical_size.width,
        physical_height: metrics.physical_size.height,
        scale_factor: metrics.scale_factor,
        input: incular_config::InputCapabilities {
            mouse: true,
            touch: true,
            keyboard: true,
            stylus: false,
        },
        ..incular_config::RuntimeEnvironment::default()
    }
}
