use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

pub use incular_widgets::{
    CompositedTransformFollower, CompositedTransformTarget, FocusNode, FocusableActionDetector,
    HitTestBehavior, Listener, MouseRegion, OverlayPortal, RawGestureDetector, RawPointerEvent,
    Semantics, SizedBox, TapRegion, Text, Widget,
};

#[path = "../src/raw_tooltip.rs"]
// This harness intentionally includes the complete production module while
// exercising a focused subset of its API. Unused public aliases here are not
// dead production code; they are simply outside this integration test's scope.
#[allow(dead_code)]
mod raw_tooltip;

use raw_tooltip::{
    RawTooltip, RawTooltipController, RawTooltipDurations, RawTooltipVisibility,
    TooltipComponentBuilder, TooltipTriggerMode,
};

fn instant() -> Instant {
    Instant::now()
}

#[test]
fn hover_wait_and_exit_are_clock_driven() {
    let controller = RawTooltipController::with_durations(RawTooltipDurations::new(
        Duration::from_millis(50),
        Duration::from_millis(500),
        Duration::from_millis(25),
    ));
    let start = instant();

    assert!(!controller.mouse_enter_at(7, start));
    assert_eq!(controller.state(), RawTooltipVisibility::Waiting);
    assert!(!controller.tick(start + Duration::from_millis(49)));
    assert!(controller.tick(start + Duration::from_millis(50)));
    assert_eq!(controller.state(), RawTooltipVisibility::Visible);

    assert!(!controller.mouse_exit_at(7, start + Duration::from_millis(50)));
    assert_eq!(controller.state(), RawTooltipVisibility::Dismissing);
    assert!(!controller.tick(start + Duration::from_millis(74)));
    assert!(controller.tick(start + Duration::from_millis(75)));
    assert_eq!(controller.state(), RawTooltipVisibility::Hidden);
}

#[test]
fn touch_trigger_uses_show_window_and_programmatic_calls_are_persistent() {
    let controller = RawTooltipController::with_durations(RawTooltipDurations::new(
        Duration::ZERO,
        Duration::from_millis(80),
        Duration::from_millis(20),
    ));
    let start = instant();

    assert!(controller.trigger_tap_at(start));
    assert!(controller.is_visible());
    assert!(!controller.tick(start + Duration::from_millis(79)));
    assert!(controller.tick(start + Duration::from_millis(80)));
    assert!(!controller.is_visible());

    controller.show_at(start + Duration::from_millis(100));
    assert!(controller.is_visible());
    assert!(!controller.mouse_exit_at(1, start + Duration::from_millis(100)));
    assert!(controller.is_visible());
    controller.hide();
    assert!(!controller.is_visible());
}

#[test]
fn focus_and_dismiss_all_override_pending_hover_transitions() {
    let focused = RawTooltipController::with_durations(RawTooltipDurations::new(
        Duration::from_millis(100),
        Duration::from_millis(500),
        Duration::from_millis(20),
    ));
    let other = RawTooltipController::new();
    let start = instant();

    assert!(!focused.mouse_enter_at(1, start));
    assert_eq!(focused.state(), RawTooltipVisibility::Waiting);
    assert!(focused.focus_changed_at(true, start));
    assert_eq!(focused.state(), RawTooltipVisibility::Visible);
    assert!(!focused.focus_changed_at(false, start));
    assert_eq!(focused.state(), RawTooltipVisibility::Visible);
    assert!(!focused.mouse_exit_at(1, start));
    assert_eq!(focused.state(), RawTooltipVisibility::Dismissing);

    other.show();
    assert!(RawTooltipController::dismiss_all_at(start));
    assert!(!focused.is_visible());
    assert!(!other.is_visible());
}

#[test]
fn trigger_callback_runs_for_programmatic_and_touch_triggers_but_not_hover() {
    let calls = Rc::new(Cell::new(0));
    let callback_calls = calls.clone();
    let controller = RawTooltipController::new();
    controller.set_on_triggered(move || {
        callback_calls.set(callback_calls.get() + 1);
    });
    let start = instant();

    controller.mouse_enter_at(1, start);
    assert_eq!(calls.get(), 0);
    controller.hide();
    controller.show_at(start);
    controller.trigger_long_press_at(start);
    assert_eq!(calls.get(), 2);
}

#[test]
fn descriptor_retains_semantics_and_composited_overlay_hooks() {
    let controller = RawTooltipController::new();
    controller.show();
    let tooltip = RawTooltip::new("Helpful label", Text::new("Trigger"))
        .controller(controller.clone())
        .trigger_mode(TooltipTriggerMode::Manual)
        .vertical_offset(8.0);

    let mut tree = incular_widgets::internal::WidgetTree::new();
    tree.mount(tooltip.into()).expect("tooltip should mount");
    tree.layout(incular_layout::Constraints::tight(
        incular_widgets::Size::new(120.0, 80.0),
    ));
    tree.update_semantics();

    let descriptions = tree
        .semantics()
        .iter()
        .filter_map(|(_, node)| node.description.as_deref())
        .collect::<Vec<_>>();
    assert!(descriptions.contains(&"Helpful label"));
    assert!(
        tree.element_count() > 1,
        "target and overlay should be retained"
    );
}

#[test]
fn rich_and_custom_builder_content_are_supported() {
    let rich = incular_widgets::RichText::new(incular_widgets::TextSpan::new("Rich help"));
    let rich_tooltip = RawTooltip::with_rich_message(rich, SizedBox::shrink());
    assert_eq!(
        rich_tooltip.rich_message().map(|value| value.plain_text()),
        Some("Rich help".to_owned())
    );

    let builder = TooltipComponentBuilder::new(|| Text::new("Custom help").into());
    let custom = RawTooltip::with_builder(SizedBox::shrink(), builder);
    assert!(custom.tooltip_builder().is_some());
}
