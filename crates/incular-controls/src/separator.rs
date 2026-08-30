//! Semantic separator primitive. Unlike the legacy `Divider`, this component
//! does not bake in margins or a particular orientation.

use crate::ControlTheme;
use incular_widgets::{Container, Widget};
use std::rc::Rc;
use typed_builder::TypedBuilder;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Orientation {
    #[default]
    Horizontal,
    Vertical,
}

#[derive(Clone, TypedBuilder)]
pub struct Separator {
    #[builder(default = Orientation::Horizontal)]
    orientation: Orientation,
    #[builder(default = 1., setter(transform = |thickness: f32| thickness.max(0.)))]
    thickness: f32,
    #[builder(default)]
    decorative: bool,
}
impl Default for Separator {
    fn default() -> Self {
        Self {
            orientation: Orientation::Horizontal,
            thickness: 1.,
            decorative: false,
        }
    }
}
impl Separator {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
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
            let theme = incular_widgets::internal::current_build_environment::<ControlTheme>()
                .unwrap_or_default();
            value.build(&theme)
        })
    }
}
