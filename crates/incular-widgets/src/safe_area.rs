//! Declarative SafeArea widget consuming ambient runtime/window safe insets.

use incular_config::EdgeInsets;
use typed_builder::TypedBuilder;

use crate::{Widget, WidgetKind};

/// Insets child from the display edges to avoid platform UI, notches, and status bars.
///
/// Nested instances accumulate: each applies the full ambient padding, so
/// an inner `SafeArea` pads inside the outer one's padding. This matches
/// the platform convention the option names follow. Avoid double-padding
/// by disabling already-handled edges on one level with [`Self::sides`]
/// (for example an inner bar with only the bottom edge enabled inside a
/// full outer guard), or by keeping separate subtrees on separate edges.
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

    /// Keeps the persistent bottom obstruction margin while transient
    /// occlusion (such as the keyboard) covers it. The maintained value
    /// comes from the ambient view padding, never the occlusion height.
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
    ///
    /// The insets are the current safe margin. Without an ambient
    /// environment there is no persistent view padding to keep, so
    /// `maintain_bottom_view_padding` has no effect here; use
    /// [`Self::resolve_with_padding`] when both signals are available.
    #[must_use]
    pub fn resolve(self, insets: EdgeInsets) -> Widget {
        let padding = self.policy().padding(insets, EdgeInsets::ZERO);
        crate::Padding::new(padding, self.child).into()
    }

    /// Resolves against both environment signals for use outside the
    /// retained tree, applying the same maintained-padding policy as
    /// retained construction: the bottom edge keeps
    /// `max(safe, minimum, persistent)` when maintenance is enabled.
    /// Transient occlusion never participates; keyboard avoidance belongs
    /// to the padded parent.
    #[must_use]
    pub fn resolve_with_padding(self, safe_insets: EdgeInsets, view_padding: EdgeInsets) -> Widget {
        let padding = self.policy().padding(safe_insets, view_padding);
        crate::Padding::new(padding, self.child).into()
    }

    /// Reports whether the descriptor keeps the persistent bottom margin
    /// while transient occlusion covers it.
    #[must_use]
    pub fn maintains_bottom_view_padding(&self) -> bool {
        self.maintain_bottom_view_padding
    }

    fn policy(&self) -> SafeAreaPolicy {
        SafeAreaPolicy {
            minimum: self.minimum,
            left: self.left,
            top: self.top,
            right: self.right,
            bottom: self.bottom,
            maintain_bottom_view_padding: self.maintain_bottom_view_padding,
        }
    }
}

/// Edge toggles and floors shared by every SafeArea construction path.
///
/// One policy feeds the retained render kind, the explicit-insets
/// [`SafeArea::resolve`], and the environment-aware
/// [`SafeArea::resolve_with_padding`], so the three paths cannot drift on
/// the max expressions. Passing zero view padding disables maintenance,
/// which is exactly the documented [`SafeArea::resolve`] contract.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SafeAreaPolicy {
    pub(crate) minimum: EdgeInsets,
    pub(crate) left: bool,
    pub(crate) top: bool,
    pub(crate) right: bool,
    pub(crate) bottom: bool,
    pub(crate) maintain_bottom_view_padding: bool,
}

impl SafeAreaPolicy {
    pub(crate) fn padding(self, safe_insets: EdgeInsets, view_padding: EdgeInsets) -> EdgeInsets {
        let ambient = safe_insets.normalized();
        // A maintained bottom edge keeps the persistent obstruction
        // margin, which the shell reports separately from transient
        // occlusion and never reduces under it. The occlusion height is
        // not persistent padding. Disabled edges stay at their minimum.
        let persistent_bottom = if self.maintain_bottom_view_padding {
            view_padding.normalized().bottom.max(0.0)
        } else {
            0.0
        };
        EdgeInsets::only(
            if self.left {
                ambient.left.max(self.minimum.left)
            } else {
                self.minimum.left
            },
            if self.top {
                ambient.top.max(self.minimum.top)
            } else {
                self.minimum.top
            },
            if self.right {
                ambient.right.max(self.minimum.right)
            } else {
                self.minimum.right
            },
            if self.bottom {
                ambient
                    .bottom
                    .max(self.minimum.bottom)
                    .max(persistent_bottom)
            } else {
                self.minimum.bottom
            },
        )
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
