use super::*;
use incular_controls::current_control_theme;
use incular_core::{Color, Size};
use incular_widgets::{Text, Widget};

#[test]
fn dialog_handle_tracks_open_state_and_result() {
    let handle = show_dialog(Dialog::new(Text::new("hello")));
    assert!(!handle.is_open());
    handle.open();
    assert!(handle.is_open());
    handle.close();
    assert!(!handle.is_open());

    let result = show_dialog_result::<u32>(Dialog::empty());
    result.open();
    result.complete(42);
    assert!(!result.is_open());
    assert_eq!(result.take_result(), Some(42));
    assert_eq!(result.take_result(), None);
}

#[test]
fn progress_indicator_theme_merges_only_unset_fields() {
    let base = ProgressIndicatorThemeData::default()
        .color(Color::WHITE)
        .stroke_width(3.0);
    let fallback = ProgressIndicatorThemeData::default()
        .color(Color::BLACK)
        .linear_track_height(4.0);
    let merged = base.merge(fallback);
    assert_eq!(merged.color, Some(Color::WHITE));
    assert_eq!(merged.stroke_width, Some(3.0));
    assert_eq!(merged.linear_track_height, Some(4.0));
}

#[test]
fn feedback_descriptors_convert_to_widgets() {
    let snackbar: Widget = SnackBar::text("saved")
        .action(SnackBarAction::new("Undo", || {}))
        .into();
    let simple: Widget = SimpleDialog::new()
        .title_text("Choose")
        .option(SimpleDialogOption::text("A").on_pressed(|| {}))
        .into();
    let tooltip: Widget = Tooltip::new("help", Text::new("?"))
        .trigger_mode(TooltipTriggerMode::Manual)
        .into();
    let _ = (snackbar, simple, tooltip);
}

#[test]
fn snackbar_action_is_semantically_exposed() {
    let snackbar = SnackBar::text("Saved")
        .action(SnackBarAction::new("Dismiss", || {}))
        .build(&current_control_theme());
    let mut tree = incular_widgets::internal::WidgetTree::new();
    tree.mount(snackbar).expect("mount snackbar");
    tree.layout(incular_config::Constraints::tight(Size::new(400.0, 100.0)));
    tree.update_semantics();

    assert!(
        tree.semantics()
            .iter()
            .any(|(_, node)| node.label.as_deref() == Some("Dismiss"))
    );
}
