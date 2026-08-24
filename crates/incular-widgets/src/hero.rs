//! Hero and shared-element route transition widgets.

use crate::Widget;

/// Marks a child widget as a candidate for shared-element route animations.
#[derive(Clone, Debug, PartialEq)]
pub struct Hero {
    tag: String,
    child: Widget,
}

impl Hero {
    #[must_use]
    pub fn new(tag: impl Into<String>, child: impl Into<Widget>) -> Self {
        Self {
            tag: tag.into(),
            child: child.into(),
        }
    }

    #[must_use]
    pub fn tag(&self) -> &str {
        &self.tag
    }
}

impl From<Hero> for Widget {
    fn from(value: Hero) -> Self {
        value.child
    }
}

/// Enables or disables [`Hero`] transitions for its subtree.
#[derive(Clone, Debug, PartialEq)]
pub struct HeroMode {
    enabled: bool,
    child: Widget,
}

impl HeroMode {
    #[must_use]
    pub fn new(enabled: bool, child: impl Into<Widget>) -> Self {
        Self {
            enabled,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl From<HeroMode> for Widget {
    fn from(value: HeroMode) -> Self {
        value.child
    }
}

/// Scopes a hero animation controller.
#[derive(Clone, Debug, PartialEq)]
pub struct HeroControllerScope {
    child: Widget,
}

impl HeroControllerScope {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl From<HeroControllerScope> for Widget {
    fn from(value: HeroControllerScope) -> Self {
        value.child
    }
}
