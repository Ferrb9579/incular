use incular_config::Constraints;
use incular_core::{Color, ImeEvent, InputEvent, Offset, PointerPhase, Size};
use incular_platform::{
    PhysicalSize, PlatformEvent, WindowEvent, WindowLifecycle, WindowMetrics, WindowOptions,
};
use incular_runtime::{Application, Runtime};
use incular_text::TextEditingController;
use incular_widgets::{
    EditableText, Widget,
    internal::{ActionId, action},
};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

fn frame(app: &mut Application) {
    app.run_window_frame_at(
        app.primary_window(),
        Constraints::tight(Size::new(200., 100.)),
        Instant::now(),
    )
    .unwrap();
}
#[test]
fn adoption_preserves_tree_identity_and_already_queued_dispatch() {
    let runtime = Runtime::new(Widget::box_(Size::new(20., 20.), Color::WHITE)).unwrap();
    let root = runtime.tree().root().unwrap();
    let ran = Arc::new(AtomicBool::new(false));
    let observed = ran.clone();
    runtime.dispatcher().dispatch(move |runtime| {
        assert_eq!(runtime.tree().root(), Some(root));
        observed.store(true, Ordering::SeqCst);
    });
    let mut app = Application::from_runtime(runtime, |_| {});
    app.process_runtime_work();
    assert!(ran.load(Ordering::SeqCst));
    assert_eq!(app.into_runtime().tree().root(), Some(root));
}
#[test]
fn adopted_legacy_actions_fire_once_and_cancelled_pointer_sequences_do_not_activate() {
    let runtime = Runtime::new(action(Size::new(200., 100.), Color::WHITE, ActionId(7))).unwrap();
    let actions = Rc::new(RefCell::new(Vec::new()));
    let observed = actions.clone();
    let mut app = Application::from_runtime(runtime, move |id| observed.borrow_mut().push(id));
    frame(&mut app);
    for phase in [
        PointerPhase::Down,
        PointerPhase::Cancel,
        PointerPhase::Up,
        PointerPhase::Down,
        PointerPhase::Up,
    ] {
        app.handle_window_event(WindowEvent::platform(
            app.primary_window(),
            PlatformEvent::Input(InputEvent::Pointer {
                phase,
                position: Offset::new(10., 10.),
            }),
        ));
    }
    assert_eq!(*actions.borrow(), vec![ActionId(7)]);
}
fn editing_replay(adopt: bool, auxiliary: bool) -> (String, bool, usize) {
    let controller = TextEditingController::new();
    let root: Widget = EditableText::new(controller.clone())
        .size(Size::new(200., 100.))
        .into();
    let mut app = if adopt {
        Application::from_runtime(Runtime::new(root).unwrap(), |_| {})
    } else {
        Application::new(move |_| root.clone()).unwrap()
    };
    if auxiliary {
        app.open_window(
            WindowOptions::default(),
            Widget::box_(Size::new(20., 20.), Color::BLACK),
        )
        .unwrap();
    }
    let id = app.primary_window();
    app.handle_window_event(WindowEvent::platform(
        id,
        PlatformEvent::Metrics(WindowMetrics::new(PhysicalSize::new(300, 150), 1.5)),
    ));
    app.handle_window_event(WindowEvent::lifecycle(id, WindowLifecycle::Focused));
    frame(&mut app);
    for phase in [PointerPhase::Down, PointerPhase::Up] {
        app.handle_window_event(WindowEvent::platform(
            id,
            PlatformEvent::Input(InputEvent::Pointer {
                phase,
                position: Offset::new(10., 10.),
            }),
        ));
    }
    app.handle_window_event(WindowEvent::platform(
        id,
        PlatformEvent::Input(InputEvent::Ime(ImeEvent::Preedit {
            text: "é".into(),
            selection: None,
        })),
    ));
    app.handle_window_event(WindowEvent::platform(
        id,
        PlatformEvent::Input(InputEvent::Ime(ImeEvent::Commit("é".into()))),
    ));
    frame(&mut app);
    let commands = app.take_window_text_input_commands(id);
    let mut runtime = app.into_runtime();
    let node = runtime
        .tree()
        .semantics()
        .iter()
        .find_map(|(id, node)| (node.role == incular_semantics::Role::TextField).then_some(id))
        .unwrap();
    assert!(runtime.dispatch_semantic_action(
        node,
        incular_semantics::SemanticAction::SetText("accessible".into())
    ));
    (
        controller.text(),
        runtime.focused_element().is_some(),
        commands.len(),
    )
}
#[test]
fn editing_focus_ime_and_semantics_match_for_both_entries_and_multiple_windows() {
    let expected = editing_replay(false, false);
    assert_eq!(expected.0, "accessible");
    assert!(expected.1);
    assert!(expected.2 > 0);
    for (adopt, auxiliary) in [(true, false), (false, true), (true, true)] {
        assert_eq!(editing_replay(adopt, auxiliary), expected);
    }
}
#[test]
fn adopted_window_scope_still_cancels_existing_tasks_on_close() {
    let runtime = Runtime::new(Widget::box_(Size::new(20., 20.), Color::WHITE)).unwrap();
    let task = runtime
        .spawner()
        .spawn_in(&runtime.window_task_scope(), std::future::pending::<()>());
    let mut app = Application::from_runtime(runtime, |_| {});
    assert!(!task.handle().is_cancelled());
    app.close_window(app.primary_window());
    assert!(task.handle().is_cancelled());
}

#[test]
fn adopted_legacy_actions_share_keyboard_and_accessibility_activation() {
    let runtime = Runtime::new(action(Size::new(200., 100.), Color::WHITE, ActionId(7))).unwrap();
    let actions = Rc::new(RefCell::new(Vec::new()));
    let observed = actions.clone();
    let mut app = Application::from_runtime(runtime, move |id| observed.borrow_mut().push(id));
    frame(&mut app);
    let mut runtime = app.into_runtime();
    let node = runtime
        .tree()
        .semantics()
        .iter()
        .find_map(|(id, node)| (node.role == incular_semantics::Role::Button).then_some(id))
        .unwrap();
    assert!(runtime.dispatch_semantic_action(node, incular_semantics::SemanticAction::Activate));
    assert!(runtime.dispatch_semantic_action(node, incular_semantics::SemanticAction::Focus));
    let _ = runtime.handle_input(InputEvent::Key(incular_core::KeyboardEvent::key_down(
        incular_core::KeyboardKey::Named(incular_core::NamedKey::Enter),
        incular_core::Code::Enter,
    )));
    assert_eq!(*actions.borrow(), vec![ActionId(7), ActionId(7)]);
}

#[test]
fn adoption_preserves_tracked_builders_and_their_signal_subscriptions() {
    let value = incular_runtime::Signal::new(20_f32);
    let read = value.clone();
    let builds = Rc::new(std::cell::Cell::new(0));
    let observed = builds.clone();
    let mut runtime = Runtime::new(Widget::box_(Size::new(20., 20.), Color::WHITE)).unwrap();
    let root = runtime.tree().root().unwrap();
    runtime
        .register_builder(root, move || {
            observed.set(observed.get() + 1);
            Widget::box_(Size::new(read.get(), 20.), Color::WHITE)
        })
        .unwrap();
    runtime
        .run_frame(Constraints::tight(Size::new(200., 100.)))
        .unwrap();
    let before = builds.get();
    let mut app = Application::from_runtime(runtime, |_| {});
    value.set(40.);
    frame(&mut app);
    assert_eq!(builds.get(), before + 1);
}
