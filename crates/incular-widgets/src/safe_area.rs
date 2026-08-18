//! Minimal logical SafeArea composition.

use incular_config::EdgeInsets;

use crate::Widget;

/// Describes padding that protects a child from runtime-provided view-safe
/// insets. Resolution stays at the runtime boundary, keeping widgets free of
/// platform/window dependencies.
pub struct SafeArea {
    child: Widget,
    minimum: EdgeInsets,
    left: bool,
    top: bool,
    right: bool,
    bottom: bool,
}

impl SafeArea {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            minimum: EdgeInsets::ZERO,
            left: true,
            top: true,
            right: true,
            bottom: true,
        }
    }

    #[must_use]
    pub const fn minimum(mut self, minimum: EdgeInsets) -> Self {
        self.minimum = minimum;
        self
    }

    #[must_use]
    pub const fn sides(mut self, left: bool, top: bool, right: bool, bottom: bool) -> Self {
        self.left = left;
        self.top = top;
        self.right = right;
        self.bottom = bottom;
        self
    }

    /// Converts the descriptor to a retained Padding widget using logical
    /// runtime insets. Disabled sides retain only their minimum padding.
    #[must_use]
    pub fn resolve(self, insets: EdgeInsets) -> Widget {
        let insets = insets.normalized();
        Widget::padding(
            EdgeInsets::only(
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
            ),
            self.child,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use incular_core::{Color, Size};

    #[test]
    fn descriptor_resolves_without_platform_dependencies() {
        let safe = SafeArea::new(Widget::box_(Size::new(1., 1.), Color::WHITE))
            .minimum(EdgeInsets::all(3.))
            .sides(true, false, true, false);
        let _widget = safe.resolve(EdgeInsets::only(8., 9., 2., 1.));
    }
}
