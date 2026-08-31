//! Padding and alignment layout primitives.

use incular_config::{Alignment, EdgeInsets};
use typed_builder::TypedBuilder;

use crate::{Widget, WidgetKind};

/// Insets its child by given [`EdgeInsets`].
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct Padding {
    #[builder(setter(into))]
    padding: EdgeInsets,
    #[builder(setter(into))]
    child: Widget,
}

impl Padding {
    /// Creates a padding widget wrapping a child with the given insets.
    #[must_use]
    pub fn new(padding: impl Into<EdgeInsets>, child: impl Into<Widget>) -> Self {
        Self {
            padding: padding.into(),
            child: child.into(),
        }
    }

    /// Convenience constructor with uniform padding on all edges.
    #[must_use]
    pub fn all(value: f32, child: impl Into<Widget>) -> Self {
        Self::new(EdgeInsets::all(value), child)
    }

    /// Convenience constructor with symmetric padding.
    #[must_use]
    pub fn symmetric(horizontal: f32, vertical: f32, child: impl Into<Widget>) -> Self {
        Self::new(EdgeInsets::symmetric(horizontal, vertical), child)
    }

    /// Convenience constructor with zero padding.
    #[must_use]
    pub fn zero(child: impl Into<Widget>) -> Self {
        Self::new(EdgeInsets::ZERO, child)
    }
}

impl From<Padding> for Widget {
    fn from(value: Padding) -> Self {
        Widget::from_kind(WidgetKind::Padding {
            padding: value.padding,
            child: std::rc::Rc::new(value.child),
        })
    }
}

/// Positions a child inside itself with fractional alignment and optional size factors.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct Align {
    alignment: Alignment,
    #[builder(
        default,
        setter(transform = |factor: f32| Some(factor.max(0.0)))
    )]
    width_factor: Option<f32>,
    #[builder(
        default,
        setter(transform = |factor: f32| Some(factor.max(0.0)))
    )]
    height_factor: Option<f32>,
    #[builder(setter(into))]
    child: Widget,
}

impl Align {
    /// Creates an align widget with explicit alignment and a child.
    #[must_use]
    pub fn new(alignment: Alignment, child: impl Into<Widget>) -> Self {
        Self {
            alignment,
            width_factor: None,
            height_factor: None,
            child: child.into(),
        }
    }

    /// Sets the width scaling factor relative to child width.
    #[must_use]
    pub fn width_factor(mut self, factor: f32) -> Self {
        self.width_factor = Some(factor.max(0.0));
        self
    }

    /// Sets the height scaling factor relative to child height.
    #[must_use]
    pub fn height_factor(mut self, factor: f32) -> Self {
        self.height_factor = Some(factor.max(0.0));
        self
    }

    #[must_use]
    pub fn get_alignment(&self) -> Alignment {
        self.alignment
    }
}

impl From<Align> for Widget {
    fn from(value: Align) -> Self {
        Widget::from_kind(WidgetKind::Align {
            alignment: value.alignment,
            width_factor: value.width_factor,
            height_factor: value.height_factor,
            child: std::rc::Rc::new(value.child),
        })
    }
}

/// Centers its child within itself.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct Center {
    #[builder(
        default,
        setter(transform = |factor: f32| Some(factor.max(0.0)))
    )]
    width_factor: Option<f32>,
    #[builder(
        default,
        setter(transform = |factor: f32| Some(factor.max(0.0)))
    )]
    height_factor: Option<f32>,
    #[builder(setter(into))]
    child: Widget,
}

impl Center {
    /// Creates a Center widget with a child.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            width_factor: None,
            height_factor: None,
            child: child.into(),
        }
    }

    /// Sets the width scaling factor relative to child width.
    #[must_use]
    pub fn width_factor(mut self, factor: f32) -> Self {
        self.width_factor = Some(factor.max(0.0));
        self
    }

    /// Sets the height scaling factor relative to child height.
    #[must_use]
    pub fn height_factor(mut self, factor: f32) -> Self {
        self.height_factor = Some(factor.max(0.0));
        self
    }
}

impl From<Center> for Widget {
    fn from(value: Center) -> Self {
        let mut align = Align::new(Alignment::CENTER, value.child);
        if let Some(wf) = value.width_factor {
            align = align.width_factor(wf);
        }
        if let Some(hf) = value.height_factor {
            align = align.height_factor(hf);
        }
        align.into()
    }
}
