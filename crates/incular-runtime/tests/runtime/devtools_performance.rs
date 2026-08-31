use super::*;

#[test]
fn installed_performance_overlay_receives_presented_renderer_metrics() {
    let mut app =
        Application::new(|_| incular_widgets::internal::performance_overlay_placeholder())
            .expect("performance application");
    app.set_profiler_mode(ProfilerMode::Diagnostic);
    let window = app.primary_window();
    app.install_performance_overlay(window)
        .expect("install performance overlay");
    app.run_window_frame_at(
        window,
        Constraints::tight(Size::new(320.0, 180.0)),
        Instant::now(),
    )
    .expect("initial overlay frame");
    app.note_presented(window, true);
    app.note_render_metrics(
        window,
        RenderFrameMetrics {
            draw_calls: 7,
            instances: 13,
            render_passes: 1,
            prepare_us: 120,
            encode_us: 80,
            submit_us: 40,
            ..RenderFrameMetrics::default()
        },
        Some(GpuSample {
            supported: true,
            frame: 1,
            main_pass_us: 75.0,
        }),
    );

    let snapshot = app.performance_hub().snapshot();
    assert_eq!(snapshot.windows[0].render.draw_calls, 7);
    assert_eq!(snapshot.windows[0].render.instances, 13);
    assert_eq!(
        snapshot.windows[0].gpu.expect("GPU sample").main_pass_us,
        75.0
    );
    assert!(snapshot.widgets.elements_total > 0);
    assert!(app.frame_requested(window));
}

#[cfg(feature = "devtools")]
#[test]
fn editable_signal_uses_typed_ui_thread_set_and_keeps_old_new_summary() {
    let signal = Signal::new(3_i64)
        .devtools("runtime-editable-signal-test")
        .devtools_editable();
    let (edited, writes, summaries, wrong_type) = devtools_registry::with_all(|signals| {
        let registration = signals
            .iter()
            .find(|(_, registration)| {
                registration.name.as_deref() == Some("runtime-editable-signal-test")
            })
            .map(|(_, registration)| registration)
            .expect("registered signal");
        let edited = (registration.apply_edit)(&incular_devtools_protocol::EditableValue::Int(9));
        let writes = (registration.write_count)();
        let summaries = (registration.last_write)();
        let wrong_type = (registration.apply_edit)(&incular_devtools_protocol::EditableValue::Str(
            "wrong type".into(),
        ));
        (edited, writes, summaries, wrong_type)
    });

    assert!(edited);
    assert_eq!(signal.get(), 9);
    assert_eq!(writes, 1);
    assert_eq!(summaries, (Some("3".into()), Some("9".into())));
    assert!(!wrong_type);
}

#[cfg(feature = "devtools")]
#[test]
fn named_signal_write_reaches_dependent_rebuild_cause() {
    let mut runtime = Runtime::new(Widget::from(incular_widgets::Row::new(vec![Widget::box_(
        Size::new(1., 1.),
        Color::WHITE,
    )])))
    .unwrap();
    let root = runtime.tree().root().unwrap();
    let dependent = runtime.tree().children(root).unwrap()[0];
    let signal = Signal::with_runtime(1_u32, &runtime).devtools("counter");
    let state = signal.clone();
    runtime
        .register_builder(dependent, move || {
            Widget::box_(Size::new(state.get() as f32, 1.), Color::WHITE)
        })
        .unwrap();
    assert!(signal.set(2));
    runtime
        .run_frame(Constraints::tight(Size::new(20., 20.)))
        .unwrap();
    let id = runtime
        .tree()
        .devtools_id_for_element(dependent)
        .expect("devtools id");
    let details = runtime
        .tree()
        .devtools_node_details(
            dependent,
            id,
            incular_devtools_protocol::DevWindowId::new(1, 0),
        )
        .expect("details");
    assert!(details.invalidation_causes.iter().any(|cause| matches!(
        cause,
        incular_devtools_protocol::InvalidationReason::SignalWrite {
            name,
            old: Some(old),
            new: Some(new),
            ..
        } if name == "counter" && old == "1" && new == "2"
    )));
}

#[cfg(feature = "devtools")]
#[test]
fn tracked_task_completion_is_coalesced_with_its_signal_cause() {
    let mut runtime = Runtime::new(Widget::from(incular_widgets::Row::new(vec![Widget::box_(
        Size::new(1., 1.),
        Color::WHITE,
    )])))
    .unwrap();
    let root = runtime.tree().root().unwrap();
    let dependent = runtime.tree().children(root).unwrap()[0];
    let signal = Signal::with_runtime(1_u32, &runtime).devtools("async-counter");
    let observed = signal.clone();
    runtime
        .register_builder(dependent, move || {
            Widget::box_(Size::new(observed.get() as f32, 1.), Color::WHITE)
        })
        .unwrap();
    let wake = Arc::new(TestWake::default());
    runtime.set_wake_handler(wake.clone());
    let completion = signal.clone();
    runtime
        .spawner()
        .spawn_into(async { 2_u32 }, move |result, _| {
            completion.set(result.expect("Tokio result"));
        });
    wait_for_wake(&wake);
    runtime.process_runtime_work();
    runtime
        .run_frame(Constraints::tight(Size::new(20., 20.)))
        .unwrap();
    let id = runtime
        .tree()
        .devtools_id_for_element(dependent)
        .expect("id");
    let details = runtime
        .tree()
        .devtools_node_details(
            dependent,
            id,
            incular_devtools_protocol::DevWindowId::new(1, 0),
        )
        .expect("details");
    assert!(details.invalidation_causes.iter().any(|cause| matches!(
        cause,
        incular_devtools_protocol::InvalidationReason::TaskCompletion
    )));
    assert!(details.invalidation_causes.iter().any(|cause| matches!(
        cause,
        incular_devtools_protocol::InvalidationReason::SignalWrite { name, .. }
            if name == "async-counter"
    )));
}

#[cfg(feature = "devtools")]
#[test]
fn devtools_overlays_and_deep_trace_are_routed_to_one_window() {
    let mut application =
        Application::new(|_| Widget::box_(Size::new(80., 40.), Color::WHITE)).unwrap();
    let a = application.primary_window();
    let b = application
        .open_window_with(test_window_options("B", 240., 120.), |_| {
            Widget::box_(Size::new(30., 20.), Color::WHITE)
        })
        .unwrap()
        .id();
    for (window, size) in [(a, Size::new(100., 60.)), (b, Size::new(240., 120.))] {
        application.handle_window_event(WindowEvent::platform(
            window,
            PlatformEvent::Metrics(WindowMetrics::new(
                incular_platform::PhysicalSize::new(size.width as u32, size.height as u32),
                1.,
            )),
        ));
        application
            .run_window_frame_at(window, Constraints::tight(size), Instant::now())
            .unwrap();
    }
    let a_bounds = application.devtools_window_layout_bounds(a, 8);
    let b_bounds = application.devtools_window_layout_bounds(b, 8);
    assert_eq!(a_bounds[0][2..], [100., 60.]);
    assert_eq!(b_bounds[0][2..], [240., 120.]);

    assert!(application.devtools_begin_deep_trace(b, 128));
    application
        .run_window_frame_at(b, Constraints::tight(Size::new(250., 120.)), Instant::now())
        .unwrap();
    assert!(application.devtools_take_deep_trace(a, 1).is_none());
    let trace = application
        .devtools_take_deep_trace(b, 1)
        .expect("window B trace");
    assert_eq!(trace.window.index(), u64::from(b.index()) + 1);
    assert!(!trace.events.is_empty());
}

#[cfg(feature = "devtools")]
#[test]
fn devtools_property_edit_routes_to_the_live_window_and_requests_a_frame() {
    let mut application = Application::new(|_| {
        Widget::from(incular_widgets::Opacity::new(
            0.8,
            Widget::box_(Size::new(20., 20.), Color::WHITE),
        ))
    })
    .unwrap();
    let window = application.primary_window();
    application
        .run_window_frame_at(
            window,
            Constraints::tight(Size::new(40., 40.)),
            Instant::now(),
        )
        .unwrap();
    let protocol_window = incular_devtools_protocol::DevWindowId::new(
        u64::from(window.index()) + 1,
        u64::from(window.generation()),
    );
    let snapshot = application
        .devtools_widget_tree(protocol_window)
        .expect("widget tree");
    let incular_devtools_protocol::TreeDelta::Snapshot { root, .. } = &snapshot[0] else {
        panic!("full tree snapshot");
    };
    assert!(application.devtools_edit_property(
        root.id,
        "opacity",
        &incular_devtools_protocol::DebugValue::Float(0.3)
    ));
    assert!(application.frame_requested(window));
    let details = application
        .devtools_node_details(root.id)
        .expect("edited details");
    assert!(details.properties.iter().any(|property| {
        property.name == "opacity"
            && matches!(property.value, incular_devtools_protocol::DebugValue::Float(value) if (value - 0.3).abs() < 0.000_001)
    }));
}
