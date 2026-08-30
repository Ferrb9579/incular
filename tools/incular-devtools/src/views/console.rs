use super::{DANGER, TEXT_MUTED, TEXT_PRIMARY, compact_button, gap, section, ui_text};
use crate::inspector::{ConsoleEntry, Shared};
use incular::controls::TextField;
use incular::prelude::*;
use incular::widgets::internal::TextEditingController;

pub(crate) fn build_console(
    entries: Vec<ConsoleEntry>,
    shared: Shared,
    tick: Signal<u64>,
    filter: TextEditingController,
) -> Widget {
    let mut controls = Vec::new();
    let mut body = Vec::new();
    let submit_shared = shared.clone();
    let submit_tick = tick.clone();
    let filter_field: Widget = TextField::new(filter)
        .placeholder("Filter console; press Enter")
        .on_submit(move |query| {
            if let Ok(mut state) = submit_shared.lock() {
                state.console_filter = query;
            }
            submit_tick.update(|value| *value = value.wrapping_add(1));
        })
        .into();
    controls.push(filter_field.clone());

    let clear_shared = shared.clone();
    let clear_tick = tick.clone();
    controls.push(compact_button("Clear console", false, move || {
        if let Ok(mut state) = clear_shared.lock() {
            state.console.clear();
        }
        clear_tick.update(|value| *value = value.wrapping_add(1));
    }));
    body.extend(entries.into_iter().map(|entry| {
        let color = match entry.level.to_ascii_lowercase().as_str() {
            "error" | "fatal" => DANGER,
            "warn" | "warning" => incular::core::Color::rgba(242, 188, 64, 255),
            "debug" | "trace" => TEXT_MUTED,
            _ => TEXT_PRIMARY,
        };
        ui_text(
            format!("[{}] {} · {}", entry.level, entry.target, entry.message),
            13.,
            color,
        )
    }));
    if body.is_empty() {
        body.push(ui_text(
            "No target log events have been reported.",
            13.,
            TEXT_MUTED,
        ));
        body.push(ui_text(
            "Console captures structured events emitted through the DevTools channel; application stdout is not intercepted.",
            12.,
            TEXT_MUTED,
        ));
    }

    Column::new([
        ui_text("Console", 22., TEXT_PRIMARY),
        ui_text(
            "Read structured target diagnostics and DevTools connection errors.",
            13.,
            TEXT_MUTED,
        ),
        gap(1., 16.),
        section(
            "Messages",
            "Newest messages are retained in a bounded history",
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
