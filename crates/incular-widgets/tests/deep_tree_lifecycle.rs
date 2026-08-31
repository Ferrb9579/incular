//! Depth contracts for retained lifecycle operations.

use incular_config::{Constraints, EdgeInsets};
use incular_core::{Color, Size};
use incular_widgets::internal::{Widget, WidgetTree};

fn deep_padding(depth: usize, mut child: Widget) -> Widget {
    for _ in 0..depth {
        child = Widget::padding(EdgeInsets::all(0.0), child);
    }
    child
}

#[test]
fn hundred_thousand_nested_widget_descriptors_clone_and_drop_iteratively() {
    let widget = deep_padding(
        100_000,
        Widget::box_(Size::new(1.0, 1.0), Color::TRANSPARENT),
    );
    let clone = widget.clone();
    assert!(widget.ptr_eq(&clone));
    drop(clone);
    drop(widget);
}

#[test]
fn widget_handle_is_one_pointer_and_shared_subtrees_outlive_parents() {
    assert_eq!(
        std::mem::size_of::<Widget>(),
        std::mem::size_of::<usize>(),
        "Widget must remain a one-pointer shared descriptor handle"
    );

    let shared = Widget::text("shared");
    let identity = shared.clone();
    assert!(shared.ptr_eq(&identity));

    let parent = Widget::column(vec![shared.clone(), shared.clone(), shared.clone()]);
    drop(parent);

    assert!(shared.ptr_eq(&identity));
    assert_eq!(shared.text_if_any().as_deref(), Some("shared"));
}

#[test]
fn four_thousand_level_retained_lifecycle_is_stack_safe_and_leak_free() {
    const DEPTH: usize = 4_096;

    let mut tree = WidgetTree::new();
    let baseline_layers = tree.compositor_diagnostics().layers;
    let root = tree
        .mount(deep_padding(DEPTH, Widget::text("leaf")))
        .expect("deep tree must mount");
    tree.verify_invariants()
        .expect("deep mounted tree invariants must hold");

    assert_eq!(tree.element_count(), DEPTH + 1);
    assert_eq!(tree.render_object_count(), DEPTH + 1);

    // These discovery phases used to contain unguarded recursive topology
    // walks. They run on the ordinary cargo-test thread: no enlarged test stack.
    assert!(tree.focusable_elements().is_empty());
    tree.update_semantics();

    tree.update(root, deep_padding(DEPTH, Widget::text("updated")))
        .expect("deep compatible update must succeed");
    tree.verify_invariants()
        .expect("deep updated tree invariants must hold");
    assert_eq!(tree.element_count(), DEPTH + 1);
    assert_eq!(tree.render_object_count(), DEPTH + 1);

    // Replacing the root forces post-order teardown of every retained node.
    tree.mount(Widget::box_(Size::new(1.0, 1.0), Color::WHITE))
        .expect("replacement root must mount");
    tree.verify_invariants()
        .expect("deep replacement invariants must hold");
    assert_eq!(tree.element_count(), 1);
    assert_eq!(tree.render_object_count(), 1);
    assert_eq!(tree.compositor_diagnostics().layers, baseline_layers + 2);
    assert!(tree.update(root, Widget::text("stale")).is_err());
}

#[test]
fn deep_environment_propagation_and_generated_child_replacement_are_stack_safe() {
    const DEPTH: usize = 2_048;
    let constraints = Constraints::tight(Size::new(32.0, 32.0));
    let mut tree = WidgetTree::new();

    let root = tree
        .mount(Widget::environment_scope(
            1_u64,
            deep_padding(DEPTH, Widget::text("environment leaf")),
        ))
        .expect("environment root must mount");
    tree.try_layout(constraints)
        .expect("deep environment child must materialize and layout");
    tree.verify_invariants()
        .expect("generated child invariants must hold after initial materialization");
    let stable_elements = tree.element_count();
    let stable_renders = tree.render_object_count();

    tree.update(
        root,
        Widget::environment_scope(2_u64, deep_padding(DEPTH, Widget::text("environment leaf"))),
    )
    .expect("environment update must propagate without recursive bookkeeping");
    tree.try_layout(constraints)
        .expect("updated environment child must remain layoutable");
    tree.verify_invariants()
        .expect("generated child invariants must hold after replacement");

    assert_eq!(tree.element_count(), stable_elements);
    assert_eq!(tree.render_object_count(), stable_renders);
}
