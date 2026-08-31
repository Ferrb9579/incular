use incular_config::{Axis, EdgeInsets};
use incular_core::Color;
use incular_widgets::internal::{Placeholder, SplitView};
use incular_widgets::{
    ColoredBox, FittedBox, NavigationToolbar, OverflowBar, Padding, SizedBox, Text, Visibility,
    Widget,
};

#[test]
fn sized_box_builder_defaults_match_new() {
    assert_eq!(SizedBox::builder().build(), SizedBox::new());
}

#[test]
fn optional_child_builders_accept_widgets() {
    let child: Widget = Text::new("child").into();
    let sized_box = SizedBox::builder().child(child.clone()).build();
    assert_eq!(Widget::from(sized_box), child.clone());

    let colored_box = ColoredBox::builder()
        .color(Color::BLACK)
        .child(child.clone())
        .build();
    let lowered = Widget::from(colored_box);
    assert_eq!(lowered.debug_type_name(), "DecoratedBox");
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

    assert_eq!(Widget::from(overflow_bar).debug_type_name(), "Wrap");
    assert_eq!(
        Widget::from(OverflowBar::default()).debug_type_name(),
        "Wrap"
    );
}

#[test]
fn required_child_builder_preserves_existing_lowering() {
    let child: Widget = Text::new("child").into();
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
    let child: Widget = Text::new("child").into();

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
    let child: Widget = Text::new("child").into();
    let split = SplitView::builder()
        .axis(Axis::Horizontal)
        .first(child.clone())
        .second(child)
        .build();

    assert_eq!(Widget::from(split).debug_type_name(), "Flex");
}
