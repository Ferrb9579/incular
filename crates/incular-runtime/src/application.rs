use crate::application_activations::{ActivationRouteBridge, ApplicationActivationService};
use crate::application_shell::{
    ApplicationShellService, NativeApplicationShellCompletion, NativeApplicationShellEvent,
    NativeApplicationShellRequest,
};
use crate::application_types::{
    ApplicationSchedulerCounters, LastWindowPolicy, RestorableWindowFactory,
    RestorableWindowMetadata, WindowError, WindowRestorationId,
};
use crate::context::BuildContext;
use crate::file_dialogs::{
    FileDialogBridge, FileDialogCompletionStatus, NativeFileDialogCompletion,
    NativeFileDialogRequest, QueuedFileDialogRequest, explicitly_unsupported_request,
    result_matches_request,
};
use crate::frame::{FrameStats, Runtime};
use crate::global_shortcuts::{
    GlobalShortcutBridge, GlobalShortcutCompletionStatus, GlobalShortcutService,
    NativeGlobalShortcutCompletion, NativeGlobalShortcutOperation, NativeGlobalShortcutRequest,
    PendingGlobalShortcutRequest, QueuedGlobalShortcutRequest, drain_queued,
};
use crate::profiling::{
    AccessibilitySnapshot, BudgetStatistics, FrameHistory, FrameRecord, FrameWork, GpuSample,
    PerformanceHub, PerformanceProfiler, PerformanceSnapshot, ProfilerMode, RenderFrameMetrics,
    TextCacheSnapshot, WidgetWorkSnapshot, WindowPerformance,
};
use crate::request_registry::{CompletionRejection, RequestPhase, RequestRegistry};
use crate::restoration::{self, RestorationConfig, RestorationDiagnostics};
use crate::scheduler_counters;
use crate::simulation::{self, Screenshot, Simulation, SimulationError};
use crate::tasks::{self, RuntimeWake, Task, TaskFailure, TaskHandle, TokioHandle};
use crate::transient_presentation::{
    ResolvedTransientPresentation, TransientPresentationResolution,
};
use crate::window_commands::{
    DisplayCatalog, NativeOperationCompletionStatus, NativeWindowCommand, QueuedNativeRequest,
    QueuedWindowCommand, WindowCommandBridge, WindowHandle,
};
use crate::window_state::{
    ApplicationDiagnostics, ContentSizeCoordinator, WindowDiagnostics, WindowManager, WindowRecord,
    WindowRegistry,
};
use incular_accessibility::{
    AccessKitProjection, AccessibilityDiagnostics, NativeAccessibilityUpdate, SemanticActionRequest,
};
use incular_config::{Constraints, RuntimeEnvironment};
use incular_platform::{
    ApplicationActivation, Clipboard, DisplayId, DisplaySnapshot, ExternalDragEvent,
    ExternalDragResponse, FileDialogError, FileDialogOutcome, FileDialogRequest,
    FileDialogRequestId, LaunchActivation, NativeOperationCompletion, NativeRequestId,
    PlatformCapabilities, PlatformEvent, PlatformLifecycle, PlatformOperationError,
    PlatformOperationErrorKind, PlatformOperationResult, SingleInstancePolicy, TextInputCommand,
    WindowCommand, WindowEvent, WindowEventKind, WindowId, WindowLifecycle, WindowOperation,
    WindowOptions,
};
use incular_rendering::DisplayList;
use incular_widgets::internal::{Key, MouseCursor, PlatformMenuBinding, TreeError};
use incular_widgets::{RouteInformation, RouteInformationProvider, Widget};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet, VecDeque},
    rc::Rc,
    sync::{Arc, RwLock, atomic::Ordering, mpsc},
    time::Instant,
};
use tokio::sync::oneshot;

struct PendingNativeRequest {
    window_id: WindowId,
    sender: oneshot::Sender<PlatformOperationResult>,
}

struct PendingFileDialogRequest {
    window_id: WindowId,
    request: FileDialogRequest,
    sender: Option<oneshot::Sender<Result<FileDialogOutcome, FileDialogError>>>,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
}

/// Owns application services, one Tokio runtime, and multiple retained roots.
/// A root's environment, input/focus, semantics, compositor, frame work, and
/// cancellation scope are window-local; assets, Signals, and Tokio are shared.
pub struct Application {
    pub(crate) scheduler: Rc<RefCell<tasks::TaskScheduler>>,
    pub(crate) registry: Rc<RefCell<WindowRegistry>>,
    pub(crate) manager: WindowManager,
    pub(crate) command_receiver: mpsc::Receiver<QueuedWindowCommand>,
    pub(crate) request_cancellation_receiver: mpsc::Receiver<NativeRequestId>,
    pending_native_requests: RequestRegistry<NativeRequestId, PendingNativeRequest>,
    file_dialog_receiver: mpsc::Receiver<QueuedFileDialogRequest>,
    file_dialog_cancellation_receiver: mpsc::Receiver<FileDialogRequestId>,
    file_dialog_bridge: Arc<FileDialogBridge>,
    pending_file_dialog_requests: RequestRegistry<FileDialogRequestId, PendingFileDialogRequest>,
    active_file_dialog_requests: HashMap<WindowId, FileDialogRequestId>,
    queued_file_dialog_requests: HashMap<WindowId, VecDeque<FileDialogRequestId>>,
    native_file_dialog_requests: VecDeque<NativeFileDialogRequest>,
    global_shortcut_receiver: mpsc::Receiver<QueuedGlobalShortcutRequest>,
    global_shortcut_bridge: Arc<GlobalShortcutBridge>,
    pending_global_shortcut_requests: RequestRegistry<
        crate::global_shortcuts::GlobalShortcutRequestId,
        PendingGlobalShortcutRequest,
    >,
    native_global_shortcut_requests: VecDeque<NativeGlobalShortcutRequest>,
    platform_capabilities: Arc<RwLock<PlatformCapabilities>>,
    application_shell: ApplicationShellService,
    activations: ApplicationActivationService,
    launch_activation: LaunchActivation,
    single_instance_policy: Option<SingleInstancePolicy>,
    display_catalog: Arc<RwLock<DisplayCatalog>>,
    pub(crate) simulation_receiver: mpsc::Receiver<simulation::SimulationRequest>,
    pub(crate) simulation_bridge: Arc<simulation::SimulationBridge>,
    pub(crate) simulation_frame_waiters:
        HashMap<WindowId, Vec<mpsc::SyncSender<Result<(), SimulationError>>>>,
    pub(crate) simulation_capture_waiters:
        HashMap<WindowId, Vec<mpsc::SyncSender<Result<Screenshot, SimulationError>>>>,
    pub(crate) native_commands: Rc<RefCell<VecDeque<NativeWindowCommand>>>,
    pub(crate) primary_window: WindowId,
    pub(crate) last_window_policy: LastWindowPolicy,
    pub(crate) should_exit: bool,
    pub(crate) close_request: Option<Box<dyn FnMut(WindowId) -> bool>>,
    pub(crate) restoration: Option<restoration::RestorationManager>,
    pub(crate) restoration_window_factories:
        HashMap<String, (WindowOptions, RestorableWindowFactory)>,
    pub(crate) profiler: PerformanceProfiler,
    pub(crate) hub: PerformanceHub,
    pub(crate) scheduler_counters: ApplicationSchedulerCounters,
}
impl Drop for Application {
    fn drop(&mut self) {
        self.application_shell.stop();
    }
}
impl Application {
    /// Creates an application from a retained root builder.
    ///
    /// The builder runs during construction and may run again whenever state
    /// or environment values read by the builder change. Keep the builder
    /// free of external side effects such as network requests, file writes,
    /// or analytics; put those operations in event callbacks or tasks.
    pub fn new(
        build: impl FnMut(&mut BuildContext) -> Widget + 'static,
    ) -> Result<Self, TreeError> {
        Self::new_with_options(WindowOptions::default(), build).map_err(|error| match error {
            WindowError::Tree(error) => error,
            WindowError::Options(_)
            | WindowError::Restoration(_)
            | WindowError::ApplicationStopped => {
                unreachable!("default application options are valid while constructing")
            }
        })
    }

    /// Builds the initial retained root with portable native-window options.
    pub fn new_with_options(
        options: WindowOptions,
        build: impl FnMut(&mut BuildContext) -> Widget + 'static,
    ) -> Result<Self, WindowError> {
        Self::new_inner(options, None, None, build)
    }

    /// Creates an application whose explicitly selected values can survive a
    /// restart. Restoration loads synchronously before the initial root build;
    /// a corrupt, incompatible, or unmigratable snapshot is reported through
    /// diagnostics and simply exposes application defaults instead.
    pub fn new_with_restoration(
        options: WindowOptions,
        restoration: RestorationConfig,
        build: impl FnMut(&mut BuildContext) -> Widget + 'static,
    ) -> Result<Self, WindowError> {
        Self::new_inner(options, Some(restoration), None, build)
    }

    /// Creates a restoration-enabled application whose primary native window
    /// also has a stable application identity. Use this instead of
    /// [`Self::new_with_restoration`] when its logical size/fullscreen state
    /// should be reconstructed across launches.
    pub fn new_restorable(
        restoration_id: WindowRestorationId,
        kind: impl Into<String>,
        options: WindowOptions,
        restoration: RestorationConfig,
        build: impl FnMut(&mut BuildContext) -> Widget + 'static,
    ) -> Result<Self, WindowError> {
        Self::new_inner(
            options,
            Some(restoration),
            Some(RestorableWindowMetadata::new(restoration_id, kind)?),
            build,
        )
    }

    fn new_inner(
        options: WindowOptions,
        restoration_config: Option<RestorationConfig>,
        primary_restoration: Option<RestorableWindowMetadata>,
        build: impl FnMut(&mut BuildContext) -> Widget + 'static,
    ) -> Result<Self, WindowError> {
        Self::with_primary(tasks::TaskScheduler::new(), restoration_config, |manager| {
            manager
                .open_window_with_inner(options, build, primary_restoration)
                .map(|handle| handle.id())
        })
    }

    /// Adopts an existing standalone runtime without remounting its tree or
    /// replacing its task scheduler. The action callback observes legacy action
    /// IDs on the UI thread; native event handling uses the application host.
    /// Existing reactive builders, focus and queued task completions are retained.
    pub fn from_runtime(
        mut runtime: Runtime,
        on_action: impl FnMut(incular_widgets::internal::ActionId) + 'static,
    ) -> Self {
        runtime.legacy_action_handler = Some(Box::new(on_action));
        Self::with_primary(runtime.scheduler.clone(), None, |manager| {
            Ok(manager.adopt_runtime(runtime))
        })
        .expect("adopting a mounted runtime with default window options is infallible")
    }

    fn with_primary(
        scheduler: Rc<RefCell<tasks::TaskScheduler>>,
        restoration_config: Option<RestorationConfig>,
        create: impl FnOnce(&WindowManager) -> Result<WindowId, WindowError>,
    ) -> Result<Self, WindowError> {
        let restoration = restoration_config.map(restoration::RestorationManager::load);
        if let Some(restoration) = &restoration {
            restoration.attach_scheduler(&scheduler);
        }
        let registry = Rc::new(RefCell::new(WindowRegistry::default()));
        let native_commands = Rc::new(RefCell::new(VecDeque::new()));
        let (sender, command_receiver) = mpsc::channel();
        let (cancellation_sender, request_cancellation_receiver) = mpsc::channel();
        let (file_dialog_sender, file_dialog_receiver) = mpsc::channel();
        let (file_dialog_cancellation_sender, file_dialog_cancellation_receiver) = mpsc::channel();
        let file_dialog_bridge = Arc::new(FileDialogBridge::new(
            file_dialog_sender,
            file_dialog_cancellation_sender,
        ));
        let (global_shortcut_sender, global_shortcut_receiver) = mpsc::channel();
        let global_shortcut_bridge = Arc::new(GlobalShortcutBridge::new(global_shortcut_sender));
        let (simulation_sender, simulation_receiver) = mpsc::channel();
        let simulation_bridge = Arc::new(simulation::SimulationBridge::new(simulation_sender));
        let platform_capabilities = Arc::new(RwLock::new(PlatformCapabilities::default()));
        let application_shell = ApplicationShellService::new(platform_capabilities.clone());
        let activations = ApplicationActivationService::default();
        let display_catalog = Arc::new(RwLock::new(DisplayCatalog::default()));
        let application_lifecycle = Rc::new(std::cell::Cell::new(
            crate::application_types::ApplicationLifecycle::Starting,
        ));
        let bridge = Arc::new(WindowCommandBridge::new(sender, cancellation_sender));
        let manager = WindowManager {
            registry: Rc::downgrade(&registry),
            scheduler: scheduler.clone(),
            bridge,
            file_dialog_bridge: file_dialog_bridge.clone(),
            global_shortcut_bridge: global_shortcut_bridge.clone(),
            application_shell: application_shell.clone(),
            native_commands: native_commands.clone(),
            restoration: restoration.clone(),
            application_capabilities: platform_capabilities.clone(),
            displays: display_catalog.clone(),
            application_lifecycle,
        };
        let primary_window = create(&manager)?;
        Ok(Self {
            scheduler,
            registry,
            manager,
            command_receiver,
            request_cancellation_receiver,
            pending_native_requests: RequestRegistry::new(),
            file_dialog_receiver,
            file_dialog_cancellation_receiver,
            file_dialog_bridge,
            pending_file_dialog_requests: RequestRegistry::new(),
            active_file_dialog_requests: HashMap::new(),
            queued_file_dialog_requests: HashMap::new(),
            native_file_dialog_requests: VecDeque::new(),
            global_shortcut_receiver,
            global_shortcut_bridge,
            pending_global_shortcut_requests: RequestRegistry::new(),
            native_global_shortcut_requests: VecDeque::new(),
            platform_capabilities,
            application_shell,
            activations,
            launch_activation: LaunchActivation::current_process(),
            single_instance_policy: None,
            display_catalog,
            simulation_receiver,
            simulation_bridge,
            simulation_frame_waiters: HashMap::new(),
            simulation_capture_waiters: HashMap::new(),
            native_commands,
            primary_window,
            last_window_policy: LastWindowPolicy::ExitOnLastWindow,
            should_exit: false,
            close_request: None,
            restoration,
            restoration_window_factories: HashMap::new(),
            profiler: PerformanceProfiler::new(ProfilerMode::Normal),
            hub: PerformanceHub::new(),
            scheduler_counters: ApplicationSchedulerCounters::default(),
        })
    }

    /// Adds an independent retained root. Native creation is deferred until a
    /// desktop adapter reaches an active Winit event-loop callback.
    pub fn open_window(
        &mut self,
        options: WindowOptions,
        root: Widget,
    ) -> Result<WindowHandle, WindowError> {
        self.manager.open_window(options, root)
    }

    /// Adds an independent declarative root with its own window environment
    /// and cancellation scope.
    pub fn open_window_with(
        &mut self,
        options: WindowOptions,
        build: impl FnMut(&mut BuildContext) -> Widget + 'static,
    ) -> Result<WindowHandle, WindowError> {
        self.manager.open_window_with(options, build)
    }

    /// Opens an auxiliary window with a stable restoration ID. A normal user
    /// close removes this descriptor from the next session; application
    /// shutdown preserves currently active restorable windows.
    pub fn open_restorable_window_with(
        &mut self,
        restoration_id: WindowRestorationId,
        kind: impl Into<String>,
        options: WindowOptions,
        build: impl FnMut(&mut BuildContext) -> Widget + 'static,
    ) -> Result<WindowHandle, WindowError> {
        self.manager
            .open_restorable_window_with(restoration_id, kind, options, build)
    }

    /// Registers an in-memory factory for one stable auxiliary window kind.
    /// Builders remain ordinary closures and are never stored in a snapshot.
    pub fn register_restorable_window_factory(
        &mut self,
        kind: impl Into<String>,
        options: WindowOptions,
        factory: impl Fn(&mut BuildContext) -> Widget + 'static,
    ) -> Result<(), WindowError> {
        let kind = kind.into();
        if kind.trim().is_empty() || kind.chars().any(char::is_control) {
            return Err(WindowError::Restoration(
                "window factory kind must be a non-empty printable stable identifier".to_owned(),
            ));
        }
        self.restoration_window_factories
            .insert(kind, (options, Rc::new(factory)));
        Ok(())
    }

    /// Restores all persisted auxiliary windows whose stable factories have
    /// been registered. Call this after registration and before entering the
    /// native runner; it therefore cannot flash a default auxiliary UI before
    /// restore. Unknown kinds are skipped safely and counted in diagnostics.
    pub fn restore_restorable_windows(&mut self) -> Result<usize, WindowError> {
        let Some(restoration) = self.restoration.clone() else {
            return Ok(0);
        };
        let active = self
            .registry
            .borrow()
            .slots
            .iter()
            .filter_map(|slot| slot.record.as_ref())
            .filter_map(|record| record.restoration.as_ref())
            .map(|metadata| metadata.id.as_key().as_str().to_owned())
            .collect::<HashSet<_>>();
        let descriptors = restoration.windows();
        let mut restored = 0;
        for descriptor in descriptors {
            if active.contains(&descriptor.restoration_id) {
                continue;
            }
            let Some((options, factory)) = self
                .restoration_window_factories
                .get(&descriptor.kind)
                .cloned()
            else {
                restoration.note_skipped_window();
                continue;
            };
            let restoration_id = WindowRestorationId::new(descriptor.restoration_id)
                .map_err(|error| WindowError::Restoration(error.to_string()))?;
            let kind = descriptor.kind;
            let factory = factory.clone();
            self.manager
                .open_restorable_window_with(restoration_id, kind, options, move |cx| {
                    factory(cx)
                })?;
            restored += 1;
        }
        Ok(restored)
    }

    #[must_use]
    pub const fn primary_window(&self) -> WindowId {
        self.primary_window
    }

    #[must_use]
    pub fn active_window_ids(&self) -> Vec<WindowId> {
        self.registry.borrow().ids()
    }

    /// Returns a thread-safe command handle for one live Incular window.
    #[must_use]
    pub fn window_handle(&self, window_id: WindowId) -> Option<WindowHandle> {
        self.manager.handle(window_id)
    }

    /// Effective retained cursor for the window's current hover position.
    /// Native adapters use this after input and frame updates; applications
    /// should normally express cursor intent through `MouseRegion` instead.
    #[doc(hidden)]
    #[must_use]
    pub fn window_mouse_cursor(&self, window_id: WindowId) -> Option<MouseCursor> {
        self.registry
            .borrow()
            .get(window_id)
            .map(|record| record.runtime.effective_mouse_cursor())
    }

    /// Returns the default application-menu bridges mounted in one retained
    /// window root. Desktop adapters bind them to their native menu delegate.
    #[doc(hidden)]
    #[must_use]
    pub fn platform_menu_bindings(&self, window_id: WindowId) -> Vec<PlatformMenuBinding> {
        self.registry
            .borrow()
            .get(window_id)
            .map_or_else(Vec::new, |record| record.runtime.platform_menu_bindings())
    }

    /// Returns the latest backend capability snapshot for application-level
    /// services. Before a native backend attaches, fields are `Unknown`.
    #[must_use]
    pub fn platform_capabilities(&self) -> PlatformCapabilities {
        *self
            .platform_capabilities
            .read()
            .expect("application capability snapshot lock")
    }

    /// Returns the application-scoped activation stream. Native open-file,
    /// deep-link, reopen, secondary-launch, and global-shortcut events all use
    /// this one ordered delivery path rather than a window input queue.
    #[must_use]
    pub fn activations(&self) -> ApplicationActivationService {
        self.activations.clone()
    }

    /// Returns the application-scoped native global shortcut service.
    #[must_use]
    pub fn global_shortcuts(&self) -> GlobalShortcutService {
        GlobalShortcutService::new(
            self.global_shortcut_bridge.clone(),
            self.platform_capabilities.clone(),
        )
    }

    /// Returns the application-owned native shell service. Tray/status items,
    /// notifications, and taskbar/Dock state are independent of retained widget
    /// lifetime and may therefore be created before or after windows.
    #[must_use]
    pub fn application_shell(&self) -> ApplicationShellService {
        self.application_shell.clone()
    }

    #[doc(hidden)]
    pub fn take_native_application_shell_requests(&self) -> Vec<NativeApplicationShellRequest> {
        self.application_shell.take_native_requests()
    }

    #[doc(hidden)]
    pub fn complete_application_shell_request(&self, completion: NativeApplicationShellCompletion) {
        self.application_shell.complete(completion);
    }

    #[doc(hidden)]
    pub fn handle_application_shell_event(&self, event: NativeApplicationShellEvent) -> bool {
        self.application_shell.handle_native_event(event)
    }

    /// Replaces the launch payload that the desktop runner publishes for this
    /// process. This is useful for custom launchers which already classified
    /// document or URL arguments and want to preserve those semantics.
    pub fn set_launch_activation(&mut self, activation: LaunchActivation) {
        self.launch_activation = activation;
    }

    #[doc(hidden)]
    #[must_use]
    pub fn launch_activation(&self) -> LaunchActivation {
        self.launch_activation.clone()
    }

    /// Enables or disables optional single-instance forwarding. Incular never
    /// enforces this policy unless the application explicitly supplies one.
    pub fn set_single_instance_policy(&mut self, policy: Option<SingleInstancePolicy>) {
        self.single_instance_policy = policy;
    }

    #[must_use]
    pub fn single_instance_policy(&self) -> Option<&SingleInstancePolicy> {
        self.single_instance_policy.as_ref()
    }

    /// Connects URL activations to an existing Router platform-route provider.
    ///
    /// The application supplies URL-to-route interpretation. This keeps deep
    /// link parsing in the route layer and prevents desktop adapters from
    /// acquiring application navigation policy.
    #[must_use]
    pub fn bridge_activation_routes(
        &self,
        provider: Rc<dyn RouteInformationProvider>,
        map: impl Fn(&str) -> Option<RouteInformation> + 'static,
    ) -> ActivationRouteBridge {
        ActivationRouteBridge::new(&self.activations, provider, map)
    }

    /// Publishes a normalized activation from a native/application host. This
    /// method is public only for platform embedders and deterministic tests;
    /// normal application code observes [`Self::activations`] instead.
    #[doc(hidden)]
    pub fn handle_application_activation(&mut self, activation: ApplicationActivation) {
        self.activations.publish(activation);
    }

    /// Replaces the backend's immutable connected-display snapshot.
    ///
    /// Display IDs are allocated/generationally managed by the native backend.
    /// Replacing the snapshot immediately invalidates removed IDs at this API
    /// boundary; a later display using the same slot must have a new generation.
    /// Replaces the native backend's immutable display snapshot catalog.
    ///
    /// This is public for sibling platform adapters and headless/test hosts; UI
    /// code should read displays through [`Application::displays`] or a
    /// [`WindowHandle`](crate::WindowHandle), not manufacture native topology.
    #[doc(hidden)]
    pub fn publish_displays(
        &mut self,
        displays: impl IntoIterator<Item = DisplaySnapshot>,
        primary: Option<DisplayId>,
    ) {
        self.display_catalog
            .write()
            .expect("display catalog lock")
            .replace(displays, primary);
    }

    #[must_use]
    pub fn displays(&self) -> Vec<DisplaySnapshot> {
        self.display_catalog
            .read()
            .expect("display catalog lock")
            .all()
    }

    #[must_use]
    pub fn display(&self, id: DisplayId) -> Option<DisplaySnapshot> {
        self.display_catalog
            .read()
            .expect("display catalog lock")
            .get(id)
    }

    #[must_use]
    pub fn primary_display(&self) -> Option<DisplaySnapshot> {
        self.display_catalog
            .read()
            .expect("display catalog lock")
            .primary()
    }

    #[must_use]
    pub fn current_display(&self, window_id: WindowId) -> Option<DisplaySnapshot> {
        let display_id = self
            .window_handle(window_id)?
            .observed_state()
            .current_display?;
        self.display(display_id)
    }

    /// Publishes one backend/session capability snapshot and applies it to all
    /// currently live windows. Custom embedders may call this before entering
    /// their native event loop.
    pub fn set_platform_capabilities(&mut self, capabilities: PlatformCapabilities) {
        *self
            .platform_capabilities
            .write()
            .expect("application capability snapshot lock") = capabilities;
        for slot in &self.registry.borrow().slots {
            if let Some(record) = slot.record.as_ref() {
                *record
                    .capabilities
                    .write()
                    .expect("window capability snapshot lock") = capabilities;
            }
        }
    }

    /// Returns the capability snapshot associated with one live window.
    #[must_use]
    pub fn window_capabilities(&self, window_id: WindowId) -> Option<PlatformCapabilities> {
        let registry = self.registry.borrow();
        let record = registry.get(window_id)?;
        Some(
            *record
                .capabilities
                .read()
                .expect("window capability snapshot lock"),
        )
    }

    /// Overrides capabilities for one live window when native support depends
    /// on that concrete surface/session rather than the application backend.
    pub fn set_window_capabilities(
        &mut self,
        window_id: WindowId,
        capabilities: PlatformCapabilities,
    ) -> bool {
        let registry = self.registry.borrow();
        let Some(record) = registry.get(window_id) else {
            return false;
        };
        *record
            .capabilities
            .write()
            .expect("window capability snapshot lock") = capabilities;
        true
    }

    /// Publishes the native adapter's resolved presentation for the visible
    /// semantic transients in one retained window. Custom embedders should
    /// replace the complete snapshot after each host-negotiation pass.
    pub fn set_transient_presentations(
        &mut self,
        window_id: WindowId,
        presentations: Vec<TransientPresentationResolution>,
    ) -> bool {
        self.with_window_mut(window_id, |record| {
            let native = presentations
                .iter()
                .filter_map(|presentation| {
                    (presentation.resolved == ResolvedTransientPresentation::Native)
                        .then_some(presentation.id)
                })
                .collect::<Vec<_>>();
            record.runtime.set_native_transient_presentations(native);
            *record
                .transient_presentations
                .write()
                .expect("transient presentation snapshot lock") = presentations;
        })
        .is_some()
    }

    /// Publishes the desktop bounds a confirmed native transient may use for
    /// collision placement. Bounds are parent-view-local logical geometry.
    pub fn set_native_transient_bounds(
        &mut self,
        window_id: WindowId,
        bounds: Option<incular_core::Rect>,
    ) -> bool {
        self.with_window_mut(window_id, |record| {
            record.runtime.set_native_transient_bounds(bounds)
        })
        .unwrap_or(false)
    }

    /// Development-only, read-only view of application windows.  Keeping this
    /// extraction here means platform runners never need to reach into the
    /// retained-window registry.
    #[cfg(feature = "devtools")]
    #[must_use]
    pub fn devtools_windows(&self) -> Vec<incular_devtools_protocol::WindowSummary> {
        self.active_window_ids()
            .into_iter()
            .filter_map(|id| {
                self.window_diagnostics(id)
                    .map(|window| incular_devtools_protocol::WindowSummary {
                        id: incular_devtools_protocol::DevWindowId::new(
                            u64::from(id.index()) + 1,
                            u64::from(id.generation()),
                        ),
                        title: window.title,
                        logical_size: [window.logical_size.width, window.logical_size.height],
                        scale_factor: window.scale_factor,
                    })
            })
            .collect()
    }

    /// Extracts one window's retained widget tree for DevTools.  The method
    /// performs no layout, paint, or allocation proportional to the entire
    /// application beyond the explicit bounded protocol snapshot.
    #[cfg(feature = "devtools")]
    pub fn devtools_widget_tree(
        &self,
        window: incular_devtools_protocol::DevWindowId,
    ) -> Result<Vec<incular_devtools_protocol::TreeDelta>, incular_devtools_protocol::ErrorCode>
    {
        let id = WindowId::from_parts(
            window.index().saturating_sub(1) as u32,
            window.generation() as u32,
        );
        let registry = self.registry.borrow();
        let record = registry
            .get(id)
            .ok_or(incular_devtools_protocol::ErrorCode::StaleId)?;
        let root = record
            .runtime
            .tree()
            .root()
            .ok_or(incular_devtools_protocol::ErrorCode::UnknownId)?;
        let (mut nodes, truncated) = record.runtime.tree().devtools_snapshot(root, false);
        let root = nodes
            .first()
            .cloned()
            .ok_or(incular_devtools_protocol::ErrorCode::UnknownId)?;
        nodes.remove(0);
        Ok(vec![incular_devtools_protocol::TreeDelta::Snapshot {
            window,
            root: Box::new(root),
            nodes,
            truncated,
        }])
    }

    /// Returns curated details for a live DevTools element id, rejecting stale
    /// ids before touching the retained tree.
    #[cfg(feature = "devtools")]
    pub fn devtools_node_details(
        &self,
        id: incular_devtools_protocol::DevWidgetId,
    ) -> Result<incular_devtools_protocol::NodeDetails, incular_devtools_protocol::ErrorCode> {
        for window in self.active_window_ids() {
            let registry = self.registry.borrow();
            let Some(record) = registry.get(window) else {
                continue;
            };
            let tree = record.runtime.tree();
            if let Some(element) = tree.devtools_resolve_id(id) {
                let mut details = tree
                    .devtools_node_details(
                        element,
                        id,
                        incular_devtools_protocol::DevWindowId::new(
                            u64::from(window.index()) + 1,
                            u64::from(window.generation()),
                        ),
                    )
                    .ok_or(incular_devtools_protocol::ErrorCode::StaleId)?;
                let root = record.runtime.devtools_reactive_root();
                details.consumed_signals = crate::devtools_registry::with_all(|signals| {
                    signals
                        .iter()
                        .filter(|(_, registration)| {
                            (registration.subscribers)().into_iter().any(
                                |(subscriber_root, subscriber)| {
                                    subscriber_root == root && subscriber == element
                                },
                            )
                        })
                        .map(|(signal_id, _)| {
                            incular_devtools_protocol::DevSignalId::new(*signal_id, 1)
                        })
                        .collect()
                });
                return Ok(details);
            }
        }
        Err(incular_devtools_protocol::ErrorCode::StaleId)
    }

    /// Applies a supported temporary property override to one live retained
    /// node. Generational IDs make stale edits fail closed.
    #[cfg(feature = "devtools")]
    pub fn devtools_edit_property(
        &mut self,
        id: incular_devtools_protocol::DevWidgetId,
        name: &str,
        value: &incular_devtools_protocol::DebugValue,
    ) -> bool {
        let mut registry = self.registry.borrow_mut();
        for slot in &mut registry.slots {
            let Some(record) = slot.record.as_mut() else {
                continue;
            };
            if record.runtime.devtools_edit_property(id, name, value) {
                return true;
            }
        }
        false
    }

    /// Reads the exact world-space bounds for the selected-widget overlay.
    #[cfg(feature = "devtools")]
    pub fn devtools_node_bounds(
        &self,
        id: incular_devtools_protocol::DevWidgetId,
    ) -> Option<[f32; 4]> {
        for window in self.active_window_ids() {
            let registry = self.registry.borrow();
            let Some(record) = registry.get(window) else {
                continue;
            };
            let tree = record.runtime.tree();
            if let Some(element) = tree.devtools_resolve_id(id) {
                return tree.element_bounds(element).map(|bounds| {
                    [
                        bounds.origin.x,
                        bounds.origin.y,
                        bounds.size.width,
                        bounds.size.height,
                    ]
                });
            }
        }
        None
    }

    /// Read-only retained geometry for selected-node debug adornments.
    #[cfg(feature = "devtools")]
    pub fn devtools_node_overlay_geometry(
        &self,
        id: incular_devtools_protocol::DevWidgetId,
    ) -> Option<incular_widgets::devtools::DevOverlayGeometry> {
        for window in self.active_window_ids() {
            let registry = self.registry.borrow();
            let Some(record) = registry.get(window) else {
                continue;
            };
            let tree = record.runtime.tree();
            if let Some(element) = tree.devtools_resolve_id(id) {
                return tree.devtools_overlay_geometry(element);
            }
        }
        None
    }

    /// Bounded exact retained layout rectangles for one target window. The
    /// platform adapter consumes these in its compositor-only overlay pass;
    /// no widget, layout or semantics state changes as a consequence.
    #[cfg(feature = "devtools")]
    pub fn devtools_window_layout_bounds(&self, window: WindowId, limit: usize) -> Vec<[f32; 4]> {
        self.registry
            .borrow()
            .get(window)
            .map(|record| record.runtime.tree().devtools_layout_bounds(limit))
            .unwrap_or_default()
    }

    #[cfg(feature = "devtools")]
    pub fn devtools_window_semantics_bounds(
        &self,
        window: WindowId,
        limit: usize,
    ) -> Vec<[f32; 4]> {
        self.registry
            .borrow()
            .get(window)
            .map(|record| record.runtime.tree().devtools_semantics_bounds(limit))
            .unwrap_or_default()
    }

    #[cfg(feature = "devtools")]
    pub fn devtools_window_hit_regions(&self, window: WindowId, limit: usize) -> Vec<[f32; 4]> {
        self.registry
            .borrow()
            .get(window)
            .map(|record| record.runtime.tree().devtools_hit_regions(limit))
            .unwrap_or_default()
    }

    #[cfg(feature = "devtools")]
    pub fn devtools_window_scroll_viewports(
        &self,
        window: WindowId,
        limit: usize,
    ) -> Vec<[f32; 4]> {
        self.registry
            .borrow()
            .get(window)
            .map(|record| record.runtime.tree().devtools_scroll_viewports(limit))
            .unwrap_or_default()
    }

    #[cfg(feature = "devtools")]
    pub fn devtools_window_layer_bounds(&self, window: WindowId, limit: usize) -> Vec<[f32; 4]> {
        self.registry
            .borrow()
            .get(window)
            .map(|record| record.runtime.tree().devtools_layer_bounds(limit))
            .unwrap_or_default()
    }

    /// Bounded existing per-node work counters for target-side phase flashes.
    #[cfg(feature = "devtools")]
    pub fn devtools_window_phase_nodes(
        &self,
        window: WindowId,
        limit: usize,
    ) -> Vec<incular_widgets::devtools::DevPhaseNode> {
        self.registry
            .borrow()
            .get(window)
            .map(|record| record.runtime.tree().devtools_phase_nodes(limit))
            .unwrap_or_default()
    }

    /// Enables one bounded per-node Deep trace for the next frame of this
    /// window. Normal/Performance profiler modes never call this, so the hot
    /// path has only compile-time-gated instrumentation checks.
    #[cfg(feature = "devtools")]
    pub fn devtools_begin_deep_trace(&mut self, window: WindowId, max_events: usize) -> bool {
        self.with_window_mut(window, |record| {
            record.runtime.tree_mut().begin_deep_trace(max_events);
        })
        .is_some()
    }

    /// Takes the completed per-node trace without cloning its event buffer.
    #[cfg(feature = "devtools")]
    pub fn devtools_take_deep_trace(
        &mut self,
        window: WindowId,
        frame: u64,
    ) -> Option<incular_devtools_protocol::DeepFrameTrace> {
        let (events, dropped_events) = self
            .with_window_mut(window, |record| record.runtime.tree_mut().take_deep_trace())
            .flatten()?;
        Some(incular_devtools_protocol::DeepFrameTrace {
            window: incular_devtools_protocol::DevWindowId::new(
                u64::from(window.index()) + 1,
                u64::from(window.generation()),
            ),
            frame,
            truncated: dropped_events > 0,
            dropped_events,
            events,
        })
    }

    /// Exact retained subtree bounds for the selected DevTools node, together
    /// with its owning native window. Stale IDs return no result.
    #[cfg(feature = "devtools")]
    pub fn devtools_subtree_layout_bounds(
        &self,
        id: incular_devtools_protocol::DevWidgetId,
        limit: usize,
    ) -> Option<(WindowId, Vec<[f32; 4]>)> {
        for window in self.active_window_ids() {
            let registry = self.registry.borrow();
            let Some(record) = registry.get(window) else {
                continue;
            };
            let tree = record.runtime.tree();
            if let Some(element) = tree.devtools_resolve_id(id) {
                return Some((window, tree.devtools_subtree_layout_bounds(element, limit)));
            }
        }
        None
    }

    /// Hit-tests a retained root for Select Widget mode without dispatching
    /// the pointer to application widgets.
    #[cfg(feature = "devtools")]
    pub fn devtools_hit_test(
        &self,
        window: WindowId,
        point: incular_core::Offset,
    ) -> Option<(incular_devtools_protocol::DevWidgetId, [f32; 4])> {
        let registry = self.registry.borrow();
        let record = registry.get(window)?;
        let tree = record.runtime.tree();
        let element = tree.devtools_deepest_at(point)?;
        let id = tree.devtools_id_for_element(element)?;
        let bounds = tree.element_bounds(element)?;
        Some((
            id,
            [
                bounds.origin.x,
                bounds.origin.y,
                bounds.size.width,
                bounds.size.height,
            ],
        ))
    }

    /// Framework-owned memory/resource inventory.  This is deliberately a
    /// count of retained Incular objects, not a misleading attempt to inspect
    /// Rust's allocator or another crate's heap.
    #[cfg(feature = "devtools")]
    #[must_use]
    pub fn devtools_resource_counts(&self) -> incular_devtools_protocol::ResourceCounts {
        let mut counts = incular_devtools_protocol::ResourceCounts::default();
        for window in self.active_window_ids() {
            let registry = self.registry.borrow();
            let Some(record) = registry.get(window) else {
                continue;
            };
            let tree = record.runtime.tree();
            counts.elements += tree.element_count();
            counts.render_objects += tree.render_object_count();
            counts.semantics_nodes += tree.semantics().len();
            counts.tasks_active += record.runtime.runtime_diagnostics().active_tracked_tasks;
        }
        crate::devtools_registry::with_all(|signals| counts.signals = signals.len());
        counts
    }

    /// Returns only signals which explicitly opted into DevTools visibility.
    /// Values are already reduced to bounded summaries at write time.
    #[cfg(feature = "devtools")]
    #[must_use]
    pub fn devtools_signals(&self) -> Vec<incular_devtools_protocol::SignalSummary> {
        crate::devtools_registry::with_all(|signals| {
            signals
                .iter()
                .map(|(id, registration)| {
                    let (_, last_write) = (registration.last_write)();
                    let writes = (registration.write_count)();
                    incular_devtools_protocol::SignalSummary {
                        id: incular_devtools_protocol::DevSignalId::new(*id, 1),
                        name: registration.name.clone(),
                        type_name: registration.type_name.to_owned(),
                        generation: writes,
                        write_count: writes,
                        subscriber_count: (registration.subscriber_count)(),
                        last_write_summary: last_write,
                        editable: registration.editable_kind.is_some(),
                    }
                })
                .collect()
        })
    }

    /// Resolves actual live reactive dependencies for a DevTools-visible
    /// signal.  The registry stores only weak signal state and Element ids;
    /// this method resolves those ids against the matching live window tree,
    /// so unmounted elements are never retained or reported.
    #[cfg(feature = "devtools")]
    #[must_use]
    pub fn devtools_signal_subscribers(
        &self,
        signal: incular_devtools_protocol::DevSignalId,
    ) -> Vec<incular_devtools_protocol::SignalSubscriber> {
        let entries = crate::devtools_registry::with_all(|signals| {
            signals
                .iter()
                .find(|(id, _)| *id == signal.index())
                .and_then(|(_, registration)| {
                    (signal.generation() == 1).then(|| (registration.subscribers)())
                })
                .unwrap_or_default()
        });
        if entries.is_empty() {
            return Vec::new();
        }

        let windows = self.active_window_ids();
        let registry = self.registry.borrow();
        windows
            .into_iter()
            .flat_map(|window| {
                let Some(record) = registry.get(window) else {
                    return Vec::new();
                };
                let root = record.runtime.devtools_reactive_root();
                let tree = record.runtime.tree();
                entries
                    .iter()
                    .filter(|(entry_root, _)| *entry_root == root)
                    .filter_map(|(_, element)| {
                        tree.devtools_id_for_element(*element).map(|dev_id| {
                            incular_devtools_protocol::SignalSubscriber {
                                signal,
                                element: dev_id,
                                path: tree.devtools_element_path(*element),
                            }
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    /// Applies an explicitly opted-in typed DevTools edit on the UI thread.
    /// The conversion and the normal `Signal::set` invalidation both live in
    /// the signal registration; unsupported types are rejected.
    #[cfg(feature = "devtools")]
    pub fn devtools_edit_signal(
        &self,
        signal: incular_devtools_protocol::DevSignalId,
        value: &incular_devtools_protocol::EditableValue,
    ) -> bool {
        if signal.generation() != 1 {
            return false;
        }
        crate::devtools_registry::with_all(|signals| {
            signals
                .iter()
                .find(|(id, registration)| {
                    *id == signal.index() && registration.editable_kind.is_some()
                })
                .is_some_and(|(_, registration)| (registration.apply_edit)(value))
        })
    }

    #[must_use]
    pub fn contains_window(&self, window_id: WindowId) -> bool {
        self.registry.borrow().contains(window_id)
    }

    /// Visible transient surfaces belonging to `window_id` after its latest
    /// retained layout. Geometry is window-local logical space and therefore
    /// remains anchored when the native parent moves; the platform adapter is
    /// responsible only for mapping that geometry into desktop coordinates.
    #[must_use]
    pub fn transient_surfaces(
        &self,
        window_id: WindowId,
    ) -> Vec<incular_widgets::TransientSurfaceSnapshot> {
        self.registry
            .borrow()
            .get(window_id)
            .map_or_else(Vec::new, |record| record.runtime.transient_surfaces())
    }

    pub fn set_last_window_policy(&mut self, policy: LastWindowPolicy) {
        self.last_window_policy = policy;
    }

    #[must_use]
    pub const fn last_window_policy(&self) -> LastWindowPolicy {
        self.last_window_policy
    }

    /// Installs the application policy used for a native close request. The
    /// callback returns `true` to accept closing and `false` to keep the
    /// window alive (for example, an unsaved-work dialog flow).
    pub fn on_close_request(&mut self, callback: impl FnMut(WindowId) -> bool + 'static) {
        self.close_request = Some(Box::new(callback));
    }

    pub fn set_wake_handler(&mut self, wake: Arc<dyn RuntimeWake>) {
        self.scheduler.borrow_mut().set_wake(wake.clone());
        self.manager.bridge.set_wake(wake.clone());
        self.file_dialog_bridge.set_wake(wake.clone());
        self.global_shortcut_bridge.set_wake(wake.clone());
        self.application_shell.set_wake(wake.clone());
        self.simulation_bridge.set_wake(wake);
    }

    /// Returns a cloneable, in-process controller for the primary window.
    /// Simulation commands are serviced by the native event loop and never
    /// synthesize OS-level mouse or keyboard input.
    #[must_use]
    pub fn simulation(&self) -> Simulation {
        Simulation::new(self.simulation_bridge.clone(), self.primary_window)
    }

    pub(crate) fn with_window_mut<R>(
        &mut self,
        window_id: WindowId,
        callback: impl FnOnce(&mut WindowRecord) -> R,
    ) -> Option<R> {
        let mut record = self.registry.borrow_mut().take(window_id)?;
        let result = callback(&mut record);
        // `callback` may have opened a different window through BuildContext,
        // so borrow the registry only after it returns.
        // The root being serviced remains generationally reserved throughout.
        self.registry.borrow_mut().restore(window_id, record);
        Some(result)
    }
    /// Starts application-scoped Tokio work. It is not cancelled when any
    /// individual window closes.
    pub fn spawn<F, T>(&self, future: F) -> Task<T>
    where
        F: std::future::Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        tasks::TaskScheduler::spawner(&self.scheduler).spawn(future)
    }

    pub fn spawn_blocking<T>(
        &self,
        work: impl FnOnce() -> T + Send + 'static,
        complete: impl FnOnce(Result<T, TaskFailure>, &mut Runtime) + 'static,
    ) -> TaskHandle
    where
        T: Send + 'static,
    {
        self.scheduler.borrow_mut().spawn_blocking(work, complete)
    }

    /// Queues a UI callback for the primary window. For work targeting another
    /// root, capture that root's `BuildContext` dispatcher instead.
    pub fn dispatch(&self, callback: impl FnOnce(&mut Runtime) + Send + 'static) {
        self.scheduler
            .borrow()
            .dispatcher_for(Some(self.primary_window))
            .dispatch(callback);
    }

    pub fn spawn_into<F, T>(
        &self,
        future: F,
        complete: impl FnOnce(Result<T, TaskFailure>, &mut Runtime) + 'static,
    ) -> TaskHandle
    where
        F: std::future::Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        tasks::TaskScheduler::spawner(&self.scheduler).spawn_into(future, complete)
    }

    #[must_use]
    pub fn tokio_handle(&self) -> TokioHandle {
        self.scheduler.borrow().tokio_handle()
    }

    /// Drains at most one bounded Tokio UI turn, routing each completion to
    /// the window scope that created it. Closed/reused IDs discard stale work.
    pub fn process_runtime_work_at(&mut self, _now: Instant) {
        let work = self.scheduler.borrow_mut().take_turn();
        for item in work {
            let requested_window = match &item {
                tasks::UiWork::Dispatch { window_id, .. } => *window_id,
                tasks::UiWork::Task(pending) => pending.control.window_id(),
            };
            // Application-scoped work (including restoration write completion)
            // is not owned by a particular root. If the original primary has
            // closed while another window remains, deliver it to that live UI
            // turn rather than discarding persistence bookkeeping.
            let window_id = requested_window.or_else(|| {
                if self.contains_window(self.primary_window) {
                    Some(self.primary_window)
                } else {
                    self.active_window_ids().into_iter().next()
                }
            });
            let Some(window_id) = window_id else {
                self.discard_stale_ui_work(item);
                continue;
            };
            let record = self.registry.borrow_mut().take(window_id);
            if let Some(mut record) = record {
                record.runtime.process_ui_work(&self.scheduler, item);
                self.registry.borrow_mut().restore(window_id, record);
            } else {
                self.discard_stale_ui_work(item);
            }
        }
        self.drain_window_commands();
        self.drain_native_request_cancellations();
        self.drain_file_dialog_requests();
        self.drain_file_dialog_cancellations();
        self.drain_global_shortcut_requests();
        self.process_simulation_requests();
    }

    pub fn process_runtime_work(&mut self) {
        self.process_runtime_work_at(Instant::now());
    }

    fn discard_stale_ui_work(&mut self, work: tasks::UiWork) {
        if let tasks::UiWork::Task(pending) = work {
            self.scheduler.borrow().detach_scope(&pending.control);
            (pending.discard)();
            self.scheduler
                .borrow_mut()
                .record_discard(pending.blocking, true);
        }
    }

    /// Routes external platform transfer data to one retained window and
    /// returns the operation requested by the currently hit-tested target.
    /// Native adapters with richer drag protocols can feed this response back
    /// to the source; adapters such as Winit file-drop may simply ignore it.
    #[must_use]
    pub fn handle_external_drag_event(
        &mut self,
        window_id: WindowId,
        event: ExternalDragEvent,
    ) -> ExternalDragResponse {
        self.with_window_mut(window_id, |record| {
            record.input_events = record.input_events.wrapping_add(1);
            record.runtime.handle_external_drag(event)
        })
        .unwrap_or_default()
    }

    /// Routes a normalized event to exactly one retained root.
    pub fn handle_window_event(&mut self, event: WindowEvent) {
        let window_id = event.window_id;
        match event.kind {
            WindowEventKind::Platform(PlatformEvent::CloseRequested) => {
                self.request_close(window_id);
            }
            WindowEventKind::Platform(PlatformEvent::Metrics(metrics)) => {
                let _ = self.with_window_mut(window_id, |record| {
                    if record.metrics != metrics {
                        record.metrics = metrics;
                        record.surface_generation = record.surface_generation.wrapping_add(1);
                        record.runtime.update_window_metrics(metrics);
                    }
                });
                self.manager.sync_restorable_windows(true);
            }
            WindowEventKind::Platform(PlatformEvent::Environment(environment)) => {
                let _ = self.with_window_mut(window_id, |record| {
                    record.runtime.set_environment(environment);
                });
            }
            WindowEventKind::Platform(PlatformEvent::Input(input)) => {
                let _ = self.with_window_mut(window_id, |record| {
                    record.input_events = record.input_events.wrapping_add(1);
                    let _ = record.runtime.handle_input(input);
                });
            }
            WindowEventKind::Platform(PlatformEvent::TextInputAction(action)) => {
                let _ = self.with_window_mut(window_id, |record| {
                    record.input_events = record.input_events.wrapping_add(1);
                    let _ = record.runtime.handle_text_input_action(action);
                });
            }
            WindowEventKind::Platform(PlatformEvent::Lifecycle(lifecycle)) => {
                self.handle_application_lifecycle(lifecycle);
            }
            WindowEventKind::Platform(PlatformEvent::ExternalDrag(event)) => {
                let _ = self.handle_external_drag_event(window_id, event);
            }
            WindowEventKind::Lifecycle(lifecycle) => {
                let _ = self.with_window_mut(window_id, |record| {
                    record.lifecycle = lifecycle;
                    match lifecycle {
                        WindowLifecycle::Focused => {
                            record.native_focused = true;
                            let mut environment = record.runtime.environment();
                            environment.window_focused = true;
                            record.runtime.set_environment(environment);
                        }
                        WindowLifecycle::Unfocused => {
                            record.native_focused = false;
                            record.runtime.dismiss_transients(
                                incular_widgets::TransientDismissReason::ParentDeactivated,
                            );
                            let mut environment = record.runtime.environment();
                            environment.window_focused = false;
                            record.runtime.set_environment(environment);
                        }
                        WindowLifecycle::Visible
                        | WindowLifecycle::Hidden
                        | WindowLifecycle::Creating
                        | WindowLifecycle::Closing
                        | WindowLifecycle::Closed => {}
                    }
                });
            }
            WindowEventKind::RedrawRequested => {}
            WindowEventKind::StateChanged(state) => {
                let _ = self.with_window_mut(window_id, |record| {
                    *record
                        .observed_state
                        .write()
                        .expect("window observed-state snapshot lock") = state;
                });
            }
        }
    }

    /// Applies one genuine application-level lifecycle transition to every
    /// retained window root. Window focus remains a separate per-window
    /// lifecycle channel and never synthesizes these application states.
    pub fn handle_application_lifecycle(&mut self, lifecycle: PlatformLifecycle) {
        self.manager.application_lifecycle.set(match lifecycle {
            PlatformLifecycle::Active | PlatformLifecycle::Resumed => {
                crate::application_types::ApplicationLifecycle::Active
            }
            PlatformLifecycle::Inactive => crate::application_types::ApplicationLifecycle::Inactive,
            PlatformLifecycle::Suspended => {
                crate::application_types::ApplicationLifecycle::Suspended
            }
            PlatformLifecycle::Stopping => crate::application_types::ApplicationLifecycle::Stopping,
        });
        if matches!(lifecycle, PlatformLifecycle::Stopping) {
            self.shutdown();
            return;
        }
        let ids = self.active_window_ids();
        for id in ids {
            let _ = self.with_window_mut(id, |record| {
                let _ = record
                    .runtime
                    .handle_platform_event(PlatformEvent::Lifecycle(lifecycle));
            });
        }
        if matches!(lifecycle, PlatformLifecycle::Suspended) {
            let _ = self.flush_restoration();
        }
    }

    /// Records a backend failure for a user-driven operation that does not
    /// have an explicit result consumer (for example a `WindowDragRegion`).
    pub fn record_platform_operation_error(
        &mut self,
        window_id: WindowId,
        error: PlatformOperationError,
    ) -> bool {
        self.with_window_mut(window_id, |record| {
            record.last_platform_error = Some(error);
        })
        .is_some()
    }

    pub fn set_window_environment(
        &mut self,
        window_id: WindowId,
        environment: RuntimeEnvironment,
    ) -> bool {
        self.with_window_mut(window_id, |record| {
            record.runtime.set_environment(environment)
        })
        .unwrap_or(false)
    }

    /// Installs a platform clipboard for one window's input/editing bridge.
    /// The clipboard service itself may be application-wide; the runtime keeps
    /// the mutable adapter at the window input boundary.
    pub fn set_window_clipboard(&mut self, window_id: WindowId, clipboard: Box<dyn Clipboard>) {
        let _ = self.with_window_mut(window_id, |record| {
            record.runtime.set_clipboard(clipboard);
        });
    }

    /// Drains text-input commands for one native window. The caller applies
    /// them on that window's platform/UI thread.
    pub fn take_window_text_input_commands(
        &mut self,
        window_id: WindowId,
    ) -> Vec<TextInputCommand> {
        self.with_window_mut(window_id, |record| {
            record.runtime.take_text_input_commands()
        })
        .unwrap_or_default()
    }

    #[must_use]
    pub fn frame_requested(&self, window_id: WindowId) -> bool {
        self.registry
            .borrow()
            .slots
            .get(window_id.index() as usize)
            .filter(|slot| slot.generation == window_id.generation())
            .and_then(|slot| slot.record.as_ref())
            .is_some_and(|record| record.runtime.frame_requested())
    }

    pub fn note_frame_requested(&mut self, window_id: WindowId) {
        scheduler_counters::REDRAW_REQUESTS.fetch_add(1, Ordering::Relaxed);
        self.scheduler_counters.redraw_requests =
            self.scheduler_counters.redraw_requests.wrapping_add(1);
        let _ = self.with_window_mut(window_id, |record| {
            record.requested_frames = record.requested_frames.wrapping_add(1);
        });
    }

    /// Records one UI-relevant runtime wake (async completion or message).
    /// Wakes that mutate no visible state must not produce redraws; the idle
    /// contract test relies on these counters staying independent.
    pub fn note_runtime_wake(&mut self) {
        scheduler_counters::RUNTIME_WAKES.fetch_add(1, Ordering::Relaxed);
        self.scheduler_counters.runtime_wakes =
            self.scheduler_counters.runtime_wakes.wrapping_add(1);
    }

    pub fn run_window_frame_at(
        &mut self,
        window_id: WindowId,
        constraints: Constraints,
        now: Instant,
    ) -> Result<Option<(DisplayList, FrameStats)>, TreeError> {
        let mut sensitivity_update = None;
        let mut content_size_request = None;
        let outcome = self
            .with_window_mut(window_id, |record| {
                if record.metrics.physical_size.is_zero() {
                    record.skipped_frames = record.skipped_frames.wrapping_add(1);
                    scheduler_counters::FRAMES_SKIPPED.fetch_add(1, Ordering::Relaxed);
                    Ok(None)
                } else {
                    let frame_constraints =
                        ContentSizeCoordinator::layout_constraints(&record.options, constraints);
                    record
                        .runtime
                        .run_frame_at(frame_constraints, now)
                        .map(|(list, stats)| {
                            let sensitivity = record.runtime.content_sensitivity();
                            if record.last_content_sensitivity != Some(sensitivity) {
                                record.last_content_sensitivity = Some(sensitivity);
                                sensitivity_update = Some(sensitivity);
                            }
                            content_size_request = record.content_sizing.reconcile(
                                &record.options,
                                record.metrics,
                                record.runtime.tree().root_layout_size(),
                            );
                            record.last_frame = FrameRecord {
                                frame: 0,
                                timings: stats.timings,
                                work: FrameWork {
                                    updated_elements: stats.updated_elements as u64,
                                    rebuilt_elements: stats.rebuilt_elements,
                                    laid_out_render_objects: stats.laid_out_render_objects,
                                    repainted_render_objects: stats.repainted_render_objects,
                                    composited_layers: stats.composited,
                                    active_animations: stats.active_animations,
                                    display_list_commands: stats.display_list_commands,
                                    requested_another_frame: stats.requested_another_frame,
                                },
                                over_budget: false,
                            };
                            Some((list, stats))
                        })
                }
            })
            .unwrap_or(Ok(None));
        if let Some(size) = content_size_request {
            self.native_commands
                .borrow_mut()
                .push_back(NativeWindowCommand::Operate(WindowCommand::new(
                    window_id,
                    WindowOperation::SetLogicalSize(size),
                )));
        }
        if let Some(sensitivity) = sensitivity_update {
            self.native_commands
                .borrow_mut()
                .push_back(NativeWindowCommand::Operate(WindowCommand::new(
                    window_id,
                    WindowOperation::SetContentSensitivity(sensitivity),
                )));
        }
        if matches!(outcome, Ok(None)) && self.contains_window(window_id) {
            self.scheduler_counters.frames_skipped =
                self.scheduler_counters.frames_skipped.wrapping_add(1);
        }
        if matches!(outcome, Ok(Some(_))) {
            scheduler_counters::FRAMES_STARTED.fetch_add(1, Ordering::Relaxed);
            self.scheduler_counters.frames_started =
                self.scheduler_counters.frames_started.wrapping_add(1);
        }
        outcome
    }

    /// Projects this window's retained semantics through its window-local
    /// AccessKit bridge. Native adapters call this on the UI/event-loop thread
    /// only after AccessKit activation; a changed semantic revision produces a
    /// full or incremental tree update while ordinary paint/compositor frames
    /// produce no work.
    pub fn sync_accessibility(
        &mut self,
        window_id: WindowId,
        projection: &mut AccessKitProjection,
    ) -> Option<NativeAccessibilityUpdate> {
        let update = self
            .with_window_mut(window_id, |record| {
                projection.sync(
                    record.runtime.tree().semantics(),
                    record.metrics.scale_factor,
                )
            })
            .flatten();
        let diagnostics = projection.diagnostics();
        let _ = self.with_window_mut(window_id, |record| {
            record.accessibility = diagnostics;
        });
        update
    }

    /// Receives an already validated native semantic request for exactly one
    /// window. It uses the ordinary retained runtime action dispatcher, so
    /// native callbacks never mutate widget/controller state directly.
    pub fn dispatch_accessibility_action(
        &mut self,
        window_id: WindowId,
        request: SemanticActionRequest,
    ) -> bool {
        self.with_window_mut(window_id, |record| {
            record
                .runtime
                .dispatch_semantic_action(request.node, request.action)
        })
        .unwrap_or(false)
    }

    /// Updates one window's value-free bridge diagnostics after a lifecycle or
    /// action event that did not emit a tree update.
    pub fn set_accessibility_diagnostics(
        &mut self,
        window_id: WindowId,
        diagnostics: AccessibilityDiagnostics,
    ) {
        let _ = self.with_window_mut(window_id, |record| {
            record.accessibility = diagnostics;
        });
    }

    /// Selects how much profiling state is retained. Normal keeps only
    /// counters and the latest frame; higher modes retain bounded history.
    pub fn set_profiler_mode(&mut self, mode: ProfilerMode) {
        self.profiler.set_mode(mode);
    }

    /// Applies the DevTools animation speed to every live window. This is a
    /// retained animation-clock setting, not a scheduler or Tokio setting.
    pub fn set_animation_time_scale(&mut self, scale: f32) {
        for window in self.active_window_ids() {
            let _ = self.with_window_mut(window, |record| {
                record.runtime.set_animation_time_scale(scale);
            });
        }
    }

    #[must_use]
    pub const fn profiler_mode(&self) -> ProfilerMode {
        self.profiler.mode()
    }

    /// Derives frame budgets from an actual refresh rate (Hz). `None` clears.
    pub fn set_refresh_rate_hz(&mut self, hz: Option<f32>) {
        self.profiler.set_refresh_rate_hz(hz);
    }

    /// Merges renderer-reported metrics into one window's latest record and
    /// feeds the application profiler history with a complete frame sample.
    pub fn note_render_metrics(
        &mut self,
        window_id: WindowId,
        mut render: RenderFrameMetrics,
        gpu: Option<GpuSample>,
    ) {
        let frame_id = self.profiler.next_frame_id();
        render.frame = frame_id;
        let mut completed = None;
        {
            let mut registry = self.registry.borrow_mut();
            if let Some(slot) = registry.slots.get_mut(window_id.index() as usize)
                && slot.generation == window_id.generation()
                && let Some(record) = slot.record.as_mut()
            {
                record.render_metrics = render;
                record.gpu_sample = gpu;
                record.last_frame.frame = frame_id;
                // Renderer prepare/encode/submit completes the CPU picture.
                let mut timings = record.last_frame.timings;
                timings.cpu_total = timings
                    .cpu_total
                    .saturating_add(render.prepare_us)
                    .saturating_add(render.encode_us)
                    .saturating_add(render.submit_us);
                record.last_frame.timings = timings;
                completed = Some(record.last_frame);
            }
        }
        if let Some(record) = completed {
            self.profiler.record(record);
        }
        // Throttled publish for observers (debug overlays). Production frames
        // with no observer and Normal mode skip the snapshot build entirely.
        if self.hub.observed()
            && self.profiler.mode() >= ProfilerMode::Diagnostic
            && self.hub.publish_due(std::time::Duration::from_millis(200))
        {
            self.hub.publish(self.performance_snapshot());
        }
    }

    /// Shared handle to the observable performance snapshot. Overlay builders
    /// read [`PerformanceHub::version`] so only they rebuild on publish.
    #[must_use]
    pub const fn performance_hub(&self) -> &PerformanceHub {
        &self.hub
    }

    /// Installs the debug performance overlay into `window_id`.
    ///
    /// The application tree must contain a widget keyed with
    /// [`PERFORMANCE_OVERLAY_KEY`]; that placeholder element is replaced by a
    /// repaint-contained overlay whose builder re-runs **only** when the hub
    /// publishes, so measured widget work is unaffected. Requires a profiler
    /// mode of [`ProfilerMode::Diagnostic`] or higher to observe data.
    pub fn install_performance_overlay(&mut self, window_id: WindowId) -> Result<(), TreeError> {
        let hub = self.hub.clone();
        let installed = self.with_window_mut(window_id, |record| {
            let tree = record.runtime.tree();
            let Some(target) = tree.element_with_key(&Key::from(PERFORMANCE_OVERLAY_KEY)) else {
                return Err(TreeError::MissingElement(
                    tree.root().expect("mounted window root"),
                ));
            };
            record.runtime.register_builder(target, move || {
                hub.set_observed(true);
                // Reading the version subscribes this element alone.
                let _version = hub.version();
                overlay_widget(&hub.snapshot())
            })?;
            Ok(())
        });
        match installed {
            Some(result) => result,
            None => Err(TreeError::WindowUnknown),
        }
    }

    #[must_use]
    pub fn profiler_history(&self) -> &FrameHistory {
        self.profiler.history()
    }

    /// Builds a complete read-only performance view. This allocates and is
    /// intended for diagnostics tooling, the debug overlay, and JSON export —
    /// never for the per-frame hot path.
    #[must_use]
    pub fn performance_snapshot(&self) -> PerformanceSnapshot {
        let mut windows = Vec::new();
        let registry = self.registry.borrow();
        for slot in &registry.slots {
            let Some(record) = slot.record.as_ref() else {
                continue;
            };
            windows.push(WindowPerformance {
                requested_frames: record.requested_frames,
                presented_frames: record.presented_frames,
                skipped_frames: record.skipped_frames,
                latest: Some(record.last_frame),
                render: record.render_metrics,
                gpu: record.gpu_sample,
            });
        }
        // Widget/text totals come from the primary window's retained tree;
        // accessibility counters merge across every live window adapter.
        let mut widgets = WidgetWorkSnapshot::default();
        let mut text = TextCacheSnapshot::default();
        let mut accessibility = AccessibilitySnapshot::default();
        for slot in &registry.slots {
            let Some(record) = slot.record.as_ref() else {
                continue;
            };
            let tree = record.runtime.tree();
            if widgets.elements_total == 0 {
                let diagnostics = tree.diagnostics();
                widgets = WidgetWorkSnapshot {
                    mounts: diagnostics.mounts,
                    unmounts: diagnostics.unmounts,
                    rebuilds: diagnostics.rebuilds,
                    layouts: diagnostics.layouts,
                    paints: diagnostics.paints,
                    composites: diagnostics.composites,
                    animation_ticks: diagnostics.animation_ticks,
                    scroll_offset_updates: diagnostics.scroll_offset_updates,
                    reconciliation_fast_paths: diagnostics.reconciliation_fast_paths,
                    layout_cache_hits: diagnostics.layout_cache_hits,
                    display_lists_reused: diagnostics.display_lists_reused,
                    compositor_only_updates: diagnostics.compositor_only_updates,
                    lazy_layouts: diagnostics.lazy_layouts,
                    items_built: diagnostics.items_built,
                    items_reused: diagnostics.items_reused,
                    child_list_scans: diagnostics.child_list_scans,
                    identical_child_bailouts: diagnostics.identical_child_bailouts,
                    elements_created: diagnostics.elements_created,
                    elements_removed: diagnostics.elements_removed,
                    elements_moved: diagnostics.elements_moved,
                    dirty_requests: diagnostics.dirty_requests,
                    dirty_queue_deduplicated: diagnostics.dirty_queue_deduplicated,
                    elements_total: tree.element_count(),
                    render_objects_total: tree.render_object_count(),
                    layers_total: usize::try_from(tree.compositor_diagnostics().layers)
                        .unwrap_or(0),
                };
                let text_diagnostics = tree.text_diagnostics();
                text = TextCacheSnapshot {
                    layouts_requested: text_diagnostics.layouts_requested,
                    cache_hits: text_diagnostics.cache_hits,
                    cache_misses: text_diagnostics.cache_misses,
                    paragraphs_reshaped: text_diagnostics.paragraphs_reshaped,
                    parley_layouts_reused: text_diagnostics.parley_layouts_reused,
                    documents_composed: text_diagnostics.documents_composed,
                };
            }
            accessibility.nodes_published += record.accessibility.nodes_published;
            accessibility.updates_skipped_unchanged +=
                record.accessibility.semantic_updates_skipped_unchanged;
        }
        let mut scheduler = scheduler_counters();
        self.scheduler_counters.apply_to(&mut scheduler);
        PerformanceSnapshot {
            scheduler,
            budget: *self.profiler.budget(),
            fps: self.profiler.frames_per_second(),
            windows,
            frame_statistics: self.profiler.history().statistics(),
            widgets,
            text,
            accessibility,
        }
    }

    #[must_use]
    pub const fn profiler_budget(&self) -> &BudgetStatistics {
        self.profiler.budget()
    }

    pub fn note_presented(&mut self, window_id: WindowId, presented: bool) {
        if presented {
            scheduler_counters::FRAMES_PRESENTED.fetch_add(1, Ordering::Relaxed);
            self.scheduler_counters.frames_presented =
                self.scheduler_counters.frames_presented.wrapping_add(1);
        } else {
            scheduler_counters::FRAMES_SKIPPED.fetch_add(1, Ordering::Relaxed);
            self.scheduler_counters.frames_skipped =
                self.scheduler_counters.frames_skipped.wrapping_add(1);
        }
        self.profiler.note_present(presented);
        let _ = self.with_window_mut(window_id, |record| {
            if presented {
                record.presented_frames = record.presented_frames.wrapping_add(1);
            } else {
                record.skipped_frames = record.skipped_frames.wrapping_add(1);
            }
        });
    }

    pub fn request_close(&mut self, window_id: WindowId) -> bool {
        if !self.contains_window(window_id) {
            return false;
        }
        if self
            .close_request
            .as_mut()
            .is_some_and(|callback| !callback(window_id))
        {
            return false;
        }
        self.close_window(window_id)
    }

    pub fn close_window(&mut self, window_id: WindowId) -> bool {
        self.fail_simulation_window(window_id);
        self.fail_native_requests_for_window(window_id, PlatformOperationError::stale_resource());
        self.fail_file_dialog_requests_for_window(window_id, FileDialogError::ParentClosed);
        let record = self.registry.borrow_mut().close(window_id);
        let Some(mut record) = record else {
            self.registry.borrow_mut().stale_window_commands += 1;
            return false;
        };
        record
            .transient_presentations
            .write()
            .expect("transient presentation snapshot lock")
            .clear();
        record.scope.cancel();
        record.runtime.dispose_window();
        self.native_commands
            .borrow_mut()
            .push_back(NativeWindowCommand::Operate(WindowCommand::new(
                window_id,
                WindowOperation::Close,
            )));
        // Explicit user close removes the auxiliary descriptor; shutdown
        // bypasses this method and therefore preserves active descriptors.
        self.manager.sync_restorable_windows(false);
        if self.registry.borrow().visible_count() == 0
            && self.last_window_policy == LastWindowPolicy::ExitOnLastWindow
        {
            self.should_exit = true;
        }
        true
    }

    fn drain_window_commands(&mut self) {
        while let Ok(queued) = self.command_receiver.try_recv() {
            let QueuedWindowCommand { command, request } = queued;
            let window_id = command.window_id;
            match command.operation.clone() {
                WindowOperation::Close => {
                    if let Some(request) = request {
                        Self::resolve_queued_request(
                            request,
                            Err(PlatformOperationError::with_context(
                                PlatformOperationErrorKind::InvalidState,
                                "window close does not expose a native completion request",
                            )),
                        );
                    }
                    let _ = self.close_window(window_id);
                }
                operation => {
                    let applied = self.with_window_mut(window_id, |record| {
                        match &operation {
                            WindowOperation::SetTitle(title) => {
                                record.options.title = title.clone()
                            }
                            WindowOperation::SetVisible(visible) => {
                                record.options.visible = *visible;
                                record.requested_state.visible = *visible;
                                record.lifecycle = if *visible {
                                    WindowLifecycle::Visible
                                } else {
                                    WindowLifecycle::Hidden
                                };
                            }
                            WindowOperation::SetLogicalSize(size) => {
                                if size.width.is_finite()
                                    && size.height.is_finite()
                                    && size.width > 0.0
                                    && size.height > 0.0
                                {
                                    record.content_sizing.reset();
                                    record.options.initial_logical_size = *size;
                                } else {
                                    return false;
                                }
                            }
                            WindowOperation::SetOuterPosition(_) => {}
                            WindowOperation::BeginMoveDrag
                            | WindowOperation::BeginResizeDrag(_) => {}
                            WindowOperation::SetMinimized(minimized) => {
                                record.requested_state.minimized = *minimized;
                            }
                            WindowOperation::SetMaximized(maximized) => {
                                record.options.maximized = *maximized;
                                record.requested_state.maximized = *maximized;
                            }
                            WindowOperation::SetFullscreen(fullscreen) => {
                                record.options.fullscreen = *fullscreen;
                                record.requested_state.fullscreen = *fullscreen;
                            }
                            WindowOperation::SetResizable(resizable) => {
                                record.options.resizable = *resizable;
                                record.requested_state.resizable = *resizable;
                            }
                            WindowOperation::SetDecorations(decorations) => {
                                record.options.decorations = *decorations;
                                record.requested_state.decorations = *decorations;
                            }
                            WindowOperation::SetLogicalSizeLimits(limits) => {
                                record.options.minimum_logical_size = limits.minimum();
                                record.options.maximum_logical_size = limits.maximum();
                                record.requested_state.size_limits = *limits;
                                record.content_sizing.reset();
                            }
                            WindowOperation::SetWindowLevel(level) => {
                                record.options.window_level = *level;
                                record.requested_state.level = *level;
                            }
                            WindowOperation::SetWindowIcon(icon) => {
                                record.options.window_icon = icon.clone();
                            }
                            WindowOperation::RequestUserAttention(_) => {}
                            WindowOperation::SetCursorGrab(_)
                            | WindowOperation::SetCursorVisible(_)
                            | WindowOperation::SetCursorPosition(_) => {}
                            WindowOperation::SetContentSensitivity(sensitivity) => {
                                record.last_content_sensitivity = Some(*sensitivity);
                            }
                            WindowOperation::RequestFocus => {}
                            WindowOperation::RequestRedraw => record.runtime.request_frame(),
                            WindowOperation::Close => unreachable!(),
                        }
                        true
                    });
                    if applied == Some(true) {
                        if let Some(request) = request {
                            let request_id = command
                                .request_id
                                .expect("result-bearing command has request id");
                            if !request.cancelled.load(Ordering::Acquire) {
                                self.pending_native_requests.insert(
                                    request_id,
                                    PendingNativeRequest {
                                        window_id,
                                        sender: request.sender,
                                    },
                                );
                            }
                        } else {
                            debug_assert!(command.request_id.is_none());
                        }
                        self.native_commands
                            .borrow_mut()
                            .push_back(NativeWindowCommand::Operate(command));
                        if matches!(operation, WindowOperation::SetVisible(false))
                            && self.registry.borrow().visible_count() == 0
                            && self.last_window_policy == LastWindowPolicy::ExitOnLastWindow
                        {
                            self.should_exit = true;
                        }
                        self.manager.sync_restorable_windows(true);
                    } else if applied == Some(false) {
                        if let Some(request) = request {
                            Self::resolve_queued_request(
                                request,
                                Err(PlatformOperationError::with_context(
                                    PlatformOperationErrorKind::InvalidState,
                                    "window operation arguments were rejected by the runtime",
                                )),
                            );
                        }
                    } else if applied.is_none() {
                        if let Some(request) = request {
                            Self::resolve_queued_request(
                                request,
                                Err(PlatformOperationError::stale_resource()),
                            );
                        }
                        self.registry.borrow_mut().stale_window_commands += 1;
                    }
                }
            }
        }
    }

    fn resolve_queued_request(request: QueuedNativeRequest, result: PlatformOperationResult) {
        if !request.cancelled.load(Ordering::Acquire) {
            let _ = request.sender.send(result);
        }
    }

    fn drain_native_request_cancellations(&mut self) {
        while let Ok(request_id) = self.request_cancellation_receiver.try_recv() {
            self.pending_native_requests.remove(&request_id);
        }
    }

    fn fail_native_requests_for_window(
        &mut self,
        window_id: WindowId,
        error: PlatformOperationError,
    ) {
        let request_ids = self
            .pending_native_requests
            .iter()
            .filter_map(|(request_id, request)| {
                (request.window_id == window_id).then_some(*request_id)
            })
            .collect::<Vec<_>>();
        for request_id in request_ids {
            if let Some(request) = self.pending_native_requests.remove(&request_id) {
                let _ = request.sender.send(Err(error.clone()));
            }
        }
    }

    fn fail_all_native_requests(&mut self, error: PlatformOperationError) {
        while self.request_cancellation_receiver.try_recv().is_ok() {}
        for (_, request) in self.pending_native_requests.drain() {
            let _ = request.sender.send(Err(error.clone()));
        }
        while let Ok(queued) = self.command_receiver.try_recv() {
            if let Some(request) = queued.request {
                Self::resolve_queued_request(request, Err(error.clone()));
            }
        }
    }

    fn drain_file_dialog_requests(&mut self) {
        while let Ok(queued) = self.file_dialog_receiver.try_recv() {
            if queued.cancelled.load(std::sync::atomic::Ordering::Acquire) {
                continue;
            }
            if !self.contains_window(queued.window_id) {
                let _ = queued.sender.send(Err(FileDialogError::ParentClosed));
                continue;
            }
            let capabilities = self
                .window_capabilities(queued.window_id)
                .expect("live file-dialog parent has capabilities")
                .application_services
                .file_dialogs;
            if let Some(error) = explicitly_unsupported_request(capabilities, &queued.request) {
                let _ = queued.sender.send(Err(error));
                continue;
            }
            let request_id = queued.request_id;
            let window_id = queued.window_id;
            self.pending_file_dialog_requests.insert(
                request_id,
                PendingFileDialogRequest {
                    window_id,
                    request: queued.request,
                    sender: Some(queued.sender),
                    cancelled: queued.cancelled,
                },
            );
            if self.active_file_dialog_requests.contains_key(&window_id) {
                self.queued_file_dialog_requests
                    .entry(window_id)
                    .or_default()
                    .push_back(request_id);
            } else {
                self.activate_file_dialog(request_id);
            }
        }
    }

    fn activate_file_dialog(&mut self, request_id: FileDialogRequestId) {
        let Some(pending) = self.pending_file_dialog_requests.get(&request_id) else {
            return;
        };
        if pending.cancelled.load(std::sync::atomic::Ordering::Acquire) {
            let window_id = pending.window_id;
            self.pending_file_dialog_requests.remove(&request_id);
            self.promote_next_file_dialog(window_id);
            return;
        }
        let window_id = pending.window_id;
        debug_assert!(!self.active_file_dialog_requests.contains_key(&window_id));
        self.active_file_dialog_requests
            .insert(window_id, request_id);
        self.native_file_dialog_requests
            .push_back(NativeFileDialogRequest {
                request_id,
                window_id,
                request: pending.request.clone(),
            });
    }

    fn promote_next_file_dialog(&mut self, window_id: WindowId) {
        if self.active_file_dialog_requests.contains_key(&window_id)
            || !self.contains_window(window_id)
        {
            return;
        }
        loop {
            let next = self
                .queued_file_dialog_requests
                .get_mut(&window_id)
                .and_then(VecDeque::pop_front);
            let Some(request_id) = next else {
                self.queued_file_dialog_requests.remove(&window_id);
                return;
            };
            let cancelled = self
                .pending_file_dialog_requests
                .get(&request_id)
                .is_none_or(|pending| pending.cancelled.load(std::sync::atomic::Ordering::Acquire));
            if cancelled {
                self.pending_file_dialog_requests.remove(&request_id);
                continue;
            }
            self.activate_file_dialog(request_id);
            return;
        }
    }

    fn drain_file_dialog_cancellations(&mut self) {
        while let Ok(request_id) = self.file_dialog_cancellation_receiver.try_recv() {
            let Some(window_id) = self
                .pending_file_dialog_requests
                .get(&request_id)
                .map(|pending| pending.window_id)
            else {
                continue;
            };
            let is_active = self.active_file_dialog_requests.get(&window_id) == Some(&request_id);
            if is_active {
                // If the desktop host has not taken this request yet, cancellation
                // can prevent native presentation and the FIFO may advance.
                let not_started = self.pending_file_dialog_requests.phase(&request_id)
                    == Some(RequestPhase::Queued);
                self.native_file_dialog_requests
                    .retain(|request| request.request_id != request_id);
                if not_started {
                    self.active_file_dialog_requests.remove(&window_id);
                    self.pending_file_dialog_requests.remove(&request_id);
                    self.promote_next_file_dialog(window_id);
                } else if let Some(pending) = self.pending_file_dialog_requests.get_mut(&request_id)
                {
                    // The native modal operation already owns the slot. Detach
                    // result delivery, but keep serialization until completion.
                    pending.sender = None;
                    self.pending_file_dialog_requests.abandon(&request_id);
                }
            } else {
                if let Some(queue) = self.queued_file_dialog_requests.get_mut(&window_id) {
                    queue.retain(|queued| *queued != request_id);
                    if queue.is_empty() {
                        self.queued_file_dialog_requests.remove(&window_id);
                    }
                }
                self.pending_file_dialog_requests.remove(&request_id);
            }
        }
    }

    fn fail_file_dialog_requests_for_window(
        &mut self,
        window_id: WindowId,
        error: FileDialogError,
    ) {
        self.native_file_dialog_requests
            .retain(|request| request.window_id != window_id);
        self.active_file_dialog_requests.remove(&window_id);
        self.queued_file_dialog_requests.remove(&window_id);
        let request_ids = self
            .pending_file_dialog_requests
            .iter()
            .filter_map(|(request_id, pending)| {
                (pending.window_id == window_id).then_some(*request_id)
            })
            .collect::<Vec<_>>();
        for request_id in request_ids {
            if let Some(mut pending) = self.pending_file_dialog_requests.remove(&request_id)
                && !pending.cancelled.load(std::sync::atomic::Ordering::Acquire)
                && let Some(sender) = pending.sender.take()
            {
                let _ = sender.send(Err(error.clone()));
            }
        }
    }

    fn fail_all_file_dialog_requests(&mut self, error: FileDialogError) {
        self.file_dialog_bridge.stop();
        while self.file_dialog_cancellation_receiver.try_recv().is_ok() {}
        while let Ok(queued) = self.file_dialog_receiver.try_recv() {
            if !queued.cancelled.load(std::sync::atomic::Ordering::Acquire) {
                let _ = queued.sender.send(Err(error.clone()));
            }
        }
        for (_, mut pending) in self.pending_file_dialog_requests.drain() {
            if !pending.cancelled.load(std::sync::atomic::Ordering::Acquire)
                && let Some(sender) = pending.sender.take()
            {
                let _ = sender.send(Err(error.clone()));
            }
        }
        self.active_file_dialog_requests.clear();
        self.queued_file_dialog_requests.clear();
        self.native_file_dialog_requests.clear();
    }

    /// Takes native file-dialog work ready to start. At most one request per
    /// parent window is active; later requests for that window remain FIFO in
    /// the runtime until completion or cancellation.
    pub fn take_native_file_dialog_requests(&mut self) -> Vec<NativeFileDialogRequest> {
        self.drain_file_dialog_requests();
        self.drain_file_dialog_cancellations();
        let requests: Vec<_> = self.native_file_dialog_requests.drain(..).collect();
        for request in &requests {
            self.pending_file_dialog_requests
                .dispatch(&request.request_id);
        }
        requests
    }

    /// Completes one active file dialog and promotes the next request for the
    /// same parent. Backends must invoke this only on the application UI/event
    /// loop thread; native dialog objects never cross this boundary.
    pub fn complete_file_dialog(
        &mut self,
        completion: NativeFileDialogCompletion,
    ) -> FileDialogCompletionStatus {
        let Some(pending) = self
            .pending_file_dialog_requests
            .get(&completion.request_id)
        else {
            return FileDialogCompletionStatus::UnknownRequest;
        };
        if pending.window_id != completion.window_id {
            return FileDialogCompletionStatus::TargetMismatch;
        }
        if self.active_file_dialog_requests.get(&completion.window_id)
            != Some(&completion.request_id)
        {
            return FileDialogCompletionStatus::UnknownRequest;
        }

        let Ok(mut pending) = self
            .pending_file_dialog_requests
            .complete(&completion.request_id, |_| true)
        else {
            return FileDialogCompletionStatus::UnknownRequest;
        };
        self.active_file_dialog_requests
            .remove(&completion.window_id);

        let (result, status) = match completion.result {
            Ok(outcome) if result_matches_request(&pending.request, &outcome) => {
                (Ok(outcome), FileDialogCompletionStatus::Completed)
            }
            Ok(_) => (
                Err(FileDialogError::InvalidResult),
                FileDialogCompletionStatus::InvalidResult,
            ),
            Err(error) => (Err(error), FileDialogCompletionStatus::Completed),
        };
        if !pending.cancelled.load(std::sync::atomic::Ordering::Acquire)
            && let Some(sender) = pending.sender.take()
        {
            let _ = sender.send(result);
        }
        self.promote_next_file_dialog(completion.window_id);
        status
    }

    fn drain_global_shortcut_requests(&mut self) {
        drain_queued(
            &self.global_shortcut_receiver,
            &mut self.pending_global_shortcut_requests,
            &mut self.native_global_shortcut_requests,
        );
        let abandoned: Vec<_> = self
            .pending_global_shortcut_requests
            .iter()
            .filter(|(_, pending)| pending.reply.is_abandoned())
            .map(|(id, pending)| (*id, pending.operation))
            .collect();
        for (id, operation) in abandoned {
            if self.pending_global_shortcut_requests.phase(&id) == Some(RequestPhase::Queued) {
                if matches!(operation, NativeGlobalShortcutOperation::Register { .. }) {
                    self.pending_global_shortcut_requests.remove(&id);
                    self.native_global_shortcut_requests
                        .retain(|request| request.request_id != id);
                }
            } else {
                self.pending_global_shortcut_requests.abandon(&id);
            }
        }
    }

    fn fail_all_global_shortcut_requests(&mut self) {
        self.global_shortcut_bridge.stop();
        while let Ok(queued) = self.global_shortcut_receiver.try_recv() {
            queued.reply.finish(
                queued.operation,
                Err(incular_platform::GlobalShortcutError::ApplicationStopped),
            );
        }
        for (_, pending) in self.pending_global_shortcut_requests.drain() {
            pending.reply.finish(
                pending.operation,
                Err(incular_platform::GlobalShortcutError::ApplicationStopped),
            );
        }
        self.native_global_shortcut_requests.clear();
    }

    /// Takes application-scoped global-shortcut work ready for the native UI
    /// thread. Custom desktop embedders may implement this boundary without
    /// exposing native hotkey handles to runtime/application code.
    #[doc(hidden)]
    pub fn take_native_global_shortcut_requests(&mut self) -> Vec<NativeGlobalShortcutRequest> {
        self.drain_global_shortcut_requests();
        let requests: Vec<_> = self.native_global_shortcut_requests.drain(..).collect();
        for request in &requests {
            self.pending_global_shortcut_requests
                .dispatch(&request.request_id);
        }
        requests
    }

    /// Completes one native global-shortcut request. If the requesting future
    /// was abandoned after a registration succeeded, the caller is instructed
    /// to roll that native registration back immediately.
    #[doc(hidden)]
    pub fn complete_global_shortcut_request(
        &mut self,
        completion: NativeGlobalShortcutCompletion,
    ) -> GlobalShortcutCompletionStatus {
        let Ok(pending) = self
            .pending_global_shortcut_requests
            .complete(&completion.request_id, |_| true)
        else {
            return GlobalShortcutCompletionStatus::UnknownRequest;
        };
        pending.reply.finish(pending.operation, completion.result)
    }

    /// Completes one result-bearing native operation. Backends call this only
    /// after applying the operation on the owning native/UI thread.
    pub fn complete_native_operation(
        &mut self,
        completion: NativeOperationCompletion,
    ) -> NativeOperationCompletionStatus {
        let pending = match self
            .pending_native_requests
            .complete(&completion.request_id, |pending| {
                pending.window_id == completion.window_id
            }) {
            Ok(pending) => pending,
            Err(CompletionRejection::TargetMismatch) => {
                return NativeOperationCompletionStatus::TargetMismatch;
            }
            Err(_) => return NativeOperationCompletionStatus::UnknownRequest,
        };
        if !self.contains_window(completion.window_id) {
            let _ = pending
                .sender
                .send(Err(PlatformOperationError::stale_resource()));
            return NativeOperationCompletionStatus::StaleTarget;
        }
        if let Err(error) = &completion.result {
            let error = error.clone();
            let _ = self.with_window_mut(completion.window_id, |record| {
                record.last_platform_error = Some(error);
            });
        }
        let _ = pending.sender.send(completion.result);
        NativeOperationCompletionStatus::Completed
    }

    /// Takes all UI-thread native operations. Adapters must call this only
    /// from their event-loop callback; creation is consequently valid for
    /// Winit 0.30's active-loop restriction.
    pub fn take_native_window_commands(&mut self) -> Vec<NativeWindowCommand> {
        self.drain_window_commands();
        self.drain_native_request_cancellations();
        let commands: Vec<_> = self.native_commands.borrow_mut().drain(..).collect();
        for command in &commands {
            if let NativeWindowCommand::Operate(command) = command
                && let Some(id) = command.request_id
            {
                self.pending_native_requests.dispatch(&id);
            }
        }
        commands
    }

    #[must_use]
    pub const fn should_exit(&self) -> bool {
        self.should_exit
    }

    #[must_use]
    pub fn window_diagnostics(&self, window_id: WindowId) -> Option<WindowDiagnostics> {
        let registry = self.registry.borrow();
        let slot = registry.slots.get(window_id.index() as usize)?;
        if slot.generation != window_id.generation() {
            return None;
        }
        let record = slot.record.as_ref()?;
        Some(WindowDiagnostics {
            id: window_id,
            title: record.options.title.clone(),
            logical_size: record.metrics.logical_size(),
            physical_size: record.metrics.physical_size,
            scale_factor: record.metrics.scale_factor,
            visible: record.options.visible,
            native_focused: record.native_focused,
            lifecycle: record.lifecycle,
            application_lifecycle: record.runtime.lifecycle(),
            frame_requested: record.runtime.frame_requested(),
            requested_frames: record.requested_frames,
            presented_frames: record.presented_frames,
            skipped_frames: record.skipped_frames,
            input_events: record.input_events,
            surface_generation: record.surface_generation,
            content_sensitivity: record.runtime.content_sensitivity(),
            elements: record.runtime.tree().element_count(),
            render_objects: record.runtime.tree().render_object_count(),
            semantics: record.runtime.tree().semantics().len(),
            accessibility: record.accessibility,
            capabilities: *record
                .capabilities
                .read()
                .expect("window capability snapshot lock"),
            last_platform_error: record.last_platform_error.clone(),
            requested_state: record.requested_state,
            observed_state: *record
                .observed_state
                .read()
                .expect("window observed-state snapshot lock"),
            transient_presentations: record
                .transient_presentations
                .read()
                .expect("transient presentation snapshot lock")
                .clone(),
        })
    }

    #[must_use]
    pub fn diagnostics(&self) -> ApplicationDiagnostics {
        let registry = self.registry.borrow();
        ApplicationDiagnostics {
            windows_created: registry.windows_created,
            windows_closed: registry.windows_closed,
            active_windows: registry.ids().len(),
            stale_window_commands: registry.stale_window_commands,
            pending_native_requests: self.pending_native_requests.len(),
            pending_file_dialog_requests: self.pending_file_dialog_requests.len(),
        }
    }

    /// Returns value-free health and persistence-coalescing data when the
    /// application opted into restoration.
    #[must_use]
    pub fn restoration_diagnostics(&self) -> Option<RestorationDiagnostics> {
        self.restoration
            .as_ref()
            .map(restoration::RestorationManager::diagnostics)
    }

    /// Returns the configured snapshot path for developer tooling. In-memory
    /// and custom stores intentionally have no filesystem location.
    #[must_use]
    pub fn restoration_path(&self) -> Option<std::path::PathBuf> {
        self.restoration
            .as_ref()
            .and_then(restoration::RestorationManager::store_location)
    }

    /// Removes every opt-in persisted value and every restorable-window
    /// descriptor. The deletion is coalesced through the normal Tokio-backed
    /// persistence pipeline; call [`Self::flush_restoration`] before an
    /// immediate process handoff if required.
    pub fn reset_restoration(&mut self) -> bool {
        let Some(restoration) = &self.restoration else {
            return false;
        };
        restoration.reset();
        true
    }

    /// Bypasses the normal 250ms debounce but still writes on Tokio's blocking
    /// pool. This is useful for an explicit user action or lifecycle flush.
    pub fn flush_restoration(&mut self) -> bool {
        let Some(restoration) = &self.restoration else {
            return false;
        };
        restoration.flush();
        true
    }

    #[must_use]
    pub fn debug_dump(&self) -> String {
        let mut lines = vec!["Application".to_owned()];
        if let Some(restoration) = &self.restoration {
            lines.push(restoration.debug_dump());
        }
        for id in self.active_window_ids() {
            if let Some(window) = self.window_diagnostics(id) {
                lines.push(format!(
                    "  {id} title={:?} {:.0}x{:.0} @{} focused={}\\n    root elements={} render={} semantics={} dirty={}",
                    window.title,
                    window.logical_size.width,
                    window.logical_size.height,
                    window.scale_factor,
                    window.native_focused,
                    window.elements,
                    window.render_objects,
                    window.semantics,
                    window.frame_requested,
                ));
            }
        }
        lines.join("\\n")
    }

    pub fn shutdown(&mut self) {
        self.flush_restoration_before_shutdown();
        self.application_shell.stop();
        self.manager.bridge.stop();
        self.fail_all_file_dialog_requests(FileDialogError::ApplicationStopped);
        self.fail_all_global_shortcut_requests();
        self.fail_all_native_requests(PlatformOperationError::with_context(
            PlatformOperationErrorKind::Unavailable,
            "application runtime stopped before native completion",
        ));
        let ids = self.active_window_ids();
        for id in ids {
            self.fail_simulation_window(id);
            if let Some(mut record) = self.registry.borrow_mut().close(id) {
                record.scope.cancel();
                record.runtime.dispose_window();
            }
        }
        self.native_commands.borrow_mut().clear();
        self.scheduler.borrow_mut().shutdown();
        self.stop_simulation();
        self.should_exit = true;
    }

    fn flush_restoration_before_shutdown(&mut self) {
        let Some(restoration) = self.restoration.clone() else {
            return;
        };
        restoration.flush();
        let deadline = Instant::now() + std::time::Duration::from_millis(100);
        while !restoration.is_clean() && Instant::now() < deadline {
            self.process_runtime_work();
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    /// Compatibility escape hatch for existing single-window embedders and
    /// tests. Extra windows are cancelled; desktop `run` retains all windows.
    #[must_use]
    pub fn into_runtime(mut self) -> Runtime {
        self.manager.bridge.stop();
        self.fail_all_file_dialog_requests(FileDialogError::ApplicationStopped);
        self.fail_all_global_shortcut_requests();
        let primary = self.primary_window;
        let mut primary_record = self
            .registry
            .borrow_mut()
            .close(primary)
            .expect("application primary window exists");
        for id in self.active_window_ids() {
            if let Some(record) = self.registry.borrow_mut().close(id) {
                record.scope.cancel();
            }
        }
        primary_record.runtime.window_manager = None;
        primary_record.runtime.window_id = None;
        primary_record.runtime
    }
}
/// Widget key that marks the performance-overlay mount point.
pub const PERFORMANCE_OVERLAY_KEY: &str = incular_widgets::internal::PERFORMANCE_OVERLAY_KEY;

/// Builds the repaint-contained overlay visual from one snapshot.
fn overlay_widget(snapshot: &PerformanceSnapshot) -> incular_widgets::Widget {
    use incular_widgets::{DecoratedBox, Padding, Text, Widget};
    let lines = snapshot.overlay_lines();
    let monospace = || {
        incular_widgets::TextStyle::default()
            .family(incular_text::FontFamily::Monospace)
            .font_size(11.)
            .color(incular_core::Color::rgba(190, 220, 255, 255))
    };
    let rows: Vec<Widget> = lines
        .into_iter()
        .map(|line| Widget::from(Text::new(line).style(monospace())))
        .collect();
    // The overlay is diagnostic chrome, not an application control. Keep the
    // visual mounted in the tree while allowing the widgets underneath it to
    // receive pointer events, just like Flutter's non-interactive performance
    // overlay.
    Widget::from(
        incular_widgets::IgnorePointer::new(Widget::from(incular_widgets::RepaintBoundary::new(
            Widget::from(
                DecoratedBox::new(Padding::all(
                    6.,
                    Widget::from(incular_widgets::Column::new(rows)),
                ))
                .background(incular_core::Color::rgba(12, 14, 18, 216))
                .radius(4.)
                .border(incular_rendering::Border {
                    color: incular_core::Color::rgba(120, 170, 245, 90),
                    width: 1.,
                }),
            ),
        )))
        .ignoring(true),
    )
}
