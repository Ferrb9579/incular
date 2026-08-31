//! Declarative SafeArea widget consuming ambient runtime/window safe insets.

use incular_config::EdgeInsets;
use typed_builder::TypedBuilder;

use crate::{Widget, WidgetKind};

/// Insets child from the display edges to avoid platform UI, notches, and status bars.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct SafeArea {
    #[builder(setter(into))]
    pub(crate) child: Widget,
    #[builder(default = EdgeInsets::ZERO)]
    pub(crate) minimum: EdgeInsets,
    #[builder(default = true)]
    pub(crate) left: bool,
    #[builder(default = true)]
    pub(crate) top: bool,
    #[builder(default = true)]
    pub(crate) right: bool,
    #[builder(default = true)]
    pub(crate) bottom: bool,
    #[builder(default = false)]
    pub(crate) maintain_bottom_view_padding: bool,
}

impl SafeArea {
    /// Creates a SafeArea protecting all 4 edges.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            minimum: EdgeInsets::ZERO,
            left: true,
            top: true,
            right: true,
            bottom: true,
            maintain_bottom_view_padding: false,
        }
    }

    /// Toggles left-edge inset.
    #[must_use]
    pub fn left(mut self, left: bool) -> Self {
        self.left = left;
        self
    }

    /// Toggles top-edge inset.
    #[must_use]
    pub fn top(mut self, top: bool) -> Self {
        self.top = top;
        self
    }

    /// Toggles right-edge inset.
    #[must_use]
    pub fn right(mut self, right: bool) -> Self {
        self.right = right;
        self
    }

    /// Toggles bottom-edge inset.
    #[must_use]
    pub fn bottom(mut self, bottom: bool) -> Self {
        self.bottom = bottom;
        self
    }

    /// Sets the minimum fallback padding for edges.
    #[must_use]
    pub fn minimum(mut self, minimum: EdgeInsets) -> Self {
        self.minimum = minimum;
        self
    }

    /// Sets whether to maintain bottom view padding when keyboard/IME is shown.
    #[must_use]
    pub fn maintain_bottom_view_padding(mut self, maintain: bool) -> Self {
        self.maintain_bottom_view_padding = maintain;
        self
    }

    /// Sets active sides explicitly.
    #[must_use]
    pub fn sides(mut self, left: bool, top: bool, right: bool, bottom: bool) -> Self {
        self.left = left;
        self.top = top;
        self.right = right;
        self.bottom = bottom;
        self
    }

    /// Resolves against explicit insets for standalone use without a tree.
    #[must_use]
    pub fn resolve(self, insets: EdgeInsets) -> Widget {
        let insets = insets.normalized();
        let padding = EdgeInsets::only(
            if self.left {
                insets.left.max(self.minimum.left)
            } else {
                self.minimum.left
            },
            if self.top {
                insets.top.max(self.minimum.top)
            } else {
                self.minimum.top
            },
            if self.right {
                insets.right.max(self.minimum.right)
            } else {
                self.minimum.right
            },
            if self.bottom {
                insets.bottom.max(self.minimum.bottom)
            } else {
                self.minimum.bottom
            },
        );
        crate::Padding::new(padding, self.child).into()
    }
}

impl From<SafeArea> for Widget {
    fn from(value: SafeArea) -> Self {
        Widget::from_kind(WidgetKind::SafeArea {
            minimum: value.minimum,
            left: value.left,
            top: value.top,
            right: value.right,
            bottom: value.bottom,
            maintain_bottom_view_padding: value.maintain_bottom_view_padding,
            child: value.child,
        })
    }
}
