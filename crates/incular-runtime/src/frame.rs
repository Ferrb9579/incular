use crate::application_types::{
    ApplicationLifecycle, LifecycleTransition, RuntimeErrorReport, WindowError,
};
#[cfg(feature = "devtools")]
use crate::environment::DEV_TASK_COMPLETION;
#[cfg(feature = "devtools")]
use crate::environment::ReactiveRootId;
use crate::environment::{
    BuildScope, BuildScopeGuard, InitialBuildDependencies, ReactiveQueue, environment_change_mask,
    install_focus_scope_watch,
};
use crate::profiling;
use crate::profiling::FrameTimings;
use crate::tasks::{
    self, RuntimeDiagnostics, RuntimeSpawner, RuntimeWake, Task, TaskFailure, TaskHandle,
    TaskScope, TokioHandle, UiDispatcher,
};
use crate::undo::UndoHistoryController;
use crate::window_commands::{WindowHandle, WindowOpener};
use crate::window_state::WindowManager;
use incular_config::{Constraints, RuntimeEnvironment};
use incular_core::{
    Code, ImeEvent, InputEvent, KeyboardEvent, Modifiers, Offset, PRIMARY_POINTER_BUTTON,
    PointerPhase, Rect,
};
use incular_platform::{
    Clipboard, MemoryClipboard, PlatformEvent, PlatformLifecycle, TextInputAction,
    TextInputClientId, TextInputCommand, TextInputConfiguration, TextInputState, TextInputType,
    WindowId, WindowMetrics, WindowOperation, WindowOptions,
};
use incular_rendering::DisplayList;
use incular_semantics::{SemanticAction, SemanticNodeId};
#[cfg(feature = "devtools")]
use incular_widgets::internal::InvalidationCause;
use incular_widgets::internal::{
    ActionId, Diagnostics, ElementId, PointerDeviceKind, PointerEvent, RawPointerEvent, TextRange,
    TextSelection, TreeError, WidgetTree, WindowInteraction,
};
use incular_widgets::{TextInputActionHint, TextInputTypeHint, Widget};
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, VecDeque},
    rc::Rc,
    time::Instant,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameStats {
    pub updated_elements: usize,
    pub rebuilt_elements: u64,
    pub laid_out_render_objects: u64,
    pub repainted_render_objects: u64,
    pub composited: u64,
    pub active_animations: u64,
    pub display_list_commands: usize,
    pub requested_another_frame: bool,
    /// Monotonic per-phase CPU durations. Phases never nest.
    pub timings: FrameTimings,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventTarget {
    pub element: ElementId,
    pub action: Option<ActionId>,
}
/// Debug-facing focus state without exposing native event-loop details.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FocusDiagnostics {
    pub focused_element: Option<ElementId>,
    pub text_pointer_capture: Option<ElementId>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EditingDiagnostics {
    pub key_down_received: u64,
    pub key_up_received: u64,
    pub backspace_commands: u64,
    pub delete_commands: u64,
    pub text_commits: u64,
    pub ime_events: u64,
}
/// DevTools signal registry. Active only under the `devtools` feature.
pub struct Runtime {
    pub(crate) tree: WidgetTree,
    /// Input-dispatch time accumulated since the last frame consumed it.
    pub(crate) pending_event_processing_us: u32,
    pub(crate) pending: HashMap<ElementId, Widget>,
    pub(crate) order: VecDeque<ElementId>,
    pub(crate) reactive: Rc<RefCell<ReactiveQueue>>,
    pub(crate) builders: HashMap<ElementId, Box<dyn FnMut() -> Widget>>,
    pub(crate) handlers: HashMap<ActionId, Rc<dyn Fn()>>,
    pub(crate) hovered_button: Option<ElementId>,
    pub(crate) pressed_button: Option<ElementId>,
    /// The contact currently allowed to drive single-contact controls. Other
    /// contacts still reach retained gesture regions for scale/pan input.
    pub(crate) legacy_pointer: Option<u64>,
    pub(crate) last_pointer: Offset,
    pub(crate) focused: Option<ElementId>,
    pub(crate) captured_text_field: Option<ElementId>,
    pub(crate) captured_selectable_text: Option<ElementId>,
    pub(crate) text_histories: HashMap<ElementId, UndoHistoryController>,
    pub(crate) text_input_commands: VecDeque<TextInputCommand>,
    pub(crate) text_input_client: Option<TextInputClientId>,
    pub(crate) text_input_configuration: Option<TextInputConfiguration>,
    pub(crate) text_input_state: Option<TextInputState>,
    pub(crate) text_input_caret_rect: Option<Rect>,
    pub(crate) clipboard: Box<dyn Clipboard>,
    pub(crate) editing_diagnostics: EditingDiagnostics,
    pub(crate) frame_requested: bool,
    pub(crate) scheduler: Rc<RefCell<tasks::TaskScheduler>>,
    pub(crate) environment: Rc<RefCell<RuntimeEnvironment>>,
    pub(crate) lifecycle: ApplicationLifecycle,
    pub(crate) lifecycle_observers: Vec<Rc<dyn Fn(LifecycleTransition)>>,
    pub(crate) error_observers: Vec<Rc<dyn Fn(RuntimeErrorReport)>>,
    pub(crate) environment_generation: u64,
    pub(crate) environment_dependencies: Rc<Cell<u16>>,
    pub(crate) application_root: Option<ElementId>,
    pub(crate) owner_scopes: HashMap<ElementId, TaskScope>,
    pub(crate) window_id: Option<WindowId>,
    pub(crate) window_scope: TaskScope,
    pub(crate) window_manager: Option<WindowManager>,
}

pub(crate) fn diagnostic_input_trigger(event: &InputEvent) -> String {
    match event {
        InputEvent::Pointer { phase, position } => {
            format!("pointer({phase:?}, {:.1}, {:.1})", position.x, position.y)
        }
        InputEvent::PointerWithId {
            pointer,
            phase,
            position,
        } => format!(
            "pointer({pointer}, {phase:?}, {:.1}, {:.1})",
            position.x, position.y
        ),
        InputEvent::PointerWithMetadata {
            pointer,
            device,
            kind,
            buttons,
            phase,
            position,
        } => format!(
            "pointer({pointer}, device={device}, {kind:?}, buttons={buttons:#x}, {phase:?}, {:.1}, {:.1})",
            position.x, position.y
        ),
        InputEvent::Scroll { delta } => format!("scroll({:.1}, {:.1})", delta.x, delta.y),
        InputEvent::Key(event) => format!("key({:?}, {:?})", event.state, event.code),
        InputEvent::Text(text) => format!("text_input(chars={})", text.chars().count()),
        InputEvent::Ime(ImeEvent::Preedit { text, .. }) => {
            format!("ime_preedit(chars={})", text.chars().count())
        }
        InputEvent::Ime(ImeEvent::Commit(text)) => {
            format!("ime_commit(chars={})", text.chars().count())
        }
        InputEvent::Ime(ImeEvent::End) => "ime_end".to_owned(),
        InputEvent::WindowResized { size, scale_factor } => format!(
            "window_resize({:.1}x{:.1}, scale={scale_factor:.2})",
            size.width, size.height
        ),
    }
}

impl Runtime {
    pub fn new(root: Widget) -> Result<Self, TreeError> {
        Self::with_scheduler(root, tasks::TaskScheduler::new())
    }
    fn with_scheduler(
        root: Widget,
        scheduler: Rc<RefCell<tasks::TaskScheduler>>,
    ) -> Result<Self, TreeError> {
        let window_scope = tasks::TaskScheduler::spawner(&scheduler).scope();
        Self::with_window(root, scheduler, None, window_scope, None)
    }

    pub(crate) fn with_window(
        root: Widget,
        scheduler: Rc<RefCell<tasks::TaskScheduler>>,
        window_id: Option<WindowId>,
        window_scope: TaskScope,
        window_manager: Option<WindowManager>,
    ) -> Result<Self, TreeError> {
        Self::with_window_and_reactive(
            root,
            scheduler,
            window_id,
            window_scope,
            window_manager,
            ReactiveQueue::new(),
        )
    }
    pub(crate) fn with_window_and_reactive(
        mut root: Widget,
        scheduler: Rc<RefCell<tasks::TaskScheduler>>,
        window_id: Option<WindowId>,
        window_scope: TaskScope,
        window_manager: Option<WindowManager>,
        reactive: Rc<RefCell<ReactiveQueue>>,
    ) -> Result<Self, TreeError> {
        let mut tree = WidgetTree::new();
        let mut handlers = HashMap::new();
        root.bind_callbacks(&mut |callback| {
            let action = tree.allocate_action();
            handlers.insert(action, callback);
            action
        });
        tree.mount(root)?;
        for (action, handler) in tree.take_pending_handlers() {
            handlers.insert(action, handler);
        }
        let mut runtime = Self {
            pending_event_processing_us: 0,
            tree,
            pending: HashMap::new(),
            order: VecDeque::new(),
            reactive,
            builders: HashMap::new(),
            handlers,
            hovered_button: None,
            pressed_button: None,
            legacy_pointer: None,
            last_pointer: Offset::ZERO,
            focused: None,
            captured_text_field: None,
            captured_selectable_text: None,
            text_histories: HashMap::new(),
            text_input_commands: VecDeque::new(),
            text_input_client: None,
            text_input_configuration: None,
            text_input_state: None,
            text_input_caret_rect: None,
            clipboard: Box::new(MemoryClipboard::default()),
            editing_diagnostics: EditingDiagnostics::default(),
            frame_requested: true,
            scheduler,
            environment: Rc::new(RefCell::new(RuntimeEnvironment::default())),
            lifecycle: ApplicationLifecycle::Starting,
            lifecycle_observers: Vec::new(),
            error_observers: Vec::new(),
            environment_generation: 0,
            environment_dependencies: Rc::new(Cell::new(0)),
            application_root: None,
            owner_scopes: HashMap::new(),
            window_id,
            window_scope,
            window_manager,
        };
        // Focus nodes live outside the retained tree so they may be shared by
        // rebuildable descriptors. Resolve the first mounted autofocus listener
        // once the element IDs exist, then keep runtime and node focus mirrored
        // for subsequent keyboard traversal.
        if let Some(autofocus) = runtime.tree.autofocus_element() {
            runtime.set_focus(Some(autofocus));
        }
        Ok(runtime)
    }
    /// Starts work owned by this retained root. Component code should prefer
    /// the build context's spawn API so completion lifetime follows its owner.
    #[must_use]
    pub fn spawner(&self) -> RuntimeSpawner {
        self.window_id.map_or_else(
            || tasks::TaskScheduler::spawner(&self.scheduler),
            |window_id| tasks::TaskScheduler::spawner_for(&self.scheduler, window_id),
        )
    }
    /// Sends a `Send` callback from any worker thread to the UI owner. The
    /// callback is never run concurrently with retained-tree mutation.
    #[must_use]
    pub fn dispatcher(&self) -> UiDispatcher {
        self.scheduler.borrow().dispatcher_for(self.window_id)
    }
    /// Returns the application's Tokio handle for advanced libraries. Direct
    /// Tokio tasks have Tokio/application lifetime and are not component-owned.
    #[must_use]
    pub fn tokio_handle(&self) -> TokioHandle {
        self.scheduler.borrow().tokio_handle()
    }
    pub fn set_wake_handler(&mut self, wake: std::sync::Arc<dyn RuntimeWake>) {
        self.scheduler.borrow_mut().set_wake(wake);
    }

    /// Returns the owning normalized window when this runtime is managed by a
    /// multi-window application. Standalone runtimes remain windowless.
    #[must_use]
    pub const fn window_id(&self) -> Option<WindowId> {
        self.window_id
    }

    /// Marks only this retained root for a future presentation.
    pub fn request_frame(&mut self) {
        self.frame_requested = true;
    }

    /// Drains native text-input commands produced since the last platform
    /// event-loop turn. A desktop runner applies these to Winit; mobile hosts
    /// can translate the same data to their platform text services.
    pub fn take_text_input_commands(&mut self) -> Vec<TextInputCommand> {
        self.text_input_commands.drain(..).collect()
    }

    /// Opens a second retained root through the owning application. This can
    /// be called from a build or a UI completion; native creation is queued
    /// for the desktop event-loop callback.
    pub fn open_window(
        &mut self,
        options: WindowOptions,
        root: Widget,
    ) -> Result<WindowHandle, WindowError> {
        self.window_manager
            .as_ref()
            .ok_or(WindowError::ApplicationStopped)?
            .open_window(options, root)
    }

    /// Returns a retained UI capability that callbacks may use to open a
    /// window after this build has returned.
    #[must_use]
    pub fn window_opener(&self) -> Option<WindowOpener> {
        self.window_manager
            .as_ref()
            .cloned()
            .map(|manager| WindowOpener { manager })
    }

    /// Returns a command handle for this retained root's owning native window.
    #[must_use]
    pub fn window_handle(&self) -> Option<WindowHandle> {
        let id = self.window_id?;
        self.window_manager.as_ref()?.handle(id)
    }

    /// Returns the scope cancelled as soon as this window closes. Child
    /// component scopes inherit from it automatically.
    #[must_use]
    pub fn window_task_scope(&self) -> TaskScope {
        self.window_scope.clone()
    }

    /// Starts bounded blocking work. Its completion callback runs later on the
    /// UI thread, so it may safely update a local `Signal` or schedule a build.
    pub fn spawn_blocking<T>(
        &self,
        work: impl FnOnce() -> T + Send + 'static,
        complete: impl FnOnce(Result<T, TaskFailure>, &mut Runtime) + 'static,
    ) -> TaskHandle
    where
        T: Send + 'static,
    {
        self.spawner()
            .spawn_blocking_in(&self.window_scope, work, complete)
    }
    /// Starts Tokio work cancelled with this window. Use a `BuildContext` for
    /// work whose completion must be tied to a mounted declarative owner.
    pub fn spawn<F, T>(&self, future: F) -> Task<T>
    where
        F: std::future::Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        self.spawner().spawn_in(&self.window_scope, future)
    }
    /// Starts window-scoped Tokio work with a UI-thread completion.
    pub fn spawn_into<F, T>(
        &self,
        future: F,
        complete: impl FnOnce(Result<T, TaskFailure>, &mut Runtime) + 'static,
    ) -> TaskHandle
    where
        F: std::future::Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        self.spawner()
            .spawn_into_in(&self.window_scope, future, complete)
    }
    /// Processes a bounded batch of UI-relevant Tokio completion messages and
    /// dispatch callbacks. Native runners call this only from their user-event
    /// path; it never polls Tokio or implies a redraw.
    pub fn process_runtime_work_at(&mut self, _now: Instant) {
        let scheduler = self.scheduler.clone();
        // Take message-owned work while borrowing the local scheduler, then
        // release that borrow before any UI callback runs. A completion may
        // safely start its next Tokio task without re-entering `RefCell`.
        let work = scheduler.borrow_mut().take_turn();
        for work in work {
            self.process_ui_work(&scheduler, work);
        }
        if self.reactive.borrow().has_work() || !self.pending.is_empty() {
            self.frame_requested = true;
        }
    }

    pub(crate) fn process_ui_work(
        &mut self,
        scheduler: &Rc<RefCell<tasks::TaskScheduler>>,
        work: tasks::UiWork,
    ) {
        let mut failures = Vec::new();
        match work {
            tasks::UiWork::Dispatch { callback, .. } => {
                if scheduler.borrow().accepting()
                    && std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| callback(self)))
                        .is_err()
                {
                    failures.push(TaskFailure::Panicked);
                }
            }
            tasks::UiWork::Task(pending) => {
                scheduler.borrow().detach_scope(&pending.control);
                let stale_owner = pending.control.owner_is_stale(self);
                if !scheduler.borrow().accepting() || pending.control.is_cancelled() || stale_owner
                {
                    (pending.discard)();
                    scheduler
                        .borrow_mut()
                        .record_discard(pending.blocking, stale_owner);
                } else {
                    #[cfg(feature = "devtools")]
                    let previous_task_context = DEV_TASK_COMPLETION.with(|context| {
                        let previous = context.get();
                        context.set(true);
                        previous
                    });
                    let failure = (pending.finish)(self);
                    #[cfg(feature = "devtools")]
                    DEV_TASK_COMPLETION.with(|context| context.set(previous_task_context));
                    if let Some(failure) = scheduler
                        .borrow_mut()
                        .record_completion(pending.blocking, failure)
                    {
                        failures.push(failure);
                    }
                }
            }
        }
        for failure in failures {
            for observer in &self.error_observers {
                observer(RuntimeErrorReport {
                    failure: failure.clone(),
                });
            }
        }
        if self.reactive.borrow().has_work() || !self.pending.is_empty() {
            self.frame_requested = true;
        }
    }
    pub fn process_runtime_work(&mut self) {
        self.process_runtime_work_at(Instant::now());
    }
    #[must_use]
    pub fn runtime_diagnostics(&self) -> RuntimeDiagnostics {
        self.scheduler.borrow().diagnostics()
    }
    /// Compact ownership-facing state for framework diagnostics. Tokio's own
    /// tracing ecosystem remains the source of executor-level telemetry.
    #[must_use]
    pub fn debug_dump(&self) -> String {
        let diagnostics = self.runtime_diagnostics();
        let (application_tasks, owner_tasks) = self.scheduler.borrow().active_ownership();
        format!(
            "Lifecycle: {:?}\nTokio tasks tracked: {}\n  application: {}\n  owner-scoped: {}\nPending UI messages: {}\nEnvironment generation: {}",
            self.lifecycle,
            diagnostics.active_tracked_tasks,
            application_tasks,
            owner_tasks,
            diagnostics
                .ui_messages_enqueued
                .saturating_sub(diagnostics.ui_messages_processed),
            self.environment_generation,
        )
    }
    #[must_use]
    pub fn environment(&self) -> RuntimeEnvironment {
        self.environment.borrow().clone()
    }
    #[must_use]
    pub const fn lifecycle(&self) -> ApplicationLifecycle {
        self.lifecycle
    }
    #[must_use]
    pub const fn environment_generation(&self) -> u64 {
        self.environment_generation
    }
    /// Replaces normalized runtime values from a platform event. Rebuild is
    /// requested only when the declarative root read a changed value.
    pub fn set_environment(&mut self, environment: RuntimeEnvironment) -> bool {
        let environment = environment.normalized();
        let previous = self.environment.borrow().clone();
        if previous == environment {
            return false;
        }
        let changed = environment_change_mask(&previous, &environment);
        *self.environment.borrow_mut() = environment;
        self.environment_generation = self.environment_generation.wrapping_add(1);
        if self.environment_dependencies.get() & changed != 0
            && let Some(root) = self.application_root
        {
            #[cfg(feature = "devtools")]
            self.tree
                .note_invalidation(root, InvalidationCause::EnvironmentChanged);
            let _ = self.rebuild_from_builder(root);
            self.frame_requested = true;
        }
        true
    }
    pub fn transition_lifecycle(&mut self, lifecycle: ApplicationLifecycle) -> bool {
        if self.lifecycle == lifecycle {
            return false;
        }
        let transition = LifecycleTransition {
            previous: self.lifecycle,
            current: lifecycle,
        };
        self.lifecycle = lifecycle;
        for observer in &self.lifecycle_observers {
            observer(transition);
        }
        if matches!(
            lifecycle,
            ApplicationLifecycle::Stopping | ApplicationLifecycle::Terminated
        ) {
            self.scheduler.borrow_mut().shutdown();
        }
        true
    }
    pub fn observe_lifecycle(&mut self, observer: impl Fn(LifecycleTransition) + 'static) {
        self.lifecycle_observers.push(Rc::new(observer));
    }
    pub fn observe_runtime_error(&mut self, observer: impl Fn(RuntimeErrorReport) + 'static) {
        self.error_observers.push(Rc::new(observer));
    }
    /// Cancels runtime-owned work and prevents later queued callbacks from
    /// touching retained state. Native runners should call this before their
    /// renderer/window teardown.
    pub fn shutdown(&mut self) {
        if self.focused.is_some() {
            self.set_focus(None);
        } else {
            self.sync_text_input_client();
        }
        let _ = self.transition_lifecycle(ApplicationLifecycle::Stopping);
        for (_, scope) in self.owner_scopes.drain() {
            scope.cancel();
        }
        self.scheduler.borrow_mut().shutdown();
        self.lifecycle = ApplicationLifecycle::Terminated;
    }

    /// Releases window-owned subscriptions and callback state without
    /// stopping the shared application scheduler. `Application` calls this
    /// before dropping a closed retained root.
    pub(crate) fn dispose_window(&mut self) {
        if self.focused.is_some() {
            self.set_focus(None);
        } else {
            self.sync_text_input_client();
        }
        self.window_scope.cancel();
        for (_, scope) in self.owner_scopes.drain() {
            scope.cancel();
        }
        self.reactive.borrow_mut().clear();
        self.builders.clear();
        self.pending.clear();
        self.order.clear();
        self.handlers.clear();
        self.text_histories.clear();
        self.text_input_commands.clear();
        self.text_input_client = None;
        self.text_input_configuration = None;
        self.text_input_state = None;
        self.text_input_caret_rect = None;
        self.captured_text_field = None;
        self.captured_selectable_text = None;
        self.hovered_button = None;
        self.pressed_button = None;
    }
    #[must_use]
    pub fn tree(&self) -> &WidgetTree {
        &self.tree
    }

    #[cfg(feature = "devtools")]
    pub(crate) fn devtools_reactive_root(&self) -> ReactiveRootId {
        self.reactive.borrow().root
    }
    #[must_use]
    pub fn tree_mut(&mut self) -> &mut WidgetTree {
        &mut self.tree
    }

    /// Returns the effective capture-protection policy contributed by the
    /// mounted retained tree. The window/application boundary is responsible
    /// for forwarding changes to its native adapter.
    #[must_use]
    pub fn content_sensitivity(&self) -> incular_config::ContentSensitivity {
        self.tree.content_sensitivity()
    }

    /// Visible transient portals after the latest layout. Desktop adapters use
    /// this retained snapshot to bind popup presentation to the owning window
    /// without coupling controls to a native event loop.
    #[must_use]
    pub fn transient_surfaces(&self) -> Vec<incular_widgets::TransientSurfaceSnapshot> {
        self.tree.transient_surfaces()
    }
    #[cfg(feature = "devtools")]
    pub(crate) fn devtools_edit_property(
        &mut self,
        id: incular_devtools_protocol::DevWidgetId,
        name: &str,
        value: &incular_devtools_protocol::DebugValue,
    ) -> bool {
        let edited = self.tree.devtools_edit_property(id, name, value);
        self.frame_requested |= edited;
        edited
    }
    /// Sets the animation-only time scale for this retained root. Runtime
    /// messages, Tokio tasks and profiler wall-clock measurements remain on
    /// the platform monotonic clock.
    pub fn set_animation_time_scale(&mut self, scale: f32) {
        self.tree.set_animation_time_scale(scale);
    }
    #[must_use]
    pub fn focused_element(&self) -> Option<ElementId> {
        self.focused
    }
    /// Dispatches an owned semantic action through the same control state used
    /// by pointer and keyboard input. Native adapters queue these requests;
    /// they never borrow mutable element storage.
    pub fn dispatch_semantic_action(
        &mut self,
        node: SemanticNodeId,
        action: SemanticAction,
    ) -> bool {
        let Some(element) = self.tree.element_for_semantic_node(node) else {
            return false;
        };
        self.tree.note_semantic_action();
        let handled = match action {
            SemanticAction::Focus => {
                self.set_focus(Some(element));
                true
            }
            SemanticAction::Activate => self
                .tree
                .semantic_action_callback(element, incular_semantics::SemanticActionKind::Activate)
                .map(|callback| {
                    callback();
                    true
                })
                .or_else(|| {
                    self.tree
                        .action_for_element(element)
                        .and_then(|action| self.handlers.get(&action).cloned())
                        .map(|callback| {
                            callback();
                            true
                        })
                })
                .unwrap_or(false),
            SemanticAction::SetText(text) => {
                if !self.tree.text_field_is_editable(element) {
                    false
                } else {
                    self.tree
                        .text_controller(element)
                        .map(|controller| {
                            controller.set_text(text);
                            true
                        })
                        .unwrap_or(false)
                }
            }
            SemanticAction::SetSelection { base, extent } => self
                .tree
                .text_controller(element)
                .map(|controller| {
                    let length = controller.text().len();
                    controller.set_selection(TextSelection {
                        base: base.min(length),
                        extent: extent.min(length),
                    });
                    true
                })
                .unwrap_or(false),
            SemanticAction::ScrollForward => self
                .tree
                .semantic_action_callback(
                    element,
                    incular_semantics::SemanticActionKind::ScrollForward,
                )
                .map(|callback| {
                    callback();
                    true
                })
                .unwrap_or_else(|| self.tree.semantic_scroll(element, true)),
            SemanticAction::ScrollBackward => self
                .tree
                .semantic_action_callback(
                    element,
                    incular_semantics::SemanticActionKind::ScrollBackward,
                )
                .map(|callback| {
                    callback();
                    true
                })
                .unwrap_or_else(|| self.tree.semantic_scroll(element, false)),
            SemanticAction::Increment => self
                .tree
                .semantic_action_callback(element, incular_semantics::SemanticActionKind::Increment)
                .map(|callback| {
                    callback();
                    true
                })
                .unwrap_or_else(|| self.tree.dispatch_semantic_increment(element, true)),
            SemanticAction::Decrement => self
                .tree
                .semantic_action_callback(element, incular_semantics::SemanticActionKind::Decrement)
                .map(|callback| {
                    callback();
                    true
                })
                .unwrap_or_else(|| self.tree.dispatch_semantic_increment(element, false)),
        };
        if handled {
            self.frame_requested = true;
        }
        handled
    }
    #[must_use]
    pub fn focus_diagnostics(&self) -> FocusDiagnostics {
        FocusDiagnostics {
            focused_element: self.focused,
            text_pointer_capture: self.captured_text_field,
        }
    }
    #[must_use]
    pub const fn editing_diagnostics(&self) -> EditingDiagnostics {
        self.editing_diagnostics
    }
    pub fn set_clipboard(&mut self, clipboard: Box<dyn Clipboard>) {
        self.clipboard = clipboard;
    }
    pub fn set_clipboard_text(&mut self, text: impl Into<String>) {
        self.clipboard.set_text(text.into());
    }
    /// Processes a normalized platform event without exposing native window or
    /// event-loop types to the runtime API.
    pub fn handle_platform_event(&mut self, event: PlatformEvent) -> Option<EventTarget> {
        match event {
            PlatformEvent::Input(input) => self.handle_input(input),
            PlatformEvent::TextInputAction(action) => {
                self.handle_text_input_action(action);
                None
            }
            PlatformEvent::Metrics(metrics) => {
                self.update_window_metrics(metrics);
                None
            }
            PlatformEvent::Lifecycle(lifecycle) => {
                self.transition_lifecycle(match lifecycle {
                    PlatformLifecycle::Active => ApplicationLifecycle::Active,
                    PlatformLifecycle::Inactive => ApplicationLifecycle::Inactive,
                    PlatformLifecycle::Suspended => ApplicationLifecycle::Suspended,
                    PlatformLifecycle::Stopping => ApplicationLifecycle::Stopping,
                });
                None
            }
            PlatformEvent::CloseRequested => {
                self.shutdown();
                None
            }
        }
    }
    /// Applies the authoritative logical/physical conversion supplied by the
    /// platform metrics owner. Existing environment values such as locale are
    /// preserved while size and scale change together.
    pub fn update_window_metrics(&mut self, metrics: WindowMetrics) -> bool {
        let mut environment = self.environment();
        environment.viewport = metrics.logical_size();
        environment.physical_width = metrics.physical_size.width;
        environment.physical_height = metrics.physical_size.height;
        environment.scale_factor = metrics.scale_factor;
        self.set_environment(environment)
    }
    pub fn register_builder(
        &mut self,
        id: ElementId,
        builder: impl FnMut() -> Widget + 'static,
    ) -> Result<(), TreeError> {
        self.register_builder_without_rebuild(id, builder)?;
        self.rebuild_from_builder(id)
    }
    pub(crate) fn register_builder_without_rebuild(
        &mut self,
        id: ElementId,
        builder: impl FnMut() -> Widget + 'static,
    ) -> Result<(), TreeError> {
        if !self.tree.element_exists(id) {
            return Err(TreeError::MissingElement(id));
        }
        self.builders.insert(id, Box::new(builder));
        Ok(())
    }
    pub(crate) fn install_initial_dependencies(
        &mut self,
        id: ElementId,
        initial_dependencies: Rc<RefCell<InitialBuildDependencies>>,
    ) {
        let initial_dependencies = std::mem::take(&mut *initial_dependencies.borrow_mut());
        let queue = self.reactive.clone();
        let root = queue.borrow().root;
        let queue_weak = Rc::downgrade(&queue);
        for dependency in initial_dependencies.signals {
            dependency.subscribe(root, id, queue_weak.clone());
            queue.borrow_mut().record(id, Rc::downgrade(&dependency));
        }
        for scope in initial_dependencies.focus_scopes {
            install_focus_scope_watch(&queue, id, &scope);
        }
    }
    pub fn schedule_update(&mut self, id: ElementId, widget: Widget) -> Result<(), TreeError> {
        if !self.tree.element_exists(id) {
            return Err(TreeError::MissingElement(id));
        }
        #[cfg(feature = "devtools")]
        self.tree
            .note_invalidation(id, InvalidationCause::WidgetConfigurationChanged);
        self.tree.mark_build(id)?;
        let mut widget = widget;
        self.prepare_widget(&mut widget);
        if self.pending.insert(id, widget).is_none() {
            self.order.push_back(id);
        }
        self.frame_requested = true;
        Ok(())
    }
    #[must_use]
    pub fn frame_requested(&self) -> bool {
        self.frame_requested || self.reactive.borrow().has_work()
    }
    #[must_use]
    pub fn handle_input(&mut self, event: InputEvent) -> Option<EventTarget> {
        let trigger = diagnostic_input_trigger(&event);
        self.handle_input_with_trigger(event, trigger)
    }

    pub(crate) fn handle_input_with_trigger(
        &mut self,
        event: InputEvent,
        trigger: impl Into<String>,
    ) -> Option<EventTarget> {
        self.tree.set_diagnostic_trigger(trigger);
        let started = std::time::Instant::now();
        let target = self.handle_input_inner(event);
        self.pending_event_processing_us = self
            .pending_event_processing_us
            .saturating_add(us_since_instant(started));
        target
    }

    /// Associates subsequent frame work with a higher-level command such as a
    /// semantic simulation click. The trigger survives the input callback so
    /// deferred build/layout failures retain their original cause.
    pub fn set_diagnostic_trigger(&self, trigger: impl Into<String>) {
        self.tree.set_diagnostic_trigger(trigger);
    }
    fn handle_input_inner(&mut self, event: InputEvent) -> Option<EventTarget> {
        let (pointer, device, kind, buttons, phase, position) = match event {
            InputEvent::Pointer { phase, position } => {
                (0, 0, PointerDeviceKind::Mouse, 0, phase, position)
            }
            InputEvent::PointerWithId {
                pointer,
                phase,
                position,
            } => (pointer, 0, PointerDeviceKind::Mouse, 0, phase, position),
            InputEvent::PointerWithMetadata {
                pointer,
                device,
                kind,
                buttons,
                phase,
                position,
            } => (pointer, device, kind, buttons, phase, position),
            event => {
                match event {
                    InputEvent::Scroll { delta } => {
                        // The platform normalizes wheel values to logical pixels. The
                        // latest pointer position selects the nearest viewport.
                        if self.tree.scroll_at(self.last_pointer, delta) {
                            self.frame_requested = true;
                        }
                    }
                    InputEvent::Key(key) => {
                        self.handle_key(key);
                    }
                    InputEvent::Text(text) => {
                        self.insert_text(&text);
                    }
                    InputEvent::Ime(ime) => {
                        self.handle_ime(ime);
                    }
                    InputEvent::WindowResized { .. } => {}
                    InputEvent::Pointer { .. } => unreachable!(),
                    InputEvent::PointerWithId { .. } => unreachable!(),
                    InputEvent::PointerWithMetadata { .. } => unreachable!(),
                }
                return None;
            }
        };
        let legacy_pointer = match phase {
            PointerPhase::Down if self.legacy_pointer.is_none() => {
                self.legacy_pointer = Some(pointer);
                true
            }
            PointerPhase::Move if pointer == 0 && self.legacy_pointer.is_none() => true,
            _ => self.legacy_pointer == Some(pointer),
        };
        if legacy_pointer {
            self.last_pointer = position;
        }
        let gesture_window = self.window_id.map_or(0, |window| {
            (u64::from(window.index()) << 32) | u64::from(window.generation())
        });
        if let Some(element) = self.tree.dispatch_raw_pointer_in_window(
            gesture_window,
            RawPointerEvent {
                pointer,
                device,
                kind,
                buttons,
                position,
                phase,
                time: Instant::now(),
            },
        ) {
            self.frame_requested = true;
            self.release_legacy_pointer(pointer, phase);
            return Some(EventTarget {
                element,
                action: None,
            });
        }
        if let Some(element) = self.tree.dispatch_gesture_in_window(
            gesture_window,
            PointerEvent {
                pointer,
                position,
                phase,
                time: Instant::now(),
            },
        ) {
            self.frame_requested = true;
            self.release_legacy_pointer(pointer, phase);
            return Some(EventTarget {
                element,
                action: None,
            });
        }
        // Legacy controls and scrollbars admit just the primary contact.
        // Additional contacts remain available to retained gesture regions.
        if !legacy_pointer {
            return None;
        }
        if self.tree.scrollbar_pointer(phase, position) {
            self.frame_requested = true;
            self.release_legacy_pointer(pointer, phase);
            return None;
        }
        let text_target = self.tree.text_field_at(position);
        let selectable_target = self.tree.selectable_text_at(position);
        let target = self
            .tree
            .hit_test(position)
            .and_then(|render| self.tree.element_for_render(render))
            .and_then(|element| self.tree.button_ancestor(element));
        let result = match phase {
            PointerPhase::Move => {
                if let Some(field) = self.captured_text_field {
                    self.tree
                        .text_field_set_caret(field, position, true, Instant::now());
                    self.frame_requested = true;
                }
                if let Some(label) = self.captured_selectable_text {
                    if let Some(target) = selectable_target {
                        if self
                            .tree
                            .selectable_text_set_selection(target, position, true)
                        {
                            self.captured_selectable_text = Some(target);
                            self.set_focus(Some(target));
                            self.frame_requested = true;
                        }
                    } else if self.tree.is_selectable_text(label) {
                        self.frame_requested = true;
                    }
                }
                self.set_hover(target.map(|(element, _)| element));
                target.map(|(element, action)| EventTarget { element, action })
            }
            PointerPhase::Down => {
                if let Some(label) = selectable_target {
                    self.set_focus(Some(label));
                    self.captured_selectable_text = Some(label);
                    if self
                        .tree
                        .selectable_text_set_selection(label, position, false)
                    {
                        self.frame_requested = true;
                        return Some(EventTarget {
                            element: label,
                            action: None,
                        });
                    }
                }
                let primary_press = buttons == 0 || buttons & PRIMARY_POINTER_BUTTON != 0;
                if primary_press
                    && selectable_target.is_none()
                    && text_target.is_none()
                    && target.is_none()
                    && let Some((element, interaction)) = self.tree.window_interaction_at(position)
                    && let (Some(window_id), Some(manager)) =
                        (self.window_id, self.window_manager.as_ref())
                {
                    let operation = match interaction {
                        WindowInteraction::Move => WindowOperation::BeginMoveDrag,
                        WindowInteraction::Resize(direction) => {
                            WindowOperation::BeginResizeDrag(direction)
                        }
                    };
                    if manager.send_window_operation(window_id, operation) {
                        self.release_legacy_pointer(pointer, phase);
                        return Some(EventTarget {
                            element,
                            action: None,
                        });
                    }
                }
                let keep_text_field_focus = self
                    .focused
                    .is_some_and(|focused| self.tree.is_text_field(focused))
                    && self.tree.preserves_text_field_focus(position);
                self.set_focus(if keep_text_field_focus {
                    self.focused
                } else {
                    text_target
                });
                self.captured_text_field = text_target;
                if let Some(field) = text_target {
                    self.tree
                        .text_field_set_caret(field, position, false, Instant::now());
                    self.frame_requested = true;
                    return Some(EventTarget {
                        element: field,
                        action: None,
                    });
                }
                self.set_hover(target.map(|(element, _)| element));
                self.pressed_button = target.map(|(element, _)| element);
                if let Some(element) = self.pressed_button {
                    let _ = self
                        .tree
                        .set_button_interaction(element, None, Some(true), None);
                    self.frame_requested = true;
                }
                target.map(|(element, action)| EventTarget { element, action })
            }
            PointerPhase::Up => {
                self.captured_text_field = None;
                self.captured_selectable_text = None;
                let pressed = self.pressed_button.take();
                let valid = pressed
                    .zip(target)
                    .filter(|(pressed, (target, _))| pressed == target)
                    .map(|(_, target)| target);
                if let Some(element) = pressed {
                    let _ = self
                        .tree
                        .set_button_interaction(element, None, Some(false), None);
                }
                if let Some((element, action)) = valid {
                    if let Some(action) = action
                        && let Some(callback) = self.handlers.get(&action).cloned()
                    {
                        callback();
                        self.frame_requested = true;
                    }
                    Some(EventTarget { element, action })
                } else {
                    None
                }
            }
            PointerPhase::Cancel => {
                self.captured_text_field = None;
                self.captured_selectable_text = None;
                if let Some(element) = self.pressed_button.take() {
                    let _ = self
                        .tree
                        .set_button_interaction(element, None, Some(false), None);
                    self.frame_requested = true;
                }
                None
            }
        };
        self.release_legacy_pointer(pointer, phase);
        result
    }
    fn release_legacy_pointer(&mut self, pointer: u64, phase: PointerPhase) {
        if matches!(phase, PointerPhase::Up | PointerPhase::Cancel)
            && self.legacy_pointer == Some(pointer)
        {
            self.legacy_pointer = None;
        }
    }
    fn set_focus(&mut self, next: Option<ElementId>) {
        if self.focused == next {
            return;
        }
        if let Some(previous) = self.focused {
            self.tree.set_keyboard_focus(previous, false);
            let _ = self.tree.set_focused(previous, false, Instant::now());
        }
        self.focused = next;
        if let Some(current) = next {
            let _ = self.tree.set_focused(current, true, Instant::now());
            self.tree.set_keyboard_focus(current, true);
            if self.tree.is_text_field(current) {
                let _ = self.ensure_text_history(current);
            }
        }
        self.frame_requested = true;
        self.sync_text_input_client();
    }

    fn ensure_text_history(&mut self, field: ElementId) -> Option<UndoHistoryController> {
        let editor = self.tree.text_controller(field)?;
        let replace = self
            .text_histories
            .get(&field)
            .is_none_or(|history| history.editor() != editor);
        if replace {
            self.text_histories
                .insert(field, UndoHistoryController::new(editor));
        }
        let history = self.text_histories.get(&field).cloned()?;
        if let Some(max_entries) = self.tree.text_field_history_max_entries(field)
            && history.max_entries() != max_entries
        {
            history.set_max_entries(max_entries);
        }
        Some(history)
    }

    fn sync_text_input_client(&mut self) {
        let snapshot = self
            .focused
            .and_then(|id| self.tree.text_field_input_snapshot(id));
        let Some(snapshot) = snapshot else {
            if let Some(client) = self.text_input_client.take() {
                self.text_input_commands
                    .push_back(TextInputCommand::Hide { client });
                self.text_input_commands
                    .push_back(TextInputCommand::Clear { client });
            }
            self.text_input_configuration = None;
            self.text_input_state = None;
            self.text_input_caret_rect = None;
            return;
        };

        let client = TextInputClientId::new(snapshot.client_id);
        let input_type = match snapshot.input_type {
            TextInputTypeHint::Text if snapshot.multiline => TextInputType::Multiline,
            TextInputTypeHint::Text => TextInputType::Text,
            TextInputTypeHint::Multiline => TextInputType::Multiline,
            TextInputTypeHint::Number => TextInputType::Number,
            TextInputTypeHint::Phone => TextInputType::Phone,
            TextInputTypeHint::Email => TextInputType::Email,
            TextInputTypeHint::Url => TextInputType::Url,
            TextInputTypeHint::Password => TextInputType::Password,
        };
        let action = match snapshot.input_action {
            TextInputActionHint::Unspecified => {
                if snapshot.multiline {
                    TextInputAction::Newline
                } else {
                    TextInputAction::Done
                }
            }
            TextInputActionHint::None => TextInputAction::None,
            TextInputActionHint::Done => TextInputAction::Done,
            TextInputActionHint::Go => TextInputAction::Go,
            TextInputActionHint::Search => TextInputAction::Search,
            TextInputActionHint::Send => TextInputAction::Send,
            TextInputActionHint::Next => TextInputAction::Next,
            TextInputActionHint::Previous => TextInputAction::Previous,
            TextInputActionHint::Newline => TextInputAction::Newline,
        };
        let configuration = TextInputConfiguration {
            client,
            input_type: if snapshot.obscure_text {
                TextInputType::Password
            } else {
                input_type
            },
            action,
            multiline: snapshot.multiline,
            enabled: snapshot.enabled,
            read_only: snapshot.read_only,
            obscure_text: snapshot.obscure_text,
        };
        let state = TextInputState {
            text: snapshot.text,
            selection_start: snapshot.selection.base,
            selection_end: snapshot.selection.extent,
            composing: snapshot.composing.map(|range| (range.start, range.end)),
        };
        let client_changed = self.text_input_client != Some(client);
        let configuration_changed = self.text_input_configuration.as_ref() != Some(&configuration);
        if client_changed || configuration_changed {
            self.text_input_commands
                .push_back(TextInputCommand::SetClient {
                    configuration: configuration.clone(),
                    state: state.clone(),
                    caret_rect: snapshot.caret_rect,
                });
        } else if self.text_input_state.as_ref() != Some(&state) {
            self.text_input_commands
                .push_back(TextInputCommand::Update {
                    client,
                    state: state.clone(),
                });
        }
        if !client_changed
            && self.text_input_caret_rect != Some(snapshot.caret_rect)
            && !configuration_changed
        {
            self.text_input_commands
                .push_back(TextInputCommand::SetCaretRect {
                    client,
                    rect: snapshot.caret_rect,
                });
        }
        self.text_input_client = Some(client);
        self.text_input_configuration = Some(configuration);
        self.text_input_state = Some(state);
        self.text_input_caret_rect = Some(snapshot.caret_rect);
    }
    fn focus_next(&mut self, reverse: bool) {
        let fields = self.tree.focusable_elements();
        if fields.is_empty() {
            return;
        }
        let index = self
            .focused
            .and_then(|focused| fields.iter().position(|id| *id == focused));
        let next = match index {
            Some(index) if reverse => fields[(index + fields.len() - 1) % fields.len()],
            Some(index) => fields[(index + 1) % fields.len()],
            None if reverse => *fields.last().expect("not empty"),
            None => fields[0],
        };
        self.set_focus(Some(next));
    }
    fn handle_key(&mut self, event: KeyboardEvent) {
        if event.state.is_down() {
            self.editing_diagnostics.key_down_received += 1;
        } else {
            self.editing_diagnostics.key_up_received += 1;
        }

        // A FocusNode may be requested by application code or a FocusScopeNode
        // without going through runtime pointer/Tab handling. Adopt that
        // external selection before routing the event through retained parents.
        if let Some(external) = self.tree.focused_keyboard_element()
            && self.focused != Some(external)
        {
            self.set_focus(Some(external));
        }

        // KeyboardListener/Shortcuts are the nearest retained command scopes.
        // Give them first refusal for every transition, including key-up and
        // auto-repeat; text editing and Tab traversal remain fallthrough paths.
        if self.tree.dispatch_keyboard(self.focused, event.clone()) {
            self.frame_requested = true;
            return;
        }
        if !event.state.is_down() {
            return;
        }
        if event.code == Code::Tab {
            self.focus_next(event.modifiers.shift());
            return;
        }
        if let Some(label) = self.focused.filter(|id| self.tree.is_selectable_text(*id)) {
            let extend = event.modifiers.shift();
            let handled = if command_modifier(event.modifiers) {
                match event.code {
                    Code::KeyA => self.tree.selectable_text_select_all(label),
                    Code::KeyC => {
                        if let Some(text) = self.tree.selectable_text_selected_text(label) {
                            self.clipboard.set_text(text);
                            true
                        } else {
                            false
                        }
                    }
                    _ => false,
                }
            } else {
                match event.code {
                    Code::ArrowLeft => self.tree.selectable_text_move(label, false, extend),
                    Code::ArrowRight => self.tree.selectable_text_move(label, true, extend),
                    Code::Home => self.tree.selectable_text_move_to_edge(label, false, extend),
                    Code::End => self.tree.selectable_text_move_to_edge(label, true, extend),
                    _ => false,
                }
            };
            if handled {
                self.frame_requested = true;
            }
            return;
        }
        let Some(field) = self.focused.filter(|id| self.tree.is_text_field(*id)) else {
            return;
        };
        let Some(controller) = self.tree.text_controller(field) else {
            return;
        };
        let editable = self.tree.text_field_is_editable(field);
        let extend = event.modifiers.shift();
        if command_modifier(event.modifiers) {
            match event.code {
                Code::KeyA => controller.select_all(),
                Code::KeyC => self.clipboard.set_text(controller.selected_text()),
                Code::KeyZ => {
                    let Some(history) = self.ensure_text_history(field) else {
                        return;
                    };
                    let changed = if event.modifiers.shift() {
                        history.redo()
                    } else {
                        history.undo()
                    };
                    if !changed {
                        return;
                    }
                }
                Code::KeyY => {
                    let Some(history) = self.ensure_text_history(field) else {
                        return;
                    };
                    if !history.redo() {
                        return;
                    }
                }
                Code::KeyX => {
                    if !editable {
                        return;
                    }
                    self.clipboard.set_text(controller.selected_text());
                    controller.replace_selection("");
                }
                Code::KeyV => {
                    if !editable {
                        return;
                    }
                    if let Some(text) = self.clipboard.get_text() {
                        controller.insert(&text);
                    }
                }
                _ => return,
            }
        } else {
            match event.code {
                Code::Backspace => {
                    if !editable {
                        return;
                    }
                    controller.backspace();
                    self.editing_diagnostics.backspace_commands += 1;
                }
                Code::Delete => {
                    if !editable {
                        return;
                    }
                    controller.delete();
                    self.editing_diagnostics.delete_commands += 1;
                }
                Code::ArrowLeft => {
                    if !self.tree.text_field_move_horizontal(field, false, extend) {
                        controller.move_left(extend);
                    }
                }
                Code::ArrowRight => {
                    if !self.tree.text_field_move_horizontal(field, true, extend) {
                        controller.move_right(extend);
                    }
                }
                Code::ArrowUp => {
                    let _ = self.tree.text_field_move_vertical(field, false, extend);
                }
                Code::ArrowDown => {
                    let _ = self.tree.text_field_move_vertical(field, true, extend);
                }
                Code::Home => {
                    if !self.tree.text_field_move_line_edge(field, false, extend) {
                        controller.move_home(extend);
                    }
                }
                Code::End => {
                    if !self.tree.text_field_move_line_edge(field, true, extend) {
                        controller.move_end(extend);
                    }
                }
                Code::Enter => {
                    if !editable {
                        return;
                    }
                    if self.tree.is_multiline_text_field(field) {
                        controller.insert("\n");
                    } else {
                        self.tree.submit_text_field(field);
                    }
                }
                _ => return,
            }
        }
        controller.reset_caret(Instant::now());
        self.frame_requested = true;
    }
    fn insert_text(&mut self, text: &str) {
        let Some(field) = self.focused.filter(|id| self.tree.is_text_field(*id)) else {
            return;
        };
        if !self.tree.text_field_is_editable(field) {
            return;
        }
        if text.chars().any(char::is_control) {
            return;
        }
        if let Some(controller) = self.tree.text_controller(field) {
            controller.insert(text);
            self.editing_diagnostics.text_commits += 1;
            controller.reset_caret(Instant::now());
            self.frame_requested = true;
        }
    }

    /// Handles the action chosen by a native software keyboard. Desktop
    /// Winit backends reach the same code path through Enter, while mobile
    /// hosts can preserve the IME's explicit Done/Next/Search intent.
    pub fn handle_text_input_action(&mut self, action: TextInputAction) -> bool {
        let Some(field) = self.focused.filter(|id| self.tree.is_text_field(*id)) else {
            return false;
        };
        let editable = self.tree.text_field_is_editable(field);
        if !editable {
            return false;
        }
        let handled = match action {
            TextInputAction::Newline => {
                if self.tree.is_multiline_text_field(field) {
                    self.tree.text_controller(field).is_some_and(|controller| {
                        controller.insert("\n");
                        controller.reset_caret(Instant::now());
                        true
                    })
                } else {
                    self.tree.submit_text_field(field)
                }
            }
            TextInputAction::Next => {
                let submitted = self.tree.submit_text_field(field);
                self.focus_next(false);
                submitted
            }
            TextInputAction::Previous => {
                let submitted = self.tree.submit_text_field(field);
                self.focus_next(true);
                submitted
            }
            TextInputAction::Done
            | TextInputAction::Go
            | TextInputAction::Search
            | TextInputAction::Send => self.tree.submit_text_field(field),
            TextInputAction::Unspecified | TextInputAction::None => false,
        };
        if handled {
            self.frame_requested = true;
        }
        handled
    }
    fn handle_ime(&mut self, event: ImeEvent) {
        self.editing_diagnostics.ime_events += 1;
        let Some(field) = self.focused.filter(|id| self.tree.is_text_field(*id)) else {
            return;
        };
        let Some(controller) = self.tree.text_controller(field) else {
            return;
        };
        if !self.tree.text_field_is_editable(field) {
            return;
        }
        match event {
            ImeEvent::Preedit { text, selection } => controller.set_preedit(
                text,
                selection.map(|(start, end)| TextRange::new(start, end)),
            ),
            ImeEvent::Commit(text) => controller.commit_preedit(&text),
            ImeEvent::End => controller.clear_preedit(),
        }
        controller.reset_caret(Instant::now());
        self.frame_requested = true;
    }
    pub fn run_frame(
        &mut self,
        constraints: Constraints,
    ) -> Result<(DisplayList, FrameStats), TreeError> {
        self.run_frame_at(constraints, Instant::now())
    }
    /// Deterministic frame entry point used by animation tests and embedders
    /// with an existing monotonic clock.
    pub fn run_frame_at(
        &mut self,
        constraints: Constraints,
        now: Instant,
    ) -> Result<(DisplayList, FrameStats), TreeError> {
        let message_guard = tracing::info_span!("incular.messages").entered();
        let message_span = profiling::PhaseSpan::start();
        self.process_runtime_work_at(now);
        let runtime_messages = message_span.elapsed_us();
        drop(message_guard);
        let before = self.tree.diagnostics();
        let build_guard =
            tracing::info_span!("incular.build", elements_updated = tracing::field::Empty,)
                .entered();
        let build_span = profiling::PhaseSpan::start();
        let mut updated = 0;
        while let Some(id) = self.order.pop_front() {
            if let Some(widget) = self.pending.remove(&id)
                && self.tree.element_exists(id)
            {
                self.tree.update(id, widget)?;
                updated += 1;
            }
        }
        loop {
            while let Some((root, node)) = { self.reactive.borrow_mut().take_node() } {
                if let Some(node) = node.upgrade() {
                    node.run(root);
                }
            }
            while let Some(id) = { self.reactive.borrow_mut().take() } {
                if self.tree.element_exists(id) && self.builders.contains_key(&id) {
                    #[cfg(feature = "devtools")]
                    for cause in self.reactive.borrow_mut().take_causes(id) {
                        self.tree.note_invalidation(id, cause);
                    }
                    self.rebuild_from_builder(id)?;
                    updated += 1;
                }
            }
            if !self.reactive.borrow().has_work() {
                break;
            }
        }
        for id in self.tree.take_unmounted() {
            self.builders.remove(&id);
            self.reactive.borrow_mut().forget(id);
            if let Some(scope) = self.owner_scopes.remove(&id) {
                scope.cancel();
            }
        }
        self.prune_handlers();
        let build = build_span.elapsed_us();
        drop(build_guard);
        let _layout_guard = tracing::info_span!("incular.layout").entered();
        let layout_span = profiling::PhaseSpan::start();
        self.tree.layout(constraints)?;
        let layout = layout_span.elapsed_us();
        drop(_layout_guard);
        for (action, handler) in self.tree.take_pending_handlers() {
            self.handlers.insert(action, handler);
        }
        // Lazy viewport expiry occurs during layout, after the ordinary dirty
        // queue drain above. Release those builder subscriptions and callbacks
        // in the same frame rather than retaining one stale cache generation.
        for id in self.tree.take_unmounted() {
            self.builders.remove(&id);
            self.reactive.borrow_mut().forget(id);
            if let Some(scope) = self.owner_scopes.remove(&id) {
                scope.cancel();
            }
        }
        self.prune_handlers();
        let _composite_guard = tracing::info_span!("incular.composite").entered();
        let composite_span = profiling::PhaseSpan::start();
        let (composited, animations_active) = self.tree.update_compositor(now)?;
        let composite = composite_span.elapsed_us();
        drop(_composite_guard);
        let _semantics_guard = tracing::info_span!("incular.semantics").entered();
        let semantics_span = profiling::PhaseSpan::start();
        self.tree.update_semantics();
        let semantics = semantics_span.elapsed_us();
        drop(_semantics_guard);
        let _paint_guard = tracing::info_span!("incular.paint").entered();
        let paint_span = profiling::PhaseSpan::start();
        let display_list = self.tree.paint();
        let paint = paint_span.elapsed_us();
        drop(_paint_guard);
        self.sync_text_input_client();
        self.frame_requested =
            !self.pending.is_empty() || self.reactive.borrow().has_work() || animations_active;
        let after = self.tree.diagnostics();
        let timings = FrameTimings {
            event_processing: std::mem::take(&mut self.pending_event_processing_us),
            runtime_messages,
            build,
            layout,
            composite,
            semantics,
            paint,
            cpu_total: runtime_messages + build + layout + composite + semantics + paint,
        };
        tracing::debug!(
            target: "incular::frame",
            cpu_total_us = timings.cpu_total,
            build_us = timings.build,
            layout_us = timings.layout,
            paint_us = timings.paint,
        );
        Ok((
            display_list.clone(),
            FrameStats {
                updated_elements: updated,
                rebuilt_elements: after.rebuilds - before.rebuilds,
                laid_out_render_objects: after.layouts - before.layouts,
                repainted_render_objects: after.paints - before.paints,
                composited: u64::from(composited),
                active_animations: u64::from(animations_active),
                display_list_commands: display_list.len(),
                requested_another_frame: self.frame_requested,
                timings,
            },
        ))
    }
    #[must_use]
    pub fn diagnostics(&self) -> Diagnostics {
        self.tree.diagnostics()
    }
    fn rebuild_from_builder(&mut self, id: ElementId) -> Result<(), TreeError> {
        self.reactive.borrow_mut().refresh(id);
        let widget = {
            let _build_scope = BuildScopeGuard::enter(BuildScope {
                root: self.reactive.borrow().root,
                element: Some(id),
                queue: Rc::downgrade(&self.reactive),
                initial_dependencies: None,
                spawner: self.spawner(),
                owner_scope: self
                    .owner_scopes
                    .get(&id)
                    .cloned()
                    .unwrap_or_else(|| self.window_scope.clone()),
            });
            self.builders.get_mut(&id).expect("registered builder")()
        };
        let mut widget = widget;
        self.prepare_widget(&mut widget);
        self.tree.update(id, widget)?;
        self.prune_handlers();
        Ok(())
    }
    fn prepare_widget(&mut self, widget: &mut Widget) {
        let handlers = &mut self.handlers;
        let tree = &mut self.tree;
        widget.bind_callbacks(&mut |callback| {
            let id = tree.allocate_action();
            handlers.insert(id, callback);
            id
        });
    }
    fn prune_handlers(&mut self) {
        let active = self.tree.action_ids();
        self.handlers.retain(|id, _| active.contains(id));
    }
    fn set_hover(&mut self, next: Option<ElementId>) {
        if self.hovered_button == next {
            return;
        }
        if let Some(previous) = self.hovered_button {
            let _ = self
                .tree
                .set_button_interaction(previous, Some(false), None, None);
            if let Some(action) = self.tree.hover_actions_for_element(previous).1
                && let Some(callback) = self.handlers.get(&action).cloned()
            {
                callback();
            }
        }
        self.hovered_button = next;
        if let Some(current) = next {
            if self.pressed_button != Some(current) {
                let _ = self
                    .tree
                    .set_button_interaction(current, Some(true), None, None);
            }
            if let Some(action) = self.tree.hover_actions_for_element(current).0
                && let Some(callback) = self.handlers.get(&action).cloned()
            {
                callback();
            }
        }
        self.frame_requested = true;
    }
}

/// Conventional desktop shortcut modifier: Control on Linux/Windows and Meta
/// (Command) on macOS.  `keyboard-types` retains the actual modifier bits;
/// this policy belongs to the runtime command dispatcher rather than input
/// normalization.
fn command_modifier(modifiers: Modifiers) -> bool {
    modifiers.ctrl() || modifiers.meta()
}

/// Framework-controlled build scope for declarative roots. It intentionally
/// exposes no element IDs or scheduler handles. Signals use the runtime's
/// scoped collector while a builder executes; reads outside a build simply
/// do not create dependencies.
fn us_since_instant(started: std::time::Instant) -> u32 {
    started.elapsed().as_micros().try_into().unwrap_or(u32::MAX)
}
