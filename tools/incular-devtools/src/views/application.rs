use super::{DANGER, SUCCESS, TEXT_MUTED, TEXT_PRIMARY, compact_button, gap, section, ui_text};
use crate::transport::ClientBridge;
use incular::prelude::*;
use incular_devtools_protocol::{FrameRecordEvent, RequestMethod, TargetInfo, WindowSummary};

pub(crate) fn build_application(
    target_info: Option<TargetInfo>,
    connected: bool,
    windows: Vec<WindowSummary>,
    frames: Vec<FrameRecordEvent>,
    bridge: ClientBridge,
) -> Widget {
    let mut controls = Vec::new();
    let mut body = Vec::new();
    let windows_empty = windows.is_empty();
    controls.push(compact_button("Refresh target info", false, move || {
        bridge.send(RequestMethod::GetTargetInfo)
    }));
    if let Some(info) = target_info.as_ref() {
        body.extend([
            ui_text(
                format!("Framework {}", info.framework_version),
                14.,
                TEXT_PRIMARY,
            ),
            ui_text(
                format!("Process {} · {}", info.pid, info.executable),
                13.,
                TEXT_MUTED,
            ),
            ui_text(
                format!(
                    "Platform {} · protocol {}",
                    info.platform, info.protocol_version
                ),
                13.,
                TEXT_MUTED,
            ),
            ui_text(
                format!(
                    "DevTools session: {}",
                    if connected {
                        "connected"
                    } else {
                        "disconnected"
                    }
                ),
                13.,
                if connected { SUCCESS } else { DANGER },
            ),
        ]);
    } else {
        body.push(ui_text("Waiting for target information…", 13., TEXT_MUTED));
    }
    body.push(ui_text("Windows", 16., TEXT_PRIMARY));
    for window in windows {
        let observed = frames
            .iter()
            .filter(|frame| frame.window == window.id)
            .count();
        body.push(ui_text(
            format!(
                "{} · {:.0}×{:.0} logical · scale {:.2} · {} streamed frames",
                window.title,
                window.logical_size[0],
                window.logical_size[1],
                window.scale_factor,
                observed,
            ),
            13.,
            TEXT_MUTED,
        ));
    }
    if windows_empty {
        body.push(ui_text("No live windows reported.", 13., TEXT_MUTED));
    }

    Column::new([
        ui_text("Application", 22., TEXT_PRIMARY),
        ui_text(
            "Target identity, windows, protocol, and runtime session state.",
            13.,
            TEXT_MUTED,
        ),
        gap(1., 16.),
        section(
            "Target",
            "Connection and framework metadata",
            Column::new([
                Wrap::new(controls).spacing(8.).run_spacing(8.).into(),
                gap(1., 12.),
                Column::new(body).into(),
            ])
            .into(),
        ),
    ])
    .into()
}
