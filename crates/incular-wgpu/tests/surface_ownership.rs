use incular_config::TransparencyMode;
use incular_core::Color;
use incular_platform::PhysicalSize;
use incular_wgpu::{SharedGpuContext, WgpuRenderer, WindowSurfaceTarget};
use std::sync::Arc;
use wgpu::rwh::{DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle};

// An unavailable native target is a valid safe provider. No fabricated native
// pointers or GPU access are necessary to verify ownership before polling.
struct UnavailableWindow;

impl HasWindowHandle for UnavailableWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        Err(HandleError::Unavailable)
    }
}

impl HasDisplayHandle for UnavailableWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Err(HandleError::Unavailable)
    }
}

#[test]
fn cloned_surface_targets_retain_the_window_until_the_last_owner_drops() {
    let window = Arc::new(UnavailableWindow);
    let weak = Arc::downgrade(&window);
    let target = WindowSurfaceTarget::new(window.clone());
    let recovery_target = target.clone();
    drop(window);
    drop(target);
    assert!(weak.upgrade().is_some());
    drop(recovery_target);
    assert!(weak.upgrade().is_none());
}

#[test]
fn pending_renderer_initialization_owns_the_window_and_cancellation_releases_it() {
    let window = Arc::new(UnavailableWindow);
    let weak = Arc::downgrade(&window);
    let initialization = WgpuRenderer::new(
        WindowSurfaceTarget::new(window),
        PhysicalSize::new(1, 1),
        TransparencyMode::Opaque,
        Color::TRANSPARENT,
    );
    assert!(weak.upgrade().is_some());
    drop(initialization);
    assert!(weak.upgrade().is_none());
}

#[test]
fn pending_adapter_selection_owns_the_window_and_cancellation_releases_it() {
    let window = Arc::new(UnavailableWindow);
    let weak = Arc::downgrade(&window);
    let initialization =
        SharedGpuContext::new(WindowSurfaceTarget::new(window), TransparencyMode::Opaque);
    assert!(weak.upgrade().is_some());
    drop(initialization);
    assert!(weak.upgrade().is_none());
}
