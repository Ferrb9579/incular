use incular_core::{Code, Color, Modifiers, Size};
use incular_platform::*;
use incular_runtime::*;
use incular_widgets::Widget;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};

fn application() -> Application {
    Application::new(|_| Widget::box_(Size::new(1., 1.), Color::WHITE)).unwrap()
}

trait ServiceCase {
    type Completion: Clone;
    fn new() -> Self;
    fn dispatch(&mut self, success: bool) -> Self::Completion;
    fn complete(&mut self, completion: Self::Completion) -> bool;
    fn consume(&mut self) -> bool;
    fn stop(&mut self);
}

fn lifecycle_contract<C: ServiceCase>() {
    for success in [true, false] {
        let mut case = C::new();
        let completion = case.dispatch(success);
        assert!(case.complete(completion.clone()));
        assert!(
            !case.complete(completion),
            "duplicate completion must be ignored"
        );
        assert_eq!(case.consume(), success);
    }
    let mut case = C::new();
    let completion = case.dispatch(true);
    case.stop();
    assert!(
        !case.complete(completion),
        "late completion must not survive shutdown"
    );
    assert!(
        !case.consume(),
        "shutdown must resolve pending work with a typed failure"
    );
}

struct WindowCase {
    app: Application,
    request: NativeOperationRequest,
}
impl ServiceCase for WindowCase {
    type Completion = NativeOperationCompletion;
    fn new() -> Self {
        let app = application();
        let request = app
            .window_handle(app.primary_window())
            .unwrap()
            .set_content_sensitivity(incular_config::ContentSensitivity::Sensitive)
            .unwrap();
        Self { app, request }
    }
    fn dispatch(&mut self, success: bool) -> Self::Completion {
        self.app.take_native_window_commands();
        NativeOperationCompletion::new(
            self.app.primary_window(),
            self.request.id(),
            if success {
                Ok(())
            } else {
                Err(PlatformOperationError::unavailable())
            },
        )
    }
    fn complete(&mut self, completion: Self::Completion) -> bool {
        self.app.complete_native_operation(completion) == NativeOperationCompletionStatus::Completed
    }
    fn consume(&mut self) -> bool {
        let success = self.request.try_result().expect("terminal reply").is_ok();
        assert!(self.request.try_result().is_none());
        success
    }
    fn stop(&mut self) {
        self.app.shutdown();
    }
}

struct DialogCase {
    app: Application,
    request: incular_runtime::FileDialogRequest<Option<DocumentDescriptor>>,
}
impl ServiceCase for DialogCase {
    type Completion = NativeFileDialogCompletion;
    fn new() -> Self {
        let app = application();
        let request = app
            .window_handle(app.primary_window())
            .unwrap()
            .file_dialogs()
            .request_open_file(FileDialogOptions::new())
            .unwrap();
        Self { app, request }
    }
    fn dispatch(&mut self, success: bool) -> Self::Completion {
        let native = self.app.take_native_file_dialog_requests().pop().unwrap();
        NativeFileDialogCompletion {
            request_id: native.request_id,
            window_id: native.window_id,
            result: if success {
                Ok(FileDialogOutcome::Cancelled)
            } else {
                Err(FileDialogError::ApplicationStopped)
            },
        }
    }
    fn complete(&mut self, completion: Self::Completion) -> bool {
        self.app.complete_file_dialog(completion) == FileDialogCompletionStatus::Completed
    }
    fn consume(&mut self) -> bool {
        let success = self.request.try_result().expect("terminal reply").is_ok();
        assert!(self.request.try_result().is_none());
        success
    }
    fn stop(&mut self) {
        self.app.shutdown();
    }
}

struct ShortcutCase {
    app: Application,
    request: GlobalShortcutRegistrationRequest,
}
impl ServiceCase for ShortcutCase {
    type Completion = NativeGlobalShortcutCompletion;
    fn new() -> Self {
        let app = application();
        let chord = GlobalShortcutChord::new(Code::KeyR, Modifiers::CONTROL).unwrap();
        let request = app
            .global_shortcuts()
            .register(GlobalShortcutId::new(17), chord)
            .unwrap();
        Self { app, request }
    }
    fn dispatch(&mut self, success: bool) -> Self::Completion {
        let native = self
            .app
            .take_native_global_shortcut_requests()
            .pop()
            .unwrap();
        NativeGlobalShortcutCompletion {
            request_id: native.request_id,
            result: if success {
                Ok(())
            } else {
                Err(GlobalShortcutError::Unsupported)
            },
        }
    }
    fn complete(&mut self, completion: Self::Completion) -> bool {
        self.app.complete_global_shortcut_request(completion)
            == GlobalShortcutCompletionStatus::Completed
    }
    fn consume(&mut self) -> bool {
        match Pin::new(&mut self.request).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(result) => result.is_ok(),
            Poll::Pending => panic!("terminal reply missing"),
        }
    }
    fn stop(&mut self) {
        self.app.shutdown();
    }
}

struct ShellCase {
    app: Application,
    _tray: TrayItemHandle,
    replies: Vec<NativeApplicationShellCompletion>,
}
impl ServiceCase for ShellCase {
    type Completion = NativeApplicationShellCompletion;
    fn new() -> Self {
        let app = application();
        let tray = app
            .application_shell()
            .create_tray_item(TrayItemPresentation::new(), &[])
            .unwrap();
        Self {
            app,
            _tray: tray,
            replies: Vec::new(),
        }
    }
    fn dispatch(&mut self, success: bool) -> Self::Completion {
        let native = self
            .app
            .take_native_application_shell_requests()
            .pop()
            .unwrap();
        NativeApplicationShellCompletion {
            request_id: native.request_id,
            result: if success {
                Ok(())
            } else {
                Err(ApplicationShellError::NativeFailure("injected".into()))
            },
        }
    }
    fn complete(&mut self, completion: Self::Completion) -> bool {
        self.app.complete_application_shell_request(completion);
        let replies = self.app.application_shell().take_completions();
        let completed = !replies.is_empty();
        self.replies.extend(replies);
        completed
    }
    fn consume(&mut self) -> bool {
        self.replies.pop().expect("terminal reply").result.is_ok()
    }
    fn stop(&mut self) {
        self.app.shutdown();
        self.replies
            .extend(self.app.application_shell().take_completions());
    }
}

#[test]
fn windows_obey_the_shared_lifecycle_contract() {
    lifecycle_contract::<WindowCase>();
}
#[test]
fn dialogs_obey_the_shared_lifecycle_contract() {
    lifecycle_contract::<DialogCase>();
}
#[test]
fn shortcuts_obey_the_shared_lifecycle_contract() {
    lifecycle_contract::<ShortcutCase>();
}
#[test]
fn shell_obeys_the_shared_lifecycle_contract() {
    lifecycle_contract::<ShellCase>();
}

#[test]
fn dropped_successful_unread_shortcut_reply_releases_native_registration() {
    let mut case = ShortcutCase::new();
    let completion = case.dispatch(true);
    assert!(case.complete(completion));
    drop(case.request);
    let cleanup = case.app.take_native_global_shortcut_requests();
    assert_eq!(cleanup.len(), 1);
    assert!(matches!(
        cleanup[0].operation,
        NativeGlobalShortcutOperation::Unregister { .. }
    ));
}

#[test]
fn dropped_queued_shortcut_does_not_reach_the_native_host() {
    let mut case = ShortcutCase::new();
    case.app.process_runtime_work();
    drop(case.request);
    assert!(case.app.take_native_global_shortcut_requests().is_empty());
}

#[test]
fn queued_window_request_cannot_be_completed_before_native_dispatch() {
    let mut case = WindowCase::new();
    case.app.process_runtime_work();
    let completion =
        NativeOperationCompletion::new(case.app.primary_window(), case.request.id(), Ok(()));
    assert!(!case.complete(completion.clone()));
    assert!(case.request.try_result().is_none());
    case.app.take_native_window_commands();
    assert!(case.complete(completion));
    assert!(case.consume());
}

#[test]
fn shutdown_discards_window_operations_not_yet_handed_to_the_host() {
    let mut case = WindowCase::new();
    case.app.process_runtime_work();
    case.app.shutdown();
    assert!(case.app.take_native_window_commands().is_empty());
    assert!(!case.consume());
}
