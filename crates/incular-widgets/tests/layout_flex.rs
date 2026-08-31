//! Flex descriptor behavior tests.

use incular_config::{
    Axis, CrossAxisAlignment, FlexFit, MainAxisAlignment, MainAxisSize, TextDirection,
    VerticalDirection,
};
use incular_widgets::{
    Column, Expanded, Flex, Flexible, Row, SizedBox, Spacer, Widget, internal::WidgetKind,
};

#[test]
fn builders_match_explicit_defaults() {
    assert_eq!(Row::builder().build(), Row::default());
    assert_eq!(Column::builder().build(), Column::default());
    assert_eq!(Flex::builder().build(), Flex::default());
    assert_eq!(Spacer::builder().build(), Spacer::default());

    let child = Widget::text("child");
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
        .children(vec![Widget::text("first"), Widget::text("second")])
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
fn builder_preserves_flex_lowering() {
    let row = Row::builder()
        .children([Widget::text("first"), Widget::text("second")])
        .main_axis_alignment(MainAxisAlignment::SpaceEvenly)
        .main_axis_size(MainAxisSize::Min)
        .cross_axis_alignment(CrossAxisAlignment::End)
        .text_direction(TextDirection::Rtl)
        .vertical_direction(VerticalDirection::Up)
        .spacing(12.0)
        .build();

    match Widget::from(row).kind().clone().clone() {
        WidgetKind::Flex {
            axis,
            main_axis_alignment,
            main_axis_size,
            cross_axis_alignment,
            text_direction,
            vertical_direction,
            spacing,
            children,
        } => {
            assert_eq!(axis, Axis::Horizontal);
            assert_eq!(main_axis_alignment, MainAxisAlignment::SpaceEvenly);
            assert_eq!(main_axis_size, MainAxisSize::Min);
            assert_eq!(cross_axis_alignment, CrossAxisAlignment::End);
            assert_eq!(text_direction, TextDirection::Rtl);
            assert_eq!(vertical_direction, VerticalDirection::Up);
            assert_eq!(spacing, 12.0);
            assert_eq!(children.len(), 2);
        }
        _ => panic!("Row did not lower to a flex widget"),
    }

    let flex = Flex::builder()
        .direction(Axis::Vertical)
        .children([Widget::text("child")])
        .build();
    match Widget::from(flex).kind().clone().clone() {
        WidgetKind::Flex { axis, children, .. } => {
            assert_eq!(axis, Axis::Vertical);
            assert_eq!(children.len(), 1);
        }
        _ => panic!("Flex did not lower to a flex widget"),
    }
}

#[test]
fn flexible_and_expanded_preserve_lowering() {
    let flexible = Flexible::builder()
        .child(Widget::text("flexible"))
        .flex(3)
        .fit(FlexFit::Loose)
        .build();
    match Widget::from(flexible).kind().clone().clone() {
        WidgetKind::Flexible { flex, fit, .. } => {
            assert_eq!(flex, 3);
            assert_eq!(fit, FlexFit::Loose);
        }
        _ => panic!("Flexible did not lower to a flexible widget"),
    }

    let expanded = Expanded::builder()
        .child(Widget::text("expanded"))
        .flex(2)
        .build();
    match Widget::from(expanded).kind().clone().clone() {
        WidgetKind::Flexible { flex, fit, .. } => {
            assert_eq!(flex, 2);
            assert_eq!(fit, FlexFit::Tight);
        }
        _ => panic!("Expanded did not lower to a flexible widget"),
    }
}
