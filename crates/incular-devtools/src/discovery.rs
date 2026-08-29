//! Per-user session discovery. DevTools-enabled targets write one JSON file
//! under the user's data directory; the DevTools launcher lists them and
//! prunes records whose processes have exited.

use incular_devtools_protocol::DiscoveryRecord;
use std::path::PathBuf;

fn sessions_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("dev", "incular", "incular-devtools")
        .map(|dirs| dirs.data_dir().join("sessions"))
}

fn session_path(port: u16) -> Option<PathBuf> {
    sessions_dir().map(|dir| dir.join(format!("session-{port}.json")))
}

#[derive(Clone, Debug)]
pub struct DiscoveryFileEntry {
    pub pid: u32,
    pub app_name: String,
    pub port: u16,
    pub auth_token: String,
    pub started_unix_ms: u64,
    pub protocol_version: u32,
}

/// Writes/refreshes this target's discovery record (atomic replace).
pub fn register_session(entry: DiscoveryFileEntry) {
    let Some(path) = session_path(entry.port) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let record = DiscoveryRecord {
        pid: entry.pid,
        app_name: entry.app_name,
        port: entry.port,
        auth_token: entry.auth_token,
        started_unix_ms: entry.started_unix_ms,
        protocol_version: entry.protocol_version,
    };
    if let Ok(json) = serde_json::to_string(&record) {
        let temporary = path.with_extension("tmp");
        if std::fs::write(&temporary, json).is_ok() {
            let _ = std::fs::rename(&temporary, &path);
        }
    }
}

pub fn remove_session(port: u16) {
    if let Some(path) = session_path(port) {
        let _ = std::fs::remove_file(path);
    }
}

/// Lists live sessions, pruning records whose PIDs no longer exist.
pub fn list_sessions() -> Vec<(DiscoveryRecord, PathBuf)> {
    let mut out = Vec::new();
    let Some(dir) = sessions_dir() else {
        return out;
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            let _ = std::fs::remove_file(&path);
            continue;
        };
        match serde_json::from_str::<DiscoveryRecord>(&text) {
            Ok(record) => {
                if process_alive(record.pid) {
                    out.push((record, path));
                } else {
                    // Stale record after a crash: prune it.
                    let _ = std::fs::remove_file(&path);
                }
            }
            Err(_) => {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    out.sort_by_key(|(record, _)| record.started_unix_ms);
    out
}

#[cfg(target_os = "linux")]
fn process_alive(pid: u32) -> bool {
    std::path::Path::new(&format!("/proc/{pid}")).exists()
}

#[cfg(not(target_os = "linux"))]
fn process_alive(_pid: u32) -> bool {
    // There is no portable process-existence API in the standard library on
    // these targets. The discovery directory is user-scoped and the target
    // still authenticates the WebSocket before any data is exchanged; stale
    // records are therefore rejected and removed by the connection path.
    true
}
