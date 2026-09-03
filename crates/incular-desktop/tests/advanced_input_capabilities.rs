use incular_desktop::desktop_advanced_input_capabilities;
use incular_platform::{CapabilitySupport, NativeWindowSystem};

#[test]
fn aggregate_trackpad_support_matches_the_winit_desktop_backends() {
    let appkit = desktop_advanced_input_capabilities(NativeWindowSystem::AppKit);
    assert_eq!(appkit.trackpad_gestures, CapabilitySupport::Supported);
    assert_eq!(appkit.stylus, CapabilitySupport::Unsupported);

    for system in [
        NativeWindowSystem::Win32,
        NativeWindowSystem::X11,
        NativeWindowSystem::Wayland,
        NativeWindowSystem::Other,
    ] {
        let capabilities = desktop_advanced_input_capabilities(system);
        assert_eq!(
            capabilities.trackpad_gestures,
            CapabilitySupport::Unsupported
        );
        assert_eq!(capabilities.stylus, CapabilitySupport::Unsupported);
    }
}
