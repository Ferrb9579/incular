use incular_config::Constraints;
use incular_core::{Color, InputEvent, Offset, PointerPhase, Size};
use incular_runtime::Runtime;
use incular_widgets::{
    Column, FocusScope, LayoutBuilder, Widget,
    internal::{ActionId, action},
};

fn deferred_autofocus() -> Widget {
    LayoutBuilder::new(|_, _| {
        FocusScope::new(action(Size::new(80., 40.), Color::WHITE, ActionId(2)))
            .autofocus(true)
            .into()
    })
    .into()
}

fn frame(runtime: &mut Runtime) {
    runtime
        .run_frame(Constraints::loose(Size::new(200., 200.)))
        .expect("frame");
}

fn click_outside(runtime: &mut Runtime) {
    for phase in [PointerPhase::Down, PointerPhase::Up] {
        let _ = runtime.handle_input(InputEvent::Pointer {
            phase,
            position: Offset::new(500., 500.),
        });
    }
}

#[test]
fn initial_layout_resolves_autofocus_without_reclaiming_cleared_focus() {
    let mut runtime = Runtime::new(deferred_autofocus()).expect("mount");
    assert_eq!(runtime.focused_element(), None);
    frame(&mut runtime);
    let target = runtime
        .tree()
        .autofocus_element()
        .expect("autofocus target");
    assert_eq!(runtime.focused_element(), Some(target));
    click_outside(&mut runtime);
    assert_eq!(runtime.focused_element(), None);
    frame(&mut runtime);
    assert_eq!(runtime.focused_element(), None);
}

#[test]
fn explicit_clear_before_initial_layout_cancels_autofocus() {
    let mut runtime = Runtime::new(deferred_autofocus()).expect("mount");
    click_outside(&mut runtime);
    frame(&mut runtime);
    assert!(runtime.tree().autofocus_element().is_some());
    assert_eq!(runtime.focused_element(), None);
}

#[test]
fn an_empty_initial_frame_does_not_leave_an_autofocus_request_pending() {
    let mut runtime = Runtime::new(Column::new(Vec::<Widget>::new()).into()).expect("mount");
    frame(&mut runtime);
    let root = runtime.tree().root().expect("root");
    runtime
        .schedule_update(root, Column::new([deferred_autofocus()]).into())
        .expect("replace root");
    frame(&mut runtime);
    assert!(runtime.tree().autofocus_element().is_some());
    assert_eq!(runtime.focused_element(), None);
}

#[test]
fn eager_autofocus_remains_selected_when_layout_creates_another_request() {
    let root: Widget = Column::new([
        FocusScope::new(action(Size::new(80., 40.), Color::WHITE, ActionId(1)))
            .autofocus(true)
            .into(),
        deferred_autofocus(),
    ])
    .into();
    let mut runtime = Runtime::new(root).expect("mount");
    let original = runtime.focused_element().expect("eager focus");
    frame(&mut runtime);
    assert_eq!(runtime.focused_element(), Some(original));
}
