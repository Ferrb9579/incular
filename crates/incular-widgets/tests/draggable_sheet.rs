//! Draggable-sheet behavior audit.
//!
//! W3 property/lifecycle contracts only: extent validation, snap
//! filtering, boundary handoff, snap interruption, reset reporting,
//! notifications, activity, and retained replacement/geometry. One
//! controller across unrelated sheets stays a W5 gap.

#![allow(clippy::float_cmp)]

use std::{cell::RefCell, rc::Rc, time::Duration};

use incular_config::Constraints;
use incular_core::{Color, Offset, Size};
use incular_scroll::ScrollController;
use incular_widgets::{
    DraggableScrollableActuator, DraggableScrollableController, DraggableScrollableSheet,
    DraggableSheetExtent, Text, Widget,
    internal::{ElementId, WidgetTree},
};

fn approx(left: f32, right: f32) -> bool {
    (left - right).abs() < 1.0e-4
}

fn text_sheet(label: &str) -> DraggableScrollableSheet<Widget> {
    let label = label.to_owned();
    DraggableScrollableSheet::new(move |_| Text::new(label.clone()).into())
}

#[test]
fn draggable_sheet_builder_defaults_match_explicit_configuration() {
    // Sheet::new defaults equal the explicit spelling: same initial size,
    // expansion, close behavior, and disabled snap.
    let (default_state, _) = text_sheet("v1").mount();
    let (explicit_state, _) = text_sheet("v1")
        .extents(0.25, 1.0, 0.5)
        .expand(true)
        .should_close_on_min_extent(true)
        .mount();
    assert_eq!(default_state.extent(), explicit_state.extent());
    assert!(default_state.snap_target(0.0).is_none());
    assert!(explicit_state.snap_target(0.0).is_none());
    assert!(text_sheet("v1").expands());
}

#[test]
#[should_panic]
fn draggable_sheet_rejects_min_above_max() {
    let _ = text_sheet("x").extents(0.75, 0.25, 0.5);
}

#[test]
#[should_panic]
fn draggable_sheet_rejects_initial_above_max() {
    let _ = text_sheet("x").extents(0.25, 0.75, 1.0);
}

#[test]
#[should_panic]
fn draggable_sheet_rejects_max_above_one() {
    let _ = text_sheet("x").extents(0.25, 1.5, 0.5);
}

#[test]
#[should_panic]
fn draggable_sheet_extent_owner_rejects_negative_min() {
    let _ = DraggableSheetExtent::new(-0.25, 1.0, 0.5, true);
}

#[test]
fn draggable_sheet_snap_filters_sorts_and_dedups() {
    // Out-of-range sizes drop, survivors sort and dedup: the nearest
    // target to 0.6 must be the filtered 0.3, not a rejected value.
    let sheet = text_sheet("v1")
        .extents(0.25, 1.0, 0.5)
        .snap([0.9, 0.3, 0.75, 0.75, 5.0, -1.0], Duration::from_millis(80));
    let (state, _) = sheet.mount();
    assert!(state.set_size(0.4, false));
    let target = state.snap_target(0.0).expect("snap enabled");
    assert!(approx(target.size, 0.3));
    assert_eq!(target.duration, Duration::from_millis(80));
    // Velocity picks the next point in its direction, not the nearest.
    assert!(approx(state.snap_target(1.0).expect("up").size, 0.75));
    assert!(approx(state.snap_target(-1.0).expect("down").size, 0.3));
    // Applying the snap moves the sheet to the resolved target.
    let applied = state.snap_now(0.0).expect("snap applies");
    assert!(approx(applied.size, 0.3));
    assert!(approx(state.extent().current_size, 0.3));
}

#[test]
fn draggable_sheet_boundary_handoff_accounts_every_delta() {
    let (state, _) = text_sheet("v1").extents(0.25, 1.0, 0.5).mount();
    state.set_parent_height(400.0);
    state.set_inner_extents(1_000.0, 200.0);

    // Mid-sheet drag resizes; nothing reaches the list or the parents.
    let drag = state.apply_user_offset(-100.0);
    assert!(approx(state.extent().current_size, 0.75));
    assert!(approx(drag.sheet_consumed, -100.0));
    assert_eq!(drag.inner_consumed, 0.0);
    assert_eq!(drag.parent_consumed, 0.0);
    assert_eq!(drag.unconsumed, 0.0);
    assert_eq!(
        drag.input_delta,
        drag.sheet_consumed + drag.inner_consumed + drag.parent_consumed + drag.unconsumed
    );

    // At the minimum a further opening drag still resizes upward.
    assert!(state.set_size(0.25, false));
    let open = state.apply_user_offset(-100.0);
    assert!(approx(state.extent().current_size, 0.5));
    assert!(approx(open.sheet_consumed, -100.0));

    // Pinned at the maximum, an opening drag cannot resize: the sheet
    // stays put while the inner list takes the delta (here it cannot go
    // below zero, so the delta is reported back as unconsumed).
    assert!(state.set_size(1.0, false));
    let pinned = state.apply_user_offset(-100.0);
    assert_eq!(pinned.sheet_consumed, 0.0);
    assert!(approx(state.extent().current_size, 1.0));
    assert_eq!(state.inner_controller().offset(), 0.0);
    assert!(approx(pinned.unconsumed, -100.0));

    // While the inner list is scrolled, drags scroll it first and leave
    // the sheet untouched.
    assert!(state.set_size(0.5, false));
    assert!(state.inner_controller().jump_to(100.0));
    let listed = state.apply_user_offset(60.0);
    assert_eq!(listed.sheet_consumed, 0.0);
    assert!(approx(listed.inner_consumed, 60.0));
    assert!(approx(state.extent().current_size, 0.5));

    // With the sheet and the inner list both pinned, leftover reaches the
    // parent controllers exactly once.
    assert!(state.set_size(1.0, false));
    assert!(state.inner_controller().jump_to(0.0));
    let parent = ScrollController::new();
    parent.update_extents(600.0, 400.0);
    assert!(parent.jump_to(100.0));
    state.set_parent_controllers([parent.clone()]);
    let nested = state.apply_user_offset(-100.0);
    assert_eq!(nested.sheet_consumed, 0.0);
    assert_eq!(nested.inner_consumed, 0.0);
    assert!(approx(nested.parent_consumed, -100.0));
    assert_eq!(nested.unconsumed, 0.0);
    assert_eq!(parent.offset(), 0.0);
}

#[test]
fn draggable_sheet_inner_physics_swap_takes_effect_on_next_input() {
    use incular_scroll::ScrollPhysics;

    let (state, _) = text_sheet("v1").extents(0.25, 1.0, 0.5).mount();
    state.set_parent_height(400.0);
    state.set_inner_extents(1_000.0, 200.0);
    // Pin the sheet so input routes to the inner list.
    assert!(state.set_size(1.0, false));
    assert!(state.inner_controller().jump_to(100.0));

    let scrolled = state.apply_user_offset(60.0);
    assert!(approx(scrolled.inner_consumed, 60.0));

    state.set_inner_physics(ScrollPhysics::clamping().never_scrollable());
    let refused = state.apply_user_offset(60.0);
    assert_eq!(refused.inner_consumed, 0.0);
    assert!(approx(refused.unconsumed, 60.0));

    state.set_inner_physics(ScrollPhysics::clamping());
    let scrolled_again = state.apply_user_offset(60.0);
    assert!(approx(scrolled_again.inner_consumed, 60.0));
}

#[test]
fn draggable_sheet_snap_animation_interruption_starts_from_current() {
    let sheet = text_sheet("v1").extents(0.25, 1.0, 0.5);
    let controller = sheet.controller();
    let (state, _) = sheet.mount();

    let mut first = controller
        .animate_to(0.9, Duration::from_millis(80))
        .expect("attached animation");
    assert!(approx(first.target(), 0.9));
    assert!(first.tick(Duration::from_millis(40)));
    let mid = state.extent().current_size;
    assert!(mid > 0.5 && mid < 0.9);

    // Interrupting drops the old handle and animates from the current
    // size; the new animation alone completes at its target.
    drop(first);
    let mut second = controller
        .animate_to(0.3, Duration::from_millis(80))
        .expect("replacement animation");
    assert!(!second.tick(Duration::from_millis(80)));
    assert!(!second.tick(Duration::from_millis(80)));
    assert!(approx(state.extent().current_size, 0.3));

    // A zero duration finishes on the first tick at its target.
    let mut instant = controller
        .animate_to(0.6, Duration::ZERO)
        .expect("instant animation");
    assert!(!instant.tick(Duration::ZERO));
    assert!(approx(state.extent().current_size, 0.6));
}

#[test]
fn draggable_sheet_reset_reports_genuine_change() {
    // A pristine sheet reports no change — parent height alone (which only
    // scales fractions to pixels) is not a change signal.
    let (state, _) = text_sheet("v1").extents(0.25, 1.0, 0.5).mount();
    state.set_parent_height(400.0);
    state.set_inner_extents(1_000.0, 200.0);
    assert!(!state.reset());

    // A moved extent resets to initial and reports the change.
    assert!(state.set_size(0.8, true));
    assert!(state.reset());
    assert!(approx(state.extent().current_size, 0.5));
    assert!(!state.extent().has_dragged);
    assert!(!state.extent().has_changed);

    // Inner-only movement still counts: the inner position is restored.
    assert!(state.inner_controller().jump_to(50.0));
    assert!(state.reset());
    assert_eq!(state.inner_controller().offset(), 0.0);

    // A cancelled activity counts as well.
    let _generation = state.start_activity();
    assert!(state.reset());
    assert_eq!(state.activity_generation(), None);

    // The actuator folds per-sheet reports: an untouched sheet keeps it
    // quiet, a moved one does not.
    let actuator = DraggableScrollableActuator::new();
    state.attach_actuator(&actuator);
    assert!(!actuator.reset());
    assert!(state.set_size(0.9, true));
    assert!(actuator.reset());
    assert!(approx(state.extent().current_size, 0.5));
}

#[test]
fn draggable_sheet_reset_notifies_with_committed_state() {
    // A reset listener must observe the final reset state, not the
    // transient setter state: initial size with pristine flags.
    let (state, _) = text_sheet("v1").extents(0.25, 1.0, 0.5).mount();
    assert!(state.set_size(0.8, true));
    let observed = Rc::new(RefCell::new(Vec::new()));
    let observed_for_listener = observed.clone();
    let state_for_listener = state.clone();
    let _subscription = state.add_notification_listener(move |notification| {
        let extent = state_for_listener.extent();
        observed_for_listener.borrow_mut().push((
            notification.extent,
            extent.current_size,
            extent.has_dragged,
            extent.has_changed,
        ));
        false
    });
    assert!(state.reset());
    let seen = observed.borrow();
    assert_eq!(seen.len(), 1);
    assert!(approx(seen[0].0, 0.5));
    assert!(approx(seen[0].1, 0.5));
    assert!(
        !seen[0].2,
        "has_dragged must read cleared inside reset notify"
    );
    assert!(
        !seen[0].3,
        "has_changed must read cleared inside reset notify"
    );
    assert!(!state.extent().has_dragged);
    assert!(!state.extent().has_changed);
}

#[test]
fn draggable_sheet_reset_survives_reentrant_drag() {
    // A listener drag during reset notification wins: the outer reset must
    // not clear flags after callbacks have begun.
    let (state, _) = text_sheet("v1").extents(0.25, 1.0, 0.5).mount();
    assert!(state.set_size(0.8, true));
    let state_for_listener = state.clone();
    let _subscription = state.add_notification_listener(move |_| {
        let _ = state_for_listener.set_size(0.9, true);
        false
    });
    assert!(state.reset());
    assert!(approx(state.extent().current_size, 0.9));
    assert!(state.extent().has_dragged);
    assert!(state.extent().has_changed);
}

#[test]
fn draggable_sheet_reset_reentrant_reset_terminates_pristine() {
    // A nested reset during reset notification sees committed state,
    // reports no change, emits nothing, and leaves the outer reset intact.
    let (state, _) = text_sheet("v1").extents(0.25, 1.0, 0.5).mount();
    assert!(state.set_size(0.8, true));
    let calls = Rc::new(RefCell::new(0));
    let calls_for_listener = calls.clone();
    let state_for_listener = state.clone();
    let _subscription = state.add_notification_listener(move |_| {
        *calls_for_listener.borrow_mut() += 1;
        assert!(!state_for_listener.reset());
        false
    });
    assert!(state.reset());
    assert_eq!(*calls.borrow(), 1);
    assert!(approx(state.extent().current_size, 0.5));
    assert!(!state.extent().has_dragged);
    assert!(!state.extent().has_changed);
}

#[test]
fn draggable_sheet_reset_inner_listeners_observe_committed_sheet() {
    // Ordering contract: the sheet commits silently first, the inner
    // position restores second, the sheet notifies last. Inner-controller
    // listeners therefore observe the already-committed sheet extent.
    let (state, _) = text_sheet("v1").extents(0.25, 1.0, 0.5).mount();
    state.set_inner_extents(1_000.0, 200.0);
    assert!(state.set_size(0.8, true));
    assert!(state.inner_controller().jump_to(50.0));
    let observed = Rc::new(RefCell::new(Vec::new()));
    let observed_for_listener = observed.clone();
    let state_for_listener = state.clone();
    let _subscription = state
        .inner_controller()
        .add_notification_listener(move |_| {
            observed_for_listener
                .borrow_mut()
                .push(state_for_listener.extent().current_size);
            false
        });
    assert!(state.reset());
    let seen = observed.borrow();
    assert_eq!(seen.len(), 1);
    assert!(
        approx(seen[0], 0.5),
        "inner listeners must see the committed sheet, got {}",
        seen[0]
    );
    assert_eq!(state.inner_controller().offset(), 0.0);
}

#[test]
fn draggable_sheet_notifications_carry_full_state_and_unsubscribe() {
    let (state, _) = text_sheet("v1")
        .extents(0.25, 1.0, 0.5)
        .should_close_on_min_extent(false)
        .mount();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let seen_for_listener = seen.clone();
    let subscription = state.add_notification_listener(move |notification| {
        seen_for_listener.borrow_mut().push(notification);
        false
    });
    assert!(state.set_size(0.75, true));
    {
        let notifications = seen.borrow();
        assert_eq!(notifications.len(), 1);
        let notification = notifications[0];
        assert!(approx(notification.min_extent, 0.25));
        assert!(approx(notification.max_extent, 1.0));
        assert!(approx(notification.extent, 0.75));
        assert!(approx(notification.initial_extent, 0.5));
        assert!(!notification.should_close_on_min_extent);
    }

    // Returning true stops propagation to later listeners: the stopper
    // subscribes before the second listener, so the second stays silent.
    let second = Rc::new(RefCell::new(0));
    let second_for_listener = second.clone();
    let _stopper = state.add_notification_listener(|_| true);
    let _second = state.add_notification_listener(move |_| {
        *second_for_listener.borrow_mut() += 1;
        false
    });
    assert!(state.set_size(0.6, true));
    assert_eq!(*second.borrow(), 0);

    // Dropping the subscription silences its listener.
    drop(subscription);
    drop(_stopper);
    assert!(state.set_size(0.9, true));
    assert_eq!(*second.borrow(), 1);
}

#[test]
fn draggable_sheet_activity_lifecycle_and_unmount_detach() {
    let sheet = text_sheet("v1").extents(0.25, 1.0, 0.5);
    let controller = sheet.controller();
    assert!(!controller.is_attached());
    let (state, _) = sheet.mount();
    assert!(controller.is_attached());

    let first = state.start_activity();
    let second = state.start_activity();
    assert_ne!(first, second);
    assert_eq!(state.activity_generation(), Some(second));
    state.cancel_activity();
    assert_eq!(state.activity_generation(), None);

    // Dropping the mounted state detaches its controller, even with an
    // activity in flight.
    let _activity = state.start_activity();
    drop(state);
    assert!(!controller.is_attached());
}

#[test]
fn draggable_sheet_detached_controller_paths_stay_explicit() {
    let controller = DraggableScrollableController::new();
    assert!(!controller.is_attached());
    assert_eq!(controller.size(), None);
    assert_eq!(controller.pixels(), None);
    assert!(!controller.jump_to(0.7));
    assert!(
        controller
            .animate_to(0.7, Duration::from_millis(50))
            .is_none()
    );
}

fn mount_tree(tree: &mut WidgetTree, widget: Widget) -> ElementId {
    let root = tree.mount(widget).expect("mount");
    tree.layout(Constraints::tight(Size::new(200.0, 400.0)))
        .expect("layout");
    root
}

fn wrap(child: Widget) -> Widget {
    incular_widgets::Container::with_child(child).into()
}

#[test]
fn draggable_sheet_retained_replacement_geometry_hit_and_semantics() {
    // Each sheet value owns its controller handle, so every replacement
    // below also swaps controllers: the old handle detaches while the new
    // one inherits the live (clamped) size.
    let sheet = text_sheet("v1").extents(0.25, 1.0, 0.5).expand(true);
    let first = sheet.controller();
    let mut tree = WidgetTree::new();
    let root = mount_tree(&mut tree, wrap(sheet.into()));
    tree.update_semantics();
    assert!(first.is_attached());
    assert!(approx(first.size().expect("attached size"), 0.5));
    assert!(approx(first.pixels().expect("attached pixels"), 200.0));
    let dump = tree.semantics_debug_dump();
    assert!(dump.contains("v1"), ":\n{dump}");
    assert!(
        tree.hit_test(Offset::new(100.0, 300.0)).is_some(),
        "child area hit-tests"
    );

    // Mounted extent replacement clamps the live size into the new range
    // on the incoming controller; the old handle detaches.
    let replacing = text_sheet("v1").extents(0.75, 1.0, 0.75).expand(true);
    let second = replacing.controller();
    tree.update(root, wrap(replacing.into())).expect("update");
    tree.layout(Constraints::tight(Size::new(200.0, 400.0)))
        .expect("layout");
    assert!(!first.is_attached());
    assert!(second.is_attached());
    assert!(approx(second.size().expect("clamped size"), 0.75));
    assert!(!first.jump_to(0.9));
    tree.layout(Constraints::tight(Size::new(200.0, 400.0)))
        .expect("layout");
    assert!(approx(second.size().expect("isolated size"), 0.75));

    // Child replacement publishes the new content and retargets the
    // controller again; resizing keeps the fractional size while pixels
    // and geometry follow the viewport.
    let relabeled = text_sheet("v2").extents(0.75, 1.0, 0.75).expand(true);
    let third = relabeled.controller();
    tree.update(root, wrap(relabeled.into())).expect("update");
    // Controller handoff materializes at layout, like all mounted state.
    tree.layout(Constraints::tight(Size::new(200.0, 200.0)))
        .expect("layout");
    assert!(!second.is_attached());
    assert!(third.is_attached());
    tree.update_semantics();
    let dump = tree.semantics_debug_dump();
    assert!(dump.contains("v2"), ":\n{dump}");
    assert!(!dump.contains("v1"), ":\n{dump}");
    assert!(
        tree.hit_test(Offset::new(100.0, 150.0)).is_some(),
        "resized child area hit-tests"
    );

    // Unmounting through a same-type parent detaches the controller.
    tree.update(
        root,
        wrap(Widget::box_(Size::new(10.0, 10.0), Color::WHITE)),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(200.0, 400.0)))
        .expect("layout");
    assert!(!third.is_attached());
    assert_eq!(third.size(), None);
}

#[test]
fn draggable_sheet_expansion_policy_controls_retained_height() {
    // Loose constraints let the policy decide: expansion fills the
    // viewport while a fitted sheet wraps its fractional child height.
    fn loose_height(widget: Widget) -> f32 {
        let mut tree = WidgetTree::new();
        let root = tree.mount(widget).expect("mount");
        tree.layout(Constraints::new(0.0, 200.0, 0.0, 400.0))
            .expect("layout");
        tree.render_size(tree.render_id(root).expect("sheet render"))
            .expect("sheet size")
            .height
    }
    let expanded_height =
        loose_height(text_sheet("v1").extents(0.25, 1.0, 0.5).expand(true).into());
    assert!(approx(expanded_height, 400.0));
    let fitted_height = loose_height(
        text_sheet("v1")
            .extents(0.25, 1.0, 0.5)
            .expand(false)
            .into(),
    );
    assert!(fitted_height < expanded_height);
    assert!(approx(fitted_height, 200.0));
}
