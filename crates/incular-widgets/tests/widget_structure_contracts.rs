//! Public retained-tree contracts for declarative child topology.

use incular_config::{Constraints, EdgeInsets};
use incular_core::Size;
use incular_widgets::internal::WidgetTree;

#[test]
fn ordinary_child_shapes_materialize_the_expected_retained_topology() {
    let mut leaf_tree = WidgetTree::new();
    leaf_tree
        .mount(incular_widgets::Text::new("leaf").into())
        .expect("leaf mount");
    assert_eq!(leaf_tree.element_count(), 1);

    let mut single_tree = WidgetTree::new();
    single_tree
        .mount(
            incular_widgets::Padding::new(
                EdgeInsets::all(4.0),
                incular_widgets::Text::new("child"),
            )
            .into(),
        )
        .expect("single-child mount");
    assert_eq!(single_tree.element_count(), 2);

    let mut many_tree = WidgetTree::new();
    many_tree
        .mount(
            incular_widgets::Row::new([
                incular_widgets::Text::new("first"),
                incular_widgets::Text::new("second"),
            ])
            .into(),
        )
        .expect("many-child mount");
    assert_eq!(many_tree.element_count(), 3);
}

#[test]
fn dynamic_children_are_not_eager_declarative_children() {
    let mut tree = WidgetTree::new();
    tree.mount(
        incular_widgets::LayoutBuilder::new(|_, _| incular_widgets::Text::new("generated").into())
            .into(),
    )
    .expect("layout-builder mount");

    assert_eq!(tree.element_count(), 1);
    tree.layout(Constraints::tight(Size::new(100.0, 100.0)))
        .expect("generated child layout");
    assert_eq!(tree.element_count(), 2);
}
