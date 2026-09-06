//! Native file-dialog adapters.
//!
//! The portable runtime owns request identity, serialization and lifecycle.
//! This module does only the native translation: it associates a request with
//! the concrete Winit parent, executes the platform dialog asynchronously, and
//! sends a portable completion back through Winit's event-loop proxy.

#[cfg(target_os = "linux")]
use crate::winit_adapter::raw_window_handles;
use incular_platform::{
    CapabilitySupport, FileDialogCapabilities, FileDialogError, FileDialogOutcome,
    NativeWindowSystem,
};
#[cfg(any(target_os = "windows", target_os = "macos"))]
use incular_platform::{
    DocumentActivation, FileDialogFilter, FileDialogKind, FileDialogRequest, FileDialogSelection,
};
use incular_runtime::{NativeFileDialogCompletion, NativeFileDialogRequest, TokioHandle};
use winit::{event_loop::EventLoopProxy, window::Window};

use crate::RuntimeWakeEvent;

#[cfg(target_os = "linux")]
mod linux;

#[must_use]
#[doc(hidden)]
pub fn desktop_file_dialog_capabilities(
    system: Option<NativeWindowSystem>,
) -> FileDialogCapabilities {
    let mut capabilities = FileDialogCapabilities {
        open_file: CapabilitySupport::Supported,
        open_files: CapabilitySupport::Supported,
        save_file: CapabilitySupport::Supported,
        select_folder: CapabilitySupport::Supported,
        extension_filters: CapabilitySupport::Supported,
        mime_type_filters: if matches!(
            system,
            Some(NativeWindowSystem::X11 | NativeWindowSystem::Wayland)
        ) {
            CapabilitySupport::Supported
        } else {
            CapabilitySupport::Unsupported
        },
        type_identifier_filters: CapabilitySupport::Unsupported,
        suggested_file_name: CapabilitySupport::Supported,
        suggested_directory: CapabilitySupport::Supported,
        parent_window: CapabilitySupport::Unknown,
    };

    capabilities.parent_window = match system {
        Some(
            NativeWindowSystem::Win32
            | NativeWindowSystem::AppKit
            | NativeWindowSystem::X11
            | NativeWindowSystem::Wayland,
        ) => CapabilitySupport::Supported,
        Some(NativeWindowSystem::Other) => CapabilitySupport::Unsupported,
        None => CapabilitySupport::Unknown,
    };
    capabilities
}

/// Starts one request after the runtime has granted this window's native modal
/// slot. The builder is created while the Winit parent is unquestionably live;
/// only backend-owned/copyable parent identity crosses into asynchronous work.
pub(crate) fn start_native_file_dialog(
    request: NativeFileDialogRequest,
    parent: &Window,
    tokio: TokioHandle,
    proxy: EventLoopProxy<RuntimeWakeEvent>,
) {
    let capabilities =
        desktop_file_dialog_capabilities(Some(crate::winit_adapter::native_window_system(parent)));
    if let Err(error) = capabilities.validate_request(&request.request) {
        send_completion(&proxy, request, Err(error));
        return;
    }

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    start_native_dialog(request, parent, tokio, proxy);

    #[cfg(target_os = "linux")]
    start_portal_dialog(request, parent, tokio, proxy);

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let _ = (parent, tokio);
        let kind = request.request.kind;
        send_completion(
            &proxy,
            request,
            Err(FileDialogError::UnsupportedOperation(kind)),
        );
    }
}

fn send_completion(
    proxy: &EventLoopProxy<RuntimeWakeEvent>,
    request: NativeFileDialogRequest,
    result: Result<FileDialogOutcome, FileDialogError>,
) {
    let _ = proxy.send_event(RuntimeWakeEvent::FileDialog(NativeFileDialogCompletion {
        request_id: request.request_id,
        window_id: request.window_id,
        result,
    }));
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn default_title(kind: FileDialogKind) -> &'static str {
    match kind {
        FileDialogKind::OpenFile => "Open File",
        FileDialogKind::OpenFiles => "Open Files",
        FileDialogKind::SaveFile => "Save File",
        FileDialogKind::SelectFolder => "Select Folder",
    }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn filter_label(filter: &FileDialogFilter) -> &str {
    filter.label().unwrap_or_else(|| filter.id().as_str())
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn native_dialog_builder(
    request: &FileDialogRequest,
    parent: &Window,
) -> native_dialog::FileDialogBuilder {
    let mut builder = native_dialog::DialogBuilder::file()
        .set_owner(parent)
        .set_title(
            request
                .options
                .title_value()
                .unwrap_or_else(|| default_title(request.kind)),
        );
    if let Some(directory) = request.options.suggested_directory_value() {
        builder = builder.set_location(directory);
    }
    if let Some(name) = request.options.suggested_file_name_value() {
        builder = builder.set_filename(name);
    }
    for filter in request.options.filters_value() {
        let extensions = filter
            .extensions_list()
            .iter()
            .map(|extension| extension.as_str().to_owned())
            .collect::<Vec<_>>();
        builder = builder.add_filter(filter_label(filter), &extensions);
    }
    builder
}

#[cfg(target_os = "windows")]
fn start_native_dialog(
    request: NativeFileDialogRequest,
    parent: &Window,
    tokio: TokioHandle,
    proxy: EventLoopProxy<RuntimeWakeEvent>,
) {
    // native-dialog's Win32 async facade calls the synchronous COM dialog from
    // its first poll. Put it on Tokio's blocking pool rather than occupying a
    // Tokio worker while the user interacts with the modal dialog.
    let builder = native_dialog_builder(&request.request, parent);
    tokio.spawn_blocking(move || {
        let result = match request.request.kind {
            FileDialogKind::OpenFile => builder
                .open_single_file()
                .show()
                .map(single_document_outcome),
            FileDialogKind::OpenFiles => builder
                .open_multiple_file()
                .show()
                .map(multiple_document_outcome),
            FileDialogKind::SaveFile => builder
                .save_single_file()
                .show()
                .map(single_document_outcome),
            FileDialogKind::SelectFolder => builder.open_single_dir().show().map(folder_outcome),
        }
        .map_err(|error| FileDialogError::Backend(error.to_string()));
        send_completion(&proxy, request, result);
    });
}

#[cfg(target_os = "macos")]
fn start_native_dialog(
    request: NativeFileDialogRequest,
    parent: &Window,
    tokio: TokioHandle,
    proxy: EventLoopProxy<RuntimeWakeEvent>,
) {
    // native-dialog dispatches AppKit panel creation/presentation to the main
    // queue and its async API waits without blocking Tokio's worker thread.
    let builder = native_dialog_builder(&request.request, parent);
    tokio.spawn(async move {
        let result = match request.request.kind {
            FileDialogKind::OpenFile => builder
                .open_single_file()
                .spawn()
                .await
                .map(single_document_outcome),
            FileDialogKind::OpenFiles => builder
                .open_multiple_file()
                .spawn()
                .await
                .map(multiple_document_outcome),
            FileDialogKind::SaveFile => builder
                .save_single_file()
                .spawn()
                .await
                .map(single_document_outcome),
            FileDialogKind::SelectFolder => {
                builder.open_single_dir().spawn().await.map(folder_outcome)
            }
        }
        .map_err(|error| FileDialogError::Backend(error.to_string()));
        send_completion(&proxy, request, result);
    });
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn single_document_outcome(path: Option<std::path::PathBuf>) -> FileDialogOutcome {
    match path {
        Some(path) => FileDialogOutcome::Selected(FileDialogSelection::Documents(
            DocumentActivation::from_paths([path]),
        )),
        None => FileDialogOutcome::Cancelled,
    }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn multiple_document_outcome(paths: Vec<std::path::PathBuf>) -> FileDialogOutcome {
    if paths.is_empty() {
        FileDialogOutcome::Cancelled
    } else {
        FileDialogOutcome::Selected(FileDialogSelection::Documents(
            DocumentActivation::from_paths(paths),
        ))
    }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn folder_outcome(path: Option<std::path::PathBuf>) -> FileDialogOutcome {
    path.map_or(FileDialogOutcome::Cancelled, |path| {
        FileDialogOutcome::Selected(FileDialogSelection::Folder(path))
    })
}

#[cfg(target_os = "linux")]
fn start_portal_dialog(
    request: NativeFileDialogRequest,
    parent: &Window,
    tokio: TokioHandle,
    proxy: EventLoopProxy<RuntimeWakeEvent>,
) {
    let handles = raw_window_handles(parent);
    tokio.spawn(async move {
        let result = linux::run_portal_dialog(&request.request, handles).await;
        send_completion(&proxy, request, result);
    });
}
