//! Controlled BUILD → LAYOUT → PAINT coordination and local reactive state.

mod application;
mod application_activations;
mod application_shell;
mod application_types;
mod context;
mod environment;
mod file_dialogs;
mod frame;
mod global_shortcuts;
mod profiling;
mod reactive;
mod request_admission;
mod request_channel;
mod request_registry;
mod restoration;
mod simulation;
mod tasks;
mod transient_presentation;
mod undo;
mod window_commands;
mod window_state;

#[allow(unused_imports)]
use incular_accessibility::{
    AccessKitProjection, NativeAccessibilityUpdate, SemanticActionRequest,
};
#[allow(unused_imports)]
use incular_config::{Constraints, ContentSensitivity, RuntimeEnvironment};
#[allow(unused_imports)]
use incular_core::{
    Code, ImeEvent, InputEvent, KeyboardEvent, Modifiers, Offset, PointerPhase, Rect,
    RestorationKey, RestorationScope,
};
#[allow(unused_imports)]
use incular_platform::{
    Clipboard, MemoryClipboard, PlatformEvent, PlatformLifecycle, TextInputAction,
    TextInputClientId, TextInputCommand, TextInputConfiguration, TextInputState, TextInputType,
    WindowCommand, WindowEvent, WindowEventKind, WindowId, WindowLifecycle, WindowMetrics,
    WindowOperation, WindowOptions, WindowOptionsError,
};
#[allow(unused_imports)]
use incular_rendering::DisplayList;
#[allow(unused_imports)]
use incular_semantics::{SemanticAction, SemanticNodeId};
#[cfg(feature = "devtools")]
#[allow(unused_imports)]
use incular_widgets::internal::InvalidationCause;
#[allow(unused_imports)]
use incular_widgets::internal::{
    ActionId, Diagnostics, ElementId, Key, PointerEvent, TextRange, TextSelection, TreeError,
    WidgetTree,
};
#[allow(unused_imports)]
use incular_widgets::{
    FocusScopeNode, FocusScopeSubscription, TextInputActionHint, TextInputTypeHint, Widget,
};
#[cfg(feature = "devtools")]
#[allow(unused_imports)]
use std::any::{Any, TypeId};
#[allow(unused_imports)]
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

pub use application::{Application, PERFORMANCE_OVERLAY_KEY};
pub use application_activations::{
    ActivationRouteBridge, ApplicationActivationListener, ApplicationActivationService,
    ApplicationActivationSubscription,
};
pub use application_shell::{
    ApplicationShellRequestId, ApplicationShellService, MemoryApplicationShellAdapter,
    NativeApplicationShellApplyResult, NativeApplicationShellCompletion,
    NativeApplicationShellEvent, NativeApplicationShellOperation, NativeApplicationShellRequest,
    NotificationHandle, TrayItemHandle,
};
pub use application_types::{
    ApplicationLifecycle, LastWindowPolicy, LifecycleTransition, RestorableWindowFactory,
    RuntimeErrorReport, WindowError, WindowRestorationId,
};
pub use context::BuildContext;
#[cfg(feature = "devtools")]
pub use environment::devtools_registry;
pub use environment::{EditableSignalKind, Signal, SignalRegistration};
pub use file_dialogs::{
    FileDialogCompletionStatus, FileDialogRequest, FileDialogService, MemoryFileDialogAdapter,
    NativeFileDialogCompletion, NativeFileDialogRequest,
};
pub use frame::{EditingDiagnostics, EventTarget, FocusDiagnostics, FrameStats, Runtime};
pub use global_shortcuts::{
    GlobalShortcutCompletionStatus, GlobalShortcutRegistration, GlobalShortcutRegistrationRequest,
    GlobalShortcutRequestId, GlobalShortcutService, GlobalShortcutUnregistrationRequest,
    MemoryGlobalShortcutAdapter, NativeGlobalShortcutCompletion, NativeGlobalShortcutOperation,
    NativeGlobalShortcutRequest,
};
pub use incular_accessibility::AccessibilityDiagnostics;
pub use profiling::{
    AccessibilitySnapshot, BudgetStatistics, FrameHistory, FrameRecord, FrameStatistics,
    FrameTimings, FrameWork, GpuResourceSummary, GpuSample, PerformanceHub, PerformanceProfiler,
    PerformanceSnapshot, ProfilerMode, RenderFrameMetrics, SchedulerCounters, TextCacheSnapshot,
    WidgetWorkSnapshot, WindowPerformance,
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
pub use transient_presentation::{
    ResolvedTransientPresentation, TransientFallbackReason, TransientPresentationResolution,
};
pub use undo::{UndoHistoryController, UndoHistoryState};
pub use window_commands::{
    NativeOperationCompletionStatus, NativeOperationRequest, NativeWindowCommand,
    WindowCommandEnqueueError, WindowHandle, WindowOpener, WindowPlacementError,
};
pub use window_state::{ApplicationDiagnostics, WindowDiagnostics};

// Compatibility imports for the existing implementation submodules. These
// remain private to the facade while preserving their original super:: paths.
pub(crate) use environment::{
    BUILD_SCOPE, BuildScope, Dependency, NEXT_REACTIVE_NODE, ReactiveNode, ReactiveNodeId,
    ReactiveQueue, ReactiveRootId, TrackedDependency,
};
pub(crate) use frame::diagnostic_input_trigger;

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
