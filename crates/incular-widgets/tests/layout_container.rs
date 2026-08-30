//! Container descriptor behavior tests.

use incular_config::{Clip, EdgeInsets};
use incular_core::{Color, Size, Transform};
use incular_widgets::{Container, DecoratedBox, Text, Widget, internal::WidgetKind};

#[test]
fn constructors_and_empty_builder_share_the_same_defaults() {
    let default = Container::default();

    assert_eq!(Container::new(), default);
    assert_eq!(Container::empty(), default);
    assert_eq!(Container::builder().build(), default);
    assert_eq!(default.child, None);
    assert_eq!(default.alignment, None);
    assert_eq!(default.padding, None);
    assert_eq!(default.margin, None);
    assert_eq!(default.color, None);
    assert_eq!(default.background, None);
    assert_eq!(default.border, None);
    assert_eq!(default.radius, incular_rendering::CornerRadii::default());
    assert_eq!(default.constraints, None);
    assert_eq!(default.width, None);
    assert_eq!(default.height, None);
    assert_eq!(default.transform, None);
    assert_eq!(default.clip_behavior, Clip::default());
}

#[test]
fn builder_accepts_text_and_existing_widget_values() {
    let text = Container::builder().child(Text::new("hello")).build();
    assert_eq!(
        text.child.as_ref().and_then(Widget::text_if_any),
        Some(String::from("hello"))
    );

    let widget = Widget::box_(Size::ZERO, Color::TRANSPARENT);
    let existing_widget = Container::builder().child(widget.clone()).build();
    assert_eq!(existing_widget.child, Some(widget));
}

#[test]
fn builder_accepts_arbitrary_into_widget_children() {
    let decorated = DecoratedBox::new(Text::new("decorated"));
    let container = Container::builder().child(decorated).build();

    assert!(container.child.is_some());
}

#[test]
fn struct_literals_require_explicit_widget_conversion() {
    let container = Container {
        child: Some(Text::new("literal").into()),
        ..Default::default()
    };

    assert_eq!(
        container.child.as_ref().and_then(Widget::text_if_any),
        Some(String::from("literal"))
    );
}

#[test]
fn fluent_child_apis_accept_widgets() {
    let with_child = Container::with_child(Text::new("fluent"));
    let fluent = Container::empty().child(Text::new("fluent"));

    assert_eq!(
        with_child.child.as_ref().and_then(Widget::text_if_any),
        Some(String::from("fluent"))
    );
    assert_eq!(
        fluent.child.as_ref().and_then(Widget::text_if_any),
        Some(String::from("fluent"))
    );
}

#[test]
fn lowering_preserves_the_outer_wrapper_order() {
    let decorated: Widget = Container::builder()
        .child(Text::new("child"))
        .padding(EdgeInsets::all(4.0))
        .color(Color::WHITE)
        .clip_behavior(Clip::None)
        .build()
        .into();
    assert!(matches!(decorated.kind, WidgetKind::Decorated { .. }));

    let constrained: Widget = Container::builder()
        .child(Text::new("child"))
        .padding(EdgeInsets::all(4.0))
        .width(120.0)
        .clip_behavior(Clip::None)
        .build()
        .into();
    assert!(matches!(constrained.kind, WidgetKind::Constrained { .. }));

    let margin: Widget = Container::builder()
        .child(Text::new("child"))
        .width(120.0)
        .margin(EdgeInsets::all(8.0))
        .clip_behavior(Clip::None)
        .build()
        .into();
    assert!(matches!(margin.kind, WidgetKind::Padding { .. }));

    let transformed: Widget = Container::builder()
        .child(Text::new("child"))
        .margin(EdgeInsets::all(8.0))
        .transform(Transform::IDENTITY)
        .clip_behavior(Clip::None)
        .build()
        .into();
    assert!(matches!(transformed.kind, WidgetKind::Transform { .. }));

    let clipped: Widget = Container::builder()
        .child(Text::new("child"))
        .clip_behavior(Clip::AntiAlias)
        .build()
        .into();
    assert!(matches!(clipped.kind, WidgetKind::ClipRect { .. }));

    let rounded: Widget = Container::builder()
        .child(Text::new("child"))
        .radius(incular_rendering::CornerRadii::uniform(8.0))
        .clip_behavior(Clip::AntiAlias)
        .build()
        .into();
    assert!(matches!(rounded.kind, WidgetKind::ClipRRect { .. }));
}

#[test]
fn lowering_without_clipping_keeps_content_sized_child_path() {
    let widget: Widget = Container::builder()
        .child(Text::new("content"))
        .clip_behavior(Clip::None)
        .build()
        .into();

    assert!(matches!(widget.kind, WidgetKind::Text { .. }));
}

#[test]
fn empty_container_lowering_keeps_fallback_alignment_behavior() {
    let widget: Widget = Container::builder()
        .clip_behavior(Clip::None)
        .build()
        .into();

    assert!(matches!(widget.kind, WidgetKind::Align { .. }));
}
