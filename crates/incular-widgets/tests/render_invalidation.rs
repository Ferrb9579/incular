//! Behavioral contracts for retained render invalidation.
//!
//! These tests intentionally observe public tree diagnostics instead of
//! reaching into the crate-private invalidation representation. That keeps the
//! contract focused on scheduled work rather than implementation details.

use incular_config::Constraints;
use incular_core::{Color, Offset, Size, Transform};
use incular_text::{TextAlign, TextStyle};
use incular_widgets::internal::{Widget, WidgetTree};

fn frame() -> Constraints {
    Constraints::tight(Size::new(320.0, 120.0))
}

fn prepared(widget: Widget) -> (WidgetTree, incular_widgets::internal::ElementId) {
    let mut tree = WidgetTree::default();
    let root = tree.mount(widget).expect("mount");
    tree.layout(frame()).expect("layout");
    let _ = tree.paint();
    (tree, root)
}

#[test]
fn opacity_property_update_is_compositor_only() {
    let child = || Widget::box_(Size::new(20.0, 20.0), Color::WHITE);
    let (mut tree, root) = prepared(incular_widgets::Opacity::new(0.25, child()).into());
    let before = tree.diagnostics();

    tree.update(root, incular_widgets::Opacity::new(0.75, child()).into())
        .expect("update");
    tree.layout(frame()).expect("layout");
    let _ = tree.paint();

    let after = tree.diagnostics();
    assert_eq!(after.layouts, before.layouts, "opacity must not relayout");
    assert_eq!(after.paints, before.paints, "opacity must not repaint");
    assert_eq!(
        after.compositor_only_updates - before.compositor_only_updates,
        1
    );
}

#[test]
fn affine_property_update_is_compositor_only() {
    let child = || Widget::box_(Size::new(20.0, 20.0), Color::WHITE);
    let (mut tree, root) = prepared(
        incular_widgets::Transform::new(Transform::translation(Offset::new(4.0, 2.0)), child())
            .into(),
    );
    let before = tree.diagnostics();

    tree.update(
        root,
        incular_widgets::Transform::new(Transform::translation(Offset::new(18.0, 7.0)), child())
            .into(),
    )
    .expect("update");
    tree.layout(frame()).expect("layout");
    let _ = tree.paint();

    let after = tree.diagnostics();
    assert_eq!(after.layouts, before.layouts, "transform must not relayout");
    assert_eq!(after.paints, before.paints, "transform must not repaint");
    assert_eq!(
        after.compositor_only_updates - before.compositor_only_updates,
        1
    );
}

#[test]
fn text_color_update_repaints_without_relayout() {
    let style = |color| TextStyle::default().font_size(18.0).color(color);
    let (mut tree, root) = prepared(
        incular_widgets::Text::new("retained")
            .style(style(Color::BLACK))
            .align(TextAlign::Start)
            .into(),
    );
    let before = tree.diagnostics();

    tree.update(
        root,
        incular_widgets::Text::new("retained")
            .style(style(Color::WHITE))
            .align(TextAlign::Start)
            .into(),
    )
    .expect("update");
    tree.layout(frame()).expect("layout");
    let _ = tree.paint();

    let after = tree.diagnostics();
    assert_eq!(after.layouts, before.layouts, "color does not reshape text");
    assert_eq!(after.paints - before.paints, 1, "text must repaint");
}

#[test]
fn text_metric_update_invalidates_layout_and_paint() {
    let (mut tree, root) = prepared(
        incular_widgets::Text::new("retained")
            .style(TextStyle::default().font_size(18.0))
            .align(TextAlign::Start)
            .into(),
    );
    let before = tree.diagnostics();

    tree.update(
        root,
        incular_widgets::Text::new("retained")
            .style(TextStyle::default().font_size(28.0))
            .align(TextAlign::Start)
            .into(),
    )
    .expect("update");
    tree.layout(frame()).expect("layout");
    let _ = tree.paint();

    let after = tree.diagnostics();
    assert_eq!(after.layouts - before.layouts, 1, "font size must relayout");
    assert_eq!(after.paints - before.paints, 1, "font size must repaint");
}

#[test]
fn fixed_box_color_update_repaints_without_relayout() {
    let size = Size::new(40.0, 24.0);
    let (mut tree, root) = prepared(Widget::box_(size, Color::BLACK));
    let before = tree.diagnostics();

    tree.update(root, Widget::box_(size, Color::WHITE))
        .expect("update");
    tree.layout(frame()).expect("layout");
    let _ = tree.paint();

    let after = tree.diagnostics();
    assert_eq!(
        after.layouts, before.layouts,
        "color does not change geometry"
    );
    assert_eq!(after.paints - before.paints, 1, "box must repaint");
}

#[test]
fn compositor_translation_updates_hit_testing_and_semantics_without_layout_or_paint() {
    use incular_widgets::internal::{ActionId, action};
    let widget = |x| {
        incular_widgets::Transform::new(
            Transform::translation(Offset::new(x, 0.0)),
            action(Size::new(20.0, 20.0), Color::WHITE, ActionId(7))
                .accessibility_label("moving action"),
        )
        .into()
    };
    let mut tree = WidgetTree::default();
    let root = tree.mount(widget(0.0)).expect("mount");
    let constraints = Constraints::loose(Size::new(320.0, 120.0));
    tree.layout(constraints).expect("layout");
    let _ = tree.paint();
    tree.update_semantics();
    let before = tree.diagnostics();
    let target = tree
        .hit_test(Offset::new(5.0, 5.0))
        .expect("initial target");
    let (semantic_id, original_bounds) = tree
        .semantics()
        .iter()
        .find(|(_, node)| node.label.as_deref() == Some("moving action"))
        .map(|(id, node)| (id, node.bounds))
        .expect("semantic action");

    tree.update(root, widget(40.0)).expect("update");
    // Geometry must be observable even before the next layout/paint pass.
    assert!(tree.hit_test(Offset::new(5.0, 5.0)).is_none());
    assert_eq!(tree.hit_test(Offset::new(45.0, 5.0)), Some(target));
    tree.update_semantics();
    let moved = tree
        .semantics()
        .node(semantic_id)
        .expect("stable semantic identity");
    assert_eq!(
        moved.bounds.origin,
        original_bounds.origin + Offset::new(40.0, 0.0)
    );
    assert_eq!(moved.bounds.size, original_bounds.size);
    tree.layout(constraints).expect("layout");
    let _ = tree.paint();
    let after = tree.diagnostics();
    assert_eq!(after.layouts, before.layouts);
    assert_eq!(after.paints, before.paints);
}
