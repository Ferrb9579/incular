//! Retained semantic-tree visualization widget.

use crate::Widget;
use incular_text::TextStyle;

/// Maximum number of semantic nodes a debugger overlay will draw by default.
pub const DEFAULT_SEMANTICS_DEBUGGER_NODE_LIMIT: usize = 10_000;

/// Draws the retained semantic tree as bounded, labeled geometry over a child.
///
/// The debugger is transparent to the semantic tree it visualizes. Its
/// overlay is emitted after the child display list, so labels and nested
/// outlines remain visible even when the child is cached in a compositor
/// picture layer.
#[derive(Clone, Debug, PartialEq)]
pub struct SemanticsDebugger {
    child: Widget,
    label_style: TextStyle,
    max_nodes: usize,
}

impl SemanticsDebugger {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            label_style: TextStyle::default()
                .font_size(10.0)
                .color(incular_core::Color::BLACK),
            max_nodes: DEFAULT_SEMANTICS_DEBUGGER_NODE_LIMIT,
        }
    }

    #[must_use]
    pub fn label_style(mut self, label_style: TextStyle) -> Self {
        self.label_style = label_style;
        self
    }

    #[must_use]
    pub fn max_nodes(mut self, max_nodes: usize) -> Self {
        self.max_nodes = max_nodes;
        self
    }

    #[must_use]
    pub fn child(&self) -> &Widget {
        &self.child
    }

    #[must_use]
    pub fn label_style_value(&self) -> &TextStyle {
        &self.label_style
    }

    #[must_use]
    pub const fn node_limit(&self) -> usize {
        self.max_nodes
    }
}

impl From<SemanticsDebugger> for Widget {
    fn from(value: SemanticsDebugger) -> Self {
        Widget::semantics_debugger(value.label_style, value.max_nodes, value.child)
    }
}
