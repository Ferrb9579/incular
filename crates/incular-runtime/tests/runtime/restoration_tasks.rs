use super::*;

#[test]
fn tokio_sleep_completion_wakes_the_ui_bridge() {
    let mut runtime = Runtime::new(Widget::box_(Size::new(1., 1.), Color::WHITE)).unwrap();
    let wake = Arc::new(TestWake::default());
    runtime.set_wake_handler(wake.clone());
    let completed = Arc::new(AtomicBool::new(false));
    let observed = completed.clone();
    runtime.spawner().spawn_into(
        async move {
            tokio::time::sleep(Duration::from_millis(1)).await;
        },
        move |result, _| {
            assert!(result.is_ok());
            observed.store(true, Ordering::Release);
        },
    );
    wait_for_wake(&wake);
    runtime.process_runtime_work();
    assert!(completed.load(Ordering::Acquire));
    assert_eq!(runtime.runtime_diagnostics().active_tracked_tasks, 0);
}

#[test]
fn async_signal_completion_invalidates_only_its_dependent_build() {
    let mut runtime = Runtime::new(Widget::from(incular_widgets::Row::new(vec![
        Widget::box_(Size::new(1., 1.), Color::WHITE),
        Widget::box_(Size::new(1., 1.), Color::WHITE),
    ])))
    .unwrap();
    let root = runtime.tree().root().unwrap();
    let dependent = runtime.tree().children(root).unwrap()[1];
    let signal = Signal::with_runtime(1_u32, &runtime);
    let observed = signal.clone();
    runtime
        .register_builder(dependent, move || {
            Widget::box_(Size::new(observed.get() as f32, 1.), Color::WHITE)
        })
        .unwrap();
    let wake = Arc::new(TestWake::default());
    runtime.set_wake_handler(wake.clone());
    let spawner = runtime.spawner();
    let completion = signal.clone();
    spawner.spawn_into(async { 2_u32 }, move |result, _| {
        completion.set(result.expect("Tokio result"));
    });
    wait_for_wake(&wake);
    let start = Instant::now();
    runtime.process_runtime_work_at(start);
    assert!(runtime.frame_requested());
    let (_, frame) = runtime
        .run_frame_at(
            Constraints::tight(Size::new(20., 20.)),
            start + Duration::from_millis(1),
        )
        .unwrap();
    assert_eq!(frame.updated_elements, 1);
    assert!(!runtime.tree().is_build_dirty(root));
}

#[test]
fn cancelled_scope_never_runs_its_ready_task() {
    let mut runtime = Runtime::new(Widget::box_(Size::new(1., 1.), Color::WHITE)).unwrap();
    let wake = Arc::new(TestWake::default());
    runtime.set_wake_handler(wake.clone());
    let spawner = runtime.spawner();
    let scope = spawner.scope();
    let hit = Arc::new(AtomicBool::new(false));
    let observed = hit.clone();
    let handle = spawner.spawn_in(&scope, async move {
        std::future::pending::<()>().await;
        observed.store(true, Ordering::Release);
    });
    scope.cancel();
    wait_for_wake(&wake);
    runtime.process_runtime_work();
    assert!(!hit.load(Ordering::Acquire));
    assert_eq!(handle.value(), AsyncValue::Error(TaskFailure::Cancelled));
    assert_eq!(runtime.runtime_diagnostics().tasks_cancelled, 1);
}

#[test]
fn cancelled_scoped_completion_is_discarded_after_unmount() {
    let mut runtime = Runtime::new(Widget::from(incular_widgets::Row::new(vec![Widget::box_(
        Size::new(1., 1.),
        Color::WHITE,
    )])))
    .unwrap();
    let root = runtime.tree().root().unwrap();
    let child = runtime.tree().children(root).unwrap()[0];
    let wake = Arc::new(TestWake::default());
    runtime.set_wake_handler(wake.clone());
    let scope = runtime.spawner().scope();
    let hit = Arc::new(AtomicBool::new(false));
    let observed = hit.clone();
    runtime
        .spawner()
        .spawn_into_in(&scope, async { 7_u8 }, move |_, _| {
            observed.store(true, Ordering::Release);
        });
    wait_for_wake(&wake);

    // Public scope cancellation is the lifetime boundary available to an
    // application-owned task. The completion must not run after the element
    // it was associated with has been removed.
    runtime
        .tree_mut()
        .update(
            root,
            Widget::from(incular_widgets::Row::new(Vec::<Widget>::new())),
        )
        .unwrap();
    assert!(!runtime.tree().element_exists(child));
    scope.cancel();
    runtime.process_runtime_work();
    assert!(!hit.load(Ordering::Acquire));
    assert!(runtime.runtime_diagnostics().tasks_cancelled >= 1);
}

#[test]
fn application_task_survives_an_unrelated_component_unmount() {
    let mut runtime = Runtime::new(Widget::from(incular_widgets::Row::new(vec![Widget::box_(
        Size::new(1., 1.),
        Color::WHITE,
    )])))
    .unwrap();
    let root = runtime.tree().root().unwrap();
    let wake = Arc::new(TestWake::default());
    runtime.set_wake_handler(wake.clone());
    let hit = Arc::new(AtomicBool::new(false));
    let observed = hit.clone();
    runtime.spawner().spawn_into(async { 1_u8 }, move |_, _| {
        observed.store(true, Ordering::Release);
    });
    wait_for_wake(&wake);
    runtime
        .tree_mut()
        .update(
            root,
            Widget::from(incular_widgets::Row::new(Vec::<Widget>::new())),
        )
        .unwrap();
    runtime.process_runtime_work();
    assert!(hit.load(Ordering::Acquire));
}

#[test]
fn blocking_result_after_owner_unmount_is_discarded() {
    let mut runtime = Runtime::new(Widget::from(incular_widgets::Row::new(vec![Widget::box_(
        Size::new(1., 1.),
        Color::WHITE,
    )])))
    .unwrap();
    let root = runtime.tree().root().unwrap();
    let wake = Arc::new(TestWake::default());
    runtime.set_wake_handler(wake.clone());
    let scope = runtime.spawner().scope();
    let hit = Arc::new(AtomicBool::new(false));
    let observed = hit.clone();
    runtime.spawner().spawn_blocking_in(
        &scope,
        || 42_u64,
        move |_, _| {
            observed.store(true, Ordering::Release);
        },
    );
    wait_for_wake(&wake);
    runtime
        .tree_mut()
        .update(
            root,
            Widget::from(incular_widgets::Row::new(Vec::<Widget>::new())),
        )
        .unwrap();
    scope.cancel();
    runtime.process_runtime_work();
    assert!(!hit.load(Ordering::Acquire));
    assert_eq!(runtime.runtime_diagnostics().blocking_results_discarded, 1);
}

#[test]
fn task_panic_reaches_the_runtime_error_hook() {
    let mut runtime = Runtime::new(Widget::box_(Size::new(1., 1.), Color::WHITE)).unwrap();
    let wake = Arc::new(TestWake::default());
    runtime.set_wake_handler(wake.clone());
    let panics = Rc::new(Cell::new(0));
    let observed = panics.clone();
    runtime.observe_runtime_error(move |report| {
        if report.failure == TaskFailure::Panicked {
            observed.set(observed.get() + 1);
        }
    });
    runtime.spawner().spawn(async {
        panic!("Tokio task panic stays outside Winit");
    });
    wait_for_wake(&wake);
    runtime.process_runtime_work();
    assert_eq!(panics.get(), 1);
    assert_eq!(runtime.runtime_diagnostics().task_panics, 1);
}

#[test]
fn ordinary_result_error_remains_application_data() {
    let mut runtime = Runtime::new(Widget::box_(Size::new(1., 1.), Color::WHITE)).unwrap();
    let wake = Arc::new(TestWake::default());
    runtime.set_wake_handler(wake.clone());
    let recovered = Arc::new(AtomicBool::new(false));
    let observed = recovered.clone();
    runtime.spawner().spawn_into(
        async { Result::<u8, &'static str>::Err("expected validation error") },
        move |result, _| {
            assert_eq!(
                result.expect("Tokio completion"),
                Err("expected validation error")
            );
            observed.store(true, Ordering::Release);
        },
    );
    wait_for_wake(&wake);
    runtime.process_runtime_work();
    assert!(recovered.load(Ordering::Acquire));
}

#[test]
fn completion_can_start_follow_up_tokio_work_without_reentrant_ui_borrow() {
    let mut runtime = Runtime::new(Widget::box_(Size::new(1., 1.), Color::WHITE)).unwrap();
    let wake = Arc::new(TestWake::default());
    runtime.set_wake_handler(wake.clone());
    let completed = Arc::new(AtomicU64::new(0));
    let observed = completed.clone();
    runtime
        .spawner()
        .spawn_into(async { 1_u8 }, move |_, runtime| {
            let observed = observed.clone();
            runtime.spawn_into(async { 2_u8 }, move |_, _| {
                observed.store(2, Ordering::Release);
            });
        });
    wait_for_wake(&wake);
    runtime.process_runtime_work();
    let first_wakes = wake.0.load(Ordering::Acquire);
    for _ in 0..100 {
        if wake.0.load(Ordering::Acquire) > first_wakes {
            break;
        }
        std::thread::park_timeout(Duration::from_millis(2));
    }
    runtime.process_runtime_work();
    assert_eq!(completed.load(Ordering::Acquire), 2);
}

#[test]
fn wake_coalescing_and_message_budget_preserve_native_fairness() {
    let mut runtime = Runtime::new(Widget::box_(Size::new(1., 1.), Color::WHITE)).unwrap();
    let wake = Arc::new(TestWake::default());
    runtime.set_wake_handler(wake.clone());
    let processed = Arc::new(AtomicU64::new(0));
    let dispatcher = runtime.dispatcher();
    for _ in 0..129 {
        let observed = processed.clone();
        dispatcher.dispatch(move |_| {
            observed.fetch_add(1, Ordering::AcqRel);
        });
    }
    assert_eq!(wake.0.load(Ordering::Acquire), 1);
    runtime.process_runtime_work();
    assert_eq!(processed.load(Ordering::Acquire), 128);
    let diagnostics = runtime.runtime_diagnostics();
    assert!(diagnostics.coalesced_wakes >= 128);
    assert_eq!(diagnostics.ui_messages_processed, 128);
    runtime.process_runtime_work();
    assert_eq!(processed.load(Ordering::Acquire), 129);
}

#[test]
fn completion_without_ui_mutation_does_not_request_a_frame_and_idle_does_not_spin() {
    let mut runtime = Runtime::new(Widget::box_(Size::new(1., 1.), Color::WHITE)).unwrap();
    runtime
        .run_frame(Constraints::tight(Size::new(10., 10.)))
        .unwrap();
    assert!(!runtime.frame_requested());
    let before = runtime.runtime_diagnostics();
    runtime.process_runtime_work();
    assert_eq!(
        runtime.runtime_diagnostics().ui_messages_processed,
        before.ui_messages_processed
    );

    runtime.dispatcher().dispatch(|_| {});
    runtime.process_runtime_work();
    assert!(!runtime.frame_requested());
}

#[test]
fn shutdown_cancels_tracked_tokio_tasks() {
    let mut runtime = Runtime::new(Widget::box_(Size::new(1., 1.), Color::WHITE)).unwrap();
    let task = runtime
        .spawner()
        .spawn(async { std::future::pending::<u8>().await });
    runtime.shutdown();
    assert_eq!(task.value(), AsyncValue::Error(TaskFailure::Cancelled));
    assert_eq!(runtime.runtime_diagnostics().active_tracked_tasks, 0);
}

#[test]
fn shutdown_discards_dispatched_callbacks() {
    let mut runtime = Runtime::new(Widget::box_(Size::new(1., 1.), Color::WHITE)).unwrap();
    let dispatcher = runtime.dispatcher();
    let hit = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let observed = hit.clone();
    dispatcher.dispatch(move |_| observed.store(true, std::sync::atomic::Ordering::Release));
    runtime.shutdown();
    runtime.process_runtime_work();
    assert!(!hit.load(std::sync::atomic::Ordering::Acquire));
}

#[test]
fn restoration_persists_opt_in_values_before_the_next_build() {
    fn make_application(
        store: Arc<InMemoryRestorationStore>,
        slot: Rc<RefCell<Option<Restorable<u32>>>>,
    ) -> Application {
        let build_slot = slot.clone();
        Application::new_with_restoration(
            test_window_options("Restoration", 200., 100.),
            RestorationConfig::new("com.example.runtime-restoration", 1, store)
                .with_debounce(Duration::ZERO),
            move |cx| {
                if build_slot.borrow().is_none() {
                    *build_slot.borrow_mut() =
                        cx.restorable(RestorationKey::new("count").unwrap(), 0_u32);
                }
                Text::new(build_slot.borrow().as_ref().unwrap().get().to_string()).into()
            },
        )
        .unwrap()
    }

    let store = Arc::new(InMemoryRestorationStore::new());
    let first_slot = Rc::new(RefCell::new(None));
    let mut first = make_application(store.clone(), first_slot.clone());
    first_slot.borrow().as_ref().unwrap().set(17);
    first.shutdown();
    assert!(store.bytes().is_some(), "shutdown flush stores a snapshot");

    let restored_slot = Rc::new(RefCell::new(None));
    let restored = make_application(store.clone(), restored_slot.clone());
    assert_eq!(restored_slot.borrow().as_ref().unwrap().get(), 17);
    drop(restored);
}

#[test]
fn restorable_windows_use_stable_ids_and_user_close_removes_auxiliary_descriptor() {
    let store = Arc::new(InMemoryRestorationStore::new());
    let config = RestorationConfig::new("com.example.window-restore", 1, store.clone())
        .with_debounce(Duration::ZERO);
    let mut first = Application::new_restorable(
        WindowRestorationId::new("main").unwrap(),
        "main",
        test_window_options("Main", 320., 200.),
        config,
        |_| Widget::box_(Size::new(1., 1.), Color::WHITE),
    )
    .unwrap();
    let inspector = first
        .open_restorable_window_with(
            WindowRestorationId::new("inspector").unwrap(),
            "inspector",
            test_window_options("Inspector", 480., 260.),
            |_| Widget::box_(Size::new(1., 1.), Color::WHITE),
        )
        .unwrap();
    assert_eq!(first.active_window_ids().len(), 2);
    first.shutdown();

    let config = RestorationConfig::new("com.example.window-restore", 1, store.clone())
        .with_debounce(Duration::ZERO);
    let mut second = Application::new_restorable(
        WindowRestorationId::new("main").unwrap(),
        "main",
        test_window_options("Main default", 100., 100.),
        config,
        |_| Widget::box_(Size::new(1., 1.), Color::WHITE),
    )
    .unwrap();
    second
        .register_restorable_window_factory(
            "inspector",
            test_window_options("Inspector default", 100., 100.),
            |_| Widget::box_(Size::new(1., 1.), Color::WHITE),
        )
        .unwrap();
    assert_eq!(second.restore_restorable_windows().unwrap(), 1);
    assert_eq!(second.active_window_ids().len(), 2);
    let inspector_id = second
        .active_window_ids()
        .into_iter()
        .find(|id| *id != second.primary_window())
        .unwrap();
    assert_eq!(
        second
            .window_diagnostics(inspector_id)
            .unwrap()
            .logical_size,
        Size::new(480., 260.)
    );
    assert!(second.close_window(inspector_id));
    second.shutdown();
    let bytes = store.bytes().unwrap();
    let snapshot: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(snapshot["windows"].as_array().unwrap().len(), 1);
    // Session-local WindowIds remain unrelated to the stable persistence
    // key; reopening the same descriptor never serializes this value.
    assert_ne!(inspector.id(), second.primary_window());
}
