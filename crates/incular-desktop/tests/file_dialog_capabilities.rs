use incular_desktop::desktop_file_dialog_capabilities;
use incular_platform::{
    CapabilitySupport, FileDialogError, FileDialogFilter, FileDialogKind, FileDialogOption,
    FileDialogOptions, FileDialogRequest, NativeWindowSystem,
};

#[test]
fn windows_native_dialog_capabilities_are_explicit() {
    let capabilities = desktop_file_dialog_capabilities(Some(NativeWindowSystem::Win32));
    assert_eq!(capabilities.open_file, CapabilitySupport::Supported);
    assert_eq!(capabilities.open_files, CapabilitySupport::Supported);
    assert_eq!(capabilities.save_file, CapabilitySupport::Supported);
    assert_eq!(capabilities.select_folder, CapabilitySupport::Supported);
    assert_eq!(capabilities.extension_filters, CapabilitySupport::Supported);
    assert_eq!(
        capabilities.mime_type_filters,
        CapabilitySupport::Unsupported
    );
    assert_eq!(
        capabilities.type_identifier_filters,
        CapabilitySupport::Unsupported
    );
    assert_eq!(capabilities.parent_window, CapabilitySupport::Supported);
}

#[test]
fn unsupported_semantic_filter_is_rejected_instead_of_dropped() {
    let capabilities = desktop_file_dialog_capabilities(Some(NativeWindowSystem::Win32));
    let options = FileDialogOptions::new()
        .filters([FileDialogFilter::content_types("image", "Images", ["image/png"]).unwrap()])
        .unwrap();
    let request = FileDialogRequest::new(FileDialogKind::OpenFile, options).unwrap();

    assert_eq!(
        capabilities.validate_request(&request),
        Err(FileDialogError::UnsupportedOption(
            FileDialogOption::MimeTypeFilters
        ))
    );
}

#[test]
fn unparentable_native_window_system_is_rejected() {
    let capabilities = desktop_file_dialog_capabilities(Some(NativeWindowSystem::Other));
    let request =
        FileDialogRequest::new(FileDialogKind::OpenFile, FileDialogOptions::new()).unwrap();

    assert_eq!(
        capabilities.validate_request(&request),
        Err(FileDialogError::UnsupportedOption(
            FileDialogOption::ParentWindow
        ))
    );
}

#[test]
fn extension_filter_is_accepted_by_windows_backend_contract() {
    let capabilities = desktop_file_dialog_capabilities(Some(NativeWindowSystem::Win32));
    let options = FileDialogOptions::new()
        .filters([FileDialogFilter::extensions("text", "Text", ["txt", "md"]).unwrap()])
        .unwrap();
    let request = FileDialogRequest::new(FileDialogKind::OpenFiles, options).unwrap();
    assert_eq!(capabilities.validate_request(&request), Ok(()));
}

#[test]
fn linux_portal_backends_advertise_mime_but_not_uti_like_filters() {
    for system in [NativeWindowSystem::X11, NativeWindowSystem::Wayland] {
        let capabilities = desktop_file_dialog_capabilities(Some(system));
        assert_eq!(capabilities.parent_window, CapabilitySupport::Supported);
        assert_eq!(capabilities.mime_type_filters, CapabilitySupport::Supported);
        assert_eq!(
            capabilities.type_identifier_filters,
            CapabilitySupport::Unsupported
        );
    }
}
