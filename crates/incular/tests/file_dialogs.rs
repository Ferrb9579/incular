#![cfg(feature = "desktop")]

use incular::prelude::*;

#[test]
fn facade_exposes_portable_file_dialog_vocabulary() {
    let filter = FileDialogFilter::extensions("documents", "Documents", ["txt", "md"])
        .expect("valid filter");
    let options = FileDialogOptions::new()
        .filters([filter])
        .expect("valid options")
        .suggested_directory("workspace");
    let request = FileDialogKind::OpenFile;

    assert_eq!(request, FileDialogKind::OpenFile);
    assert_eq!(options.filters_value()[0].id().as_str(), "documents");
}

async fn _typed_async_surface(
    service: &FileDialogService,
    options: FileDialogOptions,
) -> Result<Option<DocumentDescriptor>, FileDialogError> {
    service.open_file(options).await
}

fn _explicit_request_surface(
    service: &FileDialogService,
    options: FileDialogOptions,
) -> Result<FileDialogRequest<Option<DocumentDescriptor>>, FileDialogError> {
    service.request_open_file(options)
}
