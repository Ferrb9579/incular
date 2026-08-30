//! Controlled BUILD → LAYOUT → PAINT coordination and local reactive state.
mod profiling;
mod reactive;
mod restoration;
mod simulation;
mod tasks;
mod undo;

use incular_accessibility::{
    AccessKitProjection, NativeAccessibilityUpdate, SemanticActionRequest,
};
use incular_config::Constraints;
use incular_config::RuntimeEnvironment;
use incular_core::{
    Code, ImeEvent, InputEvent, KeyboardEvent, Modifiers, Offset, PointerPhase, Rect,
    RestorationKey, RestorationScope,
};
use incular_platform::{
    Clipboard, MemoryClipboard, PlatformEvent, PlatformLifecycle, TextInputAction,
    TextInputClientId, TextInputCommand, TextInputConfiguration, TextInputState, TextInputType,
    WindowCommand, WindowEvent, WindowEventKind, WindowId, WindowLifecycle, WindowMetrics,
    WindowOperation, WindowOptions, WindowOptionsError,
};
use incular_rendering::DisplayList;
use incular_semantics::{SemanticAction, SemanticNodeId};
#[cfg(feature = "devtools")]
use incular_widgets::internal::InvalidationCause;
use incular_widgets::internal::{
    ActionId, Diagnostics, ElementId, Key, PointerEvent, TextRange, TextSelection, TreeError,
    WidgetTree,
};
use incular_widgets::{
    FocusScopeNode, FocusScopeSubscription, TextInputActionHint, TextInputTypeHint, Widget,
};
#[cfg(feature = "devtools")]
use std::any::{Any, TypeId};
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet, VecDeque},
    rc::{Rc, Weak},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::Instant,
};

pub use incular_accessibility::AccessibilityDiagnostics;
pub use profiling::{
    AccessibilitySnapshot, BudgetStatistics, FrameHistory, FrameRecord, FrameStatistics,
    FrameTimings, FrameWork, GpuSample, PerformanceHub, PerformanceProfiler, PerformanceSnapshot,
    ProfilerMode, RenderFrameMetrics, SchedulerCounters, TextCacheSnapshot, WidgetWorkSnapshot,
    WindowPerformance,
};
pub use reactive::{Action, ActionDispatchError, ActionError, ActionState, Effect, Memo};
pub use restoration::{
    DEFAULT_RESTORATION_DEBOUNCE, DEFAULT_RESTORATION_SNAPSHOT_LIMIT,
    FRAMEWORK_RESTORATION_FORMAT_VERSION, FileRestorationStore, InMemoryRestorationStore,
    Restorable, RestorationConfig, RestorationDiagnostics, RestorationHandle, RestorationMigration,
    RestorationStore, RestorationStoreError,
};
pub use simulation::{Screenshot, Simulation, SimulationError};
pub use tasks::{
    AsyncState, AsyncValue, RuntimeDiagnostics, RuntimeSpawner, RuntimeWake, Task, TaskFailure,
    TaskHandle, TaskScope, TokioHandle, UiDispatcher,
};
pub use undo::{UndoHistoryController, UndoHistoryState};

/// Process-wide scheduler/reactivity counters. Relaxed atomics on the UI
/// thread cost a single increment per event and never allocate.
mod scheduler_counters {
    use std::sync::atomic::{AtomicU64, Ordering};
    pub static SIGNAL_READS: AtomicU64 = AtomicU64::new(0);
    pub static SIGNAL_WRITES: AtomicU64 = AtomicU64::new(0);
    pub static DEPENDENTS_ENQUEUED: AtomicU64 = AtomicU64::new(0);
    pub static RUNTIME_WAKES: AtomicU64 = AtomicU64::new(0);
    pub static REDRAW_REQUESTS: AtomicU64 = AtomicU64::new(0);
    pub static FRAMES_STARTED: AtomicU64 = AtomicU64::new(0);
    pub static FRAMES_PRESENTED: AtomicU64 = AtomicU64::new(0);
    pub static FRAMES_SKIPPED: AtomicU64 = AtomicU64::new(0);
    #[must_use]
    pub fn load(counter: &AtomicU64) -> u64 {
        counter.load(Ordering::Relaxed)
    }
}

#[must_use]
pub fn scheduler_counters() -> SchedulerCounters {
    SchedulerCounters {
        signal_reads: scheduler_counters::load(&scheduler_counters::SIGNAL_READS),
        signal_writes: scheduler_counters::load(&scheduler_counters::SIGNAL_WRITES),
        dependents_enqueued: scheduler_counters::load(&scheduler_counters::DEPENDENTS_ENQUEUED),
        runtime_wakes: scheduler_counters::load(&scheduler_counters::RUNTIME_WAKES),
        redraw_requests: scheduler_counters::load(&scheduler_counters::REDRAW_REQUESTS),
        frames_started: scheduler_counters::load(&scheduler_counters::FRAMES_STARTED),
        frames_presented: scheduler_counters::load(&scheduler_counters::FRAMES_PRESENTED),
        frames_skipped: scheduler_counters::load(&scheduler_counters::FRAMES_SKIPPED),
    }
}

/// Runtime and frame counters owned by one [`Application`]. The free
/// [`scheduler_counters`] function intentionally remains a process-wide
/// diagnostic for callers that need aggregate totals, while an application's
/// performance snapshot must not be affected by another application's test,
/// preview, or embedded runtime.
#[derive(Clone, Copy, Debug, Default)]
struct ApplicationSchedulerCounters {
    runtime_wakes: u64,
    redraw_requests: u64,
    frames_started: u64,
    frames_presented: u64,
    frames_skipped: u64,
}

impl ApplicationSchedulerCounters {
    fn apply_to(self, scheduler: &mut SchedulerCounters) {
        scheduler.runtime_wakes = self.runtime_wakes;
        scheduler.redraw_requests = self.redraw_requests;
        scheduler.frames_started = self.frames_started;
        scheduler.frames_presented = self.frames_presented;
        scheduler.frames_skipped = self.frames_skipped;
    }
}

/// Backend-neutral application lifecycle. Desktop adapters may only emit a
/// subset; runtime users must therefore treat transitions as advisory rather
/// than assume every state is observable on every platform.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ApplicationLifecycle {
    #[default]
    Starting,
    Active,
    Inactive,
    Suspended,
    Stopping,
    Terminated,
}

/// A lifecycle update normalized by a platform adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LifecycleTransition {
    pub previous: ApplicationLifecycle,
    pub current: ApplicationLifecycle,
}

/// Runtime-level failure report intended for application logging/telemetry
/// hooks. Framework panics never unwind into the native event-loop callback.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeErrorReport {
    pub failure: TaskFailure,
}

/// Policy applied after a close accepts the final visible Incular window.
/// `ExitOnLastWindow` is the default so a hidden auxiliary window cannot keep
/// an application alive accidentally. `KeepRunning` is explicit headless or
/// background-service policy; a native adapter remains free to wait idle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LastWindowPolicy {
    #[default]
    ExitOnLastWindow,
    KeepRunning,
}

/// Failure while creating or addressing an Incular-owned window.
#[derive(Debug)]
pub enum WindowError {
    Options(WindowOptionsError),
    Tree(TreeError),
    Restoration(String),
    ApplicationStopped,
}

impl From<WindowOptionsError> for WindowError {
    fn from(error: WindowOptionsError) -> Self {
        Self::Options(error)
    }
}

impl From<TreeError> for WindowError {
    fn from(error: TreeError) -> Self {
        Self::Tree(error)
    }
}

impl std::fmt::Display for WindowError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Options(error) => write!(formatter, "invalid window options: {error}"),
            Self::Tree(error) => write!(formatter, "window root could not mount: {error:?}"),
            Self::Restoration(error) => write!(formatter, "invalid restoration window: {error}"),
            Self::ApplicationStopped => formatter.write_str("the Incular application has stopped"),
        }
    }
}

/// Application-defined identity for a window that should survive a restart.
/// It is intentionally unrelated to the session-local generational
/// [`WindowId`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct WindowRestorationId(RestorationKey);

impl WindowRestorationId {
    pub fn new(value: impl Into<String>) -> Result<Self, incular_core::RestorationKeyError> {
        RestorationKey::new(value).map(Self)
    }

    #[must_use]
    pub fn as_key(&self) -> &RestorationKey {
        &self.0
    }
}

/// Application-owned factory for a persisted auxiliary window kind. It is
/// registered in memory at startup; it is never serialized.
pub type RestorableWindowFactory = Rc<dyn Fn(&mut BuildContext) -> Widget>;

impl TryFrom<&str> for WindowRestorationId {
    type Error = incular_core::RestorationKeyError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

#[derive(Clone)]
struct RestorableWindowMetadata {
    id: WindowRestorationId,
    kind: String,
}

impl RestorableWindowMetadata {
    fn new(id: WindowRestorationId, kind: impl Into<String>) -> Result<Self, WindowError> {
        let kind = kind.into();
        if kind.trim().is_empty() || kind.chars().any(char::is_control) {
            return Err(WindowError::Restoration(
                "window kind must be a non-empty printable stable identifier".to_owned(),
            ));
        }
        Ok(Self { id, kind })
    }
}

impl std::error::Error for WindowError {}

/// A portable UI-thread command emitted by the application for a native
/// desktop adapter. It contains Incular IDs and options only—never Winit
/// IDs, native pointers, or a GPU surface.
#[derive(Clone, Debug, PartialEq)]
pub enum NativeWindowCommand {
    Create {
        window_id: WindowId,
        options: WindowOptions,
    },
    Operate(WindowCommand),
}

/// A thread-safe reference to a generational Incular window. Methods only
/// enqueue data for the UI/event-loop turn; they never touch native state.
#[derive(Clone)]
pub struct WindowHandle {
    id: WindowId,
    bridge: Arc<WindowCommandBridge>,
}

impl std::fmt::Debug for WindowHandle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("WindowHandle")
            .field(&self.id)
            .finish()
    }
}

impl WindowHandle {
    #[must_use]
    pub const fn id(&self) -> WindowId {
        self.id
    }

    pub fn set_title(&self, title: impl Into<String>) -> bool {
        self.send(WindowOperation::SetTitle(title.into()))
    }

    pub fn set_visible(&self, visible: bool) -> bool {
        self.send(WindowOperation::SetVisible(visible))
    }

    pub fn request_logical_size(&self, size: incular_core::Size) -> bool {
        self.send(WindowOperation::SetLogicalSize(size))
    }

    pub fn request_focus(&self) -> bool {
        self.send(WindowOperation::RequestFocus)
    }

    pub fn request_redraw(&self) -> bool {
        self.send(WindowOperation::RequestRedraw)
    }

    pub fn close(&self) -> bool {
        self.send(WindowOperation::Close)
    }

    fn send(&self, operation: WindowOperation) -> bool {
        self.bridge.send(WindowCommand::new(self.id, operation))
    }
}

/// Cloneable UI-side capability for opening retained windows after startup.
/// It contains no native handle and can be retained by button callbacks; native
/// creation is still queued for the active desktop event-loop callback.
#[derive(Clone)]
pub struct WindowOpener {
    manager: WindowManager,
}

impl WindowOpener {
    pub fn open_window(
        &self,
        options: WindowOptions,
        root: Widget,
    ) -> Result<WindowHandle, WindowError> {
        self.manager.open_window(options, root)
    }

    pub fn open_window_with(
        &self,
        options: WindowOptions,
        build: impl FnMut(&mut BuildContext) -> Widget + 'static,
    ) -> Result<WindowHandle, WindowError> {
        self.manager.open_window_with(options, build)
    }

    /// Opens a window whose descriptor is persisted under a stable
    /// application-provided ID. Its factory still lives in application code;
    /// no widget or native handle is serialized.
    pub fn open_restorable_window_with(
        &self,
        restoration_id: WindowRestorationId,
        kind: impl Into<String>,
        options: WindowOptions,
        build: impl FnMut(&mut BuildContext) -> Widget + 'static,
    ) -> Result<WindowHandle, WindowError> {
        self.manager
            .open_restorable_window_with(restoration_id, kind, options, build)
    }
}

struct WindowCommandBridge {
    sender: mpsc::Sender<WindowCommand>,
    wake: Mutex<Option<Arc<dyn RuntimeWake>>>,
}

impl WindowCommandBridge {
    fn send(&self, command: WindowCommand) -> bool {
        if self.sender.send(command).is_err() {
            return false;
        }
        if let Some(wake) = self
            .wake
            .lock()
            .expect("window command wake mutex")
            .as_ref()
        {
            wake.wake();
        }
        true
    }

    fn set_wake(&self, wake: Arc<dyn RuntimeWake>) {
        *self.wake.lock().expect("window command wake mutex") = Some(wake);
    }
}

const ENV_VIEWPORT: u16 = 1 << 0;
const ENV_SCALE: u16 = 1 << 1;
const ENV_BRIGHTNESS: u16 = 1 << 2;
const ENV_TEXT_SCALE: u16 = 1 << 3;
const ENV_SAFE_INSETS: u16 = 1 << 4;
const ENV_VIEW_INSETS: u16 = 1 << 5;
const ENV_LOCALE: u16 = 1 << 6;
const ENV_DIRECTION: u16 = 1 << 7;
const ENV_REDUCED_MOTION: u16 = 1 << 8;
const ENV_INPUT: u16 = 1 << 9;
const ENV_WINDOW_FOCUS: u16 = 1 << 10;
const ENV_ALL: u16 = (1 << 11) - 1;

fn environment_change_mask(previous: &RuntimeEnvironment, next: &RuntimeEnvironment) -> u16 {
    let mut mask = 0;
    if previous.viewport != next.viewport
        || previous.physical_width != next.physical_width
        || previous.physical_height != next.physical_height
    {
        mask |= ENV_VIEWPORT;
    }
    if previous.scale_factor != next.scale_factor {
        mask |= ENV_SCALE;
    }
    if previous.brightness != next.brightness {
        mask |= ENV_BRIGHTNESS;
    }
    if previous.text_scale != next.text_scale {
        mask |= ENV_TEXT_SCALE;
    }
    if previous.safe_insets != next.safe_insets {
        mask |= ENV_SAFE_INSETS;
    }
    if previous.view_insets != next.view_insets {
        mask |= ENV_VIEW_INSETS;
    }
    if previous.locales != next.locales {
        mask |= ENV_LOCALE;
    }
    if previous.text_direction != next.text_direction {
        mask |= ENV_DIRECTION;
    }
    if previous.reduced_motion != next.reduced_motion {
        mask |= ENV_REDUCED_MOTION;
    }
    if previous.input != next.input {
        mask |= ENV_INPUT;
    }
    if previous.window_focused != next.window_focused {
        mask |= ENV_WINDOW_FOCUS;
    }
    mask
}

type ReactiveRootId = u64;
type ReactiveNodeId = u64;
type ReactiveNodeDependents =
    HashMap<ReactiveRootId, HashMap<ReactiveNodeId, Weak<dyn ReactiveNode>>>;

static NEXT_REACTIVE_ROOT: AtomicU64 = AtomicU64::new(1);
static NEXT_REACTIVE_NODE: AtomicU64 = AtomicU64::new(1);

trait ReactiveNode {
    fn id(&self) -> ReactiveNodeId;
    fn mark_dirty(&self);
    fn run(&self, root: ReactiveRootId);
    fn record_dependency(&self, dependency: Weak<dyn Dependency>);
    fn dispose(&self, root: ReactiveRootId);
}

trait Dependency {
    fn subscribe(
        &self,
        root: ReactiveRootId,
        element: ElementId,
        queue: Weak<RefCell<ReactiveQueue>>,
    );
    fn remove(&self, root: ReactiveRootId, element: ElementId);
    fn subscribe_node(
        &self,
        root: ReactiveRootId,
        node: ReactiveNodeId,
        subscriber: Weak<dyn ReactiveNode>,
        queue: Weak<RefCell<ReactiveQueue>>,
    );
    fn remove_node(&self, root: ReactiveRootId, node: ReactiveNodeId);
}
struct ReactiveQueue {
    root: ReactiveRootId,
    queued: HashSet<ElementId>,
    order: VecDeque<ElementId>,
    queued_nodes: HashSet<ReactiveNodeId>,
    node_order: VecDeque<ReactiveNodeId>,
    nodes: HashMap<ReactiveNodeId, Weak<dyn ReactiveNode>>,
    dependencies: HashMap<ElementId, Vec<Weak<dyn Dependency>>>,
    /// Focus-scope subscriptions registered by builders. Keeping the token in
    /// the same lifetime bucket as signal dependencies makes unmount and
    /// rebuild cleanup deterministic rather than leaking callbacks into a
    /// long-lived scope node.
    focus_scopes: HashMap<ElementId, Vec<FocusScopeSubscription>>,
    #[cfg(feature = "devtools")]
    causes: HashMap<ElementId, Vec<InvalidationCause>>,
}
impl ReactiveQueue {
    fn new() -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            root: NEXT_REACTIVE_ROOT.fetch_add(1, Ordering::Relaxed),
            queued: HashSet::new(),
            order: VecDeque::new(),
            queued_nodes: HashSet::new(),
            node_order: VecDeque::new(),
            nodes: HashMap::new(),
            dependencies: HashMap::new(),
            focus_scopes: HashMap::new(),
            #[cfg(feature = "devtools")]
            causes: HashMap::new(),
        }))
    }

    fn enqueue(&mut self, id: ElementId) {
        if self.queued.insert(id) {
            self.order.push_back(id);
        }
    }
    fn enqueue_node(&mut self, id: ReactiveNodeId, node: Weak<dyn ReactiveNode>) {
        self.register_node(id, node);
        if self.queued_nodes.insert(id) {
            self.node_order.push_back(id);
        }
    }
    fn register_node(&mut self, id: ReactiveNodeId, node: Weak<dyn ReactiveNode>) {
        self.nodes.insert(id, node);
    }
    #[cfg(feature = "devtools")]
    fn note_cause(&mut self, id: ElementId, cause: InvalidationCause) {
        const MAX_CAUSES: usize = 8;
        let causes = self.causes.entry(id).or_default();
        if causes.len() == MAX_CAUSES {
            causes.remove(0);
        }
        causes.push(cause);
    }
    #[cfg(feature = "devtools")]
    fn take_causes(&mut self, id: ElementId) -> Vec<InvalidationCause> {
        self.causes.remove(&id).unwrap_or_default()
    }
    fn take(&mut self) -> Option<ElementId> {
        let id = self.order.pop_front()?;
        self.queued.remove(&id);
        Some(id)
    }
    fn take_node(&mut self) -> Option<(ReactiveRootId, Weak<dyn ReactiveNode>)> {
        let id = self.node_order.pop_front()?;
        self.queued_nodes.remove(&id);
        self.nodes.get(&id).cloned().map(|node| (self.root, node))
    }
    fn has_work(&self) -> bool {
        !self.queued.is_empty() || !self.queued_nodes.is_empty()
    }
    fn refresh(&mut self, id: ElementId) {
        if let Some(deps) = self.dependencies.remove(&id) {
            for dep in deps {
                if let Some(dep) = dep.upgrade() {
                    dep.remove(self.root, id);
                }
            }
        }
        self.focus_scopes.remove(&id);
    }
    fn record(&mut self, id: ElementId, dep: Weak<dyn Dependency>) {
        let entries = self.dependencies.entry(id).or_default();
        if !entries.iter().any(|current| current.ptr_eq(&dep)) {
            entries.push(dep);
        }
    }

    fn watch_focus_scope(&mut self, id: ElementId, subscription: FocusScopeSubscription) {
        self.focus_scopes.entry(id).or_default().push(subscription);
    }
    fn forget(&mut self, id: ElementId) {
        self.refresh(id);
        self.queued.remove(&id);
        #[cfg(feature = "devtools")]
        self.causes.remove(&id);
    }

    fn clear(&mut self) {
        // Focus-only builders do not appear in `dependencies`; include their
        // owner IDs so closing a window drops every subscription token too.
        let mut ids: HashSet<_> = self.dependencies.keys().copied().collect();
        ids.extend(self.focus_scopes.keys().copied());
        let ids: Vec<_> = ids.into_iter().collect();
        for id in ids {
            self.refresh(id);
        }
        self.queued.clear();
        self.order.clear();
        let nodes: Vec<_> = self.nodes.values().filter_map(Weak::upgrade).collect();
        self.queued_nodes.clear();
        self.node_order.clear();
        self.nodes.clear();
        for node in nodes {
            node.dispose(self.root);
        }
        #[cfg(feature = "devtools")]
        self.causes.clear();
    }
}
fn install_focus_scope_watch(
    queue: &Rc<RefCell<ReactiveQueue>>,
    element: ElementId,
    scope: &FocusScopeNode,
) {
    let weak_queue = Rc::downgrade(queue);
    let subscription = scope.observe(move || {
        if let Some(queue) = weak_queue.upgrade() {
            let mut queue = queue.borrow_mut();
            queue.enqueue(element);
            #[cfg(feature = "devtools")]
            queue.note_cause(element, InvalidationCause::Manual);
        }
    });
    queue.borrow_mut().watch_focus_scope(element, subscription);
}
#[derive(Default)]
struct InitialBuildDependencies {
    signals: Vec<Rc<dyn Dependency>>,
    focus_scopes: Vec<FocusScopeNode>,
}
impl InitialBuildDependencies {
    fn record_signal(&mut self, dependency: Rc<dyn Dependency>) {
        if !self
            .signals
            .iter()
            .any(|current| Rc::ptr_eq(current, &dependency))
        {
            self.signals.push(dependency);
        }
    }
}
struct BuildScope {
    root: ReactiveRootId,
    element: Option<ElementId>,
    queue: Weak<RefCell<ReactiveQueue>>,
    initial_dependencies: Option<Rc<RefCell<InitialBuildDependencies>>>,
    spawner: tasks::RuntimeSpawner,
    owner_scope: tasks::TaskScope,
}
thread_local! { static BUILD_SCOPE: RefCell<Option<BuildScope>> = const { RefCell::new(None) }; }
#[cfg(feature = "devtools")]
thread_local! { static DEV_TASK_COMPLETION: Cell<bool> = const { Cell::new(false) }; }
struct SignalInner<T> {
    value: RefCell<T>,
    dependents: RefCell<HashMap<ReactiveRootId, HashSet<ElementId>>>,
    queues: RefCell<HashMap<ReactiveRootId, Weak<RefCell<ReactiveQueue>>>>,
    node_dependents: RefCell<ReactiveNodeDependents>,
    node_queues: RefCell<HashMap<ReactiveRootId, Weak<RefCell<ReactiveQueue>>>>,
    /// DevTools write counter; always present, only read under the feature.
    dev_write_count: std::cell::Cell<u64>,
    #[cfg(feature = "devtools")]
    dev_signal_id: std::cell::Cell<Option<u64>>,
    #[cfg(feature = "devtools")]
    dev_name: RefCell<Option<String>>,
    #[cfg(feature = "devtools")]
    dev_editable_kind: std::cell::Cell<Option<EditableSignalKind>>,
    #[cfg(feature = "devtools")]
    dev_last_write: RefCell<(Option<String>, Option<String>)>,
    #[cfg(feature = "devtools")]
    dev_summarize: RefCell<Option<DevSummarizer<T>>>,
}
impl<T> Dependency for SignalInner<T> {
    fn subscribe(
        &self,
        root: ReactiveRootId,
        element: ElementId,
        queue: Weak<RefCell<ReactiveQueue>>,
    ) {
        self.queues.borrow_mut().insert(root, queue);
        self.dependents
            .borrow_mut()
            .entry(root)
            .or_default()
            .insert(element);
    }

    fn remove(&self, root: ReactiveRootId, element: ElementId) {
        let mut dependents = self.dependents.borrow_mut();
        if let Some(elements) = dependents.get_mut(&root) {
            elements.remove(&element);
            if elements.is_empty() {
                dependents.remove(&root);
                self.queues.borrow_mut().remove(&root);
            }
        }
    }

    fn subscribe_node(
        &self,
        root: ReactiveRootId,
        node: ReactiveNodeId,
        subscriber: Weak<dyn ReactiveNode>,
        queue: Weak<RefCell<ReactiveQueue>>,
    ) {
        if let Some(queue) = queue.upgrade() {
            queue.borrow_mut().register_node(node, subscriber.clone());
        }
        self.node_queues.borrow_mut().insert(root, queue);
        self.node_dependents
            .borrow_mut()
            .entry(root)
            .or_default()
            .insert(node, subscriber);
    }

    fn remove_node(&self, root: ReactiveRootId, node: ReactiveNodeId) {
        let mut dependents = self.node_dependents.borrow_mut();
        if let Some(nodes) = dependents.get_mut(&root) {
            nodes.remove(&node);
            if nodes.is_empty() {
                dependents.remove(&root);
                self.node_queues.borrow_mut().remove(&root);
            }
        }
    }
}
/// Shared state. A read in a registered builder subscribes that Element.
#[derive(Clone)]
pub struct Signal<T> {
    inner: Rc<SignalInner<T>>,
}
#[cfg(feature = "devtools")]
type DevSummarizer<T> = Box<dyn Fn(&T) -> String>;

#[cfg(feature = "devtools")]
fn editable_signal_value<T: 'static>(
    value: &incular_devtools_protocol::EditableValue,
) -> Option<T> {
    fn cast<T: 'static, U: Any>(value: U) -> Option<T> {
        let value: Box<dyn Any> = Box::new(value);
        value.downcast::<T>().ok().map(|value| *value)
    }

    let target = TypeId::of::<T>();
    match value {
        incular_devtools_protocol::EditableValue::Bool(value) if target == TypeId::of::<bool>() => {
            cast(*value)
        }
        incular_devtools_protocol::EditableValue::Int(value) if target == TypeId::of::<i64>() => {
            cast(*value)
        }
        incular_devtools_protocol::EditableValue::Uint(value) if target == TypeId::of::<u64>() => {
            cast(*value)
        }
        incular_devtools_protocol::EditableValue::Float(value) if target == TypeId::of::<f64>() => {
            cast(*value)
        }
        incular_devtools_protocol::EditableValue::Str(value)
            if target == TypeId::of::<String>() =>
        {
            cast(value.clone())
        }
        _ => None,
    }
}

impl<T: 'static> Signal<T> {
    /// Creates single-threaded application state. It attaches to the runtime
    /// that first reads it during a build; a signal is therefore not shared
    /// between independent applications.
    #[must_use]
    pub fn new(value: T) -> Self {
        Self {
            inner: Rc::new(SignalInner {
                value: RefCell::new(value),
                dependents: RefCell::new(HashMap::new()),
                queues: RefCell::new(HashMap::new()),
                node_dependents: RefCell::new(HashMap::new()),
                node_queues: RefCell::new(HashMap::new()),
                dev_write_count: std::cell::Cell::new(0),
                #[cfg(feature = "devtools")]
                dev_signal_id: std::cell::Cell::new(None),
                #[cfg(feature = "devtools")]
                dev_name: RefCell::new(None),
                #[cfg(feature = "devtools")]
                dev_editable_kind: std::cell::Cell::new(None),
                #[cfg(feature = "devtools")]
                dev_last_write: RefCell::new((None, None)),
                #[cfg(feature = "devtools")]
                dev_summarize: RefCell::new(None),
            }),
        }
    }

    #[must_use]
    pub fn with_runtime(value: T, _runtime: &Runtime) -> Self {
        Self::new(value)
    }
    /// Attaches a DevTools-visible debug name and registers the signal in
    /// the DevTools registry. Requires `T: Debug` for value summaries.
    #[cfg(feature = "devtools")]
    pub fn devtools(self, name: &str) -> Self
    where
        T: std::fmt::Debug + 'static,
    {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        self.inner.dev_signal_id.set(Some(id));
        *self.inner.dev_name.borrow_mut() = Some(name.to_owned());
        *self.inner.dev_summarize.borrow_mut() = Some(Box::new(|value: &T| truncate_debug(value)));

        let weak_value = Rc::downgrade(&self.inner);
        let weak_value_count = weak_value.clone();
        let weak_value_subscribers = weak_value.clone();
        let weak_value_subscriber_entries = weak_value.clone();
        let registration = crate::devtools_registry::SignalRegistration {
            name: Some(name.to_owned()),
            type_name: std::any::type_name::<T>(),
            editable_kind: None,
            write_count: Box::new(move || {
                weak_value_count
                    .upgrade()
                    .map(|inner| inner.dev_write_count.get())
                    .unwrap_or_default()
            }),
            subscriber_count: Box::new(move || {
                weak_value_subscribers
                    .upgrade()
                    .map(|inner| inner.dependents.borrow().values().map(HashSet::len).sum())
                    .unwrap_or_default()
            }),
            subscribers: Box::new(move || {
                weak_value_subscriber_entries
                    .upgrade()
                    .map(|inner| {
                        inner
                            .dependents
                            .borrow()
                            .iter()
                            .flat_map(|(root, elements)| {
                                elements.iter().copied().map(|element| (*root, element))
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            }),
            last_write: Box::new(move || {
                weak_value.upgrade().map_or((None, None), |inner| {
                    let guard = inner.dev_last_write.borrow();
                    (guard.0.clone(), guard.1.clone())
                })
            }),
            apply_edit: Box::new(move |_| false),
        };
        crate::devtools_registry::register(id, registration);
        self
    }

    /// Marks this named signal development-editable from DevTools. Only the
    /// supported primitive kinds can round-trip an `EditableValue`.
    #[cfg(feature = "devtools")]
    pub fn devtools_editable(self) -> Self
    where
        T: std::fmt::Debug + PartialEq + 'static,
    {
        let kind = if std::any::TypeId::of::<T>() == std::any::TypeId::of::<bool>() {
            Some(EditableSignalKind::Bool)
        } else if std::any::TypeId::of::<T>() == std::any::TypeId::of::<i64>() {
            Some(EditableSignalKind::Int)
        } else if std::any::TypeId::of::<T>() == std::any::TypeId::of::<u64>() {
            Some(EditableSignalKind::Uint)
        } else if std::any::TypeId::of::<T>() == std::any::TypeId::of::<f64>() {
            Some(EditableSignalKind::Float)
        } else if std::any::TypeId::of::<T>() == std::any::TypeId::of::<String>() {
            Some(EditableSignalKind::Str)
        } else {
            None
        };
        self.inner.dev_editable_kind.set(kind);
        let weak_value = Rc::downgrade(&self.inner);
        // Re-register with an explicit, typed conversion closure. The closure
        // can only create the documented primitive types, then uses Signal's
        // ordinary UI-thread mutation path and dependency invalidation.
        crate::devtools_registry::set_editable(
            self.inner.dev_signal_id.get().unwrap_or_default(),
            kind,
            Box::new(move |value| {
                let Some(value) = editable_signal_value::<T>(value) else {
                    return false;
                };
                let Some(inner) = weak_value.upgrade() else {
                    return false;
                };
                Signal { inner }.set(value)
            }),
        );
        self
    }

    pub fn get(&self) -> T
    where
        T: Clone,
    {
        scheduler_counters::SIGNAL_READS.fetch_add(1, Ordering::Relaxed);
        reactive::track_dependency(self.inner.clone());
        self.inner.value.borrow().clone()
    }
    pub fn set(&self, value: T) -> bool
    where
        T: PartialEq,
    {
        if *self.inner.value.borrow() == value {
            return false;
        }
        #[cfg(feature = "devtools")]
        let old_summary = self
            .inner
            .dev_summarize
            .borrow()
            .as_ref()
            .map(|format| format(&self.inner.value.borrow()));
        #[cfg(feature = "devtools")]
        let new_summary = self
            .inner
            .dev_summarize
            .borrow()
            .as_ref()
            .map(|format| format(&value));
        *self.inner.value.borrow_mut() = value;
        #[cfg(feature = "devtools")]
        if old_summary.is_some() || new_summary.is_some() {
            *self.inner.dev_last_write.borrow_mut() = (old_summary, new_summary);
        }
        self.enqueue_dependents();
        true
    }
    pub fn dependent_count(&self) -> usize {
        self.inner
            .dependents
            .borrow()
            .values()
            .map(HashSet::len)
            .sum()
    }
    /// Mutates the value once and schedules only the Elements that read it.
    pub fn update(&self, update: impl FnOnce(&mut T)) {
        #[cfg(feature = "devtools")]
        let old_summary = self
            .inner
            .dev_summarize
            .borrow()
            .as_ref()
            .map(|format| format(&self.inner.value.borrow()));
        update(&mut self.inner.value.borrow_mut());
        #[cfg(feature = "devtools")]
        {
            let new_summary = self
                .inner
                .dev_summarize
                .borrow()
                .as_ref()
                .map(|format| format(&self.inner.value.borrow()));
            if old_summary.is_some() || new_summary.is_some() {
                *self.inner.dev_last_write.borrow_mut() = (old_summary, new_summary);
            }
        }
        self.enqueue_dependents();
    }

    fn enqueue_dependents(&self) {
        self.inner
            .dev_write_count
            .set(self.inner.dev_write_count.get() + 1);
        scheduler_counters::SIGNAL_WRITES.fetch_add(1, Ordering::Relaxed);
        let mut enqueued = 0_usize;
        let dependencies: Vec<_> = self
            .inner
            .dependents
            .borrow()
            .iter()
            .map(|(root, elements)| (*root, elements.iter().copied().collect::<Vec<_>>()))
            .collect();
        let nodes: Vec<_> = self
            .inner
            .node_dependents
            .borrow()
            .iter()
            .map(|(root, nodes)| {
                (
                    *root,
                    nodes
                        .iter()
                        .map(|(id, node)| (*id, node.clone()))
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        let queues = self.inner.queues.borrow();
        #[cfg(feature = "devtools")]
        let cause = self.inner.dev_signal_id.get().map(|id| {
            let (old, new) = self.inner.dev_last_write.borrow().clone();
            InvalidationCause::Signal {
                id,
                name: self.inner.dev_name.borrow().clone(),
                old,
                new,
            }
        });
        for (root, elements) in dependencies {
            if let Some(queue) = queues.get(&root).and_then(Weak::upgrade) {
                let mut queue = queue.borrow_mut();
                for element in elements {
                    queue.enqueue(element);
                    #[cfg(feature = "devtools")]
                    {
                        if DEV_TASK_COMPLETION.with(Cell::get) {
                            queue.note_cause(element, InvalidationCause::TaskCompletion);
                        }
                        if let Some(cause) = &cause {
                            queue.note_cause(element, cause.clone());
                        }
                    }
                    enqueued += 1;
                }
            }
        }
        let node_queues = self.inner.node_queues.borrow();
        for (root, nodes) in nodes {
            if let Some(queue) = node_queues.get(&root).and_then(Weak::upgrade) {
                let mut queue = queue.borrow_mut();
                for (node_id, weak_node) in nodes {
                    if let Some(node) = weak_node.upgrade() {
                        node.mark_dirty();
                    }
                    queue.enqueue_node(node_id, weak_node);
                }
            }
        }
        if enqueued > 0 {
            scheduler_counters::DEPENDENTS_ENQUEUED.fetch_add(enqueued as u64, Ordering::Relaxed);
        }
    }
}
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
#[cfg(feature = "devtools")]
pub mod devtools_registry {
    pub use super::SignalRegistration;

    use std::cell::RefCell;
    std::thread_local! {
        static SIGNALS: RefCell<Vec<(u64, SignalRegistration)>> = const { RefCell::new(Vec::new()) };
    }

    pub fn register(id: u64, registration: SignalRegistration) {
        SIGNALS.with(|signals| signals.borrow_mut().push((id, registration)));
    }

    pub fn set_editable(
        id: u64,
        kind: Option<super::EditableSignalKind>,
        apply_edit: Box<dyn Fn(&incular_devtools_protocol::EditableValue) -> bool>,
    ) {
        SIGNALS.with(|signals| {
            for (registered_id, registration) in signals.borrow_mut().iter_mut() {
                if *registered_id == id {
                    registration.editable_kind = kind;
                    registration.apply_edit = apply_edit;
                    return;
                }
            }
        });
    }

    pub fn with_all<R>(visit: impl FnOnce(&[(u64, SignalRegistration)]) -> R) -> R {
        SIGNALS.with(|signals| visit(&signals.borrow()))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditableSignalKind {
    Bool,
    Int,
    Uint,
    Float,
    Str,
}

#[cfg_attr(not(feature = "devtools"), allow(dead_code))]
pub struct SignalRegistration {
    pub name: Option<String>,
    pub type_name: &'static str,
    pub editable_kind: Option<EditableSignalKind>,
    pub write_count: Box<dyn Fn() -> u64>,
    pub subscriber_count: Box<dyn Fn() -> usize>,
    pub subscribers: Box<dyn Fn() -> Vec<(ReactiveRootId, ElementId)>>,
    pub last_write: Box<dyn Fn() -> (Option<String>, Option<String>)>,
    #[cfg(feature = "devtools")]
    pub apply_edit: Box<dyn Fn(&incular_devtools_protocol::EditableValue) -> bool>,
}

/// Backend-neutral scheduler; it owns no window or GPU resource.
pub struct Runtime {
    tree: WidgetTree,
    /// Input-dispatch time accumulated since the last frame consumed it.
    pending_event_processing_us: u32,
    pending: HashMap<ElementId, Widget>,
    order: VecDeque<ElementId>,
    reactive: Rc<RefCell<ReactiveQueue>>,
    builders: HashMap<ElementId, Box<dyn FnMut() -> Widget>>,
    handlers: HashMap<ActionId, Rc<dyn Fn()>>,
    hovered_button: Option<ElementId>,
    pressed_button: Option<ElementId>,
    /// The contact currently allowed to drive single-contact controls. Other
    /// contacts still reach retained gesture regions for scale/pan input.
    legacy_pointer: Option<u64>,
    last_pointer: Offset,
    focused: Option<ElementId>,
    captured_text_field: Option<ElementId>,
    captured_selectable_text: Option<ElementId>,
    text_histories: HashMap<ElementId, UndoHistoryController>,
    text_input_commands: VecDeque<TextInputCommand>,
    text_input_client: Option<TextInputClientId>,
    text_input_configuration: Option<TextInputConfiguration>,
    text_input_state: Option<TextInputState>,
    text_input_caret_rect: Option<Rect>,
    clipboard: Box<dyn Clipboard>,
    editing_diagnostics: EditingDiagnostics,
    frame_requested: bool,
    scheduler: Rc<RefCell<tasks::TaskScheduler>>,
    environment: Rc<RefCell<RuntimeEnvironment>>,
    lifecycle: ApplicationLifecycle,
    lifecycle_observers: Vec<Rc<dyn Fn(LifecycleTransition)>>,
    error_observers: Vec<Rc<dyn Fn(RuntimeErrorReport)>>,
    environment_generation: u64,
    environment_dependencies: Rc<Cell<u16>>,
    application_root: Option<ElementId>,
    owner_scopes: HashMap<ElementId, TaskScope>,
    window_id: Option<WindowId>,
    window_scope: TaskScope,
    window_manager: Option<WindowManager>,
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

    fn with_window(
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
    fn with_window_and_reactive(
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
    /// [`BuildContext::spawn`] so completion lifetime follows its owner.
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
    /// multi-window [`Application`]. Standalone runtimes remain windowless.
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

    fn process_ui_work(
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
        if self.environment_dependencies.get() & changed != 0 {
            if let Some(root) = self.application_root {
                #[cfg(feature = "devtools")]
                self.tree
                    .note_invalidation(root, InvalidationCause::EnvironmentChanged);
                let _ = self.rebuild_from_builder(root);
                self.frame_requested = true;
            }
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
    fn dispose_window(&mut self) {
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
    fn devtools_reactive_root(&self) -> ReactiveRootId {
        self.reactive.borrow().root
    }
    #[must_use]
    pub fn tree_mut(&mut self) -> &mut WidgetTree {
        &mut self.tree
    }
    #[cfg(feature = "devtools")]
    fn devtools_edit_property(
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
    fn register_builder_without_rebuild(
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
    fn install_initial_dependencies(
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
        let started = std::time::Instant::now();
        let target = self.handle_input_inner(event);
        self.pending_event_processing_us = self
            .pending_event_processing_us
            .saturating_add(us_since_instant(started));
        target
    }
    fn handle_input_inner(&mut self, event: InputEvent) -> Option<EventTarget> {
        let (pointer, phase, position) = match event {
            InputEvent::Pointer { phase, position } => (0, phase, position),
            InputEvent::PointerWithId {
                pointer,
                phase,
                position,
            } => (pointer, phase, position),
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
                self.set_focus(text_target);
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
                    if let Some(action) = action {
                        if let Some(callback) = self.handlers.get(&action).cloned() {
                            callback();
                            self.frame_requested = true;
                        }
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
            if let Some(widget) = self.pending.remove(&id) {
                if self.tree.element_exists(id) {
                    self.tree.update(id, widget)?;
                    updated += 1;
                }
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
        self.tree.layout(constraints);
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
        let (composited, animations_active) = self.tree.update_compositor(now);
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
        let old = BUILD_SCOPE.with(|scope| {
            scope.replace(Some(BuildScope {
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
            }))
        });
        let widget = self.builders.get_mut(&id).expect("registered builder")();
        BUILD_SCOPE.with(|scope| {
            scope.replace(old);
        });
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
/// return the current value and create no dependency.
pub struct BuildContext {
    spawner: RuntimeSpawner,
    owner_scope: TaskScope,
    environment: Rc<RefCell<RuntimeEnvironment>>,
    environment_dependencies: Rc<Cell<u16>>,
    window_manager: Option<WindowManager>,
    restoration_scope: Option<RestorationScope>,
    restoration: Option<restoration::RestorationManager>,
}
impl BuildContext {
    fn new(
        spawner: RuntimeSpawner,
        owner_scope: TaskScope,
        environment: Rc<RefCell<RuntimeEnvironment>>,
        environment_dependencies: Rc<Cell<u16>>,
        window_manager: Option<WindowManager>,
        restoration_scope: Option<RestorationScope>,
    ) -> Self {
        Self {
            spawner,
            owner_scope,
            environment,
            environment_dependencies,
            restoration_scope,
            restoration: window_manager
                .as_ref()
                .and_then(|manager| manager.restoration.clone()),
            window_manager,
        }
    }
    /// Spawns Tokio work owned by this declarative owner. Its output must be
    /// `Send`; use [`Self::spawn_into`] to update UI-local state on completion.
    pub fn spawn<F, T>(&self, future: F) -> Task<T>
    where
        F: std::future::Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        self.spawner.spawn_in(&self.owner_scope, future)
    }
    pub fn spawn_in<F, T>(&self, scope: &TaskScope, future: F) -> Task<T>
    where
        F: std::future::Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        self.spawner.spawn_in(scope, future)
    }
    /// Starts owner-scoped Tokio work and handles the owned result on the UI
    /// thread. The callback may safely capture an Incular `Signal`.
    pub fn spawn_into<F, T>(
        &self,
        future: F,
        complete: impl FnOnce(Result<T, TaskFailure>, &mut Runtime) + 'static,
    ) -> TaskHandle
    where
        F: std::future::Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        self.spawner
            .spawn_into_in(&self.owner_scope, future, complete)
    }
    /// Queues a `Send` callback for the UI owner without granting any Tokio
    /// worker mutable access to the widget/runtime tree.
    pub fn dispatch(&self, callback: impl FnOnce(&mut Runtime) + Send + 'static) {
        self.spawner.dispatcher().dispatch(callback);
    }
    pub fn spawn_blocking<T>(
        &self,
        work: impl FnOnce() -> T + Send + 'static,
        complete: impl FnOnce(Result<T, TaskFailure>, &mut Runtime) + 'static,
    ) -> TaskHandle
    where
        T: Send + 'static,
    {
        self.spawner
            .spawn_blocking_in(&self.owner_scope, work, complete)
    }
    #[must_use]
    pub fn task_scope(&self) -> TaskScope {
        self.owner_scope.child()
    }

    /// Makes the current retained builder depend on a focus scope.
    ///
    /// A scope is an application-owned handle rather than a signal, so this
    /// explicit watch registers a weak, owner-lifetime subscription in the
    /// runtime's ordinary reactive queue. The initial root build records the
    /// scope and installs the subscription once its element identity exists.
    pub fn watch_focus_scope(&self, scope: &FocusScopeNode) {
        let target = BUILD_SCOPE.with(|current| {
            let current = current.borrow();
            current.as_ref().map(|build| {
                (
                    build.element,
                    build.queue.clone(),
                    build.initial_dependencies.clone(),
                )
            })
        });
        let Some((element, queue, initial_dependencies)) = target else {
            return;
        };
        if let Some(element) = element {
            if let Some(queue) = queue.upgrade() {
                install_focus_scope_watch(&queue, element, scope);
            }
        } else if let Some(initial_dependencies) = initial_dependencies {
            initial_dependencies
                .borrow_mut()
                .focus_scopes
                .push(scope.clone());
        }
    }

    /// Returns the stable restoration scope for this build when the owning
    /// application explicitly enabled restoration. Ordinary applications have
    /// no restoration scope and therefore no persistence side effects.
    #[must_use]
    pub fn restoration_scope(&self) -> Option<RestorationScope> {
        self.restoration_scope.clone()
    }

    /// Returns a callback-safe capability for reset, flush, and diagnostics
    /// when restoration is enabled. It contains no widget tree or native
    /// filesystem handle.
    #[must_use]
    pub fn restoration(&self) -> Option<RestorationHandle> {
        self.restoration.clone().map(RestorationHandle::new)
    }

    /// Creates an opt-in reactive value under this build's restoration scope.
    /// `key` must be stable across reordering and rebuilds; use a separate
    /// child scope for dynamic collections rather than an element position.
    #[must_use]
    pub fn restorable<T>(&self, key: RestorationKey, default: T) -> Option<Restorable<T>>
    where
        T: Clone + PartialEq + serde::Serialize + serde::de::DeserializeOwned + 'static,
    {
        self.restoration_scope
            .clone()
            .map(|scope| Restorable::from_scope(scope, key, default))
    }

    /// Alias matching the common `cx.restored_signal("count", 0)` pattern.
    /// The returned [`Restorable`] wraps a normal [`Signal`]; write via its
    /// `set`/`update` methods so a mutation is captured for persistence.
    #[must_use]
    pub fn restored_signal<T>(&self, key: RestorationKey, default: T) -> Option<Restorable<T>>
    where
        T: Clone + PartialEq + serde::Serialize + serde::de::DeserializeOwned + 'static,
    {
        self.restorable(key, default)
    }

    /// Queues another retained root for native creation. Its state, focus,
    /// semantics, environment, compositor, and task scope are independent of
    /// this context's window; shared `Signal`s may still be captured by the
    /// supplied root and will subscribe each root independently.
    pub fn open_window(
        &self,
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

    /// Returns the Tokio handle for advanced integrations. Tasks spawned
    /// directly through it are not owner-scoped; prefer [`Self::spawn`] or
    /// [`Self::spawn_into`] for UI work.
    #[must_use]
    pub fn tokio_handle(&self) -> TokioHandle {
        self.spawner.tokio_handle()
    }
    /// Reads all environment fields. Prefer the typed accessors below when a
    /// build uses only one value, so unrelated changes do not rebuild it.
    #[must_use]
    pub fn environment(&self) -> RuntimeEnvironment {
        self.environment_dependencies
            .set(self.environment_dependencies.get() | ENV_ALL);
        self.environment.borrow().clone()
    }
    #[must_use]
    pub fn viewport(&self) -> incular_core::Size {
        self.record_environment(ENV_VIEWPORT);
        self.environment.borrow().viewport
    }
    #[must_use]
    pub fn scale_factor(&self) -> f64 {
        self.record_environment(ENV_SCALE);
        self.environment.borrow().scale_factor
    }
    #[must_use]
    pub fn text_scale(&self) -> f32 {
        self.record_environment(ENV_TEXT_SCALE);
        self.environment.borrow().text_scale
    }
    #[must_use]
    pub fn safe_insets(&self) -> incular_config::EdgeInsets {
        self.record_environment(ENV_SAFE_INSETS);
        self.environment.borrow().safe_insets
    }
    /// Resolves `SafeArea` from the authoritative logical insets while
    /// recording only that environment dependency.
    #[must_use]
    pub fn safe_area(&self, safe_area: incular_widgets::SafeArea) -> Widget {
        safe_area.resolve(self.safe_insets())
    }
    #[must_use]
    pub fn brightness(&self) -> incular_config::Brightness {
        self.record_environment(ENV_BRIGHTNESS);
        self.environment.borrow().brightness
    }
    #[must_use]
    pub fn locales(&self) -> Vec<incular_config::Locale> {
        self.record_environment(ENV_LOCALE);
        self.environment.borrow().locales.clone()
    }
    /// Resolves a supported application locale while recording a locale-only
    /// dependency, so platform locale changes rebuild only consumers of it.
    #[must_use]
    pub fn resolve_locale(
        &self,
        supported: &[incular_config::Locale],
    ) -> Option<incular_config::Locale> {
        self.record_environment(ENV_LOCALE);
        self.environment.borrow().resolve_locale(supported)
    }
    #[must_use]
    pub fn text_direction(&self) -> incular_config::TextDirection {
        self.record_environment(ENV_DIRECTION);
        self.environment.borrow().text_direction
    }
    #[must_use]
    pub fn window_focused(&self) -> bool {
        self.record_environment(ENV_WINDOW_FOCUS);
        self.environment.borrow().window_focused
    }
    fn record_environment(&self, field: u16) {
        self.environment_dependencies
            .set(self.environment_dependencies.get() | field);
    }
}

/// Debug-facing, native-free state of one retained application window.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowDiagnostics {
    pub id: WindowId,
    pub title: String,
    pub logical_size: incular_core::Size,
    pub physical_size: incular_platform::PhysicalSize,
    pub scale_factor: f64,
    pub visible: bool,
    pub native_focused: bool,
    pub lifecycle: WindowLifecycle,
    pub frame_requested: bool,
    pub requested_frames: u64,
    pub presented_frames: u64,
    pub skipped_frames: u64,
    pub input_events: u64,
    pub surface_generation: u64,
    pub elements: usize,
    pub render_objects: usize,
    pub semantics: usize,
    /// Adapter health for this window only. It contains no native IDs or text
    /// values, and remains zero when no desktop accessibility adapter exists.
    pub accessibility: AccessibilityDiagnostics,
}

/// Aggregate lifecycle and stale-command diagnostics for an application.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ApplicationDiagnostics {
    pub windows_created: u64,
    pub windows_closed: u64,
    pub active_windows: usize,
    pub stale_window_commands: u64,
}

struct WindowRecord {
    runtime: Runtime,
    scope: TaskScope,
    last_frame: FrameRecord,
    render_metrics: RenderFrameMetrics,
    gpu_sample: Option<GpuSample>,
    options: WindowOptions,
    lifecycle: WindowLifecycle,
    metrics: WindowMetrics,
    native_focused: bool,
    requested_frames: u64,
    presented_frames: u64,
    skipped_frames: u64,
    input_events: u64,
    surface_generation: u64,
    accessibility: AccessibilityDiagnostics,
    restoration: Option<RestorableWindowMetadata>,
    _restoration_scope_lease: Option<restoration::ScopeLease>,
}

struct WindowSlot {
    generation: u32,
    reserved: bool,
    record: Option<WindowRecord>,
}

#[derive(Default)]
struct WindowRegistry {
    slots: Vec<WindowSlot>,
    windows_created: u64,
    windows_closed: u64,
    stale_window_commands: u64,
}

impl WindowRegistry {
    fn reserve(&mut self) -> WindowId {
        if let Some((index, slot)) = self
            .slots
            .iter_mut()
            .enumerate()
            .find(|(_, slot)| !slot.reserved && slot.record.is_none())
        {
            slot.reserved = true;
            return WindowId::from_parts(index as u32, slot.generation);
        }
        let index = self.slots.len() as u32;
        self.slots.push(WindowSlot {
            generation: 0,
            reserved: true,
            record: None,
        });
        WindowId::from_parts(index, 0)
    }

    fn insert(&mut self, id: WindowId, record: WindowRecord) {
        let slot = &mut self.slots[id.index() as usize];
        debug_assert!(slot.reserved && slot.generation == id.generation());
        slot.record = Some(record);
        self.windows_created += 1;
    }

    fn take(&mut self, id: WindowId) -> Option<WindowRecord> {
        self.slots
            .get_mut(id.index() as usize)
            .filter(|slot| slot.reserved && slot.generation == id.generation())?
            .record
            .take()
    }

    fn restore(&mut self, id: WindowId, record: WindowRecord) {
        let slot = &mut self.slots[id.index() as usize];
        debug_assert!(slot.reserved && slot.generation == id.generation());
        debug_assert!(slot.record.is_none());
        slot.record = Some(record);
    }

    fn close(&mut self, id: WindowId) -> Option<WindowRecord> {
        let slot = self
            .slots
            .get_mut(id.index() as usize)
            .filter(|slot| slot.reserved && slot.generation == id.generation())?;
        let record = slot.record.take()?;
        slot.reserved = false;
        slot.generation = slot.generation.wrapping_add(1);
        self.windows_closed += 1;
        Some(record)
    }

    fn ids(&self) -> Vec<WindowId> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| {
                slot.record
                    .as_ref()
                    .map(|_| WindowId::from_parts(index as u32, slot.generation))
            })
            .collect()
    }

    fn contains(&self, id: WindowId) -> bool {
        self.slots.get(id.index() as usize).is_some_and(|slot| {
            slot.reserved && slot.generation == id.generation() && slot.record.is_some()
        })
    }

    #[cfg_attr(not(feature = "devtools"), allow(dead_code))]
    fn get(&self, id: WindowId) -> Option<&WindowRecord> {
        self.slots
            .get(id.index() as usize)
            .filter(|slot| slot.reserved && slot.generation == id.generation())?
            .record
            .as_ref()
    }

    fn visible_count(&self) -> usize {
        self.slots
            .iter()
            .filter_map(|slot| slot.record.as_ref())
            .filter(|record| record.options.visible)
            .count()
    }
}

#[derive(Clone)]
struct WindowManager {
    registry: Weak<RefCell<WindowRegistry>>,
    scheduler: Rc<RefCell<tasks::TaskScheduler>>,
    bridge: Arc<WindowCommandBridge>,
    native_commands: Rc<RefCell<VecDeque<NativeWindowCommand>>>,
    restoration: Option<restoration::RestorationManager>,
}

impl WindowManager {
    fn open_window(
        &self,
        options: WindowOptions,
        root: Widget,
    ) -> Result<WindowHandle, WindowError> {
        self.open_window_inner(options, root, None)
    }

    fn open_window_inner(
        &self,
        mut options: WindowOptions,
        root: Widget,
        restoration: Option<RestorableWindowMetadata>,
    ) -> Result<WindowHandle, WindowError> {
        self.apply_restored_window_options(&mut options, restoration.as_ref());
        options.validate()?;
        let registry = self
            .registry
            .upgrade()
            .ok_or(WindowError::ApplicationStopped)?;
        let restoration_lease = self.acquire_restoration_scope(restoration.as_ref())?;
        let id = registry.borrow_mut().reserve();
        let scope = tasks::TaskScheduler::spawner(&self.scheduler).scope();
        scope.bind_window(id);
        let metrics = initial_metrics(&options);
        let mut runtime = Runtime::with_window(
            root,
            self.scheduler.clone(),
            Some(id),
            scope.clone(),
            Some(self.clone()),
        )?;
        runtime.update_window_metrics(metrics);
        registry.borrow_mut().insert(
            id,
            WindowRecord {
                runtime,
                scope,
                last_frame: FrameRecord::default(),
                render_metrics: RenderFrameMetrics::default(),
                gpu_sample: None,
                lifecycle: if options.visible {
                    WindowLifecycle::Visible
                } else {
                    WindowLifecycle::Hidden
                },
                metrics,
                options: options.clone(),
                native_focused: false,
                requested_frames: 0,
                presented_frames: 0,
                skipped_frames: 0,
                input_events: 0,
                surface_generation: 1,
                accessibility: AccessibilityDiagnostics::default(),
                restoration,
                _restoration_scope_lease: restoration_lease,
            },
        );
        self.sync_restorable_windows(true);
        self.native_commands
            .borrow_mut()
            .push_back(NativeWindowCommand::Create {
                window_id: id,
                options,
            });
        Ok(WindowHandle {
            id,
            bridge: self.bridge.clone(),
        })
    }

    fn open_window_with(
        &self,
        options: WindowOptions,
        build: impl FnMut(&mut BuildContext) -> Widget + 'static,
    ) -> Result<WindowHandle, WindowError> {
        self.open_window_with_inner(options, build, None)
    }

    fn open_restorable_window_with(
        &self,
        restoration_id: WindowRestorationId,
        kind: impl Into<String>,
        options: WindowOptions,
        build: impl FnMut(&mut BuildContext) -> Widget + 'static,
    ) -> Result<WindowHandle, WindowError> {
        self.open_window_with_inner(
            options,
            build,
            Some(RestorableWindowMetadata::new(restoration_id, kind)?),
        )
    }

    fn open_window_with_inner(
        &self,
        mut options: WindowOptions,
        build: impl FnMut(&mut BuildContext) -> Widget + 'static,
        restoration: Option<RestorableWindowMetadata>,
    ) -> Result<WindowHandle, WindowError> {
        self.apply_restored_window_options(&mut options, restoration.as_ref());
        options.validate()?;
        let registry = self
            .registry
            .upgrade()
            .ok_or(WindowError::ApplicationStopped)?;
        let restoration_scope = self.restoration_scope(restoration.as_ref());
        let restoration_lease = self.acquire_restoration_scope(restoration.as_ref())?;
        let id = registry.borrow_mut().reserve();
        let spawner = tasks::TaskScheduler::spawner_for(&self.scheduler, id);
        let window_scope = spawner.scope();
        window_scope.bind_window(id);
        let root_scope = window_scope.child();
        let metrics = initial_metrics(&options);
        let initial_environment = RuntimeEnvironment {
            viewport: metrics.logical_size(),
            physical_width: metrics.physical_size.width,
            physical_height: metrics.physical_size.height,
            scale_factor: metrics.scale_factor,
            ..RuntimeEnvironment::default()
        };
        let environment = Rc::new(RefCell::new(initial_environment));
        let environment_dependencies = Rc::new(Cell::new(0));
        let build = Rc::new(RefCell::new(build));
        let reactive = ReactiveQueue::new();
        let initial_dependencies = Rc::new(RefCell::new(InitialBuildDependencies::default()));
        let mut build_context = BuildContext::new(
            spawner.clone(),
            root_scope.clone(),
            environment.clone(),
            environment_dependencies.clone(),
            Some(self.clone()),
            restoration_scope.clone(),
        );
        let initial = {
            let previous = BUILD_SCOPE.with(|scope| {
                scope.replace(Some(BuildScope {
                    root: reactive.borrow().root,
                    element: None,
                    queue: Rc::downgrade(&reactive),
                    initial_dependencies: Some(initial_dependencies.clone()),
                    spawner: spawner.clone(),
                    owner_scope: root_scope.clone(),
                }))
            });
            let result = (build.borrow_mut())(&mut build_context);
            BUILD_SCOPE.with(|scope| {
                scope.replace(previous);
            });
            result
        };
        let mut runtime = Runtime::with_window_and_reactive(
            initial,
            self.scheduler.clone(),
            Some(id),
            window_scope.clone(),
            Some(self.clone()),
            reactive,
        )?;
        runtime.environment = environment;
        runtime.environment_dependencies = environment_dependencies;
        let root = runtime.tree().root().expect("new runtime has root");
        root_scope.bind_owner(root);
        runtime.install_initial_dependencies(root, initial_dependencies);
        let closure = build.clone();
        let builder_spawner = spawner.clone();
        let builder_scope = root_scope.clone();
        let builder_environment = runtime.environment.clone();
        let builder_dependencies = runtime.environment_dependencies.clone();
        let builder_manager = self.clone();
        runtime.register_builder_without_rebuild(root, move || {
            (closure.borrow_mut())(&mut BuildContext::new(
                builder_spawner.clone(),
                builder_scope.clone(),
                builder_environment.clone(),
                builder_dependencies.clone(),
                Some(builder_manager.clone()),
                restoration_scope.clone(),
            ))
        })?;
        runtime.application_root = Some(root);
        runtime.owner_scopes.insert(root, root_scope);
        runtime.lifecycle = ApplicationLifecycle::Active;
        registry.borrow_mut().insert(
            id,
            WindowRecord {
                runtime,
                scope: window_scope,
                last_frame: FrameRecord::default(),
                render_metrics: RenderFrameMetrics::default(),
                gpu_sample: None,
                lifecycle: if options.visible {
                    WindowLifecycle::Visible
                } else {
                    WindowLifecycle::Hidden
                },
                metrics,
                options: options.clone(),
                native_focused: false,
                requested_frames: 0,
                presented_frames: 0,
                skipped_frames: 0,
                input_events: 0,
                surface_generation: 1,
                accessibility: AccessibilityDiagnostics::default(),
                restoration,
                _restoration_scope_lease: restoration_lease,
            },
        );
        self.sync_restorable_windows(true);
        self.native_commands
            .borrow_mut()
            .push_back(NativeWindowCommand::Create {
                window_id: id,
                options,
            });
        Ok(WindowHandle {
            id,
            bridge: self.bridge.clone(),
        })
    }

    fn restoration_scope(
        &self,
        restoration: Option<&RestorableWindowMetadata>,
    ) -> Option<RestorationScope> {
        self.restoration.as_ref().map(|manager| {
            let root = manager.scope();
            restoration.map_or_else(
                || manager.application_scope(),
                |metadata| {
                    root.child_unchecked(RestorationKey::new("window").expect("static key"))
                        .child_unchecked(metadata.id.0.clone())
                },
            )
        })
    }

    fn acquire_restoration_scope(
        &self,
        restoration: Option<&RestorableWindowMetadata>,
    ) -> Result<Option<restoration::ScopeLease>, WindowError> {
        let Some(metadata) = restoration else {
            return Ok(None);
        };
        let Some(manager) = &self.restoration else {
            return Ok(None);
        };
        let path = [
            RestorationKey::new("window").expect("static key"),
            metadata.id.0.clone(),
        ];
        manager
            .acquire_scope(&path)
            .map(Some)
            .map_err(WindowError::Restoration)
    }

    fn apply_restored_window_options(
        &self,
        options: &mut WindowOptions,
        restoration: Option<&RestorableWindowMetadata>,
    ) {
        let Some(metadata) = restoration else {
            return;
        };
        let Some(manager) = &self.restoration else {
            return;
        };
        let Some(saved) = manager
            .windows()
            .into_iter()
            .find(|saved| saved.restoration_id == metadata.id.as_key().as_str())
        else {
            return;
        };
        if saved.kind != metadata.kind {
            return;
        }
        let saved_size = incular_core::Size::new(saved.logical_width, saved.logical_height);
        if saved_size.width.is_finite()
            && saved_size.height.is_finite()
            && saved_size.width > 0.0
            && saved_size.height > 0.0
        {
            let minimum = options
                .minimum_logical_size
                .unwrap_or(incular_core::Size::ZERO);
            let maximum = options.maximum_logical_size;
            options.initial_logical_size = incular_core::Size::new(
                saved_size
                    .width
                    .clamp(minimum.width, maximum.map_or(f32::MAX, |size| size.width)),
                saved_size
                    .height
                    .clamp(minimum.height, maximum.map_or(f32::MAX, |size| size.height)),
            );
        }
        options.maximized = saved.maximized;
        options.fullscreen = saved
            .fullscreen
            .then_some(incular_platform::Fullscreen::Borderless);
        manager.note_restored_window();
    }

    fn sync_restorable_windows(&self, retain_unopened: bool) {
        let Some(manager) = &self.restoration else {
            return;
        };
        let Some(registry) = self.registry.upgrade() else {
            return;
        };
        let active_windows = registry
            .borrow()
            .slots
            .iter()
            .filter_map(|slot| slot.record.as_ref())
            .filter_map(|record| {
                let metadata = record.restoration.as_ref()?;
                let logical = record.metrics.logical_size();
                Some(restoration::RestoredWindow {
                    restoration_id: metadata.id.as_key().as_str().to_owned(),
                    kind: metadata.kind.clone(),
                    logical_width: logical.width,
                    logical_height: logical.height,
                    maximized: record.options.maximized,
                    fullscreen: record.options.fullscreen.is_some(),
                })
            })
            .collect::<Vec<_>>();
        let windows = if retain_unopened {
            let active_ids = active_windows
                .iter()
                .map(|window| window.restoration_id.clone())
                .collect::<HashSet<_>>();
            manager
                .windows()
                .into_iter()
                .filter(|window| !active_ids.contains(&window.restoration_id))
                .chain(active_windows)
                .collect()
        } else {
            active_windows
        };
        manager.replace_windows(windows);
    }
}

fn initial_metrics(options: &WindowOptions) -> WindowMetrics {
    WindowMetrics::new(
        incular_platform::PhysicalSize::new(
            options.initial_logical_size.width.ceil() as u32,
            options.initial_logical_size.height.ceil() as u32,
        ),
        1.0,
    )
}

/// Owns application services, one Tokio runtime, and multiple retained roots.
/// A root's environment, input/focus, semantics, compositor, frame work, and
/// cancellation scope are window-local; assets, Signals, and Tokio are shared.
pub struct Application {
    scheduler: Rc<RefCell<tasks::TaskScheduler>>,
    registry: Rc<RefCell<WindowRegistry>>,
    manager: WindowManager,
    command_receiver: mpsc::Receiver<WindowCommand>,
    simulation_receiver: mpsc::Receiver<simulation::SimulationRequest>,
    simulation_bridge: Arc<simulation::SimulationBridge>,
    simulation_frame_waiters: HashMap<WindowId, Vec<mpsc::SyncSender<Result<(), SimulationError>>>>,
    simulation_capture_waiters:
        HashMap<WindowId, Vec<mpsc::SyncSender<Result<Screenshot, SimulationError>>>>,
    native_commands: Rc<RefCell<VecDeque<NativeWindowCommand>>>,
    primary_window: WindowId,
    last_window_policy: LastWindowPolicy,
    should_exit: bool,
    close_request: Option<Box<dyn FnMut(WindowId) -> bool>>,
    restoration: Option<restoration::RestorationManager>,
    restoration_window_factories: HashMap<String, (WindowOptions, RestorableWindowFactory)>,
    profiler: PerformanceProfiler,
    hub: PerformanceHub,
    scheduler_counters: ApplicationSchedulerCounters,
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
        let scheduler = tasks::TaskScheduler::new();
        let restoration = restoration_config.map(restoration::RestorationManager::load);
        if let Some(restoration) = &restoration {
            restoration.attach_scheduler(&scheduler);
        }
        let registry = Rc::new(RefCell::new(WindowRegistry::default()));
        let native_commands = Rc::new(RefCell::new(VecDeque::new()));
        let (sender, command_receiver) = mpsc::channel();
        let (simulation_sender, simulation_receiver) = mpsc::channel();
        let simulation_bridge = Arc::new(simulation::SimulationBridge::new(simulation_sender));
        let bridge = Arc::new(WindowCommandBridge {
            sender,
            wake: Mutex::new(None),
        });
        let manager = WindowManager {
            registry: Rc::downgrade(&registry),
            scheduler: scheduler.clone(),
            bridge,
            native_commands: native_commands.clone(),
            restoration: restoration.clone(),
        };
        let primary_window = manager
            .open_window_with_inner(options, build, primary_restoration)?
            .id();
        Ok(Self {
            scheduler,
            registry,
            manager,
            command_receiver,
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
        self.simulation_bridge.set_wake(wake);
    }

    /// Returns a cloneable, in-process controller for the primary window.
    /// Simulation commands are serviced by the native event loop and never
    /// synthesize OS-level mouse or keyboard input.
    #[must_use]
    pub fn simulation(&self) -> Simulation {
        Simulation::new(self.simulation_bridge.clone(), self.primary_window)
    }

    fn with_window_mut<R>(
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
                if matches!(lifecycle, PlatformLifecycle::Stopping) {
                    self.shutdown();
                } else {
                    let _ = self.with_window_mut(window_id, |record| {
                        let _ = record
                            .runtime
                            .handle_platform_event(PlatformEvent::Lifecycle(lifecycle));
                    });
                    if matches!(lifecycle, PlatformLifecycle::Suspended) {
                        let _ = self.flush_restoration();
                    }
                }
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
        }
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
        let outcome = self
            .with_window_mut(window_id, |record| {
                if record.metrics.physical_size.is_zero() {
                    record.skipped_frames = record.skipped_frames.wrapping_add(1);
                    scheduler_counters::FRAMES_SKIPPED.fetch_add(1, Ordering::Relaxed);
                    Ok(None)
                } else {
                    record
                        .runtime
                        .run_frame_at(constraints, now)
                        .map(|(list, stats)| {
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
        let record = self.registry.borrow_mut().close(window_id);
        let Some(mut record) = record else {
            self.registry.borrow_mut().stale_window_commands += 1;
            return false;
        };
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
        while let Ok(command) = self.command_receiver.try_recv() {
            let window_id = command.window_id;
            match command.operation.clone() {
                WindowOperation::Close => {
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
                                    record.options.initial_logical_size = *size;
                                } else {
                                    return false;
                                }
                            }
                            WindowOperation::RequestFocus => {}
                            WindowOperation::RequestRedraw => record.runtime.request_frame(),
                            WindowOperation::Close => unreachable!(),
                        }
                        true
                    });
                    if applied == Some(true) {
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
                    } else if applied.is_none() {
                        self.registry.borrow_mut().stale_window_commands += 1;
                    }
                }
            }
        }
    }

    /// Takes all UI-thread native operations. Adapters must call this only
    /// from their event-loop callback; creation is consequently valid for
    /// Winit 0.30's active-loop restriction.
    pub fn take_native_window_commands(&mut self) -> Vec<NativeWindowCommand> {
        self.drain_window_commands();
        self.native_commands.borrow_mut().drain(..).collect()
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
            frame_requested: record.runtime.frame_requested(),
            requested_frames: record.requested_frames,
            presented_frames: record.presented_frames,
            skipped_frames: record.skipped_frames,
            input_events: record.input_events,
            surface_generation: record.surface_generation,
            elements: record.runtime.tree().element_count(),
            render_objects: record.runtime.tree().render_object_count(),
            semantics: record.runtime.tree().semantics().len(),
            accessibility: record.accessibility,
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
        let ids = self.active_window_ids();
        for id in ids {
            self.fail_simulation_window(id);
            if let Some(mut record) = self.registry.borrow_mut().close(id) {
                record.scope.cancel();
                record.runtime.dispose_window();
            }
        }
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
    pub fn into_runtime(self) -> Runtime {
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
    Widget::repaint_boundary(Widget::align(
        incular_config::Alignment::TOP_LEFT,
        Widget::from(
            DecoratedBox::new(Padding::all(6., Widget::column(rows)))
                .background(incular_core::Color::rgba(12, 14, 18, 216))
                .radius(4.)
                .border(incular_rendering::Border {
                    color: incular_core::Color::rgba(120, 170, 245, 90),
                    width: 1.,
                }),
        ),
    ))
}

/// Truncates a Debug rendering to keep DevTools payloads bounded.
#[cfg(feature = "devtools")]
fn truncate_debug<T: std::fmt::Debug>(value: &T) -> String {
    let rendered = format!("{value:?}");
    if rendered.chars().count() <= 80 {
        rendered
    } else {
        let mut out: String = rendered.chars().take(77).collect();
        out.push('…');
        out
    }
}

fn us_since_instant(started: std::time::Instant) -> u32 {
    started.elapsed().as_micros().try_into().unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use accesskit::{Action, ActionRequest, NodeId, TreeId};
    use incular_core::{Code, Color, KeyboardEvent, KeyboardKey, Modifiers, Offset, Size};
    use incular_rendering::{DisplayList, PaintCommand};
    use incular_semantics::{Role as SemanticRole, SemanticAction};
    use incular_widgets::internal::{GestureCallbacks, TextEditingController};
    use incular_widgets::{
        CustomScrollView, DecoratedBox, GestureDetector, SliverFixedExtentList, Text,
        internal::ActionSurface,
    };
    use std::time::{Duration, Instant};
    use std::{
        cell::Cell,
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicU64, Ordering},
        },
    };

    fn fixed_sliver_list<W>(
        item_count: usize,
        item_extent: f32,
        controller: incular_widgets::ScrollController,
        builder: impl Fn(usize) -> W + 'static,
    ) -> Widget
    where
        W: Into<Widget> + 'static,
    {
        CustomScrollView::new(vec![Box::new(SliverFixedExtentList::new(
            item_count,
            item_extent,
            builder,
        )) as Box<dyn incular_widgets::Sliver>])
        .controller(controller)
        .into()
    }

    #[derive(Default)]
    struct TestWake(AtomicU64);
    impl RuntimeWake for TestWake {
        fn wake(&self) {
            self.0.fetch_add(1, Ordering::AcqRel);
        }
    }

    fn key_down(code: Code) -> KeyboardEvent {
        KeyboardEvent::key_down(
            KeyboardKey::Named(incular_core::NamedKey::Unidentified),
            code,
        )
    }

    fn shortcut_key_down(code: Code) -> KeyboardEvent {
        let mut event = key_down(code);
        event.modifiers = Modifiers::CONTROL;
        event
    }

    fn wait_for_wake(wake: &TestWake) {
        for _ in 0..100 {
            if wake.0.load(Ordering::Acquire) > 0 {
                return;
            }
            std::thread::park_timeout(Duration::from_millis(2));
        }
        panic!("Tokio completion did not wake the UI bridge");
    }

    fn semantic_node(runtime: &Runtime, role: SemanticRole) -> incular_semantics::SemanticNodeId {
        runtime
            .tree()
            .semantics()
            .iter()
            .find_map(|(id, node)| (node.role == role).then_some(id))
            .expect("semantic node")
    }

    #[cfg(feature = "devtools")]
    #[test]
    fn editable_signal_uses_typed_ui_thread_set_and_keeps_old_new_summary() {
        let signal = Signal::new(3_i64)
            .devtools("runtime-editable-signal-test")
            .devtools_editable();
        let (edited, writes, summaries, wrong_type) = devtools_registry::with_all(|signals| {
            let registration = signals
                .iter()
                .find(|(_, registration)| {
                    registration.name.as_deref() == Some("runtime-editable-signal-test")
                })
                .map(|(_, registration)| registration)
                .expect("registered signal");
            let edited =
                (registration.apply_edit)(&incular_devtools_protocol::EditableValue::Int(9));
            let writes = (registration.write_count)();
            let summaries = (registration.last_write)();
            let wrong_type = (registration.apply_edit)(
                &incular_devtools_protocol::EditableValue::Str("wrong type".into()),
            );
            (edited, writes, summaries, wrong_type)
        });

        assert!(edited);
        assert_eq!(signal.get(), 9);
        assert_eq!(writes, 1);
        assert_eq!(summaries, (Some("3".into()), Some("9".into())));
        assert!(!wrong_type);
    }

    #[test]
    fn semantic_actions_share_logical_button_and_editing_state() {
        use incular_widgets::{
            EditableText,
            internal::{ActionSurface, TextEditingController},
        };
        let hits = Rc::new(Cell::new(0));
        let controller = TextEditingController::with_text("Ada");
        let mut runtime = Runtime::new(Widget::column(vec![
            ActionSurface::new("Increment")
                .on_press({
                    let hits = hits.clone();
                    move || hits.set(hits.get() + 1)
                })
                .into(),
            EditableText::new(controller.clone()).into(),
        ]))
        .unwrap();
        let root = runtime.tree().root().unwrap();
        runtime
            .schedule_update(
                root,
                Widget::column(vec![
                    ActionSurface::new("Increment")
                        .on_press({
                            let hits = hits.clone();
                            move || hits.set(hits.get() + 1)
                        })
                        .into(),
                    EditableText::new(controller.clone()).into(),
                ]),
            )
            .unwrap();
        runtime
            .run_frame(Constraints::tight(Size::new(300., 200.)))
            .unwrap();
        let button = semantic_node(&runtime, SemanticRole::Button);
        let field = semantic_node(&runtime, SemanticRole::TextField);
        assert!(runtime.dispatch_semantic_action(button, SemanticAction::Activate));
        assert_eq!(hits.get(), 1);
        assert!(runtime.dispatch_semantic_action(field, SemanticAction::Focus));
        assert_eq!(
            runtime.focused_element(),
            runtime.tree().element_for_semantic_node(field)
        );
        assert!(runtime.dispatch_semantic_action(field, SemanticAction::SetText("hello".into())));
        assert!(
            runtime.dispatch_semantic_action(
                field,
                SemanticAction::SetSelection { base: 1, extent: 4 }
            )
        );
        assert_eq!(controller.text(), "hello");
        assert_eq!(
            controller.value().selection,
            TextSelection { base: 1, extent: 4 }
        );
    }

    #[test]
    fn semantic_callbacks_make_custom_controls_actionable() {
        let activations = Rc::new(Cell::new(0));
        let observed = activations.clone();
        let mut runtime = Runtime::new(
            incular_widgets::Semantics::new(Text::new("custom"))
                .on_tap(move || observed.set(observed.get() + 1))
                .into(),
        )
        .unwrap();
        runtime
            .run_frame(Constraints::tight(Size::new(200., 50.)))
            .unwrap();
        let node = semantic_node(&runtime, SemanticRole::Button);
        assert!(runtime.dispatch_semantic_action(node, SemanticAction::Activate));
        assert_eq!(activations.get(), 1);
    }

    #[test]
    fn focused_editors_publish_native_text_input_state_and_actions() {
        let submitted = Rc::new(Cell::new(0));
        let observed = submitted.clone();
        let controller = TextEditingController::new();
        let mut runtime = Runtime::new(
            incular_widgets::EditableText::new(controller.clone())
                .on_submit(move |_| observed.set(observed.get() + 1))
                .into(),
        )
        .unwrap();
        let constraints = Constraints::tight(Size::new(220., 50.));
        runtime.run_frame(constraints).unwrap();
        assert!(runtime.take_text_input_commands().is_empty());

        let _ = runtime.handle_input(InputEvent::Pointer {
            phase: PointerPhase::Down,
            position: Offset::new(5., 5.),
        });
        let commands = runtime.take_text_input_commands();
        assert!(matches!(
            commands.as_slice(),
            [incular_platform::TextInputCommand::SetClient {
                configuration,
                state,
                ..
            }] if configuration.input_type == incular_platform::TextInputType::Text
                && state.text.is_empty()
        ));

        let _ = runtime.handle_input(InputEvent::Text("hello".into()));
        runtime.run_frame(constraints).unwrap();
        let commands = runtime.take_text_input_commands();
        assert!(commands.iter().any(|command| matches!(
            command,
            incular_platform::TextInputCommand::Update { state, .. } if state.text == "hello"
        )));

        assert!(
            runtime
                .handle_platform_event(incular_platform::PlatformEvent::TextInputAction(
                    incular_platform::TextInputAction::Done,
                ),)
                .is_none()
        );
        assert_eq!(submitted.get(), 1);
    }

    #[test]
    fn native_text_input_hints_survive_editor_conversion() {
        let controller = TextEditingController::new();
        let mut runtime = Runtime::new(
            incular_widgets::EditableText::new(controller)
                .input_type(TextInputTypeHint::Email)
                .input_action(TextInputActionHint::Search)
                .into(),
        )
        .unwrap();
        let constraints = Constraints::tight(Size::new(220., 50.));
        runtime.run_frame(constraints).unwrap();
        let _ = runtime.handle_input(InputEvent::Pointer {
            phase: PointerPhase::Down,
            position: Offset::new(5., 5.),
        });
        let commands = runtime.take_text_input_commands();
        assert!(matches!(
            commands.as_slice(),
            [incular_platform::TextInputCommand::SetClient { configuration, .. }]
                if configuration.input_type == incular_platform::TextInputType::Email
                    && configuration.action == incular_platform::TextInputAction::Search
        ));
    }

    #[test]
    fn undo_history_widget_configures_runtime_capacity() {
        let controller = TextEditingController::new();
        let mut runtime = Runtime::new(
            incular_widgets::UndoHistory::new(incular_widgets::EditableText::new(
                controller.clone(),
            ))
            .max_entries(1)
            .into(),
        )
        .unwrap();
        let constraints = Constraints::tight(Size::new(220., 50.));
        runtime.run_frame(constraints).unwrap();
        let _ = runtime.handle_input(InputEvent::Pointer {
            phase: PointerPhase::Down,
            position: Offset::new(5., 5.),
        });
        let _ = runtime.handle_input(InputEvent::Text("one".into()));
        let _ = runtime.handle_input(InputEvent::Text("two".into()));
        let _ = runtime.handle_input(InputEvent::Key(shortcut_key_down(Code::KeyZ)));
        assert_eq!(controller.text(), "one");
    }

    #[test]
    fn sliver_list_semantics_are_bounded_and_follow_materialization() {
        let controller = incular_widgets::ScrollController::new();
        let mut runtime = Runtime::new(fixed_sliver_list(
            1_000_000,
            40.,
            controller.clone(),
            |index| ActionSurface::new(format!("Item {index}")),
        ))
        .unwrap();
        runtime
            .run_frame(Constraints::tight(Size::new(200., 600.)))
            .unwrap();
        let initial = runtime.tree().semantics().len();
        assert!(initial < 100, "{initial}");
        let _viewport = semantic_node(&runtime, SemanticRole::ScrollView);
        assert_eq!(
            runtime
                .tree()
                .sliver_viewport_diagnostics()
                .unwrap()
                .logical_item_count,
            1_000_000
        );
        controller.jump_to(900_000. * 40.);
        runtime
            .run_frame(Constraints::tight(Size::new(200., 600.)))
            .unwrap();
        assert!(runtime.tree().semantics().len() < 100);
        assert!(runtime.tree().semantics().iter().any(|(_, node)| {
            node.label
                .as_deref()
                .is_some_and(|label| label.contains("900000"))
        }));
    }

    #[test]
    fn semantic_scroll_uses_existing_controller() {
        let controller = incular_widgets::ScrollController::new();
        let mut runtime = Runtime::new(incular_widgets::internal::ScrollView::vertical(
            controller.clone(),
            Widget::fixed_box(Size::new(100., 2000.), Color::WHITE),
        ))
        .unwrap();
        runtime
            .run_frame(Constraints::tight(Size::new(100., 200.)))
            .unwrap();
        let scroll = semantic_node(&runtime, SemanticRole::ScrollView);
        assert!(runtime.dispatch_semantic_action(scroll, SemanticAction::ScrollForward));
        assert!(controller.offset() > 0.);
        assert!(runtime.dispatch_semantic_action(scroll, SemanticAction::ScrollBackward));
        assert_eq!(controller.offset(), 0.);
    }

    fn picture_origins(list: &DisplayList) -> (Offset, Offset) {
        let mut transforms = vec![Offset::ZERO];
        let mut rect = None;
        let mut glyph = None;
        for command in list.commands() {
            match command {
                PaintCommand::PushTransform { transform } => {
                    transforms.push(*transforms.last().unwrap() + transform.translation_offset());
                }
                PaintCommand::PopTransform => {
                    transforms.pop();
                }
                PaintCommand::Rect { rect: bounds, .. } => {
                    rect = Some(bounds.origin + *transforms.last().unwrap());
                }
                PaintCommand::GlyphRun { run, .. } => {
                    glyph = Some(run.origin + *transforms.last().unwrap());
                }
                PaintCommand::Image { .. }
                | PaintCommand::RRect { .. }
                | PaintCommand::Border { .. }
                | PaintCommand::FillPath { .. }
                | PaintCommand::StrokePath { .. }
                | PaintCommand::PushClip { .. }
                | PaintCommand::PushClipRRect { .. }
                | PaintCommand::PushClipOval { .. }
                | PaintCommand::PushClipPath { .. }
                | PaintCommand::PopClip
                | PaintCommand::PushOpacity { .. }
                | PaintCommand::PopOpacity
                | PaintCommand::PushBlur { .. }
                | PaintCommand::PushDropShadow { .. }
                | PaintCommand::PushColorFilter { .. }
                | PaintCommand::PushBlend { .. }
                | PaintCommand::PopEffect => {}
            }
        }
        (rect.unwrap(), glyph.unwrap())
    }
    #[test]
    fn signals_schedule_only_subscribed_element_once() {
        let mut runtime = Runtime::new(Widget::row(vec![
            Widget::box_(Size::new(1., 1.), Color::WHITE),
            Widget::box_(Size::new(2., 2.), Color::WHITE),
        ]))
        .unwrap();
        let root = runtime.tree().root().unwrap();
        let children = runtime.tree().children(root).unwrap().to_vec();
        let signal = Signal::with_runtime(2_u32, &runtime);
        let state = signal.clone();
        runtime
            .register_builder(children[1], move || {
                Widget::box_(Size::new(state.get() as f32, 2.), Color::WHITE)
            })
            .unwrap();
        assert_eq!(signal.dependent_count(), 1);
        assert!(signal.set(3));
        let (_, stats) = runtime
            .run_frame(Constraints::tight(Size::new(20., 20.)))
            .unwrap();
        assert_eq!(stats.updated_elements, 1);
        assert!(!runtime.tree().is_build_dirty(children[0]));
        assert!(!signal.set(3));
    }

    #[test]
    fn memo_tracks_sources_without_a_build_context_and_rebuilds_its_consumers() {
        let source = Signal::new(1_u32);
        let computations = Rc::new(Cell::new(0));
        let memo = Memo::new({
            let source = source.clone();
            let computations = computations.clone();
            move || {
                computations.set(computations.get() + 1);
                source.get() * 2
            }
        });
        let builds = Rc::new(Cell::new(0));
        let mut runtime = Application::new({
            let memo = memo.clone();
            let builds = builds.clone();
            move |_| {
                builds.set(builds.get() + 1);
                Widget::text(memo.with(ToString::to_string))
            }
        })
        .unwrap()
        .into_runtime();
        let constraints = Constraints::tight(Size::new(100., 30.));

        runtime.run_frame(constraints).unwrap();
        assert_eq!(computations.get(), 1);
        assert_eq!(builds.get(), 1);

        source.set(2);
        let (_, stats) = runtime.run_frame(constraints).unwrap();
        assert_eq!(computations.get(), 2);
        assert_eq!(builds.get(), 2);
        assert_eq!(stats.updated_elements, 1);
    }

    #[test]
    fn memo_filters_unchanged_results_before_invalidating_widgets() {
        let source = Signal::new(0_u32);
        let computations = Rc::new(Cell::new(0));
        let memo = Memo::new({
            let source = source.clone();
            let computations = computations.clone();
            move || {
                computations.set(computations.get() + 1);
                source.get() % 2
            }
        });
        let builds = Rc::new(Cell::new(0));
        let mut runtime = Application::new({
            let memo = memo.clone();
            let builds = builds.clone();
            move |_| {
                builds.set(builds.get() + 1);
                Widget::text(memo.get().to_string())
            }
        })
        .unwrap()
        .into_runtime();
        let constraints = Constraints::tight(Size::new(100., 30.));
        runtime.run_frame(constraints).unwrap();

        source.set(2);
        let (_, stats) = runtime.run_frame(constraints).unwrap();
        assert_eq!(computations.get(), 2);
        assert_eq!(builds.get(), 1);
        assert_eq!(stats.updated_elements, 0);

        source.set(3);
        let (_, stats) = runtime.run_frame(constraints).unwrap();
        assert_eq!(computations.get(), 3);
        assert_eq!(builds.get(), 2);
        assert_eq!(stats.updated_elements, 1);
    }

    #[test]
    fn shared_memo_tracks_each_window_without_losing_a_root_subscription() {
        let source = Signal::new(0_u32);
        let computations = Rc::new(Cell::new(0));
        let memo = Memo::new({
            let source = source.clone();
            let computations = computations.clone();
            move || {
                computations.set(computations.get() + 1);
                source.get() + 1
            }
        });
        let builds_a = Rc::new(Cell::new(0));
        let mut application = Application::new({
            let memo = memo.clone();
            let builds_a = builds_a.clone();
            move |_| {
                builds_a.set(builds_a.get() + 1);
                Widget::text(memo.get().to_string())
            }
        })
        .unwrap();
        let builds_b = Rc::new(Cell::new(0));
        let window_b = application
            .open_window_with(test_window_options("memo", 100., 30.), {
                let memo = memo.clone();
                let builds_b = builds_b.clone();
                move |_| {
                    builds_b.set(builds_b.get() + 1);
                    Widget::text(memo.get().to_string())
                }
            })
            .unwrap()
            .id();
        let window_a = application.primary_window();
        let constraints = Constraints::tight(Size::new(100., 30.));
        for window in [window_a, window_b] {
            application
                .run_window_frame_at(window, constraints, Instant::now())
                .unwrap();
        }
        assert_eq!(builds_a.get(), 1);
        assert_eq!(builds_b.get(), 1);

        source.set(1);
        application
            .run_window_frame_at(window_b, constraints, Instant::now())
            .unwrap();
        application
            .run_window_frame_at(window_a, constraints, Instant::now())
            .unwrap();
        assert_eq!(builds_a.get(), 2);
        assert_eq!(builds_b.get(), 2);
        assert_eq!(computations.get(), 3);
    }

    #[test]
    fn memo_replaces_dynamic_branch_dependencies_after_a_switch() {
        let branch = Signal::new(true);
        let first = Signal::new(1_u32);
        let second = Signal::new(2_u32);
        let memo = Memo::new({
            let branch = branch.clone();
            let first = first.clone();
            let second = second.clone();
            move || {
                if branch.get() {
                    first.get()
                } else {
                    second.get()
                }
            }
        });
        let builds = Rc::new(Cell::new(0));
        let mut runtime = Application::new({
            let memo = memo.clone();
            let builds = builds.clone();
            move |_| {
                builds.set(builds.get() + 1);
                Widget::text(memo.get().to_string())
            }
        })
        .unwrap()
        .into_runtime();
        let constraints = Constraints::tight(Size::new(100., 30.));
        runtime.run_frame(constraints).unwrap();

        first.set(3);
        runtime.run_frame(constraints).unwrap();
        assert_eq!(builds.get(), 2);

        branch.set(false);
        runtime.run_frame(constraints).unwrap();
        assert_eq!(builds.get(), 3);

        first.set(4);
        runtime.run_frame(constraints).unwrap();
        assert_eq!(builds.get(), 3);

        second.set(5);
        runtime.run_frame(constraints).unwrap();
        assert_eq!(builds.get(), 4);
    }

    #[test]
    fn mounted_effect_runs_once_then_retracks_signal_dependencies() {
        let source = Signal::new(1_u32);
        let trigger = Signal::new(0_u32);
        let runs = Rc::new(Cell::new(0));
        let observed = Rc::new(Cell::new(0));
        let effect = Effect::new({
            let source = source.clone();
            let observed = observed.clone();
            let runs = runs.clone();
            move || {
                runs.set(runs.get() + 1);
                observed.set(source.get());
            }
        });
        let mut runtime = Application::new({
            let effect = effect.clone();
            let trigger = trigger.clone();
            move |_| {
                assert!(effect.mount());
                let _ = trigger.get();
                Widget::text("effect")
            }
        })
        .unwrap()
        .into_runtime();
        let constraints = Constraints::tight(Size::new(100., 30.));

        runtime.run_frame(constraints).unwrap();
        assert_eq!(runs.get(), 1);
        assert_eq!(observed.get(), 1);

        trigger.set(1);
        runtime.run_frame(constraints).unwrap();
        assert_eq!(runs.get(), 1);

        source.set(2);
        runtime.run_frame(constraints).unwrap();
        assert_eq!(runs.get(), 2);
        assert_eq!(observed.get(), 2);

        runtime.run_frame(constraints).unwrap();
        assert_eq!(runs.get(), 2);
    }

    #[test]
    fn action_is_lazy_tracks_state_and_ignores_stale_completions() {
        let action =
            super::Action::<u32, u32, &'static str>::new(|input| async move { Ok(input + 1) });
        let mut runtime = Application::new({
            let action = action.clone();
            move |_| Widget::text(format!("{:?}", action.state()))
        })
        .unwrap()
        .into_runtime();
        let wake = Arc::new(TestWake::default());
        runtime.set_wake_handler(wake.clone());
        let constraints = Constraints::tight(Size::new(100., 30.));
        runtime.run_frame(constraints).unwrap();
        assert!(matches!(action.state(), ActionState::Idle));

        action.dispatch(4).unwrap();
        assert!(matches!(action.state(), ActionState::Loading));
        wait_for_wake(&wake);
        runtime.process_runtime_work();
        runtime.run_frame(constraints).unwrap();
        assert!(matches!(action.state(), ActionState::Ready(5)));
    }

    #[cfg(feature = "devtools")]
    #[test]
    fn named_signal_write_reaches_dependent_rebuild_cause() {
        let mut runtime = Runtime::new(Widget::row(vec![Widget::box_(
            Size::new(1., 1.),
            Color::WHITE,
        )]))
        .unwrap();
        let root = runtime.tree().root().unwrap();
        let dependent = runtime.tree().children(root).unwrap()[0];
        let signal = Signal::with_runtime(1_u32, &runtime).devtools("counter");
        let state = signal.clone();
        runtime
            .register_builder(dependent, move || {
                Widget::box_(Size::new(state.get() as f32, 1.), Color::WHITE)
            })
            .unwrap();
        assert!(signal.set(2));
        runtime
            .run_frame(Constraints::tight(Size::new(20., 20.)))
            .unwrap();
        let id = runtime
            .tree()
            .devtools_id_for_element(dependent)
            .expect("devtools id");
        let details = runtime
            .tree()
            .devtools_node_details(
                dependent,
                id,
                incular_devtools_protocol::DevWindowId::new(1, 0),
            )
            .expect("details");
        assert!(details.invalidation_causes.iter().any(|cause| matches!(
            cause,
            incular_devtools_protocol::InvalidationReason::SignalWrite {
                name,
                old: Some(old),
                new: Some(new),
                ..
            } if name == "counter" && old == "1" && new == "2"
        )));
    }
    #[cfg(feature = "devtools")]
    #[test]
    fn tracked_task_completion_is_coalesced_with_its_signal_cause() {
        let mut runtime = Runtime::new(Widget::row(vec![Widget::box_(
            Size::new(1., 1.),
            Color::WHITE,
        )]))
        .unwrap();
        let root = runtime.tree().root().unwrap();
        let dependent = runtime.tree().children(root).unwrap()[0];
        let signal = Signal::with_runtime(1_u32, &runtime).devtools("async-counter");
        let observed = signal.clone();
        runtime
            .register_builder(dependent, move || {
                Widget::box_(Size::new(observed.get() as f32, 1.), Color::WHITE)
            })
            .unwrap();
        let wake = Arc::new(TestWake::default());
        runtime.set_wake_handler(wake.clone());
        let completion = signal.clone();
        runtime
            .spawner()
            .spawn_into(async { 2_u32 }, move |result, _| {
                completion.set(result.expect("Tokio result"));
            });
        wait_for_wake(&wake);
        runtime.process_runtime_work();
        runtime
            .run_frame(Constraints::tight(Size::new(20., 20.)))
            .unwrap();
        let id = runtime
            .tree()
            .devtools_id_for_element(dependent)
            .expect("id");
        let details = runtime
            .tree()
            .devtools_node_details(
                dependent,
                id,
                incular_devtools_protocol::DevWindowId::new(1, 0),
            )
            .expect("details");
        assert!(details.invalidation_causes.iter().any(|cause| matches!(
            cause,
            incular_devtools_protocol::InvalidationReason::TaskCompletion
        )));
        assert!(details.invalidation_causes.iter().any(|cause| matches!(
            cause,
            incular_devtools_protocol::InvalidationReason::SignalWrite { name, .. }
                if name == "async-counter"
        )));
    }
    #[test]
    fn tokio_sleep_completion_wakes_the_ui_bridge() {
        let mut runtime = Runtime::new(Widget::box_(Size::new(1., 1.), Color::WHITE)).unwrap();
        let wake = Arc::new(TestWake::default());
        runtime.set_wake_handler(wake.clone());
        let completed = Arc::new(AtomicBool::new(false));
        let observed = completed.clone();
        runtime.spawner().spawn_into(
            async move {
                tokio::time::sleep(Duration::from_millis(1)).await;
            },
            move |result, _| {
                assert!(result.is_ok());
                observed.store(true, Ordering::Release);
            },
        );
        wait_for_wake(&wake);
        runtime.process_runtime_work();
        assert!(completed.load(Ordering::Acquire));
        assert_eq!(runtime.runtime_diagnostics().active_tracked_tasks, 0);
    }
    #[test]
    fn async_signal_completion_invalidates_only_its_dependent_build() {
        let mut runtime = Runtime::new(Widget::row(vec![
            Widget::box_(Size::new(1., 1.), Color::WHITE),
            Widget::box_(Size::new(1., 1.), Color::WHITE),
        ]))
        .unwrap();
        let root = runtime.tree().root().unwrap();
        let dependent = runtime.tree().children(root).unwrap()[1];
        let signal = Signal::with_runtime(1_u32, &runtime);
        let observed = signal.clone();
        runtime
            .register_builder(dependent, move || {
                Widget::box_(Size::new(observed.get() as f32, 1.), Color::WHITE)
            })
            .unwrap();
        let wake = Arc::new(TestWake::default());
        runtime.set_wake_handler(wake.clone());
        let spawner = runtime.spawner();
        let completion = signal.clone();
        spawner.spawn_into(async { 2_u32 }, move |result, _| {
            completion.set(result.expect("Tokio result"));
        });
        wait_for_wake(&wake);
        let start = Instant::now();
        runtime.process_runtime_work_at(start);
        assert!(runtime.frame_requested());
        let (_, frame) = runtime
            .run_frame_at(
                Constraints::tight(Size::new(20., 20.)),
                start + Duration::from_millis(1),
            )
            .unwrap();
        assert_eq!(frame.updated_elements, 1);
        assert!(!runtime.tree().is_build_dirty(root));
    }
    #[test]
    fn cancelled_scope_never_runs_its_ready_task() {
        let mut runtime = Runtime::new(Widget::box_(Size::new(1., 1.), Color::WHITE)).unwrap();
        let wake = Arc::new(TestWake::default());
        runtime.set_wake_handler(wake.clone());
        let spawner = runtime.spawner();
        let scope = spawner.scope();
        let hit = Arc::new(AtomicBool::new(false));
        let observed = hit.clone();
        let handle = spawner.spawn_in(&scope, async move {
            std::future::pending::<()>().await;
            observed.store(true, Ordering::Release);
        });
        scope.cancel();
        wait_for_wake(&wake);
        runtime.process_runtime_work();
        assert!(!hit.load(Ordering::Acquire));
        assert_eq!(handle.value(), AsyncValue::Error(TaskFailure::Cancelled));
        assert_eq!(runtime.runtime_diagnostics().tasks_cancelled, 1);
    }
    #[test]
    fn stale_owner_completion_is_rejected_by_generational_element_identity() {
        let mut runtime = Runtime::new(Widget::row(vec![Widget::box_(
            Size::new(1., 1.),
            Color::WHITE,
        )]))
        .unwrap();
        let root = runtime.tree().root().unwrap();
        let child = runtime.tree().children(root).unwrap()[0];
        let wake = Arc::new(TestWake::default());
        runtime.set_wake_handler(wake.clone());
        let scope = runtime.spawner().scope();
        scope.bind_owner(child);
        let hit = Arc::new(AtomicBool::new(false));
        let observed = hit.clone();
        runtime
            .spawner()
            .spawn_into_in(&scope, async { 7_u8 }, move |_, _| {
                observed.store(true, Ordering::Release);
            });
        wait_for_wake(&wake);

        // The exact ArenaId (including its generation) no longer exists before
        // the completion is delivered. A future replacement of this slot is
        // therefore never mistaken for the old owner.
        runtime.tree.update(root, Widget::row(Vec::new())).unwrap();
        assert!(!runtime.tree().element_exists(child));
        runtime.process_runtime_work();
        assert!(!hit.load(Ordering::Acquire));
        assert_eq!(runtime.runtime_diagnostics().stale_completion_rejections, 1);
    }
    #[test]
    fn application_task_survives_an_unrelated_component_unmount() {
        let mut runtime = Runtime::new(Widget::row(vec![Widget::box_(
            Size::new(1., 1.),
            Color::WHITE,
        )]))
        .unwrap();
        let root = runtime.tree().root().unwrap();
        let wake = Arc::new(TestWake::default());
        runtime.set_wake_handler(wake.clone());
        let hit = Arc::new(AtomicBool::new(false));
        let observed = hit.clone();
        runtime.spawner().spawn_into(async { 1_u8 }, move |_, _| {
            observed.store(true, Ordering::Release);
        });
        wait_for_wake(&wake);
        runtime.tree.update(root, Widget::row(Vec::new())).unwrap();
        runtime.process_runtime_work();
        assert!(hit.load(Ordering::Acquire));
    }
    #[test]
    fn blocking_result_after_owner_unmount_is_discarded() {
        let mut runtime = Runtime::new(Widget::row(vec![Widget::box_(
            Size::new(1., 1.),
            Color::WHITE,
        )]))
        .unwrap();
        let root = runtime.tree().root().unwrap();
        let child = runtime.tree().children(root).unwrap()[0];
        let wake = Arc::new(TestWake::default());
        runtime.set_wake_handler(wake.clone());
        let scope = runtime.spawner().scope();
        scope.bind_owner(child);
        let hit = Arc::new(AtomicBool::new(false));
        let observed = hit.clone();
        runtime.spawner().spawn_blocking_in(
            &scope,
            || 42_u64,
            move |_, _| {
                observed.store(true, Ordering::Release);
            },
        );
        wait_for_wake(&wake);
        runtime.tree.update(root, Widget::row(Vec::new())).unwrap();
        runtime.process_runtime_work();
        assert!(!hit.load(Ordering::Acquire));
        assert_eq!(runtime.runtime_diagnostics().blocking_results_discarded, 1);
    }
    #[test]
    fn task_panic_reaches_the_runtime_error_hook() {
        let mut runtime = Runtime::new(Widget::box_(Size::new(1., 1.), Color::WHITE)).unwrap();
        let wake = Arc::new(TestWake::default());
        runtime.set_wake_handler(wake.clone());
        let panics = Rc::new(Cell::new(0));
        let observed = panics.clone();
        runtime.observe_runtime_error(move |report| {
            if report.failure == TaskFailure::Panicked {
                observed.set(observed.get() + 1);
            }
        });
        runtime.spawner().spawn(async {
            panic!("Tokio task panic stays outside Winit");
        });
        wait_for_wake(&wake);
        runtime.process_runtime_work();
        assert_eq!(panics.get(), 1);
        assert_eq!(runtime.runtime_diagnostics().task_panics, 1);
    }
    #[test]
    fn ordinary_result_error_remains_application_data() {
        let mut runtime = Runtime::new(Widget::box_(Size::new(1., 1.), Color::WHITE)).unwrap();
        let wake = Arc::new(TestWake::default());
        runtime.set_wake_handler(wake.clone());
        let recovered = Arc::new(AtomicBool::new(false));
        let observed = recovered.clone();
        runtime.spawner().spawn_into(
            async { Result::<u8, &'static str>::Err("expected validation error") },
            move |result, _| {
                assert_eq!(
                    result.expect("Tokio completion"),
                    Err("expected validation error")
                );
                observed.store(true, Ordering::Release);
            },
        );
        wait_for_wake(&wake);
        runtime.process_runtime_work();
        assert!(recovered.load(Ordering::Acquire));
    }
    #[test]
    fn completion_can_start_follow_up_tokio_work_without_reentrant_ui_borrow() {
        let mut runtime = Runtime::new(Widget::box_(Size::new(1., 1.), Color::WHITE)).unwrap();
        let wake = Arc::new(TestWake::default());
        runtime.set_wake_handler(wake.clone());
        let completed = Arc::new(AtomicU64::new(0));
        let observed = completed.clone();
        runtime
            .spawner()
            .spawn_into(async { 1_u8 }, move |_, runtime| {
                let observed = observed.clone();
                runtime.spawn_into(async { 2_u8 }, move |_, _| {
                    observed.store(2, Ordering::Release);
                });
            });
        wait_for_wake(&wake);
        runtime.process_runtime_work();
        let first_wakes = wake.0.load(Ordering::Acquire);
        for _ in 0..100 {
            if wake.0.load(Ordering::Acquire) > first_wakes {
                break;
            }
            std::thread::park_timeout(Duration::from_millis(2));
        }
        runtime.process_runtime_work();
        assert_eq!(completed.load(Ordering::Acquire), 2);
    }
    #[test]
    fn wake_coalescing_and_message_budget_preserve_native_fairness() {
        let mut runtime = Runtime::new(Widget::box_(Size::new(1., 1.), Color::WHITE)).unwrap();
        let wake = Arc::new(TestWake::default());
        runtime.set_wake_handler(wake.clone());
        let processed = Arc::new(AtomicU64::new(0));
        let dispatcher = runtime.dispatcher();
        for _ in 0..129 {
            let observed = processed.clone();
            dispatcher.dispatch(move |_| {
                observed.fetch_add(1, Ordering::AcqRel);
            });
        }
        assert_eq!(wake.0.load(Ordering::Acquire), 1);
        runtime.process_runtime_work();
        assert_eq!(processed.load(Ordering::Acquire), 128);
        let diagnostics = runtime.runtime_diagnostics();
        assert!(diagnostics.coalesced_wakes >= 128);
        assert_eq!(diagnostics.ui_messages_processed, 128);
        runtime.process_runtime_work();
        assert_eq!(processed.load(Ordering::Acquire), 129);
    }
    #[test]
    fn completion_without_ui_mutation_does_not_request_a_frame_and_idle_does_not_spin() {
        let mut runtime = Runtime::new(Widget::box_(Size::new(1., 1.), Color::WHITE)).unwrap();
        runtime
            .run_frame(Constraints::tight(Size::new(10., 10.)))
            .unwrap();
        assert!(!runtime.frame_requested());
        let before = runtime.runtime_diagnostics();
        runtime.process_runtime_work();
        assert_eq!(
            runtime.runtime_diagnostics().ui_messages_processed,
            before.ui_messages_processed
        );

        runtime.dispatcher().dispatch(|_| {});
        runtime.process_runtime_work();
        assert!(!runtime.frame_requested());
    }
    #[test]
    fn shutdown_cancels_tracked_tokio_tasks() {
        let mut runtime = Runtime::new(Widget::box_(Size::new(1., 1.), Color::WHITE)).unwrap();
        let task = runtime
            .spawner()
            .spawn(async { std::future::pending::<u8>().await });
        runtime.shutdown();
        assert_eq!(task.value(), AsyncValue::Error(TaskFailure::Cancelled));
        assert_eq!(runtime.runtime_diagnostics().active_tracked_tasks, 0);
    }
    #[test]
    fn shutdown_discards_dispatched_callbacks() {
        let mut runtime = Runtime::new(Widget::box_(Size::new(1., 1.), Color::WHITE)).unwrap();
        let dispatcher = runtime.dispatcher();
        let hit = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let observed = hit.clone();
        dispatcher.dispatch(move |_| observed.store(true, std::sync::atomic::Ordering::Release));
        runtime.shutdown();
        runtime.process_runtime_work();
        assert!(!hit.load(std::sync::atomic::Ordering::Acquire));
    }
    #[test]
    fn application_root_builds_once_and_subscribes_during_initial_build() {
        let builds = Rc::new(Cell::new(0));
        let observed_builds = builds.clone();
        let signal = Signal::new(false);
        let observed_signal = signal.clone();
        let application = Application::new(move |_| {
            observed_builds.set(observed_builds.get() + 1);
            let _ = observed_signal.get();
            Widget::box_(Size::new(1., 1.), Color::WHITE)
        })
        .unwrap();

        assert_eq!(builds.get(), 1);
        assert_eq!(signal.dependent_count(), 1);

        let mut runtime = application.into_runtime();
        assert!(signal.set(true));
        runtime
            .run_frame(Constraints::tight(Size::new(20., 20.)))
            .unwrap();
        assert_eq!(builds.get(), 2);
    }

    #[test]
    fn typed_environment_rebuilds_only_when_a_read_field_changes() {
        let builds = Rc::new(Cell::new(0));
        let observed = builds.clone();
        let application = Application::new(move |cx| {
            observed.set(observed.get() + 1);
            Widget::box_(Size::new(cx.text_scale(), 1.), Color::WHITE)
        })
        .unwrap();
        let mut runtime = application.into_runtime();
        let baseline = builds.get();
        let mut environment = runtime.environment();
        environment.brightness = incular_config::Brightness::Dark;
        assert!(runtime.set_environment(environment.clone()));
        assert_eq!(builds.get(), baseline);
        environment.text_scale = 1.5;
        assert!(runtime.set_environment(environment));
        assert_eq!(builds.get(), baseline + 1);
    }

    #[test]
    fn watched_focus_scope_invalidates_a_mounted_builder() {
        let scope = FocusScopeNode::new();
        let node = incular_widgets::FocusNode::new();
        scope.register(&node);
        let builds = Rc::new(Cell::new(0));
        let observed_builds = builds.clone();
        let observed_scope = scope.clone();
        let application = Application::new(move |cx| {
            cx.watch_focus_scope(&observed_scope);
            observed_builds.set(observed_builds.get() + 1);
            Text::new(if observed_scope.focused().is_some() {
                "focused"
            } else {
                "unfocused"
            })
            .into()
        })
        .unwrap();
        let mut runtime = application.into_runtime();
        let constraints = Constraints::tight(Size::new(220., 50.));
        runtime.run_frame(constraints).unwrap();
        let baseline = builds.get();
        assert!(!runtime.frame_requested());

        assert!(scope.request_focus(&node));
        assert!(runtime.frame_requested());
        runtime.run_frame(constraints).unwrap();
        assert!(builds.get() > baseline);
    }

    #[test]
    fn locale_resolution_rebuilds_only_locale_consumers() {
        let builds = Rc::new(Cell::new(0));
        let observed = builds.clone();
        let supported = vec![
            "en".parse().expect("valid ICU locale"),
            "fr".parse().expect("valid ICU locale"),
        ];
        let application = Application::new(move |cx| {
            observed.set(observed.get() + 1);
            let locale = cx
                .resolve_locale(&supported)
                .map(|locale| locale.to_string())
                .unwrap_or_else(|| "none".to_owned());
            Text::new(locale).into()
        })
        .unwrap();
        let mut runtime = application.into_runtime();
        let baseline = builds.get();

        let mut environment = runtime.environment();
        environment.brightness = incular_config::Brightness::Dark;
        assert!(runtime.set_environment(environment.clone()));
        assert_eq!(builds.get(), baseline);

        environment.locales = vec!["fr-CA".parse().expect("valid ICU locale")];
        assert!(runtime.set_environment(environment));
        assert_eq!(builds.get(), baseline + 1);
    }
    #[test]
    fn hit_test_resolves_button_action() {
        let mut runtime = Runtime::new(incular_widgets::internal::action(
            Size::new(10., 10.),
            Color::WHITE,
            ActionId(1),
        ))
        .unwrap();
        runtime
            .run_frame(Constraints::tight(Size::new(20., 20.)))
            .unwrap();
        let _ = runtime.handle_input(InputEvent::Pointer {
            phase: PointerPhase::Down,
            position: Offset::new(5., 5.),
        });
        assert_eq!(
            runtime
                .handle_input(InputEvent::Pointer {
                    phase: PointerPhase::Up,
                    position: Offset::new(5., 5.)
                })
                .unwrap()
                .action,
            Some(ActionId(1))
        );
    }
    #[test]
    fn identified_primary_contact_can_activate_a_button() {
        let mut runtime = Runtime::new(incular_widgets::internal::action(
            Size::new(10., 10.),
            Color::WHITE,
            ActionId(2),
        ))
        .unwrap();
        runtime
            .run_frame(Constraints::tight(Size::new(20., 20.)))
            .unwrap();
        let _ = runtime.handle_input(InputEvent::PointerWithId {
            pointer: 72,
            phase: PointerPhase::Down,
            position: Offset::new(5., 5.),
        });
        assert_eq!(
            runtime
                .handle_input(InputEvent::PointerWithId {
                    pointer: 72,
                    phase: PointerPhase::Up,
                    position: Offset::new(5., 5.),
                })
                .unwrap()
                .action,
            Some(ActionId(2))
        );
    }
    #[test]
    fn button_hover_callbacks_fire_once_on_enter_and_exit() {
        let enters = Rc::new(Cell::new(0_u32));
        let exits = Rc::new(Cell::new(0_u32));
        let mut runtime = Runtime::new(
            ActionSurface::new("Hover")
                .on_hover({
                    let enters = enters.clone();
                    move || enters.set(enters.get() + 1)
                })
                .on_exit({
                    let exits = exits.clone();
                    move || exits.set(exits.get() + 1)
                })
                .into(),
        )
        .unwrap();
        runtime
            .run_frame(Constraints::tight(Size::new(100., 40.)))
            .unwrap();
        for position in [
            Offset::new(10., 10.),
            Offset::new(20., 10.),
            Offset::new(150., 10.),
            Offset::new(160., 10.),
        ] {
            let _ = runtime.handle_input(InputEvent::Pointer {
                phase: PointerPhase::Move,
                position,
            });
        }
        assert_eq!(enters.get(), 1);
        assert_eq!(exits.get(), 1);
    }
    #[test]
    fn wheel_updates_only_retained_scroll_transform() {
        let controller = incular_widgets::ScrollController::new();
        let child = Widget::column(
            (0..8)
                .map(|_| Widget::box_(Size::new(80., 40.), Color::WHITE))
                .collect::<Vec<_>>(),
        );
        let mut runtime = Runtime::new(Widget::scroll_view(controller.clone(), child)).unwrap();
        let constraints = Constraints::tight(Size::new(100., 100.));
        let (_, initial) = runtime.run_frame(constraints).unwrap();
        assert!(
            initial.laid_out_render_objects > 0
                && initial.repainted_render_objects > 0
                && initial.composited > 0
        );
        let _ = runtime.handle_input(InputEvent::Pointer {
            phase: PointerPhase::Move,
            position: Offset::new(10., 10.),
        });
        let _ = runtime.handle_input(InputEvent::Scroll {
            delta: Offset::new(0., 60.),
        });
        let (_, frame) = runtime.run_frame(constraints).unwrap();
        assert_eq!(frame.rebuilt_elements, 0);
        assert_eq!(frame.laid_out_render_objects, 0);
        assert!(frame.repainted_render_objects <= 1); // overlay scrollbar only
        assert!(frame.composited > 0);
        assert_eq!(controller.offset(), 60.);
    }
    #[test]
    fn translated_button_hit_tests_at_its_visible_position_without_repaint() {
        let controller = incular_widgets::internal::TranslationController::new();
        controller.set_offset(Offset::new(0., 30.));
        let mut runtime = Runtime::new(Widget::translate(
            controller.clone(),
            incular_widgets::internal::action(Size::new(20., 20.), Color::WHITE, ActionId(9)),
        ))
        .unwrap();
        let constraints = Constraints::tight(Size::new(100., 100.));
        let _ = runtime.run_frame(constraints).unwrap();
        let (_, frame) = runtime.run_frame(constraints).unwrap();
        assert_eq!(frame.repainted_render_objects, 0);
        assert!(
            runtime
                .handle_input(InputEvent::Pointer {
                    phase: PointerPhase::Down,
                    position: Offset::new(5., 35.)
                })
                .is_some()
        );
        assert!(
            runtime
                .handle_input(InputEvent::Pointer {
                    phase: PointerPhase::Down,
                    position: Offset::new(5., 5.)
                })
                .is_none()
        );
    }
    #[test]
    fn scrolled_button_hits_at_visible_not_old_location() {
        let controller = incular_widgets::ScrollController::new();
        let content = Widget::column(vec![
            Widget::box_(Size::new(80., 160.), Color::WHITE),
            incular_widgets::internal::action(Size::new(20., 20.), Color::WHITE, ActionId(12)),
        ]);
        let mut runtime = Runtime::new(Widget::scroll_view(controller.clone(), content)).unwrap();
        let constraints = Constraints::tight(Size::new(100., 100.));
        let _ = runtime.run_frame(constraints).unwrap();
        assert!(controller.jump_to(80.));
        let (_, frame) = runtime.run_frame(constraints).unwrap();
        assert!(frame.repainted_render_objects <= 1); // overlay scrollbar only
        assert_eq!(
            runtime
                .handle_input(InputEvent::Pointer {
                    phase: PointerPhase::Down,
                    position: Offset::new(5., 85.)
                })
                .unwrap()
                .action,
            Some(ActionId(12))
        );
        assert!(
            runtime
                .handle_input(InputEvent::Pointer {
                    phase: PointerPhase::Down,
                    position: Offset::new(5., 165.)
                })
                .is_none()
        );
    }
    #[test]
    fn animation_ticks_request_frames_without_rebuild_or_paint() {
        let controller = incular_widgets::internal::TranslationController::new();
        let mut runtime = Runtime::new(Widget::translate(
            controller.clone(),
            Widget::text("warm text"),
        ))
        .unwrap();
        let constraints = Constraints::tight(Size::new(100., 100.));
        let origin = Instant::now();
        let _ = runtime.run_frame_at(constraints, origin).unwrap();
        let text_before = runtime.tree().text_diagnostics();
        controller.animate_to(Offset::new(50., 0.), Duration::from_millis(1000), origin);
        let (_, frame) = runtime
            .run_frame_at(constraints, origin + Duration::from_millis(500))
            .unwrap();
        assert_eq!(frame.rebuilt_elements, 0);
        assert_eq!(frame.laid_out_render_objects, 0);
        assert_eq!(frame.repainted_render_objects, 0);
        assert!(
            frame.composited > 0 && frame.active_animations > 0 && frame.requested_another_frame
        );
        assert_eq!(runtime.tree().text_diagnostics(), text_before);
    }
    #[test]
    fn retained_card_text_and_background_move_together_without_repaint() {
        let controller = incular_widgets::internal::TranslationController::new();
        let mut runtime = Runtime::new(Widget::padding(
            incular_config::EdgeInsets {
                left: 20.,
                top: 10.,
                right: 0.,
                bottom: 0.,
            },
            Widget::translate(
                controller.clone(),
                Widget::column(vec![
                    Widget::box_(Size::new(80., 20.), Color::WHITE),
                    Widget::text("cached card text"),
                ]),
            ),
        ))
        .unwrap();
        let constraints = Constraints::tight(Size::new(200., 100.));
        let (before, _) = runtime.run_frame(constraints).unwrap();
        let text_before = runtime.tree().text_diagnostics();
        let (before_rect, before_glyph) = picture_origins(&before);
        controller.set_offset(Offset::new(50., 0.));
        let (after, frame) = runtime.run_frame(constraints).unwrap();
        let (after_rect, after_glyph) = picture_origins(&after);
        assert_eq!(frame.rebuilt_elements, 0);
        assert_eq!(frame.laid_out_render_objects, 0);
        assert_eq!(frame.repainted_render_objects, 0);
        assert!(frame.composited > 0);
        assert_eq!(after_rect - before_rect, Offset::new(50., 0.));
        assert_eq!(after_glyph - before_glyph, Offset::new(50., 0.));
        assert_eq!(runtime.tree().text_diagnostics(), text_before);
    }
    #[test]
    fn unmount_removes_signal_subscription() {
        let mut runtime = Runtime::new(Widget::row(vec![Widget::box_(
            Size::new(2., 2.),
            Color::WHITE,
        )]))
        .unwrap();
        let root = runtime.tree().root().unwrap();
        let child = runtime.tree().children(root).unwrap()[0];
        let signal = Signal::with_runtime(1_u32, &runtime);
        let state = signal.clone();
        runtime
            .register_builder(child, move || {
                Widget::box_(Size::new(state.get() as f32, 2.), Color::WHITE)
            })
            .unwrap();
        runtime
            .schedule_update(root, Widget::row(Vec::new()))
            .unwrap();
        runtime
            .run_frame(Constraints::tight(Size::new(20., 20.)))
            .unwrap();
        assert_eq!(signal.dependent_count(), 0);
    }
    #[test]
    fn declarative_button_dispatches_only_a_completed_press() {
        let hits = Rc::new(Cell::new(0));
        let callback_hits = hits.clone();
        let app = Application::new(move |_| {
            ActionSurface::new("Add")
                .on_press({
                    let callback_hits = callback_hits.clone();
                    move || callback_hits.set(callback_hits.get() + 1)
                })
                .into()
        })
        .unwrap();
        let mut runtime = app.into_runtime();
        runtime
            .run_frame(Constraints::tight(Size::new(120., 60.)))
            .unwrap();
        let _ = runtime.handle_input(InputEvent::Pointer {
            phase: PointerPhase::Down,
            position: Offset::new(10., 10.),
        });
        let _ = runtime.handle_input(InputEvent::Pointer {
            phase: PointerPhase::Up,
            position: Offset::new(121., 50.),
        });
        assert_eq!(hits.get(), 0);
        let _ = runtime.handle_input(InputEvent::Pointer {
            phase: PointerPhase::Down,
            position: Offset::new(10., 10.),
        });
        let _ = runtime.handle_input(InputEvent::Pointer {
            phase: PointerPhase::Up,
            position: Offset::new(10., 10.),
        });
        assert_eq!(hits.get(), 1);
    }

    #[test]
    fn decorated_ancestor_binds_nested_button_callback() {
        let hits = Rc::new(Cell::new(0));
        let observed = hits.clone();
        let root: Widget = DecoratedBox::new(
            ActionSurface::new("Nested").on_press(move || observed.set(observed.get() + 1)),
        )
        .background(Color::BLACK)
        .into();
        let mut runtime = Runtime::new(root).unwrap();
        runtime
            .run_frame(Constraints::tight(Size::new(120., 60.)))
            .unwrap();
        let down = runtime.handle_input(InputEvent::Pointer {
            phase: PointerPhase::Down,
            position: Offset::new(10., 10.),
        });
        let up = runtime.handle_input(InputEvent::Pointer {
            phase: PointerPhase::Up,
            position: Offset::new(10., 10.),
        });
        assert!(down.is_some_and(|target| target.action.is_some()));
        assert!(up.is_some_and(|target| target.action.is_some()));
        assert_eq!(hits.get(), 1);
    }

    #[test]
    fn sliver_list_keeps_small_scrolls_compositor_only_and_direct_jumps_bounded() {
        let controller = incular_widgets::ScrollController::new();
        let calls = Rc::new(Cell::new(0));
        let observed = calls.clone();
        let mut runtime = Runtime::new(fixed_sliver_list(
            1_000_000,
            40.,
            controller.clone(),
            move |index| {
                observed.set(observed.get() + 1);
                Widget::text(format!("Item {index}"))
            },
        ))
        .unwrap();
        let constraints = Constraints::tight(Size::new(120., 100.));
        let (_, initial) = runtime.run_frame(constraints).unwrap();
        assert!(initial.laid_out_render_objects > 0 && initial.repainted_render_objects > 0);
        let warm_calls = calls.get();
        let warm_text = runtime.tree().text_diagnostics();
        assert!(controller.jump_to(3.));
        let (_, small) = runtime.run_frame(constraints).unwrap();
        assert_eq!(calls.get(), warm_calls);
        assert_eq!(small.rebuilt_elements, 0);
        assert_eq!(small.laid_out_render_objects, 0);
        assert!(small.repainted_render_objects <= 1); // overlay scrollbar only
        assert!(small.composited > 0);
        assert_eq!(runtime.tree().text_diagnostics(), warm_text);

        let boundary_before = runtime.diagnostics();
        assert!(controller.jump_to(40.));
        let (_, boundary) = runtime.run_frame(constraints).unwrap();
        assert_eq!(calls.get(), warm_calls + 1);
        let boundary_after = runtime.diagnostics();
        assert_eq!(boundary_after.items_built - boundary_before.items_built, 1);
        assert_eq!(
            boundary_after.items_mounted - boundary_before.items_mounted,
            1
        );
        assert_eq!(
            boundary_after.items_unmounted - boundary_before.items_unmounted,
            0
        );
        assert!(boundary.laid_out_render_objects <= 2);
        assert!(boundary.repainted_render_objects <= 2);

        assert!(controller.jump_to(900_000. * 40.));
        let (_, jumped) = runtime.run_frame(constraints).unwrap();
        let diagnostics = runtime.tree().sliver_viewport_diagnostics().unwrap();
        assert!(diagnostics.materialized_range.contains(&900_000));
        assert!(diagnostics.materialized_item_count < 100);
        assert!(calls.get() < 200);
        assert!(jumped.laid_out_render_objects < 100);
        assert!(diagnostics.picture_layer_count < 250);
    }

    #[test]
    fn sliver_button_rows_hit_their_logical_index_and_drop_stale_handlers() {
        let controller = incular_widgets::ScrollController::new();
        let hit = Rc::new(Cell::new(None));
        let observed = hit.clone();
        let mut runtime = Runtime::new(fixed_sliver_list(
            2_000,
            40.,
            controller.clone(),
            move |index| {
                let hit = observed.clone();
                ActionSurface::new(format!("Item {index}")).on_press(move || hit.set(Some(index)))
            },
        ))
        .unwrap();
        let constraints = Constraints::tight(Size::new(120., 40.));
        runtime.run_frame(constraints).unwrap();
        assert!(controller.jump_to(500. * 40.));
        runtime.run_frame(constraints).unwrap();
        let down = runtime
            .handle_input(InputEvent::Pointer {
                phase: PointerPhase::Down,
                position: Offset::new(10., 10.),
            })
            .unwrap();
        let _ = runtime.handle_input(InputEvent::Pointer {
            phase: PointerPhase::Up,
            position: Offset::new(10., 10.),
        });
        assert_eq!(hit.get(), Some(500));
        let stale_action = down.action.unwrap();
        assert!(controller.jump_to(1_500. * 40.));
        runtime.run_frame(constraints).unwrap();
        assert!(!runtime.handlers.contains_key(&stale_action));
    }

    #[test]
    fn long_sliver_scroll_keeps_retained_resources_bounded() {
        let controller = incular_widgets::ScrollController::new();
        let mut runtime = Runtime::new(fixed_sliver_list(
            50_000,
            40.,
            controller.clone(),
            move |index| ActionSurface::new(format!("Item {index}")),
        ))
        .unwrap();
        let constraints = Constraints::tight(Size::new(120., 120.));
        runtime.run_frame(constraints).unwrap();
        for index in (97..10_000).step_by(97) {
            assert!(controller.jump_to(index as f32 * 40.));
            runtime.run_frame(constraints).unwrap();
            let view = runtime.tree().sliver_viewport_diagnostics().unwrap();
            assert!(view.materialized_item_count < 30);
            assert!(view.element_count < 100);
            assert!(view.render_object_count < 100);
            assert!(view.picture_layer_count < 200);
            assert!(runtime.handlers.len() < 30);
        }
    }

    #[test]
    fn mounted_keyboard_listener_dispatches_shortcuts_and_key_up() {
        use incular_widgets::{
            Actions, Command, FocusNode, KeyboardListener, ShortcutKey, Shortcuts,
        };

        let shortcut_hits = Rc::new(Cell::new(0));
        let observed_shortcut = shortcut_hits.clone();
        let mut actions = Actions::new();
        actions.register(Command::new("save"), move || {
            observed_shortcut.set(observed_shortcut.get() + 1);
        });
        let actions = Rc::new(actions);
        let mut shortcuts = Shortcuts::new();
        shortcuts.bind(
            ShortcutKey::new(Code::KeyS, Modifiers::CONTROL),
            Command::new("save"),
        );
        let shortcuts = Rc::new(shortcuts);

        let key_ups = Rc::new(Cell::new(0));
        let observed_key_up = key_ups.clone();
        let focus_node = FocusNode::new();
        let root: Widget = KeyboardListener::new(Text::new("keyboard target"))
            .focus_node(focus_node.clone())
            .autofocus(true)
            .with_shortcuts(shortcuts, actions)
            .on_key_up(move |_| observed_key_up.set(observed_key_up.get() + 1))
            .into();
        let mut runtime = Runtime::new(root).unwrap();
        runtime
            .run_frame(Constraints::tight(Size::new(220., 50.)))
            .unwrap();

        assert!(focus_node.has_focus());
        assert_eq!(runtime.focused_element(), runtime.tree().root());

        let mut down = shortcut_key_down(Code::KeyS);
        down.repeat = false;
        assert!(runtime.handle_input(InputEvent::Key(down)).is_none());
        assert_eq!(shortcut_hits.get(), 1);

        let mut up = KeyboardEvent::key_up(
            KeyboardKey::Named(incular_core::NamedKey::Unidentified),
            Code::KeyS,
        );
        up.modifiers = Modifiers::CONTROL;
        assert!(runtime.handle_input(InputEvent::Key(up)).is_none());
        assert_eq!(key_ups.get(), 1);
    }

    #[test]
    fn focus_routes_text_shortcuts_and_ime_without_rebuilding_tree() {
        use incular_core::ImeEvent;
        use incular_widgets::{EditableText, internal::TextEditingController};
        let first = TextEditingController::new();
        let second = TextEditingController::new();
        let mut runtime = Runtime::new(Widget::column(vec![
            EditableText::new(first.clone())
                .size(Size::new(120., 40.))
                .into(),
            EditableText::new(second.clone())
                .size(Size::new(120., 40.))
                .into(),
        ]))
        .unwrap();
        let constraints = Constraints::tight(Size::new(160., 100.));
        runtime.run_frame(constraints).unwrap();
        let before = runtime.tree().diagnostics();
        let _ = runtime.handle_input(InputEvent::Pointer {
            phase: PointerPhase::Down,
            position: Offset::new(5., 5.),
        });
        let _ = runtime.handle_input(InputEvent::Text("café".into()));
        let _ = runtime.handle_input(InputEvent::Ime(ImeEvent::Preedit {
            text: "世界".into(),
            selection: None,
        }));
        assert_eq!(first.text(), "café");
        let _ = runtime.handle_input(InputEvent::Ime(ImeEvent::Commit("世界".into())));
        assert_eq!(first.text(), "café世界");
        let _ = runtime.handle_input(InputEvent::Key(key_down(Code::Tab)));
        assert_ne!(runtime.focused_element(), runtime.tree().root());
        let _ = runtime.handle_input(InputEvent::Text("next".into()));
        assert_eq!(second.text(), "next");
        let _ = runtime.handle_input(InputEvent::Key(shortcut_key_down(Code::KeyA)));
        let _ = runtime.handle_input(InputEvent::Key(shortcut_key_down(Code::KeyX)));
        assert_eq!(second.text(), "");
        let _ = runtime.handle_input(InputEvent::Key(shortcut_key_down(Code::KeyV)));
        assert_eq!(second.text(), "next");
        let (_, stats) = runtime.run_frame(constraints).unwrap();
        assert_eq!(runtime.tree().diagnostics().rebuilds, before.rebuilds);
        // The two fields plus their flex ancestor are repainted; unrelated
        // widget descriptions were not rebuilt.
        assert!(stats.repainted_render_objects <= 3);
    }

    #[test]
    fn focused_native_style_backspace_repeat_and_delete_edit_the_buffer() {
        use incular_widgets::{EditableText, internal::TextEditingController};
        let controller = TextEditingController::with_text("abc");
        let mut runtime = Runtime::new(EditableText::new(controller.clone()).into()).unwrap();
        let constraints = Constraints::tight(Size::new(140., 50.));
        runtime.run_frame(constraints).unwrap();
        let _ = runtime.handle_input(InputEvent::Pointer {
            phase: PointerPhase::Down,
            position: Offset::new(100., 5.),
        });
        for expected in ["ab", "a", "", ""] {
            let mut event = key_down(Code::Backspace);
            event.repeat = true;
            let _ = runtime.handle_input(InputEvent::Key(event));
            assert_eq!(controller.text(), expected);
        }
        controller.set_text("é👩‍💻");
        let _ = runtime.handle_input(InputEvent::Key(key_down(Code::Backspace)));
        assert_eq!(controller.text(), "é");
        controller.set_selection(TextSelection::collapsed(0));
        let _ = runtime.handle_input(InputEvent::Key(key_down(Code::Delete)));
        assert_eq!(controller.text(), "");
        assert_eq!(runtime.editing_diagnostics().backspace_commands, 5);
        assert_eq!(runtime.editing_diagnostics().delete_commands, 1);
    }

    #[test]
    fn selectable_text_pointer_drag_shift_extension_and_copy_are_read_only() {
        use incular_widgets::internal::SelectionAreaController;

        #[derive(Clone)]
        struct TestClipboard(Rc<RefCell<String>>);
        impl Clipboard for TestClipboard {
            fn get_text(&mut self) -> Option<String> {
                (!self.0.borrow().is_empty()).then(|| self.0.borrow().clone())
            }
            fn set_text(&mut self, text: String) {
                *self.0.borrow_mut() = text;
            }
        }

        let controller = SelectionAreaController::new();
        let mut runtime = Runtime::new(Widget::selection_area(
            controller.clone(),
            Widget::column(vec![
                Widget::selectable_text_styled(
                    "first",
                    incular_text::TextStyle::default(),
                    incular_text::TextAlign::Start,
                ),
                Widget::selectable_text_styled(
                    "second",
                    incular_text::TextStyle::default(),
                    incular_text::TextAlign::Start,
                ),
            ]),
        ))
        .unwrap();
        let constraints = Constraints::tight(Size::new(180., 80.));
        let _ = runtime.run_frame(constraints).unwrap();
        let area = runtime.tree().root().unwrap();
        let column = runtime.tree().children(area).unwrap()[0];
        let labels = runtime.tree().children(column).unwrap();
        let second = labels[1];
        let first_bounds = runtime.tree().element_bounds(labels[0]).unwrap();
        let second_bounds = runtime.tree().element_bounds(second).unwrap();
        let first_point = first_bounds.origin + Offset::new(1., first_bounds.size.height * 0.5);
        // Stay inside the hit-test box while asking Parley's layout for the
        // final visual cluster.
        let second_point = second_bounds.origin
            + Offset::new(
                (second_bounds.size.width - 0.1).max(0.),
                second_bounds.size.height * 0.5,
            );
        assert_eq!(
            runtime.tree().selectable_text_at(first_point),
            Some(labels[0])
        );
        assert_eq!(
            runtime.tree().selectable_text_at(second_point),
            Some(second)
        );
        let copied = Rc::new(RefCell::new(String::new()));
        runtime.set_clipboard(Box::new(TestClipboard(copied.clone())));
        let _ = runtime.handle_input(InputEvent::Pointer {
            phase: PointerPhase::Down,
            position: first_point,
        });
        let _ = runtime.handle_input(InputEvent::Pointer {
            phase: PointerPhase::Move,
            position: second_point,
        });
        let _ = runtime.handle_input(InputEvent::Pointer {
            phase: PointerPhase::Up,
            position: second_point,
        });
        let mut shift_right = key_down(Code::ArrowRight);
        shift_right.modifiers = Modifiers::SHIFT;
        let _ = runtime.handle_input(InputEvent::Key(shift_right));
        assert_eq!(controller.selected_text(), "first\nsecond");
        let _ = runtime.handle_input(InputEvent::Key(shortcut_key_down(Code::KeyC)));
        assert_eq!(&*copied.borrow(), "first\nsecond");
        assert_eq!(runtime.focused_element(), Some(second));
    }

    #[test]
    fn multiline_enter_replaces_selection_while_single_line_submits() {
        use incular_widgets::{
            EditableText,
            internal::{TextEditingController, TextSelection},
        };
        let single = TextEditingController::with_text("one");
        let multi = TextEditingController::with_text("ab cdef");
        let submitted = Rc::new(RefCell::new(0));
        let observed = submitted.clone();
        let mut runtime = Runtime::new(Widget::column(vec![
            EditableText::new(single.clone())
                .on_submit(move |_| *observed.borrow_mut() += 1)
                .into(),
            EditableText::new(multi.clone())
                .multiline(true)
                .height(100.)
                .into(),
        ]))
        .unwrap();
        runtime
            .run_frame(Constraints::tight(Size::new(300., 200.)))
            .unwrap();
        let enter = || InputEvent::Key(key_down(Code::Enter));
        let _ = runtime.handle_input(InputEvent::Pointer {
            phase: PointerPhase::Down,
            position: Offset::new(5., 5.),
        });
        let _ = runtime.handle_input(enter());
        assert_eq!(single.text(), "one");
        assert_eq!(*submitted.borrow(), 1);
        let _ = runtime.handle_input(InputEvent::Pointer {
            phase: PointerPhase::Down,
            position: Offset::new(5., 50.),
        });
        multi.set_selection(TextSelection { base: 2, extent: 5 });
        let _ = runtime.handle_input(enter());
        assert_eq!(multi.text(), "ab\nef");
        let mut shifted_enter = key_down(Code::Enter);
        shifted_enter.modifiers = Modifiers::SHIFT;
        let _ = runtime.handle_input(InputEvent::Key(shifted_enter));
        assert_eq!(multi.text(), "ab\n\nef");
    }

    #[test]
    fn runtime_routes_pointer_sequences_to_retained_gesture_regions() {
        let taps = Rc::new(Cell::new(0));
        let observed = taps.clone();
        let mut runtime = Runtime::new(
            GestureDetector::new(Widget::box_(Size::new(80., 40.), Color::WHITE))
                .callbacks(GestureCallbacks {
                    on_tap: Some(Rc::new(move || observed.set(observed.get() + 1))),
                    ..GestureCallbacks::default()
                })
                .into(),
        )
        .unwrap();
        runtime
            .run_frame(Constraints::tight(Size::new(100., 100.)))
            .unwrap();
        let down = runtime.handle_input(InputEvent::Pointer {
            phase: PointerPhase::Down,
            position: Offset::new(10., 10.),
        });
        assert!(down.is_some_and(|target| target.action.is_none()));
        let up = runtime.handle_input(InputEvent::Pointer {
            phase: PointerPhase::Up,
            position: Offset::new(90., 90.),
        });
        assert!(up.is_some_and(|target| target.action.is_none()));
        assert_eq!(taps.get(), 1);
    }

    #[test]
    fn runtime_routes_identified_contacts_to_retained_scale_regions() {
        let scale = Rc::new(Cell::new(0.));
        let observed = scale.clone();
        let mut runtime = Runtime::new(
            GestureDetector::new(Widget::box_(Size::new(100., 100.), Color::WHITE))
                .callbacks(GestureCallbacks {
                    on_scale_update: Some(Rc::new(move |details| observed.set(details.scale))),
                    ..GestureCallbacks::default()
                })
                .into(),
        )
        .unwrap();
        runtime
            .run_frame(Constraints::tight(Size::new(100., 100.)))
            .unwrap();
        for (pointer, position) in [(1, Offset::new(10., 10.)), (2, Offset::new(20., 10.))] {
            assert!(
                runtime
                    .handle_input(InputEvent::PointerWithId {
                        pointer,
                        phase: PointerPhase::Down,
                        position,
                    })
                    .is_some()
            );
        }
        assert!(
            runtime
                .handle_input(InputEvent::PointerWithId {
                    pointer: 2,
                    phase: PointerPhase::Move,
                    position: Offset::new(30., 10.),
                })
                .is_some()
        );
        assert_eq!(scale.get(), 2.);
    }

    fn test_window_options(title: &str, width: f32, height: f32) -> WindowOptions {
        WindowOptions {
            title: title.into(),
            initial_logical_size: Size::new(width, height),
            ..WindowOptions::default()
        }
    }

    #[test]
    fn virtual_windows_keep_trees_environments_focus_and_semantics_independent() {
        let mut application = Application::new(|_| ActionSurface::new("A").into()).unwrap();
        let a = application.primary_window();
        let b = application
            .open_window_with(test_window_options("B", 360., 240.), |_| {
                ActionSurface::new("B").into()
            })
            .unwrap()
            .id();
        let environment_a = RuntimeEnvironment {
            viewport: Size::new(640., 400.),
            scale_factor: 1.,
            safe_insets: incular_config::EdgeInsets::all(4.),
            ..RuntimeEnvironment::default()
        };
        let environment_b = RuntimeEnvironment {
            viewport: Size::new(320., 180.),
            scale_factor: 2.,
            safe_insets: incular_config::EdgeInsets::all(9.),
            ..RuntimeEnvironment::default()
        };
        assert!(application.set_window_environment(a, environment_a));
        assert!(application.set_window_environment(b, environment_b));
        application.handle_window_event(WindowEvent::platform(
            a,
            PlatformEvent::Metrics(WindowMetrics::new(
                incular_platform::PhysicalSize::new(640, 400),
                1.,
            )),
        ));
        application.handle_window_event(WindowEvent::platform(
            b,
            PlatformEvent::Metrics(WindowMetrics::new(
                incular_platform::PhysicalSize::new(640, 360),
                2.,
            )),
        ));
        application.handle_window_event(WindowEvent::lifecycle(a, WindowLifecycle::Focused));
        application.handle_window_event(WindowEvent::lifecycle(b, WindowLifecycle::Unfocused));
        let _ = application
            .run_window_frame_at(a, Constraints::tight(Size::new(640., 400.)), Instant::now())
            .unwrap();
        let _ = application
            .run_window_frame_at(b, Constraints::tight(Size::new(320., 180.)), Instant::now())
            .unwrap();
        let a_state = application.window_diagnostics(a).unwrap();
        let b_state = application.window_diagnostics(b).unwrap();
        assert_ne!(a_state.logical_size, b_state.logical_size);
        assert_eq!(a_state.scale_factor, 1.);
        assert_eq!(b_state.scale_factor, 2.);
        assert!(a_state.native_focused);
        assert!(!b_state.native_focused);
        assert!(a_state.semantics > 0 && b_state.semantics > 0);
        let b_surface = b_state.surface_generation;
        application.handle_window_event(WindowEvent::platform(
            a,
            PlatformEvent::Metrics(WindowMetrics::new(
                incular_platform::PhysicalSize::new(1_200, 800),
                1.5,
            )),
        ));
        assert!(
            application
                .window_diagnostics(a)
                .unwrap()
                .surface_generation
                > a_state.surface_generation
        );
        assert_eq!(
            application
                .window_diagnostics(b)
                .unwrap()
                .surface_generation,
            b_surface
        );
    }

    #[cfg(feature = "devtools")]
    #[test]
    fn devtools_overlays_and_deep_trace_are_routed_to_one_window() {
        let mut application =
            Application::new(|_| Widget::box_(Size::new(80., 40.), Color::WHITE)).unwrap();
        let a = application.primary_window();
        let b = application
            .open_window_with(test_window_options("B", 240., 120.), |_| {
                Widget::box_(Size::new(30., 20.), Color::WHITE)
            })
            .unwrap()
            .id();
        for (window, size) in [(a, Size::new(100., 60.)), (b, Size::new(240., 120.))] {
            application.handle_window_event(WindowEvent::platform(
                window,
                PlatformEvent::Metrics(WindowMetrics::new(
                    incular_platform::PhysicalSize::new(size.width as u32, size.height as u32),
                    1.,
                )),
            ));
            application
                .run_window_frame_at(window, Constraints::tight(size), Instant::now())
                .unwrap();
        }
        let a_bounds = application.devtools_window_layout_bounds(a, 8);
        let b_bounds = application.devtools_window_layout_bounds(b, 8);
        assert_eq!(a_bounds[0][2..], [100., 60.]);
        assert_eq!(b_bounds[0][2..], [240., 120.]);

        assert!(application.devtools_begin_deep_trace(b, 128));
        application
            .run_window_frame_at(b, Constraints::tight(Size::new(250., 120.)), Instant::now())
            .unwrap();
        assert!(application.devtools_take_deep_trace(a, 1).is_none());
        let trace = application
            .devtools_take_deep_trace(b, 1)
            .expect("window B trace");
        assert_eq!(trace.window.index(), u64::from(b.index()) + 1);
        assert!(!trace.events.is_empty());
    }

    #[cfg(feature = "devtools")]
    #[test]
    fn devtools_property_edit_routes_to_the_live_window_and_requests_a_frame() {
        let mut application = Application::new(|_| {
            Widget::opacity(0.8, Widget::box_(Size::new(20., 20.), Color::WHITE))
        })
        .unwrap();
        let window = application.primary_window();
        application
            .run_window_frame_at(
                window,
                Constraints::tight(Size::new(40., 40.)),
                Instant::now(),
            )
            .unwrap();
        let protocol_window = incular_devtools_protocol::DevWindowId::new(
            u64::from(window.index()) + 1,
            u64::from(window.generation()),
        );
        let snapshot = application
            .devtools_widget_tree(protocol_window)
            .expect("widget tree");
        let incular_devtools_protocol::TreeDelta::Snapshot { root, .. } = &snapshot[0] else {
            panic!("full tree snapshot");
        };
        assert!(application.devtools_edit_property(
            root.id,
            "opacity",
            &incular_devtools_protocol::DebugValue::Float(0.3)
        ));
        assert!(application.frame_requested(window));
        let details = application
            .devtools_node_details(root.id)
            .expect("edited details");
        assert!(details.properties.iter().any(|property| {
            property.name == "opacity"
                && matches!(property.value, incular_devtools_protocol::DebugValue::Float(value) if (value - 0.3).abs() < 0.000_001)
        }));
    }

    #[test]
    fn accessibility_projections_actions_and_close_are_window_local() {
        let a_hits = Rc::new(Cell::new(0));
        let mut application = Application::new({
            let a_hits = a_hits.clone();
            move |_| {
                ActionSurface::new("A")
                    .on_press({
                        let a_hits = a_hits.clone();
                        move || a_hits.set(a_hits.get() + 1)
                    })
                    .into()
            }
        })
        .unwrap();
        let a = application.primary_window();
        let b = application
            .open_window_with(test_window_options("B", 180., 100.), |_| {
                ActionSurface::new("B").into()
            })
            .unwrap()
            .id();
        for id in [a, b] {
            application
                .run_window_frame_at(
                    id,
                    Constraints::tight(Size::new(180., 100.)),
                    Instant::now(),
                )
                .unwrap();
        }
        let mut adapter_a = AccessKitProjection::new();
        let mut adapter_b = AccessKitProjection::new();
        adapter_a.activate();
        adapter_b.activate();
        assert_eq!(
            application
                .sync_accessibility(a, &mut adapter_a)
                .expect("A initial tree")
                .kind(),
            incular_accessibility::AccessKitUpdateKind::Full
        );
        assert_eq!(
            application
                .sync_accessibility(b, &mut adapter_b)
                .expect("B initial tree")
                .kind(),
            incular_accessibility::AccessKitUpdateKind::Full
        );
        assert!(application.sync_accessibility(b, &mut adapter_b).is_none());

        let a_button = application
            .with_window_mut(a, |record| {
                semantic_node(&record.runtime, SemanticRole::Button)
            })
            .unwrap();
        let native = NodeId(adapter_a.native_node_id(a_button).unwrap());
        let request = ActionRequest {
            action: Action::Click,
            target_tree: TreeId::ROOT,
            target_node: native,
            data: None,
        };
        let request = adapter_a.translate_action(&request).expect("live A node");
        assert!(application.dispatch_accessibility_action(a, request));
        assert_eq!(a_hits.get(), 1);
        // A's action does not publish or mutate B's adapter/tree.
        assert!(application.sync_accessibility(b, &mut adapter_b).is_none());

        assert!(application.close_window(a));
        adapter_a.note_adapter_destroyed();
        assert!(!application.contains_window(a));
        assert!(application.contains_window(b));
        application
            .run_window_frame_at(b, Constraints::tight(Size::new(180., 100.)), Instant::now())
            .unwrap();
        assert!(application.window_diagnostics(b).is_some());
    }

    #[test]
    fn window_events_route_to_only_the_selected_retained_root() {
        let a_hits = Rc::new(Cell::new(0));
        let b_hits = Rc::new(Cell::new(0));
        let a_observed = a_hits.clone();
        let mut application = Application::new(move |_| {
            ActionSurface::new("A")
                .on_press({
                    let hits = a_observed.clone();
                    move || hits.set(hits.get() + 1)
                })
                .into()
        })
        .unwrap();
        let a = application.primary_window();
        let b_observed = b_hits.clone();
        let b = application
            .open_window_with(test_window_options("B", 120., 80.), move |_| {
                ActionSurface::new("B")
                    .on_press({
                        let hits = b_observed.clone();
                        move || hits.set(hits.get() + 1)
                    })
                    .into()
            })
            .unwrap()
            .id();
        for id in [a, b] {
            let _ = application
                .run_window_frame_at(id, Constraints::tight(Size::new(120., 80.)), Instant::now())
                .unwrap();
            for phase in [PointerPhase::Down, PointerPhase::Up] {
                application.handle_window_event(WindowEvent::platform(
                    id,
                    PlatformEvent::Input(InputEvent::Pointer {
                        phase,
                        position: Offset::new(10., 10.),
                    }),
                ));
            }
        }
        assert_eq!(a_hits.get(), 1);
        assert_eq!(b_hits.get(), 1);
    }

    #[test]
    fn shared_and_window_local_signals_dirty_only_subscribed_windows() {
        let shared = Signal::new(0_u32);
        let local_a = Signal::new(0_u32);
        let a_shared = shared.clone();
        let a_local = local_a.clone();
        let mut application = Application::new(move |_| {
            Text::new(format!("{}:{}", a_shared.get(), a_local.get())).into()
        })
        .unwrap();
        let a = application.primary_window();
        let b_shared = shared.clone();
        let b = application
            .open_window_with(test_window_options("B", 180., 100.), move |_| {
                Text::new(format!("{}", b_shared.get())).into()
            })
            .unwrap()
            .id();
        for id in [a, b] {
            let _ = application
                .run_window_frame_at(
                    id,
                    Constraints::tight(Size::new(180., 100.)),
                    Instant::now(),
                )
                .unwrap();
        }
        assert!(shared.set(1));
        assert!(application.frame_requested(a));
        assert!(application.frame_requested(b));
        let _ = application
            .run_window_frame_at(a, Constraints::tight(Size::new(180., 100.)), Instant::now())
            .unwrap();
        let _ = application
            .run_window_frame_at(b, Constraints::tight(Size::new(180., 100.)), Instant::now())
            .unwrap();
        local_a.set(1);
        assert!(application.frame_requested(a));
        assert!(!application.frame_requested(b));
    }

    #[test]
    fn close_cancels_only_its_window_scope_and_rejects_stale_handle_commands() {
        let saved_scope = Rc::new(RefCell::new(None));
        let capture = saved_scope.clone();
        let mut application =
            Application::new(|_| Widget::fixed_box(Size::new(1., 1.), Color::WHITE)).unwrap();
        let application_task = application.spawn(async { 7_u32 });
        let stale = application
            .open_window_with(test_window_options("Old", 100., 100.), move |cx| {
                *capture.borrow_mut() = Some(cx.task_scope());
                Widget::fixed_box(Size::new(1., 1.), Color::WHITE)
            })
            .unwrap();
        let stale_id = stale.id();
        assert!(application.close_window(stale_id));
        assert!(saved_scope.borrow().as_ref().unwrap().is_cancelled());
        assert!(!application_task.handle().is_cancelled());
        let replacement = application
            .open_window(
                test_window_options("Replacement", 100., 100.),
                Widget::fixed_box(Size::new(1., 1.), Color::WHITE),
            )
            .unwrap();
        assert_eq!(replacement.id().index(), stale_id.index());
        assert_ne!(replacement.id().generation(), stale_id.generation());
        assert!(stale.set_title("stale"));
        application.process_runtime_work();
        assert_eq!(
            application
                .window_diagnostics(replacement.id())
                .unwrap()
                .title,
            "Replacement"
        );
        assert!(application.diagnostics().stale_window_commands >= 1);
    }

    #[test]
    fn closing_one_window_and_stress_open_close_leave_other_roots_alive() {
        let mut application =
            Application::new(|_| Widget::fixed_box(Size::new(2., 2.), Color::WHITE)).unwrap();
        let primary = application.primary_window();
        let other = application
            .open_window(
                test_window_options("Other", 100., 100.),
                Widget::fixed_box(Size::new(2., 2.), Color::WHITE),
            )
            .unwrap()
            .id();
        assert!(application.close_window(other));
        assert!(application.contains_window(primary));
        for index in 0..1_000 {
            let handle = application
                .open_window(
                    test_window_options(&format!("Transient {index}"), 80., 60.),
                    Widget::fixed_box(Size::new(1., 1.), Color::WHITE),
                )
                .unwrap();
            assert!(application.close_window(handle.id()));
        }
        assert_eq!(application.active_window_ids(), vec![primary]);
        assert_eq!(application.diagnostics().active_windows, 1);
    }

    #[test]
    fn final_window_policy_and_simultaneous_virtual_roots_are_explicit() {
        let mut application =
            Application::new(|_| Widget::fixed_box(Size::new(1., 1.), Color::WHITE)).unwrap();
        for index in 0..31 {
            application
                .open_window(
                    test_window_options(&format!("Window {index}"), 80., 60.),
                    Widget::fixed_box(Size::new(1., 1.), Color::WHITE),
                )
                .unwrap();
        }
        assert_eq!(application.diagnostics().active_windows, 32);
        application.set_last_window_policy(LastWindowPolicy::KeepRunning);
        for id in application.active_window_ids() {
            assert!(application.close_window(id));
        }
        assert!(!application.should_exit());

        let mut default_policy =
            Application::new(|_| Widget::fixed_box(Size::new(1., 1.), Color::WHITE)).unwrap();
        default_policy
            .open_window(
                WindowOptions {
                    visible: false,
                    ..test_window_options("Hidden helper", 80., 60.)
                },
                Widget::fixed_box(Size::new(1., 1.), Color::WHITE),
            )
            .unwrap();
        assert!(default_policy.close_window(default_policy.primary_window()));
        assert!(default_policy.should_exit());
    }

    #[test]
    fn restoration_persists_opt_in_values_before_the_next_build() {
        fn make_application(
            store: Arc<InMemoryRestorationStore>,
            slot: Rc<RefCell<Option<Restorable<u32>>>>,
        ) -> Application {
            let build_slot = slot.clone();
            Application::new_with_restoration(
                test_window_options("Restoration", 200., 100.),
                RestorationConfig::new("com.example.runtime-restoration", 1, store)
                    .with_debounce(Duration::ZERO),
                move |cx| {
                    if build_slot.borrow().is_none() {
                        *build_slot.borrow_mut() =
                            cx.restorable(RestorationKey::new("count").unwrap(), 0_u32);
                    }
                    Text::new(build_slot.borrow().as_ref().unwrap().get().to_string()).into()
                },
            )
            .unwrap()
        }

        let store = Arc::new(InMemoryRestorationStore::new());
        let first_slot = Rc::new(RefCell::new(None));
        let mut first = make_application(store.clone(), first_slot.clone());
        first_slot.borrow().as_ref().unwrap().set(17);
        first.shutdown();
        assert!(store.bytes().is_some(), "shutdown flush stores a snapshot");

        let restored_slot = Rc::new(RefCell::new(None));
        let restored = make_application(store.clone(), restored_slot.clone());
        assert_eq!(restored_slot.borrow().as_ref().unwrap().get(), 17);
        drop(restored);
    }

    #[test]
    fn restorable_windows_use_stable_ids_and_user_close_removes_auxiliary_descriptor() {
        let store = Arc::new(InMemoryRestorationStore::new());
        let config = RestorationConfig::new("com.example.window-restore", 1, store.clone())
            .with_debounce(Duration::ZERO);
        let mut first = Application::new_restorable(
            WindowRestorationId::new("main").unwrap(),
            "main",
            test_window_options("Main", 320., 200.),
            config,
            |_| Widget::fixed_box(Size::new(1., 1.), Color::WHITE),
        )
        .unwrap();
        let inspector = first
            .open_restorable_window_with(
                WindowRestorationId::new("inspector").unwrap(),
                "inspector",
                test_window_options("Inspector", 480., 260.),
                |_| Widget::fixed_box(Size::new(1., 1.), Color::WHITE),
            )
            .unwrap();
        assert_eq!(first.active_window_ids().len(), 2);
        first.shutdown();

        let config = RestorationConfig::new("com.example.window-restore", 1, store.clone())
            .with_debounce(Duration::ZERO);
        let mut second = Application::new_restorable(
            WindowRestorationId::new("main").unwrap(),
            "main",
            test_window_options("Main default", 100., 100.),
            config,
            |_| Widget::fixed_box(Size::new(1., 1.), Color::WHITE),
        )
        .unwrap();
        second
            .register_restorable_window_factory(
                "inspector",
                test_window_options("Inspector default", 100., 100.),
                |_| Widget::fixed_box(Size::new(1., 1.), Color::WHITE),
            )
            .unwrap();
        assert_eq!(second.restore_restorable_windows().unwrap(), 1);
        assert_eq!(second.active_window_ids().len(), 2);
        let inspector_id = second
            .active_window_ids()
            .into_iter()
            .find(|id| *id != second.primary_window())
            .unwrap();
        assert_eq!(
            second
                .window_diagnostics(inspector_id)
                .unwrap()
                .logical_size,
            Size::new(480., 260.)
        );
        assert!(second.close_window(inspector_id));
        second.shutdown();
        let bytes = store.bytes().unwrap();
        let snapshot: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(snapshot["windows"].as_array().unwrap().len(), 1);
        // Session-local WindowIds remain unrelated to the stable persistence
        // key; reopening the same descriptor never serializes this value.
        assert_ne!(inspector.id(), second.primary_window());
    }
}
