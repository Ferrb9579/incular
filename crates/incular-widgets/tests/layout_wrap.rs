//! Wrap descriptor behavior tests.

use incular_config::{Axis, TextDirection, VerticalDirection, WrapAlignment, WrapCrossAlignment};
use incular_widgets::{Text, Widget, Wrap};

#[test]
fn wrap_builder_defaults_match_default() {
    assert_eq!(Wrap::builder().build(), Wrap::default());
}

#[test]
fn wrap_builder_preserves_normalization_and_lowering() {
    let wrap = Wrap::builder()
        .children(vec![Text::new("child").into()])
        .direction(Axis::Vertical)
        .alignment(WrapAlignment::Center)
        .spacing(-2.0)
        .run_alignment(WrapAlignment::SpaceBetween)
        .run_spacing(-4.0)
        .cross_axis_alignment(WrapCrossAlignment::End)
        .text_direction(TextDirection::Rtl)
        .vertical_direction(VerticalDirection::Up)
        .build();

    let expected = Wrap::new([Text::new("child")])
        .direction(Axis::Vertical)
        .alignment(WrapAlignment::Center)
        .spacing(0.0)
        .run_alignment(WrapAlignment::SpaceBetween)
        .run_spacing(0.0)
        .cross_axis_alignment(WrapCrossAlignment::End)
        .text_direction(TextDirection::Rtl)
        .vertical_direction(VerticalDirection::Up);
    assert_eq!(wrap, expected);
    assert_eq!(Widget::from(wrap).debug_type_name(), "Wrap");
}

#[test]
fn constructor_accepts_arbitrary_widget_descriptors() {
    let wrap = Wrap::new([incular_widgets::Text::new("child")]);
    assert_eq!(Widget::from(wrap).debug_type_name(), "Wrap");
}
