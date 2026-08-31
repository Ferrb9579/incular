use incular_config::{Axis, EdgeInsets};
use incular_core::Color;
use incular_widgets::internal::{Placeholder, SplitView, WidgetKind};
use incular_widgets::{
    ColoredBox, FittedBox, NavigationToolbar, OverflowBar, Padding, SizedBox, Visibility, Widget,
};

#[test]
fn sized_box_builder_defaults_match_new() {
    assert_eq!(SizedBox::builder().build(), SizedBox::new());
}

#[test]
fn optional_child_builders_accept_widgets() {
    let child = Widget::text("child");
    let sized_box = SizedBox::builder().child(child.clone()).build();
    assert_eq!(Widget::from(sized_box), child.clone());

    let colored_box = ColoredBox::builder()
        .color(Color::BLACK)
        .child(child.clone())
        .build();
    let WidgetKind::Decorated { child: lowered, .. } =
        Widget::from(colored_box).kind().clone().clone()
    else {
        panic!("expected ColoredBox to lower to a decorated widget");
    };
    assert_eq!(lowered, child);
}

#[test]
fn multiple_children_use_vec_widget_storage() {
    let overflow_bar = OverflowBar::builder()
        .children([
            incular_widgets::Text::new("first"),
            incular_widgets::Text::new("second"),
        ])
        .spacing(4.0)
        .build();

    let WidgetKind::Wrap { children, .. } = Widget::from(overflow_bar).kind().clone().clone()
    else {
        panic!("expected OverflowBar to lower to Wrap");
    };
    assert_eq!(children[0].text_if_any().as_deref(), Some("first"));
    assert_eq!(children[1].text_if_any().as_deref(), Some("second"));
    let WidgetKind::Wrap { children, .. } =
        Widget::from(OverflowBar::default()).kind().clone().clone()
    else {
        panic!("expected default OverflowBar to lower to Wrap");
    };
    assert!(children.is_empty());
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

    let WidgetKind::Flex { children, .. } = Widget::from(split).kind().clone().clone() else {
        panic!("expected SplitView to lower to a flex row");
    };
    assert_eq!(children.len(), 3);
    let WidgetKind::Flexible { child: first, .. } = &children[0].kind().clone().clone() else {
        panic!("expected the first pane to retain its flexible wrapper");
    };
    let WidgetKind::Flexible { child: second, .. } = &children[2].kind().clone().clone() else {
        panic!("expected the second pane to retain its flexible wrapper");
    };
    assert_eq!(first.text_if_any().as_deref(), Some("child"));
    assert_eq!(second.text_if_any().as_deref(), Some("child"));
}
