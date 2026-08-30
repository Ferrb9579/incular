use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use incular_core::{RestorationKey, RestorationScope, Size};
use serde_json::{Value, json};

use super::*;

#[derive(Default)]
struct MemoryRestorationBackend(RefCell<BTreeMap<Vec<RestorationKey>, Value>>);

impl incular_core::RestorationBackend for MemoryRestorationBackend {
    fn read_value(&self, path: &[RestorationKey]) -> Option<Value> {
        self.0.borrow().get(path).cloned()
    }

    fn write_value(&self, path: &[RestorationKey], value: Value) {
        self.0.borrow_mut().insert(path.to_vec(), value);
    }

    fn remove_value(&self, path: &[RestorationKey]) {
        self.0.borrow_mut().remove(path);
    }
}

fn restoration_key(value: &str) -> RestorationKey {
    RestorationKey::new(value).unwrap()
}

fn restoration_scope() -> RestorationScope {
    RestorationScope::root(Rc::new(MemoryRestorationBackend::default()))
        .child_unchecked(restoration_key("window"))
        .child_unchecked(restoration_key("main"))
}

#[test]
fn extents_clamp_an_existing_offset() {
    let controller = ScrollController::new();
    controller.update_extents(100., 20.);
    assert!(controller.jump_to(80.));
    controller.update_extents(40., 20.);
    assert_eq!(controller.offset(), 20.);
}

#[test]
fn scroll_notifications_follow_activity_lifecycle_and_unsubscribe() {
    let controller = ScrollController::new();
    controller.set_metrics_context(incular_config::Axis::Horizontal, true);
    let events = Rc::new(RefCell::new(Vec::new()));
    let observed = events.clone();
    let subscription = controller.add_listener(move |notification| {
        observed.borrow_mut().push(notification);
        false
    });

    controller.update_extents(500., 100.);
    assert!(controller.begin_activity());
    assert!(!controller.begin_activity());
    assert!(controller.notify_user_scroll(12.));
    assert!(
        controller
            .apply_physics(ScrollPhysics::clamping(), 12.)
            .accepted
    );
    assert!(controller.end_activity());
    assert!(!controller.end_activity());

    let observed_events = events.borrow();
    assert_eq!(
        observed_events
            .iter()
            .map(|event| event.kind)
            .collect::<Vec<_>>(),
        vec![
            ScrollNotificationType::Metrics,
            ScrollNotificationType::Start,
            ScrollNotificationType::UserScroll,
            ScrollNotificationType::Update,
            ScrollNotificationType::End,
        ]
    );
    assert_eq!(
        observed_events[0].metrics.axis,
        incular_config::Axis::Horizontal
    );
    assert_eq!(
        observed_events[0].metrics.axis_direction,
        incular_config::AxisDirection::Left
    );
    drop(observed_events);
    drop(subscription);
    assert!(controller.jump_to(20.));
    assert_eq!(events.borrow().len(), 5);
}

#[test]
fn scrollbar_round_trips_offset() {
    let controller = ScrollController::new();
    controller.update_extents(200., 100.);
    controller.jump_to(50.);
    let geometry = scrollbar_geometry(
        Size::new(100., 100.),
        &controller,
        ScrollbarStyle::default(),
    );
    assert!(geometry.visible);
    assert!((geometry.offset_for_thumb_top(geometry.thumb.origin.y) - 50.).abs() < 0.001);
}

#[test]
fn restored_offsets_wait_for_layout_and_survive_temporary_short_content() {
    let scope = restoration_scope();
    let key = restoration_key("sidebar");
    scope.set_json(&key, json!({ "offset": 80. }));

    let controller = ScrollController::restored(scope.clone(), key.clone());
    assert_eq!(controller.offset(), 0.);

    controller.update_extents(40., 20.);
    assert_eq!(controller.offset(), 20.);
    assert_eq!(scope.get_json(&key), Some(json!({ "offset": 80. })));

    controller.update_extents(120., 20.);
    assert_eq!(controller.offset(), 80.);
    assert!(controller.jump_to(50.));
    assert_eq!(scope.get_json(&key), Some(json!({ "offset": 50. })));
}

#[test]
fn measured_extent_index_seeks_a_million_unmeasured_rows_without_prefix_work() {
    let index = MeasuredExtentIndex::new(1_000_000, 40.);
    assert_eq!(index.index_at_offset(36_000_012.), Some(900_000));
    assert_eq!(index.offset_for_index(900_000), 36_000_000.);
    assert_eq!(
        index.materialized_range(36_000_000., 600., 240.),
        899_994..900_022
    );
    // Only rows directly corrected by layout become measured; seeking did
    // not manufacture a prefix of 900k measurements.
    assert_eq!(index.measured_count(), 0);
}

#[test]
fn measured_extent_index_updates_prefix_and_preserves_other_measurements() {
    let index = MeasuredExtentIndex::new(8, 10.);
    assert!(index.set_measured_extent(2, 25.));
    assert_eq!(index.offset_for_index(3), 45.);
    assert_eq!(index.index_at_offset(44.9), Some(2));
    assert_eq!(index.index_at_offset(45.), Some(3));
    assert!(index.invalidate_extent(2));
    assert_eq!(index.offset_for_index(3), 30.);
}

#[test]
fn measured_extent_index_structural_mutations_retain_and_invalidate_mapping() {
    let index = MeasuredExtentIndex::new(4, 10.);
    index.set_measured_extent(1, 30.);
    index.insert(1, 2);
    assert_eq!(index.len(), 6);
    assert_eq!(index.offset_for_index(4), 60.);
    assert!(index.move_item(3, 0));
    assert_eq!(index.offset_for_index(1), 30.);
    index.remove(0..2);
    assert_eq!(index.len(), 4);
    assert_eq!(index.measured_count(), 0);
    assert!(index.structure_revision() >= 3);
}

#[test]
fn clamping_and_scrollability_policies_return_precise_unused_delta() {
    let clamped = ScrollPhysics::clamping().apply_delta(95., 20., 0., 100.);
    assert_eq!(clamped.position, 100.);
    assert_eq!(clamped.consumed, 5.);
    assert_eq!(clamped.unconsumed, 15.);

    let never = ScrollPhysics::clamping()
        .never_scrollable()
        .apply_delta(0., 10., 0., 100.);
    assert!(!never.accepted);
    assert_eq!(never.unconsumed, 10.);

    let always = ScrollPhysics::clamping()
        .always_scrollable()
        .apply_delta(0., 10., 0., 0.);
    assert!(always.accepted);
    assert_eq!(always.unconsumed, 10.);
}

#[test]
fn always_scrollable_accepts_small_content() {
    let controller = ScrollController::new();
    controller.update_extents(40., 100.);
    let result = controller.apply_physics(ScrollPhysics::clamping().always_scrollable(), 12.);
    assert!(result.accepted);
    assert_eq!(result.position, 0.);
    assert_eq!(result.unconsumed, 12.);
}

#[test]
fn never_scrollable_rejects_user_drag_but_controller_jump_still_works() {
    let controller = ScrollController::new();
    controller.update_extents(300., 100.);
    let result = controller.apply_physics(ScrollPhysics::clamping().never_scrollable(), 40.);
    assert!(!result.accepted);
    assert_eq!(controller.offset(), 0.);
    assert!(controller.jump_to(80.));
    assert_eq!(controller.offset(), 80.);
}

#[test]
fn bouncing_is_resistant_bounded_and_returns_with_monotonic_spring_steps() {
    let physics = ScrollPhysics::clamping().bouncing();
    let bounced = physics.apply_delta(0., -1_000., 0., 100.);
    assert!(bounced.position < 0.);
    assert!(bounced.position >= -160.);
    assert_eq!(bounced.unconsumed, 0.);
    let mut position = bounced.position;
    let mut velocity = 0.;
    for _ in 0..240 {
        let step = physics.spring_step(position, velocity, 1. / 120., 0., 100.);
        position = step.position;
        velocity = step.velocity;
        if step.settled {
            break;
        }
    }
    assert!((position - 0.).abs() < 0.01);
    let controller = ScrollController::new();
    controller.update_extents(200., 100.);
    let result = controller.apply_physics(physics, -20.);
    assert!(result.position < 0.);
    assert!(controller.offset() < 0.);
}

#[test]
fn page_and_fixed_extent_snap_targets_are_velocity_deterministic() {
    let page = ScrollPhysics::clamping().page_snapping(100.);
    assert_eq!(page.snap_target(149., 0., 0., 500.), 100.);
    assert_eq!(page.snap_target(149., 500., 0., 500.), 200.);
    let fixed = ScrollPhysics::clamping().fixed_extent_snapping(32.);
    assert_eq!(fixed.snap_target(70., -500., 0., 320.), 64.);
}

#[test]
fn page_physics_targets_the_current_viewport_page() {
    let page = ScrollPhysics::clamping().page();
    assert_eq!(page.snap_target_for_extent(149., 0., 0., 500., 100.), 100.);
    assert_eq!(
        page.snap_target_for_extent(149., 500., 0., 500., 100.),
        200.
    );
    assert_eq!(page.snap_extent(100.), Some(100.));
}

#[test]
fn range_maintaining_preserves_trailing_edge_when_extents_change() {
    let controller = ScrollController::new();
    let physics = ScrollPhysics::clamping().range_maintaining();
    controller.update_extents_with_physics(300., 100., physics);
    assert!(controller.jump_to(200.));

    // Growing content keeps the visible end anchored.
    controller.update_extents_with_physics(420., 100., physics);
    assert_eq!(controller.max_offset(), 320.);
    assert_eq!(controller.offset(), 320.);

    // A viewport resize that shrinks the range keeps the same edge
    // anchored and never leaves the valid range.
    controller.update_extents_with_physics(420., 180., physics);
    assert_eq!(controller.max_offset(), 240.);
    assert_eq!(controller.offset(), 240.);
}

#[test]
fn range_maintaining_adjusts_anchor_for_insertions_before_viewport() {
    let controller = ScrollController::new();
    let physics = ScrollPhysics::clamping().range_maintaining();
    controller.update_extents_with_physics(1_000., 100., physics);
    controller.jump_to(400.);
    assert!(controller.adjust_for_content_change(48., physics));
    assert_eq!(controller.offset(), 448.);
    assert!(controller.adjust_for_content_change(-24., physics));
    assert_eq!(controller.offset(), 424.);
}

#[test]
fn physics_composition_order_is_deterministic() {
    let first = ScrollPhysics::clamping()
        .bouncing()
        .then(ScrollPhysics::clamping().always_scrollable())
        .range_maintaining();
    let second = ScrollPhysics::clamping()
        .range_maintaining()
        .then(ScrollPhysics::clamping().bouncing().always_scrollable());
    assert_eq!(first.scrollability, Scrollability::Always);
    assert_eq!(first.boundary, second.boundary);
    assert_eq!(first.scrollability, second.scrollability);
    assert!(first.is_range_maintaining());
    assert!(second.is_range_maintaining());
}

#[test]
fn controller_settle_returns_bounce_or_page_positions_to_stable_targets() {
    let controller = ScrollController::new();
    controller.update_extents(300., 100.);
    let bounce = ScrollPhysics::clamping().bouncing();
    assert!(controller.apply_physics(bounce, -40.).position < 0.);
    assert!(controller.settle_physics(bounce, 0.));
    assert_eq!(controller.offset(), 0.);

    assert!(controller.jump_to(149.));
    let page = ScrollPhysics::clamping().page_snapping(100.);
    assert!(controller.settle_physics(page, 0.));
    assert_eq!(controller.offset(), 100.);
}

#[test]
fn nested_coordinator_transfers_only_unconsumed_delta_to_the_outer_viewport() {
    let inner = ScrollController::new();
    let outer = ScrollController::new();
    inner.update_extents(300., 100.);
    outer.update_extents(1_000., 100.);
    inner.jump_to(0.);
    outer.jump_to(200.);
    let mut coordinator = NestedScrollCoordinator::new([inner.clone(), outer.clone()]);
    let result = coordinator.apply_drag_delta(-50.);
    assert_eq!(inner.offset(), 0.);
    assert_eq!(outer.offset(), 150.);
    assert_eq!(result.consumed, -50.);
    assert_eq!(result.unconsumed, 0.);
    assert_eq!(coordinator.active_index(), Some(1));

    let result = coordinator.apply_momentum_delta(250.);
    assert_eq!(inner.offset(), 200.);
    assert_eq!(outer.offset(), 200.);
    assert_eq!(result.unconsumed, 0.);
    assert_eq!(coordinator.active_index(), Some(1));
    coordinator.cancel();
    assert_eq!(coordinator.active_index(), None);
}
