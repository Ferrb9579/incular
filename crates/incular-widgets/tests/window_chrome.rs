use incular_core::{Offset, Size, WindowResizeDirection};
use incular_layout::Constraints;
use incular_widgets::internal::{WidgetTree, WindowInteraction};
use incular_widgets::{Color, GestureDetector, WindowDragRegion, WindowResizeRegion};

fn layout(tree: &mut WidgetTree, root: impl Into<incular_widgets::Widget>) {
    tree.mount(root.into()).expect("mount custom chrome");
    tree.layout(Constraints::tight(Size::new(120., 40.)))
        .expect("layout custom chrome");
}

#[test]
fn window_drag_region_is_a_transparent_retained_hit_annotation() {
    let mut tree = WidgetTree::new();
    layout(
        &mut tree,
        WindowDragRegion::new(incular_widgets::Widget::box_(
            Size::new(120., 40.),
            Color::WHITE,
        )),
    );

    let (_, interaction) = tree
        .window_interaction_at(Offset::new(20., 10.))
        .expect("drag annotation");
    assert_eq!(interaction, WindowInteraction::Move);
    assert_eq!(tree.root_layout_size(), Some(Size::new(120., 40.)));
}

#[test]
fn nested_resize_region_wins_over_outer_drag_region() {
    let mut tree = WidgetTree::new();
    layout(
        &mut tree,
        WindowDragRegion::new(WindowResizeRegion::new(
            WindowResizeDirection::SouthEast,
            incular_widgets::Widget::box_(Size::new(120., 40.), Color::WHITE),
        )),
    );

    let (_, interaction) = tree
        .window_interaction_at(Offset::new(60., 20.))
        .expect("nested chrome annotation");
    assert_eq!(
        interaction,
        WindowInteraction::Resize(WindowResizeDirection::SouthEast)
    );
}

#[test]
fn every_resize_direction_survives_descriptor_lowering() {
    for direction in [
        WindowResizeDirection::East,
        WindowResizeDirection::North,
        WindowResizeDirection::NorthEast,
        WindowResizeDirection::NorthWest,
        WindowResizeDirection::South,
        WindowResizeDirection::SouthEast,
        WindowResizeDirection::SouthWest,
        WindowResizeDirection::West,
    ] {
        let mut tree = WidgetTree::new();
        layout(
            &mut tree,
            WindowResizeRegion::new(
                direction,
                incular_widgets::Widget::box_(Size::new(120., 40.), Color::WHITE),
            ),
        );
        let (_, interaction) = tree
            .window_interaction_at(Offset::new(1., 1.))
            .expect("resize annotation");
        assert_eq!(interaction, WindowInteraction::Resize(direction));
    }
}

#[test]
fn gesture_descendant_suppresses_window_drag_before_tap_resolves() {
    let child = GestureDetector::new(incular_widgets::Widget::box_(
        Size::new(120., 40.),
        Color::WHITE,
    ))
    .on_tap(|| {});
    let mut tree = WidgetTree::new();
    layout(&mut tree, WindowDragRegion::new(child));

    assert!(
        tree.window_interaction_at(Offset::new(20., 10.)).is_none(),
        "joining a descendant gesture arena must suppress native titlebar drag even before the tap resolves"
    );
}
