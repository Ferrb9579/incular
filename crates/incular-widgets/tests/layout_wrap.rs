//! Wrap descriptor behavior tests.

use incular_config::{Axis, TextDirection, VerticalDirection, WrapAlignment, WrapCrossAlignment};
use incular_widgets::{Widget, Wrap, internal::WidgetKind};

#[test]
fn wrap_builder_defaults_match_default() {
    assert_eq!(Wrap::builder().build(), Wrap::default());
}

#[test]
fn wrap_builder_preserves_normalization_and_lowering() {
    let wrap = Wrap::builder()
        .children(vec![Widget::text("child")])
        .direction(Axis::Vertical)
        .alignment(WrapAlignment::Center)
        .spacing(-2.0)
        .run_alignment(WrapAlignment::SpaceBetween)
        .run_spacing(-4.0)
        .cross_axis_alignment(WrapCrossAlignment::End)
        .text_direction(TextDirection::Rtl)
        .vertical_direction(VerticalDirection::Up)
        .build();

    let WidgetKind::Wrap {
        axis,
        alignment,
        spacing,
        run_alignment,
        run_spacing,
        cross_axis_alignment,
        text_direction,
        vertical_direction,
        children,
    } = Widget::from(wrap).into_kind()
    else {
        panic!("expected Wrap widget kind")
    };

    assert_eq!(axis, Axis::Vertical);
    assert_eq!(alignment, WrapAlignment::Center);
    assert_eq!(spacing, 0.0);
    assert_eq!(run_alignment, WrapAlignment::SpaceBetween);
    assert_eq!(run_spacing, 0.0);
    assert_eq!(cross_axis_alignment, WrapCrossAlignment::End);
    assert_eq!(text_direction, TextDirection::Rtl);
    assert_eq!(vertical_direction, VerticalDirection::Up);
    assert_eq!(children.len(), 1);
}

#[test]
fn constructor_accepts_arbitrary_widget_descriptors() {
    let wrap = Wrap::new([incular_widgets::Text::new("child")]);
    let WidgetKind::Wrap { children, .. } = Widget::from(wrap).into_kind() else {
        panic!("expected Wrap widget kind")
    };

    assert_eq!(children.len(), 1);
}
