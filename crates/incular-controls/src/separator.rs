//! Semantic separator primitive. Unlike the legacy `Divider`, this component
//! does not bake in margins or a particular orientation.

use crate::ControlTheme;
use incular_widgets::{Container, Widget};
use std::rc::Rc;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Orientation {
    #[default]
    Horizontal,
    Vertical,
}

#[derive(Clone)]
pub struct Separator {
    orientation: Orientation,
    thickness: f32,
    decorative: bool,
}
impl Default for Separator {
    fn default() -> Self {
        Self::new()
    }
}
impl Separator {
    #[must_use]
    pub fn new() -> Self {
        Self {
            orientation: Orientation::Horizontal,
            thickness: 1.,
            decorative: false,
        }
    }
    #[must_use]
    pub fn orientation(mut self, value: Orientation) -> Self {
        self.orientation = value;
        self
    }
    #[must_use]
    pub fn vertical(self) -> Self {
        self.orientation(Orientation::Vertical)
    }
    #[must_use]
    pub fn horizontal(self) -> Self {
        self.orientation(Orientation::Horizontal)
    }
    #[must_use]
    pub fn thickness(mut self, value: f32) -> Self {
        self.thickness = value.max(0.);
        self
    }
    #[must_use]
    pub fn decorative(mut self, value: bool) -> Self {
        self.decorative = value;
        self
    }
    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        let mut widget = if self.orientation == Orientation::Horizontal {
            Container::new().height(self.thickness)
        } else {
            Container::new().width(self.thickness)
        };
        widget = widget.color(theme.colors.border);
        widget.into()
    }
}
impl From<Separator> for Widget {
    fn from(value: Separator) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| {
            let theme =
                incular_widgets::current_build_environment::<ControlTheme>().unwrap_or_default();
            value.build(&theme)
        })
    }
}
