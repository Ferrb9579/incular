use incular_core::Color;
use incular_platform::{PhysicalSize, RawWindowHandles, TransparencyMode};
use incular_wgpu::WgpuRenderer;

fn from_detached(handles: RawWindowHandles) {
    drop(WgpuRenderer::new(
        handles,
        PhysicalSize::new(64, 64),
        TransparencyMode::Opaque,
        Color::BLACK,
    ));
}

fn main() {}
