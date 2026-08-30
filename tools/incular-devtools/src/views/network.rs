use super::{TEXT_MUTED, TEXT_PRIMARY, gap, section, ui_text};
use incular::prelude::*;

pub(crate) fn build_network() -> Widget {
    Column::new([
        ui_text("Network", 22., TEXT_PRIMARY),
        ui_text(
            "Inspect application network activity and transfer costs.",
            13.,
            TEXT_MUTED,
        ),
        gap(1., 16.),
        section(
            "Requests",
            "Request instrumentation is opt-in at the application transport boundary",
            Column::new([
                ui_text("No application network requests recorded.", 16., TEXT_PRIMARY),
                ui_text(
                    "This target does not currently expose an HTTP/client transport to DevTools. When a network adapter is registered, requests will appear here with URL, status, type, duration, and transfer size.",
                    13.,
                    TEXT_MUTED,
                ),
                ui_text("Request   Status   Type   Duration   Size", 13., TEXT_MUTED),
            ])
            .into(),
        ),
    ])
    .into()
}
