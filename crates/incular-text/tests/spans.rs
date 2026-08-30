use incular_core::Size;
use incular_text::{Text, TextAlign, TextSpan, TextSpanVisitor, TextStyle, WidgetSpan};

struct Collector {
    text: String,
    widgets: usize,
}

impl TextSpanVisitor for Collector {
    fn visit_text(&mut self, text: &str, _style: &TextStyle) {
        self.text.push_str(text);
    }

    fn visit_widget(&mut self, _widget: WidgetSpan, _style: &TextStyle) {
        self.widgets += 1;
    }
}

#[test]
fn rich_tree_flattens_in_order_and_inherits_style() {
    let child = TextSpan::new("world").style(TextStyle::default().font_size(24.0));
    let root = TextSpan::new("hello ").child(child);
    assert_eq!(root.plain_text(), "hello world");
    let runs = root.flatten(&TextStyle::default());
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[1].style.size, 24.0);
    assert_eq!(runs[1].range, 6..11);
}

#[test]
fn widget_span_is_an_object_replacement_character() {
    let widget = WidgetSpan::sized(7, Size::new(10.0, 12.0));
    let root = TextSpan::new("a").child(widget).child(TextSpan::new("b"));
    assert_eq!(root.plain_text(), "a\u{fffc}b");
    let mut collector = Collector {
        text: String::new(),
        widgets: 0,
    };
    root.visit(&mut collector);
    assert_eq!(collector.text, "ab");
    assert_eq!(collector.widgets, 1);
}

#[test]
fn plain_text_converts_to_rich_text() {
    let text = Text::new("hello").align(TextAlign::Center);
    let rich = text.rich_text();
    assert_eq!(rich.plain_text(), "hello");
    assert_eq!(rich.text_align, TextAlign::Center);
}
