use incular_core::{Color, Size};
use incular_platform::{
    CapabilitySupport, DocumentActivation, DocumentDescriptor, FileDialogCapabilities,
    FileDialogError, FileDialogFilter, FileDialogOption, FileDialogOptions, FileDialogOutcome,
    FileDialogSelection, PlatformCapabilities, WindowOptions,
};
use incular_runtime::{Application, FileDialogCompletionStatus, MemoryFileDialogAdapter};
use incular_widgets::Widget;
use std::path::PathBuf;
use std::time::Duration;

fn application() -> Application {
    Application::new(|_| Widget::box_(Size::new(1.0, 1.0), Color::WHITE)).expect("application")
}

fn service(application: &Application) -> incular_runtime::FileDialogService {
    application
        .window_handle(application.primary_window())
        .expect("primary window handle")
        .file_dialogs()
}

#[test]
fn memory_adapter_round_trips_selection_user_cancel_and_detached_request() {
    let mut application = application();
    let dialogs = service(&application);
    let mut adapter = MemoryFileDialogAdapter::default();
    let selected_path = PathBuf::from(r"C:\work\report.txt");
    adapter.push_result(Ok(FileDialogOutcome::Selected(
        FileDialogSelection::Documents(DocumentActivation::from_paths([selected_path.clone()])),
    )));
    adapter.push_result(Ok(FileDialogOutcome::Cancelled));

    let mut selected = dialogs
        .request_open_file(FileDialogOptions::new())
        .expect("open file request");
    let mut native = application.take_native_file_dialog_requests();
    assert_eq!(native.len(), 1);
    let completion = adapter.respond(native.remove(0));
    assert_eq!(
        application.complete_file_dialog(completion),
        FileDialogCompletionStatus::Completed
    );
    assert_eq!(
        selected.try_result(),
        Some(Ok(Some(DocumentDescriptor::new(selected_path))))
    );

    let mut cancelled = dialogs
        .request_open_file(FileDialogOptions::new())
        .expect("cancelled request");
    let completion = adapter.respond(
        application
            .take_native_file_dialog_requests()
            .pop()
            .expect("native request"),
    );
    assert_eq!(
        application.complete_file_dialog(completion),
        FileDialogCompletionStatus::Completed
    );
    assert_eq!(cancelled.try_result(), Some(Ok(None)));

    let detached = dialogs
        .request_open_file(FileDialogOptions::new())
        .expect("detached request");
    drop(detached);
    assert!(application.take_native_file_dialog_requests().is_empty());
    assert_eq!(application.diagnostics().pending_file_dialog_requests, 0);
}

#[test]
fn same_parent_dialogs_are_fifo_while_distinct_windows_can_run_concurrently() {
    let mut application = application();
    let primary = application.primary_window();
    let primary_dialogs = service(&application);
    let second = application
        .open_window(
            WindowOptions::new("second"),
            Widget::box_(Size::new(1.0, 1.0), Color::WHITE),
        )
        .expect("second window");
    let second_dialogs = second.file_dialogs();
    let _ = application.take_native_window_commands();

    let mut first = primary_dialogs
        .request_open_file(FileDialogOptions::new())
        .expect("first");
    let mut second_for_primary = primary_dialogs
        .request_open_file(FileDialogOptions::new())
        .expect("second for primary");
    let mut other_window = second_dialogs
        .request_select_folder(FileDialogOptions::new())
        .expect("other window");

    let active = application.take_native_file_dialog_requests();
    assert_eq!(active.len(), 2, "one active request per parent window");
    let primary_native = active
        .iter()
        .find(|request| request.window_id == primary)
        .cloned()
        .expect("primary native request");
    let second_native = active
        .iter()
        .find(|request| request.window_id == second.id())
        .cloned()
        .expect("second-window native request");
    assert_eq!(primary_native.request_id, first.id());
    assert_eq!(second_native.request_id, other_window.id());
    assert_eq!(application.diagnostics().pending_file_dialog_requests, 3);

    assert_eq!(
        application.complete_file_dialog(incular_runtime::NativeFileDialogCompletion {
            request_id: primary_native.request_id,
            window_id: primary,
            result: Ok(FileDialogOutcome::Cancelled),
        }),
        FileDialogCompletionStatus::Completed
    );
    assert_eq!(first.try_result(), Some(Ok(None)));
    let promoted = application.take_native_file_dialog_requests();
    assert_eq!(promoted.len(), 1);
    assert_eq!(promoted[0].request_id, second_for_primary.id());

    assert_eq!(
        application.complete_file_dialog(incular_runtime::NativeFileDialogCompletion {
            request_id: promoted[0].request_id,
            window_id: primary,
            result: Ok(FileDialogOutcome::Cancelled),
        }),
        FileDialogCompletionStatus::Completed
    );
    assert_eq!(second_for_primary.try_result(), Some(Ok(None)));

    assert_eq!(
        application.complete_file_dialog(incular_runtime::NativeFileDialogCompletion {
            request_id: second_native.request_id,
            window_id: second.id(),
            result: Ok(FileDialogOutcome::Selected(FileDialogSelection::Folder(
                PathBuf::from(r"D:\workspace"),
            ))),
        }),
        FileDialogCompletionStatus::Completed
    );
    assert_eq!(
        other_window.try_result(),
        Some(Ok(Some(PathBuf::from(r"D:\workspace"))))
    );
}

#[test]
fn closing_parent_resolves_active_and_queued_requests_once_and_rejects_late_completion() {
    let mut application = application();
    let window = application.primary_window();
    let dialogs = service(&application);
    let mut active = dialogs
        .request_open_file(FileDialogOptions::new())
        .expect("active");
    let mut queued = dialogs
        .request_open_file(FileDialogOptions::new())
        .expect("queued");
    let native = application.take_native_file_dialog_requests();
    assert_eq!(native.len(), 1);
    let active_id = native[0].request_id;

    assert!(application.close_window(window));
    assert_eq!(
        active.try_result(),
        Some(Err(FileDialogError::ParentClosed))
    );
    assert_eq!(
        queued.try_result(),
        Some(Err(FileDialogError::ParentClosed))
    );
    assert_eq!(application.diagnostics().pending_file_dialog_requests, 0);
    assert_eq!(
        application.complete_file_dialog(incular_runtime::NativeFileDialogCompletion {
            request_id: active_id,
            window_id: window,
            result: Ok(FileDialogOutcome::Cancelled),
        }),
        FileDialogCompletionStatus::UnknownRequest
    );
    assert!(
        active.try_result().is_none(),
        "completion is delivered exactly once"
    );
    assert!(
        queued.try_result().is_none(),
        "completion is delivered exactly once"
    );
}

#[test]
fn unsupported_semantic_option_is_reported_without_native_execution() {
    let mut application = application();
    let window = application.primary_window();
    let mut capabilities = PlatformCapabilities::unsupported();
    capabilities.application_services.file_dialogs =
        FileDialogCapabilities::all(CapabilitySupport::Supported);
    capabilities
        .application_services
        .file_dialogs
        .mime_type_filters = CapabilitySupport::Unsupported;
    assert!(application.set_window_capabilities(window, capabilities));
    let dialogs = service(&application);
    let options = FileDialogOptions::new()
        .filters([FileDialogFilter::content_types("images", "Images", ["image/png"]).unwrap()])
        .unwrap();
    let mut request = dialogs
        .request_open_file(options)
        .expect("portable request is valid");

    assert!(application.take_native_file_dialog_requests().is_empty());
    assert_eq!(
        request.try_result(),
        Some(Err(FileDialogError::UnsupportedOption(
            FileDialogOption::MimeTypeFilters
        )))
    );
}

#[test]
fn save_result_preserves_backend_destination_exactly() {
    let mut application = application();
    let dialogs = service(&application);
    let exact = PathBuf::from(r"relative\draft.final.TXT");
    let mut request = dialogs
        .request_save_file(
            FileDialogOptions::new()
                .suggested_file_name("draft.txt")
                .unwrap(),
        )
        .expect("save request");
    let native = application
        .take_native_file_dialog_requests()
        .pop()
        .expect("native save request");
    assert_eq!(
        application.complete_file_dialog(incular_runtime::NativeFileDialogCompletion {
            request_id: native.request_id,
            window_id: native.window_id,
            result: Ok(FileDialogOutcome::Selected(FileDialogSelection::Documents(
                DocumentActivation::from_paths([exact.clone()])
            ),)),
        }),
        FileDialogCompletionStatus::Completed
    );
    assert_eq!(
        request.try_result(),
        Some(Ok(Some(DocumentDescriptor::new(exact))))
    );
}

#[test]
fn dropping_an_already_started_dialog_detaches_delivery_but_preserves_modal_serialization() {
    let mut application = application();
    let window = application.primary_window();
    let dialogs = service(&application);
    let started = dialogs
        .request_open_file(FileDialogOptions::new())
        .expect("started request");
    let mut next = dialogs
        .request_open_file(FileDialogOptions::new())
        .expect("queued request");
    let native = application.take_native_file_dialog_requests();
    assert_eq!(native.len(), 1);
    let started_native = native[0].clone();

    drop(started);
    application.process_runtime_work();
    assert!(
        application.take_native_file_dialog_requests().is_empty(),
        "dropping an already-started result must not create overlapping native dialogs"
    );
    assert_eq!(application.diagnostics().pending_file_dialog_requests, 2);

    assert_eq!(
        application.complete_file_dialog(incular_runtime::NativeFileDialogCompletion {
            request_id: started_native.request_id,
            window_id: window,
            result: Ok(FileDialogOutcome::Cancelled),
        }),
        FileDialogCompletionStatus::Completed
    );
    let promoted = application.take_native_file_dialog_requests();
    assert_eq!(promoted.len(), 1);
    assert_eq!(promoted[0].request_id, next.id());
    assert_eq!(
        application.complete_file_dialog(incular_runtime::NativeFileDialogCompletion {
            request_id: promoted[0].request_id,
            window_id: window,
            result: Ok(FileDialogOutcome::Cancelled),
        }),
        FileDialogCompletionStatus::Completed
    );
    assert_eq!(next.try_result(), Some(Ok(None)));
    assert_eq!(application.diagnostics().pending_file_dialog_requests, 0);
}

#[test]
fn async_convenience_api_resolves_without_exposing_native_details() {
    let mut application = application();
    let dialogs = service(&application);
    let selected_path = PathBuf::from(r"C:\workspace\async.txt");
    let (sender, receiver) = std::sync::mpsc::channel();
    application.tokio_handle().spawn(async move {
        let result = dialogs.open_file(FileDialogOptions::new()).await;
        let _ = sender.send(result);
    });

    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    let native = loop {
        if let Some(request) = application.take_native_file_dialog_requests().pop() {
            break request;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "async dialog request did not reach the native bridge"
        );
        std::thread::sleep(Duration::from_millis(1));
    };
    assert_eq!(
        application.complete_file_dialog(incular_runtime::NativeFileDialogCompletion {
            request_id: native.request_id,
            window_id: native.window_id,
            result: Ok(FileDialogOutcome::Selected(FileDialogSelection::Documents(
                DocumentActivation::from_paths([selected_path.clone(),])
            ),)),
        }),
        FileDialogCompletionStatus::Completed
    );

    assert_eq!(
        receiver.recv_timeout(Duration::from_secs(2)).unwrap(),
        Ok(Some(DocumentDescriptor::new(selected_path)))
    );
}
