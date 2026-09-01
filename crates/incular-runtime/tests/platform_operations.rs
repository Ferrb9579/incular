use incular_config::ContentSensitivity;
use incular_core::{Color, Size};
use incular_platform::{
    CapabilitySupport, NativeOperationCompletion, PlatformCapabilities, PlatformOperationError,
    PlatformOperationErrorKind, WindowOperation, WindowOptions,
};
use incular_runtime::{
    Application, NativeOperationCompletionStatus, NativeWindowCommand, WindowCommandEnqueueError,
};
use incular_widgets::Widget;

fn application() -> Application {
    Application::new(|_| Widget::box_(Size::new(1., 1.), Color::WHITE)).expect("application")
}

fn window_options(title: &str) -> WindowOptions {
    WindowOptions {
        title: title.to_owned(),
        initial_logical_size: Size::new(160., 90.),
        ..WindowOptions::default()
    }
}

#[test]
fn capability_snapshots_propagate_to_existing_and_future_window_handles() {
    let mut application = application();
    assert_eq!(
        application.platform_capabilities().window.set_title,
        CapabilitySupport::Unknown
    );
    let existing = application
        .open_window(
            window_options("existing"),
            Widget::box_(Size::new(1., 1.), Color::WHITE),
        )
        .expect("existing window");

    let mut capabilities = PlatformCapabilities::unsupported();
    capabilities.window.set_title = CapabilitySupport::Supported;
    capabilities.data_transfer.clipboard_text = CapabilitySupport::Supported;
    application.set_platform_capabilities(capabilities);

    assert_eq!(existing.capabilities(), capabilities);
    assert_eq!(
        application.window_capabilities(existing.id()),
        Some(capabilities)
    );

    let future = application
        .open_window(
            window_options("future"),
            Widget::box_(Size::new(1., 1.), Color::WHITE),
        )
        .expect("future window");
    assert_eq!(future.capabilities(), capabilities);

    let mut per_window = capabilities;
    per_window.window.content_sensitivity = CapabilitySupport::Supported;
    assert!(application.set_window_capabilities(existing.id(), per_window));
    assert_eq!(existing.capabilities(), per_window);
    assert_eq!(future.capabilities(), capabilities);
    assert_eq!(application.platform_capabilities(), capabilities);
}

#[test]
fn stopped_command_bridge_returns_typed_enqueue_error() {
    let mut application = application();
    let handle = application
        .open_window(
            window_options("target"),
            Widget::box_(Size::new(1., 1.), Color::WHITE),
        )
        .expect("window");
    application.shutdown();

    assert_eq!(
        handle.set_title("too late"),
        Err(WindowCommandEnqueueError::RuntimeStopped)
    );
    assert_eq!(
        handle.request_logical_size(Size::new(200., 100.)),
        Err(WindowCommandEnqueueError::RuntimeStopped)
    );
    assert!(matches!(
        handle.set_content_sensitivity(ContentSensitivity::Sensitive),
        Err(WindowCommandEnqueueError::RuntimeStopped)
    ));
}

#[test]
fn unsupported_native_request_resolves_once_and_is_visible_in_diagnostics() {
    let mut application = application();
    let handle = application
        .open_window(
            window_options("privacy"),
            Widget::box_(Size::new(1., 1.), Color::WHITE),
        )
        .expect("window");
    let _ = application.take_native_window_commands();

    let mut request = handle
        .set_content_sensitivity(ContentSensitivity::Sensitive)
        .expect("enqueue request");
    let request_id = request.id();
    let commands = application.take_native_window_commands();
    assert!(commands.iter().any(|command| matches!(
        command,
        NativeWindowCommand::Operate(command)
            if command.window_id == handle.id()
                && command.request_id == Some(request_id)
                && matches!(command.operation, WindowOperation::SetContentSensitivity(ContentSensitivity::Sensitive))
    )));
    assert_eq!(application.diagnostics().pending_native_requests, 1);

    let error = PlatformOperationError::with_context(
        PlatformOperationErrorKind::Unsupported,
        "test backend has no capture protection",
    );
    assert_eq!(
        application.complete_native_operation(NativeOperationCompletion::new(
            handle.id(),
            request_id,
            Err(error.clone()),
        )),
        NativeOperationCompletionStatus::Completed
    );
    assert_eq!(request.try_result(), Some(Err(error.clone())));
    assert_eq!(application.diagnostics().pending_native_requests, 0);
    assert_eq!(
        application
            .window_diagnostics(handle.id())
            .expect("window diagnostics")
            .last_platform_error,
        Some(error.clone())
    );

    assert_eq!(
        application.complete_native_operation(NativeOperationCompletion::new(
            handle.id(),
            request_id,
            Ok(()),
        )),
        NativeOperationCompletionStatus::UnknownRequest,
        "a request can complete at most once"
    );
}

#[test]
fn closed_and_reused_window_rejects_pending_stale_and_late_completions() {
    let mut application = application();
    let stale = application
        .open_window(
            window_options("old"),
            Widget::box_(Size::new(1., 1.), Color::WHITE),
        )
        .expect("old window");
    let stale_id = stale.id();
    let _ = application.take_native_window_commands();

    let mut pending = stale
        .set_content_sensitivity(ContentSensitivity::Sensitive)
        .expect("enqueue pending request");
    let pending_id = pending.id();
    let _ = application.take_native_window_commands();
    assert_eq!(application.diagnostics().pending_native_requests, 1);
    assert!(application.close_window(stale_id));
    assert_eq!(
        pending.try_result(),
        Some(Err(PlatformOperationError::stale_resource()))
    );

    let replacement = application
        .open_window(
            window_options("replacement"),
            Widget::box_(Size::new(1., 1.), Color::WHITE),
        )
        .expect("replacement");
    assert_eq!(replacement.id().index(), stale_id.index());
    assert_ne!(replacement.id().generation(), stale_id.generation());

    assert_eq!(
        application.complete_native_operation(NativeOperationCompletion::new(
            stale_id,
            pending_id,
            Ok(()),
        )),
        NativeOperationCompletionStatus::UnknownRequest
    );
    assert_eq!(
        application
            .window_diagnostics(replacement.id())
            .expect("replacement diagnostics")
            .title,
        "replacement"
    );

    let mut stale_after_reuse = stale
        .set_content_sensitivity(ContentSensitivity::AutoSensitive)
        .expect("stale handle can enqueue before runtime validates generation");
    let commands = application.take_native_window_commands();
    assert!(!commands.iter().any(|command| matches!(
        command,
        NativeWindowCommand::Operate(command)
            if command.request_id == Some(stale_after_reuse.id())
    )));
    assert_eq!(
        stale_after_reuse.try_result(),
        Some(Err(PlatformOperationError::stale_resource()))
    );
}

#[test]
fn concurrent_window_requests_resolve_by_request_and_target_not_delivery_order() {
    let mut application = application();
    let first = application
        .open_window(
            window_options("first"),
            Widget::box_(Size::new(1., 1.), Color::WHITE),
        )
        .expect("first");
    let second = application
        .open_window(
            window_options("second"),
            Widget::box_(Size::new(1., 1.), Color::WHITE),
        )
        .expect("second");
    let _ = application.take_native_window_commands();

    let mut first_request = first
        .set_content_sensitivity(ContentSensitivity::Sensitive)
        .expect("first request");
    let mut second_request = second
        .set_content_sensitivity(ContentSensitivity::Sensitive)
        .expect("second request");
    assert_ne!(first_request.id(), second_request.id());
    let _ = application.take_native_window_commands();
    assert_eq!(application.diagnostics().pending_native_requests, 2);

    let rejected = PlatformOperationError::new(PlatformOperationErrorKind::RejectedByPlatform);
    assert_eq!(
        application.complete_native_operation(NativeOperationCompletion::new(
            second.id(),
            second_request.id(),
            Err(rejected.clone()),
        )),
        NativeOperationCompletionStatus::Completed
    );
    assert_eq!(
        application.complete_native_operation(NativeOperationCompletion::new(
            first.id(),
            first_request.id(),
            Ok(()),
        )),
        NativeOperationCompletionStatus::Completed
    );

    assert_eq!(first_request.try_result(), Some(Ok(())));
    assert_eq!(second_request.try_result(), Some(Err(rejected)));
    assert_eq!(application.diagnostics().pending_native_requests, 0);
}

#[test]
fn target_mismatch_does_not_consume_the_real_pending_request() {
    let mut application = application();
    let first = application
        .open_window(
            window_options("first"),
            Widget::box_(Size::new(1., 1.), Color::WHITE),
        )
        .expect("first");
    let second = application
        .open_window(
            window_options("second"),
            Widget::box_(Size::new(1., 1.), Color::WHITE),
        )
        .expect("second");
    let _ = application.take_native_window_commands();
    let mut request = first
        .set_content_sensitivity(ContentSensitivity::Sensitive)
        .expect("request");
    let _ = application.take_native_window_commands();

    assert_eq!(
        application.complete_native_operation(NativeOperationCompletion::new(
            second.id(),
            request.id(),
            Ok(()),
        )),
        NativeOperationCompletionStatus::TargetMismatch
    );
    assert_eq!(application.diagnostics().pending_native_requests, 1);
    assert!(request.try_result().is_none());

    assert_eq!(
        application.complete_native_operation(NativeOperationCompletion::new(
            first.id(),
            request.id(),
            Ok(()),
        )),
        NativeOperationCompletionStatus::Completed
    );
    assert_eq!(request.try_result(), Some(Ok(())));
}

#[test]
fn dropping_request_handle_removes_pending_registration_without_cancelling_operation() {
    let mut application = application();
    let handle = application
        .open_window(
            window_options("drop-result"),
            Widget::box_(Size::new(1., 1.), Color::WHITE),
        )
        .expect("window");
    let _ = application.take_native_window_commands();
    let request = handle
        .set_content_sensitivity(ContentSensitivity::Sensitive)
        .expect("request");
    let request_id = request.id();
    let commands = application.take_native_window_commands();
    assert!(commands.iter().any(|command| matches!(
        command,
        NativeWindowCommand::Operate(command) if command.request_id == Some(request_id)
    )));
    assert_eq!(application.diagnostics().pending_native_requests, 1);

    drop(request);
    application.process_runtime_work();
    assert_eq!(application.diagnostics().pending_native_requests, 0);
    assert_eq!(
        application.complete_native_operation(NativeOperationCompletion::new(
            handle.id(),
            request_id,
            Ok(()),
        )),
        NativeOperationCompletionStatus::UnknownRequest
    );
}
