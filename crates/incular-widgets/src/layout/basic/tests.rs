use super::*;
use crate::Widget;
use incular_config::{Axis, EdgeInsets};
use incular_core::Color;

#[test]
fn sized_box_builder_defaults_match_new() {
    assert_eq!(SizedBox::builder().build(), SizedBox::new());
}

#[test]
fn optional_child_builders_accept_widgets() {
    let child = Widget::text("child");
    let sized_box = SizedBox::builder().child(child.clone()).build();
    assert_eq!(sized_box.child, Some(child.clone()));

    let colored_box = ColoredBox::builder()
        .color(Color::BLACK)
        .child(child.clone())
        .build();
    assert_eq!(colored_box.child, Some(child));
}

#[test]
fn multiple_children_use_vec_widget_storage() {
    let overflow_bar = OverflowBar::builder()
        .children([crate::Text::new("first"), crate::Text::new("second")])
        .spacing(4.0)
        .build();

    assert_eq!(
        overflow_bar.children,
        vec![Widget::text("first"), Widget::text("second")]
    );
    assert_eq!(OverflowBar::default().children, Vec::new());
}

#[test]
fn required_child_builder_preserves_existing_lowering() {
    let child = Widget::text("child");
    let padding = Padding::builder()
        .padding(EdgeInsets::all(4.0))
        .child(child.clone())
        .build();

    assert_eq!(
        Widget::from(padding),
        Widget::from(Padding::new(EdgeInsets::all(4.0), child,))
    );
}

#[test]
fn builder_defaults_match_constructor_defaults() {
    let child = Widget::text("child");

    let fitted = FittedBox::builder().child(child.clone()).build();
    let fitted_from_constructor = FittedBox::new(child.clone());
    assert_eq!(fitted, fitted_from_constructor);

    let visibility = Visibility::builder().child(child.clone()).build();
    let visibility_from_constructor = Visibility::new(child);
    assert_eq!(visibility, visibility_from_constructor);

    let placeholder = Placeholder::builder().build();
    assert_eq!(placeholder, Placeholder::new());

    let toolbar = NavigationToolbar::builder().build();
    assert_eq!(toolbar, NavigationToolbar::new());
}

#[test]
fn split_view_builder_keeps_callback_private_and_children_generic() {
    let child = Widget::text("child");
    let split = SplitView::builder()
        .axis(Axis::Horizontal)
        .first(child.clone())
        .second(child)
        .build();

    assert_eq!(split.position, SplitPosition::Fraction(0.5));
    assert_eq!(split.divider_hit_extent, 8.0);
    assert_eq!(split.divider_visual_extent, 1.0);
    assert!(split.on_split_changed.is_none());
}
