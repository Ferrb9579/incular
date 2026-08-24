use incular::prelude::*;

#[test]
fn test_controllers_contract() {
    let scroll_ctrl = ScrollController::new();
    scroll_ctrl.update_extents(1000.0, 300.0);
    assert_eq!(scroll_ctrl.max_offset(), 700.0);
    scroll_ctrl.jump_to(150.0);
    assert_eq!(scroll_ctrl.offset(), 150.0);

    let page_ctrl = PageController::new();
    page_ctrl.update_extents(1000.0, 300.0);
    page_ctrl.jump_to(200.0);
    assert_eq!(page_ctrl.offset(), 200.0);

    let text_ctrl = TextEditingController::with_text("Initial");
    assert_eq!(text_ctrl.text(), "Initial");
    text_ctrl.set_text("Updated");
    assert_eq!(text_ctrl.text(), "Updated");
    text_ctrl.clear();
    assert_eq!(text_ctrl.text(), "");

    let focus = FocusNode::new();
    assert!(!focus.has_focus());
    focus.request_focus();
    assert!(focus.has_focus());
    focus.unfocus();
    assert!(!focus.has_focus());
}
