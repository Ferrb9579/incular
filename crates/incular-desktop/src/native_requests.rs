//! Domain request drains independent of frame scheduling and event routing.
use crate::{
    DesktopApplicationShellServices, global_shortcuts::DesktopGlobalShortcuts,
    window_host::NativeWindowState,
};
use incular_platform::{NativeWindowSystem, WindowId as IncularWindowId};
use incular_runtime::{
    Application, GlobalShortcutCompletionStatus, NativeApplicationShellCompletion,
    NativeApplicationShellOperation, NativeGlobalShortcutCompletion, NativeGlobalShortcutOperation,
};
use std::collections::HashMap;
use winit::window::WindowId as NativeWindowId;

pub(crate) fn drain_shortcuts(
    application: &mut Application,
    shortcuts: Option<&mut DesktopGlobalShortcuts>,
) {
    let Some(shortcuts) = shortcuts else {
        return;
    };
    for request in application.take_native_global_shortcut_requests() {
        let result = match request.operation {
            NativeGlobalShortcutOperation::Register { id, chord } => shortcuts.register(id, chord),
            NativeGlobalShortcutOperation::Unregister { id } => shortcuts.unregister(id),
        };
        let status = application.complete_global_shortcut_request(NativeGlobalShortcutCompletion {
            request_id: request.request_id,
            result,
        });
        if let GlobalShortcutCompletionStatus::AbandonedRegistration(id) = status {
            let _ = shortcuts.unregister(id);
        }
    }
}
pub(crate) fn drain_shell(
    application: &mut Application,
    services: &dyn DesktopApplicationShellServices,
    system: Option<NativeWindowSystem>,
    native_ids: &HashMap<IncularWindowId, NativeWindowId>,
    windows: &HashMap<NativeWindowId, NativeWindowState>,
) {
    let Some(system) = system else {
        return;
    };
    services.set_application_shell_notification_identity(
        application.application_shell().notification_identity(),
    );
    for request in application.take_native_application_shell_requests() {
        let request_id = request.request_id;
        let target_window = match &request.operation {
            NativeApplicationShellOperation::SetTaskbarDockState(state) => state
                .window_id
                .and_then(|id| native_ids.get(&id))
                .and_then(|native_id| windows.get(native_id))
                .map(|state| state.window.as_ref()),
            _ => None,
        };
        let result = services.apply_application_shell_request(system, request, target_window);
        if let incular_runtime::NativeApplicationShellApplyResult::Completed(result) = result {
            application.complete_application_shell_request(NativeApplicationShellCompletion {
                request_id,
                result,
            });
        }
    }
}
