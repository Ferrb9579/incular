#![cfg(feature = "devtools")]

//! Public DevTools observation and editing behavior tests.

use incular_config::{Constraints, CrossAxisAlignment, FlexFit, MainAxisSize};
use incular_core::{Color, Offset, Size};
use incular_devtools_protocol::{
    DebugValue, DevWidgetId, DevWindowId, InvalidationReason, LayoutDetails, TracePhase,
};
use incular_widgets::{
    Flexible, Widget,
    internal::{ActionSurface, InvalidationCause, WidgetTree},
};
use std::time::{Duration, Instant};

fn dev_id(tree: &WidgetTree, id: incular_widgets::internal::ElementId) -> DevWidgetId {
    tree.devtools_id_for_element(id).expect("devtools id")
}

#[test]
fn snapshots_keep_arena_ids_stable_between_polls() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            incular_widgets::Column::new([
                incular_widgets::Text::new("first"),
                incular_widgets::Text::new("second"),
            ])
            .into(),
        )
        .expect("mount");
    let (first, _) = tree.devtools_snapshot(root, false);
    let (second, _) = tree.devtools_snapshot(root, false);
    let first_ids: Vec<_> = first.iter().map(|node| node.id).collect();
    let second_ids: Vec<_> = second.iter().map(|node| node.id).collect();
    assert_eq!(first_ids, second_ids);
    assert!(
        first
            .iter()
            .all(|node| tree.devtools_resolve_id(node.id).is_some())
    );
}

#[test]
fn editable_opacity_override_is_typed_and_visible_in_details() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            incular_widgets::Opacity::new(0.75, Widget::box_(Size::new(10., 10.), Color::WHITE))
                .into(),
        )
        .expect("mount");
    tree.layout(Constraints::tight(Size::new(10., 10.)))
        .expect("layout");
    let id = dev_id(&tree, root);
    let before = tree
        .devtools_node_details(root, id, DevWindowId::new(1, 0))
        .expect("details");
    assert!(before.properties.iter().any(|property| {
        property.name == "opacity" && property.editable && property.value == DebugValue::Float(0.75)
    }));

    assert!(tree.devtools_edit_property(id, "opacity", &DebugValue::Float(0.25)));
    let after = tree
        .devtools_node_details(root, id, DevWindowId::new(1, 0))
        .expect("details");
    assert!(after.properties.iter().any(|property| {
        property.name == "opacity" && property.value == DebugValue::Float(0.25)
    }));
    assert!(!tree.devtools_edit_property(id, "opacity", &DebugValue::Str("invalid".into())));
}

#[test]
fn compatible_updates_report_curated_property_changes_through_details() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(incular_widgets::Text::new("Count: 7").into())
        .expect("mount");
    tree.layout(Constraints::tight(Size::new(100., 30.)))
        .expect("layout");
    tree.update(root, incular_widgets::Text::new("Count: 8").into())
        .expect("update");
    let details = tree
        .devtools_node_details(root, dev_id(&tree, root), DevWindowId::new(1, 0))
        .expect("details");

    assert_eq!(details.property_changes.len(), 1);
    assert_eq!(details.property_changes[0].name, "text");
    assert_eq!(
        details.property_changes[0].old,
        Some(DebugValue::Str("Count: 7".into()))
    );
    assert_eq!(
        details.property_changes[0].new,
        Some(DebugValue::Str("Count: 8".into()))
    );
}

#[test]
fn flex_inspection_uses_retained_child_constraints_and_offsets() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            incular_widgets::Row::new(vec![
                Widget::box_(Size::new(20., 10.), Color::WHITE),
                Flexible::new(Widget::box_(Size::new(4., 10.), Color::WHITE))
                    .flex(1)
                    .fit(FlexFit::Tight)
                    .into(),
            ])
            .main_axis_size(MainAxisSize::Min)
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .into(),
        )
        .expect("mount");
    tree.layout(Constraints::tight(Size::new(100., 30.)))
        .expect("layout");
    let details = tree
        .devtools_node_details(root, dev_id(&tree, root), DevWindowId::new(1, 1))
        .expect("details");
    let Some(layout) = details.layout else {
        panic!("layout snapshot");
    };
    let LayoutDetails::Flex {
        children, overflow, ..
    } = layout.details
    else {
        panic!("flex inspection");
    };
    assert_eq!(children.len(), 2);
    assert_eq!(children[0].actual_main_extent, 20.);
    assert_eq!(children[1].offset, [20., 0.]);
    assert_eq!(children[1].allocated_main_extent, Some(80.));
    assert_eq!(children[1].actual_main_extent, 80.);
    assert_eq!(overflow, 0.);
}

#[test]
fn whole_tree_overlay_snapshot_has_a_hard_bound() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            incular_widgets::Column::new(
                (0..32)
                    .map(|_| Widget::box_(Size::new(1., 1.), Color::WHITE))
                    .collect::<Vec<_>>(),
            )
            .into(),
        )
        .expect("mount");
    tree.layout(Constraints::tight(Size::new(10., 40.)))
        .expect("layout");
    assert_eq!(tree.devtools_layout_bounds(3).len(), 3);
    assert!(tree.devtools_subtree_layout_bounds(root, 2).len() <= 2);
}

#[test]
fn coalesced_invalidation_snapshot_keeps_multiple_real_causes_bounded() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(incular_widgets::Text::new("cause").into())
        .expect("mount");
    for _ in 0..10 {
        tree.note_invalidation(root, InvalidationCause::Manual);
    }
    let details = tree
        .devtools_node_details(root, dev_id(&tree, root), DevWindowId::new(1, 1))
        .expect("details");
    assert_eq!(details.invalidation_causes.len(), 8);
    assert!(matches!(
        details.invalidation,
        Some(InvalidationReason::ManualInvalidation)
    ));
}

#[test]
fn animation_time_scale_changes_only_retained_animation_progression() {
    let controller = incular_widgets::internal::TranslationController::new();
    let mut tree = WidgetTree::new();
    tree.mount(Widget::translate(
        controller.clone(),
        Widget::box_(Size::new(10., 10.), Color::WHITE),
    ))
    .expect("mount");
    tree.layout(Constraints::tight(Size::new(20., 20.)))
        .expect("layout");
    let origin = Instant::now();
    tree.set_animation_time_scale(0.5);
    tree.update_compositor(origin).expect("compositor update");
    controller.animate_to(Offset::new(100., 0.), Duration::from_millis(100), origin);
    tree.update_compositor(origin + Duration::from_millis(16))
        .expect("compositor update");
    assert!((controller.offset().x - 8.).abs() < 0.1);

    tree.set_animation_time_scale(0.);
    tree.update_compositor(origin + Duration::from_millis(32))
        .expect("compositor update");
    assert!((controller.offset().x - 8.).abs() < 0.1);
}

#[test]
fn phase_snapshot_uses_existing_work_counters_and_respects_limit() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(incular_widgets::Text::new("phase").into())
        .expect("mount");
    tree.layout(Constraints::tight(Size::new(50., 20.)))
        .expect("layout");
    let _ = tree.paint();
    let phase = tree.devtools_phase_nodes(1);
    assert_eq!(phase.len(), 1);
    assert_eq!(phase[0].id, dev_id(&tree, root));
    assert!(phase[0].layouts > 0);
    assert!(phase[0].paints > 0);
}

#[test]
fn deep_trace_is_opt_in_and_layout_events_preserve_parentage() {
    let mut tree = WidgetTree::new();
    tree.mount(
        incular_widgets::Column::new(vec![Widget::box_(Size::new(10., 10.), Color::WHITE)]).into(),
    )
    .expect("mount");
    tree.layout(Constraints::tight(Size::new(20., 20.)))
        .expect("layout");
    assert!(tree.take_deep_trace().is_none());

    tree.begin_deep_trace(32);
    tree.layout(Constraints::tight(Size::new(24., 24.)))
        .expect("layout");
    let (events, dropped) = tree.take_deep_trace().expect("deep trace");
    let layouts = events
        .iter()
        .filter(|event| event.phase == TracePhase::Layout)
        .collect::<Vec<_>>();
    assert!(layouts.len() >= 2);
    assert_eq!(layouts[0].parent, None);
    assert_eq!(layouts[1].parent, Some(0));
    assert_eq!(dropped, 0);
    let root = tree.root().expect("root");
    let details = tree
        .devtools_node_details(root, dev_id(&tree, root), DevWindowId::new(1, 0))
        .expect("details");
    assert!(!details.layout_history.is_empty());
}

#[test]
fn deep_trace_event_buffer_reports_truncation_inputs() {
    let mut tree = WidgetTree::new();
    tree.mount(
        incular_widgets::Column::new(
            (0..8)
                .map(|_| Widget::box_(Size::new(1., 1.), Color::WHITE))
                .collect::<Vec<_>>(),
        )
        .into(),
    )
    .expect("mount");
    tree.begin_deep_trace(2);
    tree.layout(Constraints::tight(Size::new(10., 20.)))
        .expect("layout");
    let (events, dropped) = tree.take_deep_trace().expect("trace");
    assert_eq!(events.len(), 2);
    assert!(dropped > 0);
}

#[test]
fn auxiliary_overlays_read_retained_subsystems_and_stay_bounded() {
    let mut tree = WidgetTree::new();
    tree.mount(
        incular_widgets::SingleChildScrollView::new(ActionSurface::new("Still active"))
            .controller(incular_widgets::internal::ScrollController::new())
            .into(),
    )
    .expect("mount");
    tree.layout(Constraints::tight(Size::new(40., 30.)))
        .expect("layout");
    tree.update_semantics();
    let before = tree.diagnostics();
    assert_eq!(tree.devtools_scroll_viewports(1).len(), 1);
    assert!(!tree.devtools_hit_regions(8).is_empty());
    assert!(!tree.devtools_semantics_bounds(8).is_empty());
    assert!(!tree.devtools_layer_bounds(8).is_empty());
    assert!(tree.devtools_hit_regions(1).len() <= 1);
    assert_eq!(tree.diagnostics(), before);
}
