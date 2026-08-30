//! Collection-index semantics widget.

use crate::Widget;

/// Annotates one semantic subtree with its logical collection index.
///
/// A lazy sliver can materialize only the visible window while its semantic
/// child still reports the original index. The wrapper is transparent to
/// layout, painting, and accessibility role selection.
#[derive(Clone, Debug, PartialEq)]
pub struct IndexedSemantics {
    index: usize,
    child: Widget,
}

impl IndexedSemantics {
    #[must_use]
    pub fn new(index: usize, child: impl Into<Widget>) -> Self {
        Self {
            index,
            child: child.into(),
        }
    }

    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }

    #[must_use]
    pub fn child(&self) -> &Widget {
        &self.child
    }
}

impl From<IndexedSemantics> for Widget {
    fn from(value: IndexedSemantics) -> Self {
        Widget::indexed_semantics(value.index, value.child)
    }
}
