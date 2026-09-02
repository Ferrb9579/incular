use crate::TransientPresentationResolution;
use crate::application_types::ApplicationLifecycle;
use crate::application_types::{RestorableWindowMetadata, WindowError, WindowRestorationId};
use crate::context::BuildContext;
use crate::environment::{BuildScope, BuildScopeGuard, InitialBuildDependencies, ReactiveQueue};
use crate::frame::Runtime;
use crate::profiling::{FrameRecord, GpuSample, RenderFrameMetrics};
use crate::restoration;
use crate::tasks::{self, TaskScope};
use crate::window_commands::{
    DisplayCatalog, NativeWindowCommand, WindowCommandBridge, WindowHandle,
};
use incular_accessibility::AccessibilityDiagnostics;
use incular_config::{Constraints, ContentSensitivity, RuntimeEnvironment, WindowSizePolicy};
use incular_core::{RestorationKey, RestorationScope, Size};
use incular_platform::{
    PlatformCapabilities, PlatformOperationError, WindowId, WindowLifecycle, WindowMetrics,
    WindowObservedState, WindowOptions, WindowRequestedState,
};
use incular_widgets::Widget;
use std::{
    cell::{Cell, RefCell},
    collections::{HashSet, VecDeque},
    rc::{Rc, Weak},
    sync::{Arc, RwLock},
};

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
    /// Effective retained-tree capture policy at the last diagnostics read.
    pub content_sensitivity: ContentSensitivity,
    pub elements: usize,
    pub render_objects: usize,
    pub semantics: usize,
    /// Adapter health for this window only. It contains no native IDs or text
    /// values, and remains zero when no desktop accessibility adapter exists.
    pub accessibility: AccessibilityDiagnostics,
    /// Native backend capabilities currently published for this window.
    pub capabilities: PlatformCapabilities,
    /// Most recent native operation failure, if any. The stable error category
    /// is suitable for diagnostics; no native error object is retained.
    pub last_platform_error: Option<PlatformOperationError>,
    /// Last application-requested mutable native state.
    pub requested_state: WindowRequestedState,
    /// State most recently observed from the native backend. Unknown fields are
    /// `None` rather than copied from requested state.
    pub observed_state: WindowObservedState,
    /// Resolved native-vs-overlay presentation for every currently visible
    /// semantic transient in this window.
    pub transient_presentations: Vec<TransientPresentationResolution>,
}

/// Aggregate lifecycle and stale-command diagnostics for an application.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ApplicationDiagnostics {
    pub windows_created: u64,
    pub windows_closed: u64,
    pub active_windows: usize,
    pub stale_window_commands: u64,
    pub pending_native_requests: usize,
}

pub(crate) struct WindowRecord {
    pub(crate) runtime: Runtime,
    pub(crate) scope: TaskScope,
    pub(crate) last_frame: FrameRecord,
    pub(crate) render_metrics: RenderFrameMetrics,
    pub(crate) gpu_sample: Option<GpuSample>,
    pub(crate) options: WindowOptions,
    pub(crate) lifecycle: WindowLifecycle,
    pub(crate) metrics: WindowMetrics,
    pub(crate) native_focused: bool,
    pub(crate) requested_frames: u64,
    pub(crate) presented_frames: u64,
    pub(crate) skipped_frames: u64,
    pub(crate) input_events: u64,
    pub(crate) surface_generation: u64,
    pub(crate) accessibility: AccessibilityDiagnostics,
    pub(crate) last_content_sensitivity: Option<ContentSensitivity>,
    pub(crate) content_sizing: ContentSizeCoordinator,
    pub(crate) restoration: Option<RestorableWindowMetadata>,
    pub(crate) _restoration_scope_lease: Option<restoration::ScopeLease>,
    pub(crate) capabilities: Arc<RwLock<PlatformCapabilities>>,
    pub(crate) last_platform_error: Option<PlatformOperationError>,
    pub(crate) requested_state: WindowRequestedState,
    pub(crate) observed_state: Arc<RwLock<WindowObservedState>>,
    pub(crate) transient_presentations: Arc<RwLock<Vec<TransientPresentationResolution>>>,
}

/// Window-local negotiation state for content-driven native sizing.
///
/// The retained root layout produces a logical target, while the native host
/// remains asynchronous. Paint-only overflow (including transient overlays,
/// shadows, and filters) is deliberately excluded. Remembering the outstanding target prevents redraws from
/// flooding the event loop with identical resize requests before metrics catch
/// up.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct ContentSizeCoordinator {
    pending: Option<Size>,
}

impl ContentSizeCoordinator {
    pub(crate) fn layout_constraints(
        options: &WindowOptions,
        viewport_constraints: Constraints,
    ) -> Constraints {
        if options.size_policy != WindowSizePolicy::Content {
            return viewport_constraints;
        }
        let minimum = content_minimum(options);
        let (maximum_width, maximum_height) = options
            .maximum_logical_size
            .map_or((f32::INFINITY, f32::INFINITY), |maximum| {
                (maximum.width, maximum.height)
            });
        Constraints::new(
            minimum.width,
            maximum_width.max(minimum.width),
            minimum.height,
            maximum_height.max(minimum.height),
        )
    }

    pub(crate) fn reconcile(
        &mut self,
        options: &WindowOptions,
        metrics: WindowMetrics,
        root_layout_size: Option<Size>,
    ) -> Option<Size> {
        if options.size_policy != WindowSizePolicy::Content {
            self.pending = None;
            return None;
        }

        let minimum = content_minimum(options);
        let extent = root_layout_size.unwrap_or(Size::ZERO);
        let mut target = Size::new(
            minimum.width.max(extent.width),
            minimum.height.max(extent.height),
        );
        if let Some(maximum) = options.maximum_logical_size {
            target.width = target.width.min(maximum.width);
            target.height = target.height.min(maximum.height);
        }
        target = snap_outward_to_physical_pixels(target, metrics.scale_factor);
        if let Some(maximum) = options.maximum_logical_size {
            target.width = target.width.min(maximum.width);
            target.height = target.height.min(maximum.height);
        }

        let current = metrics.logical_size();
        if equivalent_logical_size(target, current, metrics.scale_factor) {
            self.pending = None;
            return None;
        }
        if self
            .pending
            .is_some_and(|pending| equivalent_logical_size(pending, target, metrics.scale_factor))
        {
            return None;
        }
        self.pending = Some(target);
        Some(target)
    }

    pub(crate) fn reset(&mut self) {
        self.pending = None;
    }
}

fn content_minimum(options: &WindowOptions) -> Size {
    let declared = options.minimum_logical_size.unwrap_or(Size::ZERO);
    Size::new(
        options.initial_logical_size.width.max(declared.width),
        options.initial_logical_size.height.max(declared.height),
    )
}

fn snap_outward_to_physical_pixels(size: Size, scale_factor: f64) -> Size {
    let scale = if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor as f32
    } else {
        1.0
    };
    Size::new(
        (size.width * scale).ceil() / scale,
        (size.height * scale).ceil() / scale,
    )
}

fn equivalent_logical_size(left: Size, right: Size, scale_factor: f64) -> bool {
    let scale = if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor as f32
    } else {
        1.0
    };
    let tolerance = 0.5 / scale + f32::EPSILON;
    (left.width - right.width).abs() <= tolerance && (left.height - right.height).abs() <= tolerance
}

pub(crate) struct WindowSlot {
    pub(crate) generation: u32,
    pub(crate) reserved: bool,
    pub(crate) record: Option<WindowRecord>,
}

#[derive(Default)]
pub(crate) struct WindowRegistry {
    pub(crate) slots: Vec<WindowSlot>,
    pub(crate) windows_created: u64,
    pub(crate) windows_closed: u64,
    pub(crate) stale_window_commands: u64,
}

impl WindowRegistry {
    pub(crate) fn reserve(&mut self) -> WindowId {
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

    pub(crate) fn insert(&mut self, id: WindowId, record: WindowRecord) {
        let slot = &mut self.slots[id.index() as usize];
        debug_assert!(slot.reserved && slot.generation == id.generation());
        slot.record = Some(record);
        self.windows_created += 1;
    }

    pub(crate) fn take(&mut self, id: WindowId) -> Option<WindowRecord> {
        self.slots
            .get_mut(id.index() as usize)
            .filter(|slot| slot.reserved && slot.generation == id.generation())?
            .record
            .take()
    }

    pub(crate) fn restore(&mut self, id: WindowId, record: WindowRecord) {
        let slot = &mut self.slots[id.index() as usize];
        debug_assert!(slot.reserved && slot.generation == id.generation());
        debug_assert!(slot.record.is_none());
        slot.record = Some(record);
    }

    pub(crate) fn close(&mut self, id: WindowId) -> Option<WindowRecord> {
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

    pub(crate) fn ids(&self) -> Vec<WindowId> {
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

    pub(crate) fn contains(&self, id: WindowId) -> bool {
        self.slots.get(id.index() as usize).is_some_and(|slot| {
            slot.reserved && slot.generation == id.generation() && slot.record.is_some()
        })
    }

    #[cfg_attr(not(feature = "devtools"), allow(dead_code))]
    pub(crate) fn get(&self, id: WindowId) -> Option<&WindowRecord> {
        self.slots
            .get(id.index() as usize)
            .filter(|slot| slot.reserved && slot.generation == id.generation())?
            .record
            .as_ref()
    }

    pub(crate) fn visible_count(&self) -> usize {
        self.slots
            .iter()
            .filter_map(|slot| slot.record.as_ref())
            .filter(|record| record.options.visible)
            .count()
    }
}

#[derive(Clone)]
pub(crate) struct WindowManager {
    pub(crate) registry: Weak<RefCell<WindowRegistry>>,
    pub(crate) scheduler: Rc<RefCell<tasks::TaskScheduler>>,
    pub(crate) bridge: Arc<WindowCommandBridge>,
    pub(crate) native_commands: Rc<RefCell<VecDeque<NativeWindowCommand>>>,
    pub(crate) restoration: Option<restoration::RestorationManager>,
    pub(crate) application_capabilities: Arc<RwLock<PlatformCapabilities>>,
    pub(crate) displays: Arc<RwLock<DisplayCatalog>>,
}

impl WindowManager {
    pub(crate) fn open_window(
        &self,
        options: WindowOptions,
        root: Widget,
    ) -> Result<WindowHandle, WindowError> {
        self.open_window_inner(options, root, None)
    }

    pub(crate) fn open_window_inner(
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
        let capabilities = Arc::new(RwLock::new(
            *self
                .application_capabilities
                .read()
                .expect("application capability snapshot lock"),
        ));
        let observed_state = Arc::new(RwLock::new(WindowObservedState::default()));
        let transient_presentations = Arc::new(RwLock::new(Vec::new()));
        let scope = tasks::TaskScheduler::spawner(&self.scheduler).scope();
        scope.bind_window(id);
        let metrics = initial_metrics(&options);
        let requested_state = options
            .requested_state()
            .expect("WindowOptions were validated before requested-state construction");
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
                last_content_sensitivity: None,
                content_sizing: ContentSizeCoordinator::default(),
                restoration,
                _restoration_scope_lease: restoration_lease,
                capabilities: capabilities.clone(),
                last_platform_error: None,
                requested_state,
                observed_state: observed_state.clone(),
                transient_presentations: transient_presentations.clone(),
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
            capabilities,
            observed_state,
            displays: self.displays.clone(),
            transient_presentations,
        })
    }

    pub(crate) fn open_window_with(
        &self,
        options: WindowOptions,
        build: impl FnMut(&mut BuildContext) -> Widget + 'static,
    ) -> Result<WindowHandle, WindowError> {
        self.open_window_with_inner(options, build, None)
    }

    pub(crate) fn open_restorable_window_with(
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

    pub(crate) fn open_window_with_inner(
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
        let capabilities = Arc::new(RwLock::new(
            *self
                .application_capabilities
                .read()
                .expect("application capability snapshot lock"),
        ));
        let observed_state = Arc::new(RwLock::new(WindowObservedState::default()));
        let transient_presentations = Arc::new(RwLock::new(Vec::new()));
        let spawner = tasks::TaskScheduler::spawner_for(&self.scheduler, id);
        let window_scope = spawner.scope();
        window_scope.bind_window(id);
        let root_scope = window_scope.child();
        let metrics = initial_metrics(&options);
        let requested_state = options
            .requested_state()
            .expect("WindowOptions were validated before requested-state construction");
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
            let _build_scope = BuildScopeGuard::enter(BuildScope {
                root: reactive.borrow().root,
                element: None,
                queue: Rc::downgrade(&reactive),
                initial_dependencies: Some(initial_dependencies.clone()),
                spawner: spawner.clone(),
                owner_scope: root_scope.clone(),
            });
            (build.borrow_mut())(&mut build_context)
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
                last_content_sensitivity: None,
                content_sizing: ContentSizeCoordinator::default(),
                restoration,
                _restoration_scope_lease: restoration_lease,
                capabilities: capabilities.clone(),
                last_platform_error: None,
                requested_state,
                observed_state: observed_state.clone(),
                transient_presentations: transient_presentations.clone(),
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
            capabilities,
            observed_state,
            displays: self.displays.clone(),
            transient_presentations,
        })
    }

    pub(crate) fn handle(&self, id: WindowId) -> Option<WindowHandle> {
        let registry = self.registry.upgrade()?;
        let (capabilities, observed_state, transient_presentations) = {
            let registry = registry.borrow();
            let record = registry.get(id)?;
            (
                record.capabilities.clone(),
                record.observed_state.clone(),
                record.transient_presentations.clone(),
            )
        };
        Some(WindowHandle {
            id,
            bridge: self.bridge.clone(),
            capabilities,
            observed_state,
            displays: self.displays.clone(),
            transient_presentations,
        })
    }

    pub(crate) fn send_window_operation(
        &self,
        id: WindowId,
        operation: incular_platform::WindowOperation,
    ) -> bool {
        self.bridge
            .send(incular_platform::WindowCommand::new(id, operation))
            .is_ok()
    }

    pub(crate) fn restoration_scope(
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

    pub(crate) fn acquire_restoration_scope(
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

    pub(crate) fn apply_restored_window_options(
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

    pub(crate) fn sync_restorable_windows(&self, retain_unopened: bool) {
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
                // Content-driven expansion is derived presentation state.
                // Persist the stable application baseline instead; transient
                // overlays are a separate surface concern and never contribute
                // to top-level content sizing. Explicit SetLogicalSize updates
                // the baseline before synchronization reaches this point.
                let logical = if record.options.size_policy == WindowSizePolicy::Content {
                    record.options.initial_logical_size
                } else {
                    record.metrics.logical_size()
                };
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

pub(crate) fn initial_metrics(options: &WindowOptions) -> WindowMetrics {
    WindowMetrics::new(
        incular_platform::PhysicalSize::new(
            options.initial_logical_size.width.ceil() as u32,
            options.initial_logical_size.height.ceil() as u32,
        ),
        incular_config::ApplicationDefaults::DEFAULT.scale_factor,
    )
}
