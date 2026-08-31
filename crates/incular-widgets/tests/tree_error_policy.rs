use incular_config::Constraints;
use incular_core::{Color, Size};
use incular_scroll::ScrollController;
use incular_widgets::internal::{GeneratedChildIdentity, TreeError, Widget, WidgetTree};
use incular_widgets::{
    CustomScrollView, LayoutBuilder, ListWheelScrollView, Sliver, SliverList, WheelChildDelegate,
};

fn invalid_generated_widget() -> Widget {
    Widget::column(vec![
        Widget::box_(Size::new(10., 10.), Color::WHITE).with_key(7_u64),
        Widget::box_(Size::new(10., 10.), Color::WHITE).with_key(7_u64),
    ])
}

fn assert_duplicate_generated(error: TreeError, expected: GeneratedChildIdentity) {
    match error {
        TreeError::InvalidGeneratedChild { child, source, .. } => {
            assert_eq!(child, expected);
            assert!(matches!(*source, TreeError::DuplicateKey { .. }));
        }
        other => panic!("expected generated-child error, got {other:?}"),
    }
}

#[test]
fn eager_mount_rejects_duplicate_keys_before_allocating_retained_state() {
    let mut tree = WidgetTree::new();
    let before_layers = tree.compositor_diagnostics().layers;

    let error = tree
        .mount(invalid_generated_widget())
        .expect_err("invalid mount");

    assert!(matches!(error, TreeError::DuplicateKey { .. }));
    assert_eq!(tree.element_count(), 0);
    assert_eq!(tree.render_object_count(), 0);
    assert_eq!(tree.compositor_diagnostics().layers, before_layers);
}

#[test]
fn layout_builder_returns_contextual_error_without_leaking_child_state() {
    let mut tree = WidgetTree::new();
    tree.mount(LayoutBuilder::new(|_| invalid_generated_widget()).into())
        .expect("layout builder mount");
    let before_elements = tree.element_count();
    let before_renders = tree.render_object_count();
    let before_layers = tree.compositor_diagnostics().layers;

    let error = tree
        .try_layout(Constraints::tight(Size::new(100., 100.)))
        .expect_err("builder output must be rejected");

    assert_duplicate_generated(error, GeneratedChildIdentity::LayoutBuilder);
    assert_eq!(tree.element_count(), before_elements);
    assert_eq!(tree.render_object_count(), before_renders);
    assert_eq!(tree.compositor_diagnostics().layers, before_layers);
}

#[test]
fn sliver_builder_returns_contextual_error_without_leaking_child_state() {
    let sliver = SliverList::builder(4, |_| invalid_generated_widget());
    let root: Widget = CustomScrollView::new(vec![Box::new(sliver) as Box<dyn Sliver>]).into();
    let mut tree = WidgetTree::new();
    tree.mount(root).expect("sliver root mount");
    let before_elements = tree.element_count();
    let before_renders = tree.render_object_count();
    let before_layers = tree.compositor_diagnostics().layers;

    let error = tree
        .try_layout(Constraints::tight(Size::new(100., 100.)))
        .expect_err("sliver output must be rejected");

    assert!(matches!(
        error,
        TreeError::InvalidGeneratedChild {
            child: GeneratedChildIdentity::Sliver(_),
            ..
        }
    ));
    assert_eq!(tree.element_count(), before_elements);
    assert_eq!(tree.render_object_count(), before_renders);
    assert_eq!(tree.compositor_diagnostics().layers, before_layers);
}

#[test]
fn wheel_builder_returns_contextual_error_without_leaking_child_state() {
    let delegate = WheelChildDelegate::builder(Some(4), |_| Some(invalid_generated_widget()));
    let root: Widget = ListWheelScrollView::new(ScrollController::new(), 24., delegate).into();
    let mut tree = WidgetTree::new();
    tree.mount(root).expect("wheel root mount");
    let before_elements = tree.element_count();
    let before_renders = tree.render_object_count();
    let before_layers = tree.compositor_diagnostics().layers;

    let error = tree
        .try_layout(Constraints::tight(Size::new(120., 120.)))
        .expect_err("wheel output must be rejected");

    assert!(matches!(
        error,
        TreeError::InvalidGeneratedChild {
            child: GeneratedChildIdentity::Advanced(_),
            ..
        }
    ));
    assert_eq!(tree.element_count(), before_elements);
    assert_eq!(tree.render_object_count(), before_renders);
    assert_eq!(tree.compositor_diagnostics().layers, before_layers);
}

#[test]
fn compatibility_layout_records_recoverable_error_instead_of_panicking() {
    let mut tree = WidgetTree::new();
    tree.mount(LayoutBuilder::new(|_| invalid_generated_widget()).into())
        .expect("layout builder mount");

    tree.layout(Constraints::tight(Size::new(100., 100.)));

    assert!(matches!(
        tree.last_tree_error(),
        Some(TreeError::InvalidGeneratedChild {
            child: GeneratedChildIdentity::LayoutBuilder,
            ..
        })
    ));
}
