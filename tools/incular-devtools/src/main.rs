//! Standalone Incular DevTools, built with Incular itself.
//!
//! The inspector retains data rows, not widget rows. A 100k-node target thus
//! keeps a compact id/depth index while the retained sliver viewport creates
//! only rows near the viewport.

use std::sync::{Arc, Mutex};

mod inspector;
mod performance;
mod session;
#[cfg(test)]
mod tests;
mod transport;
mod views;

#[cfg(test)]
pub(crate) use inspector::{InspectorModel, TreeRow, editable_value, parse_debug_value};
#[cfg(test)]
pub(crate) use performance::{flamegraph_boxes, rank_traces};
#[cfg(test)]
pub(crate) use session::{requested_target_pid, select_session};
#[cfg(test)]
pub(crate) use views::{
    APP_BACKGROUND, BORDER, SURFACE, TEXT_PRIMARY, ToolView, compact_button, gap, ui_text,
};

fn main() {
    let sessions = session::list_sessions();
    let target_pid = session::requested_target_pid(std::env::args_os());
    let Some(record) = session::select_session(&sessions, target_pid) else {
        if let Some(pid) = target_pid {
            eprintln!("no live Incular DevTools target found for pid {pid}");
        } else {
            eprintln!("no DevTools targets discovered; run a devtools-enabled app with --devtools");
        }
        return;
    };
    let shared = Arc::new(Mutex::new(inspector::InspectorModel::default()));
    let bridge = transport::start_client(record, Arc::clone(&shared));
    views::run(shared, bridge);
}
