use incular_core::{Color, RestorationBackend, RestorationKey, RestorationScope, Size};
use incular_platform::WindowOptions;
use incular_runtime::{
    Application, FRAMEWORK_RESTORATION_FORMAT_VERSION, FileRestorationStore,
    InMemoryRestorationStore, Restorable, RestorationConfig, RestorationStore,
};
use incular_widgets::Widget;
use serde_json::{Value, json};
use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc, time::Duration};

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
