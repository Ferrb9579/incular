use incular_config::{Constraints, EdgeInsets};
use incular_core::{Color, Offset, PointerPhase, Rect, Size};
use incular_gestures::PointerEvent;
use incular_rendering::PaintCommand;
use incular_widgets::internal::WidgetTree;
use incular_widgets::{
    Column, GestureDetector, OverlayPortal, Padding, Positioned, SizedBox, TransientPlacement,
    TransientPresentation, TransientRole, Widget,
};
use std::time::Instant;
use std::{cell::Cell, rc::Rc};

fn anchored_portal(inset: f32) -> Widget {
    let anchor = Widget::box_(Size::new(20.0, 10.0), Color::WHITE);
    let popup = Widget::box_(Size::new(30.0, 40.0), Color::BLACK);
    Padding::all(
        inset,
        OverlayPortal::new(anchor)
            .overlay_child(popup)
            .placement(TransientPlacement::new().alignment_offset(Offset::new(5.0, 2.0)))
            .role(TransientRole::Menu)
            .show(true),
    )
    .into()
}

#[test]
fn transient_portal_reports_anchor_and_popup_but_not_barrier_geometry() {
    let anchor = Widget::box_(Size::new(20.0, 10.0), Color::WHITE);
    let barrier: Widget = Positioned::fill(SizedBox::expand()).into();
    let popup = Widget::box_(Size::new(30.0, 40.0), Color::BLACK);
    let portal: Widget = OverlayPortal::new(anchor)
        .barrier_child(barrier)
        .overlay_child(popup)
        .placement(
            TransientPlacement::new()
                .alignment_offset(Offset::new(5.0, 2.0))
                .safe_margin(EdgeInsets::ZERO),
        )
        .role(TransientRole::Menu)
        .presentation(TransientPresentation::Auto)
        .show(true)
        .into();

    let mut tree = WidgetTree::new();
    tree.mount(portal).expect("mount transient portal");
    tree.layout(Constraints::tight(Size::new(100.0, 80.0)))
        .expect("layout transient portal");

    let surfaces = tree.transient_surfaces();
    assert_eq!(surfaces.len(), 1);
    let surface = surfaces[0];
    assert_eq!(surface.role, TransientRole::Menu);
    assert_eq!(surface.presentation, TransientPresentation::Auto);
    assert_eq!(
        surface.anchor_rect,
        Rect::from_origin_size(Offset::ZERO, Size::new(20.0, 10.0))
    );
    assert_eq!(
        surface.content_rect,
        Rect::from_origin_size(Offset::new(5.0, 12.0), Size::new(30.0, 40.0))
    );
}

#[test]
fn retained_barrier_stays_behind_an_open_transient_anchor() {
    let anchor_hits = Rc::new(Cell::new(0));
    let anchor_observed = anchor_hits.clone();
    let barrier_hits = Rc::new(Cell::new(0));
    let barrier_observed = barrier_hits.clone();
    let anchor: Widget = GestureDetector::new(
        SizedBox::new()
            .width(40.0)
            .height(20.0)
            .child(Widget::box_(Size::new(40.0, 20.0), Color::WHITE)),
    )
    .on_tap(move || anchor_observed.set(anchor_observed.get() + 1))
    .into();
    let barrier: Widget = Positioned::fill(
        GestureDetector::new(SizedBox::expand())
            .on_tap(move || barrier_observed.set(barrier_observed.get() + 1)),
    )
    .into();
    let portal: Widget = OverlayPortal::new(anchor)
        .barrier_child(barrier)
        .overlay_child(Widget::box_(Size::new(30.0, 40.0), Color::BLACK))
        .show(true)
        .into();
    let mut tree = WidgetTree::new();
    tree.mount(portal).expect("mount transient portal");
    tree.layout(Constraints::tight(Size::new(100.0, 80.0)))
        .expect("layout transient portal");
    let now = Instant::now();
    for phase in [PointerPhase::Down, PointerPhase::Up] {
        let _ = tree.dispatch_gesture(PointerEvent {
            pointer: 1,
            position: Offset::new(10.0, 10.0),
            phase,
            time: now,
        });
    }
    assert_eq!(anchor_hits.get(), 1);
    assert_eq!(barrier_hits.get(), 0);
}

#[test]
fn closed_transient_portal_has_no_surface_snapshot() {
    let portal: Widget = OverlayPortal::new(Widget::box_(Size::new(20.0, 10.0), Color::WHITE))
        .overlay_child(Widget::box_(Size::new(30.0, 40.0), Color::BLACK))
        .role(TransientRole::Menu)
        .show(false)
        .into();
    let mut tree = WidgetTree::new();
    tree.mount(portal).expect("mount closed transient portal");
    tree.layout(Constraints::tight(Size::new(100.0, 80.0)))
        .expect("layout closed transient portal");
    assert!(tree.transient_surfaces().is_empty());
}

#[test]
fn context_menu_can_anchor_to_a_pointer_position_independent_of_trigger_bounds() {
    let portal: Widget = OverlayPortal::new(Widget::box_(Size::new(20.0, 10.0), Color::WHITE))
        .overlay_child(Widget::box_(Size::new(30.0, 40.0), Color::BLACK))
        .anchor_point(Offset::new(70.0, 55.0))
        .placement(TransientPlacement::new())
        .role(TransientRole::ContextMenu)
        .show(true)
        .into();
    let mut tree = WidgetTree::new();
    tree.mount(portal).unwrap();
    tree.layout(Constraints::tight(Size::new(200.0, 160.0)))
        .unwrap();
    let snapshot = tree.transient_surfaces()[0];
    assert_eq!(
        snapshot.anchor_rect,
        Rect::from_origin_size(Offset::new(70.0, 55.0), Size::ZERO)
    );
    assert_eq!(snapshot.content_rect.origin, Offset::new(70.0, 55.0));
}

#[test]
fn transient_snapshot_tracks_anchor_relayout_without_changing_identity() {
    let mut tree = WidgetTree::new();
    let root = tree.mount(anchored_portal(8.0)).expect("mount portal");
    tree.layout(Constraints::loose(Size::new(200.0, 160.0)))
        .expect("initial layout");
    let first = tree.transient_surfaces()[0];
    assert_eq!(first.anchor_rect.origin, Offset::new(8.0, 8.0));

    tree.update(root, anchored_portal(24.0))
        .expect("update anchor layout");
    tree.layout(Constraints::loose(Size::new(200.0, 160.0)))
        .expect("updated layout");
    let second = tree.transient_surfaces()[0];

    assert_eq!(
        first.id, second.id,
        "mounted portal identity must remain stable"
    );
    assert_eq!(second.anchor_rect.origin, Offset::new(24.0, 24.0));
    assert_eq!(second.content_rect.origin, Offset::new(29.0, 36.0));
}

#[test]
fn painted_transient_subtree_is_marked_with_the_snapshot_partition_identity() {
    let portal = anchored_portal(8.0);
    let mut tree = WidgetTree::new();
    tree.mount(portal).expect("mount portal");
    tree.layout(Constraints::loose(Size::new(200.0, 160.0)))
        .expect("layout portal");
    let snapshot = tree.transient_surfaces()[0];
    let list = tree.paint();
    let partition = snapshot.id.surface_partition();

    assert!(list.commands().iter().any(|command| {
        matches!(command, PaintCommand::PushSurfacePartition { id } if *id == partition)
    }));
    let selected = [partition].into_iter().collect();
    let (parent, detached) = list.detach_surface_partitions(&selected);
    assert!(
        detached
            .get(&partition)
            .is_some_and(|list| !list.is_empty())
    );
    assert!(parent.commands().iter().all(|command| !matches!(
        command,
        PaintCommand::PushSurfacePartition { .. } | PaintCommand::PopSurfacePartition
    )));
}

#[test]
fn simultaneous_transients_keep_distinct_retained_and_render_partition_identity() {
    let root: Widget = Column::new([anchored_portal(4.0), anchored_portal(12.0)]).into();
    let mut tree = WidgetTree::new();
    tree.mount(root).expect("mount simultaneous portals");
    tree.layout(Constraints::loose(Size::new(200.0, 200.0)))
        .expect("layout simultaneous portals");

    let surfaces = tree.transient_surfaces();
    assert_eq!(surfaces.len(), 2);
    assert_ne!(surfaces[0].id, surfaces[1].id);
    let selected = surfaces
        .iter()
        .map(|surface| surface.id.surface_partition())
        .collect();
    let (parent, detached) = tree.paint().detach_surface_partitions(&selected);

    assert_eq!(detached.len(), 2);
    for surface in surfaces {
        assert!(
            detached
                .get(&surface.id.surface_partition())
                .is_some_and(|list| !list.is_empty())
        );
    }
    assert!(parent.commands().iter().all(|command| !matches!(
        command,
        PaintCommand::PushSurfacePartition { .. } | PaintCommand::PopSurfacePartition
    )));
}

#[test]
fn outside_pointer_dismisses_unrelated_sibling_but_preserves_hit_chain() {
    let first_hits = Rc::new(Cell::new(0));
    let second_hits = Rc::new(Cell::new(0));
    let first_observed = first_hits.clone();
    let second_observed = second_hits.clone();
    let first: Widget = OverlayPortal::new(Widget::box_(Size::new(30.0, 20.0), Color::WHITE))
        .overlay_child(Widget::box_(Size::new(60.0, 40.0), Color::BLACK))
        .placement(TransientPlacement::new())
        .role(TransientRole::Menu)
        .on_dismiss(move |_| first_observed.set(first_observed.get() + 1))
        .show(true)
        .into();
    let second: Widget = OverlayPortal::new(Widget::box_(Size::new(30.0, 20.0), Color::WHITE))
        .overlay_child(Widget::box_(Size::new(60.0, 40.0), Color::BLACK))
        .placement(TransientPlacement::new())
        .role(TransientRole::Menu)
        .on_dismiss(move |_| second_observed.set(second_observed.get() + 1))
        .show(true)
        .into();
    let root: Widget = Column::new(vec![first, SizedBox::new().height(80.0).into(), second]).into();
    let mut tree = WidgetTree::new();
    tree.mount(root).unwrap();
    tree.layout(Constraints::tight(Size::new(240.0, 240.0)))
        .unwrap();
    let surfaces = tree.transient_surfaces();
    assert_eq!(surfaces.len(), 2);
    let first_point = surfaces[0].content_rect.origin + Offset::new(4.0, 4.0);
    assert_eq!(tree.dismiss_transients_for_pointer(first_point), 1);
    assert_eq!(first_hits.get(), 0);
    assert_eq!(second_hits.get(), 1);

    assert_eq!(
        tree.dismiss_transients_for_pointer(Offset::new(220.0, 220.0)),
        2
    );
    assert_eq!(first_hits.get(), 1);
    assert_eq!(second_hits.get(), 2);
}

#[test]
fn nested_transient_hit_protects_its_retained_parent_chain() {
    let parent_hits = Rc::new(Cell::new(0));
    let child_hits = Rc::new(Cell::new(0));
    let parent_observed = parent_hits.clone();
    let child_observed = child_hits.clone();
    let nested: Widget = OverlayPortal::new(Widget::box_(Size::new(40.0, 20.0), Color::WHITE))
        .overlay_child(Widget::box_(Size::new(50.0, 30.0), Color::BLACK))
        .placement(TransientPlacement::new().alignment_offset(Offset::new(60.0, 0.0)))
        .role(TransientRole::Menu)
        .on_dismiss(move |_| child_observed.set(child_observed.get() + 1))
        .show(true)
        .into();
    let root: Widget = OverlayPortal::new(Widget::box_(Size::new(30.0, 20.0), Color::WHITE))
        .overlay_child(nested)
        .placement(TransientPlacement::new())
        .role(TransientRole::Menu)
        .on_dismiss(move |_| parent_observed.set(parent_observed.get() + 1))
        .show(true)
        .into();
    let mut tree = WidgetTree::new();
    tree.mount(root).unwrap();
    tree.layout(Constraints::tight(Size::new(260.0, 180.0)))
        .unwrap();
    let surfaces = tree.transient_surfaces();
    assert_eq!(surfaces.len(), 2);
    let child = surfaces
        .iter()
        .find(|surface| surface.parent.is_some())
        .copied()
        .expect("nested surface");
    let parent = surfaces
        .iter()
        .find(|surface| surface.id == child.parent.unwrap())
        .copied()
        .expect("parent surface");
    let child_point = child.content_rect.origin + Offset::new(2.0, 2.0);
    assert!(!parent.content_rect.contains(child_point));
    assert_eq!(tree.dismiss_transients_for_pointer(child_point), 0);
    assert_eq!(parent_hits.get(), 0);
    assert_eq!(child_hits.get(), 0);
}
