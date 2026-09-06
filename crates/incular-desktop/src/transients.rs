//! Native transient presentation hosts for the shared desktop event loop.

use crate::DesktopPlatformServices;
use crate::input::WindowInputState;
use crate::pointer::{NativeCursorCoordinator, PointerDeviceRegistry};
use crate::transient_input::offset_transient_platform_event;
use crate::window_host::NativeWindowState;
use incular_config::{TransientPresentation, TransientRole};
use incular_core::{Color, Offset};
use incular_platform::{
    NativeWindowSystem, PhysicalSize, PlatformEvent, TransparencyMode,
    WindowEvent as IncularWindowEvent, WindowId as IncularWindowId, WindowMetrics,
};
use incular_rendering::DisplayList;
use incular_runtime::{
    Application, ResolvedTransientPresentation, TransientFallbackReason,
    TransientPresentationResolution,
};
use incular_wgpu::{RendererError, SharedGpuContext, WgpuRenderer};
use incular_widgets::{TransientSurfaceId, TransientSurfaceSnapshot};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use winit::{
    dpi::PhysicalPosition,
    event::WindowEvent,
    event_loop::ActiveEventLoop,
    window::{Window, WindowAttributes, WindowId as NativeWindowId},
};

pub(crate) struct TransientContext<'a> {
    pub(crate) application: &'a mut Application,
    pub(crate) windows: &'a mut HashMap<NativeWindowId, NativeWindowState>,
    pub(crate) native_ids: &'a HashMap<IncularWindowId, NativeWindowId>,
    pub(crate) transient_windows: &'a mut HashMap<NativeWindowId, NativeTransientState>,
    pub(crate) transient_native_ids: &'a mut HashMap<TransientHostKey, NativeWindowId>,
    pub(crate) transient_native_rejections:
        &'a mut HashMap<TransientHostKey, TransientNativeRejection>,
    pub(crate) shared_gpu: &'a Option<SharedGpuContext>,
    pub(crate) pointer_devices: &'a mut PointerDeviceRegistry,
    pub(crate) platform_services: &'a dyn DesktopPlatformServices,
    pub(crate) window_system: Option<NativeWindowSystem>,
    pub(crate) pending_native_destructions: &'a mut HashSet<NativeWindowId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct TransientHostKey {
    pub(super) owner: IncularWindowId,
    pub(super) transient: TransientSurfaceId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct TransientNativeRejection {
    pub(super) role: TransientRole,
    pub(super) reason: TransientFallbackReason,
}

pub(super) struct NativeTransientState {
    key: TransientHostKey,
    snapshot: TransientSurfaceSnapshot,
    renderer: WgpuRenderer,
    metrics: WindowMetrics,
    input: WindowInputState,
    native_cursor: NativeCursorCoordinator,
    display_list: DisplayList,
    requires_opaque_surface_base: bool,
    native_rect: incular_core::Rect,
    requested_position: Option<PhysicalPosition<i32>>,
    visible: bool,
    /// The renderer also owns this window through its surface target. Keeping
    /// the host reference last preserves renderer-before-window teardown.
    window: Arc<Window>,
}

impl TransientContext<'_> {
    fn track_native_window_drop(&mut self, native_id: NativeWindowId) {
        if self.window_system.is_some_and(|system| {
            self.platform_services
                .wait_for_destroyed_event_after_window_drop(system)
        }) {
            self.pending_native_destructions.insert(native_id);
        }
    }
    fn note_window_input(&mut self, native_id: NativeWindowId, kind: crate::input::InputKind) {
        crate::host_environment::note_input(self.application, self.windows, native_id, kind);
    }

    fn transient_owner_native_id(&self, native_id: NativeWindowId) -> Option<NativeWindowId> {
        let owner = self.transient_windows.get(&native_id)?.key.owner;
        self.native_ids.get(&owner).copied()
    }

    pub(super) fn publish_native_transient_bounds(&mut self, owner: IncularWindowId) {
        let bounds = self.native_transient_available_rect(owner);
        let _ = self.application.set_native_transient_bounds(owner, bounds);
    }

    fn native_transient_available_rect(
        &self,
        owner: IncularWindowId,
    ) -> Option<incular_core::Rect> {
        self.window_system?;
        let capabilities = self.application.window_capabilities(owner)?;
        if !capabilities.transients.native_surface.is_supported() {
            return None;
        }
        let native_id = self.native_ids.get(&owner)?;
        let parent = self.windows.get(native_id)?;
        let scale = parent.metrics.scale_factor;
        if !scale.is_finite() || scale <= 0.0 {
            return None;
        }
        let monitor = parent.window.current_monitor()?;
        let monitor_position = monitor.position();
        let monitor_size = monitor.size();
        let physical = self
            .platform_services
            .work_area(&monitor)
            .unwrap_or_else(|| {
                incular_platform::PhysicalScreenRect::new(
                    monitor_position.x,
                    monitor_position.y,
                    monitor_size.width,
                    monitor_size.height,
                )
            });
        let parent_origin = parent.window.inner_position().ok()?;
        Some(incular_core::Rect::from_origin_size(
            Offset::new(
                ((f64::from(physical.origin.x) - f64::from(parent_origin.x)) / scale) as f32,
                ((f64::from(physical.origin.y) - f64::from(parent_origin.y)) / scale) as f32,
            ),
            incular_core::Size::new(
                (f64::from(physical.size.width) / scale) as f32,
                (f64::from(physical.size.height) / scale) as f32,
            ),
        ))
    }

    fn native_transient_rect(
        &self,
        owner: IncularWindowId,
        snapshot: TransientSurfaceSnapshot,
    ) -> Option<incular_core::Rect> {
        let available_rect = self.native_transient_available_rect(owner)?;
        Some(
            incular_widgets::place_transient(incular_widgets::TransientPlacementInput {
                anchor_rect: snapshot.anchor_rect,
                desired_size: snapshot.desired_size,
                available_rect,
                role: snapshot.role,
                text_direction: snapshot.text_direction,
                placement: snapshot.placement,
            })
            .rect,
        )
    }

    pub(super) fn synchronize_transient_hosts(
        &mut self,
        target: &ActiveEventLoop,
        owner: IncularWindowId,
        snapshots: &[TransientSurfaceSnapshot],
    ) -> HashSet<incular_rendering::SurfacePartitionId> {
        let capabilities = self
            .application
            .window_capabilities(owner)
            .unwrap_or_default();
        let desired = snapshots
            .iter()
            .copied()
            .filter(|snapshot| {
                snapshot.presentation == TransientPresentation::Auto
                    && capabilities.transients.native_surface.is_supported()
                    && capabilities
                        .transients
                        .support(snapshot.role)
                        .is_supported()
                    && valid_transient_size(snapshot.desired_size)
            })
            .map(|snapshot| (snapshot.id, snapshot))
            .collect::<HashMap<_, _>>();

        self.transient_native_rejections.retain(|key, rejection| {
            key.owner != owner
                || desired
                    .get(&key.transient)
                    .is_some_and(|snapshot| snapshot.role == rejection.role)
        });

        let stale = self
            .transient_native_ids
            .keys()
            .copied()
            .filter(|key| key.owner == owner && !desired.contains_key(&key.transient))
            .collect::<Vec<_>>();
        for key in stale {
            self.destroy_transient_host(key);
        }

        for snapshot in desired.values().copied() {
            let key = TransientHostKey {
                owner,
                transient: snapshot.id,
            };
            if self
                .transient_native_rejections
                .get(&key)
                .is_some_and(|rejection| rejection.role == snapshot.role)
            {
                continue;
            }
            let recreate = self
                .transient_native_ids
                .get(&key)
                .and_then(|native_id| self.transient_windows.get(native_id))
                .is_some_and(|state| state.snapshot.role != snapshot.role);
            if recreate {
                self.destroy_transient_host(key);
            }
            if self.transient_native_ids.contains_key(&key) {
                self.update_transient_host_geometry(key, snapshot);
            } else if !self.create_transient_host(target, key, snapshot) {
                self.transient_native_rejections.insert(
                    key,
                    TransientNativeRejection {
                        role: snapshot.role,
                        reason: TransientFallbackReason::NativeHostUnavailable,
                    },
                );
            }
        }

        desired
            .keys()
            .copied()
            .filter_map(|transient| {
                let key = TransientHostKey { owner, transient };
                self.transient_native_ids
                    .contains_key(&key)
                    .then(|| transient.surface_partition())
            })
            .collect()
    }

    pub(super) fn publish_transient_presentations(
        &mut self,
        owner: IncularWindowId,
        snapshots: &[TransientSurfaceSnapshot],
    ) {
        let capabilities = self
            .application
            .window_capabilities(owner)
            .unwrap_or_default();
        let presentations = snapshots
            .iter()
            .copied()
            .map(|snapshot| {
                let key = TransientHostKey {
                    owner,
                    transient: snapshot.id,
                };
                if snapshot.presentation == TransientPresentation::Overlay {
                    return TransientPresentationResolution {
                        id: snapshot.id,
                        role: snapshot.role,
                        requested: snapshot.presentation,
                        resolved: ResolvedTransientPresentation::Overlay,
                        fallback_reason: Some(TransientFallbackReason::RequestedOverlay),
                    };
                }
                if self.transient_native_ids.contains_key(&key) {
                    return TransientPresentationResolution {
                        id: snapshot.id,
                        role: snapshot.role,
                        requested: snapshot.presentation,
                        resolved: ResolvedTransientPresentation::Native,
                        fallback_reason: None,
                    };
                }
                let fallback_reason = self
                    .transient_native_rejections
                    .get(&key)
                    .filter(|rejection| rejection.role == snapshot.role)
                    .map_or_else(
                        || {
                            if !capabilities.transients.native_surface.is_supported()
                                || !capabilities
                                    .transients
                                    .support(snapshot.role)
                                    .is_supported()
                            {
                                TransientFallbackReason::NativeUnsupported
                            } else {
                                TransientFallbackReason::NativeHostUnavailable
                            }
                        },
                        |rejection| rejection.reason,
                    );
                TransientPresentationResolution {
                    id: snapshot.id,
                    role: snapshot.role,
                    requested: snapshot.presentation,
                    resolved: ResolvedTransientPresentation::Overlay,
                    fallback_reason: Some(fallback_reason),
                }
            })
            .collect();
        let _ = self
            .application
            .set_transient_presentations(owner, presentations);
    }

    pub(super) fn create_transient_host(
        &mut self,
        target: &ActiveEventLoop,
        key: TransientHostKey,
        snapshot: TransientSurfaceSnapshot,
    ) -> bool {
        let Some(system) = self.window_system else {
            return false;
        };
        let Some(parent_native_id) = self.native_ids.get(&key.owner).copied() else {
            return false;
        };
        let Some(native_rect) = self.native_transient_rect(key.owner, snapshot) else {
            return false;
        };
        let Some((position, attributes)) = self.windows.get(&parent_native_id).and_then(|parent| {
            let position =
                transient_screen_position(&parent.window, parent.metrics, native_rect.origin)?;
            let base = WindowAttributes::default()
                .with_title("")
                .with_inner_size(winit::dpi::LogicalSize::new(
                    f64::from(native_rect.size.width),
                    f64::from(native_rect.size.height),
                ))
                .with_position(position)
                .with_resizable(false)
                .with_decorations(false)
                .with_visible(false)
                .with_transparent(true)
                .with_active(false);
            Some((
                position,
                self.platform_services.configure_transient_attributes(
                    system,
                    &parent.window,
                    snapshot.role,
                    base,
                ),
            ))
        }) else {
            return false;
        };

        let window = match target.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                eprintln!("Incular transient window error: {error}");
                return false;
            }
        };
        let native_id = window.id();
        let metrics = WindowMetrics::new(
            PhysicalSize::new(window.inner_size().width, window.inner_size().height),
            window.scale_factor(),
        );
        let Some(shared) = self.shared_gpu.clone() else {
            self.track_native_window_drop(native_id);
            return false;
        };
        let surface_target = incular_wgpu::WindowSurfaceTarget::new(window.clone());
        let transparent_renderer = pollster::block_on(shared.create_renderer(
            surface_target.clone(),
            metrics.physical_size,
            TransparencyMode::Transparent,
            Color::TRANSPARENT,
        ));
        let (renderer, requires_opaque_surface_base) = match transparent_renderer {
            Ok(renderer) => (renderer, false),
            Err(RendererError::SurfaceAlpha(_)) => match pollster::block_on(shared.create_renderer(
                surface_target.clone(),
                metrics.physical_size,
                TransparencyMode::Opaque,
                Color::TRANSPARENT,
            )) {
                Ok(renderer) => (renderer, true),
                Err(error) => {
                    eprintln!("Incular transient renderer initialization failed: {error}");
                    self.track_native_window_drop(native_id);
                    return false;
                }
            },
            Err(error) => {
                eprintln!("Incular transient renderer initialization failed: {error}");
                self.track_native_window_drop(native_id);
                return false;
            }
        };
        let Some(parent) = self.windows.get(&parent_native_id) else {
            self.track_native_window_drop(native_id);
            return false;
        };
        if let Err(error) =
            self.platform_services
                .attach_transient(system, &parent.window, &window, snapshot.role)
        {
            eprintln!("Incular transient attachment failed: {error}");
            self.track_native_window_drop(native_id);
            return false;
        }

        self.transient_native_ids.insert(key, native_id);
        self.transient_windows.insert(
            native_id,
            NativeTransientState {
                key,
                snapshot,
                window,
                renderer,
                metrics,
                input: WindowInputState::default(),
                native_cursor: NativeCursorCoordinator::default(),
                display_list: DisplayList::new(),
                requires_opaque_surface_base,
                native_rect,
                requested_position: Some(position),
                visible: false,
            },
        );
        true
    }

    pub(super) fn update_transient_host_geometry(
        &mut self,
        key: TransientHostKey,
        snapshot: TransientSurfaceSnapshot,
    ) {
        let Some(native_id) = self.transient_native_ids.get(&key).copied() else {
            return;
        };
        let Some(parent_native_id) = self.native_ids.get(&key.owner).copied() else {
            return;
        };
        let Some(native_rect) = self.native_transient_rect(key.owner, snapshot) else {
            return;
        };
        let position = self.windows.get(&parent_native_id).and_then(|parent| {
            transient_screen_position(&parent.window, parent.metrics, native_rect.origin)
        });
        let Some(state) = self.transient_windows.get_mut(&native_id) else {
            return;
        };
        let old_size = state.native_rect.size;
        if old_size != native_rect.size
            && let Some(size) = state
                .window
                .request_inner_size(winit::dpi::LogicalSize::new(
                    f64::from(native_rect.size.width),
                    f64::from(native_rect.size.height),
                ))
        {
            let physical = PhysicalSize::new(size.width, size.height);
            state.metrics = WindowMetrics::new(physical, state.window.scale_factor());
            state.renderer.resize(physical);
        }
        if position != state.requested_position {
            if let Some(position) = position {
                state.window.set_outer_position(position);
            }
            state.requested_position = position;
        }
        state.native_rect = native_rect;
        state.snapshot = snapshot;
    }

    pub(super) fn reposition_transient_hosts(&mut self, owner: IncularWindowId) {
        let snapshots = self
            .transient_native_ids
            .keys()
            .copied()
            .filter(|key| key.owner == owner)
            .filter_map(|key| {
                let native_id = self.transient_native_ids.get(&key)?;
                let snapshot = self.transient_windows.get(native_id)?.snapshot;
                Some((key, snapshot))
            })
            .collect::<Vec<_>>();
        for (key, snapshot) in snapshots {
            self.update_transient_host_geometry(key, snapshot);
        }
    }

    pub(super) fn destroy_transient_host(&mut self, key: TransientHostKey) {
        let Some(native_id) = self.transient_native_ids.remove(&key) else {
            return;
        };
        let Some(state) = self.transient_windows.remove(&native_id) else {
            return;
        };
        if let (Some(system), Some(parent_native_id)) =
            (self.window_system, self.native_ids.get(&key.owner).copied())
            && let Some(parent) = self.windows.get(&parent_native_id)
        {
            self.platform_services.detach_transient(
                system,
                &parent.window,
                &state.window,
                state.snapshot.role,
            );
        }
        self.track_native_window_drop(native_id);
    }

    pub(super) fn destroy_transient_hosts_for_owner(&mut self, owner: IncularWindowId) {
        let keys = self
            .transient_native_ids
            .keys()
            .copied()
            .filter(|key| key.owner == owner)
            .collect::<Vec<_>>();
        for key in keys {
            self.destroy_transient_host(key);
        }
        self.transient_native_rejections
            .retain(|key, _| key.owner != owner);
    }

    pub(super) fn render_transient_partition(
        &mut self,
        key: TransientHostKey,
        list: &DisplayList,
    ) -> Result<bool, TransientFallbackReason> {
        let Some(system) = self.window_system else {
            return Err(TransientFallbackReason::NativeHostUnavailable);
        };
        let Some(native_id) = self.transient_native_ids.get(&key).copied() else {
            return Err(TransientFallbackReason::NativeHostUnavailable);
        };
        let Some(state) = self.transient_windows.get_mut(&native_id) else {
            return Err(TransientFallbackReason::NativeHostUnavailable);
        };
        state.display_list = list.translated(Offset::new(
            -state.snapshot.content_offset().x,
            -state.snapshot.content_offset().y,
        ));
        if state.requires_opaque_surface_base
            && !state
                .display_list
                .begins_with_opaque_surface_rect(state.snapshot.content_size())
        {
            eprintln!(
                "Incular transient requires compositor alpha but this GPU surface is opaque; falling back to in-view overlay"
            );
            return Err(TransientFallbackReason::SurfaceTransparencyRequired);
        }
        if state.snapshot.content_rect != state.native_rect {
            // Native negotiation succeeded, but the retained tree is still at
            // its viewport-safe overlay placement. Keep the host hidden and
            // the parent partition attached for this frame; publishing Native
            // below requests the canonical work-area placement frame.
            return Ok(false);
        }
        let stats = match state
            .renderer
            .render(&state.display_list, state.metrics.scale_factor)
        {
            Ok(stats) => stats,
            Err(RendererError::OutOfMemory) => {
                eprintln!("Incular transient renderer stopped: out of GPU memory");
                return Err(TransientFallbackReason::NativeHostUnavailable);
            }
            Err(error) => {
                eprintln!("Incular transient renderer error: {error}");
                return Err(TransientFallbackReason::NativeHostUnavailable);
            }
        };
        if stats.presented && !state.visible {
            if let Err(error) =
                self.platform_services
                    .show_transient(system, &state.window, state.snapshot.role)
            {
                eprintln!("Incular transient show failed: {error}");
                return Err(TransientFallbackReason::NativeHostUnavailable);
            }
            state.visible = true;
        }
        if !stats.presented {
            state.window.request_redraw();
        }
        Ok(true)
    }

    pub(super) fn redraw_transient_host(&mut self, native_id: NativeWindowId) {
        let Some(system) = self.window_system else {
            return;
        };
        let mut failed = None;
        {
            let Some(state) = self.transient_windows.get_mut(&native_id) else {
                return;
            };
            if state.snapshot.content_rect != state.native_rect {
                return;
            }
            if state.requires_opaque_surface_base
                && !state
                    .display_list
                    .begins_with_opaque_surface_rect(state.snapshot.content_size())
            {
                failed = Some((
                    state.key,
                    state.snapshot.role,
                    TransientFallbackReason::SurfaceTransparencyRequired,
                ));
            } else {
                match state
                    .renderer
                    .render(&state.display_list, state.metrics.scale_factor)
                {
                    Ok(stats) => {
                        if stats.presented && !state.visible {
                            match self.platform_services.show_transient(
                                system,
                                &state.window,
                                state.snapshot.role,
                            ) {
                                Ok(()) => state.visible = true,
                                Err(error) => {
                                    eprintln!("Incular transient show failed: {error}");
                                    failed = Some((
                                        state.key,
                                        state.snapshot.role,
                                        TransientFallbackReason::NativeHostUnavailable,
                                    ));
                                }
                            }
                        }
                        if !stats.presented {
                            state.window.request_redraw();
                        }
                    }
                    Err(RendererError::OutOfMemory) => {
                        eprintln!("Incular transient renderer stopped: out of GPU memory");
                        failed = Some((
                            state.key,
                            state.snapshot.role,
                            TransientFallbackReason::NativeHostUnavailable,
                        ));
                    }
                    Err(error) => {
                        eprintln!("Incular transient renderer error: {error}");
                        failed = Some((
                            state.key,
                            state.snapshot.role,
                            TransientFallbackReason::NativeHostUnavailable,
                        ));
                    }
                }
            }
        }
        if let Some((key, role, reason)) = failed {
            self.destroy_transient_host(key);
            self.transient_native_rejections
                .insert(key, TransientNativeRejection { role, reason });
            let snapshots = self.application.transient_surfaces(key.owner);
            self.publish_transient_presentations(key.owner, &snapshots);
            if let Some(parent_native_id) = self.native_ids.get(&key.owner).copied()
                && let Some(parent) = self.windows.get(&parent_native_id)
            {
                parent.window.request_redraw();
                self.application.note_frame_requested(key.owner);
            }
        }
    }

    pub(super) fn route_transient_platform_event(
        &mut self,
        native_id: NativeWindowId,
        event: PlatformEvent,
    ) {
        let Some(state) = self.transient_windows.get(&native_id) else {
            return;
        };
        let owner = state.key.owner;
        let offset = state.snapshot.content_offset();
        self.application
            .handle_window_event(IncularWindowEvent::platform(
                owner,
                offset_transient_platform_event(event, offset),
            ));
    }

    pub(super) fn sync_transient_cursor(&mut self, native_id: NativeWindowId) {
        let Some(owner) = self
            .transient_windows
            .get(&native_id)
            .map(|state| state.key.owner)
        else {
            return;
        };
        let Some(cursor) = self.application.window_mouse_cursor(owner) else {
            return;
        };
        let Some(state) = self.transient_windows.get_mut(&native_id) else {
            return;
        };
        if let Some(native) = state.native_cursor.update(cursor) {
            state.window.set_cursor(native);
        }
    }

    pub(super) fn cancel_transient_mouse_pointer(&mut self, native_id: NativeWindowId) {
        let Some(state) = self.transient_windows.get_mut(&native_id) else {
            return;
        };
        let events = state.input.cancel(state.metrics);
        for event in events.into_iter().flatten() {
            self.route_transient_platform_event(native_id, event);
        }
        self.sync_transient_cursor(native_id);
    }

    fn handle_transient_input(&mut self, native_id: NativeWindowId, event: &WindowEvent) {
        let native = if let WindowEvent::Touch(touch) = event {
            self.platform_services.take_native_pointer_sample(touch.id)
        } else {
            None
        };
        let state = self
            .transient_windows
            .get_mut(&native_id)
            .expect("known transient window");
        let Some(input) = state
            .input
            .translate(event, state.metrics, self.pointer_devices, native)
        else {
            return;
        };
        if let Some(parent) = self.transient_owner_native_id(native_id) {
            self.note_window_input(parent, input.kind);
        }
        for event in input.events.into_iter().flatten() {
            self.route_transient_platform_event(native_id, event);
        }
        self.sync_transient_cursor(native_id);
    }

    pub(super) fn handle_transient_window_event(
        &mut self,
        native_id: NativeWindowId,
        event: &WindowEvent,
    ) -> bool {
        if !self.transient_windows.contains_key(&native_id) {
            return false;
        }
        match event {
            WindowEvent::Resized(size) => {
                if let Some(state) = self.transient_windows.get_mut(&native_id) {
                    let physical = PhysicalSize::new(size.width, size.height);
                    state.metrics = WindowMetrics::new(physical, state.metrics.scale_factor);
                    if state.renderer.physical_size() != physical {
                        state.renderer.resize(physical);
                    }
                    if !physical.is_zero() {
                        state.window.request_redraw();
                    }
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                if let Some(state) = self.transient_windows.get_mut(&native_id) {
                    let size = state.window.inner_size();
                    let physical = PhysicalSize::new(size.width, size.height);
                    state.metrics = WindowMetrics::new(physical, *scale_factor);
                    if state.renderer.physical_size() != physical {
                        state.renderer.resize(physical);
                    }
                    if !physical.is_zero() {
                        state.window.request_redraw();
                    }
                }
            }
            WindowEvent::Moved(position) => {
                if let Some(state) = self.transient_windows.get_mut(&native_id) {
                    state.requested_position = Some(*position);
                }
            }
            WindowEvent::Focused(false) => self.cancel_transient_mouse_pointer(native_id),
            WindowEvent::RedrawRequested => self.redraw_transient_host(native_id),
            WindowEvent::CloseRequested | WindowEvent::Focused(true) => {}
            _ => self.handle_transient_input(native_id, event),
        }
        true
    }
}

fn valid_transient_size(size: incular_core::Size) -> bool {
    size.width.is_finite() && size.height.is_finite() && size.width > 0.0 && size.height > 0.0
}

fn transient_screen_position(
    parent: &Window,
    metrics: WindowMetrics,
    offset: Offset,
) -> Option<PhysicalPosition<i32>> {
    let origin = parent.inner_position().ok()?;
    let scale = metrics.scale_factor;
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    if !offset.x.is_finite() || !offset.y.is_finite() {
        return None;
    }
    let x = f64::from(origin.x) + f64::from(offset.x) * scale;
    let y = f64::from(origin.y) + f64::from(offset.y) * scale;
    if !x.is_finite()
        || !y.is_finite()
        || x < f64::from(i32::MIN)
        || x > f64::from(i32::MAX)
        || y < f64::from(i32::MIN)
        || y > f64::from(i32::MAX)
    {
        return None;
    }
    Some(PhysicalPosition::new(x.round() as i32, y.round() as i32))
}
