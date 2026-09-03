#[cfg(target_os = "windows")]
#[test]
fn windows_facade_adds_pen_metadata_without_claiming_trackpad_gestures() {
    use incular_platform::{CapabilitySupport, NativeWindowSystem};

    let capabilities = incular_windows::advanced_input_capabilities(NativeWindowSystem::Win32);
    assert_eq!(capabilities.stylus, CapabilitySupport::Supported);
    assert_eq!(
        capabilities.trackpad_gestures,
        CapabilitySupport::Unsupported
    );

    let other = incular_windows::advanced_input_capabilities(NativeWindowSystem::Other);
    assert_eq!(other.stylus, CapabilitySupport::Unsupported);
}
