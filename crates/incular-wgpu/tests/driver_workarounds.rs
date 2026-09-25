use incular_wgpu::{RendererError, SurfaceAlphaPlan};

#[path = "../src/driver_workarounds.rs"]
mod driver_workarounds;

#[test]
fn bootstrap_is_limited_to_reproduced_native_driver_versions() {
    // The native execution is exercised by the windowed idle probe and desktop
    // smoke suite; this test guards the automatic workaround's eligibility.
    let _bootstrap = driver_workarounds::bootstrap_presentation;
    let mut info = wgpu::AdapterInfo::new(wgpu::DeviceType::IntegratedGpu, wgpu::Backend::Dx12);
    info.vendor = 0x1002;
    info.device = 0x164e;
    info.driver = "32.0.21036.11002".into();
    assert!(driver_workarounds::needs_present_bootstrap(&info));
    for driver in ["", "32.0.21036.11003", "32.0.31041.3013"] {
        info.driver = driver.into();
        assert!(!driver_workarounds::needs_present_bootstrap(&info));
    }
    info.backend = wgpu::Backend::Vulkan;
    info.driver_info = "25.10.36.11 (AMD proprietary shader compiler)".into();
    assert!(driver_workarounds::needs_present_bootstrap(&info));
    info.driver_info = "25.10.36.110 (AMD proprietary shader compiler)".into();
    assert!(!driver_workarounds::needs_present_bootstrap(&info));
    info.driver_info = "25.10.36.11 (AMD proprietary shader compiler)".into();
    info.device = 0x9999;
    assert!(!driver_workarounds::needs_present_bootstrap(&info));
    info.device = 0x164e;
    info.vendor = 0x10de;
    assert!(!driver_workarounds::needs_present_bootstrap(&info));
    info.vendor = 0x1002;
    for backend in [wgpu::Backend::Gl, wgpu::Backend::Metal, wgpu::Backend::Noop] {
        info.backend = backend;
        assert!(!driver_workarounds::needs_present_bootstrap(&info));
    }
}
#[allow(dead_code)]
#[path = "../src/built_in_shaders.rs"]
mod built_in_shaders;
