//! Flex descriptor behavior tests.

use incular_config::{
    Axis, CrossAxisAlignment, FlexFit, MainAxisAlignment, MainAxisSize, TextDirection,
    VerticalDirection,
};
use incular_widgets::{Column, Expanded, Flex, Flexible, Row, SizedBox, Spacer, Text, Widget};

#[test]
fn builders_match_explicit_defaults() {
    assert_eq!(Row::builder().build(), Row::default());
    assert_eq!(Column::builder().build(), Column::default());
    assert_eq!(Flex::builder().build(), Flex::default());
    assert_eq!(Spacer::builder().build(), Spacer::default());

    let child: Widget = Text::new("child").into();
    assert_eq!(
        Flexible::builder().child(child.clone()).build(),
        Flexible::new(child.clone())
    );
    assert_eq!(
        Expanded::builder().child(child.clone()).build(),
        Expanded::new(child)
    );
}

#[test]
fn builders_convert_widget_children_and_normalize_values() {
    let row = Row::builder()
        .children([SizedBox::shrink(), SizedBox::shrink()])
        .spacing(-8.0)
        .build();
    assert_eq!(row.children.len(), 2);
    assert_eq!(row.spacing, 0.0);

    let column = Column::builder()
        .children([Text::new("first"), Text::new("second")])
        .build();
    assert_eq!(column.children.len(), 2);

    let flexible = Flexible::builder()
        .child(SizedBox::shrink())
        .flex(0)
        .fit(FlexFit::Tight)
        .build();
    assert_eq!(flexible.flex, 1);
    assert_eq!(flexible.fit, FlexFit::Tight);

    let expanded = Expanded::builder()
        .child(SizedBox::shrink())
        .flex(0)
        .build();
    assert_eq!(expanded.flex, 1);

    assert_eq!(Spacer::builder().flex(0).build().flex, 1);
}

#[test]
fn builder_preserves_flex_configuration() {
    let row = Row::builder()
        .children([Text::new("first"), Text::new("second")])
        .main_axis_alignment(MainAxisAlignment::SpaceEvenly)
        .main_axis_size(MainAxisSize::Min)
        .cross_axis_alignment(CrossAxisAlignment::End)
        .text_direction(TextDirection::Rtl)
        .vertical_direction(VerticalDirection::Up)
        .spacing(12.0)
        .build();

    assert_eq!(row.main_axis_alignment, MainAxisAlignment::SpaceEvenly);
    assert_eq!(row.main_axis_size, MainAxisSize::Min);
    assert_eq!(row.cross_axis_alignment, CrossAxisAlignment::End);
    assert_eq!(row.text_direction, Some(TextDirection::Rtl));
    assert_eq!(row.vertical_direction, VerticalDirection::Up);
    assert_eq!(row.spacing, 12.0);
    assert_eq!(row.children.len(), 2);
    let _: Widget = row.into();

    let flex = Flex::builder()
        .direction(Axis::Vertical)
        .children([Text::new("child")])
        .build();
    assert_eq!(flex.direction, Axis::Vertical);
    assert_eq!(flex.children.len(), 1);
    let _: Widget = flex.into();
}

#[test]
fn flexible_and_expanded_preserve_lowering() {
    let flexible = Flexible::builder()
        .child(Text::new("flexible"))
        .flex(3)
        .fit(FlexFit::Loose)
        .build();
    assert_eq!(flexible.flex, 3);
    assert_eq!(flexible.fit, FlexFit::Loose);
    assert_eq!(Widget::from(flexible).debug_type_name(), "Flexible");

    let expanded = Expanded::builder()
        .child(Text::new("expanded"))
        .flex(2)
        .build();
    assert_eq!(expanded.flex, 2);
    assert_eq!(Widget::from(expanded).debug_type_name(), "Flexible");
}
