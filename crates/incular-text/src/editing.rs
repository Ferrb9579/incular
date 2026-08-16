//! Renderer-independent editing and selection state.

use crate::TextStyle;
use std::{cell::RefCell, fmt, ops::Range, rc::Rc};

/// Whether a caret is associated with the leading or trailing edge of a
/// bidirectional run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextAffinity {
    #[default]
    Downstream,
    Upstream,
}

/// A half-open UTF-8 byte range. Text engines use byte offsets so ranges can
/// be passed directly to shaping and grapheme segmentation without lossy
/// conversions. Call [`Self::clamp_to`] before slicing arbitrary user input.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextRange {
    pub start: usize,
    pub end: usize,
}

impl TextRange {
    pub const EMPTY: Self = Self { start: 0, end: 0 };

    #[must_use]
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    #[must_use]
    pub const fn collapsed(offset: usize) -> Self {
        Self::new(offset, offset)
    }

    #[must_use]
    pub const fn is_collapsed(self) -> bool {
        self.start == self.end
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.start <= self.end
    }

    #[must_use]
    pub const fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }

    #[must_use]
    pub const fn as_range(self) -> Range<usize> {
        self.start..self.end
    }

    /// Clamps the range to a string and moves each endpoint to a valid UTF-8
    /// boundary. This keeps editing operations panic-free for IME byte offsets.
    #[must_use]
    pub fn clamp_to(self, text: &str) -> Self {
        let start = nearest_char_boundary(text, self.start.min(text.len()));
        let end = nearest_char_boundary(text, self.end.min(text.len())).max(start);
        Self::new(start, end)
    }

    #[must_use]
    pub fn normalized(self) -> Self {
        if self.start <= self.end {
            self
        } else {
            Self::new(self.end, self.start)
        }
    }
}

/// A selection preserves directional base/extent while exposing a normalized
/// range for text replacement.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextSelection {
    pub base: usize,
    pub extent: usize,
}

impl TextSelection {
    #[must_use]
    pub const fn new(base: usize, extent: usize) -> Self {
        Self { base, extent }
    }

    #[must_use]
    pub const fn collapsed(offset: usize) -> Self {
        Self::new(offset, offset)
    }

    #[must_use]
    pub const fn start(self) -> usize {
        if self.base < self.extent {
            self.base
        } else {
            self.extent
        }
    }

    #[must_use]
    pub const fn end(self) -> usize {
        if self.base > self.extent {
            self.base
        } else {
            self.extent
        }
    }

    #[must_use]
    pub const fn is_collapsed(self) -> bool {
        self.base == self.extent
    }

    #[must_use]
    pub const fn is_forward(self) -> bool {
        self.base <= self.extent
    }

    #[must_use]
    pub const fn range(self) -> TextRange {
        TextRange::new(self.start(), self.end())
    }

    #[must_use]
    pub fn clamp_to(self, text: &str) -> Self {
        let base = nearest_char_boundary(text, self.base.min(text.len()));
        let extent = nearest_char_boundary(text, self.extent.min(text.len()));
        Self::new(base, extent)
    }
}

/// A composing/marked range supplied by an IME.
pub type ComposingRange = TextRange;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextEditingValue {
    pub text: String,
    pub selection: TextSelection,
    pub composing: Option<ComposingRange>,
}

impl TextEditingValue {
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let caret = text.len();
        Self {
            text,
            selection: TextSelection::collapsed(caret),
            composing: None,
        }
    }

    #[must_use]
    pub const fn empty() -> Self {
        Self {
            text: String::new(),
            selection: TextSelection::collapsed(0),
            composing: None,
        }
    }

    #[must_use]
    pub fn with_selection(mut self, selection: TextSelection) -> Self {
        self.selection = selection.clamp_to(&self.text);
        self
    }

    #[must_use]
    pub fn with_composing(mut self, composing: Option<ComposingRange>) -> Self {
        self.composing = composing.map(|range| range.clamp_to(&self.text));
        self
    }

    #[must_use]
    pub fn replace(mut self, range: TextRange, replacement: &str) -> Self {
        let range = range.normalized().clamp_to(&self.text);
        self.text.replace_range(range.as_range(), replacement);
        let caret = range.start + replacement.len();
        self.selection = TextSelection::collapsed(caret);
        self.composing = None;
        self
    }
}

impl Default for TextEditingValue {
    fn default() -> Self {
        Self::empty()
    }
}

/// An atomic editing operation, useful for integrating native IMEs and undo
/// stacks without coupling those systems to a widget.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextEditingDelta {
    pub range: TextRange,
    pub replacement: String,
    pub selection: Option<TextSelection>,
    pub composing: Option<ComposingRange>,
}

impl TextEditingDelta {
    #[must_use]
    pub fn replace(range: TextRange, replacement: impl Into<String>) -> Self {
        Self {
            range,
            replacement: replacement.into(),
            selection: None,
            composing: None,
        }
    }

    #[must_use]
    pub fn apply(&self, value: &TextEditingValue) -> TextEditingValue {
        let mut next = value.clone().replace(self.range, &self.replacement);
        if let Some(selection) = self.selection {
            next.selection = selection.clamp_to(&next.text);
        }
        next.composing = self.composing.map(|range| range.clamp_to(&next.text));
        next
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SelectionChangedCause {
    #[default]
    Unknown,
    Keyboard,
    Tap,
    DoubleTap,
    LongPress,
    Drag,
    Accessibility,
    Programmatic,
}

type Listener = Rc<dyn Fn(&TextEditingValue)>;

#[derive(Default)]
struct ControllerState {
    value: TextEditingValue,
    listeners: Vec<(usize, Listener)>,
    next_listener: usize,
}

/// Cloneable editing controller. It owns no platform resources; platform and
/// widget layers can subscribe to value changes and decide when to repaint.
#[derive(Clone, Default)]
pub struct TextEditingController {
    inner: Rc<RefCell<ControllerState>>,
}

impl fmt::Debug for TextEditingController {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TextEditingController")
            .field("value", &self.value())
            .finish()
    }
}

impl TextEditingController {
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self::from_value(TextEditingValue::new(text))
    }

    #[must_use]
    pub fn from_value(value: TextEditingValue) -> Self {
        Self {
            inner: Rc::new(RefCell::new(ControllerState {
                value,
                ..ControllerState::default()
            })),
        }
    }

    #[must_use]
    pub fn value(&self) -> TextEditingValue {
        self.inner.borrow().value.clone()
    }

    #[must_use]
    pub fn text(&self) -> String {
        self.inner.borrow().value.text.clone()
    }

    #[must_use]
    pub fn selection(&self) -> TextSelection {
        self.inner.borrow().value.selection
    }

    #[must_use]
    pub fn composing(&self) -> Option<ComposingRange> {
        self.inner.borrow().value.composing
    }

    pub fn set_value(&self, value: TextEditingValue) {
        self.update(value);
    }

    pub fn set_text(&self, text: impl Into<String>) {
        self.update(TextEditingValue::new(text));
    }

    pub fn set_selection(&self, selection: TextSelection) {
        let mut value = self.value();
        value.selection = selection.clamp_to(&value.text);
        self.update(value);
    }

    pub fn set_composing(&self, composing: Option<ComposingRange>) {
        let mut value = self.value();
        value.composing = composing.map(|range| range.clamp_to(&value.text));
        self.update(value);
    }

    /// Adds a listener and returns an opaque token accepted by
    /// [`Self::remove_listener`].
    pub fn add_listener(&self, listener: impl Fn(&TextEditingValue) + 'static) -> usize {
        let mut state = self.inner.borrow_mut();
        let token = state.next_listener;
        state.next_listener = state.next_listener.wrapping_add(1);
        state.listeners.push((token, Rc::new(listener)));
        token
    }

    pub fn remove_listener(&self, token: usize) -> bool {
        let mut state = self.inner.borrow_mut();
        let before = state.listeners.len();
        state.listeners.retain(|(id, _)| *id != token);
        before != state.listeners.len()
    }

    pub fn apply_delta(&self, delta: &TextEditingDelta) {
        let next = delta.apply(&self.value());
        self.update(next);
    }

    pub fn replace_selection(&self, replacement: &str) {
        let value = self.value();
        let range = value.selection.range();
        self.update(value.replace(range, replacement));
    }

    pub fn insert_text(&self, text: &str) {
        self.replace_selection(text);
    }

    pub fn delete_backward(&self) {
        let value = self.value();
        let range = if value.selection.is_collapsed() {
            previous_boundary(&value.text, value.selection.base)
                .map_or(TextRange::collapsed(value.selection.base), |start| {
                    TextRange::new(start, value.selection.base)
                })
        } else {
            value.selection.range()
        };
        self.update(value.replace(range, ""));
    }

    pub fn delete_forward(&self) {
        let value = self.value();
        let range = if value.selection.is_collapsed() {
            next_boundary(&value.text, value.selection.extent)
                .map_or(TextRange::collapsed(value.selection.extent), |end| {
                    TextRange::new(value.selection.extent, end)
                })
        } else {
            value.selection.range()
        };
        self.update(value.replace(range, ""));
    }

    fn update(&self, value: TextEditingValue) {
        let listeners = {
            let mut state = self.inner.borrow_mut();
            if state.value == value {
                return;
            }
            state.value = value;
            state
                .listeners
                .iter()
                .map(|(_, listener)| listener.clone())
                .collect::<Vec<_>>()
        };
        let value = self.value();
        for listener in listeners {
            listener(&value);
        }
    }
}

/// A renderer-neutral editable text description.
#[derive(Clone, Debug)]
pub struct EditableText {
    pub controller: TextEditingController,
    pub style: TextStyle,
    pub read_only: bool,
    pub obscure_text: bool,
    pub max_lines: Option<usize>,
}

impl EditableText {
    #[must_use]
    pub fn new(controller: TextEditingController) -> Self {
        Self {
            controller,
            style: TextStyle::default(),
            read_only: false,
            obscure_text: false,
            max_lines: Some(1),
        }
    }

    #[must_use]
    pub fn style(mut self, style: TextStyle) -> Self {
        self.style = style;
        self
    }

    #[must_use]
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    #[must_use]
    pub fn obscure_text(mut self, obscure_text: bool) -> Self {
        self.obscure_text = obscure_text;
        self
    }

    #[must_use]
    pub fn max_lines(mut self, max_lines: Option<usize>) -> Self {
        self.max_lines = max_lines;
        self
    }
}

/// Convenience text-field configuration built on [`EditableText`].
#[derive(Clone, Debug)]
pub struct TextField {
    pub editable: EditableText,
    pub placeholder: String,
}

impl TextField {
    #[must_use]
    pub fn new(controller: TextEditingController) -> Self {
        Self {
            editable: EditableText::new(controller),
            placeholder: String::new(),
        }
    }

    #[must_use]
    pub fn style(mut self, style: TextStyle) -> Self {
        self.editable = self.editable.style(style);
        self
    }

    #[must_use]
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    #[must_use]
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.editable = self.editable.read_only(read_only);
        self
    }
}

fn nearest_char_boundary(text: &str, offset: usize) -> usize {
    if text.is_char_boundary(offset) {
        return offset;
    }
    let mut boundary = offset;
    while boundary > 0 && !text.is_char_boundary(boundary) {
        boundary -= 1;
    }
    boundary
}

fn previous_boundary(text: &str, offset: usize) -> Option<usize> {
    let offset = nearest_char_boundary(text, offset.min(text.len()));
    (offset > 0).then(|| {
        text[..offset]
            .char_indices()
            .next_back()
            .map_or(0, |(index, _)| index)
    })
}

fn next_boundary(text: &str, offset: usize) -> Option<usize> {
    let offset = nearest_char_boundary(text, offset.min(text.len()));
    (offset < text.len()).then(|| {
        text[offset..]
            .chars()
            .next()
            .map_or(text.len(), |character| offset + character.len_utf8())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn selection_and_replacement_preserve_utf8_boundaries() {
        let value = TextEditingValue::new("a🙂b").with_selection(TextSelection::new(1, 5));
        assert_eq!(value.selection, TextSelection::new(1, 5));
        let range = value.selection.range();
        let replaced = value.replace(range, "x");
        assert_eq!(replaced.text, "axb");
        assert_eq!(replaced.selection, TextSelection::collapsed(2));
    }

    #[test]
    fn controller_notifies_listeners_and_applies_deltas() {
        let controller = TextEditingController::new("hello");
        let calls = Rc::new(Cell::new(0));
        let calls_for_listener = calls.clone();
        let token = controller.add_listener(move |_| {
            calls_for_listener.set(calls_for_listener.get() + 1);
        });
        controller.set_selection(TextSelection::new(0, 5));
        controller.apply_delta(&TextEditingDelta::replace(TextRange::new(0, 5), "world"));
        assert_eq!(controller.text(), "world");
        assert_eq!(calls.get(), 2);
        assert!(controller.remove_listener(token));
    }

    #[test]
    fn deletion_moves_by_codepoint_not_by_byte() {
        let controller = TextEditingController::new("a🙂");
        controller.delete_backward();
        assert_eq!(controller.text(), "a");
        controller.delete_backward();
        assert_eq!(controller.text(), "");
    }
}
