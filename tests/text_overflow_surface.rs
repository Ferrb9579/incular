use incular::prelude::*;

#[test]
fn public_text_and_rich_text_overflow_configuration_mounts_retained_widgets() {
    let text: Widget = Text::new("one two three four five")
        .max_lines(Some(1))
        .overflow(TextOverflow::Ellipsis)
        .soft_wrap(true)
        .into();
    let rich: Widget = RichText::new(TextSpan::new("עברית mixed text"))
        .max_lines(Some(1))
        .overflow(TextOverflow::Clip)
        .into();

    let mut tree = incular::widgets::internal::WidgetTree::new();
    tree.mount(Widget::column(vec![text, rich])).unwrap();
    tree.layout(incular::config::Constraints::tight(Size::new(80., 60.)))
        .expect("layout");
}
