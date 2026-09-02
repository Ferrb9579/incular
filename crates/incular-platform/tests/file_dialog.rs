use incular_platform::{
    CapabilitySupport, DocumentActivation, FileContentType, FileContentTypeKind,
    FileDialogCapabilities, FileDialogError, FileDialogFilter, FileDialogFilterId, FileDialogKind,
    FileDialogOption, FileDialogOptions, FileDialogRequest, FileDialogValidationError,
    FileExtension,
};
use std::path::PathBuf;

#[test]
fn extensions_and_mime_content_types_are_normalized_and_deduplicated() {
    let filter = FileDialogFilter::new(
        FileDialogFilterId::new("images").unwrap(),
        Some("Images".to_owned()),
        [
            FileExtension::new(".PNG").unwrap(),
            FileExtension::new("png").unwrap(),
            FileExtension::new(" JpG ").unwrap(),
        ],
        [
            FileContentType::new("IMAGE/PNG").unwrap(),
            FileContentType::new("image/png").unwrap(),
            FileContentType::new("public.jpeg").unwrap(),
        ],
    )
    .unwrap();

    assert_eq!(
        filter
            .extensions_list()
            .iter()
            .map(FileExtension::as_str)
            .collect::<Vec<_>>(),
        ["png", "jpg"]
    );
    assert_eq!(
        filter
            .content_types_list()
            .iter()
            .map(FileContentType::as_str)
            .collect::<Vec<_>>(),
        ["image/png", "public.jpeg"]
    );
    assert_eq!(
        filter.content_types_list()[0].kind(),
        FileContentTypeKind::Mime
    );
    assert_eq!(
        filter.content_types_list()[1].kind(),
        FileContentTypeKind::TypeIdentifier
    );
}

#[test]
fn semantic_filter_identity_is_independent_of_display_label() {
    let first = FileDialogFilter::extensions("source", "Rust source", ["rs"]).unwrap();
    let second = FileDialogFilter::extensions("source", "Source code", ["rs"]).unwrap();
    assert_eq!(first.id(), second.id());
    assert_ne!(first.label(), second.label());
}

#[test]
fn duplicate_filter_ids_are_rejected_even_when_labels_differ() {
    let first = FileDialogFilter::extensions("text", "Text", ["txt"]).unwrap();
    let second = FileDialogFilter::extensions("text", "Documents", ["md"]).unwrap();
    let error = FileDialogOptions::new()
        .filters([first, second])
        .unwrap_err();
    assert!(matches!(
        error,
        FileDialogValidationError::DuplicateFilterId(_)
    ));
}

#[test]
fn options_that_do_not_apply_are_rejected_before_native_execution() {
    let options = FileDialogOptions::new()
        .suggested_file_name("report.txt")
        .unwrap();
    assert_eq!(
        FileDialogRequest::new(FileDialogKind::OpenFile, options).unwrap_err(),
        FileDialogValidationError::OptionNotApplicable(FileDialogOption::SuggestedFileName)
    );

    let folder_options = FileDialogOptions::new()
        .filters([FileDialogFilter::extensions("text", "Text", ["txt"]).unwrap()])
        .unwrap();
    assert_eq!(
        FileDialogRequest::new(FileDialogKind::SelectFolder, folder_options).unwrap_err(),
        FileDialogValidationError::OptionNotApplicable(FileDialogOption::ExtensionFilters)
    );

    let mime_folder_options = FileDialogOptions::new()
        .filters([FileDialogFilter::content_types("images", "Images", ["image/png"]).unwrap()])
        .unwrap();
    assert_eq!(
        FileDialogRequest::new(FileDialogKind::SelectFolder, mime_folder_options).unwrap_err(),
        FileDialogValidationError::OptionNotApplicable(FileDialogOption::MimeTypeFilters)
    );
}

#[test]
fn suggested_filename_validation_is_platform_independent() {
    for invalid in ["", "   ", ".", "..", "folder/file.txt", r"folder\file.txt"] {
        assert_eq!(
            FileDialogOptions::new().suggested_file_name(invalid),
            Err(FileDialogValidationError::InvalidSuggestedFileName),
            "{invalid:?} must not be interpreted using host-specific path separators"
        );
    }
    assert_eq!(
        FileDialogOptions::new()
            .suggested_file_name("report.final.txt")
            .unwrap()
            .suggested_file_name_value(),
        Some("report.final.txt")
    );
}

#[test]
fn capability_validation_reports_unsupported_content_types_explicitly() {
    let mut capabilities = FileDialogCapabilities::all(CapabilitySupport::Supported);
    capabilities.mime_type_filters = CapabilitySupport::Unsupported;
    let options = FileDialogOptions::new()
        .filters([FileDialogFilter::content_types("images", "Images", ["image/png"]).unwrap()])
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
fn document_activation_preserves_paths_exactly_and_in_order() {
    let first = PathBuf::from(r"relative\report.txt");
    let second = PathBuf::from(r"D:\work\draft.md");
    let activation = DocumentActivation::from_paths([first.clone(), second.clone()]);
    assert_eq!(activation.documents()[0].path(), first.as_path());
    assert_eq!(activation.documents()[1].path(), second.as_path());
}

#[test]
fn every_native_dialog_requires_parent_window_support() {
    let mut capabilities = FileDialogCapabilities::all(CapabilitySupport::Supported);
    capabilities.parent_window = CapabilitySupport::Unsupported;
    let request =
        FileDialogRequest::new(FileDialogKind::OpenFile, FileDialogOptions::new()).unwrap();
    assert_eq!(
        capabilities.validate_request(&request),
        Err(FileDialogError::UnsupportedOption(
            FileDialogOption::ParentWindow
        ))
    );
}
