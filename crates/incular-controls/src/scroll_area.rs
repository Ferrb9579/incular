//! Scroll-area anatomy backed by Incular's retained ScrollController.

use crate::Scrollbar;
use incular_scroll::ScrollController;
use incular_widgets::{SingleChildScrollView, Widget};
use typed_builder::TypedBuilder;

#[derive(Clone, TypedBuilder)]
pub struct Root {
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
    #[builder(default, setter(strip_option))]
    controller: Option<ScrollController>,
    #[builder(default = true)]
    show_scrollbar: bool,
}

impl Default for Root {
    fn default() -> Self {
        Self {
            child: None,
            controller: None,
            show_scrollbar: true,
        }
    }
}

impl Root {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn child(mut self, value: impl Into<Widget>) -> Self {
        self.child = Some(value.into());
        self
    }

    #[must_use]
    pub fn controller(mut self, value: ScrollController) -> Self {
        self.controller = Some(value);
        self
    }

    #[must_use]
    pub fn show_scrollbar(mut self, value: bool) -> Self {
        self.show_scrollbar = value;
        self
    }
}

impl From<Root> for Widget {
    fn from(value: Root) -> Self {
        let child = value
            .child
            .unwrap_or_else(|| incular_widgets::SizedBox::shrink().into());
        let controller = value.controller.unwrap_or_default();
        let viewport: Widget = SingleChildScrollView::new(child)
            .controller(controller.clone())
            .into();
        if value.show_scrollbar {
            Scrollbar::new(viewport).controller(controller).into()
        } else {
            viewport
        }
    }
}

#[derive(Clone, TypedBuilder)]
pub struct Viewport {
    #[builder(setter(into))]
    child: Widget,
}

impl Viewport {
    #[must_use]
    pub fn new(value: impl Into<Widget>) -> Self {
        Self::builder().child(value).build()
    }
}
impl From<Viewport> for Widget {
    fn from(value: Viewport) -> Self {
        value.child
    }
}

pub type ScrollbarPart = crate::Scrollbar;

#[derive(Clone, TypedBuilder)]
pub struct Thumb {
    #[builder(setter(into))]
    child: Widget,
}

impl Thumb {
    #[must_use]
    pub fn new(value: impl Into<Widget>) -> Self {
        Self::builder().child(value).build()
    }
}
impl From<Thumb> for Widget {
    fn from(value: Thumb) -> Self {
        value.child
    }
}

#[derive(Clone, TypedBuilder)]
pub struct Corner {
    #[builder(setter(into))]
    child: Widget,
}

impl Corner {
    #[must_use]
    pub fn new(value: impl Into<Widget>) -> Self {
        Self::builder().child(value).build()
    }
}
impl From<Corner> for Widget {
    fn from(value: Corner) -> Self {
        value.child
    }
}
