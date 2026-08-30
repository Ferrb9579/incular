use super::*;
use crate::ThemeMode;
use crate::feedback::SnackBar;
use incular_widgets::{ScrollPhysics, SizedBox, Text};

#[test]
fn scroll_behavior_preserves_physics_and_devices() {
    let behavior = MaterialScrollBehavior::new()
        .physics(ScrollPhysics::clamping().always_scrollable())
        .scrollbars(false)
        .drag_devices([MaterialPointerDevice::Mouse]);
    assert!(!behavior.get_scrollbars());
    assert_eq!(behavior.get_drag_devices(), &[MaterialPointerDevice::Mouse]);
    assert_ne!(behavior.get_physics(), ScrollPhysics::clamping());
}

#[test]
fn messenger_controller_queues_and_removes_current_snackbar() {
    let controller = ScaffoldMessengerController::new();
    assert!(!controller.is_showing());
    controller.show_snack_bar(SnackBar::text("first"));
    assert!(controller.is_showing());
    controller.show_snack_bar(SnackBar::text("second"));
    assert!(controller.current_snack_bar().is_some());
    assert_eq!(controller.queued_count(), 1);
    assert!(controller.hide_current_snack_bar().is_some());
    assert!(controller.is_showing());
    assert!(controller.hide_current_snack_bar().is_some());
    assert!(!controller.is_showing());
}

#[test]
fn messenger_mount_exposes_the_current_snackbar_action() {
    let controller = ScaffoldMessengerController::new();
    let mut tree = incular_widgets::internal::WidgetTree::new();
    tree.mount(ScaffoldMessenger::with_controller(controller.clone(), SizedBox::shrink()).into())
        .expect("mount messenger");
    tree.layout(incular_config::Constraints::tight(incular_core::Size::new(
        400.0, 100.0,
    )));

    controller.show_snack_bar(
        SnackBar::text("Saved").action(crate::SnackBarAction::new("Dismiss", || {})),
    );
    tree.layout(incular_config::Constraints::tight(incular_core::Size::new(
        400.0, 100.0,
    )));
    tree.update_semantics();

    assert!(
        tree.semantics()
            .iter()
            .any(|(_, node)| node.label.as_deref() == Some("Dismiss"))
    );
}

#[test]
fn app_uses_named_initial_route_before_home() {
    let app = MaterialApp::new(Text::new("home"))
        .route("/settings", Text::new("settings"))
        .initial_route("/settings")
        .title("sample");
    assert_eq!(app.get_title(), Some("sample"));
    assert_eq!(app.get_theme_mode(), ThemeMode::System);
}
