//! Linux desktop event-loop bridge for Incular.
use incular_config::Constraints;
use incular_core::PointerPhase;
use incular_platform::{
    Clipboard, PhysicalSize, PlatformEvent, WindowMetrics, ime_event, key_event, pointer_event,
    raw_window_handles, text_event, touch_event, wheel_event,
};
use incular_runtime::{Application, Runtime};
use incular_wgpu::{RendererError, WgpuRenderer};
use incular_widgets::ActionId;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowAttributes, WindowId},
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
/// Runs a native Linux window until close. The native window stays alive for the
/// complete lifetime of the renderer's unsafe raw-handle surface.
pub fn run_window(
    mut runtime: Runtime,
    on_action: impl FnMut(ActionId) + 'static,
) -> Result<(), RunError> {
    runtime.set_clipboard(Box::new(LinuxClipboard::new()));
    let event_loop = EventLoop::new().map_err(RunError::EventLoop)?;
    let mut app = App {
        window: None,
        runtime: Some(runtime),
        renderer: None,
        metrics: None,
        cursor: PhysicalPosition::new(0., 0.),
        modifiers: winit::keyboard::ModifiersState::default(),
        on_action,
    };
    event_loop.run_app(&mut app).map_err(RunError::EventLoop)
}
/// Native desktop clipboard with a local fallback for headless sessions or a
/// temporarily unavailable X11/Wayland clipboard service.
struct LinuxClipboard {
    native: Option<arboard::Clipboard>,
    fallback: String,
}
impl LinuxClipboard {
    fn new() -> Self {
        Self {
            native: arboard::Clipboard::new().ok(),
            fallback: String::new(),
        }
    }
}
impl Clipboard for LinuxClipboard {
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
pub fn run_application(application: Application) -> Result<(), RunError> {
    run_window(application.into_runtime(), |_| {})
}
struct App<F: FnMut(ActionId)> {
    window: Option<Window>,
    runtime: Option<Runtime>,
    renderer: Option<WgpuRenderer>,
    metrics: Option<WindowMetrics>,
    cursor: PhysicalPosition<f64>,
    modifiers: winit::keyboard::ModifiersState,
    on_action: F,
}
impl<F: FnMut(ActionId)> ApplicationHandler for App<F> {
    fn resumed(&mut self, loop_target: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let window = match loop_target.create_window(
            WindowAttributes::default()
                .with_title("Incular counter")
                .with_inner_size(winit::dpi::LogicalSize::new(420., 300.)),
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
        self.renderer = Some(renderer);
        self.window = Some(window);
    }
    fn window_event(
        &mut self,
        loop_target: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if self
            .window
            .as_ref()
            .is_none_or(|window| window.id() != window_id)
        {
            return;
        }
        match event {
            WindowEvent::CloseRequested => loop_target.exit(),
            WindowEvent::Resized(size) => self.resize(PhysicalSize::new(size.width, size.height)),
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                let size = self.window.as_ref().expect("window exists").inner_size();
                self.metrics = Some(WindowMetrics::new(
                    PhysicalSize::new(size.width, size.height),
                    scale_factor,
                ));
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
                        PlatformEvent::CloseRequested => unreachable!(),
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
                        PlatformEvent::CloseRequested => unreachable!(),
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
                        PlatformEvent::CloseRequested => unreachable!(),
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
    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if self.runtime.as_ref().is_some_and(Runtime::frame_requested) {
            self.window
                .as_ref()
                .expect("window exists")
                .request_redraw();
        }
    }
}
impl<F: FnMut(ActionId)> App<F> {
    fn resize(&mut self, size: PhysicalSize) {
        let scale = self.metrics.expect("metrics exist").scale_factor;
        self.metrics = Some(WindowMetrics::new(size, scale));
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
    }
}
