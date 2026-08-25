//! Ambient theme and application configuration scope for Incular Controls.

use crate::theme::ControlTheme;
use incular_widgets::Widget;

/// Ambient scope widget holding a ControlTheme for its subtree.
#[derive(Clone)]
pub struct ControlThemeScope {
    theme: ControlTheme,
    child: Widget,
}

impl ControlThemeScope {
    #[must_use]
    pub fn new(theme: ControlTheme, child: impl Into<Widget>) -> Self {
        Self {
            theme,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn theme(&self) -> &ControlTheme {
        &self.theme
    }

    #[must_use]
    pub fn child(&self) -> &Widget {
        &self.child
    }

    #[must_use]
    pub fn into_parts(self) -> (ControlTheme, Widget) {
        (self.theme, self.child)
    }
}

impl From<ControlThemeScope> for Widget {
    fn from(value: ControlThemeScope) -> Self {
        Widget::environment_scope(value.theme, value.child)
    }
}
