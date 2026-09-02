//! Linux XDG FileChooser portal translation.
//!
//! This module intentionally depends only on the portable platform contract and
//! portal crates. Keeping it free of renderer/runtime dependencies makes the
//! native translation independently cross-checkable even on non-Linux hosts.

use ashpd::desktop::file_chooser::{FileFilter, SelectedFiles};
use incular_platform::{
    DocumentActivation, FileContentTypeKind, FileDialogError, FileDialogKind, FileDialogOption,
    FileDialogOutcome, FileDialogRequest, FileDialogSelection, RawWindowHandles,
};

pub(crate) async fn run_portal_dialog(
    request: &FileDialogRequest,
    handles: RawWindowHandles,
) -> Result<FileDialogOutcome, FileDialogError> {
    let identifier =
        ashpd::WindowIdentifier::from_raw_handle(&handles.window, handles.display.as_ref())
            .await
            .ok_or_else(|| {
                FileDialogError::Backend(
                    "XDG portal parent-window identifier could not be created".to_owned(),
                )
            })?;

    let title = request
        .options
        .title_value()
        .unwrap_or_else(|| default_title(request.kind));
    let filters = request
        .options
        .filters_value()
        .iter()
        .map(native_filter)
        .collect::<Result<Vec<_>, _>>()?;

    match request.kind {
        FileDialogKind::OpenFile | FileDialogKind::OpenFiles | FileDialogKind::SelectFolder => {
            let mut portal = SelectedFiles::open_file()
                .identifier(identifier)
                .title(title)
                .modal(true)
                .multiple(request.kind == FileDialogKind::OpenFiles)
                .directory(request.kind == FileDialogKind::SelectFolder)
                .filters(filters);
            if let Some(directory) = request.options.suggested_directory_value() {
                portal = portal
                    .current_folder(directory)
                    .map_err(|error| FileDialogError::Backend(error.to_string()))?;
            }
            let response = match portal.send().await {
                Ok(response) => response.response(),
                Err(error) => Err(error),
            };
            let Some(selected) = portal_result(response)? else {
                return Ok(FileDialogOutcome::Cancelled);
            };
            portal_selection_outcome(request.kind, selected.uris())
        }
        FileDialogKind::SaveFile => {
            let mut portal = SelectedFiles::save_file()
                .identifier(identifier)
                .title(title)
                .modal(true)
                .filters(filters)
                .current_name(request.options.suggested_file_name_value());
            if let Some(directory) = request.options.suggested_directory_value() {
                portal = portal
                    .current_folder(directory)
                    .map_err(|error| FileDialogError::Backend(error.to_string()))?;
            }
            let response = match portal.send().await {
                Ok(response) => response.response(),
                Err(error) => Err(error),
            };
            let Some(selected) = portal_result(response)? else {
                return Ok(FileDialogOutcome::Cancelled);
            };
            portal_selection_outcome(request.kind, selected.uris())
        }
    }
}

fn native_filter(
    filter: &incular_platform::FileDialogFilter,
) -> Result<FileFilter, FileDialogError> {
    let mut native = FileFilter::new(filter.label().unwrap_or_else(|| filter.id().as_str()));
    for extension in filter.extensions_list() {
        native = native.glob(&format!("*.{}", extension.as_str()));
    }
    for content_type in filter.content_types_list() {
        match content_type.kind() {
            FileContentTypeKind::Mime => {
                native = native.mimetype(content_type.as_str());
            }
            FileContentTypeKind::TypeIdentifier => {
                return Err(FileDialogError::UnsupportedOption(
                    FileDialogOption::TypeIdentifierFilters,
                ));
            }
        }
    }
    Ok(native)
}

fn portal_result<T>(result: Result<T, ashpd::Error>) -> Result<Option<T>, FileDialogError> {
    use ashpd::{PortalError, desktop::ResponseError};
    match result {
        Ok(value) => Ok(Some(value)),
        Err(
            ashpd::Error::Response(ResponseError::Cancelled)
            | ashpd::Error::Portal(PortalError::Cancelled(_)),
        ) => Ok(None),
        Err(ashpd::Error::Portal(PortalError::WindowDestroyed(_))) => {
            Err(FileDialogError::ParentClosed)
        }
        Err(error) => Err(FileDialogError::Backend(error.to_string())),
    }
}

fn portal_selection_outcome(
    kind: FileDialogKind,
    uris: &[ashpd::Uri],
) -> Result<FileDialogOutcome, FileDialogError> {
    let paths = uris
        .iter()
        .map(|uri| {
            url::Url::parse(uri.as_str())
                .map_err(|error| FileDialogError::Backend(error.to_string()))?
                .to_file_path()
                .map_err(|_| FileDialogError::InvalidResult)
        })
        .collect::<Result<Vec<_>, _>>()?;
    match kind {
        FileDialogKind::OpenFile | FileDialogKind::SaveFile if paths.len() == 1 => {
            Ok(FileDialogOutcome::Selected(FileDialogSelection::Documents(
                DocumentActivation::from_paths(paths),
            )))
        }
        FileDialogKind::OpenFiles if !paths.is_empty() => Ok(FileDialogOutcome::Selected(
            FileDialogSelection::Documents(DocumentActivation::from_paths(paths)),
        )),
        FileDialogKind::SelectFolder if paths.len() == 1 => Ok(FileDialogOutcome::Selected(
            FileDialogSelection::Folder(paths.into_iter().next().expect("one portal path")),
        )),
        _ => Err(FileDialogError::InvalidResult),
    }
}

fn default_title(kind: FileDialogKind) -> &'static str {
    match kind {
        FileDialogKind::OpenFile => "Open File",
        FileDialogKind::OpenFiles => "Open Files",
        FileDialogKind::SaveFile => "Save File",
        FileDialogKind::SelectFolder => "Select Folder",
    }
}
