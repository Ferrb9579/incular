//! Environment publication has no access to native request or GPU ownership.
use crate::{input::InputKind, window_host::NativeWindowState};
use incular_platform::{PlatformEvent, WindowEvent};
use incular_runtime::Application;
use std::collections::HashMap;
use winit::window::WindowId;

pub(crate) fn publish(
    application: &mut Application,
    windows: &HashMap<WindowId, NativeWindowState>,
    id: WindowId,
) {
    if let Some(state) = windows.get(&id) {
        application.handle_window_event(WindowEvent::platform(
            state.id,
            PlatformEvent::Environment(state.environment.snapshot(state.metrics)),
        ));
    }
}
pub(crate) fn note_input(
    application: &mut Application,
    windows: &mut HashMap<WindowId, NativeWindowState>,
    id: WindowId,
    kind: InputKind,
) {
    let Some(state) = windows.get_mut(&id) else {
        return;
    };
    let changed = match kind {
        InputKind::Mouse => state.environment.note_mouse(),
        InputKind::Touch => state.environment.note_touch(),
        InputKind::Stylus => state.environment.note_stylus(),
        InputKind::Keyboard => state.environment.note_keyboard(),
        InputKind::Trackpad => state.environment.note_trackpad(),
        InputKind::Other => false,
    };
    if changed {
        publish(application, windows, id);
    }
}
