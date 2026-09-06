use incular_core::Color;
use incular_platform::{PhysicalSize, TransparencyMode};
use incular_wgpu::WgpuRenderer;

#[derive(Clone, Copy)]
struct RawWindowHandles { window: raw_window_handle::RawWindowHandle }

fn from_detached(handles: RawWindowHandles) {
    drop(WgpuRenderer::new(
        handles,
        PhysicalSize::new(64, 64),
        TransparencyMode::Opaque,
        Color::BLACK,
    ));
}

fn main() {}
