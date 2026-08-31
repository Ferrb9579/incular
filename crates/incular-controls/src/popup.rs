//! Popover-style compound parts sharing the overlay foundation.

use crate::overlay::{Align, AnchoredPositioner, OverlayPortal, Side};
use incular_core::Offset;
use incular_semantics::{Role as SemanticRole, SemanticState};
use incular_widgets::Widget;
use incular_widgets::internal::ExplicitSemantics;
use std::rc::Rc;
use typed_builder::TypedBuilder;

#[derive(Clone, Default, TypedBuilder)]
pub struct Root {
    #[builder(default = false)]
    open: bool,
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
    #[builder(default, setter(skip))]
    on_change: Option<Rc<dyn Fn(bool) + 'static>>,
}
impl Root {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
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
        Widget::from(incular_widgets::Visibility::new(child).visible(value.open))
    }
}

#[derive(Clone, TypedBuilder)]
pub struct Trigger {
    #[builder(setter(into))]
    child: Widget,
}
impl Trigger {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self::builder().child(child).build()
    }
}
impl From<Trigger> for Widget {
    fn from(value: Trigger) -> Self {
        value.child
    }
}
#[derive(Clone, TypedBuilder)]
pub struct Portal {
    #[builder(setter(into))]
    child: Widget,
    #[builder(default, setter(strip_option, into))]
    overlay: Option<Widget>,
    #[builder(default = false)]
    open: bool,
}
impl Portal {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self::builder().child(child).build()
    }
    #[must_use]
    pub fn overlay(mut self, value: impl Into<Widget>) -> Self {
        self.overlay = Some(value.into());
        self
    }
    #[must_use]
    pub fn open(mut self, value: bool) -> Self {
        self.open = value;
        self
    }
}
impl From<Portal> for Widget {
    fn from(value: Portal) -> Self {
        let mut portal = OverlayPortal::new(value.child).open(value.open);
        if let Some(overlay) = value.overlay {
            portal = portal.overlay(overlay);
        }
        portal.into()
    }
}
#[derive(Clone, TypedBuilder)]
pub struct Positioner {
    #[builder(setter(into))]
    child: Widget,
    #[builder(default = Side::Bottom)]
    side: Side,
    #[builder(default = Align::Center)]
    align: Align,
    #[builder(default = 4.)]
    side_offset: f32,
    #[builder(default = 0.)]
    align_offset: f32,
    #[builder(default = Offset::ZERO)]
    anchor: Offset,
}
impl Positioner {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self::builder().child(child).build()
    }
    #[must_use]
    pub fn side(mut self, value: Side) -> Self {
        self.side = value;
        self
    }
    #[must_use]
    pub fn align(mut self, value: Align) -> Self {
        self.align = value;
        self
    }

    #[must_use]
    pub fn side_offset(mut self, value: f32) -> Self {
        self.side_offset = value;
        self
    }

    #[must_use]
    pub fn align_offset(mut self, value: f32) -> Self {
        self.align_offset = value;
        self
    }

    #[must_use]
    pub fn anchor(mut self, value: Offset) -> Self {
        self.anchor = value;
        self
    }
}
impl From<Positioner> for Widget {
    fn from(value: Positioner) -> Self {
        AnchoredPositioner::new(value.child)
            .side(value.side)
            .align(value.align)
            .side_offset(value.side_offset)
            .align_offset(value.align_offset)
            .anchor(value.anchor)
            .into()
    }
}
#[derive(Clone, TypedBuilder)]
pub struct Popup {
    #[builder(setter(into))]
    child: Widget,
}
impl Popup {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self::builder().child(child).build()
    }
}
impl From<Popup> for Widget {
    fn from(value: Popup) -> Self {
        value.child
    }
}
#[derive(Clone, TypedBuilder)]
pub struct Arrow {
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
}
impl Arrow {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn child(mut self, value: impl Into<Widget>) -> Self {
        self.child = Some(value.into());
        self
    }
}
impl Default for Arrow {
    fn default() -> Self {
        Self::builder().build()
    }
}
impl From<Arrow> for Widget {
    fn from(value: Arrow) -> Self {
        value
            .child
            .unwrap_or_else(|| incular_widgets::SizedBox::shrink().into())
    }
}
#[derive(Clone, TypedBuilder)]
pub struct Title {
    #[builder(setter(into))]
    child: Widget,
}
impl Title {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self::builder()
            .child(incular_widgets::Text::new(value))
            .build()
    }
    #[must_use]
    pub fn with_child(child: impl Into<Widget>) -> Self {
        Self::builder().child(child).build()
    }
}
impl From<Title> for Widget {
    fn from(value: Title) -> Self {
        value.child
    }
}
#[derive(Clone, TypedBuilder)]
pub struct Description {
    #[builder(setter(into))]
    child: Widget,
}
impl Description {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self::builder()
            .child(incular_widgets::Text::new(value))
            .build()
    }
    #[must_use]
    pub fn with_child(child: impl Into<Widget>) -> Self {
        Self::builder().child(child).build()
    }
}
impl From<Description> for Widget {
    fn from(value: Description) -> Self {
        value.child
    }
}
#[derive(Clone, TypedBuilder)]
pub struct Close {
    #[builder(setter(into))]
    child: Widget,
}
impl Close {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self::builder().child(child).build()
    }
    #[must_use]
    pub fn with_child(child: impl Into<Widget>) -> Self {
        Self::builder().child(child).build()
    }
}
impl From<Close> for Widget {
    fn from(value: Close) -> Self {
        value.child
    }
}
