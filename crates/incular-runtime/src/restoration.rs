//! Versioned, opt-in declarative application restoration.
//!
//! This module deliberately stores only application-supplied JSON values and
//! stable descriptors. Retained widgets, native windows, GPU objects, tasks,
//! callbacks, gestures, and semantics are never part of a snapshot.

use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    fmt,
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    rc::{Rc, Weak},
    sync::{Arc, Mutex},
    time::Duration,
};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;

use incular_core::{RestorationBackend, RestorationKey, RestorationScope};

use crate::{Signal, tasks};

/// The Incular-owned snapshot format. It is intentionally independent from
/// the application's own restoration schema version.
pub const FRAMEWORK_RESTORATION_FORMAT_VERSION: u32 = 1;

/// Normal local-state debounce. Writes are coalesced and happen on Tokio's
/// blocking pool, never on an ordinary UI mutation.
pub const DEFAULT_RESTORATION_DEBOUNCE: Duration = Duration::from_millis(250);

/// A deliberately generous bound for small declarative session state. This is
/// not a database or a place for files, images, credentials, or tokens.
pub const DEFAULT_RESTORATION_SNAPSHOT_LIMIT: usize = 8 * 1024 * 1024;

/// Error reported by a persistence store. Store errors are diagnostics, not
/// application-fatal failures.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RestorationStoreError {
    message: String,
}

impl RestorationStoreError {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for RestorationStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for RestorationStoreError {}

/// Small persistence boundary for restoration snapshots. Implementations must
/// leave the previous valid snapshot untouched when `save` returns an error.
pub trait RestorationStore: Send + Sync + 'static {
    fn load(&self) -> Result<Option<Vec<u8>>, RestorationStoreError>;
    fn save(&self, bytes: &[u8]) -> Result<(), RestorationStoreError>;
    fn remove(&self) -> Result<(), RestorationStoreError>;

    /// Returns a human-readable location when one exists. In-memory stores
    /// intentionally return `None`.
    fn location(&self) -> Option<PathBuf> {
        None
    }
}

/// Deterministic store for tests and embedding. It also supports injected
/// write failures so crash-safety behaviour can be verified without a real
/// user directory.
#[derive(Default)]
pub struct InMemoryRestorationStore {
    bytes: Mutex<Option<Vec<u8>>>,
    fail_next_save: Mutex<Option<String>>,
}

impl InMemoryRestorationStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn fail_next_save(&self, message: impl Into<String>) {
        *self
            .fail_next_save
            .lock()
            .expect("restoration test-store failure mutex") = Some(message.into());
    }

    #[must_use]
    pub fn bytes(&self) -> Option<Vec<u8>> {
        self.bytes
            .lock()
            .expect("restoration test-store bytes mutex")
            .clone()
    }

    pub fn set_bytes(&self, bytes: Option<Vec<u8>>) {
        *self
            .bytes
            .lock()
            .expect("restoration test-store bytes mutex") = bytes;
    }
}

impl RestorationStore for InMemoryRestorationStore {
    fn load(&self) -> Result<Option<Vec<u8>>, RestorationStoreError> {
        Ok(self.bytes())
    }

    fn save(&self, bytes: &[u8]) -> Result<(), RestorationStoreError> {
        if let Some(message) = self
            .fail_next_save
            .lock()
            .expect("restoration test-store failure mutex")
            .take()
        {
            return Err(RestorationStoreError::new(message));
        }
        *self
            .bytes
            .lock()
            .expect("restoration test-store bytes mutex") = Some(bytes.to_vec());
        Ok(())
    }

    fn remove(&self) -> Result<(), RestorationStoreError> {
        *self
            .bytes
            .lock()
            .expect("restoration test-store bytes mutex") = None;
        Ok(())
    }
}

/// File-backed default persistence store. A new snapshot is completely
/// flushed to a temporary sibling and then renamed into place. On platforms
/// where replacement cannot be atomic, the rename fails rather than deleting
/// the only known-good file; the next mutation may retry it. This guarantees
/// that a reported failed save never overwrites the previous valid snapshot,
/// while filesystem/power-loss durability remains platform/filesystem-specific.
#[derive(Clone, Debug)]
pub struct FileRestorationStore {
    path: PathBuf,
}

impl FileRestorationStore {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Derives an application-specific user state path rather than using the
    /// working directory. `application_id` is encoded into one safe directory
    /// name and must be stable across launches.
    pub fn for_application(application_id: &str) -> Result<Self, RestorationStoreError> {
        let base = application_state_directory()?;
        let namespace = application_id
            .chars()
            .map(|character| match character {
                'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '-' | '_' => character,
                _ => '_',
            })
            .collect::<String>();
        if namespace.is_empty() || namespace == "." || namespace == ".." {
            return Err(RestorationStoreError::new(
                "application restoration identifier is not usable as a storage namespace",
            ));
        }
        Ok(Self::new(base.join(namespace).join("restoration.json")))
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl RestorationStore for FileRestorationStore {
    fn load(&self) -> Result<Option<Vec<u8>>, RestorationStoreError> {
        match File::open(&self.path) {
            Ok(mut file) => {
                let mut bytes = Vec::new();
                file.read_to_end(&mut bytes)
                    .map_err(|error| RestorationStoreError::new(error.to_string()))?;
                Ok(Some(bytes))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(RestorationStoreError::new(error.to_string())),
        }
    }

    fn save(&self, bytes: &[u8]) -> Result<(), RestorationStoreError> {
        let parent = self.path.parent().ok_or_else(|| {
            RestorationStoreError::new("restoration snapshot path has no parent directory")
        })?;
        fs::create_dir_all(parent)
            .map_err(|error| RestorationStoreError::new(error.to_string()))?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)
            .map_err(|error| RestorationStoreError::new(error.to_string()))?;
        temporary
            .write_all(bytes)
            .and_then(|()| temporary.flush())
            .and_then(|()| temporary.as_file().sync_all())
            .map_err(|error| RestorationStoreError::new(error.to_string()))?;
        temporary
            .persist(&self.path)
            .map_err(|error| RestorationStoreError::new(error.error.to_string()))?;
        // Syncing the directory is unavailable on some desktop platforms; the
        // snapshot file itself has been flushed before replacement.
        Ok(())
    }

    fn remove(&self) -> Result<(), RestorationStoreError> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(RestorationStoreError::new(error.to_string())),
        }
    }

    fn location(&self) -> Option<PathBuf> {
        Some(self.path.clone())
    }
}

fn application_state_directory() -> Result<PathBuf, RestorationStoreError> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .ok_or_else(|| RestorationStoreError::new("APPDATA is unavailable for restoration"))
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join("Library").join("Application Support"))
            .ok_or_else(|| RestorationStoreError::new("HOME is unavailable for restoration"))
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        if let Some(directory) = std::env::var_os("XDG_STATE_HOME") {
            return Ok(PathBuf::from(directory));
        }
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join(".local").join("state"))
            .ok_or_else(|| RestorationStoreError::new("HOME is unavailable for restoration"))
    }
}

/// Stable configuration for an opt-in application restoration session.
#[derive(Clone)]
pub struct RestorationConfig {
    application_id: String,
    application_schema_version: u32,
    store: Arc<dyn RestorationStore>,
    snapshot_limit: usize,
    debounce: Duration,
    migration: Option<RestorationMigration>,
}

impl fmt::Debug for RestorationConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RestorationConfig")
            .field("application_id", &self.application_id)
            .field(
                "application_schema_version",
                &self.application_schema_version,
            )
            .field("snapshot_limit", &self.snapshot_limit)
            .field("debounce", &self.debounce)
            .field("store_location", &self.store.location())
            .finish_non_exhaustive()
    }
}

/// Application-owned migration invoked before any restored value is exposed to
/// a UI build. It receives the old schema version and the complete JSON
/// document; failures safely fall back to default state.
pub type RestorationMigration = Arc<dyn Fn(u32, &mut Value) -> Result<(), String> + 'static>;

impl RestorationConfig {
    #[must_use]
    pub fn new(
        application_id: impl Into<String>,
        application_schema_version: u32,
        store: Arc<dyn RestorationStore>,
    ) -> Self {
        Self {
            application_id: application_id.into(),
            application_schema_version,
            store,
            snapshot_limit: DEFAULT_RESTORATION_SNAPSHOT_LIMIT,
            debounce: DEFAULT_RESTORATION_DEBOUNCE,
            migration: None,
        }
    }

    pub fn file_backed(
        application_id: impl Into<String>,
        application_schema_version: u32,
    ) -> Result<Self, RestorationStoreError> {
        let application_id = application_id.into();
        let store = Arc::new(FileRestorationStore::for_application(&application_id)?);
        Ok(Self::new(application_id, application_schema_version, store))
    }

    #[must_use]
    pub fn with_snapshot_limit(mut self, bytes: usize) -> Self {
        self.snapshot_limit = bytes.max(1);
        self
    }

    #[must_use]
    pub fn with_debounce(mut self, debounce: Duration) -> Self {
        self.debounce = debounce;
        self
    }

    #[must_use]
    pub fn with_migration(mut self, migration: RestorationMigration) -> Self {
        self.migration = Some(migration);
        self
    }

    #[must_use]
    pub fn application_id(&self) -> &str {
        &self.application_id
    }

    #[must_use]
    pub const fn application_schema_version(&self) -> u32 {
        self.application_schema_version
    }
}

/// Native-free, stable data for a restorable window. DPI, physical framebuffer
/// dimensions, positions, native handles, and runtime `WindowId`s are omitted.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RestoredWindow {
    pub restoration_id: String,
    pub kind: String,
    pub logical_width: f32,
    pub logical_height: f32,
    pub maximized: bool,
    pub fullscreen: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SnapshotDocument {
    framework_format_version: u32,
    application_schema_version: u32,
    snapshot_generation: u64,
    application_id: String,
    #[serde(default)]
    state: BTreeMap<String, Value>,
    #[serde(default)]
    windows: Vec<RestoredWindow>,
}

impl SnapshotDocument {
    fn empty(config: &RestorationConfig) -> Self {
        Self {
            framework_format_version: FRAMEWORK_RESTORATION_FORMAT_VERSION,
            application_schema_version: config.application_schema_version,
            snapshot_generation: 0,
            application_id: config.application_id.clone(),
            state: BTreeMap::new(),
            windows: Vec::new(),
        }
    }
}

/// Value-free restoration health and write-coalescing diagnostics.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RestorationDiagnostics {
    pub restoration_loads: u64,
    pub restoration_load_failures: u64,
    pub restoration_migrations: u64,
    pub restoration_values: usize,
    pub restoration_scopes: usize,
    pub dirty_generation: u64,
    pub persisted_generation: u64,
    pub save_requests: u64,
    pub save_coalesced: u64,
    pub saves_started: u64,
    pub saves_completed: u64,
    pub save_failures: u64,
    pub snapshot_bytes: usize,
    pub peak_snapshot_bytes: usize,
    pub restored_windows: u64,
    pub skipped_windows: u64,
    pub restored_routes: u64,
    pub invalid_routes: u64,
    pub restored_scroll_positions: u64,
    pub stale_or_invalid_values: u64,
}

struct ManagerState {
    snapshot: SnapshotDocument,
    diagnostics: RestorationDiagnostics,
    dirty: bool,
    debounce_pending: bool,
    save_in_flight: bool,
    active_scope_paths: HashSet<String>,
}

struct RestorationManagerInner {
    config: RestorationConfig,
    state: std::cell::RefCell<ManagerState>,
    scheduler: std::cell::RefCell<Option<Weak<std::cell::RefCell<tasks::TaskScheduler>>>>,
}

/// Runtime-owned manager. The core restoration scope adapts to this manager;
/// applications normally interact through `BuildContext` and `Restorable<T>`.
#[derive(Clone)]
pub(crate) struct RestorationManager {
    inner: Rc<RestorationManagerInner>,
}

/// Cloneable UI-thread capability for explicit restoration actions. It is safe
/// to retain in callbacks: operations update only the in-memory declarative
/// snapshot and schedule normal Tokio-backed persistence work.
#[derive(Clone)]
pub struct RestorationHandle {
    manager: RestorationManager,
}

impl RestorationHandle {
    pub(crate) fn new(manager: RestorationManager) -> Self {
        Self { manager }
    }

    pub fn reset(&self) {
        self.manager.reset();
    }

    pub fn flush(&self) {
        self.manager.flush();
    }

    /// Explicitly removes all values below a stable scope. This is suitable
    /// for a permanently popped restorable route; temporary rebuilds should
    /// keep their scope intact.
    pub fn remove_scope(&self, scope: &RestorationScope) -> usize {
        self.manager.remove_scope(scope.path())
    }

    /// Records navigation recovery without exposing live navigator internals.
    pub fn note_navigation_restore(&self, restored_routes: u64, invalid_routes: u64) {
        self.manager
            .note_navigation_restore(restored_routes, invalid_routes);
    }

    #[must_use]
    pub fn diagnostics(&self) -> RestorationDiagnostics {
        self.manager.diagnostics()
    }

    #[must_use]
    pub fn path(&self) -> Option<PathBuf> {
        self.manager.store_location()
    }
}

impl RestorationManager {
    pub(crate) fn load(config: RestorationConfig) -> Self {
        let mut diagnostics = RestorationDiagnostics::default();
        let mut snapshot = SnapshotDocument::empty(&config);
        match config.store.load() {
            Ok(Some(bytes)) => {
                diagnostics.restoration_loads = 1;
                diagnostics.snapshot_bytes = bytes.len();
                diagnostics.peak_snapshot_bytes = bytes.len();
                match decode_snapshot(&config, &bytes, &mut diagnostics) {
                    Ok(decoded) => snapshot = decoded,
                    Err(()) => diagnostics.restoration_load_failures += 1,
                }
            }
            Ok(None) => diagnostics.restoration_loads = 1,
            Err(_) => {
                diagnostics.restoration_loads = 1;
                diagnostics.restoration_load_failures = 1;
            }
        }
        diagnostics.restoration_values = snapshot.state.len();
        diagnostics.restoration_scopes = scope_count(&snapshot.state);
        diagnostics.dirty_generation = snapshot.snapshot_generation;
        diagnostics.persisted_generation = snapshot.snapshot_generation;
        Self {
            inner: Rc::new(RestorationManagerInner {
                config,
                state: std::cell::RefCell::new(ManagerState {
                    snapshot,
                    diagnostics,
                    dirty: false,
                    debounce_pending: false,
                    save_in_flight: false,
                    active_scope_paths: HashSet::new(),
                }),
                scheduler: std::cell::RefCell::new(None),
            }),
        }
    }

    pub(crate) fn attach_scheduler(
        &self,
        scheduler: &Rc<std::cell::RefCell<tasks::TaskScheduler>>,
    ) {
        *self.inner.scheduler.borrow_mut() = Some(Rc::downgrade(scheduler));
    }

    pub(crate) fn read(&self, path: &[RestorationKey]) -> Option<Value> {
        self.inner
            .state
            .borrow()
            .snapshot
            .state
            .get(&encode_path(path))
            .cloned()
    }

    pub(crate) fn write(&self, path: &[RestorationKey], value: Value) {
        let encoded = encode_path(path);
        let mut state = self.inner.state.borrow_mut();
        if state.snapshot.state.get(&encoded) == Some(&value) {
            return;
        }
        state.snapshot.state.insert(encoded, value);
        refresh_counts(&mut state);
        drop(state);
        self.mark_dirty();
    }

    pub(crate) fn remove(&self, path: &[RestorationKey]) -> bool {
        let mut state = self.inner.state.borrow_mut();
        let removed = state.snapshot.state.remove(&encode_path(path)).is_some();
        if removed {
            refresh_counts(&mut state);
        }
        drop(state);
        if removed {
            self.mark_dirty();
        }
        removed
    }

    pub(crate) fn remove_scope(&self, path: &[RestorationKey]) -> usize {
        let prefix = encode_path(path);
        let prefix_with_separator = format!("{prefix}/");
        let mut state = self.inner.state.borrow_mut();
        let before = state.snapshot.state.len();
        state
            .snapshot
            .state
            .retain(|key, _| key != &prefix && !key.starts_with(&prefix_with_separator));
        let removed = before - state.snapshot.state.len();
        if removed > 0 {
            refresh_counts(&mut state);
        }
        drop(state);
        if removed > 0 {
            self.mark_dirty();
        }
        removed
    }

    pub(crate) fn reset(&self) {
        let mut state = self.inner.state.borrow_mut();
        state.snapshot.state.clear();
        state.snapshot.windows.clear();
        refresh_counts(&mut state);
        drop(state);
        self.mark_dirty();
    }

    pub(crate) fn replace_windows(&self, windows: Vec<RestoredWindow>) {
        let mut state = self.inner.state.borrow_mut();
        if state.snapshot.windows == windows {
            return;
        }
        state.snapshot.windows = windows;
        drop(state);
        self.mark_dirty();
    }

    #[must_use]
    pub(crate) fn windows(&self) -> Vec<RestoredWindow> {
        self.inner.state.borrow().snapshot.windows.clone()
    }

    #[must_use]
    pub(crate) fn diagnostics(&self) -> RestorationDiagnostics {
        self.inner.state.borrow().diagnostics.clone()
    }

    #[must_use]
    pub(crate) fn store_location(&self) -> Option<PathBuf> {
        self.inner.config.store.location()
    }

    pub(crate) fn note_restored_window(&self) {
        self.inner.state.borrow_mut().diagnostics.restored_windows += 1;
    }

    pub(crate) fn note_skipped_window(&self) {
        self.inner.state.borrow_mut().diagnostics.skipped_windows += 1;
    }

    pub(crate) fn note_navigation_restore(&self, restored_routes: u64, invalid_routes: u64) {
        let mut state = self.inner.state.borrow_mut();
        state.diagnostics.restored_routes += restored_routes;
        state.diagnostics.invalid_routes += invalid_routes;
    }

    pub(crate) fn scope(&self) -> RestorationScope {
        RestorationScope::root(Rc::new(self.clone()))
    }

    pub(crate) fn application_scope(&self) -> RestorationScope {
        self.scope()
            .child_unchecked(RestorationKey::new("application").expect("static key"))
    }

    /// Acquires a sibling scope identity for the duration of the returned
    /// lease. A second concurrently mounted scope at the same stable path is
    /// a programming error, never an auto-numbered persistence target.
    pub(crate) fn acquire_scope(&self, path: &[RestorationKey]) -> Result<ScopeLease, String> {
        let path = encode_path(path);
        let mut state = self.inner.state.borrow_mut();
        if !state.active_scope_paths.insert(path.clone()) {
            return Err(format!("duplicate restoration sibling scope `{path}`"));
        }
        Ok(ScopeLease {
            manager: Rc::downgrade(&self.inner),
            path,
        })
    }

    fn mark_dirty(&self) {
        {
            let mut state = self.inner.state.borrow_mut();
            state.dirty = true;
            state.diagnostics.dirty_generation = state.diagnostics.dirty_generation.wrapping_add(1);
            state.snapshot.snapshot_generation = state.diagnostics.dirty_generation;
            state.diagnostics.save_requests += 1;
            if state.save_in_flight || state.debounce_pending {
                state.diagnostics.save_coalesced += 1;
                return;
            }
            state.debounce_pending = true;
        }
        self.schedule_debounce();
    }

    fn schedule_debounce(&self) {
        let Some(scheduler) = self
            .inner
            .scheduler
            .borrow()
            .as_ref()
            .and_then(Weak::upgrade)
        else {
            return;
        };
        let manager = self.clone();
        let debounce = self.inner.config.debounce;
        tasks::TaskScheduler::spawner(&scheduler).spawn_into(
            async move { tokio::time::sleep(debounce).await },
            move |result, _runtime| {
                if result.is_ok() {
                    manager.start_save();
                }
            },
        );
    }

    pub(crate) fn flush(&self) {
        {
            let mut state = self.inner.state.borrow_mut();
            state.debounce_pending = false;
        }
        self.start_save();
    }

    #[must_use]
    pub(crate) fn is_clean(&self) -> bool {
        let state = self.inner.state.borrow();
        !state.dirty && !state.save_in_flight && !state.debounce_pending
    }

    fn start_save(&self) {
        let (snapshot, generation) = {
            let mut state = self.inner.state.borrow_mut();
            state.debounce_pending = false;
            if !state.dirty || state.save_in_flight {
                return;
            }
            state.save_in_flight = true;
            state.diagnostics.saves_started += 1;
            (state.snapshot.clone(), state.diagnostics.dirty_generation)
        };
        let Some(scheduler) = self
            .inner
            .scheduler
            .borrow()
            .as_ref()
            .and_then(Weak::upgrade)
        else {
            self.finish_save(
                generation,
                Err(RestorationStoreError::new(
                    "runtime scheduler is unavailable",
                )),
                0,
            );
            return;
        };
        let store = self.inner.config.store.clone();
        let limit = self.inner.config.snapshot_limit;
        let manager = self.clone();
        tasks::TaskScheduler::spawner(&scheduler).spawn_blocking(
            move || {
                let bytes = serde_json::to_vec(&snapshot)
                    .map_err(|error| RestorationStoreError::new(error.to_string()))?;
                if bytes.len() > limit {
                    return Err(RestorationStoreError::new(format!(
                        "restoration snapshot is {} bytes, exceeding configured {limit}-byte limit",
                        bytes.len()
                    )));
                }
                store.save(&bytes)?;
                Ok(bytes.len())
            },
            move |result, _runtime| match result {
                Ok(Ok(bytes)) => manager.finish_save(generation, Ok(()), bytes),
                Ok(Err(error)) => manager.finish_save(generation, Err(error), 0),
                Err(_) => manager.finish_save(
                    generation,
                    Err(RestorationStoreError::new(
                        "restoration save task did not complete",
                    )),
                    0,
                ),
            },
        );
    }

    fn finish_save(
        &self,
        generation: u64,
        result: Result<(), RestorationStoreError>,
        bytes: usize,
    ) {
        let needs_latest;
        {
            let mut state = self.inner.state.borrow_mut();
            state.save_in_flight = false;
            match result {
                Ok(()) => {
                    state.diagnostics.saves_completed += 1;
                    state.diagnostics.snapshot_bytes = bytes;
                    state.diagnostics.peak_snapshot_bytes =
                        state.diagnostics.peak_snapshot_bytes.max(bytes);
                    state.diagnostics.persisted_generation = generation;
                    state.dirty = state.diagnostics.dirty_generation != generation;
                    needs_latest = state.dirty;
                }
                Err(_) => {
                    state.diagnostics.save_failures += 1;
                    state.dirty = true;
                    // Preserve the last known-good file and await an explicit
                    // later mutation or lifecycle flush. Retrying an unknown
                    // filesystem failure in a tight loop would both defeat
                    // coalescing and obscure crash-safety diagnostics.
                    needs_latest = false;
                }
            }
        }
        if needs_latest {
            self.schedule_after_in_flight();
        }
    }

    fn schedule_after_in_flight(&self) {
        let mut state = self.inner.state.borrow_mut();
        if state.debounce_pending || state.save_in_flight {
            return;
        }
        state.debounce_pending = true;
        state.diagnostics.save_coalesced += 1;
        drop(state);
        self.schedule_debounce();
    }

    pub(crate) fn debug_dump(&self) -> String {
        let state = self.inner.state.borrow();
        let diagnostics = &state.diagnostics;
        let mut lines = vec![format!(
            "Restoration generation={} persisted={} bytes={}",
            diagnostics.dirty_generation,
            diagnostics.persisted_generation,
            diagnostics.snapshot_bytes
        )];
        let mut scopes: BTreeMap<String, usize> = BTreeMap::new();
        for key in state.snapshot.state.keys() {
            let mut segments = key.split('/').collect::<Vec<_>>();
            let _ = segments.pop();
            let scope = if segments.is_empty() {
                "application".to_owned()
            } else {
                segments.join("/")
            };
            *scopes.entry(scope).or_default() += 1;
        }
        for (scope, values) in scopes {
            lines.push(format!("  {scope}/ ({values} values)"));
        }
        lines.join("\n")
    }
}

/// Duplicate-sibling guard held by a mounted restoration scope.
pub(crate) struct ScopeLease {
    manager: Weak<RestorationManagerInner>,
    path: String,
}

impl Drop for ScopeLease {
    fn drop(&mut self) {
        if let Some(manager) = self.manager.upgrade() {
            manager
                .state
                .borrow_mut()
                .active_scope_paths
                .remove(&self.path);
        }
    }
}

fn decode_snapshot(
    config: &RestorationConfig,
    bytes: &[u8],
    diagnostics: &mut RestorationDiagnostics,
) -> Result<SnapshotDocument, ()> {
    let mut value: Value = serde_json::from_slice(bytes).map_err(|_| ())?;
    let document: SnapshotDocument = serde_json::from_value(value.clone()).map_err(|_| ())?;
    if document.framework_format_version != FRAMEWORK_RESTORATION_FORMAT_VERSION
        || document.application_id != config.application_id
    {
        return Err(());
    }
    if document.application_schema_version != config.application_schema_version {
        let Some(migration) = &config.migration else {
            return Err(());
        };
        migration(document.application_schema_version, &mut value).map_err(|_| ())?;
        let mut migrated: SnapshotDocument = serde_json::from_value(value).map_err(|_| ())?;
        migrated.framework_format_version = FRAMEWORK_RESTORATION_FORMAT_VERSION;
        migrated.application_schema_version = config.application_schema_version;
        migrated.application_id = config.application_id.clone();
        diagnostics.restoration_migrations += 1;
        Ok(migrated)
    } else {
        Ok(document)
    }
}

fn encode_path(path: &[RestorationKey]) -> String {
    path.iter()
        .map(|segment| segment.as_str().replace('~', "~0").replace('/', "~1"))
        .collect::<Vec<_>>()
        .join("/")
}

fn scope_count(values: &BTreeMap<String, Value>) -> usize {
    values
        .keys()
        .filter_map(|key| {
            key.rsplit_once('/')
                .map(|(scope, _)| scope)
                .or(Some("application"))
        })
        .collect::<BTreeSet<_>>()
        .len()
}

fn refresh_counts(state: &mut ManagerState) {
    state.diagnostics.restoration_values = state.snapshot.state.len();
    state.diagnostics.restoration_scopes = scope_count(&state.snapshot.state);
}

/// A typed, opt-in persistent value built on the existing local [`Signal`]
/// primitive. Mutations through this wrapper update the snapshot; an ordinary
/// `Signal` stays non-persistent by default.
#[derive(Clone)]
pub struct Restorable<T> {
    signal: Signal<T>,
    write: Rc<dyn Fn(&T)>,
    remove: Rc<dyn Fn()>,
}

impl<T> Restorable<T>
where
    T: Clone + PartialEq + Serialize + DeserializeOwned + 'static,
{
    pub(crate) fn new(
        initial: T,
        write: impl Fn(&T) + 'static,
        remove: impl Fn() + 'static,
    ) -> Self {
        Self {
            signal: Signal::new(initial),
            write: Rc::new(write),
            remove: Rc::new(remove),
        }
    }

    /// Reads `key` before returning the normal reactive signal wrapper. A
    /// missing, corrupt, or type-incompatible stored value exposes `default`
    /// without failing the build that requested it.
    #[must_use]
    pub fn from_scope(scope: RestorationScope, key: RestorationKey, default: T) -> Self {
        let initial = scope
            .get_json(&key)
            .and_then(|value| serde_json::from_value(value).ok())
            .unwrap_or(default);
        let write_scope = scope.clone();
        let write_key = key.clone();
        let remove_scope = scope;
        Self::new(
            initial,
            move |value| {
                if let Ok(value) = serde_json::to_value(value) {
                    write_scope.set_json(&write_key, value);
                }
            },
            move || remove_scope.remove(&key),
        )
    }

    #[must_use]
    pub fn get(&self) -> T {
        self.signal.get()
    }

    pub fn set(&self, value: T) -> bool {
        if !self.signal.set(value.clone()) {
            return false;
        }
        (self.write)(&value);
        true
    }

    pub fn update(&self, update: impl FnOnce(&mut T)) {
        let mut value = self.signal.get();
        update(&mut value);
        let _ = self.set(value);
    }

    /// Stops restoring this value and removes its current persisted entry.
    pub fn remove(&self) {
        (self.remove)();
    }

    /// Exposes the reactive read primitive. Write through `Restorable::set`
    /// or `update` so persistence remains opt-in and observable.
    #[must_use]
    pub fn signal(&self) -> Signal<T> {
        self.signal.clone()
    }
}

impl RestorationBackend for RestorationManager {
    fn read_value(&self, path: &[RestorationKey]) -> Option<Value> {
        self.read(path)
    }

    fn write_value(&self, path: &[RestorationKey], value: Value) {
        self.write(path, value);
    }

    fn remove_value(&self, path: &[RestorationKey]) {
        let _ = self.remove(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(value: &str) -> RestorationKey {
        RestorationKey::new(value).unwrap()
    }

    fn manager(store: Arc<InMemoryRestorationStore>) -> RestorationManager {
        RestorationManager::load(RestorationConfig::new("com.example.restore-test", 1, store))
    }

    #[test]
    fn paths_escape_segments_deterministically() {
        assert_eq!(
            encode_path(&[
                RestorationKey::new("window/main").unwrap(),
                RestorationKey::new("editor~draft").unwrap(),
            ]),
            "window~1main/editor~0draft"
        );
    }

    #[test]
    fn in_memory_store_failure_keeps_previous_snapshot() {
        let store = InMemoryRestorationStore::new();
        store.save(b"A").unwrap();
        store.fail_next_save("interrupted replacement");
        assert!(store.save(b"B").is_err());
        assert_eq!(store.load().unwrap(), Some(b"A".to_vec()));
    }

    #[test]
    fn corrupt_and_future_snapshots_fall_back_cleanly() {
        let store = Arc::new(InMemoryRestorationStore::new());
        let config = RestorationConfig::new("com.example.test", 1, store.clone());
        store.set_bytes(Some(b"not json".to_vec()));
        assert_eq!(
            RestorationManager::load(config.clone())
                .diagnostics()
                .restoration_load_failures,
            1
        );
        store.set_bytes(Some(
            serde_json::to_vec(&SnapshotDocument {
                framework_format_version: 99,
                application_schema_version: 1,
                snapshot_generation: 0,
                application_id: "com.example.test".to_owned(),
                state: BTreeMap::new(),
                windows: Vec::new(),
            })
            .unwrap(),
        ));
        assert_eq!(
            RestorationManager::load(config)
                .diagnostics()
                .restoration_load_failures,
            1
        );
    }

    #[test]
    fn typed_values_default_update_and_remove_without_serializing_a_signal() {
        let manager = manager(Arc::new(InMemoryRestorationStore::new()));
        let scope = manager
            .scope()
            .child_unchecked(key("application"))
            .child_unchecked(key("settings"));
        let value = Restorable::from_scope(scope, key("count"), 7_u32);
        assert_eq!(value.get(), 7);
        assert!(value.set(9));
        assert_eq!(
            manager.read(&[key("application"), key("settings"), key("count")]),
            Some(serde_json::json!(9))
        );
        value.remove();
        assert!(
            manager
                .read(&[key("application"), key("settings"), key("count")])
                .is_none()
        );
    }

    #[test]
    fn equal_local_keys_in_independent_scopes_do_not_collide() {
        let manager = manager(Arc::new(InMemoryRestorationStore::new()));
        let root = manager.scope();
        let first = root
            .child_unchecked(key("window"))
            .child_unchecked(key("main"));
        let second = root
            .child_unchecked(key("window"))
            .child_unchecked(key("inspector"));
        first.set_json(&key("scroll"), serde_json::json!(12));
        second.set_json(&key("scroll"), serde_json::json!(96));
        assert_eq!(first.get_json(&key("scroll")), Some(serde_json::json!(12)));
        assert_eq!(second.get_json(&key("scroll")), Some(serde_json::json!(96)));
    }

    #[test]
    fn duplicate_sibling_scope_is_detected_until_the_original_unmounts() {
        let manager = manager(Arc::new(InMemoryRestorationStore::new()));
        let path = [key("window"), key("main")];
        let lease = manager.acquire_scope(&path).unwrap();
        assert!(manager.acquire_scope(&path).is_err());
        drop(lease);
        assert!(manager.acquire_scope(&path).is_ok());
    }

    #[test]
    fn schema_migration_runs_before_values_are_exposed_and_failure_falls_back() {
        let store = Arc::new(InMemoryRestorationStore::new());
        store.set_bytes(Some(
            serde_json::to_vec(&SnapshotDocument {
                framework_format_version: FRAMEWORK_RESTORATION_FORMAT_VERSION,
                application_schema_version: 1,
                snapshot_generation: 2,
                application_id: "com.example.migrate".to_owned(),
                state: BTreeMap::from([(String::from("application/count"), serde_json::json!(4))]),
                windows: Vec::new(),
            })
            .unwrap(),
        ));
        let migrated = RestorationManager::load(
            RestorationConfig::new("com.example.migrate", 2, store.clone()).with_migration(
                Arc::new(|_old, document| {
                    document["application_schema_version"] = serde_json::json!(2);
                    document["state"]["application/count"] = serde_json::json!(5);
                    Ok(())
                }),
            ),
        );
        assert_eq!(migrated.diagnostics().restoration_migrations, 1);
        assert_eq!(
            migrated.read(&[key("application"), key("count")]),
            Some(serde_json::json!(5))
        );

        let failed = RestorationManager::load(
            RestorationConfig::new("com.example.migrate", 3, store).with_migration(Arc::new(
                |_old, _document| Err("cannot migrate safely".to_owned()),
            )),
        );
        assert_eq!(failed.diagnostics().restoration_load_failures, 1);
        assert!(failed.read(&[key("application"), key("count")]).is_none());
    }

    #[test]
    fn scope_removal_preserves_siblings() {
        let manager = manager(Arc::new(InMemoryRestorationStore::new()));
        manager.write(
            &[key("window"), key("main"), key("draft")],
            serde_json::json!("A"),
        );
        manager.write(
            &[key("window"), key("inspector"), key("draft")],
            serde_json::json!("B"),
        );
        assert_eq!(manager.remove_scope(&[key("window"), key("main")]), 1);
        assert!(
            manager
                .read(&[key("window"), key("main"), key("draft")])
                .is_none()
        );
        assert_eq!(
            manager.read(&[key("window"), key("inspector"), key("draft")]),
            Some(serde_json::json!("B"))
        );
    }
}
