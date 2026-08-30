use incular_devtools_protocol::DiscoveryRecord;

pub(crate) fn sessions_dir() -> Option<std::path::PathBuf> {
    directories::ProjectDirs::from("dev", "incular", "incular-devtools")
        .map(|dirs| dirs.data_dir().join("sessions"))
}

pub(crate) fn list_sessions() -> Vec<DiscoveryRecord> {
    let mut sessions = Vec::new();
    let Some(dir) = sessions_dir() else {
        return sessions;
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return sessions;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        match std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<DiscoveryRecord>(&text).ok())
        {
            Some(record) if process_alive(record.pid) => sessions.push(record),
            _ => {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    sessions.sort_by_key(|record| record.started_unix_ms);
    sessions
}

#[cfg(target_os = "linux")]
pub(crate) fn process_alive(pid: u32) -> bool {
    std::path::Path::new(&format!("/proc/{pid}")).exists()
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn process_alive(_pid: u32) -> bool {
    // There is no portable process-existence API in the standard library on
    // these targets. The discovery directory is user-scoped and the target
    // still authenticates the WebSocket before any data is exchanged; stale
    // records are therefore rejected and removed by the connection path.
    true
}

pub fn requested_target_pid(
    args: impl IntoIterator<Item = impl AsRef<std::ffi::OsStr>>,
) -> Option<u32> {
    let mut arguments = args.into_iter();
    while let Some(argument) = arguments.next() {
        if argument.as_ref() == "--target-pid"
            && let Some(pid) = arguments
                .next()
                .and_then(|value| value.as_ref().to_str()?.parse().ok())
        {
            return Some(pid);
        }
    }
    None
}

pub fn select_session(
    sessions: &[DiscoveryRecord],
    target_pid: Option<u32>,
) -> Option<DiscoveryRecord> {
    target_pid.map_or_else(
        || sessions.last().cloned(),
        |pid| sessions.iter().find(|record| record.pid == pid).cloned(),
    )
}
