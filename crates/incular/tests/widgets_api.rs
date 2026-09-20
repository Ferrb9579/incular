//! Compile-family coverage for the public Flutter Widgets surface.
//!
//! This intentionally imports only the base facade prelude. Material controls
//! belong to `incular::material_prelude` and must not be required here.

use incular::prelude::*;

#[test]
fn canonical_widgets_compose_without_material() {
    let controller = TextEditingController::with_text("hello");
    let text: Widget = Text::new("hello").into();
    let editor: Widget = EditableText::new(controller).into();
    let content = Container::new().child(Column::new([text, editor]));
    let row = Row::new([Widget::from(content), SizedBox::shrink().into()]);

    let list = ListView::builder(3, |index| Text::new(format!("row {index}")));
    assert_eq!(list.get_scroll_direction(), Axis::Vertical);
    let page = PageView::builder(2, |index| Text::new(format!("page {index}")));
    assert_eq!(page.get_scroll_direction(), Axis::Horizontal);
    let single = SingleChildScrollView::new(row);
    assert_eq!(single.get_scroll_direction(), Axis::Vertical);

    let _root: Widget = Column::new([
        Widget::from(list),
        Widget::from(page),
        Widget::from(single),
        Widget::from(RawRadio::new("a", Some("a"))),
    ])
    .into();
}

#[test]
fn canonical_widget_defaults_are_explicit() {
    assert_eq!(
        SingleChildScrollView::new(Text::new("x")).get_scroll_direction(),
        Axis::Vertical
    );
    assert_eq!(
        ListView::new([Text::new("x")]).get_scroll_direction(),
        Axis::Vertical
    );
    assert_eq!(
        PageView::new([Text::new("x")]).get_scroll_direction(),
        Axis::Horizontal
    );
    let _ = FittedBox::new(Text::new("x")).fit(BoxFit::Contain);
}

#[test]
fn widget_is_only_the_opaque_composition_transport() {
    let root: Widget = Padding::all(12.0, Text::new("opaque")).into();
    assert_eq!(root.debug_type_name(), "Padding");
    assert!(root.key().is_none());
}
