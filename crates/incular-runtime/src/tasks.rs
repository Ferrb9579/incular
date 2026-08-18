//! Tokio-backed application work and the UI-thread completion bridge.
//!
//! Tokio owns every general future, timer, and blocking job. This module owns
//! only Incular-specific lifetime metadata and a bounded message drain into the
//! retained UI runtime; it never polls a user future itself.

use std::{
    collections::HashMap,
    panic::{self, AssertUnwindSafe},
    rc::{Rc, Weak},
    sync::{
        Arc, Mutex, Weak as ArcWeak,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};

use incular_platform::WindowId;
use incular_widgets::ElementId;
use tokio::{
    runtime::{Builder, Handle, Runtime as TokioRuntime},
    task::{AbortHandle, JoinHandle},
};

use crate::Runtime;

const DEFAULT_UI_MESSAGE_BUDGET: usize = 128;
const SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(100);

/// Native event-loop wake bridge. Implementations are thread-safe and must
/// never borrow the retained UI runtime directly.
pub trait RuntimeWake: Send + Sync + 'static {
    fn wake(&self);
}

/// Failure of Tokio-backed work as observed by the UI owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TaskFailure {
    Cancelled,
    Panicked,
    RuntimeStopped,
}

/// A small application-state value for asynchronous results. It is not an
/// executor; applications normally store it in a `Signal` from a UI callback.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AsyncValue<T> {
    Idle,
    Loading,
    Ready(T),
    Error(TaskFailure),
}

impl<T> AsyncValue<T> {
    #[must_use]
    pub const fn is_loading(&self) -> bool {
        matches!(self, Self::Loading)
    }

    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Ready(_) | Self::Error(_))
    }
}

/// A cancellation and diagnostics handle for Tokio work tracked by Incular.
/// It intentionally is not another `JoinHandle`: use [`TokioHandle`] for
/// direct Tokio APIs when component ownership is not required.
#[derive(Clone)]
pub struct TaskHandle {
    control: Arc<TaskControl>,
}

impl TaskHandle {
    /// Requests cancellation. Tokio async tasks are aborted; a started
    /// `spawn_blocking` job may finish, but its UI result is discarded.
    pub fn cancel(&self) {
        self.control.cancel();
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.control.is_cancelled()
    }
}

/// A typed UI-observable result of a Tokio task. This wrapper adds ownership
/// and cancellation metadata; it does not implement `Future` or duplicate
/// Tokio's join API.
pub struct Task<T> {
    state: Arc<Mutex<AsyncValue<T>>>,
    handle: TaskHandle,
}

impl<T: Clone> Task<T> {
    #[must_use]
    pub fn value(&self) -> AsyncValue<T> {
        if self.handle.is_cancelled() {
            AsyncValue::Error(TaskFailure::Cancelled)
        } else {
            self.state.lock().expect("task state mutex").clone()
        }
    }
}

impl<T> Task<T> {
    pub fn cancel(&self) {
        self.handle.cancel();
    }

    #[must_use]
    pub fn handle(&self) -> TaskHandle {
        self.handle.clone()
    }
}

/// Structured Incular ownership for Tokio tasks. Scopes may be associated with
/// a generational element identity; completions verify that identity on the UI
/// thread before running a local callback.
#[derive(Clone)]
pub struct TaskScope {
    state: Arc<ScopeState>,
}

struct ScopeState {
    cancelled: AtomicBool,
    owner: Mutex<Option<ElementId>>,
    window: Mutex<Option<WindowId>>,
    children: Mutex<HashMap<u64, ArcWeak<TaskControl>>>,
    child_scopes: Mutex<Vec<ArcWeak<ScopeState>>>,
    parent: ArcWeak<ScopeState>,
}

impl TaskScope {
    fn new() -> Self {
        Self {
            state: Arc::new(ScopeState {
                cancelled: AtomicBool::new(false),
                owner: Mutex::new(None),
                window: Mutex::new(None),
                children: Mutex::new(HashMap::new()),
                child_scopes: Mutex::new(Vec::new()),
                parent: ArcWeak::new(),
            }),
        }
    }

    /// Creates a scope whose cancellation is bounded by this scope. This is
    /// the ownership relation used for application → window → component work.
    #[must_use]
    pub(crate) fn child(&self) -> Self {
        let child = Self {
            state: Arc::new(ScopeState {
                cancelled: AtomicBool::new(self.is_cancelled()),
                owner: Mutex::new(None),
                window: Mutex::new(self.window_id()),
                children: Mutex::new(HashMap::new()),
                child_scopes: Mutex::new(Vec::new()),
                parent: Arc::downgrade(&self.state),
            }),
        };
        self.state
            .child_scopes
            .lock()
            .expect("task scope children mutex")
            .push(Arc::downgrade(&child.state));
        child
    }

    pub fn cancel(&self) {
        if self.state.cancelled.swap(true, Ordering::AcqRel) {
            return;
        }
        let children: Vec<_> = self
            .state
            .children
            .lock()
            .expect("task scope mutex")
            .values()
            .filter_map(ArcWeak::upgrade)
            .collect();
        for child in children {
            child.cancel();
        }
        let child_scopes: Vec<_> = self
            .state
            .child_scopes
            .lock()
            .expect("task scope children mutex")
            .drain(..)
            .filter_map(|scope| scope.upgrade())
            .collect();
        for scope in child_scopes {
            TaskScope { state: scope }.cancel();
        }
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.state.cancelled.load(Ordering::Acquire)
            || self
                .state
                .parent
                .upgrade()
                .is_some_and(|parent| parent.cancelled.load(Ordering::Acquire))
    }

    pub(crate) fn bind_owner(&self, owner: ElementId) {
        *self.state.owner.lock().expect("task scope owner mutex") = Some(owner);
    }

    pub(crate) fn bind_window(&self, window: WindowId) {
        *self.state.window.lock().expect("task scope window mutex") = Some(window);
    }

    #[must_use]
    pub(crate) fn window_id(&self) -> Option<WindowId> {
        (*self.state.window.lock().expect("task scope window mutex")).or_else(|| {
            self.state
                .parent
                .upgrade()
                .and_then(|parent| *parent.window.lock().expect("task scope window mutex"))
        })
    }

    fn attach(&self, control: &Arc<TaskControl>) {
        self.state
            .children
            .lock()
            .expect("task scope mutex")
            .insert(control.id, Arc::downgrade(control));
    }

    fn detach(&self, id: u64) {
        self.state
            .children
            .lock()
            .expect("task scope mutex")
            .remove(&id);
    }
}

impl Drop for TaskScope {
    fn drop(&mut self) {
        // A task only keeps a weak reference to its scope. The final owner
        // therefore cancels children rather than accidentally extending them.
        if Arc::strong_count(&self.state) == 1 {
            self.cancel();
        }
    }
}

pub(crate) struct TaskControl {
    id: u64,
    cancelled: AtomicBool,
    scope: ArcWeak<ScopeState>,
    abort: Mutex<Option<AbortHandle>>,
}

impl TaskControl {
    fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
        if let Some(abort) = self.abort.lock().expect("task abort mutex").as_ref() {
            abort.abort();
        }
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
            || self
                .scope
                .upgrade()
                .is_none_or(|scope| scope.cancelled.load(Ordering::Acquire))
    }

    pub(crate) fn owner_is_stale(&self, runtime: &Runtime) -> bool {
        let Some(scope) = self.scope.upgrade() else {
            return true;
        };
        scope
            .owner
            .lock()
            .expect("task scope owner mutex")
            .is_some_and(|owner| !runtime.tree().element_exists(owner))
    }

    pub(crate) fn window_id(&self) -> Option<WindowId> {
        self.scope.upgrade().and_then(|scope| {
            (*scope.window.lock().expect("task scope window mutex")).or_else(|| {
                scope
                    .parent
                    .upgrade()
                    .and_then(|parent| *parent.window.lock().expect("task scope window mutex"))
            })
        })
    }
}

/// The explicit Tokio handle exposed for advanced libraries. Direct tasks use
/// Tokio/application lifetime; they are not automatically component-scoped.
pub type TokioHandle = Handle;

/// UI-thread-only helper that launches Tokio work with Incular owner tracking.
#[derive(Clone)]
pub struct RuntimeSpawner {
    scheduler: Weak<std::cell::RefCell<TaskScheduler>>,
    application_scope: TaskScope,
    window_id: Option<WindowId>,
}

impl RuntimeSpawner {
    /// Starts application-scoped Tokio work. Its output is observed through the
    /// returned state handle; use `spawn_into` to update UI-local state.
    pub fn spawn<F, T>(&self, future: F) -> Task<T>
    where
        F: std::future::Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        self.spawn_in(&self.application_scope, future)
    }

    pub fn spawn_in<F, T>(&self, scope: &TaskScope, future: F) -> Task<T>
    where
        F: std::future::Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        self.scheduler
            .upgrade()
            .expect("runtime spawner outlived runtime")
            .borrow_mut()
            .spawn_task(scope, future)
    }

    /// Starts Tokio work and invokes `complete` later on the UI thread. The
    /// callback may capture non-`Send` `Signal`s because only the `Send` output
    /// crosses the Tokio/UI boundary.
    pub fn spawn_into<F, T>(
        &self,
        future: F,
        complete: impl FnOnce(Result<T, TaskFailure>, &mut Runtime) + 'static,
    ) -> TaskHandle
    where
        F: std::future::Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        self.spawn_into_in(&self.application_scope, future, complete)
    }

    pub fn spawn_into_in<F, T>(
        &self,
        scope: &TaskScope,
        future: F,
        complete: impl FnOnce(Result<T, TaskFailure>, &mut Runtime) + 'static,
    ) -> TaskHandle
    where
        F: std::future::Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        self.scheduler
            .upgrade()
            .expect("runtime spawner outlived runtime")
            .borrow_mut()
            .spawn_into(scope, future, complete)
    }

    /// Submits work to Tokio's blocking facility. A cancellation after work
    /// begins is represented as an uninterested/discarded UI completion.
    pub fn spawn_blocking<T>(
        &self,
        work: impl FnOnce() -> T + Send + 'static,
        complete: impl FnOnce(Result<T, TaskFailure>, &mut Runtime) + 'static,
    ) -> TaskHandle
    where
        T: Send + 'static,
    {
        self.spawn_blocking_in(&self.application_scope, work, complete)
    }

    pub fn spawn_blocking_in<T>(
        &self,
        scope: &TaskScope,
        work: impl FnOnce() -> T + Send + 'static,
        complete: impl FnOnce(Result<T, TaskFailure>, &mut Runtime) + 'static,
    ) -> TaskHandle
    where
        T: Send + 'static,
    {
        self.scheduler
            .upgrade()
            .expect("runtime spawner outlived runtime")
            .borrow_mut()
            .spawn_blocking_in(scope, work, complete)
    }

    #[must_use]
    pub fn scope(&self) -> TaskScope {
        self.application_scope.child()
    }

    #[must_use]
    pub fn tokio_handle(&self) -> TokioHandle {
        self.scheduler
            .upgrade()
            .expect("runtime spawner outlived runtime")
            .borrow()
            .tokio_handle()
    }

    #[must_use]
    pub fn dispatcher(&self) -> UiDispatcher {
        self.scheduler
            .upgrade()
            .expect("runtime spawner outlived runtime")
            .borrow()
            .dispatcher_for(self.window_id)
    }
}

/// Cross-thread dispatcher into the unique UI owner. It is a message sender,
/// not access to `Runtime`, and does not imply a redraw.
#[derive(Clone)]
pub struct UiDispatcher {
    bridge: Arc<RuntimeBridge>,
    window_id: Option<WindowId>,
}

impl UiDispatcher {
    pub fn dispatch(&self, callback: impl FnOnce(&mut Runtime) + Send + 'static) {
        self.bridge.enqueue(RuntimeMessage::Dispatch {
            window_id: self.window_id,
            callback: Box::new(callback),
        });
    }
}

pub(crate) type UiCallback = Box<dyn FnOnce(&mut Runtime) + Send>;
pub(crate) type TaskFinish = Box<dyn FnOnce(&mut Runtime) -> Option<TaskFailure>>;
pub(crate) type TaskDiscard = Box<dyn FnOnce()>;

enum RuntimeMessage {
    TaskCompleted(u64),
    Dispatch {
        window_id: Option<WindowId>,
        callback: UiCallback,
    },
}

struct RuntimeBridge {
    sender: mpsc::Sender<RuntimeMessage>,
    wake: Mutex<Option<Arc<dyn RuntimeWake>>>,
    accepting: AtomicBool,
    wake_pending: AtomicBool,
    queued: AtomicUsize,
    queue_peak: AtomicUsize,
    messages_enqueued: AtomicU64,
    wakes: AtomicU64,
    coalesced_wakes: AtomicU64,
}

impl RuntimeBridge {
    fn enqueue(&self, message: RuntimeMessage) {
        if !self.accepting.load(Ordering::Acquire) {
            return;
        }
        if self.sender.send(message).is_err() {
            return;
        }
        self.messages_enqueued.fetch_add(1, Ordering::Relaxed);
        let queued = self.queued.fetch_add(1, Ordering::AcqRel) + 1;
        self.queue_peak.fetch_max(queued, Ordering::Relaxed);
        self.request_wake();
    }

    fn request_wake(&self) {
        if self.wake_pending.swap(true, Ordering::AcqRel) {
            self.coalesced_wakes.fetch_add(1, Ordering::Relaxed);
            return;
        }
        self.wakes.fetch_add(1, Ordering::Relaxed);
        if let Some(wake) = self.wake.lock().expect("runtime wake mutex").as_ref() {
            wake.wake();
        }
    }

    fn set_wake(&self, wake: Arc<dyn RuntimeWake>) {
        *self.wake.lock().expect("runtime wake mutex") = Some(wake.clone());
        if self.queued.load(Ordering::Acquire) > 0 {
            self.wakes.fetch_add(1, Ordering::Relaxed);
            wake.wake();
        }
    }
}

pub(crate) struct PendingTask {
    pub(crate) control: Arc<TaskControl>,
    pub(crate) finish: TaskFinish,
    pub(crate) discard: TaskDiscard,
    pub(crate) blocking: bool,
}

/// Diagnostics of Incular/Tokio integration, not Tokio's internal executor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RuntimeDiagnostics {
    pub app_scoped_tasks_spawned: u64,
    pub owner_scoped_tasks_spawned: u64,
    pub active_tracked_tasks: usize,
    pub tasks_completed: u64,
    pub tasks_cancelled: u64,
    pub task_join_failures: u64,
    pub task_panics: u64,
    pub blocking_jobs_spawned: u64,
    pub blocking_jobs_completed: u64,
    pub blocking_results_discarded: u64,
    pub ui_messages_enqueued: u64,
    pub ui_messages_processed: u64,
    pub ui_message_queue_peak: usize,
    pub winit_runtime_wakes: u64,
    pub coalesced_wakes: u64,
    pub stale_completion_rejections: u64,
}

pub(crate) enum UiWork {
    Dispatch {
        window_id: Option<WindowId>,
        callback: UiCallback,
    },
    Task(PendingTask),
}

/// Owns a Tokio runtime and a UI-thread message receiver. It is not an
/// executor implementation: it starts Tokio tasks and drains their owned UI
/// completion messages fairly.
pub(crate) struct TaskScheduler {
    bridge: Arc<RuntimeBridge>,
    receiver: mpsc::Receiver<RuntimeMessage>,
    pending: HashMap<u64, PendingTask>,
    tokio: Option<TokioRuntime>,
    handle: TokioHandle,
    application_scope: TaskScope,
    next_task: u64,
    diagnostics: RuntimeDiagnostics,
}

impl TaskScheduler {
    pub(crate) fn new() -> Rc<std::cell::RefCell<Self>> {
        let tokio = Builder::new_multi_thread()
            .enable_time()
            .enable_io()
            .build()
            .expect("Tokio runtime construction");
        let handle = tokio.handle().clone();
        let (sender, receiver) = mpsc::channel();
        let bridge = Arc::new(RuntimeBridge {
            sender,
            wake: Mutex::new(None),
            accepting: AtomicBool::new(true),
            wake_pending: AtomicBool::new(false),
            queued: AtomicUsize::new(0),
            queue_peak: AtomicUsize::new(0),
            messages_enqueued: AtomicU64::new(0),
            wakes: AtomicU64::new(0),
            coalesced_wakes: AtomicU64::new(0),
        });
        Rc::new(std::cell::RefCell::new(Self {
            bridge,
            receiver,
            pending: HashMap::new(),
            tokio: Some(tokio),
            handle,
            application_scope: TaskScope::new(),
            next_task: 1,
            diagnostics: RuntimeDiagnostics::default(),
        }))
    }

    pub(crate) fn spawner(scheduler: &Rc<std::cell::RefCell<Self>>) -> RuntimeSpawner {
        let borrowed = scheduler.borrow();
        RuntimeSpawner {
            scheduler: Rc::downgrade(scheduler),
            application_scope: borrowed.application_scope.clone(),
            window_id: None,
        }
    }

    pub(crate) fn spawner_for(
        scheduler: &Rc<std::cell::RefCell<Self>>,
        window_id: WindowId,
    ) -> RuntimeSpawner {
        let mut spawner = Self::spawner(scheduler);
        spawner.window_id = Some(window_id);
        spawner
    }

    pub(crate) fn tokio_handle(&self) -> TokioHandle {
        self.handle.clone()
    }

    fn next_control(&mut self, scope: &TaskScope) -> Arc<TaskControl> {
        let id = self.next_task;
        self.next_task = self.next_task.wrapping_add(1);
        let control = Arc::new(TaskControl {
            id,
            cancelled: AtomicBool::new(
                scope.is_cancelled() || !self.bridge.accepting.load(Ordering::Acquire),
            ),
            scope: Arc::downgrade(&scope.state),
            abort: Mutex::new(None),
        });
        scope.attach(&control);
        // The declarative root is bound to its Element after its initial
        // builder returns, so scope identity—not only an already-bound ID—is
        // the stable classification during application construction.
        if !Arc::ptr_eq(&scope.state, &self.application_scope.state) {
            self.diagnostics.owner_scoped_tasks_spawned += 1;
        } else {
            self.diagnostics.app_scoped_tasks_spawned += 1;
        }
        control
    }

    fn spawn_task<F, T>(&mut self, scope: &TaskScope, future: F) -> Task<T>
    where
        F: std::future::Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        let state = Arc::new(Mutex::new(AsyncValue::Loading));
        let state_for_finish = state.clone();
        let state_for_discard = state.clone();
        let handle = self.spawn_impl(
            scope,
            future,
            move |outcome, _| {
                let failure = outcome.as_ref().err().cloned();
                *state_for_finish.lock().expect("task state mutex") = match outcome {
                    Ok(value) => AsyncValue::Ready(value),
                    Err(failure) => AsyncValue::Error(failure),
                };
                failure
            },
            move || {
                *state_for_discard.lock().expect("task state mutex") =
                    AsyncValue::Error(TaskFailure::Cancelled);
            },
        );
        Task { state, handle }
    }

    fn spawn_into<F, T>(
        &mut self,
        scope: &TaskScope,
        future: F,
        complete: impl FnOnce(Result<T, TaskFailure>, &mut Runtime) + 'static,
    ) -> TaskHandle
    where
        F: std::future::Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        self.spawn_impl(
            scope,
            future,
            move |outcome, runtime| {
                let failure = outcome.as_ref().err().cloned();
                let callback = panic::catch_unwind(AssertUnwindSafe(|| complete(outcome, runtime)));
                if callback.is_err() {
                    Some(TaskFailure::Panicked)
                } else {
                    failure
                }
            },
            || {},
        )
    }

    pub(crate) fn spawn_blocking<T>(
        &mut self,
        work: impl FnOnce() -> T + Send + 'static,
        complete: impl FnOnce(Result<T, TaskFailure>, &mut Runtime) + 'static,
    ) -> TaskHandle
    where
        T: Send + 'static,
    {
        let scope = self.application_scope.clone();
        self.spawn_blocking_in(&scope, work, complete)
    }

    fn spawn_blocking_in<T>(
        &mut self,
        scope: &TaskScope,
        work: impl FnOnce() -> T + Send + 'static,
        complete: impl FnOnce(Result<T, TaskFailure>, &mut Runtime) + 'static,
    ) -> TaskHandle
    where
        T: Send + 'static,
    {
        self.diagnostics.blocking_jobs_spawned += 1;
        let join = self.handle.spawn_blocking(work);
        self.register_join(
            scope,
            true,
            join,
            move |outcome, runtime| {
                let failure = outcome.as_ref().err().cloned();
                let callback = panic::catch_unwind(AssertUnwindSafe(|| complete(outcome, runtime)));
                if callback.is_err() {
                    Some(TaskFailure::Panicked)
                } else {
                    failure
                }
            },
            || {},
        )
    }

    fn spawn_impl<F, T>(
        &mut self,
        scope: &TaskScope,
        future: F,
        finish: impl FnOnce(Result<T, TaskFailure>, &mut Runtime) -> Option<TaskFailure> + 'static,
        discard: impl FnOnce() + 'static,
    ) -> TaskHandle
    where
        F: std::future::Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        let join = self.handle.spawn(future);
        self.register_join(scope, false, join, finish, discard)
    }

    fn register_join<T>(
        &mut self,
        scope: &TaskScope,
        blocking: bool,
        join: JoinHandle<T>,
        finish: impl FnOnce(Result<T, TaskFailure>, &mut Runtime) -> Option<TaskFailure> + 'static,
        discard: impl FnOnce() + 'static,
    ) -> TaskHandle
    where
        T: Send + 'static,
    {
        let control = self.next_control(scope);
        let task_handle = TaskHandle {
            control: control.clone(),
        };
        if control.is_cancelled() {
            join.abort();
            discard();
            self.diagnostics.tasks_cancelled += u64::from(!blocking);
            self.diagnostics.blocking_results_discarded += u64::from(blocking);
            scope.detach(control.id);
            return task_handle;
        }

        let result = Arc::new(Mutex::new(None));
        let result_for_completion = result.clone();
        let complete_id = control.id;
        let bridge = self.bridge.clone();
        *control.abort.lock().expect("task abort mutex") = Some(join.abort_handle());
        self.handle.spawn(async move {
            let outcome = match join.await {
                Ok(value) => Ok(value),
                Err(error) if error.is_cancelled() => Err(TaskFailure::Cancelled),
                Err(error) if error.is_panic() => Err(TaskFailure::Panicked),
                Err(_) => Err(TaskFailure::RuntimeStopped),
            };
            *result_for_completion.lock().expect("task result mutex") = Some(outcome);
            bridge.enqueue(RuntimeMessage::TaskCompleted(complete_id));
        });
        self.pending.insert(
            control.id,
            PendingTask {
                control,
                finish: Box::new(move |runtime| {
                    let outcome = result
                        .lock()
                        .expect("task result mutex")
                        .take()
                        .unwrap_or(Err(TaskFailure::RuntimeStopped));
                    finish(outcome, runtime)
                }),
                discard: Box::new(discard),
                blocking,
            },
        );
        task_handle
    }

    pub(crate) fn take_turn(&mut self) -> Vec<UiWork> {
        // Clearing before the drain prevents a producer racing with this turn
        // from being lost: it will enqueue one new native user event.
        self.bridge.wake_pending.store(false, Ordering::Release);
        let mut work = Vec::new();
        let mut processed = 0;
        while processed < DEFAULT_UI_MESSAGE_BUDGET {
            let message = match self.receiver.try_recv() {
                Ok(message) => message,
                Err(mpsc::TryRecvError::Empty | mpsc::TryRecvError::Disconnected) => break,
            };
            processed += 1;
            self.bridge.queued.fetch_sub(1, Ordering::AcqRel);
            self.diagnostics.ui_messages_processed += 1;
            match message {
                RuntimeMessage::Dispatch {
                    window_id,
                    callback,
                } => work.push(UiWork::Dispatch {
                    window_id,
                    callback,
                }),
                RuntimeMessage::TaskCompleted(id) => {
                    if let Some(pending) = self.pending.remove(&id) {
                        work.push(UiWork::Task(pending));
                    }
                }
            }
        }
        if processed == DEFAULT_UI_MESSAGE_BUDGET && self.bridge.queued.load(Ordering::Acquire) > 0
        {
            self.bridge.request_wake();
        }
        work
    }

    pub(crate) fn accepting(&self) -> bool {
        self.bridge.accepting.load(Ordering::Acquire)
    }

    pub(crate) fn detach_scope(&self, control: &TaskControl) {
        if let Some(scope) = control.scope.upgrade() {
            TaskScope { state: scope }.detach(control.id);
        }
    }

    pub(crate) fn record_discard(&mut self, blocking: bool, stale_owner: bool) {
        if blocking {
            self.diagnostics.blocking_results_discarded += 1;
        } else {
            self.diagnostics.tasks_cancelled += 1;
        }
        if stale_owner {
            self.diagnostics.stale_completion_rejections += 1;
        }
    }

    pub(crate) fn record_completion(
        &mut self,
        blocking: bool,
        failure: Option<TaskFailure>,
    ) -> Option<TaskFailure> {
        if blocking {
            self.diagnostics.blocking_jobs_completed += 1;
        } else {
            self.diagnostics.tasks_completed += 1;
        }
        match failure {
            Some(TaskFailure::Cancelled) => {
                self.diagnostics.tasks_cancelled += 1;
                None
            }
            Some(TaskFailure::Panicked) => {
                self.diagnostics.task_join_failures += 1;
                self.diagnostics.task_panics += 1;
                Some(TaskFailure::Panicked)
            }
            Some(TaskFailure::RuntimeStopped) => {
                self.diagnostics.task_join_failures += 1;
                None
            }
            None => None,
        }
    }

    pub(crate) fn dispatcher_for(&self, window_id: Option<WindowId>) -> UiDispatcher {
        UiDispatcher {
            bridge: self.bridge.clone(),
            window_id,
        }
    }

    pub(crate) fn set_wake(&self, wake: Arc<dyn RuntimeWake>) {
        self.bridge.set_wake(wake);
    }

    pub(crate) fn shutdown(&mut self) {
        if !self.bridge.accepting.swap(false, Ordering::AcqRel) {
            return;
        }
        self.application_scope.cancel();
        let pending = std::mem::take(&mut self.pending);
        for (_, task) in pending {
            task.control.cancel();
            (task.discard)();
            if task.blocking {
                self.diagnostics.blocking_results_discarded += 1;
            } else {
                self.diagnostics.tasks_cancelled += 1;
            }
        }
        while self.receiver.try_recv().is_ok() {
            self.bridge.queued.fetch_sub(1, Ordering::AcqRel);
        }
        if let Some(tokio) = self.tokio.take() {
            // Tokio does not force-stop an already-running blocking closure.
            // The bounded wait keeps UI shutdown from hanging indefinitely.
            tokio.shutdown_timeout(SHUTDOWN_TIMEOUT);
        }
    }

    pub(crate) fn diagnostics(&self) -> RuntimeDiagnostics {
        let mut diagnostics = self.diagnostics;
        diagnostics.active_tracked_tasks = self.pending.len();
        diagnostics.ui_messages_enqueued = self.bridge.messages_enqueued.load(Ordering::Relaxed);
        diagnostics.ui_message_queue_peak = self.bridge.queue_peak.load(Ordering::Relaxed);
        diagnostics.winit_runtime_wakes = self.bridge.wakes.load(Ordering::Relaxed);
        diagnostics.coalesced_wakes = self.bridge.coalesced_wakes.load(Ordering::Relaxed);
        diagnostics
    }

    pub(crate) fn active_ownership(&self) -> (usize, usize) {
        self.pending.values().fold((0, 0), |(app, owner), task| {
            let application_owned = task
                .control
                .scope
                .upgrade()
                .is_some_and(|scope| Arc::ptr_eq(&scope, &self.application_scope.state));
            if application_owned {
                (app + 1, owner)
            } else {
                (app, owner + 1)
            }
        })
    }
}

impl Drop for TaskScheduler {
    fn drop(&mut self) {
        // A headless/runtime owner that is simply dropped gets the same bounded
        // Tokio shutdown policy as an explicit native-window close.
        self.shutdown();
    }
}
