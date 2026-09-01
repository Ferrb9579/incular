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

mod runtime {
    use super::*;

    mod devtools_performance;
    mod layout_semantics;
    mod reactive_scheduling;
    mod restoration_tasks;
    mod retained_interactions;
}

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
        Widget::from(incular_widgets::Column::new(vec![
            Widget::box_(Size::new(10., 10.), Color::WHITE).with_key(11_u64),
            Widget::box_(Size::new(10., 10.), Color::WHITE).with_key(11_u64),
        ]))
    };
    let mut runtime = Runtime::new(LayoutBuilder::new(move |_, _| invalid()).into())
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
    let mut runtime = Runtime::new(Widget::from(
        incular_widgets::Column::new(vec![
            Widget::from(EditableText::new(first.clone()).size(Size::new(120., 40.))),
            Widget::from(EditableText::new(second.clone()).size(Size::new(120., 40.))),
        ])
        .main_axis_size(incular_config::MainAxisSize::Min)
        .cross_axis_alignment(incular_config::CrossAxisAlignment::Start),
    ))
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
        Widget::from(incular_widgets::Column::new(vec![
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
        ])),
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
    let mut runtime = Runtime::new(Widget::from(incular_widgets::Column::new(vec![
        Widget::from(
            EditableText::new(single.clone()).on_submit(move |_| *observed.borrow_mut() += 1),
        ),
        Widget::from(
            EditableText::new(multi.clone())
                .multiline(true)
                .height(100.),
        ),
    ])))
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

fn take_resize_requests(application: &mut Application) -> Vec<Size> {
    application
        .take_native_window_commands()
        .into_iter()
        .filter_map(|command| match command {
            NativeWindowCommand::Operate(WindowCommand {
                operation: WindowOperation::SetLogicalSize(size),
                ..
            }) => Some(size),
            _ => None,
        })
        .collect()
}

#[test]
fn content_sized_window_tracks_primary_layout_and_ignores_paint_overflow() {
    use incular_config::WindowSizePolicy;
    use incular_widgets::{Positioned, Stack};

    let settings_open = Signal::new(false);
    let observed = settings_open.clone();
    let options = WindowOptions {
        initial_logical_size: Size::new(100., 200.),
        decorations: false,
        size_policy: WindowSizePolicy::Content,
        ..WindowOptions::default()
    };
    let mut application = Application::new_with_options(options, move |_| {
        let base_size = if observed.get() {
            Size::new(420., 560.)
        } else {
            Size::new(100., 200.)
        };
        let base = Widget::box_(base_size, Color::WHITE);
        // Paint/layout overflow outside the primary root extent is not window
        // content sizing. This models a transient overlay/menu that must be
        // presented independently rather than making the top-level chase it.
        let panel: Widget = Positioned::new(Widget::box_(Size::new(100., 120.), Color::BLACK))
            .left(0.)
            .top(base_size.height)
            .width(100.)
            .height(120.)
            .into();
        Stack::new([base, panel]).into()
    })
    .unwrap();
    let id = application.primary_window();
    let _ = application.take_native_window_commands();

    application
        .run_window_frame_at(
            id,
            Constraints::tight(Size::new(100., 200.)),
            Instant::now(),
        )
        .unwrap();
    assert!(take_resize_requests(&mut application).is_empty());

    assert!(settings_open.set(true));
    application
        .run_window_frame_at(
            id,
            Constraints::tight(Size::new(100., 200.)),
            Instant::now(),
        )
        .unwrap();
    assert_eq!(
        take_resize_requests(&mut application),
        [Size::new(420., 560.)]
    );

    // The native resize is asynchronous. Re-rendering before a metrics event
    // must not flood the event loop with the same request.
    application
        .run_window_frame_at(
            id,
            Constraints::tight(Size::new(100., 200.)),
            Instant::now(),
        )
        .unwrap();
    assert!(take_resize_requests(&mut application).is_empty());

    application.handle_window_event(WindowEvent::platform(
        id,
        PlatformEvent::Metrics(WindowMetrics::new(
            incular_platform::PhysicalSize::new(420, 560),
            1.,
        )),
    ));
    application
        .run_window_frame_at(
            id,
            Constraints::tight(Size::new(420., 560.)),
            Instant::now(),
        )
        .unwrap();
    assert!(take_resize_requests(&mut application).is_empty());

    assert!(settings_open.set(false));
    application
        .run_window_frame_at(
            id,
            Constraints::tight(Size::new(420., 560.)),
            Instant::now(),
        )
        .unwrap();
    assert_eq!(
        take_resize_requests(&mut application),
        [Size::new(100., 200.)]
    );
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
        Application::new(|_| Widget::box_(Size::new(1., 1.), Color::WHITE)).unwrap();
    let application_task = application.spawn(async { 7_u32 });
    let stale = application
        .open_window_with(test_window_options("Old", 100., 100.), move |cx| {
            *capture.borrow_mut() = Some(cx.task_scope());
            Widget::box_(Size::new(1., 1.), Color::WHITE)
        })
        .unwrap();
    let stale_id = stale.id();
    assert!(application.close_window(stale_id));
    assert!(saved_scope.borrow().as_ref().unwrap().is_cancelled());
    assert!(!application_task.handle().is_cancelled());
    let replacement = application
        .open_window(
            test_window_options("Replacement", 100., 100.),
            Widget::box_(Size::new(1., 1.), Color::WHITE),
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
        Application::new(|_| Widget::box_(Size::new(2., 2.), Color::WHITE)).unwrap();
    let primary = application.primary_window();
    let other = application
        .open_window(
            test_window_options("Other", 100., 100.),
            Widget::box_(Size::new(2., 2.), Color::WHITE),
        )
        .unwrap()
        .id();
    assert!(application.close_window(other));
    assert!(application.contains_window(primary));
    for index in 0..1_000 {
        let handle = application
            .open_window(
                test_window_options(&format!("Transient {index}"), 80., 60.),
                Widget::box_(Size::new(1., 1.), Color::WHITE),
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
        Application::new(|_| Widget::box_(Size::new(1., 1.), Color::WHITE)).unwrap();
    for index in 0..31 {
        application
            .open_window(
                test_window_options(&format!("Window {index}"), 80., 60.),
                Widget::box_(Size::new(1., 1.), Color::WHITE),
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
        Application::new(|_| Widget::box_(Size::new(1., 1.), Color::WHITE)).unwrap();
    default_policy
        .open_window(
            WindowOptions {
                visible: false,
                ..test_window_options("Hidden helper", 80., 60.)
            },
            Widget::box_(Size::new(1., 1.), Color::WHITE),
        )
        .unwrap();
    assert!(default_policy.close_window(default_policy.primary_window()));
    assert!(default_policy.should_exit());
}
