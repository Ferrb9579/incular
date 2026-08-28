//! Row-major table layout descriptor.

use typed_builder::TypedBuilder;

use crate::{Widget, WidgetKind};

/// A row-major max-content table layout.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct Table {
    #[builder(default = 1, setter(transform = |columns: usize| columns.max(1)))]
    columns: usize,
    #[builder(default, setter(transform = |spacing: f32| spacing.max(0.0)))]
    column_spacing: f32,
    #[builder(default, setter(transform = |spacing: f32| spacing.max(0.0)))]
    row_spacing: f32,
    #[builder(default, setter(into))]
    children: Vec<Widget>,
}

impl Default for Table {
    fn default() -> Self {
        Self {
            columns: 1,
            column_spacing: 0.0,
            row_spacing: 0.0,
            children: Vec::new(),
        }
    }
}

impl Table {
    /// Creates a table with a fixed number of columns and children in row-major order.
    #[must_use]
    pub fn new(columns: usize, children: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        Self {
            columns: columns.max(1),
            children: children.into_iter().map(Into::into).collect(),
            ..Self::default()
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

#[cfg(test)]
mod tests {
    use super::*;

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
            .children(vec![Widget::text("a"), Widget::text("b")])
            .build();

        let WidgetKind::Table {
            columns,
            column_spacing,
            row_spacing,
            children,
        } = Widget::from(table).kind
        else {
            panic!("expected Table widget kind")
        };

        assert_eq!(columns, 1);
        assert_eq!(column_spacing, 0.0);
        assert_eq!(row_spacing, 0.0);
        assert_eq!(children.len(), 2);
    }

    #[test]
    fn constructor_accepts_arbitrary_widget_descriptors() {
        let table = Table::new(2, [crate::Text::new("cell")]);
        let WidgetKind::Table {
            columns, children, ..
        } = Widget::from(table).kind
        else {
            panic!("expected Table widget kind")
        };

        assert_eq!(columns, 2);
        assert_eq!(children.len(), 1);
    }
}
