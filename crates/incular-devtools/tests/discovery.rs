use incular_devtools::discovery::{DiscoveryFileEntry, list_sessions, register_session};
use incular_devtools::session::generate_token;
use std::io::Write;
use std::path::PathBuf;

struct RemoveFile(PathBuf);

impl Drop for RemoveFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[test]
fn discovery_is_private_and_does_not_overwrite_predictable_temporary_files() {
    // Reserve the port throughout the test so this record cannot collide with a
    // live DevTools listener. No environment variables or other records change.
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let entry = DiscoveryFileEntry {
        pid: std::process::id(),
        app_name: "discovery security regression".into(),
        port: listener.local_addr().unwrap().port(),
        auth_token: generate_token().unwrap(),
        started_unix_ms: 1,
        protocol_version: incular_devtools_protocol::PROTOCOL_VERSION,
    };
    let original = register_session(entry.clone()).unwrap();
    let (_, path) = list_sessions()
        .into_iter()
        .find(|(record, _)| record.auth_token == entry.auth_token)
        .expect("new session is discoverable");
    let trap = RemoveFile(path.with_extension(format!("{}.tmp", std::process::id())));
    let mut sentinel = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&trap.0)
        .unwrap();
    sentinel.write_all(b"must not be overwritten").unwrap();
    drop(sentinel);
    let replacement_entry = DiscoveryFileEntry {
        auth_token: generate_token().unwrap(),
        ..entry
    };
    let replacement = register_session(replacement_entry.clone()).unwrap();
    assert_eq!(std::fs::read(&trap.0).unwrap(), b"must not be overwritten");
    drop(original);
    let record: incular_devtools_protocol::DiscoveryRecord =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(record.auth_token, replacement_entry.auth_token);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    drop(replacement);
    assert!(!path.exists());
}
