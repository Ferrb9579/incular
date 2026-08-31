//! Stack descriptor behavior tests.

use incular_config::{Alignment, Clip, StackFit, TextDirection};
use incular_widgets::{IndexedStack, Positioned, Stack, Text, Widget};

#[test]
fn stack_builder_defaults_match_default_and_lowering() {
    assert_eq!(Stack::builder().build(), Stack::default());

    let stack = Stack::builder()
        .children(vec![Text::new("child").into()])
        .alignment(Alignment::CENTER)
        .fit(StackFit::Expand)
        .clip_behavior(Clip::None)
        .text_direction(TextDirection::Rtl)
        .build();
    let expected = Stack::new([Text::new("child")])
        .alignment(Alignment::CENTER)
        .fit(StackFit::Expand)
        .clip_behavior(Clip::None)
        .text_direction(TextDirection::Rtl);
    assert_eq!(stack, expected);
    assert_eq!(Widget::from(stack).debug_type_name(), "Stack");
}

#[test]
fn positioned_builder_keeps_required_generic_child() {
    let positioned = Positioned::builder()
        .left(4.0)
        .child(Text::new("child"))
        .build();

    let expected = Positioned::new(Text::new("child")).left(4.0);
    assert_eq!(positioned, expected);
    assert_eq!(Widget::from(positioned).debug_type_name(), "Positioned");
}

#[test]
fn indexed_stack_builder_defaults_and_constructor_keep_children() {
    assert_eq!(IndexedStack::builder().build(), IndexedStack::default());

    let indexed = IndexedStack::new([Text::new("first")])
        .index(1)
        .alignment(Alignment::BOTTOM_RIGHT);
    let expected = IndexedStack::new([Text::new("first")])
        .index(1)
        .alignment(Alignment::BOTTOM_RIGHT);
    assert_eq!(indexed, expected);
    assert_eq!(Widget::from(indexed).debug_type_name(), "IndexedStack");
}
