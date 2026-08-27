//! Popover-style compound parts sharing the overlay foundation.

use crate::overlay::{Align, AnchoredPositioner, OverlayPortal, Side};
use incular_semantics::{Role as SemanticRole, SemanticState};
use incular_widgets::Widget;
use incular_widgets::internal::ExplicitSemantics;
use std::rc::Rc;

#[derive(Clone)]
pub struct Root {
    open: bool,
    child: Option<Widget>,
    on_change: Option<Rc<dyn Fn(bool) + 'static>>,
}
impl Default for Root {
    fn default() -> Self {
        Self::new()
    }
}
impl Root {
    #[must_use]
    pub fn new() -> Self {
        Self {
            open: false,
            child: None,
            on_change: None,
        }
    }
    #[must_use]
    pub fn open(mut self, value: bool) -> Self {
        self.open = value;
        self
    }
    #[must_use]
    pub fn default_open(self, value: bool) -> Self {
        self.open(value)
    }
    #[must_use]
    pub fn child(mut self, value: impl Into<Widget>) -> Self {
        self.child = Some(value.into());
        self
    }
    #[must_use]
    pub fn on_open_change(mut self, callback: impl Fn(bool) + 'static) -> Self {
        self.on_change = Some(Rc::new(callback));
        self
    }
}
impl From<Root> for Widget {
    fn from(value: Root) -> Self {
        let child = value
            .child
            .unwrap_or_else(|| incular_widgets::SizedBox::shrink().into());
        let child = child.semantics(
            ExplicitSemantics::new(SemanticRole::GenericContainer).state(SemanticState {
                enabled: true,
                expanded: Some(value.open),
                ..SemanticState::default()
            }),
        );
        Widget::visibility(value.open, child)
    }
}

#[derive(Clone)]
pub struct Trigger {
    child: Widget,
}
impl Trigger {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}
impl From<Trigger> for Widget {
    fn from(value: Trigger) -> Self {
        value.child
    }
}
#[derive(Clone)]
pub struct Portal {
    inner: OverlayPortal,
}
impl Portal {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            inner: OverlayPortal::new(child),
        }
    }
    #[must_use]
    pub fn overlay(mut self, value: impl Into<Widget>) -> Self {
        self.inner = self.inner.overlay(value);
        self
    }
    #[must_use]
    pub fn open(mut self, value: bool) -> Self {
        self.inner = self.inner.open(value);
        self
    }
}
impl From<Portal> for Widget {
    fn from(value: Portal) -> Self {
        value.inner.into()
    }
}
#[derive(Clone)]
pub struct Positioner {
    inner: AnchoredPositioner,
}
impl Positioner {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            inner: AnchoredPositioner::new(child),
        }
    }
    #[must_use]
    pub fn side(mut self, value: Side) -> Self {
        self.inner = self.inner.side(value);
        self
    }
    #[must_use]
    pub fn align(mut self, value: Align) -> Self {
        self.inner = self.inner.align(value);
        self
    }

    #[must_use]
    pub fn side_offset(mut self, value: f32) -> Self {
        self.inner = self.inner.side_offset(value);
        self
    }

    #[must_use]
    pub fn align_offset(mut self, value: f32) -> Self {
        self.inner = self.inner.align_offset(value);
        self
    }

    #[must_use]
    pub fn anchor(mut self, value: incular_core::Offset) -> Self {
        self.inner = self.inner.anchor(value);
        self
    }
}
impl From<Positioner> for Widget {
    fn from(value: Positioner) -> Self {
        value.inner.into()
    }
}
#[derive(Clone)]
pub struct Popup {
    child: Widget,
}
impl Popup {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}
impl From<Popup> for Widget {
    fn from(value: Popup) -> Self {
        value.child
    }
}
#[derive(Clone)]
pub struct Arrow {
    child: Option<Widget>,
}
impl Arrow {
    #[must_use]
    pub fn new() -> Self {
        Self { child: None }
    }
    #[must_use]
    pub fn child(mut self, value: impl Into<Widget>) -> Self {
        self.child = Some(value.into());
        self
    }
}
impl Default for Arrow {
    fn default() -> Self {
        Self::new()
    }
}
impl From<Arrow> for Widget {
    fn from(value: Arrow) -> Self {
        value
            .child
            .unwrap_or_else(|| incular_widgets::SizedBox::shrink().into())
    }
}
#[derive(Clone)]
pub struct Title {
    child: Widget,
}
impl Title {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            child: incular_widgets::Text::new(value).into(),
        }
    }
}
impl From<Title> for Widget {
    fn from(value: Title) -> Self {
        value.child
    }
}
#[derive(Clone)]
pub struct Description {
    child: Widget,
}
impl Description {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            child: incular_widgets::Text::new(value).into(),
        }
    }
}
impl From<Description> for Widget {
    fn from(value: Description) -> Self {
        value.child
    }
}
#[derive(Clone)]
pub struct Close {
    child: Widget,
}
impl Close {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}
impl From<Close> for Widget {
    fn from(value: Close) -> Self {
        value.child
    }
}
