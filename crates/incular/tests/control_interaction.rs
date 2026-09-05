use incular::prelude::*;
use incular_config::Constraints;
use incular_controls::{Checkbox, Switch, slider};
use incular_core::{Offset, PointerPhase, Size};

fn click(runtime: &mut incular_runtime::Runtime, point: Offset) {
    let _ = runtime.handle_input(InputEvent::Pointer {
        phase: PointerPhase::Down,
        position: point,
    });
    let _ = runtime.handle_input(InputEvent::Pointer {
        phase: PointerPhase::Up,
        position: point,
    });
}

#[test]
fn checkbox_visual_slots_share_pointer_keyboard_and_semantic_activation() {
    use incular_controls::checkbox::{CheckedState, Root};
    use incular_core::{Code, KeyState, KeyboardEvent, KeyboardKey};
    use incular_semantics::{Role, SemanticAction};
    use std::{cell::RefCell, rc::Rc};
    for state in [
        CheckedState::Unchecked,
        CheckedState::Checked,
        CheckedState::Indeterminate,
    ] {
        for enabled in [false, true] {
            for read_only in [false, true] {
                for custom in [false, true] {
                    for input in 0..3 {
                        let observed = Rc::new(RefCell::new(Vec::new()));
                        let output = observed.clone();
                        let app = Application::new(move |_| {
                            let output = output.clone();
                            let mut checkbox = Root::new()
                                .state(state)
                                .enabled(enabled)
                                .read_only(read_only)
                                .on_checked_change(move |next| output.borrow_mut().push(next));
                            if custom {
                                checkbox = checkbox.child(Widget::box_(
                                    Size::new(24., 24.),
                                    incular_core::Color::WHITE,
                                ));
                            }
                            checkbox.into()
                        })
                        .unwrap();
                        let mut runtime = app.into_runtime();
                        runtime
                            .run_frame(Constraints::loose(Size::new(200., 100.)))
                            .unwrap();
                        let id = runtime
                            .tree()
                            .semantics()
                            .iter()
                            .find(|(_, node)| node.role == Role::Checkbox)
                            .unwrap()
                            .0;
                        match input {
                            0 => click(&mut runtime, Offset::new(8., 8.)),
                            1 => {
                                let _ = runtime.dispatch_semantic_action(id, SemanticAction::Focus);
                                let mut key = KeyboardEvent::key_down(
                                    KeyboardKey::Character(" ".into()),
                                    Code::Space,
                                );
                                let _ = runtime.handle_input(InputEvent::Key(key.clone()));
                                key.state = KeyState::Up;
                                let _ = runtime.handle_input(InputEvent::Key(key));
                            }
                            _ => {
                                let _ =
                                    runtime.dispatch_semantic_action(id, SemanticAction::Activate);
                            }
                        }
                        let expected = if enabled && !read_only {
                            vec![CheckedState::from(!state.is_checked())]
                        } else {
                            vec![]
                        };
                        assert_eq!(
                            *observed.borrow(),
                            expected,
                            "state={state:?}, enabled={enabled}, read_only={read_only}, custom={custom}, input={input}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn checkbox_and_switch_have_uncontrolled_visual_state() {
    let app = Application::new(|_| {
        Widget::from(Column::new([
            Widget::from(Checkbox::new(false)),
            Widget::from(Switch::new(false)),
        ]))
    })
    .expect("application");
    let mut runtime = app.into_runtime();
    let constraints = Constraints::loose(Size::new(280., 100.));
    runtime.run_frame(constraints).expect("initial frame");

    // Both controls use the stateful retained-builder primitive.  A click
    // therefore requests a frame and rematerializes only the control child.
    click(&mut runtime, Offset::new(8., 8.));
    runtime.run_frame(constraints).expect("checkbox frame");
    click(&mut runtime, Offset::new(8., 48.));
    runtime.run_frame(constraints).expect("switch frame");
}

#[test]
fn slider_activation_advances_the_quantized_value() {
    let observed = Signal::new(0.0_f32);
    let app_observed = observed.clone();
    let app = Application::new(move |_| {
        let value = app_observed.get();
        Widget::from(
            slider::Root::new()
                .value(value)
                .step(0.25)
                .on_value_change({
                    let observed = app_observed.clone();
                    move |next| {
                        observed.set(next);
                    }
                }),
        )
    })
    .expect("application");
    let mut runtime = app.into_runtime();
    let constraints = Constraints::loose(Size::new(260., 80.));
    runtime.run_frame(constraints).expect("initial frame");
    click(&mut runtime, Offset::new(20., 3.));
    runtime.run_frame(constraints).expect("slider frame");
    assert_eq!(observed.get(), 0.25);
}

#[test]
fn slider_drag_reports_a_continuous_quantized_value() {
    let observed = Signal::new(0.0_f32);
    let app_observed = observed.clone();
    let app = Application::new(move |_| {
        let value = app_observed.get();
        Widget::from(
            slider::Root::new()
                .value(value)
                .step(0.05)
                .on_value_change({
                    let observed = app_observed.clone();
                    move |next| {
                        observed.set(next);
                    }
                }),
        )
    })
    .expect("application");
    let mut runtime = app.into_runtime();
    let constraints = Constraints::loose(Size::new(260., 80.));
    runtime.run_frame(constraints).expect("initial frame");
    let _ = runtime.handle_input(InputEvent::Pointer {
        phase: PointerPhase::Down,
        position: Offset::new(20., 3.),
    });
    let _ = runtime.handle_input(InputEvent::Pointer {
        phase: PointerPhase::Move,
        position: Offset::new(110., 3.),
    });
    let _ = runtime.handle_input(InputEvent::Pointer {
        phase: PointerPhase::Up,
        position: Offset::new(110., 3.),
    });
    runtime.run_frame(constraints).expect("slider drag frame");
    assert!((observed.get() - 0.5).abs() < f32::EPSILON);
}
