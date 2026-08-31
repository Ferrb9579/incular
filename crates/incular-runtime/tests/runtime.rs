#![allow(unused_imports)]

use accesskit::{Action as AccessKitAction, ActionRequest, TreeId};
use incular_accessibility::AccessKitProjection;
use incular_config::{Constraints, EdgeInsets, RuntimeEnvironment};
use incular_core::{
    Code, Color, ImeEvent, InputEvent, KeyboardEvent, KeyboardKey, Modifiers, Offset, PointerPhase,
    Rect, RestorationKey, RestorationScope, Size,
};
use incular_platform::{
    Clipboard, MemoryClipboard, PlatformEvent, TextInputAction, TextInputClientId,
    TextInputCommand, TextInputConfiguration, TextInputState, TextInputType, WindowCommand,
    WindowEvent, WindowEventKind, WindowId, WindowLifecycle, WindowMetrics, WindowOperation,
    WindowOptions,
};
use incular_rendering::{DisplayList, PaintCommand};
use incular_runtime::*;
use incular_semantics::{Role as SemanticRole, SemanticAction};
use incular_text::TextStyle;
use incular_widgets::internal::{
    ActionId, Diagnostics, ElementId, GeneratedChildIdentity, GestureCallbacks, Key, PointerEvent,
    TextEditingController, TextRange, TextSelection, TreeError, WidgetTree,
};
use incular_widgets::{
    Align, BorderRadius, BoxDecoration, Container, CustomScrollView, DecoratedBox,
    DefaultTextStyle, FocusScopeNode, FocusScopeSubscription, GestureDetector, KeyboardListener,
    LayoutBuilder, SliverFixedExtentList, Text, TextInputActionHint, TextInputTypeHint, Widget,
    internal::ActionSurface,
};
use std::time::{Duration, Instant};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

#[path = "runtime/devtools_performance.rs"]
mod devtools_performance;
#[path = "runtime/layout_semantics.rs"]
mod layout_semantics;
#[path = "runtime/reactive_scheduling.rs"]
mod reactive_scheduling;
#[path = "runtime/restoration_tasks.rs"]
mod restoration_tasks;
#[path = "runtime/retained_interactions.rs"]
mod retained_interactions;

fn fixed_sliver_list<W>(
    item_count: usize,
    item_extent: f32,
    controller: incular_widgets::ScrollController,
    builder: impl Fn(usize) -> W + 'static,
) -> Widget
where
    W: Into<Widget> + 'static,
{
    CustomScrollView::new(vec![Box::new(SliverFixedExtentList::new(
        item_count,
        item_extent,
        builder,
    )) as Box<dyn incular_widgets::Sliver>])
    .controller(controller)
    .into()
}

#[derive(Default)]
struct TestWake(AtomicU64);
impl RuntimeWake for TestWake {
    fn wake(&self) {
        self.0.fetch_add(1, Ordering::AcqRel);
    }
}

fn key_down(code: Code) -> KeyboardEvent {
    KeyboardEvent::key_down(
        KeyboardKey::Named(incular_core::NamedKey::Unidentified),
        code,
    )
}

#[test]
fn runtime_frame_propagates_layout_builder_configuration_error() {
    let invalid = || {
        Widget::column(vec![
            Widget::box_(Size::new(10., 10.), Color::WHITE).with_key(11_u64),
            Widget::box_(Size::new(10., 10.), Color::WHITE).with_key(11_u64),
        ])
    };
    let mut runtime = Runtime::new(LayoutBuilder::new(move |_| invalid()).into())
        .expect("layout builder root mounts before generated output exists");

    let error = runtime
        .run_frame(Constraints::tight(Size::new(100., 100.)))
        .expect_err("runtime must surface generated layout error");

    assert!(matches!(
        error,
        TreeError::InvalidGeneratedChild {
            child: GeneratedChildIdentity::LayoutBuilder,
            ..
        }
    ));
}

fn shortcut_key_down(code: Code) -> KeyboardEvent {
    let mut event = key_down(code);
    event.modifiers = Modifiers::CONTROL;
    event
}

fn wait_for_wake(wake: &TestWake) {
    for _ in 0..100 {
        if wake.0.load(Ordering::Acquire) > 0 {
            return;
        }
        std::thread::park_timeout(Duration::from_millis(2));
    }
    panic!("Tokio completion did not wake the UI bridge");
}

fn semantic_node(runtime: &Runtime, role: SemanticRole) -> incular_semantics::SemanticNodeId {
    runtime
        .tree()
        .semantics()
        .iter()
        .find_map(|(id, node)| (node.role == role).then_some(id))
        .expect("semantic node")
}

#[test]
fn rounded_text_inserted_by_a_layout_builder_is_frame_stable() {
    let open = Rc::new(Cell::new(false));
    let revision = Rc::new(Cell::new(0));
    let open_for_builder = open.clone();
    let root = Widget::stateful_layout_builder(revision.clone(), move |_| {
        if open_for_builder.get() {
            Container::new()
                .height(32.0)
                .padding(EdgeInsets::symmetric(12.0, 0.0))
                .decoration(BoxDecoration::new().border_radius(BorderRadius::circular(4.0)))
                .child(Align::new(
                    incular_config::Alignment::CENTER,
                    Text::new("New document"),
                ))
                .into()
        } else {
            Text::new("Menu").into()
        }
    });
    let root = DefaultTextStyle::new(TextStyle::default().font_size(14.0), root).into();
    let mut runtime = Runtime::new(root).expect("mount rounded text test");
    let constraints = Constraints::tight(Size::new(320.0, 120.0));
    runtime.run_frame(constraints).expect("initial frame");
    open.set(true);
    revision.set(1);
    runtime.run_frame(constraints).expect("opened frame");
    runtime.run_frame(constraints).expect("stable opened frame");
}

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

#[test]
fn semantic_actions_share_logical_button_and_editing_state() {
    use incular_widgets::{
        EditableText,
        internal::{ActionSurface, TextEditingController},
    };
    let hits = Rc::new(Cell::new(0));
    let controller = TextEditingController::with_text("Ada");
    let mut runtime = Runtime::new(Widget::column(vec![
        ActionSurface::new("Increment")
            .on_press({
                let hits = hits.clone();
                move || hits.set(hits.get() + 1)
            })
            .into(),
        EditableText::new(controller.clone()).into(),
    ]))
    .unwrap();
    let root = runtime.tree().root().unwrap();
    runtime
        .schedule_update(
            root,
            Widget::column(vec![
                ActionSurface::new("Increment")
                    .on_press({
                        let hits = hits.clone();
                        move || hits.set(hits.get() + 1)
                    })
                    .into(),
                EditableText::new(controller.clone()).into(),
            ]),
        )
        .unwrap();
    runtime
        .run_frame(Constraints::tight(Size::new(300., 200.)))
        .unwrap();
    let button = semantic_node(&runtime, SemanticRole::Button);
    let field = semantic_node(&runtime, SemanticRole::TextField);
    assert!(runtime.dispatch_semantic_action(button, SemanticAction::Activate));
    assert_eq!(hits.get(), 1);
    assert!(runtime.dispatch_semantic_action(field, SemanticAction::Focus));
    assert_eq!(
        runtime.focused_element(),
        runtime.tree().element_for_semantic_node(field)
    );
    assert!(runtime.dispatch_semantic_action(field, SemanticAction::SetText("hello".into())));
    assert!(
        runtime
            .dispatch_semantic_action(field, SemanticAction::SetSelection { base: 1, extent: 4 })
    );
    assert_eq!(controller.text(), "hello");
    assert_eq!(
        controller.value().selection,
        TextSelection { base: 1, extent: 4 }
    );
}

#[test]
fn semantic_callbacks_make_custom_controls_actionable() {
    let activations = Rc::new(Cell::new(0));
    let observed = activations.clone();
    let mut runtime = Runtime::new(
        incular_widgets::Semantics::new(Text::new("custom"))
            .on_tap(move || observed.set(observed.get() + 1))
            .into(),
    )
    .unwrap();
    runtime
        .run_frame(Constraints::tight(Size::new(200., 50.)))
        .unwrap();
    let node = semantic_node(&runtime, SemanticRole::Button);
    assert!(runtime.dispatch_semantic_action(node, SemanticAction::Activate));
    assert_eq!(activations.get(), 1);
}

#[test]
fn focused_editors_publish_native_text_input_state_and_actions() {
    let submitted = Rc::new(Cell::new(0));
    let observed = submitted.clone();
    let controller = TextEditingController::new();
    let mut runtime = Runtime::new(
        incular_widgets::EditableText::new(controller.clone())
            .on_submit(move |_| observed.set(observed.get() + 1))
            .into(),
    )
    .unwrap();
    let constraints = Constraints::tight(Size::new(220., 50.));
    runtime.run_frame(constraints).unwrap();
    assert!(runtime.take_text_input_commands().is_empty());

    let _ = runtime.handle_input(InputEvent::Pointer {
        phase: PointerPhase::Down,
        position: Offset::new(5., 5.),
    });
    let commands = runtime.take_text_input_commands();
    assert!(matches!(
        commands.as_slice(),
        [incular_platform::TextInputCommand::SetClient {
            configuration,
            state,
            ..
        }] if configuration.input_type == incular_platform::TextInputType::Text
            && state.text.is_empty()
    ));

    let _ = runtime.handle_input(InputEvent::Text("hello".into()));
    runtime.run_frame(constraints).unwrap();
    let commands = runtime.take_text_input_commands();
    assert!(commands.iter().any(|command| matches!(
        command,
        incular_platform::TextInputCommand::Update { state, .. } if state.text == "hello"
    )));

    assert!(
        runtime
            .handle_platform_event(incular_platform::PlatformEvent::TextInputAction(
                incular_platform::TextInputAction::Done,
            ),)
            .is_none()
    );
    assert_eq!(submitted.get(), 1);
}

#[test]
fn native_text_input_hints_survive_editor_conversion() {
    let controller = TextEditingController::new();
    let mut runtime = Runtime::new(
        incular_widgets::EditableText::new(controller)
            .input_type(TextInputTypeHint::Email)
            .input_action(TextInputActionHint::Search)
            .into(),
    )
    .unwrap();
    let constraints = Constraints::tight(Size::new(220., 50.));
    runtime.run_frame(constraints).unwrap();
    let _ = runtime.handle_input(InputEvent::Pointer {
        phase: PointerPhase::Down,
        position: Offset::new(5., 5.),
    });
    let commands = runtime.take_text_input_commands();
    assert!(matches!(
        commands.as_slice(),
        [incular_platform::TextInputCommand::SetClient { configuration, .. }]
            if configuration.input_type == incular_platform::TextInputType::Email
                && configuration.action == incular_platform::TextInputAction::Search
    ));
}

#[test]
fn undo_history_widget_configures_runtime_capacity() {
    let controller = TextEditingController::new();
    let mut runtime = Runtime::new(
        incular_widgets::UndoHistory::new(incular_widgets::EditableText::new(controller.clone()))
            .max_entries(1)
            .into(),
    )
    .unwrap();
    let constraints = Constraints::tight(Size::new(220., 50.));
    runtime.run_frame(constraints).unwrap();
    let _ = runtime.handle_input(InputEvent::Pointer {
        phase: PointerPhase::Down,
        position: Offset::new(5., 5.),
    });
    let _ = runtime.handle_input(InputEvent::Text("one".into()));
    let _ = runtime.handle_input(InputEvent::Text("two".into()));
    let _ = runtime.handle_input(InputEvent::Key(shortcut_key_down(Code::KeyZ)));
    assert_eq!(controller.text(), "one");
}

#[test]
fn sliver_list_semantics_are_bounded_and_follow_materialization() {
    let controller = incular_widgets::ScrollController::new();
    let mut runtime = Runtime::new(fixed_sliver_list(
        1_000_000,
        40.,
        controller.clone(),
        |index| ActionSurface::new(format!("Item {index}")),
    ))
    .unwrap();
    runtime
        .run_frame(Constraints::tight(Size::new(200., 600.)))
        .unwrap();
    let initial = runtime.tree().semantics().len();
    assert!(initial < 100, "{initial}");
    let _viewport = semantic_node(&runtime, SemanticRole::ScrollView);
    assert_eq!(
        runtime
            .tree()
            .sliver_viewport_diagnostics()
            .unwrap()
            .logical_item_count,
        1_000_000
    );
    controller.jump_to(900_000. * 40.);
    runtime
        .run_frame(Constraints::tight(Size::new(200., 600.)))
        .unwrap();
    assert!(runtime.tree().semantics().len() < 100);
    assert!(runtime.tree().semantics().iter().any(|(_, node)| {
        node.label
            .as_deref()
            .is_some_and(|label| label.contains("900000"))
    }));
}

#[test]
fn semantic_scroll_uses_existing_controller() {
    let controller = incular_widgets::ScrollController::new();
    let mut runtime = Runtime::new(incular_widgets::internal::ScrollView::vertical(
        controller.clone(),
        Widget::fixed_box(Size::new(100., 2000.), Color::WHITE),
    ))
    .unwrap();
    runtime
        .run_frame(Constraints::tight(Size::new(100., 200.)))
        .unwrap();
    let scroll = semantic_node(&runtime, SemanticRole::ScrollView);
    assert!(runtime.dispatch_semantic_action(scroll, SemanticAction::ScrollForward));
    assert!(controller.offset() > 0.);
    assert!(runtime.dispatch_semantic_action(scroll, SemanticAction::ScrollBackward));
    assert_eq!(controller.offset(), 0.);
}

fn picture_origins(list: &DisplayList) -> (Offset, Offset) {
    let mut transforms = vec![Offset::ZERO];
    let mut rect = None;
    let mut glyph = None;
    for command in list.commands() {
        match command {
            PaintCommand::PushTransform { transform } => {
                transforms.push(*transforms.last().unwrap() + transform.translation_offset());
            }
            PaintCommand::PopTransform => {
                transforms.pop();
            }
            PaintCommand::Rect { rect: bounds, .. } => {
                rect = Some(bounds.origin + *transforms.last().unwrap());
            }
            PaintCommand::GlyphRun { run, .. } => {
                glyph = Some(run.origin + *transforms.last().unwrap());
            }
            PaintCommand::Image { .. }
            | PaintCommand::RRect { .. }
            | PaintCommand::Border { .. }
            | PaintCommand::FillPath { .. }
            | PaintCommand::StrokePath { .. }
            | PaintCommand::PushClip { .. }
            | PaintCommand::PushClipRRect { .. }
            | PaintCommand::PushClipOval { .. }
            | PaintCommand::PushClipPath { .. }
            | PaintCommand::PopClip
            | PaintCommand::PushOpacity { .. }
            | PaintCommand::PopOpacity
            | PaintCommand::PushBlur { .. }
            | PaintCommand::PushDropShadow { .. }
            | PaintCommand::PushColorFilter { .. }
            | PaintCommand::PushBlend { .. }
            | PaintCommand::PushShaderMask { .. }
            | PaintCommand::PushBackdropFilter { .. }
            | PaintCommand::PopEffect => {}
        }
    }
    (rect.unwrap(), glyph.unwrap())
}
#[test]
fn signals_schedule_only_subscribed_element_once() {
    let mut runtime = Runtime::new(Widget::row(vec![
        Widget::box_(Size::new(1., 1.), Color::WHITE),
        Widget::box_(Size::new(2., 2.), Color::WHITE),
    ]))
    .unwrap();
    let root = runtime.tree().root().unwrap();
    let children = runtime.tree().children(root).unwrap().to_vec();
    let signal = Signal::with_runtime(2_u32, &runtime);
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
            Widget::text(memo.with(ToString::to_string))
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
            Widget::text(memo.get().to_string())
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
            Widget::text(memo.get().to_string())
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
                Widget::text(memo.get().to_string())
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
            Widget::text(memo.get().to_string())
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
            Widget::text("effect")
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
        move |_| Widget::text(format!("{:?}", action.state()))
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

#[cfg(feature = "devtools")]
#[test]
fn named_signal_write_reaches_dependent_rebuild_cause() {
    let mut runtime = Runtime::new(Widget::row(vec![Widget::box_(
        Size::new(1., 1.),
        Color::WHITE,
    )]))
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
    let mut runtime = Runtime::new(Widget::row(vec![Widget::box_(
        Size::new(1., 1.),
        Color::WHITE,
    )]))
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
    let mut runtime = Runtime::new(Widget::row(vec![
        Widget::box_(Size::new(1., 1.), Color::WHITE),
        Widget::box_(Size::new(1., 1.), Color::WHITE),
    ]))
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
    let mut runtime = Runtime::new(Widget::row(vec![Widget::box_(
        Size::new(1., 1.),
        Color::WHITE,
    )]))
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
        .update(root, Widget::row(Vec::new()))
        .unwrap();
    assert!(!runtime.tree().element_exists(child));
    scope.cancel();
    runtime.process_runtime_work();
    assert!(!hit.load(Ordering::Acquire));
    assert!(runtime.runtime_diagnostics().tasks_cancelled >= 1);
}
#[test]
fn application_task_survives_an_unrelated_component_unmount() {
    let mut runtime = Runtime::new(Widget::row(vec![Widget::box_(
        Size::new(1., 1.),
        Color::WHITE,
    )]))
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
        .update(root, Widget::row(Vec::new()))
        .unwrap();
    runtime.process_runtime_work();
    assert!(hit.load(Ordering::Acquire));
}
#[test]
fn blocking_result_after_owner_unmount_is_discarded() {
    let mut runtime = Runtime::new(Widget::row(vec![Widget::box_(
        Size::new(1., 1.),
        Color::WHITE,
    )]))
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
        .update(root, Widget::row(Vec::new()))
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
fn hit_test_resolves_button_action() {
    let mut runtime = Runtime::new(incular_widgets::internal::action(
        Size::new(10., 10.),
        Color::WHITE,
        ActionId(1),
    ))
    .unwrap();
    runtime
        .run_frame(Constraints::tight(Size::new(20., 20.)))
        .unwrap();
    let _ = runtime.handle_input(InputEvent::Pointer {
        phase: PointerPhase::Down,
        position: Offset::new(5., 5.),
    });
    assert_eq!(
        runtime
            .handle_input(InputEvent::Pointer {
                phase: PointerPhase::Up,
                position: Offset::new(5., 5.)
            })
            .unwrap()
            .action,
        Some(ActionId(1))
    );
}
#[test]
fn identified_primary_contact_can_activate_a_button() {
    let mut runtime = Runtime::new(incular_widgets::internal::action(
        Size::new(10., 10.),
        Color::WHITE,
        ActionId(2),
    ))
    .unwrap();
    runtime
        .run_frame(Constraints::tight(Size::new(20., 20.)))
        .unwrap();
    let _ = runtime.handle_input(InputEvent::PointerWithId {
        pointer: 72,
        phase: PointerPhase::Down,
        position: Offset::new(5., 5.),
    });
    assert_eq!(
        runtime
            .handle_input(InputEvent::PointerWithId {
                pointer: 72,
                phase: PointerPhase::Up,
                position: Offset::new(5., 5.),
            })
            .unwrap()
            .action,
        Some(ActionId(2))
    );
}
#[test]
fn button_hover_callbacks_fire_once_on_enter_and_exit() {
    let enters = Rc::new(Cell::new(0_u32));
    let exits = Rc::new(Cell::new(0_u32));
    let mut runtime = Runtime::new(
        ActionSurface::new("Hover")
            .on_hover({
                let enters = enters.clone();
                move || enters.set(enters.get() + 1)
            })
            .on_exit({
                let exits = exits.clone();
                move || exits.set(exits.get() + 1)
            })
            .into(),
    )
    .unwrap();
    runtime
        .run_frame(Constraints::tight(Size::new(100., 40.)))
        .unwrap();
    for position in [
        Offset::new(10., 10.),
        Offset::new(20., 10.),
        Offset::new(150., 10.),
        Offset::new(160., 10.),
    ] {
        let _ = runtime.handle_input(InputEvent::Pointer {
            phase: PointerPhase::Move,
            position,
        });
    }
    assert_eq!(enters.get(), 1);
    assert_eq!(exits.get(), 1);
}
#[test]
fn wheel_updates_only_retained_scroll_transform() {
    let controller = incular_widgets::ScrollController::new();
    let child = Widget::column(
        (0..8)
            .map(|_| Widget::box_(Size::new(80., 40.), Color::WHITE))
            .collect::<Vec<_>>(),
    );
    let mut runtime = Runtime::new(Widget::scroll_view(controller.clone(), child)).unwrap();
    let constraints = Constraints::tight(Size::new(100., 100.));
    let (_, initial) = runtime.run_frame(constraints).unwrap();
    assert!(
        initial.laid_out_render_objects > 0
            && initial.repainted_render_objects > 0
            && initial.composited > 0
    );
    let _ = runtime.handle_input(InputEvent::Pointer {
        phase: PointerPhase::Move,
        position: Offset::new(10., 10.),
    });
    let _ = runtime.handle_input(InputEvent::Scroll {
        delta: Offset::new(0., 60.),
    });
    let (_, frame) = runtime.run_frame(constraints).unwrap();
    assert_eq!(frame.rebuilt_elements, 0);
    assert_eq!(frame.laid_out_render_objects, 0);
    assert!(frame.repainted_render_objects <= 1); // overlay scrollbar only
    assert!(frame.composited > 0);
    assert_eq!(controller.offset(), 60.);
}
#[test]
fn translated_button_hit_tests_at_its_visible_position_without_repaint() {
    let controller = incular_widgets::internal::TranslationController::new();
    controller.set_offset(Offset::new(0., 30.));
    let mut runtime = Runtime::new(Widget::translate(
        controller.clone(),
        incular_widgets::internal::action(Size::new(20., 20.), Color::WHITE, ActionId(9)),
    ))
    .unwrap();
    let constraints = Constraints::tight(Size::new(100., 100.));
    let _ = runtime.run_frame(constraints).unwrap();
    let (_, frame) = runtime.run_frame(constraints).unwrap();
    assert_eq!(frame.repainted_render_objects, 0);
    assert!(
        runtime
            .handle_input(InputEvent::Pointer {
                phase: PointerPhase::Down,
                position: Offset::new(5., 35.)
            })
            .is_some()
    );
    assert!(
        runtime
            .handle_input(InputEvent::Pointer {
                phase: PointerPhase::Down,
                position: Offset::new(5., 5.)
            })
            .is_none()
    );
}
#[test]
fn scrolled_button_hits_at_visible_not_old_location() {
    let controller = incular_widgets::ScrollController::new();
    let content = Widget::column(vec![
        Widget::box_(Size::new(80., 160.), Color::WHITE),
        incular_widgets::internal::action(Size::new(20., 20.), Color::WHITE, ActionId(12)),
    ]);
    let mut runtime = Runtime::new(Widget::scroll_view(controller.clone(), content)).unwrap();
    let constraints = Constraints::tight(Size::new(100., 100.));
    let _ = runtime.run_frame(constraints).unwrap();
    assert!(controller.jump_to(80.));
    let (_, frame) = runtime.run_frame(constraints).unwrap();
    assert!(frame.repainted_render_objects <= 1); // overlay scrollbar only
    assert_eq!(
        runtime
            .handle_input(InputEvent::Pointer {
                phase: PointerPhase::Down,
                position: Offset::new(5., 85.)
            })
            .unwrap()
            .action,
        Some(ActionId(12))
    );
    assert!(
        runtime
            .handle_input(InputEvent::Pointer {
                phase: PointerPhase::Down,
                position: Offset::new(5., 165.)
            })
            .is_none()
    );
}
#[test]
fn animation_ticks_request_frames_without_rebuild_or_paint() {
    let controller = incular_widgets::internal::TranslationController::new();
    let mut runtime = Runtime::new(Widget::translate(
        controller.clone(),
        Widget::text("warm text"),
    ))
    .unwrap();
    let constraints = Constraints::tight(Size::new(100., 100.));
    let origin = Instant::now();
    let _ = runtime.run_frame_at(constraints, origin).unwrap();
    let text_before = runtime.tree().text_diagnostics();
    controller.animate_to(Offset::new(50., 0.), Duration::from_millis(1000), origin);
    let (_, frame) = runtime
        .run_frame_at(constraints, origin + Duration::from_millis(500))
        .unwrap();
    assert_eq!(frame.rebuilt_elements, 0);
    assert_eq!(frame.laid_out_render_objects, 0);
    assert_eq!(frame.repainted_render_objects, 0);
    assert!(frame.composited > 0 && frame.active_animations > 0 && frame.requested_another_frame);
    assert_eq!(runtime.tree().text_diagnostics(), text_before);
}
#[test]
fn retained_card_text_and_background_move_together_without_repaint() {
    let controller = incular_widgets::internal::TranslationController::new();
    let mut runtime = Runtime::new(Widget::padding(
        incular_config::EdgeInsets {
            left: 20.,
            top: 10.,
            right: 0.,
            bottom: 0.,
        },
        Widget::translate(
            controller.clone(),
            Widget::column(vec![
                Widget::box_(Size::new(80., 20.), Color::WHITE),
                Widget::text("cached card text"),
            ]),
        ),
    ))
    .unwrap();
    let constraints = Constraints::tight(Size::new(200., 100.));
    let (before, _) = runtime.run_frame(constraints).unwrap();
    let text_before = runtime.tree().text_diagnostics();
    let (before_rect, before_glyph) = picture_origins(&before);
    controller.set_offset(Offset::new(50., 0.));
    let (after, frame) = runtime.run_frame(constraints).unwrap();
    let (after_rect, after_glyph) = picture_origins(&after);
    assert_eq!(frame.rebuilt_elements, 0);
    assert_eq!(frame.laid_out_render_objects, 0);
    assert_eq!(frame.repainted_render_objects, 0);
    assert!(frame.composited > 0);
    assert_eq!(after_rect - before_rect, Offset::new(50., 0.));
    assert_eq!(after_glyph - before_glyph, Offset::new(50., 0.));
    assert_eq!(runtime.tree().text_diagnostics(), text_before);
}
#[test]
fn unmount_removes_signal_subscription() {
    let mut runtime = Runtime::new(Widget::row(vec![Widget::box_(
        Size::new(2., 2.),
        Color::WHITE,
    )]))
    .unwrap();
    let root = runtime.tree().root().unwrap();
    let child = runtime.tree().children(root).unwrap()[0];
    let signal = Signal::with_runtime(1_u32, &runtime);
    let state = signal.clone();
    runtime
        .register_builder(child, move || {
            Widget::box_(Size::new(state.get() as f32, 2.), Color::WHITE)
        })
        .unwrap();
    runtime
        .schedule_update(root, Widget::row(Vec::new()))
        .unwrap();
    runtime
        .run_frame(Constraints::tight(Size::new(20., 20.)))
        .unwrap();
    assert_eq!(signal.dependent_count(), 0);
}
#[test]
fn declarative_button_dispatches_only_a_completed_press() {
    let hits = Rc::new(Cell::new(0));
    let callback_hits = hits.clone();
    let app = Application::new(move |_| {
        ActionSurface::new("Add")
            .on_press({
                let callback_hits = callback_hits.clone();
                move || callback_hits.set(callback_hits.get() + 1)
            })
            .into()
    })
    .unwrap();
    let mut runtime = app.into_runtime();
    runtime
        .run_frame(Constraints::tight(Size::new(120., 60.)))
        .unwrap();
    let _ = runtime.handle_input(InputEvent::Pointer {
        phase: PointerPhase::Down,
        position: Offset::new(10., 10.),
    });
    let _ = runtime.handle_input(InputEvent::Pointer {
        phase: PointerPhase::Up,
        position: Offset::new(121., 50.),
    });
    assert_eq!(hits.get(), 0);
    let _ = runtime.handle_input(InputEvent::Pointer {
        phase: PointerPhase::Down,
        position: Offset::new(10., 10.),
    });
    let _ = runtime.handle_input(InputEvent::Pointer {
        phase: PointerPhase::Up,
        position: Offset::new(10., 10.),
    });
    assert_eq!(hits.get(), 1);
}

#[test]
fn decorated_ancestor_binds_nested_button_callback() {
    let hits = Rc::new(Cell::new(0));
    let observed = hits.clone();
    let root: Widget = DecoratedBox::new(
        ActionSurface::new("Nested").on_press(move || observed.set(observed.get() + 1)),
    )
    .background(Color::BLACK)
    .into();
    let mut runtime = Runtime::new(root).unwrap();
    runtime
        .run_frame(Constraints::tight(Size::new(120., 60.)))
        .unwrap();
    let down = runtime.handle_input(InputEvent::Pointer {
        phase: PointerPhase::Down,
        position: Offset::new(10., 10.),
    });
    let up = runtime.handle_input(InputEvent::Pointer {
        phase: PointerPhase::Up,
        position: Offset::new(10., 10.),
    });
    assert!(down.is_some_and(|target| target.action.is_some()));
    assert!(up.is_some_and(|target| target.action.is_some()));
    assert_eq!(hits.get(), 1);
}

#[test]
fn sliver_list_keeps_small_scrolls_compositor_only_and_direct_jumps_bounded() {
    let controller = incular_widgets::ScrollController::new();
    let calls = Rc::new(Cell::new(0));
    let observed = calls.clone();
    let mut runtime = Runtime::new(fixed_sliver_list(
        1_000_000,
        40.,
        controller.clone(),
        move |index| {
            observed.set(observed.get() + 1);
            Widget::text(format!("Item {index}"))
        },
    ))
    .unwrap();
    let constraints = Constraints::tight(Size::new(120., 100.));
    let (_, initial) = runtime.run_frame(constraints).unwrap();
    assert!(initial.laid_out_render_objects > 0 && initial.repainted_render_objects > 0);
    let warm_calls = calls.get();
    let warm_text = runtime.tree().text_diagnostics();
    assert!(controller.jump_to(3.));
    let (_, small) = runtime.run_frame(constraints).unwrap();
    assert_eq!(calls.get(), warm_calls);
    assert_eq!(small.rebuilt_elements, 0);
    assert_eq!(small.laid_out_render_objects, 0);
    assert!(small.repainted_render_objects <= 1); // overlay scrollbar only
    assert!(small.composited > 0);
    assert_eq!(runtime.tree().text_diagnostics(), warm_text);

    let boundary_before = runtime.diagnostics();
    assert!(controller.jump_to(40.));
    let (_, boundary) = runtime.run_frame(constraints).unwrap();
    assert_eq!(calls.get(), warm_calls + 1);
    let boundary_after = runtime.diagnostics();
    assert_eq!(boundary_after.items_built - boundary_before.items_built, 1);
    assert_eq!(
        boundary_after.items_mounted - boundary_before.items_mounted,
        1
    );
    assert_eq!(
        boundary_after.items_unmounted - boundary_before.items_unmounted,
        0
    );
    assert!(boundary.laid_out_render_objects <= 2);
    assert!(boundary.repainted_render_objects <= 2);

    assert!(controller.jump_to(900_000. * 40.));
    let (_, jumped) = runtime.run_frame(constraints).unwrap();
    let diagnostics = runtime.tree().sliver_viewport_diagnostics().unwrap();
    assert!(diagnostics.materialized_range.contains(&900_000));
    assert!(diagnostics.materialized_item_count < 100);
    assert!(calls.get() < 200);
    assert!(jumped.laid_out_render_objects < 100);
    assert!(diagnostics.picture_layer_count < 250);
}

#[test]
fn sliver_button_rows_hit_their_logical_index_and_drop_stale_handlers() {
    let controller = incular_widgets::ScrollController::new();
    let hit = Rc::new(Cell::new(None));
    let observed = hit.clone();
    let mut runtime = Runtime::new(fixed_sliver_list(
        2_000,
        40.,
        controller.clone(),
        move |index| {
            let hit = observed.clone();
            ActionSurface::new(format!("Item {index}")).on_press(move || hit.set(Some(index)))
        },
    ))
    .unwrap();
    let constraints = Constraints::tight(Size::new(120., 40.));
    runtime.run_frame(constraints).unwrap();
    assert!(controller.jump_to(500. * 40.));
    runtime.run_frame(constraints).unwrap();
    let _ = runtime.handle_input(InputEvent::Pointer {
        phase: PointerPhase::Down,
        position: Offset::new(10., 10.),
    });
    let _ = runtime.handle_input(InputEvent::Pointer {
        phase: PointerPhase::Up,
        position: Offset::new(10., 10.),
    });
    assert_eq!(hit.get(), Some(500));
    assert!(controller.jump_to(1_500. * 40.));
    runtime.run_frame(constraints).unwrap();
    assert_eq!(hit.get(), Some(500));
}

#[test]
fn long_sliver_scroll_keeps_retained_resources_bounded() {
    let controller = incular_widgets::ScrollController::new();
    let mut runtime = Runtime::new(fixed_sliver_list(
        50_000,
        40.,
        controller.clone(),
        move |index| ActionSurface::new(format!("Item {index}")),
    ))
    .unwrap();
    let constraints = Constraints::tight(Size::new(120., 120.));
    runtime.run_frame(constraints).unwrap();
    for index in (97..10_000).step_by(97) {
        assert!(controller.jump_to(index as f32 * 40.));
        runtime.run_frame(constraints).unwrap();
        let view = runtime.tree().sliver_viewport_diagnostics().unwrap();
        assert!(view.materialized_item_count < 30);
        assert!(view.element_count < 100);
        assert!(view.render_object_count < 100);
        assert!(view.picture_layer_count < 200);
    }
}

#[test]
fn mounted_keyboard_listener_dispatches_shortcuts_and_key_up() {
    use incular_widgets::{Actions, Command, FocusNode, KeyboardListener, ShortcutKey, Shortcuts};

    let shortcut_hits = Rc::new(Cell::new(0));
    let observed_shortcut = shortcut_hits.clone();
    let mut actions = Actions::new();
    actions.register(Command::new("save"), move || {
        observed_shortcut.set(observed_shortcut.get() + 1);
    });
    let actions = Rc::new(actions);
    let mut shortcuts = Shortcuts::new();
    shortcuts.bind(
        ShortcutKey::new(Code::KeyS, Modifiers::CONTROL),
        Command::new("save"),
    );
    let shortcuts = Rc::new(shortcuts);

    let key_ups = Rc::new(Cell::new(0));
    let observed_key_up = key_ups.clone();
    let focus_node = FocusNode::new();
    let root: Widget = KeyboardListener::new(Text::new("keyboard target"))
        .focus_node(focus_node.clone())
        .autofocus(true)
        .with_shortcuts(shortcuts, actions)
        .on_key_up(move |_| observed_key_up.set(observed_key_up.get() + 1))
        .into();
    let mut runtime = Runtime::new(root).unwrap();
    runtime
        .run_frame(Constraints::tight(Size::new(220., 50.)))
        .unwrap();

    assert!(focus_node.has_focus());
    assert_eq!(runtime.focused_element(), runtime.tree().root());

    let mut down = shortcut_key_down(Code::KeyS);
    down.repeat = false;
    assert!(runtime.handle_input(InputEvent::Key(down)).is_none());
    assert_eq!(shortcut_hits.get(), 1);

    let mut up = KeyboardEvent::key_up(
        KeyboardKey::Named(incular_core::NamedKey::Unidentified),
        Code::KeyS,
    );
    up.modifiers = Modifiers::CONTROL;
    assert!(runtime.handle_input(InputEvent::Key(up)).is_none());
    assert_eq!(key_ups.get(), 1);
}

#[test]
fn focus_routes_text_shortcuts_and_ime_without_rebuilding_tree() {
    use incular_core::ImeEvent;
    use incular_widgets::{EditableText, internal::TextEditingController};
    let first = TextEditingController::new();
    let second = TextEditingController::new();
    let mut runtime = Runtime::new(Widget::column(vec![
        EditableText::new(first.clone())
            .size(Size::new(120., 40.))
            .into(),
        EditableText::new(second.clone())
            .size(Size::new(120., 40.))
            .into(),
    ]))
    .unwrap();
    let constraints = Constraints::tight(Size::new(160., 100.));
    runtime.run_frame(constraints).unwrap();
    let before = runtime.tree().diagnostics();
    let _ = runtime.handle_input(InputEvent::Pointer {
        phase: PointerPhase::Down,
        position: Offset::new(5., 5.),
    });
    let _ = runtime.handle_input(InputEvent::Text("café".into()));
    let _ = runtime.handle_input(InputEvent::Ime(ImeEvent::Preedit {
        text: "世界".into(),
        selection: None,
    }));
    assert_eq!(first.text(), "café");
    let _ = runtime.handle_input(InputEvent::Ime(ImeEvent::Commit("世界".into())));
    assert_eq!(first.text(), "café世界");
    let _ = runtime.handle_input(InputEvent::Key(key_down(Code::Tab)));
    assert_ne!(runtime.focused_element(), runtime.tree().root());
    let _ = runtime.handle_input(InputEvent::Text("next".into()));
    assert_eq!(second.text(), "next");
    let _ = runtime.handle_input(InputEvent::Key(shortcut_key_down(Code::KeyA)));
    let _ = runtime.handle_input(InputEvent::Key(shortcut_key_down(Code::KeyX)));
    assert_eq!(second.text(), "");
    let _ = runtime.handle_input(InputEvent::Key(shortcut_key_down(Code::KeyV)));
    assert_eq!(second.text(), "next");
    let (_, stats) = runtime.run_frame(constraints).unwrap();
    assert_eq!(runtime.tree().diagnostics().rebuilds, before.rebuilds);
    // The two fields plus their flex ancestor are repainted; unrelated
    // widget descriptions were not rebuilt.
    assert!(stats.repainted_render_objects <= 3);
}

#[test]
fn focused_native_style_backspace_repeat_and_delete_edit_the_buffer() {
    use incular_widgets::{EditableText, internal::TextEditingController};
    let controller = TextEditingController::with_text("abc");
    let mut runtime = Runtime::new(EditableText::new(controller.clone()).into()).unwrap();
    let constraints = Constraints::tight(Size::new(140., 50.));
    runtime.run_frame(constraints).unwrap();
    let _ = runtime.handle_input(InputEvent::Pointer {
        phase: PointerPhase::Down,
        position: Offset::new(100., 5.),
    });
    for expected in ["ab", "a", "", ""] {
        let mut event = key_down(Code::Backspace);
        event.repeat = true;
        let _ = runtime.handle_input(InputEvent::Key(event));
        assert_eq!(controller.text(), expected);
    }
    controller.set_text("é👩‍💻");
    let _ = runtime.handle_input(InputEvent::Key(key_down(Code::Backspace)));
    assert_eq!(controller.text(), "é");
    controller.set_selection(TextSelection::collapsed(0));
    let _ = runtime.handle_input(InputEvent::Key(key_down(Code::Delete)));
    assert_eq!(controller.text(), "");
    assert_eq!(runtime.editing_diagnostics().backspace_commands, 5);
    assert_eq!(runtime.editing_diagnostics().delete_commands, 1);
}

#[test]
fn selectable_text_pointer_drag_shift_extension_and_copy_are_read_only() {
    use incular_widgets::internal::SelectionAreaController;

    #[derive(Clone)]
    struct TestClipboard(Rc<RefCell<String>>);
    impl Clipboard for TestClipboard {
        fn get_text(&mut self) -> Option<String> {
            (!self.0.borrow().is_empty()).then(|| self.0.borrow().clone())
        }
        fn set_text(&mut self, text: String) {
            *self.0.borrow_mut() = text;
        }
    }

    let controller = SelectionAreaController::new();
    let mut runtime = Runtime::new(Widget::selection_area(
        controller.clone(),
        Widget::column(vec![
            Widget::selectable_text_styled(
                "first",
                incular_text::TextStyle::default(),
                incular_text::TextAlign::Start,
            ),
            Widget::selectable_text_styled(
                "second",
                incular_text::TextStyle::default(),
                incular_text::TextAlign::Start,
            ),
        ]),
    ))
    .unwrap();
    let constraints = Constraints::tight(Size::new(180., 80.));
    let _ = runtime.run_frame(constraints).unwrap();
    let area = runtime.tree().root().unwrap();
    let column = runtime.tree().children(area).unwrap()[0];
    let labels = runtime.tree().children(column).unwrap();
    let second = labels[1];
    let first_bounds = runtime.tree().element_bounds(labels[0]).unwrap();
    let second_bounds = runtime.tree().element_bounds(second).unwrap();
    let first_point = first_bounds.origin + Offset::new(1., first_bounds.size.height * 0.5);
    // Stay inside the hit-test box while asking Parley's layout for the
    // final visual cluster.
    let second_point = second_bounds.origin
        + Offset::new(
            (second_bounds.size.width - 0.1).max(0.),
            second_bounds.size.height * 0.5,
        );
    assert_eq!(
        runtime.tree().selectable_text_at(first_point),
        Some(labels[0])
    );
    assert_eq!(
        runtime.tree().selectable_text_at(second_point),
        Some(second)
    );
    let copied = Rc::new(RefCell::new(String::new()));
    runtime.set_clipboard(Box::new(TestClipboard(copied.clone())));
    let _ = runtime.handle_input(InputEvent::Pointer {
        phase: PointerPhase::Down,
        position: first_point,
    });
    let _ = runtime.handle_input(InputEvent::Pointer {
        phase: PointerPhase::Move,
        position: second_point,
    });
    let _ = runtime.handle_input(InputEvent::Pointer {
        phase: PointerPhase::Up,
        position: second_point,
    });
    let mut shift_right = key_down(Code::ArrowRight);
    shift_right.modifiers = Modifiers::SHIFT;
    let _ = runtime.handle_input(InputEvent::Key(shift_right));
    assert_eq!(controller.selected_text(), "first\nsecond");
    let _ = runtime.handle_input(InputEvent::Key(shortcut_key_down(Code::KeyC)));
    assert_eq!(&*copied.borrow(), "first\nsecond");
    assert_eq!(runtime.focused_element(), Some(second));
}

#[test]
fn multiline_enter_replaces_selection_while_single_line_submits() {
    use incular_widgets::{
        EditableText,
        internal::{TextEditingController, TextSelection},
    };
    let single = TextEditingController::with_text("one");
    let multi = TextEditingController::with_text("ab cdef");
    let submitted = Rc::new(RefCell::new(0));
    let observed = submitted.clone();
    let mut runtime = Runtime::new(Widget::column(vec![
        EditableText::new(single.clone())
            .on_submit(move |_| *observed.borrow_mut() += 1)
            .into(),
        EditableText::new(multi.clone())
            .multiline(true)
            .height(100.)
            .into(),
    ]))
    .unwrap();
    runtime
        .run_frame(Constraints::tight(Size::new(300., 200.)))
        .unwrap();
    let enter = || InputEvent::Key(key_down(Code::Enter));
    let _ = runtime.handle_input(InputEvent::Pointer {
        phase: PointerPhase::Down,
        position: Offset::new(5., 5.),
    });
    let _ = runtime.handle_input(enter());
    assert_eq!(single.text(), "one");
    assert_eq!(*submitted.borrow(), 1);
    let _ = runtime.handle_input(InputEvent::Pointer {
        phase: PointerPhase::Down,
        position: Offset::new(5., 50.),
    });
    multi.set_selection(TextSelection { base: 2, extent: 5 });
    let _ = runtime.handle_input(enter());
    assert_eq!(multi.text(), "ab\nef");
    let mut shifted_enter = key_down(Code::Enter);
    shifted_enter.modifiers = Modifiers::SHIFT;
    let _ = runtime.handle_input(InputEvent::Key(shifted_enter));
    assert_eq!(multi.text(), "ab\n\nef");
}

#[test]
fn runtime_routes_pointer_sequences_to_retained_gesture_regions() {
    let taps = Rc::new(Cell::new(0));
    let observed = taps.clone();
    let mut runtime = Runtime::new(
        GestureDetector::new(Widget::box_(Size::new(80., 40.), Color::WHITE))
            .callbacks(GestureCallbacks {
                on_tap: Some(Rc::new(move || observed.set(observed.get() + 1))),
                ..GestureCallbacks::default()
            })
            .into(),
    )
    .unwrap();
    runtime
        .run_frame(Constraints::tight(Size::new(100., 100.)))
        .unwrap();
    let down = runtime.handle_input(InputEvent::Pointer {
        phase: PointerPhase::Down,
        position: Offset::new(10., 10.),
    });
    assert!(down.is_some_and(|target| target.action.is_none()));
    let up = runtime.handle_input(InputEvent::Pointer {
        phase: PointerPhase::Up,
        position: Offset::new(90., 90.),
    });
    assert!(up.is_some_and(|target| target.action.is_none()));
    assert_eq!(taps.get(), 1);
}

#[test]
fn runtime_routes_identified_contacts_to_retained_scale_regions() {
    let scale = Rc::new(Cell::new(0.));
    let observed = scale.clone();
    let mut runtime = Runtime::new(
        GestureDetector::new(Widget::box_(Size::new(100., 100.), Color::WHITE))
            .callbacks(GestureCallbacks {
                on_scale_update: Some(Rc::new(move |details| observed.set(details.scale))),
                ..GestureCallbacks::default()
            })
            .into(),
    )
    .unwrap();
    runtime
        .run_frame(Constraints::tight(Size::new(100., 100.)))
        .unwrap();
    for (pointer, position) in [(1, Offset::new(10., 10.)), (2, Offset::new(20., 10.))] {
        assert!(
            runtime
                .handle_input(InputEvent::PointerWithId {
                    pointer,
                    phase: PointerPhase::Down,
                    position,
                })
                .is_some()
        );
    }
    assert!(
        runtime
            .handle_input(InputEvent::PointerWithId {
                pointer: 2,
                phase: PointerPhase::Move,
                position: Offset::new(30., 10.),
            })
            .is_some()
    );
    assert_eq!(scale.get(), 2.);
}

fn test_window_options(title: &str, width: f32, height: f32) -> WindowOptions {
    WindowOptions {
        title: title.into(),
        initial_logical_size: Size::new(width, height),
        ..WindowOptions::default()
    }
}

#[test]
fn virtual_windows_keep_trees_environments_focus_and_semantics_independent() {
    let mut application = Application::new(|_| ActionSurface::new("A").into()).unwrap();
    let a = application.primary_window();
    let b = application
        .open_window_with(test_window_options("B", 360., 240.), |_| {
            ActionSurface::new("B").into()
        })
        .unwrap()
        .id();
    let environment_a = RuntimeEnvironment {
        viewport: Size::new(640., 400.),
        scale_factor: 1.,
        safe_insets: incular_config::EdgeInsets::all(4.),
        ..RuntimeEnvironment::default()
    };
    let environment_b = RuntimeEnvironment {
        viewport: Size::new(320., 180.),
        scale_factor: 2.,
        safe_insets: incular_config::EdgeInsets::all(9.),
        ..RuntimeEnvironment::default()
    };
    assert!(application.set_window_environment(a, environment_a));
    assert!(application.set_window_environment(b, environment_b));
    application.handle_window_event(WindowEvent::platform(
        a,
        PlatformEvent::Metrics(WindowMetrics::new(
            incular_platform::PhysicalSize::new(640, 400),
            1.,
        )),
    ));
    application.handle_window_event(WindowEvent::platform(
        b,
        PlatformEvent::Metrics(WindowMetrics::new(
            incular_platform::PhysicalSize::new(640, 360),
            2.,
        )),
    ));
    application.handle_window_event(WindowEvent::lifecycle(a, WindowLifecycle::Focused));
    application.handle_window_event(WindowEvent::lifecycle(b, WindowLifecycle::Unfocused));
    let _ = application
        .run_window_frame_at(a, Constraints::tight(Size::new(640., 400.)), Instant::now())
        .unwrap();
    let _ = application
        .run_window_frame_at(b, Constraints::tight(Size::new(320., 180.)), Instant::now())
        .unwrap();
    let a_state = application.window_diagnostics(a).unwrap();
    let b_state = application.window_diagnostics(b).unwrap();
    assert_ne!(a_state.logical_size, b_state.logical_size);
    assert_eq!(a_state.scale_factor, 1.);
    assert_eq!(b_state.scale_factor, 2.);
    assert!(a_state.native_focused);
    assert!(!b_state.native_focused);
    assert!(a_state.semantics > 0 && b_state.semantics > 0);
    let b_surface = b_state.surface_generation;
    application.handle_window_event(WindowEvent::platform(
        a,
        PlatformEvent::Metrics(WindowMetrics::new(
            incular_platform::PhysicalSize::new(1_200, 800),
            1.5,
        )),
    ));
    assert!(
        application
            .window_diagnostics(a)
            .unwrap()
            .surface_generation
            > a_state.surface_generation
    );
    assert_eq!(
        application
            .window_diagnostics(b)
            .unwrap()
            .surface_generation,
        b_surface
    );
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
    let mut application =
        Application::new(|_| Widget::opacity(0.8, Widget::box_(Size::new(20., 20.), Color::WHITE)))
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

#[test]
fn accessibility_projections_actions_and_close_are_window_local() {
    let a_hits = Rc::new(Cell::new(0));
    let mut application = Application::new({
        let a_hits = a_hits.clone();
        move |_| {
            ActionSurface::new("A")
                .on_press({
                    let a_hits = a_hits.clone();
                    move || a_hits.set(a_hits.get() + 1)
                })
                .into()
        }
    })
    .unwrap();
    let a = application.primary_window();
    let b = application
        .open_window_with(test_window_options("B", 180., 100.), |_| {
            ActionSurface::new("B").into()
        })
        .unwrap()
        .id();
    for id in [a, b] {
        application
            .run_window_frame_at(
                id,
                Constraints::tight(Size::new(180., 100.)),
                Instant::now(),
            )
            .unwrap();
    }
    let mut adapter_a = AccessKitProjection::new();
    let mut adapter_b = AccessKitProjection::new();
    adapter_a.activate();
    adapter_b.activate();
    let a_update = application
        .sync_accessibility(a, &mut adapter_a)
        .expect("A initial tree");
    assert_eq!(
        a_update.kind(),
        incular_accessibility::AccessKitUpdateKind::Full
    );
    assert_eq!(
        application
            .sync_accessibility(b, &mut adapter_b)
            .expect("B initial tree")
            .kind(),
        incular_accessibility::AccessKitUpdateKind::Full
    );
    assert!(application.sync_accessibility(b, &mut adapter_b).is_none());

    let update = a_update.into_accesskit();
    let native = update
        .nodes
        .iter()
        .find(|(_, node)| node.role() == accesskit::Role::Button && node.label() == Some("A"))
        .map(|(id, _)| *id)
        .expect("A button has a native node id");
    let request = ActionRequest {
        action: AccessKitAction::Click,
        target_tree: TreeId::ROOT,
        target_node: native,
        data: None,
    };
    let request = adapter_a.translate_action(&request).expect("live A node");
    assert!(application.dispatch_accessibility_action(a, request));
    assert_eq!(a_hits.get(), 1);
    // A's action does not publish or mutate B's adapter/tree.
    assert!(application.sync_accessibility(b, &mut adapter_b).is_none());

    assert!(application.close_window(a));
    adapter_a.note_adapter_destroyed();
    assert!(!application.contains_window(a));
    assert!(application.contains_window(b));
    application
        .run_window_frame_at(b, Constraints::tight(Size::new(180., 100.)), Instant::now())
        .unwrap();
    assert!(application.window_diagnostics(b).is_some());
}

#[test]
fn window_events_route_to_only_the_selected_retained_root() {
    let a_hits = Rc::new(Cell::new(0));
    let b_hits = Rc::new(Cell::new(0));
    let a_observed = a_hits.clone();
    let mut application = Application::new(move |_| {
        ActionSurface::new("A")
            .on_press({
                let hits = a_observed.clone();
                move || hits.set(hits.get() + 1)
            })
            .into()
    })
    .unwrap();
    let a = application.primary_window();
    let b_observed = b_hits.clone();
    let b = application
        .open_window_with(test_window_options("B", 120., 80.), move |_| {
            ActionSurface::new("B")
                .on_press({
                    let hits = b_observed.clone();
                    move || hits.set(hits.get() + 1)
                })
                .into()
        })
        .unwrap()
        .id();
    for id in [a, b] {
        let _ = application
            .run_window_frame_at(id, Constraints::tight(Size::new(120., 80.)), Instant::now())
            .unwrap();
        for phase in [PointerPhase::Down, PointerPhase::Up] {
            application.handle_window_event(WindowEvent::platform(
                id,
                PlatformEvent::Input(InputEvent::Pointer {
                    phase,
                    position: Offset::new(10., 10.),
                }),
            ));
        }
    }
    assert_eq!(a_hits.get(), 1);
    assert_eq!(b_hits.get(), 1);
}

#[test]
fn shared_and_window_local_signals_dirty_only_subscribed_windows() {
    let shared = Signal::new(0_u32);
    let local_a = Signal::new(0_u32);
    let a_shared = shared.clone();
    let a_local = local_a.clone();
    let mut application = Application::new(move |_| {
        Text::new(format!("{}:{}", a_shared.get(), a_local.get())).into()
    })
    .unwrap();
    let a = application.primary_window();
    let b_shared = shared.clone();
    let b = application
        .open_window_with(test_window_options("B", 180., 100.), move |_| {
            Text::new(format!("{}", b_shared.get())).into()
        })
        .unwrap()
        .id();
    for id in [a, b] {
        let _ = application
            .run_window_frame_at(
                id,
                Constraints::tight(Size::new(180., 100.)),
                Instant::now(),
            )
            .unwrap();
    }
    assert!(shared.set(1));
    assert!(application.frame_requested(a));
    assert!(application.frame_requested(b));
    let _ = application
        .run_window_frame_at(a, Constraints::tight(Size::new(180., 100.)), Instant::now())
        .unwrap();
    let _ = application
        .run_window_frame_at(b, Constraints::tight(Size::new(180., 100.)), Instant::now())
        .unwrap();
    local_a.set(1);
    assert!(application.frame_requested(a));
    assert!(!application.frame_requested(b));
}

#[test]
fn close_cancels_only_its_window_scope_and_rejects_stale_handle_commands() {
    let saved_scope = Rc::new(RefCell::new(None));
    let capture = saved_scope.clone();
    let mut application =
        Application::new(|_| Widget::fixed_box(Size::new(1., 1.), Color::WHITE)).unwrap();
    let application_task = application.spawn(async { 7_u32 });
    let stale = application
        .open_window_with(test_window_options("Old", 100., 100.), move |cx| {
            *capture.borrow_mut() = Some(cx.task_scope());
            Widget::fixed_box(Size::new(1., 1.), Color::WHITE)
        })
        .unwrap();
    let stale_id = stale.id();
    assert!(application.close_window(stale_id));
    assert!(saved_scope.borrow().as_ref().unwrap().is_cancelled());
    assert!(!application_task.handle().is_cancelled());
    let replacement = application
        .open_window(
            test_window_options("Replacement", 100., 100.),
            Widget::fixed_box(Size::new(1., 1.), Color::WHITE),
        )
        .unwrap();
    assert_eq!(replacement.id().index(), stale_id.index());
    assert_ne!(replacement.id().generation(), stale_id.generation());
    assert!(stale.set_title("stale"));
    application.process_runtime_work();
    assert_eq!(
        application
            .window_diagnostics(replacement.id())
            .unwrap()
            .title,
        "Replacement"
    );
    assert!(application.diagnostics().stale_window_commands >= 1);
}

#[test]
fn closing_one_window_and_stress_open_close_leave_other_roots_alive() {
    let mut application =
        Application::new(|_| Widget::fixed_box(Size::new(2., 2.), Color::WHITE)).unwrap();
    let primary = application.primary_window();
    let other = application
        .open_window(
            test_window_options("Other", 100., 100.),
            Widget::fixed_box(Size::new(2., 2.), Color::WHITE),
        )
        .unwrap()
        .id();
    assert!(application.close_window(other));
    assert!(application.contains_window(primary));
    for index in 0..1_000 {
        let handle = application
            .open_window(
                test_window_options(&format!("Transient {index}"), 80., 60.),
                Widget::fixed_box(Size::new(1., 1.), Color::WHITE),
            )
            .unwrap();
        assert!(application.close_window(handle.id()));
    }
    assert_eq!(application.active_window_ids(), vec![primary]);
    assert_eq!(application.diagnostics().active_windows, 1);
}

#[test]
fn final_window_policy_and_simultaneous_virtual_roots_are_explicit() {
    let mut application =
        Application::new(|_| Widget::fixed_box(Size::new(1., 1.), Color::WHITE)).unwrap();
    for index in 0..31 {
        application
            .open_window(
                test_window_options(&format!("Window {index}"), 80., 60.),
                Widget::fixed_box(Size::new(1., 1.), Color::WHITE),
            )
            .unwrap();
    }
    assert_eq!(application.diagnostics().active_windows, 32);
    application.set_last_window_policy(LastWindowPolicy::KeepRunning);
    for id in application.active_window_ids() {
        assert!(application.close_window(id));
    }
    assert!(!application.should_exit());

    let mut default_policy =
        Application::new(|_| Widget::fixed_box(Size::new(1., 1.), Color::WHITE)).unwrap();
    default_policy
        .open_window(
            WindowOptions {
                visible: false,
                ..test_window_options("Hidden helper", 80., 60.)
            },
            Widget::fixed_box(Size::new(1., 1.), Color::WHITE),
        )
        .unwrap();
    assert!(default_policy.close_window(default_policy.primary_window()));
    assert!(default_policy.should_exit());
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
        |_| Widget::fixed_box(Size::new(1., 1.), Color::WHITE),
    )
    .unwrap();
    let inspector = first
        .open_restorable_window_with(
            WindowRestorationId::new("inspector").unwrap(),
            "inspector",
            test_window_options("Inspector", 480., 260.),
            |_| Widget::fixed_box(Size::new(1., 1.), Color::WHITE),
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
        |_| Widget::fixed_box(Size::new(1., 1.), Color::WHITE),
    )
    .unwrap();
    second
        .register_restorable_window_factory(
            "inspector",
            test_window_options("Inspector default", 100., 100.),
            |_| Widget::fixed_box(Size::new(1., 1.), Color::WHITE),
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
