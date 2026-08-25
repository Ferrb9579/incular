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
fn checkbox_and_switch_have_uncontrolled_visual_state() {
    let app = Application::new(|_| {
        Widget::column([
            Widget::from(Checkbox::new(false)),
            Widget::from(Switch::new(false)),
        ])
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
