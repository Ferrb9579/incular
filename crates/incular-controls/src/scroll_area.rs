//! Scroll-area anatomy backed by Incular's retained ScrollController.

use crate::Scrollbar;
use incular_scroll::ScrollController;
use incular_widgets::{SingleChildScrollView, Widget};

#[derive(Clone)]
pub struct Root {
    child: Option<Widget>,
    controller: Option<ScrollController>,
    show_scrollbar: bool,
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
            child: None,
            controller: None,
            show_scrollbar: true,
        }
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

#[derive(Clone)]
pub struct Viewport(Widget);
impl Viewport {
    #[must_use]
    pub fn new(value: impl Into<Widget>) -> Self {
        Self(value.into())
    }
}
impl From<Viewport> for Widget {
    fn from(value: Viewport) -> Self {
        value.0
    }
}

pub type ScrollbarPart = crate::Scrollbar;

#[derive(Clone)]
pub struct Thumb(Widget);
impl Thumb {
    #[must_use]
    pub fn new(value: impl Into<Widget>) -> Self {
        Self(value.into())
    }
}
impl From<Thumb> for Widget {
    fn from(value: Thumb) -> Self {
        value.0
    }
}

#[derive(Clone)]
pub struct Corner(Widget);
impl Corner {
    #[must_use]
    pub fn new(value: impl Into<Widget>) -> Self {
        Self(value.into())
    }
}
impl From<Corner> for Widget {
    fn from(value: Corner) -> Self {
        value.0
    }
}
