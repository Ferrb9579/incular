//! Table descriptor behavior tests.

use incular_widgets::{Table, Text, Widget, internal::WidgetKind};

#[test]
fn table_builder_defaults_match_explicit_default() {
    assert_eq!(Table::builder().build(), Table::default());
}

#[test]
fn table_builder_preserves_normalization_and_lowering() {
    let table = Table::builder()
        .columns(0)
        .column_spacing(-3.0)
        .row_spacing(-5.0)
        .children(vec![Widget::text("a"), Widget::text("b")])
        .build();

    let WidgetKind::Table {
        columns,
        column_spacing,
        row_spacing,
        children,
    } = Widget::from(table).into_kind()
    else {
        panic!("expected Table widget kind")
    };

    assert_eq!(columns, 1);
    assert_eq!(column_spacing, 0.0);
    assert_eq!(row_spacing, 0.0);
    assert_eq!(children.len(), 2);
}

#[test]
fn constructor_accepts_arbitrary_widget_descriptors() {
    let table = Table::new(2, [Text::new("cell")]);
    let WidgetKind::Table {
        columns, children, ..
    } = Widget::from(table).into_kind()
    else {
        panic!("expected Table widget kind")
    };

    assert_eq!(columns, 2);
    assert_eq!(children.len(), 1);
}
