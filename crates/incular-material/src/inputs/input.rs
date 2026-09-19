//! Material input decoration and input-border primitives.

use incular_config::{Constraints, EdgeInsets};
use incular_controls::TextFieldStyle;
use incular_core::{Color, Size};
use incular_text::TextStyle;
use incular_widgets::{Border, BorderRadius, BoxDecoration, Container, Row, Text, Widget};
use typed_builder::TypedBuilder;

/// Full Material input-decoration configuration.
#[derive(Clone, Debug, Default, PartialEq, TypedBuilder)]
#[builder(field_defaults(default, setter(strip_option, into)))]
pub struct InputDecorationThemeData {
    pub label_style: Option<TextStyle>,
    pub floating_label_style: Option<TextStyle>,
    pub hint_style: Option<TextStyle>,
    #[builder(default, setter(!strip_option, transform = |value: usize| Some(value.max(1))))]
    pub hint_max_lines: Option<usize>,
    pub helper_style: Option<TextStyle>,
    #[builder(default, setter(!strip_option, transform = |value: usize| Some(value.max(1))))]
    pub helper_max_lines: Option<usize>,
    pub error_style: Option<TextStyle>,
    #[builder(default, setter(!strip_option, transform = |value: usize| Some(value.max(1))))]
    pub error_max_lines: Option<usize>,
    pub counter_style: Option<TextStyle>,
    pub prefix_style: Option<TextStyle>,
    pub suffix_style: Option<TextStyle>,
    pub floating_label_behavior: Option<FloatingLabelBehavior>,
    pub floating_label_alignment: Option<FloatingLabelAlignment>,
    pub is_dense: Option<bool>,
    pub is_collapsed: Option<bool>,
    pub align_label_with_hint: Option<bool>,
    pub content_padding: Option<EdgeInsets>,
    pub filled: Option<bool>,
    pub fill_color: Option<Color>,
    pub hover_color: Option<Color>,
    pub border: Option<InputBorder>,
    pub enabled_border: Option<InputBorder>,
    pub focused_border: Option<InputBorder>,
    pub error_border: Option<InputBorder>,
    pub focused_error_border: Option<InputBorder>,
    pub disabled_border: Option<InputBorder>,
    pub constraints: Option<Size>,
}

impl InputDecorationThemeData {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn label_style(mut self, value: TextStyle) -> Self {
        self.label_style = Some(value);
        self
    }

    #[must_use]
    pub fn floating_label_style(mut self, value: TextStyle) -> Self {
        self.floating_label_style = Some(value);
        self
    }

    #[must_use]
    pub fn hint_style(mut self, value: TextStyle) -> Self {
        self.hint_style = Some(value);
        self
    }

    #[must_use]
    pub fn hint_max_lines(mut self, value: usize) -> Self {
        self.hint_max_lines = Some(value.max(1));
        self
    }

    #[must_use]
    pub fn helper_style(mut self, value: TextStyle) -> Self {
        self.helper_style = Some(value);
        self
    }

    #[must_use]
    pub fn helper_max_lines(mut self, value: usize) -> Self {
        self.helper_max_lines = Some(value);
        self
    }

    #[must_use]
    pub fn error_style(mut self, value: TextStyle) -> Self {
        self.error_style = Some(value);
        self
    }

    #[must_use]
    pub fn error_max_lines(mut self, value: usize) -> Self {
        self.error_max_lines = Some(value);
        self
    }

    #[must_use]
    pub fn counter_style(mut self, value: TextStyle) -> Self {
        self.counter_style = Some(value);
        self
    }

    #[must_use]
    pub fn prefix_style(mut self, value: TextStyle) -> Self {
        self.prefix_style = Some(value);
        self
    }

    #[must_use]
    pub fn suffix_style(mut self, value: TextStyle) -> Self {
        self.suffix_style = Some(value);
        self
    }

    #[must_use]
    pub fn floating_label_behavior(mut self, value: FloatingLabelBehavior) -> Self {
        self.floating_label_behavior = Some(value);
        self
    }

    #[must_use]
    pub fn floating_label_alignment(mut self, value: FloatingLabelAlignment) -> Self {
        self.floating_label_alignment = Some(value);
        self
    }

    #[must_use]
    pub fn is_dense(mut self, value: bool) -> Self {
        self.is_dense = Some(value);
        self
    }

    #[must_use]
    pub fn is_collapsed(mut self, value: bool) -> Self {
        self.is_collapsed = Some(value);
        self
    }

    #[must_use]
    pub fn align_label_with_hint(mut self, value: bool) -> Self {
        self.align_label_with_hint = Some(value);
        self
    }

    #[must_use]
    pub fn content_padding(mut self, value: EdgeInsets) -> Self {
        self.content_padding = Some(value);
        self
    }

    #[must_use]
    pub fn filled(mut self, value: bool) -> Self {
        self.filled = Some(value);
        self
    }

    #[must_use]
    pub fn fill_color(mut self, value: Color) -> Self {
        self.fill_color = Some(value);
        self
    }

    #[must_use]
    pub fn hover_color(mut self, value: Color) -> Self {
        self.hover_color = Some(value);
        self
    }

    #[must_use]
    pub fn border(mut self, value: InputBorder) -> Self {
        self.border = Some(value);
        self
    }

    #[must_use]
    pub fn enabled_border(mut self, value: InputBorder) -> Self {
        self.enabled_border = Some(value);
        self
    }

    #[must_use]
    pub fn focused_border(mut self, value: InputBorder) -> Self {
        self.focused_border = Some(value);
        self
    }

    #[must_use]
    pub fn error_border(mut self, value: InputBorder) -> Self {
        self.error_border = Some(value);
        self
    }

    #[must_use]
    pub fn focused_error_border(mut self, value: InputBorder) -> Self {
        self.focused_error_border = Some(value);
        self
    }

    #[must_use]
    pub fn disabled_border(mut self, value: InputBorder) -> Self {
        self.disabled_border = Some(value);
        self
    }

    #[must_use]
    pub fn constraints(mut self, value: Size) -> Self {
        self.constraints = Some(value);
        self
    }
}

/// Inherited wrapper for an [`InputDecorationThemeData`] value.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct InputDecorationTheme {
    #[builder(setter(into))]
    pub data: InputDecorationThemeData,
    #[builder(setter(into))]
    pub child: Widget,
}

impl InputDecorationTheme {
    #[must_use]
    pub fn new(data: InputDecorationThemeData, child: impl Into<Widget>) -> Self {
        Self {
            data,
            child: child.into(),
        }
    }
}

impl From<InputDecorationTheme> for Widget {
    fn from(value: InputDecorationTheme) -> Self {
        Widget::environment_scope(value.data, value.child)
    }
}

/// Per-field Material decoration. `None` fields inherit from the nearest
/// [`InputDecorationThemeData`] and then the component defaults.
#[derive(Clone, Debug, Default, PartialEq, TypedBuilder)]
pub struct InputDecoration {
    #[builder(default, setter(strip_option, into))]
    pub icon: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    pub label_text: Option<String>,
    #[builder(default, setter(strip_option, into))]
    pub hint_text: Option<String>,
    #[builder(default, setter(strip_option, into))]
    pub helper_text: Option<String>,
    #[builder(default, setter(strip_option, into))]
    pub error_text: Option<String>,
    #[builder(default, setter(strip_option, into))]
    pub counter_text: Option<String>,
    #[builder(default, setter(strip_option, into))]
    pub counter: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    pub prefix_text: Option<String>,
    #[builder(default, setter(strip_option, into))]
    pub suffix_text: Option<String>,
    #[builder(default, setter(strip_option, into))]
    pub label_style: Option<TextStyle>,
    #[builder(default, setter(strip_option, into))]
    pub floating_label_style: Option<TextStyle>,
    #[builder(default, setter(strip_option, into))]
    pub hint_style: Option<TextStyle>,
    #[builder(default, setter(transform = |value: usize| Some(value.max(1))))]
    pub hint_max_lines: Option<usize>,
    #[builder(default, setter(transform = |value: usize| Some(value.max(1))))]
    pub helper_max_lines: Option<usize>,
    #[builder(default, setter(transform = |value: usize| Some(value.max(1))))]
    pub error_max_lines: Option<usize>,
    #[builder(default, setter(strip_option, into))]
    pub helper_style: Option<TextStyle>,
    #[builder(default, setter(strip_option, into))]
    pub error_style: Option<TextStyle>,
    #[builder(default, setter(strip_option, into))]
    pub counter_style: Option<TextStyle>,
    #[builder(default, setter(strip_option, into))]
    pub prefix_style: Option<TextStyle>,
    #[builder(default, setter(strip_option, into))]
    pub suffix_style: Option<TextStyle>,
    #[builder(default, setter(strip_option, into))]
    pub prefix_icon: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    pub suffix_icon: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    pub icon_color: Option<Color>,
    #[builder(default, setter(strip_option, into))]
    pub prefix_icon_color: Option<Color>,
    #[builder(default, setter(strip_option, into))]
    pub suffix_icon_color: Option<Color>,
    #[builder(default, setter(strip_option, into))]
    pub prefix: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    pub suffix: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    pub is_dense: Option<bool>,
    #[builder(default)]
    pub is_collapsed: bool,
    #[builder(default, setter(strip_option, into))]
    pub align_label_with_hint: Option<bool>,
    #[builder(default, setter(strip_option, into))]
    pub semantic_counter_text: Option<String>,
    #[builder(default, setter(strip_option, into))]
    pub filled: Option<bool>,
    #[builder(default, setter(strip_option, into))]
    pub fill_color: Option<Color>,
    #[builder(default, setter(strip_option, into))]
    pub hover_color: Option<Color>,
    #[builder(default, setter(strip_option, into))]
    pub content_padding: Option<EdgeInsets>,
    #[builder(default, setter(strip_option, into))]
    pub floating_label_behavior: Option<FloatingLabelBehavior>,
    #[builder(default, setter(strip_option, into))]
    pub floating_label_alignment: Option<FloatingLabelAlignment>,
    #[builder(default, setter(strip_option, into))]
    pub constraints: Option<Size>,
    #[builder(default, setter(strip_option, into))]
    pub border: Option<InputBorder>,
    #[builder(default, setter(strip_option, into))]
    pub enabled_border: Option<InputBorder>,
    #[builder(default, setter(strip_option, into))]
    pub focused_border: Option<InputBorder>,
    #[builder(default, setter(strip_option, into))]
    pub error_border: Option<InputBorder>,
    #[builder(default, setter(strip_option, into))]
    pub focused_error_border: Option<InputBorder>,
    #[builder(default, setter(strip_option, into))]
    pub disabled_border: Option<InputBorder>,
}

impl InputDecoration {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn collapsed(hint: impl Into<String>) -> Self {
        Self::new().hint(hint).is_collapsed(true)
    }
    #[must_use]
    pub fn none() -> Self {
        Self::new().border(InputBorder::None).is_collapsed(true)
    }
    #[must_use]
    pub fn label(mut self, value: impl Into<String>) -> Self {
        self.label_text = Some(value.into());
        self
    }

    #[must_use]
    pub fn label_text(self, value: impl Into<String>) -> Self {
        self.label(value)
    }
    #[must_use]
    pub fn hint(mut self, value: impl Into<String>) -> Self {
        self.hint_text = Some(value.into());
        self
    }

    #[must_use]
    pub fn hint_text(self, value: impl Into<String>) -> Self {
        self.hint(value)
    }
    #[must_use]
    pub fn helper(mut self, value: impl Into<String>) -> Self {
        self.helper_text = Some(value.into());
        self
    }

    #[must_use]
    pub fn helper_text(self, value: impl Into<String>) -> Self {
        self.helper(value)
    }
    #[must_use]
    pub fn error(mut self, value: impl Into<String>) -> Self {
        self.error_text = Some(value.into());
        self
    }

    #[must_use]
    pub fn error_text(self, value: impl Into<String>) -> Self {
        self.error(value)
    }
    #[must_use]
    pub fn counter(mut self, value: impl Into<String>) -> Self {
        self.counter_text = Some(value.into());
        self
    }

    #[must_use]
    pub fn counter_text(self, value: impl Into<String>) -> Self {
        self.counter(value)
    }

    #[must_use]
    pub fn icon(mut self, value: impl Into<Widget>) -> Self {
        self.icon = Some(value.into());
        self
    }

    #[must_use]
    pub fn counter_widget(mut self, value: impl Into<Widget>) -> Self {
        self.counter = Some(value.into());
        self
    }
    #[must_use]
    pub fn prefix(mut self, value: impl Into<String>) -> Self {
        self.prefix_text = Some(value.into());
        self
    }
    #[must_use]
    pub fn suffix(mut self, value: impl Into<String>) -> Self {
        self.suffix_text = Some(value.into());
        self
    }
    #[must_use]
    pub fn label_style(mut self, value: TextStyle) -> Self {
        self.label_style = Some(value);
        self
    }
    #[must_use]
    pub fn floating_label_style(mut self, value: TextStyle) -> Self {
        self.floating_label_style = Some(value);
        self
    }

    #[must_use]
    pub fn hint_style(mut self, value: TextStyle) -> Self {
        self.hint_style = Some(value);
        self
    }

    #[must_use]
    pub fn hint_max_lines(mut self, value: usize) -> Self {
        self.hint_max_lines = Some(value.max(1));
        self
    }

    #[must_use]
    pub fn helper_max_lines(mut self, value: usize) -> Self {
        self.helper_max_lines = Some(value.max(1));
        self
    }

    #[must_use]
    pub fn error_max_lines(mut self, value: usize) -> Self {
        self.error_max_lines = Some(value.max(1));
        self
    }
    #[must_use]
    pub fn helper_style(mut self, value: TextStyle) -> Self {
        self.helper_style = Some(value);
        self
    }
    #[must_use]
    pub fn error_style(mut self, value: TextStyle) -> Self {
        self.error_style = Some(value);
        self
    }
    #[must_use]
    pub fn counter_style(mut self, value: TextStyle) -> Self {
        self.counter_style = Some(value);
        self
    }
    #[must_use]
    pub fn prefix_style(mut self, value: TextStyle) -> Self {
        self.prefix_style = Some(value);
        self
    }
    #[must_use]
    pub fn suffix_style(mut self, value: TextStyle) -> Self {
        self.suffix_style = Some(value);
        self
    }
    #[must_use]
    pub fn prefix_icon(mut self, value: impl Into<Widget>) -> Self {
        self.prefix_icon = Some(value.into());
        self
    }
    #[must_use]
    pub fn suffix_icon(mut self, value: impl Into<Widget>) -> Self {
        self.suffix_icon = Some(value.into());
        self
    }

    #[must_use]
    pub fn icon_color(mut self, value: Color) -> Self {
        self.icon_color = Some(value);
        self
    }

    #[must_use]
    pub fn prefix_icon_color(mut self, value: Color) -> Self {
        self.prefix_icon_color = Some(value);
        self
    }

    #[must_use]
    pub fn suffix_icon_color(mut self, value: Color) -> Self {
        self.suffix_icon_color = Some(value);
        self
    }
    #[must_use]
    pub fn prefix_widget(mut self, value: impl Into<Widget>) -> Self {
        self.prefix = Some(value.into());
        self
    }
    #[must_use]
    pub fn suffix_widget(mut self, value: impl Into<Widget>) -> Self {
        self.suffix = Some(value.into());
        self
    }
    #[must_use]
    pub fn is_dense(mut self, value: bool) -> Self {
        self.is_dense = Some(value);
        self
    }
    #[must_use]
    pub fn is_collapsed(mut self, value: bool) -> Self {
        self.is_collapsed = value;
        self
    }
    #[must_use]
    pub fn align_label_with_hint(mut self, value: bool) -> Self {
        self.align_label_with_hint = Some(value);
        self
    }
    #[must_use]
    pub fn semantic_counter_text(mut self, value: impl Into<String>) -> Self {
        self.semantic_counter_text = Some(value.into());
        self
    }
    #[must_use]
    pub fn filled(mut self, value: bool) -> Self {
        self.filled = Some(value);
        self
    }
    #[must_use]
    pub fn fill_color(mut self, value: Color) -> Self {
        self.fill_color = Some(value);
        self
    }
    #[must_use]
    pub fn hover_color(mut self, value: Color) -> Self {
        self.hover_color = Some(value);
        self
    }
    #[must_use]
    pub fn content_padding(mut self, value: EdgeInsets) -> Self {
        self.content_padding = Some(value);
        self
    }
    #[must_use]
    pub fn floating_label_behavior(mut self, value: FloatingLabelBehavior) -> Self {
        self.floating_label_behavior = Some(value);
        self
    }
    #[must_use]
    pub fn floating_label_alignment(mut self, value: FloatingLabelAlignment) -> Self {
        self.floating_label_alignment = Some(value);
        self
    }
    #[must_use]
    pub fn constraints(mut self, value: Size) -> Self {
        self.constraints = Some(value);
        self
    }
    #[must_use]
    pub fn border(mut self, value: InputBorder) -> Self {
        self.border = Some(value);
        self
    }
    #[must_use]
    pub fn enabled_border(mut self, value: InputBorder) -> Self {
        self.enabled_border = Some(value);
        self
    }
    #[must_use]
    pub fn focused_border(mut self, value: InputBorder) -> Self {
        self.focused_border = Some(value);
        self
    }
    #[must_use]
    pub fn error_border(mut self, value: InputBorder) -> Self {
        self.error_border = Some(value);
        self
    }
    #[must_use]
    pub fn focused_error_border(mut self, value: InputBorder) -> Self {
        self.focused_error_border = Some(value);
        self
    }
    #[must_use]
    pub fn disabled_border(mut self, value: InputBorder) -> Self {
        self.disabled_border = Some(value);
        self
    }

    /// Applies only the values present in `other`, retaining this
    /// decoration's sparse override semantics.
    #[must_use]
    pub fn merge(mut self, other: &Self) -> Self {
        macro_rules! override_field {
            ($field:ident) => {
                if other.$field.is_some() {
                    self.$field = other.$field.clone();
                }
            };
        }
        override_field!(icon);
        override_field!(label_text);
        override_field!(hint_text);
        override_field!(helper_text);
        override_field!(error_text);
        override_field!(counter_text);
        override_field!(counter);
        override_field!(prefix_text);
        override_field!(suffix_text);
        override_field!(label_style);
        override_field!(floating_label_style);
        override_field!(hint_style);
        override_field!(hint_max_lines);
        override_field!(helper_max_lines);
        override_field!(error_max_lines);
        override_field!(helper_style);
        override_field!(error_style);
        override_field!(counter_style);
        override_field!(prefix_style);
        override_field!(suffix_style);
        override_field!(prefix_icon);
        override_field!(suffix_icon);
        override_field!(icon_color);
        override_field!(prefix_icon_color);
        override_field!(suffix_icon_color);
        override_field!(prefix);
        override_field!(suffix);
        override_field!(is_dense);
        override_field!(align_label_with_hint);
        override_field!(semantic_counter_text);
        override_field!(filled);
        override_field!(fill_color);
        override_field!(hover_color);
        override_field!(content_padding);
        override_field!(floating_label_behavior);
        override_field!(floating_label_alignment);
        override_field!(constraints);
        override_field!(border);
        override_field!(enabled_border);
        override_field!(focused_border);
        override_field!(error_border);
        override_field!(focused_error_border);
        override_field!(disabled_border);
        if other.is_collapsed {
            self.is_collapsed = true;
        }
        self
    }

    #[must_use]
    pub fn apply_defaults(&self, theme: &InputDecorationThemeData) -> Self {
        let mut result = self.clone();
        if result.label_style.is_none() {
            result.label_style = theme.label_style.clone();
        }
        if result.floating_label_style.is_none() {
            result.floating_label_style = theme.floating_label_style.clone();
        }
        if result.hint_style.is_none() {
            result.hint_style = theme.hint_style.clone();
        }
        if result.hint_max_lines.is_none() {
            result.hint_max_lines = theme.hint_max_lines;
        }
        if result.helper_max_lines.is_none() {
            result.helper_max_lines = theme.helper_max_lines;
        }
        if result.error_max_lines.is_none() {
            result.error_max_lines = theme.error_max_lines;
        }
        if result.helper_style.is_none() {
            result.helper_style = theme.helper_style.clone();
        }
        if result.error_style.is_none() {
            result.error_style = theme.error_style.clone();
        }
        if result.counter_style.is_none() {
            result.counter_style = theme.counter_style.clone();
        }
        if result.prefix_style.is_none() {
            result.prefix_style = theme.prefix_style.clone();
        }
        if result.suffix_style.is_none() {
            result.suffix_style = theme.suffix_style.clone();
        }
        if result.is_dense.is_none() {
            result.is_dense = theme.is_dense;
        }
        if !result.is_collapsed {
            result.is_collapsed = theme.is_collapsed.unwrap_or(false);
        }
        if result.align_label_with_hint.is_none() {
            result.align_label_with_hint = theme.align_label_with_hint;
        }
        if result.content_padding.is_none() {
            result.content_padding = theme.content_padding;
        }
        if result.filled.is_none() {
            result.filled = theme.filled;
        }
        if result.fill_color.is_none() {
            result.fill_color = theme.fill_color;
        }
        if result.hover_color.is_none() {
            result.hover_color = theme.hover_color;
        }
        if result.floating_label_behavior.is_none() {
            result.floating_label_behavior = theme.floating_label_behavior;
        }
        if result.floating_label_alignment.is_none() {
            result.floating_label_alignment = theme.floating_label_alignment;
        }
        if result.border.is_none() {
            result.border = theme.border;
        }
        if result.enabled_border.is_none() {
            result.enabled_border = theme.enabled_border;
        }
        if result.focused_border.is_none() {
            result.focused_border = theme.focused_border;
        }
        if result.error_border.is_none() {
            result.error_border = theme.error_border;
        }
        if result.focused_error_border.is_none() {
            result.focused_error_border = theme.focused_error_border;
        }
        if result.disabled_border.is_none() {
            result.disabled_border = theme.disabled_border;
        }
        if result.constraints.is_none() {
            result.constraints = theme.constraints;
        }
        result
    }
}

/// Material's chrome-only input decorator. Editing, focus and selection stay
/// in the child (normally core `EditableText`); this widget only resolves the
/// decoration state and composes labels, icons, helper/error text, and borders.
#[derive(Clone, TypedBuilder)]
pub struct InputDecorator {
    #[builder(setter(into))]
    pub decoration: InputDecoration,
    #[builder(setter(into))]
    pub child: Widget,
    #[builder(default)]
    pub is_focused: bool,
    #[builder(default = true)]
    pub is_empty: bool,
    #[builder(default = true)]
    pub enabled: bool,
}

impl InputDecorator {
    #[must_use]
    pub fn new(decoration: InputDecoration, child: impl Into<Widget>) -> Self {
        Self {
            decoration,
            child: child.into(),
            is_focused: false,
            is_empty: true,
            enabled: true,
        }
    }

    #[must_use]
    pub fn focused(mut self, value: bool) -> Self {
        self.is_focused = value;
        self
    }

    #[must_use]
    pub fn empty(mut self, value: bool) -> Self {
        self.is_empty = value;
        self
    }

    #[must_use]
    pub fn enabled(mut self, value: bool) -> Self {
        self.enabled = value;
        self
    }

    #[must_use]
    pub fn decoration(mut self, value: InputDecoration) -> Self {
        self.decoration = value;
        self
    }

    #[must_use]
    pub fn child(mut self, value: impl Into<Widget>) -> Self {
        self.child = value.into();
        self
    }

    fn build(self) -> Widget {
        let decoration = self.decoration;
        let border = if !self.enabled {
            decoration
                .disabled_border
                .or(decoration.border)
                .unwrap_or_default()
        } else if decoration.error_text.is_some() && self.is_focused {
            decoration
                .focused_error_border
                .or(decoration.error_border)
                .or(decoration.focused_border)
                .or(decoration.border)
                .unwrap_or_default()
        } else if decoration.error_text.is_some() {
            decoration
                .error_border
                .or(decoration.border)
                .unwrap_or_default()
        } else if self.is_focused {
            decoration
                .focused_border
                .or(decoration.border)
                .unwrap_or_default()
        } else {
            decoration
                .enabled_border
                .or(decoration.border)
                .unwrap_or_default()
        };
        let (border, radius) = match border {
            InputBorder::None => (Border::new(0.0, Color::TRANSPARENT), 0.0),
            InputBorder::Underline { border, radius } | InputBorder::Outline { border, radius } => {
                (border, radius)
            }
        };
        let fill = if decoration.filled.unwrap_or(false) {
            decoration.fill_color.unwrap_or(Color::TRANSPARENT)
        } else {
            Color::TRANSPARENT
        };
        let padding = if decoration.is_collapsed {
            EdgeInsets::all(0.0)
        } else {
            decoration
                .content_padding
                .unwrap_or_else(|| EdgeInsets::symmetric(12.0, 8.0))
        };
        let mut content = Vec::with_capacity(5);
        if let Some(label) = decoration.label_text
            && (self.is_focused
                || !self.is_empty
                || decoration.floating_label_behavior == Some(FloatingLabelBehavior::Always))
        {
            content.push(Widget::from(
                Text::new(label).style(decoration.label_style.clone().unwrap_or_default()),
            ));
        }
        let mut row = Vec::with_capacity(5);
        if let Some(icon) = decoration.prefix_icon {
            row.push(icon);
        }
        if let Some(prefix) = decoration.prefix {
            row.push(prefix);
        }
        if let Some(text) = decoration.prefix_text {
            row.push(
                Text::new(text)
                    .style(decoration.prefix_style.clone().unwrap_or_default())
                    .into(),
            );
        }
        row.push(self.child);
        if let Some(text) = decoration.suffix_text {
            row.push(
                Text::new(text)
                    .style(decoration.suffix_style.clone().unwrap_or_default())
                    .into(),
            );
        }
        if let Some(suffix) = decoration.suffix {
            row.push(suffix);
        }
        if let Some(icon) = decoration.suffix_icon {
            row.push(icon);
        }
        content.push(Widget::from(incular_widgets::Row::new(row)));
        if let Some(message) = decoration.error_text {
            content.push(Widget::from(
                Text::new(message).style(decoration.error_style.unwrap_or_default()),
            ));
        } else if let Some(message) = decoration.helper_text {
            content.push(Widget::from(
                Text::new(message).style(decoration.helper_style.unwrap_or_default()),
            ));
        }
        if let Some(counter) = decoration.counter {
            content.push(counter);
        } else if let Some(counter) = decoration.counter_text {
            content.push(Widget::from(
                Text::new(counter).style(decoration.counter_style.unwrap_or_default()),
            ));
        }
        let inner: Widget = if content.len() == 1 {
            content.remove(0)
        } else {
            incular_widgets::Column::new(content).spacing(4.0).into()
        };
        let mut result = Container::new()
            .padding(padding)
            .decoration(
                BoxDecoration::new()
                    .color(fill)
                    .border(border)
                    .border_radius(BorderRadius::circular(radius)),
            )
            .child(inner);
        if let Some(size) = decoration.constraints {
            result = result.constraints(Constraints::tight(size));
        }
        if let Some(icon) = decoration.icon {
            Row::new([icon, result.into()])
                .spacing(8.0)
                .cross_axis_alignment(incular_config::CrossAxisAlignment::Center)
                .into()
        } else {
            result.into()
        }
    }
}

impl From<InputDecorator> for Widget {
    fn from(value: InputDecorator) -> Self {
        value.build()
    }
}

impl From<InputDecoration> for TextFieldStyle {
    fn from(value: InputDecoration) -> Self {
        let border = value
            .border
            .or(value.enabled_border)
            .map(|border| match border {
                InputBorder::None => Border::new(0.0, Color::TRANSPARENT),
                InputBorder::Underline { border, .. } | InputBorder::Outline { border, .. } => {
                    border
                }
            });
        let border_focused = value
            .focused_border
            .or(value.focused_error_border)
            .map(|border| match border {
                InputBorder::None => Border::new(0.0, Color::TRANSPARENT),
                InputBorder::Underline { border, .. } | InputBorder::Outline { border, .. } => {
                    border
                }
            });
        let radius = value
            .border
            .as_ref()
            .or(value.enabled_border.as_ref())
            .and_then(|border| match border {
                InputBorder::Outline { radius, .. } => Some(*radius),
                _ => None,
            });
        Self {
            background: value.fill_color,
            foreground: None,
            placeholder_color: value.label_style.as_ref().map(|style| style.color),
            border,
            border_focused,
            border_radius: radius,
            padding: value.content_padding,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FloatingLabelBehavior {
    #[default]
    Auto,
    Always,
    Never,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FloatingLabelAlignment {
    #[default]
    Start,
    Center,
}

/// Rust representation of Flutter's Material input borders.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum InputBorder {
    None,
    Underline { border: Border, radius: f32 },
    Outline { border: Border, radius: f32 },
}

impl Default for InputBorder {
    fn default() -> Self {
        Self::Underline {
            border: Border::new(1.0, Color::rgba(121, 116, 126, 255)),
            radius: 0.0,
        }
    }
}

impl InputBorder {
    #[must_use]
    pub fn underline() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn outline(radius: f32, border: Border) -> Self {
        Self::Outline {
            border,
            radius: radius.max(0.0),
        }
    }
    #[must_use]
    pub fn none() -> Self {
        Self::None
    }
}

/// Flutter-shaped outline border descriptor. The renderer-neutral border is
/// retained in [`InputBorder`] so Material text fields do not need a second
/// painting implementation.
#[derive(Clone, Copy, Debug, PartialEq, TypedBuilder)]
pub struct OutlineInputBorder {
    #[builder(default = Border::new(1.0, Color::rgba(121, 116, 126, 255)))]
    pub border: Border,
    #[builder(default = 4.0, setter(transform = |value: f32| value.max(0.0)))]
    pub radius: f32,
}

impl Default for OutlineInputBorder {
    fn default() -> Self {
        Self {
            border: Border::new(1.0, Color::rgba(121, 116, 126, 255)),
            radius: 4.0,
        }
    }
}

impl OutlineInputBorder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn border(mut self, value: Border) -> Self {
        self.border = value;
        self
    }
    #[must_use]
    pub fn radius(mut self, value: f32) -> Self {
        self.radius = value.max(0.0);
        self
    }
}

impl From<OutlineInputBorder> for InputBorder {
    fn from(value: OutlineInputBorder) -> Self {
        InputBorder::outline(value.radius, value.border)
    }
}

/// Flutter-shaped underline border descriptor.
#[derive(Clone, Copy, Debug, PartialEq, TypedBuilder)]
pub struct UnderlineInputBorder {
    #[builder(default = Border::new(1.0, Color::rgba(121, 116, 126, 255)))]
    pub border: Border,
    #[builder(default = 0.0, setter(transform = |value: f32| value.max(0.0)))]
    pub radius: f32,
}

impl Default for UnderlineInputBorder {
    fn default() -> Self {
        Self {
            border: Border::new(1.0, Color::rgba(121, 116, 126, 255)),
            radius: 0.0,
        }
    }
}

impl UnderlineInputBorder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn border(mut self, value: Border) -> Self {
        self.border = value;
        self
    }
    #[must_use]
    pub fn radius(mut self, value: f32) -> Self {
        self.radius = value.max(0.0);
        self
    }
}

impl From<UnderlineInputBorder> for InputBorder {
    fn from(value: UnderlineInputBorder) -> Self {
        InputBorder::Underline {
            border: value.border,
            radius: value.radius,
        }
    }
}

/// Common shaped-border spelling used by Flutter's input border hierarchy.
pub type ShapedInputBorder = InputBorder;
