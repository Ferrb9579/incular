//! Plain and rich text descriptions.

use crate::{
    TextAlign, TextEngine, TextLayout, TextLayoutOptions, TextOverflow, TextScaler, TextStyle,
};
use incular_core::Size;
use std::{fmt, ops::Range, sync::Arc};

/// A widget-independent inline child. The text crate stores a stable identity
/// and measured size rather than depending on `incular-widgets`, which keeps
/// the crate graph acyclic. Widgets can resolve the identity during layout.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WidgetSpan {
    pub id: u64,
    pub size: Size,
    pub alignment: WidgetSpanAlignment,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum WidgetSpanAlignment {
    #[default]
    Baseline,
    AboveBaseline,
    BelowBaseline,
    Middle,
    TextTop,
    TextBottom,
}

impl WidgetSpan {
    #[must_use]
    pub const fn new(id: u64) -> Self {
        Self {
            id,
            size: Size::ZERO,
            alignment: WidgetSpanAlignment::Baseline,
        }
    }

    #[must_use]
    pub const fn sized(id: u64, size: Size) -> Self {
        Self {
            id,
            size,
            alignment: WidgetSpanAlignment::Baseline,
        }
    }

    #[must_use]
    pub const fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    #[must_use]
    pub const fn alignment(mut self, alignment: WidgetSpanAlignment) -> Self {
        self.alignment = alignment;
        self
    }
}

/// A node in a rich text tree.
#[derive(Clone, Debug, PartialEq)]
pub enum InlineSpan {
    Text(TextSpan),
    Widget(WidgetSpan),
}

impl From<TextSpan> for InlineSpan {
    fn from(value: TextSpan) -> Self {
        Self::Text(value)
    }
}

impl From<WidgetSpan> for InlineSpan {
    fn from(value: WidgetSpan) -> Self {
        Self::Widget(value)
    }
}

/// A recursively nestable text node. Styles are inherited by descendants;
/// inline widget spans occupy one object-replacement character in plain text.
#[derive(Clone, Debug, PartialEq)]
pub struct TextSpan {
    pub text: String,
    pub style: Option<TextStyle>,
    pub children: Vec<InlineSpan>,
}

impl TextSpan {
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: None,
            children: Vec::new(),
        }
    }

    #[must_use]
    pub fn empty() -> Self {
        Self::new("")
    }

    #[must_use]
    pub fn style(mut self, style: TextStyle) -> Self {
        self.style = Some(style);
        self
    }

    #[must_use]
    pub fn child(mut self, child: impl Into<InlineSpan>) -> Self {
        self.children.push(child.into());
        self
    }

    #[must_use]
    pub fn children<I, S>(mut self, children: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<InlineSpan>,
    {
        self.children = children.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn with_children<I, S>(text: impl Into<String>, children: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<InlineSpan>,
    {
        Self::new(text).children(children)
    }

    #[must_use]
    pub fn plain_text(&self) -> String {
        let mut output = String::new();
        self.write_plain_text(&mut output);
        output
    }

    #[must_use]
    pub fn text_len(&self) -> usize {
        self.plain_text().len()
    }

    /// Returns leaf text runs with inherited styles and byte ranges into the
    /// flattened plain text. Widget spans are represented by `\u{fffc}`.
    #[must_use]
    pub fn flatten(&self, inherited: &TextStyle) -> Vec<StyledTextRun> {
        let mut runs = Vec::new();
        let mut offset = 0;
        self.flatten_into(inherited, &mut offset, &mut runs);
        runs
    }

    pub fn visit(&self, visitor: &mut impl TextSpanVisitor) {
        self.visit_with(&TextStyle::default(), visitor);
    }

    fn write_plain_text(&self, output: &mut String) {
        output.push_str(&self.text);
        for child in &self.children {
            match child {
                InlineSpan::Text(span) => span.write_plain_text(output),
                InlineSpan::Widget(_) => output.push('\u{fffc}'),
            }
        }
    }

    fn flatten_into(
        &self,
        inherited: &TextStyle,
        offset: &mut usize,
        runs: &mut Vec<StyledTextRun>,
    ) {
        let style = self
            .style
            .as_ref()
            .map_or_else(|| inherited.clone(), |local| inherited.merge(local));
        if !self.text.is_empty() {
            let start = *offset;
            *offset += self.text.len();
            runs.push(StyledTextRun {
                text: self.text.clone(),
                style: style.clone(),
                range: start..*offset,
            });
        }
        for child in &self.children {
            match child {
                InlineSpan::Text(span) => span.flatten_into(&style, offset, runs),
                InlineSpan::Widget(widget) => {
                    let start = *offset;
                    *offset += '\u{fffc}'.len_utf8();
                    runs.push(StyledTextRun {
                        text: '\u{fffc}'.to_string(),
                        style: style.clone(),
                        range: start..*offset,
                    });
                    visitor_placeholder(widget, &style, start..*offset, runs);
                }
            }
        }
    }

    fn visit_with(&self, inherited: &TextStyle, visitor: &mut impl TextSpanVisitor) {
        let style = self
            .style
            .as_ref()
            .map_or_else(|| inherited.clone(), |local| inherited.merge(local));
        if !self.text.is_empty() {
            visitor.visit_text(&self.text, &style);
        }
        for child in &self.children {
            match child {
                InlineSpan::Text(span) => span.visit_with(&style, visitor),
                InlineSpan::Widget(widget) => visitor.visit_widget(*widget, &style),
            }
        }
    }
}

/// A flattened leaf with inherited style information.
#[derive(Clone, Debug, PartialEq)]
pub struct StyledTextRun {
    pub text: String,
    pub style: TextStyle,
    pub range: Range<usize>,
}

/// Visitor for rich text leaves. Both methods have defaults so callers can
/// collect only the kind of leaf they need.
pub trait TextSpanVisitor {
    fn visit_text(&mut self, _text: &str, _style: &TextStyle) {}
    fn visit_widget(&mut self, _widget: WidgetSpan, _style: &TextStyle) {}
}

// A widget leaf is already represented by a StyledTextRun. This helper keeps
// the flattening loop explicit without requiring a second public run variant.
fn visitor_placeholder(
    _widget: &WidgetSpan,
    _style: &TextStyle,
    _range: Range<usize>,
    _runs: &mut Vec<StyledTextRun>,
) {
}

/// A rich paragraph with paragraph-level alignment and scaling.
#[derive(Clone, Debug, PartialEq)]
pub struct RichText {
    pub text: TextSpan,
    pub text_align: TextAlign,
    pub text_scaler: TextScaler,
    pub max_lines: Option<usize>,
    pub soft_wrap: bool,
    pub overflow: TextOverflow,
}

impl RichText {
    #[must_use]
    pub fn new(text: TextSpan) -> Self {
        Self {
            text,
            text_align: TextAlign::Start,
            text_scaler: TextScaler::default(),
            max_lines: None,
            soft_wrap: true,
            overflow: TextOverflow::Clip,
        }
    }

    #[must_use]
    pub fn text_align(mut self, text_align: TextAlign) -> Self {
        self.text_align = text_align;
        self
    }

    #[must_use]
    pub fn align(self, text_align: TextAlign) -> Self {
        self.text_align(text_align)
    }

    #[must_use]
    pub fn text_scaler(mut self, text_scaler: TextScaler) -> Self {
        self.text_scaler = text_scaler;
        self
    }

    #[must_use]
    pub fn max_lines(mut self, max_lines: Option<usize>) -> Self {
        self.max_lines = max_lines;
        self
    }

    #[must_use]
    pub fn soft_wrap(mut self, soft_wrap: bool) -> Self {
        self.soft_wrap = soft_wrap;
        self
    }

    #[must_use]
    pub fn overflow(mut self, overflow: TextOverflow) -> Self {
        self.overflow = overflow;
        self
    }

    #[must_use]
    pub fn plain_text(&self) -> String {
        self.text.plain_text()
    }

    #[must_use]
    pub fn flatten(&self) -> Vec<StyledTextRun> {
        self.text
            .flatten(&TextStyle::default().scaled(self.text_scaler))
    }

    #[must_use]
    pub fn layout(&self, engine: &mut TextEngine, max_width: Option<f32>) -> Arc<TextLayout> {
        let style = self
            .flatten()
            .first()
            .map_or_else(TextStyle::default, |run| run.style.clone());
        engine.layout_with_options(
            &self.plain_text(),
            &style,
            TextLayoutOptions::new(max_width, self.text_align)
                .soft_wrap(self.soft_wrap)
                .max_lines(self.max_lines)
                .overflow(self.overflow),
        )
    }
}

/// A plain paragraph description. Rendering code can turn it into a
/// [`TextSpan`] without losing the familiar `Text::new("...")` API.
#[derive(Clone, Debug, PartialEq)]
pub struct Text {
    pub text: String,
    pub style: TextStyle,
    pub text_align: TextAlign,
    pub text_scaler: TextScaler,
    pub max_lines: Option<usize>,
    pub soft_wrap: bool,
    pub overflow: TextOverflow,
}

impl Text {
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: TextStyle::default(),
            text_align: TextAlign::Start,
            text_scaler: TextScaler::default(),
            max_lines: None,
            soft_wrap: true,
            overflow: TextOverflow::Clip,
        }
    }

    #[must_use]
    pub fn style(mut self, style: TextStyle) -> Self {
        self.style = style;
        self
    }

    #[must_use]
    pub fn align(mut self, align: TextAlign) -> Self {
        self.text_align = align;
        self
    }

    #[must_use]
    pub fn text_align(self, align: TextAlign) -> Self {
        self.align(align)
    }

    #[must_use]
    pub fn text_scaler(mut self, scaler: TextScaler) -> Self {
        self.text_scaler = scaler;
        self
    }

    #[must_use]
    pub fn max_lines(mut self, max_lines: Option<usize>) -> Self {
        self.max_lines = max_lines;
        self
    }

    #[must_use]
    pub fn soft_wrap(mut self, soft_wrap: bool) -> Self {
        self.soft_wrap = soft_wrap;
        self
    }

    #[must_use]
    pub fn overflow(mut self, overflow: TextOverflow) -> Self {
        self.overflow = overflow;
        self
    }

    #[must_use]
    pub fn span(&self) -> TextSpan {
        TextSpan::new(self.text.clone()).style(self.style.clone())
    }

    #[must_use]
    pub fn rich_text(&self) -> RichText {
        RichText::new(self.span())
            .align(self.text_align)
            .text_scaler(self.text_scaler)
            .max_lines(self.max_lines)
            .soft_wrap(self.soft_wrap)
            .overflow(self.overflow)
    }

    #[must_use]
    pub fn layout(&self, engine: &mut TextEngine, max_width: Option<f32>) -> Arc<TextLayout> {
        engine.layout_with_options(
            &self.text,
            &self.style.scaled(self.text_scaler),
            TextLayoutOptions::new(max_width, self.text_align)
                .soft_wrap(self.soft_wrap)
                .max_lines(self.max_lines)
                .overflow(self.overflow),
        )
    }
}

impl From<&str> for Text {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for Text {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for Text {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use incular_core::Size;

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
}
