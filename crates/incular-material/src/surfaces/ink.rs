//! Material ink backgrounds and retained ripple/action surfaces.

use incular_core::Color;
use incular_widgets::internal::ActionSurface;
use incular_widgets::{BoxDecoration, Widget};
use std::fmt;
use std::rc::Rc;
use typed_builder::TypedBuilder;

/// Material ink background wrapper. The actual ink is painted by the shared
/// retained decoration/action primitives.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct Ink {
    #[builder(default, setter(strip_option))]
    pub color: Option<Color>,
    #[builder(setter(into))]
    pub child: Widget,
}

impl Ink {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            color: None,
            child: child.into(),
        }
    }
    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }
}

impl From<Ink> for Widget {
    fn from(value: Ink) -> Self {
        let mut decoration = BoxDecoration::new();
        if let Some(color) = value.color {
            decoration = decoration.color(color);
        }
        incular_widgets::DecoratedBox::new(value.child)
            .decoration(decoration)
            .into()
    }
}

/// Retained pointer/keyboard ink interaction surface.
#[derive(Clone, TypedBuilder)]
pub struct InkWell {
    #[builder(setter(into))]
    pub child: Widget,
    #[builder(
        default,
        setter(
            fn transform<F>(callback: F) -> Option<Rc<dyn Fn() + 'static>>
            where
                F: Fn() + 'static,
            {
                Some(Rc::new(callback))
            }
        )
    )]
    on_tap: Option<Rc<dyn Fn() + 'static>>,
    #[builder(default = Color::TRANSPARENT)]
    pub color: Color,
    #[builder(default = Color::TRANSPARENT)]
    pub overlay_color: Color,
}

impl fmt::Debug for InkWell {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InkWell")
            .field("color", &self.color)
            .field("overlay_color", &self.overlay_color)
            .finish()
    }
}

impl PartialEq for InkWell {
    fn eq(&self, other: &Self) -> bool {
        self.child == other.child
            && self.color == other.color
            && self.overlay_color == other.overlay_color
    }
}

impl InkWell {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            on_tap: None,
            color: Color::TRANSPARENT,
            overlay_color: Color::TRANSPARENT,
        }
    }
    #[must_use]
    pub fn on_tap(mut self, callback: impl Fn() + 'static) -> Self {
        self.on_tap = Some(std::rc::Rc::new(callback));
        self
    }
    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }
    #[must_use]
    pub fn overlay_color(mut self, color: Color) -> Self {
        self.overlay_color = color;
        self
    }
}

impl From<InkWell> for Widget {
    fn from(value: InkWell) -> Self {
        let mut surface = ActionSurface::with_child(value.child)
            .color(value.color)
            .hover_color(value.overlay_color)
            .pressed_color(value.overlay_color);
        if let Some(callback) = value.on_tap {
            surface = surface.on_click(move || callback());
        }
        surface.into()
    }
}

/// `InkResponse` shares the retained implementation with `InkWell`; the
/// distinction in Flutter is about clipping/gesture details, not a second
/// state system.
pub type InkResponse = InkWell;
