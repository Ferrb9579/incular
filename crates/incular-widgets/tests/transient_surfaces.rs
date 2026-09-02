use incular_config::Constraints;
use incular_core::{Color, Offset, Rect, Size};
use incular_rendering::PaintCommand;
use incular_widgets::internal::WidgetTree;
use incular_widgets::{
    Column, OverlayPortal, Padding, Positioned, SizedBox, TransientPresentation, TransientRole,
    Widget,
};

fn anchored_portal(inset: f32) -> Widget {
    let anchor = Widget::box_(Size::new(20.0, 10.0), Color::WHITE);
    let popup: Widget = Positioned::new(Widget::box_(Size::new(30.0, 40.0), Color::BLACK))
        .left(5.0)
        .top(12.0)
        .width(30.0)
        .height(40.0)
        .into();
    Padding::all(
        inset,
        OverlayPortal::new(anchor)
            .overlay_child(popup)
            .role(TransientRole::Menu)
            .show(true),
    )
    .into()
}

#[test]
fn transient_portal_reports_anchor_and_popup_but_not_barrier_geometry() {
    let anchor = Widget::box_(Size::new(20.0, 10.0), Color::WHITE);
    let barrier: Widget = Positioned::fill(SizedBox::expand()).into();
    let popup: Widget = Positioned::new(Widget::box_(Size::new(30.0, 40.0), Color::BLACK))
        .left(5.0)
        .top(12.0)
        .width(30.0)
        .height(40.0)
        .into();
    let portal: Widget = OverlayPortal::new(anchor)
        .barrier_child(barrier)
        .overlay_child(popup)
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
