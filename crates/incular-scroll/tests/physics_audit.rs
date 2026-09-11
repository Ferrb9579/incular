//! Scroll-physics audit: boundary conservation, range shapes, snap
//! thresholds, builder consistency, controller paths, and coordination.
//! Expected values below are derived by hand from the documented rules,
//! never by re-running the implementation under test.

use std::{cell::RefCell, rc::Rc};

use incular_scroll::{
    BoundaryPhysics, ClampingScrollPhysics, NestedScrollCoordinator, ScrollController,
    ScrollNotificationType, ScrollPhysics, Scrollability, SnapPhysics,
};

fn approx(left: f32, right: f32) -> bool {
    (left - right).abs() < 1.0e-3
}

#[test]
fn clamping_conserves_delta_across_positions_and_ranges() {
    // Hand-derived: position clamps, consumed is the clamped travel, and
    // the two parts always sum back to the input delta.
    for (current, delta, min, max) in [
        (50., 20., 0., 100.),
        (95., 20., 0., 100.),
        (5., -20., 0., 100.),
        (0., -7., 0., 100.),
        (100., 7., 0., 100.),
        (150., -30., 0., 100.),
        (-50., 30., 0., 100.),
        (0., 0., 0., 100.),
        (30., 500., 0., 40.),
    ] {
        let result = ScrollPhysics::clamping().apply_delta(current, delta, min, max);
        let lo = min.min(max);
        let hi = max.max(min);
        assert!(
            result.position >= lo - 1.0e-4 && result.position <= hi + 1.0e-4,
            "position leaves range for ({current}, {delta})"
        );
        assert!(
            approx(result.consumed + result.unconsumed, delta),
            "delta not conserved for ({current}, {delta})"
        );
        assert_eq!(result.consumed != 0., result.accepted);
    }
    assert_eq!(
        ScrollPhysics::clamping().apply_delta(95., 20., 0., 100.),
        incular_scroll::ScrollDelta {
            position: 100.,
            consumed: 5.,
            unconsumed: 15.,
            overscroll: 0.,
            accepted: true,
        }
    );
}

#[test]
fn empty_reversed_and_outside_ranges() {
    // Empty range under WhenScrollable: clamped in place, all unconsumed.
    let empty = ScrollPhysics::clamping().apply_delta(50., 25., 40., 40.);
    assert_eq!(empty.position, 40.);
    assert_eq!(empty.unconsumed, 25.);
    assert!(!empty.accepted);

    // Reversed bounds normalize to the same range.
    let forward = ScrollPhysics::clamping().apply_delta(50., 30., 0., 100.);
    let reversed = ScrollPhysics::clamping().apply_delta(50., 30., 100., 0.);
    assert_eq!(forward, reversed);

    // Starting outside pulls back to the bound, consuming the difference.
    let outside = ScrollPhysics::clamping().apply_delta(150., -30., 0., 100.);
    assert_eq!(outside.position, 100.);
    assert_eq!(outside.consumed, -50.);
    assert_eq!(outside.unconsumed, 20.);
}

#[test]
fn nonfinite_input_is_rejected_without_movement() {
    let nan_delta = ScrollPhysics::clamping().apply_delta(50., f32::NAN, 0., 100.);
    assert_eq!(nan_delta.position, 50.);
    assert_eq!(nan_delta.consumed, 0.);
    assert!(nan_delta.unconsumed.is_nan());

    let nan_current = ScrollPhysics::clamping().apply_delta(f32::NAN, 10., 0., 100.);
    assert!(nan_current.position.is_nan());
    assert_eq!(nan_current.unconsumed, 10.);

    let inf_delta = ScrollPhysics::clamping().apply_delta(50., f32::INFINITY, 0., 100.);
    assert_eq!(inf_delta.position, 50.);
    assert_eq!(inf_delta.unconsumed, f32::INFINITY);

    // The low-level helper passes NaN through without panicking and
    // normalizes reversed bounds itself.
    assert!(
        ClampingScrollPhysics
            .apply(50., f32::NAN, 0., 100.)
            .is_nan()
    );
    assert_eq!(ClampingScrollPhysics.apply(50., 10., 100., 0.), 60.);
}

#[test]
fn bouncing_owns_its_sequence_with_hand_derived_values() {
    // Defaults: resistance 0.5, limit 160. raw = -1000 pulls to
    // 0 + (-1000)(0.5) = -500, floored at -160.
    let physics = ScrollPhysics::clamping().bouncing();
    let bounced = physics.apply_delta(0., -1_000., 0., 100.);
    assert_eq!(bounced.position, -160.);
    assert_eq!(bounced.consumed, -160.);
    assert_eq!(bounced.unconsumed, 0.);
    assert_eq!(bounced.overscroll, -160.);
    assert!(bounced.accepted);

    // Above the top: 100 + (30)(0.5) = 115, overscroll 15.
    let top = physics.apply_delta(100., 30., 0., 100.);
    assert_eq!(top.position, 115.);
    assert_eq!(top.overscroll, 15.);
    assert_eq!(top.unconsumed, 0.);

    // In range behaves like clamping with no overscroll.
    let mid = physics.apply_delta(50., 10., 0., 100.);
    assert_eq!(mid.position, 60.);
    assert_eq!(mid.overscroll, 0.);

    // Monotonic spring steps return exactly to the bound and report it.
    let mut position = bounced.position;
    let mut velocity = 0.;
    let mut settled = false;
    for _ in 0..600 {
        let step = physics.spring_step(position, velocity, 1. / 120., 0., 100.);
        position = step.position;
        velocity = step.velocity;
        if step.settled {
            settled = true;
            break;
        }
    }
    assert!(settled);
    assert_eq!(position, 0.);
    assert_eq!(velocity, 0.);

    // Non-bouncing policies settle trivially to the clamped position.
    let plain = ScrollPhysics::clamping().spring_step(150., 999., 0.016, 0., 100.);
    assert_eq!(plain.position, 100.);
    assert_eq!(plain.velocity, 0.);
    assert!(plain.settled);
}

#[test]
fn snap_velocity_thresholds_and_exact_multiples_are_pinned() {
    let page = ScrollPhysics::clamping().page_snapping(20.);
    // Off-multiple: fling strength rounds directionally, slow rounds near.
    assert_eq!(page.snap_target(105., 121., 0., 500.), 120.);
    assert_eq!(page.snap_target(105., 120., 0., 500.), 100.);
    assert_eq!(page.snap_target(95., -121., 0., 500.), 80.);
    assert_eq!(page.snap_target(95., -120., 0., 500.), 100.);
    // Exact multiples stay at any velocity.
    for velocity in [-500., -121., -120., 0., 100., 120., 121., 500.] {
        assert_eq!(page.snap_target(100., velocity, 0., 500.), 100.);
    }
    // Targets clamp to the range.
    assert_eq!(page.snap_target(495., 500., 0., 500.), 500.);
    assert_eq!(page.snap_target(5., -500., 0., 500.), 0.);
    // Degenerate extents fall back to a plain clamp.
    for bad in [0., -20., f32::NAN] {
        let policy = ScrollPhysics::clamping().page_snapping(bad);
        assert_eq!(policy.snap_target(149., 500., 0., 500.), 149.);
        assert_eq!(policy.snap_extent(100.), None);
    }
    assert_eq!(
        ScrollPhysics::clamping()
            .page_snapping(100.)
            .snap_extent(100.),
        Some(100.)
    );
    assert_eq!(ScrollPhysics::clamping().snap_extent(100.), None);
    assert_eq!(ScrollPhysics::clamping().page().snap_extent(75.), Some(75.));
    // Viewport pages resolve against the live viewport extent.
    assert_eq!(
        ScrollPhysics::clamping()
            .page()
            .snap_target_for_extent(149., 500., 0., 500., 100.),
        ScrollPhysics::clamping()
            .page_snapping(100.)
            .snap_target(149., 500., 0., 500.)
    );
}

#[test]
fn builders_are_consistent_with_constructors_and_defaults() {
    assert_eq!(ScrollPhysics::clamping(), ScrollPhysics::default());
    assert_eq!(
        ScrollPhysics::clamping().bouncing().boundary,
        BoundaryPhysics::Bouncing {
            resistance: 0.5,
            max_overscroll: 160.,
            spring: 220.,
            damping: 28.,
        }
    );
    let carousel = ScrollPhysics::clamping().carousel(32.);
    let fixed = ScrollPhysics::clamping().fixed_extent_snapping(32.);
    assert_eq!(carousel.snap, fixed.snap);
    assert_eq!(
        carousel.snap_target(70., -500., 0., 320.),
        fixed.snap_target(70., -500., 0., 320.)
    );
    assert_eq!(
        ScrollPhysics::clamping().page().snap,
        SnapPhysics::PageViewport
    );
    // Composition: explicit child choices win, defaults inherit, range
    // maintenance ORs, and operand order does not matter.
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
    assert_eq!(first.snap, second.snap);
    assert!(first.is_range_maintaining());
    assert!(second.is_range_maintaining());
    assert!(!ScrollPhysics::clamping().is_range_maintaining());
    let parented = ScrollPhysics::clamping().parent(ScrollPhysics::clamping().bouncing());
    assert_eq!(
        parented.boundary,
        BoundaryPhysics::Bouncing {
            resistance: 0.5,
            max_overscroll: 160.,
            spring: 220.,
            damping: 28.,
        }
    );
    // Threshold constants are locked API.
    assert_eq!(ScrollPhysics::clamping().min_fling_velocity(), 50.0);
    assert_eq!(ScrollPhysics::clamping().max_fling_velocity(), 8000.0);
    assert_eq!(
        ScrollPhysics::clamping().drag_start_distance_motion_threshold(),
        3.5
    );
}

#[test]
fn controller_distinguishes_input_from_programmatic_movement() {
    let controller = ScrollController::new();
    controller.update_extents(300., 100.);
    let kinds = Rc::new(RefCell::new(Vec::new()));
    let kinds_for_listener = kinds.clone();
    let _subscription = controller.add_listener(move |notification| {
        kinds_for_listener.borrow_mut().push(notification.kind);
        false
    });

    // User input path persists and reports Update.
    let input = controller.apply_physics(ScrollPhysics::clamping(), 40.);
    assert!(input.accepted);
    assert_eq!(controller.offset(), 40.);
    // Programmatic path persists and reports Update too; both move state,
    // only policy-gated input can refuse.
    assert!(controller.jump_to(80.));
    assert!(controller.scroll_by(-30.));
    assert_eq!(controller.offset(), 50.);
    assert_eq!(
        *kinds.borrow(),
        vec![
            ScrollNotificationType::Update,
            ScrollNotificationType::Update,
            ScrollNotificationType::Update,
        ]
    );

    // Policy swap on the same controller takes effect on the next input:
    // Never refuses while jump_to still programs movement (W5 owns
    // attachment policy, not per-call policy selection).
    let refused = controller.apply_physics(ScrollPhysics::clamping().never_scrollable(), 40.);
    assert!(!refused.accepted);
    assert_eq!(controller.offset(), 50.);
    assert!(controller.jump_to(90.));
    let moved = controller.apply_physics(ScrollPhysics::clamping(), 40.);
    assert!(moved.accepted);
    assert_eq!(controller.offset(), 130.);
}

#[test]
fn extents_sanitize_ranges_and_apply_pending_jumps() {
    let controller = ScrollController::new();
    // Negative content/viewport collapse to an empty range at zero.
    controller.update_extents(-50., -10.);
    assert_eq!(controller.max_offset(), 0.);
    assert_eq!(controller.offset(), 0.);

    // A pre-layout jump defers instead of clamping away, then applies
    // once the range exists.
    let controller = ScrollController::new();
    assert!(controller.deferred_jump_to(150.));
    controller.update_extents(300., 100.);
    assert_eq!(controller.offset(), 150.);

    // Shrinking the range clamps a live offset back inside.
    assert!(controller.jump_to(200.));
    controller.update_extents(120., 100.);
    assert_eq!(controller.max_offset(), 20.);
    assert_eq!(controller.offset(), 20.);
}

#[test]
fn settle_and_anchor_adjustment_follow_their_gates() {
    let controller = ScrollController::new();
    controller.update_extents(300., 100.);
    // Viewport-relative settle uses the live viewport extent.
    assert!(controller.jump_to(149.));
    assert!(controller.settle_physics(ScrollPhysics::clamping().page(), 0.));
    assert_eq!(controller.offset(), 100.);

    // Anchor adjustment requires a range-maintaining policy and a finite
    // delta; otherwise it reports no change.
    let plain = ScrollPhysics::clamping();
    let maintaining = ScrollPhysics::clamping().range_maintaining();
    assert!(!controller.adjust_for_content_change(48., plain));
    assert!(!controller.adjust_for_content_change(f32::NAN, maintaining));
    assert!(controller.adjust_for_content_change(48., maintaining));
    assert_eq!(controller.offset(), 148.);
    assert!(controller.adjust_for_content_change(-24., maintaining));
    assert_eq!(controller.offset(), 124.);
}

#[test]
fn coordinator_spans_ranges_innermost_first_with_conservation() {
    let inner = ScrollController::new();
    let mid = ScrollController::new();
    let outer = ScrollController::new();
    inner.update_extents(120., 100.);
    mid.update_extents(300., 100.);
    outer.update_extents(1_000., 100.);
    let mut coordinator = NestedScrollCoordinator::new([inner.clone(), mid.clone(), outer.clone()]);
    // 20 + 200 + remainder: inner takes 20, mid takes 200, outer the rest.
    let result = coordinator.apply_drag_delta(400.);
    assert_eq!(inner.offset(), 20.);
    assert_eq!(mid.offset(), 200.);
    assert_eq!(outer.offset(), 180.);
    assert!(approx(result.consumed + result.unconsumed, 400.));
    assert_eq!(result.unconsumed, 0.);
    assert_eq!(coordinator.active_index(), Some(2));

    // Reversing hands back down the same chain without duplication.
    let result = coordinator.apply_drag_delta(-400.);
    assert_eq!(inner.offset(), 0.);
    assert_eq!(mid.offset(), 0.);
    assert_eq!(outer.offset(), 0.);
    assert_eq!(result.unconsumed, 0.);
    coordinator.cancel();
    assert_eq!(coordinator.active_index(), None);
}
