//! Row-major table layout descriptor.

use crate::{Widget, WidgetKind};

/// A row-major max-content table layout.
#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    columns: usize,
    column_spacing: f32,
    row_spacing: f32,
    children: Vec<Widget>,
}

impl Table {
    /// Creates a table with a fixed number of columns and children in row-major order.
    #[must_use]
    pub fn new(columns: usize, children: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        Self {
            columns: columns.max(1),
            column_spacing: 0.0,
            row_spacing: 0.0,
            children: children.into_iter().map(Into::into).collect(),
        }
    }

    /// Sets the horizontal spacing between columns.
    #[must_use]
    pub fn column_spacing(mut self, spacing: f32) -> Self {
        self.column_spacing = spacing.max(0.0);
        self
    }

    /// Sets the vertical spacing between rows.
    #[must_use]
    pub fn row_spacing(mut self, spacing: f32) -> Self {
        self.row_spacing = spacing.max(0.0);
        self
    }
}

impl From<Table> for Widget {
    fn from(value: Table) -> Self {
        Widget::from_kind(WidgetKind::Table {
            columns: value.columns,
            column_spacing: value.column_spacing,
            row_spacing: value.row_spacing,
            children: value.children,
        })
    }
}
