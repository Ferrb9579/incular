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
    tree.layout(frame());
    let _ = tree.paint();
    (tree, root)
}

#[test]
fn opacity_property_update_is_compositor_only() {
    let child = || Widget::box_(Size::new(20.0, 20.0), Color::WHITE);
    let (mut tree, root) = prepared(Widget::opacity(0.25, child()));
    let before = tree.diagnostics();

    tree.update(root, Widget::opacity(0.75, child()))
        .expect("update");
    tree.layout(frame());
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
    let (mut tree, root) = prepared(Widget::transform(
        Transform::translation(Offset::new(4.0, 2.0)),
        child(),
    ));
    let before = tree.diagnostics();

    tree.update(
        root,
        Widget::transform(Transform::translation(Offset::new(18.0, 7.0)), child()),
    )
    .expect("update");
    tree.layout(frame());
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
    let (mut tree, root) = prepared(Widget::text_styled(
        "retained",
        style(Color::BLACK),
        TextAlign::Start,
    ));
    let before = tree.diagnostics();

    tree.update(
        root,
        Widget::text_styled("retained", style(Color::WHITE), TextAlign::Start),
    )
    .expect("update");
    tree.layout(frame());
    let _ = tree.paint();

    let after = tree.diagnostics();
    assert_eq!(after.layouts, before.layouts, "color does not reshape text");
    assert_eq!(after.paints - before.paints, 1, "text must repaint");
}

#[test]
fn text_metric_update_invalidates_layout_and_paint() {
    let (mut tree, root) = prepared(Widget::text_styled(
        "retained",
        TextStyle::default().font_size(18.0),
        TextAlign::Start,
    ));
    let before = tree.diagnostics();

    tree.update(
        root,
        Widget::text_styled(
            "retained",
            TextStyle::default().font_size(28.0),
            TextAlign::Start,
        ),
    )
    .expect("update");
    tree.layout(frame());
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
    tree.layout(frame());
    let _ = tree.paint();

    let after = tree.diagnostics();
    assert_eq!(
        after.layouts, before.layouts,
        "color does not change geometry"
    );
    assert_eq!(after.paints - before.paints, 1, "box must repaint");
}
