mod support;
use incular_wgpu::WindowSurfaceTarget;
use std::sync::Arc;

fn from_borrowed(window: &support::Window) -> WindowSurfaceTarget {
    WindowSurfaceTarget::new(Arc::new(window))
}

fn main() {}
