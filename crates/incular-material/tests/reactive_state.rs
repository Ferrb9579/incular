use incular_core::BuildContext;
use incular_material::{WidgetState, WidgetStatesController};

#[test]
fn widget_states_invalidate_readers_only_on_a_change() {
    let context = BuildContext::new();
    let controller = WidgetStatesController::new();
    context.build(|_| {
        let _ = controller.states();
    });
    controller.set(WidgetState::Hovered, false);
    assert!(!context.is_dirty());
    controller.set(WidgetState::Hovered, true);
    assert!(context.take_dirty());
    controller.set(WidgetState::Hovered, true);
    assert!(!context.is_dirty());
}
