use incular_core::{Color, RestorationBackend, RestorationKey, RestorationScope, Size};
use incular_platform::{PlatformLifecycle, WindowOptions};
use incular_runtime::{
    Application, FRAMEWORK_RESTORATION_FORMAT_VERSION, FileRestorationStore,
    InMemoryRestorationStore, MAX_RESTORATION_SAVE_RETRIES, RESTORATION_RETRY_BACKOFF, Restorable,
    RestorationConfig, RestorationStore, RestorationStoreError, WindowRestorationId,
};
use incular_widgets::Widget;
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Default)]
struct MemoryBackend {
    values: RefCell<HashMap<Vec<RestorationKey>, Value>>,
}

impl RestorationBackend for MemoryBackend {
    fn read_value(&self, path: &[RestorationKey]) -> Option<Value> {
        self.values.borrow().get(path).cloned()
    }

    fn write_value(&self, path: &[RestorationKey], value: Value) {
        self.values.borrow_mut().insert(path.to_vec(), value);
    }

    fn remove_value(&self, path: &[RestorationKey]) {
        self.values.borrow_mut().remove(path);
    }
}

fn key(value: &str) -> RestorationKey {
    RestorationKey::new(value).unwrap()
}

fn test_widget() -> Widget {
    Widget::box_(Size::new(1., 1.), Color::WHITE)
}

#[test]
fn restoration_keys_remain_structured_across_nested_scopes() {
    let backend = Rc::new(MemoryBackend::default());
    let scope = RestorationScope::root(backend.clone())
        .child(key("window/main"))
        .child(key("editor~draft"));
    scope.set_json(&key("count"), json!(12));

    assert_eq!(scope.get_json(&key("count")), Some(json!(12)));
    let paths = backend.values.borrow();
    assert!(paths.contains_key(&vec![key("window/main"), key("editor~draft"), key("count")]));
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
fn default_file_store_uses_a_project_data_location_without_writing() {
    let store = FileRestorationStore::for_application("org.incular.restore-test")
        .expect("test environment has standard application directories");
    assert_eq!(
        store.path().file_name().and_then(|name| name.to_str()),
        Some("restoration.json")
    );
    assert!(store.path().parent().is_some());
}

#[test]
fn corrupt_and_future_snapshots_fall_back_cleanly() {
    let store = Arc::new(InMemoryRestorationStore::new());
    let config = RestorationConfig::new("com.example.test", 1, store.clone());
    store.set_bytes(Some(b"not json".to_vec()));
    let application =
        Application::new_with_restoration(WindowOptions::default(), config.clone(), |_| {
            test_widget()
        })
        .unwrap();
    assert_eq!(
        application
            .restoration_diagnostics()
            .unwrap()
            .restoration_load_failures,
        1
    );
    drop(application);

    store.set_bytes(Some(
        serde_json::to_vec(&json!({
            "framework_format_version": FRAMEWORK_RESTORATION_FORMAT_VERSION + 1,
            "application_schema_version": 1,
            "snapshot_generation": 0,
            "application_id": "com.example.test",
            "state": {},
            "windows": []
        }))
        .unwrap(),
    ));
    let application =
        Application::new_with_restoration(WindowOptions::default(), config, |_| test_widget())
            .unwrap();
    assert_eq!(
        application
            .restoration_diagnostics()
            .unwrap()
            .restoration_load_failures,
        1
    );
}

#[test]
fn typed_values_default_update_and_remove_without_serializing_a_signal() {
    let backend = Rc::new(MemoryBackend::default());
    let scope = RestorationScope::root(backend.clone())
        .child(key("application"))
        .child(key("settings"));
    let value = Restorable::from_scope(scope, key("count"), 7_u32);
    assert_eq!(value.get(), 7);
    assert!(value.set(9));
    assert_eq!(value.signal().get(), 9);
    value.remove();
    assert!(!backend.values.borrow().contains_key(&vec![
        key("application"),
        key("settings"),
        key("count")
    ]));
}

#[test]
fn equal_local_keys_in_independent_scopes_do_not_collide() {
    let backend = Rc::new(MemoryBackend::default());
    let root = RestorationScope::root(backend);
    let first = root.child(key("window")).child(key("main"));
    let second = root.child(key("window")).child(key("inspector"));
    first.set_json(&key("scroll"), json!(12));
    second.set_json(&key("scroll"), json!(96));
    assert_eq!(first.get_json(&key("scroll")), Some(json!(12)));
    assert_eq!(second.get_json(&key("scroll")), Some(json!(96)));
}

#[test]
fn schema_migration_runs_before_values_are_exposed_and_failure_falls_back() {
    let store = Arc::new(InMemoryRestorationStore::new());
    store.set_bytes(Some(
        serde_json::to_vec(&json!({
            "framework_format_version": FRAMEWORK_RESTORATION_FORMAT_VERSION,
            "application_schema_version": 1,
            "snapshot_generation": 2,
            "application_id": "com.example.migrate",
            "state": {"application/count": 4},
            "windows": []
        }))
        .unwrap(),
    ));
    let restored = Rc::new(RefCell::new(None));
    let slot = restored.clone();
    let application = Application::new_with_restoration(
        WindowOptions::default(),
        RestorationConfig::new("com.example.migrate", 2, store.clone()).with_migration(Arc::new(
            |_old, document| {
                document["application_schema_version"] = json!(2);
                document["state"]["application/count"] = json!(5);
                Ok(())
            },
        )),
        move |cx| {
            *slot.borrow_mut() = cx.restorable(key("count"), 0_u32);
            test_widget()
        },
    )
    .unwrap();
    assert_eq!(restored.borrow().as_ref().unwrap().get(), 5);
    assert_eq!(
        application
            .restoration_diagnostics()
            .unwrap()
            .restoration_migrations,
        1
    );
    drop(application);

    let failed_slot = Rc::new(RefCell::new(None));
    let slot = failed_slot.clone();
    let application = Application::new_with_restoration(
        WindowOptions::default(),
        RestorationConfig::new("com.example.migrate", 3, store)
            .with_migration(Arc::new(|_, _| Err("cannot migrate safely".to_owned())))
            .with_debounce(Duration::ZERO),
        move |cx| {
            *slot.borrow_mut() = cx.restorable(key("count"), 0_u32);
            test_widget()
        },
    )
    .unwrap();
    assert_eq!(failed_slot.borrow().as_ref().unwrap().get(), 0);
    assert_eq!(
        application
            .restoration_diagnostics()
            .unwrap()
            .restoration_load_failures,
        1
    );
}

#[test]
fn scope_removal_preserves_siblings() {
    let scope_slot = Rc::new(RefCell::new(None));
    let handle_slot = Rc::new(RefCell::new(None));
    let scope_for_build = scope_slot.clone();
    let handle_for_build = handle_slot.clone();
    let mut application = Application::new_with_restoration(
        WindowOptions::default(),
        RestorationConfig::new(
            "com.example.scope-removal",
            1,
            Arc::new(InMemoryRestorationStore::new()),
        ),
        move |cx| {
            *scope_for_build.borrow_mut() = cx.restoration_scope();
            *handle_for_build.borrow_mut() = cx.restoration();
            test_widget()
        },
    )
    .unwrap();
    let root = scope_slot.borrow().as_ref().unwrap().clone();
    let handle = handle_slot.borrow().as_ref().unwrap().clone();
    let main = root.child(key("window")).child(key("main"));
    let inspector = root.child(key("window")).child(key("inspector"));
    main.set_json(&key("draft"), json!("A"));
    inspector.set_json(&key("draft"), json!("B"));
    assert_eq!(handle.remove_scope(&main), 1);
    assert_eq!(main.get_json(&key("draft")), None);
    assert_eq!(inspector.get_json(&key("draft")), Some(json!("B")));
    application.shutdown();
}

#[test]
fn suspend_flush_persists_without_driving_work_afterward() {
    let store = Arc::new(InMemoryRestorationStore::new());
    let application_slot = Rc::new(RefCell::new(None));
    let application_for_build = application_slot.clone();
    let value_slot = Rc::new(RefCell::new(None));
    let value_for_build = value_slot.clone();
    let mut application = Application::new_with_restoration(
        WindowOptions::default(),
        RestorationConfig::new("com.example.suspend", 1, store.clone()),
        move |cx| {
            *value_for_build.borrow_mut() = cx.restorable(key("count"), 0_u32);
            test_widget()
        },
    )
    .unwrap();
    let value = value_slot.borrow().as_ref().unwrap().clone();
    value.set(41);
    assert!(
        application
            .restoration_diagnostics()
            .unwrap()
            .last_save_error
            .is_none()
    );
    application.handle_application_lifecycle(PlatformLifecycle::Suspended);
    let diagnostics = application.restoration_diagnostics().unwrap();
    assert_eq!(
        diagnostics.persisted_generation,
        diagnostics.dirty_generation
    );
    assert!(diagnostics.saves_completed >= 1);
    assert_eq!(diagnostics.last_save_error, None);
    let persisted = store.bytes().expect("suspend flush wrote a snapshot");
    let document: Value = serde_json::from_slice(&persisted).unwrap();
    assert_eq!(document["state"]["application/count"], json!(41));

    let store_for_relaunch = store.clone();
    drop(application);
    let application = Application::new_with_restoration(
        WindowOptions::default(),
        RestorationConfig::new("com.example.suspend", 1, store_for_relaunch),
        move |cx| {
            *application_for_build.borrow_mut() = Some(cx.restoration());
            test_widget()
        },
    )
    .unwrap();
    let diagnostics = application.restoration_diagnostics().unwrap();
    assert_eq!(diagnostics.restoration_values, 1);
    assert_eq!(diagnostics.last_save_error, None);
}

#[test]
fn migration_is_persisted_on_attach_and_not_rerun_on_next_launch() {
    let store = Arc::new(InMemoryRestorationStore::new());
    store.set_bytes(Some(
        serde_json::to_vec(&json!({
            "framework_format_version": FRAMEWORK_RESTORATION_FORMAT_VERSION,
            "application_schema_version": 1,
            "snapshot_generation": 3,
            "application_id": "com.example.migrate-persist",
            "state": {"application/legacy": 7},
            "windows": []
        }))
        .unwrap(),
    ));
    let migrations = Arc::new(AtomicUsize::new(0));
    let migrations_for_config = migrations.clone();
    let mut application = Application::new_with_restoration(
        WindowOptions::default(),
        RestorationConfig::new("com.example.migrate-persist", 2, store.clone()).with_migration(
            Arc::new(move |_old, document| {
                migrations_for_config.fetch_add(1, Ordering::SeqCst);
                document["application_schema_version"] = json!(2);
                document["state"]["application/count"] =
                    document["state"]["application/legacy"].clone();
                document["state"]
                    .as_object_mut()
                    .unwrap()
                    .remove("application/legacy");
                Ok(())
            }),
        ),
        |_| test_widget(),
    )
    .unwrap();
    assert_eq!(migrations.load(Ordering::SeqCst), 1);
    drive_until(
        &mut application,
        |application| {
            application
                .restoration_diagnostics()
                .is_some_and(|state| state.persisted_generation == state.dirty_generation)
        },
        Instant::now() + Duration::from_secs(5),
    );
    assert_eq!(
        application
            .restoration_diagnostics()
            .unwrap()
            .last_save_error,
        None
    );
    let persisted = store.bytes().expect("migrated snapshot was persisted");
    let document: Value = serde_json::from_slice(&persisted).unwrap();
    assert_eq!(document["application_schema_version"], json!(2));
    assert_eq!(document["state"]["application/count"], json!(7));
    assert!(document["state"].get("application/legacy").is_none());
    drop(application);

    let application = Application::new_with_restoration(
        WindowOptions::default(),
        RestorationConfig::new("com.example.migrate-persist", 2, store).with_migration(Arc::new(
            |_, _| {
                panic!("migration must not rerun after the migrated snapshot was persisted");
            },
        )),
        |_| test_widget(),
    )
    .unwrap();
    assert_eq!(
        application
            .restoration_diagnostics()
            .unwrap()
            .restoration_migrations,
        0
    );
}

#[test]
fn transient_failure_retries_with_backoff_and_settles_cleanly() {
    let store = Arc::new(InMemoryRestorationStore::new());
    let value_slot = Rc::new(RefCell::new(None));
    let value_for_build = value_slot.clone();
    let mut application = Application::new_with_restoration(
        WindowOptions::default(),
        RestorationConfig::new("com.example.transient", 1, store.clone())
            .with_debounce(Duration::ZERO),
        move |cx| {
            *value_for_build.borrow_mut() = cx.restorable(key("count"), 0_u32);
            test_widget()
        },
    )
    .unwrap();
    let value = value_slot.borrow().as_ref().unwrap().clone();
    store.fail_next_save("transient outage");
    value.set(5);
    let started = Instant::now();
    drive_until(
        &mut application,
        |application| {
            application
                .restoration_diagnostics()
                .is_some_and(|state| state.persisted_generation == state.dirty_generation)
        },
        started + Duration::from_secs(5),
    );
    let diagnostics = application.restoration_diagnostics().unwrap();
    assert!(diagnostics.save_failures >= 1);
    assert!(diagnostics.saves_completed >= 1);
    assert_eq!(diagnostics.last_save_error, None);
    let persisted = store.bytes().expect("retry eventually persisted");
    let document: Value = serde_json::from_slice(&persisted).unwrap();
    assert_eq!(document["state"]["application/count"], json!(5));
}

#[test]
fn persistent_failures_stop_after_bounded_attempts_and_flush_can_recover() {
    let store = FailingStore::new();
    let value_slot = Rc::new(RefCell::new(None));
    let value_for_build = value_slot.clone();
    let mut application = Application::new_with_restoration(
        WindowOptions::default(),
        RestorationConfig::new("com.example.persistent", 1, store.clone())
            .with_debounce(Duration::ZERO),
        move |cx| {
            *value_for_build.borrow_mut() = cx.restorable(key("count"), 0_u32);
            test_widget()
        },
    )
    .unwrap();
    let value = value_slot.borrow().as_ref().unwrap().clone();
    value.set(9);
    let started = Instant::now();
    let settle_deadline =
        started + RESTORATION_RETRY_BACKOFF * MAX_RESTORATION_SAVE_RETRIES + Duration::from_secs(2);
    drive_until(
        &mut application,
        |application| {
            application.restoration_diagnostics().is_some_and(|state| {
                state.save_failures == u64::from(MAX_RESTORATION_SAVE_RETRIES) + 1
                    && state.last_save_error.is_some()
            })
        },
        settle_deadline,
    );
    let failures = store.attempts();
    assert_eq!(
        failures,
        MAX_RESTORATION_SAVE_RETRIES as usize + 1,
        "one debounced save plus three retries"
    );
    let diagnostics = application.restoration_diagnostics().unwrap();
    assert_eq!(
        diagnostics.last_save_error.as_deref(),
        Some("persistent store outage")
    );
    assert_eq!(diagnostics.saves_completed, 0);
    thread::sleep(Duration::from_millis(200));
    let settled_attempts = store.attempts();
    application.process_runtime_work();
    application.process_runtime_work();
    assert_eq!(store.attempts(), settled_attempts);

    store.fail_saves.store(false, Ordering::SeqCst);
    application.flush_restoration();
    drive_until(
        &mut application,
        |application| {
            application
                .restoration_diagnostics()
                .is_some_and(|state| state.saves_completed >= 1)
        },
        Instant::now() + Duration::from_secs(5),
    );
    let diagnostics = application.restoration_diagnostics().unwrap();
    assert_eq!(diagnostics.last_save_error, None);
    assert_eq!(store.attempts(), failures + 1);
}

#[test]
fn oversize_snapshot_reports_error_preserves_old_snapshot_and_recovers() {
    let store = Arc::new(InMemoryRestorationStore::new());
    let value_slot = Rc::new(RefCell::new(None));
    let value_for_build = value_slot.clone();
    let mut application = Application::new_with_restoration(
        WindowOptions::default(),
        RestorationConfig::new("com.example.oversize", 1, store.clone())
            .with_snapshot_limit(512)
            .with_debounce(Duration::ZERO),
        move |cx| {
            *value_for_build.borrow_mut() = cx.restorable(key("payload"), String::new());
            test_widget()
        },
    )
    .unwrap();
    let value = value_slot.borrow().as_ref().unwrap().clone();
    value.set("small".to_owned());
    drive_until(
        &mut application,
        |application| {
            application
                .restoration_diagnostics()
                .is_some_and(|state| state.persisted_generation == state.dirty_generation)
        },
        Instant::now() + Duration::from_secs(5),
    );
    let baseline = store.bytes().expect("small snapshot persisted first");
    let baseline: Value = serde_json::from_slice(&baseline).unwrap();
    assert_eq!(baseline["state"]["application/payload"], json!("small"));

    value.set("large".repeat(400));
    drive_until(
        &mut application,
        |application| {
            application
                .restoration_diagnostics()
                .is_some_and(|state| state.last_save_error.is_some())
        },
        Instant::now() + Duration::from_secs(5),
    );
    let diagnostics = application.restoration_diagnostics().unwrap();
    assert!(
        diagnostics
            .last_save_error
            .as_deref()
            .is_some_and(|message| message.contains("exceeding configured 512-byte limit"))
    );
    let preserved = store.bytes().expect("old snapshot preserved");
    let preserved: Value = serde_json::from_slice(&preserved).unwrap();
    assert_eq!(preserved["state"]["application/payload"], json!("small"));

    value.set("recovered".to_owned());
    drive_until(
        &mut application,
        |application| {
            application
                .restoration_diagnostics()
                .is_some_and(|state| state.last_save_error.is_none())
        },
        Instant::now() + Duration::from_secs(5),
    );
    let diagnostics = application.restoration_diagnostics().unwrap();
    assert_eq!(diagnostics.last_save_error, None);
    let recovered = store.bytes().expect("reduced snapshot persisted");
    let recovered: Value = serde_json::from_slice(&recovered).unwrap();
    assert_eq!(
        recovered["state"]["application/payload"],
        json!("recovered")
    );
}

fn drive_until(
    application: &mut Application,
    mut condition: impl FnMut(&Application) -> bool,
    deadline: Instant,
) {
    while Instant::now() < deadline {
        application.process_runtime_work();
        if condition(application) {
            return;
        }
        thread::sleep(Duration::from_millis(1));
    }
    panic!("condition was not reached before deadline");
}

struct FailingStore {
    fail_saves: AtomicBool,
    save_attempts: AtomicUsize,
}

impl FailingStore {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            fail_saves: AtomicBool::new(true),
            save_attempts: AtomicUsize::new(0),
        })
    }

    fn attempts(&self) -> usize {
        self.save_attempts.load(Ordering::SeqCst)
    }
}

impl RestorationStore for FailingStore {
    fn load(&self) -> Result<Option<Vec<u8>>, RestorationStoreError> {
        Ok(None)
    }

    fn save(&self, _bytes: &[u8]) -> Result<(), RestorationStoreError> {
        self.save_attempts.fetch_add(1, Ordering::SeqCst);
        if self.fail_saves.load(Ordering::SeqCst) {
            Err(RestorationStoreError::new("persistent store outage"))
        } else {
            Ok(())
        }
    }

    fn remove(&self) -> Result<(), RestorationStoreError> {
        Ok(())
    }
}

fn snapshot_bytes(application_id: &str, state: Value, windows: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "framework_format_version": FRAMEWORK_RESTORATION_FORMAT_VERSION,
        "application_schema_version": 1,
        "snapshot_generation": 1,
        "application_id": application_id,
        "state": state,
        "windows": windows,
    }))
    .unwrap()
}

fn window_descriptor(restoration_id: &str, kind: &str) -> Value {
    json!({
        "restoration_id": restoration_id,
        "kind": kind,
        "logical_width": 480.,
        "logical_height": 260.,
        "maximized": false,
        "fullscreen": false,
    })
}

fn restorable_application(
    store: Arc<InMemoryRestorationStore>,
    application_id: &str,
) -> Application {
    Application::new_restorable(
        WindowRestorationId::new("main").unwrap(),
        "main",
        WindowOptions::default(),
        RestorationConfig::new(application_id, 1, store).with_debounce(Duration::ZERO),
        |_| test_widget(),
    )
    .unwrap()
}

#[test]
fn claimed_scopes_reject_a_second_live_mount_of_the_same_path() {
    let scope_slot = Rc::new(RefCell::new(None));
    let handle_slot = Rc::new(RefCell::new(None));
    let scope_for_build = scope_slot.clone();
    let handle_for_build = handle_slot.clone();
    let application = Application::new_with_restoration(
        WindowOptions::default(),
        RestorationConfig::new(
            "com.example.claim",
            1,
            Arc::new(InMemoryRestorationStore::new()),
        ),
        move |cx| {
            *scope_for_build.borrow_mut() = cx.restoration_scope();
            *handle_for_build.borrow_mut() = cx.restoration();
            test_widget()
        },
    )
    .unwrap();
    let scope = scope_slot.borrow().as_ref().unwrap().clone();
    let handle = handle_slot.borrow().as_ref().unwrap().clone();
    let first = handle.claim_scope(&scope).expect("first claim succeeds");
    assert!(handle.claim_scope(&scope).is_err());
    drop(first);
    assert!(handle.claim_scope(&scope).is_ok());
    drop(application);
}

#[test]
fn restore_skips_corrupt_duplicate_and_unknown_descriptors() {
    let store = Arc::new(InMemoryRestorationStore::new());
    store.set_bytes(Some(snapshot_bytes(
        "com.example.win-skip",
        json!({}),
        json!([
            window_descriptor("inspector", "inspector"),
            window_descriptor("inspector", "inspector"),
            window_descriptor("", "inspector"),
            window_descriptor("ghost", "unregistered-kind"),
        ]),
    )));
    let mut application = restorable_application(store, "com.example.win-skip");
    application
        .register_restorable_window_factory("inspector", WindowOptions::default(), |_| {
            test_widget()
        })
        .unwrap();
    assert_eq!(application.restore_restorable_windows().unwrap(), 1);
    assert_eq!(application.active_window_ids().len(), 2);
    let diagnostics = application.restoration_diagnostics().unwrap();
    assert_eq!(diagnostics.restored_windows, 1);
    assert_eq!(diagnostics.skipped_windows, 2);
    application.shutdown();
}

#[test]
fn user_close_retains_unopened_descriptors_but_drops_the_closed_one() {
    let store = Arc::new(InMemoryRestorationStore::new());
    store.set_bytes(Some(snapshot_bytes(
        "com.example.win-close",
        json!({}),
        json!([window_descriptor("panel", "unregistered-kind")]),
    )));
    let mut application = restorable_application(store.clone(), "com.example.win-close");
    application
        .register_restorable_window_factory("inspector", WindowOptions::default(), |_| {
            test_widget()
        })
        .unwrap();
    assert_eq!(application.restore_restorable_windows().unwrap(), 0);
    let inspector = application
        .open_restorable_window_with(
            WindowRestorationId::new("inspector").unwrap(),
            "inspector",
            WindowOptions::default(),
            |_| test_widget(),
        )
        .unwrap();
    assert!(application.close_window(inspector.id()));
    application.shutdown();
    let persisted = store.bytes().expect("close persists descriptors");
    let document: Value = serde_json::from_slice(&persisted).unwrap();
    let ids: Vec<&str> = document["windows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|window| window["restoration_id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&"panel"));
    assert!(!ids.contains(&"inspector"));
}

#[test]
fn user_close_purges_window_scoped_values_but_keeps_siblings() {
    let main_scope_slot = Rc::new(RefCell::new(None));
    let main_scope_for_build = main_scope_slot.clone();
    let store = Arc::new(InMemoryRestorationStore::new());
    let mut application = Application::new_restorable(
        WindowRestorationId::new("main").unwrap(),
        "main",
        WindowOptions::default(),
        RestorationConfig::new("com.example.win-values", 1, store.clone())
            .with_debounce(Duration::ZERO),
        move |cx| {
            *main_scope_for_build.borrow_mut() = cx.restoration_scope();
            test_widget()
        },
    )
    .unwrap();
    let inspector_scope_slot = Rc::new(RefCell::new(None));
    let inspector_scope_for_build = inspector_scope_slot.clone();
    let inspector = application
        .open_restorable_window_with(
            WindowRestorationId::new("inspector").unwrap(),
            "inspector",
            WindowOptions::default(),
            move |cx| {
                *inspector_scope_for_build.borrow_mut() = cx.restoration_scope();
                test_widget()
            },
        )
        .unwrap();
    let main_scope = main_scope_slot.borrow().as_ref().unwrap().clone();
    let inspector_scope = inspector_scope_slot.borrow().as_ref().unwrap().clone();
    main_scope.set_json(&key("draft"), json!("M"));
    inspector_scope.set_json(&key("draft"), json!("I"));
    assert!(application.close_window(inspector.id()));
    assert_eq!(main_scope.get_json(&key("draft")), Some(json!("M")));
    assert_eq!(inspector_scope.get_json(&key("draft")), None);
    application.shutdown();
    let persisted = store.bytes().expect("close persists values");
    let document: Value = serde_json::from_slice(&persisted).unwrap();
    let state = document["state"].as_object().unwrap();
    assert!(state.keys().all(|path| !path.contains("inspector")));
    assert!(
        state.values().any(|value| value == &json!("M")),
        "sibling window values survive"
    );
}

#[test]
fn type_incompatible_stored_values_fall_back_and_count_as_stale() {
    let scope_slot = Rc::new(RefCell::new(None));
    let scope_for_build = scope_slot.clone();
    let application = Application::new_with_restoration(
        WindowOptions::default(),
        RestorationConfig::new(
            "com.example.stale",
            1,
            Arc::new(InMemoryRestorationStore::new()),
        ),
        move |cx| {
            *scope_for_build.borrow_mut() = cx.restoration_scope();
            test_widget()
        },
    )
    .unwrap();
    let scope = scope_slot.borrow().as_ref().unwrap().clone();
    scope.set_json(&key("count"), json!("not-a-number"));
    let value = Restorable::from_scope(scope, key("count"), 0_u32);
    assert_eq!(value.get(), 0);
    assert_eq!(
        application
            .restoration_diagnostics()
            .unwrap()
            .stale_or_invalid_values,
        1
    );
    drop(application);
}

#[test]
fn oversize_snapshots_fall_back_on_load_without_destroying_data() {
    let store = Arc::new(InMemoryRestorationStore::new());
    store.set_bytes(Some(snapshot_bytes(
        "com.example.oversize-load",
        json!({"application/big": "x".repeat(200)}),
        json!([]),
    )));
    let value_slot = Rc::new(RefCell::new(None));
    let value_for_build = value_slot.clone();
    let application = Application::new_with_restoration(
        WindowOptions::default(),
        RestorationConfig::new("com.example.oversize-load", 1, store.clone())
            .with_snapshot_limit(64),
        move |cx| {
            *value_for_build.borrow_mut() = cx.restorable(key("big"), String::new());
            test_widget()
        },
    )
    .unwrap();
    assert_eq!(value_slot.borrow().as_ref().unwrap().get(), String::new());
    let diagnostics = application.restoration_diagnostics().unwrap();
    assert_eq!(diagnostics.restoration_load_failures, 1);
    assert_eq!(diagnostics.restoration_values, 0);
    drop(application);
    let preserved: Value = serde_json::from_slice(&store.bytes().unwrap()).unwrap();
    assert_eq!(
        preserved["state"]["application/big"],
        json!("x".repeat(200))
    );
}

#[test]
fn file_backed_application_ids_reject_filesystem_unsafe_values() {
    for unsafe_id in ["", "   ", ".", "..", "a/b", "a\\b", "../evil", "evil\u{0}"] {
        assert!(
            FileRestorationStore::for_application(unsafe_id).is_err(),
            "{unsafe_id:?} must be rejected"
        );
    }
    let store = FileRestorationStore::for_application("org.incular.restore-test")
        .expect("ordinary reverse-domain ids stay valid");
    assert_eq!(
        store.path().file_name().and_then(|name| name.to_str()),
        Some("restoration.json")
    );
}

#[test]
fn into_runtime_flushes_pending_restoration() {
    let store = Arc::new(InMemoryRestorationStore::new());
    let value_slot = Rc::new(RefCell::new(None));
    let value_for_build = value_slot.clone();
    let application = Application::new_with_restoration(
        WindowOptions::default(),
        RestorationConfig::new("com.example.into-runtime", 1, store.clone()),
        move |cx| {
            *value_for_build.borrow_mut() = cx.restorable(key("count"), 0_u32);
            test_widget()
        },
    )
    .unwrap();
    value_slot.borrow().as_ref().unwrap().set(11);
    let runtime = application.into_runtime();
    drop(runtime);
    let persisted = store.bytes().expect("into_runtime flushed the snapshot");
    let document: Value = serde_json::from_slice(&persisted).unwrap();
    assert_eq!(document["state"]["application/count"], json!(11));
}
