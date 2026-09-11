//! Fixed-extent wheel scrolling audit.
//!
//! W3 property/lifecycle contracts only: selection reporting, delegate
//! paths, bounds, matrix/projection math, rendering policy, the
//! fixed-extent controller, and retained-tree window refresh. Activity
//! and offsets stay owned by [`ScrollController`](incular_scroll::ScrollController);
//! one controller across unrelated viewports stays a W5 gap.

#![allow(clippy::float_cmp)]

use std::{cell::RefCell, rc::Rc};

use incular_config::{Clip, Constraints};
use incular_core::{Offset, Size};
use incular_scroll::{ScrollController, ScrollPhysics};
use incular_widgets::{
    ChangeReportingBehavior, FixedExtentScrollController, ListWheelScrollView, ListWheelViewport,
    Text, WheelChildDelegate, WheelMatrix, WheelProjection, Widget, internal::WidgetTree,
};

fn sized_children(count: usize) -> Vec<Widget> {
    (0..count)
        .map(|index| Text::new(format!("item{index}")).into())
        .collect()
}

fn numbered(count: usize) -> Vec<u32> {
    (0..count as u32).collect()
}

#[test]
fn wheel_reports_selection_on_update_but_defers_on_end() {
    let mut viewport = ListWheelViewport::new(
        ScrollController::new(),
        20.0,
        WheelChildDelegate::children(numbered(12)),
    );
    let reported = Rc::new(RefCell::new(Vec::new()));
    let reported_for_callback = reported.clone();
    viewport.set_selection_callback(
        ChangeReportingBehavior::OnScrollUpdate,
        Some(move |index| reported_for_callback.borrow_mut().push(index)),
    );
    let first = viewport.layout(Size::new(100.0, 100.0));
    assert_eq!(first.selected_index, Some(0));
    assert_eq!(*reported.borrow(), vec![0]);

    // An unchanged layout reports nothing new.
    let _ = viewport.layout(Size::new(100.0, 100.0));
    assert_eq!(*reported.borrow(), vec![0]);

    assert!(viewport.jump_to_item(5));
    let _ = viewport.layout(Size::new(100.0, 100.0));
    assert_eq!(*reported.borrow(), vec![0, 5]);

    // End-of-scroll mode stays silent through scrolls and layouts and
    // reports exactly once when the viewport settles.
    let mut settling = ListWheelViewport::new(
        ScrollController::new(),
        20.0,
        WheelChildDelegate::children(numbered(12)),
    );
    let reported = Rc::new(RefCell::new(Vec::new()));
    let reported_for_callback = reported.clone();
    settling.set_selection_callback(
        ChangeReportingBehavior::OnScrollEnd,
        Some(move |index| reported_for_callback.borrow_mut().push(index)),
    );
    assert!(settling.jump_to_item(5));
    let _ = settling.layout(Size::new(100.0, 100.0));
    assert!(reported.borrow().is_empty());
    assert_eq!(settling.settle(), Some(5));
    assert_eq!(*reported.borrow(), vec![5]);
}

#[test]
fn wheel_delegate_builder_known_unknown_and_empty() {
    // A known-count builder behaves like the eager list.
    let known_delegate =
        WheelChildDelegate::builder(Some(12), |index| (index < 12).then_some(index));
    assert_eq!(known_delegate.child_count(), Some(12));
    let mut known = ListWheelViewport::new(ScrollController::new(), 20.0, known_delegate);
    let layout = known.layout(Size::new(100.0, 100.0));
    assert_eq!(layout.selected_index, Some(0));
    assert_eq!(known.selected_item(), Some(0));
    assert!(!layout.children.is_empty());

    // A builder that returns None terminates the queried range there.
    let mut short = ListWheelViewport::new(
        ScrollController::new(),
        20.0,
        WheelChildDelegate::builder(Some(12), |index| (index < 3).then_some(index)),
    );
    let layout = short.layout(Size::new(100.0, 100.0));
    assert!(layout.children.iter().all(|child| child.index < 3));

    // An unknown tail reports a rounded selection with a huge range and
    // still materializes the visible window.
    let lazy_delegate = WheelChildDelegate::builder(None, |index| (index < 30).then_some(index));
    assert_eq!(lazy_delegate.child_count(), None);
    let mut lazy = ListWheelViewport::new(ScrollController::new(), 20.0, lazy_delegate);
    assert!(lazy.jump_to_item(5));
    let layout = lazy.layout(Size::new(100.0, 100.0));
    assert_eq!(layout.selected_index, Some(5));
    assert_eq!(lazy.selected_item(), Some(5));
    assert!(layout.max_scroll_extent > 1.0e30);

    // An empty delegate selects nothing, lays out nothing, and settles to
    // nothing; jumping anywhere is still accepted but stays clamped.
    let mut empty = ListWheelViewport::new(
        ScrollController::new(),
        20.0,
        WheelChildDelegate::children(Vec::<u32>::new()),
    );
    let layout = empty.layout(Size::new(100.0, 100.0));
    assert_eq!(layout.selected_index, None);
    assert_eq!(empty.selected_item(), None);
    assert!(layout.children.is_empty());
    assert_eq!(layout.max_scroll_extent, 0.0);
    assert_eq!(empty.settle(), None);
}

#[test]
fn wheel_bounds_clamp_and_settle_keeps_logical_and_visual_agreement() {
    let controller = ScrollController::new();
    let mut viewport = ListWheelViewport::new(
        controller.clone(),
        20.0,
        WheelChildDelegate::children(numbered(12)),
    );
    let _ = viewport.layout(Size::new(100.0, 100.0));
    assert_eq!(controller.max_offset(), 220.0);

    // Jumping past the final item clamps to it; there is no looping
    // delegate, so bounds always win.
    assert!(viewport.jump_to_item(50));
    assert_eq!(controller.offset(), 220.0);
    assert_eq!(viewport.selected_item(), Some(11));

    // Scrolling past either bound consumes nothing and moves nowhere.
    let end = viewport.apply_delta(5_000.0);
    assert_eq!(controller.offset(), 220.0);
    assert!(end.unconsumed.abs() > 0.0);
    assert!(controller.jump_to(0.0));
    let start = viewport.apply_delta(-5_000.0);
    assert_eq!(controller.offset(), 0.0);
    assert!(start.unconsumed.abs() > 0.0);

    // After settling from a mid-item offset, the rounded selection, the
    // centered child, and center hit testing all name the same item.
    for target in [1, 4, 9, 11] {
        assert!(controller.jump_to(target as f32 * 20.0 + 5.0));
        assert_eq!(viewport.settle(), Some(target));
        assert_eq!(controller.offset(), target as f32 * 20.0);
        let layout = viewport.layout(Size::new(100.0, 100.0));
        assert_eq!(layout.selected_index, Some(target));
        let center = layout
            .children
            .iter()
            .find(|child| child.in_center)
            .expect("one centered child");
        assert_eq!(center.index, target);
        assert!(center.angle.abs() < 1.0e-4);
        assert_eq!(layout.hit_test(Offset::new(50.0, 50.0)), Some(target));
    }
}

#[test]
fn wheel_matrix_and_projection_math() {
    let translation = WheelMatrix::translation(3.0, -2.0, 5.0);
    assert_eq!(WheelMatrix::identity().multiply(translation), translation);
    assert_eq!(translation.multiply(WheelMatrix::identity()), translation);

    let inverse = translation.inverse().expect("translation inverts");
    let roundtrip = translation.multiply(inverse);
    for (index, value) in WheelMatrix::identity().values.iter().enumerate() {
        assert!((roundtrip.values[index] - value).abs() < 1.0e-5);
    }

    let projected = translation
        .project_point(Offset::new(1.0, 2.0))
        .expect("finite projection");
    assert!((projected.point.x - 4.0).abs() < 1.0e-5);
    assert!((projected.point.y - 0.0).abs() < 1.0e-5);
    let unprojected = translation
        .inverse_transform_point(projected.point)
        .expect("invertible hit path");
    assert!((unprojected.x - 1.0).abs() < 1.0e-4);
    assert!((unprojected.y - 2.0).abs() < 1.0e-4);

    // The flat-list center maps to a zero cylinder angle.
    let projection = WheelProjection::new(2.0, 0.003, 0.0, 1.0);
    assert_eq!(projection.angle_for(50.0, 100.0), 0.0);
    assert_eq!(projection.radius(Size::new(100.0, 200.0)), 200.0);
    assert!((projection.max_visible_radian() - 0.5_f32.asin()).abs() < 1.0e-6);
    assert_eq!(
        WheelProjection::new(0.5, 0.003, 0.0, 1.0).max_visible_radian(),
        std::f32::consts::FRAC_PI_2
    );
    assert_eq!(
        projection.top_scroll_margin(Size::new(100.0, 100.0), 20.0),
        -40.0
    );
}

#[test]
#[should_panic]
fn wheel_rejects_non_positive_diameter_ratio() {
    let _ = WheelProjection::new(0.0, 0.003, 0.0, 1.0);
}

#[test]
#[should_panic]
fn wheel_rejects_non_positive_item_extent() {
    let _ = ListWheelViewport::new(
        ScrollController::new(),
        0.0,
        WheelChildDelegate::children(vec![1_u32]),
    );
}

#[test]
#[should_panic]
fn wheel_rejects_outside_viewport_children_with_clipping() {
    let mut viewport = ListWheelViewport::new(
        ScrollController::new(),
        20.0,
        WheelChildDelegate::children(vec![1_u32]),
    );
    viewport.set_rendering_policy(true, Clip::HardEdge);
}

#[test]
fn wheel_rendering_policy_magnifier_opacity_and_measure() {
    let mut viewport = ListWheelViewport::new(
        ScrollController::new(),
        20.0,
        WheelChildDelegate::children(numbered(12)),
    );
    let plain = viewport.layout(Size::new(100.0, 100.0));
    viewport.set_rendering_policy(true, Clip::None);
    let eager = viewport.layout(Size::new(100.0, 100.0));
    assert!(eager.children.len() > plain.children.len());

    viewport.set_rendering_policy(false, Clip::HardEdge);
    viewport.set_magnifier(true, 1.5);
    let magnified = viewport.layout(Size::new(100.0, 100.0));
    let plain_center = plain
        .children
        .iter()
        .find(|child| child.in_center)
        .expect("plain center");
    let zoomed_center = magnified
        .children
        .iter()
        .find(|child| child.in_center)
        .expect("zoomed center");
    assert_eq!(zoomed_center.index, plain_center.index);
    assert_ne!(
        zoomed_center.transform.values,
        plain_center.transform.values
    );

    viewport.set_magnifier(false, 1.0);
    viewport.set_over_under_center_opacity(0.25);
    let faded = viewport.layout(Size::new(100.0, 100.0));
    assert!(!faded.children.is_empty());
    for child in &faded.children {
        if child.in_center {
            assert_eq!(child.opacity, 1.0);
        } else {
            assert_eq!(child.opacity, 0.25);
        }
    }

    // Custom measurement keeps its width while the fixed extent still
    // dictates the item height, and the layout stays sliver-compatible.
    let measured = viewport.layout_with_measure(Size::new(100.0, 100.0), |_, _| {
        incular_config::Constraints::new(0.0, 100.0, 20.0, 20.0).biggest()
    });
    assert!(!measured.children.is_empty());
    for child in &measured.children {
        assert_eq!(child.untransformed_rect.size.height, 20.0);
    }
    assert_eq!(measured.sliver_constraints.viewport_main_axis_extent, 100.0);
}

#[test]
fn wheel_fixed_extent_controller_paths() {
    // Without established extents the controller reports its initial item.
    let fixed = FixedExtentScrollController::new(3);
    assert_eq!(fixed.initial_item(), 3);
    assert_eq!(fixed.selected_item(20.0), 3);

    // A known extent defers the initial jump until layout establishes the
    // range, then reports the deferred item.
    let fixed = FixedExtentScrollController::with_item_extent(3, 20.0);
    let controller = fixed.controller();
    assert_eq!(fixed.selected_item(20.0), 3);
    controller.update_extents(260.0, 100.0);
    assert_eq!(fixed.selected_item(20.0), 3);

    // Pre-layout jumps defer instead of clamping away; the pending offset
    // applies once the range exists.
    let fixed = FixedExtentScrollController::new(0);
    assert!(fixed.jump_to_item(7, 20.0));
    fixed.controller().update_extents(260.0, 100.0);
    assert_eq!(fixed.selected_item(20.0), 7);

    // Post-layout jumps apply immediately through the shared controller.
    assert!(fixed.jump_to_item(2, 20.0));
    assert_eq!(fixed.selected_item(20.0), 2);
}

#[test]
fn wheel_scroll_view_wrapper_exposes_viewport_models() {
    let mut scroll_view = ListWheelScrollView::new(
        ScrollController::new(),
        20.0,
        WheelChildDelegate::children(sized_children(5)),
    );
    assert_eq!(scroll_view.viewport().selected_item(), Some(0));
    scroll_view.viewport_mut().set_magnifier(true, 1.25);
    let layout = scroll_view.layout(Size::new(100.0, 80.0));
    assert_eq!(layout.selected_index, Some(0));
    assert_eq!(layout.hit_test(Offset::new(50.0, 40.0)), Some(0));
    let retained: Widget = scroll_view.into();
    assert_eq!(retained.debug_type_name(), "ListWheelScrollView");
}

#[test]
fn wheel_physics_selection_survives_replacement() {
    let mut viewport = ListWheelViewport::new(
        ScrollController::new(),
        20.0,
        WheelChildDelegate::children(numbered(8)),
    );
    viewport.set_physics(ScrollPhysics::clamping());
    let _ = viewport.layout(Size::new(100.0, 100.0));
    assert!(viewport.jump_to_item(4));
    assert_eq!(viewport.selected_item(), Some(4));
}

fn mount(tree: &mut WidgetTree, widget: Widget, width: f32, height: f32) {
    let _ = tree.mount(widget).expect("mount");
    tree.layout(Constraints::tight(Size::new(width, height)))
        .expect("layout");
}

#[test]
fn wheel_retained_window_follows_controller_replacement_resize_and_removal() {
    // An externally driven offset re-prepares the materialized window on
    // the next layout: logical selection and visual placement agree.
    let controller_a = ScrollController::new();
    let viewport_a = ListWheelViewport::new(
        controller_a.clone(),
        20.0,
        WheelChildDelegate::children(sized_children(12)),
    );
    let mut tree = WidgetTree::new();
    let root = tree.mount(viewport_a.into()).expect("mount");
    tree.layout(Constraints::tight(Size::new(100.0, 100.0)))
        .expect("layout");
    assert!(controller_a.jump_to(80.0));
    tree.layout(Constraints::tight(Size::new(100.0, 100.0)))
        .expect("layout");
    tree.update_semantics();
    let dump = tree.semantics_debug_dump();
    assert!(dump.contains("Some(\"item4\")"), ":\n{dump}");
    assert!(!dump.contains("Some(\"item0\")"), ":\n{dump}");
    assert!(!dump.contains("Some(\"item10\")"), ":\n{dump}");

    // Replacing the controller retargets the window; the deferred jump on
    // the new controller applies once the range exists.
    let controller_b = ScrollController::new();
    assert!(controller_b.deferred_jump_to(180.0));
    let viewport_b = ListWheelViewport::new(
        controller_b.clone(),
        20.0,
        WheelChildDelegate::children(sized_children(12)),
    );
    tree.update(root, viewport_b.into()).expect("update");
    tree.layout(Constraints::tight(Size::new(100.0, 100.0)))
        .expect("layout");
    tree.update_semantics();
    let dump = tree.semantics_debug_dump();
    assert!(dump.contains("Some(\"item9\")"), ":\n{dump}");
    assert!(!dump.contains("Some(\"item4\")"), ":\n{dump}");

    // The old controller is isolated: driving it moves nothing now.
    assert!(controller_a.jump_to(0.0));
    tree.layout(Constraints::tight(Size::new(100.0, 100.0)))
        .expect("layout");
    tree.update_semantics();
    let dump = tree.semantics_debug_dump();
    assert!(dump.contains("Some(\"item9\")"), ":\n{dump}");

    // Resizing keeps the same selection with an adapted window.
    tree.layout(Constraints::tight(Size::new(100.0, 60.0)))
        .expect("layout");
    tree.update_semantics();
    let dump = tree.semantics_debug_dump();
    assert!(dump.contains("Some(\"item9\")"), ":\n{dump}");

    // Removing items clamps the window to the surviving range.
    let viewport_c = ListWheelViewport::new(
        controller_b.clone(),
        20.0,
        WheelChildDelegate::children(sized_children(3)),
    );
    tree.update(root, viewport_c.into()).expect("update");
    tree.layout(Constraints::tight(Size::new(100.0, 100.0)))
        .expect("layout");
    tree.update_semantics();
    let dump = tree.semantics_debug_dump();
    assert!(dump.contains("Some(\"item2\")"), ":\n{dump}");
    assert!(!dump.contains("Some(\"item5\")"), ":\n{dump}");
    assert!(!dump.contains("Some(\"item9\")"), ":\n{dump}");
}

#[test]
fn wheel_retained_semantics_expose_items_without_scroll_actions() {
    // The retained wheel exposes its visible item texts with no ScrollView
    // wrapper and no scroll actions: fixed-extent selection changes are
    // callback-only (on_selected_item_changed), not semantic actions.
    // Locked as observed contract; scroll-action parity stays open.
    let mut tree = WidgetTree::new();
    mount(
        &mut tree,
        ListWheelViewport::new(
            ScrollController::new(),
            20.0,
            WheelChildDelegate::children(sized_children(5)),
        )
        .into(),
        100.0,
        100.0,
    );
    tree.update_semantics();
    let dump = tree.semantics_debug_dump();
    assert!(dump.contains("Some(\"item0\")"), ":\n{dump}");
    assert!(!dump.contains("ScrollView"), ":\n{dump}");
    assert!(!dump.contains("Activate"), ":\n{dump}");
    assert!(!dump.contains("ScrollForward"), ":\n{dump}");
    assert!(!dump.contains("ScrollBackward"), ":\n{dump}");
}
