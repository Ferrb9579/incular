//! Standalone Incular DevTools application and its testable model surfaces.

use std::sync::{Arc, Mutex};

pub mod inspector;
pub mod performance;
pub mod session;
mod transport;
pub mod views;

/// Runs the standalone DevTools application against the selected target.
pub fn run() {
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
