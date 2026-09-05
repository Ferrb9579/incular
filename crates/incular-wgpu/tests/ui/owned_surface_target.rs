mod support;
use incular_wgpu::WindowSurfaceTarget;
use std::sync::Arc;

fn main() {
    let target = {
        let window = Arc::new(support::Window);
        WindowSurfaceTarget::new(window)
    };
    drop(target);
}
