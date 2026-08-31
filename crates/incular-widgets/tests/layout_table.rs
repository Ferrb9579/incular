//! Table descriptor behavior tests.

use incular_widgets::{Table, Text, Widget};

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
        .children(vec![Text::new("a").into(), Text::new("b").into()])
        .build();
    let expected = Table::new(1, [Text::new("a"), Text::new("b")])
        .column_spacing(0.0)
        .row_spacing(0.0);
    assert_eq!(table, expected);
    assert_eq!(Widget::from(table).debug_type_name(), "Table");
}

#[test]
fn constructor_accepts_arbitrary_widget_descriptors() {
    let table = Table::new(2, [Text::new("cell")]);
    assert_eq!(Widget::from(table).debug_type_name(), "Table");
}
