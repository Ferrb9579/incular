//! Stack descriptor behavior tests.

use incular_config::{Alignment, Clip, StackFit, TextDirection};
use incular_widgets::{IndexedStack, Positioned, Stack, Text, Widget, internal::WidgetKind};

#[test]
fn stack_builder_defaults_match_default_and_lowering() {
    assert_eq!(Stack::builder().build(), Stack::default());

    let widget: Widget = Stack::builder()
        .children(vec![Text::new("child").into()])
        .alignment(Alignment::CENTER)
        .fit(StackFit::Expand)
        .clip_behavior(Clip::None)
        .text_direction(TextDirection::Rtl)
        .build()
        .into();

    let WidgetKind::Stack {
        alignment,
        text_direction,
        fit,
        clip_behavior,
        children,
    } = widget.into_kind()
    else {
        panic!("expected Stack widget kind")
    };

    assert_eq!(alignment, Alignment::CENTER);
    assert_eq!(text_direction, TextDirection::Rtl);
    assert_eq!(fit, StackFit::Expand);
    assert_eq!(clip_behavior, Clip::None);
    assert_eq!(children.len(), 1);
}

#[test]
fn positioned_builder_keeps_required_generic_child() {
    let positioned = Positioned::builder()
        .left(4.0)
        .child(Text::new("child"))
        .build();

    let WidgetKind::Positioned {
        left,
        top,
        right,
        bottom,
        width,
        height,
        child,
    } = Widget::from(positioned).into_kind()
    else {
        panic!("expected Positioned widget kind")
    };

    assert_eq!(left, Some(4.0));
    assert_eq!(top, None);
    assert_eq!(right, None);
    assert_eq!(bottom, None);
    assert_eq!(width, None);
    assert_eq!(height, None);
    assert_eq!(child.text_if_any().as_deref(), Some("child"));
}

#[test]
fn indexed_stack_builder_defaults_and_constructor_keep_children() {
    assert_eq!(IndexedStack::builder().build(), IndexedStack::default());

    let indexed = IndexedStack::new([Text::new("first")])
        .index(1)
        .alignment(Alignment::BOTTOM_RIGHT);
    let WidgetKind::IndexedStack {
        alignment,
        index,
        children,
    } = Widget::from(indexed).into_kind()
    else {
        panic!("expected IndexedStack widget kind")
    };

    assert_eq!(alignment, Alignment::BOTTOM_RIGHT);
    assert_eq!(index, 1);
    assert_eq!(children.len(), 1);
}
