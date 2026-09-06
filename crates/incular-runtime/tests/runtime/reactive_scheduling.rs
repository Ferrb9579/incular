use super::*;

#[test]
fn canonical_signal_tracks_core_context_and_untracked_reads_do_not() {
    let context = incular_core::BuildContext::new();
    let tracked = Signal::new(1_u32);
    let untracked = Signal::new(2_u32);
    context.build(|_| {
        assert_eq!(tracked.get(), 1);
        assert_eq!(untracked.get_untracked(), 2);
    });
    assert_eq!(tracked.dependent_count(), 1);
    assert_eq!(untracked.dependent_count(), 0);
    untracked.set(3);
    assert!(!context.is_dirty());
    tracked.set(4);
    assert!(context.is_dirty());
    drop(context);
    assert_eq!(tracked.dependent_count(), 0);
}

#[test]
fn controller_reads_flow_through_memos_and_owned_listeners() {
    let controller = TextEditingController::from_value(incular_text::TextEditingValue::new("a"));
    let calls = Rc::new(Cell::new(0));
    let listener = controller.observe({
        let calls = calls.clone();
        move |_| calls.set(calls.get() + 1)
    });
    let memo = Memo::new({
        let controller = controller.clone();
        move || controller.text().len()
    });
    let seen = Rc::new(Cell::new(0));
    let mut runtime = Application::new({
        let seen = seen.clone();
        move |_| {
            seen.set(memo.get());
            Widget::box_(Size::new(1., 1.), Color::WHITE)
        }
    })
    .unwrap()
    .into_runtime();
    let constraints = Constraints::tight(Size::new(20., 20.));
    runtime.run_frame(constraints).unwrap();
    controller.set_text("abcd");
    runtime.run_frame(constraints).unwrap();
    assert_eq!(seen.get(), 4);
    assert_eq!(calls.get(), 1);
    drop(listener);
    controller.set_text("ab");
    runtime.run_frame(constraints).unwrap();
    assert_eq!(seen.get(), 2);
    assert_eq!(calls.get(), 1);
}

#[test]
fn diamond_memos_deliver_consistent_values_and_dispose_effect_edges() {
    let source = Signal::new(1_u32);
    let left = Memo::new({
        let source = source.clone();
        move || source.get() * 2
    });
    let right = Memo::new({
        let source = source.clone();
        move || source.get() * 3
    });
    let sum = Memo::new(move || left.get() + right.get());
    let values = Rc::new(RefCell::new(Vec::new()));
    let effect = Effect::new({
        let values = values.clone();
        move || values.borrow_mut().push(sum.get())
    });
    let mut runtime = Application::new({
        let effect = effect.clone();
        move |_| {
            assert!(effect.mount());
            Widget::box_(Size::new(1., 1.), Color::WHITE)
        }
    })
    .unwrap()
    .into_runtime();
    let constraints = Constraints::tight(Size::new(20., 20.));
    runtime.run_frame(constraints).unwrap();
    source.set(2);
    source.set(3);
    runtime.run_frame(constraints).unwrap();
    assert_eq!(*values.borrow(), vec![5, 15]);
    effect.dispose();
    source.set(4);
    runtime.run_frame(constraints).unwrap();
    assert_eq!(*values.borrow(), vec![5, 15]);
    drop(runtime);
    assert_eq!(source.dependent_count(), 0);
}

#[cfg(feature = "devtools")]
#[test]
fn diagnostics_rename_in_place_and_allow_owner_drop_during_snapshot() {
    let signal = Signal::new(1_i64)
        .devtools("stage-c-before")
        .devtools_editable();
    let id = devtools_registry::with_all(|entries| {
        entries
            .iter()
            .find(|(_, item)| item.name.as_deref() == Some("stage-c-before"))
            .unwrap()
            .0
    });
    let signal = signal.devtools("stage-c-after");
    devtools_registry::with_all(move |entries| {
        let entry = entries
            .iter()
            .find(|(entry_id, _)| *entry_id == id)
            .unwrap();
        assert_eq!(entry.1.name.as_deref(), Some("stage-c-after"));
        assert!(entry.1.editable_kind.is_some());
        drop(signal);
    });
    devtools_registry::with_all(|entries| {
        assert!(!entries.iter().any(|(entry_id, _)| *entry_id == id))
    });
}

#[test]
fn signals_schedule_only_subscribed_element_once() {
    let mut runtime = Runtime::new(Widget::from(incular_widgets::Row::new(vec![
        Widget::box_(Size::new(1., 1.), Color::WHITE),
        Widget::box_(Size::new(2., 2.), Color::WHITE),
    ])))
    .unwrap();
    let root = runtime.tree().root().unwrap();
    let children = runtime.tree().children(root).unwrap().to_vec();
    let signal = Signal::new(2_u32);
    let state = signal.clone();
    runtime
        .register_builder(children[1], move || {
            Widget::box_(Size::new(state.get() as f32, 2.), Color::WHITE)
        })
        .unwrap();
    assert_eq!(signal.dependent_count(), 1);
    assert!(signal.set(3));
    assert!(signal.set(4));
    let (_, stats) = runtime
        .run_frame(Constraints::tight(Size::new(20., 20.)))
        .unwrap();
    assert_eq!(stats.updated_elements, 1);
    assert!(!runtime.tree().is_build_dirty(children[0]));
    assert!(!signal.set(4));
}

#[test]
fn equality_aware_signal_update_skips_noop_invalidations() {
    let signal = Signal::new(1_u32);

    assert!(!signal.update_if_changed(|value| *value = 1));
    assert_eq!(signal.get(), 1);
    assert!(signal.update_if_changed(|value| *value = 2));
    assert_eq!(signal.get(), 2);
}

#[test]
fn memo_tracks_sources_without_a_build_context_and_rebuilds_its_consumers() {
    let source = Signal::new(1_u32);
    let computations = Rc::new(Cell::new(0));
    let memo = Memo::new({
        let source = source.clone();
        let computations = computations.clone();
        move || {
            computations.set(computations.get() + 1);
            source.get() * 2
        }
    });
    let builds = Rc::new(Cell::new(0));
    let mut runtime = Application::new({
        let memo = memo.clone();
        let builds = builds.clone();
        move |_| {
            builds.set(builds.get() + 1);
            Widget::from(incular_widgets::Text::new(memo.with(ToString::to_string)))
        }
    })
    .unwrap()
    .into_runtime();
    let constraints = Constraints::tight(Size::new(100., 30.));

    runtime.run_frame(constraints).unwrap();
    assert_eq!(computations.get(), 1);
    assert_eq!(builds.get(), 1);

    source.set(2);
    source.set(3);
    let (_, stats) = runtime.run_frame(constraints).unwrap();
    assert_eq!(computations.get(), 2);
    assert_eq!(builds.get(), 2);
    assert_eq!(stats.updated_elements, 1);
}

#[test]
fn memo_filters_unchanged_results_before_invalidating_widgets() {
    let source = Signal::new(0_u32);
    let computations = Rc::new(Cell::new(0));
    let memo = Memo::new({
        let source = source.clone();
        let computations = computations.clone();
        move || {
            computations.set(computations.get() + 1);
            source.get() % 2
        }
    });
    let builds = Rc::new(Cell::new(0));
    let mut runtime = Application::new({
        let memo = memo.clone();
        let builds = builds.clone();
        move |_| {
            builds.set(builds.get() + 1);
            Widget::from(incular_widgets::Text::new(memo.get().to_string()))
        }
    })
    .unwrap()
    .into_runtime();
    let constraints = Constraints::tight(Size::new(100., 30.));
    runtime.run_frame(constraints).unwrap();

    source.set(2);
    let (_, stats) = runtime.run_frame(constraints).unwrap();
    assert_eq!(computations.get(), 2);
    assert_eq!(builds.get(), 1);
    assert_eq!(stats.updated_elements, 0);

    source.set(3);
    let (_, stats) = runtime.run_frame(constraints).unwrap();
    assert_eq!(computations.get(), 3);
    assert_eq!(builds.get(), 2);
    assert_eq!(stats.updated_elements, 1);
}

#[test]
fn shared_memo_tracks_each_window_without_losing_a_root_subscription() {
    let source = Signal::new(0_u32);
    let computations = Rc::new(Cell::new(0));
    let memo = Memo::new({
        let source = source.clone();
        let computations = computations.clone();
        move || {
            computations.set(computations.get() + 1);
            source.get() + 1
        }
    });
    let builds_a = Rc::new(Cell::new(0));
    let mut application = Application::new({
        let memo = memo.clone();
        let builds_a = builds_a.clone();
        move |_| {
            builds_a.set(builds_a.get() + 1);
            Widget::from(incular_widgets::Text::new(memo.get().to_string()))
        }
    })
    .unwrap();
    let builds_b = Rc::new(Cell::new(0));
    let window_b = application
        .open_window_with(test_window_options("memo", 100., 30.), {
            let memo = memo.clone();
            let builds_b = builds_b.clone();
            move |_| {
                builds_b.set(builds_b.get() + 1);
                Widget::from(incular_widgets::Text::new(memo.get().to_string()))
            }
        })
        .unwrap()
        .id();
    let window_a = application.primary_window();
    let constraints = Constraints::tight(Size::new(100., 30.));
    for window in [window_a, window_b] {
        application
            .run_window_frame_at(window, constraints, Instant::now())
            .unwrap();
    }
    assert_eq!(builds_a.get(), 1);
    assert_eq!(builds_b.get(), 1);

    source.set(1);
    application
        .run_window_frame_at(window_b, constraints, Instant::now())
        .unwrap();
    application
        .run_window_frame_at(window_a, constraints, Instant::now())
        .unwrap();
    assert_eq!(builds_a.get(), 2);
    assert_eq!(builds_b.get(), 2);
    assert_eq!(computations.get(), 3);
}

#[test]
fn memo_replaces_dynamic_branch_dependencies_after_a_switch() {
    let branch = Signal::new(true);
    let first = Signal::new(1_u32);
    let second = Signal::new(2_u32);
    let memo = Memo::new({
        let branch = branch.clone();
        let first = first.clone();
        let second = second.clone();
        move || {
            if branch.get() {
                first.get()
            } else {
                second.get()
            }
        }
    });
    let builds = Rc::new(Cell::new(0));
    let mut runtime = Application::new({
        let memo = memo.clone();
        let builds = builds.clone();
        move |_| {
            builds.set(builds.get() + 1);
            Widget::from(incular_widgets::Text::new(memo.get().to_string()))
        }
    })
    .unwrap()
    .into_runtime();
    let constraints = Constraints::tight(Size::new(100., 30.));
    runtime.run_frame(constraints).unwrap();

    first.set(3);
    runtime.run_frame(constraints).unwrap();
    assert_eq!(builds.get(), 2);

    branch.set(false);
    runtime.run_frame(constraints).unwrap();
    assert_eq!(builds.get(), 3);

    first.set(4);
    runtime.run_frame(constraints).unwrap();
    assert_eq!(builds.get(), 3);

    second.set(5);
    runtime.run_frame(constraints).unwrap();
    assert_eq!(builds.get(), 4);
}

#[test]
fn mounted_effect_runs_once_then_retracks_signal_dependencies() {
    let source = Signal::new(1_u32);
    let trigger = Signal::new(0_u32);
    let runs = Rc::new(Cell::new(0));
    let observed = Rc::new(Cell::new(0));
    let effect = Effect::new({
        let source = source.clone();
        let observed = observed.clone();
        let runs = runs.clone();
        move || {
            runs.set(runs.get() + 1);
            observed.set(source.get());
        }
    });
    let mut runtime = Application::new({
        let effect = effect.clone();
        let trigger = trigger.clone();
        move |_| {
            assert!(effect.mount());
            let _ = trigger.get();
            Widget::from(incular_widgets::Text::new("effect"))
        }
    })
    .unwrap()
    .into_runtime();
    let constraints = Constraints::tight(Size::new(100., 30.));

    runtime.run_frame(constraints).unwrap();
    assert_eq!(runs.get(), 1);
    assert_eq!(observed.get(), 1);

    trigger.set(1);
    runtime.run_frame(constraints).unwrap();
    assert_eq!(runs.get(), 1);

    source.set(2);
    runtime.run_frame(constraints).unwrap();
    assert_eq!(runs.get(), 2);
    assert_eq!(observed.get(), 2);

    runtime.run_frame(constraints).unwrap();
    assert_eq!(runs.get(), 2);
}

#[test]
fn action_is_lazy_tracks_state_and_ignores_stale_completions() {
    let action =
        incular_runtime::Action::<u32, u32, &'static str>::new(
            |input| async move { Ok(input + 1) },
        );
    let mut runtime = Application::new({
        let action = action.clone();
        move |_| Widget::from(incular_widgets::Text::new(format!("{:?}", action.state())))
    })
    .unwrap()
    .into_runtime();
    let wake = Arc::new(TestWake::default());
    runtime.set_wake_handler(wake.clone());
    let constraints = Constraints::tight(Size::new(100., 30.));
    runtime.run_frame(constraints).unwrap();
    assert!(matches!(action.state(), ActionState::Idle));

    action.dispatch(4).unwrap();
    assert!(matches!(action.state(), ActionState::Loading));
    wait_for_wake(&wake);
    runtime.process_runtime_work();
    runtime.run_frame(constraints).unwrap();
    assert!(matches!(action.state(), ActionState::Ready(5)));
}

#[test]
fn application_root_builds_once_and_subscribes_during_initial_build() {
    let builds = Rc::new(Cell::new(0));
    let observed_builds = builds.clone();
    let signal = Signal::new(false);
    let observed_signal = signal.clone();
    let application = Application::new(move |_| {
        observed_builds.set(observed_builds.get() + 1);
        let _ = observed_signal.get();
        Widget::box_(Size::new(1., 1.), Color::WHITE)
    })
    .unwrap();

    assert_eq!(builds.get(), 1);
    assert_eq!(signal.dependent_count(), 1);

    let mut runtime = application.into_runtime();
    runtime
        .run_frame(Constraints::tight(Size::new(20., 20.)))
        .unwrap();
    assert_eq!(builds.get(), 1);
    assert!(signal.set(true));
    runtime
        .run_frame(Constraints::tight(Size::new(20., 20.)))
        .unwrap();
    assert_eq!(builds.get(), 2);
}

#[test]
fn reactive_mutation_during_build_fails_without_poisoning_scope() {
    let signal = Signal::new(0_u32);
    let observed_signal = signal.clone();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = Application::new(move |_| {
            observed_signal.set(1);
            Widget::box_(Size::new(1., 1.), Color::WHITE)
        });
    }));

    assert!(result.is_err());
    assert_eq!(signal.get(), 0);
    assert!(signal.update_if_changed(|value| *value = 1));
    assert_eq!(signal.get(), 1);
}

#[test]
fn typed_environment_rebuilds_only_when_a_read_field_changes() {
    let builds = Rc::new(Cell::new(0));
    let observed = builds.clone();
    let application = Application::new(move |cx| {
        observed.set(observed.get() + 1);
        Widget::box_(Size::new(cx.text_scale(), 1.), Color::WHITE)
    })
    .unwrap();
    let mut runtime = application.into_runtime();
    let baseline = builds.get();
    let mut environment = runtime.environment();
    environment.brightness = incular_config::Brightness::Dark;
    assert!(runtime.set_environment(environment.clone()));
    assert_eq!(builds.get(), baseline);
    environment.text_scale = 1.5;
    assert!(runtime.set_environment(environment));
    assert_eq!(builds.get(), baseline + 1);
}

#[test]
fn watched_focus_scope_invalidates_a_mounted_builder() {
    let scope = FocusScopeNode::new();
    let node = incular_widgets::FocusNode::new();
    scope.register(&node);
    let builds = Rc::new(Cell::new(0));
    let observed_builds = builds.clone();
    let observed_scope = scope.clone();
    let application = Application::new(move |cx| {
        cx.watch_focus_scope(&observed_scope);
        observed_builds.set(observed_builds.get() + 1);
        Text::new(if observed_scope.focused().is_some() {
            "focused"
        } else {
            "unfocused"
        })
        .into()
    })
    .unwrap();
    let mut runtime = application.into_runtime();
    let constraints = Constraints::tight(Size::new(220., 50.));
    runtime.run_frame(constraints).unwrap();
    let baseline = builds.get();
    assert!(!runtime.frame_requested());

    assert!(scope.request_focus(&node));
    assert!(runtime.frame_requested());
    runtime.run_frame(constraints).unwrap();
    assert!(builds.get() > baseline);
}

#[test]
fn locale_resolution_rebuilds_only_locale_consumers() {
    let builds = Rc::new(Cell::new(0));
    let observed = builds.clone();
    let supported = vec![
        "en".parse().expect("valid ICU locale"),
        "fr".parse().expect("valid ICU locale"),
    ];
    let application = Application::new(move |cx| {
        observed.set(observed.get() + 1);
        let locale = cx
            .resolve_locale(&supported)
            .map(|locale| locale.to_string())
            .unwrap_or_else(|| "none".to_owned());
        Text::new(locale).into()
    })
    .unwrap();
    let mut runtime = application.into_runtime();
    let baseline = builds.get();

    let mut environment = runtime.environment();
    environment.brightness = incular_config::Brightness::Dark;
    assert!(runtime.set_environment(environment.clone()));
    assert_eq!(builds.get(), baseline);

    environment.locales = vec!["fr-CA".parse().expect("valid ICU locale")];
    assert!(runtime.set_environment(environment));
    assert_eq!(builds.get(), baseline + 1);
}

#[test]
fn unmount_removes_signal_subscription() {
    let mut runtime = Runtime::new(Widget::from(incular_widgets::Row::new(vec![Widget::box_(
        Size::new(2., 2.),
        Color::WHITE,
    )])))
    .unwrap();
    let root = runtime.tree().root().unwrap();
    let child = runtime.tree().children(root).unwrap()[0];
    let signal = Signal::new(1_u32);
    let state = signal.clone();
    runtime
        .register_builder(child, move || {
            Widget::box_(Size::new(state.get() as f32, 2.), Color::WHITE)
        })
        .unwrap();
    runtime
        .schedule_update(
            root,
            Widget::from(incular_widgets::Row::new(Vec::<Widget>::new())),
        )
        .unwrap();
    runtime
        .run_frame(Constraints::tight(Size::new(20., 20.)))
        .unwrap();
    assert_eq!(signal.dependent_count(), 0);
}

#[test]
fn unbound_memo_never_keeps_a_value_without_an_invalidation_owner() {
    let source = Signal::new(1_u32);
    let memo = Memo::new({
        let source = source.clone();
        move || source.get() * 2
    });
    assert_eq!(memo.get_untracked(), 2);
    source.set(3);
    assert_eq!(memo.get_untracked(), 6);
}

#[test]
fn unmount_disposes_effect_even_when_the_application_keeps_its_handle() {
    let source = Signal::new(1_u32);
    let effect = Effect::new({
        let source = source.clone();
        move || {
            source.get();
        }
    });
    let mut runtime = Runtime::new(Widget::from(incular_widgets::Row::new(vec![Widget::box_(
        Size::new(1., 1.),
        Color::WHITE,
    )])))
    .unwrap();
    let root = runtime.tree().root().unwrap();
    let child = runtime.tree().children(root).unwrap()[0];
    runtime
        .register_builder(child, {
            let effect = effect.clone();
            move || {
                assert!(effect.mount());
                Widget::box_(Size::new(1., 1.), Color::WHITE)
            }
        })
        .unwrap();
    let constraints = Constraints::tight(Size::new(20., 20.));
    runtime.run_frame(constraints).unwrap();
    assert_eq!(source.dependent_count(), 1);
    runtime
        .schedule_update(
            root,
            Widget::from(incular_widgets::Row::new(Vec::<Widget>::new())),
        )
        .unwrap();
    runtime.run_frame(constraints).unwrap();
    assert!(!effect.is_mounted());
    assert_eq!(source.dependent_count(), 0);
}

#[test]
fn shared_memo_branch_switch_unsubscribes_the_old_source_in_every_root() {
    let branch = Signal::new(true);
    let first = Signal::new(1_u32);
    let second = Signal::new(2_u32);
    let memo = Memo::new({
        let branch = branch.clone();
        let first = first.clone();
        let second = second.clone();
        move || {
            if branch.get() {
                first.get()
            } else {
                second.get()
            }
        }
    });
    let mut first_runtime = Application::new({
        let memo = memo.clone();
        move |_| Widget::from(Text::new(memo.get().to_string()))
    })
    .unwrap()
    .into_runtime();
    let mut second_runtime = Application::new({
        let memo = memo.clone();
        move |_| Widget::from(Text::new(memo.get().to_string()))
    })
    .unwrap()
    .into_runtime();
    let constraints = Constraints::tight(Size::new(20., 20.));
    first_runtime.run_frame(constraints).unwrap();
    second_runtime.run_frame(constraints).unwrap();
    assert_eq!(first.dependent_count(), 2);
    branch.set(false);
    first_runtime.run_frame(constraints).unwrap();
    assert_eq!(first.dependent_count(), 0);
    assert_eq!(second.dependent_count(), 2);
    second_runtime.run_frame(constraints).unwrap();
    drop(first_runtime);
    assert_eq!(second.dependent_count(), 1);
    drop(second_runtime);
    assert_eq!(second.dependent_count(), 0);
}

#[test]
fn recursive_memo_fails_explicitly_and_restores_pure_scope() {
    let slot = Rc::new(RefCell::new(None::<Memo<u32>>));
    let memo = Memo::new({
        let slot = slot.clone();
        move || slot.borrow().as_ref().unwrap().get()
    });
    *slot.borrow_mut() = Some(memo.clone());
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| memo.get()));
    slot.borrow_mut().take();
    assert!(result.is_err());
    assert!(Signal::new(0_u32).set(1));
}

#[test]
fn focus_and_scroll_reads_invalidate_runtime_builders() {
    let focus = incular_widgets::FocusNode::new();
    let scroll = incular_widgets::ScrollController::new();
    scroll.update_extents(100., 10.);
    let observed = Rc::new(Cell::new((false, 0.0_f32)));
    let mut runtime = Application::new({
        let focus = focus.clone();
        let scroll = scroll.clone();
        let observed = observed.clone();
        move |_| {
            observed.set((focus.has_focus(), scroll.offset()));
            Widget::box_(Size::new(1., 1.), Color::WHITE)
        }
    })
    .unwrap()
    .into_runtime();
    let constraints = Constraints::tight(Size::new(20., 20.));
    runtime.run_frame(constraints).unwrap();
    focus.request_focus();
    scroll.jump_to(12.0);
    runtime.run_frame(constraints).unwrap();
    assert_eq!(observed.get(), (true, 12.0));
}

#[test]
fn an_effect_can_converge_by_writing_its_controller_dependency() {
    let controller = TextEditingController::with_text("a");
    let runs = Rc::new(Cell::new(0));
    let effect = Effect::new({
        let controller = controller.clone();
        let runs = runs.clone();
        move || {
            runs.set(runs.get() + 1);
            if controller.text().len() < 2 {
                controller.set_text("ab");
            }
        }
    });
    let mut runtime = Application::new(move |_| {
        effect.mount();
        Widget::box_(Size::new(1., 1.), Color::WHITE)
    })
    .unwrap()
    .into_runtime();
    runtime
        .run_frame(Constraints::tight(Size::new(20., 20.)))
        .unwrap();
    assert_eq!(runs.get(), 2);
}
