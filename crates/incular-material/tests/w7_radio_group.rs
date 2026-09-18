use incular_config::Constraints;
use incular_core::{InputEvent, Offset, PointerPhase, Size};
use incular_material::RadioGroup;
use incular_runtime::Runtime;
use incular_semantics::{CheckedState, Role};
use incular_widgets::{Column, Widget};

fn click(runtime: &mut Runtime, point: Offset) {
    for phase in [PointerPhase::Down, PointerPhase::Up] {
        let _ = runtime.handle_input(InputEvent::Pointer {
            phase,
            position: point,
        });
    }
}

#[test]
fn radio_group_selection_invalidates_already_mounted_radios() {
    let group = RadioGroup::new();
    group.set_selected(Some(1_u32));
    let mut runtime = Runtime::new(
        Column::new([
            Widget::from(group.radio(1_u32)),
            Widget::from(group.radio(2_u32)),
        ])
        .into(),
    )
    .expect("radio group runtime");
    let constraints = Constraints::loose(Size::new(120.0, 80.0));
    runtime.run_frame(constraints).expect("initial frame");

    let before = runtime
        .tree()
        .semantics()
        .iter()
        .filter(|(_, node)| node.role == Role::Radio)
        .map(|(_, node)| node.state.checked)
        .collect::<Vec<_>>();
    assert_eq!(
        before,
        vec![Some(CheckedState::Checked), Some(CheckedState::Unchecked)]
    );

    click(&mut runtime, Offset::new(8.0, 26.0));
    runtime.run_frame(constraints).expect("selection frame");
    assert_eq!(group.selected(), Some(2_u32));
    let after = runtime
        .tree()
        .semantics()
        .iter()
        .filter(|(_, node)| node.role == Role::Radio)
        .map(|(_, node)| node.state.checked)
        .collect::<Vec<_>>();
    assert_eq!(
        after,
        vec![Some(CheckedState::Unchecked), Some(CheckedState::Checked)]
    );
}

#[test]
fn radio_group_noop_selection_does_not_trigger_change_callback() {
    use std::{cell::Cell, rc::Rc};

    let calls = Rc::new(Cell::new(0_u32));
    let observed = calls.clone();
    let group = RadioGroup::new().on_changed(move |_| observed.set(observed.get() + 1));
    group.set_selected(Some(1_u32));
    let mut runtime = Runtime::new(group.radio(1_u32).into()).expect("radio runtime");
    runtime
        .run_frame(Constraints::loose(Size::new(80.0, 40.0)))
        .expect("frame");
    click(&mut runtime, Offset::new(8.0, 8.0));
    assert_eq!(calls.get(), 0);
}
